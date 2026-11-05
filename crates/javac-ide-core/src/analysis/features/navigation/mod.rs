//! Navigation: definition, type definition, implementations, references,
//! document symbols.

use std::path::{Path, PathBuf};

use super::super::host::location_at;
use super::super::positions::{occurrences, offset_to_line_col, symbol_range, word_at};
use super::super::{canonical, AnalysisSession};

impl AnalysisSession {
    /// Go to definition.
    pub(crate) fn definition_impl(
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
        if let Some(jdk_src) = super::super::jdk_sources::default_jdk_src() {
            return super::super::jdk_sources::jdk_source_location(
                &jdk_src,
                &word,
                &self.config.output_dir,
            );
        }
        None
    }

    /// Go to the type of a variable or method.
    pub(crate) fn type_definition_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        use crate::code::type_::TypeKindData;
        let symbol = AnalysisSession::symbol_at(
            self,
            file,
            position.line as usize + 1,
            position.column as usize + 1,
        )?;
        let mut decl_tree = None;
        for tree in self.compiler.ctx.trees.iter() {
            let sym = match &tree.kind {
                crate::tree::jc_tree::TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
                crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                _ => continue,
            };
            if self.compiler.ctx.sym(sym).name.to_java_string().to_string_lossy() == symbol.name {
                decl_tree = Some(tree);
                break;
            }
        }
        let ty = decl_tree.and_then(|tree| match &tree.kind {
            crate::tree::jc_tree::TreeKind::VariableDecl { vartype, .. } => {
                vartype.and_then(|v| self.compiler.ctx.tree(v).ty)
            }
            crate::tree::jc_tree::TreeKind::MethodDecl { restype, .. } => {
                restype.and_then(|r| self.compiler.ctx.tree(r).ty)
            }
            _ => None,
        })?;
        let class = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::ClassType { .. } => self.compiler.ctx.typ(ty).tsym,
            _ => None,
        }?;
        let class_pos = self.compiler.ctx.sym(class).pos.get();
        let sourcefile = match &self.compiler.ctx.sym(class).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                .as_ref()
                .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
            _ => None,
        };
        if let Some(path) = sourcefile {
            if class_pos >= 0 {
                return self.location_for(&path, class_pos);
            }
        }
        if let Some(jdk_src) = super::super::jdk_sources::default_jdk_src() {
            let name = self
                .compiler
                .ctx
                .sym(class)
                .name
                .to_java_string()
                .to_string_lossy();
            return super::super::jdk_sources::jdk_source_location(
                &jdk_src,
                &name,
                &self.config.output_dir,
            );
        }
        None
    }

    /// Implementations of the interface/class at a position.
    pub(crate) fn implementation_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        use crate::code::type_::TypeKindData;
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
        let Some(target) = self.class_symbol_named(&word) else {
            return Vec::new();
        };
        let mut locations = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            let Ok((sym, extending, implementing)) = (|| {
                match &tree.kind {
                    crate::tree::jc_tree::TreeKind::ClassDecl {
                        sym: Some(sym),
                        extending,
                        implementing,
                        ..
                    } => Ok((*sym, *extending, implementing.clone())),
                    _ => Err(()),
                }
            })() else {
                continue;
            };
            if sym == target {
                continue;
            }
            let supertrees: Vec<crate::ids::TreeId> =
                extending.iter().copied().chain(implementing.iter()).collect();
            let target_flat = self.compiler.ctx.sym(target).flat_name().to_string();
            let mut implements_target = false;
            for supertree in supertrees {
                if let Some(ty) = self.compiler.ctx.tree(supertree).ty {
                    let matches = self
                        .compiler
                        .ctx
                        .typ(ty)
                        .tsym
                        .map(|tsym| self.compiler.ctx.sym(tsym).flat_name().to_string() == target_flat)
                        .unwrap_or(false);
                    if matches {
                        implements_target = true;
                        break;
                    }
                }
            }
            if !implements_target {
                continue;
            }
            let class = self.compiler.ctx.sym(sym);
            let pos = class.pos.get();
            let sourcefile = match &class.kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                    .as_ref()
                    .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
                _ => None,
            };
            if let (Some(path), true) = (sourcefile, pos >= 0) {
                if let Some(location) = self.location_for(&path, pos) {
                    locations.push(location);
                }
            }
        }
        locations
    }

    /// References to the symbol at a position.
    pub(crate) fn references_impl(
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
        let _ = occurrences;
        locations
    }

    /// Document symbols (outline).
    pub(crate) fn document_symbols_impl(
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
                kind: super::map_kind(symbol.kind),
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
