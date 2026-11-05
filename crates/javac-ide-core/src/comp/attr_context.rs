//! Contains information specific to the attribute and enter passes, to be used
//! in place of the generic field in environments.
//!
//! Adapters (registered):
//! - Scope.WriteableScope -> ScopeId; Symbol -> SymbolId; Type -> TypeId;
//!   JCTree -> TreeId; Lint object reference -> Rc<Lint>;
//! - Attr.ResultInfo (returnResult/yieldResult) is not ported (Attr): the
//!   fields are omitted and dup() skips them; registered REGISTERED, no substitute;
//! - DeferredAttr.AttributionMode / Resolve.MethodResolutionPhase are ported as
//!   standalone enums in comp/deferred_attr.rs and comp/resolve.rs.

use std::rc::Rc;

use crate::code::lint::Lint;
use crate::comp::deferred_attr::AttributionMode;
use crate::comp::resolve::MethodResolutionPhase;
use crate::compiler_context::CompilerContext;
use crate::ids::{ScopeId, SymbolId, TreeId, TypeId};

/// Java: public class AttrContext
#[derive(Clone)]
pub struct AttrContext {
    /// Java: WriteableScope scope = null;
    pub scope: Option<ScopeId>,
    /// Java: int staticLevel = 0;
    pub static_level: i32,
    /// Java: boolean isSelfCall = false;
    pub is_self_call: bool,
    /// Java: boolean selectSuper = false;
    pub select_super: bool,
    /// Java: boolean isSerializable = false;
    pub is_serializable: bool,
    /// Java: boolean isSerializableLambda = false;
    pub is_serializable_lambda: bool,
    /// Java: boolean isLambda = false;
    pub is_lambda: bool,
    /// Java: AttributionMode attributionMode = AttributionMode.FULL;
    pub attribution_mode: AttributionMode,
    /// Java: boolean isAnonymousDiamond = false;
    pub is_anonymous_diamond: bool,
    /// Java: boolean isNewClass = false;
    pub is_new_class: bool,
    /// Java: boolean visitingServiceImplementation = false;
    pub visiting_service_implementation: bool,
    /// Java: Resolve.MethodResolutionPhase pendingResolutionPhase = null;
    pub pending_resolution_phase: Option<MethodResolutionPhase>,
    /// Java: Lint lint;
    pub lint: Option<Rc<Lint>>,
    /// Java: Symbol enclVar = null;
    pub encl_var: Option<SymbolId>,
    /// Java: Attr.ResultInfo returnResult = null;
    pub return_result: Option<crate::comp::attr::ResultInfo>,
    /// Java: Attr.ResultInfo yieldResult = null;
    pub yield_result: Option<crate::comp::attr::ResultInfo>,
    /// Java: Type defaultSuperCallSite = null;
    pub default_super_call_site: Option<TypeId>,
    /// Java: JCTree preferredTreeForDiagnostics;
    pub preferred_tree_for_diagnostics: Option<TreeId>,
}

impl AttrContext {
    /// Java: new AttrContext()
    pub fn new() -> AttrContext {
        AttrContext {
            scope: None,
            static_level: 0,
            is_self_call: false,
            select_super: false,
            is_serializable: false,
            is_serializable_lambda: false,
            is_lambda: false,
            attribution_mode: AttributionMode::FULL,
            is_anonymous_diamond: false,
            is_new_class: false,
            visiting_service_implementation: false,
            pending_resolution_phase: None,
            lint: None,
            encl_var: None,
            return_result: None,
            yield_result: None,
            default_super_call_site: None,
            preferred_tree_for_diagnostics: None,
        }
    }

    /// Java: AttrContext dup(WriteableScope scope)
    pub fn dup(&self, scope: Option<ScopeId>) -> AttrContext {
        AttrContext {
            scope,
            static_level: self.static_level,
            is_self_call: self.is_self_call,
            select_super: self.select_super,
            pending_resolution_phase: self.pending_resolution_phase,
            lint: self.lint.clone(),
            encl_var: self.encl_var,
            return_result: self.return_result.clone(),
            yield_result: self.yield_result.clone(),
            default_super_call_site: self.default_super_call_site,
            is_serializable: self.is_serializable,
            is_lambda: self.is_lambda,
            is_serializable_lambda: self.is_serializable_lambda,
            attribution_mode: self.attribution_mode,
            is_anonymous_diamond: self.is_anonymous_diamond,
            is_new_class: self.is_new_class,
            preferred_tree_for_diagnostics: self.preferred_tree_for_diagnostics,
            visiting_service_implementation: self.visiting_service_implementation,
        }
    }

    /// Java: AttrContext dup()
    pub fn dup_ctx(&self) -> AttrContext {
        self.dup(self.scope)
    }

    /// Java: public Iterable<Symbol> getLocalElements()
    pub fn get_local_elements(&self, ctx: &mut CompilerContext) -> Vec<SymbolId> {
        match self.scope {
            None => Vec::new(),
            Some(scope) => ctx.scope_get_symbols(
                scope,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            ),
        }
    }

    /// Java: boolean lastResolveVarargs()
    pub fn last_resolve_varargs(&self) -> bool {
        self.pending_resolution_phase
            .map(|phase| phase.is_varargs_required())
            .unwrap_or(false)
    }
}

impl Default for AttrContext {
    fn default() -> Self {
        AttrContext::new()
    }
}

impl std::fmt::Display for AttrContext {
    /// Java: public String toString() { return "AttrContext[" + scope + "]"; }
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AttrContext[{:?}]", self.scope)
    }
}
