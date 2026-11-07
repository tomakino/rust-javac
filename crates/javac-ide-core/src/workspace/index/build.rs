//! Build a [`ProjectIndex`] from a temporary compiler pass.
//!
//! Files are compiled in batches with a throw-away compiler; only the batch's
//! own compilation units are indexed. The compiler is dropped after every
//! batch, so the index never owns a compiler session.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::code::flags;
use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId as CompilerSymbolId, TreeId};
use crate::main::mini_compiler::MiniCompiler;
use crate::tree::doc_comment_table::DocCommentTable;
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_scanner::TreeScannerVisitor;

use super::super::analysis::FileAnalysis;
use super::super::model::WorkspaceModel;
use super::key::*;
use super::model::*;

/// Default number of files compiled in one temporary compiler session.
pub const DEFAULT_BATCH_SIZE: usize = 128;
/// Files above this count are skipped and the index reports `Partial`.
pub const DEFAULT_MAX_INDEXED_FILES: usize = 200_000;
/// Occurrence storage above this many bytes stops growing and reports `Partial`.
pub const DEFAULT_MAX_OCCURRENCE_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Default, Clone)]
pub struct IndexLimits {
    pub max_indexed_files: usize,
    pub max_occurrence_bytes: usize,
}

impl IndexLimits {
    pub fn new(max_indexed_files: usize, max_occurrence_bytes: usize) -> Self {
        Self {
            max_indexed_files,
            max_occurrence_bytes,
        }
    }

    pub fn defaults() -> Self {
        Self::new(DEFAULT_MAX_INDEXED_FILES, DEFAULT_MAX_OCCURRENCE_BYTES)
    }
}

#[derive(Debug)]
pub struct IndexBuildResult {
    pub index: ProjectIndex,
    pub skipped_files: Vec<PathBuf>,
    pub failed_files: Vec<PathBuf>,
    pub occurrence_bytes: usize,
}

/// Build a complete index from the workspace model.
pub fn build(
    model: &WorkspaceModel,
    config: &crate::analysis::AnalysisConfig,
    overlays: &HashMap<PathBuf, String>,
    batch_size: usize,
    limits: &IndexLimits,
) -> IndexBuildResult {
    IndexBuilder::new(model, config, limits).build(overlays, batch_size)
}

pub struct IndexBuilder {
    model: WorkspaceModel,
    config: crate::analysis::AnalysisConfig,
    limits: IndexLimits,
    files: Vec<PathBuf>,
    file_ids: HashMap<PathBuf, FileId>,
    records: Vec<FileRecord>,
    texts: HashMap<FileId, String>,
    symbols: Vec<SymbolRecord>,
    by_key: HashMap<SymbolKey, SymbolId>,
    by_name: HashMap<NameId, Vec<SymbolId>>,
    by_flat_name: HashMap<String, SymbolId>,
    by_supertype: HashMap<SymbolId, Vec<SymbolId>>,
    by_file: HashMap<FileId, FileIndex>,
    occurrences: HashMap<SymbolId, Vec<Occurrence>>,
    occurrence_seen: HashSet<(FileId, u32, u32, SymbolId, OccurrenceRole)>,
    occurrence_bytes: usize,
    occurrences_full: bool,
    pending_supertypes: Vec<(SymbolId, Vec<String>)>,
    failed_files: Vec<PathBuf>,
    skipped_files: Vec<PathBuf>,
    status_reason: Option<String>,
    overlays: HashMap<PathBuf, String>,
    strings: StringInterner,
    /// Rendered Javadoc of the symbols in the batch being indexed.
    javadocs: HashMap<CompilerSymbolId, String>,
    /// Batch-local AST name data: declaring trees and parser name tables.
    symbol_trees: HashMap<CompilerSymbolId, TreeId>,
    name_positions: HashMap<FileId, Option<Rc<dyn crate::tree::end_pos_table::EndPosTable>>>,
}

