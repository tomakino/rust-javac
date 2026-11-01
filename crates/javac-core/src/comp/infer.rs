#![allow(clippy::result_large_err, clippy::needless_pass_by_ref_mut)]
// Origin: com/sun/tools/javac/comp/Infer.java @ 982d53c (jdk-17.0.19-ga)
// com/sun/tools/javac/comp/InferenceContext.java @ 982d53c (jdk-17.0.19-ga)
//
// Registration mapping: the InferenceContext.java unit maps to the InferenceContext
// type below (same upstream class, ported into one Rust module because it and
// Infer are mutually dependent); Infer.java maps to the Infer/Incorporation*/
// GraphSolver/InferenceStep types below.
//
// Ported in this file (Infer):
// - InferenceContext: addVar/inferenceVars/undetVars/restvars/instvars/
// boundedVars/filterVars/free/freeVarsIn/asUndetVar(s)/instTypes/asInstType(s)/
// addFreeTypeListener/notifyChange/save/rollback/dupTo/min (ReachabilityVisitor)/
// solve/solveAny/solveBasic/solveLegacy/cachedCapture (InferenceContext.java:
// 76-606);
// - Infer: allowGraphInference/fromTypeVarFun/incorporationEngine (graph+legacy)/
// doIncorporation/doIncorporationOp/IncorporationBinaryOp/
// getParameterizedSupers/asSuper/reportInstError/reportBoundError/
// getBoundFragment/BoundFilter/instantiateAsUninferredVars (Infer.java:82-140,
// 519-545, 616-630, 692-1290);
// - the inference engine: GraphStrategy/LeafSolver/BestLeafSolver/InferenceStep/
// LegacyInferenceSteps/GraphInferenceSteps/DependencyKind/GraphSolver/
// InferenceGraph/Node (Infer.java:1299-1945); the Tarjan reduction is a
// faithful re-implementation of GraphUtils.tarjan (GraphUtils.java:171-235)
// over the graph's node indices, because the ported GraphUtils.tarjan takes
// references into an immutable node collection while the inference graph
// mutates its nodes (PORTING_RULES adapter adapter, registered);
//
// NOT PORTED IN THIS FILE (registered: dependency batches; no substitutes are
// written, the upstream signatures depend on services that are not ported yet):
// Ported in the Resolve B (this file; Registration rows in the registration):
// - Infer.instantiateMethod (Infer.java:166-260) with the graph/legacy solver
// paths, PartiallyInferredMethodType/check (287-360);
// - generateReturnConstraints/needsEagerInstantiation/
// generateReferenceToTargetConstraint/shouldPropagate/roots
// (Infer.java:257-520) — Log.note(Notes.DeferredMethodInst) remains registered
// (Log/Notes).
// NOT PORTED (registered):
// - Infer.instantiatePolymorphicSignatureInstance/ImplicitArgType (Infer.java:
// 555-630): needs Attr tree tags and DeferredAttr.DeferredTypeMap wiring
// (Attr);
// - Infer.instantiateFunctionalInterface (Infer.java:633-690): needs
// Types.findDescriptorType and Check.checkValidGenericType (FunctionDescriptor
//);
// - Infer.dumpGraphsIfNeeded (Infer.java:360-390): file dumping (Options);
// - Infer.instance(Context)/Infer(Context) constructor: the port uses explicit
// service construction (ADR 0001), registered.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::code::symtab::Symtab;
use crate::code::type_::{InferenceBound, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::resources::compiler_properties::fragments;
use crate::resources::diagnostic_contract::TypeArg;
use crate::util::jc_diagnostic::{Factory, JCDiagnostic};
use crate::util::list::List;
use crate::util::names::Names;
use crate::util::warner::Warner;

/// Infer.MAX_INCORPORATION_STEPS
pub const MAX_INCORPORATION_STEPS: i32 = 10000;

/// Java: public static class Infer.InferenceException
#[derive(Clone)]
pub struct InferenceException {
    /// Java: transient List<JCDiagnostic> messages = List.nil();
    pub messages: List<JCDiagnostic>,
}

impl InferenceException {
    /// Java: InferenceException() { super(null); }
    pub fn new() -> InferenceException {
        InferenceException {
            messages: List::nil(),
        }
    }

    /// Java: public JCDiagnostic getDiagnostic() { return messages.head; }
    pub fn get_diagnostic(&self) -> Option<JCDiagnostic> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.messages.head().clone())
        }
    }
}

impl Default for InferenceException {
    fn default() -> Self {
        InferenceException::new()
    }
}

impl std::fmt::Debug for InferenceException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferenceException")
            .field("messages", &self.messages.length())
            .finish()
    }
}

/// Java: Infer.FreeTypeListener (Infer.java:1939-1941)
pub trait FreeTypeListener {
    /// Java: void typesInferred(InferenceContext inferenceContext)
    fn types_inferred(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<(), InferenceException>;
}

/// Java: public static interface Type.UndetVar.UndetVarListener
#[allow(clippy::too_many_arguments)]
pub trait UndetVarListener {
    /// Java: void varBoundChanged(UndetVar uv, InferenceBound ib, Type bound,
    /// boolean update)
    fn var_bound_changed(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        ib: InferenceBound,
        bound: TypeId,
        update: bool,
    );

    /// Java: default void varInstantiated(UndetVar uv) { Assert.error(); }
    fn var_instantiated(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
    );
}

/// Java: the BiFunction<InferenceContext, Type, Type> used by CheckBounds
/// (InferenceContext::asUndetVar / InferenceContext::asInstType).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeFunc {
    AsUndetVar,
    AsInstType,
}

/// Java: the BiPredicate<InferenceContext, Type> used by CheckBounds
/// (InferenceContext::free).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeFilter {
    Free,
}

/// Java: Infer.IncorporationAction (the action hierarchy: CheckBounds/
/// EqCheckLegacy/CheckInst/SubstBounds/CheckUpperBounds/PropagateBounds).
///
/// PORTING_RULES: the Java class hierarchy maps to one enum; the
/// BiFunction/BiPredicate fields map to TypeFunc/TypeFilter.
#[derive(Clone)]
pub enum IncorporationAction {
    /// Java: class CheckBounds
    CheckBounds {
        uv: TypeId,
        t: TypeId,
        type_func: TypeFunc,
        opt_filter: Option<TypeFilter>,
        from: InferenceBound,
    },
    /// Java: class EqCheckLegacy extends CheckBounds
    EqCheckLegacy {
        uv: TypeId,
        t: TypeId,
        from: InferenceBound,
    },
    /// Java: class CheckInst extends CheckBounds
    CheckInst { uv: TypeId, to: Vec<InferenceBound> },
    /// Java: class SubstBounds extends CheckInst
    SubstBounds { uv: TypeId },
    /// Java: class CheckUpperBounds
    CheckUpperBounds { uv: TypeId, t: TypeId },
    /// Java: class PropagateBounds
    PropagateBounds { uv: TypeId, t: TypeId, ib: InferenceBound },
}

impl IncorporationAction {
    /// Java: public abstract IncorporationAction dup(UndetVar that)
    pub fn dup(&self, that: TypeId) -> IncorporationAction {
        match self {
            IncorporationAction::CheckBounds {
                t,
                type_func,
                opt_filter,
                from,
                ..
            } => IncorporationAction::CheckBounds {
                uv: that,
                t: *t,
                type_func: *type_func,
                opt_filter: *opt_filter,
                from: *from,
            },
            IncorporationAction::EqCheckLegacy { t, from, .. } => {
                IncorporationAction::EqCheckLegacy {
                    uv: that,
                    t: *t,
                    from: *from,
                }
            }
            IncorporationAction::CheckInst { to, .. } => IncorporationAction::CheckInst {
                uv: that,
                to: to.clone(),
            },
            IncorporationAction::SubstBounds { .. } => {
                IncorporationAction::SubstBounds { uv: that }
            }
            IncorporationAction::CheckUpperBounds { t, .. } => {
                IncorporationAction::CheckUpperBounds { uv: that, t: *t }
            }
            IncorporationAction::PropagateBounds { t, ib, .. } => {
                IncorporationAction::PropagateBounds {
                    uv: that,
                    t: *t,
                    ib: *ib,
                }
            }
        }
    }

    /// Java: InferenceBound[] InferenceBound.values() as the Vec form used by
    /// the bound-kind sets of CheckBounds/PropagateBounds.
    fn complement_of(from: InferenceBound) -> Vec<InferenceBound> {
        let mut result = Vec::new();
        for ib in InferenceBound::values() {
            if ib != from {
                result.push(ib);
            }
        }
        result
    }
}

/// Java: public abstract class Infer.IncorporationAction apply(InferenceContext
/// ic, Warner warn) — ported as a free function because the action needs the
/// Infer service (incorporation cache/ops) and the context.
#[allow(clippy::collapsible_if)]
fn incorporation_action_apply(
    infer: &Infer,
    action: &IncorporationAction,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    warn: &mut Warner,
) -> Result<(), InferenceException> {
    match action {
        IncorporationAction::CheckBounds {
            uv,
            t,
            type_func,
            opt_filter,
            from,
        } => {
            let t = type_func_apply(*type_func, ctx, syms, types, inference_context, *t);
            if let Some(filter) = opt_filter {
                if type_filter_test(*filter, ctx, syms, types, inference_context, t) {
                    return Ok(());
                }
            }
            for to in check_bounds_to_check(*from) {
                let mut bounds = types.undet_var_get_bounds(ctx, *uv, &[to]);
                while bounds.non_empty() {
                    let b = type_func_apply(
                        *type_func,
                        ctx,
                        syms,
                        types,
                        inference_context,
                        *bounds.head(),
                    );
                    if let Some(filter) = opt_filter {
                        if type_filter_test(*filter, ctx, syms, types, inference_context, b) {
                            bounds = bounds.tail();
                            continue;
                        }
                    }
                    if !check_bound(infer, ctx, syms, types, t, b, *from, to, warn)? {
                        report_bound_error_from_to(infer, ctx, syms, types, *uv, *from, to)?;
                    }
                    bounds = bounds.tail();
                }
            }
            Ok(())
        }
        IncorporationAction::EqCheckLegacy { uv, t, from } => {
            // Java: EqCheckLegacy extends CheckBounds with typeFunc
            // InferenceContext::asInstType and filter InferenceContext::free.
            let action = IncorporationAction::CheckBounds {
                uv: *uv,
                t: *t,
                type_func: TypeFunc::AsInstType,
                opt_filter: Some(TypeFilter::Free),
                from: *from,
            };
            incorporation_action_apply(infer, &action, ctx, syms, types, inference_context, warn)
        }
        IncorporationAction::CheckInst { uv, to } => {
            let t = types.undet_var_get_inst(ctx, *uv)
                .unwrap_or_else(|| panic!("AssertionError: CheckInst on an uninstantiated undet var"));
            let action = IncorporationAction::CheckBounds {
                uv: *uv,
                t,
                type_func: TypeFunc::AsUndetVar,
                opt_filter: None,
                from: InferenceBound::EQ,
            };
            // CheckInst overrides boundsToCheck/report; apply the check directly.
            for ib_to in to {
                let bounds = types.undet_var_get_bounds(ctx, *uv, &[*ib_to]);
                let mut current = bounds;
                while current.non_empty() {
                    let b = type_func_apply(
                        TypeFunc::AsUndetVar,
                        ctx,
                        syms,
                        types,
                        inference_context,
                        *current.head(),
                    );
                    if !check_bound(
                        infer,
                        ctx,
                        syms,
                        types,
                        t,
                        b,
                        InferenceBound::EQ,
                        *ib_to,
                        warn,
                    )? {
                        report_inst_error(infer, ctx, types, *uv, *ib_to)?;
                    }
                    current = current.tail();
                }
            }
            let _ = action;
            Ok(())
        }
        IncorporationAction::SubstBounds { uv } => {
            // Java: SubstBounds.apply: replace undetvars in bounds and check the
            // inferred type against all bounds.
            let inst = types.undet_var_get_inst(ctx, *uv);
            let mut undets = inference_context.undet_vars();
            while undets.non_empty() {
                let undet = *undets.head();
                if let Some(inst) = inst {
                    let from = List::of(types.undet_var_qtype(ctx, *uv));
                    let to = List::of(inst);
                    types.undet_var_subst_bounds(ctx, syms, undet, &from, &to);
                }
                check_compatible_upper_bounds(infer, ctx, syms, types, inference_context, undet)?;
                undets = undets.tail();
            }
            let action = IncorporationAction::CheckInst {
                uv: *uv,
                to: vec![
                    InferenceBound::LOWER,
                    InferenceBound::EQ,
                    InferenceBound::UPPER,
                ],
            };
            incorporation_action_apply(infer, &action, ctx, syms, types, inference_context, warn)
        }
        IncorporationAction::CheckUpperBounds { uv, t } => {
            // Java: CheckUpperBounds.apply (Infer.java:922-950)
            let upper_bounds = types.undet_var_get_bounds(ctx, *uv, &[InferenceBound::UPPER]);
            let bound_list = types.closure_collect(ctx, syms, upper_bounds);
            let mut bounds_current = bound_list;
            while bounds_current.non_empty() {
                let b2 = *bounds_current.head();
                if t != &b2 && !ctx.typ(*t).has_tag(TypeTag::WILDCARD) && !ctx.typ(b2).has_tag(TypeTag::WILDCARD) {
                    let common_supers = infer.get_parameterized_supers(ctx, syms, types, *t, b2);
                    let mut supers_current = common_supers;
                    while supers_current.non_empty() {
                        let Pair { fst: sup1, snd: sup2 } = supers_current.head().clone();
                        let mut all1 = types.class_allparams(ctx, sup1);
                        let mut all2 = types.class_allparams(ctx, sup2);
                        while all1.non_empty() && all2.non_empty() {
                            let p1 = *all1.head();
                            let p2 = *all2.head();
                            if !ctx.typ(p1).has_tag(TypeTag::WILDCARD)
                                && !ctx.typ(p2).has_tag(TypeTag::WILDCARD)
                            {
                                let u1 = inference_context.as_undet_var(ctx, syms, types, p1);
                                let u2 = inference_context.as_undet_var(ctx, syms, types, p2);
                                if !types.is_same_type(ctx, syms, u1, u2) {
                                    infer.report_bound_error_upper(ctx, syms, types, *uv)?;
                                }
                            }
                            all1 = all1.tail();
                            all2 = all2.tail();
                        }
                        assert!(
                            all1.is_empty() && all2.is_empty(),
                            "AssertionError: CheckUpperBounds arity"
                        );
                        supers_current = supers_current.tail();
                    }
                }
                bounds_current = bounds_current.tail();
            }
            Ok(())
        }
        IncorporationAction::PropagateBounds { uv, t, ib } => {
            let undet_t = inference_context.as_undet_var(ctx, syms, types, *t);
            if ctx.typ(undet_t).has_tag(TypeTag::UNDETVAR)
                && !types.undet_var_is_captured(ctx, undet_t)
            {
                // symmetric propagation
                types.undet_var_add_bound(
                    ctx,
                    syms,
                    undet_t,
                    ib.complement(),
                    *uv,
                    false,
                );
                // backwards propagation
                for ib2 in propagate_backwards(*ib) {
                    let mut bounds = types.undet_var_get_bounds(ctx, undet_t, &[ib2]);
                    while bounds.non_empty() {
                        types.undet_var_add_bound(ctx, syms, *uv, ib2, *bounds.head(), false);
                        bounds = bounds.tail();
                    }
                }
            }
            // forward propagation
            for ib2 in propagate_forward(*ib) {
                let mut bounds = types.undet_var_get_bounds(ctx, *uv, &[ib2]);
                while bounds.non_empty() {
                    let undet =
                        inference_context.as_undet_var(ctx, syms, types, *bounds.head());
                    if ctx.typ(undet).has_tag(TypeTag::UNDETVAR)
                        && !types.undet_var_is_captured(ctx, undet)
                    {
                        let t_inst = inference_context.as_inst_type(ctx, syms, types, *t);
                        types.undet_var_add_bound(ctx, syms, undet, *ib, t_inst, false);
                    }
                    bounds = bounds.tail();
                }
            }
            Ok(())
        }
    }
}

fn propagate_forward(ib: InferenceBound) -> Vec<InferenceBound> {
    if ib == InferenceBound::EQ {
        vec![InferenceBound::EQ]
    } else {
        IncorporationAction::complement_of(ib)
    }
}

fn propagate_backwards(ib: InferenceBound) -> Vec<InferenceBound> {
    if ib == InferenceBound::EQ {
        InferenceBound::values().to_vec()
    } else {
        vec![ib]
    }
}

fn check_bounds_to_check(from: InferenceBound) -> Vec<InferenceBound> {
    if from == InferenceBound::EQ {
        InferenceBound::values().to_vec()
    } else {
        IncorporationAction::complement_of(from)
    }
}

fn type_func_apply(
    func: TypeFunc,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    t: TypeId,
) -> TypeId {
    match func {
        TypeFunc::AsUndetVar => inference_context.as_undet_var(ctx, syms, types, t),
        TypeFunc::AsInstType => inference_context.as_inst_type(ctx, syms, types, t),
    }
}

fn type_filter_test(
    filter: TypeFilter,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    t: TypeId,
) -> bool {
    match filter {
        TypeFilter::Free => inference_context.free(ctx, syms, types, t),
    }
}


// =====================================================================
// Infer
// =====================================================================

/// Java: public class Infer (the ported subset; see the file header).
pub struct Infer {
    /// Java: boolean allowGraphInference
    pub allow_graph_inference: bool,
    /// Java: JCDiagnostic.Factory diags
    pub diags: Rc<Factory>,
    /// Java: final Names names (upstream from the constructor; needed by
    /// makeIntersectionType in instantiateAsUninferredVars)
    pub names: Rc<Names>,
    /// Java: public static final Type anyPoly = new JCNoType()
    /// (Infer.java:132; a dedicated NONE instance compared by identity)
    pub any_poly: TypeId,
    /// Diagnostic services used by the deferred-instantiation verbose note
    /// (installed by the compiler driver).
    pub log: std::cell::RefCell<Option<Rc<RefCell<crate::util::log::Log>>>>,
    /// Java: Resolve.verboseResolutionMode (the DEFERRED_INST members).
    pub verbose_resolution_mode:
        std::cell::RefCell<Vec<crate::comp::resolve::VerboseResolutionMode>>,
}

impl Infer {
    /// Ported service construction (ADR 0001: the Java instance(Context) /
    /// protected constructor registration is registered in the file header).
    pub fn new(
        ctx: &mut CompilerContext,
        allow_graph_inference: bool,
        diags: Rc<Factory>,
        names: Rc<Names>,
    ) -> Rc<Infer> {
        let any_poly = ctx.alloc_type(crate::code::type_::Type::new(
            TypeTag::NONE,
            None,
            TypeKindData::JCNoType { kind: crate::code::type_::JCNoTypeKind::Base },
        ));
        Rc::new(Infer {
            allow_graph_inference,
            diags,
            names,
            any_poly,
            log: std::cell::RefCell::new(None),
            verbose_resolution_mode: std::cell::RefCell::new(Vec::new()),
        })
    }

