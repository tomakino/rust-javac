//! Navigation: definition, type definition, implementations, references,
//! document symbols.

use std::path::{Path, PathBuf};

use super::super::host::location_at;
use super::super::positions::{line_offset, offset_to_line_col, symbol_range, word_at};
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::tree_scanner::TreeScannerVisitor;
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


    fn source_text(&self, file: &Path) -> Option<String> {
        let key = PathBuf::from(canonical(file));
        self.texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
    }

    /// The source file that owns each tree arena slot. The parser appends one
    /// compilation unit at a time, so a unit owns the slots up to the next unit.
    fn tree_files(&self) -> Vec<Option<PathBuf>> {
        let mut files = vec![None; self.compiler.ctx.trees.len()];
        let mut current = None;
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            if let crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } = &tree.kind {
                current = sourcefile.as_ref().map(|file| {
                    PathBuf::from(canonical(Path::new(&file.get_name().to_string_lossy())))
                });
            }
            files[index] = current.clone();
        }
        files
    }

    /// Symbol attributed at a source position, plus the identifier's byte start.
    fn symbol_id_at(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<(SymbolId, usize)> {
        let text = self.source_text(file)?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let word = word_at(&text, line, column)?;
        if word.is_empty() {
            return None;
        }
        let cursor = line_offset(&text, line, column);
        let start = word_start_offset(&text, cursor);
        let Some(root) = self.latest_root_for(file) else {
            return None;
        };
        let mut collector = ReferenceCollector::new(
            text.clone(),
            file.to_path_buf(),
            None,
            Some(start as i32),
        );
        collector.scan(&mut self.compiler.ctx, Some(root));
        if let Some(symbol) = collector.query {
            return Some((symbol, start));
        }
        // Declarations have exact symbol positions even when their tree has no
        // reliable token position (for example a field access node).
        for tree in self.compiler.ctx.trees.iter() {
            let symbol = match &tree.kind {
                crate::tree::jc_tree::TreeKind::VariableDecl {
                    sym: Some(sym),
                    decl_pos,
                    ..
                } if *decl_pos == start as i32 => Some(*sym),
                crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. }
                | crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => {
                    (self.compiler.ctx.sym(*sym).pos.get() == start as i32).then_some(*sym)
                }
                _ => None,
            };
            if let Some(symbol) = symbol {
                return Some((symbol, start));
            }
        }
        None
    }

    fn latest_root_for(&self, file: &Path) -> Option<crate::ids::TreeId> {
        let key = canonical(file);
        let mut root = None;
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            if let crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } = &tree.kind {
                let matches = sourcefile.as_ref().is_some_and(|source| {
                    canonical(Path::new(&source.get_name().to_string_lossy())) == key
                });
                if matches {
                    root = Some(crate::ids::TreeId::new(index as u32));
                }
            }
        }
        root
    }

    /// Range of the identifier that can be renamed, if it is a symbol.
    pub(crate) fn prepare_rename_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Range> {
        let text = self.source_text(file)?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let word = word_at(&text, line, column)?;
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
        let Some((target, _)) = self.symbol_id_at(file, position) else {
            return Vec::new();
        };
        let mut roots = Vec::new();
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } = &tree.kind else {
                continue;
            };
            let Some(source) = sourcefile else {
                continue;
            };
            let tree_file = PathBuf::from(canonical(Path::new(&source.get_name().to_string_lossy())));
            let Some(tree_text) = self.source_text(&tree_file) else {
                continue;
            };
            roots.push((crate::ids::TreeId::new(index as u32), tree_file, tree_text));
        }
        let mut locations = Vec::new();
        for (root, tree_file, tree_text) in roots {
            let mut collector = ReferenceCollector::new(
                tree_text.clone(),
                tree_file,
                Some(target),
                None,
            );
            collector.scan(&mut self.compiler.ctx, Some(root));
            locations.extend(collector.locations);
        }
        // The declaration tree carries the token position for fields and
        // parameters even when sym.pos is NOPOS.
        if let Some(tree_text) = self.source_text(file) {
            let declaration_file = PathBuf::from(canonical(file));
            for tree in self.compiler.ctx.trees.iter() {
                let position = match &tree.kind {
                    crate::tree::jc_tree::TreeKind::VariableDecl {
                        sym: Some(sym),
                        decl_pos,
                        ..
                    } if *sym == target => *decl_pos,
                    crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. }
                    | crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. }
                        if *sym == target =>
                    {
                        self.compiler.ctx.sym(*sym).pos.get()
                    }
                    _ => continue,
                };
                if position >= 0 {
                    if let Some(location) = location_at(&tree_text, &declaration_file, position) {
                        locations.push(location);
                    }
                }
            }
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

fn word_start_offset(text: &str, cursor: usize) -> usize {
    let mut start = cursor.min(text.len());
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    for (index, ch) in text[..start].char_indices().rev() {
        if is_ident(ch) {
            start = index;
        } else {
            break;
        }
    }
    start
}


struct ReferenceCollector {
    text: String,
    file: PathBuf,
    target: Option<SymbolId>,
    query_start: Option<i32>,
    query: Option<SymbolId>,
    locations: Vec<javac_analysis_api::Location>,
}

impl ReferenceCollector {
    fn new(
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
            if let Some(location) =
                location_at(&self.text, &self.file, position)
            {
                self.locations.push(location);
            }
        }
    }
}

impl crate::tree::tree_scanner::TreeScannerVisitor for ReferenceCollector {
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