impl IndexBuilder {
    pub fn new(
        model: &WorkspaceModel,
        config: &crate::analysis::AnalysisConfig,
        limits: &IndexLimits,
    ) -> Self {
        let all_files = collect_java_files(&model.source_roots());
        let (files, skipped_files) = if all_files.len() > limits.max_indexed_files {
            let files = all_files[..limits.max_indexed_files].to_vec();
            let skipped = all_files[limits.max_indexed_files..].to_vec();
            (files, skipped)
        } else {
            (all_files, Vec::new())
        };
        let mut file_ids = HashMap::new();
        let mut records = Vec::with_capacity(files.len());
        let mut texts = HashMap::new();
        for (index, path) in files.iter().enumerate() {
            let id = index as FileId;
            file_ids.insert(path.clone(), id);
            records.push(FileRecord {
                path: path.clone(),
                hash: 0,
                source_set: model
                    .source_set_for(path)
                    .unwrap_or(SourceSetKind::Main),
                status: FileStatus::Skipped,
            });
            texts.insert(id, String::new());
        }
        Self {
            model: model.clone(),
            config: config.clone(),
            limits: limits.clone(),
            files,
            file_ids,
            records,
            texts,
            symbols: Vec::new(),
            by_key: HashMap::new(),
            by_name: HashMap::new(),
            by_flat_name: HashMap::new(),
            by_supertype: HashMap::new(),
            by_file: HashMap::new(),
            occurrences: HashMap::new(),
            occurrence_seen: HashSet::new(),
            occurrence_bytes: 0,
            occurrences_full: false,
            pending_supertypes: Vec::new(),
            failed_files: Vec::new(),
            skipped_files,
            status_reason: None,
            overlays: HashMap::new(),
            strings: StringInterner::default(),
            javadocs: HashMap::new(),
            symbol_trees: HashMap::new(),
            name_positions: HashMap::new(),
        }
    }

    pub fn build(
        mut self,
        overlays: &HashMap<PathBuf, String>,
        batch_size: usize,
    ) -> IndexBuildResult {
        self.overlays = overlays.clone();
        self.load_texts(overlays);
        if self.files.is_empty() {
            return self.finish(IndexStatus::Ready);
        }
        let batch_size = batch_size.max(1);
        let batches = self.batches(batch_size);
        for batch in batches {
            self.build_batch(&batch);
        }
        let mut status = if self.failed_files.is_empty() && self.skipped_files.is_empty() {
            IndexStatus::Ready
        } else {
            IndexStatus::Partial {
                failed_files: self.failed_files.clone(),
                reason: self
                    .status_reason
                    .clone()
                    .unwrap_or_else(|| "some project files failed to compile".to_string()),
            }
        };
        if !self.occurrences_full {
            // Nothing to do: every occurrence fit within the byte budget.
        } else if let IndexStatus::Ready = status {
            status = IndexStatus::Partial {
                failed_files: Vec::new(),
                reason: "occurrence byte budget exhausted".to_string(),
            };
        }
        self.finish(status)
    }

    fn load_texts(&mut self, overlays: &HashMap<PathBuf, String>) {
        for (index, path) in self.files.iter().enumerate() {
            let id = index as FileId;
            let text = overlays
                .get(path)
                .cloned()
                .or_else(|| std::fs::read_to_string(path).ok())
                .unwrap_or_default();
            let hash = hash_text(&text);
            if let Some(record) = self.records.get_mut(id as usize) {
                record.hash = hash;
            }
            self.texts.insert(id, text);
        }
    }

    fn batches(&self, batch_size: usize) -> Vec<Vec<PathBuf>> {
        if self.overlays.is_empty() {
            return self
                .files
                .chunks(batch_size)
                .map(|batch| batch.to_vec())
                .collect();
        }
        let mut batches = Vec::new();
        let mut normal = Vec::new();
        for path in &self.files {
            if self.overlays.contains_key(path) {
                // A batch has one overlay buffer; only overlay files get their own batch.
                batches.push(vec![path.clone()]);
            } else {
                normal.push(path.clone());
            }
        }
        batches.extend(
            normal
                .chunks(batch_size)
                .map(|batch| batch.to_vec())
                .collect::<Vec<_>>(),
        );
        batches
    }

    fn finish(mut self, status: IndexStatus) -> IndexBuildResult {
        for (id, names) in self.pending_supertypes.drain(..) {
            let mut supertypes = Vec::new();
            for name in names {
                if let Some(target) = self.by_flat_name.get(&name).copied() {
                    supertypes.push(TypeRef::Project(target));
                    self.by_supertype.entry(target).or_default().push(id);
                } else {
                    supertypes.push(TypeRef::External(name));
                }
            }
            if let Some(record) = self.symbols.get_mut(id as usize) {
                record.supertypes = supertypes;
            }
        }
        let failed_files = self.failed_files.clone();
        let mut index = ProjectIndex {
            generation: 1,
            workspace: self.model.fingerprint(),
            strings: self.strings,
            symbols: self.symbols,
            by_key: self.by_key,
            by_name: self.by_name,
            by_file: self.by_file,
            by_supertype: self.by_supertype,
            occurrences: self.occurrences,
            dependents: HashMap::new(),
            files: self.records,
            status,
        };
        index.compute_dependents();
        IndexBuildResult {
            index,
            skipped_files: self.skipped_files,
            failed_files,
            occurrence_bytes: self.occurrence_bytes,
        }
    }

