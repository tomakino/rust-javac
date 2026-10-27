// Origin: com/sun/tools/javac/comp/Lower.java @ 982d53c (jdk-17.0.19-ga)

//! Translates away syntactic sugar before code generation: inner classes and
//! their access bridges, class literals, assertions, foreach loops, enum
//! switches, string concatenation and assignment operators.
//!
//! The Java base class TreeTranslator is expressed here through the same visitor
//! framework methods used by the other lowering pass: a translate/result pair,
//! prototype-type scoped helpers, and the standard tree navigation helpers.
//!
//! The pass keeps mutable lower state (current class/method, proxy map, access
//! method cache, enum switch maps) on the Lower value itself, as upstream does;
//! service handles are injected explicitly.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::code::kinds::Kind;
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::type_::{Type, TypeKindData};
use crate::code::types::{SignatureGenerator, Types};
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::comp::const_fold::ConstFold;
use crate::comp::env::Env;
use crate::comp::operators::Operators;
use crate::comp::resolve::Resolve;
use crate::comp::type_envs::TypeEnvs;
use crate::compiler_context::CompilerContext;
use crate::ids::{ScopeId, SymbolId, TreeId, TypeId};
use crate::jvm::target::Target;
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_info;
use crate::tree::tree_scanner::TreeScannerVisitor;
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::util::jc_diagnostic::DiagnosticPosition;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;

/// The lowering pass for one compilation session.  It is constructed with the
/// shared services; the per-compilation mutable state is reset by
/// translate_top_level_class so a single instance can lower several units in
/// sequence.
pub struct Lower {
    /// The name table.
    pub names: Rc<Names>,
    /// The diagnostic log.
    pub log: Rc<RefCell<Log>>,
    /// The symbol table.
    pub syms: Rc<RefCell<Symtab>>,
    /// The resolver.
    pub rs: Rc<RefCell<Resolve>>,
    /// The operator service (installed by the compiler wiring).
    pub operators: Rc<RefCell<Option<Rc<Operators>>>>,
    /// The check service.
    pub chk: Rc<RefCell<Check>>,
    /// The attribution service.
    pub attr: Rc<RefCell<Attr>>,
    /// The tree factory.
    pub make: Rc<RefCell<TreeMaker>>,
    /// The diagnostic position of the last make_at call.
    pub make_pos: Option<Rc<dyn DiagnosticPosition>>,
    /// The constant folder.
    pub cfolder: Rc<ConstFold>,
    /// The class-file target.
    pub target: Target,
    /// The environment table used to find class environments.
    pub type_envs: Rc<RefCell<TypeEnvs>>,
    /// The name of the synthetic assertions-disabled field.
    pub dollar_assertions_disabled: Name,
    /// The type relations service.
    pub types: Rc<Types>,
    /// The type inference service (required by the explicit resolve adapters).
    pub infer: Rc<crate::comp::infer::Infer>,
    /// Whether debug lowering checks are enabled.
    pub debug_lower: bool,
    /// Whether protected accessor synthesis is disabled.
    pub disable_protected_accessors: bool,
    /// The package-info generation option.
    pub pkginfo_opt: crate::main::option::PkgInfo,

    /// The class currently being lowered.
    pub current_class: Option<SymbolId>,
    /// The queue of classes translated in the current top-level class.
    pub translated: Vec<TreeId>,
    /// The attribution environment of the current top-level class.
    pub attr_env: Option<Rc<Env<AttrContext>>>,
    /// The end-position table of the current compilation unit (when present).
    pub end_pos_table: Option<Rc<dyn EndPosTable>>,
    /// Local class symbols to their class declaration trees.
    pub classdefs: HashMap<SymbolId, TreeId>,
    /// Local class symbols to their pruned trees.
    pub pruned_tree: HashMap<SymbolId, Vec<TreeId>>,
    /// Virtual access symbols to the symbols actually referenced.
    pub actual_symbols: HashMap<SymbolId, SymbolId>,
    /// The method declaration currently being lowered.
    pub current_method_def: Option<TreeId>,
    /// The method symbol currently being lowered.
    pub current_method_sym: Option<SymbolId>,
    /// The outermost class declaration of the current top-level class.
    pub outermost_class_def: Option<TreeId>,
    /// The outermost member declaration of the current top-level class.
    pub outermost_member_def: Option<TreeId>,
    /// Captured-symbol replacement map when lowering a lambda body.
    pub lambda_translation_map: Option<HashMap<SymbolId, SymbolId>>,
    /// Local class symbols to the free variables they capture.
    pub freevar_cache: HashMap<SymbolId, List<SymbolId>>,
    /// Enum switch helper maps, per enum symbol.
    enum_switch_map: HashMap<SymbolId, Rc<RefCell<EnumMapping>>>,
    /// Per-symbol access numbering for synthesized access methods.
    pub access_nums: HashMap<SymbolId, i32>,
    /// Per-symbol access method cache, indexed by access code.
    pub access_syms: HashMap<SymbolId, Vec<Option<SymbolId>>>,
    /// Per-class access constructors synthesized for private access.
    pub access_constrs: HashMap<SymbolId, SymbolId>,
    /// The order in which access constructors were synthesized.
    pub access_constr_tags: Vec<SymbolId>,
    /// The symbols accessed in the current constructor, in order.
    pub accessed: Vec<SymbolId>,
    /// Free-variable proxies and implicit outer-this symbols.
    pub proxies: HashMap<SymbolId, SymbolId>,
    /// Stack of implicit outer-this parameter symbols.
    pub outer_this_stack: Vec<SymbolId>,
    /// Scope of try-with-resources variables during lowering.
    pub twr_vars: Option<ScopeId>,
    /// The last assertions-disabled class synthesized.
    pub assertions_disabled_class_cache: Option<SymbolId>,
    /// The cached System.arraycopy method symbol.
    pub system_arraycopy_method: Option<SymbolId>,
    /// The current tree translation result.
    pub result: Option<TreeId>,
    /// The prototype type of the current translation (when any).
    pub pt: Option<TypeId>,
    /// The closest enclosing operator node of the tree being translated.
    pub encl_op: Option<TreeId>,
}

