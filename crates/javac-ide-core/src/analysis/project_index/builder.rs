//! Build a ProjectIndex from a temporary compiler pass.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::code::flags;
use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId as CompilerSymbolId, TreeId};
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_scanner::TreeScannerVisitor;

use super::super::type_names::type_name;
use super::super::{canonical, AnalysisConfig, AnalysisSession};
use super::model::*;

/// Build a complete index from the project source roots.
pub fn build_project_index(config: &AnalysisConfig, batch_size: usize) -> ProjectIndex {
    IndexBuilder::new(config).build(batch_size)
}

pub fn build_project_index_with_overlays(
    config: &AnalysisConfig,
    batch_size: usize,
    overlays: &HashMap<PathBuf, String>,
) -> ProjectIndex {
    IndexBuilder::new_with_overlays(config, overlays).build(batch_size)
}

/// Build the workspace fingerprint without touching source files.
pub fn build_workspace_model(config: &AnalysisConfig) -> WorkspaceModel {
    WorkspaceModel::from_config(config)
}

pub struct IndexBuilder {
    config: AnalysisConfig,
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
    pending_supertypes: Vec<(SymbolId, Vec<String>)>,
    failed_files: Vec<PathBuf>,
    overlays: HashMap<PathBuf, String>,
    strings: StringInterner,
}

impl IndexBuilder {
    pub fn new(config: &AnalysisConfig) -> Self {
        Self::new_with_overlays(config, &HashMap::new())
    }

    pub fn new_with_overlays(
        config: &AnalysisConfig,
        overlays: &HashMap<PathBuf, String>,
    ) -> Self {
        let files = collect_java_files(&config.source_path);
        let mut file_ids = HashMap::new();
        let mut records = Vec::with_capacity(files.len());
        let mut texts = HashMap::new();
        for (index, path) in files.iter().enumerate() {
            let id = index as FileId;
            file_ids.insert(path.clone(), id);
            let text = overlays
                .get(path)
                .cloned()
                .or_else(|| std::fs::read_to_string(path).ok())
                .unwrap_or_default();
            let hash = hash_text(&text);
            records.push(FileRecord {
                path: path.clone(),
                hash,
                source_set: SourceSetKind::Main,
                failed: false,
            });
            texts.insert(id, text);
        }
        Self {
            config: config.clone(),
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
            pending_supertypes: Vec::new(),
            failed_files: Vec::new(),
            overlays: overlays.clone(),
            strings: StringInterner::default(),
        }
    }

    pub fn build(mut self, batch_size: usize) -> ProjectIndex {
        if self.files.is_empty() {
            return self.finish(IndexStatus::Ready);
        }
        let batch_size = batch_size.max(1);
        let batches: Vec<Vec<PathBuf>> = if self.overlays.is_empty() {
            self.files
                .chunks(batch_size)
                .map(|batch| batch.to_vec())
                .collect()
        } else {
            // overlay_text carries one buffer; compile each overlay file alone.
            self.files.iter().map(|path| vec![path.clone()]).collect()
        };
        for batch in batches {
            self.build_batch(&batch);
        }
        let status = if self.failed_files.is_empty() {
            IndexStatus::Ready
        } else {
            IndexStatus::Partial {
                failed_files: self.failed_files.clone(),
                reason: "some project files failed to compile".to_string(),
            }
        };
        self.finish(status)
    }

    fn finish(mut self, status: IndexStatus) -> ProjectIndex {
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
        ProjectIndex {
            generation: 1,
            strings: self.strings,
            symbols: self.symbols,
            by_key: self.by_key,
            by_name: self.by_name,
            by_file: self.by_file,
            by_supertype: self.by_supertype,
            occurrences: self.occurrences,
            files: self.records,
            status,
        }
    }