    fn build_batch(&mut self, batch: &[PathBuf]) {
        let batch_set: HashSet<PathBuf> = batch.iter().cloned().collect();
        let config = self.config.clone();
        let mut session = FileAnalysis::new(config);
        for path in batch {
            if let Some(text) = self.overlays.get(path) {
                session.texts.insert(path.clone(), text.clone());
                session.compiler.overlay_text = Some((path.clone(), text.clone()));
            }
        }
        let compiled = session.compiler.compile_files(batch);
        // AST name data for this batch: declaring trees and the parser name
        // tables published on the compilation units.
        self.symbol_trees.clear();
        self.name_positions.clear();
        {
            let ctx = &session.compiler.ctx;
            // Two passes: class declarations must win their symbol mapping; an
            // enum constant's `VariableDecl` can carry the enum's own symbol
            // as `sym`, otherwise the enum would resolve to the constant's
            // position.
            for pass in 0..2 {
                for index in 0..ctx.trees.len() {
                    let tree = TreeId::new(index as u32);
                    let symbol = match &ctx.tree(tree).kind {
                        TreeKind::ClassDecl {
                            sym: Some(symbol), ..
                        } if pass == 0 => *symbol,
                        TreeKind::MethodDecl {
                            sym: Some(symbol), ..
                        }
                        | TreeKind::VariableDecl {
                            sym: Some(symbol), ..
                        } if pass == 1 => *symbol,
                        _ => continue,
                    };
                    self.symbol_trees.entry(symbol).or_insert(tree);
                }
            }
            for index in 0..ctx.trees.len() {
                let tree = TreeId::new(index as u32);
                if let TreeKind::CompilationUnit {
                    sourcefile: Some(source),
                    name_positions,
                    ..
                } = &ctx.tree(tree).kind
                {
                    let path = PathBuf::from(canonical(Path::new(
                        &source.get_name().to_string_lossy(),
                    )));
                    if let Some(id) = self.file_ids.get(&path).copied() {
                        self.name_positions.insert(id, name_positions.clone());
                    }
                }
            }
        }
        self.javadocs = collect_javadocs(&mut session.compiler);
        let failed = compiled.is_err() || session.compiler.log.nerrors.get() > 0;
        if failed {
            for path in batch {
                self.failed_files.push(path.clone());
                if let Some(id) = self.file_ids.get(path).copied() {
                    if let Some(record) = self.records.get_mut(id as usize) {
                        record.status = FileStatus::Failed;
                    }
                }
            }
        } else {
            for path in batch {
                if let Some(id) = self.file_ids.get(path).copied() {
                    if let Some(record) = self.records.get_mut(id as usize) {
                        record.status = FileStatus::Indexed;
                    }
                }
            }
        }

        let mut local: HashMap<CompilerSymbolId, SymbolId> = HashMap::new();
        {
            let compiler = &mut session.compiler;
            let classes = compiler.syms.get_all_classes();
            for class in classes {
                let Some(file) = source_file(&compiler.ctx, class) else {
                    continue;
                };
                let path = PathBuf::from(canonical(&file));
                if !batch_set.contains(&path) {
                    continue;
                }
                self.index_class(&mut compiler.ctx, class, &mut local);
            }

            for index in 0..compiler.ctx.trees.len() {
                let unit = TreeId::new(index as u32);
                let source = match &compiler.ctx.tree(unit).kind {
                    TreeKind::CompilationUnit {
                        sourcefile: Some(source),
                        ..
                    } => source.get_name().to_string_lossy().to_string(),
                    _ => continue,
                };
                let path = PathBuf::from(canonical(Path::new(&source)));
                if !batch_set.contains(&path) {
                    continue;
                }
                let Some(file_id) = self.file_ids.get(&path).copied() else {
                    continue;
                };
                let text = self.texts.get(&file_id).cloned().unwrap_or_default();
                let mut collector = OccurrenceCollector::new(file_id, text);
                collector.scan(&mut compiler.ctx, Some(unit));
                let raw = std::mem::take(&mut collector.raw);
                for occurrence in raw {
                    self.add_compiler_occurrence(
                        &mut compiler.ctx,
                        &mut local,
                        file_id,
                        occurrence.symbol,
                        occurrence.position,
                        occurrence.role,
                    );
                }
            }
        }
    }

    fn index_class(
        &mut self,
        ctx: &mut CompilerContext,
        class: CompilerSymbolId,
        local: &mut HashMap<CompilerSymbolId, SymbolId>,
    ) {
        let Some(id) = self.resolve_symbol(ctx, class, local) else {
            return;
        };
        let mut seen = HashSet::new();
        self.index_members(ctx, class, local, &mut seen, id);
    }