impl Lower {
    /// Create a lowering pass.
    ///
    /// The services are injected explicitly: the lowering pass is constructed
    /// after attribution and before code generation, so all services it needs
    /// already exist.  Mutable lowering state starts empty;
    /// translate_top_level_class initializes the per-unit parts.
    ///
    /// # Arguments
    ///   - names/log/syms/rs/operators/chk/attr/make/cfolder/target/type_envs/types:
    ///     the shared compiler services;
    ///   - infer: the type inference service used by the resolve adapters;
    ///   - pkginfo_opt: the package-info generation mode;
    ///   - debug_lower: whether debug checks are enabled;
    ///   - disable_protected_accessors: whether accessor synthesis is disabled.
    ///
    /// # Returns
    ///   - the lowering pass.
    ///
    /// # Panics
    ///   - none.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        syms: Rc<RefCell<Symtab>>,
        rs: Rc<RefCell<Resolve>>,
        operators: Rc<RefCell<Option<Rc<Operators>>>>,
        chk: Rc<RefCell<Check>>,
        attr: Rc<RefCell<Attr>>,
        make: Rc<RefCell<TreeMaker>>,
        cfolder: Rc<ConstFold>,
        target: Target,
        type_envs: Rc<RefCell<TypeEnvs>>,
        types: Rc<Types>,
        infer: Rc<crate::comp::infer::Infer>,
        pkginfo_opt: crate::main::option::PkgInfo,
        debug_lower: bool,
        disable_protected_accessors: bool,
    ) -> Lower {
        let dollar = names.from_string(&javac_strings::JavaString::from(
            format!("{}{}", target.synthetic_name_char(), "assertionsDisabled").as_str(),
        ));
        Lower {
            names,
            log,
            syms,
            rs,
            operators,
            chk,
            attr,
            make,
            make_pos: None,
            cfolder,
            target,
            type_envs,
            dollar_assertions_disabled: dollar,
            types,
            infer,
            debug_lower,
            disable_protected_accessors,
            pkginfo_opt,
            current_class: None,
            translated: Vec::new(),
            attr_env: None,
            end_pos_table: None,
            classdefs: HashMap::new(),
            pruned_tree: HashMap::new(),
            actual_symbols: HashMap::new(),
            current_method_def: None,
            current_method_sym: None,
            outermost_class_def: None,
            outermost_member_def: None,
            lambda_translation_map: None,
            freevar_cache: HashMap::new(),
            enum_switch_map: HashMap::new(),
            access_nums: HashMap::new(),
            access_syms: HashMap::new(),
            access_constrs: HashMap::new(),
            access_constr_tags: Vec::new(),
            accessed: Vec::new(),
            proxies: HashMap::new(),
            outer_this_stack: Vec::new(),
            twr_vars: None,
            assertions_disabled_class_cache: None,
            system_arraycopy_method: None,
            result: None,
            pt: None,
            encl_op: None,
        }
    }


    /// Set the diagnostic position of the next make call and position it at
    /// the diagnostic start position.
    ///
    /// The position is cached for later diagnostics issued by the helper
    /// builders (mirrors make_at upstream).
    fn make_at(&mut self, pos: &Rc<dyn DiagnosticPosition>) {
        self.make_pos = Some(Rc::clone(pos));
        self.make.borrow_mut().at(pos.get_start_position());
    }

    /// Map a constant value to the literal value carried by a literal tree.
    ///
    /// Integral constants narrower than int are widened to the int literal
    /// representation, matching the upstream Literal(typeTag, value) call.
    fn literal_value(
        value: &crate::util::constants::ConstantValue,
    ) -> crate::tree::jc_tree::LiteralValue {
        use crate::util::constants::ConstantValue;
        use crate::tree::jc_tree::LiteralValue;
        match value {
            ConstantValue::Integer(v) => LiteralValue::Integer(*v),
            ConstantValue::Byte(v) => LiteralValue::Byte(*v),
            ConstantValue::Short(v) => LiteralValue::Short(*v),
            ConstantValue::Long(v) => LiteralValue::Long(*v),
            ConstantValue::Float(v) => LiteralValue::Float(*v),
            ConstantValue::Double(v) => LiteralValue::Double(*v),
            ConstantValue::Character(v) => LiteralValue::Character(*v),
            ConstantValue::Boolean(v) => LiteralValue::Boolean(*v),
            ConstantValue::String(v) => LiteralValue::String(v.clone()),
        }
    }

    /// Build an attributed literal of a given type and constant value.
    ///
    /// The literal tag comes from the type tag, and the resulting type is the
    /// constant type of the given type, as the upstream makeLit does.
    fn make_lit(
        &mut self,
        ctx: &mut CompilerContext,
        ty: TypeId,
        value: &crate::util::constants::ConstantValue,
    ) -> TreeId {
        let type_tag = ctx.typ(ty).get_tag();
        let literal = Self::literal_value(value);
        let constant_ty = ctx.typ(ty).const_type(value.clone());
        let constant_ty = ctx.alloc_type(constant_ty);
        let tree = self
            .make
            .borrow_mut()
            .literal(ctx, type_tag, Some(literal));
        ctx.tree_mut(tree).ty = Some(constant_ty);
        tree
    }

    /// Build the attributed null literal of the bottom type.
    ///
    /// The null value has no constant payload, so the literal tree carries no
    /// value and the type remains the bottom type.
    fn make_null(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let bot_type = self.syms.borrow().bot_type;
        let tree = self.make.borrow_mut().literal(ctx, TypeTag::BOT, None);
        ctx.tree_mut(tree).ty = Some(bot_type);
        tree
    }

    /// Return the operator service, failing loudly when it is not installed.
    ///
    /// The lowering pass needs operators for the synthesized arithmetic and
    /// string concatenation trees; the compiler wiring installs the service
    /// before lowering starts.
    fn operators(&mut self) -> Rc<Operators> {
        self.operators
            .borrow()
            .as_ref()
            .expect("operators service not installed")
            .clone()
    }

    /// Convert an argument tree list to the type list expected by resolution.
    ///
    /// TreeInfo.types maps each tree to its attributed type; an unattributed
    /// argument is replaced by the no-type sentinel so that resolution reports
    /// the appropriate error instead of panicking.
    fn tree_types(&self, ctx: &mut CompilerContext, args: &List<TreeId>) -> List<TypeId> {
        let mut out = Vec::new();
        for arg in args.iter() {
            let ty = ctx.tree(arg).ty;
            match ty {
                Some(ty) => out.push(ty),
                None => out.push(crate::code::symtab::no_type_id(ctx)),
            }
        }
        List::from_iterable(out)
    }


    /// Build a field-selection tree for a symbol, attaching the symbol type.
    ///
    /// The upstream Select(base, symbol) overload is expressed as
    /// Select(base, symbol.name) followed by the symbol type assignment.
    fn select_symbol(&mut self, ctx: &mut CompilerContext, base: TreeId, sym: SymbolId) -> TreeId {
        let selector = ctx.sym(sym).name.clone();
        let symbol_ty = ctx.sym(sym).ty;
        let tree = self.make.borrow_mut().select(ctx, base, selector);
        ctx.tree_mut(tree).ty = symbol_ty;
        tree
    }

    /// Resolve and attach a constructor call to a fresh new-class tree.
    ///
    /// The constructor is resolved against the current attribution environment
    /// so that later phases see an attributed creation tree.
    fn make_new_class(
        &mut self,
        ctx: &mut CompilerContext,
        ctype: TypeId,
        args: List<TreeId>,
    ) -> TreeId {
        let tsym = ctx.typ(ctype).tsym.expect("class type symbol");
        let qual = {
            self.make.borrow_mut().qual_ident(ctx, tsym)
        };
        let tree = self.make.borrow_mut().new_class(
            ctx,
            None,
            List::nil(),
            qual,
            args.clone(),
            None,
        );
        let argtypes = self.tree_types(ctx, &args);
        let env = self.attr_env.clone().expect("attribution environment");
        let pos = self.make_pos.clone();
        let constructor = {
            let mut rs = self.rs.borrow_mut();
            let mut syms = self.syms.borrow_mut();
            let mut attr = self.attr.borrow_mut();
            let chk = self.chk.borrow();
            rs.resolve_constructor(
                ctx,
                &mut syms,
                &self.types,
                &mut attr,
                &chk,
                &self.infer,
                pos,
                &env,
                ctype,
                argtypes,
                None,
            )
        };
        if let TreeKind::NewClass {
            constructor: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = Some(constructor);
        }
        ctx.tree_mut(tree).ty = Some(ctype);
        tree
    }

    /// Build an attributed unary operator tree.
    ///
    /// The operator symbol is resolved through the operator service and its
    /// return type becomes the expression type.
    fn make_unary(&mut self, ctx: &mut CompilerContext, optag: Tag, arg: TreeId) -> TreeId {
        let arg_type = ctx.tree(arg).ty.expect("attributed unary argument");
        let pos = ctx.tree(arg).pos;
        let operators = self.operators();
        let operator = {
            let mut syms = self.syms.borrow_mut();
            operators.resolve_unary(ctx, &mut syms, &self.types, pos, optag, arg_type)
        };
        let tree = self.make.borrow_mut().unary(ctx, optag, arg);
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let restype = ctx.typ(operator_ty).get_return_type(ctx);
        if let TreeKind::Unary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(operator);
        }
        ctx.tree_mut(tree).ty = restype;
        tree
    }

    /// Build an attributed binary operator tree.
    ///
    /// The operator symbol is resolved from both operand types and its return
    /// type becomes the expression type.
    fn make_binary(
        &mut self,
        ctx: &mut CompilerContext,
        optag: Tag,
        lhs: TreeId,
        rhs: TreeId,
    ) -> TreeId {
        let lhs_type = ctx.tree(lhs).ty.expect("attributed left operand");
        let rhs_type = ctx.tree(rhs).ty.expect("attributed right operand");
        let pos = ctx.tree(lhs).pos;
        let operators = self.operators();
        let operator = {
            let mut syms = self.syms.borrow_mut();
            operators.resolve_binary(ctx, &mut syms, &self.types, pos, optag, lhs_type, rhs_type)
        };
        let tree = self.make.borrow_mut().binary(ctx, optag, lhs, rhs);
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let restype = ctx.typ(operator_ty).get_return_type(ctx);
        if let TreeKind::Binary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(operator);
        }
        ctx.tree_mut(tree).ty = restype;
        tree
    }

    /// Build an attributed compound-assignment operator tree.
    ///
    /// The operator is resolved from the corresponding binary operator tag,
    /// while the assignop expression itself keeps the left-hand type.
    fn make_assignop(
        &mut self,
        ctx: &mut CompilerContext,
        optag: Tag,
        lhs: TreeId,
        rhs: TreeId,
    ) -> TreeId {
        let lhs_type = ctx.tree(lhs).ty.expect("attributed left operand");
        let rhs_type = ctx.tree(rhs).ty.expect("attributed right operand");
        let pos = ctx.tree(lhs).pos;
        let binary_tag = optag.no_assign_op();
        let operators = self.operators();
        let operator = {
            let mut syms = self.syms.borrow_mut();
            operators.resolve_binary(ctx, &mut syms, &self.types, pos, binary_tag, lhs_type, rhs_type)
        };
        let tree = self.make.borrow_mut().assignop(ctx, optag, lhs, rhs);
        if let TreeKind::AssignOp { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(operator);
        }
        ctx.tree_mut(tree).ty = Some(lhs_type);
        tree
    }

    /// Convert a tree into a string object unless it already has a reference
    /// type.
    ///
    /// A primitive or void operand is wrapped in String.valueOf(...) so that
    /// string concatenation is built entirely out of reference operations.
    fn make_string(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let ty = ctx.tree(tree).ty.expect("attributed string operand");
        if !ctx.typ(ty).is_primitive_or_void() {
            return tree;
        }
        let pos = ctx.tree(tree).pos;
        let diagnostic_pos = Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos));
        let string_type = self.syms.borrow().string_type;
        let value_of = self.names.valueOf.clone();
        let value_of_sym = self.lookup_method(
            ctx,
            Some(diagnostic_pos as Rc<dyn DiagnosticPosition>),
            value_of,
            string_type,
            List::of(ty),
        );
        let qual = {
            self.make.borrow_mut().qual_ident(ctx, value_of_sym)
        };
        self.make.borrow_mut().app_with_args(ctx, qual, List::of(tree))
    }

    /// Enter a synthetic symbol in the given scope.
    ///
    /// The upstream helper reports a conflict through the scope itself; the
    /// ported scope reports conflicting entries, so the symbol is entered
    /// directly.
    fn enter_synthetic(&mut self, ctx: &mut CompilerContext, sym: SymbolId, scope: ScopeId) {
        ctx.scope_enter(scope, sym);
    }

    /// Return the first synthetic symbol with the given name in a scope.
    ///
    /// Non-synthetic symbols never satisfy the lookup, matching the upstream
    /// lookupSynthetic helper.
    fn lookup_synthetic(
        &self,
        ctx: &CompilerContext,
        name: &Name,
        scope: ScopeId,
    ) -> Option<SymbolId> {
        let arena = ctx.symbols_arena();
        ctx.scope(scope).find_first(
            name,
            &|sym| (ctx.sym(sym).flags_field & crate::code::flags::SYNTHETIC as i64) != 0,
            arena,
        )
    }

    /// Create a fresh synthetic name in the given scope.
    ///
    /// The synthetic name character is appended repeatedly until the name does
    /// not collide with another synthetic symbol in the scope.
    fn make_synthetic_name(&self, ctx: &CompilerContext, name: Name, scope: ScopeId) -> Name {
        let mut name = name;
        loop {
            name = name.append_char(
                self.target.synthetic_name_char() as u8,
                &self.names.empty,
            );
            if self.lookup_synthetic(ctx, &name, scope).is_none() {
                return name;
            }
        }
    }

    /// Resolve a method in the attribution environment.
    ///
    /// The explicit resolve adapter needs the resolver, symbol table, attributor
    /// and inference engine; all are borrowed from this pass so the internal
    /// resolution sees the same services the compiler used.
    fn lookup_method(
        &mut self,
        ctx: &mut CompilerContext,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        name: Name,
        qual: TypeId,
        args: List<TypeId>,
    ) -> SymbolId {
        let env = self.attr_env.clone().expect("attribution environment");
        let mut rs = self.rs.borrow_mut();
        let mut syms = self.syms.borrow_mut();
        let mut attr = self.attr.borrow_mut();
        let chk = self.chk.borrow();
        rs.resolve_internal_method(
            ctx,
            &mut syms,
            &self.types,
            &mut attr,
            &chk,
            &self.infer,
            pos,
            &env,
            qual,
            name,
            args,
            None,
        )
    }

    /// Record that a tree was pruned from the current class.
    ///
    /// The pruned tree list is consulted later when the accessor synthesis
    /// rewrites references, matching the upstream addPrunedInfo helper.
    fn add_pruned_info(&mut self, tree: TreeId) {
        let current = self.current_class.expect("current class");
        self.pruned_tree
            .entry(current)
            .or_default()
            .insert(0, tree);
    }


    /// Return the class declaration tree of a class symbol.
    ///
    /// The declaration is first looked up in the local cache; otherwise the
    /// outermost member or class definition is scanned to fill the cache, as
    /// the upstream classDef helper does.
    fn class_def(&mut self, ctx: &mut CompilerContext, c: SymbolId) -> Option<TreeId> {
        if let Some(def) = self.classdefs.get(&c) {
            return Some(*def);
        }
        if let Some(member) = self.outermost_member_def {
            let mut map = ClassMap::new();
            crate::tree::jc_tree::accept(ctx, member, &mut map);
            self.classdefs.extend(map.classdefs);
            if let Some(def) = self.classdefs.get(&c) {
                return Some(*def);
            }
        }
        if let Some(outer) = self.outermost_class_def {
            let mut map = ClassMap::new();
            crate::tree::jc_tree::accept(ctx, outer, &mut map);
            self.classdefs.extend(map.classdefs);
        }
        self.classdefs.get(&c).copied()
    }

    /// Create an empty anonymous class definition owned by the given class.
    ///
    /// The class symbol is entered in the owner scope and the compiled table;
    /// the synthetic class declaration is optionally prepended to the owner
    /// definitions, exactly as the upstream makeEmptyClass does.
    fn make_empty_class(
        &mut self,
        ctx: &mut CompilerContext,
        flags: i64,
        owner: SymbolId,
        flatname: Option<Name>,
        add_to_defs: bool,
    ) -> TreeId {
        let empty_name = self.names.empty.clone();
        let c = {
            let mut syms = self.syms.borrow_mut();
            syms.define_class(ctx, empty_name, Some(owner))
        };
        let flat = match flatname {
            Some(flat) => flat,
            None => self.chk.borrow_mut().local_class_name(ctx, c),
        };
        let sourcefile = match &ctx.sym(owner).kind_data {
            SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile.clone(),
            _ => panic!("ClassCastException: class symbol expected"),
        };
        let members = ctx.alloc_scope(crate::code::scope::Scope::new_scope_impl(Some(c)));
        if let SymbolKind::ClassSymbol {
            flatname: flat_slot,
            sourcefile: source_slot,
            members_field,
            ..
        } = &mut ctx.sym_mut(c).kind_data
        {
            *flat_slot = flat;
            *source_slot = sourcefile;
            *members_field = Some(members);
        }
        ctx.sym_mut(c).flags_field = flags;
        let ctype = ctx.sym(c).ty.expect("class type");
        let object_type = self.syms.borrow().object_type;
        if let TypeKindData::ClassType {
            supertype_field,
            interfaces_field,
            ..
        } = &mut ctx.typ_mut(ctype).kind
        {
            *supertype_field = Some(object_type);
            *interfaces_field = Some(List::nil());
        }
        let odef = self.class_def(ctx, owner).expect("owner class definition");
        let odef_pos = ctx.tree(odef).pos;
        self.enter_synthetic(ctx, c, members);
        self.chk.borrow_mut().put_compiled(ctx, c);
        let mods = self.make.borrow_mut().modifiers(ctx, flags);
        let empty_name = self.names.empty.clone();
        let cdef = self.make.borrow_mut().class_def(
            ctx,
            mods,
            empty_name,
            List::nil(),
            None,
            List::nil(),
            List::nil(),
        );
        if let TreeKind::ClassDecl { sym: sym_slot, .. } = &mut ctx.tree_mut(cdef).kind {
            *sym_slot = Some(c);
        }
        ctx.tree_mut(cdef).ty = Some(ctype);
        ctx.tree_mut(cdef).pos = odef_pos;
        if add_to_defs
            && let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(odef).kind
        {
            let new_defs = defs.prepend(cdef);
            *defs = new_defs;
        }
        cdef
    }

    /// Return the anonymous cache class of the current top-level class.
    ///
    /// The first nested class with an empty name is reused; otherwise a
    /// synthetic empty class is created, as the upstream outerCacheClass does.
    fn outer_cache_class(&mut self, ctx: &mut CompilerContext) -> SymbolId {
        let outermost = self.outermost_class_def.expect("outermost class definition");
        let clazz = match &ctx.tree(outermost).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("class declaration expected"),
        };
        let members = ctx.sym(clazz).members().expect("class members");
    let scope_snapshot = ctx.scope(members).clone();
        let symbols = scope_snapshot.get_symbols(
            members,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            ctx,
        );
        for sym in symbols {
            if ctx.sym(sym).kind == Kind::TYP
                && ctx.sym(sym).name == self.names.empty
                && (ctx.sym(sym).flags_field & crate::code::flags::INTERFACE as i64) == 0
            {
                return sym;
            }
        }
        let flags =
            (crate::code::flags::STATIC | crate::code::flags::SYNTHETIC) as i64;
        let cdef = self.make_empty_class(ctx, flags, clazz, None, true);
        match &ctx.tree(cdef).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("class declaration expected"),
        }
    }

    /// Return (or create) the enum switch mapping for an enum class.
    ///
    /// The mapping is created lazily, stores the switch-map field symbol and the
    /// per-constant assigned integers, and is shared by all switches over the
    /// same enum in one top-level class.
    fn map_for_enum(
        &mut self,
        ctx: &mut CompilerContext,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        enum_class: SymbolId,
    ) -> Rc<RefCell<EnumMapping>> {
        if let Some(map) = self.enum_switch_map.get(&enum_class) {
            return Rc::clone(map);
        }
        let map = Rc::new(RefCell::new(EnumMapping::new(self, ctx, pos, enum_class)));
        self.enum_switch_map
            .insert(enum_class, Rc::clone(&map));
        map
    }


    /// Return the variables of the local class owner that are accessed from
    /// within the given local class.
    ///
    /// The result is cached; a non-local owner copies the free variables of
    /// its enclosing local class when it is a subclass of that class, and an
    /// otherwise unconstrained class has no free variables (None mirrors the
    /// upstream nullable return of the cache lookup).
    fn freevars(
        &mut self,
        ctx: &mut CompilerContext,
        c: SymbolId,
    ) -> Option<List<SymbolId>> {
        if let Some(fvs) = self.freevar_cache.get(&c) {
            return Some(fvs.clone());
        }
        let owner = ctx.sym(c).owner.expect("local class owner");
        if ctx.sym(owner).kind.matches(crate::code::kinds::KindSelector::VAL_MTH) {
            let class_tree = self.class_def(ctx, c).expect("local class definition");
            let mut collector = FreeVarCollector::new(self, c, owner);
            collector.scan(ctx, Some(class_tree));
            let fvs = collector.fvs.clone();
            self.freevar_cache.insert(c, fvs.clone());
            Some(fvs)
        } else if let Some(copy_owner) = self.owner_to_copy_free_vars_from(ctx, c) {
            let fvs = self.freevar_cache.get(&copy_owner).cloned();
            self.freevar_cache.insert(c, fvs.clone().unwrap_or_else(List::nil));
            fvs
        } else {
            Some(List::nil())
        }
    }

    /// Return the local class whose free variables are inherited by the given
    /// class, if the upstream ownership rules allow the copy.
    ///
    /// A class copies the free variables of the nearest enclosing local class
    /// when it is a member of a value/method owner and is a subclass of that
    /// class; otherwise the upstream helper returns null.
    fn owner_to_copy_free_vars_from(
        &mut self,
        ctx: &mut CompilerContext,
        c: SymbolId,
    ) -> Option<SymbolId> {
        if !Symbol::is_directly_or_indirectly_local(ctx, c) {
            return None;
        }
        let mut current_owner = ctx.sym(c).owner.expect("class owner");
        while let Some(owner_of_owner) = ctx.sym(current_owner).owner {
            if !ctx.sym(owner_of_owner)
                .kind
                .matches(crate::code::kinds::KindSelector::TYP)
            {
                break;
            }
            if !Symbol::is_directly_or_indirectly_local(ctx, current_owner) {
                break;
            }
            current_owner = owner_of_owner;
        }
        let outer_owner = ctx.sym(current_owner).owner?;
        let is_sub = {
            let syms = self.syms.borrow();
            crate::code::symbol::symbol_is_sub_class(ctx, &self.types, &syms, c, current_owner)
        };
        if ctx.sym(outer_owner)
            .kind
            .matches(crate::code::kinds::KindSelector::VAL_MTH)
            && is_sub
        {
            Some(current_owner)
        } else {
            None
        }
    }


    /// Compute the access code of a tree use relative to its enclosing
    /// operator node.
    ///
    /// The code identifies how the value is used (plain dereference, assignment,
    /// increment/decrement or a compound assignment) and drives which accessor
    /// method the accessor synthesis selects.
    fn access_code(&self, ctx: &CompilerContext, tree: TreeId, encl_op: Option<TreeId>) -> i32 {
        let Some(encl_op) = encl_op else {
            return crate::code::symbol::AccessCode::DEREF.code();
        };
        let encl_tag = ctx.tree(encl_op).get_tag();
        if encl_tag == Tag::ASSIGN
            && let TreeKind::Assign { lhs: Some(lhs), .. } = &ctx.tree(encl_op).kind
            && tree == tree_info::skip_parens(ctx, *lhs)
        {
            return crate::code::symbol::AccessCode::ASSIGN.code();
        }
        if (encl_tag.is_inc_or_dec_unary_op() || encl_tag.is_assignop())
            && let Some(operand) = Self::operator_operand_left(ctx, encl_op)
            && tree == tree_info::skip_parens(ctx, operand)
        {
            let operator = match &ctx.tree(encl_op).kind {
                TreeKind::Unary { operator, .. } | TreeKind::AssignOp { operator, .. } => {
                    *operator
                }
                _ => None,
            };
            if let Some(operator) = operator {
                let mut operator_symbol = ctx.sym(operator).clone();
                return operator_symbol.get_access_code(encl_tag);
            }
        }
        crate::code::symbol::AccessCode::DEREF.code()
    }

    /// Return the left operand of an operator expression tree.
    ///
    /// Unary operators have their operand in arg and compound assignments in
    /// lhs; the helper is the Rust form of getOperand(LEFT).
    fn operator_operand_left(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
        match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            TreeKind::AssignOp { lhs, .. } => *lhs,
            TreeKind::Binary { lhs, .. } => *lhs,
            _ => None,
        }
    }

    /// Return the binary operator matching an access code and tag.
    ///
    /// The access code comparison uses the non-cached AccessCode.from value,
    /// which is observably equivalent to the cached getAccessCode result because
    /// the cache only memoizes the same computation.
    fn binary_access_operator(&mut self, ctx: &mut CompilerContext, acode: i32, tag: Tag) -> SymbolId {
        let operators = self.operators();
        let mut syms = self.syms.borrow_mut();
        let found = operators.lookup_binary_op(ctx, &mut syms, &self.types, &|ctx, _syms, sym| {
            let access = match &ctx.sym(sym).kind_data {
                SymbolKind::OperatorSymbol { opcode, .. } => {
                    crate::code::symbol::AccessCode::from(tag, *opcode)
                }
                _ => crate::code::symbol::AccessCode::DEREF.code(),
            };
            access == acode
        });
        found.expect("binary access operator")
    }

    /// Return the tree tag of the compound assignment corresponding to a
    /// binary operator.
    ///
    /// The mapping follows the backend opcode stored on the operator symbol, as
    /// the upstream treeTag helper does.
    fn tree_tag(operator: SymbolId, ctx: &CompilerContext) -> Tag {
        use crate::jvm::byte_codes as bc;
        let opcode = match &ctx.sym(operator).kind_data {
            SymbolKind::OperatorSymbol { opcode, .. } => *opcode,
            _ => panic!("AssertionError: operator symbol expected"),
        };
        match opcode {
            bc::IOR | bc::LOR => Tag::BITOR_ASG,
            bc::IXOR | bc::LXOR => Tag::BITXOR_ASG,
            bc::IAND | bc::LAND => Tag::BITAND_ASG,
            bc::ISHL | bc::LSHL | bc::ISHLL | bc::LSHLL => Tag::SL_ASG,
            bc::ISHR | bc::LSHR | bc::ISHRL | bc::LSHRL => Tag::SR_ASG,
            bc::IUSHR | bc::LUSHR | bc::IUSHRL | bc::LUSHRL => Tag::USR_ASG,
            bc::IADD | bc::LADD | bc::FADD | bc::DADD | bc::STRING_ADD => Tag::PLUS_ASG,
            bc::ISUB | bc::LSUB | bc::FSUB | bc::DSUB => Tag::MINUS_ASG,
            bc::IMUL | bc::LMUL | bc::FMUL | bc::DMUL => Tag::MUL_ASG,
            bc::IDIV | bc::LDIV | bc::FDIV | bc::DDIV => Tag::DIV_ASG,
            bc::IMOD | bc::LMOD | bc::FMOD | bc::DMOD => Tag::MOD_ASG,
            _ => panic!("AssertionError: unexpected operator opcode"),
        }
    }

    /// Return the name of the access method with the given number and code.
    ///
    /// The name embeds both numbers so that the synthesized methods can be
    /// recognized in stack traces while staying unique per symbol and code.
    fn access_name(&self, anum: i32, acode: i32) -> Name {
        let text = format!(
            "access{}{}{}{}",
            self.target.synthetic_name_char(),
            anum,
            acode / 10,
            acode % 10
        );
        self.names
            .from_string(&javac_strings::JavaString::from(text.as_str()))
    }

    /// Clone a symbol with a different owner.
    ///
    /// The upstream Symbol.clone(Symbol) copies flags, name and type while
    /// replacing the owner; the ported clone/alloc pair preserves exactly that
    /// state and registers the mapping to the actual symbol.
    fn clone_symbol_with_owner(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        owner: SymbolId,
        actual_symbols: &mut HashMap<SymbolId, SymbolId>,
    ) -> SymbolId {
        let mut cloned = ctx.sym(sym).clone();
        cloned.owner = Some(owner);
        let cloned_id = ctx.alloc_symbol(cloned);
        actual_symbols.insert(cloned_id, sym);
        cloned_id
    }

    /// Find or create the access method for a private or protected symbol.
    ///
    /// The access number and the access method table are kept per (possibly
    /// re-owned) symbol; the method type is derived from the symbol kind and the
    /// access code, exactly as the upstream accessSymbol helper computes it.
    fn access_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        tree: TreeId,
        encl_op: Option<TreeId>,
        prot_access: bool,
        ref_super: bool,
    ) -> SymbolId {
        let acc_owner = if ref_super && prot_access {
            match &ctx.tree(tree).kind {
                TreeKind::FieldAccess {
                    selected: Some(selected),
                    ..
                } => ctx
                    .tree(*selected)
                    .ty
                    .and_then(|ty| ctx.typ(ty).tsym)
                    .expect("access owner symbol"),
                _ => panic!("AssertionError: qualified super access expected"),
            }
        } else {
            self.access_class(ctx, sym, prot_access, tree)
        };
        let vsym = if ctx.sym(sym).owner == Some(acc_owner) {
            sym
        } else {
            let mut actual = std::mem::take(&mut self.actual_symbols);
            let cloned = self.clone_symbol_with_owner(ctx, sym, acc_owner, &mut actual);
            self.actual_symbols = actual;
            cloned
        };
        let anum = match self.access_nums.get(&vsym) {
            Some(anum) => *anum,
            None => {
                let anum = self.accessed.len() as i32;
                self.access_nums.insert(vsym, anum);
                self.access_syms
                    .insert(vsym, vec![None; crate::code::symbol::AccessCode::number_of_access_codes() as usize]);
                self.accessed.push(vsym);
                anum
            }
        };
        let vsym_kind = ctx.sym(vsym).kind;
        let vsym_ty = ctx.sym(vsym).ty.expect("access symbol type");
        let expanded = self.types.erasure(ctx, &self.syms.borrow(), vsym_ty);
        let (mut acode, argtypes, restype, thrown) = match vsym_kind {
            Kind::VAR => {
                let code = self.access_code(ctx, tree, encl_op);
                let argtypes = if code >= crate::code::symbol::AccessCode::FIRSTASGOP.code() {
                    let tag = ctx
                        .tree(encl_op.expect("assignop access needs the operator"))
                        .get_tag();
                    let operator = self.binary_access_operator(ctx, code, tag);
                    let opcode = match &ctx.sym(operator).kind_data {
                        SymbolKind::OperatorSymbol { opcode, .. } => *opcode,
                        _ => panic!("AssertionError: operator symbol expected"),
                    };
                    if opcode == crate::jvm::byte_codes::STRING_ADD {
                        List::of(self.syms.borrow().object_type)
                    } else {
                        let operator_ty = ctx.sym(operator).ty.expect("operator type");
                        let params = self.types.get_parameter_types_public(ctx, operator_ty);
                        params.tail()
                    }
                } else if code == crate::code::symbol::AccessCode::ASSIGN.code() {
                    List::of(self.types.erasure(ctx, &self.syms.borrow(), vsym_ty))
                } else {
                    List::nil()
                };
                (code, argtypes, expanded, List::nil())
            }
            Kind::MTH => {
                let params = self.types.get_parameter_types_public(ctx, expanded);
                let restype = self.types.get_return_type_public(ctx, expanded);
                let thrown = ctx.typ(vsym_ty).get_thrown_types(ctx);
                (
                    crate::code::symbol::AccessCode::DEREF.code(),
                    params,
                    restype,
                    thrown,
                )
            }
            _ => panic!("AssertionError: unexpected access symbol kind"),
        };
        if prot_access && ref_super {
            acode += 1;
        }
        let mut argtypes = argtypes;
        if (ctx.sym(vsym).flags_field & crate::code::flags::STATIC as i64) == 0 {
            let owner = ctx.sym(vsym).owner.expect("access symbol owner");
            let owner_ty = ctx.sym(owner).ty.expect("access symbol owner type");
            let owner_erasure = self.types.erasure(ctx, &self.syms.borrow(), owner_ty);
            argtypes = argtypes.prepend(owner_erasure);
        }
        let index = acode as usize;
        if let Some(cached) = self
            .access_syms
            .get(&vsym)
            .and_then(|accessors| accessors.get(index).copied().flatten())
        {
            return cached;
        }
        let acc_owner_interface = ctx.sym(acc_owner).is_interface();
        let mut flags = crate::code::flags::STATIC | crate::code::flags::SYNTHETIC;
        if acc_owner_interface {
            flags |= crate::code::flags::PUBLIC;
        }
        let name = self.access_name(anum, acode);
        let method_class = self.syms.borrow().method_class;
        let method_ty = ctx.alloc_type(Type::new(
            TypeTag::METHOD,
            Some(method_class),
            TypeKindData::MethodType {
                argtypes,
                restype: Some(restype),
                thrown,
                recvtype: None,
            },
        ));
        let accessor_symbol = Symbol::new_method_symbol(flags as i64, name, method_ty, acc_owner);
        let accessor = ctx.alloc_symbol(accessor_symbol);
        if let Some(accessors) = self.access_syms.get_mut(&vsym) {
            accessors[index] = Some(accessor);
        }
        let members = ctx.sym(acc_owner).members().expect("access owner members");
        self.enter_synthetic(ctx, accessor, members);
        accessor
    }

    /// Return the class in which the access method for a symbol goes.
    ///
    /// For protected cross-package accesses the method is placed in the nearest
    /// enclosing subclass of the symbol owner; private accesses are placed in
    /// the owner's enclosing class.
    fn access_class(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        prot_access: bool,
        tree: TreeId,
    ) -> SymbolId {
        if prot_access {
            let mut c = self.current_class.expect("current class");
            if ctx.tree(tree).has_tag(Tag::SELECT)
                && (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) == 0
            {
                let qualifier = match &ctx.tree(tree).kind {
                    TreeKind::FieldAccess {
                        selected: Some(selected),
                        ..
                    } => ctx
                        .tree(*selected)
                        .ty
                        .and_then(|ty| ctx.typ(ty).tsym)
                        .expect("qualifier symbol"),
                    _ => panic!("AssertionError: qualified access expected"),
                };
                loop {
                    let is_sub = {
                        let syms = self.syms.borrow();
                        crate::code::symbol::symbol_is_sub_class(
                            ctx, &self.types, &syms, qualifier, c,
                        )
                    };
                    if is_sub {
                        return c;
                    }
                    let owner = ctx.sym(c).owner.expect("class owner");
                    c = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
                }
            } else {
                let sym_owner = ctx.sym(sym).owner.expect("symbol owner");
                loop {
                    let is_sub = {
                        let syms = self.syms.borrow();
                        crate::code::symbol::symbol_is_sub_class(
                            ctx, &self.types, &syms, c, sym_owner,
                        )
                    };
                    if is_sub {
                        return c;
                    }
                    let owner = ctx.sym(c).owner.expect("class owner");
                    c = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
                }
            }
        } else {
            let owner = ctx.sym(sym).owner.expect("symbol owner");
            crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class")
        }
    }


    /// Ensure a tree accesses its symbol through the required access path.
    ///
    /// The tree is rewritten according to the symbol kind: free variables are
    /// replaced by their proxies (or constant values), type identifiers are
    /// flattened, and private/protected or outer-class member accesses are
    /// routed through synthesized access methods or qualified receivers.
    fn access(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        tree: TreeId,
        encl_op: Option<TreeId>,
        ref_super: bool,
    ) -> TreeId {
        let mut sym = sym;
        let mut tree = tree;
        let current_class = self.current_class.expect("current class");
        loop {
            if ctx.sym(sym).kind != Kind::VAR {
                break;
            }
            let owner = ctx.sym(sym).owner.expect("free variable owner");
            if ctx.sym(owner).kind != Kind::MTH {
                break;
            }
            let owner_encl = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
            if Some(owner_encl) == Some(current_class) {
                break;
            }
            if let Some(constant) = ctx.sym(sym).get_const_value() {
                let sym_ty = ctx.sym(sym).ty.expect("variable type");
                return self.make_lit(ctx, sym_ty, &constant);
            }
            if let Some(translation) = &self.lambda_translation_map
                && let Some(translated) = translation.get(&sym).copied()
            {
                return self.make.borrow_mut().ident_symbol(ctx, translated);
            }
            let proxy = match self.proxies.get(&sym) {
                Some(proxy) => *proxy,
                None => break,
            };
            assert!(
                (ctx.sym(proxy).flags_field & crate::code::flags::FINAL as i64) != 0,
                "AssertionError: proxy is not final"
            );
            sym = proxy;
            tree = self.make.borrow_mut().ident_symbol(ctx, sym);
        }
        let base = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => None,
        };
        let sym_kind = ctx.sym(sym).kind;
        match sym_kind {
            Kind::TYP => {
                let owner = ctx.sym(sym).owner.expect("type owner");
                if ctx.sym(owner).kind != Kind::PCK {
                    let flatname = ctx.sym(sym).flat_name();
                    let shortname = crate::util::convert::short_name(&flatname);
                    let mut base = base;
                    while let Some(current_base) = base {
                        if let Some(base_sym) = tree_info::symbol(ctx, current_base)
                            && ctx.sym(base_sym).kind != Kind::PCK
                        {
                            base = match &ctx.tree(current_base).kind {
                                TreeKind::FieldAccess { selected, .. } => *selected,
                                _ => None,
                            };
                        } else {
                            break;
                        }
                    }
                    match &mut ctx.tree_mut(tree).kind {
                        TreeKind::Ident { name, .. } => {
                            *name = shortname;
                        }
                        TreeKind::FieldAccess { name, .. } => {
                            *name = shortname;
                        }
                        _ => {
                            if base.is_none() {
                                tree = self.make.borrow_mut().ident_symbol(ctx, sym);
                                if let TreeKind::Ident { name, .. } = &mut ctx.tree_mut(tree).kind {
                                    *name = shortname;
                                }
                            } else if let Some(base) = base
                                && let TreeKind::FieldAccess { selected, name, .. } =
                                    &mut ctx.tree_mut(tree).kind
                            {
                                *selected = Some(base);
                                *name = shortname;
                            }
                        }
                    }
                }
                tree
            }
            Kind::MTH | Kind::VAR => {
                let owner = ctx.sym(sym).owner.expect("member owner");
                if ctx.sym(owner).kind == Kind::TYP {
                    let prot_access = (ref_super && !self.needs_private_access(ctx, sym))
                        || self.needs_protected_access(ctx, sym, tree);
                    let acc_req = prot_access || self.needs_private_access(ctx, sym);
                    let predef_class = self.syms.borrow().predef_class;
                    let base_req = base.is_none()
                        && Some(owner) != Some(predef_class)
                        && !{
                            let syms = self.syms.borrow();
                            crate::code::symbol::symbol_is_member_of(
                                ctx, &self.types, &syms, sym, current_class,
                            )
                        };
                    if acc_req || base_req {
                        if ctx.sym(sym).kind == Kind::VAR
                            && let Some(constant) = ctx.sym(sym).get_const_value()
                        {
                            self.add_pruned_info(tree);
                            let sym_ty = ctx.sym(sym).ty.expect("variable type");
                            return self.make_lit(ctx, sym_ty, &constant);
                        }
                        if acc_req {
                            let mut args = List::nil();
                            let mut base = base;
                            if (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) == 0 {
                                if base.is_none() {
                                    base = Some(self.make_owner_this(ctx, ctx.tree(tree).pos, sym, true));
                                }
                                args = args.prepend(base.expect("access base"));
                                base = None;
                            }
                            let access = self.access_symbol(
                                ctx, sym, tree, encl_op, prot_access, ref_super,
                            );
                            let access_owner = ctx.sym(access).owner.expect("access owner");
                            let receiver = if let Some(base) = base {
                                self.select_symbol(ctx, base, access)
                            } else {
                                let qual = {
                                    self.make.borrow_mut().qual_ident(ctx, access_owner)
                                };
                                self.select_symbol(ctx, qual, access)
                            };
                            return self.make.borrow_mut().app_with_args(ctx, receiver, args);
                        } else if base_req {
                            let tree_pos = ctx.tree(tree).pos;
                            let pos: Rc<dyn DiagnosticPosition> = Rc::new(
                                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(tree_pos),
                            );
                            let base = self.access_base(ctx, &pos, sym);
                            let selected = self.select_symbol(ctx, base, sym);
                            if let Some(ty) = ctx.tree(tree).ty {
                                ctx.tree_mut(selected).ty = Some(ty);
                            }
                            return selected;
                        }
                    }
                } else if ctx.sym(owner).kind == Kind::MTH
                    && let Some(translation) = &self.lambda_translation_map
                {
                    let base_sym = crate::code::symbol::base_symbol(ctx, sym);
                    if let Some(translated) = translation.get(&base_sym).copied() {
                        tree = self.make.borrow_mut().ident_symbol(ctx, translated);
                    }
                }
                tree
            }
            _ => tree,
        }
    }

    /// Return the qualifier used to access a symbol of an outer class.
    ///
    /// Static symbols are qualified by the owner class; instance symbols by the
    /// synthesized (or implicit) outer-this chain.
    fn access_base(
        &mut self,
        ctx: &mut CompilerContext,
        pos: &Rc<dyn DiagnosticPosition>,
        sym: SymbolId,
    ) -> TreeId {
        if (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) != 0 {
            let owner = ctx.sym(sym).owner.expect("symbol owner");
            let qual = {
                self.make.borrow_mut().qual_ident(ctx, owner)
            };
            self.access(ctx, sym, qual, None, false)
        } else {
            self.make_owner_this_diag(ctx, pos, sym, true)
        }
    }

    /// Determine whether a symbol needs an access method for private access.
    ///
    /// Trusted targets with nestmate support never need one; private members of
    /// the current class and private constructors of directly or indirectly
    /// local classes are relaxed to package access and do not need one either.
    fn needs_private_access(&mut self, ctx: &mut CompilerContext, sym: SymbolId) -> bool {
        if self.target.has_nestmate_access() {
            return false;
        }
        let current_class = self.current_class.expect("current class");
        if (ctx.sym(sym).flags_field & crate::code::flags::PRIVATE as i64) == 0
            || ctx.sym(sym).owner == Some(current_class)
        {
            return false;
        }
        let is_init = ctx.sym(sym).name == self.names.init;
        let owner = ctx.sym(sym).owner.expect("symbol owner");
        if is_init && Symbol::is_directly_or_indirectly_local(ctx, owner) {
            ctx.sym_mut(sym).flags_field &= !(crate::code::flags::PRIVATE as i64);
            return false;
        }
        true
    }

    /// Determine whether a symbol needs an access method for protected access
    /// from another package.
    fn needs_protected_access(&mut self, ctx: &mut CompilerContext, sym: SymbolId, tree: TreeId) -> bool {
        if self.disable_protected_accessors {
            return false;
        }
        if (ctx.sym(sym).flags_field & crate::code::flags::PROTECTED as i64) == 0 {
            return false;
        }
        let current_class = self.current_class.expect("current class");
        let owner = ctx.sym(sym).owner.expect("symbol owner");
        let current_owner = ctx.sym(current_class).owner;
        if ctx.sym(owner).owner == current_owner {
            return false;
        }
        let sym_packge = crate::code::symbol::packge(ctx, sym);
        let current_packge = crate::code::symbol::packge(ctx, current_class);
        if sym_packge == current_packge {
            return false;
        }
        let is_sub = {
            let syms = self.syms.borrow();
            crate::code::symbol::symbol_is_sub_class(ctx, &self.types, &syms, current_class, owner)
        };
        if !is_sub {
            return true;
        }
        if (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) != 0
            || !ctx.tree(tree).has_tag(Tag::SELECT)
        {
            return false;
        }
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess {
                selected: Some(selected),
                ..
            } => *selected,
            _ => return false,
        };
        if tree_info::name(ctx, selected) == Some(self.names._super.clone()) {
            return false;
        }
        let selected_tsym = ctx
            .tree(selected)
            .ty
            .and_then(|ty| ctx.typ(ty).tsym);
        match selected_tsym {
            Some(selected_tsym) => {
                let syms = self.syms.borrow();
                !crate::code::symbol::symbol_is_sub_class(
                    ctx, &self.types, &syms, selected_tsym, current_class,
                )
            }
            None => false,
        }
    }


    /// Construct a tree simulating the expression C.this.
    ///
    /// If the qualifier class is the current class the ordinary this expression
    /// is used; otherwise the tree is built through the this$n chain.
    fn make_this(
        &mut self,
        ctx: &mut CompilerContext,
        pos: &Rc<dyn DiagnosticPosition>,
        c: SymbolId,
    ) -> TreeId {
        if self.current_class == Some(c) {
            let c_ty = ctx.sym(c).ty.expect("qualifier type");
            let erasure = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, c_ty)
            };
            self.make.borrow_mut().this_expr(ctx, erasure)
        } else {
            self.make_outer_this(ctx, pos, c)
        }
    }

    /// Construct a tree representing the outer instance of the given class.
    ///
    /// The outer-this chain is walked until the requested class is reached; an
    /// empty chain means the attribution stage failed to catch a missing outer
    /// instance, which is reported and turned into the assertion failure the
    /// upstream helper raises.
    fn make_outer_this(
        &mut self,
        ctx: &mut CompilerContext,
        pos: &Rc<dyn DiagnosticPosition>,
        c: SymbolId,
    ) -> TreeId {
        let mut ots = self.outer_this_stack.clone();
        if ots.is_empty() {
            self.report_no_outer_instance(ctx, pos, c);
            panic!("AssertionError: no outer instance in scope");
        }
        let mut ot = ots[0];
        let ident = self.make.borrow_mut().ident_symbol(ctx, ot);
        let mut tree = self.access(ctx, ot, ident, None, false);
        let mut otc = ctx
            .sym(ot)
            .ty
            .and_then(|ty| ctx.typ(ty).tsym)
            .expect("outer this type symbol");
        while otc != c {
            loop {
                ots.remove(0);
                if ots.is_empty() {
                    self.report_no_outer_instance(ctx, pos, c);
                    return tree;
                }
                ot = ots[0];
                if ctx.sym(ot).owner == Some(otc) {
                    break;
                }
            }
            let owner_kind = ctx.sym(otc).owner.map(|owner| ctx.sym(owner).kind);
            if owner_kind != Some(Kind::PCK) && !ctx.sym(otc).has_outer_instance(ctx) {
                self.chk.borrow().early_ref_error(
                    ctx,
                    &self.log,
                    pos.get_preferred_position(),
                    c,
                );
                panic!("AssertionError: class has no outer instance");
            }
            let select = self.select_symbol(ctx, tree, ot);
            tree = self.access(ctx, ot, select, None, false);
            otc = ctx
                .sym(ot)
                .ty
                .and_then(|ty| ctx.typ(ty).tsym)
                .expect("outer this type symbol");
        }
        tree
    }

    /// Construct a tree for the closest outer instance such that the symbol is
    /// a member of it (precise match) or of a subtype of its owner.
    fn make_owner_this(
        &mut self,
        ctx: &mut CompilerContext,
        tree_pos: i32,
        sym: SymbolId,
        precise_match: bool,
    ) -> TreeId {
        let pos: Rc<dyn DiagnosticPosition> = Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(tree_pos),
        );
        self.make_owner_this_diag(ctx, &pos, sym, precise_match)
    }

    /// Construct an owner-this tree using an existing diagnostic position.
    fn make_owner_this_diag(
        &mut self,
        ctx: &mut CompilerContext,
        pos: &Rc<dyn DiagnosticPosition>,
        sym: SymbolId,
        precise_match: bool,
    ) -> TreeId {
        let sym_owner = ctx.sym(sym).owner.expect("symbol owner");
        let current_class = self.current_class.expect("current class");
        let matches = if precise_match {
            let syms = self.syms.borrow();
            crate::code::symbol::symbol_is_member_of(ctx, &self.types, &syms, sym, current_class)
        } else {
            let syms = self.syms.borrow();
            crate::code::symbol::symbol_is_sub_class(
                ctx, &self.types, &syms, current_class, sym_owner,
            )
        };
        if matches {
            let owner_ty = ctx.sym(sym_owner).ty.expect("symbol owner type");
            let erasure = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, owner_ty)
            };
            self.make.borrow_mut().this_expr(ctx, erasure)
        } else {
            self.make_owner_this_n(ctx, pos, sym, precise_match)
        }
    }

    /// Construct an owner-this tree that never picks the current this.
    ///
    /// The outer-this chain is walked until an instance whose class contains
    /// the symbol (precise match) or is a subclass of the symbol owner is
    /// reached.
    fn make_owner_this_n(
        &mut self,
        ctx: &mut CompilerContext,
        pos: &Rc<dyn DiagnosticPosition>,
        sym: SymbolId,
        precise_match: bool,
    ) -> TreeId {
        let sym_owner = ctx.sym(sym).owner.expect("symbol owner");
        let mut ots = self.outer_this_stack.clone();
        if ots.is_empty() {
            self.report_no_outer_instance(ctx, pos, sym_owner);
            panic!("AssertionError: no outer instance in scope");
        }
        let mut ot = ots[0];
        let ident = self.make.borrow_mut().ident_symbol(ctx, ot);
        let mut tree = self.access(ctx, ot, ident, None, false);
        let mut otc = ctx
            .sym(ot)
            .ty
            .and_then(|ty| ctx.typ(ty).tsym)
            .expect("outer this type symbol");
        loop {
            let matched = if precise_match {
                let syms = self.syms.borrow();
                crate::code::symbol::symbol_is_member_of(ctx, &self.types, &syms, sym, otc)
            } else {
                let syms = self.syms.borrow();
                crate::code::symbol::symbol_is_sub_class(
                    ctx, &self.types, &syms, otc, sym_owner,
                )
            };
            if matched {
                return tree;
            }
            loop {
                ots.remove(0);
                if ots.is_empty() {
                    self.report_no_outer_instance(ctx, pos, sym_owner);
                    return tree;
                }
                ot = ots[0];
                if ctx.sym(ot).owner == Some(otc) {
                    break;
                }
            }
            let select = self.select_symbol(ctx, tree, ot);
            tree = self.access(ctx, ot, select, None, false);
            otc = ctx
                .sym(ot)
                .ty
                .and_then(|ty| ctx.typ(ty).tsym)
                .expect("outer this type symbol");
        }
    }

    /// Report a missing enclosing instance of the requested class.
    fn report_no_outer_instance(
        &self,
        ctx: &CompilerContext,
        pos: &Rc<dyn DiagnosticPosition>,
        c: SymbolId,
    ) {
        let info = crate::resources::compiler_properties::errors::no_encl_instance_of_type_in_scope(
            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                ctx.sym(c).name.to_java_string().to_string_lossy().to_string(),
            ),
        );
        crate::util::abstract_log::AbstractLog::error_at(
            &mut *self.log.borrow_mut(),
            pos.get_preferred_position(),
            crate::util::jc_diagnostic::Error::from(info),
        );
    }


    /// Build a type tree for an attributed type.
    ///
    /// The maker needs the symbol table to qualify class names; the borrow is
    /// kept local so callers can continue using the context afterwards.
    fn type_tree(&mut self, ctx: &mut CompilerContext, t: TypeId) -> TreeId {
        self.make.borrow_mut().type_from_type(ctx, t)
    }

    /// Build a variable declaration tree for a variable symbol.
    ///
    /// The declaration takes its modifiers from the symbol flags, its type from
    /// the symbol type and the provided initializer; the symbol is attached to
    /// the tree so later phases can resolve it directly.
    fn var_def_for_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        init: Option<TreeId>,
    ) -> TreeId {
        let flags = ctx.sym(sym).flags_field;
        let name = ctx.sym(sym).name.clone();
        let vartype = ctx.sym(sym).ty.expect("variable type");
        let mods = self.make.borrow_mut().modifiers(ctx, flags);
        let type_tree = self.type_tree(ctx, vartype);
        let tree = self.make.borrow_mut().var_def(
            ctx,
            mods,
            name,
            Some(type_tree),
            init,
        );
        if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(sym);
        }
        ctx.tree_mut(tree).ty = Some(vartype);
        tree
    }

    /// Build a parameter declaration tree for an argument type.
    ///
    /// An existing parameter symbol is reused when its type list matches the
    /// argument list; otherwise a fresh parameter symbol named x&lt;i&gt; is
    /// created, exactly as the upstream Params helper does.
    fn param_def(
        &mut self,
        ctx: &mut CompilerContext,
        owner: Option<SymbolId>,
        index: usize,
        argtype: TypeId,
        existing: Option<SymbolId>,
    ) -> TreeId {
        let param = match existing {
            Some(param) => param,
            None => {
                let name = self.make.borrow_mut().param_name(index as i32);
                let owner = owner.expect("parameter owner");
                let flags = crate::code::flags::PARAMETER;
                let symbol = Symbol::new_var_symbol(flags, name, argtype, owner);
                ctx.alloc_symbol(symbol)
            }
        };
        self.var_def_for_symbol(ctx, param, None)
    }

    /// Build a method declaration tree from a method symbol and body.
    ///
    /// The declaration mirrors what TreeMaker.MethodDef(MethodSymbol, JCBlock)
    /// produces for a synthetic symbol: modifiers from the symbol flags, the
    /// return type unless the name is the constructor name, existing parameter
    /// symbols, the thrown types and the supplied body.
    fn method_def_for_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        m: SymbolId,
        mtype: TypeId,
        body: Option<TreeId>,
    ) -> TreeId {
        let flags = ctx.sym(m).flags_field;
        let name = ctx.sym(m).name.clone();
        let is_init = name == self.names.init;
        let argtypes = self.types.get_parameter_types_public(ctx, mtype);
        let params_symbols = match &ctx.sym(m).kind_data {
            SymbolKind::MethodSymbol { params, .. } => params.clone(),
            _ => List::nil(),
        };
        let arg_count = {
            let mut count = 0usize;
            let mut current = argtypes.clone();
            while current.non_empty() {
                count += 1;
                current = current.tail();
            }
            count
        };
        let mut params_count = 0usize;
        let mut params_current = params_symbols.clone();
        while params_current.non_empty() {
            params_count += 1;
            params_current = params_current.tail();
        }
        let use_existing = params_count > 0 && params_count == arg_count;
        let mut params: List<TreeId> = List::nil();
        let mut args_current = argtypes.clone();
        let mut args_index = 0usize;
        let mut symbols_current = params_symbols.clone();
        while args_current.non_empty() {
            let argtype = *args_current.head();
            let existing = if use_existing {
                let symbol = *symbols_current.head();
                symbols_current = symbols_current.tail();
                Some(symbol)
            } else {
                None
            };
            let param = self.param_def(ctx, Some(m), args_index, argtype, existing);
            params = params.append(param);
            args_index += 1;
            args_current = args_current.tail();
        }
        let restype = if is_init {
            None
        } else {
            let return_ty = self.types.get_return_type_public(ctx, mtype);
            Some(self.type_tree(ctx, return_ty))
        };
        let thrown_types = ctx.typ(mtype).get_thrown_types(ctx);
        let mut thrown: List<TreeId> = List::nil();
        let mut thrown_current = thrown_types;
        while thrown_current.non_empty() {
            let tree = self.type_tree(ctx, *thrown_current.head());
            thrown = thrown.append(tree);
            thrown_current = thrown_current.tail();
        }
        let mods = self.make.borrow_mut().modifiers(ctx, flags);
        let tree = self.make.borrow_mut().method_def(
            ctx,
            mods,
            name,
            restype,
            List::nil(),
            None,
            params,
            thrown,
            body,
            None,
        );
        if let TreeKind::MethodDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(m);
        }
        ctx.tree_mut(tree).ty = Some(mtype);
        tree
    }


    /// Return the name of the proxy variable for a free variable.
    ///
    /// The name embeds the source name and, when a duplicate would arise, a
    /// disambiguating index, matching the upstream proxyName helper.
    fn proxy_name(&self, name: &Name, index: i32) -> Name {
        let text = format!(
            "val{}{}",
            self.target.synthetic_name_char(),
            name.to_java_string()
        );
        let text = if index > 0 {
            format!("{}{}{}", text, self.target.synthetic_name_char(), index)
        } else {
            text
        };
        self.names
            .from_string(&javac_strings::JavaString::from(text.as_str()))
    }

    /// Create proxy definitions for all free variables in the given order.
    ///
    /// Each proxy is a final synthetic variable in the class owner; the proxy
    /// map records the replacement so later accesses are rewritten, and the
    /// definitions are prepended so the list ends up in the upstream order.
    fn freevar_defs(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        freevars: List<SymbolId>,
        owner: SymbolId,
        additional_flags: i64,
    ) -> List<TreeId> {
        let flags = (crate::code::flags::FINAL | crate::code::flags::SYNTHETIC) as i64
            | additional_flags;
        let mut defs: List<TreeId> = List::nil();
        let mut proxy_names: Vec<Name> = Vec::new();
        let mut current = freevars;
        self.make.borrow_mut().at(pos);
        while current.non_empty() {
            let v = *current.head();
            let mut index = 0;
            let mut proxy_name = self.proxy_name(&ctx.sym(v).name.clone(), index);
            index += 1;
            while proxy_names.iter().any(|n| n == &proxy_name) {
                proxy_name = self.proxy_name(&ctx.sym(v).name.clone(), index);
                index += 1;
            }
            proxy_names.push(proxy_name.clone());
            let v_ty = ctx.sym(v).ty.expect("free variable type");
            let erasure = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, v_ty)
            };
            let symbol = Symbol::new_var_symbol(flags, proxy_name, erasure, owner);
            let proxy = ctx.alloc_symbol(symbol);
            self.proxies.insert(v, proxy);
            let vd = self.var_def_for_symbol(ctx, proxy, None);
            let vartype = match &ctx.tree(vd).kind {
                TreeKind::VariableDecl { vartype, .. } => *vartype,
                _ => None,
            };
            if let Some(vartype) = vartype {
                let accessed = self.access(ctx, v, vartype, None, false);
                if let TreeKind::VariableDecl { vartype, .. } = &mut ctx.tree_mut(vd).kind {
                    *vartype = Some(accessed);
                }
            }
            defs = defs.prepend(vd);
            current = current.tail();
        }
        defs
    }

    /// Return the name of a synthetic outer-this field.
    ///
    /// The nesting level of the enclosing type decides the numeric suffix; an
    /// existing member with the same name causes the synthetic character to be
    /// appended until the name is free.
    fn outer_this_name(&mut self, ctx: &mut CompilerContext, ty: TypeId, owner: SymbolId) -> Name {
        let mut current = ctx.typ(ty).get_enclosing_type();
        let mut nesting_level = 0;
        while let Some(enclosing) = current {
            if ctx.typ(enclosing).has_tag(TypeTag::CLASS) {
                current = ctx.typ(enclosing).get_enclosing_type();
                nesting_level += 1;
            } else {
                break;
            }
        }
        let text = format!("this{}{}", self.target.synthetic_name_char(), nesting_level);
        let mut result = self
            .names
            .from_string(&javac_strings::JavaString::from(text.as_str()));
        while ctx.sym(owner).kind == Kind::TYP {
            let members = ctx.sym(owner).members().expect("class members");
            let arena = ctx.symbols_arena();
            let found = ctx
                .scope(members)
                .find_first(&result, &|_| true, arena)
                .is_some();
            if !found {
                break;
            }
            let next = format!("{}{}", result.to_java_string(), self.target.synthetic_name_char());
            result = self
                .names
                .from_string(&javac_strings::JavaString::from(next.as_str()));
        }
        result
    }

    /// Create the symbol of a synthetic outer-this variable.
    ///
    /// The erasure of the enclosing type of the owner class becomes the
    /// variable type; the symbol is pushed on the outer-this stack so
    /// subsequent owner-this lookups see it.
    fn make_outer_this_var_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        owner: SymbolId,
        flags: i64,
    ) -> SymbolId {
        let encl = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
        let encl_ty = ctx.sym(encl).ty.expect("enclosing class type");
        let enclosing = ctx
            .typ(encl_ty)
            .get_enclosing_type()
            .expect("class has no enclosing type");
        let target = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, enclosing)
        };
        let name = self.outer_this_name(ctx, target, owner);
        let symbol = Symbol::new_var_symbol(flags, name, target, owner);
        let outer_this = ctx.alloc_symbol(symbol);
        self.outer_this_stack.insert(0, outer_this);
        outer_this
    }

    /// Create the declaration of a synthetic outer-this variable.
    fn make_outer_this_var_decl(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        sym: SymbolId,
    ) -> TreeId {
        self.make.borrow_mut().at(pos);
        let vd = self.var_def_for_symbol(ctx, sym, None);
        let vartype = match &ctx.tree(vd).kind {
            TreeKind::VariableDecl { vartype, .. } => *vartype,
            _ => None,
        };
        if let Some(vartype) = vartype {
            let owner = ctx.sym(sym).owner.expect("outer this owner");
            let accessed = self.access(ctx, owner, vartype, None, false);
            if let TreeKind::VariableDecl { vartype, .. } = &mut ctx.tree_mut(vd).kind {
                *vartype = Some(accessed);
            }
        }
        vd
    }

    /// Create the outer-this definition for a method or class owner.
    ///
    /// Constructors of anonymous classes and non-private inner member classes
    /// carry the mandated flag; the definition is an extra parameter of the
    /// method or a synthetic field of the class.
    fn outer_this_def(&mut self, ctx: &mut CompilerContext, pos: i32, owner: SymbolId) -> TreeId {
        let owner_is_method = ctx.sym(owner).kind == Kind::MTH;
        let encl = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
        let is_mandated = if owner_is_method {
            let is_constructor = ctx.sym(owner).name == self.names.init;
            let is_anonymous = ctx.sym(owner).name == self.names.empty
                && ctx.sym(encl).is_anonymous();
            let c = encl;
            let inner = ctx.sym(c).has_outer_instance(ctx);
            let private = crate::code::symbol::Symbol::is_private(ctx.sym(c));
            let static_flag = (ctx.sym(c).flags_field & crate::code::flags::STATIC as i64) != 0;
            (is_constructor && is_anonymous) || (is_constructor && inner && !private && !static_flag)
        } else {
            false
        };
        let flags = (crate::code::flags::FINAL as i64)
            | (crate::code::flags::PARAMETER)
            | if is_mandated {
                crate::code::flags::MANDATED as i64
            } else {
                crate::code::flags::SYNTHETIC as i64
            };
        let outer_this = self.make_outer_this_var_symbol(ctx, owner, flags);
        if owner_is_method
            && let SymbolKind::MethodSymbol { extra_params, .. } = &mut ctx.sym_mut(owner).kind_data
        {
            *extra_params = extra_params.prepend(outer_this);
        }
        self.make_outer_this_var_decl(ctx, pos, outer_this)
    }

    /// Return the trees that load the given free variables, in reverse order.
    fn load_freevars(&mut self, ctx: &mut CompilerContext, pos: i32, freevars: List<SymbolId>) -> List<TreeId> {
        let mut args: List<TreeId> = List::nil();
        let mut current = freevars;
        while current.non_empty() {
            let v = *current.head();
            self.make.borrow_mut().at(pos);
            let ident = self.make.borrow_mut().ident_symbol(ctx, v);
            let loaded = self.access(ctx, v, ident, None, false);
            args = args.prepend(loaded);
            current = current.tail();
        }
        args
    }

    /// Create the assignment this.name = name used to initialize a captured
    /// field of an inner class from the corresponding constructor parameter.
    fn init_field(&mut self, ctx: &mut CompilerContext, pos: i32, rhs: SymbolId, lhs: SymbolId) -> TreeId {
        self.make.borrow_mut().at(pos);
        let lhs_owner = ctx.sym(lhs).owner.expect("field owner");
        let lhs_owner_ty = ctx.sym(lhs_owner).ty.expect("field owner type");
        let owner_erasure = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, lhs_owner_ty)
        };
        let this = self.make.borrow_mut().this_expr(ctx, owner_erasure);
        let field = self.select_symbol(ctx, this, lhs);
        let value = self.make.borrow_mut().ident_symbol(ctx, rhs);
        let assign = self.make.borrow_mut().assign(ctx, field, value);
        let lhs_erasure = {
            let syms = self.syms.borrow();
            let lhs_ty = ctx.sym(lhs).ty.expect("field type");
            self.types.erasure(ctx, &syms, lhs_ty)
        };
        ctx.tree_mut(assign).ty = Some(lhs_erasure);
        self.make.borrow_mut().exec(ctx, assign)
    }

    /// Create the assignment this.this$n = this$n for nested classes.
    fn init_outer_this(&mut self, ctx: &mut CompilerContext, pos: i32) -> TreeId {
        let rhs = self.outer_this_stack.first().copied().expect("outer this");
        let lhs = self.outer_this_stack.get(1).copied().expect("outer this field");
        self.init_field(ctx, pos, rhs, lhs)
    }

    /// Build an attributed call left.name(args) by resolving the method first.
    fn make_call(&mut self, ctx: &mut CompilerContext, left: TreeId, name: Name, args: List<TreeId>) -> TreeId {
        assert!(ctx.tree(left).ty.is_some(), "AssertionError: left has no type");
        let left_ty = ctx.tree(left).ty.expect("left type");
        let argtypes = self.tree_types(ctx, &args);
        let pos = self.make_pos.clone();
        let funcsym = self.lookup_method(ctx, pos, name, left_ty, argtypes);
        let select = self.select_symbol(ctx, left, funcsym);
        self.make.borrow_mut().app_with_args(ctx, select, args)
    }


    /// Return the access constructor for a private constructor, or the
    /// constructor itself when no access constructor is needed.
    ///
    /// The access constructor takes the original parameters plus the access
    /// constructor tag; enum constructors additionally receive the name and
    /// ordinal parameters.
    fn access_constructor(
        &mut self,
        ctx: &mut CompilerContext,
        _pos: &Rc<dyn DiagnosticPosition>,
        constr: SymbolId,
    ) -> SymbolId {
        if !self.needs_private_access(ctx, constr) {
            return constr;
        }
        let owner = ctx.sym(constr).owner.expect("constructor owner");
        let acc_owner = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
        if let Some(access) = self.access_constrs.get(&constr) {
            return *access;
        }
        let constr_ty = ctx.sym(constr).ty.expect("constructor type");
        let mut argtypes = self.types.get_parameter_types_public(ctx, constr_ty);
        if (ctx.sym(acc_owner).flags_field & crate::code::flags::ENUM as i64) != 0 {
            let string_type = self.syms.borrow().string_type;
            let int_type = self.syms.borrow().int_type;
            argtypes = argtypes.prepend(int_type).prepend(string_type);
        }
        let tag = self.access_constructor_tag(ctx);
        let tag_erasure = {
            let syms = self.syms.borrow();
            let tag_ty = ctx.sym(tag).ty.expect("tag type");
            self.types.erasure(ctx, &syms, tag_ty)
        };
        let restype = self.types.get_return_type_public(ctx, constr_ty);
        let thrown = ctx.typ(constr_ty).get_thrown_types(ctx);
        let method_class = self.syms.borrow().method_class;
        let method_ty = ctx.alloc_type(Type::new(
            TypeTag::METHOD,
            Some(method_class),
            TypeKindData::MethodType {
                argtypes: argtypes.append(tag_erasure),
                restype: Some(restype),
                thrown,
                recvtype: None,
            },
        ));
        let init_name = self.names.init.clone();
        let flags = crate::code::flags::SYNTHETIC as i64;
        let accessor_symbol = Symbol::new_method_symbol(flags, init_name, method_ty, acc_owner);
        let accessor = ctx.alloc_symbol(accessor_symbol);
        let members = ctx.sym(acc_owner).members().expect("access owner members");
        self.enter_synthetic(ctx, accessor, members);
        self.access_constrs.insert(constr, accessor);
        self.accessed.push(constr);
        accessor
    }

    /// Return an anonymous class nested in this top-level class, used as the
    /// tag type of access constructors.
    fn access_constructor_tag(&mut self, ctx: &mut CompilerContext) -> SymbolId {
        let current_class = self.current_class.expect("current class");
        let top_class =
            crate::code::symbol::symbol_outermost_class(ctx, current_class).expect("top level class");
        let top_modle = crate::code::symbol::packge(ctx, top_class)
            .and_then(|packge| match &ctx.sym(packge).kind_data {
                SymbolKind::PackageSymbol { modle, .. } => *modle,
                _ => None,
            })
            .expect("top level module");
        let mut i = 1;
        loop {
            let text = format!(
                "{}{}{}",
                ctx.sym(top_class).get_qualified_name().to_java_string(),
                self.target.synthetic_name_char(),
                i
            );
            let flatname = self
                .names
                .from_string(&javac_strings::JavaString::from(text.as_str()));
            let found = self.chk.borrow().get_compiled_by(top_modle, &flatname);
            let ctag = match found {
                None => {
                    let flags =
                        (crate::code::flags::STATIC | crate::code::flags::SYNTHETIC) as i64;
                    let cdef = self.make_empty_class(ctx, flags, top_class, None, true);
                    match &ctx.tree(cdef).kind {
                        TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                        _ => panic!("class declaration expected"),
                    }
                }
                Some(ctag) => {
                    if !ctx.sym(ctag).is_anonymous() {
                        i += 1;
                        continue;
                    }
                    ctag
                }
            };
            self.access_constr_tags.insert(0, ctag);
            return ctag;
        }
    }

    /// Verify that all synthesized access constructor tags are generated.
    ///
    /// A tag hidden in dead code is not translated, so an empty class is created
    /// here and the accessor signatures are rewritten to reference it.
    fn check_access_constructor_tags(&mut self, ctx: &mut CompilerContext) {
        let tags = self.access_constr_tags.clone();
        for c in tags {
            if self.is_translated_class_available(ctx, c) {
                continue;
            }
            let outer = crate::code::symbol::symbol_outermost_class(ctx, c).expect("outermost class");
            let flatname = match &ctx.sym(c).kind_data {
                SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
                _ => panic!("class symbol expected"),
            };
            let flags = (crate::code::flags::STATIC | crate::code::flags::SYNTHETIC) as i64;
            let cdef = self.make_empty_class(ctx, flags, outer, Some(flatname), false);
            let new_ctag = match &ctx.tree(cdef).kind {
                TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                _ => panic!("class declaration expected"),
            };
            self.swap_access_constructor_tag(ctx, c, new_ctag);
            self.translated.push(cdef);
        }
    }

    /// Check whether a translated class declaration for the symbol exists.
    fn is_translated_class_available(&self, ctx: &CompilerContext, c: SymbolId) -> bool {
        for tree in &self.translated {
            if ctx.tree(*tree).has_tag(Tag::CLASSDEF)
                && let TreeKind::ClassDecl { sym: Some(sym), .. } = &ctx.tree(*tree).kind
                && *sym == c
            {
                return true;
            }
        }
        false
    }

    /// Rewrite access constructor signatures to reference a newly created tag.
    fn swap_access_constructor_tag(&mut self, ctx: &mut CompilerContext, old_ctag: SymbolId, new_ctag: SymbolId) {
        let constructors: Vec<SymbolId> = self.access_constrs.values().copied().collect();
        for constr in constructors {
            let constr_ty = ctx.sym(constr).ty.expect("constructor type");
            assert!(
                ctx.typ(constr_ty).has_tag(TypeTag::METHOD),
                "AssertionError: class constructor is not a method"
            );
            let params = self.types.get_parameter_types_public(ctx, constr_ty);
            let first = *params.head();
            let first_tsym = ctx.typ(first).tsym;
            if first_tsym != Some(old_ctag) {
                continue;
            }
            let new_erasure = {
                let syms = self.syms.borrow();
                let new_ty = ctx.sym(new_ctag).ty.expect("tag type");
                self.types.erasure(ctx, &syms, new_ty)
            };
            let dropped = params.tail();
            let new_params = dropped.prepend(new_erasure);
            let new_ty = self
                .types
                .create_method_type_with_parameters(ctx, constr_ty, new_params);
            ctx.sym_mut(constr).ty = Some(new_ty);
        }
    }

    /// Add all required access methods for a private symbol to its enclosing
    /// class.
    fn make_accessible(&mut self, ctx: &mut CompilerContext, sym: SymbolId) {
        let owner = ctx.sym(sym).owner.expect("symbol owner");
        let encl = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
        let cdef = self.class_def(ctx, encl).expect("class definition");
        let cdef_pos = ctx.tree(cdef).pos;
        let is_init = ctx.sym(sym).name == self.names.init;
        if is_init {
            let accessor = self.access_constrs.get(&sym).copied();
            if let Some(accessor) = accessor {
                let def = self.access_constructor_def(ctx, cdef_pos, sym, accessor);
                if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(cdef).kind {
                    let new_defs = defs.prepend(def);
                    *defs = new_defs;
                }
            }
        } else {
            let accessors = self.access_syms.get(&sym).cloned();
            if let Some(accessors) = accessors {
                for (i, accessor) in accessors.iter().enumerate() {
                    if let Some(accessor) = accessor {
                        let def = self.access_def(ctx, cdef_pos, sym, *accessor, i as i32);
                        if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(cdef).kind {
                            let new_defs = defs.prepend(def);
                            *defs = new_defs;
                        }
                    }
                }
            }
        }
    }

    /// Construct the definition of an access method.
    ///
    /// The method body reads, writes or updates the symbol through its
    /// reference and returns the value where the access code requires it.
    fn access_def(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        vsym: SymbolId,
        accessor: SymbolId,
        acode: i32,
    ) -> TreeId {
        let vsym_owner = ctx.sym(vsym).owner.expect("access symbol owner");
        let current_class =
            crate::code::symbol::symbol_encl_class(ctx, vsym_owner).expect("enclosing class");
        self.current_class = Some(current_class);
        self.make.borrow_mut().at(pos);
        let accessor_ty = ctx.sym(accessor).ty.expect("accessor type");
        let md = self.method_def_for_symbol(ctx, accessor, accessor_ty, None);
        let params = match &ctx.tree(md).kind {
            TreeKind::MethodDecl { params, .. } => params.clone(),
            _ => panic!("method declaration expected"),
        };
        let sym = self.actual_symbols.get(&vsym).copied().unwrap_or(vsym);
        let is_static = (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) != 0;
        let (reference, args) = if is_static {
            let ident = self.make.borrow_mut().ident_symbol(ctx, sym);
            let args = self.make.borrow_mut().idents(ctx, params.clone());
            (ident, args)
        } else {
            let first = *params.head();
            let first_sym = Self::param_sym(ctx, first);
            let site = self.make.borrow_mut().ident_symbol(ctx, first_sym);
            if acode % 2 != 0 {
                let owner = ctx.sym(vsym).owner.expect("access owner");
                let encl = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
                let encl_ty = ctx.sym(encl).ty.expect("enclosing type");
                let super_ty = {
                    let syms = self.syms.borrow();
                    self.types.supertype(ctx, &syms, encl_ty)
                };
                let erasure = {
                    let syms = self.syms.borrow();
                    self.types.erasure(ctx, &syms, super_ty)
                };
                ctx.tree_mut(site).ty = Some(erasure);
            }
            let reference = self.select_symbol(ctx, site, sym);
            let tail = params.tail();
            let args = self.make.borrow_mut().idents(ctx, tail);
            (reference, args)
        };
        let sym_kind = ctx.sym(sym).kind;
        let stat = if sym_kind == Kind::VAR {
            let acode1 = acode - (acode & 1);
            let a_code = crate::code::symbol::AccessCode::get_from_code(acode1);
            let expr = match a_code {
                crate::code::symbol::AccessCode::DEREF => reference,
                crate::code::symbol::AccessCode::ASSIGN => {
                    let rhs = *args.head();
                    self.make.borrow_mut().assign(ctx, reference, rhs)
                }
                crate::code::symbol::AccessCode::PREINC
                | crate::code::symbol::AccessCode::POSTINC
                | crate::code::symbol::AccessCode::PREDEC
                | crate::code::symbol::AccessCode::POSTDEC => {
                    let tag = a_code.tag();
                    self.make_unary(ctx, tag, reference)
                }
                _ => {
                    let tag = Tag::NO_TAG;
                    let operator = self.binary_access_operator(ctx, acode1, tag);
                    let tree_tag = Self::tree_tag(operator, ctx);
                    let rhs = *args.head();
                    let expr = self.make_assignop(ctx, tree_tag, reference, rhs);
                    if let TreeKind::AssignOp { operator: slot, .. } = &mut ctx.tree_mut(expr).kind {
                        *slot = Some(operator);
                    }
                    expr
                }
            };
            let sym_ty = ctx.sym(sym).ty.expect("accessed variable type");
            ctx.tree_mut(expr).ty = Some(sym_ty);
            self.make.borrow_mut().return_(ctx, Some(expr))
        } else {
            let apply = self.make.borrow_mut().app_with_args(ctx, reference, args.clone());
            self.make.borrow_mut().call_statement(ctx, apply)
        };
        let body = self.make.borrow_mut().block(ctx, 0, List::of(stat));
        if let TreeKind::MethodDecl { body: body_slot, .. } = &mut ctx.tree_mut(md).kind {
            *body_slot = Some(body);
        }
        let params = match &ctx.tree(md).kind {
            TreeKind::MethodDecl { params, .. } => params.clone(),
            _ => panic!("method declaration expected"),
        };
        let mut current = params;
        while current.non_empty() {
            let param = *current.head();
            let vartype = match &ctx.tree(param).kind {
                TreeKind::VariableDecl { vartype, .. } => *vartype,
                _ => None,
            };
            if let Some(vartype) = vartype {
                let param_sym = Self::param_sym(ctx, param);
                let accessed = self.access(ctx, param_sym, vartype, None, false);
                if let TreeKind::VariableDecl { vartype, .. } = &mut ctx.tree_mut(param).kind {
                    *vartype = Some(accessed);
                }
            }
            current = current.tail();
        }
        {
            let restype = match &ctx.tree(md).kind {
                TreeKind::MethodDecl { restype, .. } => *restype,
                _ => None,
            };
            if let Some(restype) = restype {
                let owner = ctx.sym(accessor).owner.expect("accessor owner");
                let accessed = self.access(ctx, owner, restype, None, false);
                if let TreeKind::MethodDecl { restype, .. } = &mut ctx.tree_mut(md).kind {
                    *restype = Some(accessed);
                }
            }
        }
        {
            let thrown = match &ctx.tree(md).kind {
                TreeKind::MethodDecl { thrown, .. } => thrown.clone(),
                _ => List::nil(),
            };
            let mut new_thrown: List<TreeId> = List::nil();
            let mut current = thrown;
            while current.non_empty() {
                let tree = *current.head();
                let owner = ctx.sym(accessor).owner.expect("accessor owner");
                let accessed = self.access(ctx, owner, tree, None, false);
                new_thrown = new_thrown.append(accessed);
                current = current.tail();
            }
            if let TreeKind::MethodDecl { thrown, .. } = &mut ctx.tree_mut(md).kind {
                *thrown = new_thrown;
            }
        }
        md
    }

    /// Return the symbol of a variable declaration tree.
    fn param_sym(ctx: &CompilerContext, tree: TreeId) -> SymbolId {
        match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("variable declaration expected"),
        }
    }

    /// Construct the definition of an access constructor.
    ///
    /// The constructor body delegates to the private constructor, forwarding
    /// every parameter except the trailing access constructor tag.
    fn access_constructor_def(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        constr: SymbolId,
        accessor: SymbolId,
    ) -> TreeId {
        self.make.borrow_mut().at(pos);
        let external = {
            let syms = self.syms.borrow();
            crate::code::symbol::external_type(ctx, &self.types, &syms, &self.names, accessor)
        };
        let md = self.method_def_for_symbol(ctx, accessor, external, None);
        let params = match &ctx.tree(md).kind {
            TreeKind::MethodDecl { params, .. } => params.clone(),
            _ => panic!("method declaration expected"),
        };
        let this_name = self.names._this.clone();
        let callee = self.make.borrow_mut().ident(ctx, this_name);
        if let TreeKind::Ident {
            sym: sym_slot,
            ..
        } = &mut ctx.tree_mut(callee).kind
        {
            *sym_slot = Some(constr);
        }
        let constr_ty = ctx.sym(constr).ty.expect("constructor type");
        ctx.tree_mut(callee).ty = Some(constr_ty);
        let forwarded = params.reverse().tail().reverse();
        let args = self.make.borrow_mut().idents(ctx, forwarded);
        let apply = self.make.borrow_mut().app_with_args(ctx, callee, args);
        let call = self.make.borrow_mut().call_statement(ctx, apply);
        let body = self.make.borrow_mut().block(ctx, 0, List::of(call));
        if let TreeKind::MethodDecl { body: body_slot, .. } = &mut ctx.tree_mut(md).kind {
            *body_slot = Some(body);
        }
        md
    }


    /// Translate a whole top-level class and return all translated classes.
    ///
    /// The pass state is reset for the unit, the class is translated through the
    /// visitor framework, and the queued accessors, enum switch maps and
    /// conflict checks are completed before the translated classes are returned.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - env: the attribution environment of the class being lowered;
    ///   - cdef: the class definition tree to translate;
    ///   - make: the tree factory to use for the translated unit.
    ///
    /// # Returns
    ///   - the translated classes, in reporting order.
    ///
    /// # Panics
    ///   - the input tree is not a class definition, or an internal lowering
    ///     invariant is violated.
    pub fn translate_top_level_class(
        &mut self,
        ctx: &mut CompilerContext,
        env: Rc<Env<AttrContext>>,
        cdef: TreeId,
        make: Rc<RefCell<TreeMaker>>,
    ) -> List<TreeId> {
        self.attr_env = Some(env.clone());
        self.make = make;
        self.end_pos_table = env
            .toplevel
            .and_then(|cu| match &ctx.tree(cu).kind {
                TreeKind::CompilationUnit { end_positions, .. } => end_positions.clone(),
                _ => None,
            });
        self.current_class = None;
        self.current_method_def = None;
        self.outermost_class_def = if ctx.tree(cdef).has_tag(Tag::CLASSDEF) {
            Some(cdef)
        } else {
            None
        };
        self.outermost_member_def = None;
        self.translated = Vec::new();
        self.classdefs = HashMap::new();
        self.actual_symbols = HashMap::new();
        self.freevar_cache = HashMap::new();
        self.proxies = HashMap::new();
        self.twr_vars = Some(ctx.alloc_scope(crate::code::scope::Scope::new_scope_impl(
            Some(self.syms.borrow().no_symbol),
        )));
        self.outer_this_stack = Vec::new();
        self.access_nums = HashMap::new();
        self.access_syms = HashMap::new();
        self.access_constrs = HashMap::new();
        self.access_constr_tags = Vec::new();
        self.accessed = Vec::new();
        self.encl_op = None;
        let _ = self.translate(ctx, Some(cdef));
        let accessed = self.accessed.clone();
        for sym in accessed {
            self.make_accessible(ctx, sym);
        }
        let maps: Vec<Rc<RefCell<EnumMapping>>> = self.enum_switch_map.values().cloned().collect();
        for map in maps {
            let mut mapping = map.borrow_mut();
            mapping.translate(self, ctx);
        }
        let translated = self.translated.clone();
        self.check_conflicts(ctx, &translated);
        self.check_access_constructor_tags(ctx);
        let result = List::from_iterable(self.translated.clone());
        self.attr_env = None;
        self.end_pos_table = None;
        self.current_class = None;
        self.current_method_def = None;
        self.outermost_class_def = None;
        self.outermost_member_def = None;
        self.translated = Vec::new();
        self.classdefs = HashMap::new();
        self.actual_symbols = HashMap::new();
        self.freevar_cache = HashMap::new();
        self.proxies = HashMap::new();
        self.outer_this_stack = Vec::new();
        self.access_nums = HashMap::new();
        self.access_syms = HashMap::new();
        self.access_constrs = HashMap::new();
        self.access_constr_tags = Vec::new();
        self.accessed = Vec::new();
        self.enum_switch_map.clear();
        self.assertions_disabled_class_cache = None;
        result
    }

    /// Check that synthesized names do not conflict with user declared
    /// symbols in the translated classes.
    fn check_conflicts(&mut self, ctx: &mut CompilerContext, translated: &[TreeId]) {
        let mut checker = ConflictsChecker {
            current_class: None,
            types: Rc::clone(&self.types),
            syms: Rc::clone(&self.syms),
            conflicts: Vec::new(),
        };
        for tree in translated {
            crate::tree::jc_tree::accept(ctx, *tree, &mut checker);
        }
        let conflicts = checker.conflicts;
        for (tree, sym) in conflicts {
            self.synthetic_error(ctx, tree, sym);
        }
    }

    /// Report one synthetic name conflict diagnostic.
    fn synthetic_error(&mut self, ctx: &mut CompilerContext, tree: TreeId, sym: SymbolId) {
        let sym_ty = ctx.sym(sym).ty.expect("symbol type");
        if ctx.typ(sym_ty).has_tag(TypeTag::ERROR) {
            return;
        }
        let location = ctx
            .sym(sym)
            .get_qualified_name()
            .to_java_string()
            .to_string_lossy()
            .to_string();
        let info = crate::resources::compiler_properties::errors::cannot_generate_class(
            crate::resources::diagnostic_contract::SymbolArg::placeholder(location.clone()),
            crate::resources::diagnostic_contract::DiagnosticArg::Text(location),
        );
        crate::util::abstract_log::AbstractLog::error_at(
            &mut *self.log.borrow_mut(),
            ctx.tree(tree).pos,
            crate::util::jc_diagnostic::Error::from(info),
        );
    }


    /// Translate a single tree through the visitor.
    ///
    /// The tree position becomes the maker position, the visitor runs with a
    /// fresh result slot, and the result is returned; a null tree translates to
    /// a null result.
    fn translate(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        let tree = tree?;
        let saved_result = self.result.take();
        self.result = None;
        let pos = ctx.tree(tree).pos;
        self.make.borrow_mut().at(pos);
        crate::tree::jc_tree::accept(ctx, tree, self);
        let result = self.result.take().or(Some(tree));
        self.result = saved_result;
        result
    }

    /// Translate a tree and box or unbox it to the given prototype type.
    fn translate_with_type(
        &mut self,
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        ty: TypeId,
    ) -> Option<TreeId> {
        let translated = self.translate(ctx, tree)?;
        Some(self.box_if_needed(ctx, translated, ty))
    }

    /// Translate a tree with the given closest enclosing operator node.
    fn translate_with_encl_op(
        &mut self,
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        encl_op: Option<TreeId>,
    ) -> Option<TreeId> {
        let previous = self.encl_op;
        self.encl_op = encl_op;
        let result = self.translate(ctx, tree);
        self.encl_op = previous;
        result
    }

    /// Translate a list of trees, preserving the list order.
    fn translate_list(&mut self, ctx: &mut CompilerContext, mut trees: List<TreeId>) -> List<TreeId> {
        let mut out: List<TreeId> = List::nil();
        while trees.non_empty() {
            let tree = *trees.head();
            let translated = self.translate(ctx, Some(tree)).expect("translated tree");
            out = out.append(translated);
            trees = trees.tail();
        }
        out
    }

    /// Translate a list of trees with a prototype type for each element.
    fn translate_list_with_type(
        &mut self,
        ctx: &mut CompilerContext,
        mut trees: List<TreeId>,
        ty: TypeId,
    ) -> List<TreeId> {
        let mut out: List<TreeId> = List::nil();
        while trees.non_empty() {
            let tree = *trees.head();
            let translated = self
                .translate_with_type(ctx, Some(tree), ty)
                .expect("translated tree");
            out = out.append(translated);
            trees = trees.tail();
        }
        out
    }


    /// Return the flags of a modifiers tree (zero when the tree is absent).
    fn mods_flags(ctx: &CompilerContext, mods: Option<TreeId>) -> i64 {
        match mods {
            Some(mods) => match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => *flags,
                _ => panic!("expected modifiers"),
            },
            None => 0,
        }
    }

    /// Replace the flags of a modifiers tree.
    fn set_mods_flags(ctx: &mut CompilerContext, mods: Option<TreeId>, flags: i64) {
        if let Some(mods) = mods
            && let TreeKind::Modifiers { flags: slot, .. } = &mut ctx.tree_mut(mods).kind
        {
            *slot = flags;
        }
    }

    /// Return the type parameters of the current class for a method body.
    fn make_translation_map(&mut self, ctx: &CompilerContext, tree: TreeId) -> HashMap<SymbolId, SymbolId> {
        let params = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { params, .. } => params.clone(),
            _ => panic!("expected a method declaration"),
        };
        let mut translation_map: HashMap<SymbolId, SymbolId> = HashMap::new();
        let mut current = params;
        while current.non_empty() {
            let vd = *current.head();
            if let TreeKind::VariableDecl { sym: Some(p), .. } = &ctx.tree(vd).kind {
                let base = crate::code::symbol::base_symbol(ctx, *p);
                if base != *p {
                    translation_map.insert(base, *p);
                }
            }
            current = current.tail();
        }
        translation_map
    }

    /// Translate a class declaration and queue it as a translated class.
    fn lower_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, extending, implementing, sym) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                mods,
                extending,
                implementing,
                sym,
                ..
            } => (*mods, *extending, implementing.clone(), *sym),
            _ => panic!("expected a class declaration"),
        };
        let tree_sym = sym.expect("class symbol");
        let prev_attr_env = self.attr_env.clone();
        let prev_class = self.current_class;
        let prev_method_sym = self.current_method_sym;
        self.current_class = Some(tree_sym);
        self.current_method_sym = None;
        let removed = self.type_envs.borrow_mut().remove(tree_sym);
        self.attr_env = Some(match removed {
            Some(env) => env,
            None => prev_attr_env.clone().expect("attribution environment"),
        });
        self.classdefs.insert(tree_sym, tree);
        let prev_proxies = std::mem::take(&mut self.proxies);
        self.proxies = prev_proxies.clone();
        let prev_outer_this_stack = std::mem::take(&mut self.outer_this_stack);
        self.outer_this_stack = prev_outer_this_stack.clone();
        let pos = ctx.tree(tree).pos;
        let flags = Self::mods_flags(ctx, mods);
        let tree_ty = ctx.sym(tree_sym).ty.expect("class type");
        let super_tsym = self
            .types
            .supertype(ctx, &self.syms.borrow(), tree_ty);
        let super_is_enum = ctx
            .typ(super_tsym)
            .tsym
            .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::ENUM as i64) != 0)
            .unwrap_or(false);
        if (flags & crate::code::flags::ENUM as i64) != 0 && !super_is_enum {
            self.visit_enum_def(ctx, tree);
        }
        if (flags & crate::code::flags::RECORD) != 0 {
            self.visit_record_def(ctx, tree);
        }
        let mut outer_this_def: Option<TreeId> = None;
        if ctx.sym(tree_sym).has_outer_instance(ctx) {
            outer_this_def = Some(self.outer_this_def(ctx, pos, tree_sym));
        }
        let freevars = self.freevars(ctx, tree_sym).unwrap_or_else(List::nil);
        let freevar_defs = self.freevar_defs(ctx, pos, freevars, tree_sym, 0);
        let extending = self.translate(ctx, extending);
        let implementing = self.translate_list(ctx, implementing);
        if let TreeKind::ClassDecl { extending: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = extending;
        }
        if let TreeKind::ClassDecl { implementing: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = implementing;
        }
        if Symbol::is_directly_or_indirectly_local(ctx, tree_sym) {
            let owner = ctx.sym(tree_sym).owner.expect("class owner");
            let encl = crate::code::symbol::symbol_encl_class(ctx, owner).expect("enclosing class");
            if let SymbolKind::ClassSymbol { trans_local, .. } = &mut ctx.sym_mut(encl).kind_data {
                *trans_local = trans_local.prepend(tree_sym);
            }
        }
        // Translate the members, re-reading the definition list because the
        // translation may prepend synthetic members to it.
        let mut seen_len: i32 = 0;
        loop {
            let defs = match &ctx.tree(tree).kind {
                TreeKind::ClassDecl { defs, .. } => defs.clone(),
                _ => panic!("expected a class declaration"),
            };
            let total = defs.length();
            let new_count = total - seen_len;
            if new_count <= 0 {
                break;
            }
            let mut current = defs;
            let previous_outermost_member = self.outermost_member_def;
            for index in 0..new_count {
                let member = *current.head();
                if index == 0 && previous_outermost_member.is_none() {
                    self.outermost_member_def = Some(member);
                }
                self.translate(ctx, Some(member));
                current = current.tail();
            }
            self.outermost_member_def = previous_outermost_member;
            seen_len = total;
        }
        // Convert the protected modifier to public and mask class flags.
        let mut mods_flags = Self::mods_flags(ctx, mods);
        if (mods_flags & crate::code::flags::PROTECTED as i64) != 0 {
            mods_flags |= crate::code::flags::PUBLIC as i64;
        }
        mods_flags &= crate::code::flags::ClassFlags as i64;
        Self::set_mods_flags(ctx, mods, mods_flags);
        let flatname = ctx.sym(tree_sym).flat_name();
        let shortname = crate::util::convert::short_name(&flatname);
        if let TreeKind::ClassDecl { name, .. } = &mut ctx.tree_mut(tree).kind {
            *name = shortname;
        }
        let members = ctx.sym(tree_sym).members().expect("class members");
        let mut current_fv = freevar_defs;
        while current_fv.non_empty() {
            let def = *current_fv.head();
            if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(tree).kind {
                let new_defs = defs.prepend(def);
                *defs = new_defs;
            }
            let def_sym = Self::param_sym(ctx, def);
            self.enter_synthetic(ctx, def_sym, members);
            current_fv = current_fv.tail();
        }
        if ctx.sym(tree_sym).has_outer_instance(ctx)
            && let Some(outer_this_def) = outer_this_def
        {
            if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(tree).kind {
                let new_defs = defs.prepend(outer_this_def);
                *defs = new_defs;
            }
            let def_sym = Self::param_sym(ctx, outer_this_def);
            self.enter_synthetic(ctx, def_sym, members);
        }
        self.proxies = prev_proxies;
        self.outer_this_stack = prev_outer_this_stack;
        self.translated.push(tree);
        self.attr_env = prev_attr_env;
        self.current_class = prev_class;
        self.current_method_sym = prev_method_sym;
        let empty = self.make.borrow_mut().block(ctx, crate::code::flags::SYNTHETIC as i64, List::nil());
        self.result = Some(empty);
    }

    /// Translate a method declaration, adding enum constructor parameters when
    /// needed.
    fn lower_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (name, sym, params) = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl {
                name, sym, params, ..
            } => (name.clone(), *sym, params.clone()),
            _ => panic!("expected a method declaration"),
        };
        let current_class = self.current_class.expect("current class");
        if name == self.names.init
            && (ctx.sym(current_class).flags_field & crate::code::flags::ENUM as i64) != 0
        {
            let tree_sym = sym.expect("method symbol");
            let pos = ctx.tree(tree).pos;
            let synthetic_char = self.target.synthetic_name_char();
            let name_text = format!("{}enum{}name", synthetic_char, synthetic_char);
            let ordinal_text = format!("{}enum{}ordinal", synthetic_char, synthetic_char);
            let name_param = self.make_named_param(ctx, pos, &name_text, true, tree_sym);
            let ord_param = self.make_named_param(ctx, pos, &ordinal_text, false, tree_sym);
            let name_param_mods = match &ctx.tree(name_param).kind {
                TreeKind::VariableDecl { mods, .. } => *mods,
                _ => None,
            };
            let name_param_sym = Self::param_sym(ctx, name_param);
            let mut flags = Self::mods_flags(ctx, name_param_mods);
            flags |= crate::code::flags::SYNTHETIC as i64;
            Self::set_mods_flags(ctx, name_param_mods, flags);
            ctx.sym_mut(name_param_sym).flags_field |= crate::code::flags::SYNTHETIC as i64;
            let ord_param_mods = match &ctx.tree(ord_param).kind {
                TreeKind::VariableDecl { mods, .. } => *mods,
                _ => None,
            };
            let ord_param_sym = Self::param_sym(ctx, ord_param);
            let mut flags = Self::mods_flags(ctx, ord_param_mods);
            flags |= crate::code::flags::SYNTHETIC as i64;
            Self::set_mods_flags(ctx, ord_param_mods, flags);
            ctx.sym_mut(ord_param_sym).flags_field |= crate::code::flags::SYNTHETIC as i64;
            let new_params = params.prepend(ord_param).prepend(name_param);
            if let TreeKind::MethodDecl { params: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = new_params;
            }
            let name_sym = Self::param_sym(ctx, name_param);
            let ord_sym = Self::param_sym(ctx, ord_param);
            if let SymbolKind::MethodSymbol { extra_params, .. } = &mut ctx.sym_mut(tree_sym).kind_data {
                *extra_params = extra_params.prepend(ord_sym).prepend(name_sym);
            }
            let old_erasure = {
                let method_ty = ctx.sym(tree_sym).ty.expect("method type");
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, method_ty)
            };
            let string_type = self.syms.borrow().string_type;
            let int_type = self.syms.borrow().int_type;
            let params = self.types.get_parameter_types_public(ctx, old_erasure);
            let restype = self.types.get_return_type_public(ctx, old_erasure);
            let thrown = ctx.typ(old_erasure).get_thrown_types(ctx);
            let method_class = self.syms.borrow().method_class;
            let new_erasure = ctx.alloc_type(Type::new(
                TypeTag::METHOD,
                Some(method_class),
                TypeKindData::MethodType {
                    argtypes: params.prepend(int_type).prepend(string_type),
                    restype: Some(restype),
                    thrown,
                    recvtype: None,
                },
            ));
            ctx.sym_mut(tree_sym).ty = Some(new_erasure);
        }
        let prev_method_def = self.current_method_def;
        let prev_method_sym = self.current_method_sym;
        self.current_method_def = Some(tree);
        self.current_method_sym = sym;
        self.lower_method_def_internal(ctx, tree);
        self.current_method_def = prev_method_def;
        self.current_method_sym = prev_method_sym;
    }

    /// Create a parameter declaration with an explicit name and type.
    fn make_named_param(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        text: &str,
        is_string: bool,
        owner: SymbolId,
    ) -> TreeId {
        let name = self
            .names
            .from_string(&javac_strings::JavaString::from(text));
        let ty = if is_string {
            self.syms.borrow().string_type
        } else {
            self.syms.borrow().int_type
        };
        let flags = crate::code::flags::PARAMETER;
        let symbol = Symbol::new_var_symbol(flags, name, ty, owner);
        let param = ctx.alloc_symbol(symbol);
        self.make.borrow_mut().at(pos);
        self.var_def_for_symbol(ctx, param, None)
    }

    /// Translate the body of a method declaration, handling inner-class
    /// constructors specially.
    fn lower_method_def_internal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (name, sym, params, body) = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl {
                name,
                sym,
                params,
                body,
                ..
            } => (name.clone(), *sym, params.clone(), *body),
            _ => panic!("expected a method declaration"),
        };
        let current_class = self.current_class.expect("current class");
        let is_ctor = name == self.names.init;
        let sym = sym.expect("method symbol");
        let is_inner = ctx.sym(current_class).has_outer_instance(ctx)
            || Symbol::is_directly_or_indirectly_local(ctx, current_class);
        if is_ctor && is_inner {
            let pos = ctx.tree(tree).pos;
            let prev_proxies = std::mem::take(&mut self.proxies);
            self.proxies = prev_proxies.clone();
            let prev_outer_this_stack = std::mem::take(&mut self.outer_this_stack);
            self.outer_this_stack = prev_outer_this_stack.clone();
            let freevars = self.freevars(ctx, current_class).unwrap_or_else(List::nil);
            let mut outer_this_def: Option<TreeId> = None;
            if ctx.sym(current_class).has_outer_instance(ctx) {
                outer_this_def = Some(self.outer_this_def(ctx, pos, sym));
            }
            let flags = crate::code::flags::PARAMETER;
            let freevar_defs = self.freevar_defs(ctx, pos, freevars.clone(), sym, flags);
            let restype = match &ctx.tree(tree).kind {
                TreeKind::MethodDecl { restype, .. } => *restype,
                _ => None,
            };
            let restype = self.translate(ctx, restype);
            let params = self.translate_list(ctx, params);
            let thrown = match &ctx.tree(tree).kind {
                TreeKind::MethodDecl { thrown, .. } => thrown.clone(),
                _ => List::nil(),
            };
            let thrown = self.translate_list(ctx, thrown);
            if let TreeKind::MethodDecl {
                restype: r, params: p, thrown: t, ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *r = restype;
                *p = params.clone();
                *t = thrown;
            }
            let Some(body) = body else {
                self.result = Some(tree);
                return;
            };
            let mut new_params = params.append_list(&freevar_defs);
            if ctx.sym(current_class).has_outer_instance(ctx)
                && let Some(outer_this_def) = outer_this_def
            {
                new_params = new_params.prepend(outer_this_def);
            }
            let body_stats = match &ctx.tree(body).kind {
                TreeKind::Block { stats, .. } => stats.clone(),
                _ => panic!("expected a block"),
            };
            let self_call = self
                .translate(ctx, Some(*body_stats.head()))
                .expect("translated super call");
            let mut added: List<TreeId> = List::nil();
            let is_initial_ctor = tree_info::is_initial_constructor(ctx, tree);
            if freevars.non_empty() {
                let mut added_argtypes: List<TypeId> = List::nil();
                let mut current = freevars;
                while current.non_empty() {
                    let v = *current.head();
                    let proxy = self.proxies.get(&v).copied().expect("proxy");
                    if let SymbolKind::MethodSymbol { captured_locals, .. } =
                        &mut ctx.sym_mut(sym).kind_data
                    {
                        *captured_locals = captured_locals.prepend(proxy);
                    }
                    if is_initial_ctor {
                        let prev_proxy = prev_proxies.get(&v).copied().expect("previous proxy");
                        let init = self.init_field(ctx, ctx.tree(body).pos, proxy, prev_proxy);
                        added = added.prepend(init);
                    }
                    let v_ty = ctx.sym(v).ty.expect("free variable type");
                    let erasure = {
                        let syms = self.syms.borrow();
                        self.types.erasure(ctx, &syms, v_ty)
                    };
                    added_argtypes = added_argtypes.prepend(erasure);
                    current = current.tail();
                }
                let old_erasure = {
                    let method_ty = ctx.sym(sym).ty.expect("method type");
                    let syms = self.syms.borrow();
                    self.types.erasure(ctx, &syms, method_ty)
                };
                let params = self.types.get_parameter_types_public(ctx, old_erasure);
                let restype = self.types.get_return_type_public(ctx, old_erasure);
                let thrown = ctx.typ(old_erasure).get_thrown_types(ctx);
                let method_class = self.syms.borrow().method_class;
                let new_erasure = ctx.alloc_type(Type::new(
                    TypeTag::METHOD,
                    Some(method_class),
                    TypeKindData::MethodType {
                        argtypes: params.append_list(&added_argtypes),
                        restype: Some(restype),
                        thrown,
                        recvtype: None,
                    },
                ));
                ctx.sym_mut(sym).ty = Some(new_erasure);
            }
            if ctx.sym(current_class).has_outer_instance(ctx) && is_initial_ctor {
                let init = self.init_outer_this(ctx, ctx.tree(body).pos);
                added = added.prepend(init);
            }
            self.proxies = prev_proxies;
            let stats = self.translate_list(ctx, body_stats.tail());
            let mut new_stats = stats.prepend(self_call);
            let mut added_rev = added;
            let mut accumulated: List<TreeId> = new_stats;
            let mut added_vec: Vec<TreeId> = Vec::new();
            while added_rev.non_empty() {
                added_vec.push(*added_rev.head());
                added_rev = added_rev.tail();
            }
            for def in added_vec.into_iter().rev() {
                accumulated = accumulated.prepend(def);
            }
            new_stats = accumulated;
            if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(body).kind {
                *slot = new_stats;
            }
            if let TreeKind::MethodDecl { params: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = new_params;
            }
            self.outer_this_stack = prev_outer_this_stack;
        } else {
            let prev_lambda_map = self.lambda_translation_map.take();
            let is_synthetic = (ctx.sym(sym).flags_field & crate::code::flags::SYNTHETIC as i64) != 0;
            let is_lambda = is_synthetic && ctx.sym(sym).name.starts_with(&self.names.lambda);
            if is_lambda {
                self.lambda_translation_map = Some(self.make_translation_map(ctx, tree));
            }
            self.visit_method_def_default(ctx, tree);
            self.lambda_translation_map = prev_lambda_map;
        }
        if name == self.names.init
            && (ctx.sym(sym).flags_field & crate::code::flags::COMPACT_RECORD_CONSTRUCTOR) != 0
        {
            self.lower_compact_record_ctor(ctx, tree);
        }
        self.result = Some(tree);
    }


    /// Box or unbox a tree when the primitive-ness differs from the prototype.
    ///
    /// A primitive value is boxed through the wrapper class valueOf method; a
    /// reference value is unboxed through the primitive-value accessor. When both
    /// sides agree the tree is returned unchanged.
    fn box_if_needed(&mut self, ctx: &mut CompilerContext, tree: TreeId, ty: TypeId) -> TreeId {
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        let have_primitive = ctx.typ(tree_ty).is_primitive();
        let want_primitive = ctx.typ(ty).is_primitive();
        if have_primitive == want_primitive {
            return tree;
        }
        if have_primitive {
            let unboxed = {
                let syms = self.syms.borrow();
                self.types.unboxed_type_ro(ctx, &syms, ty)
            };
            if !ctx.typ(unboxed).has_tag(TypeTag::NONE) {
                let is_sub = {
                    let syms = self.syms.borrow();
                    self.types.is_subtype(ctx, &syms, tree_ty, unboxed)
                };
                if !is_sub {
                    let const_value = ctx.typ(tree_ty).const_value().cloned();
                    if let Some(const_value) = const_value {
                        let new_ty = ctx.typ(unboxed).const_type(const_value);
                        let new_ty = ctx.alloc_type(new_ty);
                        ctx.tree_mut(tree).ty = Some(new_ty);
                    }
                }
                let wrapper = {
                    let syms = self.syms.borrow();
                    self.types.erasure(ctx, &syms, ty)
                };
                self.box_primitive(ctx, tree, wrapper)
            } else {
                self.box_primitive_auto(ctx, tree)
            }
        } else {
            self.unbox(ctx, tree, ty)
        }
    }

    /// Box a primitive expression into the wrapper of its own type.
    fn box_primitive_auto(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        let boxed = {
            let mut syms = self.syms.borrow_mut();
            self.types.boxed_class(ctx, &mut syms, tree_ty)
        };
        let wrapper = ctx.sym(boxed).ty.expect("boxed type");
        self.box_primitive(ctx, tree, wrapper)
    }

    /// Box a primitive expression into the given wrapper class valueOf method.
    fn box_primitive(&mut self, ctx: &mut CompilerContext, tree: TreeId, wrapper: TypeId) -> TreeId {
        let pos = ctx.tree(tree).pos;
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
        );
        let value_of = self.names.valueOf.clone();
        let value_of_sym = self.lookup_method(
            ctx,
            Some(diagnostic_pos),
            value_of,
            wrapper,
            List::of(tree_ty),
        );
        let qual = {
            self.make.borrow_mut().qual_ident(ctx, value_of_sym)
        };
        self.make.borrow_mut().app_with_args(ctx, qual, List::of(tree))
    }

    /// Unbox an object expression to the given primitive type.
    fn unbox(&mut self, ctx: &mut CompilerContext, tree: TreeId, primitive: TypeId) -> TreeId {
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        let mut unboxed = {
            let syms = self.syms.borrow();
            self.types.unboxed_type_ro(ctx, &syms, tree_ty)
        };
        let mut tree = tree;
        if ctx.typ(unboxed).has_tag(TypeTag::NONE) {
            unboxed = primitive;
            if !ctx.typ(unboxed).is_primitive() {
                panic!("AssertionError: unboxed type is not primitive");
            }
            let boxed = {
                let mut syms = self.syms.borrow_mut();
                self.types.boxed_class(ctx, &mut syms, unboxed)
            };
            let boxed_ty = ctx.sym(boxed).ty.expect("boxed type");
            let pos = ctx.tree(tree).pos;
            self.make.borrow_mut().at(pos);
            let clazz = self.type_tree(ctx, boxed_ty);
            tree = self.make.borrow_mut().type_cast(ctx, clazz, tree);
            let tree_ty = ctx.tree(tree).ty.expect("tree type");
            let _ = tree_ty;
        } else {
            let is_sub = {
                let syms = self.syms.borrow();
                self.types.is_subtype(ctx, &syms, unboxed, primitive)
            };
            if !is_sub {
                panic!("AssertionError: unboxed type is not a subtype of the primitive");
            }
        }
        let pos = ctx.tree(tree).pos;
        self.make.borrow_mut().at(pos);
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        let unboxed_tsym = ctx.typ(unboxed).tsym.expect("unboxed type symbol");
        let method_name = {
            let base = ctx.sym(unboxed_tsym).name.clone();
            base.append(&self.names.Value)
        };
        let accessor = self.lookup_method(ctx, None, method_name, tree_ty, List::nil());
        let select = self.select_symbol(ctx, tree, accessor);
        self.make.borrow_mut().app(ctx, select)
    }

    /// Translate a method declaration through the default TreeTranslator
    /// navigation (return type, parameters, thrown types and body).
    fn visit_method_def_default(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, default_value, body) =
            match &ctx.tree(tree).kind {
                TreeKind::MethodDecl {
                    mods,
                    restype,
                    typarams,
                    recvparam,
                    params,
                    thrown,
                    default_value,
                    body,
                    ..
                } => (
                    *mods,
                    *restype,
                    typarams.clone(),
                    *recvparam,
                    params.clone(),
                    thrown.clone(),
                    *default_value,
                    *body,
                ),
                _ => panic!("expected a method declaration"),
            };
        let mods = self.translate(ctx, mods);
        let restype = self.translate(ctx, restype);
        let typarams = self.translate_list(ctx, typarams);
        let recvparam = self.translate(ctx, recvparam);
        let params = self.translate_list(ctx, params);
        let thrown = self.translate_list(ctx, thrown);
        let default_value = self.translate(ctx, default_value);
        let body = self.translate(ctx, body);
        if let TreeKind::MethodDecl {
            mods: m,
            restype: r,
            typarams: t,
            recvparam: rv,
            params: p,
            thrown: th,
            default_value: d,
            body: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *r = restype;
            *t = typarams;
            *rv = recvparam;
            *p = params;
            *th = thrown;
            *d = default_value;
            *b = body;
        }
        self.result = Some(tree);
    }

    /// Return the synthetic name for an enum helper, avoiding clashes.
    fn synthetic_name(&self, ctx: &CompilerContext, tree: TreeId, base_name: &str) -> Name {
        let tree_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        let members = ctx.sym(tree_sym).members().expect("class members");
        let text = format!("{}{}", self.target.synthetic_name_char(), base_name);
        let mut name = self
            .names
            .from_string(&javac_strings::JavaString::from(text.as_str()));
        loop {
            let arena = ctx.symbols_arena();
            if ctx
                .scope(members)
                .find_first(&name, &|_| true, arena)
                .is_none()
            {
                return name;
            }
            let next = format!("{}{}", name.to_java_string(), self.target.synthetic_name_char());
            name = self
                .names
                .from_string(&javac_strings::JavaString::from(next.as_str()));
        }
    }

    /// Translate an enum constant definition by adding its name and ordinal
    /// arguments to the constructor call.
    fn visit_enum_constant_def(&mut self, ctx: &mut CompilerContext, var: TreeId, ordinal: i32) {
        let init = match &ctx.tree(var).kind {
            TreeKind::VariableDecl { init: Some(init), .. } => *init,
            _ => panic!("expected an enum constant initializer"),
        };
        let args = match &ctx.tree(init).kind {
            TreeKind::NewClass { args, .. } => args.clone(),
            _ => panic!("expected a new class expression"),
        };
        let int_type = self.syms.borrow().int_type;
        let string_type = self.syms.borrow().string_type;
        let ordinal_lit = self.make_lit(
            ctx,
            int_type,
            &crate::util::constants::ConstantValue::Integer(ordinal),
        );
        let name = match &ctx.tree(var).kind {
            TreeKind::VariableDecl { name, .. } => name.clone(),
            _ => panic!("expected a variable declaration"),
        };
        let name_const = crate::util::constants::ConstantValue::String(name.to_java_string());
        let name_lit = self.make_lit(ctx, string_type, &name_const);
        let new_args = args.prepend(ordinal_lit).prepend(name_lit);
        if let TreeKind::NewClass { args: slot, .. } = &mut ctx.tree_mut(init).kind {
            *slot = new_args;
        }
    }

    /// Return true when the Object.clone method is available for the enum values
    /// array copy strategy.
    fn use_clone(&mut self, ctx: &mut CompilerContext) -> bool {
        let object_type = self.syms.borrow().object_type;
        let object_sym = match ctx.typ(object_type).tsym {
            Some(sym) => sym,
            None => return false,
        };
        let members = match ctx.sym(object_sym).members() {
            Some(members) => members,
            None => return false,
        };
        let name = self.names.clone.clone();
        let arena = ctx.symbols_arena();
        ctx.scope(members).find_first(&name, &|_| true, arena).is_some()
    }

    /// Translate an enum class body.
    fn visit_enum_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pos = ctx.tree(tree).pos;
        self.make.borrow_mut().at(pos);
        let tree_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        let tree_ty = ctx.sym(tree_sym).ty.expect("class type");
        let (extending, defs) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                extending, defs, ..
            } => (*extending, defs.clone()),
            _ => panic!("expected a class declaration"),
        };
        let extending = match extending {
            Some(extending) => extending,
            None => {
                let super_ty = {
                    let syms = self.syms.borrow();
                    self.types.supertype(ctx, &syms, tree_ty)
                };
                self.type_tree(ctx, super_ty)
            }
        };
        if let TreeKind::ClassDecl { extending: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(extending);
        }
        let class_ty = self.class_of_type(ctx, tree_ty, pos);
        let class_lit_ty = self.syms.borrow().class_type;
        let class_val = self
            .types
            .erasure(ctx, &self.syms.borrow(), class_lit_ty);
        ctx.tree_mut(class_ty).ty = Some(class_val);
        let mut next_ordinal = 0;
        let mut values: List<TreeId> = List::nil();
        let mut enum_defs: List<TreeId> = List::nil();
        let mut other_defs: List<TreeId> = List::nil();
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            let is_enum_constant = ctx.tree(def).has_tag(Tag::VARDEF)
                && {
                    let mods = match &ctx.tree(def).kind {
                        TreeKind::VariableDecl { mods, .. } => *mods,
                        _ => None,
                    };
                    (Self::mods_flags(ctx, mods) & crate::code::flags::ENUM as i64) != 0
                };
            if is_enum_constant {
                self.visit_enum_constant_def(ctx, def, next_ordinal);
                next_ordinal += 1;
                let def_sym = match &ctx.tree(def).kind {
                    TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                    _ => panic!("expected an enum constant symbol"),
                };
                let value_ident = {
                    self.make
                        .borrow_mut()
                        .qual_ident(ctx, def_sym)
                };
                values = values.append(value_ident);
                enum_defs = enum_defs.append(def);
            } else {
                other_defs = other_defs.append(def);
            }
            current = current.tail();
        }
        let values_name = self.synthetic_name(ctx, tree, "VALUES");
        let erased_enum_ty = self.types.erasure(ctx, &self.syms.borrow(), tree_ty);
        let array_class = self.syms.borrow().array_class;
        let array_ty = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(array_class),
            TypeKindData::ArrayType {
                elemtype: Some(erased_enum_ty),
                varargs: false,
            },
        ));
        let flags =
            (crate::code::flags::PRIVATE
                | crate::code::flags::FINAL
                | crate::code::flags::STATIC
                | crate::code::flags::SYNTHETIC) as i64;
        let values_var_symbol = Symbol::new_var_symbol(flags, values_name, array_ty, tree_sym);
        let values_var = ctx.alloc_symbol(values_var_symbol);
        let elem_tree = self.type_tree(ctx, erased_enum_ty);
        let new_array = self.make.borrow_mut().new_array(
            ctx,
            Some(elem_tree),
            List::nil(),
            Some(values),
        );
        ctx.tree_mut(new_array).ty = Some(array_ty);
        let values_method_name = self.synthetic_name(ctx, tree, "values");
        let method_class = self.syms.borrow().method_class;
        let values_method_ty = ctx.alloc_type(Type::new(
            TypeTag::METHOD,
            Some(method_class),
            TypeKindData::MethodType {
                argtypes: List::nil(),
                restype: Some(array_ty),
                thrown: List::nil(),
                recvtype: None,
            },
        ));
        let method_flags =
            (crate::code::flags::PRIVATE | crate::code::flags::STATIC | crate::code::flags::SYNTHETIC)
                as i64;
        let values_method_symbol =
            Symbol::new_method_symbol(method_flags, values_method_name, values_method_ty, tree_sym);
        let values_method = ctx.alloc_symbol(values_method_symbol);
        let values_return = self.make.borrow_mut().return_(ctx, Some(new_array));
        let values_block = self.make.borrow_mut().block(ctx, 0, List::of(values_return));
        let values_method_def =
            self.method_def_for_symbol(ctx, values_method, values_method_ty, Some(values_block));
        enum_defs = enum_defs.append(values_method_def);
        let members = ctx.sym(tree_sym).members().expect("class members");
        ctx.scope_enter(members, values_method);
        let values_var_def = self.var_def_for_symbol(ctx, values_var, None);
        let values_ident = self.make.borrow_mut().ident_symbol(ctx, values_method);
        let values_app = self.make.borrow_mut().app(ctx, values_ident);
        let values_var_def = {
            let old = values_var_def;
            if let TreeKind::VariableDecl { init, .. } = &mut ctx.tree_mut(old).kind {
                *init = Some(values_app);
            }
            old
        };
        enum_defs = enum_defs.append(values_var_def);
        ctx.scope_enter(members, values_var);
        let values_name_lookup = self.names.values.clone();
        let values_sym = self.lookup_method(ctx, None, values_name_lookup, tree_ty, List::nil());
        let use_clone = self.use_clone(ctx);
        let values_body = if use_clone {
            let values_sym_ty = ctx.sym(values_sym).ty.expect("values method type");
            let return_ty = self.types.get_return_type_public(ctx, values_sym_ty);
            let values_var_ident = self.make.borrow_mut().ident_symbol(ctx, values_var);
            let array_clone = self.syms.borrow().array_clone_method;
            let clone_sel = self.select_symbol(ctx, values_var_ident, array_clone);
            let clone_app = self.make.borrow_mut().app(ctx, clone_sel);
            let return_ty_tree = self.type_tree(ctx, return_ty);
            let cast = self.make.borrow_mut().type_cast(ctx, return_ty_tree, clone_app);
            ctx.tree_mut(cast).ty = Some(return_ty);
            let ret = self.make.borrow_mut().return_(ctx, Some(cast));
            List::of(ret)
        } else {
            let result_name = self.synthetic_name(ctx, tree, "result");
            let values_owner_ty = ctx.sym(values_sym).ty.expect("values method type");
            let result_flags = (crate::code::flags::FINAL | crate::code::flags::SYNTHETIC) as i64;
            let result_symbol =
                Symbol::new_var_symbol(result_flags, result_name, array_ty, values_sym);
            let result_var = ctx.alloc_symbol(result_symbol);
            let length_var = self.syms.borrow().length_var;
            let values_var_ident = self.make.borrow_mut().ident_symbol(ctx, values_var);
            let length_sel = self.select_symbol(ctx, values_var_ident, length_var);
            let result_elem_tree = self.type_tree(ctx, erased_enum_ty);
            let result_array = self.make.borrow_mut().new_array(
                ctx,
                Some(result_elem_tree),
                List::of(length_sel),
                None,
            );
            ctx.tree_mut(result_array).ty = Some(array_ty);
            let decl = self.var_def_for_symbol(ctx, result_var, Some(result_array));
            let system_arraycopy = self.system_arraycopy_method(ctx);
            let system_type = self.syms.borrow().system_type;
            let system_tsym = ctx.typ(system_type).tsym.expect("system type symbol");
            let system_qual = self.make.borrow_mut().ident_symbol(ctx, system_tsym);
            let copy_sel = self.select_symbol(ctx, system_qual, system_arraycopy);
            let values_var_ident2 = self.make.borrow_mut().ident_symbol(ctx, values_var);
            let result_var_ident = self.make.borrow_mut().ident_symbol(ctx, result_var);
            let values_var_ident3 = self.make.borrow_mut().ident_symbol(ctx, values_var);
            let length_sel2 = self.select_symbol(ctx, values_var_ident3, length_var);
            let int_type = self.syms.borrow().int_type;
            let zero1 = self.make_lit(ctx, int_type, &crate::util::constants::ConstantValue::Integer(0));
            let zero2 = self.make_lit(ctx, int_type, &crate::util::constants::ConstantValue::Integer(0));
            let copy_app = self.make.borrow_mut().app_with_args(
                ctx,
                copy_sel,
                List::of4(values_var_ident2, zero1, result_var_ident, vec![zero2]).append(length_sel2),
            );
            let copy = self.make.borrow_mut().exec(ctx, copy_app);
            let result_var_ident2 = self.make.borrow_mut().ident_symbol(ctx, result_var);
            let ret = self.make.borrow_mut().return_(ctx, Some(result_var_ident2));
            let _ = values_owner_ty;
            List::of3(decl, copy, ret)
        };
        let values_block = self.make.borrow_mut().block(ctx, 0, values_body);
        let values_def = self.method_def_for_symbol(ctx, values_sym, ctx.sym(values_sym).ty.expect("values method type"), Some(values_block));
        enum_defs = enum_defs.append(values_def);
        let value_of_name = self.names.valueOf.clone();
        let string_type = self.syms.borrow().string_type;
        let value_of_sym = self.lookup_method(ctx, None, value_of_name, tree_ty, List::of(string_type));
        let name_arg = match &ctx.sym(value_of_sym).kind_data {
            SymbolKind::MethodSymbol { params, .. } => *params.head(),
            _ => panic!("expected a method symbol"),
        };
        let name_val = self.make.borrow_mut().ident_symbol(ctx, name_arg);
        let enum_sym = self.syms.borrow().enum_sym;
        let enum_qual = self.make.borrow_mut().ident_symbol(ctx, enum_sym);
        let value_of_name2 = self.names.valueOf.clone();
        let call = self.make_call(ctx, enum_qual, value_of_name2, List::of2(class_ty, name_val));
        let tree_ty_tree = self.type_tree(ctx, tree_ty);
        let cast = self.make.borrow_mut().type_cast(ctx, tree_ty_tree, call);
        let value_of_ret = self.make.borrow_mut().return_(ctx, Some(cast));
        let value_of_block = self.make.borrow_mut().block(ctx, 0, List::of(value_of_ret));
        let value_of_def = self.method_def_for_symbol(
            ctx,
            value_of_sym,
            ctx.sym(value_of_sym).ty.expect("valueOf type"),
            Some(value_of_block),
        );
        enum_defs = enum_defs.append(value_of_def);
        let mut all_defs = enum_defs;
        let mut others = other_defs;
        while others.non_empty() {
            all_defs = all_defs.append(*others.head());
            others = others.tail();
        }
        if let TreeKind::ClassDecl { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = all_defs;
        }
    }

    /// Return the cached System.arraycopy method symbol, creating it on first
    /// use.
    fn system_arraycopy_method(&mut self, ctx: &mut CompilerContext) -> SymbolId {
        if let Some(method) = self.system_arraycopy_method {
            return method;
        }
        let object_type = self.syms.borrow().object_type;
        let int_type = self.syms.borrow().int_type;
        let void_type = self.syms.borrow().void_type;
        let method_class = self.syms.borrow().method_class;
        let method_ty = ctx.alloc_type(Type::new(
            TypeTag::METHOD,
            Some(method_class),
            TypeKindData::MethodType {
                argtypes: List::of4(object_type, int_type, object_type, vec![int_type, int_type]),
                restype: Some(void_type),
                thrown: List::nil(),
                recvtype: None,
            },
        ));
        let name = self
            .names
            .from_string(&javac_strings::JavaString::from("arraycopy"));
        let system_type = self.syms.borrow().system_type;
        let system_tsym = ctx.typ(system_type).tsym.expect("system type symbol");
        let flags = (crate::code::flags::PUBLIC | crate::code::flags::STATIC) as i64;
        let symbol = Symbol::new_method_symbol(flags, name, method_ty, system_tsym);
        let method = ctx.alloc_symbol(symbol);
        self.system_arraycopy_method = Some(method);
        method
    }

    /// Return the record components of the given type as variable symbols.
    fn record_vars(&mut self, ctx: &mut CompilerContext, t: TypeId) -> List<SymbolId> {
        let mut vars: List<SymbolId> = List::nil();
        let mut current_t = t;
        while !ctx.typ(current_t).has_tag(TypeTag::NONE) {
            if ctx.typ(current_t).has_tag(TypeTag::CLASS)
                && let Some(tsym) = ctx.typ(current_t).tsym
                && let Some(members) = ctx.sym(tsym).members()
            {
                let scope_snapshot = ctx.scope(members).clone();
                let candidates = scope_snapshot.get_symbols(
                    members,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|_| true,
                    ctx,
                );
                let symbols: Vec<SymbolId> = candidates
                    .into_iter()
                    .filter(|sym| {
                        ctx.sym(*sym).kind == Kind::VAR
                            && (ctx.sym(*sym).flags_field & crate::code::flags::RECORD) != 0
                    })
                    .collect();
                for sym in symbols {
                    vars = vars.prepend(sym);
                }
            }
            let syms = self.syms.borrow();
            current_t = self.types.supertype(ctx, &syms, current_t);
        }
        vars
    }

    /// Generate the mandated accessor methods for record components.
    fn generate_mandated_accessors(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> List<TreeId> {
        let fields = tree_info::record_fields(ctx, tree);
        let tree_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        let components = match &ctx.sym(tree_sym).kind_data {
            SymbolKind::ClassSymbol { record_components, .. } => record_components.clone(),
            _ => return List::nil(),
        };
        let mut result: List<TreeId> = List::nil();
        let mut current = components;
        while current.non_empty() {
            let rc_sym = *current.head();
            let (accessor, rc_name) = match &ctx.sym(rc_sym).kind_data {
                SymbolKind::RecordComponent { accessor, .. } => {
                    (*accessor, ctx.sym(rc_sym).name.clone())
                }
                _ => (None, ctx.sym(rc_sym).name.clone()),
            };
            if let Some(accessor) = accessor
                && (ctx.sym(accessor).flags_field & crate::code::flags::GENERATED_MEMBER as i64) != 0
            {
                    let mut field_tree: Option<TreeId> = None;
                    let mut fields_current = fields.clone();
                    while fields_current.non_empty() {
                        let field = *fields_current.head();
                        if let TreeKind::VariableDecl { name, .. } = &ctx.tree(field).kind
                            && name == &rc_name
                        {
                            field_tree = Some(field);
                            break;
                        }
                        fields_current = fields_current.tail();
                    }
                    if let Some(field) = field_tree {
                        let pos = ctx.tree(tree).pos;
                        self.make.borrow_mut().at(pos);
                        let field_sym = match &ctx.tree(field).kind {
                            TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                            _ => panic!("expected a record field symbol"),
                        };
                        let field_ident = self.make.borrow_mut().ident_symbol(ctx, field_sym);
                        let ret = self.make.borrow_mut().return_(ctx, Some(field_ident));
                        let block = self.make.borrow_mut().block(ctx, 0, List::of(ret));
                        let accessor_ty = ctx.sym(accessor).ty.expect("accessor type");
                        let def = self.method_def_for_symbol(ctx, accessor, accessor_ty, Some(block));
                        result = result.append(def);
                    }
            }
            current = current.tail();
        }
        result
    }

    /// Translate a record class body, adding mandated accessors and the
    /// generated toString/hashCode/equals methods.
    fn visit_record_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pos = ctx.tree(tree).pos;
        self.make.borrow_mut().at(pos);
        let tree_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        let tree_ty = ctx.sym(tree_sym).ty.expect("class type");
        let vars = self.record_vars(ctx, tree_ty);
        let mut handles: Vec<SymbolId> = Vec::new();
        let mut current = vars.clone();
        while current.non_empty() {
            let mut var = *current.head();
            if ctx.sym(var).owner != Some(tree_sym) {
                let mut cloned = ctx.sym(var).clone();
                cloned.owner = Some(tree_sym);
                var = ctx.alloc_symbol(cloned);
            }
            let handle = crate::code::symbol::as_method_handle(ctx, var, true);
            handles.push(handle);
            current = current.tail();
        }
        let mut defs = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => panic!("expected a class declaration"),
        };
        let accessors = self.generate_mandated_accessors(ctx, tree);
        let mut current_acc = accessors;
        while current_acc.non_empty() {
            defs = defs.append(*current_acc.head());
            current_acc = current_acc.tail();
        }
        let names_to_generate = [
            self.names.toString.clone(),
            self.names.hashCode.clone(),
            self.names.equals.clone(),
        ];
        for name in names_to_generate {
            let method = self.generate_record_method(ctx, tree, name, vars.clone(), handles.clone());
            defs = defs.append(method);
        }
        if let TreeKind::ClassDecl { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
    }

    /// Build one generated record method (toString, hashCode or equals).
    fn generate_record_method(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        name: Name,
        vars: List<SymbolId>,
        handles: Vec<SymbolId>,
    ) -> TreeId {
        let pos = ctx.tree(tree).pos;
        self.make.borrow_mut().at(pos);
        let tree_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        let tree_ty = ctx.sym(tree_sym).ty.expect("class type");
        let is_equals = name == self.names.equals;
        let object_type = self.syms.borrow().object_type;
        let object_sym = ctx.typ(object_type).tsym.expect("object symbol");
        let argtypes = if is_equals {
            List::of(object_type)
        } else {
            List::nil()
        };
        let _ = object_sym;
        let msym = self.lookup_method(ctx, None, name.clone(), tree_ty, argtypes);
        if (ctx.sym(msym).flags_field & crate::code::flags::RECORD) == 0 {
            return self.make.borrow_mut().block(ctx, crate::code::flags::SYNTHETIC as i64, List::nil());
        }
        let bootstrap_name = self.names.bootstrap.clone();
        let mut static_args: Vec<crate::jvm::pool_constant::LoadableConstantRef> = Vec::new();
        static_args.push(crate::jvm::pool_constant::LoadableConstantRef::Class(tree_ty));
        let mut concat = String::new();
        let mut vars_current = vars;
        let mut first_name = true;
        while vars_current.non_empty() {
            let var = *vars_current.head();
            if !first_name {
                concat.push(';');
            }
            first_name = false;
            concat.push_str(&ctx.sym(var).name.to_java_string().to_string_lossy());
            vars_current = vars_current.tail();
        }
        let string_const = crate::jvm::pool_constant::BasicConstant::string(
            javac_strings::JavaString::from(concat.as_str()),
        );
        static_args.push(crate::jvm::pool_constant::LoadableConstantRef::Basic(string_const));
        for handle in &handles {
            static_args.push(crate::jvm::pool_constant::LoadableConstantRef::MethodHandle(*handle));
        }
        let string_type = self.syms.borrow().string_type;
        let method_handle_type = self.syms.borrow().method_handle_type;
        let array_class = self.syms.borrow().array_class;
        let method_handle_array = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(array_class),
            TypeKindData::ArrayType {
                elemtype: Some(method_handle_type),
                varargs: false,
            },
        ));
        let class_type = self.syms.borrow().class_type;
        let method_handle_lookup_type = self.syms.borrow().method_handle_lookup_type;
        let type_descriptor_type = self.syms.borrow().type_descriptor_type;
        let object_methods_type = self.syms.borrow().object_methods_type;
        let static_arg_types = List::of3(class_type, string_type, method_handle_array);
        let mut call_arg_types = List::of3(
            method_handle_lookup_type,
            string_type,
            type_descriptor_type,
        );
        let mut sat = static_arg_types;
        while sat.non_empty() {
            call_arg_types = call_arg_types.append(*sat.head());
            sat = sat.tail();
        }
        let qualifier = self.make_indy_qualifier(
            ctx,
            object_methods_type,
            tree,
            msym,
            call_arg_types,
            static_args,
            bootstrap_name,
            name.clone(),
            false,
        );
        let this_name = self.names._this.clone();
        let this_flags = crate::code::flags::SYNTHETIC as i64;
        let this_symbol = Symbol::new_var_symbol(this_flags, this_name, tree_ty, tree_sym);
        let this_sym = ctx.alloc_symbol(this_symbol);
        let this_ident = self.make.borrow_mut().ident_symbol(ctx, this_sym);
        let proxy_args = if is_equals {
            let first_param = match &ctx.sym(msym).kind_data {
                SymbolKind::MethodSymbol { params, .. } => *params.head(),
                _ => panic!("expected a method symbol"),
            };
            if let SymbolKind::VarSymbol { adr, .. } = &mut ctx.sym_mut(first_param).kind_data {
                *adr = 0;
            }
            let param_ident = self.make.borrow_mut().ident_symbol(ctx, first_param);
            List::of2(this_ident, param_ident)
        } else {
            List::of(this_ident)
        };
        let proxy_call = self.make.borrow_mut().app_with_args(ctx, qualifier, proxy_args);
        let qualifier_ty = ctx.tree(qualifier).ty.expect("qualifier type");
        ctx.tree_mut(proxy_call).ty = Some(qualifier_ty);
        let ret = self.make.borrow_mut().return_(ctx, Some(proxy_call));
        let body = self.make.borrow_mut().block(ctx, 0, List::of(ret));
        let msym_ty = ctx.sym(msym).ty.expect("record method type");
        self.method_def_for_symbol(ctx, msym, msym_ty, Some(body))
    }

    /// Create an invokedynamic qualifier field access for a bootstrap method.
    #[allow(clippy::too_many_arguments)]
    fn make_indy_qualifier(
        &mut self,
        ctx: &mut CompilerContext,
        site: TypeId,
        tree: TreeId,
        msym: SymbolId,
        static_arg_types: List<TypeId>,
        static_arg_values: Vec<crate::jvm::pool_constant::LoadableConstantRef>,
        bootstrap_name: Name,
        arg_name: Name,
        is_static: bool,
    ) -> TreeId {
        let pos = ctx.tree(tree).pos;
        let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
        );
        let bsm = self.lookup_method(
            ctx,
            Some(diagnostic_pos),
            bootstrap_name,
            site,
            static_arg_types,
        );
        let msym_ty = ctx.sym(msym).ty.expect("method type");
        let indy_type = if let TypeKindData::MethodType {
            argtypes,
            restype,
            thrown,
            ..
        } = &ctx.typ(msym_ty).kind
        {
            let mut new_argtypes = argtypes.clone();
            if !is_static {
                let tree_sym = match &ctx.tree(tree).kind {
                    TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                    _ => panic!("expected a class declaration"),
                };
                let tree_ty = ctx.sym(tree_sym).ty.expect("class type");
                new_argtypes = new_argtypes.prepend(tree_ty);
            }
            let method_class = self.syms.borrow().method_class;
            ctx.alloc_type(Type::new(
                TypeTag::METHOD,
                Some(method_class),
                TypeKindData::MethodType {
                    argtypes: new_argtypes,
                    restype: *restype,
                    thrown: thrown.clone(),
                    recvtype: None,
                },
            ))
        } else {
            msym_ty
        };
        let dynamic_sym = crate::code::symbol::new_dynamic_method_symbol(
            ctx,
            arg_name.clone(),
            self.syms.borrow().no_symbol,
            bsm,
            indy_type,
            static_arg_values,
        );
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        let site_qual = {
            self.make
                .borrow_mut()
                .qual_ident(ctx, site_tsym)
        };
        let qualifier = {
            let syms = self.syms.borrow();
            let _ = syms;
            self.make
                .borrow_mut()
                .select(ctx, site_qual, arg_name)
        };
        if let TreeKind::FieldAccess { sym: sym_slot, .. } = &mut ctx.tree_mut(qualifier).kind {
            *sym_slot = Some(dynamic_sym);
        }
        let restype = self.types.get_return_type_public(ctx, msym_ty);
        ctx.tree_mut(qualifier).ty = Some(restype);
        qualifier
    }


    /// Build the tree representing a class literal for a type.
    ///
    /// Primitive and void types are replaced by the boxed class TYPE field; class
    /// and array types use a synthetic _class field selected from the type tree.
    fn class_of_type(&mut self, ctx: &mut CompilerContext, ty: TypeId, pos: i32) -> TreeId {
        let tag = ctx.typ(ty).get_tag();
        match tag {
            TypeTag::BYTE
            | TypeTag::SHORT
            | TypeTag::CHAR
            | TypeTag::INT
            | TypeTag::LONG
            | TypeTag::FLOAT
            | TypeTag::DOUBLE
            | TypeTag::BOOLEAN
            | TypeTag::VOID => {
                let boxed = {
                    let mut syms = self.syms.borrow_mut();
                    self.types.boxed_class(ctx, &mut syms, ty)
                };
                let boxed_ty = ctx.sym(boxed).ty.expect("boxed class type");
                let env = self.attr_env.clone().expect("attribution environment");
                let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                );
                let type_sym = {
                    let mut rs = self.rs.borrow_mut();
                    let mut syms = self.syms.borrow_mut();
                    rs.find_ident_in_type(
                        ctx,
                        &mut syms,
                        Some(Rc::clone(&diagnostic_pos)),
                        &env,
                        boxed_ty,
                        &self.names.TYPE,
                        crate::code::kinds::KindSelector::VAR,
                    )
                };
                let type_sym = {
                    let mut rs = self.rs.borrow_mut();
                    let mut syms = self.syms.borrow_mut();
                    rs.access_base(
                        ctx,
                        &mut syms,
                        type_sym,
                        Some(diagnostic_pos),
                        Some(boxed),
                        Some(boxed_ty),
                        &self.names.TYPE,
                        true,
                    )
                };
                if ctx.sym(type_sym).kind == Kind::VAR {
                    let _ = ctx.sym(type_sym).get_const_value();
                }
                self.make
                    .borrow_mut()
                    .qual_ident(ctx, type_sym)
            }
            TypeTag::CLASS | TypeTag::ARRAY => {
                let ty_tsym = ctx.typ(ty).tsym.expect("class type symbol");
                let class_type = self.syms.borrow().class_type;
                let flags = (crate::code::flags::STATIC
                    | crate::code::flags::PUBLIC
                    | crate::code::flags::FINAL) as i64;
                let class_name = self.names._class.clone();
                let symbol = Symbol::new_var_symbol(flags, class_name, class_type, ty_tsym);
                let class_field = ctx.alloc_symbol(symbol);
                self.make.borrow_mut().at(pos);
                let type_tree = self.type_tree(ctx, ty);
                self.select_symbol(ctx, type_tree, class_field)
            }
            _ => panic!("AssertionError: unexpected type for class literal"),
        }
    }

    /// Insert the missing field initializers into a compact record constructor.
    ///
    /// Fields that still carry the uninitialized-field flag are assigned from the
    /// matching constructor parameter, and the flag is cleared afterwards.
    fn lower_compact_record_ctor(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let current_class = self.current_class.expect("current class");
        let members = match ctx.sym(current_class).members() {
            Some(members) => members,
            None => return,
        };
        let scope_snapshot = ctx.scope(members).clone();
        let fields = scope_snapshot.get_symbols(
            members,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            ctx,
        );
        let mut record_fields: Vec<SymbolId> = Vec::new();
        for field in fields {
            if ctx.sym(field).kind == Kind::VAR
                && (ctx.sym(field).flags_field & crate::code::flags::RECORD) != 0
                && (ctx.sym(field).flags_field
                    & crate::code::flags::UNINITIALIZED_FIELD)
                    != 0
            {
                record_fields.push(field);
            }
        }
        let params = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { params, .. } => params.clone(),
            _ => return,
        };
        let pos = ctx.tree(tree).pos;
        let body = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { body: Some(body), .. } => *body,
            _ => return,
        };
        for field in record_fields {
            let field_name = ctx.sym(field).name.clone();
            let mut param_sym: Option<SymbolId> = None;
            let mut current = params.clone();
            while current.non_empty() {
                let param = *current.head();
                if let TreeKind::VariableDecl { sym: Some(sym), .. } = &ctx.tree(param).kind
                    && ctx.sym(*sym).name == field_name
                {
                    param_sym = Some(*sym);
                    break;
                }
                current = current.tail();
            }
            let Some(param_sym) = param_sym else { continue };
            self.make.borrow_mut().at(pos);
            let field_owner = ctx.sym(field).owner.expect("field owner");
            let field_owner_ty = ctx.sym(field_owner).ty.expect("field owner type");
            let owner_erasure = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, field_owner_ty)
            };
            let this = self.make.borrow_mut().this_expr(ctx, owner_erasure);
            let field_sel = self.select_symbol(ctx, this, field);
            let param_ident = self.make.borrow_mut().ident_symbol(ctx, param_sym);
            let assign = self.make.borrow_mut().assign(ctx, field_sel, param_ident);
            let field_ty = ctx.sym(field).ty.expect("field type");
            let field_erasure = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, field_ty)
            };
            ctx.tree_mut(assign).ty = Some(field_erasure);
            let exec = self.make.borrow_mut().exec(ctx, assign);
            if let TreeKind::Block { stats, .. } = &mut ctx.tree_mut(body).kind {
                *stats = stats.append(exec);
            }
            ctx.sym_mut(field).flags_field &= !(crate::code::flags::UNINITIALIZED_FIELD);
        }
    }


    /// Return true when a constant expression is known to be true.
    fn is_true(&mut self, ctx: &mut CompilerContext, exp: TreeId) -> bool {
        let ty = ctx.tree(exp).ty;
        if let Some(ty) = ty && ctx.typ(ty).is_true() {
            return true;
        }
        let value = self.exp_value(ctx, exp);
        value.unwrap_or(false)
    }

    /// Return true when a constant expression is known to be false.
    fn is_false(&mut self, ctx: &mut CompilerContext, exp: TreeId) -> bool {
        let ty = ctx.tree(exp).ty;
        if let Some(ty) = ty && ctx.typ(ty).is_false() {
            return true;
        }
        let value = self.exp_value(ctx, exp);
        value.map(|value| !value).unwrap_or(false)
    }

    /// Evaluate a nullable equality expression against a nullable operand.
    fn exp_value(&mut self, ctx: &mut CompilerContext, exp: TreeId) -> Option<bool> {
        let mut exp = exp;
        while ctx.tree(exp).has_tag(Tag::PARENS) {
            exp = match &ctx.tree(exp).kind {
                TreeKind::Parens { expr: Some(expr) } => *expr,
                _ => panic!("expected parentheses"),
            };
        }
        let eq = match ctx.tree(exp).get_tag() {
            Tag::EQ => true,
            Tag::NE => false,
            _ => return None,
        };
        let (lhs, rhs) = match &ctx.tree(exp).kind {
            TreeKind::Binary {
                lhs: Some(lhs),
                rhs: Some(rhs),
                ..
            } => (*lhs, *rhs),
            _ => return None,
        };
        let lhs_ty = ctx.tree(lhs).ty;
        if let Some(lhs_ty) = lhs_ty && ctx.typ(lhs_ty).has_tag(TypeTag::BOT) {
            return self.exp_value_is_null(ctx, eq, rhs);
        }
        let rhs_ty = ctx.tree(rhs).ty;
        if let Some(rhs_ty) = rhs_ty && ctx.typ(rhs_ty).has_tag(TypeTag::BOT) {
            return self.exp_value_is_null(ctx, eq, lhs);
        }
        None
    }

    /// Evaluate a nullable equality expression operand.
    fn exp_value_is_null(&mut self, ctx: &mut CompilerContext, eq: bool, t: TreeId) -> Option<bool> {
        if let Some(ty) = ctx.tree(t).ty && ctx.typ(ty).has_tag(TypeTag::BOT) {
            return Some(eq);
        }
        if ctx.tree(t).has_tag(Tag::LITERAL) {
            return Some(!eq);
        }
        None
    }

    /// Convert a tree to the prototype type with a cast when needed.
    fn convert(&mut self, ctx: &mut CompilerContext, tree: TreeId, pt: TypeId) -> TreeId {
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        if tree_ty == pt || ctx.typ(tree_ty).has_tag(TypeTag::BOT) {
            return tree;
        }
        let pos = ctx.tree(tree).pos;
        self.make.borrow_mut().at(pos);
        let pt_tree = self.type_tree(ctx, pt);
        let result = self.make.borrow_mut().type_cast(ctx, pt_tree, tree);
        let result_ty = if ctx.typ(tree_ty).const_value().is_some() {
            {
            let syms = self.syms.borrow();
            self.cfolder.coerce(ctx, &syms, tree_ty, pt)
        }
        } else {
            pt
        };
        ctx.tree_mut(result).ty = Some(result_ty);
        result
    }

    /// Box the arguments according to the parameter types, handling varargs.
    fn box_args(
        &mut self,
        ctx: &mut CompilerContext,
        mut parameters: List<TypeId>,
        args: List<TreeId>,
        varargs_element: Option<TypeId>,
    ) -> List<TreeId> {
        let mut args = args;
        if parameters.is_empty() {
            return args;
        }
        let mut any_changes = false;
        let mut result: List<TreeId> = List::nil();
        while parameters.tail().non_empty() {
            let param = *parameters.head();
            let arg = *args.head();
            let translated = self
                .translate_with_type(ctx, Some(arg), param)
                .expect("translated argument");
            any_changes |= translated != arg;
            result = result.append(translated);
            parameters = parameters.tail();
            args = args.tail();
        }
        let parameter = *parameters.head();
        if let Some(varargs_element) = varargs_element {
            any_changes = true;
            let mut elems: List<TreeId> = List::nil();
            while args.non_empty() {
                let arg = *args.head();
                let translated = self
                    .translate_with_type(ctx, Some(arg), varargs_element)
                    .expect("translated varargs argument");
                elems = elems.append(translated);
                args = args.tail();
            }
            let elem_tree = self.type_tree(ctx, varargs_element);
            let boxed_args = self.make.borrow_mut().new_array(
                ctx,
                Some(elem_tree),
                List::nil(),
                Some(elems),
            );
            let array_class = self.syms.borrow().array_class;
            let boxed_ty = ctx.alloc_type(Type::new(
                TypeTag::ARRAY,
                Some(array_class),
                TypeKindData::ArrayType {
                    elemtype: Some(varargs_element),
                    varargs: false,
                },
            ));
            ctx.tree_mut(boxed_args).ty = Some(boxed_ty);
            result = result.append(boxed_args);
        } else {
            let arg = *args.head();
            let translated = self
                .translate_with_type(ctx, Some(arg), parameter)
                .expect("translated argument");
            any_changes |= translated != arg;
            result = result.append(translated);
            if !any_changes {
                return args;
            }
        }
        let _ = any_changes;
        result
    }

    /// Create the block that closes try-with-resources resources.
    fn make_twr_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let pos = ctx.tree(tree).pos;
        let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
        );
        self.make_at(&diagnostic_pos);
        let previous_twr = self.twr_vars;
        let new_scope = ctx.alloc_scope(crate::code::scope::Scope::new_scope_impl(
            Some(self.syms.borrow().no_symbol),
        ));
        self.twr_vars = Some(new_scope);
        let resources = match &ctx.tree(tree).kind {
            TreeKind::Try { resources, .. } => resources.clone(),
            _ => panic!("expected a try statement"),
        };
        let body = match &ctx.tree(tree).kind {
            TreeKind::Try { body: Some(body), .. } => *body,
            _ => panic!("expected a try body"),
        };
        let twr_block = self.make_twr_block(ctx, resources, body, 0);
        let catchers = match &ctx.tree(tree).kind {
            TreeKind::Try { catchers, .. } => catchers.clone(),
            _ => List::nil(),
        };
        let finalizer = match &ctx.tree(tree).kind {
            TreeKind::Try { finalizer, .. } => *finalizer,
            _ => None,
        };
        let result = if catchers.is_empty() && finalizer.is_none() {
            self.translate(ctx, Some(twr_block)).expect("translated try block")
        } else {
            let try_tree = self.make.borrow_mut().try_block(ctx, twr_block, catchers, finalizer);
            self.translate(ctx, Some(try_tree)).expect("translated try")
        };
        self.twr_vars = previous_twr;
        result
    }

    /// Build the nested try blocks of a try-with-resources statement.
    fn make_twr_block(
        &mut self,
        ctx: &mut CompilerContext,
        resources: List<TreeId>,
        block: TreeId,
        depth: i32,
    ) -> TreeId {
        if resources.is_empty() {
            return block;
        }
        let mut stats: List<TreeId> = List::nil();
        let resource = *resources.head();
        let (resource_use, resource_non_null) = if ctx.tree(resource).has_tag(Tag::VARDEF) {
            let var = match &ctx.tree(resource).kind {
                TreeKind::VariableDecl {
                    sym: Some(sym),
                    init,
                    ..
                } => (*sym, *init),
                _ => panic!("expected a resource variable"),
            };
            let ident = self.make.borrow_mut().ident_symbol(ctx, var.0);
            let ident_ty = ctx.tree(resource).ty;
            if let Some(ident_ty) = ident_ty {
                ctx.tree_mut(ident).ty = Some(ident_ty);
            }
            let non_null = var.1.is_some()
                && match var.1 {
                    Some(init) => ctx
                        .tree(tree_info::skip_parens(ctx, init))
                        .has_tag(Tag::NEWCLASS),
                    None => false,
                };
            stats = stats.append(resource);
            (ident, non_null)
        } else {
            let resource_ty = ctx.tree(resource).ty.expect("resource type");
            let twr_ty = if ctx.typ(resource_ty).has_tag(TypeTag::BOT) {
                self.syms.borrow().auto_closeable_type
            } else {
                resource_ty
            };
            let base_name = format!("twrVar{}", depth);
            let base = self
                .names
                .from_string(&javac_strings::JavaString::from(base_name.as_str()));
            let twr_scope = self.twr_vars.expect("try-with-resources scope");
            let name = self.make_synthetic_name(ctx, base, twr_scope);
            let current_method_sym = self.current_method_sym.expect("current method symbol");
            let flags = (crate::code::flags::SYNTHETIC | crate::code::flags::FINAL) as i64;
            let symbol = Symbol::new_var_symbol(flags, name, twr_ty, current_method_sym);
            let twr_var = ctx.alloc_symbol(symbol);
            ctx.scope_enter(twr_scope, twr_var);
            let decl = self.var_def_for_symbol(ctx, twr_var, Some(resource));
            let ident = self.make.borrow_mut().ident_symbol(ctx, twr_var);
            stats = stats.append(decl);
            (ident, false)
        };
        let old_pos = ctx.tree(block).pos;
        let block_end = tree_info::end_pos(ctx, block);
        self.make.borrow_mut().at(block_end);
        let mut body_close = self.make_resource_close_invocation(ctx, resource_use);
        if !resource_non_null {
            let null_check = self.make_non_null_check(ctx, resource_use);
            body_close = self.make.borrow_mut().if_(ctx, null_check, body_close, None);
        }
        let body_only_finalize = crate::code::flags::BODY_ONLY_FINALIZE;
        let finally_clause = self
            .make
            .borrow_mut()
            .block(ctx, body_only_finalize, List::of(body_close));
        self.make.borrow_mut().at(old_pos);
        let current_method_sym = self.current_method_sym.expect("current method symbol");
        let throwable_type = self.syms.borrow().throwable_type;
        let primary_name = {
            let text = format!("t{}", self.target.synthetic_name_char());
            self.names
                .from_string(&javac_strings::JavaString::from(text.as_str()))
        };
        let primary_flags = (crate::code::flags::FINAL | crate::code::flags::SYNTHETIC) as i64;
        let primary_symbol =
            Symbol::new_var_symbol(primary_flags, primary_name, throwable_type, current_method_sym);
        let primary_exception = ctx.alloc_symbol(primary_symbol);
        let primary_decl = self.var_def_for_symbol(ctx, primary_exception, None);
        let suppressed_flags = crate::code::flags::SYNTHETIC as i64;
        let suppressed_name = self.make.borrow_mut().param_name(2);
        let suppressed_symbol = Symbol::new_var_symbol(
            suppressed_flags,
            suppressed_name,
            throwable_type,
            current_method_sym,
        );
        let suppressed_exception = ctx.alloc_symbol(suppressed_symbol);
        let primary_ident = self.make.borrow_mut().ident_symbol(ctx, primary_exception);
        let suppressed_ident = self.make.borrow_mut().ident_symbol(ctx, suppressed_exception);
        let add_suppressed_name = self.names.addSuppressed.clone();
        let add_suppressed = self.make_call(
            ctx,
            primary_ident,
            add_suppressed_name,
            List::of(suppressed_ident),
        );
        let add_suppressed = self.make.borrow_mut().exec(ctx, add_suppressed);
        let mut close_resource_stats: List<TreeId> = List::nil();
        let close_invocation = self.make_resource_close_invocation(ctx, resource_use);
        close_resource_stats = close_resource_stats.append(close_invocation);
        let close_resource_block = self
            .make
            .borrow_mut()
            .block(ctx, 0, close_resource_stats);
        let catch_suppressed_decl = self.var_def_for_symbol(ctx, suppressed_exception, None);
        let catch_suppressed_block = self.make.borrow_mut().block(ctx, 0, List::of(add_suppressed));
        let catch_suppressed = self
            .make
            .borrow_mut()
            .catch(ctx, catch_suppressed_decl, catch_suppressed_block);
        let close_resource_try = self.make.borrow_mut().try_block(
            ctx,
            close_resource_block,
            List::of(catch_suppressed),
            None,
        );
        if let TreeKind::Try {
            finally_can_complete_normally,
            ..
        } = &mut ctx.tree_mut(close_resource_try).kind
        {
            *finally_can_complete_normally = true;
        }
        let mut exceptional_close = close_resource_try;
        if !resource_non_null {
            let null_check = self.make_non_null_check(ctx, resource_use);
            exceptional_close = self.make.borrow_mut().if_(ctx, null_check, exceptional_close, None);
        }
        let primary_ident2 = self.make.borrow_mut().ident_symbol(ctx, primary_exception);
        let rethrow = self.make.borrow_mut().throw_(ctx, primary_ident2);
        let exceptional_block = self
            .make
            .borrow_mut()
            .block(ctx, 0, List::of2(exceptional_close, rethrow));
        let exceptional_catch = self
            .make
            .borrow_mut()
            .catch(ctx, primary_decl, exceptional_block);
        let inner_block = self.make_twr_block(ctx, resources.tail(), block, depth + 1);
        let outer_try = self.make.borrow_mut().try_block(
            ctx,
            inner_block,
            List::of(exceptional_catch),
            Some(finally_clause),
        );
        if let TreeKind::Try {
            finally_can_complete_normally,
            ..
        } = &mut ctx.tree_mut(outer_try).kind
        {
            *finally_can_complete_normally = true;
        }
        stats = stats.append(outer_try);
        let _ = old_pos;
        self.make.borrow_mut().block(ctx, 0, stats)
    }

    /// Build the invocation that closes a try-with-resources resource.
    fn make_resource_close_invocation(&mut self, ctx: &mut CompilerContext, mut resource: TreeId) -> TreeId {
        let resource_ty = ctx.tree(resource).ty.expect("resource type");
        let auto_closeable = self.syms.borrow().auto_closeable_type;
        let as_super = {
            let syms = self.syms.borrow();
            let auto_tsym = ctx.typ(auto_closeable).tsym.expect("auto closeable symbol");
            self.types.as_super(ctx, &syms, resource_ty, auto_tsym)
        };
        if as_super.is_none() {
            resource = self.convert(ctx, resource, auto_closeable);
        }
        let close_name = self.names.close.clone();
        let close_call = self.make_call(ctx, resource, close_name, List::nil());
        self.make.borrow_mut().exec(ctx, close_call)
    }

    /// Build the null check for a try-with-resources resource.
    fn make_non_null_check(&mut self, ctx: &mut CompilerContext, expression: TreeId) -> TreeId {
        let null = self.make_null(ctx);
        self.make_binary(ctx, Tag::NE, expression, null)
    }


    /// Abstract a right-hand-side expression so it is evaluated only once.
    ///
    /// Literals, final locals and this/super references are already stable and
    /// are passed to the builder directly; every other expression is stored in a
    /// synthetic final temporary inside a let expression and the builder is
    /// applied to the temporary reference.
    fn abstract_rval(
        &mut self,
        ctx: &mut CompilerContext,
        rval: TreeId,
        ty: TypeId,
        builder: &mut dyn TreeBuilder,
    ) -> TreeId {
        let rval = tree_info::skip_parens(ctx, rval);
        match ctx.tree(rval).get_tag() {
            Tag::LITERAL => return builder.build(self, ctx, rval),
            Tag::IDENT => {
                if let TreeKind::Ident { sym: Some(sym), .. } = &ctx.tree(rval).kind
                    && (ctx.sym(*sym).flags_field & crate::code::flags::FINAL as i64) != 0
                    && ctx
                        .sym(*sym)
                        .owner
                        .map(|owner| ctx.sym(owner).kind == Kind::MTH)
                        .unwrap_or(false)
                {
                    return builder.build(self, ctx, rval);
                }
            }
            _ => {}
        }
        if let Some(name) = tree_info::name(ctx, rval)
            && (name == self.names._super || name == self.names._this)
        {
            return builder.build(self, ctx, rval);
        }
        let current_method_sym = self.current_method_sym.expect("current method symbol");
        let name_text = format!("{}{}", self.target.synthetic_name_char(), rval.index());
        let name = self
            .names
            .from_string(&javac_strings::JavaString::from(name_text.as_str()));
        let flags = (crate::code::flags::FINAL | crate::code::flags::SYNTHETIC) as i64;
        let symbol = Symbol::new_var_symbol(flags, name, ty, current_method_sym);
        let var = ctx.alloc_symbol(symbol);
        let converted = self.convert(ctx, rval, ty);
        let def = self.var_def_for_symbol(ctx, var, Some(converted));
        let ident = self.make.borrow_mut().ident_symbol(ctx, var);
        let built = builder.build(self, ctx, ident);
        let built_ty = ctx.tree(built).ty;
        let res = self.make.borrow_mut().let_expr(ctx, List::of(def), built);
        ctx.tree_mut(res).ty = built_ty;
        res
    }

    /// Abstract a right-hand-side expression using its own type.
    fn abstract_rval_auto(
        &mut self,
        ctx: &mut CompilerContext,
        rval: TreeId,
        builder: &mut dyn TreeBuilder,
    ) -> TreeId {
        let ty = ctx.tree(rval).ty.expect("expression type");
        self.abstract_rval(ctx, rval, ty, builder)
    }

    /// Abstract a left-hand-side expression so it is evaluated only once.
    ///
    /// Identifiers are already stable; field selections abstract their selector,
    /// array accesses abstract both the array and the index, and casts are
    /// stripped before the abstraction.
    fn abstract_lval(
        &mut self,
        ctx: &mut CompilerContext,
        lval: TreeId,
        builder: &mut dyn TreeBuilder,
    ) -> TreeId {
        let lval = tree_info::skip_parens(ctx, lval);
        match ctx.tree(lval).get_tag() {
            Tag::IDENT => builder.build(self, ctx, lval),
            Tag::SELECT => {
                let (selected, field_sym) = match &ctx.tree(lval).kind {
                    TreeKind::FieldAccess {
                        selected: Some(selected),
                        sym,
                        ..
                    } => (*selected, *sym),
                    _ => panic!("expected a field access"),
                };
                let lid = tree_info::symbol(ctx, selected);
                if let Some(lid) = lid
                    && ctx.sym(lid).kind == Kind::TYP
                {
                    return builder.build(self, ctx, lval);
                }
                let mut select_builder = SelectBuilder {
                    inner: builder,
                    field_sym: field_sym.expect("field symbol"),
                };
                self.abstract_rval_auto(ctx, selected, &mut select_builder)
            }
            Tag::INDEXED => {
                let (indexed, index) = match &ctx.tree(lval).kind {
                    TreeKind::ArrayAccess {
                        indexed: Some(indexed),
                        index: Some(index),
                    } => (*indexed, *index),
                    _ => panic!("expected an array access"),
                };
                let int_type = self.syms.borrow().int_type;
                let lval_ty = ctx.tree(lval).ty;
                let _ = lval;
                let mut indexed_builder = IndexedBuilder {
                    inner: builder,
                    index,
                    int_type,
                    lval_ty,
                };
                self.abstract_rval_auto(ctx, indexed, &mut indexed_builder)
            }
            Tag::TYPECAST => {
                let expr = match &ctx.tree(lval).kind {
                    TreeKind::TypeCast { expr: Some(expr), .. } => *expr,
                    _ => panic!("expected a type cast"),
                };
                self.abstract_lval(ctx, expr, builder)
            }
            _ => panic!("AssertionError: unexpected lvalue"),
        }
    }

    /// Evaluate and discard the first expression, then evaluate the second.
    fn make_comma(&mut self, ctx: &mut CompilerContext, expr1: TreeId, expr2: TreeId) -> TreeId {
        let exec = self.make.borrow_mut().exec(ctx, expr1);
        let expr2_ty = ctx.tree(expr2).ty;
        let res = self.make.borrow_mut().let_expr(ctx, List::of(exec), expr2);
        ctx.tree_mut(res).ty = expr2_ty;
        res
    }

    /// Return the signature string of a type list.
    #[allow(dead_code)]
    fn args_type_sig(&mut self, ctx: &mut CompilerContext, type_list: List<TypeId>) -> String {
        let mut signature = LowerSignatureGenerator::new(Rc::clone(&self.types));
        let syms = self.syms.borrow();
        signature
            .assemble_sig_list(ctx, &syms, type_list)
            .expect("assembled signature");
        signature.to_signature_string()
    }


    /// Lower a conditional expression, pruning constant conditions.
    fn lower_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond: Some(cond),
                truepart,
                falsepart,
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("expected a conditional"),
        };
        let bool_type = self.syms.borrow().boolean_type;
        let cond = self
            .translate_with_type(ctx, Some(cond), bool_type)
            .expect("translated condition");
        let tree_ty = ctx.tree(tree).ty.expect("conditional type");
        if let TreeKind::Conditional { cond: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(cond);
        }
        if self.is_true(ctx, cond) {
            let translated = self
                .translate_with_type(ctx, truepart, tree_ty)
                .expect("translated then part");
            let converted = self.convert(ctx, translated, tree_ty);
            self.result = Some(converted);
            self.add_pruned_info(cond);
        } else if self.is_false(ctx, cond) {
            let translated = self
                .translate_with_type(ctx, falsepart, tree_ty)
                .expect("translated else part");
            let converted = self.convert(ctx, translated, tree_ty);
            self.result = Some(converted);
            self.add_pruned_info(cond);
        } else {
            let truepart = self.translate_with_type(ctx, truepart, tree_ty);
            let falsepart = self.translate_with_type(ctx, falsepart, tree_ty);
            if let TreeKind::Conditional {
                truepart: t, falsepart: f, ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *t = truepart;
                *f = falsepart;
            }
            self.result = Some(tree);
        }
    }

    /// Lower an if statement, pruning constant conditions.
    fn lower_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond: Some(cond),
                thenpart,
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let bool_type = self.syms.borrow().boolean_type;
        let cond = self
            .translate_with_type(ctx, Some(cond), bool_type)
            .expect("translated condition");
        if let TreeKind::If { cond: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(cond);
        }
        if self.is_true(ctx, cond) {
            self.result = self.translate(ctx, thenpart);
            self.add_pruned_info(cond);
        } else if self.is_false(ctx, cond) {
            if let Some(elsepart) = elsepart {
                self.result = self.translate(ctx, Some(elsepart));
            } else {
                let skip = self.make.borrow_mut().skip(ctx);
                self.result = Some(skip);
            }
            self.add_pruned_info(cond);
        } else {
            let thenpart = self.translate(ctx, thenpart);
            let elsepart = self.translate(ctx, elsepart);
            if let TreeKind::If {
                thenpart: t, elsepart: e, ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *t = thenpart;
                *e = elsepart;
            }
            self.result = Some(tree);
        }
    }

    /// Lower an assert statement into the assertions-disabled flag check.
    fn lower_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert {
                cond: Some(cond),
                detail,
            } => (*cond, *detail),
            _ => panic!("expected an assert statement"),
        };
        let bool_type = self.syms.borrow().boolean_type;
        let cond = self
            .translate_with_type(ctx, Some(cond), bool_type)
            .expect("translated assert condition");
        if let TreeKind::Assert { cond: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(cond);
        }
        let cond_ty = ctx.tree(cond).ty.expect("condition type");
        if !ctx.typ(cond_ty).is_true() {
            let pos = ctx.tree(tree).pos;
            let flag = self.assert_flag_test(ctx, pos);
            let exn_args = match detail {
                Some(detail) => List::of(self.translate(ctx, Some(detail)).expect("translated detail")),
                None => List::nil(),
            };
            let mut check = flag;
            if !ctx.typ(cond_ty).is_false() {
                let not_cond = self.make_unary(ctx, Tag::NOT, cond);
                check = self.make_binary(ctx, Tag::AND, check, not_cond);
            }
            let assertion_error_type = self.syms.borrow().assertion_error_type;
            let exception = self.make_new_class(ctx, assertion_error_type, exn_args);
            self.make.borrow_mut().at(pos);
            let throw = self.make.borrow_mut().throw_(ctx, exception);
            let if_tree = self.make.borrow_mut().if_(ctx, check, throw, None);
            self.result = Some(if_tree);
        } else {
            let skip = self.make.borrow_mut().skip(ctx);
            self.result = Some(skip);
        }
    }

    /// Lower a method invocation, rewriting constructor calls and accessors.
    fn lower_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (_typeargs, meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation {
                typeargs,
                meth: Some(meth),
                args,
                ..
            } => (typeargs.clone(), *meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        let meth_sym = tree_info::symbol(ctx, meth).expect("method symbol");
        let meth_ty = ctx.sym(meth_sym).ty.expect("method type");
        let mut argtypes = self.types.get_parameter_types_public(ctx, meth_ty);
        let meth_name = ctx.sym(meth_sym).name.clone();
        let enum_sym = self.syms.borrow().enum_sym;
        if meth_name == self.names.init && ctx.sym(meth_sym).owner == Some(enum_sym) {
            argtypes = argtypes.tail().tail();
        }
        let varargs_element = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { varargs_element, .. } => *varargs_element,
            _ => None,
        };
        let args = self.box_args(ctx, argtypes, args, varargs_element);
        if let TreeKind::MethodInvocation {
            args: slot,
            varargs_element: varargs_slot,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = args.clone();
            *varargs_slot = None;
        }
        let meth_tree_name = tree_info::name(ctx, meth);
        if meth_name == self.names.init {
            let pos = ctx.tree(tree).pos;
            let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
            );
            let constructor = self.access_constructor(ctx, &diagnostic_pos, meth_sym);
            let mut args = args;
            if constructor != meth_sym {
                let null = self.make_null(ctx);
                args = args.append(null);
                let new_meth = self.make.borrow_mut().ident_symbol(ctx, constructor);
                let ident_name = meth_tree_name
                    .clone()
                    .unwrap_or_else(|| ctx.sym(constructor).name.clone());
                if let TreeKind::Ident { name, .. } = &mut ctx.tree_mut(new_meth).kind {
                    *name = ident_name;
                }
                if let TreeKind::MethodInvocation { meth: slot, .. } = &mut ctx.tree_mut(tree).kind {
                    *slot = Some(new_meth);
                }
            }
            let constructor_owner = ctx.sym(constructor).owner.expect("constructor owner");
            let c = crate::code::symbol::symbol_encl_class(ctx, constructor_owner)
                .expect("constructor class");
            if Symbol::is_directly_or_indirectly_local(ctx, c) {
                let freevars = self.freevars(ctx, c).unwrap_or_else(List::nil);
                let loaded = self.load_freevars(ctx, pos, freevars);
                args = args.append_list(&loaded);
            }
            let is_enum = (ctx.sym(c).flags_field & crate::code::flags::ENUM as i64) != 0
                || ctx.sym(c).get_qualified_name().content_equals(
                    &javac_strings::JavaString::from("java.lang.Enum"),
                );
            if is_enum {
                let current_method_def = self.current_method_def.expect("current method definition");
                let mut params = match &ctx.tree(current_method_def).kind {
                    TreeKind::MethodDecl { params, .. } => params.clone(),
                    _ => List::nil(),
                };
                let current_method_sym = self.current_method_sym.expect("current method symbol");
                let method_owner = ctx.sym(current_method_sym).owner.expect("method owner");
                if ctx.sym(method_owner).has_outer_instance(ctx) {
                    params = params.tail();
                }
                let name_param = Self::param_sym(ctx, *params.head());
                let ord_param = Self::param_sym(ctx, *params.tail().head());
                let name_ident = self.make.borrow_mut().ident_symbol(ctx, name_param);
                let ord_ident = self.make.borrow_mut().ident_symbol(ctx, ord_param);
                args = args.prepend(ord_ident).prepend(name_ident);
            }
            if ctx.sym(c).has_outer_instance(ctx) {
                let this_arg = if ctx.tree(meth).has_tag(Tag::SELECT) {
                    let selected = match &ctx.tree(meth).kind {
                        TreeKind::FieldAccess {
                            selected: Some(selected),
                            ..
                        } => *selected,
                        _ => panic!("expected a field access"),
                    };
                    let translated = self.translate(ctx, Some(selected)).expect("translated qualifier");
                    let checked = self.attr.borrow_mut().make_null_check(ctx, translated);
                    let new_meth = self.make.borrow_mut().ident_symbol(ctx, constructor);
                    if let Some(meth_name) = meth_tree_name.clone()
                        && let TreeKind::Ident { name, .. } = &mut ctx.tree_mut(new_meth).kind
                    {
                        *name = meth_name;
                    }
                    if let TreeKind::MethodInvocation { meth: slot, .. } = &mut ctx.tree_mut(tree).kind {
                        *slot = Some(new_meth);
                    }
                    checked
                } else if Symbol::is_directly_or_indirectly_local(ctx, c)
                    || meth_tree_name == Some(self.names._this.clone())
                {
                    let c_ty = ctx.sym(c).ty.expect("constructor class type");
                    let enclosing = ctx.typ(c_ty).get_enclosing_type().expect("enclosing type");
                    let enclosing_tsym = ctx.typ(enclosing).tsym.expect("enclosing symbol");
                    let pos = ctx.tree(meth).pos;
                    let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
                        crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                    );
                    self.make_this(ctx, &diagnostic_pos, enclosing_tsym)
                } else {
                    let current_class = self.current_class.expect("current class");
                    if ctx.sym(current_class).is_static(ctx) {
                        let c_ty = ctx.sym(c).ty.expect("constructor class type");
                        let enclosing = ctx.typ(c_ty).get_enclosing_type().expect("enclosing type");
                        let info = crate::resources::compiler_properties::errors::no_encl_instance_of_type_in_scope(
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                ctx.typ(enclosing).tsym.map(|sym| ctx.sym(sym).name.to_java_string().to_string_lossy().to_string()).unwrap_or_default(),
                            ),
                        );
                        crate::util::abstract_log::AbstractLog::error_at(
                            &mut *self.log.borrow_mut(),
                            ctx.tree(tree).pos,
                            crate::util::jc_diagnostic::Error::from(info),
                        );
                        let bot_type = self.syms.borrow().bot_type;
                        self.make_lit(ctx, bot_type, &crate::util::constants::ConstantValue::Integer(0))
                    } else {
                        let pos = ctx.tree(meth).pos;
                        let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
                            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                        );
                        self.make_owner_this_n(ctx, &diagnostic_pos, c, false)
                    }
                };
                args = args.prepend(this_arg);
            }
            if let TreeKind::MethodInvocation { args: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = args;
            }
        } else {
            let translated_meth = self.translate(ctx, Some(meth)).expect("translated method");
            if let TreeKind::MethodInvocation { meth: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(translated_meth);
            }
            if ctx.tree(translated_meth).has_tag(Tag::APPLY) {
                let (app_args, app_typeargs, app_meth, app_varargs, app_poly) =
                    match &ctx.tree(translated_meth).kind {
                        TreeKind::MethodInvocation {
                            typeargs,
                            meth,
                            args,
                            varargs_element,
                            poly_kind,
                        } => (
                            args.clone(),
                            typeargs.clone(),
                            *meth,
                            *varargs_element,
                            *poly_kind,
                        ),
                        _ => panic!("expected a method invocation"),
                    };
                let mut new_args = app_args;
                let mut current = args;
                while current.non_empty() {
                    new_args = new_args.prepend(*current.head());
                    current = current.tail();
                }
                let app = ctx.alloc_tree(crate::tree::jc_tree::JCTree {
                    pos: ctx.tree(translated_meth).pos,
                    ty: ctx.tree(translated_meth).ty,
                    tag: Tag::APPLY,
                    kind: TreeKind::MethodInvocation {
                        typeargs: app_typeargs,
                        meth: app_meth,
                        args: new_args,
                        varargs_element: app_varargs,
                        poly_kind: app_poly,
                    },
                });
                self.result = Some(app);
                return;
            }
        }
        self.result = Some(tree);
    }


    /// Create the auxiliary class that holds the assertions-disabled flag for
    /// interfaces.
    fn assertions_disabled_class(&mut self, ctx: &mut CompilerContext) -> SymbolId {
        if let Some(cached) = self.assertions_disabled_class_cache {
            return cached;
        }
        let outermost = self.outermost_class_def.expect("outermost class definition");
        let outermost_sym = match &ctx.tree(outermost).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        let flags = (crate::code::flags::STATIC | crate::code::flags::SYNTHETIC) as i64;
        let cdef = self.make_empty_class(ctx, flags, outermost_sym, None, true);
        let sym = match &ctx.tree(cdef).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        self.assertions_disabled_class_cache = Some(sym);
        sym
    }

    /// Return the tree testing the assertions-disabled flag, creating the flag
    /// and its initializer when needed.
    fn assert_flag_test(&mut self, ctx: &mut CompilerContext, pos: i32) -> TreeId {
        let _ = pos;
        let current_class = self.current_class.expect("current class");
        let container = if !ctx.sym(current_class).is_interface() {
            current_class
        } else {
            self.assertions_disabled_class(ctx)
        };
        let members = ctx.sym(container).members().expect("class members");
        let dollar_name = self.dollar_assertions_disabled.clone();
        let mut assert_disabled = self.lookup_synthetic(ctx, &dollar_name, members);
        if assert_disabled.is_none() {
            let boolean_type = self.syms.borrow().boolean_type;
            let flags =
                (crate::code::flags::STATIC | crate::code::flags::FINAL | crate::code::flags::SYNTHETIC)
                    as i64;
            let symbol = Symbol::new_var_symbol(flags, dollar_name, boolean_type, container);
            let var = ctx.alloc_symbol(symbol);
            self.enter_synthetic(ctx, var, members);
            let desired_name = self.names.desiredAssertionStatus.clone();
            let class_type = self.syms.borrow().class_type;
            let class_erasure = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, class_type)
            };
            let desired_sym = self.lookup_method(
                ctx,
                None,
                desired_name,
                class_erasure,
                List::nil(),
            );
            let container_def = self.class_def(ctx, container).expect("class definition");
            let container_pos = ctx.tree(container_def).pos;
            let outermost = self.outermost_class_def.expect("outermost class definition");
            let outermost_sym = match &ctx.tree(outermost).kind {
                TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                _ => panic!("expected a class declaration"),
            };
            let outermost_ty = ctx.sym(outermost_sym).ty.expect("outermost class type");
            let outermost_erasure = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, outermost_ty)
            };
            let class_literal = self.class_of_type(ctx, outermost_erasure, container_pos);
            let status = self.select_symbol(ctx, class_literal, desired_sym);
            let status_app = self.make.borrow_mut().app(ctx, status);
            let not_status = self.make_unary(ctx, Tag::NOT, status_app);
            let flags_for_def = ctx.sym(var).flags_field;
            let var_name = ctx.sym(var).name.clone();
            let boolean_tree = self.type_tree(ctx, boolean_type);
            let mods = self.make.borrow_mut().modifiers(ctx, flags_for_def);
            let assert_disabled_def = self.make.borrow_mut().var_def(
                ctx,
                mods,
                var_name,
                Some(boolean_tree),
                Some(not_status),
            );
            if let TreeKind::VariableDecl { sym: sym_slot, .. } = &mut ctx.tree_mut(assert_disabled_def).kind {
                *sym_slot = Some(var);
            }
            if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(container_def).kind {
                let new_defs = defs.prepend(assert_disabled_def);
                *defs = new_defs;
            }
            if ctx.sym(current_class).is_interface() {
                let current_def = self.class_def(ctx, current_class).expect("current class definition");
                let current_pos = ctx.tree(current_def).pos;
                self.make.borrow_mut().at(current_pos);
                let flag_ident = {
                    self.make.borrow_mut().qual_ident(ctx, var)
                };
                let skip = self.make.borrow_mut().skip(ctx);
                let dummy = self.make.borrow_mut().if_(ctx, flag_ident, skip, None);
                let static_flag = crate::code::flags::STATIC as i64;
                let clinit = self.make.borrow_mut().block(ctx, static_flag, List::of(dummy));
                if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(current_def).kind {
                    let new_defs = defs.prepend(clinit);
                    *defs = new_defs;
                }
            }
            assert_disabled = Some(var);
        }
        let var = assert_disabled.expect("assertions disabled flag");
        let ident = self.make.borrow_mut().ident_symbol(ctx, var);
        self.make_unary(ctx, Tag::NOT, ident)
    }


    /// Lower a type cast, translating the expression with boxing when the
    /// primitive-ness differs.
    fn lower_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast {
                clazz,
                expr: Some(expr),
            } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.translate(ctx, clazz);
        if let TreeKind::TypeCast { clazz: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = clazz;
        }
        let tree_ty = ctx.tree(tree).ty.expect("cast type");
        let clazz_ty = ctx.tree(clazz.expect("cast clazz")).ty.expect("cast clazz type");
        let expr_ty = ctx.tree(expr).ty.expect("cast expression type");
        let expr = if ctx.typ(clazz_ty).is_primitive() != ctx.typ(expr_ty).is_primitive() {
            self.translate_with_type(ctx, Some(expr), tree_ty)
                .expect("translated cast expression")
        } else {
            self.translate(ctx, Some(expr)).expect("translated cast expression")
        };
        if let TreeKind::TypeCast { expr: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(expr);
        }
        self.result = Some(tree);
    }

    /// Lower a class instance creation expression.
    fn lower_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, clazz, args, def, constructor, varargs_element) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                clazz,
                args,
                def,
                constructor,
                varargs_element,
                ..
            } => (*encl, *clazz, args.clone(), *def, *constructor, *varargs_element),
            _ => panic!("expected a new class expression"),
        };
        let constructor = constructor.expect("constructor symbol");
        let constructor_owner = ctx.sym(constructor).owner.expect("constructor owner");
        let is_enum = (ctx.sym(constructor_owner).flags_field & crate::code::flags::ENUM as i64) != 0;
        let constructor_ty = ctx.sym(constructor).ty.expect("constructor type");
        let mut arg_types = self.types.get_parameter_types_public(ctx, constructor_ty);
        if is_enum {
            let int_type = self.syms.borrow().int_type;
            let string_type = self.syms.borrow().string_type;
            arg_types = arg_types.prepend(int_type).prepend(string_type);
        }
        let mut args = self.box_args(ctx, arg_types, args, varargs_element);
        let mut encl = encl;
        let mut def = def;
        if Symbol::is_directly_or_indirectly_local(ctx, constructor_owner) {
            let pos = ctx.tree(tree).pos;
            let freevars = self.freevars(ctx, constructor_owner).unwrap_or_else(List::nil);
            let loaded = self.load_freevars(ctx, pos, freevars);
            args = args.append_list(&loaded);
        }
        let pos = ctx.tree(tree).pos;
        let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
        );
        let access_constructor = self.access_constructor(ctx, &diagnostic_pos, constructor);
        if access_constructor != constructor {
            let null = self.make_null(ctx);
            args = args.append(null);
            if let TreeKind::NewClass { constructor: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(access_constructor);
            }
        }
        let c = crate::code::symbol::symbol_encl_class(ctx, constructor_owner)
            .expect("constructor class");
        if ctx.sym(c).has_outer_instance(ctx) {
            let this_arg = if let Some(encl) = encl {
                let translated = self.translate(ctx, Some(encl)).expect("translated enclosing expression");
                let checked = self.attr.borrow_mut().make_null_check(ctx, translated);
                let encl_ty = ctx.tree(encl).ty;
                if let Some(encl_ty) = encl_ty {
                    ctx.tree_mut(checked).ty = Some(encl_ty);
                }
                checked
            } else if Symbol::is_directly_or_indirectly_local(ctx, c) {
                let c_ty = ctx.sym(c).ty.expect("class type");
                let enclosing = ctx.typ(c_ty).get_enclosing_type().expect("enclosing type");
                let enclosing_tsym = ctx.typ(enclosing).tsym.expect("enclosing symbol");
                let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                );
                self.make_this(ctx, &diagnostic_pos, enclosing_tsym)
            } else {
                let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                );
                self.make_owner_this_diag(ctx, &diagnostic_pos, c, false)
            };
            args = args.prepend(this_arg);
        }
        encl = None;
        if let Some(def_tree) = def {
            let prev_lambda_map = self.lambda_translation_map.take();
            self.translate(ctx, Some(def_tree));
            self.lambda_translation_map = prev_lambda_map;
            let def_sym = match &ctx.tree(def_tree).kind {
                TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                _ => panic!("expected a class declaration"),
            };
            let clazz_pos = ctx.tree(clazz.expect("class expression")).pos;
            let ident = self.make.borrow_mut().ident_symbol(ctx, def_sym);
            ctx.tree_mut(ident).pos = clazz_pos;
            let accessed = self.access(ctx, def_sym, ident, self.encl_op, false);
            if let TreeKind::NewClass { clazz: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(accessed);
            }
            def = None;
        } else {
            let clazz_tree = clazz.expect("class expression");
            let accessed = self.access(ctx, constructor_owner, clazz_tree, self.encl_op, false);
            if let TreeKind::NewClass { clazz: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(accessed);
            }
        }
        if let TreeKind::NewClass {
            encl: encl_slot,
            args: args_slot,
            def: def_slot,
            varargs_element: varargs_slot,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *encl_slot = encl;
            *args_slot = args;
            *def_slot = def;
            *varargs_slot = None;
        }
        self.result = Some(tree);
    }

    /// Lower an array creation expression.
    fn lower_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (elemtype, dims, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray {
                elemtype,
                dims,
                elems,
                ..
            } => (*elemtype, dims.clone(), elems.clone()),
            _ => panic!("expected a new array expression"),
        };
        let elemtype = self.translate(ctx, elemtype);
        let mut current = dims;
        let mut index = 0;
        let int_type = self.syms.borrow().int_type;
        let mut translated_dims: Vec<TreeId> = Vec::new();
        while current.non_empty() {
            let dim = *current.head();
            if index + 1 < current.length() {
                let translated = self
                    .translate_with_type(ctx, Some(dim), int_type)
                    .expect("translated dimension");
                translated_dims.push(translated);
            } else {
                translated_dims.push(dim);
            }
            index += 1;
            current = current.tail();
        }
        let new_dims = List::from_iterable(translated_dims);
        let tree_ty = ctx.tree(tree).ty.expect("array type");
        let elems = match elems {
            Some(elems) => {
                let elem_ty = {
            let syms = self.syms.borrow();
            self.types.elemtype(ctx, &syms, tree_ty)
        };
                let elem_ty = if ctx.typ(elem_ty).has_tag(TypeTag::NONE) {
                    tree_ty
                } else {
                    elem_ty
                };
                let translated = self.translate_list_with_type(ctx, elems, elem_ty);
                Some(translated)
            }
            None => None,
        };
        if let TreeKind::NewArray {
            elemtype: e,
            dims: d,
            elems: els,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = elemtype;
            *d = new_dims;
            *els = elems;
        }
        self.result = Some(tree);
    }

    /// Lower a parenthesized expression, dropping the parentheses when the
    /// subexpression changed.
    fn lower_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parentheses"),
        };
        let translated = self.translate(ctx, expr).expect("translated expression");
        if Some(translated) == expr {
            self.result = Some(tree);
        } else {
            self.result = Some(translated);
        }
    }

    /// Lower an array access expression.
    fn lower_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess {
                indexed: Some(indexed),
                index: Some(index),
            } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.translate(ctx, Some(indexed)).expect("translated indexed expression");
        let int_type = self.syms.borrow().int_type;
        let index = self
            .translate_with_type(ctx, Some(index), int_type)
            .expect("translated index");
        if let TreeKind::ArrayAccess {
            indexed: i, index: n,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = Some(indexed);
            *n = Some(index);
        }
        self.result = Some(tree);
    }

    /// Lower an assignment expression, appending the value to access-method
    /// invocations when the left hand side was rewritten to one.
    fn lower_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign {
                lhs: Some(lhs),
                rhs: Some(rhs),
            } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self
            .translate_with_encl_op(ctx, Some(lhs), Some(tree))
            .expect("translated left hand side");
        let lhs_ty = ctx.tree(lhs).ty.expect("left hand side type");
        let rhs = self
            .translate_with_type(ctx, Some(rhs), lhs_ty)
            .expect("translated right hand side");
        let (app_args, app_meth, app_typeargs, app_varargs, app_poly, app_pos, app_ty) =
            match &ctx.tree(lhs).kind {
                TreeKind::MethodInvocation {
                    typeargs,
                    meth,
                    args,
                    varargs_element,
                    poly_kind,
                } => (
                    args.clone(),
                    *meth,
                    typeargs.clone(),
                    *varargs_element,
                    *poly_kind,
                    ctx.tree(lhs).pos,
                    ctx.tree(lhs).ty,
                ),
                _ => {
                    if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
                        *l = Some(lhs);
                        *r = Some(rhs);
                    }
                    self.result = Some(tree);
                    return;
                }
            };
        let mut new_args = app_args;
        new_args = new_args.prepend(rhs);
        let app = ctx.alloc_tree(crate::tree::jc_tree::JCTree {
            pos: app_pos,
            ty: app_ty,
            tag: Tag::APPLY,
            kind: TreeKind::MethodInvocation {
                typeargs: app_typeargs,
                meth: app_meth,
                args: new_args,
                varargs_element: app_varargs,
                poly_kind: app_poly,
            },
        });
        self.result = Some(app);
    }

    /// Lower a compound assignment expression.
    fn lower_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, operator) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp {
                lhs: Some(lhs),
                rhs: Some(rhs),
                operator,
            } => (*lhs, *rhs, *operator),
            _ => panic!("expected a compound assignment"),
        };
        let lhs_ty = ctx.tree(lhs).ty.expect("left hand side type");
        let operator_ty = ctx.sym(operator.expect("operator symbol")).ty.expect("operator type");
        let return_ty = self.types.get_return_type_public(ctx, operator_ty);
        let boxing_req = !ctx.typ(lhs_ty).is_primitive() && ctx.typ(return_ty).is_primitive();
        let mut dep_scanner = AssignopDependencyScanner {
            sym: tree_info::symbol(ctx, lhs),
            dependency_found: false,
        };
        dep_scanner.scan(ctx, Some(rhs));
        let rhs_ty = ctx.tree(rhs).ty.expect("right hand side type");
        if boxing_req || dep_scanner.dependency_found {
            let tree_ty = ctx.tree(tree).ty.expect("assignop type");
            let mut builder = BoxedAssignopBuilder {
                opcode: ctx.tree(tree).get_tag(),
                tree_ty,
                rhs,
                rhs_ty,
                boxing_req,
            };
            let new_tree = self.abstract_lval(ctx, lhs, &mut builder);
            self.result = self.translate(ctx, Some(new_tree));
            return;
        }
        let lhs = self
            .translate_with_encl_op(ctx, Some(lhs), Some(tree))
            .expect("translated left hand side");
        let operator_ty = ctx.sym(operator.expect("operator symbol")).ty.expect("operator type");
        let params = self.types.get_parameter_types_public(ctx, operator_ty);
        let param = *params.tail().head();
        let rhs = self
            .translate_with_type(ctx, Some(rhs), param)
            .expect("translated right hand side");
        if let TreeKind::AssignOp {
            lhs: l, rhs: r, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = Some(lhs);
            *r = Some(rhs);
        }
        if ctx.tree(lhs).has_tag(Tag::APPLY) {
            let (app_args, app_meth, app_typeargs, app_varargs, app_poly, app_pos, app_ty) =
                match &ctx.tree(lhs).kind {
                    TreeKind::MethodInvocation {
                        typeargs,
                        meth,
                        args,
                        varargs_element,
                        poly_kind,
                    } => (
                        args.clone(),
                        *meth,
                        typeargs.clone(),
                        *varargs_element,
                        *poly_kind,
                        ctx.tree(lhs).pos,
                        ctx.tree(lhs).ty,
                    ),
                    _ => unreachable!(),
                };
            let opcode = match &ctx.sym(operator.expect("operator symbol")).kind_data {
                SymbolKind::OperatorSymbol { opcode, .. } => *opcode,
                _ => panic!("expected an operator symbol"),
            };
            let rhs = if opcode == crate::jvm::byte_codes::STRING_ADD {
                self.make_string(ctx, rhs)
            } else {
                rhs
            };
            let mut new_args = app_args;
            new_args = new_args.prepend(rhs);
            let app = ctx.alloc_tree(crate::tree::jc_tree::JCTree {
                pos: app_pos,
                ty: app_ty,
                tag: Tag::APPLY,
                kind: TreeKind::MethodInvocation {
                    typeargs: app_typeargs,
                    meth: app_meth,
                    args: new_args,
                    varargs_element: app_varargs,
                    poly_kind: app_poly,
                },
            });
            self.result = Some(app);
        } else {
            self.result = Some(tree);
        }
    }

    /// Lower a boxed increment/decrement expression.
    fn lower_boxed_postop(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg: Some(arg), .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        let cast = ctx
            .tree(tree_info::skip_parens(ctx, arg))
            .has_tag(Tag::TYPECAST);
        let arg_ty = ctx.tree(arg).ty.expect("operand type");
        let opcode = if ctx.tree(tree).has_tag(Tag::POSTINC) {
            Tag::PLUS_ASG
        } else {
            Tag::MINUS_ASG
        };
        let tree_ty = ctx.tree(tree).ty.expect("update type");
        let mut builder = BoxedPostopOuterBuilder {
            arg_ty,
            cast,
            opcode,
            tree_ty,
        };

        self.abstract_lval(ctx, arg, &mut builder)
    }


    /// Clone a tree value into a fresh arena entry.
    fn clone_tree(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let node = ctx.tree(tree);
        let cloned = crate::tree::jc_tree::JCTree {
            pos: node.pos,
            ty: node.ty,
            tag: node.tag,
            kind: node.kind.clone(),
        };
        ctx.alloc_tree(cloned)
    }


    /// Lower a unary expression.
    fn lower_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (arg, arg_ty) = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg: Some(arg), .. } => {
                let ty = ctx.tree(*arg).ty.expect("operand type");
                (*arg, ty)
            }
            _ => panic!("expected a unary expression"),
        };
        let tag = ctx.tree(tree).get_tag();
        let is_update_operator = tag.is_inc_or_dec_unary_op();
        if is_update_operator && !ctx.typ(arg_ty).is_primitive() {
            match tag {
                Tag::PREINC | Tag::PREDEC => {
                    let opcode = if tag == Tag::PREINC {
                        Tag::PLUS_ASG
                    } else {
                        Tag::MINUS_ASG
                    };
                    let one = self
                        .make
                        .borrow_mut()
                        .literal(ctx, TypeTag::INT, Some(crate::tree::jc_tree::LiteralValue::Integer(1)));
                    let new_tree = self.make_assignop(ctx, opcode, arg, one);
                    let tree_ty = ctx.tree(tree).ty.expect("update type");
                    self.result = self.translate_with_type(ctx, Some(new_tree), tree_ty);
                    return;
                }
                Tag::POSTINC | Tag::POSTDEC => {
                    let lowered = self.lower_boxed_postop(ctx, tree);
                    let tree_ty = ctx.tree(tree).ty.expect("update type");
                    self.result = self.translate_with_type(ctx, Some(lowered), tree_ty);
                    return;
                }
                _ => panic!("AssertionError: unexpected update operator"),
            }
        }
        let translated = self
            .translate_with_encl_op(ctx, Some(arg), Some(tree))
            .expect("translated operand");
        let tree_ty = ctx.tree(tree).ty.expect("unary type");
        let translated = self.box_if_needed(ctx, translated, tree_ty);
        if tag == Tag::NOT {
            let translated_ty = ctx.tree(translated).ty.expect("operand type");
            if ctx.typ(translated_ty).const_value().is_some() {
                let folded = self.cfolder.fold1(
                    ctx,
                    &self.syms.borrow(),
                    crate::jvm::byte_codes::BOOL_NOT,
                    translated_ty,
                );
                let folded = folded.unwrap_or(translated_ty);
                ctx.tree_mut(tree).ty = Some(folded);
            }
        }
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(translated);
        }
        if is_update_operator && ctx.tree(translated).has_tag(Tag::APPLY) {
            self.result = Some(translated);
        } else {
            self.result = Some(tree);
        }
    }

    /// Lower a binary expression, pruning constant short-circuit operands.
    fn lower_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, operator) = match &ctx.tree(tree).kind {
            TreeKind::Binary {
                lhs: Some(lhs),
                rhs: Some(rhs),
                operator,
            } => (*lhs, *rhs, *operator),
            _ => panic!("expected a binary expression"),
        };
        let operator = operator.expect("operator symbol");
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let formals = self.types.get_parameter_types_public(ctx, operator_ty);
        let first = *formals.head();
        let second = *formals.tail().head();
        let lhs = self
            .translate_with_type(ctx, Some(lhs), first)
            .expect("translated left operand");
        let tag = ctx.tree(tree).get_tag();
        match tag {
            Tag::OR => {
                if self.is_true(ctx, lhs) {
                    self.result = Some(lhs);
                    return;
                }
                if self.is_false(ctx, lhs) {
                    self.result = self.translate_with_type(ctx, Some(rhs), second);
                    return;
                }
            }
            Tag::AND => {
                if self.is_false(ctx, lhs) {
                    self.result = Some(lhs);
                    return;
                }
                if self.is_true(ctx, lhs) {
                    self.result = self.translate_with_type(ctx, Some(rhs), second);
                    return;
                }
            }
            _ => {}
        }
        let rhs = self
            .translate_with_type(ctx, Some(rhs), second)
            .expect("translated right operand");
        if let TreeKind::Binary {
            lhs: l, rhs: r, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = Some(lhs);
            *r = Some(rhs);
        }
        self.result = Some(tree);
    }

    /// Lower an identifier reference through the access machinery.
    fn lower_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::Ident { sym: Some(sym), .. } => *sym,
            _ => panic!("expected an identifier"),
        };
        self.result = Some(self.access(ctx, sym, tree, self.encl_op, false));
    }

    /// Lower a field selection expression.
    fn lower_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selected, field_name, field_sym) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess {
                selected: Some(selected),
                name,
                sym,
            } => (*selected, name.clone(), *sym),
            _ => panic!("expected a field access"),
        };
        let current_class = self.current_class.expect("current class");
        let qualified_super_access = ctx.tree(selected).has_tag(Tag::SELECT)
            && tree_info::name(ctx, selected) == Some(self.names._super.clone())
            && {
                let selected_type = ctx.tree(selected).ty.expect("qualifier type");
                let tsym = ctx.typ(selected_type).tsym;
                match tsym {
                    Some(tsym) => {
                        let syms = self.syms.borrow();
                        !self
                            .types
                            .is_direct_super_interface(ctx, &syms, tsym, current_class)
                    }
                    None => false,
                }
            };
        let translated = self.translate(ctx, Some(selected)).expect("translated qualifier");
        if let TreeKind::FieldAccess { selected: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(translated);
        }
        if field_name == self.names._class {
            let class_tree = self.class_of_type(
                ctx,
                ctx.tree(translated)
                    .ty
                    .expect("qualifier type"),
                ctx.tree(translated).pos,
            );
            self.result = Some(class_tree);
            return;
        }
        let qualifier_ty = ctx.tree(translated).ty.expect("qualifier type");
        let qualifier_tsym = ctx.typ(qualifier_ty).tsym;
        if field_name == self.names._super
            && let Some(qualifier_tsym) = qualifier_tsym
        {
            let is_direct = {
                let syms = self.syms.borrow();
                self.types
                    .is_direct_super_interface(ctx, &syms, qualifier_tsym, current_class)
            };
            if is_direct {
                let current_class_ty = ctx.sym(current_class).ty.expect("current class type");
                let syms = self.syms.borrow();
                assert!(
                    self.types.as_super(ctx, &syms, current_class_ty, qualifier_tsym).is_some(),
                    "AssertionError: default super not a superclass"
                );
                drop(syms);
                self.result = Some(tree);
                return;
            }
        }
        if field_name == self.names._this || field_name == self.names._super {
            let selected_ty = ctx.tree(translated).ty.expect("qualifier type");
            let selected_tsym = ctx.typ(selected_ty).tsym.expect("qualifier symbol");
            let pos = ctx.tree(tree).pos;
            let diagnostic_pos: Rc<dyn DiagnosticPosition> = Rc::new(
                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
            );
            self.result = Some(self.make_this(ctx, &diagnostic_pos, selected_tsym));
            return;
        }
        let sym = field_sym.expect("field symbol");
        self.result = Some(self.access(ctx, sym, tree, self.encl_op, qualified_super_access));
    }

    /// Lower a let expression.
    fn lower_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, defs);
        let tree_ty = ctx.tree(tree).ty.expect("let expression type");
        let expr = self.translate_with_type(ctx, expr, tree_ty);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }

    /// Lower an annotation (nothing to rewrite).
    fn lower_annotation(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Lower a break statement (nothing to rewrite).
    fn lower_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Lower a yield statement.
    fn lower_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (value, target) = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, target } => (*value, *target),
            _ => panic!("expected a yield statement"),
        };
        let target_ty = target
            .and_then(|target| ctx.tree(target).ty)
            .expect("yield target type");
        let value = self
            .translate_with_type(ctx, value, target_ty)
            .expect("translated yield value");
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(value);
        }
        self.result = Some(tree);
    }


    /// Lower an enhanced for loop into an indexed array loop or an iterator
    /// loop.
    fn lower_foreach(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { expr: Some(expr), .. } => *expr,
            _ => panic!("expected an enhanced for loop"),
        };
        let expr_ty = ctx.tree(expr).ty.expect("loop expression type");
        let elem_ty = {
            let syms = self.syms.borrow();
            self.types.elemtype(ctx, &syms, expr_ty)
        };
        if ctx.typ(elem_ty).has_tag(TypeTag::NONE) {
            self.lower_iterable_foreach_loop(ctx, tree);
        } else {
            self.lower_array_foreach_loop(ctx, tree);
        }
    }

    /// Lower an enhanced for loop over an array.
    fn lower_array_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var_tree, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop {
                var: Some(var),
                expr: Some(expr),
                body: Some(body),
            } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let expr_pos = ctx.tree(expr).pos;
        self.make.borrow_mut().at(expr_pos);
        let current_method_sym = self.current_method_sym.expect("current method symbol");
        let expr_ty = ctx.tree(expr).ty.expect("loop expression type");
        let array_name = {
            let text = format!("arr{}", self.target.synthetic_name_char());
            self.names.from_string(&javac_strings::JavaString::from(text.as_str()))
        };
        let flags = crate::code::flags::SYNTHETIC as i64;
        let array_symbol =
            Symbol::new_var_symbol(flags, array_name, expr_ty, current_method_sym);
        let array_cache = ctx.alloc_symbol(array_symbol);
        let array_cache_def = self.var_def_for_symbol(ctx, array_cache, Some(expr));
        let int_type = self.syms.borrow().int_type;
        let len_name = {
            let text = format!("len{}", self.target.synthetic_name_char());
            self.names.from_string(&javac_strings::JavaString::from(text.as_str()))
        };
        let len_symbol = Symbol::new_var_symbol(flags, len_name, int_type, current_method_sym);
        let len_cache = ctx.alloc_symbol(len_symbol);
        let array_ident = self.make.borrow_mut().ident_symbol(ctx, array_cache);
        let length_var = self.syms.borrow().length_var;
        let length_sel = self.select_symbol(ctx, array_ident, length_var);
        let len_cache_def = self.var_def_for_symbol(ctx, len_cache, Some(length_sel));
        let index_name = {
            let text = format!("i{}", self.target.synthetic_name_char());
            self.names.from_string(&javac_strings::JavaString::from(text.as_str()))
        };
        let index_symbol = Symbol::new_var_symbol(flags, index_name, int_type, current_method_sym);
        let index = ctx.alloc_symbol(index_symbol);
        let zero = self
            .make
            .borrow_mut()
            .literal(ctx, TypeTag::INT, Some(crate::tree::jc_tree::LiteralValue::Integer(0)));
        let zero_ty = ctx.typ(int_type).const_type(crate::util::constants::ConstantValue::Integer(0));
        let zero_ty = ctx.alloc_type(zero_ty);
        ctx.tree_mut(zero).ty = Some(zero_ty);
        let index_def = self.var_def_for_symbol(ctx, index, Some(zero));
        let index_init = match &ctx.tree(index_def).kind {
            TreeKind::VariableDecl { init, .. } => *init,
            _ => None,
        };
        let var_ty = ctx.sym(index).ty.expect("index type");
        let const_ty = {
            ctx.typ(var_ty).const_type(crate::util::constants::ConstantValue::Integer(0))
        };
        let const_ty = ctx.alloc_type(const_ty);
        if let Some(init) = index_init {
            ctx.tree_mut(init).ty = Some(const_ty);
        }
        let loop_init = List::of3(array_cache_def, len_cache_def, index_def);
        let index_ident = self.make.borrow_mut().ident_symbol(ctx, index);
        let len_ident = self.make.borrow_mut().ident_symbol(ctx, len_cache);
        let cond = self.make_binary(ctx, Tag::LT, index_ident, len_ident);
        let index_ident2 = self.make.borrow_mut().ident_symbol(ctx, index);
        let step = self.make_unary(ctx, Tag::PREINC, index_ident2);
        let step = self.make.borrow_mut().exec(ctx, step);
        let elemtype = {
            let syms = self.syms.borrow();
            self.types.elemtype(ctx, &syms, expr_ty)
        };
        let array_ident2 = self.make.borrow_mut().ident_symbol(ctx, array_cache);
        let index_ident3 = self.make.borrow_mut().ident_symbol(ctx, index);
        let loopvar_init = self.make.borrow_mut().indexed(ctx, array_ident2, index_ident3);
        ctx.tree_mut(loopvar_init).ty = Some(elemtype);
        let var_name = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { name, .. } => name.clone(),
            _ => panic!("expected a variable declaration"),
        };
        let var_mods = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { mods, .. } => *mods,
            _ => None,
        };
        let var_vartype = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { vartype, .. } => *vartype,
            _ => None,
        };
        let var_sym = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a variable symbol"),
        };
        let loopvar_def = self.make.borrow_mut().var_def(
            ctx,
            var_mods.expect("variable modifiers"),
            var_name,
            var_vartype,
            Some(loopvar_init),
        );
        let var_ty = ctx.sym(var_sym).ty.expect("loop variable type");
        ctx.tree_mut(loopvar_def).ty = Some(var_ty);
        if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(loopvar_def).kind {
            *slot = Some(var_sym);
        }
        let body_block = self.make.borrow_mut().block(ctx, 0, List::of2(loopvar_def, body));
        let for_loop = self.make.borrow_mut().for_loop(
            ctx,
            loop_init,
            Some(cond),
            List::of(step),
            body_block,
        );
        let result = self.translate(ctx, Some(for_loop)).expect("translated for loop");
        self.patch_targets(ctx, body_block, tree, result);
        self.result = Some(result);
    }

    /// Lower an enhanced for loop over an Iterable.
    fn lower_iterable_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var_tree, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop {
                var: Some(var),
                expr: Some(expr),
                body: Some(body),
            } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let expr_pos = ctx.tree(expr).pos;
        self.make.borrow_mut().at(expr_pos);
        let current_method_sym = self.current_method_sym.expect("current method symbol");
        let expr_ty = ctx.tree(expr).ty.expect("loop expression type");

        let object_type = self.syms.borrow().object_type;
        let mut iterator_target = object_type;
        let iterable_type = {
            let syms = self.syms.borrow();
            let cvar_upper_bound = self.types.cvar_upper_bound(ctx, expr_ty);
            let iterable_tsym = ctx.typ(self.syms.borrow().iterable_type).tsym.expect("iterable symbol");
            self.types.as_super(ctx, &syms, cvar_upper_bound, iterable_tsym)
        };
        let iterable_type = match iterable_type {
            Some(iterable_type) => iterable_type,
            None => expr_ty,
        };
        let type_args = ctx.typ(iterable_type).get_type_arguments();
        if !type_args.is_empty() {
            let head = *type_args.head();
            let syms = self.syms.borrow();
            iterator_target = self.types.erasure(ctx, &syms, head);
        }
        let e_type = {

            self.types.skip_type_vars_public(ctx, expr_ty)
        };
        let e_erasure = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, e_type)
        };
        ctx.tree_mut(expr).ty = Some(e_erasure);
        let mut expr = expr;
        let is_compound = {

            self.types.is_compound_public(ctx, e_type)
        };
        if is_compound {
            let syms = self.syms.borrow();
            let iterable_erasure = self.types.erasure(ctx, &syms, iterable_type);
            drop(syms);
            let cast_type = self.type_tree(ctx, iterable_erasure);
            expr = self.make.borrow_mut().type_cast(ctx, cast_type, expr);
            if let TreeKind::EnhancedForLoop { expr: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(expr);
            }
        }
        let iterator_name = self.names.iterator.clone();
        let iterator = self.lookup_method(ctx, None, iterator_name, e_type, List::nil());
        let iterator_ty = ctx.sym(iterator).ty.expect("iterator method type");
        let iterator_ret = self.types.get_return_type_public(ctx, iterator_ty);
        let iterator_tsym = ctx.typ(self.syms.borrow().iterator_type).tsym.expect("iterator symbol");
        let iterator_as_super = {
            let syms = self.syms.borrow();
            self.types.as_super(ctx, &syms, iterator_ret, iterator_tsym)
        };
        let itvar_ty = {
            let syms = self.syms.borrow();
            match iterator_as_super {
                Some(ty) => self.types.erasure(ctx, &syms, ty),
                None => iterator_ret,
            }
        };
        let flags = crate::code::flags::SYNTHETIC as i64;
        let it_name = {
            let text = format!("i{}", self.target.synthetic_name_char());
            self.names.from_string(&javac_strings::JavaString::from(text.as_str()))
        };
        let it_symbol = Symbol::new_var_symbol(flags, it_name, itvar_ty, current_method_sym);
        let itvar = ctx.alloc_symbol(it_symbol);
        let expr_for_call = expr;
        let iterator_sel = self.select_symbol(ctx, expr_for_call, iterator);
        let iterator_ty_erasure = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, iterator_ty)
        };
        ctx.tree_mut(iterator_sel).ty = Some(iterator_ty_erasure);
        let iterator_app = self.make.borrow_mut().app(ctx, iterator_sel);
        let init = self.var_def_for_symbol(ctx, itvar, Some(iterator_app));
        let itvar_ident = self.make.borrow_mut().ident_symbol(ctx, itvar);
        let has_next_name = self.names.hasNext.clone();
        let has_next = self.lookup_method(ctx, None, has_next_name, itvar_ty, List::nil());
        let has_next_sel = self.select_symbol(ctx, itvar_ident, has_next);
        let cond = self.make.borrow_mut().app(ctx, has_next_sel);
        let itvar_ident2 = self.make.borrow_mut().ident_symbol(ctx, itvar);
        let next_name = self.names.next.clone();
        let next = self.lookup_method(ctx, None, next_name, itvar_ty, List::nil());
        let next_sel = self.select_symbol(ctx, itvar_ident2, next);
        let mut vardef_init = self.make.borrow_mut().app(ctx, next_sel);
        let var_ty = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl {
                sym: Some(sym), ..
            } => ctx.sym(*sym).ty.expect("loop variable type"),
            _ => panic!("expected a variable declaration"),
        };
        let vardef_type = if ctx.typ(var_ty).is_primitive() {

            self.types.cvar_upper_bound(ctx, iterator_target)
        } else {
            var_ty
        };
        let cast_ty = self.type_tree(ctx, vardef_type);
        vardef_init = self.make.borrow_mut().type_cast(ctx, cast_ty, vardef_init);
        let var_name = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { name, .. } => name.clone(),
            _ => panic!("expected a variable declaration"),
        };
        let var_mods = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { mods, .. } => *mods,
            _ => None,
        };
        let var_vartype = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { vartype, .. } => *vartype,
            _ => None,
        };
        let var_sym = match &ctx.tree(var_tree).kind {
            TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a variable symbol"),
        };
        let index_def = self.make.borrow_mut().var_def(
            ctx,
            var_mods.expect("variable modifiers"),
            var_name,
            var_vartype,
            Some(vardef_init),
        );
        ctx.tree_mut(index_def).ty = Some(var_ty);
        if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(index_def).kind {
            *slot = Some(var_sym);
        }
        let body_block = self.make.borrow_mut().block(ctx, 0, List::of2(index_def, body));
        let body_end = tree_info::end_pos(ctx, body);
        if let TreeKind::Block { endpos, .. } = &mut ctx.tree_mut(body_block).kind {
            *endpos = body_end;
        }
        let for_loop = self.make.borrow_mut().for_loop(
            ctx,
            List::of(init),
            Some(cond),
            List::nil(),
            body_block,
        );
        let result = self.translate(ctx, Some(for_loop)).expect("translated for loop");
        self.patch_targets(ctx, body_block, tree, result);
        self.result = Some(result);
    }

    /// Patch break/continue/yield targets of a rewritten loop body.
    fn patch_targets(&mut self, ctx: &mut CompilerContext, body: TreeId, src: TreeId, dest: TreeId) {
        let mut patcher = Patcher { src, dest };
        patcher.scan(ctx, Some(body));
    }

    /// Lower a try statement.
    fn lower_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let resources = match &ctx.tree(tree).kind {
            TreeKind::Try { resources, .. } => resources.clone(),
            _ => panic!("expected a try statement"),
        };
        if resources.non_empty() {
            let result = self.make_twr_try(ctx, tree);
            self.result = Some(result);
            return;
        }
        let (body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                body: Some(body),
                catchers,
                finalizer,
                ..
            } => (*body, catchers.clone(), *finalizer),
            _ => panic!("expected a try statement"),
        };
        let has_body = match &ctx.tree(body).kind {
            TreeKind::Block { stats, .. } => stats.non_empty(),
            _ => true,
        };
        let has_catchers = catchers.non_empty();
        let has_finally = match finalizer {
            Some(finalizer) => match &ctx.tree(finalizer).kind {
                TreeKind::Block { stats, .. } => stats.non_empty(),
                _ => true,
            },
            None => false,
        };
        if !has_catchers && !has_finally {
            self.result = self.translate(ctx, Some(body));
            return;
        }
        if !has_body {
            if has_finally {
                self.result = match finalizer {
                    Some(finalizer) => self.translate(ctx, Some(finalizer)),
                    None => None,
                };
            } else {
                self.result = self.translate(ctx, Some(body));
            }
            return;
        }
        let body = self.translate_list(ctx, List::of(body));
        let body = *body.head();
        let catchers = self.translate_list(ctx, catchers);
        let finalizer = self.translate(ctx, finalizer);
        if let TreeKind::Try {
            body: b,
            catchers: c,
            finalizer: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *b = Some(body);
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }

    /// Lower a package declaration, creating the package-info class when
    /// configured to.
    fn lower_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if !self.need_package_info_class(ctx, tree) {
            self.result = Some(tree);
            return;
        }
        let (packge, annotations) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                pid, annotations, ..
            } => {
                let packge = pid
                    .and_then(|pid| tree_info::symbol(ctx, pid))
                    .expect("package symbol");
                (packge, annotations.clone())
            }
            _ => panic!("expected a package declaration"),
        };
        let mut flags = (crate::code::flags::ABSTRACT | crate::code::flags::INTERFACE) as i64;
        flags |= crate::code::flags::SYNTHETIC as i64;
        let package_info = match &ctx.sym(packge).kind_data {
            SymbolKind::PackageSymbol { package_info, .. } => {
                package_info.expect("package info class")
            }
            _ => panic!("expected a package symbol"),
        };
        let package_clone = ctx.sym(packge).clone();
        let mut info_symbol = ctx.sym(package_info).clone();
        info_symbol.set_attributes(ctx, &package_clone);
        info_symbol.flags_field |= flags;
        *ctx.sym_mut(package_info) = info_symbol;
        let info_ty = ctx.sym(package_info).ty.expect("package info class type");
        let object_type = self.syms.borrow().object_type;
        if let TypeKindData::ClassType {
            supertype_field,
            interfaces_field,
            ..
        } = &mut ctx.typ_mut(info_ty).kind
        {
            *supertype_field = Some(object_type);
            *interfaces_field = Some(List::nil());
        }
        self.create_info_class(ctx, annotations, package_info);
        self.result = Some(tree);
    }

    /// Return whether a package-info class must be generated.
    fn need_package_info_class(&mut self, ctx: &mut CompilerContext, pd: TreeId) -> bool {
        match self.pkginfo_opt {
            crate::main::option::PkgInfo::ALWAYS => true,
            crate::main::option::PkgInfo::LEGACY => {
                let annotations = match &ctx.tree(pd).kind {
                    TreeKind::PackageDecl { annotations, .. } => annotations.clone(),
                    _ => List::nil(),
                };
                annotations.non_empty()
            }
            crate::main::option::PkgInfo::NONEMPTY => {
                let packge = match &ctx.tree(pd).kind {
                    TreeKind::PackageDecl { pid, .. } => pid
                        .and_then(|pid| tree_info::symbol(ctx, pid))
                        .expect("package symbol"),
                    _ => panic!("expected a package declaration"),
                };
                let attributes = ctx.sym(packge).get_declaration_attributes();
                let mut current = attributes;
                while current.non_empty() {
                    let attribute = *current.head();
                    let sym = match &ctx.attr(attribute).ty {
                        Some(ty) => ctx.typ(*ty).tsym,
                        None => None,
                    };
                    let policy = {
                        let syms = self.syms.borrow();
                        match sym {
                            Some(sym) => self.types.get_retention(ctx, &syms, sym),
                            None => crate::code::attribute::RetentionPolicy::CLASS,
                        }
                    };
                    if policy != crate::code::attribute::RetentionPolicy::SOURCE {
                        return true;
                    }
                    current = current.tail();
                }
                false
            }
        }
    }

    /// Add a package-info or module-info class to the translated queue.
    fn create_info_class(
        &mut self,
        ctx: &mut CompilerContext,
        annotations: List<TreeId>,
        c: SymbolId,
    ) {
        let mut flags = (crate::code::flags::ABSTRACT | crate::code::flags::INTERFACE) as i64;
        let _ = &mut flags;
        let mods = self
            .make
            .borrow_mut()
            .modifiers_with_annotations(ctx, flags, annotations);
        let name = ctx.sym(c).name.clone();
        let info_class = self.make.borrow_mut().class_def(
            ctx,
            mods,
            name,
            List::nil(),
            None,
            List::nil(),
            List::nil(),
        );
        if let TreeKind::ClassDecl { sym: slot, .. } = &mut ctx.tree_mut(info_class).kind {
            *slot = Some(c);
        }
        self.translated.push(info_class);
    }

    /// Lower a module declaration by generating the module-info class.
    fn lower_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let msym = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                sym: Some(sym), ..
            } => *sym,
            _ => panic!("expected a module declaration"),
        };
        let module_info = match &ctx.sym(msym).kind_data {
            SymbolKind::ModuleSymbol { module_info, .. } => {
                module_info.expect("module-info class")
            }
            _ => panic!("expected a module symbol"),
        };
        let module_clone = ctx.sym(msym).clone();
        let mut info_symbol = ctx.sym(module_info).clone();
        info_symbol.set_attributes(ctx, &module_clone);
        info_symbol.flags_field |= crate::code::flags::MODULE;
        *ctx.sym_mut(module_info) = info_symbol;
        self.create_info_class(ctx, List::nil(), module_info);
        self.result = Some(tree);
    }


    /// Build an attributed literal for a literal value, matching
    /// TreeMaker.Literal(Object).
    fn make_literal_value(
        &mut self,
        ctx: &mut CompilerContext,
        value: crate::tree::jc_tree::LiteralValue,
    ) -> TreeId {
        use crate::tree::jc_tree::LiteralValue;
        use crate::util::constants::ConstantValue;
        let (tag, constant, ty) = match &value {
            LiteralValue::String(s) => {
                let ty = self.syms.borrow().string_type;
                (TypeTag::CLASS, ConstantValue::String(s.clone()), ty)
            }
            LiteralValue::Integer(v) => {
                let ty = self.syms.borrow().int_type;
                (TypeTag::INT, ConstantValue::Integer(*v), ty)
            }
            LiteralValue::Byte(v) => {
                let ty = self.syms.borrow().byte_type;
                (TypeTag::BYTE, ConstantValue::Byte(*v), ty)
            }
            LiteralValue::Short(v) => {
                let ty = self.syms.borrow().short_type;
                (TypeTag::SHORT, ConstantValue::Short(*v), ty)
            }
            LiteralValue::Long(v) => {
                let ty = self.syms.borrow().long_type;
                (TypeTag::LONG, ConstantValue::Long(*v), ty)
            }
            LiteralValue::Character(v) => {
                let ty = self.syms.borrow().char_type;
                (TypeTag::CHAR, ConstantValue::Character(*v), ty)
            }
            LiteralValue::Double(v) => {
                let ty = self.syms.borrow().double_type;
                (TypeTag::DOUBLE, ConstantValue::Double(*v), ty)
            }
            LiteralValue::Float(v) => {
                let ty = self.syms.borrow().float_type;
                (TypeTag::FLOAT, ConstantValue::Float(*v), ty)
            }
            LiteralValue::Boolean(v) => {
                let ty = self.syms.borrow().boolean_type;
                (TypeTag::BOOLEAN, ConstantValue::Boolean(*v), ty)
            }
        };
        let tree = self.make.borrow_mut().literal(ctx, tag, Some(value));
        let const_ty = ctx.typ(ty).const_type(constant);
        let const_ty = ctx.alloc_type(const_ty);
        ctx.tree_mut(tree).ty = Some(const_ty);
        tree
    }

    /// Build an int literal.
    fn make_int_literal(&mut self, ctx: &mut CompilerContext, value: i32) -> TreeId {
        self.make_literal_value(ctx, crate::tree::jc_tree::LiteralValue::Integer(value))
    }

    /// Build a String literal.
    fn make_string_literal(&mut self, ctx: &mut CompilerContext, value: javac_strings::JavaString) -> TreeId {
        self.make_literal_value(ctx, crate::tree::jc_tree::LiteralValue::String(value))
    }

    /// Lower a switch statement.
    fn lower_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch { selector, cases, .. } => (*selector, cases.clone()),
            _ => panic!("expected a switch statement"),
        };
        let pattern_switch = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                pattern_switch, ..
            } => *pattern_switch,
            _ => false,
        };
        let cases = if pattern_switch {
            self.add_default_if_needed(ctx, cases)
        } else {
            cases
        };
        let selector = selector.expect("switch selector");
        self.handle_switch(ctx, tree, selector, cases);
    }

    /// Lower a switch expression.
    fn lower_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression { selector, cases, .. } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let cases = self.add_default_if_needed(ctx, cases);
        let selector = selector.expect("switch selector");
        self.handle_switch(ctx, tree, selector, cases);
    }

    /// Add a default case throwing IncompatibleClassChangeError when the switch
    /// has no default label.
    fn add_default_if_needed(&mut self, ctx: &mut CompilerContext, cases: List<TreeId>) -> List<TreeId> {
        let mut has_default = false;
        let mut current = cases.clone();
        while current.non_empty() {
            let case = *current.head();
            let labels = match &ctx.tree(case).kind {
                TreeKind::Case { labels, .. } => labels.clone(),
                _ => List::nil(),
            };
            let mut label_current = labels;
            while label_current.non_empty() {
                if ctx
                    .tree(*label_current.head())
                    .has_tag(Tag::DEFAULTCASELABEL)
                {
                    has_default = true;
                    break;
                }
                label_current = label_current.tail();
            }
            if has_default {
                break;
            }
            current = current.tail();
        }
        if has_default {
            return cases;
        }
        let incompatible = self.syms.borrow().incompatible_class_change_error_type;
        let exception = self.make_new_class(ctx, incompatible, List::nil());
        let throw = self.make.borrow_mut().throw_(ctx, exception);
        let default_label = self.make.borrow_mut().default_case_label(ctx);
        let case_kind = javac_source::tree::case_tree::CaseKind::STATEMENT;
        let case = self.make.borrow_mut().case(
            ctx,
            case_kind,
            List::of(default_label),
            List::of(throw),
            None,
        );
        cases.prepend(case)
    }

    /// Handle a switch (statement or expression) after its cases were
    /// normalized.
    fn handle_switch(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        selector: TreeId,
        cases: List<TreeId>,
    ) {
        let mut converted: List<TreeId> = List::nil();
        let mut current = cases.clone();
        while current.non_empty() {
            let case = *current.head();
            let labels = match &ctx.tree(case).kind {
                TreeKind::Case { labels, .. } => labels.clone(),
                _ => List::nil(),
            };
            let label_count = labels.length();
            if label_count <= 1 {
                converted = converted.append(case);
            } else {
                let case_pos = ctx.tree(case).pos;
                let mut patterns = labels;
                while patterns.tail().non_empty() {
                    let head = *patterns.head();
                    self.make.borrow_mut().at(case_pos);
                    let case_kind = javac_source::tree::case_tree::CaseKind::STATEMENT;
                    let new_case = self.make.borrow_mut().case(
                        ctx,
                        case_kind,
                        List::of(head),
                        List::nil(),
                        None,
                    );
                    converted = converted.append(new_case);
                    patterns = patterns.tail();
                }
                if let TreeKind::Case { labels: slot, .. } = &mut ctx.tree_mut(case).kind {
                    *slot = patterns;
                }
                converted = converted.append(case);
            }
            current = current.tail();
        }
        let mut normalized: List<TreeId> = List::nil();
        let mut current = converted;
        while current.non_empty() {
            let case = *current.head();
            let (case_kind, completes_normally, stats) = match &ctx.tree(case).kind {
                TreeKind::Case {
                    case_kind,
                    completes_normally,
                    stats,
                    ..
                } => (*case_kind, *completes_normally, stats.clone()),
                _ => panic!("expected a case"),
            };
            if case_kind == javac_source::tree::case_tree::CaseKind::RULE && completes_normally {
                let last = stats.last().expect("case statistics");
                let end_pos = tree_info::end_pos(ctx, last);
                self.make.borrow_mut().at(end_pos);
                let break_tree = self.make.borrow_mut().break_(ctx, None);
                if let TreeKind::Break { target, .. } = &mut ctx.tree_mut(break_tree).kind {
                    *target = Some(tree);
                }
                if let TreeKind::Case { stats: slot, .. } = &mut ctx.tree_mut(case).kind {
                    *slot = slot.append(break_tree);
                }
            }
            normalized = normalized.append(case);
            current = current.tail();
        }
        let selector_ty = ctx.tree(selector).ty.expect("selector type");
        let selsuper = {
            let syms = self.syms.borrow();
            self.types.supertype(ctx, &syms, selector_ty)
        };
        let selsuper_is_class = ctx.typ(selsuper).has_tag(TypeTag::CLASS);
        let enum_switch = selsuper_is_class
            && ctx
                .typ(selector_ty)
                .tsym
                .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::ENUM as i64) != 0)
                .unwrap_or(false);
        let string_type = self.syms.borrow().string_type;
        let string_switch = selsuper_is_class && {
            let syms = self.syms.borrow();
            self.types.is_same_type(ctx, &syms, selector_ty, string_type)
        };
        let boxed_switch = !enum_switch && !string_switch && !ctx.typ(selector_ty).is_primitive();
        let selector = self
            .translate_with_type(ctx, Some(selector), selector_ty)
            .expect("translated selector");
        let normalized = self.translate_list(ctx, normalized);
        if ctx.tree(tree).has_tag(Tag::SWITCH) {
            if let TreeKind::Switch {
                selector: s, cases: c, ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *s = Some(selector);
                *c = normalized.clone();
            }
        } else if ctx.tree(tree).has_tag(Tag::SWITCH_EXPRESSION) {
            if let TreeKind::SwitchExpression {
                selector: s, cases: c, ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *s = Some(selector);
                *c = normalized.clone();
            }
        } else {
            panic!("AssertionError: unexpected switch tag");
        }
        if enum_switch {
            let result = self.visit_enum_switch(ctx, tree, selector, normalized);
            self.result = Some(result);
        } else if string_switch {
            let result = self.visit_string_switch(ctx, tree, selector, normalized);
            self.result = Some(result);
        } else if boxed_switch {
            let result = self.visit_boxed_primitive_switch(ctx, tree, selector, normalized);
            self.result = Some(result);
        } else {
            self.result = Some(tree);
        }
    }

    /// Lower a switch over an enum.
    fn visit_enum_switch(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        selector: TreeId,
        cases: List<TreeId>,
    ) -> TreeId {
        let selector_ty = ctx.tree(selector).ty.expect("selector type");
        let enum_sym = ctx.typ(selector_ty).tsym.expect("enum symbol");
        let pos = ctx.tree(tree).pos;
        let map_rc = self.map_for_enum(ctx, None, enum_sym);
        let (map_var, mut map_for_constants) = {
            let map = map_rc.borrow();
            (map.map_var, ())
        };
        let _ = &mut map_for_constants;
        self.make.borrow_mut().at(pos);
        let ordinal_name = self.names.ordinal.clone();
        let ordinal = self.lookup_method(ctx, None, ordinal_name, selector_ty, List::nil());
        let has_null_case = {
            let mut result = false;
            let mut current = cases.clone();
            while current.non_empty() {
                let case = *current.head();
                if let TreeKind::Case { labels, .. } = &ctx.tree(case).kind
                    && labels.non_empty()
                    && tree_info::is_null(ctx, *labels.head())
                {
                    result = true;
                    break;
                }
                current = current.tail();
            }
            result
        };
        let new_selector = if has_null_case {
            let current_method_sym = self.current_method_sym.expect("current method symbol");
            let name_text = format!("s{}{}", pos, self.target.synthetic_name_char());
            let dollar_name = self
                .names
                .from_string(&javac_strings::JavaString::from(name_text.as_str()));
            let flags = (crate::code::flags::FINAL | crate::code::flags::SYNTHETIC) as i64;
            let symbol = Symbol::new_var_symbol(flags, dollar_name, selector_ty, current_method_sym);
            let dollar_s = ctx.alloc_symbol(symbol);
            self.make.borrow_mut().at(pos);
            let var_def = self.var_def_for_symbol(ctx, dollar_s, Some(selector));
            let dollar_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
            let ordinal_sel = self.select_symbol(ctx, dollar_ident, ordinal);
            let ordinal_app = self.make.borrow_mut().app(ctx, ordinal_sel);
            let map_ident = {
                self.make.borrow_mut().qual_ident(ctx, map_var)
            };
            let indexed = self.make.borrow_mut().indexed(ctx, map_ident, ordinal_app);
            let dollar_ident2 = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
            let null = self.make_null(ctx);
            let ne = self.make_binary(ctx, Tag::NE, dollar_ident2, null);
            let minus_one = self.make_int_literal(ctx, -1);
            let indexed_ty = ctx.tree(indexed).ty;
            let conditional = self.make.borrow_mut().conditional(ctx, ne, indexed, minus_one);
            ctx.tree_mut(conditional).ty = indexed_ty;
            let let_expr = self.make.borrow_mut().let_expr(ctx, List::of(var_def), conditional);
            ctx.tree_mut(let_expr).ty = indexed_ty;
            let_expr
        } else {
            let ordinal_sel = self.select_symbol(ctx, selector, ordinal);
            let ordinal_app = self.make.borrow_mut().app(ctx, ordinal_sel);
            let map_ident = {
                self.make.borrow_mut().qual_ident(ctx, map_var)
            };
            self.make.borrow_mut().indexed(ctx, map_ident, ordinal_app)
        };
        let mut new_cases: List<TreeId> = List::nil();
        let mut current = cases;
        while current.non_empty() {
            let case = *current.head();
            let labels = match &ctx.tree(case).kind {
                TreeKind::Case { labels, .. } => labels.clone(),
                _ => List::nil(),
            };
            let first = *labels.head();
            let first_is_expression = case_label_is_expression(ctx, first);
            if first_is_expression {
                let pat = if tree_info::is_null(ctx, first) {
                    self.make_int_literal(ctx, -1)
                } else {
                    let label = tree_info::symbol(ctx, first).expect("enum constant symbol");
                    let mut map = map_rc.borrow_mut();
                    map.for_constant(ctx, self, label)
                };
                let stats = match &ctx.tree(case).kind {
                    TreeKind::Case { stats, .. } => stats.clone(),
                    _ => List::nil(),
                };
                let case_kind = javac_source::tree::case_tree::CaseKind::STATEMENT;
                let new_case = self
                    .make
                    .borrow_mut()
                    .case(ctx, case_kind, List::of(pat), stats, None);
                new_cases = new_cases.append(new_case);
            } else {
                new_cases = new_cases.append(case);
            }
            current = current.tail();
        }
        let enum_switch = if ctx.tree(tree).has_tag(Tag::SWITCH) {
            self.make.borrow_mut().switch(ctx, new_selector, new_cases)
        } else if ctx.tree(tree).has_tag(Tag::SWITCH_EXPRESSION) {
            let tree_ty = ctx.tree(tree).ty;
            let switch_expr = self
                .make
                .borrow_mut()
                .switch_expression(ctx, new_selector, new_cases);
            ctx.tree_mut(switch_expr).ty = tree_ty;
            switch_expr
        } else {
            panic!("AssertionError: unexpected switch tag");
        };
        self.patch_targets(ctx, enum_switch, tree, enum_switch);
        enum_switch
    }


    /// Lower a switch over String values.
    fn visit_string_switch(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        selector: TreeId,
        case_list: List<TreeId>,
    ) -> TreeId {
        let alternatives = case_list.length();
        let pos = ctx.tree(tree).pos;
        if alternatives == 0 {
            let checked = self.attr.borrow_mut().make_null_check(ctx, selector);
            return self.make.borrow_mut().exec(ctx, checked);
        }
        let string_type = self.syms.borrow().string_type;
        let int_type = self.syms.borrow().int_type;
        let current_method_sym = self.current_method_sym.expect("current method symbol");
        let mut stmts: Vec<TreeId> = Vec::new();
        let mut case_label_to_position: Vec<(String, i32)> = Vec::new();
        let mut hash_to_strings: Vec<(i32, Vec<String>)> = Vec::new();
        let mut case_position: i32 = 0;
        let mut null_case: Option<TreeId> = None;
        let mut null_case_label: i32 = -1;
        let mut current = case_list.clone();
        while current.non_empty() {
            let case = *current.head();
            let first = first_case_label(ctx, case);
            if let Some(first) = first
                && case_label_is_expression(ctx, first)
            {
                if tree_info::is_null(ctx, first) {
                    null_case = Some(case);
                    null_case_label = case_position;
                } else {
                    let first_ty = ctx.tree(first).ty.expect("case label type");
                    let label_expr = match ctx.typ(first_ty).const_value() {
                        Some(crate::util::constants::ConstantValue::String(text)) => text.clone(),
                        _ => panic!("AssertionError: string case label without constant value"),
                    };
                    case_label_to_position.push((label_expr.to_string_lossy(), case_position));
                    let hash_code = java_string_hash(&label_expr);
                    if let Some(entry) = hash_to_strings.iter_mut().find(|(hash, _)| *hash == hash_code)
                    {
                        entry.1.push(label_expr.to_string_lossy());
                    } else {
                        hash_to_strings.push((hash_code, vec![label_expr.to_string_lossy()]));
                    }
                }
            }
            case_position += 1;
            current = current.tail();
        }
        let s_name_text = format!("s{}{}", pos, self.target.synthetic_name_char());
        let s_name = self
            .names
            .from_string(&javac_strings::JavaString::from(s_name_text.as_str()));
        let flags = (crate::code::flags::FINAL | crate::code::flags::SYNTHETIC) as i64;
        let s_symbol = Symbol::new_var_symbol(flags, s_name, string_type, current_method_sym);
        let dollar_s = ctx.alloc_symbol(s_symbol);
        let dollar_s_def = self.var_def_for_symbol(ctx, dollar_s, Some(selector));
        stmts.push(dollar_s_def);
        let tmp_name_text = format!("tmp{}{}", pos, self.target.synthetic_name_char());
        let tmp_name = self
            .names
            .from_string(&javac_strings::JavaString::from(tmp_name_text.as_str()));
        let tmp_symbol = Symbol::new_var_symbol(
            crate::code::flags::SYNTHETIC as i64,
            tmp_name,
            int_type,
            current_method_sym,
        );
        let dollar_tmp = ctx.alloc_symbol(tmp_symbol);
        let minus_one = self.make_int_literal(ctx, -1);
        let tmp_def = self.var_def_for_symbol(ctx, dollar_tmp, Some(minus_one));
        stmts.push(tmp_def);
        let dollar_s_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
        let hash_name = self.names.hashCode.clone();
        let hash_call = self.make_call(ctx, dollar_s_ident, hash_name, List::nil());
        ctx.tree_mut(hash_call).ty = Some(int_type);
        let switch1 = self.make.borrow_mut().switch(ctx, hash_call, List::nil());
        let mut case_buffer: List<TreeId> = List::nil();
        let hash_entries = hash_to_strings.clone();
        for (hash_code, strings) in hash_entries {
            let mut elsepart: Option<TreeId> = None;
            for label in strings {
                let dollar_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
                let equals_name = self.names.equals.clone();
                let label_lit = self.make_string_literal(
                    ctx,
                    javac_strings::JavaString::from(label.as_str()),
                );
                let equals_call = self.make_call(ctx, dollar_ident, equals_name, List::of(label_lit));
                let position = case_label_to_position
                    .iter()
                    .find(|(label_expr, _)| label_expr == &label)
                    .map(|(_, position)| *position)
                    .expect("string case position");
                let tmp_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_tmp);
                let position_lit = self.make_int_literal(ctx, position);
                let assign = self.make.borrow_mut().assign(ctx, tmp_ident, position_lit);
                let tmp_ty = ctx.sym(dollar_tmp).ty.expect("temporary type");
                ctx.tree_mut(assign).ty = Some(tmp_ty);
                let exec = self.make.borrow_mut().exec(ctx, assign);
                elsepart = Some(self.make.borrow_mut().if_(ctx, equals_call, exec, elsepart));
            }
            let break_stmt = self.make.borrow_mut().break_(ctx, None);
            if let TreeKind::Break { target, .. } = &mut ctx.tree_mut(break_stmt).kind {
                *target = Some(switch1);
            }
            let mut stats: List<TreeId> = List::nil();
            if let Some(elsepart) = elsepart {
                stats = stats.append(elsepart);
            }
            stats = stats.append(break_stmt);
            let case_kind = javac_source::tree::case_tree::CaseKind::STATEMENT;
            let hash_lit = self.make_int_literal(ctx, hash_code);
            let new_case = self
                .make
                .borrow_mut()
                .case(ctx, case_kind, List::of(hash_lit), stats, None);
            case_buffer = case_buffer.append(new_case);
        }
        if let TreeKind::Switch { cases, .. } = &mut ctx.tree_mut(switch1).kind {
            *cases = case_buffer;
        }
        let dollar_s_ident2 = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
        let null = self.make_null(ctx);
        let ne = self.make_binary(ctx, Tag::NE, dollar_s_ident2, null);
        if let Some(null_case) = null_case {
            let tmp_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_tmp);
            let null_label = self.make_int_literal(ctx, null_case_label);
            let assign = self.make.borrow_mut().assign(ctx, tmp_ident, null_label);
            let tmp_ty = ctx.sym(dollar_tmp).ty.expect("temporary type");
            ctx.tree_mut(assign).ty = Some(tmp_ty);
            let exec = self.make.borrow_mut().exec(ctx, assign);
            let if_tree = self.make.borrow_mut().if_(ctx, ne, switch1, Some(exec));
            ctx.tree_mut(if_tree).ty = Some(int_type);
            let _ = null_case;
            stmts.push(if_tree);
        } else {
            stmts.push(switch1);
        }
        let mut lb: List<TreeId> = List::nil();
        let mut current = case_list;
        while current.non_empty() {
            let case = *current.head();
            let first = first_case_label(ctx, case);
            let first_expr = first.expect("case label");
            let is_default = !case_label_is_expression(ctx, first_expr);
            let case_expr: Option<TreeId> = if is_default {
                let default_label = self.make.borrow_mut().default_case_label(ctx);
                Some(default_label)
            } else if Some(case) == null_case {
                Some(self.make_int_literal(ctx, null_case_label))
            } else {
                let skipped = tree_info::skip_parens(ctx, first_expr);
                let first_ty = ctx.tree(skipped).ty.expect("case label type");
                let label_expr = match ctx.typ(first_ty).const_value() {
                    Some(crate::util::constants::ConstantValue::String(text)) => text.clone(),
                    _ => panic!("AssertionError: string case label without constant value"),
                };
                let position = case_label_to_position
                    .iter()
                    .find(|(label, _)| label == &label_expr.to_string_lossy())
                    .map(|(_, position)| *position)
                    .expect("string case position");
                Some(self.make_int_literal(ctx, position))
            };
            let stats = match &ctx.tree(case).kind {
                TreeKind::Case { stats, .. } => stats.clone(),
                _ => List::nil(),
            };
            let case_kind = javac_source::tree::case_tree::CaseKind::STATEMENT;
            let labels = match case_expr {
                Some(label) => List::of(label),
                None => List::nil(),
            };
            let new_case = self
                .make
                .borrow_mut()
                .case(ctx, case_kind, labels, stats, None);
            lb = lb.append(new_case);
            current = current.tail();
        }
        if ctx.tree(tree).has_tag(Tag::SWITCH) {
            let tmp_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_tmp);
            let switch2 = self.make.borrow_mut().switch(ctx, tmp_ident, lb);
            self.patch_targets(ctx, switch2, tree, switch2);
            stmts.push(switch2);
            let block = self.make.borrow_mut().block(ctx, 0, List::from_iterable(stmts));
            let end_pos = tree_info::end_pos(ctx, tree);
            if let TreeKind::Block { endpos, .. } = &mut ctx.tree_mut(block).kind {
                *endpos = end_pos;
            }
            block
        } else {
            let tmp_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_tmp);
            let switch2 = self.make.borrow_mut().switch_expression(ctx, tmp_ident, lb);
            self.patch_targets(ctx, switch2, tree, switch2);
            let tree_ty = ctx.tree(tree).ty;
            ctx.tree_mut(switch2).ty = tree_ty;
            let defs = List::from_iterable(stmts);
            let let_expr = self.make.borrow_mut().let_expr(ctx, defs, switch2);
            let tree_ty = ctx.tree(tree).ty;
            ctx.tree_mut(let_expr).ty = tree_ty;
            if let TreeKind::LetExpr { needs_cond, .. } = &mut ctx.tree_mut(let_expr).kind {
                *needs_cond = true;
            }
            let_expr
        }
    }

    /// Lower a switch over a boxed primitive.
    fn visit_boxed_primitive_switch(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        selector: TreeId,
        cases: List<TreeId>,
    ) -> TreeId {
        let int_type = self.syms.borrow().int_type;
        let mut has_null_case = false;
        let mut current = cases.clone();
        while current.non_empty() {
            let case = *current.head();
            if let TreeKind::Case { labels, .. } = &ctx.tree(case).kind
                && labels.non_empty()
                && tree_info::is_null(ctx, *labels.head())
            {
                has_null_case = true;
                break;
            }
            current = current.tail();
        }
        let new_selector = if has_null_case {
            let mut constants: Vec<i32> = Vec::new();
            let mut null_case: Option<TreeId> = None;
            let mut current = cases.clone();
            while current.non_empty() {
                let case = *current.head();
                let first = first_case_label(ctx, case);
                if let Some(first) = first {
                    if tree_info::is_null(ctx, first) {
                        null_case = Some(case);
                    } else if !ctx.tree(first).has_tag(Tag::DEFAULTCASELABEL) {
                        let first_ty = ctx.tree(first).ty.expect("case label type");
                        if let Some(crate::util::constants::ConstantValue::Integer(value)) =
                            ctx.typ(first_ty).const_value()
                        {
                            constants.push(*value);
                        }
                    }
                }
                current = current.tail();
            }
            let null_case = null_case.expect("null case");
            let mut null_value = if constants.is_empty() { 0 } else { constants[0] };
            while constants.contains(&null_value) {
                null_value += 1;
            }
            constants.push(null_value);
            let null_lit = self.make_int_literal(ctx, null_value);
            if let TreeKind::Case { labels, .. } = &mut ctx.tree_mut(null_case).kind {
                *labels = List::of(null_lit);
            }
            let mut replacement_value = null_value;
            while constants.contains(&replacement_value) {
                replacement_value += 1;
            }
            let pos = ctx.tree(tree).pos;
            let current_method_sym = self.current_method_sym.expect("current method symbol");
            let selector_ty = ctx.tree(selector).ty.expect("selector type");
            let name_text = format!("s{}{}", pos, self.target.synthetic_name_char());
            let dollar_name = self
                .names
                .from_string(&javac_strings::JavaString::from(name_text.as_str()));
            let flags = (crate::code::flags::FINAL | crate::code::flags::SYNTHETIC) as i64;
            let symbol = Symbol::new_var_symbol(flags, dollar_name, selector_ty, current_method_sym);
            let dollar_s = ctx.alloc_symbol(symbol);
            let var_def = self.var_def_for_symbol(ctx, dollar_s, Some(selector));
            let dollar_ident = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
            let unboxed = self.unbox(ctx, dollar_ident, int_type);
            let null_lit = self.make_int_literal(ctx, null_value);
            let ne = self.make_binary(ctx, Tag::NE, unboxed, null_lit);
            let dollar_ident2 = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
            let unboxed2 = self.unbox(ctx, dollar_ident2, int_type);
            let replacement_lit = self.make_int_literal(ctx, replacement_value);
            let null_value_replacement = self
                .make
                .borrow_mut()
                .conditional(ctx, ne, unboxed2, replacement_lit);
            ctx.tree_mut(null_value_replacement).ty = Some(int_type);
            let dollar_ident3 = self.make.borrow_mut().ident_symbol(ctx, dollar_s);
            let null = self.make_null(ctx);
            let ne2 = self.make_binary(ctx, Tag::NE, dollar_ident3, null);
            let null_value_lit = self.make_int_literal(ctx, null_value);
            let null_check = self
                .make
                .borrow_mut()
                .conditional(ctx, ne2, null_value_replacement, null_value_lit);
            ctx.tree_mut(null_check).ty = Some(int_type);
            let let_expr = self.make.borrow_mut().let_expr(ctx, List::of(var_def), null_check);
            ctx.tree_mut(let_expr).ty = Some(int_type);
            let_expr
        } else {
            self.unbox(ctx, selector, int_type)
        };
        if ctx.tree(tree).has_tag(Tag::SWITCH) {
            if let TreeKind::Switch { selector: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(new_selector);
            }
        } else if let TreeKind::SwitchExpression { selector: slot, .. } =
            &mut ctx.tree_mut(tree).kind
        {
            *slot = Some(new_selector);
        }
        tree
    }


    /// Translate the children of a tree with the default TreeTranslator
    /// navigation.
    ///
    /// This is the Rust form of the TreeTranslator default visitor methods: each
    /// node kind translates its child fields in declaration order and stores the
    /// results back. Nodes without children leave the tree unchanged.
    fn navigate(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => {
                let defs = self.translate_list(ctx, defs.clone());
                if let TreeKind::CompilationUnit { defs: d, .. } = &mut ctx.tree_mut(tree).kind {
                    *d = defs;
                }
            }
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => {
                let annotations = annotations.clone();
                let pid = *pid;
                let annotations = self.translate_list(ctx, annotations);
                let pid = self.translate(ctx, pid);
                if let TreeKind::PackageDecl {
                    annotations: a, pid: p, ..
                } = &mut ctx.tree_mut(tree).kind
                {
                    *a = annotations;
                    *p = pid;
                }
            }
            TreeKind::Import { qualid, .. } => {
                let qualid = *qualid;
                let qualid = self.translate(ctx, qualid);
                if let TreeKind::Import { qualid: q, .. } = &mut ctx.tree_mut(tree).kind {
                    *q = qualid;
                }
            }
            TreeKind::Skip {} => {}
            TreeKind::Block { stats, .. } => {
                let stats = stats.clone();
                let stats = self.translate_list(ctx, stats);
                if let TreeKind::Block { stats: s, .. } = &mut ctx.tree_mut(tree).kind {
                    *s = stats;
                }
            }
            TreeKind::DoWhileLoop { body, cond } => {
                let body = *body;
                let cond = *cond;
                let body = self.translate(ctx, body);
                let cond = self.translate(ctx, cond);
                if let TreeKind::DoWhileLoop {
                    body: b, cond: c,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *b = body;
                    *c = cond;
                }
            }
            TreeKind::WhileLoop { cond, body } => {
                let cond = *cond;
                let body = *body;
                let cond = self.translate(ctx, cond);
                let body = self.translate(ctx, body);
                if let TreeKind::WhileLoop {
                    cond: c, body: b,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *c = cond;
                    *b = body;
                }
            }
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => {
                let init = init.clone();
                let cond = *cond;
                let step = step.clone();
                let body = *body;
                let init = self.translate_list(ctx, init);
                let cond = self.translate(ctx, cond);
                let step = self.translate_list(ctx, step);
                let body = self.translate(ctx, body);
                if let TreeKind::ForLoop {
                    init: i,
                    cond: c,
                    step: s,
                    body: b,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *i = init;
                    *c = cond;
                    *s = step;
                    *b = body;
                }
            }
            TreeKind::LabeledStatement { body, .. } => {
                let body = *body;
                let body = self.translate(ctx, body);
                if let TreeKind::LabeledStatement { body: b, .. } = &mut ctx.tree_mut(tree).kind {
                    *b = body;
                }
            }
            TreeKind::Case {
                labels,
                stats,
                body,
                ..
            } => {
                let labels = labels.clone();
                let stats = stats.clone();
                let body = *body;
                let labels = self.translate_list(ctx, labels);
                let stats = self.translate_list(ctx, stats);
                let body = self.translate(ctx, body);
                if let TreeKind::Case {
                    labels: l,
                    stats: s,
                    body: b,
                    ..
                } = &mut ctx.tree_mut(tree).kind
                {
                    *l = labels;
                    *s = stats;
                    *b = body;
                }
            }
            TreeKind::Synchronized { lock, body } => {
                let lock = *lock;
                let body = *body;
                let lock = self.translate(ctx, lock);
                let body = self.translate(ctx, body);
                if let TreeKind::Synchronized {
                    lock: l, body: b,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *l = lock;
                    *b = body;
                }
            }
            TreeKind::Catch { param, body } => {
                let param = *param;
                let body = *body;
                let param = self.translate(ctx, param);
                let body = self.translate(ctx, body);
                if let TreeKind::Catch {
                    param: p, body: b,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *p = param;
                    *b = body;
                }
            }
            TreeKind::ExpressionStatement { expr } => {
                let inner = *expr;
                let inner = self.translate(ctx, inner);
                if let TreeKind::ExpressionStatement { expr } = &mut ctx.tree_mut(tree).kind {
                    *expr = inner;
                }
            }
            TreeKind::Return { expr } => {
                let inner = *expr;
                let inner = self.translate(ctx, inner);
                if let TreeKind::Return { expr } = &mut ctx.tree_mut(tree).kind {
                    *expr = inner;
                }
            }
            TreeKind::Throw { expr } => {
                let inner = *expr;
                let inner = self.translate(ctx, inner);
                if let TreeKind::Throw { expr } = &mut ctx.tree_mut(tree).kind {
                    *expr = inner;
                }
            }
            TreeKind::Lambda { params, body, .. } => {
                let params = params.clone();
                let body = *body;
                let params = self.translate_list(ctx, params);
                let body = self.translate(ctx, body);
                if let TreeKind::Lambda {
                    params: p, body: b, ..
                } = &mut ctx.tree_mut(tree).kind
                {
                    *p = params;
                    *b = body;
                }
            }
            TreeKind::InstanceOf { expr, pattern } => {
                let expr = *expr;
                let pattern = *pattern;
                let expr = self.translate(ctx, expr);
                let pattern = self.translate(ctx, pattern);
                if let TreeKind::InstanceOf {
                    expr: e, pattern: p,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *e = expr;
                    *p = pattern;
                }
            }
            TreeKind::MemberReference { expr, typeargs, .. } => {
                let expr = *expr;
                let typeargs = typeargs.clone();
                let expr = self.translate(ctx, expr);
                let typeargs = typeargs.map(|typeargs| self.translate_list(ctx, typeargs));
                if let TreeKind::MemberReference {
                    expr: e, typeargs: t, ..
                } = &mut ctx.tree_mut(tree).kind
                {
                    *e = expr;
                    *t = typeargs;
                }
            }
            TreeKind::ArrayTypeTree { elemtype } => {
                let inner = *elemtype;
                let inner = self.translate(ctx, inner);
                if let TreeKind::ArrayTypeTree { elemtype } = &mut ctx.tree_mut(tree).kind {
                    *elemtype = inner;
                }
            }
            TreeKind::TypeApply { clazz, arguments } => {
                let clazz = *clazz;
                let arguments = arguments.clone();
                let clazz = self.translate(ctx, clazz);
                let arguments = self.translate_list(ctx, arguments);
                if let TreeKind::TypeApply {
                    clazz: c, arguments: a,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *c = clazz;
                    *a = arguments;
                }
            }
            TreeKind::TypeUnion { alternatives } => {
                let alternatives = self.translate_list(ctx, alternatives.clone());
                if let TreeKind::TypeUnion { alternatives: a } = &mut ctx.tree_mut(tree).kind {
                    *a = alternatives;
                }
            }
            TreeKind::TypeIntersection { bounds } => {
                let bounds = bounds.clone();
                let bounds = self.translate_list(ctx, bounds);
                if let TreeKind::TypeIntersection { bounds: b } = &mut ctx.tree_mut(tree).kind {
                    *b = bounds;
                }
            }
            TreeKind::TypeParameter {
                name,
                bounds,
                annotations,
            } => {
                let name = name.clone();
                let bounds = bounds.clone();
                let annotations = annotations.clone();
                let bounds = self.translate_list(ctx, bounds);
                let annotations = self.translate_list(ctx, annotations);
                if let TreeKind::TypeParameter {
                    name: n,
                    bounds: b,
                    annotations: a,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *n = name;
                    *b = bounds;
                    *a = annotations;
                }
            }
            TreeKind::Annotation {
                annotation_type,
                args,
                ..
            } => {
                let annotation_type = *annotation_type;
                let args = args.clone();
                let annotation_type = self.translate(ctx, annotation_type);
                let args = self.translate_list(ctx, args);
                if let TreeKind::Annotation {
                    annotation_type: a,
                    args: b,
                    ..
                } = &mut ctx.tree_mut(tree).kind
                {
                    *a = annotation_type;
                    *b = args;
                }
            }
            TreeKind::Modifiers { annotations, .. } => {
                let annotations = annotations.clone();
                let annotations = self.translate_list(ctx, annotations);
                if let TreeKind::Modifiers { annotations: a, .. } = &mut ctx.tree_mut(tree).kind {
                    *a = annotations;
                }
            }
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => {
                let annotations = annotations.clone();
                let underlying_type = *underlying_type;
                let annotations = self.translate_list(ctx, annotations);
                let underlying_type = self.translate(ctx, underlying_type);
                if let TreeKind::AnnotatedType {
                    annotations: a,
                    underlying_type: u,
                } = &mut ctx.tree_mut(tree).kind
                {
                    *a = annotations;
                    *u = underlying_type;
                }
            }
            TreeKind::Erroneous { errs } => {
                let errs = errs.clone();
                let errs = self.translate_list(ctx, errs);
                if let TreeKind::Erroneous { errs: e } = &mut ctx.tree_mut(tree).kind {
                    *e = errs;
                }
            }
            TreeKind::VariableDecl {
                mods,
                nameexpr,
                vartype,
                init,
                ..
            } => {
                let mods = *mods;
                let nameexpr = *nameexpr;
                let vartype = *vartype;
                let init = *init;
                let mods = self.translate(ctx, mods);
                let nameexpr = self.translate(ctx, nameexpr);
                let vartype = self.translate(ctx, vartype);
                let init = self.translate(ctx, init);
                if let TreeKind::VariableDecl {
                    mods: m,
                    nameexpr: n,
                    vartype: v,
                    init: i,
                    ..
                } = &mut ctx.tree_mut(tree).kind
                {
                    *m = mods;
                    *n = nameexpr;
                    *v = vartype;
                    *i = init;
                }
            }
            TreeKind::ModuleDecl { directives, .. } => {
                let directives = directives.clone();
                let translated = directives.map(|directives| self.translate_list(ctx, directives));
                if let TreeKind::ModuleDecl {
                    directives: d, ..
                } = &mut ctx.tree_mut(tree).kind
                {
                    *d = translated;
                }
            }
            _ => {}
        }
        self.result = Some(tree);
    }


    /// Lower a variable declaration.
    fn lower_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mut mods, mut vartype, mut init, var_sym) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                vartype,
                init,
                sym,
                ..
            } => (*mods, *vartype, *init, *sym),
            _ => panic!("expected a variable declaration"),
        };
        let prev_method_sym = self.current_method_sym;
        mods = self.translate(ctx, mods);
        vartype = self.translate(ctx, vartype);
        if self.current_method_sym.is_none() {
            let current_class = self.current_class.expect("current class");
            let flags = (Self::mods_flags(ctx, mods) & crate::code::flags::STATIC as i64)
                | crate::code::flags::BLOCK as i64;
            let empty = self.names.empty.clone();
            let no_type = crate::code::symtab::no_type_id(ctx);
            let symbol = Symbol::new_method_symbol(flags, empty, no_type, current_class);
            let sym = ctx.alloc_symbol(symbol);
            self.current_method_sym = Some(sym);
        }
        let tree_ty = ctx.tree(tree).ty.expect("variable type");
        if init.is_some() {
            init = self.translate_with_type(ctx, init, tree_ty);
        }
        if let TreeKind::VariableDecl {
            mods: m,
            vartype: v,
            init: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *v = vartype;
            *i = init;
        }
        let _ = var_sym;
        self.result = Some(tree);
        self.current_method_sym = prev_method_sym;
    }

    /// Lower a block, providing a synthetic method symbol for initializers.
    fn lower_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_method_sym = self.current_method_sym;
        let flags = match &ctx.tree(tree).kind {
            TreeKind::Block { flags, .. } => *flags,
            _ => panic!("expected a block"),
        };
        if self.current_method_sym.is_none() {
            let current_class = self.current_class.expect("current class");
            let empty = self.names.empty.clone();
            let method_class = self.syms.borrow().method_class;
            let method_ty = ctx.alloc_type(Type::new(
                TypeTag::METHOD,
                Some(method_class),
                TypeKindData::MethodType {
                    argtypes: List::nil(),
                    restype: None,
                    thrown: List::nil(),
                    recvtype: None,
                },
            ));
            let symbol = Symbol::new_method_symbol(
                flags | crate::code::flags::BLOCK as i64,
                empty,
                method_ty,
                current_class,
            );
            let sym = ctx.alloc_symbol(symbol);
            self.current_method_sym = Some(sym);
        }
        self.navigate(ctx, tree);
        self.current_method_sym = prev_method_sym;
    }

    // __LOWER_REST__



























}


