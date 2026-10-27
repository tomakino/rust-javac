// Origin: com/sun/tools/javac/comp/AttrRecover.java @ 982d53c (jdk-17.0.19-ga)
//
// Registration mapping: AttrRecover (instance/constructor/doRecovery/
// recoverMethodInvocation/basicMethodInvocationRecovery/wrongMethodSymbolCandidate/
// RecoveryErrorType/RecoverTodo).
// adapter registered adapters:
// - the upstream service fields are explicit parameters (ADR 0001); the instance
// holds recoveryTodo and the immutable services;
// - the upstream rollback closures (Runnable) are RollbackAction values applied
// in order;
// - the anonymous TreeTranslator that rewrites lambda returns is the explicit
// statement traversal below (the port TreeTranslator has no overridable visit
// hooks);
// - RecoveryErrorType is TypeKindData::ErrorType::candidate_symbol.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::code::symtab::Symtab;
use crate::code::type_::TypeKindData;
use crate::code::types::Types;
use crate::comp::attr::Attr;
use crate::comp::attr::ResultInfo;
use crate::comp::attr_context::AttrContext;
use crate::comp::deferred_attr::AttrMode;
use crate::comp::deferred_attr::DeferredMapCtx;
use crate::comp::deferred_attr::RecoveryDeferredTypeMap;
use crate::comp::env::Env;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{JCTree, Tag, TreeKind};
use crate::util::list::List;
use crate::util::names::Names;
use crate::tree::tree_maker::TreeFactory;

/// Java: class AttrRecover
/// # Fields
///   - names/syms/types: explicit services;
///   - make: the TreeMaker cell (repair);
///   - recovery_todo: the pending recovery queue.
///
/// # Panics
///   - none.
///
/// Upstream: Attr.java:3970-4000.
pub struct AttrRecover {
    pub names: Rc<Names>,
    pub make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
    pub syms: Rc<RefCell<Symtab>>,
    pub types: Rc<Types>,
    recovery_todo: RefCell<Vec<RecoverTodo>>,
}

/// Java: private static class RecoverTodo
pub struct RecoverTodo {
    pub tree: TreeId,
    pub site: TypeId,
    pub err_sym: SymbolId,
    pub cand_sym: SymbolId,
    pub env: Rc<Env<AttrContext>>,
    pub result_info: Rc<ResultInfo>,
}

/// Porting rules: the upstream ListBuffer<Runnable> rollback actions.
enum RollbackAction {
    /// lambda.body = translate(lambda.body) replacing err with the return.
    RestoreReturnInLambdaBody { lambda: TreeId, from: TreeId, to: TreeId },
    /// tree.expr = null
    ResetReturnExpr { ret: TreeId },
    /// block.stats = List.filter(stats, ret)
    RemoveAppendedReturn { block: TreeId, ret: TreeId },
    /// mit.args = prevArgs
    RestoreArgs { mit: TreeId, prev_args: List<TreeId> },
}

impl AttrRecover {
    /// Java: protected AttrRecover(Context context)
/// # Arguments
///   - the explicit services (ctx/names/make/syms/types/log/diags).
///
/// # Returns
///   - an AttrRecover.
///
/// # Panics
///   - none.
///
/// Upstream: Attr.java:3990-4000.
    pub fn new(
        names: Rc<Names>,
        make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
        syms: Rc<RefCell<Symtab>>,
        types: Rc<Types>,
    ) -> AttrRecover {
        AttrRecover { names, make, syms, types, recovery_todo: RefCell::new(Vec::new()) }
    }

    fn make_erroneous(&self, ctx: &mut CompilerContext, ty: TypeId) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos: crate::util::position::NOPOS,
            ty: Some(ty),
            tag: Tag::ERRONEOUS,
            kind: TreeKind::Erroneous { errs: List::nil() },
        })
    }

    fn make_erroneous_trees(&self, ctx: &mut CompilerContext, trees: List<TreeId>) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos: crate::util::position::NOPOS,
            ty: None,
            tag: Tag::ERRONEOUS,
            kind: TreeKind::Erroneous { errs: trees },
        })
    }

    fn make_erroneous_return(&self, ctx: &mut CompilerContext) -> TreeId {
        let err_type = self.syms.borrow().err_type;
        let expr = self.make_erroneous(ctx, err_type);
        let maker = Rc::clone(&self.make);
        let mut maker = maker.borrow_mut();
        maker.at(crate::util::position::NOPOS).return_(ctx, Some(expr))
    }
}