    /// Install the diagnostic services used by deferred-instantiation notes.
    pub fn install_diagnostics(
        &self,
        log: Rc<RefCell<crate::util::log::Log>>,
        verbose_resolution_mode: Vec<crate::comp::resolve::VerboseResolutionMode>,
    ) {
        *self.log.borrow_mut() = Some(log);
        *self.verbose_resolution_mode.borrow_mut() = verbose_resolution_mode;
    }

    /// Java: InferenceException error(JCDiagnostic diag)
    pub fn error(&self, diag: Option<JCDiagnostic>) -> InferenceException {
        let mut result = InferenceException::new();
        if let Some(diag) = diag {
            result.messages = result.messages.append(diag);
        }
        result
    }

    /// Java: TypeMapping fromTypeVarFun (Infer.java:616-630): map a TypeVar to
    /// a fresh UndetVar carrying the incorporation engine as listener.
    #[allow(clippy::collapsible_if)]
    pub fn from_type_var_fun(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        tv: TypeId,
    ) -> TypeId {

        let listener = self.incorporation_engine();
        let uv = types.undet_var_new(ctx, syms, tv, Some(listener));
        let tsym = ctx.typ(tv).tsym;
        if let Some(tsym) = tsym {
            if (ctx.sym(tsym).flags_field & flags::THROWS) != 0 {
                types.undet_var_set_throw(ctx, uv);
            }
        }
        uv
    }

    /// Java: AbstractIncorporationEngine incorporationEngine()
    pub fn incorporation_engine(&self) -> Rc<dyn UndetVarListener> {
        if self.allow_graph_inference {
            Rc::new(GraphEngine)
        } else {
            Rc::new(LegacyEngine)
        }
    }

    /// Java: void doIncorporation(InferenceContext inferenceContext,
    /// Warner warn) (Infer.java:1093-1118)
    pub fn do_incorporation(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
        warn: &mut Warner,
    ) -> Result<(), InferenceException> {
        let mut progress = true;
        let mut round = 0;
        while progress && round < MAX_INCORPORATION_STEPS {
            progress = false;
            let mut undets = inference_context.undet_vars();
            while undets.non_empty() {
                let uv = *undets.head();
                let actions = types.undet_var_actions(ctx, uv);
                if !actions.is_empty() {
                    progress = true;
                    let action = types.undet_var_pop_action(ctx, uv);
                    if let Some(action) = action {
                        incorporation_action_apply(
                            self,
                            &action,
                            ctx,
                            syms,
                            types,
                            inference_context,
                            warn,
                        )?;
                    }
                }
                undets = undets.tail();
            }
            round += 1;
        }
        Ok(())
    }

    /// Java: private List<Pair<Type, Type>> getParameterizedSupers(Type t,
    /// Type s) (Infer.java:1128-1146)
    pub fn get_parameterized_supers(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
        s: TypeId,
    ) -> List<Pair<TypeId, TypeId>> {
        let names = Rc::clone(&self.names);
        let lub_result = types.lub(ctx, syms, &names, &[t, s]);
        if lub_result == syms.err_type || lub_result == syms.bot_type {
            return List::nil();
        }
        let supertypes_to_check = if ctx.typ(lub_result).is_intersection() {
            components_of(ctx, lub_result)
        } else {
            List::of(lub_result)
        };
        let mut common_supertypes: List<Pair<TypeId, TypeId>> = List::nil();
        let mut current = supertypes_to_check;
        while current.non_empty() {
            let sup = *current.head();
            if types.is_parameterized_type_of(ctx, sup) {
                let as_super_of_t = self.as_super(ctx, syms, types, t, sup);
                let as_super_of_s = self.as_super(ctx, syms, types, s, sup);
                if let (Some(a), Some(b)) = (as_super_of_t, as_super_of_s) {
                    common_supertypes = common_supertypes.append(Pair { fst: a, snd: b });
                }
            }
            current = current.tail();
        }
        common_supertypes
    }

    /// Java: private Type asSuper(Type t, Type sup) (Infer.java:1147-1151)
    fn as_super(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
        sup: TypeId,
    ) -> Option<TypeId> {
        if ctx.typ(sup).has_tag(TypeTag::ARRAY) {
            let elem_t = elemtype_of(ctx, t);
            let elem_sup = elemtype_of(ctx, sup);
            let inner = self.as_super(ctx, syms, types, elem_t, elem_sup)?;
            return Some(new_array_type_ctx(ctx, inner, syms.array_class));
        }
        let sup_tsym = ctx.typ(sup).tsym?;
        types.as_super(ctx, syms, t, sup_tsym)
    }

    /// Java: void instantiateAsUninferredVars(List<Type> vars,
    /// InferenceContext inferenceContext) (Infer.java:519-545)
    pub fn instantiate_as_uninferred_vars(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        vars: List<TypeId>,
        inference_context: &InferenceContext,
    ) -> Result<(), InferenceException> {
        let mut todo: List<TypeId> = List::nil();
        // step 1 - create fresh tvars
        let mut current = vars.clone();
        while current.non_empty() {
            let t = *current.head();
            let uv = inference_context.as_undet_var(ctx, syms, types, t);
            let upper_bounds = types.undet_var_get_bounds(ctx, uv, &[InferenceBound::UPPER]);
            if contains_any_list(ctx, syms, types, upper_bounds.clone(), vars.clone()) {
                let qtype = types.undet_var_qtype(ctx, uv);
                let qtype_tsym = ctx.typ(qtype).tsym.expect("undet var qtype symbol");
                let qtype_name = ctx.sym(qtype_tsym).name.clone();
                let owner = ctx.sym(qtype_tsym).owner.expect("qtype owner");
                let fresh_tvar = new_type_variable_symbol(ctx, syms, qtype_name, owner);
                let names = Rc::clone(&self.names);
                let intersection = types.make_intersection_type(
                    ctx,
                    syms,
                    &names,
                    upper_bounds,
                    false,
                );
                let fresh_type = ctx.alloc_type(crate::code::type_::Type::new(
                    TypeTag::TYPEVAR,
                    Some(fresh_tvar),
                    TypeKindData::TypeVar {
                        bound: Some(intersection),
                        lower: Some(syms.bot_type),
                    },
                ));
                ctx.sym_mut(fresh_tvar).ty = Some(fresh_type);
                todo = todo.append(uv);
                types.undet_var_set_inst(ctx, syms, uv, fresh_type);
            } else if upper_bounds.non_empty() {
                let glb = types.glb_list(ctx, syms, upper_bounds);
                types.undet_var_set_inst(ctx, syms, uv, glb);
            } else {
                types.undet_var_set_inst(ctx, syms, uv, syms.object_type);
            }
            current = current.tail();
        }
        // step 2 - replace fresh tvars in their bounds
        let mut current = todo;
        while current.non_empty() {
            let uv = *current.head();
            let inst = types
                .undet_var_get_inst(ctx, uv)
                .expect("fresh tvar instance");
            let ct_upper = ctx.upper_bound_of(inst).expect("tvar upper bound");
            // Java: types.getBounds(ct) (Infer.java:541). Keep compound
            // components; fall back to the upper bound only when the local
            // bound list is empty (glb(nil) otherwise).
            let mut bounds = types.get_bounds_of_type(ctx, syms, inst);
            if bounds.is_empty() {
                bounds = crate::util::list::List::of(ct_upper);
            }
            let as_inst = inference_context.as_inst_types(ctx, syms, types, bounds);
            let glb = types.glb_list(ctx, syms, as_inst);
            if let TypeKindData::TypeVar { bound, .. } = &mut ctx.typ_mut(inst).kind {
                *bound = Some(glb);
            }
            if types.is_erroneous_type(ctx, syms, glb) {
                self.report_bound_error(ctx, syms, types, uv, InferenceBound::UPPER)?;
            }
            let _ = ct_upper;
            current = current.tail();
        }
        Ok(())
    }

    /// Java: void reportBoundError(UndetVar uv, InferenceBound ib)
    /// (Infer.java:1261-1272)
    pub fn report_bound_error(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        ib: InferenceBound,
    ) -> Result<(), InferenceException> {
        let _ = syms;
        match ib {
            InferenceBound::EQ => {
                let qtype = types.undet_var_qtype(ctx, uv);
                let bounds = types.undet_var_get_bounds(ctx, uv, &[ib]);
                let mut args = bounds_type_args(ctx, bounds);
                // Java: equality bounds are prepended by UndetVar.addBound, so
                // the port's insertion-ordered list is reported in reverse.
                args.reverse();
                let diag = self.diags.fragment(
                    fragments::incompatible_eq_bounds(
                        crate::comp::check::type_arg_with_where(ctx, qtype),
                        args,
                    )
                    .into(),
                );
                Err(self.error(Some(diag)))
            }
            InferenceBound::UPPER => {
                let qtype = types.undet_var_qtype(ctx, uv);
                let bounds = types.undet_var_get_bounds(ctx, uv, &[ib]);
                let diag = self.diags.fragment(
                    fragments::incompatible_upper_bounds(
                        crate::comp::check::type_arg_with_where(ctx, qtype),
                        bounds_type_args(ctx, bounds),
                    )
                    .into(),
                );
                Err(self.error(Some(diag)))
            }
            InferenceBound::LOWER => {
                panic!("AssertionError: this case shouldn't happen")
            }
        }
    }

    fn report_bound_error_upper(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
    ) -> Result<(), InferenceException> {
        self.report_bound_error(ctx, syms, types, uv, InferenceBound::UPPER)
    }
}

/// Java: Pair<Type,Type> used by getParameterizedSupers.
#[derive(Clone, Debug)]
pub struct Pair<A, B> {
    pub fst: A,
    pub snd: B,
}

/// Java: class AbstractIncorporationEngine (legacy engine; Infer.java:1065-1090)
struct LegacyEngine;

impl UndetVarListener for LegacyEngine {
    fn var_bound_changed(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        ib: InferenceBound,
        t: TypeId,
        _update: bool,
    ) {
        let _ = syms;
        let mut actions: Vec<IncorporationAction> = Vec::new();
        if types.undet_var_get_inst(ctx, uv).is_some() {
            actions.push(IncorporationAction::CheckInst {
                uv,
                to: vec![ib],
            });
        }
        actions.push(IncorporationAction::EqCheckLegacy { uv, t, from: ib });
        types.undet_var_add_actions(ctx, uv, actions);
    }

    fn var_instantiated(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        types: &Types,
        uv: TypeId,
    ) {
        types.undet_var_add_action_front(
            ctx,
            uv,
            IncorporationAction::SubstBounds { uv },
        );
    }
}

/// Java: graphEngine (the standard incorporation engine; Infer.java:1075-1090)
struct GraphEngine;

impl UndetVarListener for GraphEngine {
    fn var_bound_changed(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        types: &Types,
        uv: TypeId,
        ib: InferenceBound,
        t: TypeId,
        update: bool,
    ) {
        let mut actions: Vec<IncorporationAction> = Vec::new();
        if types.undet_var_get_inst(ctx, uv).is_some() {
            actions.push(IncorporationAction::CheckInst {
                uv,
                to: vec![ib],
            });
        }
        actions.push(IncorporationAction::CheckBounds {
            uv,
            t,
            type_func: TypeFunc::AsUndetVar,
            opt_filter: None,
            from: ib,
        });
        if update {
            types.undet_var_add_actions(ctx, uv, actions);
            return;
        }
        if ib == InferenceBound::UPPER {
            actions.push(IncorporationAction::CheckUpperBounds { uv, t });
        }
        actions.push(IncorporationAction::PropagateBounds { uv, t, ib });
        types.undet_var_add_actions(ctx, uv, actions);
    }

    fn var_instantiated(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        types: &Types,
        uv: TypeId,
    ) {
        types.undet_var_add_action_front(
            ctx,
            uv,
            IncorporationAction::SubstBounds { uv },
        );
    }
}

// =====================================================================
// InferenceContext
// =====================================================================

/// Java: public class InferenceContext (InferenceContext.java:88-606).
pub struct InferenceContext {
    inner: RefCell<InferenceContextState>,
    /// Java: Infer infer (None only for the shared empty context, which never
    /// solves; registered adapter).
    pub infer: Option<Rc<Infer>>,
}

/// The mutable state of an InferenceContext (upstream instance fields).
#[derive(Default)]
pub struct InferenceContextState {
    /// Java: List<Type> undetvars;
    pub undetvars: List<TypeId>,
    /// Java: List<Type> inferencevars;
    pub inferencevars: List<TypeId>,
    /// Java: Map<FreeTypeListener, List<Type>> freeTypeListeners
    /// (LinkedHashMap -> order-preserving Vec)
    pub free_type_listeners: Vec<(Rc<dyn FreeTypeListener>, List<TypeId>)>,
    /// Java: Map<JCTree, Type> captureTypeCache (HashMap -> Vec of pairs)
    pub capture_type_cache: Vec<(TreeId, TypeId)>,
}

impl InferenceContext {
    /// Java: public InferenceContext(Infer infer, List<Type> inferencevars,
    /// List<Type> undetvars) (the explicit-service adapter; the infer link is
    /// optional for the empty context).
    pub fn with_undet_vars(
        infer: Option<Rc<Infer>>,
        inferencevars: List<TypeId>,
        undetvars: List<TypeId>,
    ) -> Rc<InferenceContext> {
        Rc::new(InferenceContext {
            inner: RefCell::new(InferenceContextState {
                undetvars,
                inferencevars,
                free_type_listeners: Vec::new(),
                capture_type_cache: Vec::new(),
            }),
            infer,
        })
    }

    /// Java: public InferenceContext(Infer infer, List<Type> inferencevars)
    /// — inferencevars.map(infer.fromTypeVarFun).
    pub fn new(
        infer: Rc<Infer>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inferencevars: List<TypeId>,
    ) -> Rc<InferenceContext> {
        if std::env::var_os("DSH_DEBUG_NEW").is_some() {
            let names: Vec<String> = inferencevars
                .iter()
                .map(|t| crate::comp::attr::type_display_of(ctx, t))
                .collect();
            let mut dup: Vec<String> = Vec::new();
            let mut seen: Vec<String> = Vec::new();
            for n in &names {
                if seen.contains(n) && !dup.contains(n) {
                    dup.push(n.clone());
                }
                seen.push(n.clone());
            }
            eprintln!("[NEW] vars=[{}] dup=[{}]", names.join(","), dup.join(","));
        }
        let mut undetvars: List<TypeId> = List::nil();
        let mut current = inferencevars.clone();
        while current.non_empty() {
            let t = *current.head();
            let uv = infer.from_type_var_fun(ctx, syms, types, t);
            undetvars = undetvars.append(uv);
            current = current.tail();
        }
        InferenceContext::with_undet_vars(Some(infer), inferencevars, undetvars)
    }

    /// Java: addVar(TypeVar t)
    pub fn add_var(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) {
        let infer = self.infer.clone().expect("InferenceContext.infer");
        let uv = infer.from_type_var_fun(ctx, syms, types, t);
        let mut state = self.inner.borrow_mut();
        state.undetvars = state.undetvars.prepend(uv);
        state.inferencevars = state.inferencevars.prepend(t);
    }

    /// Java: List<Type> inferenceVars()
    pub fn inference_vars(&self) -> List<TypeId> {
        self.inner.borrow().inferencevars.clone()
    }

    /// Java: public List<Type> undetVars()
    pub fn undet_vars(&self) -> List<TypeId> {
        self.inner.borrow().undetvars.clone()
    }

    /// Java: List<Type> restvars()
    pub fn restvars(&self, ctx: &mut CompilerContext, syms: &Symtab, types: &Types) -> List<TypeId> {
        self.filter_vars(ctx, syms, types, &|ctx, uv, types| {
            types.undet_var_get_inst(ctx, uv).is_none()
        })
    }

    /// Java: List<Type> instvars()
    pub fn instvars(&self, ctx: &mut CompilerContext, syms: &Symtab, types: &Types) -> List<TypeId> {
        self.filter_vars(ctx, syms, types, &|ctx, uv, types| {
            types.undet_var_get_inst(ctx, uv).is_some()
        })
    }

    /// Java: final List<Type> boundedVars()
    pub fn bounded_vars(&self, ctx: &mut CompilerContext, syms: &Symtab, types: &Types) -> List<TypeId> {
        self.filter_vars(ctx, syms, types, &|ctx, uv, types| {
            let upper = types.undet_var_get_bounds(ctx, uv, &[InferenceBound::UPPER]);
            let declared = types.undet_var_get_declared_bounds(ctx, uv);
            let mut extra: List<TypeId> = List::nil();
            let mut current = upper;
            while current.non_empty() {
                let t = *current.head();
                if !declared.iter().any(|d| d == t) {
                    extra = extra.append(t);
                }
                current = current.tail();
            }
            let eq_lower = types.undet_var_get_bounds(
                ctx,
                uv,
                &[InferenceBound::EQ, InferenceBound::LOWER],
            );
            !extra.is_empty() || !eq_lower.is_empty()
        })
    }

    /// Java: private List<Type> filterVars(Predicate<UndetVar> fu)
    fn filter_vars(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        predicate: &dyn Fn(&mut CompilerContext, TypeId, &Types) -> bool,
    ) -> List<TypeId> {
        let undetvars = self.inner.borrow().undetvars.clone();
        let mut result: List<TypeId> = List::nil();
        let mut current = undetvars;
        while current.non_empty() {
            let uv = *current.head();
            if predicate(ctx, uv, types) {
                result = result.append(types.undet_var_qtype(ctx, uv));
            }
            current = current.tail();
        }
        let _ = syms;
        result
    }

    /// Java: final boolean free(Type t)
    pub fn free(&self, ctx: &mut CompilerContext, syms: &Symtab, types: &Types, t: TypeId) -> bool {
        let inferencevars = self.inner.borrow().inferencevars.clone();
        types.contains_any(ctx, syms, t, &inferencevars)
    }

