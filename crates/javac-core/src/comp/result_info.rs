//! The attribution result descriptor: the expected type, the check context and
//! the deferred-check flavor.

use std::rc::Rc;

use crate::code::kinds::KindSelector;
use crate::code::symtab::Symtab;
use crate::comp::check::Check;
use crate::comp::type_display::type_display;
use crate::compiler_context::CompilerContext;
use crate::ids::{TreeId, TypeId};

/// Java: Attr.ResultInfo — the checked subset (see the file header for the
/// registered checkContext/checkMode fields).
#[derive(Clone)]
pub struct ResultInfo {
    /// Java: final KindSelector pkind
    pub pkind: KindSelector,
    /// Java: final Type pt (Java Type.noType -> None; ADR 0001).
    pub pt: Option<TypeId>,
    /// Java: final CheckContext checkContext
    pub check_context: Rc<dyn crate::comp::check::CheckContext>,
    /// Java: final CheckMode checkMode
    pub check_mode: crate::comp::check::CheckMode,
    /// Java: the ResultInfo subclass flavor (MethodAttrInfo is an inner subclass
    /// upstream; the port carries the flavor explicitly — registered adapter).
    pub kind: ResultInfoKind,
    pub argument_attr_exclude: Option<crate::comp::argument_attr::UniquePos>,
    /// Java: Attr.RecoveryInfo overrides check() to wrap the result in
    /// chk.checkNonVoid (Attr.java:598-620); expressed as a flag (adapter registered).
    pub check_non_void: bool,
    /// Java: Resolve.MethodResultInfo overrides check() so that the found type
    /// is normalised through U(found) and a re-capture before the compatibility
    /// test (Resolve.java:1072-1084). The port has a single ResultInfo
    /// structure, so the MethodResultInfo flavour is carried by the diagnostic
    /// kind that only the method-check path sets; check() reproduces the
    /// upstream normalisation when it is present (registered adapter).
    pub method_diag: Option<crate::comp::resolve::MethodCheckDiag>,
    /// The identity of this ResultInfo (the upstream ArgumentType.speculativeTypes
    /// map is keyed by ResultInfo object identity; Rust needs an explicit id).
    /// adapter registered adapter: unique ids are assigned by new_with_mode, so every
    /// dup(...) call gets a fresh identity like the upstream constructors.
    pub id: u64,
}

/// Java: the ResultInfo subclass flavor (ResultInfo / Attr.MethodAttrInfo —
/// registered adapter: Java subclassing is expressed by this enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ResultInfoKind {
    /// Java: plain Attr.ResultInfo
    #[default]
    Normal,
    /// Java: Attr.MethodAttrInfo
    MethodAttr,
}

