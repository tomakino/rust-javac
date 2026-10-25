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
use crate::code::scope::{EntryId, LookupKind, Scope};
use crate::code::symbol::Symbol;
use crate::code::type_::Type;
use crate::ids::{AttributeId, DirectiveId, ScopeId, SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::JCTree;
use crate::util::arena::Arena;
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
