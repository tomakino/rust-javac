//! References, prepare-rename, and the reference collector.

use std::path::{Path, PathBuf};

use super::super::super::host::location_at;
use super::super::super::positions::offset_to_line_col;
use super::super::super::{canonical, FileAnalysis};
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::tree_scanner::TreeScannerVisitor;

impl FileAnalysis {
    /// Range of the identifier that can be renamed, if it is a symbol.
    pub(crate) fn prepare_rename_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Range> {
        let text = self.source_text(file)?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let word = super::super::super::positions::word_at(&text, line, column)?;
        let (_, start) = self.symbol_id_at(file, position)?;
        let (line, column) = offset_to_line_col(&text, start);
        let width = word.chars().map(|ch| ch.len_utf16() as u32).sum::<u32>();
        Some(javac_analysis_api::Range::new(
            javac_analysis_api::Position::new(line, column),
            javac_analysis_api::Position::new(line, column + width),
        ))
    }

    /// References to the symbol at a position.
    pub(crate) fn references_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        // The workspace queries the project index first; this path is only
        // reached for an unsaved file that is not part of the index yet.
        let Some((target, _)) = self.symbol_id_at(file, position) else {
            return Vec::new();
        };
        self.collect_references(target)
    }

    fn symbol_declaration_location(
        &self,
        symbol: SymbolId,
    ) -> Option<javac_analysis_api::Location> {
        let position = self.compiler.ctx.sym(symbol).pos.get();
        if position < 0 {
            return None;
        }
        let mut current = symbol;
        for _ in 0..16 {
            let current_symbol = self.compiler.ctx.sym(current);
            if let crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } =
                &current_symbol.kind_data
            {
                let file = sourcefile.as_ref().map(|source| {
                    PathBuf::from(canonical(Path::new(&source.get_name().to_string_lossy())))
                })?;
                let text = self.source_text(&file)?;
                return location_at(&text, &file, position);
            }
            current = current_symbol.owner?;
        }
        None
    }

    fn collect_references(&mut self, target: SymbolId) -> Vec<javac_analysis_api::Location> {
        let mut roots = Vec::new();
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } = &tree.kind else {
                continue;
            };
            let Some(source) = sourcefile else {
                continue;
            };
            let tree_file =
                PathBuf::from(canonical(Path::new(&source.get_name().to_string_lossy())));
            let Some(tree_text) = self.source_text(&tree_file) else {
                continue;
            };
            roots.push((crate::ids::TreeId::new(index as u32), tree_file, tree_text));
        }
        let mut locations = Vec::new();
        for (root, tree_file, tree_text) in roots {
            let mut collector =
                ReferenceCollector::new(tree_text.clone(), tree_file, Some(target), None);
            collector.scan(&mut self.compiler.ctx, Some(root));
            locations.extend(collector.locations);
        }
        if let Some(location) = self.symbol_declaration_location(target) {
            locations.push(location);
        }
        locations.sort_by_key(|location| {
            (
                location.file.to_string_lossy().to_string(),
                location.range.start.line,
                location.range.start.column,
            )
        });
        locations.dedup_by(|a, b| a.file == b.file && a.range == b.range);
        locations
    }
}

pub(super) struct ReferenceCollector {
    text: String,
    file: PathBuf,
    target: Option<SymbolId>,
    query_start: Option<i32>,
    pub(super) query: Option<SymbolId>,
    locations: Vec<javac_analysis_api::Location>,
}

impl ReferenceCollector {
    pub(super) fn new(
        text: String,
        file: PathBuf,
        target: Option<SymbolId>,
        query_start: Option<i32>,
    ) -> Self {
        Self {
            text,
            file,
            target,
            query_start,
            query: None,
            locations: Vec::new(),
        }
    }

    fn field_name_pos(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        selected: crate::ids::TreeId,
        name: &str,
    ) -> Option<i32> {
        let start = ctx.tree(selected).pos.max(0) as usize;
        let tail = self.text.get(start..)?;
        let offset = tail.find(name)?;
        Some((start + offset) as i32)
    }

    fn record(&mut self, symbol: SymbolId, position: i32) {
        if self.query_start == Some(position) {
            self.query = Some(symbol);
        }
        if self.target == Some(symbol) && position >= 0 {
            if let Some(location) = location_at(&self.text, &self.file, position) {
                self.locations.push(location);
            }
        }
    }
}

impl TreeScannerVisitor for ReferenceCollector {
    fn visit_var_def(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: crate::ids::TreeId,
    ) {
        let (mods, vartype, nameexpr, init, sym, decl_pos) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::VariableDecl {
                mods,
                vartype,
                nameexpr,
                init,
                sym,
                decl_pos,
                ..
            } => (*mods, *vartype, *nameexpr, *init, *sym, *decl_pos),
            _ => return,
        };
        if let Some(sym) = sym {
            self.record(sym, decl_pos);
        }
        self.scan(ctx, mods);
        self.scan(ctx, vartype);
        self.scan(ctx, nameexpr);
        self.scan(ctx, init);
    }

    fn visit_ident(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: crate::ids::TreeId,
    ) {
        if let crate::tree::jc_tree::TreeKind::Ident { sym: Some(sym), .. } = &ctx.tree(tree).kind {
            self.record(*sym, ctx.tree(tree).pos);
        }
    }

    fn visit_select(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: crate::ids::TreeId,
    ) {
        let (selected, name, sym) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::FieldAccess { selected, name, sym, .. } => {
                (*selected, name.to_java_string().to_string_lossy().to_string(), *sym)
            }
            _ => return,
        };
        if let Some(sym) = sym {
            if let Some(selected) = selected {
                if let Some(position) = self.field_name_pos(ctx, selected, &name) {
                    self.record(sym, position);
                }
            }
        }
        self.scan(ctx, selected);
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(ReferenceCollector);