    fn index_members(
        &mut self,
        ctx: &mut CompilerContext,
        class: CompilerSymbolId,
        local: &mut HashMap<CompilerSymbolId, SymbolId>,
        seen: &mut HashSet<CompilerSymbolId>,
        _class_id: SymbolId,
    ) {
        if !seen.insert(class) {
            return;
        }
        let Some(scope) = ctx.sym(class).members() else {
            return;
        };
        let members = ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
        for member in members {
            if super::extract::is_synthesised_member(ctx, class, member) {
                continue;
            }
            let is_class = matches!(ctx.sym(member).kind_data, SymbolKind::ClassSymbol { .. });
            self.resolve_symbol(ctx, member, local);
            if is_class {
                self.index_members(ctx, member, local, seen, 0);
            }
        }
    }

    fn resolve_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        symbol: CompilerSymbolId,
        local: &mut HashMap<CompilerSymbolId, SymbolId>,
    ) -> Option<SymbolId> {
        if let Some(id) = local.get(&symbol) {
            return Some(*id);
        }
        let source = source_file(ctx, symbol)?;
        let path = PathBuf::from(canonical(&source));
        let file = *self.file_ids.get(&path)?;
        let kind = symbol_kind(ctx, symbol)?;
        let name_string = ctx
            .sym(symbol)
            .name
            .to_java_string()
            .to_string_lossy()
            .to_string();
        let owner = self.owner_id(ctx, symbol, local);
        let signature = self.signature(ctx, symbol, kind, file);
        // A top-level type's simple name is not unique across packages; its
        // key uses the fully qualified flat name. Every other symbol is scoped
        // by its owner, so the simple name is enough.
        let key_name = if owner.is_none() && kind.is_type() {
            ctx.sym(symbol).flat_name().to_string()
        } else {
            name_string.clone()
        };
        let name = self.strings.intern(&key_name);
        let key = SymbolKey {
            owner,
            name,
            kind,
            signature,
        };
        if let Some(id) = self.by_key.get(&key).copied() {
            local.insert(symbol, id);
            return Some(id);
        }

        // AST name position: parser name table, then `decl_pos`, then the
        // declaring tree position.
        let pos = name_start_of(
            ctx,
            symbol,
            &self.symbol_trees,
            self.name_positions
                .get(&file)
                .and_then(|positions| positions.as_ref())
                .map(|positions| positions.as_ref()),
            self.texts.get(&file).map(String::as_str),
        );
        let end = pos.saturating_add(name_string.encode_utf16().count() as u32);
        let range = ByteRange { start: pos, end };
        let flat_name = match &ctx.sym(symbol).kind_data {
            SymbolKind::ClassSymbol { flatname, .. } => flatname.to_string(),
            _ => owner
                .and_then(|owner| self.symbols.get(owner as usize))
                .map(|record| format!("{}.{}", record.flat_name, name_string))
                .unwrap_or_else(|| name_string.clone()),
        };
        let signature_text = self.strings.get(signature).to_string();
        let detail = match &ctx.sym(symbol).kind_data {
            SymbolKind::VarSymbol { .. } => {
                let ty = ctx
                    .sym(symbol)
                    .ty
                    .map(|ty| crate::analysis::type_names::type_name(ctx, ty));
                match ty {
                    Some(ty) => format!("{ty} {name_string}"),
                    None => format!("{} {}", kind_name(kind), name_string),
                }
            }
            _ if matches!(
                kind,
                IndexSymbolKind::Method | IndexSymbolKind::Constructor
            ) =>
            {
                format!("{name_string}{signature_text}")
            }
            _ => format!("{} {}", kind_name(kind), name_string),
        };
        let return_type = if matches!(
            kind,
            IndexSymbolKind::Method | IndexSymbolKind::Constructor
        ) {
            ctx.sym(symbol)
                .ty
                .and_then(|ty| match &ctx.typ(ty).kind {
                    crate::code::type_::TypeKindData::MethodType { restype, .. } => *restype,
                    _ => None,
                })
                .map(|restype| {
                    crate::workspace::index::model::TypeRef::External(
                        crate::analysis::type_names::type_name(ctx, restype),
                    )
                })
        } else {
            None
        };
        let id = self.symbols.len() as SymbolId;
        self.symbols.push(SymbolRecord {
            key,
            kind,
            file,
            range,
            detail,
            modifiers: ctx.sym(symbol).flags_field as u32,
            flat_name: flat_name.clone(),
            supertypes: Vec::new(),
            return_type,
            javadoc: self.javadocs.get(&symbol).cloned(),
        });
        self.by_key.insert(key, id);
        self.by_name.entry(name).or_default().push(id);
        self.by_file.entry(file).or_default().declarations.push(id);
        if kind.is_type() {
            self.by_flat_name.insert(flat_name, id);
        }
        local.insert(symbol, id);
        // Fields/locals get their declaration range from the attributed tree; the
        // compiler's symbol pos is not the identifier for those symbols.
        if !matches!(
            kind,
            IndexSymbolKind::Field | IndexSymbolKind::Local | IndexSymbolKind::Parameter
        ) {
            self.add_occurrence(file, range, id, OccurrenceRole::Declaration);
        }

        if kind.is_type() {
            let supertypes = class_supertypes(ctx, symbol);
            if !supertypes.is_empty() {
                self.pending_supertypes.push((id, supertypes));
            }
        }
        Some(id)
    }

    fn owner_id(
        &mut self,
        ctx: &mut CompilerContext,
        symbol: CompilerSymbolId,
        local: &mut HashMap<CompilerSymbolId, SymbolId>,
    ) -> Option<SymbolId> {
        let owner = ctx.sym(symbol).owner?;
        match ctx.sym(owner).kind_data {
            SymbolKind::ClassSymbol { .. } | SymbolKind::MethodSymbol { .. } => {
                self.resolve_symbol(ctx, owner, local)
            }
            _ => None,
        }
    }

    fn signature(
        &mut self,
        ctx: &mut CompilerContext,
        symbol: CompilerSymbolId,
        kind: IndexSymbolKind,
        file: FileId,
    ) -> SignatureId {
        let mut signature = String::new();
        match kind {
            IndexSymbolKind::Method | IndexSymbolKind::Constructor => {
                signature.push('(');
                if let SymbolKind::MethodSymbol { params, .. } = &ctx.sym(symbol).kind_data {
                    for (index, param) in params.iter().enumerate() {
                        if index > 0 {
                            signature.push(',');
                        }
                        if let Some(ty) = ctx.sym(param).ty {
                            signature.push_str(&crate::analysis::type_name(ctx, ty));
                        } else {
                            signature.push('?');
                        }
                    }
                }
                signature.push(')');
            }
            IndexSymbolKind::Local | IndexSymbolKind::Parameter => {
                let position = ctx.sym(symbol).pos.get().max(0);
                signature = format!("{}:{}:{}", file, position, symbol.index());
            }
            _ => {}
        }
        self.strings.intern(&signature)
    }

    fn add_compiler_occurrence(
        &mut self,
        ctx: &mut CompilerContext,
        local: &mut HashMap<CompilerSymbolId, SymbolId>,
        file: FileId,
        symbol: CompilerSymbolId,
        position: i32,
        role: OccurrenceRole,
    ) {
        if position < 0 || self.occurrences_full {
            return;
        }
        let Some(id) = self.resolve_symbol(ctx, symbol, local) else {
            return;
        };
        let Some(record) = self.symbols.get(id as usize) else {
            return;
        };
        let name = record.flat_name.rsplit('.').next().unwrap_or("");
        let name_len = name.encode_utf16().count() as u32;
        // Declaration positions already point at the AST name (`decl_pos` for
        // variables; `name_start` for builder-added declarations).
        let start = position as u32;
        let range = ByteRange {
            start,
            end: start.saturating_add(name_len.max(1)),
        };
        if role == OccurrenceRole::Declaration {
            if let Some(record) = self.symbols.get_mut(id as usize) {
                record.range = range;
            }
        }
        self.add_occurrence(file, range, id, role);
    }

    fn add_occurrence(
        &mut self,
        file: FileId,
        range: ByteRange,
        symbol: SymbolId,
        role: OccurrenceRole,
    ) {
        let key = (file, range.start, range.end, symbol, role);
        if !self.occurrence_seen.insert(key) {
            return;
        }
        // `Occurrence` is 20 bytes today (two u32 offsets, two ids, role).
        let bytes = std::mem::size_of::<Occurrence>();
        if self.occurrence_bytes + bytes > self.limits.max_occurrence_bytes {
            self.occurrences_full = true;
            self.status_reason = Some("occurrence byte budget exhausted".to_string());
            return;
        }
        self.occurrence_bytes += bytes;
        let occurrence = Occurrence {
            file,
            range,
            symbol,
            role,
        };
        self.by_file
            .entry(file)
            .or_default()
            .occurrences
            .push(occurrence.clone());
        self.occurrences.entry(symbol).or_default().push(occurrence);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RawOccurrence {
    pub(crate) symbol: CompilerSymbolId,
    pub(crate) position: i32,
    pub(crate) role: OccurrenceRole,
    /// Explicit UTF-16 end for occurrences whose range is not the symbol's
    /// simple name (a package import covers the whole `java.util` path).
    pub(crate) end: Option<u32>,
}

pub(crate) struct OccurrenceCollector {
    file: FileId,
    text: String,
    pub(crate) raw: Vec<RawOccurrence>,
}

impl OccurrenceCollector {
    pub(crate) fn new(file: FileId, text: String) -> Self {
        Self {
            file,
            text,
            raw: Vec::new(),
        }
    }

    fn field_name_pos(&self, ctx: &CompilerContext, selected: TreeId, name: &str) -> Option<i32> {
        let start = ctx.tree(selected).pos.max(0) as usize;
        // `pos` is a UTF-16 offset; text slicing is byte based.
        let base = crate::analysis::positions::utf16_to_byte_snapped(&self.text, start);
        let tail = self.text.get(base..)?;
        let offset = tail.find(name)?;
        Some(crate::analysis::positions::byte_to_utf16(&self.text, base + offset) as i32)
    }

    fn record(&mut self, symbol: CompilerSymbolId, position: i32, role: OccurrenceRole) {
        if position >= 0 {
            self.raw.push(RawOccurrence {
                symbol,
                position,
                role,
                end: None,
            });
        }
    }

    fn record_span(
        &mut self,
        symbol: CompilerSymbolId,
        position: i32,
        end: u32,
        role: OccurrenceRole,
    ) {
        if position >= 0 {
            self.raw.push(RawOccurrence {
                symbol,
                position,
                role,
                end: Some(end),
            });
        }
    }
}

impl TreeScannerVisitor for OccurrenceCollector {
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, vartype, nameexpr, init, sym, decl_pos) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
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
            self.record(sym, decl_pos, OccurrenceRole::Declaration);
        }
        self.scan(ctx, mods);
        self.scan(ctx, vartype);
        self.scan(ctx, nameexpr);
        self.scan(ctx, init);
    }

    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        // Import prefix -> namespace; the imported type is recorded as an
        // Import occurrence (not a Reference) so semantic tokens do not
        // double count it. The range points at the imported name itself so
        // goto-definition on `import a.B` still resolves (the index looks
        // occurrences up by position).
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import {
                qualid: Some(qualid),
                ..
            } => *qualid,
            _ => return,
        };
        // `import java.util.*;` parses as FieldAccess(`java.util`, `*`): the
        // import-on-demand target is the selected expression.
        let qualid = match &ctx.tree(qualid).kind {
            TreeKind::FieldAccess {
                selected: Some(selected),
                name,
                ..
            } if name.to_java_string().to_string_lossy() == "*" => *selected,
            _ => qualid,
        };
        let symbol = match &ctx.tree(qualid).kind {
            TreeKind::Ident {
                sym: Some(symbol), ..
            }
            | TreeKind::FieldAccess {
                sym: Some(symbol), ..
            } => *symbol,
            _ => return,
        };
        // `import java.util.*;`: the whole package path is the prefix, so the
        // occurrence spans it instead of the last simple name. The select
        // node starts at its `.`, so walk back over the package path.
        if let SymbolKind::PackageSymbol { fullname, .. } = &ctx.sym(symbol).kind_data {
            let fullname = fullname.to_java_string().to_string_lossy().to_string();
            let length = fullname.encode_utf16().count() as u32;
            let last_length = fullname
                .rsplit('.')
                .next()
                .map(|segment| segment.encode_utf16().count() as u32)
                .unwrap_or(length);
            let mut start = ctx.tree(qualid).pos;
            if length > last_length {
                start -= (length - last_length - 1) as i32;
            }
            let start = start.max(0);
            self.record_span(symbol, start, start as u32 + length, OccurrenceRole::Import);
            return;
        }
        let position = match &ctx.tree(qualid).kind {
            TreeKind::Ident { .. } => ctx.tree(qualid).pos,
            TreeKind::FieldAccess {
                selected: Some(selected),
                name,
                ..
            } => {
                let name = name.to_java_string().to_string_lossy().to_string();
                let Some(position) = self.field_name_pos(&*ctx, *selected, &name) else {
                    return;
                };
                position
            }
            _ => return,
        };
        self.record(symbol, position, OccurrenceRole::Import);
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Ident { sym: Some(sym), .. } = &ctx.tree(tree).kind {
            self.record(*sym, ctx.tree(tree).pos, OccurrenceRole::Reference);
        }
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selected, name, sym) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess {
                selected,
                name,
                sym,
                ..
            } => (
                *selected,
                name.to_java_string().to_string_lossy().to_string(),
                *sym,
            ),
            _ => return,
        };
        if let (Some(selected), Some(sym)) = (selected, sym)
            && let Some(position) = self.field_name_pos(ctx, selected, &name)
        {
            self.record(sym, position, OccurrenceRole::Reference);
        }
        self.scan(ctx, selected);
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(OccurrenceCollector);

