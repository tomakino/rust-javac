//! AnalysisHost implementation.

use std::path::{Path, PathBuf};

use super::diagnostics::convert_diagnostic;
use super::positions::{indexed_location, occurrences, offset_to_line_col, prefix_at, symbol_range, word_at};
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
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let text = self
            .texts
            .get(&PathBuf::from(canonical(file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let prefix = prefix_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let mut items: Vec<javac_analysis_api::CompletionItem> = Vec::new();
        for symbol in self.symbols() {
            items.push(javac_analysis_api::CompletionItem {
                label: symbol.name,
                kind: map_kind(symbol.kind),
                detail: Some(symbol.flat_name),
            });
        }
        self.ensure_index();
        let indexed: Vec<(String, &'static str)> = self
            .index
            .iter()
            .map(|(key, entry)| (key.clone(), entry.kind))
            .collect();
        for (key, kind) in indexed {
            let Some(name) = key.rsplit('.').next() else { continue };
            items.push(javac_analysis_api::CompletionItem {
                label: name.to_string(),
                kind: map_kind(kind),
                detail: Some(key),
            });
        }
        items.retain(|item| item.label.starts_with(&prefix));
        items.sort_by_key(|item| {
            let exact = if item.label == prefix { 0 } else { 1 };
            let rank = match item.kind {
                javac_analysis_api::SymbolKind::Class => 0,
                javac_analysis_api::SymbolKind::Interface => 1,
                javac_analysis_api::SymbolKind::Method => 2,
                javac_analysis_api::SymbolKind::Field => 3,
                _ => 4,
            };
            (exact, rank, item.label.clone())
        });
        items.dedup_by(|a, b| a.label == b.label && a.kind == b.kind);
        items.truncate(100);
        items
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
        if let Some(jdk_src) = super::jdk_sources::default_jdk_src() {
            return super::jdk_sources::jdk_source_location(
                &jdk_src,
                &word,
                &self.config.output_dir,
            );
        }
        None
    }

    fn references(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let Some(word) = word_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        ) else {
            return Vec::new();
        };
        let mut result = occurrences(&text, &word, file);
        // Project source files that may reference the same symbol.
        self.ensure_index();
        let indexed_files: Vec<PathBuf> = self
            .index
            .iter()
            .filter(|(name, _)| name.rsplit('.').next() == Some(word.as_str()))
            .map(|(_, entry)| entry.file.clone())
            .filter(|path| path != file)
            .collect();
        for other in indexed_files {
            if let Ok(other_text) = std::fs::read_to_string(&other) {
                result.extend(occurrences(&other_text, &word, &other));
            }
        }
        result
    }

    fn document_symbols(
        &mut self,
        file: &Path,
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
                kind: map_kind(symbol.kind),
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

    fn hover(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<String> {
        let symbol = AnalysisSession::symbol_at(
            self,
            file,
            position.line as usize + 1,
            position.column as usize + 1,
        )?;
        Some(symbol.detail)
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




