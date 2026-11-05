//! Inference support: reachability queries, change listeners and the bound
//! checking/reporting helpers shared with the inference context.

use std::rc::Rc;

use crate::code::flags;
use crate::code::symtab::Symtab;
use crate::code::type_::{InferenceBound, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::infer::{FreeTypeListener, Infer, InferenceContext, InferenceException};
use crate::comp::infer_graph::bound_filter;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::resources::compiler_properties::fragments;
use crate::resources::diagnostic_contract::TypeArg;
use crate::util::list::List;
use crate::util::warner::Warner;

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
                TypeKindData::WildcardType {
                    type_: Some(type_), ..
                } => self.visit(ctx, syms, types, *type_),
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
                TypeKindData::ArrayType {
                    elemtype: Some(elem),
                    ..
                } => self.visit(ctx, syms, types, *elem),
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
                let current = bounds;
                for b in &current {
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
pub(crate) fn inference_context_free_placeholder(
    _ctx: &mut CompilerContext,
    _syms: &Symtab,
    _types: &Types,
    t: TypeId,
) -> TypeId {
    t
}

/// Java: void dupTo free-type listener "inferenceContext ->
/// InferenceContext.this.notifyChange()" (InferenceContext.java:330-333)
pub(crate) struct NotifyChangeListener {
    /// Java: the dupTo listener is a lambda that captures InferenceContext.this,
    /// i.e. a strong reference to the context being propagated to
    /// (InferenceContext.java:330-333). Holding a Weak here instead let the
    /// context die before the target context ever notified it -- the address was
    /// then reused by a later context, so upgrade() failed while the *current*
    /// context was still alive, and the free-type listeners registered on the
    /// original context were never notified (R35).
    pub(crate) target: Rc<InferenceContext>,
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
pub(crate) struct PropagateListener {
    pub(crate) original: std::rc::Weak<InferenceContext>,
    pub(crate) var: TypeId,
    pub(crate) min_map: Vec<(TypeId, Vec<TypeId>)>,
    /// Upstream captures the caller's Warner reference (InferenceContext.java:
    /// 380-387); the Rc keeps the shared lint state alive across the deferred
    /// notification.
    pub(crate) warn: Rc<Warner>,
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
        let warn = Rc::clone(&self.warn);
        infer.do_incorporation(ctx, syms, types, &original, &warn)?;
        drop(warn);
        original.notify_change(ctx, syms, types)
    }
}

/// Java: InferenceContext.min's unreachable-variables listener
/// (InferenceContext.java:385-390)
pub(crate) struct UnreachableVarsListener {
    pub(crate) original: std::rc::Weak<InferenceContext>,
    pub(crate) unreachable_vars: List<TypeId>,
    /// Upstream captures the caller's Warner reference (InferenceContext.java:
    /// 392-395); the Rc keeps the shared lint state alive across the deferred
    /// notification.
    pub(crate) warn: Rc<Warner>,
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
        let warn = Rc::clone(&self.warn);
        let _ = inference_context;
        original.solve_list(ctx, syms, types, self.unreachable_vars.clone(), &warn)?;
        drop(warn);
        original.notify_change(ctx, syms, types)
    }
}

// =====================================================================
// Helpers
// =====================================================================

pub(crate) fn list_diff(ts: &List<TypeId>, ss: &List<TypeId>) -> List<TypeId> {
    let mut result: List<TypeId> = List::nil();
    let current = ts.clone();
    for t in &current {
        if !ss.iter().any(|s| s == t) {
            result = result.append(t);
        }
    }
    result
}

pub(crate) fn list_remove(ts: &List<TypeId>, t: TypeId) -> List<TypeId> {
    let mut result: List<TypeId> = List::nil();
    let current = ts.clone();
    for x in &current {
        if x != t {
            result = result.append(x);
        }
    }
    result
}

pub(crate) fn list_contains_all(ts: &List<TypeId>, ss: &List<TypeId>) -> bool {
    ss.iter().all(|s| ts.iter().any(|t| t == s))
}

/// Java: Type.containsAny(List<Type> ts1, List<Type> ts2)
pub(crate) fn contains_any_list(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    ts1: List<TypeId>,
    ts2: List<TypeId>,
) -> bool {
    let current = ts1;
    for t in &current {
        let mut other = ts2.clone();
        while other.non_empty() {
            if types.contains(ctx, syms, t, *other.head()) {
                return true;
            }
            other = other.tail();
        }
    }
    false
}

pub(crate) fn elemtype_of(ctx: &mut CompilerContext, t: TypeId) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
        _ => panic!("unexpected array type"),
    }
}

pub(crate) fn new_array_type_ctx(
    ctx: &mut CompilerContext,
    elem: TypeId,
    tsym: SymbolId,
) -> TypeId {
    ctx.alloc_type(crate::code::type_::Type::new(
        TypeTag::ARRAY,
        Some(tsym),
        TypeKindData::ArrayType {
            elemtype: Some(elem),
            varargs: false,
        },
    ))
}

pub(crate) fn components_of(ctx: &mut CompilerContext, t: TypeId) -> List<TypeId> {
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

pub(crate) fn new_type_variable_symbol(
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

pub(crate) fn new_captured_type_ctx(
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
pub(crate) fn check_bound(
    _infer: &Infer,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    s: TypeId,
    t: TypeId,
    ib_s: InferenceBound,
    ib_t: InferenceBound,
    warn: &Warner,
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
pub(crate) fn report_inst_error(
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
        InferenceBound::LOWER => fragments::inferred_do_not_conform_to_lower_bounds(inst_arg, args),
        InferenceBound::UPPER => fragments::inferred_do_not_conform_to_upper_bounds(inst_arg, args),
    };
    let diag = infer.diags.fragment(fragment.into());
    Err(infer.error(Some(diag)))
}

/// Java: CheckBounds.report(InferenceBound from, InferenceBound to)
/// (Infer.java:793-800)
pub(crate) fn report_bound_error_from_to(
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
pub(crate) fn report_bound_error_two(
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
        TypeKindData::UndetVar { qtype, .. } => *qtype,
        _ => uv,
    };
    let frag1_diag = infer.diags.fragment(frag1);
    let frag2_diag = infer.diags.fragment(frag2);
    let fragment = fragments::incompatible_bounds(
        crate::comp::check::type_arg_with_rich_where(ctx, qtype),
        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(Rc::new(
            frag1_diag,
        )),
        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(Rc::new(
            frag2_diag,
        )),
    );
    let diag = infer.diags.fragment(fragment.into());
    Err(infer.error(Some(diag)))
}

/// Java: Fragment getBoundFragment(InferenceBound ib, List<Type> types)
/// (Infer.java:1282-1290)
pub(crate) fn get_bound_fragment(
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
pub(crate) fn bounds_type_args(ctx: &mut CompilerContext, bounds: List<TypeId>) -> Vec<TypeArg> {
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
pub(crate) fn check_compatible_upper_bounds(
    infer: &Infer,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    uv: TypeId,
) -> Result<(), InferenceException> {
    let upper_bounds = types.undet_var_get_bounds(ctx, uv, &[InferenceBound::UPPER]);
    let mut hibounds: List<TypeId> = List::nil();
    let current = upper_bounds;
    for t in &current {
        if bound_filter(ctx, syms, types, inference_context, t) {
            hibounds = hibounds.append(t);
        }
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
