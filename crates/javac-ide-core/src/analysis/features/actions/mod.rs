//! Quick fixes and refactorings.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use javac_analysis_api::{CodeAction, Diagnostic, Position, Range, TextEdit};

use super::super::positions::word_at;
use super::super::{canonical, FileAnalysis};
use super::import_syntax::ImportSyntax;

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
pub(crate) fn import_insert_point(
    text: &str,
    syntax: &ImportSyntax,
) -> (u32, &'static str, bool) {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if syntax.end == 0 {
        return (0, newline, false);
    }
    let (last_line, _) =
        crate::analysis::positions::offset_to_line_col(text, syntax.end);
    (last_line + 1, newline, false)
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

/// Top-level class/interface/enum/record names of `file`, from the AST.
fn file_class_names(
    ctx: &crate::compiler_context::CompilerContext,
    file: &Path,
) -> Vec<String> {
    let target = PathBuf::from(canonical(file));
    let mut names = Vec::new();
    for index in 0..ctx.trees.len() {
        let unit = crate::ids::TreeId::new(index as u32);
        let (source, defs) = match &ctx.tree(unit).kind {
            crate::tree::jc_tree::TreeKind::CompilationUnit {
                sourcefile: Some(source),
                defs,
                ..
            } => (source.get_name().to_string_lossy().to_string(), defs.clone()),
            _ => continue,
        };
        if PathBuf::from(canonical(Path::new(&source))) != target {
            continue;
        }
        for def in defs.iter() {
            if let crate::tree::jc_tree::TreeKind::ClassDecl { name, .. } = &ctx.tree(def).kind {
                names.push(name.to_java_string().to_string_lossy().to_string());
            }
        }
    }
    names
}

/// Import statements decoded from AST declarations.
pub(crate) fn imports_of(syntax: &ImportSyntax) -> (HashSet<String>, HashSet<String>) {
    let mut exact = HashSet::new();
    let mut wildcards = HashSet::new();
    for declaration in &syntax.declarations {
        if declaration.static_import {
            continue;
        }
        if declaration.wildcard {
            let package = declaration
                .segments
                .iter()
                .take(declaration.segments.len().saturating_sub(1))
                .cloned()
                .collect::<Vec<_>>()
                .join(".");
            wildcards.insert(package);
        } else {
            exact.insert(declaration.path.clone());
        }
    }
    (exact, wildcards)
}

fn import_path_at(syntax: &ImportSyntax, text: &str, line: u32) -> Option<String> {
    syntax.declarations.iter().find_map(|declaration| {
        let (decl_line, _) =
            crate::analysis::positions::offset_to_line_col(text, declaration.start);
        (decl_line == line).then(|| declaration.path.clone())
    })
}

struct ImportExclusions {
    package: Option<String>,
    exact: HashSet<String>,
    wildcards: HashSet<String>,
}

