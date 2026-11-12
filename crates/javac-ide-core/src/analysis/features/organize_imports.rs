//! Organize imports (section 13): dedupe, drop unused, group and sort.
//!
//! Import declarations come from `ImportSyntax` (AST `Import` nodes).
//! Comments come from the ported scanner's comment list; no line prefix is
//! inspected.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, Range, TextEdit};

use super::super::{canonical, FileAnalysis};
use super::import_syntax::{ImportDecl, ImportSyntax};

impl FileAnalysis {
    pub(crate) fn organize_imports_impl(&mut self, file: &Path) -> Vec<TextEdit> {
        let _ = self.analyze(file);
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let syntax = self.import_syntax_impl(file);
        if syntax.declarations.is_empty() {
            return Vec::new();
        }
        let unused: HashSet<String> = self
            .unused_imports_impl(file)
            .into_iter()
            .filter_map(|diagnostic| {
                diagnostic
                    .message
                    .strip_prefix("Unused import: ")
                    .map(|path| path.to_string())
            })
            .collect();
        organize_imports_text(&text, &syntax, &unused)
            .into_iter()
            .collect()
    }
}

fn organize_imports_text(
    text: &str,
    syntax: &ImportSyntax,
    unused: &HashSet<String>,
) -> Option<TextEdit> {
    let first = syntax.declarations.first()?;
    let last = syntax.declarations.last()?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let first_line = crate::analysis::positions::offset_to_line_col(text, first.start).0;
    let last_line = crate::analysis::positions::offset_to_line_col(text, last.end).0;

    let mut kept: Vec<&ImportDecl> = Vec::new();
    let mut seen = HashSet::new();
    for declaration in &syntax.declarations {
        if !declaration.wildcard && unused.contains(&declaration.path) {
            continue;
        }
        if !seen.insert((declaration.static_import, declaration.path.clone())) {
            continue;
        }
        kept.push(declaration);
    }
    if kept.is_empty() {
        return None;
    }

    let mut body = String::new();
    // Trivia between/after AST import nodes: raw segments, no line-prefix
    // classification. Trimming leaves whole comment lines/blocks intact.
    let mut trivia: Vec<&str> = Vec::new();
    for pair in syntax.declarations.windows(2) {
        let between = text
            .get(pair[0].end..pair[1].start)
            .unwrap_or_default()
            .trim();
        if !between.is_empty() {
            trivia.push(between);
        }
    }
    let tail = text.get(last.end..).unwrap_or_default();
    let tail_end = tail.find('\n').map(|offset| &tail[..offset]).unwrap_or(tail);
    let tail = tail_end.trim();
    if !tail.is_empty() {
        trivia.push(tail);
    }
    for comment in trivia {
        body.push_str(comment);
        body.push_str(newline);
    }

    let group = |declaration: &ImportDecl| -> &'static str {
        if declaration.static_import {
            "static"
        } else if declaration.segments.first().map(String::as_str) == Some("java") {
            "java"
        } else if declaration.segments.first().map(String::as_str) == Some("javax") {
            "javax"
        } else {
            "third-party"
        }
    };
    for group_name in ["java", "javax", "third-party", "static"] {
        let mut items: Vec<&ImportDecl> = kept
            .iter()
            .copied()
            .filter(|declaration| group(declaration) == group_name)
            .collect();
        if items.is_empty() {
            continue;
        }
        items.sort_by(|left, right| left.path.cmp(&right.path));
        if !body.is_empty() && !body.ends_with(newline) {
            body.push_str(newline);
        }
        for declaration in items {
            body.push_str("import ");
            if declaration.static_import {
                body.push_str("static ");
            }
            body.push_str(&declaration.path);
            body.push(';');
            body.push_str(newline);
        }
    }
    if body.ends_with(newline) {
        body.truncate(body.len() - newline.len());
    }

    let (start_line, _) =
        crate::analysis::positions::offset_to_line_col(text, first.start);
    let (end_line, end_column) =
        crate::analysis::positions::offset_to_line_col(text, last.end);
    let start = Position::new(start_line, 0);
    let end = Position::new(end_line, end_column);
    let original = text.get(byte_offset(text, start)?..byte_offset(text, end)?)?;
    if original == body {
        return None;
    }
    Some(TextEdit {
        range: Range::new(start, end),
        new_text: body,
    })
}

fn byte_offset(text: &str, position: Position) -> Option<usize> {
    Some(crate::analysis::positions::line_offset(
        text,
        position.line as usize + 1,
        position.column as usize + 1,
    ))
}
