//! Hover: signature plus Javadoc.

use std::path::Path;

use super::super::FileAnalysis;

impl FileAnalysis {
    /// Hover contents for a position.
    pub(crate) fn hover_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<String> {
        let symbol = FileAnalysis::symbol_at(
            self,
            file,
            position.line as usize + 1,
            position.column as usize + 1,
        )?;
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
        Some(text)
    }
}