impl AttrRecover {
    /// Java: public void doRecovery() (AttrRecover.java:97-200)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/attr/rs/infer/chk/diags: explicit services.
///
/// # Returns
///   - none; the recovery queue is drained.
///
/// # Panics
///   - none.
///
/// Upstream: AttrRecover.java:97-200.
    pub fn do_recovery(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        attr: &mut Attr,
        rs: &mut crate::comp::resolve::Resolve,
        infer: &Rc<crate::comp::infer::Infer>,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
    ) {
        loop {
            let todo = {
                let mut todos = self.recovery_todo.borrow_mut();
                if todos.is_empty() {
                    return;
                }
                todos.remove(0)
            };
            let mut rollback: Vec<RollbackAction> = Vec::new();
            let mut repaired = false;
            if ctx.tree(todo.tree).tag == Tag::APPLY {
                let mit = todo.tree;
                let mit_args = match &ctx.tree(mit).kind {
                    TreeKind::MethodInvocation { args, .. } => args.clone(),
                    _ => panic!("ClassCastException: JCMethodInvocation expected"),
                };
                let vararg = ctx.sym(todo.cand_sym).flags_field & flags::VARARGS != 0;
                let cand_type = ctx.sym(todo.cand_sym).ty.expect("candidate type");
                let mut formals = self.types.get_parameter_types_public(ctx, cand_type);
                let mut args = mit_args.clone();
                let too_many = !vararg && args.length() > formals.length();
                if !too_many {
                    loop {
                        if args.is_empty() || formals.is_empty() {
                            break;
                        }
                        let arg = *args.head();
                        let formal = if !formals.tail().is_empty() || !vararg {
                            *formals.head()
                        } else {
                            self.types.elemtype(ctx, syms, *formals.head())
                        };
                        if ctx.tree(arg).tag == Tag::LAMBDA {
                            let lambda = arg;
                            let param_kind = match &ctx.tree(lambda).kind {
                                TreeKind::Lambda { param_kind, .. } => *param_kind,
                                _ => crate::util::assert::error(),
                            };
                            if param_kind == crate::tree::jc_tree::ParameterKind::IMPLICIT {
                                let params = match &ctx.tree(lambda).kind {
                                    TreeKind::Lambda { params, .. } => params.clone(),
                                    _ => crate::util::assert::error(),
                                };
                                for var in params.iter() {
                                    if let TreeKind::VariableDecl { vartype, .. } =
                                        &mut ctx.tree_mut(var).kind
                                    {
                                        *vartype = None;
                                    }
                                }
                            }
                            let formal_tsym = ctx.typ(formal).tsym;
                            let formal_is_functional = formal_tsym
                                .map(|tsym| {
                                    self.types.is_functional_interface(ctx, syms, diags, chk, tsym)
                                })
                                .unwrap_or(false);
                            if formal_is_functional {
                                let descriptor = self
                                    .types
                                    .find_descriptor_type(ctx, syms, diags, chk, formal)
                                    .expect("functional interface descriptor");
                                let return_type =
                                    self.types.get_return_type_public(ctx, descriptor);
                                let void_compatible = ctx
                                    .typ(return_type)
                                    .has_tag(crate::code::type_tag::TypeTag::VOID);
                                let (new_body, body_actions) =
                                    self.rewrite_lambda_body(ctx, lambda, void_compatible);
                                if let TreeKind::Lambda { body: slot, .. } =
                                    &mut ctx.tree_mut(lambda).kind
                                {
                                    *slot = Some(new_body);
                                }
                                rollback.extend(body_actions);
                                if !void_compatible && ctx.tree(new_body).tag == Tag::BLOCK {
                                    let ret = self.make_erroneous_return(ctx);
                                    if let TreeKind::Block { stats, .. } =
                                        &mut ctx.tree_mut(new_body).kind
                                    {
                                        *stats = stats.append(ret);
                                    }
                                    rollback.push(RollbackAction::RemoveAppendedReturn {
                                        block: new_body,
                                        ret,
                                    });
                                }
                            }
                            repaired = true;
                        }
                        args = args.tail();
                        if !formals.tail().is_empty() || !vararg {
                            formals = formals.tail();
                        }
                    }
                }
                let prev_args = match &ctx.tree(mit).kind {
                    TreeKind::MethodInvocation { args, .. } => args.clone(),
                    _ => List::nil(),
                };
                if !too_many {
                    while !formals.is_empty() {
                        let erroneous = self.make_erroneous(ctx, syms.err_type);
                        let new_args = match &ctx.tree(mit).kind {
                            TreeKind::MethodInvocation { args, .. } => args.append(erroneous),
                            _ => List::nil(),
                        };
                        if let TreeKind::MethodInvocation { args: slot, .. } =
                            &mut ctx.tree_mut(mit).kind
                        {
                            *slot = new_args;
                        }
                        formals = formals.tail();
                        repaired = true;
                    }
                }
                rollback.push(RollbackAction::RestoreArgs { mit, prev_args });
            }
            let owntype = if repaired {
                let args_trees = crate::tree::tree_info::args(ctx, todo.env.tree.expect("env tree"))
                    .unwrap_or_else(List::nil);
                let pt = todo.result_info.pt.expect("resultInfo.pt");
                let mut pats = self.types.get_parameter_types_public(ctx, pt);
                while pats.length() < args_trees.length() {
                    pats = pats.append(syms.err_type);
                }
                let ret = self.types.get_return_type_public(ctx, pt);
                let new_result = ResultInfo::new_with_mode(
                    todo.result_info.pkind,
                    Some(ret),
                    Rc::clone(&todo.result_info.check_context),
                    todo.result_info.check_mode,
                );
                let typeargs = self.types.get_type_arguments_public(ctx, pt);
                let owntype = attr.check_method(
                    ctx,
                    syms,
                    rs,
                    infer,
                    todo.site,
                    todo.cand_sym,
                    &new_result,
                    &todo.env,
                    Some(&args_trees),
                    pats,
                    Some(typeargs),
                );
                for action in rollback {
                    self.apply_rollback(ctx, action);
                }
                owntype
            } else {
                self.basic_method_invocation_recovery(
                    ctx,
                    syms,
                    attr,
                    chk,
                    diags,
                    todo.tree,
                    todo.site,
                    todo.err_sym,
                    &todo.env,
                    &todo.result_info,
                )
            };
            ctx.tree_mut(todo.tree).ty = Some(owntype);
        }
    }

