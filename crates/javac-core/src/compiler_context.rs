//! ADR 0001 的编译期对象 arena 容器(CompilerContext)。
//!
//! PORTING_RULES: 非 1:1 适配文件(Java 的可变对象图 -> arena + ID),见 ADR 0001。
//! 六种 arena:Symbol/Type/JCTree/Scope/Attribute/Directive;每种 *Id 只对同一个
//! CompilerContext 有效,编译结束整体释放。
//!
//! 借用规则(ADR 0001 规则 3-4,所有组件必须遵守):
//! - CompilerContext 独占持有六种 arena;组件逻辑为带 ctx: &mut CompilerContext
//!   参数的函数/方法(Java 的 x.field -> ctx.sym(x).field);
//! - 禁止用 Rc<RefCell<...>> 表达核心对象图(PORTING_RULES §9 白名单除外);
//! - 需要同时访问多个 arena 时用字段拆分借用:Rust 的借用检查按字段进行,
//!   例如 &mut ctx.scopes 与 &ctx.symbols 可以同时存在;
//! - 访问器返回的引用与 &self 绑定:持有引用的期间不得对同一个 ctx 再做可变
//!   借用;需要先读后写时,先用访问器把 name/kind/ID 等值取出来再释放借用;
//! - arena 只增不减(ADR 0001 规则 8);越界访问在 debug 下由 util/arena.rs 断言。
//!
//! Scope 的查找/迭代顺序语义由 code/scope.rs 的算法保证(上游开放寻址双哈希 +
//! elems/shadowed 链);本模块只提供基于 CompilerContext 的包装方法,不再暴露
//! 之前的 SymbolResolver/self_id 过渡(方法内部把 arena 切片传给 Scope 算法)。

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
}

impl CompilerContext {
    /// Creates an empty compilation context.
    pub fn new() -> CompilerContext {
        CompilerContext::default()
    }

    /// Allocates a symbol in the symbol arena and returns its ID.
    pub fn alloc_symbol(&mut self, sym: Symbol) -> SymbolId {
        SymbolId::new(self.symbols.push(sym))
    }

    /// Allocates a type in the type arena and returns its ID.
    pub fn alloc_type(&mut self, ty: Type) -> TypeId {
        TypeId::new(self.types.push(ty))
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

    /// Java: ctx.scope(id).
    pub fn scope(&self, id: ScopeId) -> &Scope {
        self.scopes.get(id.index())
    }

    /// Java: ctx.scope_mut(id).
    pub fn scope_mut(&mut self, id: ScopeId) -> &mut Scope {
        self.scopes.get_mut(id.index())
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

    /// Java: ScopeImpl.getSymbols(Predicate<Symbol>, LookupKind)
    pub fn scope_get_symbols(
        &self,
        scope: ScopeId,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Vec<SymbolId> {
        self.scope(scope).get_symbols(lookup_kind, filter)
    }

    /// Java: ScopeImpl.getSymbolsByName(Name, Predicate<Symbol>, LookupKind)
    pub fn scope_get_symbols_by_name(
        &self,
        scope: ScopeId,
        name: &Name,
        lookup_kind: LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Vec<SymbolId> {
        self.scope(scope)
            .get_symbols_by_name(scope, name, lookup_kind, filter, &self.symbols)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::kinds::Kind;
    use crate::code::scope::Scope;
    use crate::code::symbol::{Symbol, SymbolKind};
    use crate::util::name::NameTableHandle;
    use std::rc::Rc;

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
            SymbolKind::VarSymbol {
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
                SymbolKind::VarSymbol {
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
