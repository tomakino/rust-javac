// Origin: com/sun/tools/javac/comp/MemberEnter.java @ 982d53c (jdk-17.0.19-ga)
//
// Registration:
// Ported in this file: the MemberEnter structure, signature(), memberEnter (both
// overloads), visitMethodDef, methodEnv, visitVarDef, checkType, checkReceiver,
// needsLazyConstValue + InitTreeVisitor, initEnv, visitTree, visitErroneous,
// getMethodEnv, getInitEnv.
//
// NOT PORTED IN THIS STEP (registered; dependencies not ported yet, no
// substitute in this file):
// - Attr: every attr.attribTypeVariables/attribType/attribExpr/
// attribIdentAsEnumType call is routed through explicit unimplemented helpers
// (signature/visitVarDef/checkReceiver), because the Attr is not ported;
// - Annotate: annotate.annotateLater/queueScanTreeAndTypeAnnotate/
// annotateDefaultValueLater/unfinishedDefaultValue calls are TODO (Annotate
//);
// - DeferredLintHandler is ported (code/deferred_lint_handler.rs) and used for
// the setPos save/restore exactly like upstream;
// - AttrContext.returnResult/yieldResult are not represented yet (Attr,
// registered in AttrContext);
// - initEnv uses AttrContextEnv (an Attr inner class): initEnv/getInitEnv are
// explicitly unimplemented (Attr);
// - ArrayType.makeVarargs and VarSymbol.setLazyConstValue are not ported
// (types/symbol batches): the VARARGS parameter type refinement and the lazy
// constant-value computation are TODO in visitVarDef;
// - getMethodEnv mEnv.info.lint.augment(sym) is TODO (Lint), the scope
// enterIfAbsent steps are ported.
//
// The MemberEnter entry point used by TypeEnter (TypeEnter.complete /
// visitTopLevel / visitClassDef / default constructor / import processing) is in
// comp/TypeEnter.java upstream (NOT in MemberEnter.java); the TypeEnter is
// registered separately (the Enter leaves the TypeEnter completer TODO).

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::deferred_lint_handler::DeferredLintHandler;
use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{CompletionFailure, Symbol, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::type_::{Type, TypeKindData};
use crate::code::types::Types;
use crate::comp::annotate::Annotate;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::comp::enter::Enter;
use crate::comp::env::Env;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_info;
use crate::util::abstract_log::AbstractLog;
use crate::util::jc_diagnostic::SimpleDiagnosticPosition;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;

/// Java: public class MemberEnter (see the file header for the ported subset).
pub struct MemberEnter {
    pub enter: Rc<RefCell<Enter>>,
    pub log: Rc<RefCell<Log>>,
    pub chk: Rc<RefCell<Check>>,
    pub types: Rc<Types>,
    pub deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
    /// Java: protected Env<AttrContext> env;
    pub env: Option<Rc<Env<AttrContext>>>,
    /// Java: Annotate annotate = Annotate.instance(context) — explicit
    /// injection adapter (ADR 0001): the cell is filled after Annotate is
    /// constructed (Rc cycle MemberEnter/TypeEnter -> Annotate -> Attr ->
    /// MemberEnter/TypeEnter; registered in the report).
    pub annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
}

impl MemberEnter {
    /// Java: protected MemberEnter(Context context) — injected dependencies.
    pub fn new(
        enter: Rc<RefCell<Enter>>,
        log: Rc<RefCell<Log>>,
        chk: Rc<RefCell<Check>>,
        types: Rc<Types>,
        deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
        annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
    ) -> MemberEnter {
        MemberEnter {
            enter,
            log,
            chk,
            types,
            deferred_lint_handler,
            env: None,
            annotate,
        }
    }

    /// Java: Type signature(MethodSymbol msym, List<JCTypeParameter> typarams,
    /// List<JCVariableDecl> params, JCTree res, JCVariableDecl recvparam,
    /// List<JCExpression> thrown, Env<AttrContext> env)
    #[allow(clippy::too_many_arguments)]
    pub fn signature(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        msym: SymbolId,
        typarams: List<TreeId>,
        params: List<TreeId>,
        res: Option<TreeId>,
        recvparam: Option<TreeId>,
        thrown: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        // Enter and attribute type parameters.
        let tvars = self.enter.borrow_mut().class_enter_list(
            ctx,
            syms,
            typarams.clone(),
            Some(Rc::clone(env)),
        );
        self.attrib_type_variables(ctx, syms, typarams, env);

        // Enter and attribute value parameters.
        let mut argbuf: List<TypeId> = List::nil();
        let mut current = params;
        while current.non_empty() {
            let param = *current.head();
            self.member_enter(ctx, syms, param, env);
            argbuf = argbuf.append(var_decl_type(ctx, param).expect("parameter type"));
            current = current.tail();
        }

        // Attribute result type, if one is given.
        let restype = match res {
            None => syms.void_type,
            Some(res) => self.attrib_type(ctx, syms, res, env),
        };

        // Attribute receiver type, if one is given.
        let recvtype = match recvparam {
            None => None,
            Some(recvparam) => {
                self.member_enter(ctx, syms, recvparam, env);
                var_decl_type(ctx, recvparam)
            }
        };

        // Attribute thrown exceptions.
        let mut thrownbuf: List<TypeId> = List::nil();
        let mut current = thrown;
        while current.non_empty() {
            let thrown_tree = *current.head();
            let mut exc = self.attrib_type(ctx, syms, thrown_tree, env);
            if !ctx.typ(exc).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
                exc = self
                    .chk
                    .borrow()
                    .check_class_type(ctx, ctx.tree(thrown_tree).pos, exc);
            } else if ctx
                .typ(exc)
                .tsym
                .and_then(|tsym| ctx.sym(tsym).owner)
                == Some(msym)
            {
                // mark inference variables in 'throws' clause
                let flags_now = ctx.sym(msym).flags_field;
                let _ = flags_now;
                if let Some(tsym) = ctx.typ(exc).tsym {
                    ctx.sym_mut(tsym).flags_field |= flags::THROWS;
                }
            }
            thrownbuf = thrownbuf.append(exc);
            current = current.tail();
        }
        let mtype = ctx.alloc_type(Type::new_method_type(
            argbuf,
            restype,
            thrownbuf,
            syms.method_class,
        ));
        if let TypeKindData::MethodType { recvtype: slot, .. } = &mut ctx.typ_mut(mtype).kind {
            *slot = recvtype;
        }
        if tvars.is_empty() {
            mtype
        } else {
            ctx.alloc_type(Type::new_for_all(tvars, mtype))
        }
    }

    // Attr dependencies (registered TODO: Attr).
    fn attrib_type_variables(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _typarams: List<TreeId>,
        _env: &Rc<Env<AttrContext>>,
    ) {
        unimplemented!("TODO(Attr batch): MemberEnter.signature attr.attribTypeVariables")
    }

    fn attrib_type(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        unimplemented!("TODO(Attr batch): MemberEnter Attr.attribType")
    }

    fn attrib_expr(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        unimplemented!("TODO(Attr batch): MemberEnter Attr.attribExpr")
    }

    fn attrib_ident_as_enum_type(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) {
        unimplemented!("TODO(Attr batch): MemberEnter Attr.attribIdentAsEnumType")
    }

    /// Java: protected void memberEnter(JCTree tree, Env<AttrContext> env)
    pub fn member_enter(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) {
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(env));
        let result = self.visit(ctx, syms, tree);
        if let Err(failure) = result {
            // Java: chk.completionError(tree.pos(), ex)
            let _ = self.chk.borrow().completion_error(
                ctx,
                ctx.tree(tree).pos,
                &failure,
                syms,
            );
        }
        self.env = prev_env;
    }

    /// Java: void memberEnter(List<? extends JCTree> trees, Env<AttrContext> env)
    pub fn member_enter_list(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
    ) {
        let mut current = trees;
        while current.non_empty() {
            let tree = *current.head();
            self.member_enter(ctx, syms, tree, env);
            current = current.tail();
        }
    }

    fn visit(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        match ctx.tree(tree).tag {
            Tag::METHODDEF => self.visit_method_def(ctx, syms, tree),
            Tag::VARDEF => self.visit_var_def(ctx, syms, tree),
            Tag::ERRONEOUS => self.visit_erroneous(ctx, syms, tree),
            _ => self.visit_tree(),
        }
    }

    /// Java: public void visitMethodDef(JCMethodDecl tree)
    pub fn visit_method_def(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        let env = self.env.clone().expect("env");
        let encl_scope = self.enter.borrow().enter_scope(ctx, &env);
        let owner = ctx.scope(encl_scope).owner.expect("enclosing scope owner");
        let name = method_decl_name(ctx, tree);
        let m = ctx.alloc_symbol(Symbol::new(
            Kind::MTH,
            0,
            name,
            None,
            Some(owner),
            SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
            },
        ));
        let mods_flags = method_decl_mods_flags(ctx, tree);
        let checked = self.chk.borrow().check_flags(
            ctx,
            ctx.tree(tree).pos,
            mods_flags,
            m,
            tree,
        );
        ctx.sym_mut(m).flags_field = checked;
        set_method_decl_symbol(ctx, tree, m);
        // if this is a default method, add the DEFAULT flag to the enclosing interface.
        if (mods_flags & flags::DEFAULT) != 0 {
            ctx.sym_mut(owner).flags_field |= flags::DEFAULT;
        }
        let local_env = self.method_env(ctx, tree, &env);
        let prev_lint = self.deferred_lint_handler.borrow_mut().set_pos(Rc::new(
            SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
        ));
        let signature = self.signature(
            ctx,
            syms,
            m,
            method_decl_typarams(ctx, tree),
            method_decl_params(ctx, tree),
            method_decl_restype(ctx, tree),
            method_decl_recvparam(ctx, tree),
            method_decl_thrown(ctx, tree),
            &local_env,
        );
        self.deferred_lint_handler.borrow_mut().set_pos_state(prev_lint);
        ctx.sym_mut(m).ty = Some(signature);
        if self.types.is_signature_polymorphic(ctx, syms, m) {
            ctx.sym_mut(m).flags_field |= flags::SIGNATURE_POLYMORPHIC;
        }
        // Set m.params
        let mut params_list: List<SymbolId> = List::nil();
        let mut last_param: Option<TreeId> = None;
        let mut current = method_decl_params(ctx, tree);
        while current.non_empty() {
            let param = *current.head();
            last_param = Some(param);
            params_list = params_list.append(var_decl_symbol(ctx, param).expect("parameter symbol"));
            current = current.tail();
        }
        ctx.sym_mut(m).set_params(params_list);
        // mark the method varargs, if necessary
        let last_param_varargs = last_param
            .map(|last_param| (var_decl_mods_flags(ctx, last_param) & flags::VARARGS) != 0)
            .unwrap_or(false);
        if last_param_varargs {
            ctx.sym_mut(m).flags_field |= flags::VARARGS;
        }
        // localEnv.info.scope.leave();
        let local_scope = local_env.info.borrow().scope.expect("local scope");
        let _ = ctx.scope_leave(local_scope);
        let unique = self.chk.borrow().check_unique(
            ctx,
            ctx.tree(tree).pos,
            m,
            encl_scope,
        );
        if unique {
            ctx.scope_enter(encl_scope, m);
        }
        let annotate = self
            .annotate
            .borrow()
            .clone()
            .expect("MemberEnter.annotate must be injected");
        annotate.annotate_later(
            ctx,
            method_decl_mods_annotations(ctx, tree),
            Rc::clone(&local_env),
            m,
            Some(ctx.tree(tree).pos),
        );
        // Visit the signature of the method. Note that TypeAnnotate doesn't
        // descend into the body.
        annotate.services.queue_scan_tree_and_type_annotate(
            tree,
            &local_env,
            m,
            ctx.tree(tree).pos,
        );
        if let Some(default_value) = method_decl_default_value(ctx, tree) {
            // m.defaultValue = annotate.unfinishedDefaultValue(); (temporary
            // sentinel for now)
            if let crate::code::symbol::SymbolKind::MethodSymbol { default_value: slot, .. } =
                &mut ctx.sym_mut(m).kind_data
            {
                *slot = Some(annotate.unfinished_default_value());
            }
            annotate.annotate_default_value_later(
                ctx,
                default_value,
                Rc::clone(&local_env),
                m,
                Some(ctx.tree(tree).pos),
            );
        }
        Ok(())
    }

    /// Java: Env<AttrContext> methodEnv(JCMethodDecl tree, Env<AttrContext> env)
    pub fn method_env(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        let method = method_decl_symbol(ctx, tree).expect("method symbol");
        let parent_scope = env.info.borrow().scope.expect("env.info.scope");
        let local_scope = ctx.scope_dup_unshared(parent_scope, Some(method));
        let mut info = env.info.borrow().dup(Some(local_scope));
        if ctx.sym(method).ty.is_some() {
            // TODO(Attr): localEnv.info.returnResult = attr.new ResultInfo(
            // KindSelector.VAL, tree.sym.type.getReturnType());
        }
        if (method_decl_mods_flags(ctx, tree) & flags::STATIC as i64) != 0 {
            info.static_level += 1;
        }
        // TODO(Attr): localEnv.info.yieldResult = null;
        Rc::new(Env {
            next: None,
            outer: Some(Rc::clone(env)),
            tree: Some(tree),
            toplevel: env.toplevel,
            encl_class: env.encl_class,
            encl_method: Some(tree),
            info: Rc::new(RefCell::new(info)),
            base_clause: env.base_clause,
        })
    }

    /// Java: public void visitVarDef(JCVariableDecl tree)
    pub fn visit_var_def(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        let env = self.env.clone().expect("env");
        let mods_flags = var_decl_mods_flags(ctx, tree);
        let scope_owner = env.info.borrow().scope.expect("scope");
        let owner = ctx.scope(scope_owner).owner.expect("scope owner");
        let mut local_env = Rc::clone(&env);
        if (mods_flags & flags::STATIC as i64) != 0
            || (ctx.sym(owner).flags_field & flags::INTERFACE as i64) != 0
        {
            let mut info = env.info.borrow().dup_ctx();
            info.static_level += 1;
            local_env = Rc::new(Env {
                next: None,
                outer: env.outer.clone(),
                tree: Some(tree),
                toplevel: env.toplevel,
                encl_class: env.encl_class,
                encl_method: env.encl_method,
                info: Rc::new(RefCell::new(info)),
                base_clause: env.base_clause,
            });
        }
        let prev_lint = self.deferred_lint_handler.borrow_mut().set_pos(Rc::new(
            SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
        ));
        {
            if tree_info::is_enum_init(ctx, tree) {
                let vartype = var_decl_vartype(ctx, tree).expect("enum init vartype");
                self.attrib_ident_as_enum_type(ctx, syms, vartype, &local_env);
            } else if !is_implicitly_typed(ctx, tree) {
                let vartype = var_decl_vartype(ctx, tree).expect("variable type");
                let ty = self.attrib_type(ctx, syms, vartype, &local_env);
                set_tree_type(ctx, vartype, ty);
                if tree_info::is_receiver_param(ctx, tree) {
                    self.check_receiver(ctx, syms, tree, &local_env);
                }
            }
        }
        self.deferred_lint_handler.borrow_mut().set_pos_state(prev_lint);
        if (mods_flags & flags::VARARGS) != 0 {
            // TODO(Types): replace the plain array type with
            // atype.makeVarargs() (ArrayType.makeVarargs is not ported).
        }
        let encl_scope = self.enter.borrow().enter_scope(ctx, &env);
        let vartype = if is_implicitly_typed(ctx, tree) {
            if ctx.sym(owner).kind == Kind::MTH {
                crate::code::symtab::no_type_id(ctx)
            } else {
                syms.err_type
            }
        } else {
            var_decl_type(ctx, tree).expect("variable type")
        };
        let name = var_decl_name(ctx, tree);
        let v = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            name,
            Some(vartype),
            Some(owner),
            SymbolKind::VarSymbol {
                pos: 0,
                adr: -1,
                data_kind: None,
                data: None,
            },
        ));
        let checked = self.chk.borrow().check_flags(
            ctx,
            ctx.tree(tree).pos,
            mods_flags,
            v,
            tree,
        );
        ctx.sym_mut(v).flags_field = checked;
        set_var_decl_symbol(ctx, tree, v);
        if var_decl_init(ctx, tree).is_some() {
            ctx.sym_mut(v).flags_field |= flags::HASINIT as i64;
            if (ctx.sym(v).flags_field & flags::FINAL as i64) != 0
                && self.needs_lazy_const_value(ctx, var_decl_init(ctx, tree).expect("initializer"))
            {
                // TODO(Symbol/Attr): v.setLazyConstValue(initEnv(tree,
                // getInitEnv(tree, env)), env, attr, tree); setLazyConstValue is
                // not ported and initEnv needs AttrContextEnv (Attr).
            }
        }
        let unique = self
            .chk
            .borrow()
            .check_unique(ctx, ctx.tree(tree).pos, v, encl_scope);
        if unique {
            self.chk
                .borrow()
                .check_transparent_var(ctx, ctx.tree(tree).pos, v, encl_scope);
            ctx.scope_enter(encl_scope, v);
        } else if ctx.sym(v).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH)
            || (ctx.sym(v).flags_field
                & (flags::PRIVATE as i64
                    | flags::FINAL as i64
                    | flags::GENERATED_MEMBER as i64
                    | flags::RECORD))
                != 0
        {
            // if this is a parameter or a field obtained from a record component,
            // enter it
            ctx.scope_enter(encl_scope, v);
        }
        let annotate = self
            .annotate
            .borrow()
            .clone()
            .expect("MemberEnter.annotate must be injected");
        annotate.annotate_later(
            ctx,
            var_decl_mods_annotations(ctx, tree),
            Rc::clone(&local_env),
            v,
            Some(ctx.tree(tree).pos),
        );
        if let Some(vartype) = var_decl_vartype(ctx, tree) {
            annotate.services.queue_scan_tree_and_type_annotate(
                vartype,
                &local_env,
                v,
                ctx.tree(tree).pos,
            );
        }
        let pos = ctx.tree(tree).pos;
        set_var_symbol_pos(ctx, v, pos);
        Ok(())
    }

    /// Java: void checkType(JCTree tree, Type type, Error errorKey)
    ///
    /// TODO(Attr): the upstream call sites are checkReceiver's
    /// IncorrectConstructorReceiverType/IncorrectReceiverType checks, which are
    /// TODO until Attr is ported; the method is kept for that.
    #[allow(dead_code)]
    fn check_type(
        &self,
        _ctx: &mut CompilerContext,
        _tree: TreeId,
        _expected: TypeId,
        _error: crate::resources::diagnostic_contract::ErrorInfo,
    ) {
        // TODO(Attr/Check): the upstream body is
        // if (!tree.type.isErroneous() && !types.isSameType(tree.type, type))
        // log.error(tree, errorKey);
        // Types.isSameType needs the Symtab argument in the port (explicit
        // context adapter); checkReceiver therefore cannot call it before the
        // Symtab is threaded (registered).
        unimplemented!("TODO(Attr batch): MemberEnter.checkType (Types.isSameType + Symtab)")
    }

    /// Java: void checkReceiver(JCVariableDecl tree, Env<AttrContext> localEnv)
    fn check_receiver(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        local_env: &Rc<Env<AttrContext>>,
    ) {
        if let Some(nameexpr) = var_decl_nameexpr(ctx, tree) {
            let _ = self.attrib_expr(ctx, syms, nameexpr, local_env);
        }
        let encl_method = local_env.encl_method.expect("enclosing method");
        let m = method_decl_symbol(ctx, encl_method).expect("method symbol");
        let is_constructor = ctx.sym(m).is_constructor(&crate::util::names::Names::new(false));
        if is_constructor {
            let owner = ctx.sym(m).owner.expect("method owner");
            let owner_owner = ctx.sym(owner).owner.expect("owner owner");
            let mut outer_type = ctx.sym(owner_owner).ty;
            if outer_type
                .map(|ty| ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::METHOD))
                .unwrap_or(false)
            {
                // we have a local inner class
                let owner_owner_owner = ctx.sym(owner_owner).owner.expect("owner chain");
                outer_type = ctx.sym(owner_owner_owner).ty;
            }
            let outer_type = outer_type.expect("outer type");
            if ctx
                .typ(outer_type)
                .has_tag(crate::code::type_tag::TypeTag::CLASS)
            {
                // TODO(Attr): checkType(tree.vartype, outertype,
                // Errors.IncorrectConstructorReceiverType(...));
                // TODO(Attr): checkType(tree.nameexpr, outertype,
                // Errors.IncorrectConstructorReceiverName(...));
            } else {
                self.log.borrow_mut().error_at(
                    ctx.tree(tree).pos,
                    crate::util::jc_diagnostic::Error::from(
                        crate::resources::compiler_properties::errors::receiver_parameter_not_applicable_constructor_toplevel_class(),
                    ),
                );
            }
        } else {
            // TODO(Attr): checkType(tree.vartype, m.owner.type,
            // Errors.IncorrectReceiverType(...));
            // TODO(Attr): checkType(tree.nameexpr, m.owner.type,
            // Errors.IncorrectReceiverName(...));
        }
    }

    /// Java: public boolean needsLazyConstValue(JCTree tree)
    pub fn needs_lazy_const_value(&self, ctx: &CompilerContext, tree: TreeId) -> bool {
        init_tree_visitor(ctx, tree)
    }

    /// Java: Env<AttrContext> initEnv(JCVariableDecl tree, Env<AttrContext> env)
    pub fn init_env(
        &mut self,
        _ctx: &mut CompilerContext,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        // Java: env.dupto(new AttrContextEnv(tree, env.info.dup()));
        unimplemented!("TODO(Attr batch): MemberEnter.initEnv needs AttrContextEnv (Attr inner class)")
    }

    /// Java: public void visitTree(JCTree tree) { }
    fn visit_tree(&mut self) -> Result<(), CompletionFailure> {
        Ok(())
    }

    /// Java: public void visitErroneous(JCErroneous tree)
    fn visit_erroneous(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        let errs = match &ctx.tree(tree).kind {
            TreeKind::Erroneous { errs } => errs.clone(),
            _ => List::nil(),
        };
        if !errs.is_empty() {
            let env = self.env.clone().expect("env");
            self.member_enter_list(ctx, syms, errs, &env);
        }
        Ok(())
    }

    /// Java: public Env<AttrContext> getMethodEnv(JCMethodDecl tree,
    /// Env<AttrContext> env)
    pub fn get_method_env(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        let m_env = self.method_env(ctx, tree, env);
        // TODO(Lint): mEnv.info.lint = mEnv.info.lint.augment(tree.sym);
        let scope = m_env.info.borrow().scope.expect("method env scope");
        let mut typarams = method_decl_typarams(ctx, tree);
        while typarams.non_empty() {
            let typaram = *typarams.head();
            if let Some(tsym) = ctx.tree(typaram).ty.and_then(|ty| ctx.typ(ty).tsym) {
                ctx.scope_enter_if_absent(scope, tsym);
            }
            typarams = typarams.tail();
        }
        let mut params = method_decl_params(ctx, tree);
        while params.non_empty() {
            let param = *params.head();
            if let Some(sym) = var_decl_symbol(ctx, param) {
                ctx.scope_enter_if_absent(scope, sym);
            }
            params = params.tail();
        }
        m_env
    }

    /// Java: public Env<AttrContext> getInitEnv(JCVariableDecl tree,
    /// Env<AttrContext> env)
    pub fn get_init_env(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        self.init_env(ctx, tree, env)
    }
}

