// Origin: com/sun/tools/javac/comp/TransTypes.java @ 982d53c (jdk-17.0.19-ga)

//! This pass translates Generic Java to conventional Java.
//!
//! ADR 0001: the upstream Context services are injected explicitly.
//!
//! PORTING_RULES: the Java base class TreeTranslator is expressed here by the
//! same visitor-framework methods (translate/translateList/...) ported from
//! TreeTranslator.java:139-355 (see the batch report); this mirrors the
//! upstream inheritance relationship without changing tree_translator.rs.
//!
//! Upstream: TransTypes.java:60-984.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;

use crate::code::types::Types;
use crate::comp::annotate::Annotate;
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::compile_states::CompileState;
use crate::comp::env::Env;
use crate::comp::resolve::Resolve;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{Tag, TreeKind, TreeVisitor};
use crate::tree::tree_info;
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::util::jc_diagnostic::Factory;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::names::Names;

/// Java: public class TransTypes extends TreeTranslator (TransTypes.java).
pub struct TransTypes {
    /// Java: private Names names
    pub names: Rc<Names>,
    /// Java: private Log log
    pub log: Rc<RefCell<Log>>,
    /// Java: private Symtab syms
    pub syms: Rc<RefCell<Symtab>>,
    /// Java: private TreeMaker make
    pub make: Rc<RefCell<TreeMaker>>,
    /// Java: private Enter enter
    pub enter: Rc<RefCell<crate::comp::enter::Enter>>,
    /// Java: private Types types
    pub types: Rc<Types>,
    /// Java: private Annotate annotate
    pub annotate: Rc<Annotate>,
    /// Java: private Attr attr
    pub attr: Rc<RefCell<Attr>>,
    /// Java: private final Resolve resolve
    pub resolve: Rc<RefCell<Resolve>>,
    /// Java: private final CompileStates compileStates
    pub compile_states: Rc<RefCell<crate::comp::compile_states::CompileStates>>,
    /// Adapter: the diagnostic factory (needed by Types.findDescriptorType adapter).
    pub diags: Rc<Factory>,
    /// Adapter: the check service (needed by Types.findDescriptorType adapter).
    pub chk: Rc<RefCell<crate::comp::check::Check>>,
    /// Java: private final boolean allowGraphInference
    pub allow_graph_inference: bool,
    /// Java: private final boolean allowInterfaceBridges
    pub allow_interface_bridges: bool,
    /// Java: protected JCTree result (TreeTranslator)
    pub result: Option<TreeId>,
    /// Java: private Env<AttrContext> env
    pub env: Option<Rc<Env<AttrContext>>>,
    /// Java: private Type pt (visitor argument: prototype type)
    pub pt: Option<TypeId>,
    /// Java: Type returnType = null
    pub return_type: Option<TypeId>,
}

