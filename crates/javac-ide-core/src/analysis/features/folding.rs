//! Foldable regions (braces and comment blocks).

use std::path::{Path, PathBuf};

use javac_analysis_api::{FoldingRange, FoldingRangeKind};

use super::super::{canonical, FileAnalysis};

impl FileAnalysis {
    /// Foldable regions of the current file.
    pub(crate) fn folding_ranges_impl(&mut self, file: &Path) -> Vec<FoldingRange> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        fold_ranges(&text)
    }
}

fn fold_ranges(text: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    // Consecutive line comments / block-comment lines.
    let mut comment_start: Option<u32> = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let is_comment =
            trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*");
        match (is_comment, comment_start) {
            (true, None) => comment_start = Some(index as u32),
            (false, Some(start)) => {
                if index as u32 > start + 1 {
                    ranges.push(FoldingRange {
                        start_line: start,
                        end_line: index as u32 - 1,
                        kind: Some(FoldingRangeKind::Comment),
                    });
                }
                comment_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = comment_start
        && lines.len() as u32 > start + 1
    {
        ranges.push(FoldingRange {
            start_line: start,
            end_line: lines.len() as u32 - 1,
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

    #[test]
    fn braces_and_comment_blocks_fold() {
        let text = "class A {\n    // one\n    // two\n    void f() {\n        int x = 1;\n    }\n}\n";
        let ranges = fold_ranges(text);
        assert!(
            ranges
                .iter()
                .any(|range| range.start_line == 0 && range.end_line == 6),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start_line == 1 && range.end_line == 2),
            "{ranges:?}"
        );
    }

    #[test]
    fn braces_in_strings_do_not_fold() {
        let text = "class A {\n    String s = \"{\";\n}\n";
        let ranges = fold_ranges(text);
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 2);
    }
}