// ---------------------------------------------------------------------------
// Java: MemberEnter.InitTreeVisitor — the visitor for expressions which might be
// constant expressions, as per JLS 15.28. The Java JCTree.Visitor dispatch is
// expressed as a recursive match (registered adapter).
// ---------------------------------------------------------------------------

fn init_allowed_operator(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::POS
            | Tag::NEG
            | Tag::NOT
            | Tag::COMPL
            | Tag::PLUS
            | Tag::MINUS
            | Tag::MUL
            | Tag::DIV
            | Tag::MOD
            | Tag::SL
            | Tag::SR
            | Tag::USR
            | Tag::LT
            | Tag::LE
            | Tag::GT
            | Tag::GE
            | Tag::EQ
            | Tag::NE
            | Tag::BITAND
            | Tag::BITXOR
            | Tag::BITOR
            | Tag::AND
            | Tag::OR
    )
}

pub(crate) fn init_tree_visitor(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Literal { .. } => true,
        TreeKind::Ident { .. } => true,
        TreeKind::TypeCast { expr, .. } => match expr {
            Some(expr) => init_tree_visitor(ctx, *expr),
            None => true,
        },
        TreeKind::Parens { expr } => match expr {
            Some(expr) => init_tree_visitor(ctx, *expr),
            None => true,
        },
        TreeKind::Unary { arg, .. } => {
            if !init_allowed_operator(ctx.tree(tree).tag) {
                return false;
            }
            match arg {
                Some(arg) => init_tree_visitor(ctx, *arg),
                None => true,
            }
        }
        TreeKind::Binary { lhs, rhs, .. } => {
            if !init_allowed_operator(ctx.tree(tree).tag) {
                return false;
            }
            let lhs_ok = match lhs {
                Some(lhs) => init_tree_visitor(ctx, *lhs),
                None => true,
            };
            let rhs_ok = match rhs {
                Some(rhs) => init_tree_visitor(ctx, *rhs),
                None => true,
            };
            lhs_ok && rhs_ok
        }
        TreeKind::Conditional {
            cond,
            truepart,
            falsepart,
            ..
        } => {
            let cond_ok = match cond {
                Some(cond) => init_tree_visitor(ctx, *cond),
                None => true,
            };
            let true_ok = match truepart {
                Some(truepart) => init_tree_visitor(ctx, *truepart),
                None => true,
            };
            let false_ok = match falsepart {
                Some(falsepart) => init_tree_visitor(ctx, *falsepart),
                None => true,
            };
            cond_ok && true_ok && false_ok
        }
        TreeKind::FieldAccess { selected, .. } => match selected {
            Some(selected) => init_tree_visitor(ctx, *selected),
            None => true,
        },
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Local tree-field accessors (the JCTree fields are arena entries).
// ---------------------------------------------------------------------------

fn method_decl_name(ctx: &CompilerContext, tree: TreeId) -> Name {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { name, .. } => name.clone(),
        _ => panic!("MethodDecl expected"),
    }
}