/// The translation table of one enum switch.
///
/// The mapping converts enum constants to the integers used by the lowered
/// switch, creates the synthetic switch-map field, and emits the static
/// initializer that fills the map through ordinal method calls; the upstream
/// EnumMapping inner class is modelled as this standalone value because the
/// Java inner instance only carries the map state.
struct EnumMapping {
    /// The diagnostic position of the switch expression being lowered.
    pos: Option<Rc<dyn DiagnosticPosition>>,
    /// The next integer to assign to a constant (zero is reserved for the
    /// default label).
    next: i32,
    /// The enum class this map belongs to.
    for_enum: SymbolId,
    /// The synthetic static field that holds the switch map.
    map_var: SymbolId,
    /// The constants mapped so far, in first-use order.
    values: Vec<(SymbolId, i32)>,
}

impl EnumMapping {
    /// Create the mapping and enter its synthetic field in the cache class.
    ///
    /// The field is a static final int array named after the enum flat name,
    /// entered in the synthetic outer cache class of the top-level class.
    fn new(
        lower: &mut Lower,
        ctx: &mut CompilerContext,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        for_enum: SymbolId,
    ) -> EnumMapping {
        let flat_name = ctx.sym(for_enum).flat_name();
        let external = crate::jvm::class_file::externalize_name(&flat_name);
        let external = String::from_utf8_lossy(&external).to_string();
        let synthetic_char = lower.target.synthetic_name_char();
        let replaced = external
            .replace('/', ".")
            .replace('.', &synthetic_char.to_string());
        let var_text = format!(
            "{}{}{}{}",
            synthetic_char, "SwitchMap", synthetic_char, replaced
        );
        let var_name = lower
            .names
            .from_string(&javac_strings::JavaString::from(var_text.as_str()));
        let outer = lower.outer_cache_class(ctx);
        let int_type = lower.syms.borrow().int_type;
        let array_class = lower.syms.borrow().array_class;
        let map_ty = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(array_class),
            TypeKindData::ArrayType {
                elemtype: Some(int_type),
                varargs: false,
            },
        ));
        let flags =
            (crate::code::flags::STATIC | crate::code::flags::SYNTHETIC | crate::code::flags::FINAL)
                as i64;
        let map_symbol = Symbol::new_var_symbol(flags, var_name, map_ty, outer);
        let map_var = ctx.alloc_symbol(map_symbol);
        let outer_members = ctx.sym(outer).members().expect("class members");
        lower.enter_synthetic(ctx, map_var, outer_members);
        EnumMapping {
            pos,
            next: 1,
            for_enum,
            map_var,
            values: Vec::new(),
        }
    }

    /// Return the integer assigned to an enum constant, assigning a fresh one
    /// on first use.
    ///
    /// The returned literal is a plain int literal with no attributed type,
    /// matching make.Literal(result) in the upstream forConstant method.
    fn for_constant(&mut self, ctx: &mut CompilerContext, lower: &mut Lower, v: SymbolId) -> TreeId {
        let result = match self.values.iter().find(|(sym, _)| *sym == v) {
            Some((_, value)) => *value,
            None => {
                let value = self.next;
                self.next += 1;
                self.values.push((v, value));
                value
            }
        };
        lower
            .make
            .borrow_mut()
            .literal(ctx, TypeTag::INT, Some(crate::tree::jc_tree::LiteralValue::Integer(result)))
    }


    /// Emit the static initializer that fills the switch map.
    ///
    /// The map field is declared as a static final int array sized by the enum
    /// values length, and one guarded assignment per mapped constant stores the
    /// assigned integer; the whole declaration is prepended to the owner class
    /// definitions, as the upstream translate method does.
    fn translate(&mut self, lower: &mut Lower, ctx: &mut CompilerContext) {
        let pos = self.pos.clone();
        if let Some(pos) = &pos {
            lower.make_at(pos);
        }
        let map_owner = ctx.sym(self.map_var).owner.expect("switch map owner");
        let owner = lower
            .class_def(ctx, map_owner)
            .expect("switch map owner definition");
        let for_enum_ty = ctx.sym(self.for_enum).ty.expect("enum type");
        let values_name = lower.names.values.clone();
        let values_method = lower.lookup_method(ctx, pos.clone(), values_name, for_enum_ty, List::nil());
        let values_qual = {
            lower.make.borrow_mut().qual_ident(ctx, values_method)
        };
        let values_app = lower.make.borrow_mut().app(ctx, values_qual);
        let length_var = lower.syms.borrow().length_var;
        let size = lower.select_symbol(ctx, values_app, length_var);
        let int_type = lower.syms.borrow().int_type;
        let array_class = lower.syms.borrow().array_class;
        let array_ty = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(array_class),
            TypeKindData::ArrayType {
                elemtype: Some(int_type),
                varargs: false,
            },
        ));
        let type_tree = {
            lower.make.borrow_mut().type_from_type(ctx, int_type)
        };
        let map_var_init = lower.make.borrow_mut().new_array(
            ctx,
            Some(type_tree),
            List::of(size),
            None,
        );
        ctx.tree_mut(map_var_init).ty = Some(array_ty);
        let ordinal_name = lower.names.ordinal.clone();
        let ordinal_method = lower.lookup_method(ctx, pos.clone(), ordinal_name, for_enum_ty, List::nil());
        let no_such_field = lower.syms.borrow().no_such_field_error_type;
        let ex_name = lower.names.ex.clone();
        let no_symbol = lower.syms.borrow().no_symbol;
        let ex_flags = crate::code::flags::PARAMETER;
        let ex_symbol = Symbol::new_var_symbol(ex_flags, ex_name, no_such_field, no_symbol);
        let ex_sym = ctx.alloc_symbol(ex_symbol);
        let ex_type_tree = {
            lower.make.borrow_mut().type_from_type(ctx, no_such_field)
        };
        let ex_mods = lower.make.borrow_mut().modifiers(ctx, 0);
        let ex_param = {
            let syms = lower.syms.borrow();
            let _ = &syms;
            lower.make.borrow_mut().var_def(ctx, ex_mods, ctx.sym(ex_sym).name.clone(), Some(ex_type_tree), None)
        };
        if let TreeKind::VariableDecl { sym, .. } = &mut ctx.tree_mut(ex_param).kind {
            *sym = Some(ex_sym);
        }
        let empty_block = {
            let stats = List::nil();
            lower.make.borrow_mut().block(ctx, 0, stats)
        };
        let catcher = lower.make.borrow_mut().catch(ctx, ex_param, empty_block);
        let mut stmts: Vec<TreeId> = Vec::new();
        let values = self.values.clone();
        for (enumerator, mapped_value) in values {
            let enumerator_qual = {
                lower.make.borrow_mut().qual_ident(ctx, enumerator)
            };
            let ordinal_sel = lower.select_symbol(ctx, enumerator_qual, ordinal_method);
            let ordinal_app = lower.make.borrow_mut().app(ctx, ordinal_sel);
            let map_var_ident = {
                lower.make.borrow_mut().qual_ident(ctx, self.map_var)
            };
            let indexed = lower.make.borrow_mut().indexed(ctx, map_var_ident, ordinal_app);
            let literal = lower
                .make
                .borrow_mut()
                .literal(ctx, TypeTag::INT, Some(crate::tree::jc_tree::LiteralValue::Integer(mapped_value)));
            let assign = lower.make.borrow_mut().assign(ctx, indexed, literal);
            ctx.tree_mut(assign).ty = Some(int_type);
            let exec = lower.make.borrow_mut().exec(ctx, assign);
            let block = lower.make.borrow_mut().block(ctx, 0, List::of(exec));
            let try_tree = lower.make.borrow_mut().try_block(ctx, block, List::of(catcher), None);
            stmts.push(try_tree);
        }
        let stats = List::from_iterable(stmts);
        let static_block = {
            let static_flag = crate::code::flags::STATIC as i64;
            lower.make.borrow_mut().block(ctx, static_flag, stats)
        };
        let map_var_flags = ctx.sym(self.map_var).flags_field;
        let map_var_name = ctx.sym(self.map_var).name.clone();
        let map_var_type_tree = {
            lower.make.borrow_mut().type_from_type(ctx, array_ty)
        };
        let map_var_mods = lower.make.borrow_mut().modifiers(ctx, map_var_flags);
        let map_var_def = lower.make.borrow_mut().var_def(
            ctx,
            map_var_mods,
            map_var_name,
            Some(map_var_type_tree),
            Some(map_var_init),
        );
        if let TreeKind::VariableDecl { sym, .. } = &mut ctx.tree_mut(map_var_def).kind {
            *sym = Some(self.map_var);
        }
        if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(owner).kind {
            let new_defs = defs.prepend(static_block).prepend(map_var_def);
            *defs = new_defs;
        }
    }

}