    /// Java: final boolean free(List<Type> ts)
    pub fn free_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        ts: List<TypeId>,
    ) -> bool {
        let mut current = ts;
        while current.non_empty() {
            if self.free(ctx, syms, types, *current.head()) {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// Java: final List<Type> freeVarsIn(Type t)
    pub fn free_vars_in(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> List<TypeId> {
        let inferencevars = self.inner.borrow().inferencevars.clone();
        let mut result: List<TypeId> = List::nil();
        let mut current = inferencevars;
        while current.non_empty() {
            let iv = *current.head();
            if types.contains(ctx, syms, t, iv) {
                result = result.append(iv);
            }
            current = current.tail();
        }
        result
    }

    /// Java: final List<Type> freeVarsIn(List<Type> ts)
    pub fn free_vars_in_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        ts: List<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            let t = *current.head();
            let vars = self.free_vars_in(ctx, syms, types, t);
            let mut vars_current = vars;
            while vars_current.non_empty() {
                let var = *vars_current.head();
                if !result.iter().any(|existing| existing == var) {
                    result = result.append(var);
                }
                vars_current = vars_current.tail();
            }
            current = current.tail();
        }
        result
    }

    /// Java: public final Type asUndetVar(Type t)
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn as_undet_var(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> TypeId {
        let (inferencevars, undetvars) = {
            let state = self.inner.borrow();
            (state.inferencevars.clone(), state.undetvars.clone())
        };
        let result = types.subst(ctx, syms, &List::of(t), &inferencevars, &undetvars);
        *result.head()
    }

    /// Java: final List<Type> asUndetVars(List<Type> ts)
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn as_undet_vars(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        ts: List<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            result = result.append(self.as_undet_var(ctx, syms, types, *current.head()));
            current = current.tail();
        }
        result
    }

    /// Java: List<Type> instTypes()
    pub fn inst_types(&self, ctx: &mut CompilerContext, types: &Types) -> List<TypeId> {
        let undetvars = self.inner.borrow().undetvars.clone();
        let mut result: List<TypeId> = List::nil();
        let mut current = undetvars;
        while current.non_empty() {
            let uv = *current.head();
            let inst = types
                .undet_var_get_inst(ctx, uv)
                .unwrap_or_else(|| types.undet_var_qtype(ctx, uv));
            result = result.append(inst);
            current = current.tail();
        }
        result
    }

    /// Java: Type asInstType(Type t)
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn as_inst_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> TypeId {
        let (inferencevars, undetvars) = {
            let state = self.inner.borrow();
            (state.inferencevars.clone(), state.undetvars.clone())
        };
        let inst_types = self.inst_types(ctx, types);
        let _ = undetvars;
        let result = types.subst(ctx, syms, &List::of(t), &inferencevars, &inst_types);
        *result.head()
    }

    /// Java: List<Type> asInstTypes(List<Type> ts)
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn as_inst_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        ts: List<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            result = result.append(self.as_inst_type(ctx, syms, types, *current.head()));
            current = current.tail();
        }
        result
    }

    /// Java: void addFreeTypeListener(List<Type> types, FreeTypeListener ftl)
    pub fn add_free_type_listener(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        type_list: List<TypeId>,
        listener: Rc<dyn FreeTypeListener>,
    ) {
        let free_vars = self.free_vars_in_list(ctx, syms, types, type_list);
        self.inner
            .borrow_mut()
            .free_type_listeners
            .push((listener, free_vars));
    }

    /// Java: void notifyChange()
    pub fn notify_change(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
    ) -> Result<(), InferenceException> {
        let restvars = self.restvars(ctx, syms, types);
        let inferencevars = self.inner.borrow().inferencevars.clone();
        let mut inferred_vars: List<TypeId> = List::nil();
        let mut current = inferencevars;
        while current.non_empty() {
            let t = *current.head();
            if !restvars.iter().any(|r| r == t) {
                inferred_vars = inferred_vars.append(t);
            }
            current = current.tail();
        }
        self.notify_change_list(ctx, syms, types, inferred_vars)
    }

    /// Java: void notifyChange(List<Type> inferredVars)
    pub fn notify_change_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inferred_vars: List<TypeId>,
    ) -> Result<(), InferenceException> {
        let listeners: Vec<(Rc<dyn FreeTypeListener>, List<TypeId>)> = {
            let state = self.inner.borrow();
            state.free_type_listeners.clone()
        };
        let inferencevars = self.inner.borrow().inferencevars.clone();
        let mut remaining_vars: List<TypeId> = List::nil();
        let mut current = inferencevars;
        while current.non_empty() {
            let t = *current.head();
            if !inferred_vars.iter().any(|i| i == t) {
                remaining_vars = remaining_vars.append(t);
            }
            current = current.tail();
        }
        let mut thrown: Option<InferenceException> = None;
        let mut kept: Vec<(Rc<dyn FreeTypeListener>, List<TypeId>)> = Vec::new();
        for (listener, vars) in listeners {
            let mut still_free = false;
            let mut vars_current = vars.clone();
            while vars_current.non_empty() {
                let v = *vars_current.head();
                if contains_any_list(ctx, syms, types, remaining_vars.clone(), List::of(v)) {
                    still_free = true;
                    break;
                }
                vars_current = vars_current.tail();
            }
            if still_free {
                kept.push((listener, vars.clone()));
            } else {
                match listener.types_inferred(ctx, syms, types, self) {
                    Ok(()) => {}
                    Err(ex) => {
                        if thrown.is_none() {
                            thrown = Some(ex);
                        }
                    }
                }
            }
        }
        self.inner.borrow_mut().free_type_listeners = kept;
        match thrown {
            Some(ex) => Err(ex),
            None => Ok(()),
        }
    }

    /// Java: public List<Type> save()
    pub fn save(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
    ) -> List<TypeId> {
        let undetvars = self.inner.borrow().undetvars.clone();
        let mut result: List<TypeId> = List::nil();
        let mut current = undetvars;
        while current.non_empty() {
            let uv = *current.head();
            result = result.append(types.undet_var_dup(ctx, syms, uv));
            current = current.tail();
        }
        result
    }

    /// Java: public void rollback(List<Type> saved_undet)
    pub fn rollback(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        saved_undet: List<TypeId>,
    ) {
        let mut new_undet_vars: List<TypeId> = List::nil();
        let mut new_inference_vars: List<TypeId> = List::nil();
        let mut saved = saved_undet;
        loop {
            let (uv, uv_saved) = {
                let state = self.inner.borrow();
                if saved.is_empty() || state.undetvars.is_empty() {
                    break;
                }
                (*state.undetvars.head(), *saved.head())
            };
            let qtype = types.undet_var_qtype(ctx, uv);
            let saved_qtype = types.undet_var_qtype(ctx, uv_saved);
            if qtype == saved_qtype {
                types.undet_var_dup_to(ctx, syms, uv_saved, uv);
                let mut state = self.inner.borrow_mut();
                state.undetvars = state.undetvars.tail();
                saved = saved.tail();
                new_undet_vars = new_undet_vars.append(uv);
                new_inference_vars = new_inference_vars.append(qtype);
            } else {
                let mut state = self.inner.borrow_mut();
                state.undetvars = state.undetvars.tail();
            }
        }
        let mut state = self.inner.borrow_mut();
        state.undetvars = new_undet_vars;
        state.inferencevars = new_inference_vars;
    }

    /// Java: void dupTo(final InferenceContext that, boolean clone)
    pub fn dup_to(
        self: &Rc<Self>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        that: &Rc<InferenceContext>,
        clone_undets: bool,
    ) {
        let this_inferencevars = self.inner.borrow().inferencevars.clone();
        if std::env::var_os("DSH_DEBUG_DUP").is_some() {
            let names: Vec<String> = this_inferencevars
                .iter()
                .map(|t| crate::comp::attr::type_display_of(ctx, t))
                .collect();
            eprintln!("[DUP] this=[{}]", names.join(","));
        }
        {
            let mut that_state = that.inner.borrow_mut();
            let mut missing: List<TypeId> = List::nil();
            let mut current = this_inferencevars.clone();
            while current.non_empty() {
                let t = *current.head();
                // Skip variables that are already present in either the target
                // context or this batch: upstream appends inferencevars.diff(...)
                // (a set difference), so a repeated entry in this context must not
                // be propagated twice (InferenceContext.java:330-333).
                if !that_state.inferencevars.iter().any(|existing| existing == t)
                    && !missing.iter().any(|existing| existing == t)
                {
                    missing = missing.append(t);
                }
                current = current.tail();
            }
            that_state.inferencevars = that_state.inferencevars.append_list(&missing);
        }
        let undets_to_propagate = if clone_undets {
            self.save(ctx, syms, types)
        } else {
            self.inner.borrow().undetvars.clone()
        };
        {
            let mut that_state = that.inner.borrow_mut();
            let mut missing: List<TypeId> = List::nil();
            let mut current = undets_to_propagate;
            while current.non_empty() {
                let t = *current.head();
                let mut present = false;
                let mut existing = that_state.undetvars.clone();
                while existing.non_empty() {
                    if *existing.head() == t {
                        present = true;
                        break;
                    }
                    existing = existing.tail();
                }
                if !present {
                    missing = missing.append(t);
                }
                current = current.tail();
            }
            that_state.undetvars = that_state.undetvars.append_list(&missing);
        }
        // set up listeners to notify the original inference context as
        // propagated vars are inferred in the new context
        let mut current = this_inferencevars;
        while current.non_empty() {
            let t = *current.head();
            let listener = Rc::new(NotifyChangeListener {
                target: Rc::clone(self),
            });
            that.inner
                .borrow_mut()
                .free_type_listeners
                .push((listener, List::of(t)));
            current = current.tail();
        }
    }

    /// Java: InferenceContext min(List<Type> roots, boolean shouldSolve,
    /// Warner warn) (InferenceContext.java:337-405)
    pub fn min(
        self: &Rc<Self>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        roots: List<TypeId>,
        should_solve: bool,
        warn: &Rc<RefCell<Warner>>,
    ) -> Result<Rc<InferenceContext>, InferenceException> {
        let inferencevars = self.inner.borrow().inferencevars.clone();
        if roots.length() == inferencevars.length() {
            return Ok(Rc::clone(self));
        }
        // if any of the inference vars is a captured variable bail out
        let mut current = inferencevars;
        while current.non_empty() {
            let iv = *current.head();
            if ctx.typ(iv).has_tag(TypeTag::TYPEVAR) && ctx.typ(iv).is_captured() {
                return Ok(Rc::clone(self));
            }
            current = current.tail();
        }
        let mut reachability = ReachabilityVisitor::new(self);
        reachability.scan(ctx, syms, types, roots)?;
        if reachability.min.len() == self.inner.borrow().inferencevars.length() as usize {
            return Ok(Rc::clone(self));
        }
        let min_vars = List::from_iterable(reachability.min.clone());
        let redundant_vars = list_diff(&self.inner.borrow().inferencevars, &min_vars);
        // compute new undet variables
        let mut min_undet_vars: List<TypeId> = List::nil();
        let mut current = min_vars.clone();
        while current.non_empty() {
            let min_var = *current.head();
            let uv = self.as_undet_var(ctx, syms, types, min_var);
            assert!(
                types.undet_var_actions(ctx, uv).is_empty(),
                "AssertionError: min requires empty incorporation actions"
            );
            let uv2 = types.undet_var_dup(ctx, syms, uv);
            for ib in InferenceBound::values() {
                let bounds = types.undet_var_get_bounds(ctx, uv, &[ib]);
                let mut filtered: List<TypeId> = List::nil();
                let mut bounds_current = bounds;
                while bounds_current.non_empty() {
                    let b = *bounds_current.head();
                    if !redundant_vars.iter().any(|r| r == b) {
                        filtered = filtered.append(b);
                    }
                    bounds_current = bounds_current.tail();
                }
                types.undet_var_set_bounds(ctx, uv2, ib, filtered);
            }
            min_undet_vars = min_undet_vars.append(uv2);
            current = current.tail();
        }
        let min_context = InferenceContext::with_undet_vars(Some(infer_clone(self)), min_vars.clone(), min_undet_vars);
        // add listener that forwards notifications to the original context
        let min_inferencevars = min_context.inner.borrow().inferencevars.clone();
        let mut current = min_inferencevars;
        while current.non_empty() {
            let t = *current.head();
            let original = Rc::downgrade(&Rc::clone(self));
            let min_map = reachability.min_map.clone();
            let listener = Rc::new(PropagateListener {
                original,
                var: t,
                min_map,
                warn: Rc::clone(warn),
            });
            min_context
                .inner
                .borrow_mut()
                .free_type_listeners
                .push((listener, List::of(t)));
            current = current.tail();
        }
        if should_solve {
            // solve definitively unreachable variables
            let mut equiv: List<TypeId> = List::nil();
            for t in &reachability.equiv {
                equiv = equiv.append(*t);
            }
            let unreachable_vars = list_diff(&redundant_vars, &equiv);
            let original = Rc::downgrade(&Rc::clone(self));
            let min_vars2 = min_context.inner.borrow().inferencevars.clone();
            let listener = Rc::new(UnreachableVarsListener {
                original,
                unreachable_vars,
                warn: Rc::clone(warn),
            });
            min_context
                .inner
                .borrow_mut()
                .free_type_listeners
                .push((listener, min_vars2));
        }
        Ok(min_context)
    }

    /// Java: Type cachedCapture(JCTree tree, Type t, boolean readOnly)
    pub fn cached_capture(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        tree: TreeId,
        t: TypeId,
        read_only: bool,
    ) -> TypeId {
        {
            let state = self.inner.borrow();
            for (key, captured) in &state.capture_type_cache {
                if *key == tree {
                    return *captured;
                }
            }
        }
        let result = types.capture(ctx, syms, t);
        if result != t && !read_only {
            self.inner
                .borrow_mut()
                .capture_type_cache
                .push((tree, result));
        }
        result
    }

    /// Java: public void solve(Warner warn)
    pub fn solve(
        self: &Rc<InferenceContext>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        warn: &mut Warner,
    ) -> Result<(), InferenceException> {
        let strategy = Rc::new(LeafSolver {
            context: Rc::downgrade(self),
        });
        self.solve_with_strategy(ctx, syms, types, strategy, warn)
    }

    /// Java: public void solve(final List<Type> vars, Warner warn)
    pub fn solve_list(
        self: &Rc<InferenceContext>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        vars: List<TypeId>,
        warn: &mut Warner,
    ) -> Result<(), InferenceException> {
        let strategy = Rc::new(BestLeafSolver {
            context: Rc::downgrade(self),
            vars_to_solve: vars,
            tree_cache: RefCell::new(Vec::new()),
        });
        self.solve_with_strategy(ctx, syms, types, strategy, warn)
    }

    /// Java: public void solveAny(List<Type> varsToSolve, Warner warn)
    pub fn solve_any(
        self: &Rc<InferenceContext>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        vars_to_solve: List<TypeId>,
        warn: &mut Warner,
    ) -> Result<(), InferenceException> {
        let restvars = self.restvars(ctx, syms, types);
        let mut intersect: List<TypeId> = List::nil();
        let mut current = vars_to_solve;
        while current.non_empty() {
            let t = *current.head();
            if restvars.iter().any(|r| r == t) {
                intersect = intersect.append(t);
            }
            current = current.tail();
        }
        // Java: if (!intersect.isEmpty()) solve(intersect, warn)
        // (InferenceContext.java:517-525) -- an empty intersection is a no-op,
        // not a best-leaf solve over an empty variable set.
        if intersect.is_empty() {
            return Ok(());
        }
        let strategy = Rc::new(BestLeafSolver {
            context: Rc::downgrade(self),
            vars_to_solve: intersect,
            tree_cache: RefCell::new(Vec::new()),
        });
        self.solve_with_strategy(ctx, syms, types, strategy, warn)
    }

    /// Java: private void solve(GraphStrategy ss, Warner warn)
    fn solve_with_strategy(
        self: &Rc<InferenceContext>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        strategy: Rc<dyn GraphStrategy>,
        warn: &mut Warner,
    ) -> Result<(), InferenceException> {
        let infer = self.infer.clone().expect("InferenceContext.infer");
        let mut solver = GraphSolver::new(self);
        solver.solve(ctx, syms, types, &infer, strategy, warn)
    }

    /// Java: List<Type> solveBasic(List<Type> varsToSolve,
    /// EnumSet<InferenceStep> steps) (InferenceContext.java:532-552)
    pub fn solve_basic(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        vars_to_solve: List<TypeId>,
        steps: &[InferenceStep],
    ) -> Result<List<TypeId>, InferenceException> {
        let restvars = self.restvars(ctx, syms, types);
        if std::env::var_os("DSH_DEBUG_SB").is_some() {
            eprintln!("[SB] ctxvars={} tosolve={}",
                self.inner.borrow().inferencevars.length(),
                vars_to_solve.length());
        }
        let infer = self.infer.clone().expect("InferenceContext.infer");
        let mut solved_vars: List<TypeId> = List::nil();
        let mut current = vars_to_solve;
        while current.non_empty() {
            let t = *current.head();
            if restvars.iter().any(|r| r == t) {
                let uv = self.as_undet_var(ctx, syms, types, t);
                for step in steps {
                    if step.accepts(ctx, syms, types, uv, self)? {
                        let inst = step.solve(&infer, ctx, syms, types, uv, self)?;
                        types.undet_var_set_inst(
                            ctx,
                            syms,
                            uv,
                            inst,
                        );
                        solved_vars = solved_vars.append(types.undet_var_qtype(ctx, uv));
                        break;
                    }
                }
            }
            current = current.tail();
        }
        Ok(solved_vars)
    }

    /// Java: public void solveLegacy(boolean partial, Warner warn,
    /// EnumSet<InferenceStep> steps) (InferenceContext.java:554-576)
    pub fn solve_legacy(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        partial: bool,
        steps: &[InferenceStep],
        warn: &mut Warner,
    ) -> Result<(), InferenceException> {
        let infer = self.infer.clone().expect("InferenceContext.infer");
        loop {
            let solved_vars = self.solve_basic(ctx, syms, types, self.inference_vars(), steps)?;
            let restvars = self.restvars(ctx, syms, types);
            if restvars.is_empty() || partial {
                break;
            } else if solved_vars.is_empty() {
                infer.instantiate_as_uninferred_vars(ctx, syms, types, restvars, self)?;
                break;
            } else {
                let inst_types = self.as_inst_types(ctx, syms, types, solved_vars.clone());
                let mut undets = self.undet_vars();
                while undets.non_empty() {
                    let uv = *undets.head();
                    types.undet_var_subst_bounds(ctx, syms, uv, &solved_vars, &inst_types);
                    undets = undets.tail();
                }
            }
        }
        infer.do_incorporation(ctx, syms, types, self, warn)
    }
}

fn infer_clone(context: &InferenceContext) -> Rc<Infer> {
    context.infer.clone().expect("InferenceContext.infer")
}




// =====================================================================
// Inference engine: GraphStrategy/LeafSolver/BestLeafSolver/InferenceStep/
// GraphSolver/InferenceGraph (Infer.java:1299-1945)
// =====================================================================

// Java: final InferenceContext emptyContext (Infer.java:1943) — shared
// per-thread so that identity comparison (==) works like the Java field.
thread_local! {
    static EMPTY_CONTEXT: Rc<InferenceContext> =
        InferenceContext::with_undet_vars(None, List::nil(), List::nil());
}

/// Java: final InferenceContext emptyContext accessor.
pub fn empty_context() -> Rc<InferenceContext> {
    EMPTY_CONTEXT.with(Rc::clone)
}

