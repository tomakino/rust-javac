//! Foldable regions (braces and comment blocks).
//!
//! Import regions come from `ImportSyntax` AST declarations; comment lines
//! come from the scanner's token comments. No source line prefix is inspected.

use std::path::{Path, PathBuf};

use javac_analysis_api::{FoldingRange, FoldingRangeKind};

use super::super::{canonical, FileAnalysis};
use super::completion::token_stream::{self, comment_styles};
use super::import_syntax::ImportSyntax;
use crate::parser::tokens::CommentStyle;

impl FileAnalysis {
    /// Foldable regions of the current file.
    pub(crate) fn folding_ranges_impl(&mut self, file: &Path) -> Vec<FoldingRange> {
        let _ = self.analyze(file);
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let syntax = self.import_syntax_impl(file);
        let tokens = token_stream::tokenize(&self.compiler, &text);
        let comments = comment_styles(&tokens);
        let token_positions: Vec<usize> = tokens
            .iter()
            .filter(|token| token.pos >= 0)
            .map(|token| token.pos as usize)
            .collect();
        fold_ranges(&text, &syntax, &comments, &token_positions)
    }
}

fn fold_ranges(
    text: &str,
    syntax: &ImportSyntax,
    comments: &[(usize, CommentStyle)],
    token_positions: &[usize],
) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    if let (Some(first), Some(last)) = (
        syntax.declarations.first(),
        syntax.declarations.last(),
    ) {
        let start_line =
            crate::analysis::positions::offset_to_line_col(text, first.start).0;
        let end_line =
            crate::analysis::positions::offset_to_line_col(text, last.end).0;
        if end_line > start_line {
            ranges.push(FoldingRange {
                start_line,
                end_line,
                kind: Some(FoldingRangeKind::Imports),
            });
        }
    }
    // Block/Javadoc comments: start line through the line before the next
    // token (the comment text itself is not scanned).
    for (position, style) in comments {
        if *style == CommentStyle::LINE {
            continue;
        }
        let start_line =
            crate::analysis::positions::offset_to_line_col(text, *position).0;
        let Some(next) = token_positions.iter().copied().find(|token| token > position) else {
            continue;
        };
        let end_line = crate::analysis::positions::offset_to_line_col(
            text,
            next.saturating_sub(1),
        )
        .0;
        if end_line > start_line {
            ranges.push(FoldingRange {
                start_line,
                end_line,
                kind: Some(FoldingRangeKind::Comment),
            });
        }
    }
    // Consecutive line comments fold as one comment region.
    let mut comment_lines: Vec<u32> = comments
        .iter()
        .filter(|(_, style)| *style == CommentStyle::LINE)
        .map(|(position, _)| {
            crate::analysis::positions::offset_to_line_col(text, *position).0
        })
        .collect();
    comment_lines.sort_unstable();
    comment_lines.dedup();
    let mut start: Option<u32> = None;
    let mut previous = None;
    for line in comment_lines {
        match (start, previous) {
            (Some(begin), Some(last)) if line == last + 1 => {
                previous = Some(line);
                let _ = begin;
            }
            (Some(begin), Some(last)) => {
                if last > begin {
                    ranges.push(FoldingRange {
                        start_line: begin,
                        end_line: last,
                        kind: Some(FoldingRangeKind::Comment),
                    });
                }
                start = Some(line);
                previous = Some(line);
            }
            _ => {
                start = Some(line);
                previous = Some(line);
            }
        }
    }
    if let (Some(begin), Some(last)) = (start, previous)
        && last > begin
    {
        ranges.push(FoldingRange {
            start_line: begin,
            end_line: last,
            kind: Some(FoldingRangeKind::Comment),
        });
    }
    // Brace blocks; strings and comments are masked first so their braces do
    // not open a fold.
    let masked = crate::analysis::inspections::mask_non_code(text);
    let mut stack: Vec<u32> = Vec::new();
    for (index, line) in masked.lines().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => stack.push(index as u32),
                '}' => {
                    if let Some(start) = stack.pop()
                        && index as u32 > start
                    {
                        ranges.push(FoldingRange {
                            start_line: start,
                            end_line: index as u32,
                            kind: None,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    ranges.sort_by_key(|range| (range.start_line, range.end_line));
    ranges.dedup();
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold(text: &str) -> Vec<FoldingRange> {
        fold_ranges(text, &ImportSyntax::default(), &[], &[])
    }

    #[test]
    fn braces_fold() {
        let text = "class A {\n    void f() {\n        int x = 1;\n    }\n}\n";
        let ranges = fold(text);
        assert!(
            ranges
                .iter()
                .any(|range| range.start_line == 0 && range.end_line == 4),
            "{ranges:?}"
        );
    }

    fn import_syntax(text: &str) -> ImportSyntax {
        let mut syntax = ImportSyntax::default();
        for (line, path) in [(2usize, "java.util.List"), (3, "java.util.Map")] {
            let start = text
                .lines()
                .take(line)
                .map(|line| line.len() + 1)
                .sum::<usize>();
            let end = start + format!("import {path};").len();
            syntax.declarations.push(super::super::import_syntax::ImportDecl {
                path: path.to_string(),
                segments: path.split('.').map(str::to_string).collect(),
                static_import: false,
                wildcard: false,
                start,
                end,
            });
        }
        syntax
    }

    #[test]
    fn import_block_folds_with_the_imports_kind() {
        let text = "package a;\n\nimport java.util.List;\nimport java.util.Map;\n\nclass A {\n}\n";
        let syntax = import_syntax(text);
        let ranges = fold_ranges(text, &syntax, &[], &[]);
        assert!(
            ranges.iter().any(|range| {
                range.kind == Some(FoldingRangeKind::Imports)
                    && range.start_line == 2
                    && range.end_line == 3
            }),
            "{ranges:?}"
        );
    }

    #[test]
    fn a_single_import_line_does_not_fold() {
        let text = "package a;\n\nimport java.util.List;\n\nclass A {\n}\n";
        let syntax = ImportSyntax {
            declarations: vec![super::super::import_syntax::ImportDecl {
                path: "java.util.List".to_string(),
                segments: vec!["java".into(), "util".into(), "List".into()],
                static_import: false,
                wildcard: false,
                start: 13,
                end: 34,
            }],
            ..ImportSyntax::default()
        };
        let ranges = fold_ranges(text, &syntax, &[], &[]);
        assert!(
            !ranges
                .iter()
                .any(|range| range.kind == Some(FoldingRangeKind::Imports)),
            "{ranges:?}"
        );
    }

    #[test]
    fn consecutive_line_comments_fold() {
        let text = "class A {\n    // one\n    // two\n    int x;\n}\n";
        let comments = vec![
            (text.find("// one").unwrap(), CommentStyle::LINE),
            (text.find("// two").unwrap(), CommentStyle::LINE),
        ];
        let ranges = fold_ranges(text, &ImportSyntax::default(), &comments, &[]);
        assert!(
            ranges.iter().any(|range| {
                range.kind == Some(FoldingRangeKind::Comment)
                    && range.start_line == 1
                    && range.end_line == 2
            }),
            "{ranges:?}"
        );
    }

    #[test]
    fn braces_in_strings_do_not_fold() {
        let text = "class A {\n    String s = \"{\";\n}\n";
        let ranges = fold(text);
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 2);
    }
}
