//! AnalysisHost implementation.

use std::path::{Path, PathBuf};

use super::diagnostics::convert_diagnostic;
use super::positions::{indexed_location, offset_to_line_col, word_at};
use super::{canonical, AnalysisSession};

impl javac_analysis_api::AnalysisHost for AnalysisSession {
    fn analyze(
        &mut self,
        file: &Path,
        text: Option<&str>,
    ) -> Vec<javac_analysis_api::Diagnostic> {
        if let Some(text) = text {
            self.texts
                .insert(PathBuf::from(canonical(file)), text.to_string());
            self.compiler.overlay_text = Some((PathBuf::from(canonical(file)), text.to_string()));
            self.cache = None;
        } else {
            self.compiler.overlay_text = None;
        }
        let _ = self.analyze(file);
        self.compiler.overlay_text = None;
        let text = self
            .texts
            .get(&PathBuf::from(canonical(file)))
            .cloned()
            .unwrap_or_default();
        let diagnostics = self.compiler.log.take_diagnostics();
        diagnostics
            .iter()
            .map(|diag| convert_diagnostic(diag, &text))
            .collect()
    }

    fn symbols(&mut self, _file: &Path) -> Vec<javac_analysis_api::Symbol> {
        self.symbols()
            .into_iter()
            .map(|symbol| javac_analysis_api::Symbol {
                name: symbol.name,
                kind: map_kind(symbol.kind),
                range: None,
                qualified_name: Some(symbol.flat_name),
            })
            .collect()
    }

    fn symbol_at(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Symbol> {
        self.symbol_at(file, position.line as usize + 1, position.column as usize + 1)
            .map(|symbol| javac_analysis_api::Symbol {
                name: symbol.name,
                kind: map_kind(symbol.kind),
                range: None,
                qualified_name: Some(symbol.flat_name),
            })
    }

    fn completion(
        &mut self,
        _file: &Path,
        _position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        // Baseline completion: every symbol of the analyzed file. The LSP
        // client filters by the typed prefix.
        self.symbols()
            .into_iter()
            .map(|symbol| javac_analysis_api::CompletionItem {
                label: symbol.name,
                kind: map_kind(symbol.kind),
                detail: Some(symbol.flat_name),
            })
            .collect()
    }

    fn definition(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        if let Some(symbol) = AnalysisSession::symbol_at(self, file, line, column) {
            if symbol.pos >= 0 {
                let (symbol_line, symbol_column) =
                    offset_to_line_col(&text, symbol.pos as usize);
                return Some(javac_analysis_api::Location {
                    file: file.to_path_buf(),
                    range: javac_analysis_api::Range::point(
                        javac_analysis_api::Position::new(symbol_line, symbol_column),
                    ),
                });
            }
        }
        self.ensure_index();
        let word = word_at(&text, line, column)?;
        // Prefer the symbol's own key, then the word suffix.
        if let Some(symbol) = AnalysisSession::symbol_at(self, file, line, column) {
            let key = if symbol.kind == "class" {
                symbol.flat_name.clone()
            } else {
                format!("{}.{}", symbol.flat_name, symbol.name)
            };
            if let Some((indexed_file, pos)) = self.index_lookup(&key) {
                return indexed_location(indexed_file, pos);
            }
        }
        for (key, entry) in &self.index {
            if key == &word || key.ends_with(&format!(".{word}")) {
                return indexed_location(entry.file.clone(), entry.pos);
            }
        }
        None
    }

    fn references(
        &mut self,
        _file: &Path,
        _position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        Vec::new()
    }

    fn hover(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<String> {
        let symbol = AnalysisSession::symbol_at(self, file, position.line as usize + 1, position.column as usize + 1)?;
        Some(format!("{:?} {}", symbol.kind, symbol.name))
    }
}

fn map_kind(kind: &'static str) -> javac_analysis_api::SymbolKind {
    match kind {
        "class" => javac_analysis_api::SymbolKind::Class,
        "method" => javac_analysis_api::SymbolKind::Method,
        "field" => javac_analysis_api::SymbolKind::Field,
        "nested" => javac_analysis_api::SymbolKind::Class,
        _ => javac_analysis_api::SymbolKind::Field,
    }
}




