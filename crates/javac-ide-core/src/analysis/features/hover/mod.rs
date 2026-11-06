//! Hover: signature plus Javadoc.

use std::path::{Path, PathBuf};

use super::super::positions::{in_literal_or_comment, line_offset};
use super::super::{canonical, FileAnalysis};

impl FileAnalysis {
    /// Hover contents for a position.
    pub(crate) fn hover_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<String> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        if in_literal_or_comment(&text, line_offset(&text, line, column)) {
            return None;
        }
        let symbol = FileAnalysis::symbol_at(self, file, line, column)?;
        let newline = char::from(10);
        let mut text = String::from("```java");
        text.push(newline);
        text.push_str(&symbol.detail);
        text.push(newline);
        text.push_str("```");
        if let Some(doc) = symbol.doc {
            let cleaned = doc
                .lines()
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(&newline.to_string())
                .trim()
                .to_string();
            if !cleaned.is_empty() {
                text.push(newline);
                text.push(newline);
                text.push_str("---");
                text.push(newline);
                text.push(newline);
                text.push_str(&cleaned);
            }
        }
        if symbol.kind == "method" {
            let supers = self.super_methods_with_names_impl(file, position);
            if !supers.is_empty() {
                text.push_str("\n\n---\n\n**Overrides:**");
                for (_, name) in supers.iter().take(5) {
                    text.push_str(&format!("\n- `{name}`"));
                }
                if supers.len() > 5 {
                    text.push_str(&format!("\n- … (+{} more)", supers.len() - 5));
                }
            }
        }
        Some(text)
    }
}