    fn apply_rollback(&self, ctx: &mut CompilerContext, action: RollbackAction) {
        match action {
            RollbackAction::RestoreReturnInLambdaBody { lambda, from, to } => {
                let body = match &ctx.tree(lambda).kind {
                    TreeKind::Lambda { body, .. } => *body,
                    _ => panic!("ClassCastException: JCLambda expected"),
                };
                if let Some(body) = body {
                    let replaced = self.replace_in_tree(ctx, body, from, to);
                    if let TreeKind::Lambda { body: slot, .. } = &mut ctx.tree_mut(lambda).kind {
                        *slot = Some(replaced);
                    }
                }
            }
            RollbackAction::ResetReturnExpr { ret } => {
                if let TreeKind::Return { expr } = &mut ctx.tree_mut(ret).kind {
                    *expr = None;
                }
            }
            RollbackAction::RemoveAppendedReturn { block, ret } => {
                if let TreeKind::Block { stats, .. } = &mut ctx.tree_mut(block).kind {
                    let mut filtered: List<TreeId> = List::nil();
                    for s in stats.iter() {
                        if s != ret {
                            filtered = filtered.append(s);
                        }
                    }
                    *stats = filtered;
                }
            }
            RollbackAction::RestoreArgs { mit, prev_args } => {
                if let TreeKind::MethodInvocation { args, .. } = &mut ctx.tree_mut(mit).kind {
                    *args = prev_args;
                }
            }
        }
    }
}

