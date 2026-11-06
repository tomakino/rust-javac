//! Definition, type definition, and implementations.

use std::path::{Path, PathBuf};

use super::super::super::positions::{offset_to_line_col, word_at};
use super::super::super::{canonical, FileAnalysis};

impl FileAnalysis {
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
        if let Some(symbol) = FileAnalysis::symbol_at(self, file, line, column) {
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
        let word = word_at(&text, line, column)?;
        if let Some(jdk_src) = super::super::super::jdk_sources::default_jdk_src() {
            return super::super::super::jdk_sources::jdk_source_location(
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
        let symbol = FileAnalysis::symbol_at(
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
        if let Some(jdk_src) = super::super::super::jdk_sources::default_jdk_src() {
            let name = self
                .compiler
                .ctx
                .sym(class)
                .name
                .to_java_string()
                .to_string_lossy();
            return super::super::super::jdk_sources::jdk_source_location(
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
}