pub(crate) fn source_file(ctx: &CompilerContext, symbol: CompilerSymbolId) -> Option<PathBuf> {
    match &ctx.sym(symbol).kind_data {
        SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
            .as_ref()
            .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
        _ => ctx
            .sym(symbol)
            .owner
            .and_then(|owner| source_file(ctx, owner)),
    }
}

/// AST name position for a symbol: parser name table, then
/// `VariableDecl.decl_pos`, then the declaring tree position.
pub(crate) fn name_start_of(
    ctx: &CompilerContext,
    symbol: CompilerSymbolId,
    symbol_trees: &HashMap<CompilerSymbolId, TreeId>,
    name_positions: Option<&dyn crate::tree::end_pos_table::EndPosTable>,
    text: Option<&str>,
) -> u32 {
    if let Some(tree) = symbol_trees.get(&symbol) {
        let tree_pos = ctx.tree(*tree).pos.max(0) as usize;
        // AST-first order: parser-recorded name token, then the variable
        // declaration position, then a boundary-checked text match (the
        // declaration start can be a modifier such as `final`), then the tree.
        if let Some(table) = name_positions {
            let position = table.get_end_pos(*tree);
            if position > 0 {
                return position as u32;
            }
        }
        if let TreeKind::VariableDecl { decl_pos, .. } = &ctx.tree(*tree).kind
            && *decl_pos > 0
        {
            return *decl_pos as u32;
        }
        let name = ctx.sym(symbol).name.to_java_string().to_string_lossy();
        if !name.is_empty()
            && !name.starts_with('<')
            && let Some(text) = text
        {
            // `tree_pos` is a UTF-16 offset; `&str` slicing is byte based.
            let base = crate::analysis::positions::utf16_to_byte_snapped(text, tree_pos);
            if let Some(offset) = find_identifier(text, base, &name) {
                return crate::analysis::positions::byte_to_utf16(text, base + offset) as u32;
            }
        }
        return ctx.tree(*tree).pos.max(0) as u32;
    }
    ctx.sym(symbol).pos.get().max(0) as u32
}

