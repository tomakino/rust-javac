//! Organize imports (section 13): dedupe, drop unused, group and sort.
//!
//! Unused detection reuses [`super::super::inspections::unused_imports`]
//! (masked-code identifier boundary).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, Range, TextEdit};

use super::super::{canonical, FileAnalysis};

struct ImportEntry {
    line: usize,
    is_static: bool,
    fqcn: String,
    comment: String,
}

impl FileAnalysis {
    pub(crate) fn organize_imports_impl(&mut self, file: &Path) -> Vec<TextEdit> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        organize_imports_text(&text).into_iter().collect()
    }
}

fn unused_imports(text: &str) -> HashSet<String> {
    super::super::inspections::unused_imports(text)
        .into_iter()
        .filter_map(|diagnostic| {
            diagnostic
                .message
                .strip_prefix("Unused import: ")
                .map(|path| path.to_string())
        })
        .collect()
}

fn organize_imports_text(text: &str) -> Option<TextEdit> {
    let lines: Vec<&str> = text.lines().collect();
    let mut entries = Vec::new();
    let mut first = None;
    let mut last = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("import ") else {
            continue;
        };
        let is_static = rest.starts_with("static ");
        let path = rest.strip_prefix("static ").unwrap_or(rest);
        let Some(semicolon) = path.find(';') else {
            continue;
        };
        let fqcn = path[..semicolon].trim().to_string();
        let absolute = line.find(';').map(|position| position + 1).unwrap_or(line.len());
        let comment = line[absolute..].trim().to_string();
        entries.push(ImportEntry {
            line: index,
            is_static,
            fqcn,
            comment,
        });
        first.get_or_insert(index);
        last = Some(index);
    }
    let (Some(first), Some(last)) = (first, last) else {
        return None;
    };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut comments = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if index > first && index < last {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                comments.push(trimmed.to_string());
            }
        }
    }
    let unused = unused_imports(text);
    let mut kept: Vec<ImportEntry> = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let wildcard = entry.fqcn.ends_with(".*");
        if !wildcard && unused.contains(&entry.fqcn) {
            continue;
        }
        if !seen.insert((entry.is_static, entry.fqcn.clone())) {
            continue;
        }
        kept.push(entry);
    }
    if kept.is_empty() {
        return None;
    }
    let group_of = |entry: &ImportEntry| -> &'static str {
        if entry.is_static {
            "static"
        } else if entry.fqcn.starts_with("java.") {
            "java"
        } else if entry.fqcn.starts_with("javax.") {
            "javax"
        } else {
            "third-party"
        }
    };
    let order = ["java", "javax", "third-party", "static"];
    let mut body = String::new();
    for group in order {
        let mut items: Vec<&ImportEntry> = kept
            .iter()
            .filter(|entry| group_of(entry) == group)
            .collect();
        if items.is_empty() {
            continue;
        }
        items.sort_by(|left, right| left.fqcn.cmp(&right.fqcn));
        if !body.is_empty() {
            body.push_str(newline);
        }
        for entry in items {
            body.push_str("import ");
            if entry.is_static {
                body.push_str("static ");
            }
            body.push_str(&entry.fqcn);
            body.push(';');
            if !entry.comment.is_empty() {
                body.push(' ');
                body.push_str(&entry.comment);
            }
            body.push_str(newline);
        }
    }
    for comment in comments.iter().rev() {
        body.insert_str(0, &format!("{comment}{newline}"));
    }
    if body.ends_with(newline) {
        body.truncate(body.len() - newline.len());
    }
    let start = Position::new(first as u32, 0);
    let end = Position::new(
        last as u32,
        lines[last]
            .chars()
            .map(|character| character.len_utf16() as u32)
            .sum(),
    );
    let original: String = lines[first..=last]
        .iter()
        .enumerate()
        .map(|(offset, line)| {
            if offset == 0 {
                (*line).to_string()
            } else {
                format!("{newline}{line}")
            }
        })
        .collect();
    if original == body {
        return None;
    }
    Some(TextEdit {
        range: Range::new(start, end),
        new_text: body,
    })
}