/// A navigator that records every class declaration in a class map.
///
/// It is the Rust equivalent of the upstream ClassMap inner class: only
/// visitClassDef does interesting work, every other node is traversed by the
/// default tree scanner navigation.
struct ClassMap {
    /// The class symbols and declarations collected so far.
    classdefs: HashMap<SymbolId, TreeId>,
}

impl ClassMap {
    /// Create an empty class map.
    fn new() -> ClassMap {
        ClassMap {
            classdefs: HashMap::new(),
        }
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(ClassMap);

impl crate::tree::tree_scanner::TreeScannerVisitor for ClassMap {
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, typarams, extending, implementing, permitting, defs, sym) =
            match &ctx.tree(tree).kind {
                TreeKind::ClassDecl {
                    mods,
                    typarams,
                    extending,
                    implementing,
                    permitting,
                    defs,
                    sym,
                    ..
                } => (
                    *mods,
                    typarams.clone(),
                    *extending,
                    implementing.clone(),
                    permitting.clone(),
                    defs.clone(),
                    *sym,
                ),
                _ => panic!("expected a class declaration"),
            };
        if let Some(sym) = sym {
            self.classdefs.insert(sym, tree);
        }
        self.scan(ctx, mods);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, extending);
        self.scan_list(ctx, &implementing);
        self.scan_list(ctx, &permitting);
        self.scan_list(ctx, &defs);
    }
}