/// Java: enum GraphInferenceSteps / LegacyInferenceSteps step sets
/// (Infer.java:1563-1598).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferenceStep {
    EQ,
    LOWER,
    THROWS,
    UPPER,
    UPPER_LEGACY,
    CAPTURED,
}

/// Java: LegacyInferenceSteps.EQ_LOWER / EQ_UPPER
pub fn legacy_inference_steps_eq_lower() -> Vec<InferenceStep> {
    vec![InferenceStep::EQ, InferenceStep::LOWER]
}

/// Java: LegacyInferenceSteps.EQ_UPPER
pub fn legacy_inference_steps_eq_upper() -> Vec<InferenceStep> {
    vec![InferenceStep::EQ, InferenceStep::UPPER_LEGACY]
}

/// Java: GraphInferenceSteps (Infer.java:1580-1598), in declaration order.
pub fn graph_inference_steps() -> Vec<Vec<InferenceStep>> {
    vec![
        vec![InferenceStep::EQ],
        vec![InferenceStep::EQ, InferenceStep::LOWER],
        vec![
            InferenceStep::EQ,
            InferenceStep::LOWER,
            InferenceStep::UPPER,
            InferenceStep::THROWS,
            InferenceStep::CAPTURED,
        ],
    ]
}

impl InferenceStep {
    /// Java: InferenceStep.ib
    pub fn ib(self) -> InferenceBound {
        match self {
            InferenceStep::EQ => InferenceBound::EQ,
            InferenceStep::LOWER => InferenceBound::LOWER,
            InferenceStep::THROWS => InferenceBound::UPPER,
            InferenceStep::UPPER => InferenceBound::UPPER,
            InferenceStep::UPPER_LEGACY => InferenceBound::UPPER,
            InferenceStep::CAPTURED => InferenceBound::UPPER,
        }
    }

    /// Java: public boolean accepts(UndetVar t, InferenceContext
    /// inferenceContext) (Infer.java:1546-1548 plus the overrides at
    /// 1453-1464/1494-1500/1509-1515)
    pub fn accepts(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException> {
        match self {
            InferenceStep::THROWS => {
                if !types.undet_var_is_throws(ctx, uv) {
                    return Ok(false);
                }
                let mut bounds =
                    types.undet_var_get_bounds(ctx, uv, &[InferenceBound::UPPER]);
                while bounds.non_empty() {
                    let b = *bounds.head();
                    if !inference_context.free(ctx, syms, types, b)
                        && !types.is_subtype(
                            ctx,
                            syms,
                            syms.runtime_exception_type,
                            b,
                        )
                    {
                        return Ok(false);
                    }
                    bounds = bounds.tail();
                }
                Ok(true)
            }
            InferenceStep::UPPER_LEGACY => {
                let bounds =
                    types.undet_var_get_bounds(ctx, uv, &[self.ib()]);
                Ok(!inference_context.free_list(ctx, syms, types, bounds)
                    && !types.undet_var_is_captured(ctx, uv))
            }
            InferenceStep::CAPTURED => {
                if !types.undet_var_is_captured(ctx, uv) {
                    return Ok(false);
                }
                let bounds = types.undet_var_get_bounds(
                    ctx,
                    uv,
                    &[InferenceBound::UPPER, InferenceBound::LOWER],
                );
                Ok(!inference_context.free_list(ctx, syms, types, bounds))
            }
            _ => {
                let filtered = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                Ok(!filtered.is_empty() && !types.undet_var_is_captured(ctx, uv))
            }
        }
    }

    /// Java: abstract Type solve(UndetVar uv, InferenceContext
    /// inferenceContext) (Infer.java:1425-1531)
    pub fn solve(
        &self,
        infer: &Infer,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        inference_context: &InferenceContext,
    ) -> Result<TypeId, InferenceException> {
        match self {
            InferenceStep::EQ => {
                let filtered = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                Ok(*filtered.head())
            }
            InferenceStep::LOWER => {
                let lobounds = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let owntype = if lobounds.tail().is_empty() {
                    *lobounds.head()
                } else {
                    let names = Rc::clone(&infer.names);
                    types.lub_list(ctx, syms, &names, lobounds)
                };
                if ctx.typ(owntype).is_primitive() || ctx.typ(owntype).has_tag(TypeTag::ERROR) {
                    let diag = infer.diags.fragment(
                        fragments::no_unique_minimal_instance_exists(
                            TypeArg::placeholder("type"),
                            vec![TypeArg::placeholder("type")],
                        )
                        .into(),
                    );
                    Err(infer.error(Some(diag)))
                } else {
                    Ok(owntype)
                }
            }
            InferenceStep::THROWS => Ok(syms.runtime_exception_type),
            InferenceStep::UPPER | InferenceStep::UPPER_LEGACY => {
                let hibounds = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let owntype = if hibounds.tail().is_empty() {
                    *hibounds.head()
                } else {
                    types.glb_list(ctx, syms, hibounds)
                };
                if ctx.typ(owntype).is_primitive() || ctx.typ(owntype).has_tag(TypeTag::ERROR) {
                    let diag = infer.diags.fragment(
                        fragments::no_unique_maximal_instance_exists(
                            TypeArg::placeholder("type"),
                            vec![TypeArg::placeholder("type")],
                        )
                        .into(),
                    );
                    Err(infer.error(Some(diag)))
                } else {
                    Ok(owntype)
                }
            }
            InferenceStep::CAPTURED => {
                let upper_filtered =
                    InferenceStep::UPPER.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let upper = if !upper_filtered.is_empty() {
                    InferenceStep::UPPER.solve(infer, ctx, syms, types, uv, inference_context)?
                } else {
                    syms.object_type
                };
                let lower_filtered =
                    InferenceStep::LOWER.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let lower = if !lower_filtered.is_empty() {
                    InferenceStep::LOWER.solve(infer, ctx, syms, types, uv, inference_context)?
                } else {
                    syms.bot_type
                };
                let qtype = types.undet_var_qtype(ctx, uv);
                let (name, owner, wildcard) = match &ctx.typ(qtype).kind {
                    TypeKindData::CapturedType { wildcard, .. } => {
                        let qtype_tsym = ctx.typ(qtype).tsym.expect("captured symbol");
                        (
                            ctx.sym(qtype_tsym).name.clone(),
                            ctx.sym(qtype_tsym).owner,
                            *wildcard,
                        )
                    }
                    _ => panic!("AssertionError: CAPTURED on a non-captured type"),
                };
                Ok(new_captured_type_ctx(
                    ctx, syms, name, owner, upper, lower, wildcard,
                ))
            }
        }
    }

    /// Java: List<Type> filterBounds(UndetVar uv, InferenceContext
    /// inferenceContext) (Infer.java:1553-1556)
    fn filter_bounds(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        inference_context: &InferenceContext,
    ) -> Result<List<TypeId>, InferenceException> {
        let bounds = types.undet_var_get_bounds(ctx, uv, &[self.ib()]);
        let mut result: List<TypeId> = List::nil();
        let mut current = bounds;
        while current.non_empty() {
            let t = *current.head();
            if bound_filter(ctx, syms, types, inference_context, t) {
                result = result.append(t);
            }
            current = current.tail();
        }
        Ok(result)
    }
}

/// Java: Infer.BoundFilter.test (Infer.java:1233-1245)
fn bound_filter(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    t: TypeId,
) -> bool {
    !types.is_erroneous_type(ctx, syms, t)
        && !inference_context.free(ctx, syms, types, t)
        && !ctx.typ(t).has_tag(TypeTag::BOT)
}

/// Java: interface GraphStrategy + NodeNotFoundException
/// (Infer.java:1299-1326)
pub trait GraphStrategy {
    fn pick_node(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        graph: &InferenceGraph,
        inference_context: &InferenceContext,
    ) -> Result<usize, NodeNotFoundException>;

    fn done(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException>;
}

/// Java: public static class NodeNotFoundException extends RuntimeException
#[derive(Debug)]
pub struct NodeNotFoundException {
    pub node_count: usize,
}

/// Java: abstract class LeafSolver (Infer.java:1309-1341)
pub struct LeafSolver {
    pub context: std::rc::Weak<InferenceContext>,
}

impl GraphStrategy for LeafSolver {
    fn pick_node(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &Types,
        graph: &InferenceGraph,
        _inference_context: &InferenceContext,
    ) -> Result<usize, NodeNotFoundException> {
        // The node list drops deleted entries; the port keeps stable ids with
        // None holes, so the first live node is returned here.
        match graph.nodes.iter().position(|node| node.is_some()) {
            Some(index) => Ok(index),
            None => Err(NodeNotFoundException {
                node_count: graph.nodes.len(),
            }),
        }
    }

    fn done(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException> {
        let _ = self.context.upgrade();
        Ok(inference_context.restvars(ctx, syms, types).is_empty())
    }
}

/// Java: abstract class BestLeafSolver extends LeafSolver (Infer.java:1319-1341)
/// Java: Map<Node, Pair<List<Node>, Integer>> treeCache (Inference.java:1357)
pub type TreeCache = RefCell<Vec<(usize, (Vec<usize>, i32))>>;

/// /// Java: Infer.GraphSolver.BestLeafSolver (Infer.java:1290-1330).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1290-1330.
///
pub struct BestLeafSolver {
    pub context: std::rc::Weak<InferenceContext>,
    pub vars_to_solve: List<TypeId>,
    pub tree_cache: TreeCache,
}

impl BestLeafSolver {
    /// Java: Pair<List<Node>, Integer> computeTreeToLeafs(Node n)
    fn compute_tree_to_leafs(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        graph: &InferenceGraph,
        node: usize,
    ) -> (Vec<usize>, i32) {
        if let Some((_, cached)) = self.tree_cache.borrow().iter().find(|(n, _)| *n == node) {
            return cached.clone();
        }
        let node_data = graph.nodes[node].as_ref().expect("live node");
        let result = if node_data.is_leaf(graph) {
            (vec![node], node_data.data.length())
        } else {
            let mut path = (vec![node], node_data.data.length());
            for n2 in node_data.get_all_dependencies(graph) {
                if n2 == node {
                    continue;
                }
                let subpath = self.compute_tree_to_leafs(ctx, syms, types, graph, n2);
                // Java: path.fst.prependList(subpath.fst) (Infer.java:1371) puts
                // the subpath *before* this node, so the resulting list starts at
                // a leaf. BestLeafSolver.pickNode then returns bestPath.fst.head
                // (Infer.java:1409) — the leaf, not the node it was reached
                // from. Appending the subpath instead would make every pick the
                // dependent node, solving it before the variables in its bounds
                // were instantiated.
                let mut combined = subpath.0.clone();
                combined.extend(path.0.clone());
                path = (combined, path.1 + subpath.1);
            }
            path
        };
        self.tree_cache.borrow_mut().push((node, result.clone()));
        let _ = (ctx, syms, types);
        result
    }
}

impl GraphStrategy for BestLeafSolver {
    fn pick_node(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        graph: &InferenceGraph,
        inference_context: &InferenceContext,
    ) -> Result<usize, NodeNotFoundException> {
        let _ = inference_context;
        self.tree_cache.borrow_mut().clear();
        let mut best: Option<(Vec<usize>, i32)> = None;
        for (index, node) in graph.nodes.iter().enumerate() {
            let Some(node) = node else { continue };
            if !contains_any_list(ctx, syms, types, node.data.clone(), self.vars_to_solve.clone()) {
                continue;
            }
            let path = self.compute_tree_to_leafs(ctx, syms, types, graph, index);
            if best.as_ref().map(|(_, cost)| path.1 < *cost).unwrap_or(true) {
                best = Some(path);
            }
        }
        match best {
            Some((path, _)) => Ok(path[0]),
            None => Err(NodeNotFoundException {
                node_count: graph.nodes.len(),
            }),
        }
    }

    fn done(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException> {
        let _ = self.context.upgrade();
        let as_inst = inference_context.as_inst_types(ctx, syms, types, self.vars_to_solve.clone());
        Ok(!inference_context.free_list(ctx, syms, types, as_inst))
    }
}

/// Java: enum DependencyKind implements GraphUtils.DependencyKind
/// (Infer.java:1601-1610; kept for the Porting rules mapping of the graph API)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyKind {
    BOUND,
    STUCK,
}

impl DependencyKind {
/// /// Java: Infer.DependencyKind.dotStyle (Infer.java:1350-1370).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1350-1370.
///
    pub fn dot_style(self) -> &'static str {
        match self {
            DependencyKind::BOUND => "dotted",
            DependencyKind::STUCK => "dashed",
        }
    }
}

/// Java: class GraphSolver (Infer.java:1623-1945)
pub struct GraphSolver<'a> {
    pub inference_context: &'a Rc<InferenceContext>,
}

impl<'a> GraphSolver<'a> {
/// /// Java: GraphSolver(InferenceContext) (Infer.java:1370-1390).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1370-1390.
///
    pub fn new(inference_context: &'a Rc<InferenceContext>) -> GraphSolver<'a> {
        GraphSolver { inference_context }
    }

    /// Java: void solve(GraphStrategy sstrategy) (Infer.java:1638-1666)
    pub fn solve(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        infer: &Infer,
        strategy: Rc<dyn GraphStrategy>,
        warn: &mut Warner,
    ) -> Result<(), InferenceException> {
        infer.do_incorporation(ctx, syms, types, self.inference_context, warn)?;
        let mut graph = InferenceGraph::new(ctx, syms, types, self.inference_context);
        let mut iterations = 0;
        while !strategy.done(ctx, syms, types, self.inference_context)? {
            iterations += 1;
            if iterations > 1_000_000 {
                return Err(infer.error(None));
            }
            let node_to_solve = match strategy.pick_node(
                ctx,
                syms,
                types,
                &graph,
                self.inference_context,
            ) {
                Ok(node) => node,
                Err(e) => {
                    if std::env::var_os("DSH_DEBUG_GS").is_some() {
                        eprintln!("[GS] pick_node failed: {e:?}");
                    }
                    return Err(infer.error(None));
                }
            };
            let vars_to_solve = graph.nodes[node_to_solve]
                .as_ref()
                .expect("live node")
                .data
                .clone();
            let saved_undet = self.inference_context.save(ctx, syms, types);
            // The failure is caught around the whole solve loop and recovered.
            //
            // Port adapter (A3k): the ported InferenceException travels as the
            // Err value of the solve calls, so the upstream catch is expressed
            // as one outcome variable covering the whole loop body. A failure
            // is not swallowed: the same rollback/recovery as upstream runs and
            // the recovery's own failures still propagate.
            let mut inference_failure: Option<InferenceException> = None;
            let mut solved_all = false;
            'outer: while !solved_all {
                let restvars = self.inference_context.restvars(ctx, syms, types);
                if !contains_any_list(ctx, syms, types, restvars, vars_to_solve.clone()) {
                    solved_all = true;
                    break;
                }
                let mut progressed = false;
                for steps in graph_inference_steps() {
                    let solved = match self.inference_context.solve_basic(
                        ctx,
                        syms,
                        types,
                        vars_to_solve.clone(),
                        &steps,
                    ) {
                        Ok(solved) => solved,
                        Err(ex) => {
                            inference_failure = Some(ex);
                            break 'outer;
                        }
                    };
                    if !solved.is_empty() {
                        if let Err(ex) = infer.do_incorporation(
                            ctx,
                            syms,
                            types,
                            self.inference_context,
                            warn,
                        ) {
                            inference_failure = Some(ex);
                            break 'outer;
                        }
                        progressed = true;
                        continue 'outer;
                    }
                }
                if !progressed {
                    // No progress: report an inference failure.
                    inference_failure = Some(infer.error(None));
                    break;
                }
            }
            if inference_failure.is_some() {
                self.inference_context
                    .rollback(ctx, syms, types, saved_undet);
                infer.instantiate_as_uninferred_vars(
                    ctx,
                    syms,
                    types,
                    vars_to_solve.clone(),
                    self.inference_context,
                )?;
                infer.do_incorporation(ctx, syms, types, self.inference_context, warn)?;
            }
            graph.delete_node(ctx, syms, types, self.inference_context, node_to_solve);
        }
        Ok(())
    }
}

/// Java: class InferenceGraph (Infer.java:1678-1917)
pub struct InferenceGraph {
    /// The nodes; None marks a deleted node (the graph keeps stable node ids so
    /// that dependency edges stay valid while nodes are deleted — Porting rules adapter).
    pub nodes: Vec<Option<GraphNode>>,
}

/// Java: InferenceGraph.Node (Infer.java:1685-1850)
#[derive(Clone)]
pub struct GraphNode {
    pub data: List<TypeId>,
    pub deps: Vec<usize>,
    pub index: i32,
    pub lowlink: i32,
    pub active: bool,
}

impl GraphNode {
    fn is_leaf(&self, _graph: &InferenceGraph) -> bool {
        self.deps.is_empty()
    }

    fn get_all_dependencies(&self, _graph: &InferenceGraph) -> Vec<usize> {
        self.deps.clone()
    }
}

impl InferenceGraph {
    /// Java: InferenceGraph() { initNodes(); } (Infer.java:1837-1917)
    pub fn new(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> InferenceGraph {
        let mut graph = InferenceGraph { nodes: Vec::new() };
        // add nodes
        let restvars = inference_context.restvars(ctx, syms, types);
        let mut current = restvars;
        while current.non_empty() {
            graph.nodes.push(Some(GraphNode {
                data: List::of(*current.head()),
                deps: Vec::new(),
                index: -1,
                lowlink: 0,
                active: false,
            }));
            current = current.tail();
        }
        // add dependencies
        let count = graph.nodes.len();
        for i in 0..count {
            let i_type = *graph.nodes[i].as_ref().expect("live node").data.head();
            for j in 0..count {
                let j_type = *graph.nodes[j].as_ref().expect("live node").data.head();
                if i != j {
                    let uv_i = inference_context.as_undet_var(ctx, syms, types, i_type);
                    let bounds = types.undet_var_get_bounds(
                        ctx,
                        uv_i,
                        &InferenceBound::values(),
                    );
                    if contains_any_list(ctx, syms, types, bounds, List::of(j_type)) {
                        graph.nodes[i].as_mut().expect("live node").deps.push(j);
                    }
                }
            }
        }
        // merge cyclic nodes
        //
        // The SCC heads become the new node list; every dependency edge is
        // remapped from the pre-merge ids to the compacted ids (merged members
        // map to their root), so the edges never point past the list.
        let sccs = graph.tarjan_scc();
        let mut acyclic_nodes: Vec<Option<GraphNode>> = Vec::new();
        let mut old_to_new: Vec<Option<usize>> = vec![None; count];
        for scc in &sccs {
            if scc.len() > 1 {
                let root = scc[0];
                let tail: Vec<usize> = scc[1..].to_vec();
                graph.merge_with(root, &tail, count);
                for n in scc {
                    graph.notify_update(*n, Some(root), count);
                }
            }
            let head = scc[0];
            let new_index = acyclic_nodes.len();
            for n in scc {
                old_to_new[*n] = Some(new_index);
            }
            acyclic_nodes.push(graph.nodes[head].take());
        }
        for node in acyclic_nodes.iter_mut().flatten() {
            let mut new_deps: Vec<usize> = Vec::new();
            for dep in &node.deps {
                if let Some(Some(new_dep)) = old_to_new.get(*dep) {
                    if !new_deps.contains(new_dep) {
                        new_deps.push(*new_dep);
                    }
                }
            }
            node.deps = new_deps;
        }
        graph.nodes = acyclic_nodes;
        graph
    }

    /// Java: InferenceGraph.Node.closure() — the set of nodes reachable
    /// through dependencies (used by DeferredAttr.canInfluence).
    pub fn node_closure(&self, node: usize) -> Vec<usize> {
        let mut closure: Vec<usize> = Vec::new();
        self.collect_closure(node, &mut closure);
        closure
    }

    fn collect_closure(&self, node: usize, closure: &mut Vec<usize>) {
        if closure.contains(&node) {
            return;
        }
        closure.push(node);
        if let Some(data) = self.nodes.get(node).and_then(|n| n.as_ref()) {
            for dep in &data.deps {
                self.collect_closure(*dep, closure);
            }
        }
    }

    /// Java: Node findNode(Type t)
    #[allow(clippy::collapsible_if)]
    pub fn find_node(&self, t: TypeId) -> Option<usize> {
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(node) = node {
                if node.data.iter().any(|x| x == t) {
                    return Some(index);
                }
            }
        }
        None
    }

