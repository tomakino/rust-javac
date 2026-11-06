//! Definition, type definition, and implementations.

use std::path::{Path, PathBuf};

use super::super::super::positions::{
    declared_name_start, in_literal_or_comment, line_offset, offset_to_line_col, word_at,
};
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
        // Strings and comments are not code: no navigation from there.
        if in_literal_or_comment(&text, line_offset(&text, line, column)) {
            return None;
        }
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
        // A symbol that resolves through the compiler but is not declared in
        // project sources may live in a classpath jar; use its sources jar.
        if let Some((symbol, _)) = self.symbol_id_at(file, position) {
            if let Some(location) = self.external_symbol_location(symbol) {
                return Some(location);
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

    /// Location in a classpath `-sources.jar`, when `symbol` resolves to a
    /// class outside the project sources.
    fn external_symbol_location(
        &self,
        symbol: crate::ids::SymbolId,
    ) -> Option<javac_analysis_api::Location> {
        let mut current = symbol;
        for _ in 0..16 {
            let sym = self.compiler.ctx.sym(current);
            if let crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } = &sym.kind_data
            {
                let flat = sym.flat_name().to_string();
                // The member to locate is the symbol under the cursor (a method,
                // field, or the class itself), not the enclosing class name.
                let member = self
                    .compiler
                    .ctx
                    .sym(symbol)
                    .name
                    .to_java_string()
                    .to_string_lossy();
                let in_project = sourcefile.as_ref().is_some_and(|file| {
                    let name = file.get_name().to_string_lossy();
                    self.config
                        .source_path
                        .iter()
                        .any(|root| name.contains(&root.to_string_lossy().to_string()))
                });
                if in_project {
                    return None;
                }
                // JDK classes come from src.zip; other jars from their
                // `-sources.jar` on the class path.
                if let Some(jdk_src) = super::super::super::jdk_sources::default_jdk_src() {
                    if let Some(location) =
                        super::super::super::jdk_sources::jdk_member_location(
                            &jdk_src,
                            &flat,
                            &member,
                            &self.config.output_dir,
                        )
                    {
                        return Some(location);
                    }
                }
                return crate::analysis::external_sources::external_source_location(
                    &flat,
                    &member,
                    &self.config.class_path,
                    &self.config.output_dir,
                );
            }
            current = sym.owner?;
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
        // The compiler symbol works for locals and parameters too, which are
        // not part of `symbols()` (declared members only).
        let (symbol, _) = self.symbol_id_at(file, position)?;
        let ty = self.compiler.ctx.sym(symbol).ty?;
        let class = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::ClassType { .. } => self.compiler.ctx.typ(ty).tsym,
            _ => None,
        }?;
        let class_pos = self.compiler.ctx.sym(class).pos.get();
        let class_name = self
            .compiler
            .ctx
            .sym(class)
            .name
            .to_java_string()
            .to_string_lossy()
            .to_string();
        let sourcefile = match &self.compiler.ctx.sym(class).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                .as_ref()
                .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
            _ => None,
        };
        if let Some(path) = sourcefile {
            if class_pos >= 0 {
                let key = PathBuf::from(canonical(&path));
                let text = self
                    .texts
                    .get(&key)
                    .cloned()
                    .or_else(|| std::fs::read_to_string(&path).ok());
                let pos = match text {
                    Some(text) => {
                        declared_name_start(&text, class_pos as u32, &class_name) as i32
                    }
                    None => class_pos,
                };
                return self.location_for(&path, pos);
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
            let class_name = class
                .name
                .to_java_string()
                .to_string_lossy()
                .to_string();
            let pos = class.pos.get();
            let sourcefile = match &class.kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                    .as_ref()
                    .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
                _ => None,
            };
            if let (Some(path), true) = (sourcefile, pos >= 0) {
                let key = PathBuf::from(canonical(&path));
                let text = self
                    .texts
                    .get(&key)
                    .cloned()
                    .or_else(|| std::fs::read_to_string(&path).ok());
                let pos = match text {
                    Some(text) => {
                        declared_name_start(&text, pos as u32, &class_name) as i32
                    }
                    None => pos,
                };
                if let Some(location) = self.location_for(&path, pos) {
                    locations.push(location);
                }
            }
        }
        locations
    }
}
