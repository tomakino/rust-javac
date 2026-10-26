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

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::type_::{Type, TypeKindData};
use crate::code::types::Types;
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
use crate::tree::jc_tree::{Tag, TreeKind, TreeVisitor};
use crate::tree::tree_info;
use crate::tree::tree_scanner::TreeScannerVisitor;
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::util::jc_diagnostic::DiagnosticPosition;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;

/// Java: public class Lower extends TreeTranslator.
///
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
    pub end_pos_table: Option<Rc<RefCell<dyn EndPosTable>>>,
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
    pub enum_switch_map: HashMap<SymbolId, Rc<RefCell<EnumMapping>>>,
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
    /// The current tree translation result.
    pub result: Option<TreeId>,
    /// The prototype type of the current translation (when any).
    pub pt: Option<TypeId>,
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
            result: None,
            pt: None,
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
            ConstantValue::Byte(v) => LiteralValue::Integer(*v as i32),
            ConstantValue::Short(v) => LiteralValue::Integer(*v as i32),
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
            let syms = self.syms.borrow();
            self.make.borrow_mut().qual_ident(ctx, &syms, tsym)
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
            let syms = self.syms.borrow();
            self.make.borrow_mut().qual_ident(ctx, &syms, value_of_sym)
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
        if add_to_defs {
            if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(odef).kind {
                let new_defs = defs.prepend(cdef);
                *defs = new_defs;
            }
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
        let symbols = ctx.scope(members).get_symbols(
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
        loop {
            let owner_of_owner = match ctx.sym(current_owner).owner {
                Some(owner) => owner,
                None => break,
            };
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
            let syms = lower.syms.borrow();
            lower.make.borrow_mut().qual_ident(ctx, &syms, values_method)
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
            let syms = lower.syms.borrow();
            lower.make.borrow_mut().type_from_type(ctx, &syms, int_type)
        };
        let map_var_ident = {
            let syms = lower.syms.borrow();
            lower.make.borrow_mut().qual_ident(ctx, &syms, self.map_var)
        };
        let mut map_var_init = lower.make.borrow_mut().new_array(
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
        let ex_flags = crate::code::flags::PARAMETER as i64;
        let ex_symbol = Symbol::new_var_symbol(ex_flags, ex_name, no_such_field, no_symbol);
        let ex_sym = ctx.alloc_symbol(ex_symbol);
        let ex_type_tree = {
            let syms = lower.syms.borrow();
            lower.make.borrow_mut().type_from_type(ctx, &syms, no_such_field)
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
                let syms = lower.syms.borrow();
                lower.make.borrow_mut().qual_ident(ctx, &syms, enumerator)
            };
            let ordinal_sel = lower.select_symbol(ctx, enumerator_qual, ordinal_method);
            let ordinal_app = lower.make.borrow_mut().app(ctx, ordinal_sel);
            let map_var_ident = {
                let syms = lower.syms.borrow();
                lower.make.borrow_mut().qual_ident(ctx, &syms, self.map_var)
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
            let syms = lower.syms.borrow();
            lower.make.borrow_mut().type_from_type(ctx, &syms, array_ty)
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
            } else if let Some(head) = self.outer_this_stack.first().copied() {
                if head != original {
                    self.visit_symbol(ctx, head);
                }
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