/// Monotonic ResultInfo identity counter (adapter adapter, see ResultInfo::id).
static NEXT_RESULT_INFO_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl ResultInfo {
    /// Java: ResultInfo(KindSelector pkind, Type pt) — the upstream 2-arg
    /// constructor uses chk.basicHandler and CheckMode.NORMAL.
    pub fn new(
        pkind: KindSelector,
        pt: Option<TypeId>,
        check_context: Rc<dyn crate::comp::check::CheckContext>,
    ) -> ResultInfo {
        ResultInfo::new_with_mode(
            pkind,
            pt,
            check_context,
            crate::comp::check::CheckMode::NORMAL,
        )
    }

    /// Java: protected ResultInfo(KindSelector pkind, Type pt, CheckContext
    /// checkContext, CheckMode checkMode)
    pub fn new_with_mode(
        pkind: KindSelector,
        pt: Option<TypeId>,
        check_context: Rc<dyn crate::comp::check::CheckContext>,
        check_mode: crate::comp::check::CheckMode,
    ) -> ResultInfo {
        ResultInfo {
            pkind,
            pt,
            check_context,
            check_mode,
            check_non_void: false,
            kind: ResultInfoKind::Normal,
            method_diag: None,
            argument_attr_exclude: None,
            id: NEXT_RESULT_INFO_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Java: Attr.MethodAttrInfo(CheckContext checkContext) (Attr.java:568-571)
    /// — super(KindSelector.VAL, Infer.anyPoly, checkContext).
    pub fn method_attr_info(
        check_context: Rc<dyn crate::comp::check::CheckContext>,
        any_poly: TypeId,
    ) -> ResultInfo {
        let mut info = ResultInfo::new_with_mode(
            KindSelector::VAL,
            Some(any_poly),
            check_context,
            crate::comp::check::CheckMode::NORMAL,
        );
        info.kind = ResultInfoKind::MethodAttr;
        info
    }

    /// Java: protected boolean needsArgumentAttr(JCTree tree) — the base
    /// returns false; MethodAttrInfo overrides it to true (Attr.java:563-596).
    pub fn needs_argument_attr(&self, ctx: &CompilerContext, tree: TreeId) -> bool {
        if !matches!(self.kind, ResultInfoKind::MethodAttr) {
            return false;
        }
        match &self.argument_attr_exclude {
            Some(exclude) => exclude.pos != ctx.tree(tree).pos,
            None => true,
        }
    }

    /// Java: protected Type check(final DiagnosticPosition pos, final Type
    /// found) — chk.checkType(pos, found, pt, checkContext)
    pub fn check(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        chk: &Check,
        pos: i32,
        found: TypeId,
    ) -> TypeId {
        let pt = match self.pt {
            Some(pt) => pt,
            None => crate::code::symtab::no_type_id(ctx),
        };
        // Java: Resolve.MethodResultInfo.check (Resolve.java:1072-1084) replaces
        // the found type with U(found) and re-captures its upper bound before
        // the compatibility test. Without it a captured variable such as
        // "capture of ? extends Class<X>" reaches compatible() unexpanded and is
        // rejected although its upper bound satisfies the formal.
        let found = match self.method_diag {
            Some(_) => {
                let u_result = if found == pt {
                    found
                } else {
                    chk.types.cvar_upper_bound_of(ctx, found)
                };
                chk.types.capture(ctx, syms, u_result)
            }
            None => found,
        };
        let result = chk.check_type(ctx, syms, pos, found, pt, Rc::clone(&self.check_context));
        if self.check_non_void {
            // Java: RecoveryInfo overrides check() to wrap the result with
            // chk.checkNonVoid (Attr.java:598-620); adapter flag-based expression.
            chk.check_non_void(ctx, syms, pos, result)
        } else {
            result
        }
    }

    /// Java: public String toString() —
    /// (pt != null ? pt.toString() : "")
    pub fn to_string(&self, ctx: &CompilerContext) -> String {
        match self.pt {
            Some(pt) => type_display(ctx, pt),
            None => String::new(),
        }
    }

    /// Java: protected ResultInfo dup(CheckContext newContext) — ResultInfo.dup
    /// overload used by DeferredAttrNode.process (Attr.java:ResultInfo).
    pub fn dup_check_context(
        &self,
        check_context: Rc<dyn crate::comp::check::CheckContext>,
    ) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(CheckContext) -> new MethodAttrInfo(newContext)
            let mut info =
                ResultInfo::new_with_mode(self.pkind, self.pt, check_context, self.check_mode);
            info.kind = ResultInfoKind::MethodAttr;
            info.check_non_void = self.check_non_void;
            return info;
        }
        let mut result =
            ResultInfo::new_with_mode(self.pkind, self.pt, check_context, self.check_mode);
        result.check_non_void = self.check_non_void;
        result
    }

    /// Java: protected ResultInfo dup(Type newPt)
    pub fn dup(&self, new_pt: Option<TypeId>) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(Type) throws IllegalStateException.
            panic!("IllegalStateException");
        }
        let mut result = ResultInfo::new_with_mode(
            self.pkind,
            new_pt,
            Rc::clone(&self.check_context),
            self.check_mode,
        );
        result.check_non_void = self.check_non_void;
        result
    }
    /// Java: protected ResultInfo dup(CheckContext newContext)
    pub fn dup_context(&self, new_context: Rc<dyn crate::comp::check::CheckContext>) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(CheckContext) -> new MethodAttrInfo(newContext)
            let mut info =
                ResultInfo::new_with_mode(self.pkind, self.pt, new_context, self.check_mode);
            info.kind = ResultInfoKind::MethodAttr;
            info.check_non_void = self.check_non_void;
            return info;
        }
        let mut result =
            ResultInfo::new_with_mode(self.pkind, self.pt, new_context, self.check_mode);
        result.check_non_void = self.check_non_void;
        result
    }

    /// Java: protected ResultInfo dup(Type newPt, CheckContext newContext)
    pub fn dup_pt_context(
        &self,
        new_pt: Option<TypeId>,
        new_context: Rc<dyn crate::comp::check::CheckContext>,
    ) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            panic!("IllegalStateException");
        }
        let mut result =
            ResultInfo::new_with_mode(self.pkind, new_pt, new_context, self.check_mode);
        result.check_non_void = self.check_non_void;
        result
    }

    /// Java: protected ResultInfo dup(Type newPt, CheckContext newContext,
    /// CheckMode newMode)
    pub fn dup_pt_context_mode(
        &self,
        new_pt: Option<TypeId>,
        new_context: Rc<dyn crate::comp::check::CheckContext>,
        new_mode: crate::comp::check::CheckMode,
    ) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            panic!("IllegalStateException");
        }
        let mut result = ResultInfo::new_with_mode(self.pkind, new_pt, new_context, new_mode);
        result.check_non_void = self.check_non_void;
        result
    }

    /// Java: protected ResultInfo dup(CheckMode newMode)
    pub fn dup_mode(&self, new_mode: crate::comp::check::CheckMode) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            panic!("IllegalStateException");
        }
        let mut result = ResultInfo::new_with_mode(
            self.pkind,
            self.pt,
            Rc::clone(&self.check_context),
            new_mode,
        );
        result.check_non_void = self.check_non_void;
        result
    }
}