    /// Java: void deleteNode(Node n)
    pub fn delete_node(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        _inference_context: &InferenceContext,
        node: usize,
    ) {
        let _ = (ctx, syms, types);
        self.nodes[node] = None;
        self.notify_update(node, None, self.nodes.len());
    }

    /// Java: void notifyUpdate(Node from, Node to)
    fn notify_update(&mut self, from: usize, to: Option<usize>, count: usize) {
        for i in 0..count {
            if self.nodes.get(i).map(|n| n.is_some()).unwrap_or(false) {
                self.graph_changed(i, from, to);
            }
        }
    }

    /// Java: Node.graphChanged(Node from, Node to)
    fn graph_changed(&mut self, node: usize, from: usize, to: Option<usize>) {
        let Some(node_data) = self.nodes.get_mut(node).and_then(|n| n.as_mut()) else {
            return;
        };
        if let Some(position) = node_data.deps.iter().position(|d| *d == from) {
            node_data.deps.remove(position);
            if let Some(to) = to {
                node_data.deps.push(to);
            }
        }
    }

    /// Java: mergeWith(List<? extends Node> nodes) (Infer.java:1755-1780)
    fn merge_with(&mut self, root: usize, tail: &[usize], count: usize) {
        for n in tail {
            let n_data = self.nodes[*n].as_ref().expect("live node").clone();
            let root_data = self.nodes[root].as_mut().expect("live node");
            assert!(
                n_data.data.length() == 1,
                "AssertionError: Attempt to merge a compound node!"
            );
            let mut data = root_data.data.clone();
            let mut current = n_data.data;
            while current.non_empty() {
                data = data.append(*current.head());
                current = current.tail();
            }
            root_data.data = data;
            for dep in n_data.deps {
                if !root_data.deps.contains(&dep) {
                    root_data.deps.push(dep);
                }
            }
        }
        // update deps: dependencies on merged members become self-dependencies
        let members: Vec<TypeId> = self.nodes[root]
            .as_ref()
            .expect("live node")
            .data
            .iter()
            .collect();
        let mut new_deps: Vec<usize> = Vec::new();
        let old_deps = self.nodes[root].as_ref().expect("live node").deps.clone();
        for dep in old_deps {
            let Some(dep_data) = self.nodes.get(dep).and_then(|n| n.as_ref()) else {
                continue;
            };
            let first = *dep_data.data.head();
            if members.contains(&first) {
                if !new_deps.contains(&root) {
                    new_deps.push(root);
                }
            } else if !new_deps.contains(&dep) {
                new_deps.push(dep);
            }
        }
        self.nodes[root].as_mut().expect("live node").deps = new_deps;
        let _ = count;
    }

    /// GraphUtils.tarjan over the live node ids (the faithful port of
    /// GraphUtils.java:171-235, adapted to the stable node ids; see the file
    /// header registration).
    fn tarjan_scc(&mut self) -> Vec<Vec<usize>> {
        let count = self.nodes.len();
        let mut sccs: Vec<Vec<usize>> = Vec::new();
        let mut index_counter = 0i32;
        let mut stack: Vec<usize> = Vec::new();
        for start in 0..count {
            if self.nodes[start].is_none() {
                continue;
            }
            if self.nodes[start].as_ref().expect("live node").index != -1 {
                continue;
            }
            self.tarjan_visit(start, &mut index_counter, &mut stack, &mut sccs);
        }
        sccs
    }

    fn tarjan_visit(
        &mut self,
        v: usize,
        index: &mut i32,
        stack: &mut Vec<usize>,
        sccs: &mut Vec<Vec<usize>>,
    ) {
        // visitNode
        {
            let state = self.nodes[v].as_mut().expect("live node");
            state.index = *index;
            state.lowlink = *index;
            *index += 1;
            state.active = true;
        }
        stack.push(v);
        let deps = self.nodes[v].as_ref().expect("live node").deps.clone();
        for n in deps {
            if self.nodes.get(n).map(|x| x.is_none()).unwrap_or(true) {
                continue;
            }
            let n_index = self.nodes[n].as_ref().expect("live node").index;
            if n_index == -1 {
                self.tarjan_visit(n, index, stack, sccs);
                let n_lowlink = self.nodes[n].as_ref().expect("live node").lowlink;
                let v_lowlink = self.nodes[v].as_ref().expect("live node").lowlink;
                self.nodes[v].as_mut().expect("live node").lowlink = v_lowlink.min(n_lowlink);
            } else if stack.contains(&n) {
                let n_index = self.nodes[n].as_ref().expect("live node").index;
                let v_lowlink = self.nodes[v].as_ref().expect("live node").lowlink;
                self.nodes[v].as_mut().expect("live node").lowlink = v_lowlink.min(n_index);
            }
        }
        let (lowlink, node_index) = {
            let state = self.nodes[v].as_ref().expect("live node");
            (state.lowlink, state.index)
        };
        if lowlink == node_index {
            // v is the root of a SCC
            let mut cycle: Vec<usize> = Vec::new();
            loop {
                let n = stack.pop().expect("java.util.NoSuchElementException");
                self.nodes[n].as_mut().expect("live node").active = false;
                cycle.push(n);
                if n == v {
                    break;
                }
            }
            sccs.push(cycle);
        }
    }
}

// =====================================================================
// ReachabilityVisitor + deferred listeners (InferenceContext.java:406-484,
// 248-284)
// =====================================================================

/// Java: class ReachabilityVisitor extends Types.UnaryVisitor<Void>
/// (InferenceContext.java:406-484)
pub struct ReachabilityVisitor {
    pub context: Rc<InferenceContext>,
    pub equiv: Vec<TypeId>,
    pub min: Vec<TypeId>,
    pub min_map: Vec<(TypeId, Vec<TypeId>)>,
}

impl ReachabilityVisitor {
/// /// Java: ReachabilityVisitor(InferenceContext) (Infer.java:1450-1470).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1450-1470.
///
    pub fn new(context: &Rc<InferenceContext>) -> ReachabilityVisitor {
        ReachabilityVisitor {
            context: Rc::clone(context),
            equiv: Vec::new(),
            min: Vec::new(),
            min_map: Vec::new(),
        }
    }

    pub fn scan(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        roots: List<TypeId>,
    ) -> Result<(), InferenceException> {
        let mut current = roots;
        while current.non_empty() {
            self.visit(ctx, syms, types, *current.head())?;
            current = current.tail();
        }
        Ok(())
    }