impl AttrRecover {
    /// Java: Type recoverMethodInvocation(JCTree tree, Type site, Symbol sym,
    /// Env<AttrContext> env, ResultInfo resultInfo) (AttrRecover.java:202-215)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/attr/rs/infer/chk/diags, tree/site/sym/env/result_info.
///
/// # Returns
///   - the recovered type.
///
/// # Panics
///   - none.
///
/// Upstream: AttrRecover.java:202-215.
    pub fn recover_method_invocation(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        attr: &mut Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        tree: TreeId,
        site: TypeId,
        sym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        result_info: &ResultInfo,
    ) -> TypeId {
        let recoverable = env.info.borrow().attribution_mode.recover()
            && ctx.sym(sym).flags_field & flags::RECOVERABLE != 0;
        if recoverable {
            let cand = self.recovery_candidate(ctx, sym);
            self.recovery_todo.borrow_mut().push(RecoverTodo {
                tree,
                site,
                err_sym: sym,
                cand_sym: cand,
                env: attr.copy_env(ctx, syms, &self.types, env),
                result_info: Rc::new(result_info.clone()),
            });
            syms.err_type
        } else {
            self.basic_method_invocation_recovery(
                ctx,
                syms,
                attr,
                chk,
                diags,
                tree,
                site,
                sym,
                env,
                result_info,
            )
        }
    }

    fn recovery_candidate(&self, ctx: &CompilerContext, sym: SymbolId) -> SymbolId {
        let sym_type = ctx.sym(sym).ty.expect("error symbol type");
        match &ctx.typ(sym_type).kind {
            TypeKindData::ErrorType { candidate_symbol, .. } => {
                candidate_symbol.expect("RecoveryErrorType candidate")
            }
            _ => panic!("ClassCastException: RecoveryErrorType expected"),
        }
    }

    /// Java: private Type basicMethodInvocationRecovery(JCTree tree, Type site,
    /// Symbol sym, Env<AttrContext> env, ResultInfo resultInfo)
    /// (AttrRecover.java:217-226)
    #[allow(clippy::too_many_arguments)]
    fn basic_method_invocation_recovery(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        attr: &mut Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        tree: TreeId,
        site: TypeId,
        sym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        result_info: &ResultInfo,
    ) -> TypeId {
        let phase = env
            .info
            .borrow()
            .pending_resolution_phase
            .unwrap_or(crate::comp::resolve::MethodResolutionPhase::BOX);
        let pt = result_info.pt;
        let pt = if let Some(pt) = pt {
            let mut map = RecoveryDeferredTypeMap::new(AttrMode::SPECULATIVE, Some(sym), phase);
            let mut dctx = DeferredMapCtx {
                attr,
                syms,
                types: &self.types,
                chk,
                diags,
            };
            map.visit(ctx, &mut dctx, pt, &None)
        } else {
            crate::code::symtab::no_type_id(ctx)
        };
        let owntype = attr.check_id_internal(
            ctx, syms, tree, site, sym, Some(pt), env, result_info,
        );
        let mut map2 = RecoveryDeferredTypeMap::new(AttrMode::CHECK, Some(sym), phase);
        let mut dctx2 = DeferredMapCtx {
            attr,
            syms,
            types: &self.types,
            chk,
            diags,
        };
        let _ = map2.visit(ctx, &mut dctx2, pt, &None);
        owntype
    }

    /// Java: void wrongMethodSymbolCandidate(TypeSymbol errSymbol, Symbol candSym,
    /// JCDiagnostic diag) (AttrRecover.java:228-260)
/// # Arguments
///   - ctx/syms/attr/rs/infer/chk/diags and the candidate.
///
/// # Returns
///   - true when the candidate matches the recovery keys.
///
/// # Panics
///   - none.
///
/// Upstream: AttrRecover.java:228-260.
    pub fn wrong_method_symbol_candidate(
        &self,
        ctx: &mut CompilerContext,
        err_symbol: SymbolId,
        cand_sym: SymbolId,
        diag: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        let mut diags: Vec<crate::util::jc_diagnostic::JCDiagnostic> = vec![diag.clone()];
        let mut recoverable = false;
        while !recoverable && !diags.is_empty() {
            let d = diags.remove(0);
            let code = d.get_code_string();
            match code.as_str() {
                "compiler.misc.missing.ret.val"
                | "compiler.misc.unexpected.ret.val"
                | "compiler.misc.infer.arg.length.mismatch"
                | "compiler.misc.arg.length.mismatch" => recoverable = true,
                _ => {}
            }
            if !recoverable {
                for arg in d.get_args() {
                    if let Some(nested) = arg.downcast_ref::<crate::util::jc_diagnostic::JCDiagnostic>() {
                        diags.push(nested.clone());
                    }
                }
            }
        }
        if recoverable {
            let sym_type = ctx.sym(err_symbol).ty.expect("error symbol type");
            let recovery_type = self.recovery_error_type(ctx, sym_type, cand_sym);
            ctx.sym_mut(err_symbol).ty = Some(recovery_type);
            ctx.sym_mut(err_symbol).flags_field |= flags::RECOVERABLE;
        }
    }