/// Collects the free variables accessed from a local class.
///
/// The upstream BasicFreeVarCollector/FreeVarCollector pair is folded into this
/// single scanner: it walks the class definition, follows proxies and implicit
/// outer-this parameters, and records the owner variables that are used by the
/// class body. The cached maps are copied in so the scanner does not alias the
/// lowering pass while it walks.
struct FreeVarCollector {
    /// The owner of the local class.
    owner: SymbolId,
    /// The local class being scanned.
    clazz: SymbolId,
    /// The free variables collected so far, without duplicates.
    fvs: List<SymbolId>,
    /// The free-variable cache of the enclosing pass.
    freevar_cache: HashMap<SymbolId, List<SymbolId>>,
    /// The proxy table of the enclosing pass.
    proxies: HashMap<SymbolId, SymbolId>,
    /// The implicit outer-this stack of the enclosing pass.
    outer_this_stack: Vec<SymbolId>,
    /// The name table of the enclosing pass.
    names: Rc<Names>,
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(FreeVarCollector);

impl FreeVarCollector {
    /// Create a collector for a local class and copy the caches it consults.
    fn new(lower: &Lower, clazz: SymbolId, owner: SymbolId) -> FreeVarCollector {
        FreeVarCollector {
            owner,
            clazz,
            fvs: List::nil(),
            freevar_cache: lower.freevar_cache.clone(),
            proxies: lower.proxies.clone(),
            outer_this_stack: lower.outer_this_stack.clone(),
            names: Rc::clone(&lower.names),
        }
    }