    fn build_batch(&mut self, batch: &[PathBuf]) {
        let batch_set: HashSet<PathBuf> = batch.iter().cloned().collect();
        let config = self.config.clone();
        let mut session = AnalysisSession::new(config);
        for path in batch {
            if let Some(text) = self.overlays.get(path) {
                session.texts.insert(path.clone(), text.clone());
                session.compiler.overlay_text = Some((path.clone(), text.clone()));
            }
        }
        let compiled = session.compiler.compile_files(batch);
        if compiled.is_err() {
            for path in batch {
                self.failed_files.push(path.clone());
                if let Some(id) = self.file_ids.get(path).copied() {
                    if let Some(record) = self.records.get_mut(id as usize) {
                        record.failed = true;
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
                let (source, defs) = match &compiler.ctx.tree(unit).kind {
                    TreeKind::CompilationUnit {
                        sourcefile: Some(source),
                        defs,
                        ..
                    } => (source.get_name().to_string_lossy().to_string(), defs.clone()),
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
                let _ = defs;
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
        let name = self.strings.intern(&name_string);
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

        let pos = ctx.sym(symbol).pos.get().max(0) as u32;
        let end = pos.saturating_add(name_string.len() as u32);
        let range = ByteRange { start: pos, end };
        let flat_name = match &ctx.sym(symbol).kind_data {
            SymbolKind::ClassSymbol { flatname, .. } => flatname.to_string(),
            _ => owner
                .and_then(|owner| self.symbols.get(owner as usize))
                .map(|record| format!("{}.{}", record.flat_name, name_string))
                .unwrap_or_else(|| name_string.clone()),
        };
        let detail = format!("{} {}", kind_name(kind), name_string);
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
            return_type: None,
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
                            signature.push_str(&type_name(ctx, ty));
                        } else {
                            signature.push('?');
                        }
                    }
                }
                signature.push(')');
            }
            IndexSymbolKind::Local | IndexSymbolKind::Parameter => {
                signature = format!("{}:{}", file, ctx.sym(symbol).pos.get());
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
        if position < 0 {
            return;
        }
        let Some(id) = self.resolve_symbol(ctx, symbol, local) else {
            return;
        };
        let Some(record) = self.symbols.get(id as usize) else {
            return;
        };
        let name_len = record
            .flat_name
            .rsplit('.')
            .next()
            .map(str::len)
            .unwrap_or(0) as u32;
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
struct RawOccurrence {
    symbol: CompilerSymbolId,
    position: i32,
    role: OccurrenceRole,
}

struct OccurrenceCollector {
    file: FileId,
    text: String,
    raw: Vec<RawOccurrence>,
}

impl OccurrenceCollector {
    fn new(file: FileId, text: String) -> Self {
        Self {
            file,
            text,
            raw: Vec::new(),
        }
    }

    fn field_name_pos(&self, ctx: &CompilerContext, selected: TreeId, name: &str) -> Option<i32> {
        let start = ctx.tree(selected).pos.max(0) as usize;
        let tail = self.text.get(start..)?;
        let offset = tail.find(name)?;
        Some((start + offset) as i32)
    }

    fn record(&mut self, symbol: CompilerSymbolId, position: i32, role: OccurrenceRole) {
        if position >= 0 {
            self.raw.push(RawOccurrence {
                symbol,
                position,
                role,
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

fn source_file(ctx: &CompilerContext, symbol: CompilerSymbolId) -> Option<PathBuf> {
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

fn symbol_kind(ctx: &CompilerContext, symbol: CompilerSymbolId) -> Option<IndexSymbolKind> {
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
        SymbolKind::VarSymbol { .. }
        | SymbolKind::BindingSymbol { .. }
        | SymbolKind::ParamSymbol
        | SymbolKind::RecordComponent { .. } => {
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

fn class_supertypes(ctx: &CompilerContext, class: CompilerSymbolId) -> Vec<String> {
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

fn kind_name(kind: IndexSymbolKind) -> &'static str {
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

fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn collect_java_files(roots: &[PathBuf]) -> Vec<PathBuf> {
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