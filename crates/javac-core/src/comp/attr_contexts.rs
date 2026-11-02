//! Check contexts created by Attr for conditional, diamond and lambda
//! compatibility checks.

use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::util::jc_diagnostic::Factory;
use crate::util::warner::Warner;

/// Java: Attr.conditionalContext(CheckContext) (Attr.java:2092-2106)
pub struct ConditionalCheckContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
}

impl crate::comp::check::CheckContext for ConditionalCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        self.enclosing
            .compatible(ctx, syms, types, found, req, warn)
    }

    fn report(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        // Java: enclosingContext.report(pos,
        // diags.fragment(Fragments.IncompatibleTypeInConditional(details)));
        let message = details.get_message(None);
        let fragment = self
            .diags
            .fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::incompatible_type_in_conditional(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        message.to_string(),
                    ),
                ),
            ));
        self.enclosing.report(ctx, pos, &fragment);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing
            .check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing.deferred_attr_context()
    }
}

/// Java: Attr.diamondContext(JCNewClass, TypeSymbol, CheckContext)
/// (Attr.java:3007-3016)
pub struct DiamondCheckContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    pub clazz: TreeId,
    pub tsym: SymbolId,
    /// Port adapter: diagnostics produced while a deferred overload candidate is
    /// being speculatively attributed must not reach the user; upstream relies
    /// on Log.DeferredDiagnosticHandler here, the port records the mode.
    pub speculative: bool,
}

impl crate::comp::check::CheckContext for DiamondCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        self.enclosing
            .compatible(ctx, syms, types, found, req, warn)
    }

    fn report(
        &self,
        ctx: &mut CompilerContext,
        _pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        if std::env::var_os("DSH_DEBUG_DIAMOND").is_some() {
            eprintln!(
                "[DIAMOND] pos={} speculative={} detail={}",
                ctx.tree(self.clazz).pos,
                self.speculative,
                details.get_message(None)
            );
            if !self.speculative {
                eprintln!(
                    "[DIAMOND-BT]\n{}",
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        if self.speculative {
            return;
        }
        // Java: enclosingContext.report(clazz.clazz,
        // diags.fragment(Fragments.CantApplyDiamond1(Fragments.Diamond(tsym), details)));
        let diamond_text = {
            let mut display = crate::comp::resolve::symbol_display_name(ctx, self.tsym);
            if display.is_empty() {
                if let Some(ty) = ctx.sym(self.tsym).ty {
                    display = crate::comp::attr::type_display_of(ctx, ty);
                }
            }
            let fragment = self
                .diags
                .fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::diamond(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(display),
                    ),
                ));
            fragment.get_message(None).to_string()
        };
        let message = details.get_message(None);
        let key = self
            .diags
            .fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::cant_apply_diamond1(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        diamond_text,
                    ),
                    // Java: the reason argument is the details diagnostic
                    // itself (its Type arguments carry the where clauses).
                    crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                        std::rc::Rc::new(details.clone()),
                    ),
                ),
            ));
        let clazz_pos = ctx.tree(self.clazz).pos;
        self.enclosing.report(ctx, clazz_pos, &key);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing
            .check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing.deferred_attr_context()
    }
}

/// Java: Attr.FunctionalReturnContext (Attr.java:3394-3411)
pub struct FunctionalReturnContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub basic_handler: Rc<dyn crate::comp::check::CheckContext>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
}

impl crate::comp::check::CheckContext for FunctionalReturnContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        // Java: chk.basicHandler.compatible(inferenceContext().asUndetVar(found),
        // inferenceContext().asUndetVar(req), warn)
        let inference_context = Rc::clone(self.enclosing.inference_context());
        let found = inference_context.as_undet_var(ctx, syms, types, found);
        let req = inference_context.as_undet_var(ctx, syms, types, req);
        self.basic_handler
            .compatible(ctx, syms, types, found, req, warn)
    }

    fn report(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        // Java: enclosingContext.report(pos,
        // diags.fragment(Fragments.IncompatibleRetTypeInLambda(details)));
        let message = details.get_message(None);
        let fragment = self
            .diags
            .fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::incompatible_ret_type_in_lambda(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        message.to_string(),
                    ),
                ),
            ));
        self.enclosing.report(ctx, pos, &fragment);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing
            .check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing.deferred_attr_context()
    }
}

/// Java: Attr.ExpressionLambdaReturnContext (Attr.java:3413-3435)
pub struct ExpressionLambdaReturnContext {
    pub base: FunctionalReturnContext,
    pub expr: TreeId,
    pub exp_stmt_expected: std::cell::Cell<bool>,
}

impl crate::comp::check::CheckContext for ExpressionLambdaReturnContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        // Java: if (req.hasTag(VOID)) { expStmtExpected = true;
        // return TreeInfo.isExpressionStatement(expr); }
        // else return super.compatible(found, req, warn);
        if ctx.typ(req).has_tag(crate::code::type_tag::TypeTag::VOID) {
            self.exp_stmt_expected.set(true);
            return crate::tree::tree_info::is_expression_statement(ctx, self.expr);
        }
        self.base.compatible(ctx, syms, types, found, req, warn)
    }

    fn report(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        // Java: if (expStmtExpected) enclosingContext.report(pos,
        // diags.fragment(Fragments.StatExprExpected));
        // else super.report(pos, details);
        if self.exp_stmt_expected.get() {
            let fragment = self
                .base
                .diags
                .fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::stat_expr_expected(),
                ));
            self.base.enclosing.report(ctx, pos, &fragment);
        } else {
            self.base.report(ctx, pos, details);
        }
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.base.check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.base.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.base.deferred_attr_context()
    }
}


/// Java: Attr.RecoveryInfo's anonymous CheckContext (Attr.java:598-620): a
/// NestedCheckContext whose deferredAttrContext is the empty deferred context.
/// Registered adapter: the anonymous subclass is a named struct.
pub struct RecoveryCheckContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
}

impl crate::comp::check::CheckContext for RecoveryCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        self.enclosing
            .compatible(ctx, syms, types, found, req, warn)
    }

    fn report(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        self.enclosing.report(ctx, pos, details);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing
            .check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        &self.deferred_attr_context
    }
}


/// Java: the anonymous Check.NestedCheckContext in visitTypeCast
/// (Attr.java:4044-4065) overriding compatible with isCastable (adapter: the
/// anonymous subclass is a named struct; Types.isCastable is the read-only
/// castability path so the CheckContext `&Symtab` contract is preserved).
pub struct CastCheckContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub types: Rc<Types>,
}

impl crate::comp::check::CheckContext for CastCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        let _ = types;
        // Java: return types.isCastable(found, req, warn);
        self.types
            .is_castable_with_warn(ctx, syms, found, req, warn)
    }

    fn report(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        self.enclosing.report(ctx, pos, details);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing
            .check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing.deferred_attr_context()
    }
}