    /// Add a free variable unless it is already recorded.
    fn add_free_var(&mut self, v: SymbolId) {
        for current in self.fvs.iter() {
            if current == v {
                return;
            }
        }
        self.fvs = self.fvs.prepend(v);
    }

    /// Add all free variables known for a class, if any.
    fn add_free_vars(&mut self, c: SymbolId) {
        if let Some(fvs) = self.freevar_cache.get(&c) {
            let mut current = fvs.clone();
            while current.non_empty() {
                self.add_free_var(*current.head());
                current = current.tail();
            }
        }
    }

    /// Visit a symbol referenced from the class body.
    fn visit_symbol(&mut self, ctx: &CompilerContext, original: SymbolId) {
        let mut sym = original;
        if !matches!(ctx.sym(sym).kind, Kind::VAR | Kind::MTH) {
            return;
        }
        if let Some(owner) = ctx.sym(sym).owner {
            if owner != self.owner {
                match self.proxies.get(&sym) {
                    Some(proxy) => sym = *proxy,
                    None => return,
                }
            }
            if ctx.sym(sym).owner == Some(self.owner) {
                if ctx.sym(sym).get_const_value().is_none() {
                    self.add_free_var(sym);
                }
            } else if let Some(head) = self.outer_this_stack.first().copied()
                && head != original
            {
                self.visit_symbol(ctx, head);
            }
        }
    }
}

impl crate::tree::tree_scanner::TreeScannerVisitor for FreeVarCollector {
    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::Ident { sym, .. } => *sym,
            _ => panic!("expected an identifier"),
        };
        if let Some(sym) = sym {
            self.visit_symbol(ctx, sym);
        }
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, clazz, args, def, constructor) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                clazz,
                args,
                def,
                constructor,
                ..
            } => (*encl, *clazz, args.clone(), *def, *constructor),
            _ => panic!("expected a new class expression"),
        };
        let constructor = constructor.expect("attributed constructor");
        let c = ctx.sym(constructor).owner.expect("constructor owner");
        self.add_free_vars(c);
        if encl.is_none()
            && ctx.sym(c).has_outer_instance(ctx)
            && let Some(head) = self.outer_this_stack.first().copied()
        {
            self.visit_symbol(ctx, head);
        }
        self.scan(ctx, clazz);
        self.scan_list(ctx, &args);
        self.scan(ctx, def);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (typeargs, meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation {
                typeargs,
                meth,
                args,
                ..
            } => (typeargs.clone(), *meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        if let Some(meth) = meth
            && crate::tree::tree_info::name(ctx, meth) == Some(self.super_name(ctx))
            && let Some(constructor) = crate::tree::tree_info::symbol(ctx, meth)
            && let Some(c) = ctx.sym(constructor).owner
        {
            self.add_free_vars(c);
        }
        if let Some(meth) = meth
            && crate::tree::tree_info::name(ctx, meth) == Some(self.super_name(ctx))
            && let Some(constructor) = crate::tree::tree_info::symbol(ctx, meth)
            && let Some(c) = ctx.sym(constructor).owner
            && ctx.sym(c).has_outer_instance(ctx)
            && !matches!(ctx.tree(meth).kind, TreeKind::FieldAccess { .. })
            && let Some(head) = self.outer_this_stack.first().copied()
        {
            self.visit_symbol(ctx, head);
        }
        self.scan_list(ctx, &typeargs);
        self.scan(ctx, meth);
        self.scan_list(ctx, &args);
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        self.scan(ctx, value);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selected, name) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, name, .. } => (*selected, name.clone()),
            _ => panic!("expected a field access"),
        };
        if let Some(selected) = selected {
            let selected_ty = ctx.tree(selected).ty;
            let is_this_or_super = name == self.this_name(ctx) || name == self.super_name(ctx);
            let selected_sym = selected_ty.and_then(|ty| ctx.typ(ty).tsym);
            if is_this_or_super
                && selected_sym != Some(self.clazz)
                && let Some(head) = self.outer_this_stack.first().copied()
            {
                self.visit_symbol(ctx, head);
            }
            self.scan(ctx, Some(selected));
        }
    }
}

