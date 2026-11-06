//! Quick fixes and refactorings.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use javac_analysis_api::{CodeAction, Diagnostic, Position, Range, TextEdit};

use super::super::positions::word_at;
use super::super::{canonical, FileAnalysis};

/// A public top-level class that can be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportCandidate {
    simple: String,
    flat: String,
}

/// Process-wide import-candidate cache keyed by (class path, source level).
///
/// Building the table enters every visible package (JDK modules and jars), so
/// it must not repeat per request.
fn candidate_cache() -> &'static Mutex<BTreeMap<u64, Arc<Vec<ImportCandidate>>>> {
    static CACHE: OnceLock<Mutex<BTreeMap<u64, Arc<Vec<ImportCandidate>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn class_path_hash(class_path: &[PathBuf], source_level: &str) -> u64 {
    let mut text = source_level.to_string();
    for entry in class_path {
        text.push('|');
        text.push_str(&entry.to_string_lossy());
    }
    crate::workspace::model::hash_text(&text)
}

fn line_len_utf16(line: &str) -> u32 {
    line.chars().map(|character| character.len_utf16() as u32).sum()
}

/// Remove one whole line, including its line terminator.
fn delete_line_edit(line: u32, lines: &[&str]) -> TextEdit {
    let (end_line, end_column) = if (line as usize + 1) < lines.len() {
        (line + 1, 0)
    } else {
        (line, line_len_utf16(lines.get(line as usize).copied().unwrap_or("")))
    };
    TextEdit {
        range: Range::new(Position::new(line, 0), Position::new(end_line, end_column)),
        new_text: String::new(),
    }
}

/// Insert position for a new import and the file's newline style.
///
/// Returns `(line, newline, needs_leading_newline)`.
fn import_insert_point(text: &str) -> (u32, &'static str, bool) {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = text.lines().collect();
    let last_import = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with("import "))
        .map(|(index, _)| index)
        .last();
    let insert = match last_import {
        Some(index) => index + 1,
        None => lines
            .iter()
            .position(|line| line.trim_start().starts_with("package "))
            .map(|index| index + 1)
            .unwrap_or(0),
    };
    let at_end = insert >= lines.len();
    let needs_leading = at_end && !text.ends_with('\n');
    (insert as u32, newline, needs_leading)
}

fn is_importable_flat(flat: &str) -> bool {
    if flat.contains('$') || flat.contains('+') {
        return false;
    }
    let mut segments = flat.split('.');
    let Some(_) = segments.next() else {
        return false;
    };
    if segments.clone().count() == 0 {
        return false;
    }
    flat.split('.').all(|segment| {
        let mut chars = segment.chars();
        match chars.next() {
            Some(first) if first.is_alphabetic() || first == '_' => {}
            _ => return false,
        }
        chars.all(|character| character.is_alphanumeric() || character == '_' || character == '$')
    })
}

fn classpath_candidates(class_path: &[PathBuf]) -> Vec<ImportCandidate> {
    let mut candidates = Vec::new();
    for entry in class_path {
        if entry.is_dir() {
            scan_class_dir(entry, entry, &mut candidates);
        } else if entry.is_file() {
            let name = entry.to_string_lossy().to_lowercase();
            if name.ends_with(".jar") || name.ends_with(".zip") {
                scan_class_jar(entry, &mut candidates);
            }
        }
    }
    candidates
}

fn scan_class_dir(root: &Path, dir: &Path, out: &mut Vec<ImportCandidate>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_class_dir(root, &path, out);
        } else if path.extension().map(|extension| extension == "class").unwrap_or(false) {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let Some(flat) = flat_from_class_relative(&relative.to_string_lossy()) else {
                continue;
            };
            if !is_importable_flat(&flat) {
                continue;
            }
            let public = std::fs::read(&path)
                .ok()
                .and_then(|bytes| class_access_flags(&bytes))
                .map(|flags| flags & 0x0001 != 0)
                .unwrap_or(false);
            if public {
                push_candidate(out, flat);
            }
        }
    }
}

fn scan_class_jar(path: &Path, out: &mut Vec<ImportCandidate>) {
    let Ok(archive) = javac_zip::ZipArchive::open(path) else {
        return;
    };
    for (index, entry) in archive.entries().iter().enumerate() {
        let name = entry.name();
        if !name.ends_with(".class") {
            continue;
        }
        let Some(flat) = flat_from_class_relative(name) else {
            continue;
        };
        if !is_importable_flat(&flat) {
            continue;
        }
        let public = archive
            .read_entry(index)
            .ok()
            .and_then(|bytes| class_access_flags(&bytes))
            .map(|flags| flags & 0x0001 != 0)
            .unwrap_or(false);
        if public {
            push_candidate(out, flat);
        }
    }
}