    fn visit(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> Result<(), InferenceException> {
        let _ = syms;
        if let TypeKindData::UndetVar { .. } = &ctx.typ(t).kind {
            self.visit_undet_var(ctx, syms, types, t)
        } else {
            match &ctx.typ(t).kind {
                TypeKindData::WildcardType { type_: Some(type_), .. } => {
                    self.visit(ctx, syms, types, *type_)
                }
                TypeKindData::TypeVar { .. } => {
                    // Java: visitTypeVar -> asUndetVar; roots may contain
                    // TypeVars (constructor return CHM<K,V>), which must be
                    // mapped to their undet vars for reachability.
                    let und = self.context.as_undet_var(ctx, syms, types, t);
                    if und == t {
                        Ok(())
                    } else {
                        self.visit(ctx, syms, types, und)
                    }
                }
                TypeKindData::ArrayType { elemtype: Some(elem), .. } => {
                    self.visit(ctx, syms, types, *elem)
                }
                TypeKindData::ClassType { .. }
                | TypeKindData::ErasedClassType { .. }
                | TypeKindData::UnionClassType { .. }
                | TypeKindData::IntersectionClassType { .. } => {
                    if let Some(enclosing) = {
                        match &ctx.typ(t).kind {
                            TypeKindData::ClassType { outer_field, .. }
                            | TypeKindData::ErasedClassType { outer_field, .. }
                            | TypeKindData::UnionClassType { outer_field, .. }
                            | TypeKindData::IntersectionClassType { outer_field, .. } => {
                                *outer_field
                            }
                            _ => None,
                        }
                    } {
                        self.visit(ctx, syms, types, enclosing)?;
                    }
                    let mut args = types.get_type_arguments_public(ctx, syms, t);
                    while args.non_empty() {
                        self.visit(ctx, syms, types, *args.head())?;
                        args = args.tail();
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        }
    }

    fn visit_undet_var(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> Result<(), InferenceException> {
        let qtype = types.undet_var_qtype(ctx, t);
        if !self.min.contains(&qtype) {
            self.min.push(qtype);
            let mut deps: Vec<TypeId> = self
                .min_map
                .iter()
                .find(|(k, _)| *k == qtype)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| vec![qtype]);
            for bound_kind in InferenceBound::values() {
                let bounds = types.undet_var_get_bounds(ctx, t, &[bound_kind]);
                let mut current = bounds;
                while current.non_empty() {
                    let b = *current.head();
                    let undet = self.context.as_undet_var(ctx, syms, types, b);
                    if !matches!(&ctx.typ(undet).kind, TypeKindData::UndetVar { .. }) {
                        self.visit(ctx, syms, types, undet)?;
                    } else if self.is_equiv(ctx, syms, types, t, b, bound_kind)? {
                        if !deps.contains(&b) {
                            deps.push(b);
                        }
                        if !self.equiv.contains(&b) {
                            self.equiv.push(b);
                        }
                    } else {
                        self.visit(ctx, syms, types, undet)?;
                    }
                    current = current.tail();
                }
            }
            self.min_map.push((qtype, deps));
        }
        Ok(())
    }

    /// Java: boolean isEquiv(UndetVar from, Type t, InferenceBound boundKind)
    fn is_equiv(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        from: TypeId,
        t: TypeId,
        bound_kind: InferenceBound,
    ) -> Result<bool, InferenceException> {
        let uv = self.context.as_undet_var(ctx, syms, types, t);
        for ib in InferenceBound::values() {
            let mut b1 = types.undet_var_get_bounds(ctx, from, &[ib]);
            if ib == bound_kind {
                b1 = list_remove(&b1, t);
            }
            let mut b2 = types.undet_var_get_bounds(ctx, uv, &[ib]);
            if ib == bound_kind.complement() {
                let from_qtype = types.undet_var_qtype(ctx, from);
                b2 = list_remove(&b2, from_qtype);
            }
            if !list_contains_all(&b1, &b2) || !list_contains_all(&b2, &b1) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// The inference context is not threaded through the visitor (upstream stores
/// it in the visitor); the ported relation uses asUndetVar via the helper
/// below, which the call sites rebind to their context (registered adapter:
/// ReachabilityVisitor only receives types that are already UndetVars in the
/// min roots or bounds of UndetVars).
fn inference_context_free_placeholder(
    _ctx: &mut CompilerContext,
    _syms: &Symtab,
    _types: &Types,
    t: TypeId,
) -> TypeId {
    t
}

/// Java: void dupTo free-type listener "inferenceContext ->
/// InferenceContext.this.notifyChange()" (InferenceContext.java:330-333)
struct NotifyChangeListener {
    /// Java: the dupTo listener is a lambda that captures InferenceContext.this,
    /// i.e. a strong reference to the context being propagated to
    /// (InferenceContext.java:330-333). Holding a Weak here instead let the
    /// context die before the target context ever notified it -- the address was
    /// then reused by a later context, so upgrade() failed while the *current*
    /// context was still alive, and the free-type listeners registered on the
    /// original context were never notified (R35).
    target: Rc<InferenceContext>,
}

impl FreeTypeListener for NotifyChangeListener {
    fn types_inferred(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        _inference_context: &InferenceContext,
    ) -> Result<(), InferenceException> {
        self.target.notify_change(ctx, syms, types)
    }
}

/// Java: InferenceContext.min's per-variable listener (InferenceContext.java:
/// 372-380)
struct PropagateListener {
    original: std::rc::Weak<InferenceContext>,
    var: TypeId,
    min_map: Vec<(TypeId, Vec<TypeId>)>,
    /// Upstream captures the caller's Warner reference (InferenceContext.java:
    /// 380-387); the Rc keeps the shared lint state alive across the deferred
    /// notification.
    warn: Rc<RefCell<Warner>>,
}

impl FreeTypeListener for PropagateListener {
    fn types_inferred(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<(), InferenceException> {
        let inst_type = inference_context.as_inst_type(ctx, syms, types, self.var);
        let Some(original) = self.original.upgrade() else {
            return Ok(());
        };
        if let Some((_, eqs)) = self.min_map.iter().find(|(t, _)| *t == self.var) {
            for eq in eqs {
                let eq_uv = original.as_undet_var(ctx, syms, types, *eq);
                types.undet_var_set_inst(ctx, syms, eq_uv, inst_type);
            }
        }
        let infer = original.infer.clone().expect("InferenceContext.infer");
        let mut warn = self.warn.borrow_mut();
        infer.do_incorporation(ctx, syms, types, &original, &mut warn)?;
        drop(warn);
        original.notify_change(ctx, syms, types)
    }
}

/// Java: InferenceContext.min's unreachable-variables listener
/// (InferenceContext.java:385-390)
struct UnreachableVarsListener {
    original: std::rc::Weak<InferenceContext>,
    unreachable_vars: List<TypeId>,
    /// Upstream captures the caller's Warner reference (InferenceContext.java:
    /// 392-395); the Rc keeps the shared lint state alive across the deferred
    /// notification.
    warn: Rc<RefCell<Warner>>,
}

impl FreeTypeListener for UnreachableVarsListener {
    fn types_inferred(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<(), InferenceException> {
        let Some(original) = self.original.upgrade() else {
            return Ok(());
        };
        let mut warn = self.warn.borrow_mut();
        let _ = inference_context;
        original.solve_list(
            ctx,
            syms,
            types,
            self.unreachable_vars.clone(),
            &mut warn,
        )?;
        drop(warn);
        original.notify_change(ctx, syms, types)
    }
}

// =====================================================================
// Helpers
// =====================================================================

fn list_diff(ts: &List<TypeId>, ss: &List<TypeId>) -> List<TypeId> {
    let mut result: List<TypeId> = List::nil();
    let mut current = ts.clone();
    while current.non_empty() {
        let t = *current.head();
        if !ss.iter().any(|s| s == t) {
            result = result.append(t);
        }
        current = current.tail();
    }
    result
}

fn list_remove(ts: &List<TypeId>, t: TypeId) -> List<TypeId> {
    let mut result: List<TypeId> = List::nil();
    let mut current = ts.clone();
    while current.non_empty() {
        let x = *current.head();
        if x != t {
            result = result.append(x);
        }
        current = current.tail();
    }
    result
}

fn list_contains_all(ts: &List<TypeId>, ss: &List<TypeId>) -> bool {
    ss.iter().all(|s| ts.iter().any(|t| t == s))
}

/// Java: Type.containsAny(List<Type> ts1, List<Type> ts2)
fn contains_any_list(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    ts1: List<TypeId>,
    ts2: List<TypeId>,
) -> bool {
    let mut current = ts1;
    while current.non_empty() {
        let t = *current.head();
        let mut other = ts2.clone();
        while other.non_empty() {
            if types.contains(ctx, syms, t, *other.head()) {
                return true;
            }
            other = other.tail();
        }
        current = current.tail();
    }
    false
}


fn elemtype_of(ctx: &mut CompilerContext, t: TypeId) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
        _ => panic!("unexpected array type"),
    }
}

fn new_array_type_ctx(ctx: &mut CompilerContext, elem: TypeId, tsym: SymbolId) -> TypeId {
    ctx.alloc_type(crate::code::type_::Type::new(
        TypeTag::ARRAY,
        Some(tsym),
        TypeKindData::ArrayType {
            elemtype: Some(elem),
                varargs: false,
        },
    ))
}

fn components_of(ctx: &mut CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::IntersectionClassType {
            supertype_field,
            interfaces_field,
            ..
        } => {
            let supertype = supertype_field.expect("intersection supertype");
            let interfaces = interfaces_field.clone().unwrap_or_else(List::nil);
            let mut result: List<TypeId> = List::of(supertype);
            let mut current = interfaces;
            while current.non_empty() {
                result = result.append(*current.head());
                current = current.tail();
            }
            result
        }
        _ => List::of(t),
    }
}

fn new_type_variable_symbol(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    name: crate::util::name::Name,
    owner: SymbolId,
) -> SymbolId {
    let symbol = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
        crate::code::kinds::Kind::TYP,
        flags::SYNTHETIC as i64,
        name,
        None,
        Some(owner),
        crate::code::symbol::SymbolKind::TypeVariableSymbol,
    ));
    let _ = syms;
    symbol
}

fn new_captured_type_ctx(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    name: crate::util::name::Name,
    owner: Option<SymbolId>,
    upper: TypeId,
    lower: TypeId,
    wildcard: Option<TypeId>,
) -> TypeId {
    let symbol = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
        crate::code::kinds::Kind::TYP,
        0,
        name,
        None,
        owner,
        crate::code::symbol::SymbolKind::TypeVariableSymbol,
    ));
    let type_id = ctx.alloc_type(crate::code::type_::Type::new(
        TypeTag::TYPEVAR,
        Some(symbol),
        TypeKindData::CapturedType {
            bound: Some(upper),
            lower: Some(lower),
            wildcard,
        },
    ));
    ctx.sym_mut(symbol).ty = Some(type_id);
    let _ = syms;
    type_id
}


// =====================================================================
// Incorporate helpers (Infer.java:780-805, 892-910, 1247-1300)
// =====================================================================

/// Java: CheckBounds.checkBound(Type s, Type t, InferenceBound ib_s,
/// InferenceBound ib_t, Warner warn) (Infer.java:780-790)
#[allow(clippy::too_many_arguments)]
fn check_bound(
    infer: &Infer,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    s: TypeId,
    t: TypeId,
    ib_s: InferenceBound,
    ib_t: InferenceBound,
    warn: &mut Warner,
) -> Result<bool, InferenceException> {
    let r = if ib_s.less_than(ib_t) {
        types.is_subtype_unchecked_with_warn(ctx, syms, s, t, warn)
    } else if ib_t.less_than(ib_s) {
        types.is_subtype_unchecked_with_warn(ctx, syms, t, s, warn)
    } else {
        types.is_same_type(ctx, syms, s, t)
    };
    Ok(r)
}

/// Java: void reportInstError(UndetVar uv, InferenceBound ib)
/// (Infer.java:1247-1259)
fn report_inst_error(
    infer: &Infer,
    ctx: &mut CompilerContext,
    types: &Types,
    uv: TypeId,
    ib: InferenceBound,
) -> Result<(), InferenceException> {
    let bounds = types.undet_var_get_bounds(ctx, uv, &[ib]);
    let inst = types
        .undet_var_get_inst(ctx, uv)
        .expect("CheckInst requires an instantiated undet var");
    let inst_arg = crate::comp::check::type_arg_with_rich_where(ctx, inst);
    let args = bounds_type_args(ctx, bounds);
    let fragment = match ib {
        InferenceBound::EQ => fragments::inferred_do_not_conform_to_eq_bounds(inst_arg, args),
        InferenceBound::LOWER => {
            fragments::inferred_do_not_conform_to_lower_bounds(inst_arg, args)
        }
        InferenceBound::UPPER => {
            fragments::inferred_do_not_conform_to_upper_bounds(inst_arg, args)
        }
    };
    let diag = infer.diags.fragment(fragment.into());
    Err(infer.error(Some(diag)))
}

/// Java: CheckBounds.report(InferenceBound from, InferenceBound to)
/// (Infer.java:793-800)
fn report_bound_error_from_to(
    infer: &Infer,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    uv: TypeId,
    from: InferenceBound,
    to: InferenceBound,
) -> Result<(), InferenceException> {
    if from == to {
        infer.report_bound_error(ctx, syms, types, uv, from)
    } else if from == InferenceBound::LOWER || to == InferenceBound::EQ {
        report_bound_error_two(infer, ctx, types, uv, to, from)
    } else {
        report_bound_error_two(infer, ctx, types, uv, from, to)
    }
}

/// Java: void reportBoundError(UndetVar uv, InferenceBound ib1,
/// InferenceBound ib2) (Infer.java:1275-1280)
#[allow(clippy::too_many_arguments)]
fn report_bound_error_two(
    infer: &Infer,
    ctx: &mut CompilerContext,
    types: &Types,
    uv: TypeId,
    ib1: InferenceBound,
    ib2: InferenceBound,
) -> Result<(), InferenceException> {
    let frag1 = get_bound_fragment(ctx, types, uv, ib1);
    let frag2 = get_bound_fragment(ctx, types, uv, ib2);
    let qtype = match &ctx.typ(uv).kind {
        crate::code::type_::TypeKindData::UndetVar { qtype, .. } => *qtype,
        _ => uv,
    };
    let frag1_diag = infer.diags.fragment(frag1);
    let frag2_diag = infer.diags.fragment(frag2);
    let fragment = fragments::incompatible_bounds(
        crate::comp::check::type_arg_with_rich_where(ctx, qtype),
        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(std::rc::Rc::new(
            frag1_diag,
        )),
        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(std::rc::Rc::new(
            frag2_diag,
        )),
    );
    let diag = infer.diags.fragment(fragment.into());
    Err(infer.error(Some(diag)))
}

/// Java: Fragment getBoundFragment(InferenceBound ib, List<Type> types)
/// (Infer.java:1282-1290)
fn get_bound_fragment(
    ctx: &mut CompilerContext,
    types: &Types,
    uv: TypeId,
    ib: InferenceBound,
) -> crate::util::jc_diagnostic::Fragment {
    let bounds = types.undet_var_get_bounds(ctx, uv, &[ib]);
    let args = bounds_type_args(ctx, bounds);
    let fragment = match ib {
        InferenceBound::EQ => fragments::eq_bounds(args),
        InferenceBound::LOWER => fragments::lower_bounds(args),
        InferenceBound::UPPER => fragments::upper_bounds(args),
    };
    fragment.into()
}

/// Helper for the diagnostic arguments: the ADR 0002 P4 contract uses the
/// TypeArg placeholder (registered).
fn bounds_type_args(ctx: &mut CompilerContext, bounds: List<TypeId>) -> Vec<TypeArg> {
    let mut args: Vec<TypeArg> = Vec::new();
    let mut current = bounds;
    while current.non_empty() {
        args.push(crate::comp::check::type_arg_with_rich_where(
            ctx,
            *current.head(),
        ));
        current = current.tail();
    }
    args
}

/// Java: SubstBounds.checkCompatibleUpperBounds(UndetVar uv, InferenceContext
/// inferenceContext) (Infer.java:892-908)
fn check_compatible_upper_bounds(
    infer: &Infer,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    uv: TypeId,
) -> Result<(), InferenceException> {
    let upper_bounds = types.undet_var_get_bounds(ctx, uv, &[InferenceBound::UPPER]);
    let mut hibounds: List<TypeId> = List::nil();
    let mut current = upper_bounds;
    while current.non_empty() {
        let t = *current.head();
        if bound_filter(ctx, syms, types, inference_context, t) {
            hibounds = hibounds.append(t);
        }
        current = current.tail();
    }
    let hb = if hibounds.is_empty() {
        syms.object_type
    } else if hibounds.tail().is_empty() {
        *hibounds.head()
    } else {
        types.glb_list(ctx, syms, hibounds)
    };
    if types.is_erroneous_type(ctx, syms, hb) {
        infer.report_bound_error(ctx, syms, types, uv, InferenceBound::UPPER)?;
    }
    Ok(())
}





// =====================================================================
// Infer instantiateMethod / PartiallyInferredMethodType (Infer.java:166-370,
// 391-520)
// =====================================================================

impl Infer {
    /// Java: Type instantiateMethod(Env<AttrContext> env, List<Type> tvars,
    /// MethodType mt, Attr.ResultInfo resultInfo, MethodSymbol msym,
    /// List<Type> argtypes, boolean allowBoxing, boolean useVarargs,
    /// Resolve.MethodResolutionContext resolveContext, Warner warn)
    /// (Infer.java:166-250)
    #[allow(
        clippy::too_many_arguments,
        clippy::unnecessary_unwrap,
        clippy::collapsible_if,
        clippy::blocks_in_conditions
    )]
/// /// Java: Type instantiateMethod(Env, Type, List<Type>, ... ) (Infer.java:1800-1900).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1800-1900.
///
    /// Java: Type instantiateMethod(...) (Infer.java:1800-1900).
    ///
    /// The upstream method catches InferenceException and reports it through the
    /// result info check context (Infer.java:348-351); the ported throw channel is
    /// the panic channel, so the catch is modelled here.
    #[allow(clippy::too_many_arguments)]
    pub fn instantiate_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tvars: List<TypeId>,
        mt: TypeId,
        result_info: Option<&crate::comp::attr::ResultInfo>,
        msym: SymbolId,
        argtypes: List<TypeId>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut crate::comp::resolve::MethodResolutionContext,
        warn: &mut Warner,
    ) -> Result<TypeId, InferenceException> {
        let outcome = crate::comp::resolve::catch_inference_exception(|| {
            self.instantiate_method_inner(
                ctx,
                syms,
                types,
                attr,
                chk,
                env,
                tvars,
                mt,
                result_info,
                msym,
                argtypes,
                allow_boxing,
                use_varargs,
                resolve_context,
                warn,
            )
        });
        match outcome {
            Ok(result) => result,
            Err(exception) => {
                // Java: resultInfo.checkContext.report(null, ex.getDiagnostic());
                // Assert.error();
                if let Some(result_info) = result_info
                    && let Some(diagnostic) = exception.get_diagnostic()
                {
                    let pos = env
                        .tree
                        .map(|tree| ctx.tree(tree).pos)
                        .unwrap_or(crate::util::position::NOPOS);
                    result_info.check_context.report(ctx, pos, &diagnostic);
                }
                Err(exception)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn instantiate_method_inner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tvars: List<TypeId>,
        mt: TypeId,
        result_info: Option<&crate::comp::attr::ResultInfo>,
        msym: SymbolId,
        argtypes: List<TypeId>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut crate::comp::resolve::MethodResolutionContext,
        warn: &mut Warner,
    ) -> Result<TypeId, InferenceException> {
        let _ = (allow_boxing, use_varargs);
        // Port adapter: upstream Infer.instantiateMethod passes the caller's
        // Warner through the deferred contexts and the min listeners
        // (Infer.java:180, InferenceContext.java:380-395); the Rc keeps the
        // warning/lint state shared and the guard writes it back on every
        // return path (including the ?-early-returns).
        struct WarnSync<'a> {
            target: &'a mut Warner,
            shared: Rc<RefCell<Warner>>,
        }
        impl Drop for WarnSync<'_> {
            fn drop(&mut self) {
                // Java: instantiateMethod passes the caller's Warner object
                // reference through to the deferred contexts and keeps it
                // reachable from a returned PartiallyInferredMethodType
                // (Infer.java:166-370). The state must therefore be *copied*
                // back on the way out, never moved: a mem::take here would
                // leave the shared Warner empty for every later
                // PartiallyInferredMethodType.check (which is what decides
                // between the unchecked-erasure path and shouldPropagate).
                *self.target = self.shared.borrow().clone();
            }
        }
        let warn_shared = Rc::new(RefCell::new(warn.clone()));
        let _warn_sync = WarnSync {
            target: warn,
            shared: Rc::clone(&warn_shared),
        };
        let inference_context =
            InferenceContext::new(Rc::new(self.clone_shallow()), ctx, syms, types, tvars.clone());
        let defer_context = resolve_context.deferred_attr_context(
            msym,
            Rc::clone(&inference_context),
            result_info,
            None,
        );
        let deferred_context = Rc::new(defer_context);
        // resolveContext.methodCheck.argumentsAcceptable(env, deferredAttrContext,
        // argtypes, mt.getParameterTypes(), warn);
        let argtypes_for_final_check = argtypes.clone();
        let method_check = resolve_context
            .method_check
            .clone()
            .unwrap_or_else(|| Rc::new(RefCell::new(crate::comp::resolve::NilMethodCheck)));
        {
            // MethodCheck methods take &self; see Resolve::raw_instantiate.
            let check = method_check.borrow();
            check
                .arguments_acceptable(
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    &self.diags,
                    env,
                    Rc::clone(&deferred_context),
                    argtypes,
                    types.get_parameter_types_public(ctx, mt),
                    &mut warn_shared.borrow_mut(),
                )
                .map_err(|failure| match failure {
                    crate::comp::resolve::MethodCheckFailure::Inference(ex) => ex,
                    crate::comp::resolve::MethodCheckFailure::Inapplicable(ex) => {
                        // resolveContext.methodCheck.argumentsAcceptable(...)
                        // throws InapplicableMethodException; Infer.instantiateMethod
                        // catches only InferenceException, so the inapplicable
                        // failure continues to the caller (Resolve.rawInstantiate
                        // and then selectBest's catch). The ported throw/catch
                        // channel for InapplicableMethodException is preserved.
                        crate::comp::resolve::throw_inapplicable_method_exception(
                            ex.get_diagnostic(),
                        )
                    }
                })?;
        }
        if self.allow_graph_inference
            && result_info.is_some()
            && result_info.and_then(|ri| ri.pt) == Some(self.any_poly)
        {
            if std::env::var_os("DSH_DEBUG_DIAMOND_PT").is_some() {
                eprintln!(
                    "[PARTIAL] m={} pt={:?}",
                    ctx.sym(msym).name.to_java_string().to_string_lossy(),
                    result_info.and_then(|ri| ri.pt).map(|p| crate::comp::attr::type_display_of(ctx, p)),
                );
            }
            self.do_incorporation(
                ctx,
                syms,
                types,
                &inference_context,
                &mut warn_shared.borrow_mut(),
            )?;
            // Java: instantiateMethod finally (Infer.java:241-246).
            inference_context.notify_change(ctx, syms, types)?;
            return Ok(partially_inferred_method_type(
                ctx,
                mt,
                Rc::clone(&inference_context),
                Rc::clone(env),
                Rc::clone(&warn_shared),
            ));
        } else if self.allow_graph_inference && result_info.is_some() {
            // inject return constraints earlier
            self.do_incorporation(
                ctx,
                syms,
                types,
                &inference_context,
                &mut warn_shared.borrow_mut(),
            )?;
            let result_info = result_info.expect("result info");
            if !warn_shared
                .borrow()
                .has_non_silent_lint(crate::code::lint::LintCategory::UNCHECKED)
            {
                let should = self.should_propagate(
                    ctx,
                    syms,
                    types,
                    types.get_return_type_public(ctx, mt),
                    result_info,
                    &inference_context,
                );
                if std::env::var_os("DSH_DEBUG_INFER").is_some()
                    && ctx.sym(msym).name.to_java_string().to_string_lossy() == "newMap"
                {
                    eprintln!(
                        "[NEWMAP] restype={} target_pt={:?} should={} partial_context_empty={}",
                        crate::comp::attr::type_display_of(ctx, types.get_return_type_public(ctx, mt)),
                        result_info.pt.map(|p| (crate::comp::attr::type_display_of(ctx, p), ctx.typ(p).get_tag())),
                        should,
                        Rc::ptr_eq(result_info.check_context.inference_context(), &crate::comp::infer::empty_context()),
                    );
                }
                let min_context = if should {
                    // Java: inferenceContext.min(roots(mt, deferredAttrContext), true, warn)
                    // (Infer.java:197-198). In CHECK mode roots() also collects the
                    // stuck/dependency variables of the deferred attribute nodes
                    // (Infer.java:264-280); passing None here dropped them, which
                    // changes which variables min() has to clone and therefore which
                    // variables get propagated outwards.
                    let roots = self.roots(ctx, syms, types, mt, Some(&deferred_context));
                    // Java: inferenceContext.min(roots(mt, deferredAttrContext), true, warn)
                    // (Infer.java:198) -- shouldSolve is TRUE. min() then registers a
                    // listener that solves the definitively unreachable variables and
                    // calls notifyChange() again (InferenceContext.java:397-404), which
                    // is what lets the variables left behind in the original context
                    // (and the free-type listeners registered on them) make progress.
                    inference_context.min(ctx, syms, types, roots, true, &warn_shared)?
                } else {
                    Rc::clone(&inference_context)
                };
                let other = mt;
                let new_restype = self.generate_return_constraints(
                    ctx,
                    syms,
                    types,
                    env.tree,
                    result_info,
                    other,
                    &min_context,
                )?;
                let new_mt = types.create_method_type_with_return(ctx, mt, new_restype);
                if should {
                    let target_context =
                        Rc::clone(result_info.check_context.inference_context());
                    let inside_overload = result_info
                        .check_context
                        .deferred_attr_context()
                        .inside_overload_phase();
                    min_context.dup_to(
                        ctx,
                        syms,
                        types,
                        &target_context,
                        inside_overload,
                    );
                    deferred_context
            .complete(ctx, syms, types, attr, self)
            .map_err(|failure| match failure {
                crate::comp::deferred_attr::DeferredAttrFailure::Inference(ex) => ex,
                // Java: DeferredAttrContext.complete throws CompletionFailure
                // (unchecked); catch_inference_exception only catches
                // InferenceException, so the ported panic channel keeps the
                // CompletionFailure for the Attr completion boundary.
                crate::comp::deferred_attr::DeferredAttrFailure::Completion(failure) => {
                    crate::code::symbol::throw_completion_failure(failure)
                }
            })?;
            // Java: instantiateMethod finally (Infer.java:241-246).
            inference_context.notify_change(ctx, syms, types)?;
                    return Ok(new_mt);
                }
                let _ = new_mt;
            }
        }
        deferred_context
            .complete(ctx, syms, types, attr, self)
            .map_err(|failure| match failure {
                crate::comp::deferred_attr::DeferredAttrFailure::Inference(ex) => ex,
                // Java: DeferredAttrContext.complete throws CompletionFailure
                // (unchecked); catch_inference_exception only catches
                // InferenceException, so the ported panic channel keeps the
                // CompletionFailure for the Attr completion boundary.
                crate::comp::deferred_attr::DeferredAttrFailure::Completion(failure) => {
                    crate::code::symbol::throw_completion_failure(failure)
                }
            })?;
        // minimize as yet undetermined type variables
        if self.allow_graph_inference {
            let context = Rc::clone(&inference_context);
            context.solve(ctx, syms, types, &mut warn_shared.borrow_mut())?;
        } else {
            inference_context.solve_legacy(
                ctx,
                syms,
                types,
                true,
                &crate::comp::infer::legacy_inference_steps_eq_lower(),
                &mut warn_shared.borrow_mut(),
            )?;
        }
        let inst_mt = inference_context.as_inst_type(ctx, syms, types, mt);

        if let Some(result_info) = result_info
            && self
                .verbose_resolution_mode
                .borrow()
                .contains(&crate::comp::resolve::VerboseResolutionMode::DEFERRED_INST)
            && let Some(log) = self.log.borrow().clone()
        {
            use crate::resources::diagnostic_contract::SymbolArg;
            let mut inst_arg = crate::comp::check::type_arg_with_rich_where(ctx, inst_mt);
            if let Some(declared_ty) = ctx.sym(msym).ty {
                let declared_arg = crate::comp::check::type_arg_with_where(ctx, declared_ty);
                for tvar in declared_arg.where_vars {
                    if !inst_arg.where_vars.iter().any(|existing| {
                        existing.name == tvar.name && existing.owner == tvar.owner
                    }) {
                        inst_arg.where_vars.push(tvar);
                    }
                }
            }
            let target_arg = result_info
                .pt
                .map(|pt| crate::comp::check::type_arg_with_rich_where(ctx, pt))
                .unwrap_or_else(|| crate::resources::diagnostic_contract::TypeArg::placeholder("none"));
            let method_name = crate::code::printer::visit_symbol(ctx, &self.names, msym);
            let note = crate::util::jc_diagnostic::Note::from(
                crate::resources::compiler_properties::notes::deferred_method_inst(
                    SymbolArg::placeholder(method_name),
                    inst_arg,
                    target_arg,
                ),
            );
            if let Some(env_tree) = env.tree {
                crate::util::abstract_log::AbstractLog::note_at(
                    &mut *log.borrow_mut(),
                    ctx.tree(env_tree).pos,
                    note,
                );
            }
        }
        if !self.allow_graph_inference
            && !inference_context.restvars(ctx, syms, types).is_empty()
            && let Some(result_info) = result_info
            && !warn_shared
                .borrow()
                .has_non_silent_lint(crate::code::lint::LintCategory::UNCHECKED)
        {
            let new_restype = self.generate_return_constraints(
                ctx,
                syms,
                types,
                env.tree,
                result_info,
                inst_mt,
                &inference_context,
            )?;
            let _ = new_restype;
            inference_context.solve_legacy(
                ctx,
                syms,
                types,
                false,
                &crate::comp::infer::legacy_inference_steps_eq_upper(),
                &mut warn_shared.borrow_mut(),
            )?;
            // Java: instantiateMethod finally { if (resultInfo != null ||
            // !allowGraphInference) inferenceContext.notifyChange(); }
            // (Infer.java:241-246) — the legacy return path still notifies.
            inference_context.notify_change(ctx, syms, types)?;
            return Ok(inference_context.as_inst_type(ctx, syms, types, mt));
        }
        // Java: if (resultInfo != null || !allowGraphInference) notifyChange();
        if result_info.is_some() || !self.allow_graph_inference {
            let context = Rc::clone(&inference_context);
            context.notify_change(ctx, syms, types)?;
        }
        if std::env::var_os("DSH_DEBUG_INFER").is_some()
            && ctx.sym(msym).name.to_java_string().to_string_lossy() == "newMap"
        {
            eprintln!(
                "[NEWMAP-RET] inst_mt={} result_is_some={}",
                crate::comp::attr::type_display_of(ctx, inst_mt),
                result_info.is_some(),
            );
        }
        Ok(inst_mt)
    }