fn method_decl_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { sym, .. } => *sym,
        _ => None,
    }
}

fn set_method_decl_symbol(ctx: &mut CompilerContext, tree: TreeId, symbol: SymbolId) {
    if let TreeKind::MethodDecl { sym, .. } = &mut ctx.tree_mut(tree).kind {
        *sym = Some(symbol);
    }
}

fn method_decl_mods_flags(ctx: &CompilerContext, tree: TreeId) -> i64 {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { mods, .. } => mods
            .as_ref()
            .map(|mods| modifiers_flags(ctx, *mods))
            .unwrap_or(0),
        _ => 0,
    }
}

fn method_decl_typarams(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { typarams, .. } => typarams.clone(),
        _ => List::nil(),
    }
}

fn method_decl_params(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { params, .. } => params.clone(),
        _ => List::nil(),
    }
}

fn method_decl_restype(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { restype, .. } => *restype,
        _ => None,
    }
}

fn method_decl_recvparam(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { recvparam, .. } => *recvparam,
        _ => None,
    }
}

fn method_decl_thrown(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { thrown, .. } => thrown.clone(),
        _ => List::nil(),
    }
}

fn var_decl_name(ctx: &CompilerContext, tree: TreeId) -> Name {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { name, .. } => name.clone(),
        _ => panic!("VariableDecl expected"),
    }
}