/// Byte offset of `name` in `text[base..]` matched as a whole identifier, so a
/// declaration starting with a modifier (`final int f`) does not hit `final`.
fn find_identifier(text: &str, base: usize, name: &str) -> Option<usize> {
    let tail = text.get(base..)?;
    let is_name_char = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut from = 0;
    while let Some(found) = tail.get(from..)?.find(name) {
        let start = from + found;
        let end = start + name.len();
        let before = tail[..start].chars().next_back();
        let after = tail[end..].chars().next();
        if !before.is_some_and(is_name_char) && !after.is_some_and(is_name_char) {
            return Some(start);
        }
        from = end;
    }
    None
}

pub(crate) fn symbol_kind(ctx: &CompilerContext, symbol: CompilerSymbolId) -> Option<IndexSymbolKind> {
    match &ctx.sym(symbol).kind_data {
        SymbolKind::ClassSymbol { .. } => {
            let flags = ctx.sym(symbol).flags_field;
            Some(if flags & flags::ANNOTATION as i64 != 0 {
                IndexSymbolKind::Annotation
            } else if flags & flags::ENUM as i64 != 0 {
                IndexSymbolKind::Enum
            } else if flags & flags::INTERFACE as i64 != 0 {
                IndexSymbolKind::Interface
            } else if flags & flags::RECORD != 0 {
                IndexSymbolKind::Record
            } else {
                IndexSymbolKind::Class
            })
        }
        SymbolKind::MethodSymbol { .. } => {
            let name = ctx.sym(symbol).name.to_java_string().to_string_lossy();
            Some(if name == "<init>" {
                IndexSymbolKind::Constructor
            } else {
                IndexSymbolKind::Method
            })
        }
        SymbolKind::ParamSymbol => Some(IndexSymbolKind::Parameter),
        SymbolKind::VarSymbol { .. }
        | SymbolKind::BindingSymbol { .. }
        | SymbolKind::RecordComponent { .. } => {
            if ctx.sym(symbol).flags_field & flags::PARAMETER != 0 {
                return Some(IndexSymbolKind::Parameter);
            }
            if let Some(owner) = ctx.sym(symbol).owner {
                match ctx.sym(owner).kind_data {
                    SymbolKind::ClassSymbol { .. } => Some(IndexSymbolKind::Field),
                    SymbolKind::MethodSymbol { .. } => Some(IndexSymbolKind::Local),
                    _ => Some(IndexSymbolKind::Local),
                }
            } else {
                Some(IndexSymbolKind::Other)
            }
        }
        SymbolKind::TypeVariableSymbol => Some(IndexSymbolKind::TypeParameter),
        _ => None,
    }
}