    /// A shallow copy of the Infer service for Rc-based context links (the
    /// upstream object is shared; the ported Infer is stateless apart from the
    /// services/any_poly — registered adapter).
    fn clone_shallow(&self) -> Infer {
        Infer {
            allow_graph_inference: self.allow_graph_inference,
            diags: Rc::clone(&self.diags),
            names: Rc::clone(&self.names),
            any_poly: self.any_poly,
            log: std::cell::RefCell::new(self.log.borrow().clone()),
            verbose_resolution_mode: std::cell::RefCell::new(
                self.verbose_resolution_mode.borrow().clone(),
            ),
        }
    }

    /// Java: boolean shouldPropagate(Type restype, Attr.ResultInfo target,
    /// InferenceContext inferenceContext) (Infer.java:257-264)
    fn should_propagate(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        restype: TypeId,
        target: &crate::comp::attr::ResultInfo,
        inference_context: &Rc<InferenceContext>,
    ) -> bool {
        let target_context = target.check_context.inference_context();
        if !Rc::ptr_eq(target_context, &crate::comp::infer::empty_context()) {
            if !inference_context.free(ctx, syms, types, restype) {
                return false;
            }
            let inference_vars = inference_context.inference_vars();
            if !inference_vars.iter().any(|t| t == restype) {
                return true;
            }
            if let Some(pt) = target.pt {
                let uv = inference_context.as_undet_var(ctx, syms, types, restype);
                return !self.needs_eager_instantiation(
                    ctx,
                    syms,
                    types,
                    uv,
                    pt,
                    inference_context,
                );
            }
        }
        false
    }

    /// Java: private List<Type> roots(MethodType mt,
    /// DeferredAttrContext deferredAttrContext) (Infer.java:264-283)
    #[allow(clippy::collapsible_if)]
    fn roots(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        mt: TypeId,
        deferred_attr_context: Option<&crate::comp::deferred_attr::DeferredAttrContext>,
    ) -> List<TypeId> {
        let _ = syms;
        let mut roots: List<TypeId> = List::nil();
        roots = roots.append(types.get_return_type_public(ctx, mt));
        if let Some(deferred) = deferred_attr_context {
            // Java: only in CHECK mode; the caller passes None when not needed.
            let nodes = deferred.deferred_attr_nodes.borrow().clone();
            for node in nodes {
                for var in node.deferred_stuck_policy.stuck_vars() {
                    roots = roots.append(var);
                }
                for var in node.deferred_stuck_policy.dep_vars() {
                    roots = roots.append(var);
                }
            }
            let mut thrown_vars: List<TypeId> = List::nil();
            let inference_vars = deferred.inference_context.inference_vars();
            for var in inference_vars.iter() {
                let tsym = ctx.typ(var).tsym;
                if let Some(tsym) = tsym {
                    if (ctx.sym(tsym).flags_field & crate::code::flags::THROWS) != 0 {
                        thrown_vars = thrown_vars.append(var);
                    }
                }
            }
            let mut current = thrown_vars;
            while current.non_empty() {
                let var = *current.head();
                if !roots.iter().any(|r| r == var) {
                    roots = roots.append(var);
                }
                current = current.tail();
            }
        }
        roots
    }

    /// Java: Type generateReturnConstraints(JCTree tree, Attr.ResultInfo
    /// resultInfo, MethodType mt, InferenceContext inferenceContext)
    /// (Infer.java:391-430)
    #[allow(clippy::too_many_arguments, clippy::blocks_in_conditions)]
    fn generate_return_constraints(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        tree: Option<TreeId>,
        result_info: &crate::comp::attr::ResultInfo,
        mt: TypeId,
        inference_context: &Rc<InferenceContext>,
    ) -> Result<TypeId, InferenceException> {
        let rs_context = Rc::clone(result_info.check_context.inference_context());
        let mut from = types.get_return_type_public(ctx, mt);
        let inference_vars = inference_context.inference_vars();
        if types.contains_any(ctx, syms, from, &inference_vars)
            && !Rc::ptr_eq(&rs_context, &crate::comp::infer::empty_context())
        {
            from = types.capture(ctx, syms, from);
            let args = types.get_type_arguments_public(ctx, syms, from);
            for t in args.iter() {
                if ctx.typ(t).has_tag(TypeTag::TYPEVAR) && ctx.typ(t).is_captured() {
                    inference_context.add_var(ctx, syms, types, t);
                }
            }
        }
        let mut qtype = inference_context.as_undet_var(ctx, syms, types, from);
        let mut to = result_info
            .pt
            .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
        if ctx.typ(qtype).has_tag(TypeTag::VOID) {
            to = syms.void_type;
        } else if ctx.typ(to).has_tag(TypeTag::NONE) {
            to = if ctx.typ(from).is_primitive() {
                from
            } else {
                syms.object_type
            };
        } else if ctx.typ(qtype).has_tag(TypeTag::UNDETVAR) {
            if self.needs_eager_instantiation(ctx, syms, types, qtype, to, inference_context)
                && (self.allow_graph_inference || !ctx.typ(to).is_primitive())
            {
                to = self.generate_reference_to_target_constraint(
                    ctx,
                    syms,
                    types,
                    tree,
                    qtype,
                    to,
                    result_info,
                    inference_context,
                )?;
            }
        } else if {
            let free_pt = result_info
                .pt
                .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
            rs_context.free(ctx, syms, types, free_pt)
        } {
            // propagation - cache captured vars
            let captured = match tree {
                Some(tree) => rs_context.cached_capture(
                    ctx,
                    syms,
                    types,
                    tree,
                    from,
                    !result_info.check_mode.update_tree_type(),
                ),
                None => types.capture(ctx, syms, from),
            };
            qtype = inference_context.as_undet_var(ctx, syms, types, captured);
        }
        let mut ret_warn = Warner::new(None);
        let target = rs_context.as_undet_var(ctx, syms, types, to);
        if !result_info.check_context.compatible(
            ctx,
            syms,
            types,
            qtype,
            target,
            &mut ret_warn,
        ) || (!self.allow_graph_inference
            && ret_warn.has_lint(crate::code::lint::LintCategory::UNCHECKED))
        {
            if std::env::var_os("DSH_DEBUG_INFER").is_some() {
                eprintln!(
                    "[INFER] qtype={} target={} from={} pt={} target_tag={:?} to_tag={:?} empty={} mode={:?} overload={}",
                    crate::comp::attr::type_display_of(ctx, qtype),
                    crate::comp::attr::type_display_of(ctx, target),
                    crate::comp::attr::type_display_of(ctx, from),
                    crate::comp::attr::type_display_of(ctx, to),
                    ctx.typ(target).get_tag(),
                    ctx.typ(to).get_tag(),
                    Rc::ptr_eq(&rs_context, &crate::comp::infer::empty_context()),
                    result_info.check_context.deferred_attr_context().mode,
                    result_info.check_context.deferred_attr_context().inside_overload_phase(),
                );
            }
            let inference_vars: Vec<_> = inference_context
                .inference_vars()
                .iter()
                .map(|var| crate::comp::check::type_arg_with_rich_where(ctx, var))
                .filter(|arg| {
                    arg.where_vars
                        .iter()
                        .any(|tvar| tvar.owner.starts_with("method "))
                })
                .collect();
            let diag = self.diags.fragment(
                crate::resources::compiler_properties::fragments::infer_no_conforming_instance_exists(
                    inference_vars,
                    crate::comp::check::type_arg_with_rich_where(ctx, qtype),
                    crate::comp::check::type_arg_with_rich_where(ctx, target),
                )
                .into(),
            );
            return Err(self.error(Some(diag)));
        }
        Ok(from)
    }