fn var_decl_mods_flags(ctx: &CompilerContext, tree: TreeId) -> i64 {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { mods, .. } => mods
            .as_ref()
            .map(|mods| modifiers_flags(ctx, *mods))
            .unwrap_or(0),
        _ => 0,
    }
}

fn var_decl_vartype(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { vartype, .. } => *vartype,
        _ => None,
    }
}

fn var_decl_type(ctx: &CompilerContext, tree: TreeId) -> Option<TypeId> {
    var_decl_vartype(ctx, tree).and_then(|vartype| ctx.tree(vartype).ty)
}

fn var_decl_init(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { init, .. } => *init,
        _ => None,
    }
}

fn var_decl_nameexpr(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { nameexpr, .. } => *nameexpr,
        _ => None,
    }
}

fn var_decl_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { sym, .. } => *sym,
        _ => None,
    }
}

fn set_var_decl_symbol(ctx: &mut CompilerContext, tree: TreeId, symbol: SymbolId) {
    if let TreeKind::VariableDecl { sym, .. } = &mut ctx.tree_mut(tree).kind {
        *sym = Some(symbol);
    }
}

fn set_var_symbol_pos(ctx: &mut CompilerContext, symbol: SymbolId, pos: i32) {
    if let SymbolKind::VarSymbol { pos: slot, .. } = &mut ctx.sym_mut(symbol).kind_data {
        *slot = pos;
    }
}

