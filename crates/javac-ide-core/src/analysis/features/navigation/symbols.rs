//! Outline symbols.

use std::path::PathBuf;

use super::super::super::positions::symbol_range;
use super::super::super::{canonical, FileAnalysis};

impl FileAnalysis {
    /// Document symbols (outline).
    pub(crate) fn document_symbols_impl(
        &mut self,
        file: &std::path::Path,
    ) -> Vec<javac_analysis_api::DocumentSymbol> {
        let text = self
            .texts
            .get(&PathBuf::from(canonical(file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let mut classes: Vec<javac_analysis_api::DocumentSymbol> = Vec::new();
        for symbol in self.symbols() {
            let range = symbol_range(&text, symbol.pos, &symbol.name);
            let node = javac_analysis_api::DocumentSymbol {
                name: symbol.name.clone(),
                kind: super::super::map_kind(symbol.kind),
                range,
                children: Vec::new(),
            };
            if symbol.kind == "class" {
                classes.push(node);
            } else if let Some(class) = classes
                .iter_mut()
                .find(|class| class.name == symbol.flat_name.rsplit('.').next().unwrap_or(""))
            {
                class.children.push(node);
            }
        }
        classes
    }
}
