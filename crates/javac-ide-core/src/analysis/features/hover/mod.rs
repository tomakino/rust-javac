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
        let symbol = match self
            .new_class_call_at(&text, position)
            .filter(|call| !call.anonymous && preceded_by_new_keyword(&text, position))
        {
            // `new T(...)`: the cursor is on the type name, but the useful
            // signature is the selected constructor.
            Some(call) => self.symbol_info(call.constructor, -1)?,
            None => FileAnalysis::symbol_at(self, file, line, column)?,
        };
        let newline = char::from(10);
        let mut text = String::from("```java");
        text.push(newline);
        text.push_str(&symbol.detail);
        text.push(newline);
        text.push_str("```");
        if let Some(doc) = symbol.doc {
            // The renderer already emits normalized Markdown; trimming each
            // line would destroy list and code-block indentation.
            let cleaned = doc.trim().to_string();
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

/// Whether the cursor word is introduced by the `new` keyword. Enum
/// constants also have a synthetic constructor call, but their position is
/// not a `new` expression.
fn preceded_by_new_keyword(text: &str, position: javac_analysis_api::Position) -> bool {
    let line = position.line as usize + 1;
    let column = position.column as usize + 1;
    let offset = line_offset(text, line, column).min(text.len());
    let before = text[..offset].trim_end();
    let Some(prefix) = before.strip_suffix("new") else {
        return false;
    };
    match prefix.chars().next_back() {
        Some(c) => !(c.is_alphanumeric() || c == '_' || c == '$'),
        None => true,
    }
}