impl FreeVarCollector {
    /// Return the name of the synthetic outer-this parameter.
    fn this_name(&self, _ctx: &CompilerContext) -> Name {
        self.names._this.clone()
    }

    /// Return the name of the super pseudo-identifier.
    fn super_name(&self, _ctx: &CompilerContext) -> Name {
        self.names._super.clone()
    }
}


/// Collects (tree, symbol) pairs that may conflict with user declarations.
///
/// The scanner walks class bodies remembering the current class; it reports the
/// members whose name and erased type collide with another member of the same
/// kind when exactly one of them is synthetic. Diagnostics are emitted by the
/// lowering pass after the scan so the scanner stays free of logging services.
struct ConflictsChecker {
    /// The class currently being scanned.
    current_class: Option<SymbolId>,
    /// The type relations service.
    types: Rc<Types>,
    /// The symbol table.
    syms: Rc<RefCell<Symtab>>,
    /// The collected conflicts, as (tree, reported symbol) pairs.
    conflicts: Vec<(TreeId, SymbolId)>,
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(ConflictsChecker);

impl crate::tree::tree_scanner::TreeScannerVisitor for ConflictsChecker {
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let previous = self.current_class;
        let (mods, typarams, extending, implementing, permitting, defs, sym) =
            match &ctx.tree(tree).kind {
                TreeKind::ClassDecl {
                    mods,
                    typarams,
                    extending,
                    implementing,
                    permitting,
                    defs,
                    sym,
                    ..
                } => (
                    *mods,
                    typarams.clone(),
                    *extending,
                    implementing.clone(),
                    permitting.clone(),
                    defs.clone(),
                    *sym,
                ),
                _ => panic!("expected a class declaration"),
            };
        self.current_class = sym;
        self.scan(ctx, mods);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, extending);
        self.scan_list(ctx, &implementing);
        self.scan_list(ctx, &permitting);
        self.scan_list(ctx, &defs);
        self.current_class = previous;
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, default_value, body, sym) =
            match &ctx.tree(tree).kind {
                TreeKind::MethodDecl {
                    mods,
                    restype,
                    typarams,
                    recvparam,
                    params,
                    thrown,
                    default_value,
                    body,
                    sym,
                    ..
                } => (
                    *mods,
                    *restype,
                    typarams.clone(),
                    *recvparam,
                    params.clone(),
                    thrown.clone(),
                    *default_value,
                    *body,
                    *sym,
                ),
                _ => panic!("expected a method declaration"),
            };
        if let Some(sym) = sym {
            self.check_symbol(ctx, tree, sym);
        }
        self.scan(ctx, mods);
        self.scan(ctx, restype);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, recvparam);
        self.scan_list(ctx, &params);
        self.scan_list(ctx, &thrown);
        self.scan(ctx, default_value);
        self.scan(ctx, body);
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, nameexpr, vartype, init, sym) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                nameexpr,
                vartype,
                init,
                sym,
                ..
            } => (*mods, *nameexpr, *vartype, *init, *sym),
            _ => panic!("expected a variable declaration"),
        };
        if let Some(sym) = sym
            && let Some(owner) = ctx.sym(sym).owner
            && ctx.sym(owner).kind == Kind::TYP
        {
            self.check_symbol(ctx, tree, sym);
        }
        self.scan(ctx, mods);
        self.scan(ctx, nameexpr);
        self.scan(ctx, vartype);
        self.scan(ctx, init);
    }
}