impl TransTypes {
    /// Java: protected TransTypes(Context context) (TransTypes.java)
    /// — the explicit-service adapter (ADR 0001).
    ///
    /// # Arguments
    ///   - the ported service handles;
    ///   - allow_graph_inference/allow_interface_bridges: the source-feature switches.
    ///
    /// # Returns
    ///   - the translator.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: TransTypes.java:93-111.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        syms: Rc<RefCell<Symtab>>,
        make: Rc<RefCell<TreeMaker>>,
        enter: Rc<RefCell<crate::comp::enter::Enter>>,
        types: Rc<Types>,
        annotate: Rc<Annotate>,
        attr: Rc<RefCell<Attr>>,
        resolve: Rc<RefCell<Resolve>>,
        compile_states: Rc<RefCell<crate::comp::compile_states::CompileStates>>,
        diags: Rc<Factory>,
        chk: Rc<RefCell<crate::comp::check::Check>>,
        allow_graph_inference: bool,
        allow_interface_bridges: bool,
    ) -> TransTypes {
        TransTypes {
            names,
            log,
            syms,
            make,
            enter,
            types,
            annotate,
            attr,
            resolve,
            compile_states,
            diags,
            chk,
            allow_graph_inference,
            allow_interface_bridges,
            result: None,
            env: None,
            pt: None,
            return_type: None,
        }
    }

    // ------------------------------------------------------------------
    // Java: TreeTranslator.translate framework (TreeTranslator.java)
    // ------------------------------------------------------------------

    /// Java: public <T extends JCTree> T translate(T tree)
    /// (TreeTranslator.java).
    pub fn translate(
        &mut self,
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
    ) -> Option<TreeId> {
        match tree {
            None => None,
            Some(tree) => {
                crate::tree::jc_tree::accept(ctx, tree, self);
                self.result.take()
            }
        }
    }

    /// Java: public <T extends JCTree> T translate(T tree, Type pt)
    /// (TransTypes.java).
    pub fn translate_with_pt(
        &mut self,
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        pt: Option<TypeId>,
    ) -> Option<TreeId> {
        let prev_pt = self.pt;
        self.pt = pt;
        let result = self.translate(ctx, tree);
        self.pt = prev_pt;
        result
    }

    /// Java: public <T extends JCTree> List<T> translate(List<T> trees)
    /// (TreeTranslator.java).
    pub fn translate_list(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        let mut current = trees.clone();
        while current.non_empty() {
            let tree = *current.head();
            current = current.tail();
            match self.translate(ctx, Some(tree)) {
                Some(translated) => result = result.append(translated),
                None => panic!("translation of a list element produced null"),
            }
        }
        result
    }

    /// Java: public <T extends JCTree> List<T> translate(List<T> trees, Type pt)
    /// (TransTypes.java).
    pub fn translate_list_with_pt(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
        pt: Option<TypeId>,
    ) -> List<TreeId> {
        let prev_pt = self.pt;
        self.pt = pt;
        let result = self.translate_list(ctx, trees);
        self.pt = prev_pt;
        result
    }

    /// Java: public List<JCVariableDecl> translateVarDefs(List<JCVariableDecl> trees)
    pub fn translate_var_defs(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCTypeParameter> translateTypeParams(List<JCTypeParameter> trees)
    pub fn translate_type_params(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCCase> translateCases(List<JCCase> trees)
    pub fn translate_cases(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCCatch> translateCatchers(List<JCCatch> trees)
    pub fn translate_catchers(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCAnnotation> translateAnnotations(List<JCAnnotation> trees)
    pub fn translate_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    // ------------------------------------------------------------------
    // Utility methods (TransTypes.java)
    // ------------------------------------------------------------------

    /// Java: private Type erasure(Type t) (TransTypes.java).
    fn erasure(&self, ctx: &mut CompilerContext, t: TypeId) -> TypeId {
        let syms = self.syms.borrow();
        self.types.erasure(ctx, &syms, t)
    }

    /// Java: tree.sym.erasure(types) (Symbol.erasure(Types) -> Types.erasure(type)).
    fn symbol_erasure(&self, ctx: &mut CompilerContext, sym: SymbolId) -> TypeId {
        let ty = ctx.sym(sym).ty.expect("symbol type");
        self.erasure(ctx, ty)
    }

    /// Java: Type getReturnType() view for an erased method/forall type.
    fn erased_return_type(&self, ctx: &mut CompilerContext, t: TypeId) -> TypeId {
        let erased = self.erasure(ctx, t);
        self.types.get_return_type_public(ctx, erased)
    }

    /// Java: JCExpression cast(JCExpression tree, Type target) (TransTypes.java).
    fn cast(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        target: TypeId,
    ) -> TreeId {
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        let syms = self.syms.borrow();
        let same = self.types.is_same_type(ctx, &syms, tree_ty, target);
        let accessible = ctx
            .typ(target)
            .tsym
            .map(|_tsym| {
                let env = self.env.clone().expect("env");
                self.resolve.borrow().is_accessible_type(
                    ctx,
                    &syms,
                    &env,
                    target,
                    true,
                )
            })
            .unwrap_or(true);
        drop(syms);
        if same {
            return tree;
        }
        if !accessible {
            let env = self.env.clone().expect("env");
            {
                let syms_borrow = self.syms.borrow_mut();
                self.resolve.borrow().log_access_error_internal(
                    ctx,
                    &syms_borrow,
                    &env,
                    tree,
                    target,
                );
            }
        }
        let pos = ctx.tree(tree).pos;
        let clazz_tree = {
            let mut make = self.make.borrow_mut();
            make.at(pos).type_from_type(ctx, &self.syms.borrow(), target)
        };
        let cast_tree = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            make.type_cast(ctx, clazz_tree, tree)
        };
        ctx.tree_mut(cast_tree).ty = Some(target);
        cast_tree
    }

    /// Java: public JCExpression coerce(Env<AttrContext> env, JCExpression tree,
    /// Type target) (TransTypes.java).
    pub fn coerce(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
        target: TypeId,
    ) -> TreeId {
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(env));
        let result = self.coerce_tree(ctx, tree, target);
        self.env = prev_env;
        result
    }

    /// Java: JCExpression coerce(JCExpression tree, Type target) (TransTypes.java).
    fn coerce_tree(&mut self, ctx: &mut CompilerContext, tree: TreeId, target: TypeId) -> TreeId {
        let base_target = {
            let syms = self.syms.borrow();
            self.types.base_type_of(ctx, &syms, target)
        };
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        if ctx.typ(tree_ty).is_primitive() == ctx.typ(target).is_primitive() {
            let syms = self.syms.borrow();
            let assignable = self.types.is_assignable(ctx, &syms, tree_ty, base_target);
            drop(syms);
            if assignable {
                tree
            } else {
                self.cast(ctx, tree, base_target)
            }
        } else {
            tree
        }
    }

    /// Java: JCExpression retype(JCExpression tree, Type erasedType, Type target)
    /// (TransTypes.java).
    fn retype(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        erased_type: TypeId,
        target: Option<TypeId>,
    ) -> TreeId {
        if !ctx.typ(erased_type).is_primitive() {
            let mut target = target;
            if let Some(current) = target
                && ctx.typ(current).is_primitive()
            {
                let tree_ty = ctx.tree(tree).ty.expect("tree type");
                target = Some(self.erasure(ctx, tree_ty));
            }
            ctx.tree_mut(tree).ty = Some(erased_type);
            if let Some(target) = target {
                return self.coerce_tree(ctx, tree, target);
            }
        }
        tree
    }

    /// Java: <T extends JCTree> List<T> translateArgs(List<T> _args, List<Type>
    /// parameters, Type varargsElement) (TransTypes.java).
    pub fn translate_args(
        &mut self,
        ctx: &mut CompilerContext,
        args: List<TreeId>,
        parameters: List<TypeId>,
        varargs_element: Option<TypeId>,
    ) -> List<TreeId> {
        if parameters.is_empty() {
            return args;
        }
        let mut current_args = args;
        let mut current_params = parameters;
        let mut translated: List<TreeId> = List::nil();
        // Java: while (parameters.tail.nonEmpty()) { translate(args.head, parameters.head); ... }
        while current_params.tail().non_empty() {
            let arg = *current_args.head();
            let param = *current_params.head();
            translated = translated.append(
                self.translate_with_pt(ctx, Some(arg), Some(param)).expect("arg"),
            );
            current_args = current_args.tail();
            current_params = current_params.tail();
        }
        let parameter = *current_params.head();
        match varargs_element {
            Some(varargs_element) => {
                // Java: while (args.nonEmpty()) { translate(args.head, varargsElement); ... }
                while current_args.non_empty() {
                    let arg = *current_args.head();
                    translated = translated.append(
                        self.translate_with_pt(ctx, Some(arg), Some(varargs_element))
                            .expect("arg"),
                    );
                    current_args = current_args.tail();
                }
            }
            None => {
                // Java: Assert.check(varargsElement != null || args.length() == 1);
                assert!(
                    !current_args.is_empty() && current_args.tail().is_empty(),
                    "Incorrect number of arguments"
                );
                let arg = *current_args.head();
                translated = translated.append(
                    self.translate_with_pt(ctx, Some(arg), Some(parameter)).expect("arg"),
                );
            }
        }
        translated
    }

    #[allow(dead_code)] // upstream public helper (TransTypes.java)
    /// Java: public <T extends JCTree> List<T> translateArgs(List<T> _args,
    /// List<Type> parameters, Type varargsElement, Env<AttrContext> localEnv)
    /// (TransTypes.java).
    pub fn translate_args_with_env(
        &mut self,
        ctx: &mut CompilerContext,
        args: List<TreeId>,
        parameters: List<TypeId>,
        varargs_element: Option<TypeId>,
        local_env: &Rc<Env<AttrContext>>,
    ) -> List<TreeId> {
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(local_env));
        let result = self.translate_args(ctx, args, parameters, varargs_element);
        self.env = prev_env;
        result
    }
    // ------------------------------------------------------------------
    // Bridge methods (TransTypes.java)
    // ------------------------------------------------------------------

    /// Java: void addBridge(DiagnosticPosition pos, MethodSymbol meth,
    /// MethodSymbol impl, ClassSymbol origin, ListBuffer<JCTree> bridges)
    /// (TransTypes.java).
    #[allow(clippy::too_many_lines)]
    fn add_bridge(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        meth: SymbolId,
        impl_sym: SymbolId,
        origin: SymbolId,
        bridges: &mut Vec<TreeId>,
    ) {
        self.make.borrow_mut().at(pos);
        let impl_type_erasure = self.symbol_erasure(ctx, impl_sym);
        // Java: Type bridgeType = meth.erasure(types);
        let bridge_type = self.symbol_erasure(ctx, meth);
        let access_flags = flags::AccessFlags as i64;
        let mut bridge_flags = (ctx.sym(impl_sym).flags_field & access_flags)
            | crate::code::flags::SYNTHETIC as i64
            | crate::code::flags::BRIDGE;
        if (ctx.sym(origin).flags_field & crate::code::flags::INTERFACE as i64) != 0 {
            bridge_flags |= crate::code::flags::DEFAULT;
        }
        let meth_name = ctx.sym(meth).name.clone();
        let bridge = ctx.alloc_symbol(Symbol::new_method_symbol(
            bridge_flags,
            meth_name,
            bridge_type,
            origin,
        ));
        // Java: bridge.params = createBridgeParams(impl, bridge, bridgeType);
        let bridge_params = self.create_bridge_params(ctx, impl_sym, bridge, bridge_type);
        if !bridge_params.is_empty()
            && let SymbolKind::MethodSymbol { params, .. } = &mut ctx.sym_mut(bridge).kind_data
        {
            *params = bridge_params.clone();
        }
        // Java: bridge.setAttributes(impl); (the ported set_attributes takes the
        // read-only context; the target symbol is updated through a local clone)
        let impl_symbol = ctx.sym(impl_sym).clone();
        let mut bridge_symbol = ctx.sym(bridge).clone();
        // Port adapter: Symbol::clone clears the arena id because a clone only
        // becomes addressable through alloc_symbol; this clone is written back
        // into the already-allocated bridge slot, so restore that id for the
        // metadata back-reference (initedMetadata reads it).
        bridge_symbol.id.set(Some(bridge));
        bridge_symbol.set_attributes(ctx, &impl_symbol);
        *ctx.sym_mut(bridge) = bridge_symbol;
        // Java: JCMethodDecl md = make.MethodDef(bridge, null);
        let (bridge_return_type, bridge_param_types) = {
            let syms = self.syms.borrow();
            let return_type = self.types.get_return_type_public(ctx, bridge_type);
            let param_types = self.types.get_parameter_types_public(ctx, bridge_type);
            let _ = syms;
            (return_type, param_types)
        };
        let mods = { self.make.borrow_mut().modifiers(ctx, bridge_flags) };
        let restype_tree = {
            self.make
                .borrow_mut()
                .type_from_type(ctx, &self.syms.borrow(), bridge_return_type)
        };
        let mut param_trees: List<TreeId> = List::nil();
        let mut params_current = bridge_params.clone();
        let mut types_current = bridge_param_types.clone();
        while params_current.non_empty() && types_current.non_empty() {
            let param = *params_current.head();
            let param_ty = *types_current.head();
            let (p_name, p_flags) = (ctx.sym(param).name.clone(), ctx.sym(param).flags_field);
            let p_mods = { self.make.borrow_mut().modifiers(ctx, p_flags) };
            let p_type_tree = {
                self.make
                    .borrow_mut()
                    .type_from_type(ctx, &self.syms.borrow(), param_ty)
            };
            let p_tree = self.make.borrow_mut().var_def(
                ctx,
                p_mods,
                p_name,
                Some(p_type_tree),
                None,
            );
            ctx.tree_mut(p_tree).ty = Some(param_ty);
            if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(p_tree).kind {
                *slot = Some(param);
            }
            param_trees = param_trees.append(p_tree);
            params_current = params_current.tail();
            types_current = types_current.tail();
        }
        let md = self.make.borrow_mut().method_def(
            ctx,
            mods,
            ctx.sym(bridge).name.clone(),
            Some(restype_tree),
            List::nil(),
            None,
            param_trees.clone(),
            List::nil(),
            None,
            None,
        );
        if let TreeKind::MethodDecl { sym: slot, .. } = &mut ctx.tree_mut(md).kind {
            *slot = Some(bridge);
        }
        // Java: JCExpression receiver = (impl.owner == origin) ? make.This(origin.erasure(types))
        // : make.Super(types.supertype(origin.type).tsym.erasure(types), origin);
        let receiver = {
            let origin_ty = ctx.sym(origin).ty.expect("origin type");
            let origin_erasure = self.erasure(ctx, origin_ty);
            if ctx.sym(impl_sym).owner == Some(origin) {
                self.make.borrow_mut().this_expr(ctx, origin_erasure)
            } else {
                let syms = self.syms.borrow();
                let st = self.types.supertype(ctx, &syms, origin_ty, None);
                let st_tsym = ctx.typ(st).tsym.expect("supertype symbol");
                let st_erasure = self.symbol_erasure(ctx, st_tsym);
                drop(syms);
                self.make.borrow_mut().super_expr(ctx, st_erasure, origin)
            }
        };
        // Java: Type calltype = implTypeErasure.getReturnType();
        let calltype = self.types.get_return_type_public(ctx, impl_type_erasure);
        let impl_name = ctx.sym(impl_sym).name.clone();
        let select_tree = self.make.borrow_mut().select(ctx, receiver, impl_name);
        tree_info::set_symbol(ctx, select_tree, impl_sym);
        ctx.tree_mut(select_tree).ty = Some(calltype);
        let idents = {
            let method_params = match &ctx.tree(md).kind {
                TreeKind::MethodDecl { params, .. } => params.clone(),
                _ => List::nil(),
            };
            self.make.borrow_mut().idents(ctx, method_params)
        };
        let impl_param_types = self.types.get_parameter_types_public(ctx, impl_type_erasure);
        let call_args = self.translate_args(ctx, idents, impl_param_types, None);
        let call = self.make.borrow_mut().apply(ctx, List::nil(), select_tree, call_args);
        ctx.tree_mut(call).ty = Some(calltype);
        let stat = if ctx.typ(calltype).has_tag(TypeTag::VOID) {
            self.make.borrow_mut().exec(ctx, call)
        } else {
            let bridge_return = self.types.get_return_type_public(ctx, bridge_type);
            let coerced = self.coerce_tree(ctx, call, bridge_return);
            self.make.borrow_mut().return_(ctx, Some(coerced))
        };
        let body = self.make.borrow_mut().block(ctx, 0, List::of(stat));
        if let TreeKind::MethodDecl { body: slot, .. } = &mut ctx.tree_mut(md).kind {
            *slot = Some(body);
        }
        bridges.push(md);
        // Java: origin.members().enter(bridge);
        let origin_members = ctx.sym(origin).members().expect("origin members");
        ctx.scope_enter(origin_members, bridge);
    }

    /// Java: private List<VarSymbol> createBridgeParams(MethodSymbol impl,
    /// MethodSymbol bridge, Type bridgeType) (TransTypes.java).
    fn create_bridge_params(
        &mut self,
        ctx: &mut CompilerContext,
        impl_sym: SymbolId,
        bridge: SymbolId,
        bridge_type: TypeId,
    ) -> List<SymbolId> {
        let impl_params = match &ctx.sym(impl_sym).kind_data {
            SymbolKind::MethodSymbol { params, .. } => params.clone(),
            _ => List::nil(),
        };
        if impl_params.is_empty() {
            return List::nil();
        }
        let arg_types = { self.types.get_parameter_types_public(ctx, bridge_type) };
        let mut bridge_params: List<SymbolId> = List::nil();
        let mut impl_current = impl_params;
        let mut arg_current = arg_types;
        while impl_current.non_empty() && arg_current.non_empty() {
            let impl_param = *impl_current.head();
            let arg_type = *arg_current.head();
            let (name, param_flags) = (
                ctx.sym(impl_param).name.clone(),
                ctx.sym(impl_param).flags_field,
            );
            let flags_value = param_flags
                | crate::code::flags::SYNTHETIC as i64
                | crate::code::flags::PARAMETER;
            let param = ctx.alloc_symbol(Symbol::new_var_symbol(flags_value, name, arg_type, bridge));
            let impl_param_symbol = ctx.sym(impl_param).clone();
            let mut param_symbol = ctx.sym(param).clone();
            param_symbol.set_attributes(ctx, &impl_param_symbol);
            *ctx.sym_mut(param) = param_symbol;
            bridge_params = bridge_params.append(param);
            impl_current = impl_current.tail();
            arg_current = arg_current.tail();
        }
        bridge_params
    }

    /// Java: void addBridgeIfNeeded(DiagnosticPosition pos, Symbol sym,
    /// ClassSymbol origin, ListBuffer<JCTree> bridges) (TransTypes.java).
    fn add_bridge_if_needed(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        sym: SymbolId,
        origin: SymbolId,
        bridges: &mut Vec<TreeId>,
    ) {
        let init_name = self.names.init.clone();
        let sym_name = ctx.sym(sym).name.clone();
        let sym_flags = ctx.sym(sym).flags_field;
        let is_member = {
            let syms = self.syms.borrow();
            crate::code::symbol::symbol_is_member_of(ctx, &self.types, &syms, sym, origin)
        };
        if ctx.sym(sym).kind == Kind::MTH
            && !sym_name.content_equals(&init_name.to_java_string())
            && (sym_flags & (crate::code::flags::PRIVATE as i64 | crate::code::flags::STATIC as i64)) == 0
            && (sym_flags & crate::code::flags::SYNTHETIC as i64)
                != crate::code::flags::SYNTHETIC as i64
            && is_member
        {
            let meth = sym;
            let (bridge, impl_sym) = {
                let syms = self.syms.borrow_mut();
                let bridge = crate::code::symbol::binary_implementation(ctx, &syms, &self.types, meth, origin);
                let impl_sym = crate::code::symbol::implementation(ctx, &syms, &self.types, meth, origin, true);
                (bridge, impl_sym)
            };
            let bridge_is_absent_or_self = match bridge {
                None => true,
                Some(bridge) => bridge == meth,
            };
            let impl_different_owner = match (bridge, impl_sym) {
                (Some(bridge), Some(impl_sym)) => {
                    let syms = self.syms.borrow();
                    let bridge_owner = ctx.sym(bridge).owner.expect("bridge owner");
                    let impl_owner = ctx.sym(impl_sym).owner.expect("impl owner");
                    !crate::code::symbol::symbol_is_sub_class(ctx, &self.types, &syms, bridge_owner, impl_owner)
                }
                _ => false,
            };
            if (bridge_is_absent_or_self || impl_different_owner)
                && let Some(impl_sym) = impl_sym
            {
                let origin_ty = ctx.sym(origin).ty.expect("origin type");
                let needs_bridge = bridge != Some(impl_sym)
                    && self.is_bridge_needed(ctx, meth, impl_sym, origin_ty);
                let reflection_workaround = impl_sym == meth
                    && ctx.sym(impl_sym).owner != Some(origin)
                    && (ctx.sym(impl_sym).flags_field & crate::code::flags::FINAL as i64) == 0
                    && (ctx.sym(meth).flags_field
                        & (crate::code::flags::ABSTRACT as i64 | crate::code::flags::PUBLIC as i64))
                        == crate::code::flags::PUBLIC as i64
                    && ((ctx.sym(origin).flags_field & crate::code::flags::PUBLIC as i64)
                        > ctx
                            .sym(impl_sym)
                            .owner
                            .map(|o| ctx.sym(o).flags_field & crate::code::flags::PUBLIC as i64)
                            .unwrap_or(0));
                if needs_bridge || reflection_workaround {
                    self.add_bridge(ctx, pos, meth, impl_sym, origin, bridges);
                }
            }
        }
    }
    /// Java: private boolean isBridgeNeeded(MethodSymbol method, MethodSymbol impl,
    /// Type dest) (TransTypes.java).
    fn is_bridge_needed(
        &mut self,
        ctx: &mut CompilerContext,
        method: SymbolId,
        impl_sym: SymbolId,
        dest: TypeId,
    ) -> bool {
        if impl_sym != method {
            let method_erasure = self.symbol_erasure(ctx, method);
            if !self.is_same_member_when_erased(ctx, dest, method, method_erasure) {
                return true;
            }
            let impl_erasure = self.symbol_erasure(ctx, impl_sym);
            if !self.is_same_member_when_erased(ctx, dest, impl_sym, impl_erasure) {
                return true;
            }
            let syms = self.syms.borrow();
            let same = self.types.is_same_type(ctx, &syms, impl_erasure, method_erasure);
            return !same;
        }
        if (ctx.sym(method).flags_field & crate::code::flags::ABSTRACT as i64) != 0 {
            return false;
        }
        let method_erasure = self.symbol_erasure(ctx, method);
        !self.is_same_member_when_erased(ctx, dest, method, method_erasure)
    }

    /// Java: private boolean isSameMemberWhenErased(Type type, MethodSymbol method,
    /// Type erasure) (TransTypes.java).
    fn is_same_member_when_erased(
        &mut self,
        ctx: &mut CompilerContext,
        ty: TypeId,
        method: SymbolId,
        erasure: TypeId,
    ) -> bool {
        let member_ty = {
            let syms = self.syms.borrow();
            self.types.member_type(ctx, &syms, ty, method)
        };
        let erased_member = self.erasure(ctx, member_ty);
        let syms = self.syms.borrow();
        self.types.is_same_type(ctx, &syms, erased_member, erasure)
    }

    /// Java: void addBridges(DiagnosticPosition pos, TypeSymbol i, ClassSymbol origin,
    /// ListBuffer<JCTree> bridges) (TransTypes.java).
    fn add_bridges_from_type(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        i: SymbolId,
        origin: SymbolId,
        bridges: &mut Vec<TreeId>,
    ) {
        let members = ctx.sym(i).members();
        if let Some(members) = members {
            let symbols = ctx.scope_get_symbols(
                members,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for sym in symbols {
                self.add_bridge_if_needed(ctx, pos, sym, origin, bridges);
            }
        }
        let i_type = ctx.sym(i).ty.expect("type symbol type");
        let interfaces = {
            let syms = self.syms.borrow();
            self.types.interfaces(ctx, &syms, i_type)
        };
        let mut current = interfaces;
        while current.non_empty() {
            let head = *current.head();
            if let Some(tsym) = ctx.typ(head).tsym {
                self.add_bridges_from_type(ctx, pos, tsym, origin, bridges);
            }
            current = current.tail();
        }
    }

    /// Java: void addBridges(DiagnosticPosition pos, ClassSymbol origin,
    /// ListBuffer<JCTree> bridges) (TransTypes.java).
    fn add_bridges(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        origin: SymbolId,
        bridges: &mut Vec<TreeId>,
    ) {
        let origin_ty = ctx.sym(origin).ty.expect("origin type");
        let mut st = {
            let syms = self.syms.borrow();
            self.types.supertype(ctx, &syms, origin_ty, None)
        };
        while ctx.typ(st).has_tag(TypeTag::CLASS) {
            if let Some(tsym) = ctx.typ(st).tsym {
                self.add_bridges_from_type(ctx, pos, tsym, origin, bridges);
            }
            st = {
                let syms = self.syms.borrow();
                self.types.supertype(ctx, &syms, st, None)
            };
        }
        let interfaces = {
            let syms = self.syms.borrow();
            self.types.interfaces(ctx, &syms, origin_ty)
        };
        let mut current = interfaces;
        while current.non_empty() {
            let interface = *current.head();
            if let Some(tsym) = ctx.typ(interface).tsym {
                self.add_bridges_from_type(ctx, pos, tsym, origin, bridges);
            }
            current = current.tail();
        }
    }

    // ------------------------------------------------------------------
    // Main method / class translation (TransTypes.java)
    // ------------------------------------------------------------------

    /// Java: void translateClass(ClassSymbol c) (TransTypes.java).
    fn translate_class(
        &mut self,
        ctx: &mut CompilerContext,
        c: SymbolId,
    ) {
        let c_type = ctx.sym(c).ty.expect("class type");
        let st = {
            let syms = self.syms.borrow();
            self.types.supertype(ctx, &syms, c_type, None)
        };
        if ctx.typ(st).has_tag(TypeTag::CLASS)
            && let Some(st_tsym) = ctx.typ(st).tsym
        {
            self.translate_class(ctx, st_tsym);
        }
        let my_env = self.enter.borrow().get_env(c);
        let Some(my_env) = my_env else {
            return;
        };
        if (ctx.sym(c).flags_field & crate::code::flags::TYPE_TRANSLATED) != 0 {
            return;
        }
        ctx.sym_mut(c).flags_field |= crate::code::flags::TYPE_TRANSLATED;
        let env_has_comp_state = {
            let states = self.compile_states.borrow();
            states.get(&my_env).is_some()
        };
        let outermost = crate::code::symbol::symbol_outermost_class(ctx, c);
        if !env_has_comp_state && outermost == Some(c) {
            panic!("AssertionError: No info for outermost class");
        }
        if env_has_comp_state {
            let state = {
                let states = self.compile_states.borrow();
                states.get(&my_env).expect("compile state")
            };
            if CompileState::FLOW.is_after(state) {
                panic!("AssertionError: The current compile state is previous to FLOW");
            }
        }
        let old_env = self.env.clone();
        self.env = Some(Rc::clone(&my_env));
        let saved_make = Rc::clone(&self.make);
        let saved_pt = self.pt;
        {
            let toplevel = my_env.toplevel.expect("toplevel");
            let new_make = {
                let saved = saved_make.borrow();
                saved.for_toplevel(Some(toplevel))
            };
            self.make = Rc::new(RefCell::new(new_make));
        }
        self.pt = None;
        let tree = my_env.tree.expect("class env tree");
        if let TreeKind::ClassDecl { typarams: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = List::nil();
        }
        // Java: super.visitClassDef(tree) (TreeTranslator default navigation).
        self.default_visit_class_def(ctx, tree);
        let pos = ctx.tree(tree).pos;
        self.make.borrow_mut().at(pos);
        let mut bridges: Vec<TreeId> = Vec::new();
        let is_interface = (ctx.sym(c).flags_field & crate::code::flags::INTERFACE as i64) != 0;
        if self.allow_interface_bridges || !is_interface {
            self.add_bridges(ctx, pos, c, &mut bridges);
        }
        let defs = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => List::nil(),
        };
        let mut new_defs: List<TreeId> = List::nil();
        for bridge in bridges.iter() {
            new_defs = new_defs.append(*bridge);
        }
        let mut current = defs;
        while current.non_empty() {
            new_defs = new_defs.append(*current.head());
            current = current.tail();
        }
        if let TreeKind::ClassDecl { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = new_defs;
        }
        let tree_ty = ctx.tree(tree).ty.expect("class tree type");
        let erased = self.erasure(ctx, tree_ty);
        ctx.tree_mut(tree).ty = Some(erased);
        self.make = saved_make;
        self.pt = saved_pt;
        self.env = old_env;
    }

    /// Java: public JCTree translateTopLevelClass(JCTree cdef, TreeMaker make)
    /// (TransTypes.java).
    pub fn translate_top_level_class(
        &mut self,
        ctx: &mut CompilerContext,
        cdef: TreeId,
        make: Rc<RefCell<TreeMaker>>,
    ) -> Option<TreeId> {
        self.make = make;
        self.pt = None;
        self.translate_with_pt(ctx, Some(cdef), None)
    }

    /// Java: TreeTranslator.visitClassDef default navigation, invoked by
    /// translateClass (TransTypes.java:955: super.visitClassDef(tree)).
    fn default_visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, typarams, extending, implementing, defs) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { mods, typarams, extending, implementing, defs, .. } => {
                (*mods, typarams.clone(), *extending, implementing.clone(), defs.clone())
            }
            _ => panic!("class declaration expected"),
        };
        let mods = self.translate_with_pt(ctx, mods, None);
        let typarams = self.translate_type_params(ctx, &typarams);
        let extending = self.translate_with_pt(ctx, extending, None);
        let implementing = self.translate_list(ctx, &implementing);
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::ClassDecl {
            mods: m,
            typarams: t,
            extending: e,
            implementing: i,
            defs: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *t = typarams;
            *e = extending;
            *i = implementing;
            *d = defs;
        }
        self.result = Some(tree);
    }
}


