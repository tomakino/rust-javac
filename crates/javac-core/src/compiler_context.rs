//! The compile-time object arena container of ADR 0001 (CompilerContext).
//!
//! PORTING_RULES: non-1:1 adapter file (Java mutable object graph -> arena + ID), see ADR 0001.
//! Six arenas: Symbol/Type/JCTree/Scope/Attribute/Directive; every *Id is valid only for the same
//! CompilerContext and is released with the compilation.
//!
//! Borrow rules (ADR 0001 rules 3-4; all components must follow them):
//! - CompilerContext exclusively owns the six arenas; component logic lives in functions/methods taking
//!   ctx: &mut CompilerContext (Java x.field -> ctx.sym(x).field);
//! - Rc<RefCell<...>> must not express the core object graph (except the PORTING_RULES Porting rules whitelist);
//! - when several arenas are needed at once, split the borrow by field: Rust borrow checking is per field,
//!   e.g. &mut ctx.scopes and &ctx.symbols can coexist;
//! - accessor references borrow &self: while a reference is held the same ctx must not be borrowed mutably;
//!   for read-then-write, copy out name/kind/IDs through accessors first and then release the borrow;
//! - arenas only grow (ADR 0001 rule 8); out-of-bounds access is asserted in util/arena.rs in debug builds.
//!
//! Scope lookup/iteration order is guaranteed by the algorithm in code/scope.rs (upstream open addressing double hashing +
//! elems/shadowed chains); this module only provides CompilerContext-based wrappers and no longer exposes
//! the former SymbolResolver/self_id transition (the methods pass arena slices to the Scope algorithm).