    /// Java: RecoveryErrorType(ErrorType original, Symbol candidateSymbol)
    fn recovery_error_type(
        &self,
        ctx: &mut CompilerContext,
        original: TypeId,
        candidate: SymbolId,
    ) -> TypeId {
        let (original_type, tsym, outer, typarams, allparams, supertype, interfaces, all_interfaces) =
            match &ctx.typ(original).kind {
                TypeKindData::ErrorType {
                    outer_field,
                    typarams_field,
                    allparams_field,
                    supertype_field,
                    interfaces_field,
                    all_interfaces_field,
                    original_type,
                    ..
                } => (
                    *original_type,
                    ctx.typ(original).tsym,
                    *outer_field,
                    typarams_field.clone(),
                    allparams_field.clone(),
                    *supertype_field,
                    interfaces_field.clone(),
                    all_interfaces_field.clone(),
                ),
                _ => panic!("ClassCastException: ErrorType expected"),
            };
        ctx.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::ERROR,
            tsym,
            TypeKindData::ErrorType {
                outer_field: outer,
                typarams_field: typarams,
                allparams_field: allparams,
                supertype_field: supertype,
                interfaces_field: interfaces,
                all_interfaces_field: all_interfaces,
                original_type,
                candidate_symbol: Some(candidate),
            },
        ))
    }
}

