//! The check-mode enum and the check-context hierarchy used by attribution and
//! the type checker.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::types::Types;
use crate::comp::deferred_attr::DeferredAttrContext;
use crate::comp::infer::InferenceContext;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;
use crate::util::abstract_log::AbstractLog;
use crate::util::jc_diagnostic::{JCDiagnostic, SimpleDiagnosticPosition};
use crate::util::log::Log;
use crate::util::warner::Warner;

/// Java: enum Attr.CheckMode (Attr.java:461-491) — the mode controlling the
/// Attr/Check tree update behavior.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CheckMode {
    /// Java: NORMAL
    #[default]
    NORMAL,
    /// Java: NO_TREE_UPDATE
    NO_TREE_UPDATE,
    /// Java: NO_INFERENCE_HOOK
    NO_INFERENCE_HOOK,
}

impl CheckMode {
    /// Java: public boolean updateTreeType() (overridden by NO_TREE_UPDATE)
    pub fn update_tree_type(&self) -> bool {
        !matches!(self, CheckMode::NO_TREE_UPDATE)
    }

    /// Java: public boolean installPostInferenceHook() (overridden by
    /// NO_INFERENCE_HOOK)
    pub fn install_post_inference_hook(&self) -> bool {
        !matches!(self, CheckMode::NO_INFERENCE_HOOK)
    }
}

/// Java: public interface Check.CheckContext
pub trait CheckContext {
    /// Java: boolean compatible(Type found, Type req, Warner warn)
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &Warner,
    ) -> bool;

    /// Java: void report(DiagnosticPosition pos, JCDiagnostic details)
    fn report(&self, ctx: &mut CompilerContext, pos: i32, details: &JCDiagnostic);

    /// Java: public Warner checkWarner(DiagnosticPosition pos, Type found,
    /// Type req)
    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> Warner;

    /// Port adapter: the shared Warner view used by callers that must observe
    /// the lint state recorded during the check (upstream checkWarner returns
    /// the shared rsWarner). Contexts without a shared Warner keep the
    /// per-call value.
    fn check_warner_shared(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> Rc<Warner> {
        Rc::new(self.check_warner(
            ctx, syms, types, pos, found, req,
        ))
    }

    /// Java: public InferenceContext inferenceContext()
    fn inference_context(&self) -> &Rc<InferenceContext>;

    /// Java: public DeferredAttr.DeferredAttrContext deferredAttrContext()
    fn deferred_attr_context(&self) -> &Rc<DeferredAttrContext>;
}

/// Java: static class Check.NestedCheckContext (redirects all calls to the
/// enclosing context).
pub struct NestedCheckContext {
    /// Java: CheckContext enclosingContext
    pub enclosing_context: Rc<dyn CheckContext>,
}

impl CheckContext for NestedCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &Warner,
    ) -> bool {
        self.enclosing_context
            .compatible(ctx, syms, types, found, req, warn)
    }

    fn report(&self, ctx: &mut CompilerContext, pos: i32, details: &JCDiagnostic) {
        self.enclosing_context.report(ctx, pos, details);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> Warner {
        self.enclosing_context
            .check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<InferenceContext> {
        self.enclosing_context.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<DeferredAttrContext> {
        self.enclosing_context.deferred_attr_context()
    }
}

/// Java: Check.basicHandler = new CheckContext() { ... } — the default check
/// context (assignment/return checking).
pub struct BasicCheckContext {
    /// Java: Check.log
    pub log: Rc<Log>,
    /// Java: Check.types
    pub types: Rc<Types>,
    /// Java: infer.emptyContext
    pub infer_empty: Rc<InferenceContext>,
    /// Java: deferredAttr.emptyDeferredAttrContext
    pub deferred_empty: Rc<DeferredAttrContext>,
}

impl CheckContext for BasicCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        _types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &Warner,
    ) -> bool {
        // Java: return types.isAssignable(found, req, warn);
        self.types
            .is_assignable_with_warn(ctx, syms, found, req, warn)
    }

    fn report(&self, _ctx: &mut CompilerContext, pos: i32, details: &JCDiagnostic) {
        // Java: log.error(pos, Errors.ProbFoundReq(details)); keep the nested
        // diagnostic object so its TypeArg where metadata survives.
        self.log.error_at(
            pos,
            crate::util::jc_diagnostic::Error::from(
                crate::resources::compiler_properties::errors::prob_found_req(
                    crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                        std::rc::Rc::new(details.clone()),
                    ),
                ),
            ),
        );
    }

    fn check_warner(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &Types,
        pos: i32,
        _found: TypeId,
        _req: TypeId,
    ) -> Warner {
        // Java: return convertWarner(pos, found, req);
        // adapter adapter (same as cast_warner): ConversionWarner is expressed as a
        // plain Warner; the unchecked/UNCHECKED lint is recorded by isAssignable.
        Warner::new(Some(Rc::new(SimpleDiagnosticPosition::new(pos))))
    }

    fn inference_context(&self) -> &Rc<InferenceContext> {
        &self.infer_empty
    }

    fn deferred_attr_context(&self) -> &Rc<DeferredAttrContext> {
        &self.deferred_empty
    }
}