pub(crate) fn class_supertypes(ctx: &CompilerContext, class: CompilerSymbolId) -> Vec<String> {
    let mut names = Vec::new();
    for index in 0..ctx.trees.len() {
        if let TreeKind::ClassDecl {
            sym: Some(sym),
            extending,
            implementing,
            ..
        } = &ctx.tree(TreeId::new(index as u32)).kind
            && *sym == class
        {
            for tree in extending.iter().copied().chain(implementing.iter()) {
                if let Some(ty) = ctx.tree(tree).ty
                    && let Some(tsym) = ctx.typ(ty).tsym
                {
                    names.push(ctx.sym(tsym).flat_name().to_string());
                }
            }
            break;
        }
    }
    names
}

pub(crate) fn kind_name(kind: IndexSymbolKind) -> &'static str {
    match kind {
        IndexSymbolKind::Class => "class",
        IndexSymbolKind::Interface => "interface",
        IndexSymbolKind::Enum => "enum",
        IndexSymbolKind::Annotation => "annotation",
        IndexSymbolKind::Record => "record",
        IndexSymbolKind::Field => "field",
        IndexSymbolKind::Method => "method",
        IndexSymbolKind::Constructor => "constructor",
        IndexSymbolKind::Local => "local",
        IndexSymbolKind::Parameter => "parameter",
        IndexSymbolKind::TypeParameter => "type parameter",
        IndexSymbolKind::Other => "symbol",
    }
}