impl AttrRecover {
    /// Rewrite the returns of a lambda body (the upstream anonymous
    /// TreeTranslator, Porting rules); returns the body and its rollback actions.
    fn rewrite_lambda_body(
        &self,
        ctx: &mut CompilerContext,
        lambda: TreeId,
        void_compatible: bool,
    ) -> (TreeId, Vec<RollbackAction>) {
        let body = match &ctx.tree(lambda).kind {
            TreeKind::Lambda { body, .. } => body.expect("lambda body"),
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let mut actions: Vec<RollbackAction> = Vec::new();
        let err_type = self.syms.borrow().err_type;
        let new_body = self.walk_returns(
            ctx,
            body,
            &mut |ctx: &mut CompilerContext, ret: TreeId| -> TreeId {
                if void_compatible {
                    let has_expr = matches!(&ctx.tree(ret).kind, TreeKind::Return { expr: Some(_) });
                    if has_expr {
                        let err = self.make_erroneous_trees(ctx, List::of(ret));
                        actions.push(RollbackAction::RestoreReturnInLambdaBody {
                            lambda,
                            from: err,
                            to: ret,
                        });
                        return err;
                    }
                    ret
                } else {
                    let is_none = matches!(&ctx.tree(ret).kind, TreeKind::Return { expr: None });
                    if is_none {
                        let erroneous = self.make_erroneous(ctx, err_type);
                        if let TreeKind::Return { expr: slot } = &mut ctx.tree_mut(ret).kind {
                            *slot = Some(erroneous);
                        }
                        actions.push(RollbackAction::ResetReturnExpr { ret });
                    }
                    ret
                }
            },
        );
        (new_body, actions)
    }

    fn walk_returns(
        &self,
        ctx: &mut CompilerContext,
        node: TreeId,
        on_return: &mut dyn FnMut(&mut CompilerContext, TreeId) -> TreeId,
    ) -> TreeId {
        match ctx.tree(node).tag {
            Tag::LAMBDA | Tag::CLASSDEF => node,
            Tag::RETURN => on_return(ctx, node),
            _ => match ctx.tree(node).kind.clone() {
                TreeKind::Block { stats, .. } => {
                    let new_stats = self.walk_list(ctx, &stats, on_return);
                    if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(node).kind {
                        *slot = new_stats;
                    }
                    node
                }
                TreeKind::If { cond, thenpart, elsepart } => {
                    let cond1 = cond.map(|c| self.walk_returns(ctx, c, on_return));
                    let then1 = thenpart.map(|c| self.walk_returns(ctx, c, on_return));
                    let else1 = elsepart.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::If { cond, thenpart, elsepart } = &mut ctx.tree_mut(node).kind {
                        *cond = cond1;
                        *thenpart = then1;
                        *elsepart = else1;
                    }
                    node
                }
                TreeKind::WhileLoop { cond, body } => {
                    let cond1 = cond.map(|c| self.walk_returns(ctx, c, on_return));
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::WhileLoop { cond, body } = &mut ctx.tree_mut(node).kind {
                        *cond = cond1;
                        *body = body1;
                    }
                    node
                }
                TreeKind::DoWhileLoop { body, cond } => {
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    let cond1 = cond.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::DoWhileLoop { body, cond } = &mut ctx.tree_mut(node).kind {
                        *body = body1;
                        *cond = cond1;
                    }
                    node
                }
                TreeKind::ForLoop { init, cond, step, body } => {
                    let init1 = self.walk_list(ctx, &init, on_return);
                    let cond1 = cond.map(|c| self.walk_returns(ctx, c, on_return));
                    let step1 = self.walk_list(ctx, &step, on_return);
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::ForLoop { init, cond, step, body } = &mut ctx.tree_mut(node).kind {
                        *init = init1;
                        *cond = cond1;
                        *step = step1;
                        *body = body1;
                    }
                    node
                }
                TreeKind::EnhancedForLoop { var, expr, body } => {
                    let var1 = var.map(|c| self.walk_returns(ctx, c, on_return));
                    let expr1 = expr.map(|c| self.walk_returns(ctx, c, on_return));
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::EnhancedForLoop { var, expr, body } = &mut ctx.tree_mut(node).kind {
                        *var = var1;
                        *expr = expr1;
                        *body = body1;
                    }
                    node
                }
                TreeKind::LabeledStatement { body, .. } => {
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::LabeledStatement { label: _, body } = &mut ctx.tree_mut(node).kind {
                        *body = body1;
                    }
                    node
                }
                TreeKind::Switch { selector, cases, .. } => {
                    let selector1 = selector.map(|c| self.walk_returns(ctx, c, on_return));
                    let cases1 = self.walk_list(ctx, &cases, on_return);
                    if let TreeKind::Switch { selector, cases, .. } = &mut ctx.tree_mut(node).kind {
                        *selector = selector1;
                        *cases = cases1;
                    }
                    node
                }
                TreeKind::Case { labels, stats, body, .. } => {
                    let labels1 = labels;
                    let stats1 = self.walk_list(ctx, &stats, on_return);
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::Case { case_kind: _, labels, stats, body, .. } =
                        &mut ctx.tree_mut(node).kind
                    {
                        *labels = labels1;
                        *stats = stats1;
                        *body = body1;
                    }
                    node
                }
                TreeKind::Try { body, catchers, finalizer, resources, .. } => {
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    let catchers1 = self.walk_list(ctx, &catchers, on_return);
                    let finalizer1 = finalizer.map(|c| self.walk_returns(ctx, c, on_return));
                    let resources1 = self.walk_list(ctx, &resources, on_return);
                    if let TreeKind::Try { body, catchers, finalizer, resources, .. } =
                        &mut ctx.tree_mut(node).kind
                    {
                        *body = body1;
                        *catchers = catchers1;
                        *finalizer = finalizer1;
                        *resources = resources1;
                    }
                    node
                }
                TreeKind::Catch { param, body } => {
                    let param1 = param.map(|c| self.walk_returns(ctx, c, on_return));
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::Catch { param, body } = &mut ctx.tree_mut(node).kind {
                        *param = param1;
                        *body = body1;
                    }
                    node
                }
                TreeKind::Synchronized { lock, body } => {
                    let lock1 = lock.map(|c| self.walk_returns(ctx, c, on_return));
                    let body1 = body.map(|c| self.walk_returns(ctx, c, on_return));
                    if let TreeKind::Synchronized { lock, body } = &mut ctx.tree_mut(node).kind {
                        *lock = lock1;
                        *body = body1;
                    }
                    node
                }
                _ => node,
            },
        }
    }

    fn walk_list(
        &self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
        on_return: &mut dyn FnMut(&mut CompilerContext, TreeId) -> TreeId,
    ) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        for tree in trees.iter() {
            result = result.append(self.walk_returns(ctx, tree, on_return));
        }
        result
    }

