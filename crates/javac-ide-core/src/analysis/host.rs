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
        if let Some(mut items) = self.receiver_completion(&text, position) {
            items.sort_by_key(|item| item.label.clone());
            items.dedup_by(|a, b| a.label == b.label && a.kind == b.kind);
            items.truncate(100);
            return items;
        }
        let mut items: Vec<javac_analysis_api::CompletionItem> = Vec::new();
        for symbol in self.symbols() {
            items.push(javac_analysis_api::CompletionItem {
                label: symbol.name,
                kind: map_kind(symbol.kind),
                detail: Some(symbol.flat_name),
                documentation: symbol.doc,
            });
        }
        self.ensure_index();
        let indexed: Vec<(String, &'static str)> = self
            .index
            .iter()
            .map(|(key, entry)| (key.clone(), entry.kind))
            .collect();
        for (key, kind) in indexed {
            // The index contributes classes only; members of other files are
            // not valid completion candidates without a receiver.
            if kind != "class" && kind != "nested" {
                continue;
            }
            let Some(name) = key.rsplit('.').next() else { continue };
            items.push(javac_analysis_api::CompletionItem {
                label: name.to_string(),
                kind: map_kind(kind),
                detail: Some(key),
                documentation: None,
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
                return self.location_for(&indexed_file, pos);
            }
        }
        let indexed_hit = self
            .index
            .iter()
            .find(|(key, _)| *key == &word || key.ends_with(&format!(".{word}")))
            .map(|(_, entry)| (entry.file.clone(), entry.pos));
        if let Some((file, pos)) = indexed_hit {
            return self.location_for(&file, pos);
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
        // The tree arena keeps every analyzed file; a tree belongs to this file
        // when the text at its position starts with the identifier.
        let in_file = |pos: i32| -> bool {
            if pos < 0 {
                return false;
            }
            let pos = pos as usize;
            pos <= text.len() && text[pos..].starts_with(&word)
        };
        let mut locations = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            match &tree.kind {
                crate::tree::jc_tree::TreeKind::VariableDecl {
                    name, decl_pos, ..
                } if name.to_java_string().to_string_lossy() == word => {
                    if in_file(*decl_pos) {
                        if let Some(location) = location_at(&text, file, *decl_pos) {
                            locations.push(location);
                        }
                    }
                }
                crate::tree::jc_tree::TreeKind::Ident { name, .. }
                    if name.to_java_string().to_string_lossy() == word =>
                {
                    if in_file(tree.pos) {
                        if let Some(location) = location_at(&text, file, tree.pos) {
                            locations.push(location);
                        }
                    }
                }
                _ => {}
            }
        }
        locations.sort_by_key(|location| (location.range.start.line, location.range.start.column));
        locations.dedup_by(|a, b| a.range == b.range);
        locations
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

    fn rename(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
        new_name: &str,
    ) -> Vec<javac_analysis_api::TextEdit> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let Some(word) = word_at(&text, line, column) else {
            return Vec::new();
        };
        let word_len: u32 = word.chars().map(|c| c.len_utf16() as u32).sum();
        let locations =
            AnalysisSession::references(self, file, position);
        locations
            .into_iter()
            .map(|location| {
                let start = location.range.start;
                javac_analysis_api::TextEdit {
                    range: javac_analysis_api::Range::new(
                        start,
                        javac_analysis_api::Position::new(start.line, start.column + word_len),
                    ),
                    new_text: new_name.to_string(),
                }
            })
            .collect()
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
        Some(match symbol.doc {
            Some(doc) if !doc.is_empty() => format!("{}\n\n{doc}", symbol.detail),
            _ => symbol.detail,
        })
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






impl AnalysisSession {
    /// Completion after a receiver dot: members of the receiver's type.
    fn receiver_completion(
        &mut self,
        text: &str,
        position: javac_analysis_api::Position,
    ) -> Option<Vec<javac_analysis_api::CompletionItem>> {
        use crate::code::type_::TypeKindData;
        let line_text = text.lines().nth(position.line as usize)?;
        let chars: Vec<char> = line_text.chars().collect();
        let cursor = super::positions::char_index_for_utf16(&chars, position.column as usize);
        let before: String = chars[..cursor].iter().collect();
        let trimmed = before.trim_end();
        if !trimmed.ends_with('.') {
            return None;
        }
        let trimmed_chars: Vec<char> = trimmed.chars().collect();
        let dot = trimmed_chars.len() - 1;
        let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        let mut start = dot;
        while start > 0 && is_ident(trimmed_chars[start - 1]) {
            start -= 1;
        }
        let receiver: String = trimmed_chars[start..dot].iter().collect();
        if receiver.is_empty() {
            return None;
        }
        let mut variable = None;
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::VariableDecl {
                name,
                sym: Some(sym),
                ..
            } = &tree.kind
            {
                if name.to_java_string().to_string_lossy() == receiver {
                    variable = Some(*sym);
                    break;
                }
            }
        }
        let Some(symbol) = variable else {
            return None;
        };
        let Some(ty) = self.compiler.ctx.sym(symbol).ty else {
            return Some(Vec::new());
        };
        let class = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::ClassType { .. } => self.compiler.ctx.typ(ty).tsym,
            _ => None,
        };
        let Some(class) = class else {
            return Some(Vec::new());
        };
        let class_flat = self.compiler.ctx.sym(class).flat_name().to_string();
        let Some(scope) = self.compiler.ctx.sym(class).members() else {
            return Some(Vec::new());
        };
        let members = self.compiler.ctx.scope_get_symbols(
            scope,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let mut items = Vec::new();
        for member in members {
            let symbol = self.compiler.ctx.sym(member);
            if (symbol.flags_field & crate::code::flags::GENERATEDCONSTR) != 0 {
                continue;
            }
            let name = symbol.name.to_java_string().to_string_lossy();
            let kind = match symbol.kind_data {
                crate::code::symbol::SymbolKind::MethodSymbol { .. } => "method",
                crate::code::symbol::SymbolKind::VarSymbol { .. } => "field",
                crate::code::symbol::SymbolKind::ClassSymbol { .. } => "nested",
                _ => "member",
            };
            items.push(javac_analysis_api::CompletionItem {
                label: name.clone(),
                kind: map_kind(kind),
                detail: Some(format!("{class_flat}.{name}")),
                documentation: None,
            });
        }
        Some(items)
    }
}


impl AnalysisSession {
    /// Location of a declaration, using the session text when available
    /// (unsaved files have no content on disk).
    fn location_for(&self, file: &Path, pos: i32) -> Option<javac_analysis_api::Location> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
        let absolute = if file.is_absolute() {
            file.to_path_buf()
        } else {
            std::env::current_dir().ok()?.join(file)
        };
        Some(javac_analysis_api::Location {
            file: absolute,
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
        })
    }
}


/// A Location for a source offset in a text.
fn location_at(
    text: &str,
    file: &Path,
    pos: i32,
) -> Option<javac_analysis_api::Location> {
    let (line, column) = offset_to_line_col(text, pos.max(0) as usize);
    Some(javac_analysis_api::Location {
        file: file.to_path_buf(),
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
    })
}