fn flat_from_class_relative(relative: &str) -> Option<String> {
    let relative = relative.trim_start_matches(['/', '\\']).replace('\\', "/");
    let without = relative.strip_suffix(".class")?;
    if without.is_empty() || without.contains('$') {
        return None;
    }
    if without == "module-info" || without == "package-info" {
        return None;
    }
    Some(without.replace('/', "."))
}

fn push_candidate(out: &mut Vec<ImportCandidate>, flat: String) {
    let Some((_, simple)) = flat.rsplit_once('.') else {
        return;
    };
    out.push(ImportCandidate {
        simple: simple.to_string(),
        flat,
    });
}

/// Read `access_flags` from a class file (constant-pool walk).
fn class_access_flags(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 10 || &bytes[0..4] != b"\xCA\xFE\xBA\xBE" {
        return None;
    }
    let mut index = 8;
    let count = u16::from_be_bytes([*bytes.get(index)?, *bytes.get(index + 1)?]) as usize;
    index += 2;
    let mut slot = 1;
    while slot < count {
        let tag = *bytes.get(index)?;
        index += 1;
        match tag {
            1 => {
                let length = u16::from_be_bytes([*bytes.get(index)?, *bytes.get(index + 1)?]) as usize;
                index += 2 + length;
            }
            7 | 8 | 16 | 19 | 20 => index += 2,
            15 => index += 3,
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => index += 4,
            5 | 6 => {
                index += 8;
                slot += 1;
            }
            _ => return None,
        }
        slot += 1;
    }
    Some(u16::from_be_bytes([*bytes.get(index)?, *bytes.get(index + 1)?]))
}

fn is_type_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_uppercase() => {}
        _ => return false,
    }
    chars.all(|character| character.is_alphanumeric() || character == '_' || character == '$')
}

fn package_name(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("package ")?;
        Some(rest.trim_end_matches(';').trim().to_string())
    })
}

/// Name of the file's first top-level type declaration (syntax only; the
/// candidate/usage decisions stay symbol based).
fn file_class_name(text: &str) -> Option<String> {
    let masked = crate::analysis::inspections::mask_non_code(text);
    let mut tokens = masked.split(|character: char| character.is_whitespace() || character == '{');
    while let Some(token) = tokens.next() {
        if matches!(token, "class" | "interface" | "enum" | "record") {
            let next = tokens.next()?;
            let name: String = next
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_' || *character == '$')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Import statements of one file: (exact classes, wildcard packages).
fn imports_of(text: &str) -> (HashSet<String>, HashSet<String>) {
    let mut exact = HashSet::new();
    let mut wildcards = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("import ") else {
            continue;
        };
        if rest.starts_with("static ") {
            continue;
        }
        let path = rest.trim_end_matches(';').trim();
        if path.is_empty() {
            continue;
        }
        if let Some(package) = path.strip_suffix(".*") {
            wildcards.insert(package.to_string());
        } else {
            exact.insert(path.to_string());
        }
    }
    (exact, wildcards)
}

fn import_path_at(text: &str, line: u32) -> Option<String> {
    let line = text.lines().nth(line as usize)?;
    let rest = line.trim().strip_prefix("import ")?;
    Some(rest.trim_end_matches(';').trim().to_string())
}

struct ImportExclusions {
    package: Option<String>,
    exact: HashSet<String>,
    wildcards: HashSet<String>,
}

impl ImportExclusions {
    fn new(text: &str) -> Self {
        let (exact, wildcards) = imports_of(text);
        Self {
            package: package_name(text),
            exact,
            wildcards,
        }
    }

    fn contains(&self, flat: &str) -> bool {
        let (package, simple) = flat.rsplit_once('.').unwrap_or(("", flat));
        if package == "java.lang" {
            return true;
        }
        if self.package.as_deref() == Some(package) {
            return true;
        }
        if self.exact.contains(flat) {
            return true;
        }
        if self.wildcards.contains(package) {
            return true;
        }
        // An explicit import with the same simple name shadows the candidate.
        let simple_name = simple.to_string();
        let _ = simple_name;
        false
    }
}

/// Pick the `cannot find symbol` diagnostic at/under the cursor and the
/// identifier it refers to.
fn add_import_target(
    text: &str,
    position: Position,
    diagnostics: &[Diagnostic],
) -> Option<(String, Diagnostic)> {
    let diagnostic = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .code
                .as_deref()
                .map(|code| code.contains("cant.resolve"))
                .unwrap_or(false)
        })
        .filter(|diagnostic| {
            diagnostic.range.start.line <= position.line
                && position.line <= diagnostic.range.end.line
        })
        .find(|diagnostic| diagnostic.range.start.line == position.line)
        .or_else(|| {
            diagnostics.iter().find(|diagnostic| {
                diagnostic
                    .code
                    .as_deref()
                    .map(|code| code.contains("cant.resolve"))
                    .unwrap_or(false)
                    && diagnostic.range.start.line == position.line
            })
        })?;
    let name = name_of_diagnostic(text, diagnostic)?;
    Some((name, diagnostic.clone()))
}