impl TreeVisitor for TransTypes {
    /// Java: public void visitClassDef(JCClassDecl tree) (TransTypes.java).
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("class declaration without a symbol"),
        };
        self.translate_class(ctx, sym);
        self.result = Some(tree);
    }

    /// Java: public void visitMethodDef(JCMethodDecl tree) (TransTypes.java).
    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_ret = self.return_type;
        let sym = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("method declaration without a symbol"),
        };
        let tree_ty = ctx.tree(tree).ty.expect("method type");
        self.return_type = Some(self.erased_return_type(ctx, tree_ty));
        let (restype, params, recvparam, thrown, body) = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl {
                restype,
                params,
                recvparam,
                thrown,
                body,
                ..
            } => (*restype, params.clone(), *recvparam, thrown.clone(), *body),
            _ => crate::util::assert::error(),
        };
        let restype = self.translate_with_pt(ctx, restype, None);
        let params = self.translate_var_defs(ctx, &params);
        let recvparam = self.translate_with_pt(ctx, recvparam, None);
        let thrown = self.translate_list_with_pt(ctx, &thrown, None);
        let body_erasure = self.symbol_erasure(ctx, sym);
        let body_pt = Some(self.types.get_return_type_public(ctx, body_erasure));
        let body = self.translate_with_pt(ctx, body, body_pt);
        let erased = self.erasure(ctx, tree_ty);
        if let TreeKind::MethodDecl {
            restype: r,
            typarams,
            params: p,
            recvparam: rp,
            thrown: t,
            body: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *r = restype;
            *typarams = List::nil();
            *p = params;
            *rp = recvparam;
            *t = thrown;
            *b = body;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(tree);
        self.return_type = prev_ret;
    }

    /// Java: public void visitVarDef(JCVariableDecl tree) (TransTypes.java).
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (vartype, init, sym, tree_ty) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { vartype, init, sym, .. } => (*vartype, *init, *sym, ctx.tree(tree).ty),
            _ => panic!("variable declaration expected"),
        };
        let vartype = self.translate_with_pt(ctx, vartype, None);
        let init_pt = sym.map(|sym| self.symbol_erasure(ctx, sym));
        let init = self.translate_with_pt(ctx, init, init_pt);
        let erased = tree_ty.map(|ty| self.erasure(ctx, ty));
        if let TreeKind::VariableDecl { vartype: vt, init: i, .. } = &mut ctx.tree_mut(tree).kind {
            *vt = vartype;
            *i = init;
        }
        ctx.tree_mut(tree).ty = erased;
        self.result = Some(tree);
    }

    /// Java: public void visitDoLoop(JCDoWhileLoop tree) (TransTypes.java).
    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("do loop expected"),
        };
        let body = self.translate_with_pt(ctx, body, None);
        let bool_ty = self.syms.borrow().boolean_type;
        let cond = self.translate_with_pt(ctx, cond, Some(bool_ty));
        if let TreeKind::DoWhileLoop { body: b, cond: c } = &mut ctx.tree_mut(tree).kind {
            *b = body;
            *c = cond;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitWhileLoop(JCWhileLoop tree) (TransTypes.java).
    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("while loop expected"),
        };
        let bool_ty = self.syms.borrow().boolean_type;
        let cond = self.translate_with_pt(ctx, cond, Some(bool_ty));
        let body = self.translate_with_pt(ctx, body, None);
        if let TreeKind::WhileLoop { cond: c, body: b } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *b = body;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitForLoop(JCForLoop tree) (TransTypes.java).
    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop { init, cond, step, body } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("for loop expected"),
        };
        let init = self.translate_list_with_pt(ctx, &init, None);
        let bool_ty = self.syms.borrow().boolean_type;
        let cond = if cond.is_some() {
            self.translate_with_pt(ctx, cond, Some(bool_ty))
        } else {
            None
        };
        let step = self.translate_list_with_pt(ctx, &step, None);
        let body = self.translate_with_pt(ctx, body, None);
        if let TreeKind::ForLoop { init: i, cond: c, step: s, body: b } = &mut ctx.tree_mut(tree).kind {
            *i = init;
            *c = cond;
            *s = step;
            *b = body;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitForeachLoop(JCEnhancedForLoop tree) (TransTypes.java).
    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("foreach loop expected"),
        };
        let var = self.translate_with_pt(ctx, var, None);
        let iterable_type = expr.and_then(|expr| ctx.tree(expr).ty);
        let expr_erasure = iterable_type.map(|ty| self.erasure(ctx, ty));
        let expr = self.translate_with_pt(ctx, expr, expr_erasure);
        let elemtype = {
            let syms = self.syms.borrow();
            let expr_ty = expr.and_then(|expr| ctx.tree(expr).ty).expect("foreach expr type");
            self.types.elemtype(ctx, &syms, expr_ty)
        };
        let no_type = crate::code::symtab::no_type_id(ctx);
        if elemtype == no_type {
            // Java: tree.expr.type = iterableType; // preserve type for Lower
            if let (Some(preserved), Some(expr)) = (iterable_type, expr) {
                ctx.tree_mut(expr).ty = Some(preserved);
            }
        }
        let body = self.translate_with_pt(ctx, body, None);
        if let TreeKind::EnhancedForLoop { var: v, expr: e, body: b } = &mut ctx.tree_mut(tree).kind {
            *v = var;
            *e = expr;
            *b = body;
        }
        self.result = Some(tree);
    }
    /// Java: public void visitLambda(JCLambda tree) (TransTypes.java).
    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_ret = self.return_type;
        let (params, body, target, tree_ty) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { params, body, target, .. } => (params.clone(), *body, *target, ctx.tree(tree).ty),
            _ => panic!("lambda expected"),
        };
        let descriptor = {
            let diags = Rc::clone(&self.diags);
            let chk = Rc::clone(&self.chk);
            let chk = chk.borrow();
            let syms = self.syms.borrow_mut();
            self.types
                .lambda_descriptor_type(ctx, &syms, &diags, &chk, target)
        };
        self.return_type = Some(self.erased_return_type(ctx, descriptor));
        let params = self.translate_list_with_pt(ctx, &params, None);
        let body_ty = body.and_then(|body| ctx.tree(body).ty);
        let body_pt = match (body_ty, self.return_type) {
            (None, _) => None,
            (Some(_), Some(ret)) => {
                if ctx.typ(ret).has_tag(TypeTag::VOID) { None } else { Some(ret) }
            }
            (Some(_), None) => None,
        };
        let body = self.translate_with_pt(ctx, body, body_pt);
        let new_ty = match tree_ty {
            Some(ty) => {
                if !ctx.typ(ty).is_intersection() {
                    Some(self.erasure(ctx, ty))
                } else {
                    let tsym = ctx.typ(ty).tsym.expect("intersection symbol");
                    let desc = {
                        let diags = Rc::clone(&self.diags);
                        let chk = Rc::clone(&self.chk);
                        let chk = chk.borrow();
                        let syms = self.syms.borrow_mut();
                        self.types
                            .find_descriptor_symbol(ctx, &syms, &diags, &chk, tsym)
                            .expect("descriptor symbol")
                    };
                    let owner = ctx.sym(desc).owner.expect("descriptor owner");
                    let owner_ty = ctx.sym(owner).ty.expect("descriptor owner type");
                    Some(self.erasure(ctx, owner_ty))
                }
            }
            None => None,
        };
        if let TreeKind::Lambda { params: p, body: b, .. } = &mut ctx.tree_mut(tree).kind {
            *p = params;
            *b = body;
        }
        ctx.tree_mut(tree).ty = new_ty;
        self.result = Some(tree);
        self.return_type = prev_ret;
    }

    /// Java: public void visitSwitch(JCSwitch tree) (TransTypes.java).
    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch { selector, cases, .. } => (*selector, cases.clone()),
            _ => panic!("switch expected"),
        };
        let selector_ty = selector.and_then(|selector| ctx.tree(selector).ty).expect("switch selector type");
        let enum_switch = {
            let syms = self.syms.borrow();
            let selsuper = self.types.supertype(ctx, &syms, selector_ty, None);
            ctx.typ(selsuper).tsym == Some(syms.enum_sym)
        };
        let target = if enum_switch {
            self.erasure(ctx, selector_ty)
        } else {
            self.syms.borrow().int_type
        };
        let selector = self.translate_with_pt(ctx, selector, Some(target));
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::Switch { selector: sel, cases: c, .. } = &mut ctx.tree_mut(tree).kind {
            *sel = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitCase(JCCase tree) (TransTypes.java).
    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("case expected"),
        };
        let labels = self.translate_list_with_pt(ctx, &labels, None);
        let stats = self.translate_list_with_pt(ctx, &stats, None);
        if let TreeKind::Case { labels: l, stats: s, .. } = &mut ctx.tree_mut(tree).kind {
            *l = labels;
            *s = stats;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitBindingPattern(JCBindingPattern tree) (TransTypes.java).
    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("binding pattern expected"),
        };
        let var = self.translate_with_pt(ctx, var, None);
        if let TreeKind::BindingPattern { var: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = var;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitSwitchExpression(JCSwitchExpression tree) (TransTypes.java).
    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, tree_ty) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression { selector, cases, .. } => {
                (*selector, cases.clone(), ctx.tree(tree).ty)
            }
            _ => panic!("switch expression expected"),
        };
        let selector_ty = selector.and_then(|selector| ctx.tree(selector).ty).expect("selector type");
        let enum_switch = {
            let syms = self.syms.borrow();
            let selsuper = self.types.supertype(ctx, &syms, selector_ty, None);
            ctx.typ(selsuper).tsym == Some(syms.enum_sym)
        };
        let target = if enum_switch {
            self.erasure(ctx, selector_ty)
        } else {
            self.syms.borrow().int_type
        };
        let selector = self.translate_with_pt(ctx, selector, Some(target));
        let cases = self.translate_cases(ctx, &cases);
        let erased = tree_ty.map(|ty| self.erasure(ctx, ty));
        if let TreeKind::SwitchExpression { selector: sel, cases: c, .. } = &mut ctx.tree_mut(tree).kind {
            *sel = selector;
            *c = cases;
        }
        ctx.tree_mut(tree).ty = erased;
        self.result = Some(self.retype(ctx, tree, erased.expect("switch type"), self.pt));
    }

    /// Java: public void visitParenthesizedPattern(JCParenthesizedPattern tree)
    /// (TransTypes.java).
    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("parenthesized pattern expected"),
        };
        let pattern = self.translate_with_pt(ctx, pattern, None);
        if let TreeKind::ParenthesizedPattern { pattern: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = pattern;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitGuardPattern(JCGuardPattern tree) (TransTypes.java).
    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("guard pattern expected"),
        };
        let patt = self.translate_with_pt(ctx, patt, None);
        let bool_ty = self.syms.borrow().boolean_type;
        let expr = self.translate_with_pt(ctx, expr, Some(bool_ty));
        if let TreeKind::GuardPattern { patt: p, expr: e } = &mut ctx.tree_mut(tree).kind {
            *p = patt;
            *e = expr;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitSynchronized(JCSynchronized tree) (TransTypes.java).
    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("synchronized expected"),
        };
        let lock_pt = lock.and_then(|lock| ctx.tree(lock).ty).map(|ty| self.erasure(ctx, ty));
        let lock = self.translate_with_pt(ctx, lock, lock_pt);
        let body = self.translate_with_pt(ctx, body, None);
        if let TreeKind::Synchronized { lock: l, body: b } = &mut ctx.tree_mut(tree).kind {
            *l = lock;
            *b = body;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitTry(JCTry tree) (TransTypes.java).
    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try { resources, body, catchers, finalizer, .. } => {
                (resources.clone(), *body, catchers.clone(), *finalizer)
            }
            _ => panic!("try expected"),
        };
        let auto_closeable = self.syms.borrow().auto_closeable_type;
        let resources = self.translate_list_with_pt(ctx, &resources, Some(auto_closeable));
        let body = self.translate_with_pt(ctx, body, None);
        let catchers = self.translate_catchers(ctx, &catchers);
        let finalizer = self.translate_with_pt(ctx, finalizer, None);
        if let TreeKind::Try { resources: r, body: b, catchers: c, finalizer: f, .. } = &mut ctx.tree_mut(tree).kind {
            *r = resources;
            *b = body;
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitConditional(JCConditional tree) (TransTypes.java).
    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart, tree_ty) = match &ctx.tree(tree).kind {
            TreeKind::Conditional { cond, truepart, falsepart, .. } => {
                (*cond, *truepart, *falsepart, ctx.tree(tree).ty)
            }
            _ => panic!("conditional expected"),
        };
        let bool_ty = self.syms.borrow().boolean_type;
        let cond = self.translate_with_pt(ctx, cond, Some(bool_ty));
        let erased = self.erasure(ctx, tree_ty.expect("conditional type"));
        let truepart = self.translate_with_pt(ctx, truepart, Some(erased));
        let falsepart = self.translate_with_pt(ctx, falsepart, Some(erased));
        if let TreeKind::Conditional { cond: c, truepart: t, falsepart: f, .. } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *t = truepart;
            *f = falsepart;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(self.retype(ctx, tree, erased, self.pt));
    }

    /// Java: public void visitIf(JCIf tree) (TransTypes.java).
    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If { cond, thenpart, elsepart } => (*cond, *thenpart, *elsepart),
            _ => panic!("if expected"),
        };
        let bool_ty = self.syms.borrow().boolean_type;
        let cond = self.translate_with_pt(ctx, cond, Some(bool_ty));
        let thenpart = self.translate_with_pt(ctx, thenpart, None);
        let elsepart = self.translate_with_pt(ctx, elsepart, None);
        if let TreeKind::If { cond: c, thenpart: t, elsepart: e } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *t = thenpart;
            *e = elsepart;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitExec(JCExpressionStatement tree) (TransTypes.java).
    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("exec expected"),
        };
        let expr = self.translate_with_pt(ctx, expr, None);
        if let TreeKind::ExpressionStatement { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitReturn(JCReturn tree) (TransTypes.java).
    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("return expected"),
        };
        if let Some(ret) = self.return_type
            && !ctx.typ(ret).has_tag(TypeTag::VOID)
        {
            let expr = self.translate_with_pt(ctx, expr, Some(ret));
            if let TreeKind::Return { expr: slot } = &mut ctx.tree_mut(tree).kind {
                *slot = expr;
            }
        }
        self.result = Some(tree);
    }

    /// Java: public void visitBreak(JCBreak tree) (TransTypes.java).
    fn visit_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitYield(JCYield tree) (TransTypes.java).
    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("yield expected"),
        };
        let value_ty = value.and_then(|value| ctx.tree(value).ty).expect("yield value type");
        let erased = self.erasure(ctx, value_ty);
        let translated = self.translate_with_pt(ctx, value, Some(erased));
        ctx.tree_mut(translated.expect("yield value")).ty = Some(erased);
        let retyped = self.retype(ctx, translated.expect("yield value"), erased, self.pt);
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(retyped);
        }
        self.result = Some(tree);
    }

    /// Java: public void visitThrow(JCThrow tree) (TransTypes.java).
    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("throw expected"),
        };
        let pt = expr.and_then(|expr| ctx.tree(expr).ty).map(|ty| self.erasure(ctx, ty));
        let expr = self.translate_with_pt(ctx, expr, pt);
        if let TreeKind::Throw { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitAssert(JCAssert tree) (TransTypes.java).
    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("assert expected"),
        };
        let bool_ty = self.syms.borrow().boolean_type;
        let cond = self.translate_with_pt(ctx, cond, Some(bool_ty));
        let detail_pt = detail.and_then(|detail| ctx.tree(detail).ty).map(|ty| self.erasure(ctx, ty));
        let detail = self.translate_with_pt(ctx, detail, detail_pt);
        if let TreeKind::Assert { cond: c, detail: d } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *d = detail;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitApply(JCMethodInvocation tree) (TransTypes.java).
    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args, varargs_element) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, varargs_element, .. } => (*meth, args.clone(), *varargs_element),
            _ => panic!("method invocation expected"),
        };
        let meth = self.translate_with_pt(ctx, meth, None).expect("method expression");
        let meth_sym = tree_info::symbol(ctx, meth).expect("method symbol");
        let mt = self.symbol_erasure(ctx, meth_sym);
        let use_instantiated_pt_args = {
            let syms = self.syms.borrow();
            self.allow_graph_inference
                && !self.types.is_signature_polymorphic(ctx, &syms, crate::code::symbol::base_symbol(ctx, meth_sym))
        };
        let mut argtypes = if use_instantiated_pt_args {
            let meth_ty = ctx.tree(meth).ty.expect("method expression type");
            self.types.get_parameter_types_public(ctx, meth_ty)
        } else {
            self.types.get_parameter_types_public(ctx, mt)
        };
        let is_enum_init = {
            let syms = self.syms.borrow();
            ctx.sym(meth_sym).name.content_equals(&self.names.init.to_java_string())
                && ctx.sym(meth_sym).owner == Some(syms.enum_sym)
        };
        if is_enum_init {
            argtypes = argtypes.tail().tail();
        }
        let varargs_element = varargs_element.map(|ty| self.erasure(ctx, ty));
        if varargs_element.is_none() {
            let args_len = args.length();
            let argtypes_len = argtypes.length();
            if args_len != argtypes_len {
                panic!("Incorrect number of arguments; expected {argtypes_len}, found {args_len}");
            }
        }
        let args = self.translate_args(ctx, args, argtypes, varargs_element);
        let tree_ty = ctx.tree(tree).ty.expect("call type");
        let erased = self.erasure(ctx, tree_ty);
        let mt_return = self.types.get_return_type_public(ctx, mt);
        if let TreeKind::MethodInvocation { meth: m, args: a, varargs_element: v, .. } = &mut ctx.tree_mut(tree).kind {
            *m = Some(meth);
            *a = args;
            *v = varargs_element;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(self.retype(ctx, tree, mt_return, self.pt));
    }

    /// Java: public void visitNewClass(JCNewClass tree) (TransTypes.java).
    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, clazz, args, def, constructor, constructor_type, varargs_element) = match &ctx.tree(tree).kind {
            TreeKind::NewClass { encl, clazz, args, def, constructor, constructor_type, varargs_element, .. } => (
                *encl, *clazz, args.clone(), *def, *constructor, *constructor_type, *varargs_element,
            ),
            _ => panic!("new class expected"),
        };
        let mut encl = encl;
        let mut args = args;
        if let Some(encl_tree) = encl {
            if def.is_none() {
                let pt = ctx.tree(encl_tree).ty.map(|ty| self.erasure(ctx, ty));
                encl = self.translate_with_pt(ctx, Some(encl_tree), pt);
            } else {
                let env = self.env.clone().expect("env");
                let null_check = self.attr.borrow_mut().make_null_check(ctx, &self.syms.borrow_mut(), encl_tree);
                let mut new_args: List<TreeId> = List::nil();
                new_args = new_args.append(null_check);
                let mut current = args;
                while current.non_empty() {
                    new_args = new_args.append(*current.head());
                    current = current.tail();
                }
                args = new_args;
                let _ = env;
                encl = None;
            }
        }
        let erased_constructor_type = constructor_type.map(|ty| self.erasure(ctx, ty));
        let argtypes = if let Some(erased) = erased_constructor_type {
            if self.allow_graph_inference {
                self.types.get_parameter_types_public(ctx, erased)
            } else {
                let ctor = constructor.expect("constructor");
                let ctor_erasure = self.symbol_erasure(ctx, ctor);
                self.types.get_parameter_types_public(ctx, ctor_erasure)
            }
        } else {
            let ctor = constructor.expect("constructor");
            let ctor_erasure = self.symbol_erasure(ctx, ctor);
            self.types.get_parameter_types_public(ctx, ctor_erasure)
        };
        let clazz = self.translate_with_pt(ctx, clazz, None).expect("class expression");
        let varargs_element = varargs_element.map(|ty| self.erasure(ctx, ty));
        let args = self.translate_args(ctx, args, argtypes, varargs_element);
        let def = self.translate_with_pt(ctx, def, None);
        let tree_ty = ctx.tree(tree).ty.expect("new class type");
        let erased = self.erasure(ctx, tree_ty);
        if let TreeKind::NewClass { encl: e, clazz: c, args: a, def: d, constructor_type: ct, varargs_element: v, .. } = &mut ctx.tree_mut(tree).kind {
            *e = encl;
            *c = Some(clazz);
            *a = args;
            *d = def;
            *ct = erased_constructor_type.or(*ct);
            *v = varargs_element;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(tree);
    }

    /// Java: public void visitNewArray(JCNewArray tree) (TransTypes.java).
    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (elemtype, dims, elems, tree_ty) = match &ctx.tree(tree).kind {
            TreeKind::NewArray { elemtype, dims, elems, .. } => (*elemtype, dims.clone(), elems.clone(), ctx.tree(tree).ty),
            _ => panic!("new array expected"),
        };
        let elemtype = self.translate_with_pt(ctx, elemtype, None);
        let int_ty = self.syms.borrow().int_type;
        let dims = self.translate_list_with_pt(ctx, &dims, Some(int_ty));
        let (elems, erased) = if let Some(tree_ty) = tree_ty {
            let elem = {
                let syms = self.syms.borrow();
                self.types.elemtype(ctx, &syms, tree_ty)
            };
            let erased = self.erasure(ctx, tree_ty);
            let elems = elems.map(|elems| self.translate_list_with_pt(ctx, &elems, Some(elem)));
            (elems, Some(erased))
        } else {
            let elems = elems.map(|elems| self.translate_list_with_pt(ctx, &elems, None));
            (elems, None)
        };
        if let TreeKind::NewArray { elemtype: e, dims: d, elems: el, .. } = &mut ctx.tree_mut(tree).kind {
            *e = elemtype;
            *d = dims;
            *el = elems;
        }
        ctx.tree_mut(tree).ty = erased;
        self.result = Some(tree);
    }

    /// Java: public void visitParens(JCParens tree) (TransTypes.java).
    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("parens expected"),
        };
        let expr = self.translate_with_pt(ctx, expr, self.pt).expect("parens expr");
        let expr_ty = ctx.tree(expr).ty.expect("parens expr type");
        let erased = self.erasure(ctx, expr_ty);
        if let TreeKind::Parens { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(expr);
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(tree);
    }

    /// Java: public void visitAssign(JCAssign tree) (TransTypes.java).
    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("assign expected"),
        };
        let lhs = self.translate_with_pt(ctx, lhs, None);
        let lhs_ty = lhs.and_then(|lhs| ctx.tree(lhs).ty).expect("lhs type");
        let erased = self.erasure(ctx, lhs_ty);
        let rhs = self.translate_with_pt(ctx, rhs, Some(erased));
        if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(self.retype(ctx, tree, erased, self.pt));
    }

    /// Java: public void visitAssignop(JCAssignOp tree) (TransTypes.java).
    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, operator) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, operator } => (*lhs, *rhs, *operator),
            _ => panic!("assignop expected"),
        };
        let lhs = self.translate_with_pt(ctx, lhs, None);
        let operator = operator.expect("operator symbol");
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let params = self.types.get_parameter_types_public(ctx, operator_ty);
        let rhs_pt = Some(*params.tail().head());
        let rhs = self.translate_with_pt(ctx, rhs, rhs_pt);
        let tree_ty = ctx.tree(tree).ty.expect("assignop type");
        let erased = self.erasure(ctx, tree_ty);
        if let TreeKind::AssignOp { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(tree);
    }

    /// Java: public void visitUnary(JCUnary tree) (TransTypes.java).
    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (arg, operator) = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, operator } => (*arg, *operator),
            _ => panic!("unary expected"),
        };
        let operator = operator.expect("operator symbol");
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let params = self.types.get_parameter_types_public(ctx, operator_ty);
        let first_param = Some(*params.head());
        let pt = if ctx.tree(tree).has_tag(Tag::NULLCHK) {
            ctx.tree(tree).ty
        } else {
            first_param
        };
        let arg = self.translate_with_pt(ctx, arg, pt);
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = arg;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitBinary(JCBinary tree) (TransTypes.java).
    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, operator) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, operator } => (*lhs, *rhs, *operator),
            _ => panic!("binary expected"),
        };
        let operator = operator.expect("operator symbol");
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let params = self.types.get_parameter_types_public(ctx, operator_ty);
        let lhs_pt = Some(*params.head());
        let rhs_pt = Some(*params.tail().head());
        let lhs = self.translate_with_pt(ctx, lhs, lhs_pt);
        let rhs = self.translate_with_pt(ctx, rhs, rhs_pt);
        if let TreeKind::Binary { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitAnnotatedType(JCAnnotatedType tree) (TransTypes.java).
    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType { annotations, underlying_type } => (annotations.clone(), *underlying_type),
            _ => panic!("annotated type expected"),
        };
        let mirrors = self.annotate.from_annotations(ctx, &annotations);
        let underlying_type = self.translate_with_pt(ctx, underlying_type, None);
        let underlying_ty = underlying_type.and_then(|underlying| ctx.tree(underlying).ty);
        let new_ty = underlying_ty.map(|ty| ctx.alloc_type(ctx.typ(ty).annotated_type(&mirrors)));
        if let TreeKind::AnnotatedType { underlying_type: u, .. } = &mut ctx.tree_mut(tree).kind {
            *u = underlying_type;
        }
        ctx.tree_mut(tree).ty = new_ty;
        self.result = Some(tree);
    }

    /// Java: public void visitTypeCast(JCTypeCast tree) (TransTypes.java).
    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("type cast expected"),
        };
        let clazz = self.translate_with_pt(ctx, clazz, None);
        let original_target = ctx.tree(tree).ty.expect("cast type");
        let erased = self.erasure(ctx, original_target);
        let new_expr = self.translate_with_pt(ctx, expr, Some(erased)).expect("cast expr");
        let mut final_expr = new_expr;
        if final_expr != expr.expect("cast expr") {
            let cast_ty = if ctx.tree(final_expr).has_tag(Tag::TYPECAST) {
                ctx.tree(final_expr).ty
            } else {
                None
            };
            if let Some(cast_ty) = cast_ty {
                let syms = self.syms.borrow();
                if self.types.is_same_type(ctx, &syms, cast_ty, erased) {
                    drop(syms);
                    final_expr = match &ctx.tree(final_expr).kind {
                        TreeKind::TypeCast { expr: Some(expr), .. } => *expr,
                        _ => final_expr,
                    };
                }
            }
        }
        if ctx.typ(original_target).is_intersection() {
            let components = ctx.typ(original_target).get_explicit_components();
            let mut current = components;
            while current.non_empty() {
                let component = *current.head();
                let ec = self.erasure(ctx, component);
                let syms = self.syms.borrow();
                let same_as_cast = self.types.is_same_type(ctx, &syms, ec, erased);
                let same_as_pt = match self.pt {
                    Some(pt) => self.types.is_same_type(ctx, &syms, ec, pt),
                    None => false,
                };
                drop(syms);
                if !same_as_cast && !same_as_pt {
                    final_expr = self.coerce_tree(ctx, final_expr, ec);
                }
                current = current.tail();
            }
        }
        if let TreeKind::TypeCast { clazz: c, expr: e } = &mut ctx.tree_mut(tree).kind {
            *c = clazz;
            *e = Some(final_expr);
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(self.retype(ctx, tree, erased, self.pt));
    }

    /// Java: public void visitTypeTest(JCInstanceOf tree) (TransTypes.java).
    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("instanceof expected"),
        };
        let expr = self.translate_with_pt(ctx, expr, None);
        let pattern = self.translate_with_pt(ctx, pattern, None);
        if let TreeKind::InstanceOf { expr: e, pattern: p } = &mut ctx.tree_mut(tree).kind {
            *e = expr;
            *p = pattern;
        }
        self.result = Some(tree);
    }

    /// Java: public void visitIndexed(JCArrayAccess tree) (TransTypes.java).
    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("indexed expected"),
        };
        let indexed_pt = indexed.and_then(|indexed| ctx.tree(indexed).ty).map(|ty| self.erasure(ctx, ty));
        let indexed = self.translate_with_pt(ctx, indexed, indexed_pt);
        let int_ty = self.syms.borrow().int_type;
        let index = self.translate_with_pt(ctx, index, Some(int_ty));
        let indexed_ty = indexed.and_then(|indexed| ctx.tree(indexed).ty).expect("indexed type");
        let elem = {
            let syms = self.syms.borrow();
            self.types.elemtype(ctx, &syms, indexed_ty)
        };
        if let TreeKind::ArrayAccess { indexed: i, index: n } = &mut ctx.tree_mut(tree).kind {
            *i = indexed;
            *n = index;
        }
        self.result = Some(self.retype(ctx, tree, elem, self.pt));
    }

    /// Java: public void visitAnnotation(JCAnnotation tree) (TransTypes.java).
    fn visit_annotation(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitIdent(JCIdent tree) (TransTypes.java).
    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = tree_info::symbol(ctx, tree).expect("ident symbol");
        let et = self.symbol_erasure(ctx, sym);
        let sym_ty = ctx.sym(sym).ty.expect("symbol type");
        if ctx.sym(sym).kind == Kind::TYP && ctx.typ(sym_ty).has_tag(TypeTag::TYPEVAR) {
            // Java: result = make.at(tree.pos).Type(et);
            let pos = ctx.tree(tree).pos;
            let type_tree = {
                self.make.borrow_mut().at(pos).type_from_type(ctx, &self.syms.borrow(), et)
            };
            self.result = Some(type_tree);
        } else if ctx.typ(ctx.tree(tree).ty.expect("ident type")).const_value().is_some() {
            self.result = Some(tree);
        } else if ctx.sym(sym).kind == Kind::VAR {
            self.result = Some(self.retype(ctx, tree, et, self.pt));
        } else {
            let tree_ty = ctx.tree(tree).ty.expect("ident type");
            let erased = self.erasure(ctx, tree_ty);
            ctx.tree_mut(tree).ty = Some(erased);
            self.result = Some(tree);
        }
    }

    /// Java: public void visitSelect(JCFieldAccess tree) (TransTypes.java).
    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selected, sym) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, sym, .. } => (*selected, *sym),
            _ => panic!("field access expected"),
        };
        let selected_ty = selected.and_then(|selected| ctx.tree(selected).ty).expect("selected type");
        let t = self.types.skip_type_vars_public(ctx, selected_ty);
        let is_compound = self.types.is_compound_public(ctx, t);
        let translated = if is_compound {
            let sym = sym.expect("field symbol");
            let owner = ctx.sym(sym).owner.expect("field owner");
            let owner_ty = ctx.sym(owner).ty.expect("field owner type");
            let owner_erasure = self.erasure(ctx, owner_ty);
            let original_erasure = self.erasure(ctx, selected_ty);
            let inner = self.translate_with_pt(ctx, selected, Some(original_erasure)).expect("selected");
            Some(self.coerce_tree(ctx, inner, owner_erasure))
        } else {
            let t_erasure = self.erasure(ctx, t);
            self.translate_with_pt(ctx, selected, Some(t_erasure))
        };
        let tree_ty = ctx.tree(tree).ty.expect("field type");
        if ctx.typ(tree_ty).const_value().is_some() {
            self.result = Some(tree);
        } else if sym.map(|sym| ctx.sym(sym).kind == Kind::VAR).unwrap_or(false) {
            let sym = sym.expect("field symbol");
            let et = self.symbol_erasure(ctx, sym);
            if let TreeKind::FieldAccess { selected: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = translated;
            }
            self.result = Some(self.retype(ctx, tree, et, self.pt));
        } else {
            let erased = self.erasure(ctx, tree_ty);
            if let TreeKind::FieldAccess { selected: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = translated;
            }
            ctx.tree_mut(tree).ty = Some(erased);
            self.result = Some(tree);
        }
    }

    /// Java: public void visitReference(JCMemberReference tree) (TransTypes.java).
    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, kind, sym, varargs_element, tree_ty) = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, kind, sym, varargs_element, .. } => {
                (*expr, *kind, *sym, *varargs_element, ctx.tree(tree).ty)
            }
            _ => panic!("member reference expected"),
        };
        let expr_ty = expr.and_then(|expr| ctx.tree(expr).ty).expect("reference expr type");
        let t = self.types.skip_type_vars_public(ctx, expr_ty);
        let is_compound = self.types.is_compound_public(ctx, t);
        let receiver_target = if is_compound {
            let sym = sym.expect("reference symbol");
            let owner = ctx.sym(sym).owner.expect("reference owner");
            let owner_ty = ctx.sym(owner).ty.expect("reference owner type");
            self.erasure(ctx, owner_ty)
        } else {
            self.erasure(ctx, t)
        };
        let is_unbound = matches!(kind, Some(crate::tree::jc_tree::ReferenceKind::UNBOUND));
        let expr = if is_unbound {
            let pos = ctx.tree(tree).pos;
            let type_tree = {
                self.make.borrow_mut().at(pos).type_from_type(ctx, &self.syms.borrow(), receiver_target)
            };
            Some(type_tree)
        } else {
            self.translate_with_pt(ctx, expr, Some(receiver_target))
        };
        let new_ty = match tree_ty {
            Some(ty) => {
                if !ctx.typ(ty).is_intersection() {
                    Some(self.erasure(ctx, ty))
                } else {
                    let tsym = ctx.typ(ty).tsym.expect("intersection symbol");
                    let desc = {
                        let diags = Rc::clone(&self.diags);
                        let chk = Rc::clone(&self.chk);
                        let chk = chk.borrow();
                        let syms = self.syms.borrow_mut();
                        self.types
                            .find_descriptor_symbol(ctx, &syms, &diags, &chk, tsym)
                            .expect("descriptor symbol")
                    };
                    let owner = ctx.sym(desc).owner.expect("descriptor owner");
                    let owner_ty = ctx.sym(owner).ty.expect("descriptor owner type");
                    Some(self.erasure(ctx, owner_ty))
                }
            }
            None => None,
        };
        let varargs_element = varargs_element.map(|ty| self.erasure(ctx, ty));
        if let TreeKind::MemberReference { expr: e, varargs_element: v, .. } = &mut ctx.tree_mut(tree).kind {
            *e = expr;
            *v = varargs_element;
        }
        ctx.tree_mut(tree).ty = new_ty;
        self.result = Some(tree);
    }

    /// Java: public void visitTypeArray(JCArrayTypeTree tree) (TransTypes.java).
    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("array type expected"),
        };
        let elemtype = self.translate_with_pt(ctx, elemtype, None);
        let tree_ty = ctx.tree(tree).ty.expect("array type");
        let erased = self.erasure(ctx, tree_ty);
        if let TreeKind::ArrayTypeTree { elemtype: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = elemtype;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(tree);
    }

    /// Java: public void visitTypeApply(JCTypeApply tree) (TransTypes.java).
    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let clazz = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, .. } => *clazz,
            _ => panic!("parameterized type expected"),
        };
        let translated = self.translate_with_pt(ctx, clazz, None);
        self.result = translated;
    }

    /// Java: public void visitTypeIntersection(JCTypeIntersection tree)
    /// (TransTypes.java).
    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("intersection type expected"),
        };
        let bounds = self.translate_list_with_pt(ctx, &bounds, None);
        let tree_ty = ctx.tree(tree).ty.expect("intersection type");
        let erased = self.erasure(ctx, tree_ty);
        if let TreeKind::TypeIntersection { bounds: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = bounds;
        }
        ctx.tree_mut(tree).ty = Some(erased);
        self.result = Some(tree);
    }


    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::CompilationUnit { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
        self.result = Some(tree);
    }

    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.translate_list(ctx, &annotations);
        let pid = self.translate(ctx, pid);
        if let TreeKind::PackageDecl {
            annotations: a,
            pid: p,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *p = pid;
        }
        self.result = Some(tree);
    }

    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Import { qualid: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = stats;
        }
        self.result = Some(tree);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let body = self.translate(ctx, body);
        if let TreeKind::LabeledStatement { body: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = body;
        }
        self.result = Some(tree);
    }

    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.translate(ctx, param);
        let body = self.translate(ctx, body);
        if let TreeKind::Catch { param: p, body: b } = &mut ctx.tree_mut(tree).kind {
            *p = param;
            *b = body;
        }
        self.result = Some(tree);
    }

    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.translate_list(ctx, &alternatives);
        if let TreeKind::TypeUnion { alternatives: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = alternatives;
        }
        self.result = Some(tree);
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                annotations,
                bounds,
                ..
            } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeParameter {
            annotations: a,
            bounds: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *b = bounds;
        }
        self.result = Some(tree);
    }

    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = self.translate(ctx, kind);
        let inner = self.translate(ctx, inner);
        if let TreeKind::Wildcard { kind: k, inner: i } = &mut ctx.tree_mut(tree).kind {
            *k = kind;
            *i = inner;
        }
        self.result = Some(tree);
    }

    fn visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        if let TreeKind::Modifiers {
            annotations: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = annotations;
        }
        self.result = Some(tree);
    }

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, &defs);
        let expr = self.translate(ctx, expr);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }

    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                qual_id,
                directives,
                ..
            } => (*mods, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.translate(ctx, mods);
        let qual_id = self.translate(ctx, qual_id);
        let directives = directives.map(|directives| self.translate_list(ctx, &directives));
        if let TreeKind::ModuleDecl {
            mods: m,
            qual_id: q,
            directives: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *q = qual_id;
            *d = directives;
        }
        self.result = Some(tree);
    }

    fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an exports directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Exports {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }

    fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an opens directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Opens {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }

    fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (service_name, impl_names) = match &ctx.tree(tree).kind {
            TreeKind::Provides {
                service_name,
                impl_names,
                ..
            } => (*service_name, impl_names.clone()),
            _ => panic!("expected a provides directive"),
        };
        let service_name = self.translate(ctx, service_name);
        let impl_names = self.translate_list(ctx, &impl_names);
        if let TreeKind::Provides {
            service_name: s,
            impl_names: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = service_name;
            *i = impl_names;
        }
        self.result = Some(tree);
    }

    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.translate(ctx, module_name);
        if let TreeKind::Requires {
            module_name: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = module_name;
        }
        self.result = Some(tree);
    }

    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Uses { qualid: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    /// Java: public void visitSkip(JCSkip tree) (TreeTranslator.java).
    fn visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitContinue(JCContinue tree) (TreeTranslator.java).
    fn visit_continue(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitDefaultCaseLabel(JCDefaultCaseLabel tree)
    /// (TreeTranslator.java).
    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitLiteral(JCLiteral tree) (TreeTranslator.java).
    fn visit_literal(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitTypeIdent(JCPrimitiveTypeTree tree)
    /// (TreeTranslator.java).
    fn visit_type_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitErroneous(JCErroneous tree) (TreeTranslator.java).
    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Java: public void visitTree(JCTree tree) (TreeTranslator.java).
    fn visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: visitTree {:?}", tree);
    }
}
