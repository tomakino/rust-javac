//! AnalysisHost implementation.

use std::path::{Path, PathBuf};

use super::diagnostics::convert_diagnostic;
use super::type_names::type_name;
use super::positions::{indexed_location, occurrences, offset_to_line_col, prefix_at, symbol_range, word_at};
use super::features::map_kind;
use super::{canonical, FileAnalysis};

impl FileAnalysis {
    /// Analyze a file and return its diagnostics (overlay text supported).
    pub(crate) fn diagnostics_impl(&mut self, file: &Path) -> Vec<javac_analysis_api::Diagnostic> {
        let text = self
            .texts
            .get(&PathBuf::from(canonical(file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let hash = crate::workspace::model::hash_text(&text);
        // Repeated requests for the same version answer from the stored result:
        // the compiler log is drained by the first caller.
        if self.diagnostics_hash == Some(hash) {
            return self.diagnostics.clone();
        }
        // The workspace pre-analyzes the file when it creates the slot; a
        // second `analyze` here is a cache hit and keeps the diagnostics.
        let _ = self.analyze(file);
        self.compiler.overlay_text = None;
        let diagnostics = self.compiler.log.take_diagnostics();
        let mut result: Vec<javac_analysis_api::Diagnostic> = diagnostics
            .iter()
            .map(|diag| convert_diagnostic(diag, &text))
            .collect();
        result.extend(self.unused_imports_impl(file));
        // A single-file analysis cannot model a module's package graph:
        // javac then reports every `exports`/`opens` package as empty even
        // though the sources exist under the source roots. Suppress that one
        // false positive for module-info; syntax errors still surface.
        if file
            .file_name()
            .is_some_and(|name| name == "module-info.java")
        {
            result.retain(|diagnostic| {
                diagnostic.code.as_deref() != Some("compiler.err.package.empty.or.not.found")
            });
        }
        self.diagnostics = result.clone();
        self.diagnostics_hash = Some(hash);
        result
    }
}








impl FileAnalysis {
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


impl FileAnalysis {
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


impl FileAnalysis {
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
        let pos = crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, class)
            .map(|position| position as i32)
            .unwrap_or(pos);
        let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
        Some(javac_analysis_api::TypeHierarchyItem {
            name,
            kind: javac_analysis_api::SymbolKind::Class,
            file,
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
        })
    }
}


impl FileAnalysis {
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
        let pos = crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, sym)
            .map(|position| position as i32)
            .unwrap_or(pos);
        let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
        Some(javac_analysis_api::CallHierarchyItem {
            name: name.to_string(),
            file,
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
        })
    }
}


impl FileAnalysis {
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
            .map(|text| text.encode_utf16().count())
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