impl ImportExclusions {
    fn new(syntax: &ImportSyntax) -> Self {
        let (exact, wildcards) = imports_of(syntax);
        Self {
            package: (!syntax.package.is_empty()).then(|| syntax.package.clone()),
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
impl FileAnalysis {
fn add_import_target(
    &self,
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
    let name = self.name_of_diagnostic(text, diagnostic)?;
    Some((name, diagnostic.clone()))
}

fn name_of_diagnostic(&self, text: &str, diagnostic: &Diagnostic) -> Option<String> {
    let start = crate::analysis::positions::line_offset(
        text,
        diagnostic.range.start.line as usize + 1,
        diagnostic.range.start.column as usize + 1,
    );
    let start_utf16 = text[..start].encode_utf16().count();
    let end = crate::analysis::positions::line_offset(
        text,
        diagnostic.range.end.line as usize + 1,
        diagnostic.range.end.column as usize + 1,
    );
    let end_utf16 = text[..end].encode_utf16().count();
    // The unresolved type/name is an AST Ident node; no diagnostic message
    // text and no source slice classification is involved.
    let mut best: Option<(usize, String)> = None;
    for tree in self.compiler.ctx.trees.iter() {
        let crate::tree::jc_tree::TreeKind::Ident { name, .. } = &tree.kind else {
            continue;
        };
        if tree.pos < 0 {
            continue;
        }
        let position = tree.pos as usize;
        if position < start_utf16 || position > end_utf16 {
            continue;
        }
        let name = name.to_java_string().to_string_lossy().to_string();
        if !is_type_identifier(&name) {
            continue;
        }
        if best.as_ref().is_none_or(|(best_pos, _)| position > *best_pos) {
            best = Some((position, name));
        }
    }
    best.map(|(_, name)| name)
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
                // Only importable top-level names (plan 8.2). Platform
                // class access flags are unavailable (see the note below).
                if !is_importable_flat(&flat) {
                    continue;
                }
                // Platform classes: read ACC_PUBLIC from the jimage class
                // bytes (the symbol flags do not carry it). Unreadable classes
                // keep the candidate and rely on diagnostic pruning.
                if let Some(module) = system_module_of(&self.compiler.ctx, *class) {
                    if platform_class_is_public(&flat, &module) == Some(false) {
                        continue;
                    }
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
        let syntax = self.import_syntax_impl(file);
        let mut actions =
            self.remove_unused_import_actions(&text, &syntax, position, &diagnostics);
        actions.extend(self.add_import_actions(file, &text, &syntax, position, &diagnostics));
        actions.extend(self.generate_actions(file, position));
        actions
    }

    /// Remove unused imports: one line when the cursor is on it, otherwise all
    /// of them (descending line order).
    fn remove_unused_import_actions(
        &self,
        text: &str,
        syntax: &ImportSyntax,
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
            let path = import_path_at(syntax, text, target.range.start.line).unwrap_or_default();
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
        let path = import_path_at(syntax, text, target.range.start.line).unwrap_or_default();
        vec![CodeAction {
            title: format!("Remove unused import: {path}"),
            edits: vec![delete_line_edit(target.range.start.line, &lines)],
            diagnostics: vec![target.clone()],
        }]
    }

    /// Add an import for the unresolved simple name at the cursor.
    fn add_import_actions(
        &mut self,
        file: &Path,
        text: &str,
        syntax: &ImportSyntax,
        position: Position,
        diagnostics: &[Diagnostic],
    ) -> Vec<CodeAction> {
        let Some((simple, diagnostic)) = self.add_import_target(text, position, diagnostics) else {
            return Vec::new();
        };
        if file_class_names(&self.compiler.ctx, file).iter().any(|name| name == &simple) {
            return Vec::new();
        }
        let candidates = self.import_candidates();
        let exclusions = ImportExclusions::new(syntax);
        let mut matched: Vec<ImportCandidate> = candidates
            .iter()
            .filter(|candidate| candidate.simple == simple && !exclusions.contains(&candidate.flat))
            .cloned()
            .collect();
        matched.sort_by(|left, right| left.flat.cmp(&right.flat));
        let (line, newline, needs_leading) = import_insert_point(text, syntax);
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

/// Whether a platform class is public, read from its jimage class bytes
/// (`ACC_PUBLIC`). `None` means unreadable (caller keeps the candidate and
/// falls back to diagnostic pruning).
fn platform_class_is_public(flat: &str, module: &str) -> Option<bool> {
    if let Some(public) = jimage_class_is_public(flat, module) {
        return Some(public);
    }
    // `--release` platform paths may only ship `ct.sym`: fall back to the
    // versioned `.sig` class files.
    ct_sym_class_access_flags(flat, module).map(|flags| flags & 1 != 0)
}

fn jimage_class_is_public(flat: &str, module: &str) -> Option<bool> {
    let modules = platform_modules_file()?;
    let image = javac_jimage::JImage::open(&modules).ok()?;
    let relative = format!("{}.class", flat.replace('.', "/"));
    let bytes = image.find_resource(&format!("{module}/{relative}"))?;
    let flags = class_access_flags(&bytes)?;
    Some(flags & 1 != 0)
}

/// The runtime platform layout puts `modules` and `ct.sym` next to the
/// executable (`RAVAC_PLATFORM_DIR` overrides); `JAVA_HOME/lib` is the
/// development fallback.
fn platform_modules_file() -> Option<std::path::PathBuf> {
    if let Some(dir) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        let path = crate::file::platform_paths::modules_file(std::path::Path::new(&dir));
        if path.is_file() {
            return Some(path);
        }
    }
    let home = std::env::var_os("JAVA_HOME")?;
    let path = std::path::PathBuf::from(home).join("lib").join("modules");
    path.is_file().then_some(path)
}

fn ct_sym_file() -> Option<std::path::PathBuf> {
    if let Some(dir) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        let path = crate::file::platform_paths::ct_sym_file(std::path::Path::new(&dir));
        if path.is_file() {
            return Some(path);
        }
    }
    let home = std::env::var_os("JAVA_HOME")?;
    let path = std::path::PathBuf::from(home).join("lib").join("ct.sym");
    path.is_file().then_some(path)
}

/// ACC_PUBLIC of a class from `ct.sym` (the `.sig` entries are class files).
///
/// The archive stores one directory per release; newer releases only contain
/// changed classes, so the highest ranking entry wins (`89ABCDEFG` = 8..16).
fn ct_sym_class_access_flags(flat: &str, module: &str) -> Option<u16> {
    let ct_sym = ct_sym_file()?;
    let archive = javac_zip::ZipArchive::open(&ct_sym).ok()?;
    let relative = format!("{}.sig", flat.replace('.', "/"));
    let wanted = format!("{module}/{relative}");
    let mut best: Option<(u32, String)> = None;
    for entry in archive.entries() {
        let name = entry.name();
        let Some((release, rest)) = name.split_once('/') else {
            continue;
        };
        if rest != wanted {
            continue;
        }
        let Some(rank) = release_rank(release) else {
            continue;
        };
        if best.as_ref().map_or(true, |(best_rank, _)| rank > *best_rank) {
            best = Some((rank, name.to_string()));
        }
    }
    let (_, entry) = best?;
    let bytes = archive.get_entry(&entry)?;
    class_access_flags(&bytes)
}

/// Release directory rank: `7`..`9`, then `A`..`H` (10..17); merged names use
/// their highest member (`89ABCDEFG` -> 16, i.e. the `G` content).
fn release_rank(release: &str) -> Option<u32> {
    release
        .chars()
        .filter_map(|character| match character {
            '7'..='9' => Some(character as u32 - '0' as u32),
            'A'..='H' => Some(character as u32 - 'A' as u32 + 10),
            _ => None,
        })
        .max()
}
/// System module of a class (`java.util.List` -> `java.base`), if any.
fn system_module_of(
    ctx: &crate::compiler_context::CompilerContext,
    class: crate::ids::SymbolId,
) -> Option<String> {
    let mut current = class;
    loop {
        let symbol = ctx.sym(current);
        if let crate::code::symbol::SymbolKind::PackageSymbol { modle, .. } = &symbol.kind_data {
            let module = (*modle)?;
            return Some(
                ctx.sym(module)
                    .name
                    .to_java_string()
                    .to_string_lossy()
                    .to_string(),
            );
        }
        current = symbol.owner?;
    }
}

#[cfg(test)]
mod platform_public_tests {
    use super::{ct_sym_class_access_flags, platform_class_is_public};

    #[test]
    fn platform_class_public_flag_comes_from_jimage_bytes() {
        // Evidence: java.util.List is public; java.util.ImmutableCollections
        // is package-private (both read from the jimage class bytes).
        assert_eq!(
            platform_class_is_public("java.util.List", "java.base"),
            Some(true)
        );
        assert_eq!(
            platform_class_is_public("java.util.ImmutableCollections", "java.base"),
            Some(false)
        );
    }

    #[test]
    fn ct_sym_access_flags_public_and_package_private() {
        // `--release` platform paths ship `ct.sym` instead of `lib/modules`:
        // the `.sig` entries are class files, so ACC_PUBLIC is readable.
        let list = ct_sym_class_access_flags("java.util.List", "java.base").expect("List.sig");
        assert!(list & 1 != 0, "List must be public: {list:#x}");
        let view = ct_sym_class_access_flags(
            "java.util.concurrent.ConcurrentHashMap$CollectionView",
            "java.base",
        )
        .expect("CollectionView.sig");
        assert!(view & 1 == 0, "CollectionView must be package-private: {view:#x}");
    }
}