fn is_implicitly_typed(ctx: &CompilerContext, tree: TreeId) -> bool {
    // Java: JCVariableDecl.isImplicitlyTyped() { return vartype == null; }
    var_decl_vartype(ctx, tree).is_none()
}

fn modifiers_flags(ctx: &CompilerContext, mods: TreeId) -> i64 {
    match &ctx.tree(mods).kind {
        TreeKind::Modifiers { flags, .. } => *flags,
        _ => 0,
    }
}

fn set_tree_type(ctx: &mut CompilerContext, tree: TreeId, ty: TypeId) {
    ctx.tree_mut(tree).ty = Some(ty);
}


// ---------------------------------------------------------------------------
// Tree accessors used by the Annotate call sites (local tree/arena adapters).
// ---------------------------------------------------------------------------

fn modifiers_annotations(ctx: &CompilerContext, mods: Option<TreeId>) -> List<TreeId> {
    match mods.map(|mods| &ctx.tree(mods).kind) {
        Some(TreeKind::Modifiers { annotations, .. }) => annotations.clone(),
        _ => List::nil(),
    }
}

fn method_decl_mods(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { mods, .. } => *mods,
        _ => None,
    }
}

fn method_decl_mods_annotations(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    modifiers_annotations(ctx, method_decl_mods(ctx, tree))
}