pub(crate) fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Rendered Javadoc of every declaration in the compiled batch, keyed by the
/// compiler symbol id. Symbols outside the batch have no entry.
///
/// A compilation unit is created *after* its children in the tree arena, so
/// tree ids cannot delimit units; tables are associated by source file.
pub(crate) fn collect_javadocs(compiler: &mut MiniCompiler) -> HashMap<CompilerSymbolId, String> {
    let mut tables: HashMap<PathBuf, Rc<RefCell<dyn DocCommentTable>>> = HashMap::new();
    for index in 0..compiler.ctx.trees.len() {
        let tree_id = TreeId::new(index as u32);
        if let TreeKind::CompilationUnit {
            sourcefile: Some(source),
            doc_comments: Some(table),
            ..
        } = &compiler.ctx.tree(tree_id).kind
        {
            let key = PathBuf::from(canonical(Path::new(&source.get_name().to_string_lossy())));
            tables.insert(key, table.clone());
        }
    }
    if tables.is_empty() {
        return HashMap::new();
    }
    let mut docs = HashMap::new();
    for index in 0..compiler.ctx.trees.len() {
        let tree_id = TreeId::new(index as u32);
        let symbol = match &compiler.ctx.tree(tree_id).kind {
            TreeKind::ClassDecl { sym, .. }
            | TreeKind::MethodDecl { sym, .. }
            | TreeKind::VariableDecl { sym, .. } => *sym,
            _ => None,
        };
        let Some(symbol) = symbol else {
            continue;
        };
        let Some(source) = source_file(&compiler.ctx, symbol) else {
            continue;
        };
        let key = PathBuf::from(canonical(&source));
        let Some(table) = tables.get(&key).cloned() else {
            continue;
        };
        let rendered = {
            let mut table_ref = table.borrow_mut();
            table_ref
                .get_comment_tree(&mut compiler.ctx, tree_id)
                .and_then(|tree| crate::analysis::doc_render::format_doc_comment(&tree))
                .or_else(|| {
                    table_ref
                        .get_comment_text(tree_id)
                        .map(|text| text.to_string_lossy())
                })
        };
        if let Some(text) = rendered {
            docs.insert(symbol, text);
        }
    }
    docs
}

pub fn collect_java_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in roots {
        collect_java_files_in(root, &mut files);
    }
    files.sort();
    files.dedup();
    files
        .into_iter()
        .filter_map(|path| std::fs::canonicalize(&path).ok())
        .map(|path| PathBuf::from(canonical(&path)))
        .collect()
}

fn collect_java_files_in(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java_files_in(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "java") {
            files.push(path);
        }
    }
}

/// Canonical path used as the identity of a source file.
pub(crate) fn canonical(path: &Path) -> String {
    if let Ok(resolved) = std::fs::canonicalize(path) {
        return resolved.to_string_lossy().into_owned();
    }
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path).to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[allow(dead_code)]
fn _assert_hasher_is_stable() {
    // `DefaultHasher` is documented as not stable across releases; the disk
    // index therefore records the toolchain hash and rebuilds on mismatch.
    let _ = DefaultHasher::new();
}
