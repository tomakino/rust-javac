//! AnalysisHost implementation.

use std::path::{Path, PathBuf};

use super::diagnostics::convert_diagnostic;
use super::type_names::type_name;
use super::positions::{indexed_location, occurrences, offset_to_line_col, prefix_at, symbol_range, word_at};
use super::features::map_kind;
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
        let mut result: Vec<javac_analysis_api::Diagnostic> = diagnostics
            .iter()
            .map(|diag| convert_diagnostic(diag, &text))
            .collect();
        result.extend(super::inspections::unused_imports(&text));
        result
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
        self.completion_impl(file, position)
    }

    fn definition(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        self.definition_impl(file, position)
    }

    fn type_definition(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        self.type_definition_impl(file, position)
    }

    fn prepare_call_hierarchy(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::CallHierarchyItem> {
        self.prepare_call_hierarchy_impl(file, position)
    }

    fn incoming_calls(
        &mut self,
        item: &javac_analysis_api::CallHierarchyItem,
    ) -> Vec<javac_analysis_api::CallHierarchyCall> {
        self.incoming_calls_impl(item)
    }

    fn outgoing_calls(
        &mut self,
        item: &javac_analysis_api::CallHierarchyItem,
    ) -> Vec<javac_analysis_api::CallHierarchyCall> {
        self.outgoing_calls_impl(item)
    }

    fn prepare_type_hierarchy(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::TypeHierarchyItem> {
        self.prepare_type_hierarchy_impl(file, position)
    }

    fn supertypes(
        &mut self,
        item: &javac_analysis_api::TypeHierarchyItem,
    ) -> Vec<javac_analysis_api::TypeHierarchyItem> {
        self.supertypes_impl(item)
    }

    fn subtypes(
        &mut self,
        item: &javac_analysis_api::TypeHierarchyItem,
    ) -> Vec<javac_analysis_api::TypeHierarchyItem> {
        self.subtypes_impl(item)
    }

    fn implementation(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        self.implementation_impl(file, position)
    }

    fn prepare_rename(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Range> {
        self.prepare_rename_impl(file, position)
    }

    fn references(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        self.references_impl(file, position)
    }

    fn document_symbols(
        &mut self,
        file: &Path,
    ) -> Vec<javac_analysis_api::DocumentSymbol> {
        self.document_symbols_impl(file)
    }

    fn code_actions(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::CodeAction> {
        self.code_actions_impl(file, position)
    }

    fn signature_help(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::SignatureHelp> {
        self.signature_help_impl(file, position)
    }

    fn inlay_hints(&mut self, file: &Path) -> Vec<javac_analysis_api::InlayHint> {
        self.inlay_hints_impl(file)
    }

    fn rename_workspace(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
        new_name: &str,
    ) -> Vec<javac_analysis_api::TextDocumentEdit> {
        self.rename_workspace_impl(file, position, new_name)
    }

    fn rename(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
        new_name: &str,
    ) -> Vec<javac_analysis_api::TextEdit> {
        self.rename_impl(file, position, new_name)
    }

    fn format(&mut self, file: &Path, text: &str) -> Option<String> {
        self.format_impl(file, text)
    }

    fn hover(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<String> {
        self.hover_impl(file, position)
    }
}








impl AnalysisSession {
    /// Location of a declaration, using the session text when available
    /// (unsaved files have no content on disk).
    pub(crate) fn location_for(&self, file: &Path, pos: i32) -> Option<javac_analysis_api::Location> {
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
pub(crate) fn location_at(
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


impl AnalysisSession {
    /// The class symbol with the given simple name (any of the compiler's
    /// source-backed classes).
    pub(crate) fn class_symbol_named(&self, name: &str) -> Option<crate::ids::SymbolId> {
        for sym in self.compiler.syms.get_all_classes() {
            let simple = self
                .compiler
                .ctx
                .sym(sym)
                .name
                .to_java_string()
                .to_string_lossy();
            if simple == name
                && let crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } =
                    &self.compiler.ctx.sym(sym).kind_data
                && sourcefile.is_some()
            {
                return Some(sym);
            }
        }
        None
    }
}


impl AnalysisSession {
    /// Type hierarchy node for a class symbol.
    pub(crate) fn type_hierarchy_item(
        &self,
        class: crate::ids::SymbolId,
    ) -> Option<javac_analysis_api::TypeHierarchyItem> {
        let symbol = self.compiler.ctx.sym(class);
        let name = symbol.name.to_java_string().to_string_lossy();
        let file = match &symbol.kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                .as_ref()
                .map(|file| PathBuf::from(file.get_name().to_string_lossy()))?,
            _ => return None,
        };
        let pos = symbol.pos.get();
        let text = self
            .texts
            .get(&PathBuf::from(canonical(&file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(&file).ok())?;
        let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
        Some(javac_analysis_api::TypeHierarchyItem {
            name,
            kind: javac_analysis_api::SymbolKind::Class,
            file,
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
        })
    }
}


impl AnalysisSession {
    /// Call hierarchy node for a method declared in an analyzed file.
    pub(crate) fn method_item(
        &self,
        name: &str,
    ) -> Option<javac_analysis_api::CallHierarchyItem> {
        let mut found = None;
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            if let crate::tree::jc_tree::TreeKind::MethodDecl {
                name: method,
                sym,
                ..
            } = &tree.kind
            {
                if method.to_java_string().to_string_lossy() != name {
                    continue;
                }
                let _ = index;
                let _ = sym;
                let _ = tree;
                found = Some(index);
                break;
            }
        }
        let index = found?;
        let tree = self.compiler.ctx.trees.get(index as u32);
        let (sym, pos) = match &tree.kind {
            crate::tree::jc_tree::TreeKind::MethodDecl { sym, .. } => (*sym, tree.pos),
            _ => return None,
        };
        let sym = sym?;
        let file = match &self.compiler.ctx.sym(sym).kind_data {
            crate::code::symbol::SymbolKind::MethodSymbol { .. } => {
                let owner = self.compiler.ctx.sym(sym).owner;
                owner.and_then(|owner| match &self.compiler.ctx.sym(owner).kind_data {
                    crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => {
                        sourcefile
                            .as_ref()
                            .map(|file| PathBuf::from(file.get_name().to_string_lossy()))
                    }
                    _ => None,
                })
            }
            _ => None,
        }?;
        let text = self
            .texts
            .get(&PathBuf::from(canonical(&file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(&file).ok())?;
        let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
        Some(javac_analysis_api::CallHierarchyItem {
            name: name.to_string(),
            file,
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
        })
    }
}


impl AnalysisSession {
    /// Method bodies of one file as (name, start, end) byte ranges.
    pub(crate) fn method_ranges(&self, file: &Path) -> Vec<(String, i32, i32)> {
        let file_name = canonical(file);
        let mut methods: Vec<(String, i32, i32)> = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::MethodDecl {
                name,
                sym: Some(sym),
                body: Some(body),
                ..
            } = &tree.kind
            {
                let owner = self.compiler.ctx.sym(*sym).owner;
                let owner_file = owner.and_then(|owner| match &self
                    .compiler
                    .ctx
                    .sym(owner)
                    .kind_data
                {
                    crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                        .as_ref()
                        .map(|file| canonical(Path::new(&file.get_name().to_string_lossy()))),
                    _ => None,
                });
                if owner_file.as_deref() != Some(file_name.as_str()) {
                    continue;
                }
                methods.push((
                    name.to_java_string().to_string_lossy(),
                    self.compiler.ctx.tree(*body).pos,
                    0,
                ));
            }
        }
        methods.sort_by_key(|(_, start, _)| *start);
        let text_len = self
            .texts
            .get(&PathBuf::from(canonical(file)))
            .map(|text| text.len())
            .or_else(|| std::fs::metadata(file).ok().map(|meta| meta.len() as usize))
            .unwrap_or(0) as i32;
        for index in 0..methods.len() {
            methods[index].2 = methods
                .get(index + 1)
                .map(|(_, start, _)| *start)
                .unwrap_or(text_len);
        }
        methods
    }
}