impl ConflictsChecker {
    /// Test the symbol against the members of the current class hierarchy.
    fn check_symbol(&mut self, ctx: &mut CompilerContext, tree: TreeId, sym: SymbolId) {
        let current_class = match self.current_class {
            Some(current_class) => current_class,
            None => return,
        };
        let no_type = crate::code::symtab::no_type_id(ctx);
        let mut ct = ctx.sym(current_class).ty.expect("class type");
        while ct != no_type {
            let tsym = match ctx.typ(ct).tsym {
                Some(tsym) => tsym,
                None => break,
            };
            let members = match ctx.sym(tsym).members() {
                Some(members) => members,
                None => break,
            };
            let name = ctx.sym(sym).name.clone();
            let scope_snapshot = ctx.scope(members).clone();
            let candidates = scope_snapshot.get_symbols_by_name(
                members,
                &name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                ctx,
            );
            for sym2 in candidates {
                if self.is_conflict(ctx, sym, sym2) {
                    let synthetic2 =
                        (ctx.sym(sym2).flags_field & crate::code::flags::SYNTHETIC as i64) != 0;
                    let reported = if !synthetic2 { sym2 } else { sym };
                    self.conflicts.push((tree, reported));
                    return;
                }
            }
            let syms = self.syms.borrow();
            ct = self.types.supertype(ctx, &syms, ct);
        }
    }

    /// Test whether two symbols conflict under the synthetic-name rules.
    fn is_conflict(&mut self, ctx: &mut CompilerContext, sym: SymbolId, sym2: SymbolId) -> bool {
        if ctx.sym(sym).kind != ctx.sym(sym2).kind || sym == sym2 {
            return false;
        }
        let (sym_ty, sym2_ty) = match (ctx.sym(sym).ty, ctx.sym(sym2).ty) {
            (Some(a), Some(b)) => (a, b),
            _ => return false,
        };
        let synthetic1 =
            (ctx.sym(sym).flags_field & crate::code::flags::SYNTHETIC as i64) != 0;
        let synthetic2 =
            (ctx.sym(sym2).flags_field & crate::code::flags::SYNTHETIC as i64) != 0;
        if synthetic1 == synthetic2 {
            return false;
        }
        let bridge1 = (ctx.sym(sym).flags_field & crate::code::flags::BRIDGE) != 0;
        let bridge2 = (ctx.sym(sym2).flags_field & crate::code::flags::BRIDGE) != 0;
        if bridge1 || bridge2 {
            return false;
        }
        let syms = self.syms.borrow_mut();
        let e1 = self.types.erasure(ctx, &syms, sym_ty);
        let e2 = self.types.erasure(ctx, &syms, sym2_ty);
        self.types.is_same_type(ctx, &syms, e1, e2)
    }
}

/// Visitor implementation for the lowering pass.
///
/// Every node kind is either lowered by a dedicated translation method or
/// traversed by the default subtree navigation, matching the upstream
/// TreeTranslator default behaviour.
impl crate::tree::jc_tree::TreeVisitor for Lower {
    fn visit_tree(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_package_def(ctx, tree);
    }

    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_class_def(ctx, tree);
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_method_def(ctx, tree);
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_var_def(ctx, tree);
    }

    fn visit_skip(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_block(ctx, tree);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_foreach(ctx, tree);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_switch(ctx, tree);
    }

    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_switch_expression(ctx, tree);
    }

    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_try(ctx, tree);
    }

    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_conditional(ctx, tree);
    }

    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_if(ctx, tree);
    }

    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_break(ctx, tree);
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_yield(ctx, tree);
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_assert(ctx, tree);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_apply(ctx, tree);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_new_class(ctx, tree);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_new_array(ctx, tree);
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_parens(ctx, tree);
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_assign(ctx, tree);
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_assignop(ctx, tree);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_unary(ctx, tree);
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_binary(ctx, tree);
    }

    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_type_cast(ctx, tree);
    }

    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_default_case_label(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_indexed(ctx, tree);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_select(ctx, tree);
    }

    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_ident(ctx, tree);
    }

    fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_type_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_type_bound_kind(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_annotation(ctx, tree);
    }

    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_erroneous(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_module_def(ctx, tree);
    }

    fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.navigate(ctx, tree);
    }

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.lower_let_expr(ctx, tree);
    }

}


/// Signature generator that assembles a JVM signature into a string buffer.
///
/// The Java subclass only overrides the append primitives; the signature
/// grammar itself is inherited from the Types.SignatureGenerator trait.
struct LowerSignatureGenerator {
    /// The output buffer.
    sb: String,
    /// The type relations service.
    types: Rc<Types>,
}

impl LowerSignatureGenerator {
    /// Create an empty signature generator.
    fn new(types: Rc<Types>) -> LowerSignatureGenerator {
        LowerSignatureGenerator {
            sb: String::new(),
            types,
        }
    }
}

impl std::fmt::Display for LowerSignatureGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.sb)
    }
}

impl LowerSignatureGenerator {
    /// Return the assembled signature text.
    fn to_signature_string(&self) -> String {
        self.sb.clone()
    }
}

impl crate::code::types::SignatureGenerator for LowerSignatureGenerator {
    fn append_char(&mut self, ch: char) {
        self.sb.push(ch);
    }

    fn append_bytes(&mut self, ba: &[u8]) {
        self.sb.push_str(&String::from_utf8_lossy(ba));
    }

    fn append_name(&mut self, name: &Name) {
        self.sb
            .push_str(&name.to_java_string().to_string_lossy());
    }

    fn types(&self) -> &Types {
        &self.types
    }
}

/// Scans an assignment right-hand side for a dependency on the assigned
/// variable, which requires the compound assignment to be rewritten through a
/// temporary.
struct AssignopDependencyScanner {
    /// The assigned variable symbol.
    sym: Option<SymbolId>,
    /// Whether a dependency was found.
    dependency_found: bool,
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(AssignopDependencyScanner);

impl crate::tree::tree_scanner::TreeScannerVisitor for AssignopDependencyScanner {
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree
            && self.sym.is_some()
        {
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let lhs = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs: Some(lhs), .. } => *lhs,
            _ => panic!("expected a compound assignment"),
        };
        if tree_info::symbol(ctx, lhs) == self.sym {
            self.dependency_found = true;
            return;
        }
        self.scan_assignop_children(ctx, tree);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg: Some(arg), .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        if tree_info::symbol(ctx, arg) == self.sym {
            self.dependency_found = true;
            return;
        }
        self.scan_unary_children(ctx, tree);
    }
}

impl AssignopDependencyScanner {
    /// Navigate the children of a compound assignment.
    fn scan_assignop_children(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp {
                lhs: Some(lhs),
                rhs: Some(rhs),
                ..
            } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        self.scan(ctx, Some(lhs));
        self.scan(ctx, Some(rhs));
    }

    /// Navigate the children of a unary expression.
    fn scan_unary_children(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => None,
        };
        self.scan(ctx, arg);
    }
}


/// Builds an expression from an abstracted temporary.
///
/// The upstream TreeBuilder interface is modelled as a trait implemented by the
/// small builder structs used by the compound-assignment and boxed-update
/// rewrites.
trait TreeBuilder {
    /// Build the resulting expression around the given argument.
    fn build(&mut self, lower: &mut Lower, ctx: &mut CompilerContext, arg: TreeId) -> TreeId;
}

/// Builds a field selection around an abstracted selector.
struct SelectBuilder<'a> {
    /// The outer builder receiving the selection.
    inner: &'a mut dyn TreeBuilder,
    /// The selected field symbol.
    field_sym: SymbolId,
}

impl TreeBuilder for SelectBuilder<'_> {
    fn build(&mut self, lower: &mut Lower, ctx: &mut CompilerContext, arg: TreeId) -> TreeId {
        let select = lower.select_symbol(ctx, arg, self.field_sym);
        self.inner.build(lower, ctx, select)
    }
}

/// Builds an array access around an abstracted array and index.
struct IndexedBuilder<'a> {
    /// The outer builder receiving the array access.
    inner: &'a mut dyn TreeBuilder,
    /// The index expression.
    index: TreeId,
    /// The int type of the index.
    int_type: TypeId,
    /// The original array access type.
    lval_ty: Option<TypeId>,
}

impl TreeBuilder for IndexedBuilder<'_> {
    fn build(&mut self, lower: &mut Lower, ctx: &mut CompilerContext, arg: TreeId) -> TreeId {
        let index = self.index;
        let int_type = self.int_type;
        let lval_ty = self.lval_ty;
        let mut access_builder = ArrayAccessBuilder {
            inner: self.inner,
            indexed: arg,
            lval_ty,
        };
        let _ = int_type;
        lower.abstract_rval(ctx, index, int_type, &mut access_builder)
    }
}

/// Builds the final array access from an abstracted index.
struct ArrayAccessBuilder<'a> {
    /// The outer builder receiving the array access.
    inner: &'a mut dyn TreeBuilder,
    /// The abstracted array expression.
    indexed: TreeId,
    /// The original array access type.
    lval_ty: Option<TypeId>,
}

impl TreeBuilder for ArrayAccessBuilder<'_> {
    fn build(&mut self, lower: &mut Lower, ctx: &mut CompilerContext, arg: TreeId) -> TreeId {
        let access = lower.make.borrow_mut().indexed(ctx, self.indexed, arg);
        if let Some(ty) = self.lval_ty {
            ctx.tree_mut(access).ty = Some(ty);
        }
        self.inner.build(lower, ctx, access)
    }
}



/// Builds the rewritten boxed compound assignment.
struct BoxedAssignopBuilder {
    /// The compound assignment tag.
    opcode: Tag,
    /// The assignment type.
    tree_ty: TypeId,
    /// The right hand side expression.
    rhs: TreeId,
    /// The right hand side type.
    rhs_ty: TypeId,
    /// Whether a boxing conversion is required.
    boxing_req: bool,
}

impl TreeBuilder for BoxedAssignopBuilder {
    fn build(&mut self, lower: &mut Lower, ctx: &mut CompilerContext, lhs: TreeId) -> TreeId {
        let new_tag = self.opcode.no_assign_op();
        let operators = lower.operators();
        let operator = {
            let mut syms = lower.syms.borrow_mut();
            operators.resolve_binary(ctx, &mut syms, &lower.types, ctx.tree(lhs).pos, new_tag, self.tree_ty, self.rhs_ty)
        };
        let mut expr = lower.clone_tree(ctx, lhs);
        let expr_ty = ctx.tree(expr).ty;
        if expr_ty != Some(self.tree_ty) {
            let cast_ty = lower.type_tree(ctx, self.tree_ty);
            expr = lower.make.borrow_mut().type_cast(ctx, cast_ty, expr);
        }
        let op_result = lower.make.borrow_mut().binary(ctx, new_tag, expr, self.rhs);
        if let TreeKind::Binary { operator: slot, .. } = &mut ctx.tree_mut(op_result).kind {
            *slot = Some(operator);
        }
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let return_ty = lower.types.get_return_type_public(ctx, operator_ty);
        ctx.tree_mut(op_result).ty = Some(return_ty);
        let new_rhs = if self.boxing_req {
            let syms = lower.syms.borrow();
            let unboxed = lower.types.unboxed_type_ro(ctx, &syms, self.tree_ty);
            drop(syms);
            let unboxed_tree = lower.type_tree(ctx, unboxed);
            lower.make.borrow_mut().type_cast(ctx, unboxed_tree, op_result)
        } else {
            op_result
        };
        let assign = lower.make.borrow_mut().assign(ctx, lhs, new_rhs);
        ctx.tree_mut(assign).ty = Some(self.tree_ty);
        assign
    }
}

/// Builds the boxed post-increment/decrement outer temporary.
struct BoxedPostopOuterBuilder {
    /// The operand type.
    arg_ty: TypeId,
    /// Whether the operand is a type cast.
    cast: bool,
    /// The compound assignment tag.
    opcode: Tag,
    /// The update type.
    tree_ty: TypeId,
}

impl TreeBuilder for BoxedPostopOuterBuilder {
    fn build(&mut self, lower: &mut Lower, ctx: &mut CompilerContext, tmp1: TreeId) -> TreeId {
        let mut inner = BoxedPostopInnerBuilder {
            tmp1,
            cast: self.cast,
            arg_ty: self.arg_ty,
            opcode: self.opcode,
        };
        let _ = self.tree_ty;
        lower.abstract_rval(ctx, tmp1, self.arg_ty, &mut inner)
    }
}

/// Builds the boxed post-increment/decrement update expression.
struct BoxedPostopInnerBuilder {
    /// The outer temporary reference.
    tmp1: TreeId,
    /// Whether the operand is a type cast.
    cast: bool,
    /// The operand type.
    arg_ty: TypeId,
    /// The compound assignment tag.
    opcode: Tag,
}

impl TreeBuilder for BoxedPostopInnerBuilder {
    fn build(&mut self, lower: &mut Lower, ctx: &mut CompilerContext, tmp2: TreeId) -> TreeId {
        let mut lhs = lower.clone_tree(ctx, self.tmp1);
        if self.cast {
            let arg_tree = lower.type_tree(ctx, self.arg_ty);
            lhs = lower.make.borrow_mut().type_cast(ctx, arg_tree, lhs);
        }
        let one = lower
            .make
            .borrow_mut()
            .literal(ctx, TypeTag::INT, Some(crate::tree::jc_tree::LiteralValue::Integer(1)));
        let update = lower.make_assignop(ctx, self.opcode, lhs, one);
        lower.make_comma(ctx, update, tmp2)
    }
}


/// Patches break/continue/yield targets from a loop source to its replacement.
struct Patcher {
    /// The original loop tree.
    src: TreeId,
    /// The replacement loop tree.
    dest: TreeId,
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(Patcher);

impl crate::tree::tree_scanner::TreeScannerVisitor for Patcher {
    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Break { target, .. } = &mut ctx.tree_mut(tree).kind
            && *target == Some(self.src)
        {
            *target = Some(self.dest);
        }
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Yield { target, value } = &mut ctx.tree_mut(tree).kind {
            if *target == Some(self.src) {
                *target = Some(self.dest);
            }
            let value = *value;
            self.scan(ctx, value);
        }
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Continue { target, .. } = &mut ctx.tree_mut(tree).kind
            && *target == Some(self.src)
        {
            *target = Some(self.dest);
        }
    }

    fn visit_class_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}


/// Return whether a case label is an expression label (rather than a pattern or
/// a default label).
fn case_label_is_expression(ctx: &CompilerContext, label: TreeId) -> bool {
    !matches!(
        ctx.tree(label).get_tag(),
        Tag::BINDINGPATTERN
            | Tag::DEFAULTCASELABEL
            | Tag::GUARDPATTERN
            | Tag::PARENTHESIZEDPATTERN
    )
}

/// Return the Java String.hashCode value of a string.
fn java_string_hash(java_string: &javac_strings::JavaString) -> i32 {
    let mut hash: i32 = 0;
    for unit in java_string.as_units() {
        hash = hash.wrapping_mul(31).wrapping_add(*unit as i32);
    }
    hash
}


/// Return the first label of a case, if any.
fn first_case_label(ctx: &CompilerContext, case: TreeId) -> Option<TreeId> {
    match &ctx.tree(case).kind {
        TreeKind::Case { labels, .. } => {
            if labels.non_empty() {
                Some(*labels.head())
            } else {
                None
            }
        }
        _ => None,
    }
}