fn method_decl_default_value(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { default_value, .. } => *default_value,
        _ => None,
    }
}

fn var_decl_mods(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { mods, .. } => *mods,
        _ => None,
    }
}

fn var_decl_mods_annotations(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    modifiers_annotations(ctx, var_decl_mods(ctx, tree))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::jc_tree::JCTree;

    fn alloc(ctx: &mut CompilerContext, tag: Tag, kind: TreeKind) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos: 0,
            ty: None,
            tag,
            kind,
        })
    }

    fn literal(ctx: &mut CompilerContext) -> TreeId {
        alloc(
            ctx,
            Tag::LITERAL,
            TreeKind::Literal {
                typetag: crate::code::type_tag::TypeTag::INT,
                value: Some(crate::tree::jc_tree::LiteralValue::Integer(1)),
            },
        )
    }

    fn ident(ctx: &mut CompilerContext) -> TreeId {
        alloc(
            ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: crate::util::names::Names::new(false).empty.clone(),
                sym: None,
            },
        )
    }

    /// Java: MemberEnter.InitTreeVisitor (JLS 15.28 constant expressions).
    #[test]
    fn needs_lazy_const_value_matches_the_init_tree_visitor_rules() {
        let mut ctx = CompilerContext::new();
        // Literal and identifier: constant expressions.
        let literal_tree = literal(&mut ctx);
        assert!(init_tree_visitor(&ctx, literal_tree));
        let ident_tree = ident(&mut ctx);
        assert!(init_tree_visitor(&ctx, ident_tree));
        // Type cast / parens recurse into the operand.
        let cast = alloc(
            &mut ctx,
            Tag::TYPECAST,
            TreeKind::TypeCast {
                clazz: None,
                expr: Some(literal_tree),
            },
        );
        assert!(init_tree_visitor(&ctx, cast));
        let parens = alloc(
            &mut ctx,
            Tag::PARENS,
            TreeKind::Parens {
                expr: Some(literal_tree),
            },
        );
        assert!(init_tree_visitor(&ctx, parens));
        // Allowed unary operators recurse; disallowed ones fail.
        let unary_allowed = alloc(
            &mut ctx,
            Tag::POS,
            TreeKind::Unary {
                arg: Some(literal_tree),
                operator: None,
            },
        );
        assert!(init_tree_visitor(&ctx, unary_allowed));
        let unary_disallowed = alloc(
            &mut ctx,
            Tag::ASSIGN,
            TreeKind::Unary {
                arg: Some(literal_tree),
                operator: None,
            },
        );
        assert!(!init_tree_visitor(&ctx, unary_disallowed));
        // Allowed binary operators recurse into both operands.
        let binary_allowed = alloc(
            &mut ctx,
            Tag::PLUS,
            TreeKind::Binary {
                lhs: Some(literal_tree),
                rhs: Some(ident_tree),
                operator: None,
            },
        );
        assert!(init_tree_visitor(&ctx, binary_allowed));
        let binary_disallowed = alloc(
            &mut ctx,
            Tag::TYPETEST,
            TreeKind::Binary {
                lhs: Some(literal_tree),
                rhs: Some(ident_tree),
                operator: None,
            },
        );
        assert!(!init_tree_visitor(&ctx, binary_disallowed));
        // Conditional expressions recurse into all three operands.
        let conditional = alloc(
            &mut ctx,
            Tag::CONDEXPR,
            TreeKind::Conditional {
                cond: Some(literal_tree),
                truepart: Some(literal_tree),
                falsepart: Some(literal_tree),
                poly_kind: crate::tree::jc_tree::PolyKind::STANDALONE,
            },
        );
        assert!(init_tree_visitor(&ctx, conditional));
        // Field access recurses into the selected expression.
        let select = alloc(
            &mut ctx,
            Tag::SELECT,
            TreeKind::FieldAccess {
                selected: Some(literal_tree),
                name: crate::util::names::Names::new(false).empty.clone(),
                sym: None,
            },
        );
        assert!(init_tree_visitor(&ctx, select));
        // Any other tree kind is not a constant expression.
        let skip = alloc(&mut ctx, Tag::SKIP, TreeKind::Skip {});
        assert!(!init_tree_visitor(&ctx, skip));
    }
}