    /// Java: boolean needsEagerInstantiation(UndetVar from, Type to,
    /// InferenceContext inferenceContext) (Infer.java:433-490)
    #[allow(clippy::collapsible_if)]
    fn needs_eager_instantiation(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        from: TypeId,
        to: TypeId,
        inference_context: &InferenceContext,
    ) -> bool {
        if ctx.typ(to).is_primitive() {
            let bounds = types.undet_var_get_bounds(
                ctx,
                from,
                &crate::code::type_::InferenceBound::values(),
            );
            for t in bounds.iter() {
                let bound_as_primitive = types.unboxed_type_ro(ctx, syms, t);
                if ctx.typ(bound_as_primitive).has_tag(TypeTag::NONE) {
                    continue;
                }
                return true;
            }
            return false;
        }
        let capture_of_to = types.capture(ctx, syms, to);
        if capture_of_to == to {
            let bounds = types.undet_var_get_bounds(
                ctx,
                from,
                &[
                    crate::code::type_::InferenceBound::EQ,
                    crate::code::type_::InferenceBound::LOWER,
                ],
            );
            for t in bounds.iter() {
                let capture_of_bound = types.capture(ctx, syms, t);
                if capture_of_bound != t {
                    return true;
                }
            }
            let lower = types.undet_var_get_bounds(
                ctx,
                from,
                &[crate::code::type_::InferenceBound::LOWER],
            );
            for a in lower.iter() {
                for b in lower.iter() {
                    if a != b
                        && !inference_context.free(ctx, syms, types, a)
                        && !inference_context.free(ctx, syms, types, b)
                        && self.common_super_with_diff_parameterization(
                            ctx, syms, types, a, b,
                        )
                    {
                        return true;
                    }
                }
            }
        }
        if types.is_parameterized_type_of(ctx, to) {
            let bound_kinds = [
                crate::code::type_::InferenceBound::EQ,
                crate::code::type_::InferenceBound::LOWER,
            ];
            let bounds = types.undet_var_get_bounds(ctx, from, &bound_kinds);
            let to_tsym = ctx.typ(to).tsym.expect("parameterized type symbol");
            for t in bounds.iter() {
                if let Some(sup) = types.as_super(ctx, syms, t, to_tsym) {
                    if is_raw_type_public(types, ctx, sup) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Java: boolean commonSuperWithDiffParameterization(Type t, Type s)
    /// (Infer.java:494-499)
    fn common_super_with_diff_parameterization(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        let supers = self.get_parameterized_supers(ctx, syms, types, t, s);
        for pair in supers.iter() {
            if !types.is_same_type(ctx, syms, pair.fst, pair.snd) {
                return true;
            }
        }
        false
    }

    /// Java: Type generateReferenceToTargetConstraint(JCTree tree, UndetVar
    /// from, Type to, Attr.ResultInfo resultInfo, InferenceContext
    /// inferenceContext) (Infer.java:501-517)
    #[allow(clippy::too_many_arguments)]
    fn generate_reference_to_target_constraint(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        tree: Option<TreeId>,
        from: TypeId,
        to: TypeId,
        result_info: &crate::comp::attr::ResultInfo,
        inference_context: &Rc<InferenceContext>,
    ) -> Result<TypeId, InferenceException> {
        let qtype = types.undet_var_qtype(ctx, from);
        let mut no_warnings = Warner::new(None);
        let context = Rc::clone(inference_context);
        context.solve_list(
            ctx,
            syms,
            types,
            List::of(qtype),
            &mut no_warnings,
        )?;
        let context = Rc::clone(inference_context);
        context.notify_change(ctx, syms, types)?;
        let inst = types
            .undet_var_get_inst(ctx, from)
            .expect("instantiated undet var");
        let rs_context = Rc::clone(result_info.check_context.inference_context());
        let captured = match tree {
            Some(tree) => rs_context.cached_capture(
                ctx,
                syms,
                types,
                tree,
                inst,
                !result_info.check_mode.update_tree_type(),
            ),
            None => types.capture(ctx, syms, inst),
        };
        let target = rs_context.as_undet_var(ctx, syms, types, to);
        if types.is_convertible(ctx, syms, captured, target) {
            Ok(syms.object_type)
        } else {
            Ok(to)
        }
    }
}

/// Helper: is_raw public view (Types.isRaw is private free function).
fn is_raw_type_public(types: &Types, ctx: &CompilerContext, t: TypeId) -> bool {
    types.is_raw_type_public(ctx, t)
}

/// Java: class Infer.PartiallyInferredMethodType (Infer.java:287-360)
pub fn partially_inferred_method_type(
    ctx: &mut CompilerContext,
    mt: TypeId,
    inference_context: Rc<InferenceContext>,
    env: Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
    warn: Rc<RefCell<Warner>>,
) -> TypeId {
    match &ctx.typ(mt).kind {
        TypeKindData::MethodType {
            argtypes,
            restype,
            thrown,
            recvtype,
        } => {
            let (argtypes, restype, thrown, recvtype) =
                (argtypes.clone(), *restype, thrown.clone(), *recvtype);
            ctx.alloc_type(crate::code::type_::Type::new(
                TypeTag::METHOD,
                None,
                TypeKindData::PartiallyInferredMethodType {
                    argtypes,
                    restype,
                    thrown,
                    recvtype,
                    inference_context,
                    env,
                    warn,
                },
            ))
        }
        _ => panic!("ClassCastException: MethodType expected"),
    }
}


impl Infer {
    /// Java: Infer.PartiallyInferredMethodType.check(Attr.ResultInfo)
    /// (Infer.java:312-365).
    ///
    /// PORTING_RULES: the upstream method returns a Type and reports the
    /// inference failure through ResultInfo.checkContext.report + Assert.error;
    /// the ported path returns the InferenceException (the Attr caller
    /// propagates/reports it) — registered Porting rules adapter.
    #[allow(
        clippy::too_many_arguments,
        clippy::needless_pass_by_ref_mut,
        clippy::collapsible_if
    )]
    pub fn partially_inferred_method_type_check(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        chk: &crate::comp::check::Check,
        t: TypeId,
        result_info: &crate::comp::attr::ResultInfo,
    ) -> Result<TypeId, InferenceException> {
        let (restype, inference_context, env, warn) = match &ctx.typ(t).kind {
            TypeKindData::PartiallyInferredMethodType {
                restype,
                inference_context,
                env,
                warn,
                ..
            } => (
                restype.expect("partially inferred return type"),
                Rc::clone(inference_context),
                Rc::clone(env),
                Rc::clone(warn),
            ),
            _ => panic!("ClassCastException: PartiallyInferredMethodType expected"),
        };
        let mut no_warnings = Warner::new(None);
        let saved_undet = inference_context.save(ctx, syms, types);
        let unchecked = warn
            .borrow()
            .has_non_silent_lint(crate::code::lint::LintCategory::UNCHECKED);
        let result: Result<TypeId, InferenceException> = (|| {
            if !unchecked {
                let should = self.should_propagate(
                    ctx,
                    syms,
                    types,
                    restype,
                    result_info,
                    &inference_context,
                );
                let min_context = if should {
                    let roots = self.roots(ctx, syms, types, t, None);
                    inference_context.min(ctx, syms, types, roots, false, &warn)?
                } else {
                    Rc::clone(&inference_context)
                };
                // Java: MethodType other = (MethodType)minContext.update(asMethodType())
                // (update is the identity in the ported InferenceContext).
                let other = t;
                let new_restype = self.generate_return_constraints(
                    ctx,
                    syms,
                    types,
                    env.tree,
                    result_info,
                    other,
                    &min_context,
                )?;
                if should {
                    let target_context =
                        Rc::clone(result_info.check_context.inference_context());
                    let inside_overload = result_info
                        .check_context
                        .deferred_attr_context()
                        .inside_overload_phase();
                    min_context.dup_to(ctx, syms, types, &target_context, inside_overload);
                    return Ok(new_restype);
                }
            }
            let context = Rc::clone(&inference_context);
            context.solve(ctx, syms, types, &mut no_warnings)?;
            let inst_mt = context.as_inst_type(ctx, syms, types, t);
            let mut ret = types.get_return_type_public(ctx, inst_mt);
            if unchecked {
                // inline logic from Attr.checkMethod - if unchecked conversion
                // was required, erase return type _after_ resolution, and check
                // against target
                ret = types.erasure(ctx, syms, ret);
            }
            let pos = env
                .tree
                .map(|tree| ctx.tree(tree).pos)
                .unwrap_or(crate::util::position::NOPOS);
            Ok(result_info.check(ctx, syms, chk, pos, ret))
        })();
        inference_context.rollback(ctx, syms, types, saved_undet);
        match result {
            Ok(ty) => Ok(ty),
            Err(exception) => {
                if let Some(diag) = exception.get_diagnostic() {
                    result_info.check_context.report(
                        ctx,
                        crate::util::position::NOPOS,
                        &diag,
                    );
                }
                // Upstream report throws ReportedException; the port records
                // the diagnostic and propagates the inference failure instead.
                Err(exception)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Infer.instantiateFunctionalInterface (Infer.java:633-690), needed
// by Attr.getTargetInfo (functional interface target typing). adapter: explicit
// ctx/syms/diags/chk/pos parameters per ADR 0001; Java's unchecked
// FunctionDescriptorLookupError is propagated as a panic (it is a RuntimeException
// upstream and is never caught inside this method).
// ---------------------------------------------------------------------------
impl Infer {
    /// Java: public Type instantiateFunctionalInterface(DiagnosticPosition pos,
    /// Type funcInterface, List<Type> paramTypes, Check.CheckContext checkContext)
    /// (Infer.java:633-690)
    #[allow(clippy::too_many_arguments)]
    pub fn instantiate_functional_interface(
        self: &Rc<Self>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        pos: i32,
        func_interface: TypeId,
        param_types: List<TypeId>,
        check_context: &Rc<dyn crate::comp::check::CheckContext>,
    ) -> Result<TypeId, crate::code::types::FunctionDescriptorLookupError> {
        use crate::util::jc_diagnostic::Fragment;
        // Java: checkContext.report(pos, diags.fragment(...)) — inlined at each
        // call site (the FragmentInfo type is module-private, so a closure type
        // annotation is not expressible; adapter adapter, no behavior change).
        macro_rules! report_fragment {
            ($ctx:expr, $fragment:expr) => {{
                let diag = diags.fragment(Fragment::from($fragment));
                check_context.report($ctx, pos, &diag);
            }};
        }
        if types.capture(ctx, syms, func_interface) == func_interface {
            // if capture doesn't change the type then return the target unchanged
            // (this means the target contains no wildcards!)
            return Ok(func_interface);
        }
        let tsym = ctx.typ(func_interface).tsym.expect("funcInterface tsym");
        let formal_interface = ctx.sym(tsym).ty.expect("funcInterface tsym type");
        let typeargs =
            crate::code::types::get_type_arguments_completed(ctx, syms, formal_interface, None);
        let func_interface_context = crate::comp::infer::InferenceContext::new(
            Rc::clone(self),
            ctx,
            syms,
            types,
            typeargs,
        );
        // Assert.check(paramTypes != null)
        let mut param_types = param_types;
        // Java: types.findDescriptorType(formalInterface) — an upstream
        // FunctionDescriptorLookupError is an unchecked exception that this
        // method does not catch; the port returns it as Err (adapter).
        let descriptor = types.find_descriptor_type(ctx, syms, diags, chk, formal_interface)?;
        let desc_parameter_types = types.get_parameter_types_public(ctx, descriptor);
        if desc_parameter_types.length() != param_types.length() {
            report_fragment!(
                ctx,
                crate::resources::compiler_properties::fragments::incompatible_arg_types_in_lambda()
            );
            return Ok(types.create_error_type_original(ctx, Some(func_interface), syms));
        }
        for p in desc_parameter_types.iter() {
            let undet = func_interface_context.as_undet_var(ctx, syms, types, p);
            if !types.is_same_type(ctx, syms, undet, *param_types.head()) {
                report_fragment!(
                    ctx,
                    crate::resources::compiler_properties::fragments::no_suitable_functional_intf_inst(
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            crate::comp::attr::type_display_of(ctx, func_interface),
                        )
                    )
                );
                return Ok(types.create_error_type_original(ctx, Some(func_interface), syms));
            }
            param_types = param_types.tail();
        }
        let mut actual_typeargs =
            crate::code::types::get_type_arguments_completed(ctx, syms, func_interface, None);
        let formal_typeargs =
            crate::code::types::get_type_arguments_completed(ctx, syms, formal_interface, None);
        let undetvars = {
            let state = func_interface_context.inner.borrow();
            state.undetvars.clone()
        };
        for undet in undetvars.iter() {
            let bounds = types.undet_var_get_bounds(
                ctx,
                undet,
                &[crate::code::type_::InferenceBound::EQ],
            );
            let mut inst: Option<TypeId> = None;
            for b in bounds.iter() {
                let contains_formal = types.contains_any(ctx, syms, b, &formal_typeargs);
                if !contains_formal {
                    inst = Some(b);
                    break;
                }
            }
            let inst = inst.unwrap_or(*actual_typeargs.head());
            types.undet_var_set_inst(ctx, syms, undet, inst);
            actual_typeargs = actual_typeargs.tail();
        }
        let owntype = func_interface_context.as_inst_type(ctx, syms, types, formal_interface);
        if !chk.check_valid_generic_type(ctx, syms, owntype) {
            // if the inferred functional interface type is not well-formed, or if
            // it's not a subtype of the original target, issue an error
            report_fragment!(
                ctx,
                crate::resources::compiler_properties::fragments::no_suitable_functional_intf_inst(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        crate::comp::attr::type_display_of(ctx, func_interface),
                    )
                )
            );
        }
        // propagate constraints as per JLS 18.2.1
        let mut no_warnings = crate::util::warner::Warner::new(None);
        let _ = check_context.compatible(ctx, syms, types, owntype, func_interface, &mut no_warnings);
        Ok(owntype)
    }
}

// ---------------------------------------------------------------------------
// prerequisite: Infer.instantiatePolymorphicSignatureInstance
// (Infer.java:555-630) + the ImplicitArgType mapping (expressed through
// DeferredTypeMap::new_implicit_arg, adapter). Explicit ctx/syms/attr/chk/diags
// parameters per ADR 0001.
// ---------------------------------------------------------------------------
impl Infer {
    /// Java: Type instantiatePolymorphicSignatureInstance(Env<AttrContext> env,
    /// MethodSymbol spMethod, Resolve.MethodResolutionContext resolveContext,
    /// List<Type> argtypes) (Infer.java:555-630)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/types/attr/chk/diags: explicit services;
///   - env: the attribution environment;
///   - sp_method: the signature polymorphic method (None for object);
///   - argtypes: the actual argument types
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Infer.java:555-630.
///
    pub fn instantiate_polymorphic_signature_instance(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        sp_method: Option<SymbolId>,
        argtypes: crate::util::list::List<TypeId>,
    ) -> TypeId {
        let sp_type = match sp_method {
            None => syms.object_type,
            Some(sp_method) => {
                let method_type = ctx.sym(sp_method).ty.expect("signature polymorphic method type");
                types.get_return_type_public(ctx, method_type)
            }
        };
        let restype = match env.next.as_ref().and_then(|next| next.tree) {
            None => sp_type,
            Some(next_tree) => match ctx.tree(next_tree).tag {
                crate::tree::jc_tree::Tag::TYPECAST => {
                    let (clazz, expr) = match &ctx.tree(next_tree).kind {
                        crate::tree::jc_tree::TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
                        _ => panic!("ClassCastException: JCTypeCast expected"),
                    };
                    let expr_matches = expr
                        .map(|expr| crate::tree::tree_info::skip_parens(ctx, expr) == env.tree.unwrap_or(expr))
                        .unwrap_or(false);
                    let _ = expr_matches;
                    let cast_expr_matches = match (expr, env.tree) {
                        (Some(expr), Some(tree)) => {
                            crate::tree::tree_info::skip_parens(ctx, expr) == tree
                        }
                        _ => false,
                    };
                    if cast_expr_matches {
                        clazz.map(|clazz| ctx.tree(clazz).ty.expect("cast type")).unwrap_or(sp_type)
                    } else {
                        sp_type
                    }
                }
                crate::tree::jc_tree::Tag::EXEC => {
                    let expr = match &ctx.tree(next_tree).kind {
                        crate::tree::jc_tree::TreeKind::ExpressionStatement { expr } => *expr,
                        _ => panic!("ClassCastException: JCExpressionStatement expected"),
                    };
                    let exec_expr_matches = match (expr, env.tree) {
                        (Some(expr), Some(tree)) => {
                            crate::tree::tree_info::skip_parens(ctx, expr) == tree
                        }
                        _ => false,
                    };
                    if exec_expr_matches {
                        syms.void_type
                    } else {
                        sp_type
                    }
                }
                _ => sp_type,
            },
        };
        let phase = crate::comp::resolve::MethodResolutionPhase::BOX;
        let mut implicit_map = crate::comp::deferred_attr::DeferredTypeMap::new_implicit_arg(
            sp_method,
            phase,
        );
        let mut dctx = crate::comp::deferred_attr::DeferredMapCtx {
            attr,
            syms,
            types,
            chk,
            diags,
        };
        let paramtypes = implicit_map.map(ctx, &mut dctx, &argtypes, &None);
        let ex_type = match sp_method {
            Some(sp_method) => {
                let method_type = ctx.sym(sp_method).ty.expect("signature polymorphic method type");
                ctx.typ(method_type).get_thrown_types(ctx)
            }
            None => crate::util::list::List::of(syms.throwable_type),
        };
        let ty = crate::code::type_::Type::new_method_type(
            paramtypes,
            restype,
            ex_type,
            syms.method_class,
        );
        ctx.alloc_type(ty)
    }
}

#[cfg(test)]
#[allow(clippy::collapsible_if, clippy::blocks_in_conditions)]
mod tests {
    use super::*;
    use crate::code::kinds::Kind;
    use crate::code::source::Source;
    use crate::code::symbol::{Symbol, SymbolKind};
    use crate::jvm::target::Target;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::names::Names;

    fn harness(graph_inference: bool) -> (CompilerContext, Symtab, Types, Rc<Infer>, Rc<Names>) {
        let mut ctx = CompilerContext::new();
        let names = Rc::new(Names::new(false));
        let symtab = Symtab::new(&mut ctx, Rc::clone(&names), Target::DEFAULT, Source::JDK17);
        let types = Types::with_names(Rc::clone(&names));
        let mut jcontext = Context::new();
        let messages = JavacMessages::instance(&mut jcontext);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let infer = Infer::new(&mut ctx, graph_inference, diags, Rc::clone(&names));
        (ctx, symtab, types, infer, names)
    }

    fn new_type_var(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        names: &Names,
        text: &str,
        flags: i64,
        upper: TypeId,
    ) -> TypeId {
        let name = names.from_utf(text.as_bytes());
        let symbol = ctx.alloc_symbol(Symbol::new(
            Kind::TYP,
            flags,
            name,
            None,
            Some(syms.no_symbol),
            SymbolKind::TypeVariableSymbol,
        ));
        let ty = ctx.alloc_type(crate::code::type_::Type::new(
            TypeTag::TYPEVAR,
            Some(symbol),
            TypeKindData::TypeVar {
                bound: Some(upper),
                lower: Some(syms.bot_type),
            },
        ));
        ctx.sym_mut(symbol).ty = Some(ty);
        ty
    }

    fn context_with_var(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        infer: &Rc<Infer>,
        names: &Names,
    ) -> (Rc<InferenceContext>, TypeId) {
        let _ = &syms;
        let tvar = new_type_var(ctx, syms, names, "T", 0, syms.object_type);
        let context = InferenceContext::new(
            Rc::clone(infer),
            ctx,
            syms,
            types,
            List::of(tvar),
        );
        (context, tvar)
    }

    #[test]
    fn from_type_var_fun_creates_an_undet_var_with_the_declared_bound() {
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { qtype, .. } => assert_eq!(*qtype, tvar),
            _ => panic!("expected an UndetVar"),
        }
        let declared = types.undet_var_get_declared_bounds(&ctx, uv);
        assert_eq!(declared, List::of(syms.object_type));
        assert_eq!(context.undet_vars(), List::of(uv));
        assert_eq!(context.inference_vars(), List::of(tvar));
    }

    #[test]
    fn eq_bound_solves_to_the_bound_type() {
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        let string_type = syms.string_type;
        types.undet_var_add_bound(
            &mut ctx,
            &syms,
            uv,
            InferenceBound::EQ,
            string_type,
            false,
        );
        let mut warn = Warner::new(None);
        context
            .solve(&mut ctx, &syms, &types, &mut warn)
            .expect("solve should succeed");
        assert!(context.restvars(&mut ctx, &syms, &types).is_empty());
        assert_eq!(
            context.inst_types(&mut ctx, &types),
            List::of(string_type)
        );
    }

    #[test]
    fn lower_bound_solves_to_the_bound_type() {
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        let string_type = syms.string_type;
        types.undet_var_add_bound(
            &mut ctx,
            &syms,
            uv,
            InferenceBound::LOWER,
            string_type,
            false,
        );
        let mut warn = Warner::new(None);
        context
            .solve(&mut ctx, &syms, &types, &mut warn)
            .expect("solve should succeed");
        assert_eq!(
            context.inst_types(&mut ctx, &types),
            List::of(string_type)
        );
    }

    #[test]
    fn upper_bound_solves_to_the_bound_type() {
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        let string_type = syms.string_type;
        types.undet_var_add_bound(
            &mut ctx,
            &syms,
            uv,
            InferenceBound::UPPER,
            string_type,
            false,
        );
        let mut warn = Warner::new(None);
        context
            .solve(&mut ctx, &syms, &types, &mut warn)
            .expect("solve should succeed");
        assert_eq!(
            context.inst_types(&mut ctx, &types),
            List::of(string_type)
        );
    }

    #[test]
    fn save_and_rollback_restore_the_bounds_and_instances() {
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        let string_type = syms.string_type;
        let int_type = syms.int_type;
        types.undet_var_add_bound(
            &mut ctx,
            &syms,
            uv,
            InferenceBound::UPPER,
            string_type,
            false,
        );
        let saved = context.save(&mut ctx, &syms, &types);
        types.undet_var_add_bound(
            &mut ctx,
            &syms,
            uv,
            InferenceBound::UPPER,
            int_type,
            false,
        );
        types.undet_var_set_inst(&mut ctx, &syms, uv, int_type);
        assert_eq!(
            types
                .undet_var_get_bounds(&ctx, uv, &[InferenceBound::UPPER])
                .length(),
            3
        );
        context.rollback(&mut ctx, &syms, &types, saved);
        let uv_after = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        assert!(types.undet_var_get_inst(&ctx, uv_after).is_none());
        let object_type = syms.object_type;
        assert_eq!(
            types.undet_var_get_bounds(&ctx, uv_after, &[InferenceBound::UPPER]),
            List::of(string_type).append(object_type)
        );
    }

    #[test]
    fn notify_change_invokes_the_free_type_listener() {
        struct RecordingListener {
            called: RefCell<bool>,
        }
        impl FreeTypeListener for RecordingListener {
            fn types_inferred(
                &self,
                _ctx: &mut CompilerContext,
                _syms: &Symtab,
                _types: &Types,
                _inference_context: &InferenceContext,
            ) -> Result<(), InferenceException> {
                *self.called.borrow_mut() = true;
                Ok(())
            }
        }
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let listener = Rc::new(RecordingListener {
            called: RefCell::new(false),
        });
        context.add_free_type_listener(
            &mut ctx,
            &syms,
            &types,
            List::of(tvar),
            listener.clone(),
        );
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        let object_type = syms.object_type;
        types.undet_var_set_inst(&mut ctx, &syms, uv, object_type);
        let syms_for_notify = syms;
        context
            .notify_change(&mut ctx, &syms_for_notify, &types)
            .expect("notify_change should succeed");
        assert!(*listener.called.borrow());
    }

    #[test]
    fn as_inst_type_replaces_inference_vars_with_their_instances() {
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        let string_type = syms.string_type;
        types.undet_var_set_inst(&mut ctx, &syms, uv, string_type);
        let list_type = {
            let list_type = syms.list_type;
            let list_tsym = ctx.typ(list_type).tsym.expect("list symbol");
            ctx.alloc_type(crate::code::type_::Type::new(
                TypeTag::CLASS,
                Some(list_tsym),
                TypeKindData::ClassType {
                    outer_field: None,
                    typarams_field: Some(List::of(tvar)),
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                },
            ))
        };
        let as_inst = context.as_inst_type(&mut ctx, &syms, &types, list_type);
        match &ctx.typ(as_inst).kind {
            TypeKindData::ClassType { typarams_field, .. } => {
                assert_eq!(typarams_field.clone(), Some(List::of(string_type)));
            }
            _ => panic!("expected a class type"),
        }
    }

    #[test]
    fn inference_steps_accept_relevant_undet_vars() {
        let (mut ctx, syms, types, infer, names) = harness(true);
        let (context, tvar) = context_with_var(&mut ctx, &syms, &types, &infer, &names);
        let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
        let string_type = syms.string_type;
        types.undet_var_add_bound(
            &mut ctx,
            &syms,
            uv,
            InferenceBound::EQ,
            string_type,
            false,
        );
        assert!(InferenceStep::EQ
            .accepts(&mut ctx, &syms, &types, uv, &context)
            .unwrap());
        assert!(!InferenceStep::LOWER
            .accepts(&mut ctx, &syms, &types, uv, &context)
            .unwrap());
        let solved = InferenceStep::EQ
            .solve(&infer, &mut ctx, &syms, &types, uv, &context)
            .unwrap();
        assert_eq!(solved, string_type);
    }

    #[test]
    fn empty_context_is_shared_and_empty() {
        let context = empty_context();
        assert!(context.inference_vars().is_empty());
        assert!(context.undet_vars().is_empty());
        assert!(Rc::ptr_eq(&context, &empty_context()));
    }

    /// JDK 17 internal-API oracle (tools/gen-infer-oracle/GenInferOracle.java):
    /// InferenceContext.solve results for hand-built constraints, collected at
    /// development time (no JVM at test time).
    #[test]
    fn infer_oracle_constraints_match_the_javac_internal_api() {
        let data = include_str!("../../tests/data/infer-oracle.txt");
        let mut checked = 0;
        for line in data.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts.len() < 6 || parts[0] != "SCENARIO" {
                continue;
            }
            let label = parts[1];
            let expected_name = parts[3];
            let oracle_actual = parts[5];
            assert_eq!(
                oracle_actual, expected_name,
                "oracle self-check for {label}"
            );
            let (mut ctx, syms, types, infer, names) = harness(true);
            let object_type = syms.object_type;
            let (tvar, ib, bound, expected_ty) = match label {
                "eq_string" => (
                    new_type_var(&mut ctx, &syms, &names, "T", 0, object_type),
                    InferenceBound::EQ,
                    syms.string_type,
                    syms.string_type,
                ),
                "lower_string" => (
                    new_type_var(&mut ctx, &syms, &names, "T", 0, object_type),
                    InferenceBound::LOWER,
                    syms.string_type,
                    syms.string_type,
                ),
                "upper_string" => (
                    new_type_var(&mut ctx, &syms, &names, "T", 0, object_type),
                    InferenceBound::UPPER,
                    syms.string_type,
                    syms.string_type,
                ),
                "upper_object" => (
                    new_type_var(&mut ctx, &syms, &names, "T", 0, object_type),
                    InferenceBound::UPPER,
                    object_type,
                    object_type,
                ),
                "throws_runtime" => (
                    new_type_var(
                        &mut ctx,
                        &syms,
                        &names,
                        "T",
                        flags::THROWS,
                        object_type,
                    ),
                    InferenceBound::UPPER,
                    syms.runtime_exception_type,
                    syms.runtime_exception_type,
                ),
                other => panic!("unknown scenario {other}"),
            };
            let context = InferenceContext::new(
                Rc::clone(&infer),
                &mut ctx,
                &syms,
                &types,
                List::of(tvar),
            );
            let uv = context.as_undet_var(&mut ctx, &syms, &types, tvar);
            types.undet_var_add_bound(&mut ctx, &syms, uv, ib, bound, false);
            let mut warn = Warner::new(None);
            context
                .solve(&mut ctx, &syms, &types, &mut warn)
                .expect("solve should succeed");
            let inst = context.inst_types(&mut ctx, &types);
            assert_eq!(
                *inst.head(),
                expected_ty,
                "scenario {label} instantiated type"
            );
            checked += 1;
        }
        assert_eq!(checked, 5, "oracle scenarios");
    }

}