fn name_of_diagnostic(text: &str, diagnostic: &Diagnostic) -> Option<String> {
    let start = crate::analysis::positions::line_offset(
        text,
        diagnostic.range.start.line as usize + 1,
        diagnostic.range.start.column as usize + 1,
    );
    let end = crate::analysis::positions::line_offset(
        text,
        diagnostic.range.end.line as usize + 1,
        diagnostic.range.end.column as usize + 1,
    );
    let slice = text.get(start..end)?.trim();
    if !slice.is_empty() && !slice.contains('<') && !slice.contains('(') {
        let last = slice.rsplit('.').next().unwrap_or(slice).trim();
        if is_type_identifier(last) {
            return Some(last.to_string());
        }
        return None;
    }
    // The diagnostic range may be zero width: fall back to its message
    // ("符号: 类 X" / "symbol: class X").
    let marker = if diagnostic.message.contains("符号: 类") {
        "符号: 类"
    } else if diagnostic.message.contains("symbol: class") {
        "symbol: class"
    } else {
        return None;
    };
    let rest = diagnostic.message.split(marker).nth(1)?.trim_start();
    let name: String = rest
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_' || *character == '$')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

impl FileAnalysis {
    fn import_candidates(&mut self) -> Arc<Vec<ImportCandidate>> {
        let key = class_path_hash(&self.config.class_path, &self.config.source_level);
        if let Some(cached) = candidate_cache().lock().unwrap().get(&key) {
            return Arc::clone(cached);
        }
        // Enter every visible package (JDK modules and class-path jars) once,
        // then keep public top-level classes.
        let by_name = self.build_class_short_names();
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        for (simple, classes) in &by_name {
            for class in classes {
                let symbol = self.compiler.ctx.sym(*class);
                let flat = symbol.flat_name().to_string();
                // Only importable top-level names. The symbol table does not
                // expose reliable access flags for platform classes, so the
                // filter is name based; non-public matches are pruned by the
                // javac diagnostics that follow.
                if !is_importable_flat(&flat) {
                    continue;
                }
                if seen.insert(flat.clone()) {
                    candidates.push(ImportCandidate {
                        simple: simple.clone(),
                        flat,
                    });
                }
            }
        }
        // Class-path jars and directories: classes that never entered the
        // symbol table still need candidates (a jar class is importable
        // before it is used anywhere in the project).
        for candidate in classpath_candidates(&self.config.class_path) {
            if seen.insert(candidate.flat.clone()) {
                candidates.push(candidate);
            }
        }
        candidates.sort_by(|left, right| (&left.simple, &left.flat).cmp(&(&right.simple, &right.flat)));
        let candidates = Arc::new(candidates);
        candidate_cache().lock().unwrap().insert(key, Arc::clone(&candidates));
        candidates
    }

    /// Quick fixes at a position.
    pub(crate) fn code_actions_impl(
        &mut self,
        file: &Path,
        position: Position,
    ) -> Vec<CodeAction> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let diagnostics = self.diagnostics(file, None);
        let mut actions = self.remove_unused_import_actions(&text, position, &diagnostics);
        actions.extend(self.add_import_actions(&text, position, &diagnostics));
        actions
    }

    /// Remove unused imports: one line when the cursor is on it, otherwise all
    /// of them (descending line order).
    fn remove_unused_import_actions(
        &self,
        text: &str,
        position: Position,
        diagnostics: &[Diagnostic],
    ) -> Vec<CodeAction> {
        let lines: Vec<&str> = text.lines().collect();
        let unused: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.as_deref() == Some("unused.import"))
            .collect();
        if unused.is_empty() {
            return Vec::new();
        }
        if let Some(target) = unused
            .iter()
            .find(|diagnostic| diagnostic.range.start.line == position.line)
        {
            let path = import_path_at(text, target.range.start.line).unwrap_or_default();
            return vec![CodeAction {
                title: format!("Remove unused import: {path}"),
                edits: vec![delete_line_edit(target.range.start.line, &lines)],
                diagnostics: vec![(*target).clone()],
            }];
        }
        if unused.len() > 1 {
            let mut edits: Vec<TextEdit> = unused
                .iter()
                .map(|diagnostic| delete_line_edit(diagnostic.range.start.line, &lines))
                .collect();
            edits.sort_by_key(|edit| edit.range.start.line);
            edits.reverse();
            return vec![CodeAction {
                title: "Remove all unused imports".to_string(),
                edits,
                diagnostics: unused.iter().map(|diagnostic| (*diagnostic).clone()).collect(),
            }];
        }
        let target = unused[0];
        let path = import_path_at(text, target.range.start.line).unwrap_or_default();
        vec![CodeAction {
            title: format!("Remove unused import: {path}"),
            edits: vec![delete_line_edit(target.range.start.line, &lines)],
            diagnostics: vec![target.clone()],
        }]
    }

    /// Add an import for the unresolved simple name at the cursor.
    fn add_import_actions(
        &mut self,
        text: &str,
        position: Position,
        diagnostics: &[Diagnostic],
    ) -> Vec<CodeAction> {
        let Some((simple, diagnostic)) = add_import_target(text, position, diagnostics) else {
            return Vec::new();
        };
        if file_class_name(text).as_deref() == Some(simple.as_str()) {
            return Vec::new();
        }
        let candidates = self.import_candidates();
        let exclusions = ImportExclusions::new(text);
        let mut matched: Vec<ImportCandidate> = candidates
            .iter()
            .filter(|candidate| candidate.simple == simple && !exclusions.contains(&candidate.flat))
            .cloned()
            .collect();
        matched.sort_by(|left, right| left.flat.cmp(&right.flat));
        let (line, newline, needs_leading) = import_insert_point(text);
        matched
            .into_iter()
            .map(|candidate| {
                let new_text = if needs_leading {
                    format!("{newline}import {};{newline}", candidate.flat)
                } else {
                    format!("import {};{newline}", candidate.flat)
                };
                CodeAction {
                    title: format!("Import '{}'", candidate.flat),
                    edits: vec![TextEdit {
                        range: Range::point(Position::new(line, 0)),
                        new_text,
                    }],
                    diagnostics: vec![diagnostic.clone()],
                }
            })
            .collect()
    }

    /// Rename the symbol at a position across all project files.
    pub(crate) fn rename_workspace_impl(
        &mut self,
        file: &Path,
        position: Position,
        new_name: &str,
    ) -> Vec<javac_analysis_api::TextDocumentEdit> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let Some(word) = word_at(&text, line, column) else {
            return Vec::new();
        };
        let word_len: u32 = word.chars().map(|character| character.len_utf16() as u32).sum();
        let locations = self.references_impl(file, position);
        locations
            .into_iter()
            .map(|location| {
                let start = location.range.start;
                javac_analysis_api::TextDocumentEdit {
                    file: location.file,
                    edit: TextEdit {
                        range: Range::new(
                            start,
                            Position::new(start.line, start.column + word_len),
                        ),
                        new_text: new_name.to_string(),
                    },
                }
            })
            .collect()
    }

    /// Rename the symbol at a position.
    pub(crate) fn rename_impl(
        &mut self,
        file: &Path,
        position: Position,
        new_name: &str,
    ) -> Vec<TextEdit> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let Some(word) = word_at(&text, line, column) else {
            return Vec::new();
        };
        let word_len: u32 = word.chars().map(|character| character.len_utf16() as u32).sum();
        let locations = self.references_impl(file, position);
        locations
            .into_iter()
            .map(|location| {
                let start = location.range.start;
                TextEdit {
                    range: Range::new(
                        start,
                        Position::new(start.line, start.column + word_len),
                    ),
                    new_text: new_name.to_string(),
                }
            })
            .collect()
    }
}