use crate::code::attribute::Attribute;
use crate::code::directive::Directive;
use crate::code::kinds;
use crate::code::scope::{probe_index, EntryId, LookupKind, Scope, ScopeData, TableSlot};
use std::rc::Rc;
use crate::code::symbol::Symbol;
use crate::code::type_::Type;
use crate::ids::{AttributeId, DirectiveId, ScopeId, SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::JCTree;
use crate::util::arena::Arena;
use crate::util::list::List;
use crate::util::name::Name;

/// The compilation context: the owner of the six arena containers (ADR 0001).
#[derive(Default)]
pub struct CompilerContext {
    pub symbols: Arena<Symbol>,
    pub types: Arena<Type>,
    pub trees: Arena<JCTree>,
    pub scopes: Arena<Scope>,
    pub attributes: Arena<Attribute>,
    pub directives: Arena<Directive>,
    /// The canonical Type.noType singleton (Java: Type.noType). The port keeps
    /// one JCNoType instance per compiler context so identity comparisons of
    /// enclosing types match the upstream singleton (registered adapter).
    no_type: Option<crate::ids::TypeId>,
}

impl CompilerContext {
    /// Creates an empty compilation context.
    pub fn new() -> CompilerContext {
        CompilerContext::default()
    }

    /// Allocates a symbol in the symbol arena and returns its ID.
    /// Allocates a symbol in the symbol arena and returns its ID. The unique
    /// allocation entry point also records the arena id on the symbol (the
    /// SymbolMetadata back-reference) and rebinds a copied metadata, if any.
    pub fn alloc_symbol(&mut self, sym: Symbol) -> SymbolId {
        let id = SymbolId::new(self.symbols.push(sym));
        let stored = self.symbols.get_mut(id.index());
        stored.id = std::cell::Cell::new(Some(id));
        if let Some(metadata) = &mut stored.metadata {
            metadata.rebind(id);
        }
        id
    }

    /// Allocates a type in the type arena and returns its ID.
    pub fn alloc_type(&mut self, ty: Type) -> TypeId {
        TypeId::new(self.types.push(ty))
    }

    /// The canonical missing type, Type.noType (a per-context singleton).
    pub fn no_type_id(&mut self) -> TypeId {
        if let Some(no_type) = self.no_type {
            return no_type;
        }
        let no_type = self.alloc_type(Type::new(
            crate::code::type_tag::TypeTag::NONE,
            None,
            crate::code::type_::TypeKindData::JCNoType {
                kind: crate::code::type_::JCNoTypeKind::NoType,
            },
        ));
        self.no_type = Some(no_type);
        no_type
    }

    /// Allocates a tree node in the tree arena and returns its ID.
    pub fn alloc_tree(&mut self, tree: JCTree) -> TreeId {
        TreeId::new(self.trees.push(tree))
    }

    /// Allocates a scope in the scope arena and returns its ID.
    pub fn alloc_scope(&mut self, scope: Scope) -> ScopeId {
        ScopeId::new(self.scopes.push(scope))
    }

    /// Allocates an attribute in the attribute arena and returns its ID.
    pub fn alloc_attribute(&mut self, attribute: Attribute) -> AttributeId {
        AttributeId::new(self.attributes.push(attribute))
    }

    /// Allocates a directive in the directive arena and returns its ID.
    pub fn alloc_directive(&mut self, directive: Directive) -> DirectiveId {
        DirectiveId::new(self.directives.push(directive))
    }

    /// Java: ctx.sym(id) (ADR 0001 rule 3).
    pub fn sym(&self, id: SymbolId) -> &Symbol {
        self.symbols.get(id.index())
    }

    /// Java: ctx.sym_mut(id).
    pub fn sym_mut(&mut self, id: SymbolId) -> &mut Symbol {
        self.symbols.get_mut(id.index())
    }

    /// Java: ctx.typ(id).
    pub fn typ(&self, id: TypeId) -> &Type {
        self.types.get(id.index())
    }

    /// Java: ctx.typ_mut(id).
    pub fn typ_mut(&mut self, id: TypeId) -> &mut Type {
        self.types.get_mut(id.index())
    }

    /// Java: ctx.tree(id).
    pub fn tree(&self, id: TreeId) -> &JCTree {
        self.trees.get(id.index())
    }

    /// Java: ctx.tree_mut(id).
    pub fn tree_mut(&mut self, id: TreeId) -> &mut JCTree {
        self.trees.get_mut(id.index())
    }

    /// The scope arena (immutable view for the recursive Scope queries).
    pub fn scopes_arena(&self) -> &Arena<Scope> {
        &self.scopes
    }

    /// The symbol arena (immutable view for the recursive Scope queries).
    pub fn symbols_arena(&self) -> &Arena<crate::code::symbol::Symbol> {
        &self.symbols
    }

    /// Java: ctx.scope(id).
    pub fn scope(&self, id: ScopeId) -> &Scope {
        self.scopes.get(id.index())
    }

    /// Java: ctx.scope_mut(id).
    pub fn scope_mut(&mut self, id: ScopeId) -> &mut Scope {
        self.scopes.get_mut(id.index())
    }

    /// Java: Symbol.initedMetadata() — creates the SymbolMetadata bound to
    /// the symbol arena id (the local Symbol does not carry its own id).
    pub fn symbol_inited_metadata(
        &mut self,
        symbol: crate::ids::SymbolId,
    ) -> &mut crate::code::symbol_metadata::SymbolMetadata {
        if self.sym(symbol).metadata.is_none() {
            self.sym_mut(symbol).metadata = Some(crate::code::symbol_metadata::SymbolMetadata::new(symbol));
        }
        self.sym_mut(symbol).metadata.as_mut().expect("metadata")
    }

    /// Java: Symbol.getRawAttributes() through the arena id.
    pub fn symbol_get_raw_attributes(
        &self,
        symbol: crate::ids::SymbolId,
    ) -> crate::util::list::List<crate::ids::AttributeId> {
        self.sym(symbol).get_raw_attributes()
    }

    /// Java: Type.constType(Object constValue) — allocates the constant type.
    pub fn type_const_type(
        &mut self,
        t: crate::ids::TypeId,
        value: crate::util::constants::ConstantValue,
    ) -> crate::ids::TypeId {
        let constant = self.typ(t).const_type(value);
        self.alloc_type(constant)
    }

    /// Java: Type.constValue().
    pub fn type_const_value(
        &self,
        t: crate::ids::TypeId,
    ) -> Option<&crate::util::constants::ConstantValue> {
        self.typ(t).const_value()
    }

    /// Java: Type.baseType() — a constant type returns its tsym.type, every
    /// other type returns itself.
    pub fn type_base_type(&self, t: crate::ids::TypeId) -> crate::ids::TypeId {
        let ty = self.typ(t);
        if ty.const_value().is_some() {
            ty.tsym
                .and_then(|tsym| self.sym(tsym).ty)
                .unwrap_or(t)
        } else {
            t
        }
    }

    /// Java: ctx.attr(id).
    pub fn attr(&self, id: AttributeId) -> &Attribute {
        self.attributes.get(id.index())
    }

    /// Java: ctx.attr_mut(id).
    pub fn attr_mut(&mut self, id: AttributeId) -> &mut Attribute {
        self.attributes.get_mut(id.index())
    }

    /// Java: ctx.directive(id).
    pub fn directive(&self, id: DirectiveId) -> &Directive {
        self.directives.get(id.index())
    }

    /// Java: ctx.directive_mut(id).
    pub fn directive_mut(&mut self, id: DirectiveId) -> &mut Directive {
        self.directives.get_mut(id.index())
    }

    /// Reads the name of a symbol (upstream: sym.name).
    pub fn symbol_name(&self, id: SymbolId) -> Name {
        self.sym(id).name.clone()
    }

    /// Reads the kind of a symbol (upstream: sym.kind).
    pub fn symbol_kind(&self, id: SymbolId) -> kinds::Kind {
        self.sym(id).kind
    }

    /// Java: Scope.WriteableScope.enter(Symbol) (replaces the old
    /// SymbolResolver/self_id transition; the scope algorithm is unchanged).
    pub fn scope_enter(&mut self, scope: ScopeId, sym: SymbolId) {
        self.scopes
            .get_mut(scope.index())
            .enter(scope, sym, &self.symbols);
    }

    /// Java: Scope.WriteableScope.enterIfAbsent(Symbol)
    pub fn scope_enter_if_absent(&mut self, scope: ScopeId, sym: SymbolId) {
        self.scopes
            .get_mut(scope.index())
            .enter_if_absent(scope, sym, &self.symbols);
    }

    /// Java: Scope.WriteableScope.remove(Symbol)
    pub fn scope_remove(&mut self, scope: ScopeId, sym: SymbolId) {
        self.scopes
            .get_mut(scope.index())
            .remove(scope, sym, &self.symbols);
    }

    /// Java: Scope.lookup(Name) (returns the entry ID).
    pub fn scope_lookup(&self, scope: ScopeId, name: &Name) -> Option<EntryId> {
        self.scope(scope).lookup(name, &self.symbols)
    }

    /// Java: Scope.findFirst(Name, Predicate<Symbol>)
    pub fn scope_find_first(
        &self,
        scope: ScopeId,
        name: &Name,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Option<SymbolId> {
        self.scope(scope)
            .find_first(name, filter, &self.symbols)
    }

    /// Java: Scope.WriteableScope.includes(Symbol)
    pub fn scope_includes(&self, scope: ScopeId, sym: SymbolId) -> bool {
        self.scope(scope).includes(scope, sym, &self.symbols)
    }

    /// Java: Scope.getSymbols(Predicate<Symbol>, LookupKind) — all scope
    /// variants (ScopeImpl recursive next-chain, Compound/Import/NamedImport/
    /// StarImport/SingleEntry/FilterImport); the scope snapshot is cloned so the
    /// map lookup and the mutable context do not borrow simultaneously.
    pub fn scope_get_symbols(
        &mut self,
        scope: ScopeId,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Vec<SymbolId> {
        let snapshot = self.scope(scope).clone();
        snapshot.get_symbols(scope, lookup_kind, filter, self)
    }

    /// Java: Scope.getSymbolsByName(Name, Predicate<Symbol>, LookupKind)
    pub fn scope_get_symbols_by_name(
        &mut self,
        scope: ScopeId,
        name: &Name,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Vec<SymbolId> {
        let snapshot = self.scope(scope).clone();
        snapshot.get_symbols_by_name(scope, name, lookup_kind, filter, self)
    }

    /// Java: Scope.getSymbolsByName(Name, Predicate<Symbol>, LookupKind) with
    /// the Types service needed by the import filter (the FilterImportScope
    /// holder adapter; see the Scope import filter registration).
    pub fn scope_get_symbols_by_name_full(
        &mut self,
        scope: ScopeId,
        name: &Name,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
    ) -> Vec<SymbolId> {
        let snapshot = self.scope(scope).clone();
        snapshot.get_symbols_by_name_full(scope, name, lookup_kind, filter, types, syms, self)
    }

    /// Prototype of the context-level scope query (the S2d borrow split): the
    /// query takes the mutable context together with the read-only arena
    /// snapshots (cloned per scope) and the Types/Symtab services, instead of a
    /// separately borrowed symbols-arena argument. Java: the Scope query reads
    /// the Context services (Scope.java:908-1016).
    ///
    /// # Arguments
    ///   - scope: the scope to query;
    ///   - lookup_kind: the lookup kind;
    ///   - filter: the symbol predicate;
    ///   - types/syms: the shared type/symbol services (import filter);
    ///   - ctx: the compiler context.
    ///
    /// # Returns
    ///   - the matching symbols.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Scope.getSymbols (Scope.java:550-620) with the FilterImportScope
    /// dispatch of Scope.java:932-952.
    pub fn scope_get_symbols_ctx(
        &mut self,
        scope: ScopeId,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
    ) -> Vec<SymbolId> {
        let snapshot = self.scope(scope).clone();
        snapshot.get_symbols_full(scope, lookup_kind, filter, types, syms, self)
    }

    /// The by-name counterpart of scope_get_symbols_ctx (prototype of the S2d
    /// borrow split). Java: Scope.getSymbolsByName(Name, Predicate, LookupKind)
    /// (Scope.java:620-700) with the FilterImportScope dispatch of
    /// Scope.java:954-976.
    ///
    /// # Arguments
    ///   - scope: the scope to query;
    ///   - name: the symbol name;
    ///   - lookup_kind: the lookup kind;
    ///   - filter: the symbol predicate;
    ///   - types/syms: the shared type/symbol services (import filter);
    ///   - ctx: the compiler context.
    ///
    /// # Returns
    ///   - the matching symbols.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Scope.getSymbolsByName (Scope.java:620-700).
    pub fn scope_get_symbols_by_name_ctx(
        &mut self,
        scope: ScopeId,
        name: &Name,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
    ) -> Vec<SymbolId> {
        let snapshot = self.scope(scope).clone();
        snapshot.get_symbols_by_name_full(scope, name, lookup_kind, filter, types, syms, self)
    }

    /// Java: Scope.isEmpty()
    pub fn scope_is_empty(&self, scope: ScopeId) -> bool {
        self.scope(scope).is_empty()
    }

    /// Java: ScopeImpl.anyMatch(Predicate<Symbol>)
    /// Java: WriteableScope.create(owner) (Scope.WriteableScope.create/dups).
    /// Creates a fresh writable scope and returns its arena ID.
    pub fn scope_create(&mut self, owner: Option<SymbolId>) -> ScopeId {
        self.alloc_scope(Scope::new_scope_impl(owner))
    }

    /// Java: new Scope.ErrorScope(owner) (members scope placeholder used while a
    /// class is being completed).
    pub fn scope_create_error(&mut self, owner: Option<SymbolId>) -> ScopeId {
        self.alloc_scope(Scope::new_error_scope(owner))
    }

    // ------------------------------------------------------------------
    // Java: Scope.CompoundScope/ImportScope/NamedImportScope/StarImportScope
    // (TypeEnter/ImportsPhase; the Scope import queries are registered
    // TODO for the Scope query).
    // ------------------------------------------------------------------

    /// Java: CompoundScope.prependSubScope(Scope that)
    pub fn scope_prepend_sub_scope(&mut self, parent: ScopeId, child: ScopeId) {
        let mark = sub_scope_fields_mut(self.scope_mut(parent));
        let Some((sub_scopes, mark)) = mark else {
            panic!("ClassCastException: not a CompoundScope");
        };
        *sub_scopes = sub_scopes.prepend(child);
        *mark += 1;
    }

    /// Java: CompoundScope.appendSubScope(Scope that)
    pub fn scope_append_sub_scope(&mut self, parent: ScopeId, child: ScopeId) {
        let mark = sub_scope_fields_mut(self.scope_mut(parent));
        let Some((sub_scopes, mark)) = mark else {
            panic!("ClassCastException: not a CompoundScope");
        };
        *sub_scopes = sub_scopes.append(child);
        *mark += 1;
    }

    /// Java: CompoundScope.getMark()
    pub fn scope_get_mark(&self, scope: ScopeId) -> i32 {
        match &self.scope(scope).data {
            crate::code::scope::ScopeData::CompoundScope { mark, .. }
            | crate::code::scope::ScopeData::ImportScope { mark, .. }
            | crate::code::scope::ScopeData::NamedImportScope { mark, .. }
            | crate::code::scope::ScopeData::StarImportScope { mark, .. } => *mark,
            _ => panic!("ClassCastException: not a CompoundScope"),
        }
    }

    /// Java: StarImportScope.isFilled() (subScopes.nonEmpty())
    pub fn scope_is_filled(&self, scope: ScopeId) -> bool {
        match &self.scope(scope).data {
            crate::code::scope::ScopeData::CompoundScope { sub_scopes, .. }
            | crate::code::scope::ScopeData::ImportScope { sub_scopes, .. }
            | crate::code::scope::ScopeData::NamedImportScope { sub_scopes, .. }
            | crate::code::scope::ScopeData::StarImportScope { sub_scopes, .. } => {
                !sub_scopes.is_empty()
            }
            _ => panic!("ClassCastException: not a CompoundScope"),
        }
    }

    /// Java: NamedImportScope.importType(Scope delegate, Scope origin, Symbol sym)
    pub fn scope_import_type(
        &mut self,
        named_scope: ScopeId,
        delegate: ScopeId,
        origin: ScopeId,
        sym: SymbolId,
    ) -> ScopeId {
        let name = self.sym(sym).name.clone();
        let owner = self.scope(delegate).owner;
        let new_scope = self.alloc_scope(crate::code::scope::Scope::new_single_entry_scope(
            owner,
            Some(sym),
            Some(origin),
        ));
        self.named_import_append(named_scope, new_scope, name);
        new_scope
    }

    /// Java: NamedImportScope.importByName(Types types, Scope origin, Name name,
    /// ImportFilter filter, JCImport imp, BiConsumer cfHandler)
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    pub fn scope_import_by_name(
        &mut self,
        named_scope: ScopeId,
        origin: ScopeId,
        name: Name,
        filter: Option<crate::code::scope::FilterImportFn>,
        imp: Option<TreeId>,
        cf_handler: Option<Rc<dyn Fn(TreeId, &crate::code::symbol::CompletionFailure)>>,
        static_import: bool,
        types: Rc<crate::code::types::Types>,
    ) -> ScopeId {
        let owner = self.scope(origin).owner;
        let new_scope = self.alloc_scope(
            crate::code::scope::Scope::new_filter_import_scope(
                owner,
                origin,
                Some(name.clone()),
                filter,
                imp,
                static_import,
                cf_handler,
                types,
            ),
        );
        self.named_import_append(named_scope, new_scope, name);
        new_scope
    }

    fn named_import_append(&mut self, named_scope: ScopeId, new_scope: ScopeId, name: Name) {
        self.scope_append_sub_scope(named_scope, new_scope);
        if let crate::code::scope::ScopeData::NamedImportScope { name2_scopes, .. } =
            &mut self.scope_mut(named_scope).data
        {
            name2_scopes
                .entry(name)
                .or_default()
                .push(new_scope);
        }
    }

    /// Java: StarImportScope.importAll(Types types, Scope origin, ImportFilter
    /// filter, JCImport imp, BiConsumer cfHandler)
    #[allow(clippy::type_complexity)]
    pub fn scope_import_all(
        &mut self,
        star_scope: ScopeId,
        origin: ScopeId,
        filter: Option<crate::code::scope::FilterImportFn>,
        imp: Option<TreeId>,
        cf_handler: Option<Rc<dyn Fn(TreeId, &crate::code::symbol::CompletionFailure)>>,
        static_import: bool,
        types: Rc<crate::code::types::Types>,
    ) {
        // avoid entering the same scope twice
        let duplicates = match &self.scope(star_scope).data {
            crate::code::scope::ScopeData::StarImportScope { sub_scopes, .. } => {
                sub_scopes.clone()
            }
            _ => panic!("ClassCastException: not a StarImportScope"),
        };
        let mut current = duplicates;
        while current.non_empty() {
            let existing = *current.head();
            if let crate::code::scope::ScopeData::FilterImportScope {
                origin: existing_origin,
                filter: existing_filter,
                static_import: existing_static,
                ..
            } = &self.scope(existing).data
            {
                let same_filter = match (&existing_filter, &filter) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                    _ => false,
                };
                if *existing_origin == origin && same_filter && *existing_static == static_import {
                    return;
                }
            }
            current = current.tail();
        }
        let owner = self.scope(origin).owner;
        let new_scope = self.alloc_scope(
            crate::code::scope::Scope::new_filter_import_scope(
                owner,
                origin,
                None,
                filter,
                imp,
                static_import,
                cf_handler,
                types,
            ),
        );
        self.scope_prepend_sub_scope(star_scope, new_scope);
    }

    /// Java: ImportScope.finalizeScope() (Scope.java:749-785) and
    /// NamedImportScope.finalizeScope() (Scope.java:830-844).
    ///
    /// # Arguments
    ///   - scope: the import scope;
    ///   - types: the type relations (for the FilterImportScope query);
    ///   - syms: the symbol table (for the FilterImportScope query).
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    #[allow(clippy::mutable_key_type)]
    pub fn scope_finalize(
        &mut self,
        scope: ScopeId,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
    ) {
        let (sub_scopes, _name2_scopes) = match &self.scope(scope).data {
            crate::code::scope::ScopeData::ImportScope { sub_scopes, .. } => {
                (sub_scopes.clone(), None)
            }
            crate::code::scope::ScopeData::NamedImportScope {
                sub_scopes,
                name2_scopes,
                ..
            } => (sub_scopes.clone(), Some(name2_scopes.clone())),
            crate::code::scope::ScopeData::StarImportScope { sub_scopes, .. } => {
                (sub_scopes.clone(), None)
            }
            _ => return,
        };
        let mut finalized: Vec<(ScopeId, ScopeId)> = Vec::new();
        let mut out: Vec<ScopeId> = Vec::new();
        for sub in sub_scopes.iter() {
            let result = self.finalize_single_scope(sub, types, syms);
            finalized.push((sub, result));
            out.push(result);
        }
        let new_sub_scopes = List::from_iterable(out);
        match &mut self.scope_mut(scope).data {
            crate::code::scope::ScopeData::ImportScope { sub_scopes, .. } => {
                *sub_scopes = new_sub_scopes;
            }
            crate::code::scope::ScopeData::NamedImportScope {
                sub_scopes,
                name2_scopes,
                ..
            } => {
                *sub_scopes = new_sub_scopes;
                let mut updated = std::collections::HashMap::new();
                for (name, scopes) in name2_scopes.iter() {
                    let remapped: Vec<ScopeId> = scopes
                        .iter()
                        .map(|candidate| {
                            finalized
                                .iter()
                                .find(|(old, _)| old == candidate)
                                .map(|(_, new)| *new)
                                .unwrap_or(*candidate)
                        })
                        .collect();
                    updated.insert(name.clone(), remapped);
                }
                *name2_scopes = updated;
            }
            crate::code::scope::ScopeData::StarImportScope { sub_scopes, .. } => {
                *sub_scopes = new_sub_scopes;
            }
            _ => {}
        }
    }

    /// Java: ImportScope.finalizeSingleScope(Scope) (Scope.java:760-785).
    fn finalize_single_scope(
        &mut self,
        scope: ScopeId,
        types: &crate::code::types::Types,
        syms: &crate::code::symtab::Symtab,
    ) -> ScopeId {
        let (owner, statically_imported) = match &self.scope(scope).data {
            crate::code::scope::ScopeData::FilterImportScope {
                static_import, ..
            } => (self.scope(scope).owner, *static_import),
            _ => return scope,
        };
        if !statically_imported {
            return scope;
        }
        let Some(owner) = owner else {
            return scope;
        };
        if self.sym(owner).kind != crate::code::kinds::Kind::TYP {
            return scope;
        }
        let snapshot = self.scope(scope).clone();
        let symbols = snapshot.get_symbols_full(
            scope,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            types,
            syms,
            self,
        );
        let finalized = self.scope_create(Some(owner));
        for sym in symbols {
            self.scope_enter(finalized, sym);
        }
        finalized
    }

    /// Java: Symbol.complete() — the arena-value adapter: the completer is taken
    /// from the symbol before calling it (the completer receives &mut context and
    /// the symbol table, ADR 0001).
    pub fn complete_symbol(
        &mut self,
        syms: &crate::code::symtab::Symtab,
        symbol: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let completer = self.sym_mut(symbol).completer.take();
        if let Some(completer) = completer {
            completer.borrow_mut().complete(self, syms, symbol)?;
        }
        Ok(())
    }

    /// Java: Symbol.ModuleSymbol.completeUsesProvides() (Symbol.java:1050-1056).
    ///
    /// # Arguments
    ///   - syms: the symbol table;
    ///   - symbol: the module symbol.
    ///
    /// # Returns
    ///   - the completion result.
    ///
    /// # Panics
    ///   - none.
    pub fn complete_uses_provides(
        &mut self,
        syms: &crate::code::symtab::Symtab,
        symbol: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let completer = match &mut self.sym_mut(symbol).kind_data {
            crate::code::symbol::SymbolKind::ModuleSymbol {
                uses_provides_completer,
                ..
            } => uses_provides_completer.take(),
            _ => None,
        };
        if let Some(completer) = completer {
            completer.borrow_mut().complete(self, syms, symbol)?;
        }
        Ok(())
    }

    /// Java: ClassSymbol.complete() — error recovery installs the error type and
    /// the public/static flags, then notifies the DeferredCompletionFailureHandler.
    pub fn complete_class_symbol(
        &mut self,
        syms: &crate::code::symtab::Symtab,
        symbol: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let orig_completer = self.sym(symbol).completer.clone();
        match self.complete_symbol(syms, symbol) {
            Ok(()) => Ok(()),
            Err(failure) => {
                if let (Some(dcfh), Some(orig)) = (failure.dcfh.clone(), orig_completer) {
                    dcfh.class_symbol_complete_failed(self, symbol, orig);
                }
                // quiet error recovery
                self.sym_mut(symbol).flags_field |= (crate::code::flags::PUBLIC
                    | crate::code::flags::STATIC) as i64;
                self.sym_mut(symbol).ty =
                    Some(crate::code::symtab::new_error_type_for_class(self, symbol));
                Err(failure)
            }
        }
    }

    /// Java: Symbol.isCompleted() (Symbol.java:232-234): a symbol is completed
    /// when its completer is terminal (the ported None value is
    /// Completer.NULL_COMPLETER).
    ///
    /// # Arguments
    ///   - symbol: the symbol to inspect.
    ///
    /// # Returns
    ///   - true when the completer is terminal.
    ///
    /// # Panics
    ///   - none.
    pub fn is_symbol_completed(&self, symbol: SymbolId) -> bool {
        match &self.sym(symbol).completer {
            None => true,
            Some(completer) => completer.borrow().is_terminal(),
        }
    }

    /// Java: Symbol.apiComplete() (Symbol.java:686-692): complete the symbol and
    /// route a CompletionFailure through the DeferredCompletionFailureHandler.
    ///
    /// # Arguments
    ///   - syms: the symbol table;
    ///   - symbol: the symbol to complete.
    ///
    /// # Returns
    ///   - Ok(()) when complete() succeeded or the handler absorbed the failure.
    ///
    /// # Panics
    ///   - none.
    pub fn api_complete_symbol(
        &mut self,
        syms: &crate::code::symtab::Symtab,
        symbol: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        match self.complete_symbol(syms, symbol) {
            Ok(()) => Ok(()),
            Err(failure) => match failure.dcfh.clone() {
                Some(dcfh) => dcfh.handle_api_completion_failure(failure),
                // Java always has a dcfh here; when the ported failure carries
                // none, rethrowing preserves the default JavacCodeHandler rule.
                None => Err(failure),
            },
        }
    }

    /// Java: PackageSymbol.getRawAttributes() (Symbol.java:1191-1198) and the
    /// private PackageSymbol.mergeAttributes() (Symbol.java:1200-1206).
    ///
    /// The upstream overrides first complete the package and its package-info
    /// class; the arena port performs those completion steps here because a
    /// symbol cannot complete itself without the explicit Symtab. The attributes
    /// of package-info are then merged once, exactly when the package has no
    /// metadata yet.
    ///
    /// # Arguments
    ///   - syms: the symbol table used by the completion steps;
    ///   - package: the package symbol.
    ///
    /// # Returns
    ///   - the package's raw attributes, including the merged package-info
    ///     declaration attributes (empty when the package has none).
    ///
    /// # Panics
    ///   - none.
    pub fn package_get_raw_attributes(
        &mut self,
        syms: &crate::code::symtab::Symtab,
        package: SymbolId,
    ) -> Result<List<AttributeId>, crate::code::symbol::CompletionFailure> {
        self.complete_symbol(syms, package)?;
        let package_info = match &self.sym(package).kind_data {
            crate::code::symbol::SymbolKind::PackageSymbol { package_info, .. } => *package_info,
            _ => None,
        };
        if let Some(package_info) = package_info {
            self.complete_symbol(syms, package_info)?;
            let merged = if self.sym(package).metadata.is_none() {
                let other = self.sym(package_info).metadata.as_ref();
                other.map(|other| {
                    let mut metadata =
                        crate::code::symbol_metadata::SymbolMetadata::new(package);
                    metadata.set_attributes(self, other);
                    metadata
                })
            } else {
                None
            };
            if let Some(metadata) = merged {
                self.sym_mut(package).metadata = Some(metadata);
            }
        }
        Ok(self.sym(package).get_raw_attributes())
    }

    /// Java: VarSymbol.getConstValue() (Symbol.java:1759-1775), including the
    /// lazy-initializer evaluation.
    ///
    /// The upstream method evaluates the captured Callable through Attr; the
    /// arena port stores the captured environments in SymbolKind::VarSymbol and
    /// receives the Attr service here (registered adapter). The value is cached
    /// in VarSymbol.data after the first evaluation, exactly like upstream.
    ///
    /// # Arguments
    ///   - syms: the symbol table used by attribution;
    ///   - symbol: the variable symbol;
    ///   - attr: the attribution service that evaluates the initializer.
    ///
    /// # Returns
    ///   - the constant value, or None for non-constant variables and for
    ///     exception/resource parameters.
    ///
    /// # Panics
    ///   - none.
    pub fn symbol_get_const_value(
        &mut self,
        syms: &crate::code::symtab::Symtab,
        symbol: SymbolId,
        attr: &mut crate::comp::attr::Attr,
    ) -> Option<crate::util::constants::ConstantValue> {
        let lazy = self.sym_mut(symbol).take_lazy_const_value();
        if let Some(lazy) = lazy {
            let value = attr.attrib_lazy_constant_value(
                self,
                syms,
                &lazy.env,
                &lazy.enclosing_env,
                lazy.variable,
                lazy.ty,
            );
            self.sym_mut(symbol).set_data(value);
        }
        self.sym(symbol).get_const_value()
    }

    /// Java: Symbol.reset() (ModuleSymbol/ClassSymbol overrides; see Symbol::reset).
    pub fn reset_symbol(&mut self, symbol: SymbolId) {
        self.sym_mut(symbol).reset();
    }

    /// Java: scope.dup(newOwner) — shares the hash table with the given scope.
    pub fn scope_dup(&mut self, scope: ScopeId, new_owner: Option<SymbolId>) -> ScopeId {
        let new_scope = self
            .scopes
            .get_mut(scope.index())
            .dup(scope, new_owner);
        ScopeId::new(self.scopes.push(new_scope))
    }

    /// Java: scope.dupUnshared(newOwner) — a new hash table; no sharing.
    pub fn scope_dup_unshared(&mut self, scope: ScopeId, new_owner: Option<SymbolId>) -> ScopeId {
        let new_scope = {
            let scopes = &self.scopes;
            scopes
                .get(scope.index())
                .dup_unshared(scope, new_owner, scopes)
        };
        ScopeId::new(self.scopes.push(new_scope))
    }

    /// Java: scope.leave() — removes this scope's entries from the shared table
    /// and returns the enclosing scope (the upstream callers reassign
    /// typevars = typevars.leave()).
    pub fn scope_leave(&mut self, scope: ScopeId) -> ScopeId {
        // Read the data needed from the leaving scope (clone the shared Rc
        // handles first; borrow rules: no RefCell borrow across members).
        let (next_id, table, entries, elems, nelems, has_symbols) = {
            let leaving = self.scopes.get(scope.index());
            let ScopeData::ScopeImpl { next, table, entries, elems, nelems, .. } =
                &leaving.data
            else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            (
                *next,
                Rc::clone(table),
                Rc::clone(entries),
                *elems,
                *nelems,
                true,
            )
        };
        let next_id = next_id.expect("leave() on a scope without an enclosing scope");
        let same_table = {
            let next_scope = self.scopes.get(next_id.index());
            match &next_scope.data {
                ScopeData::ScopeImpl { table: next_table, .. } => Rc::ptr_eq(&table, next_table),
                _ => false,
            }
        };
        if same_table {
            let mut table_ref = table.borrow_mut();
            let entries_ref = entries.borrow();
            let mut current = elems;
            while let Some(entry_id) = current {
                let entry = &entries_ref[entry_id.index() as usize];
                let sym = entry.sym.expect("scope entry without symbol");
                let name = self.symbol_name(sym);
                let mask = table_ref.len() - 1;
                let hash = probe_index(&name, &table_ref, mask, &entries_ref, &self.symbols);
                match table_ref[hash] {
                    TableSlot::Entry(head) if head == entry_id => {
                        table_ref[hash] = match entries_ref[entry_id.index() as usize].shadowed {
                            Some(shadowed) => TableSlot::Entry(shadowed),
                            None => TableSlot::Empty,
                        };
                    }
                    _ => panic!("leave(): entry not found in the shared table"),
                }
                current = entry.next_sibling;
            }
            let next_scope = self.scopes.get_mut(next_id.index());
            let ScopeData::ScopeImpl {
                shared,
                nelems: next_nelems,
                ..
            } = &mut next_scope.data
            else {
                panic!("ClassCastException: not a ScopeImpl");
            };
            assert!(*shared > 0, "leave(): enclosing scope is not shared");
            *shared -= 1;
            *next_nelems = nelems;
        }
        let _ = has_symbols;
        next_id
    }


    /// Java: new TypeVar(Name name, Symbol owner, Type lower) — creates the
    /// TypeVariableSymbol whose type is the new TypeVar (tsym back-pointer).
    pub fn new_type_var_with_owner(
        &mut self,
        names: &crate::util::names::Names,
        name: crate::util::name::Name,
        owner: SymbolId,
        lower: TypeId,
    ) -> TypeId {
        let _ = names;
        let ty = self.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::TYPEVAR,
            None,
            crate::code::type_::TypeKindData::TypeVar {
                bound: None,
                lower: Some(lower),
            },
        ));
        let sym = self.alloc_symbol(crate::code::symbol::Symbol::new_type_variable_symbol(
            0,
            name,
            ty,
            owner,
        ));
        if let crate::code::type_::TypeKindData::TypeVar { .. } = &mut self.typ_mut(ty).kind {
            self.typ_mut(ty).tsym = Some(sym);
        }
        ty
    }

    /// Java: new ClassSymbol(flags, names.empty, null, syms.noSymbol) for
    /// Types.makeIntersectionType (the COMPOUND class symbol of an
    /// intersection type); sets type/erasure/members.
    pub fn new_compound_class_symbol(
        &mut self,
        names: &crate::util::names::Names,
        flags: i64,
        no_symbol: SymbolId,
        intersection: TypeId,
        erasure: TypeId,
    ) -> SymbolId {
        let sym = self.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::TYP,
            flags,
            names.empty.clone(),
            Some(intersection),
            Some(no_symbol),
            crate::code::symbol::SymbolKind::ClassSymbol {
                members_field: None,
                fullname: names.empty.clone(),
                flatname: names.empty.clone(),
                sourcefile: None,
                classfile: None,
                trans_local: List::nil(),
                record_components: List::nil(),
                permitted: List::nil(),
                is_permitted_explicit: false,
                annotation_type_metadata: Default::default(),
            },
        ));
        self.sym_mut(sym).erasure_field = Some(erasure);
        let members = self.scope_create(Some(sym));
        if let crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } =
            &mut self.sym_mut(sym).kind_data
        {
            *members_field = Some(members);
        }
        sym
    }
    pub fn scope_any_match(&self, scope: ScopeId, filter: &dyn Fn(SymbolId) -> bool) -> bool {
        self.scope(scope).any_match(filter)
    }
}

