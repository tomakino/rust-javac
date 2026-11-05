//! Signature help.

use std::path::{Path, PathBuf};

use super::super::{canonical, AnalysisSession};

impl AnalysisSession {
    /// Signature help for the call at a position.
    pub(crate) fn signature_help_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::SignatureHelp> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let line_text = text.lines().nth(position.line as usize)?;
        let chars: Vec<char> = line_text.chars().collect();
        let cursor = super::super::positions::char_index_for_utf16(&chars, position.column as usize);
        let before: String = chars[..cursor].iter().collect();
        let open = before.rfind('(')?;
        let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        let name: String = before[..open]
            .chars()
            .rev()
            .take_while(|c| is_ident(*c))
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if name.is_empty() {
            return None;
        }
        let after_open = &before[open + 1..];
        let active_parameter = after_open.chars().filter(|c| *c == ',').count() as u32;
        let mut signatures = Vec::new();
        for symbol in self.symbols() {
            if symbol.kind != "method" || symbol.name != name {
                continue;
            }
            let label = symbol.detail;
            let parameters = signature_parameters(&label);
            signatures.push(javac_analysis_api::SignatureInformation { label, parameters });
        }
        if signatures.is_empty() {
            return None;
        }
        Some(javac_analysis_api::SignatureHelp {
            signatures,
            active_signature: 0,
            active_parameter,
        })
    }
}

/// Split the parameter list out of a method signature label.
fn signature_parameters(label: &str) -> Vec<String> {
    let Some(open) = label.find('(') else {
        return Vec::new();
    };
    let Some(close) = label[open..].find(')') else {
        return Vec::new();
    };
    let inner = &label[open + 1..open + close];
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(", ").map(str::to_string).collect()
}
