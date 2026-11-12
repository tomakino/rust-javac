//! Extra inspections aligned with IntelliJ IDEA (beyond javac errors).

use javac_analysis_api::{Diagnostic, Position, Range, Severity};

use std::path::Path;

use crate::analysis::FileAnalysis;

impl FileAnalysis {
    /// Report non-static, non-wildcard imports that are never referenced.
    ///
    /// Import declarations come from `CompilationUnit.defs` (`Import.qualid`);
    /// a name is used when an `Ident` / `FieldAccess` AST node outside the
    /// import declarations carries the same simple name.
    pub(crate) fn unused_imports_impl(&self, file: &Path) -> Vec<Diagnostic> {
        let syntax = self.import_syntax_impl(file);
        if syntax.declarations.is_empty() {
            return Vec::new();
        }
        let Some(text) = self.source_text(file) else {
            return Vec::new();
        };
        let import_spans: Vec<(usize, usize)> = syntax
            .declarations
            .iter()
            .map(|declaration| (declaration.start, declaration.end))
            .collect();
        let mut used = std::collections::HashSet::new();
        for tree in self.compiler.ctx.trees.iter() {
            let name = match &tree.kind {
                crate::tree::jc_tree::TreeKind::Ident { name, .. }
                | crate::tree::jc_tree::TreeKind::FieldAccess { name, .. } => name,
                _ => continue,
            };
            if tree.pos < 0 {
                continue;
            }
            let pos = tree.pos as usize;
            if import_spans
                .iter()
                .any(|(start, end)| pos >= *start && pos <= *end)
            {
                continue;
            }
            used.insert(name.to_java_string().to_string_lossy().to_string());
        }
        let mut diagnostics = Vec::new();
        for declaration in &syntax.declarations {
            if declaration.static_import || declaration.wildcard {
                continue;
            }
            let Some(simple) = declaration.segments.last() else {
                continue;
            };
            if used.contains(simple) {
                continue;
            }
            let (start_line, start_column) =
                crate::analysis::positions::offset_to_line_col(&text, declaration.start);
            let (end_line, end_column) =
                crate::analysis::positions::offset_to_line_col(&text, declaration.end);
            diagnostics.push(Diagnostic {
                range: Range::new(
                    Position::new(start_line, start_column),
                    Position::new(end_line, end_column),
                ),
                severity: Severity::Warning,
                message: format!("Unused import: {}", declaration.path),
                code: Some("unused.import".to_string()),
            });
        }
        diagnostics
    }
}

/// Replace comments and literals with spaces, keeping newlines.
pub(crate) fn mask_non_code(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '/' && index + 1 < chars.len() && chars[index + 1] == '/' {
            while index < chars.len() && chars[index] != '\n' {
                out.push(' ');
                index += 1;
            }
        } else if ch == '/' && index + 1 < chars.len() && chars[index + 1] == '*' {
            out.push(' ');
            out.push(' ');
            index += 2;
            while index < chars.len() {
                if chars[index] == '*' && index + 1 < chars.len() && chars[index + 1] == '/' {
                    out.push(' ');
                    out.push(' ');
                    index += 2;
                    break;
                }
                out.push(if chars[index] == '\n' { '\n' } else { ' ' });
                index += 1;
            }
        } else if ch == '"' && index + 2 < chars.len() && chars[index + 1] == '"' && chars[index + 2] == '"' {
            out.push_str("   ");
            index += 3;
            while index < chars.len() {
                if index + 2 < chars.len()
                    && chars[index] == '"'
                    && chars[index + 1] == '"'
                    && chars[index + 2] == '"'
                {
                    out.push_str("   ");
                    index += 3;
                    break;
                }
                out.push(if chars[index] == '\n' { '\n' } else { ' ' });
                index += 1;
            }
        } else if ch == '"' || ch == '\'' {
            let quote = ch;
            out.push(' ');
            index += 1;
            while index < chars.len() {
                let current = chars[index];
                if current == '\\' && index + 1 < chars.len() {
                    out.push(' ');
                    out.push(' ');
                    index += 2;
                    continue;
                }
                out.push(if current == '\n' { '\n' } else { ' ' });
                index += 1;
                if current == quote {
                    break;
                }
            }
        } else {
            out.push(ch);
            index += 1;
        }
    }
    out
}