/// Java: CompoundScope.subScopes/mark field access (the compound scope family).
fn sub_scope_fields_mut(
    scope: &mut crate::code::scope::Scope,
) -> Option<(&mut crate::util::list::List<crate::ids::ScopeId>, &mut i32)> {
    match &mut scope.data {
        crate::code::scope::ScopeData::CompoundScope { sub_scopes, mark, .. }
        | crate::code::scope::ScopeData::ImportScope { sub_scopes, mark, .. }
        | crate::code::scope::ScopeData::NamedImportScope { sub_scopes, mark, .. }
        | crate::code::scope::ScopeData::StarImportScope { sub_scopes, mark, .. } => {
            Some((sub_scopes, mark))
        }
        _ => None,
    }
}


impl CompilerContext {
    /// Java: ClassSymbol.createRecordComponent(RecordComponent existing,
    /// JCVariableDecl var, List<JCAnnotation> annotations) (Symbol.java:1522-1532).
    ///
    /// Adapter: the symbol arena allocation needs the compiler context, so the
    /// ClassSymbol method is expressed as this context operation.
    ///
    /// # Arguments
    ///   - class: the record class symbol;
    ///   - existing: the component to replace, if any;
    ///   - var: the record component field symbol;
    ///   - annotations: the (original) annotations of the new component.
    ///
    /// # Returns
    ///   - the created record component symbol.
    ///
    /// # Panics
    ///   - the symbol is not a class symbol.
    pub fn create_record_component(
        &mut self,
        class: SymbolId,
        existing: Option<SymbolId>,
        var: SymbolId,
        annotations: List<TreeId>,
    ) -> SymbolId {
        let var_name = self.sym(var).name.clone();
        let var_ty = self.sym(var).ty.expect("record component type");
        let (original_annos, is_varargs) = match existing {
            Some(existing) => match &self.sym(existing).kind_data {
                crate::code::symbol::SymbolKind::RecordComponent {
                    original_annos,
                    is_varargs,
                    ..
                } => (original_annos.clone(), *is_varargs),
                _ => (List::nil(), false),
            },
            None => (annotations, false),
        };
        let pos = match &self.sym(var).kind_data {
            crate::code::symbol::SymbolKind::VarSymbol { pos, .. } => *pos,
            _ => crate::util::position::NOPOS,
        };
        let rc = self.alloc_symbol(Symbol::new(
            crate::code::kinds::Kind::VAR,
            crate::code::flags::PUBLIC as i64,
            var_name,
            Some(var_ty),
            Some(class),
            crate::code::symbol::SymbolKind::RecordComponent {
                accessor: None,
                accessor_meth: None,
                original_annos,
                pos,
                is_varargs,
            },
        ));
        if let Some(existing) = existing {
            match &mut self.sym_mut(class).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol {
                    record_components, ..
                } => {
                    let mut filtered: List<SymbolId> = List::nil();
                    let mut current = record_components.clone();
                    while current.non_empty() {
                        let component = *current.head();
                        current = current.tail();
                        if component != existing {
                            filtered = filtered.append(component);
                        }
                    }
                    *record_components = filtered.append(rc);
                }
                _ => panic!("ClassCastException: not a class symbol"),
            }
        } else {
            match &mut self.sym_mut(class).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol {
                    record_components, ..
                } => {
                    *record_components = record_components.append(rc);
                }
                _ => panic!("ClassCastException: not a class symbol"),
            }
        }
        rc
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::kinds::Kind;
    use crate::code::scope::Scope;
    use crate::code::symbol::{Symbol, SymbolKind};
    use crate::util::name::NameTableHandle;
    use std::rc::Rc;

    #[test]
    fn prototype_scope_query_with_context_borrow_split() {
        let mut ctx = CompilerContext::new();
        let names = Rc::new(crate::util::names::Names::new(false));
        let mut syms = crate::code::symtab::Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            crate::jvm::target::Target::DEFAULT,
            crate::code::source::Source::JDK17,
        );
        let types = crate::code::types::Types::with_names(Rc::clone(&names));
        let scope = ctx.alloc_scope(crate::code::scope::Scope::new_scope_impl(None));
        let name_a = names.from_utf(b"a");
        let sym_a = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::VAR,
            0,
            name_a.clone(),
            None,
            None,
            crate::code::symbol::SymbolKind::VarSymbol {
                lazy_const_value: None,
                pos: 0,
                adr: 0,
                data_kind: None,
                data: None,
            },
        ));
        let sym_b = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::VAR,
            0,
            name_a.clone(),
            None,
            None,
            crate::code::symbol::SymbolKind::VarSymbol {
                lazy_const_value: None,
                pos: 0,
                adr: 0,
                data_kind: None,
                data: None,
            },
        ));
        let mut scope_snapshot = ctx.scope(scope).clone();
        {
            let symbols = ctx.symbols_arena();
            scope_snapshot.enter(scope, sym_a, symbols);
            scope_snapshot.enter(scope, sym_b, symbols);
        }
        *ctx.scope_mut(scope) = scope_snapshot;
        let all = ctx.scope_get_symbols_ctx(
            scope,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            &types,
            &syms,
        );
        assert_eq!(all.len(), 2);
        let by_name = ctx.scope_get_symbols_by_name_ctx(
            scope,
            &name_a,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            &types,
            &syms,
        );
        assert_eq!(by_name, vec![sym_b, sym_a]);
    }

    fn var_symbol(table: &Rc<NameTableHandle>, text: &str) -> Symbol {
        Symbol::new(
            Kind::VAR,
            0,
            name(table, text),
            None,
            None,
            crate::code::symbol::SymbolKind::VarSymbol {
                lazy_const_value: None,
                pos: 0,
                adr: -1,
                data_kind: None,
                data: None,
            },
        )
    }

    struct FlagCompleter;

    impl crate::code::symbol::Completer for FlagCompleter {
        fn complete(
            &mut self,
            ctx: &mut CompilerContext,
            _syms: &crate::code::symtab::Symtab,
            sym: SymbolId,
        ) -> Result<(), crate::code::symbol::CompletionFailure> {
            ctx.sym_mut(sym).flags_field |= crate::code::flags::DEPRECATED as i64;
            Ok(())
        }
    }

    #[test]
    fn api_complete_symbol_runs_the_completer_and_clears_it() {
        let mut ctx = CompilerContext::new();
        let names = Rc::new(crate::util::names::Names::new(false));
        let mut syms = crate::code::symtab::Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            crate::jvm::target::Target::DEFAULT,
            crate::code::source::Source::JDK17,
        );
        let id = ctx.alloc_symbol(var_symbol(&name_table(), "x"));
        let completer: crate::code::symbol::CompleterRef =
            Rc::new(std::cell::RefCell::new(FlagCompleter));
        ctx.sym_mut(id).completer = Some(completer);
        assert!(ctx.api_complete_symbol(&syms, id).is_ok());
        assert!(
            (ctx.sym(id).flags_field & crate::code::flags::DEPRECATED as i64) != 0,
            "apiComplete runs Symbol.complete() like Symbol.java:686-692"
        );
        assert!(ctx.sym(id).completer.is_none());
        assert!(ctx.api_complete_symbol(&syms, id).is_ok());
    }

    #[test]
    fn symbol_id_and_metadata_are_bound_at_allocation() {
        let mut ctx = CompilerContext::new();
        let table = name_table();
        let id = ctx.alloc_symbol(var_symbol(&table, "x"));
        assert_eq!(ctx.sym(id).id.get(), Some(id));
        // The Symbol method creates the metadata bound to the arena id.
        let empty = crate::util::list::List::nil();
        ctx.sym_mut(id).set_declaration_attributes(empty.clone());
        assert_eq!(
            ctx.sym(id).metadata.as_ref().expect("metadata").owner(),
            id
        );
        // A clone carries no arena id until it is allocated; the allocation
        // entry point records the new id and rebinds the copied metadata.
        let cloned = ctx.sym(id).clone();
        assert_eq!(cloned.id.get(), None);
        let cloned_id = ctx.alloc_symbol(cloned);
        assert_eq!(ctx.sym(cloned_id).id.get(), Some(cloned_id));
        assert_eq!(
            ctx.sym(cloned_id).metadata.as_ref().expect("metadata").owner(),
            cloned_id
        );
        assert_eq!(ctx.sym(id).metadata.as_ref().expect("metadata").owner(), id);
    }


    fn name_table() -> Rc<NameTableHandle> {
        NameTableHandle::unshared()
    }

    fn name(table: &Rc<NameTableHandle>, text: &str) -> Name {
        let units: Vec<u16> = text.encode_utf16().collect();
        table.from_chars(&units, 0, units.len() as i32)
    }

    #[test]
    fn six_arenas_allocate_and_are_addressable() {
        let mut ctx = CompilerContext::new();
        let table = name_table();
        let symbol_id = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            name(&table, "x"),
            None,
            None,
            crate::code::symbol::SymbolKind::VarSymbol {
                lazy_const_value: None,
                pos: 1,
                adr: -1,
                data_kind: None,
                data: None,
            },
        ));
        let scope_id = ctx.alloc_scope(Scope::new_scope_impl(Some(symbol_id)));
        assert_eq!(ctx.symbols.len(), 1);
        assert_eq!(ctx.symbols.len(), 1);
        assert_eq!(ctx.scopes.len(), 1);
        assert_eq!(ctx.symbol_name(symbol_id).to_java_string().to_string(), "x");
        assert_eq!(ctx.symbol_kind(symbol_id), Kind::VAR);
        assert_eq!(ctx.scope(scope_id).owner, Some(symbol_id));
        assert_eq!(ctx.symbol_kind(symbol_id), Kind::VAR);
    }

    #[test]
    fn out_of_range_access_panics() {
        let result = std::panic::catch_unwind(|| {
            let ctx = CompilerContext::new();
            let _ = ctx.sym(SymbolId::new(0));
        });
        assert!(result.is_err());
    }

    #[test]
    fn scope_operations_go_through_the_context() {
        let mut ctx = CompilerContext::new();
        let table = name_table();
        let scope = ctx.alloc_scope(Scope::new_scope_impl(None));
        for (index, text) in ["a", "b", "c"].iter().enumerate() {
            let sym = ctx.alloc_symbol(Symbol::new(
                Kind::VAR,
                0,
                name(&table, text),
                None,
                None,
                crate::code::symbol::SymbolKind::VarSymbol {
                    lazy_const_value: None,
                    pos: index as i32,
                    adr: -1,
                    data_kind: None,
                    data: None,
                },
            ));
            ctx.scope_enter(scope, sym);
        }
        assert_eq!(ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true).len(), 3);
        // The iteration order is the upstream elems chain order (LIFO).
        let symbols = ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
        assert_eq!(symbols[0], SymbolId::new(2));
        assert_eq!(symbols[2], SymbolId::new(0));
        let a = name(&table, "a");
        assert_eq!(ctx.scope_find_first(scope, &a, &|_| true), Some(SymbolId::new(0)));
        assert!(ctx.scope_includes(scope, SymbolId::new(1)));
        assert_eq!(ctx.scope_get_symbols_by_name(scope, &a, LookupKind::NON_RECURSIVE, &|_| true), vec![SymbolId::new(0)]);
        ctx.scope_remove(scope, SymbolId::new(0));
        assert!(!ctx.scope_includes(scope, SymbolId::new(0)));
        assert_eq!(ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true).len(), 2);
        assert!(!ctx.scope_is_empty(scope));
        assert!(ctx.scope_any_match(scope, &|_| true));
    }
}