    fn replace_in_tree(&self, ctx: &mut CompilerContext, node: TreeId, from: TreeId, to: TreeId) -> TreeId {
        self.walk_returns(ctx, node, &mut |_ctx: &mut CompilerContext, tree: TreeId| -> TreeId {
            if tree == from {
                to
            } else {
                tree
            }
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::symtab::Symtab;
    use crate::code::symbol::{Symbol, SymbolKind};
    use crate::code::type_::Type;
    use crate::code::type_tag::TypeTag;
    use crate::resources::compiler_properties::fragments;
    use crate::resources::diagnostic_contract::DiagnosticArg;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::jc_diagnostic::{Factory, Fragment, JCDiagnostic};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn build() -> (CompilerContext, Rc<Names>, Symtab, Rc<Types>, Rc<Factory>) {
        let mut context = Context::new();
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let names = Rc::new(Names::new(false));
        let mut ctx = CompilerContext::new();
        let syms = Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            crate::jvm::target::Target::DEFAULT,
            crate::code::source::Source::JDK17,
        );
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        (ctx, names, syms, types, diags)
    }

    fn error_type(ctx: &mut CompilerContext) -> TypeId {
        ctx.alloc_type(Type::new(
            TypeTag::ERROR,
            None,
            TypeKindData::ErrorType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
                original_type: None,
                candidate_symbol: None,
            },
        ))
    }

    #[test]
    fn nested_diagnostic_argument_is_retrievable() {
        let (_ctx, _names, _syms, _types, diags) = build();
        let nested = diags.fragment(Fragment::from(fragments::missing_ret_val(
            crate::resources::diagnostic_contract::TypeArg::placeholder("T"),
        )));
        let outer = diags.fragment(Fragment::from(
            fragments::incompatible_ret_type_in_lambda(DiagnosticArg::JCDiagnostic(Rc::new(
                nested.clone(),
            ))),
        ));
        let args = outer.get_args();
        assert_eq!(args.len(), 1);
        let retrieved = args[0]
            .downcast_ref::<JCDiagnostic>()
            .expect("nested JCDiagnostic argument");
        assert_eq!(retrieved.get_code_string(), "compiler.misc.missing.ret.val");
    }

    #[test]
    fn wrong_method_symbol_candidate_marks_recoverable_on_nested_code() {
        let (mut ctx, names, syms, types, diags) = build();
        let err_type = error_type(&mut ctx);
        let err_symbol = ctx.alloc_symbol(Symbol::new(
            crate::code::kinds::Kind::ERR,
            0,
            names.error.clone(),
            Some(err_type),
            None,
            SymbolKind::ResolveError {
                debug_name: "wrong method".to_string(),
                data: Box::new(crate::code::symbol::ResolveErrorData::SymbolNotFound),
            },
        ));
        let cand = ctx.alloc_symbol(Symbol::new(
            crate::code::kinds::Kind::MTH,
            0,
            names.empty.clone(),
            None,
            None,
            SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
            },
        ));
        let diag = diags.fragment(Fragment::from(fragments::missing_ret_val(
            crate::resources::diagnostic_contract::TypeArg::placeholder("T"),
        )));
        let recover = AttrRecover::new(
            Rc::clone(&names),
            Rc::new(RefCell::new(crate::tree::tree_maker::TreeMaker::new(Some(
                Rc::clone(&names),
            )))),
            Rc::new(RefCell::new(syms)),
            types,
        );
        recover.wrong_method_symbol_candidate(&mut ctx, err_symbol, cand, &diag);
        let flags = ctx.sym(err_symbol).flags_field;
        assert_ne!(flags & flags::RECOVERABLE, 0, "RECOVERABLE flag");
        let ty = ctx.sym(err_symbol).ty.expect("recovery type");
        match &ctx.typ(ty).kind {
            TypeKindData::ErrorType { candidate_symbol, .. } => {
                assert_eq!(*candidate_symbol, Some(cand));
            }
            _ => panic!("expected an ErrorType"),
        }
    }
}

