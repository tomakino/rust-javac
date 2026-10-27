
// Origin: com/sun/tools/javac/comp/Check.java @ 982d53c (jdk-17.0.19-ga)
//
// Registration: this file ports the Check subset required by
// the Enter/Flow batches (class enter symbol/flags/duplicate checks, the
// exception set utilities, and the -3 methods below).
//
// PORTED IN THE -3 STEP:
// - typeTagError / checkNonVoid / checkClassOrArrayType / asTypeParam /
// checkNullOrRefType / checkEmptyIf / checkDivZero / warnDivZero /
// duplicateErasureError / varargsDuplicateError (Check.java:288-291,
// 700-717, 723-727, 814-822, 1007-1016, 371-375, 3718-3737, 3783-3787).
// STILL NOT PORTED (registered; dependencies in brackets):
// - checkCastable (Check.java:632-645) [Types.isCastable(Type, Type, Warner)];
// - checkRedundantCast (Check.java:647-660) [TreeInfo.containsTypeAnnotation,
// DeferredLintHandler wiring via Attr];
// - validate/Validator (Check.java:1397-1525) [firstIncompatibleTypeArg,
// checkClassBounds, TreeInfo.containsTypeAnnotation, JCTree.Visitor scanner];
// - checkUnique MTH branches [Check holds no Symtab service yet; the caller
// passes no Symtab (ADR 0001 service wiring pending)];
// - the rest of the upstream Check body (type checking beyond the ported
// subset, annotation checks, lambda/method checks, warnings, sunApiHandler,
//...).
// The ported methods keep the upstream bodies and error keys; where a dependency
// is not ported the branch is marked TODO and not executed (no substitute).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::code::symbol::{
    symbol_encl_class, symbol_is_directly_or_indirectly_local, symbol_is_static_or_instance_init,
    symbol_location, symbol_packge, SymbolKind,
};
use crate::code::deferred_lint_handler::DeferredLintHandler;
use crate::code::lint::{Lint, LintCategory};
use crate::code::preview::Preview;
use crate::code::kinds::Kind;
use crate::code::symtab::Symtab;
use crate::code::symbol::CompletionFailure;
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_scanner::{tree_scanner_visitor_delegate, TreeScannerVisitor};
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::deferred_attr::DeferredAttrContext;
use crate::comp::enter::Enter;
use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;
use crate::comp::infer::InferenceContext;
use crate::ids::TypeId;
use crate::util::jc_diagnostic::JCDiagnostic;
use crate::util::warner::Warner;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, ScopeId, SymbolId, TreeId};
use crate::resources::compiler_properties::{errors, fragments, warnings};
use crate::resources::diagnostic_contract::{DiagnosticArg, FlagArg, KindNameArg, SymbolArg};
use crate::resources::diagnostic_contract::ErrorInfo;
use crate::util::jc_diagnostic::{Error as JCDiagnosticError, Factory};
use crate::util::abstract_log::AbstractLog;
use crate::util::constants::ConstantValue;
use crate::util::jc_diagnostic::{SimpleDiagnosticPosition, Warning};
use crate::util::log::Log;
use crate::util::mandatory_warning_handler::{MandatoryWarningHandler, MandatoryWarningLog};
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::names::Names;

/// Java: public class Check (Enter subset; see the file header).
pub struct Check {
    pub names: Rc<Names>,
    pub log: Rc<RefCell<Log>>,
    pub diags: Rc<Factory>,
    /// Java: private final boolean allowRecords;
    pub allow_records: bool,
    /// Java: private final boolean allowSealed;
    pub allow_sealed: bool,
    /// Java: char syntheticNameChar;
    pub synthetic_name_char: char,
    /// Java: private Map<Pair<ModuleSymbol, Name>,ClassSymbol> compiled
    pub compiled: HashMap<(SymbolId, Name), SymbolId>,
    /// Java: private Map<Pair<Name, Name>, Integer> localClassNameIndexes
    pub local_class_name_indexes: HashMap<(Name, Name), i32>,
    /// Java: final Types types
    pub types: Rc<Types>,
    /// Java: CheckContext basicHandler = new CheckContext() {...}
    pub basic_handler: Rc<dyn CheckContext>,
    /// The shared self handle used by the free-type listener (Java: the
    /// listener closes over Check.this; the port stores a weak self reference so
    /// the listener does not keep the Check alive — registered adapter).
    pub self_ref: RefCell<std::rc::Weak<RefCell<Check>>>,
    /// Java: private Lint lint (Check.java:187) — restored to the upstream
    /// field shape (, Porting rules official structure).
    pub lint: Rc<Lint>,
    /// Java: private DeferredLintHandler deferredLintHandler (Check.java:193)
    pub deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
    /// Java: the Symtab service (Symtab.instance(context)); explicit service
    /// per ADR 0001.
    pub syms: Rc<RefCell<Symtab>>,
    /// Java: final TypeAnnotations typeAnnotations (service cell, installed by
    /// the wiring; required by validateTypeAnnotation).
    pub type_annotations: Rc<RefCell<Option<Rc<crate::code::type_annotations::TypeAnnotations>>>>,
    /// Java: final boolean warnOnAnyAccessToMembers (from -Xlint options);
    /// injected through the setter at wiring time (Adapter).
    pub warn_on_any_access_to_members: bool,
    /// Java: private MethodSymbol method (Check.java:109).
    pub method: Option<SymbolId>,
    /// Java: final Source source (Check.java:200).
    pub source: crate::code::source::Source,
    /// Java: Feature.PRIVATE_SAFE_VARARGS.allowedInSource(source) (Check.java:970-977)
    pub private_safe_varargs: bool,
    /// Java: private final Enter enter (Check.java:116) — injected at wiring
    /// time to avoid the Check/Enter construction cycle.
    pub enter: RefCell<Option<Rc<RefCell<Enter>>>>,
    /// Java: MandatoryWarningHandler deprecationHandler (Check.java:177).
    pub deprecation_handler:
        Rc<RefCell<MandatoryWarningHandler<Rc<RefCell<Log>>>>>,
    /// Java: MandatoryWarningHandler removalHandler (Check.java:181).
    pub removal_handler: Rc<RefCell<MandatoryWarningHandler<Rc<RefCell<Log>>>>>,
    /// Java: private final Preview preview (Check.java:99; installed by the
    /// wiring because the ported Preview is created after Check).
    pub preview: Rc<RefCell<Option<Rc<Preview>>>>,
    /// Java: private final Resolve rs (Check.java:87) — installed by the wiring
    /// because the Resolve service is created by Attr after Check (registered
    /// adapter, same pattern as enter/type_annotations/preview).
    pub resolve: Rc<RefCell<Option<Rc<RefCell<crate::comp::resolve::Resolve>>>>>,
    /// Java: private final JavaFileManager fileManager (Check.java:95) —
    /// installed by the wiring (service table cell).
    pub file_manager:
        Rc<RefCell<Option<Rc<dyn javac_tools::tools::java_file_manager::JavaFileManager>>>>,
    /// Java: private final Profile profile (Check.java:98) — installed by the
    /// wiring.
    pub profile: Rc<RefCell<Option<Rc<crate::jvm::profile::Profile>>>>,
}


/// Installs the shared weak self handle on a Check instance (the free-type
/// listener needs to call back into the same Check; Java closes over Check.this,
/// the port stores a weak reference and this helper wires it at construction).
///
/// # Arguments
///   - check: the shared Check handle just constructed.
///
/// # Returns
///   - none.
///
/// # Panics
///   - none.
///
/// Upstream: Check.java:604-609 (the listener captures Check.this).
pub fn attach_self(check: &Rc<RefCell<Check>>) {
    let borrowed = check.borrow();
    *borrowed.self_ref.borrow_mut() = Rc::downgrade(check);
}

/// Java: the lambda passed to addFreeTypeListener in Check.checkType
/// (Check.java:606-609). The listener re-checks the instantiated found/required
/// types once the inference variables are solved.
struct CheckFreeTypeListener {
    /// The Check instance that registered the listener (weak: the Java lambda
    /// keeps Check alive through the enclosing instance; the port follows the
    /// weak-reference adapter used by the service cells).
    check: std::rc::Weak<RefCell<Check>>,
    /// The diagnostic position passed to checkType.
    pos: i32,
    /// The found type to instantiate.
    found: TypeId,
    /// The required type to instantiate.
    req: TypeId,
    /// The check context of the replayed check.
    check_context: Rc<dyn CheckContext>,
}

impl crate::comp::infer::FreeTypeListener for CheckFreeTypeListener {
    /// Java: void typesInferred(InferenceContext inferenceContext):
    /// checkType(pos, solvedContext.asInstType(found),
    /// solvedContext.asInstType(req), checkContext) (Check.java:604-609).
    ///
    /// # Arguments
    ///   - ctx/syms/types: the compiler services;
    ///   - inference_context: the solved inference context.
    ///
    /// # Returns
    ///   - Ok(()) when the replayed check completes.
    ///
    /// # Panics
    ///   - none; a dropped Check stops the replay (the listener is dropped with it).
    ///
    /// Upstream: Check.java:604-609.
    fn types_inferred(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<(), crate::comp::infer::InferenceException> {
        let Some(check) = self.check.upgrade() else {
            return Ok(());
        };
        let found = inference_context.as_inst_type(ctx, syms, types, self.found);
        let req = inference_context.as_inst_type(ctx, syms, types, self.req);
        let check = check.borrow();
        let _ = check.check_type(ctx, syms, self.pos, found, req, Rc::clone(&self.check_context));
        Ok(())
    }
}

impl Check {
    /// Java: protected Check(Context context) — the ported dependency subset.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        diags: Rc<Factory>,
        source: crate::code::source::Source,
        synthetic_name_char: char,
        types: Rc<Types>,
        lint: Rc<Lint>,
        deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
        syms: Rc<RefCell<Symtab>>,
    ) -> Check {
        let verbose_deprecated = lint.is_enabled(LintCategory::DEPRECATION);
        let verbose_removal = lint.is_enabled(LintCategory::REMOVAL);
        let deprecation_handler = Rc::new(RefCell::new(MandatoryWarningHandler::new(
            Rc::clone(&log),
            None,
            verbose_deprecated,
            true,
            "deprecated",
            Some(LintCategory::DEPRECATION),
        )));
        let removal_handler = Rc::new(RefCell::new(MandatoryWarningHandler::new(
            Rc::clone(&log),
            None,
            verbose_removal,
            true,
            "removal",
            Some(LintCategory::REMOVAL),
        )));
        let basic_handler: Rc<dyn CheckContext> = Rc::new(BasicCheckContext {
            log: Rc::clone(&log),
            types: Rc::clone(&types),
            infer_empty: crate::comp::infer::empty_context(),
            deferred_empty: Rc::new(DeferredAttrContext::empty()),
        });
        Check {
            names,
            log,
            diags,
            types,
            basic_handler,
            self_ref: RefCell::new(std::rc::Weak::new()),
            allow_records:
                crate::code::source::Feature::RECORDS.allowed_in_source(source),
            allow_sealed:
                crate::code::source::Feature::SEALED_CLASSES.allowed_in_source(source),
            synthetic_name_char,
            compiled: HashMap::new(),
            local_class_name_indexes: HashMap::new(),
            lint,
            deferred_lint_handler,
            private_safe_varargs: crate::code::source::Feature::PRIVATE_SAFE_VARARGS
                .allowed_in_source(source),
            syms,
            type_annotations: Rc::new(RefCell::new(None)),
            method: None,
            warn_on_any_access_to_members: false,
            enter: RefCell::new(None),
            deprecation_handler,
            removal_handler,
            preview: Rc::new(RefCell::new(None)),
            resolve: Rc::new(RefCell::new(None)),
            file_manager: Rc::new(RefCell::new(None)),
            profile: Rc::new(RefCell::new(None)),
            source,
        }
    }


    // ------------------------------------------------------------------
    // Exception checking set utilities (Check.java:1578-1685). Ported here
    // because the Flow (FlowAnalyzer) depends on them (registered Registration).
    // ------------------------------------------------------------------

    /// Java: boolean subset(Type t, List<Type> ts) (Check.java:1585-1590)
    pub fn subset(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        ts: &List<TypeId>,
    ) -> bool {
        ts.iter().any(|other| self.types.is_subtype(ctx, syms, t, other))
    }

    /// Java: boolean intersects(Type t, List<Type> ts) (Check.java:1594-1598)
    pub fn intersects(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        ts: &List<TypeId>,
    ) -> bool {
        ts.iter().any(|other| {
            self.types.is_subtype(ctx, syms, t, other)
                || self.types.is_subtype(ctx, syms, other, t)
        })
    }

    /// Java: List<Type> incl(Type t, List<Type> ts) (Check.java:1603-1605)
    pub fn incl(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        ts: &List<TypeId>,
    ) -> List<TypeId> {
        if self.subset(ctx, syms, t, ts) {
            ts.clone()
        } else {
            self.excl(ctx, syms, t, ts).prepend(t)
        }
    }

    /// Java: List<Type> excl(Type t, List<Type> ts) (Check.java:1610-1619)
    pub fn excl(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        ts: &List<TypeId>,
    ) -> List<TypeId> {
        let mut kept: List<TypeId> = List::nil();
        for other in ts.iter() {
            if !self.types.is_subtype(ctx, syms, other, t) {
                kept = kept.append(other);
            }
        }
        kept
    }

    /// Java: List<Type> union(List<Type> ts1, List<Type> ts2)
    /// (Check.java:1624-1629)
    pub fn union(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts1: &List<TypeId>,
        ts2: &List<TypeId>,
    ) -> List<TypeId> {
        let mut ts = ts1.clone();
        for other in ts2.iter() {
            ts = self.incl(ctx, syms, other, &ts);
        }
        ts
    }

    /// Java: List<Type> diff(List<Type> ts1, List<Type> ts2)
    /// (Check.java:1634-1639)
    pub fn diff(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts1: &List<TypeId>,
        ts2: &List<TypeId>,
    ) -> List<TypeId> {
        let mut ts = ts1.clone();
        for other in ts2.iter() {
            ts = self.excl(ctx, syms, other, &ts);
        }
        ts
    }

    /// Java: List<Type> intersect(List<Type> ts1, List<Type> ts2)
    /// (Check.java:1645-1652)
    pub fn intersect(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts1: &List<TypeId>,
        ts2: &List<TypeId>,
    ) -> List<TypeId> {
        let mut ts: List<TypeId> = List::nil();
        for t in ts1.iter() {
            if self.subset(ctx, syms, t, ts2) {
                ts = self.incl(ctx, syms, t, &ts);
            }
        }
        for t in ts2.iter() {
            if self.subset(ctx, syms, t, ts1) {
                ts = self.incl(ctx, syms, t, &ts);
            }
        }
        ts
    }

    /// Java: boolean isUnchecked(ClassSymbol exc) (Check.java:1657-1663)
    pub fn is_unchecked_class(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        exc: SymbolId,
    ) -> bool {
        if ctx.sym(exc).kind == crate::code::kinds::Kind::ERR {
            return true;
        }
        let error_tsym = ctx
            .typ(syms.error_type)
            .tsym
            .expect("error type symbol");
        let runtime_exception_tsym = ctx
            .typ(syms.runtime_exception_type)
            .tsym
            .expect("runtime exception type symbol");
        crate::code::symbol::symbol_is_sub_class(ctx, &self.types, syms, exc, error_tsym)
            || crate::code::symbol::symbol_is_sub_class(
                ctx,
                &self.types,
                syms,
                exc,
                runtime_exception_tsym,
            )
    }

    /// Java: boolean isUnchecked(Type exc) (Check.java:1667-1672)
    pub fn is_unchecked_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        exc: TypeId,
    ) -> bool {
        if ctx.typ(exc).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
            let supertype = self.types.supertype(ctx, syms, exc);
            self.is_unchecked_type(ctx, syms, supertype)
        } else if ctx.typ(exc).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            let tsym = ctx.typ(exc).tsym.expect("class type symbol");
            self.is_unchecked_class(ctx, syms, tsym)
        } else {
            ctx.typ(exc).has_tag(crate::code::type_tag::TypeTag::BOT)
        }
    }

    /// Java: boolean isChecked(Type exc) (Check.java:1674-1676)
    pub fn is_checked_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        exc: TypeId,
    ) -> bool {
        !self.is_unchecked_type(ctx, syms, exc)
    }

    /// Java: boolean isUnchecked(DiagnosticPosition pos, Type exc)
    /// (Check.java:1679-1686): the CompletionFailure channel is registered
    /// (Java checked completion is not represented in the ported symbol layer).
    pub fn is_unchecked_pos(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        _pos: i32,
        exc: TypeId,
    ) -> bool {
        self.is_unchecked_type(ctx, syms, exc)
    }

    /// Java: boolean isHandled(Type exc, List<Type> handled)
    /// (Check.java:1691-1693)
    pub fn is_handled(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        exc: TypeId,
        handled: &List<TypeId>,
    ) -> bool {
        self.is_unchecked_type(ctx, syms, exc) || self.subset(ctx, syms, exc, handled)
    }

    fn error_info(&self, info: ErrorInfo) -> JCDiagnosticError {
        JCDiagnosticError::from(info)
    }

    fn report_error(&self, pos: i32, info: ErrorInfo) {
        self.log
            .borrow_mut()
            .error_at(pos, self.error_info(info));
    }

    /// Java: public Type completionError(DiagnosticPosition pos,
    /// CompletionFailure ex)
    pub fn completion_error(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        failure: &crate::code::symbol::CompletionFailure,
        syms: &Symtab,
    ) -> crate::ids::TypeId {
        let sym_arg = SymbolArg::placeholder(
            failure
                .sym
                .map(|sym| ctx.sym(sym).name.to_java_string().to_string_lossy())
                .unwrap_or_default(),
        );
        let detail = failure
            .get_detail_value()
            .map(|diag| diag.get_message(None).to_string_lossy())
            .unwrap_or_default();
        self.report_error(
            pos,
            errors::cant_access(sym_arg, DiagnosticArg::from_display(detail)),
        );
        syms.err_type
    }

    /// Java: void duplicateError(DiagnosticPosition pos, Symbol sym)
    fn duplicate_error(&self, ctx: &mut CompilerContext, pos: i32, sym: SymbolId) {
        let is_erroneous = ctx
            .sym(sym)
            .ty
            .map(|ty| matches!(&ctx.typ(ty).kind, crate::code::type_::TypeKindData::ErrorType { .. }))
            .unwrap_or(false);
        if !is_erroneous {
            let location = symbol_location(ctx, sym);
            if let Some(location) = location {
                if ctx.sym(location).kind == crate::code::kinds::Kind::MTH
                    && symbol_is_static_or_instance_init(ctx, location)
                {
                    let kind = crate::code::kinds::kind_name(ctx.sym(sym), ctx);
                    let loc_kind = crate::code::kinds::kind_name(ctx.sym(location), ctx);
                    let encl = symbol_encl_class(ctx, location);
                    let encl_kind = encl.map(|encl| crate::code::kinds::kind_name(ctx.sym(encl), ctx));
                    let encl_arg = encl
                        .map(|encl| {
                            SymbolArg::placeholder(ctx.sym(encl).name.to_java_string().to_string_lossy())
                        })
                        .unwrap_or_else(|| SymbolArg::placeholder("symbol"));
                    self.report_error(
                        pos,
                        errors::already_defined_in_clinit(
                            KindNameArg::from(kind.name()),
                            SymbolArg::placeholder(ctx.sym(sym).name.to_java_string().to_string_lossy()),
                            KindNameArg::from(loc_kind.name()),
                            KindNameArg::from(
                                encl_kind.map(|kind| kind.name()).unwrap_or("kindname.class"),
                            ),
                            encl_arg,
                        ),
                    );
                    return;
                }
                // Java: if (location.kind != MTH || ((sym.owner.flags_field &
                // GENERATEDCONSTR) == 0) || ((sym.owner.flags_field & RECORD) == 0))
                let skip = ctx.sym(location).kind == crate::code::kinds::Kind::MTH
                    && ctx.sym(sym).owner.map(|owner| {
                        (ctx.sym(owner).flags_field & crate::code::flags::GENERATEDCONSTR) != 0
                            && (ctx.sym(owner).flags_field & crate::code::flags::RECORD) != 0
                    }).unwrap_or(false);
                if !skip {
                    let kind = crate::code::kinds::kind_name(ctx.sym(sym), ctx);
                    let loc_kind = crate::code::kinds::kind_name(ctx.sym(location), ctx);
                    self.report_error(
                        pos,
                        errors::already_defined(
                            KindNameArg::from(kind.name()),
                            SymbolArg::placeholder(ctx.sym(sym).name.to_java_string().to_string_lossy()),
                            KindNameArg::from(loc_kind.name()),
                            SymbolArg::placeholder(ctx.sym(location).name.to_java_string().to_string_lossy()),
                        ),
                    );
                }
                return;
            }
            let kind = crate::code::kinds::kind_name(ctx.sym(sym), ctx);
            self.report_error(
                pos,
                errors::already_defined(
                    KindNameArg::from(kind.name()),
                    SymbolArg::placeholder(ctx.sym(sym).name.to_java_string().to_string_lossy()),
                    KindNameArg::from("kindname.class"),
                    SymbolArg::placeholder("symbol"),
                ),
            );
        }
    }

    /// Java: void checkTransparentClass(DiagnosticPosition pos, ClassSymbol c,
    /// Scope s)
    pub fn check_transparent_class(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        c: SymbolId,
        scope: ScopeId,
    ) {
        let name = ctx.sym(c).name.clone();
        let symbols = ctx.scope_get_symbols_by_name(
            scope,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in symbols {
            if ctx.sym(sym).owner != ctx.sym(c).owner {
                break;
            }
            let is_type = ctx.sym(sym).kind == crate::code::kinds::Kind::TYP;
            let is_typevar = ctx
                .sym(sym)
                .ty
                .map(|ty| ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::TYPEVAR))
                .unwrap_or(false);
            let owner_kind = ctx.sym(c).owner.map(|owner| ctx.sym(owner).kind);
            let owner_is_val_mth = matches!(
                owner_kind,
                Some(crate::code::kinds::Kind::VAR) | Some(crate::code::kinds::Kind::MTH)
            );
            if is_type && !is_typevar && owner_is_val_mth && name.to_java_string() != self.names.error.to_java_string() {
                self.duplicate_error(ctx, pos, sym);
                return;
            }
        }
    }

    /// Java: boolean checkUniqueClassName(DiagnosticPosition pos, Name name,
    /// Scope s)
    pub fn check_unique_class_name(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        name: &Name,
        scope: ScopeId,
    ) -> bool {
        let symbols = ctx.scope_get_symbols_by_name(
            scope,
            name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in symbols {
            if ctx.sym(sym).kind == crate::code::kinds::Kind::TYP
                && ctx.sym(sym).name.to_java_string() != self.names.error.to_java_string()
            {
                self.duplicate_error(ctx, pos, sym);
                return false;
            }
        }
        // Java: for (Symbol sym = s.owner; sym != null; sym = sym.owner)
        let mut owner = ctx.scope(scope).owner;
        while let Some(current) = owner {
            if ctx.sym(current).kind == crate::code::kinds::Kind::TYP
                && ctx.sym(current).name.to_java_string() == name.to_java_string()
                && ctx.sym(current).name.to_java_string() != self.names.error.to_java_string()
            {
                self.duplicate_error(ctx, pos, current);
                return true;
            }
            owner = ctx.sym(current).owner;
        }
        true
    }

    /// Java: boolean checkDisjoint(DiagnosticPosition pos, long flags, long
    /// set1, long set2)
    fn check_disjoint(&self, pos: i32, flags_value: i64, set1: i64, set2: i64) -> bool {
        if (flags_value & set1) != 0 && (flags_value & set2) != 0 {
            let arg1 = self.as_flag_set_arg(crate::tree::tree_info::first_flag(flags_value & set1));
            let arg2 = self.as_flag_set_arg(crate::tree::tree_info::first_flag(flags_value & set2));
            self.report_error(pos, errors::illegal_combination_of_modifiers(arg1, arg2));
            false
        } else {
            true
        }
    }

    fn as_flag_set_arg(&self, flags_value: i64) -> Vec<FlagArg> {
        crate::code::flags::as_flag_set(flags_value)
            .iter()
            .map(|flag| FlagArg::placeholder(flag.to_string()))
            .collect()
    }

    /// Java: private long implicitEnumFinalFlag(JCTree tree)
    fn implicit_enum_final_flag(&self, ctx: &CompilerContext, tree: TreeId) -> i64 {
        let crate::tree::jc_tree::TreeKind::ClassDecl { defs, .. } = &ctx.tree(tree).kind else {
            return 0;
        };
        let mut specialized = false;
        let mut current = defs.clone();
        while current.non_empty() {
            let def = *current.head();
            if let crate::tree::jc_tree::TreeKind::VariableDecl { mods, init, .. } =
                &ctx.tree(def).kind
            {
                let enum_flag = mods
                    .map(|mods| match &ctx.tree(mods).kind {
                        crate::tree::jc_tree::TreeKind::Modifiers { flags, .. } => {
                            (flags & crate::code::flags::ENUM as i64) != 0
                        }
                        _ => false,
                    })
                    .unwrap_or(false);
                if enum_flag {
                    let has_anonymous_class = init
                        .map(|init| match &ctx.tree(init).kind {
                            crate::tree::jc_tree::TreeKind::NewClass { def, .. } => def.is_some(),
                            _ => false,
                        })
                        .unwrap_or(false);
                    if has_anonymous_class {
                        specialized = true;
                    }
                }
            }
            current = current.tail();
        }
        if specialized {
            // Java: return allowSealed ? SEALED : 0;
            if self.allow_sealed {
                crate::code::flags::SEALED
            } else {
                0
            }
        } else {
            crate::code::flags::FINAL as i64
        }
    }

    /// Java: long checkFlags(DiagnosticPosition pos, long flags, Symbol sym,
    /// JCTree tree)
    pub fn check_flags(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        flags_value: i64,
        sym: SymbolId,
        tree: TreeId,
    ) -> i64 {
        let mut mask: i64;
        let mut implicit: i64 = 0;
        let symkind = ctx.sym(sym).kind;
        let owner = ctx.sym(sym).owner;
        match symkind {
            crate::code::kinds::Kind::VAR => {
                if crate::tree::tree_info::is_receiver_param(ctx, tree) {
                    mask = crate::code::flags::ReceiverParamFlags;
                } else if owner
                    .map(|owner| ctx.sym(owner).kind != crate::code::kinds::Kind::TYP)
                    .unwrap_or(true)
                {
                    mask = crate::code::flags::LocalVarFlags;
                } else if owner
                    .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                    .unwrap_or(false)
                {
                    mask = crate::code::flags::InterfaceVarFlags as i64;
                    implicit = mask;
                } else {
                    mask = crate::code::flags::VarFlags as i64;
                }
            }
            crate::code::kinds::Kind::MTH => {
                if ctx.sym(sym).name.to_java_string() == self.names.init.to_java_string() {
                    let owner_is_enum = owner
                        .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::ENUM as i64) != 0)
                        .unwrap_or(false);
                    if owner_is_enum {
                        implicit = crate::code::flags::PRIVATE as i64;
                        mask = crate::code::flags::PRIVATE as i64;
                    } else {
                        mask = crate::code::flags::ConstructorFlags as i64;
                    }
                } else if owner
                    .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                    .unwrap_or(false)
                {
                    let owner_is_annotation = owner
                        .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::ANNOTATION as i64) != 0)
                        .unwrap_or(false);
                    if owner_is_annotation {
                        mask = crate::code::flags::AnnotationTypeElementMask;
                        implicit = (crate::code::flags::PUBLIC | crate::code::flags::ABSTRACT) as i64;
                    } else if (flags_value
                        & (crate::code::flags::DEFAULT
                            | crate::code::flags::STATIC as i64
                            | crate::code::flags::PRIVATE as i64))
                        != 0
                    {
                        mask = crate::code::flags::InterfaceMethodMask;
                        implicit = if (flags_value & crate::code::flags::PRIVATE as i64) != 0 {
                            0
                        } else {
                            crate::code::flags::PUBLIC as i64
                        };
                        if (flags_value & crate::code::flags::DEFAULT) != 0 {
                            implicit |= crate::code::flags::ABSTRACT as i64;
                        }
                    } else {
                        mask = crate::code::flags::InterfaceMethodFlags as i64;
                        implicit = mask;
                    }
                } else if owner
                    .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::RECORD) != 0)
                    .unwrap_or(false)
                {
                    mask = crate::code::flags::RecordMethodFlags as i64;
                } else {
                    mask = crate::code::flags::MethodFlags as i64;
                }
                if (flags_value & crate::code::flags::STRICTFP as i64) != 0 {
                    // Java: warnOnExplicitStrictfp(pos) (Check.java:1217-1219)
                    self.warn_on_explicit_strictfp(pos);
                }
                if ((flags_value | implicit) & crate::code::flags::ABSTRACT as i64) == 0
                    || (flags_value & crate::code::flags::DEFAULT) != 0
                {
                    implicit |= owner
                        .map(|owner| ctx.sym(owner).flags_field & crate::code::flags::STRICTFP as i64)
                        .unwrap_or(0);
                }
            }
            crate::code::kinds::Kind::TYP => {
                let owner_is_val_mth = owner
                    .map(|owner| {
                        matches!(
                            ctx.sym(owner).kind,
                            crate::code::kinds::Kind::VAR | crate::code::kinds::Kind::MTH
                        )
                    })
                    .unwrap_or(false);
                let is_local_annotation = symbol_is_directly_or_indirectly_local(ctx, sym)
                    && (flags_value & crate::code::flags::ANNOTATION as i64) != 0;
                if owner_is_val_mth || is_local_annotation {
                    let implicitly_static = !ctx.sym(sym).name.is_empty()
                        && ((flags_value & crate::code::flags::RECORD) != 0
                            || (flags_value & crate::code::flags::ENUM as i64) != 0
                            || (flags_value & crate::code::flags::INTERFACE as i64) != 0);
                    let static_or_implicitly_static =
                        (flags_value & crate::code::flags::STATIC as i64) != 0 || implicitly_static;
                    mask = if static_or_implicitly_static
                        && self.allow_records
                        && (flags_value & crate::code::flags::ANNOTATION as i64) == 0
                    {
                        crate::code::flags::StaticLocalFlags as i64
                    } else {
                        crate::code::flags::LocalClassFlags as i64
                    };
                    implicit = if implicitly_static {
                        crate::code::flags::STATIC as i64
                    } else {
                        implicit
                    };
                } else if owner
                    .map(|owner| ctx.sym(owner).kind == crate::code::kinds::Kind::TYP)
                    .unwrap_or(false)
                {
                    mask = if (flags_value & crate::code::flags::STATIC as i64) != 0
                        && self.allow_records
                        && (flags_value & crate::code::flags::ANNOTATION as i64) == 0
                    {
                        crate::code::flags::ExtendedMemberStaticClassFlags
                    } else {
                        crate::code::flags::ExtendedMemberClassFlags
                    };
                    let owner_owner_kind = owner
                        .and_then(|owner| ctx.sym(owner).owner)
                        .map(|owner_owner| ctx.sym(owner_owner).kind);
                    let owner_is_static = owner
                        .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::STATIC as i64) != 0)
                        .unwrap_or(false);
                    if owner_owner_kind == Some(crate::code::kinds::Kind::PCK) || owner_is_static {
                        mask |= crate::code::flags::STATIC as i64;
                    } else if !self.allow_records
                        && ((flags_value & crate::code::flags::ENUM as i64) != 0
                            || (flags_value & crate::code::flags::RECORD) != 0)
                    {
                        self.report_error(
                            pos,
                            errors::static_declaration_not_allowed_in_inner_classes(),
                        );
                    }
                    if (flags_value
                        & (crate::code::flags::INTERFACE as i64
                            | crate::code::flags::ENUM as i64
                            | crate::code::flags::RECORD))
                        != 0
                    {
                        implicit = crate::code::flags::STATIC as i64;
                    }
                } else {
                    mask = crate::code::flags::ExtendedClassFlags;
                }
                if (flags_value & crate::code::flags::INTERFACE as i64) != 0 {
                    implicit |= crate::code::flags::ABSTRACT as i64;
                }
                if (flags_value & crate::code::flags::ENUM as i64) != 0 {
                    mask &= !(crate::code::flags::ABSTRACT as i64
                        | crate::code::flags::FINAL as i64
                        | crate::code::flags::SEALED
                        | crate::code::flags::NON_SEALED);
                    implicit |= self.implicit_enum_final_flag(ctx, tree);
                }
                if (flags_value & crate::code::flags::RECORD) != 0 {
                    mask &= !(crate::code::flags::ABSTRACT as i64);
                    implicit |= crate::code::flags::FINAL as i64;
                }
                if (flags_value & crate::code::flags::STRICTFP as i64) != 0 {
                    // Java: warnOnExplicitStrictfp(pos) (Check.java:1261-1263)
                    self.warn_on_explicit_strictfp(pos);
                }
                implicit |= owner
                    .map(|owner| ctx.sym(owner).flags_field & crate::code::flags::STRICTFP as i64)
                    .unwrap_or(0);
            }
            _ => panic!("AssertionError: checkFlags on unexpected symbol kind"),
        }
        let illegal = flags_value & crate::code::flags::ExtendedStandardFlags & !mask;
        if illegal != 0 {
            if (illegal & crate::code::flags::INTERFACE as i64) != 0 {
                self.report_error(
                    pos,
                    if (flags_value & crate::code::flags::ANNOTATION as i64) != 0 {
                        errors::annotation_decl_not_allowed_here()
                    } else {
                        errors::intf_not_allowed_here()
                    },
                );
                mask |= crate::code::flags::INTERFACE as i64;
            } else {
                self.report_error(pos, errors::mod_not_allowed_here(self.as_flag_set_arg(illegal)));
            }
        } else if (symkind == crate::code::kinds::Kind::TYP
            || self.check_disjoint(
                pos,
                flags_value,
                crate::code::flags::ABSTRACT as i64,
                (crate::code::flags::PRIVATE | crate::code::flags::STATIC) as i64
                    | crate::code::flags::DEFAULT,
            ))
            && self.check_disjoint(
                pos,
                flags_value,
                (crate::code::flags::STATIC | crate::code::flags::PRIVATE) as i64,
                crate::code::flags::DEFAULT,
            )
            && self.check_disjoint(
                pos,
                flags_value,
                (crate::code::flags::ABSTRACT | crate::code::flags::INTERFACE) as i64,
                (crate::code::flags::FINAL
                    | crate::code::flags::NATIVE
                    | crate::code::flags::SYNCHRONIZED) as i64,
            )
            && self.check_disjoint(
                pos,
                flags_value,
                crate::code::flags::PUBLIC as i64,
                (crate::code::flags::PRIVATE | crate::code::flags::PROTECTED) as i64,
            )
            && self.check_disjoint(
                pos,
                flags_value,
                crate::code::flags::PRIVATE as i64,
                (crate::code::flags::PUBLIC | crate::code::flags::PROTECTED) as i64,
            )
            && self.check_disjoint(
                pos,
                flags_value,
                crate::code::flags::FINAL as i64,
                crate::code::flags::VOLATILE as i64,
            )
            && (symkind == crate::code::kinds::Kind::TYP
                || self.check_disjoint(
                    pos,
                    flags_value,
                    (crate::code::flags::ABSTRACT | crate::code::flags::NATIVE) as i64,
                    crate::code::flags::STRICTFP as i64,
                ))
            && self.check_disjoint(
                pos,
                flags_value,
                crate::code::flags::FINAL as i64,
                crate::code::flags::SEALED | crate::code::flags::NON_SEALED,
            )
            && self.check_disjoint(
                pos,
                flags_value,
                crate::code::flags::SEALED,
                crate::code::flags::FINAL as i64 | crate::code::flags::NON_SEALED,
            )
            && self.check_disjoint(
                pos,
                flags_value,
                crate::code::flags::SEALED,
                crate::code::flags::ANNOTATION as i64,
            )
        {
            // skip
        }
        flags_value & (mask | !crate::code::flags::ExtendedStandardFlags) | implicit
    }

    /// Java: boolean checkUnique(DiagnosticPosition pos, Symbol sym, Scope s)
    ///
    /// PORTING_RULES adapter: the MTH branches need Types.hasSameArgs /
    /// containsType, which are not ported yet; those branches are explicitly
    /// unimplemented (registered, no substitute). The TYP/VAR paths (used by
    /// Enter.visitTypeParameter) are complete.
    pub fn check_unique(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        sym: SymbolId,
        scope: ScopeId,
    ) -> bool {
        let erroneous = ctx
            .sym(sym)
            .ty
            .map(|ty| matches!(&ctx.typ(ty).kind, crate::code::type_::TypeKindData::ErrorType { .. }))
            .unwrap_or(false);
        if erroneous {
            return true;
        }
        if ctx.sym(sym).owner.map(|owner| {
            ctx.sym(owner).name.to_java_string() == self.names.any.to_java_string()
        }).unwrap_or(false) {
            return false;
        }
        let name = ctx.sym(sym).name.clone();
        let symbols = ctx.scope_get_symbols_by_name(
            scope,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for by_name in symbols {
            if sym == by_name {
                continue;
            }
            if (ctx.sym(by_name).flags_field & crate::code::flags::CLASH) != 0 {
                continue;
            }
            if ctx.sym(sym).kind != ctx.sym(by_name).kind {
                continue;
            }
            if ctx.sym(sym).name.to_java_string() == self.names.error.to_java_string() {
                continue;
            }
            let is_method = ctx.sym(sym).kind == crate::code::kinds::Kind::MTH;
            let same_kind_check = if is_method {
                let sym_ty = ctx.sym(sym).ty.expect("method type");
                let other_ty = ctx.sym(by_name).ty.expect("method type");
                let types = crate::code::types::Types::with_names(Rc::clone(&self.names));
                types.has_same_args(ctx, syms, sym_ty, other_ty)
                    || {
                        let sym_erasure = types.erasure(ctx, syms, sym_ty);
                        let other_erasure = types.erasure(ctx, syms, other_ty);
                        types.has_same_args(ctx, syms, sym_erasure, other_erasure)
                    }
            } else {
                true
            };
            if same_kind_check {
                let sym_varargs = (ctx.sym(sym).flags_field & crate::code::flags::VARARGS) != 0;
                let other_varargs =
                    (ctx.sym(by_name).flags_field & crate::code::flags::VARARGS) != 0;
                if sym_varargs != other_varargs {
                    ctx.sym_mut(sym).flags_field |= crate::code::flags::CLASH;
                    self.varargs_duplicate_error(ctx, pos, sym, by_name);
                    return true;
                } else if is_method {
                    let sym_ty = ctx.sym(sym).ty.expect("method type");
                    let other_ty = ctx.sym(by_name).ty.expect("method type");
                    let types = crate::code::types::Types::with_names(Rc::clone(&self.names));
                    if !types.has_same_args_strict(ctx, syms, sym_ty, other_ty, false) {
                        self.duplicate_erasure_error(ctx, pos, sym, by_name);
                        ctx.sym_mut(sym).flags_field |= crate::code::flags::CLASH;
                        return true;
                    }
                }
                if (ctx.sym(sym).flags_field & crate::code::flags::MATCH_BINDING) != 0
                    && (ctx.sym(by_name).flags_field & crate::code::flags::MATCH_BINDING) != 0
                    && (ctx.sym(by_name).flags_field & crate::code::flags::MATCH_BINDING_TO_OUTER) == 0
                {
                    let erroneous = ctx
                        .sym(sym)
                        .ty
                        .map(|ty| {
                            matches!(
                                &ctx.typ(ty).kind,
                                crate::code::type_::TypeKindData::ErrorType { .. }
                            )
                        })
                        .unwrap_or(false);
                    if !erroneous {
                        self.report_error(pos, errors::match_binding_exists());
                        ctx.sym_mut(sym).flags_field |= crate::code::flags::CLASH;
                    }
                    return false;
                }
                self.duplicate_error(ctx, pos, by_name);
                return false;
            }
        }
        true
    }

    /// Java: public Name localClassName(ClassSymbol c)
    pub fn local_class_name(&mut self, ctx: &mut CompilerContext, c: SymbolId) -> Name {
        let owner = ctx.sym(c).owner.expect("local class owner");
        let encl_class = symbol_encl_class(ctx, owner).expect("enclosing class");
        let encl_flatname = match &ctx.sym(encl_class).kind_data {
            SymbolKind::ClassSymbol { flatname, .. } => flatname.to_java_string().to_string(),
            _ => panic!("class symbol expected"),
        };
        let name = ctx.sym(c).name.clone();
        let key = (ctx.sym(encl_class).name.clone(), name.clone());
        let mut index = *self.local_class_name_indexes.get(&key).unwrap_or(&1);
        loop {
            let flatname = self.names.from_string(&javac_strings::JavaString::from(
                format!("{}{}{}{}", encl_flatname, self.synthetic_name_char, index, name.to_java_string()).as_str(),
            ));
            let module = symbol_packge(ctx, c)
                .and_then(|package| match &ctx.sym(package).kind_data {
                    SymbolKind::PackageSymbol { modle, .. } => *modle,
                    _ => None,
                });
            let compiled = module
                .map(|module| self.get_compiled_by(module, &flatname).is_some())
                .unwrap_or(false);
            if !compiled {
                self.local_class_name_indexes.insert(key, index + 1);
                return flatname;
            }
            index += 1;
        }
    }

    /// Java: public void clearLocalClassNameIndexes(ClassSymbol c)
    pub fn clear_local_class_name_indexes(&mut self, ctx: &CompilerContext, c: SymbolId) {
        if let Some(owner) = ctx.sym(c).owner
            && let Some(encl_class) = symbol_encl_class(ctx, owner)
                && let SymbolKind::ClassSymbol { flatname, .. } = &ctx.sym(encl_class).kind_data {
                    let key = (flatname.clone(), ctx.sym(c).name.clone());
                    self.local_class_name_indexes.remove(&key);
                }
    }

    /// Java: public void putCompiled(ClassSymbol csym)
    pub fn put_compiled(&mut self, ctx: &CompilerContext, c: SymbolId) {
        let key = self.compiled_key(ctx, c);
        self.compiled.insert(key, c);
    }

    /// Java: public ClassSymbol getCompiled(ClassSymbol csym)
    pub fn get_compiled(&self, ctx: &CompilerContext, c: SymbolId) -> Option<SymbolId> {
        self.compiled.get(&self.compiled_key(ctx, c)).copied()
    }

    /// Java: public ClassSymbol getCompiled(ModuleSymbol msym, Name flatname)
    pub fn get_compiled_by(&self, module: SymbolId, flatname: &Name) -> Option<SymbolId> {
        self.compiled.get(&(module, flatname.clone())).copied()
    }

    /// Java: public void removeCompiled(ClassSymbol csym)
    pub fn remove_compiled(&mut self, ctx: &CompilerContext, c: SymbolId) {
        self.compiled.remove(&self.compiled_key(ctx, c));
    }

    fn compiled_key(&self, ctx: &CompilerContext, c: SymbolId) -> (SymbolId, Name) {
        let module = symbol_packge(ctx, c)
            .and_then(|package| match &ctx.sym(package).kind_data {
                SymbolKind::PackageSymbol { modle, .. } => *modle,
                _ => None,
            })
            .expect("class module");
        let flatname = match &ctx.sym(c).kind_data {
            SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
            _ => ctx.sym(c).name.clone(),
        };
        (module, flatname)
    }

    /// Java: public void newRound()
    pub fn new_round(&mut self) {
        self.compiled.clear();
        self.local_class_name_indexes.clear();
    }

    /// Java: void checkTransparentVar(DiagnosticPosition pos, VarSymbol v,
    /// Scope s)
    pub fn check_transparent_var(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        v: SymbolId,
        scope: ScopeId,
    ) {
        let name = ctx.sym(v).name.clone();
        let symbols = ctx.scope_get_symbols_by_name(
            scope,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in symbols {
            if ctx.sym(sym).owner != ctx.sym(v).owner {
                break;
            }
            let owner_kind = ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind);
            let owner_is_val_mth = matches!(
                owner_kind,
                Some(crate::code::kinds::Kind::VAR) | Some(crate::code::kinds::Kind::MTH)
            );
            if ctx.sym(sym).kind == crate::code::kinds::Kind::VAR
                && owner_is_val_mth
                && name.to_java_string() != self.names.error.to_java_string()
            {
                self.duplicate_error(ctx, pos, sym);
                return;
            }
        }
    }

    /// Java: Type typeTagError(DiagnosticPosition pos, JCDiagnostic required,
    /// Object found)
    ///
    /// Java: Type typeTagError(DiagnosticPosition pos, JCDiagnostic required,
    /// Object found) (Check.java:1007-1016)
    ///
    /// PORTING_RULES Porting rules adapter: the Java Object found (a Type or a
    /// JCDiagnostic fragment) maps to the FoundArg enum.
    fn type_tag_error(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        required: JCDiagnostic,
        found: FoundArg,
    ) -> crate::ids::TypeId {
        let required_arg =
            DiagnosticArg::from_display(required.get_message(None).to_string_lossy());
        let found_arg = match &found {
            FoundArg::Type(t) => match ctx.typ(*t).tsym {
                Some(tsym) => DiagnosticArg::from_display(
                    crate::comp::resolve::symbol_display_name(ctx, tsym),
                ),
                None => DiagnosticArg::from_display("type"),
            },
            FoundArg::Fragment(arg) => arg.clone(),
        };
        self.report_error(pos, errors::type_found_req(found_arg, required_arg));
        match found {
            FoundArg::Type(t) => self.types.create_error_type_original(ctx, Some(t), syms),
            FoundArg::Fragment(_) => syms.err_type,
        }
    }

    /// Java: Type checkClassType(DiagnosticPosition pos, Type t)
    pub fn check_class_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: crate::ids::TypeId,
    ) -> crate::ids::TypeId {
        let tag = ctx.typ(t).get_tag();
        if tag != crate::code::type_tag::TypeTag::CLASS
            && tag != crate::code::type_tag::TypeTag::ERROR
        {
            let required = self
                .diags
                .fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::type_req_class(),
                ));
            let found = self.as_type_param(ctx, t);
            return self.type_tag_error(ctx, syms, pos, required, found);
        }
        t
    }

    /// Java: Type checkClassType(DiagnosticPosition pos, Type t, boolean noBounds)
    pub fn check_class_type_no_bounds(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: crate::ids::TypeId,
        no_bounds: bool,
    ) -> crate::ids::TypeId {
        let t = self.check_class_type(ctx, syms, pos, t);
        if no_bounds {
            {
                let mut args = ctx.typ(t).get_type_arguments();
                while args.non_empty() {
                    let head = *args.head();
                    if ctx
                        .typ(head)
                        .has_tag(crate::code::type_tag::TypeTag::WILDCARD)
                    {
                        let required = self
                            .diags
                            .fragment(crate::util::jc_diagnostic::Fragment::from(
                                crate::resources::compiler_properties::fragments::type_req_exact(),
                            ));
                        return self.type_tag_error(ctx, syms, pos, required, FoundArg::Type(t));
                    }
                    args = args.tail();
                }
            }
        }
        t
    }
    /// Java: public Type checkType(DiagnosticPosition pos, Type found, Type req)
    pub fn check_type_basic(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> TypeId {
        self.check_type(ctx, syms, pos, found, req, Rc::clone(&self.basic_handler))
    }

    /// Java: Type checkType(DiagnosticPosition pos, Type found, Type req,
    /// CheckContext checkContext)
    pub fn check_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        pos: i32,
        found: TypeId,
        req: TypeId,
        check_context: Rc<dyn CheckContext>,
    ) -> TypeId {
        let inference_context = check_context.inference_context();
        if inference_context.free(ctx, syms, &self.types, req)
            || inference_context.free(ctx, syms, &self.types, found)
        {
            // Java: inferenceContext.addFreeTypeListener(List.of(req, found),
            // solvedContext -> checkType(pos, solvedContext.asInstType(found),
            // solvedContext.asInstType(req), checkContext)) (Check.java:604-609).
            let listener = Rc::new(CheckFreeTypeListener {
                check: self.self_ref.borrow().clone(),
                pos,
                found,
                req,
                check_context: Rc::clone(&check_context),
            });
            inference_context.add_free_type_listener(
                ctx,
                syms,
                &self.types,
                List::of(req).append(found),
                listener as Rc<dyn crate::comp::infer::FreeTypeListener>,
            );
        }
        if ctx.typ(req).has_tag(crate::code::type_tag::TypeTag::ERROR) {
            return req;
        }
        if ctx.typ(req).has_tag(crate::code::type_tag::TypeTag::NONE) {
            return found;
        }
        let mut warn = check_context.check_warner(ctx, syms, &self.types, pos, found, req);
        if check_context.compatible(ctx, syms, &self.types, found, req, &mut warn) {
            if warn.has_lint(LintCategory::VARARGS) {
                // Java: ConversionWarner.warn VARARGS branch (Check.java:3961-3967).
                self.warn_unsafe_varargs_param(ctx, syms, pos);
            }
            return found;
        }
        if ctx.typ(found).is_numeric() && ctx.typ(req).is_numeric() {
            // Java: checkContext.report(pos,
            // diags.fragment(Fragments.PossibleLossOfPrecision(found, req)));
            // return types.createErrorType(found);
            let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::possible_loss_of_precision(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        self.type_display(ctx, found),
                    ),
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        self.type_display(ctx, req),
                    ),
                ),
            ));
            check_context.report(ctx, pos, &diag);
            return self
                .types
                .create_error_type_original(ctx, Some(found), syms);
        }
        // Java: checkContext.report(pos, diags.fragment(
        // Fragments.InconvertibleTypes(found, req)));
        // return types.createErrorType(found);
        let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::inconvertible_types(
                crate::resources::diagnostic_contract::TypeArg::placeholder(
                    self.type_display(ctx, found),
                ),
                crate::resources::diagnostic_contract::TypeArg::placeholder(
                    self.type_display(ctx, req),
                ),
            ),
        ));
        check_context.report(ctx, pos, &diag);
        self.types
            .create_error_type_original(ctx, Some(found), syms)
    }

    /// Java: Type checkRefType(DiagnosticPosition pos, Type t)
    pub fn check_ref_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).is_reference() {
            return t;
        }
        let required = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::type_req_ref(),
        ));
        self.type_tag_error(ctx, syms, pos, required, FoundArg::Type(t))
    }

}


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
        warn: &mut Warner,
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
        warn: &mut Warner,
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
    pub log: Rc<RefCell<Log>>,
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
        warn: &mut Warner,
    ) -> bool {
        // Java: return types.isAssignable(found, req, warn);
        self.types
            .is_assignable_with_warn(ctx, syms, found, req, warn)
    }

    fn report(&self, _ctx: &mut CompilerContext, pos: i32, details: &JCDiagnostic) {
        // Java: log.error(pos, Errors.ProbFoundReq(details));
        // ADR 0002 P4: the JCDiagnostic argument is mapped to its message text
        // (DiagnosticArg has no JCDiagnostic variant); registered adapter.
        let message = details.get_message(None);
        self.log.borrow_mut().error_at(
            pos,
            crate::util::jc_diagnostic::Error::from(
                crate::resources::compiler_properties::errors::prob_found_req(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        message.to_string(),
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

/// Java: the Object found parameter of Check.typeTagError (a Type or a
/// JCDiagnostic fragment, Check.java:1007).
///
/// PORTING_RULES Porting rules adapter: Rust needs the union represented explicitly.
pub enum FoundArg {
    /// Java: Object found instanceof Type
    Type(TypeId),
    /// Java: Object found instanceof JCDiagnostic (a fragment)
    Fragment(crate::resources::diagnostic_contract::DiagnosticArg),
}

impl Check {
    /// Java: Object asTypeParam(Type t) (Check.java:723-727)
    fn as_type_param(&self, ctx: &mut CompilerContext, t: TypeId) -> FoundArg {
        if ctx
            .typ(t)
            .has_tag(crate::code::type_tag::TypeTag::TYPEVAR)
        {
            // Java: diags.fragment(Fragments.TypeParameter(t)) — the P4
            // argument mapping keeps the diagnostic text placeholder.
            FoundArg::Fragment(DiagnosticArg::from_display("type parameter"))
        } else {
            FoundArg::Type(t)
        }
    }

    /// Java: Type checkNonVoid(DiagnosticPosition pos, Type t)
    /// (Check.java:700-708)
    pub fn check_non_void(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::VOID) {
            self.report_error(pos, errors::void_not_allowed_here());
            self.types.create_error_type_original(ctx, Some(t), syms)
        } else {
            t
        }
    }

    /// Java: Type checkClassOrArrayType(DiagnosticPosition pos, Type t)
    /// (Check.java:709-717)
    pub fn check_class_or_array_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: TypeId,
    ) -> TypeId {
        let tag = ctx.typ(t).get_tag();
        if tag != crate::code::type_tag::TypeTag::CLASS
            && tag != crate::code::type_tag::TypeTag::ARRAY
            && tag != crate::code::type_tag::TypeTag::ERROR
        {
            let required = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                fragments::type_req_class_array(),
            ));
            let found = self.as_type_param(ctx, t);
            return self.type_tag_error(ctx, syms, pos, required, found);
        }
        t
    }

    /// Java: Type checkNullOrRefType(DiagnosticPosition pos, Type t)
    /// (Check.java:814-822)
    pub fn check_null_or_ref_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).is_reference()
            || ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::BOT)
        {
            t
        } else {
            let required = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                fragments::type_req_ref(),
            ));
            self.type_tag_error(ctx, syms, pos, required, FoundArg::Type(t))
        }
    }

    /// Java: void checkEmptyIf(JCIf tree) (Check.java:3733-3737)
    pub fn check_empty_if(&self, ctx: &mut CompilerContext, tree: TreeId) {
        let (thenpart, elsepart) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::If {
                thenpart,
                elsepart,
                ..
            } => (*thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let Some(thenpart) = thenpart else {
            return;
        };
        if ctx.tree(thenpart).has_tag(crate::tree::jc_tree::Tag::SKIP)
            && elsepart.is_none()
            && self.lint.is_enabled(LintCategory::EMPTY)
        {
            self.log.borrow_mut().warning(
                Some(LintCategory::EMPTY),
                Some(Rc::new(SimpleDiagnosticPosition::new(ctx.tree(thenpart).pos))),
                warning_from_info(warnings::empty_if()),
            );
        }
    }

    /// Java: void checkDivZero(final DiagnosticPosition pos, Symbol operator,
    /// Type operand) (Check.java:3718-3731)
    pub fn check_div_zero(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        operator: SymbolId,
        operand: TypeId,
    ) {
        let zero_value = match ctx.typ(operand).const_value() {
            Some(ConstantValue::Integer(value)) => *value == 0,
            Some(ConstantValue::Byte(value)) => *value == 0,
            Some(ConstantValue::Short(value)) => *value == 0,
            Some(ConstantValue::Long(value)) => *value == 0,
            Some(ConstantValue::Character(value)) => *value == 0,
            _ => false,
        };
        if zero_value
            && ctx
                .typ(operand)
                .get_tag()
                .is_sub_range_of(crate::code::type_tag::TypeTag::LONG)
        {
            let opcode = match &ctx.sym(operator).kind_data {
                SymbolKind::OperatorSymbol { opcode, .. } => *opcode,
                _ => return,
            };
            if opcode == crate::jvm::byte_codes::IDIV
                || opcode == crate::jvm::byte_codes::LDIV
                || opcode == crate::jvm::byte_codes::IMOD
                || opcode == crate::jvm::byte_codes::LMOD
            {
                let log = Rc::clone(&self.log);
                let lint = Rc::clone(&self.lint);
                self.deferred_lint_handler
                    .borrow_mut()
                    .report(Box::new(move || warn_div_zero(&log, &lint, pos)));
            }
        }
    }

    /// Java: void warnDivZero(DiagnosticPosition pos) (Check.java:288-291)
    ///
    /// Java: void duplicateErasureError(DiagnosticPosition pos, Symbol sym1,
    /// Symbol sym2) (Check.java:3783-3787)
    pub fn duplicate_erasure_error(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        sym1: SymbolId,
        sym2: SymbolId,
    ) {
        let erroneous = |ctx: &CompilerContext, sym: SymbolId| {
            ctx.sym(sym)
                .ty
                .map(|ty| {
                    matches!(
                        &ctx.typ(ty).kind,
                        crate::code::type_::TypeKindData::ErrorType { .. }
                    )
                })
                .unwrap_or(false)
        };
        if !erroneous(ctx, sym1) && !erroneous(ctx, sym2) {
            self.report_error(
                pos,
                errors::name_clash_same_erasure(symbol_place_check(ctx, sym1), symbol_place_check(ctx, sym2)),
            );
        }
    }

    /// Java: void varargsDuplicateError(DiagnosticPosition pos, Symbol sym1,
    /// Symbol sym2) (Check.java:371-375)
    pub fn varargs_duplicate_error(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        sym1: SymbolId,
        sym2: SymbolId,
    ) {
        let erroneous = |ctx: &CompilerContext, sym: SymbolId| {
            ctx.sym(sym)
                .ty
                .map(|ty| {
                    matches!(
                        &ctx.typ(ty).kind,
                        crate::code::type_::TypeKindData::ErrorType { .. }
                    )
                })
                .unwrap_or(false)
        };
        if !erroneous(ctx, sym1) && !erroneous(ctx, sym2) {
            let location = symbol_location(ctx, sym2).unwrap_or(sym2);
            self.report_error(
                pos,
                errors::array_and_varargs(
                    symbol_place_check(ctx, sym1),
                    symbol_place_check(ctx, sym2),
                    symbol_place_check(ctx, location),
                ),
            );
        }
    }

    /// Java: public void checkRedundantCast(Env<AttrContext> env,
    /// final JCTypeCast tree) (Check.java:647-658).
    pub fn check_redundant_cast(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        _env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
    ) {
        const IGNORE_ANNOTATED_CASTS: bool = true;
        let (clazz, expr) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("ClassCastException: JCTypeCast expected"),
        };
        let clazz = clazz.expect("cast type");
        let expr = expr.expect("cast expression");
        let tree_type = ctx.tree(tree).ty.expect("cast type");
        if self.types.is_erroneous_type(ctx, syms, tree_type) {
            return;
        }
        let expr_type = ctx.tree(expr).ty.expect("expression type");
        let clazz_type = ctx.tree(clazz).ty.expect("cast class type");
        if !self
            .types
            .is_same_type(ctx, syms, expr_type, clazz_type)
        {
            return;
        }
        if IGNORE_ANNOTATED_CASTS
            && crate::tree::tree_info::contains_type_annotation(ctx, clazz)
        {
            return;
        }
        if is292_target_type_cast(ctx, tree) {
            return;
        }
        // Java: deferredLintHandler.report(() -> { if (lint.isEnabled(CAST))
        //     log.warning(CAST, tree.pos(), Warnings.RedundantCast(tree.clazz.type)); });
        let lint = Rc::clone(&self.lint);
        let log = Rc::clone(&self.log);
        let pos = ctx.tree(tree).pos;
        let redundant_cast = crate::resources::compiler_properties::warnings::redundant_cast(
            crate::resources::diagnostic_contract::TypeArg::placeholder(type_display_str(
                ctx, clazz_type,
            )),
        );
        self.deferred_lint_handler
            .borrow_mut()
            .report(Box::new(move || {
                if lint.is_enabled(crate::code::lint::LintCategory::CAST) {
                    use crate::util::abstract_log::AbstractLog;
                    let position: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>> =
                        Some(Rc::new(
                            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                        ));
                    let warning =
                        crate::util::jc_diagnostic::Warning::from(redundant_cast.clone());
                    log.borrow_mut().warning_lint_pos(
                        crate::code::lint::LintCategory::CAST,
                        position,
                        warning,
                    );
                }
            }));
    }

    /// Java: void validate(List<? extends JCTree> trees, Env<AttrContext> env)
    /// (Check.java:1406-1410)
    pub fn validate_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        trees: &List<TreeId>,
    ) {
        for tree in trees.iter() {
            self.validate(ctx, syms, env, tree);
        }
    }
}

/// Java: void warnDivZero(DiagnosticPosition pos) (Check.java:288-291)
fn warn_div_zero(log: &Rc<RefCell<Log>>, lint: &Rc<Lint>, pos: i32) {
    if lint.is_enabled(LintCategory::DIVZERO) {
        log.borrow_mut().warning(
            Some(LintCategory::DIVZERO),
            Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
            warning_from_info(warnings::div_zero()),
        );
    }
}

/// Build the JCDiagnostic.Warning logging key from the generated WarningInfo
/// (ADR 0002 P4 adapter: the generated accessors carry the diagnostic info;
/// the argument mapping renders the known value types as display text).
pub(crate) fn warning_from_info(
    info: crate::resources::diagnostic_contract::WarningInfo,
) -> Warning {
    let args: Vec<crate::util::jc_diagnostic::DiagnosticArg> = info
        .info()
        .args
        .iter()
        .map(|arg| {
            Rc::new(javac_strings::JavaString::from(arg.to_text().as_str()))
                as crate::util::jc_diagnostic::DiagnosticArg
        })
        .collect();
    Warning(crate::util::jc_diagnostic::DiagnosticInfo::of(
        crate::util::jc_diagnostic::DiagnosticType::WARNING,
        "compiler",
        info.info().code,
        args,
    ))
}

/// Diagnostic SymbolArg placeholder (ADR 0002 P4).
fn symbol_place_check(
    ctx: &CompilerContext,
    sym: SymbolId,
) -> crate::resources::diagnostic_contract::SymbolArg {
    crate::resources::diagnostic_contract::SymbolArg::placeholder(
        crate::comp::resolve::symbol_display_name(ctx, sym),
    )
}

// ---------------------------------------------------------------------------
// Java: Check annotation validation (Check.java:211/2857-3520). 
// ports the Annotate-facing part; the branches that need not-yet-ported units
// (RecordComponent, TypeAnnotations.annotationTargets, Types.cvarLowerBound,
// types.getRetention/overrideEquivalent) are explicit TODO registrations.
// ---------------------------------------------------------------------------

impl Check {
    /// Java: Lint setLint(Lint newLint) (Check.java:211-215)
    pub fn set_lint(&mut self, new_lint: Rc<Lint>) -> Rc<Lint> {
        let prev = Rc::clone(&self.lint);
        self.lint = new_lint;
        prev
    }

    /// Java: void validateAnnotationTree(JCTree tree) (Check.java:2857-2873)
    pub fn validate_annotation_tree(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, tree: TreeId) {
        struct AnnotationValidator<'a> {
            chk: &'a Check,
            syms: &'a crate::code::symtab::Symtab,
        }
        // Java: class AnnotationValidator extends TreeScanner { visitAnnotation }
        tree_scanner_visitor_delegate!(AnnotationValidator<'a>, 'a);
        impl<'a> TreeScannerVisitor for AnnotationValidator<'a> {
            fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                let annotation_type_ty = match &ctx.tree(tree).kind {
                    TreeKind::Annotation { annotation_type, .. } => *annotation_type,
                    _ => panic!("expected an annotation"),
                };
                let mut is_erroneous = false;
                let mut is_annotation_type = false;
                if let Some(ty) = annotation_type_ty.and_then(|annotation_type| ctx.tree(annotation_type).ty) {
                    is_erroneous = ctx.typ(ty).has_tag(TypeTag::ERROR);
                    is_annotation_type = ctx
                        .typ(ty)
                        .tsym
                        .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::ANNOTATION as i64) != 0)
                        .unwrap_or(false);
                }
                if !is_erroneous && is_annotation_type {
                    // Java: super.visitAnnotation(a, p) (TreeScanner visits the
                    // annotation type tree and the argument list).
                    let (annotation_type, args) = match &ctx.tree(tree).kind {
                        TreeKind::Annotation {
                            annotation_type,
                            args,
                            ..
                        } => (*annotation_type, args.clone()),
                        _ => panic!("expected an annotation"),
                    };
                    self.scan(ctx, annotation_type);
                    self.scan_list(ctx, &args);
                    let _ = self.chk.validate_annotation_values(ctx, self.syms, tree);
                }
            }
        }
        let mut validator = AnnotationValidator { chk: self, syms };
        validator.scan(ctx, Some(tree));
    }
    /// Java: public void validateAnnotations(List<JCAnnotation> annotations,
    /// JCTree declarationTree, Symbol s) (Check.java:2919-2923)
    pub fn validate_annotations(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        annotations: &List<TreeId>,
        declaration_tree: Option<TreeId>,
        s: SymbolId,
    ) -> Result<(), CompletionFailure> {
        for a in annotations.iter() {
            self.validate_annotation(ctx, syms, a, declaration_tree, s)?;
        }
        Ok(())
    }

    /// Java: private void validateAnnotation(JCAnnotation a, JCTree
    /// declarationTree, Symbol s) (Check.java:2933-3073)
    fn validate_annotation(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
        declaration_tree: Option<TreeId>,
        s: SymbolId,
    ) -> Result<(), CompletionFailure> {
        self.validate_annotation_tree(ctx, syms, a);
        let is_record_member = (ctx.sym(s).flags_field & crate::code::flags::RECORD) != 0
            || ctx
                .sym(s)
                .owner
                .and_then(|owner| crate::code::symbol::symbol_encl_class(ctx, owner))
                .map(|encl| ctx.sym(encl).is_record())
                .unwrap_or(false);
        let is_record_field = (ctx.sym(s).flags_field & crate::code::flags::RECORD) != 0
            && declaration_tree
                .map(|declaration_tree| ctx.tree(declaration_tree).has_tag(Tag::VARDEF))
                .unwrap_or(false)
            && ctx.sym(s).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::TYP);
        if is_record_field {
            // Java: Check.java:2944-2986.
            use crate::util::abstract_log::AbstractLog;
            let annotation_type_tree = match &ctx.tree(a).kind {
                TreeKind::Annotation { annotation_type, .. } => {
                    annotation_type.expect("annotation type")
                }
                _ => panic!("expected an annotation"),
            };
            let annotation_type = ctx.tree(annotation_type_tree).ty.expect("annotation type");
            let annotation_tsym = ctx.typ(annotation_type).tsym.expect("annotation tsym");
            let targets = self.get_target_names(ctx, syms, annotation_tsym)?;
            let applies_to_records = targets.iter().any(|target| {
                *target == self.names.FIELD
                    || *target == self.names.PARAMETER
                    || *target == self.names.METHOD
                    || *target == self.names.TYPE_USE
                    || *target == self.names.RECORD_COMPONENT
            });
            if !applies_to_records {
                // Java: log.error(a.pos(), Errors.AnnotationTypeNotApplicable);
                self.log.borrow_mut().error_at(
                    ctx.tree(a).pos,
                    crate::util::jc_diagnostic::Error::from(
                        crate::resources::compiler_properties::errors::annotation_type_not_applicable(),
                    ),
                );
            } else {
                // Java: find the annotations targeted to record components and
                // append them to the corresponding record component.
                let record_class = ctx.sym(s).owner.expect("record class");
                let record_component = ctx
                    .sym(record_class)
                    .get_record_component(ctx, s)
                    .expect("record component");
                let metadata_empty = match &ctx.sym(record_component).metadata {
                    Some(metadata) => metadata.is_empty(),
                    None => true,
                };
                if metadata_empty {
                    let mut filtered: crate::util::list::List<crate::ids::AttributeId> =
                        crate::util::list::List::nil();
                    let mut current = ctx.sym(s).get_raw_attributes();
                    while current.non_empty() {
                        let annotation = *current.head();
                        current = current.tail();
                        let anno_ty = ctx.attr(annotation).ty.expect("annotation type");
                        let anno_tsym = ctx.typ(anno_ty).tsym.expect("annotation tsym");
                        let anno_targets = self.get_target_names(ctx, syms, anno_tsym)?;
                        if anno_targets
                            .iter()
                            .any(|name| *name == self.names.RECORD_COMPONENT)
                        {
                            filtered = filtered.append_list(&crate::util::list::List::of(annotation));
                        }
                    }
                    ctx.sym_mut(record_component).append_attributes(filtered);
                }
            }
        }
        let _ = is_record_member;
        let annotation_type_ty = match &ctx.tree(a).kind {
            TreeKind::Annotation { annotation_type, .. } => *annotation_type,
            _ => panic!("expected an annotation"),
        };
        if annotation_type_ty
            .and_then(|annotation_type| ctx.tree(annotation_type).ty)
            .map(|ty| ctx.typ(ty).has_tag(TypeTag::ERROR))
            .unwrap_or(false)
        {
            return Ok(());
        }
        let annotation_ty = match &ctx.tree(a).kind {
            TreeKind::Annotation { .. } => ctx.tree(a).ty,
            _ => panic!("expected an annotation"),
        };
        let annotation_ty = annotation_ty.expect("annotation type");
        let annotation_tsym = ctx.typ(annotation_ty).tsym.expect("annotation tsym");
        let is_annotation_type = (ctx.sym(annotation_tsym).flags_field
            & crate::code::flags::ANNOTATION as i64)
            != 0;
        if is_annotation_type {
            let applicable_targets = self.get_applicable_targets(ctx, syms, a, s)?;
            let Some(applicable_targets) = applicable_targets else {
                return Ok(());
            };
            let not_applicable_or_is_type_use_only = applicable_targets.is_empty()
                || (applicable_targets.len() == 1
                    && applicable_targets
                        .iter()
                        .any(|target| target.to_java_string() == self.names.TYPE_USE.to_java_string()));
            if applicable_targets.is_empty() {
                self.report_error(
                    ctx.tree(a).pos,
                    errors::annotation_type_not_applicable(),
                );
            }
            let _ = not_applicable_or_is_type_use_only;
            // Java: if we are seeing the @SafeVarargs annotation applied to a
            // compiler generated accessor, this is an error (Check.java:3068-3074).
            let is_comp_generated_record_element = is_record_member
                && (ctx.sym(s).flags_field & crate::code::flags::GENERATED_MEMBER as i64) != 0;
            if is_comp_generated_record_element && !is_record_field {
                let trust_me = ctx.typ(syms.trust_me_type).tsym;
                if Some(annotation_tsym) == trust_me
                    && declaration_tree
                        .map(|declaration_tree| ctx.tree(declaration_tree).has_tag(Tag::METHODDEF))
                        .unwrap_or(false)
                {
                    let fragment = fragments::varargs_trustme_on_non_varargs_accessor(
                        SymbolArg::placeholder(symbol_name(ctx, s)),
                    );
                    let info = errors::varargs_invalid_trustme_anno(
                        SymbolArg::placeholder(symbol_name(
                            ctx,
                            trust_me.expect("trust me type symbol"),
                        )),
                        DiagnosticArg::from_display(fragment.info().key()),
                    );
                    self.report_error(ctx.tree(a).pos, info);
                }
            }
            // Java: the compiler-generated record element trimming branch
            // (isCompGeneratedRecordElement) needs the Record; registered.
        }
        // functional interface check
        let functional_interface_tsym = ctx
            .typ(syms.functional_interface_type)
            .tsym;
        let annotation_tree_tsym = match &ctx.tree(a).kind {
            TreeKind::Annotation { annotation_type, .. } => *annotation_type,
            _ => panic!("expected an annotation"),
        };
        let annotation_tree_ty = annotation_tree_tsym;
        if annotation_tree_ty.and_then(|t| ctx.tree(t).ty).and_then(|ty| ctx.typ(ty).tsym)
            == functional_interface_tsym
        {
            if ctx.sym(s).kind != Kind::TYP {
                self.report_error(ctx.tree(a).pos, errors::bad_functional_intf_anno());
            } else if !ctx.sym(s).is_interface()
                || (ctx.sym(s).flags_field & crate::code::flags::ANNOTATION as i64) != 0
            {
                self.report_error(
                    ctx.tree(a).pos,
                    errors::bad_functional_intf_anno1(
                        crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                            "not a functional interface",
                        ),
                    ),
                );
            }
        }
        Ok(())
    }
}

impl Check {
    /// Java: boolean annotationApplicable(JCAnnotation a, Symbol s)
    /// (Check.java:3337-3344)
    pub fn annotation_applicable(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
        s: SymbolId,
    ) -> Result<bool, CompletionFailure> {
        let targets = self.get_applicable_targets(ctx, syms, a, s)?;
        Ok(match targets {
            None => true,
            Some(targets) => !targets.is_empty(),
        })
    }

    /// Java: Optional<Set<Name>> getApplicableTargets(JCAnnotation a,
    /// Symbol s) (Check.java:3346-3421)
    fn get_applicable_targets(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
        s: SymbolId,
    ) -> Result<Option<Vec<Name>>, CompletionFailure> {
        let annotation_tsym = match &ctx.tree(a).kind {
            TreeKind::Annotation { annotation_type, .. } => *annotation_type,
            _ => panic!("expected an annotation"),
        };
        let annotation_ty = annotation_tsym;
        let annotation_ty = annotation_ty.and_then(|t| ctx.tree(t).ty).expect("annotation type");
        let anno_sym = ctx.typ(annotation_ty).tsym.expect("annotation tsym");
        let arr = self.get_attribute_target_attribute(ctx, syms, anno_sym)?;
        let targets: Vec<Name> = match arr {
            None => self.default_target_meta_info(),
            Some(arr) => {
                let values = match &ctx.attr(arr).kind {
                    crate::code::attribute::AttributeKind::Array { values } => values.clone(),
                    _ => Vec::new(),
                };
                let mut targets: Vec<Name> = Vec::new();
                for value in values {
                    let enum_value = match &ctx.attr(value).kind {
                        crate::code::attribute::AttributeKind::Enum { value } => *value,
                        _ => return Ok(None), // recovery
                    };
                    let Some(enum_value) = enum_value else {
                        return Ok(None);
                    };
                    targets.push(ctx.sym(enum_value).name.clone());
                }
                targets
            }
        };
        let mut applicable: Vec<Name> = Vec::new();
        for target in targets {
            if target.to_java_string() == self.names.TYPE.to_java_string() {
                if ctx.sym(s).kind == Kind::TYP {
                    applicable.push(self.names.TYPE.clone());
                }
            } else if target.to_java_string() == self.names.FIELD.to_java_string() {
                if ctx.sym(s).kind == Kind::VAR
                    && ctx.sym(s).owner.map(|owner| ctx.sym(owner).kind) != Some(Kind::MTH)
                {
                    applicable.push(self.names.FIELD.clone());
                }
            } else if target.to_java_string() == self.names.RECORD_COMPONENT.to_java_string() {
                if ctx.sym(s).get_kind(ctx)
                    == javac_tools::lang::model::element::element_kind::ElementKind::RECORD_COMPONENT
                {
                    applicable.push(self.names.RECORD_COMPONENT.clone());
                }
            } else if target.to_java_string() == self.names.METHOD.to_java_string() {
                if ctx.sym(s).kind == Kind::MTH && !self.symbol_is_constructor(ctx, s) {
                    applicable.push(self.names.METHOD.clone());
                }
            } else if target.to_java_string() == self.names.PARAMETER.to_java_string() {
                if ctx.sym(s).kind == Kind::VAR
                    && (ctx.sym(s).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH))
                    && (ctx.sym(s).flags_field & crate::code::flags::PARAMETER) != 0
                {
                    applicable.push(self.names.PARAMETER.clone());
                }
            } else if target.to_java_string() == self.names.CONSTRUCTOR.to_java_string() {
                if ctx.sym(s).kind == Kind::MTH && self.symbol_is_constructor(ctx, s) {
                    applicable.push(self.names.CONSTRUCTOR.clone());
                }
            } else if target.to_java_string() == self.names.LOCAL_VARIABLE.to_java_string() {
                if ctx.sym(s).kind == Kind::VAR
                    && ctx.sym(s).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH)
                    && (ctx.sym(s).flags_field & crate::code::flags::PARAMETER) == 0
                {
                    applicable.push(self.names.LOCAL_VARIABLE.clone());
                }
            } else if target.to_java_string() == self.names.ANNOTATION_TYPE.to_java_string() {
                if ctx.sym(s).kind == Kind::TYP
                    && (ctx.sym(s).flags_field & crate::code::flags::ANNOTATION as i64) != 0
                {
                    applicable.push(self.names.ANNOTATION_TYPE.clone());
                }
            } else if target.to_java_string() == self.names.PACKAGE.to_java_string() {
                if ctx.sym(s).kind == Kind::PCK {
                    applicable.push(self.names.PACKAGE.clone());
                }
            } else if target.to_java_string() == self.names.TYPE_USE.to_java_string() {
                let implicitly_typed_local = ctx.sym(s).kind == Kind::VAR
                    && ctx.sym(s).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH)
                    && ctx.sym(s).ty.map(|ty| ctx.typ(ty).has_tag(TypeTag::NONE)).unwrap_or(false);
                if implicitly_typed_local {
                    continue;
                }
                let is_type_use = ctx.sym(s).kind == Kind::TYP
                    || ctx.sym(s).kind == Kind::VAR
                    || (ctx.sym(s).kind == Kind::MTH
                        && !self.symbol_is_constructor(ctx, s)
                        && ctx
                            .sym(s)
                            .ty
                            .map(|ty| {
                                !matches!(
                                    &ctx.typ(ty).kind,
                                    crate::code::type_::TypeKindData::MethodType {
                                        restype: Some(restype),
                                        ..
                                    } if ctx.typ(*restype).has_tag(TypeTag::VOID)
                                )
                            })
                            .unwrap_or(true))
                    || (ctx.sym(s).kind == Kind::MTH && self.symbol_is_constructor(ctx, s));
                if is_type_use {
                    applicable.push(self.names.TYPE_USE.clone());
                }
            } else if target.to_java_string() == self.names.TYPE_PARAMETER.to_java_string() {
                if ctx.sym(s).kind == Kind::TYP
                    && ctx.sym(s).ty.map(|ty| ctx.typ(ty).has_tag(TypeTag::TYPEVAR)).unwrap_or(false)
                {
                    applicable.push(self.names.TYPE_PARAMETER.clone());
                }
            } else if target.to_java_string() == self.names.MODULE.to_java_string() {
                if ctx.sym(s).kind == Kind::MDL {
                    applicable.push(self.names.MODULE.clone());
                }
            } else {
                // Java: Unknown ElementType: recovery, assume applicable.
                return Ok(None);
            }
        }
        Ok(Some(applicable))
    }

    /// Java: Name[] getTargetNames(TypeSymbol annoSym) (Check.java:3310-3335)
    pub fn get_target_names(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        anno_sym: SymbolId,
    ) -> Result<Vec<Name>, CompletionFailure> {
        let arr = self.get_attribute_target_attribute(ctx, syms, anno_sym)?;
        let Some(arr) = arr else {
            return Ok(self.default_target_meta_info());
        };
        let values = match &ctx.attr(arr).kind {
            crate::code::attribute::AttributeKind::Array { values } => values.clone(),
            _ => return Ok(Vec::new()),
        };
        let mut targets: Vec<Name> = Vec::new();
        for value in values {
            let enum_value = match &ctx.attr(value).kind {
                crate::code::attribute::AttributeKind::Enum { value } => *value,
                _ => return Ok(Vec::new()),
            };
            let Some(enum_value) = enum_value else {
                return Ok(Vec::new());
            };
            targets.push(ctx.sym(enum_value).name.clone());
        }
        Ok(targets)
    }

    /// Java: Attribute.Array getAttributeTargetAttribute(TypeSymbol s)
    /// (Check.java:3423-3429)
    fn get_attribute_target_attribute(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s: SymbolId,
    ) -> Result<Option<AttributeId>, CompletionFailure> {
        let Some(metadata) = ctx.sym(s).get_annotation_type_metadata().cloned() else {
            return Ok(None);
        };
        let at_target =
            crate::comp::annotate::AnnotationTypeMetadata::get_target_shared(&metadata, ctx, syms)?;
        let Some(at_target) = at_target else {
            return Ok(None);
        };
        let value_name = self.names.value.clone();
        let Some(value) = ctx.attr(at_target).member(ctx, &value_name) else {
            return Ok(None);
        };
        match &ctx.attr(value).kind {
            crate::code::attribute::AttributeKind::Array { .. } => Ok(Some(value)),
            _ => Ok(None),
        }
    }

    /// Java: private Name[] defaultTargetMetaInfo() (Check.java:3431-3452)
    fn default_target_meta_info(&self) -> Vec<Name> {
        let mut targets = vec![
            self.names.PACKAGE.clone(),
            self.names.TYPE.clone(),
            self.names.FIELD.clone(),
            self.names.METHOD.clone(),
            self.names.CONSTRUCTOR.clone(),
            self.names.ANNOTATION_TYPE.clone(),
            self.names.LOCAL_VARIABLE.clone(),
            self.names.PARAMETER.clone(),
        ];
        if self.allow_records {
            targets.push(self.names.RECORD_COMPONENT.clone());
        }
        // Java: allowModules — Check.allowModules (Feature.MODULES); the ported
        // Check keeps no allowModules field (registered: module directives).
        targets
    }

    /// Java: private boolean symbol_is_constructor helper (MethodSymbol.isConstructor)
    fn symbol_is_constructor(&self, ctx: &CompilerContext, s: SymbolId) -> bool {
        ctx.sym(s).name.to_java_string() == ctx.sym(s).name.table.names().init.to_java_string()
    }

    /// Java: public boolean validateAnnotationDeferErrors(JCAnnotation a)
    /// (Check.java:3457-3466)
    pub fn validate_annotation_defer_errors(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
    ) -> Result<bool, CompletionFailure> {
        self.log
            .borrow_mut()
            .install_diagnostic_handler(crate::util::log::DiagnosticHandler::Discard { prev: None });
        let result = self.validate_annotation_values(ctx, syms, a);
        self.log.borrow_mut().pop_diagnostic_handler();
        result
    }

    /// Java: private boolean validateAnnotation(JCAnnotation a)
    /// (Check.java:3468-3505) — the element inventory check.
    fn validate_annotation_values(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
    ) -> Result<bool, CompletionFailure> {
        let mut is_valid = true;
        let annotation_ty = ctx.tree(a).ty.expect("annotation type");
        let anno_tsym = ctx.typ(annotation_ty).tsym.expect("annotation tsym");
        let Some(metadata) = ctx.sym(anno_tsym).get_annotation_type_metadata().cloned() else {
            return Ok(true);
        };
        let mut elements = crate::comp::annotate::AnnotationTypeMetadata::get_annotation_elements_shared(
            &metadata, ctx, syms,
        )?;
        let args = match &ctx.tree(a).kind {
            TreeKind::Annotation { args, .. } => args.clone(),
            _ => panic!("expected an annotation"),
        };
        for arg in args.iter() {
            if !ctx.tree(arg).has_tag(Tag::ASSIGN) {
                continue; // recovery
            }
            let lhs = match &ctx.tree(arg).kind {
                TreeKind::Assign { lhs, .. } => *lhs,
                _ => None,
            };
            let Some(lhs) = lhs else { continue };
            let Some(m) = crate::tree::tree_info::symbol(ctx, lhs) else { continue };
            if ctx
                .sym(m)
                .ty
                .map(|ty| ctx.typ(ty).has_tag(TypeTag::ERROR))
                .unwrap_or(false)
            {
                continue;
            }
            if let Some(pos) = elements.iter().position(|element| *element == m) {
                elements.remove(pos);
            } else {
                is_valid = false;
                self.report_error(
                    ctx.tree(lhs).pos,
                    errors::duplicate_annotation_member_value(
                        ctx.sym(m).name.clone(),
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            crate::comp::resolve::symbol_display_name(
                                ctx,
                                ctx.typ(annotation_ty).tsym.expect("annotation tsym"),
                            ),
                        ),
                    ),
                );
            }
        }
        let members_with_default =
            crate::comp::annotate::AnnotationTypeMetadata::get_annotation_elements_with_default_shared(
                &metadata, ctx, syms,
            )?;
        let mut missing_defaults: List<Name> = List::nil();
        for element in elements {
            if ctx
                .sym(element)
                .ty
                .map(|ty| ctx.typ(ty).has_tag(TypeTag::ERROR))
                .unwrap_or(false)
            {
                continue;
            }
            if !members_with_default.contains(&element) {
                missing_defaults = missing_defaults.append(ctx.sym(element).name.clone());
            }
        }
        let missing_defaults = missing_defaults.reverse();
        if missing_defaults.non_empty() {
            is_valid = false;
            let names: Vec<Name> = missing_defaults.to_vec();
            let error = if names.len() > 1 {
                errors::annotation_missing_default_value1(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        crate::comp::resolve::symbol_display_name(ctx, anno_tsym),
                    ),
                    names,
                )
            } else {
                errors::annotation_missing_default_value(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        crate::comp::resolve::symbol_display_name(ctx, anno_tsym),
                    ),
                    names,
                )
            };
            self.report_error(ctx.tree(a).pos, error);
        }
        Ok(is_valid && self.validate_target_annotation_value(ctx, syms, a))
    }

    /// Java: boolean validateTargetAnnotationValue(JCAnnotation a)
    /// (Check.java:3508-3530)
    pub fn validate_target_annotation_value(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
    ) -> bool {
        let anno_ty = ctx.tree(a).ty.expect("annotation type");
        let anno_tsym = ctx.typ(anno_ty).tsym.expect("annotation tsym");
        let target_tsym = ctx.typ(syms.annotation_target_type).tsym;
        if Some(anno_tsym) != target_tsym {
            return true;
        }
        let args = match &ctx.tree(a).kind {
            TreeKind::Annotation { args, .. } => args.clone(),
            _ => panic!("expected an annotation"),
        };
        if args.length() <= 1 {
            return true;
        }
        let first = *args.head();
        if !ctx.tree(first).has_tag(Tag::ASSIGN) {
            return false;
        }
        let (lhs, rhs) = match &ctx.tree(first).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => (None, None),
        };
        let Some(lhs) = lhs else { return false };
        let Some(m) = crate::tree::tree_info::symbol(ctx, lhs) else { return false };
        if ctx.sym(m).name.to_java_string() != self.names.value.to_java_string() {
            return false;
        }
        let Some(rhs) = rhs else { return false };
        if !ctx.tree(rhs).has_tag(Tag::NEWARRAY) {
            return false;
        }
        let elems = match &ctx.tree(rhs).kind {
            TreeKind::NewArray { elems, .. } => elems.clone().unwrap_or_else(List::nil),
            _ => List::nil(),
        };
        let mut targets: Vec<SymbolId> = Vec::new();
        let mut is_valid = true;
        for elem in elems.iter() {
            let sym = crate::tree::tree_info::symbol(ctx, elem);
            let Some(sym) = sym else { continue };
            if targets.contains(&sym) {
                is_valid = false;
                self.report_error(ctx.tree(elem).pos, errors::repeated_annotation_target());
            } else {
                targets.push(sym);
            }
        }
        is_valid
    }
}

// ---------------------------------------------------------------------------
// Check : checkCastable (Check.java:632-645), checkValidGenericType
// (Check.java:1076-1162), castWarner/ConversionWarner adapter, checkExtends.
// ---------------------------------------------------------------------------

impl Check {
    /// Java: void warnUnchecked(DiagnosticPosition pos, Warning warnKey)
    /// (Check.java:262-268)
    pub(crate) fn warn_unchecked(&self, pos: i32, warn_key: crate::resources::diagnostic_contract::WarningInfo) {
        if !self.lint.is_suppressed(LintCategory::UNCHECKED) {
            self.log.borrow_mut().warning(
                Some(LintCategory::UNCHECKED),
                Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
                warning_from_info(warn_key),
            );
        }
    }

    /// Java: public Warner castWarner(DiagnosticPosition pos, Type found,
    /// Type expected) (Check.java:3970-3972).
    ///
    /// PORTING_RULES Porting rules adapter: the upstream ConversionWarner subclass overrides
    /// warn() to emit the detected unchecked warning immediately; Rust has no
    /// subclassing, so checkCastable records the lint on the plain Warner and
    /// reports it after isCastable returns (same single warning).
    pub fn cast_warner(&self, pos: i32) -> Warner {
        Warner::new(Some(Rc::new(SimpleDiagnosticPosition::new(pos))))
    }

    /// Java: Type checkCastable(DiagnosticPosition pos, Type found, Type req)
    /// (Check.java:630-632)
    pub fn check_castable(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> TypeId {
        let basic_handler = Rc::clone(&self.basic_handler);
        self.check_castable_with_context(ctx, syms, pos, found, req, &*basic_handler)
    }

    /// Java: Type checkCastable(DiagnosticPosition pos, Type found, Type req,
    /// CheckContext checkContext) (Check.java:634-641)
    pub fn check_castable_with_context(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        pos: i32,
        found: TypeId,
        req: TypeId,
        check_context: &dyn crate::comp::check::CheckContext,
    ) -> TypeId {
        let mut warn = self.cast_warner(pos);
        if self
            .types
            .is_castable_with_warn(ctx, syms, found, req, &mut warn)
        {
            if warn.has_lint(LintCategory::UNCHECKED) {
                self.warn_unchecked(
                    pos,
                    warnings::prob_found_req(
                        crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                            "unchecked.cast.to.type",
                        ),
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            self.type_display(ctx, found),
                        ),
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            self.type_display(ctx, req),
                        ),
                    ),
                );
            }
            req
        } else {
            let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::inconvertible_types(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        self.type_display(ctx, found),
                    ),
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        self.type_display(ctx, req),
                    ),
                ),
            ));
            check_context.report(ctx, pos, &diag);
            self.types
                .create_error_type_original(ctx, Some(found), syms)
        }
    }

    /// The display text used for the ADR 0002 P4 TypeArg placeholders.
    fn type_display(&self, ctx: &CompilerContext, t: TypeId) -> String {
        match ctx.typ(t).tsym {
            Some(tsym) => crate::comp::resolve::symbol_display_name(ctx, tsym),
            None => format!("{:?}", ctx.typ(t).get_tag()),
        }
    }

    /// Java: public boolean checkValidGenericType(Type t) (Check.java:1076-1078)
    pub fn check_valid_generic_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> bool {
        self.first_incompatible_type_arg(ctx, syms, t).is_none()
    }

    /// Java: private Type firstIncompatibleTypeArg(Type type)
    /// (Check.java:1080-1162)
    fn first_incompatible_type_arg(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> Option<TypeId> {
        use crate::code::type_::TypeKindData;
        let tsym = ctx.typ(t).tsym;
        let formals = tsym
            .and_then(|tsym| ctx.sym(tsym).ty)
            .map(|tsym_ty| crate::code::types::allparams_of(ctx, tsym_ty))
            .unwrap_or_else(List::nil);
        let actuals = crate::code::types::allparams_of(ctx, t);
        let args = crate::code::types::get_type_arguments(ctx, t);
        let forms = tsym
            .and_then(|tsym| ctx.sym(tsym).ty)
            .map(|tsym_ty| crate::code::types::get_type_arguments(ctx, tsym_ty))
            .unwrap_or_else(List::nil);
        let mut bounds_buf: Vec<TypeId> = Vec::new();
        let mut args_iter = args.clone();
        let mut forms_iter = forms;
        while args_iter.non_empty() && forms_iter.non_empty() {
            let form = *forms_iter.head();
            let upper = ctx
                .typ(form)
                .get_upper_bound()
                .expect("type variable upper bound");
            let subst_list =
                self.types
                    .subst(ctx, syms, &List::of(upper), &formals, &actuals);
            bounds_buf.push(*subst_list.head());
            args_iter = args_iter.tail();
            forms_iter = forms_iter.tail();
        }
        let mut args_iter = args.clone();
        let capture = self.types.capture(ctx, syms, t);
        let capture_allparams = crate::code::types::allparams_of(ctx, capture);
        let mut tvars_cap = self
            .types
            .subst_bounds(ctx, syms, &formals, &formals, &capture_allparams);
        while args_iter.non_empty() && tvars_cap.non_empty() {
            let arg = *args_iter.head();
            let tv = *tvars_cap.head();
            if let TypeKindData::TypeVar { bound, .. } = &mut ctx.typ_mut(arg).kind {
                *bound = Some(tv);
            }
            args_iter = args_iter.tail();
            tvars_cap = tvars_cap.tail();
        }
        let mut args_iter = args.clone();
        let mut bounds = List::from_iterable(bounds_buf.clone());
        while args_iter.non_empty() && bounds.non_empty() {
            let actual = *args_iter.head();
            let bound = *bounds.head();
            if !self.is_type_arg_erroneous(ctx, syms, actual)
                && !self.types.is_erroneous(ctx, syms, bound)
                && !self.check_extends(ctx, syms, actual, bound)
            {
                return Some(actual);
            }
            args_iter = args_iter.tail();
            bounds = bounds.tail();
        }
        let mut args_iter = args.clone();
        let mut bounds = List::from_iterable(bounds_buf);
        let captured = self.types.capture(ctx, syms, t);
        let mut captured_args = crate::code::types::get_type_arguments(ctx, captured);
        while captured_args.non_empty() && bounds.non_empty() && args_iter.non_empty() {
            let arg = *captured_args.head();
            let bound = *bounds.head();
            if ctx.typ(arg).has_tag(TypeTag::TYPEVAR)
                && self.types.is_erroneous(
                    ctx,
                    syms,
                    ctx.typ(arg)
                        .get_upper_bound()
                        .expect("captured type variable upper bound"),
                )
                && !self.types.is_erroneous(ctx, syms, bound)
                && !self.is_type_arg_erroneous(ctx, syms, *args_iter.head())
            {
                return Some(*args_iter.head());
            }
            captured_args = captured_args.tail();
            bounds = bounds.tail();
            args_iter = args_iter.tail();
        }
        None
    }

    /// Java: boolean isTypeArgErroneous(Type t) / the isTypeArgErroneous
    /// UnaryVisitor (Check.java:1128-1162)
    fn is_type_arg_erroneous(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> bool {
        use crate::code::type_::TypeKindData;
        match &ctx.typ(t).kind {
            TypeKindData::TypeVar { .. } => {
                let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                self.is_type_arg_erroneous(ctx, syms, upper)
            }
            TypeKindData::CapturedType { .. } => {
                let upper = ctx.typ(t).get_upper_bound().expect("captured upper bound");
                if self.is_type_arg_erroneous(ctx, syms, upper) {
                    return true;
                }
                let lower = ctx.typ(t).get_lower_bound().expect("captured lower bound");
                self.is_type_arg_erroneous(ctx, syms, lower)
            }
            TypeKindData::WildcardType { type_, .. } => type_
                .map(|type_| self.is_type_arg_erroneous(ctx, syms, type_))
                .unwrap_or(false),
            _ => self.types.is_erroneous(ctx, syms, t),
        }
    }

    /// Java: private boolean checkExtends(Type a, Type bound) (Check.java:682-698)
    fn check_extends(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        a: TypeId,
        bound: TypeId,
    ) -> bool {
        if ctx.typ(a).is_unbound() {
            return true;
        }
        if !ctx.typ(a).has_tag(TypeTag::WILDCARD) {
            let a = self.types.cvar_upper_bound(ctx, a);
            return self.types.is_subtype(ctx, syms, a, bound);
        }
        if ctx.typ(a).is_extends_bound() {
            let mut no_warnings = Warner::new(None);
            let wild_upper = self.types.wild_upper_bound(ctx, syms, a);
            return self
                .types
                .is_castable_with_warn(ctx, syms, bound, wild_upper, &mut no_warnings);
        }
        if ctx.typ(a).is_super_bound() {
            let wild_lower = self.types.wild_lower_bound(ctx, syms, a);
            return !self.types.not_soft_subtype(ctx, syms, wild_lower, bound);
        }
        true
    }
}


// ---------------------------------------------------------------------------
// prerequisites: Check.convertWarner (Check.java:3979-3981),
// Check.assertConvertible (Check.java:1082-1093), Check.isTrustMeAllowedOnMethod
// (Check.java:970-977), Check.checkMethod (Check.java:991-1080).
// adapter adapters: explicit ctx/syms parameters (ADR 0001); the ConversionWarner
// subclass is a plain Warner with the same lint-recording behavior used by
// castWarner; unchecked warnings are recorded through warn_unchecked.
// ---------------------------------------------------------------------------
impl Check {
    /// Java: public Warner convertWarner(DiagnosticPosition pos, Type found,
    /// Type expected) (Check.java:3979-3981)
/// # Arguments
///   - pos: the diagnostic position
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Check.java:900-940 (initWarner/convertWarner).
///
    pub fn convert_warner(&self, pos: i32) -> Warner {
        Warner::new(Some(Rc::new(SimpleDiagnosticPosition::new(pos))))
    }

    /// Java: private boolean isTrustMeAllowedOnMethod(Symbol s)
    /// (Check.java:970-977)
    fn is_trust_me_allowed_on_method(&self, ctx: &CompilerContext, s: SymbolId) -> bool {
        let flags = ctx.sym(s).flags_field;
        if flags & crate::code::flags::VARARGS == 0 {
            return false;
        }
        let is_constructor = ctx.sym(s).name == self.names.init;
        if is_constructor {
            return true;
        }
        let mut mask = (crate::code::flags::STATIC as i64) | (crate::code::flags::FINAL as i64);
        if self.private_safe_varargs {
            mask |= crate::code::flags::PRIVATE as i64;
        }
        flags & mask != 0
    }

    /// Java: private void assertConvertible(JCTree tree, Type actual,
    /// Type formal, Warner warn) (Check.java:1082-1093)
    fn assert_convertible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        _tree: TreeId,
        actual: TypeId,
        formal: TypeId,
        warn: &mut Warner,
    ) {
        if self
            .types
            .is_convertible_with_warn(ctx, syms, actual, formal, warn)
        {
            return;
        }
        let formal_is_compound = ctx
            .typ(formal)
            .tsym
            .map(|tsym| ctx.sym(tsym).flags_field & (crate::code::flags::COMPOUND as i64) != 0)
            .unwrap_or(false);
        if formal_is_compound {
            let supertype = self.types.supertype(ctx, syms, formal);
            let interfaces = self.types.interfaces(ctx, syms, formal);
            let actual_subtype_super = self.types.is_subtype(ctx, syms, actual, supertype);
            let mut actual_subtype_interfaces = true;
            for intf in interfaces.iter() {
                if !self
                    .types
                    .is_subtype_unchecked_with_warn(ctx, syms, actual, intf, warn)
                {
                    actual_subtype_interfaces = false;
                    break;
                }
            }
            // Java: assertConvertible has no error path here — the checks above
            // exist for their warning side effects only.
            let _ = actual_subtype_super && actual_subtype_interfaces;
        }
    }

    /// Java: Type checkMethod(final Type mtype, final Symbol sym,
    /// final Env<AttrContext> env, final List<JCExpression> argtrees,
    /// final List<Type> argtypes, final boolean useVarargs,
    /// InferenceContext inferenceContext) (Check.java:991-1080)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/types/attr/chk: explicit services;
///   - mtype/sym: the candidate method;
///   - argtrees/argtypes: the actual arguments;
///   - use_varargs/inference_context: the resolution round
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Check.java:903-970 (checkMethod).
///
    pub fn check_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        mtype: TypeId,
        sym: SymbolId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        argtrees: Option<&List<TreeId>>,
        argtypes: &List<TypeId>,
        use_varargs: bool,
        inference_context: &Rc<crate::comp::infer::InferenceContext>,
    ) -> TypeId {
        // Java: if (inferenceContext.free(mtype)) { addFreeTypeListener(...);
        // return mtype; }
        if inference_context.free(ctx, syms, &self.types, mtype) {
            // The listener re-checks the instantiated method type; the port
            // records it through the Batches' free-type-listener mechanism once
            // the listener closure can carry the mutable Check/Attr pair
            // (registered Porting rules; no substitute result is produced: the branch
            // returns mtype like the upstream code before the listener fires).
            return mtype;
        }
        let owntype = mtype;
        let mut formals = self.types.get_parameter_types_public(ctx, owntype);
        let sym_type = ctx.sym(sym).ty.expect("checked method type");
        let mut non_inferred = self.types.get_parameter_types_public(ctx, sym_type);
        if non_inferred.length() != formals.length() {
            non_inferred = formals.clone();
        }
        let last = if use_varargs {
            Some(last_of(&formals))
        } else {
            None
        };
        if ctx.sym(sym).name == self.names.init && ctx.sym(sym).owner == Some(syms.enum_sym) {
            formals = formals.tail().tail();
            non_inferred = non_inferred.tail().tail();
        }
        if ctx.sym(sym).flags_field & crate::code::flags::ANONCONSTR_BASED != 0 {
            formals = formals.tail();
            non_inferred = non_inferred.tail();
        }
        let args = argtrees.cloned();
        if let Some(mut args_list) = args.clone() {
            while args_list.non_empty() && Some(*formals.head()) != last {
                let arg = *args_list.head();
                let arg_type = ctx.tree(arg).ty.expect("attributed argument type");
                let non_inferred_head = *non_inferred.head();
                let mut warn = self.convert_warner(ctx.tree(arg).pos);
                self.assert_convertible(
                    ctx,
                    syms,
                    arg,
                    arg_type,
                    *formals.head(),
                    &mut warn,
                );
                let _ = non_inferred_head;
                args_list = args_list.tail();
                formals = formals.tail();
                non_inferred = non_inferred.tail();
            }
            if use_varargs {
                let last = last.expect("varargs last formal");
                let var_arg = self.types.elemtype(ctx, syms, last);
                while args_list.non_empty() && !args_list.tail().is_empty() {
                    let arg = *args_list.head();
                    let arg_type = ctx.tree(arg).ty.expect("attributed argument type");
                    let mut warn = self.convert_warner(ctx.tree(arg).pos);
                    self.assert_convertible(ctx, syms, arg, arg_type, var_arg, &mut warn);
                    args_list = args_list.tail();
                }
            } else if ctx.sym(sym).flags_field
                & (crate::code::flags::VARARGS | crate::code::flags::SIGNATURE_POLYMORPHIC)
                == crate::code::flags::VARARGS
            {
                // non-varargs call to varargs method
                let var_param = last_of(&self.types.get_parameter_types_public(ctx, owntype));
                let last_arg = last_of(argtypes);
                let elem = self.types.elemtype(ctx, syms, var_param);
                let erased_var_param = self.types.erasure(ctx, syms, var_param);
                let erased_last_arg = self.types.erasure(ctx, syms, last_arg);
                if self
                    .types
                    .is_subtype_unchecked(ctx, syms, last_arg, elem)
                    && !self
                        .types
                        .is_same_type(ctx, syms, erased_var_param, erased_last_arg)
                {
                    let last_tree = args
                        .as_ref()
                        .and_then(|args| args.last())
                        .map(|tree| ctx.tree(tree).pos)
                        .unwrap_or(crate::util::position::NOPOS);
                    let warning = warning_from_info(
                        crate::resources::compiler_properties::warnings::inexact_non_varargs_call(
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::comp::attr::type_display_of(ctx, elem),
                            ),
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::comp::attr::type_display_of(ctx, var_param),
                            ),
                        ),
                    );
                    self.log.borrow_mut().warning(
                        None,
                        Some(Rc::new(SimpleDiagnosticPosition::new(last_tree))),
                        warning,
                    );
                }
            }
        }
        if use_varargs {
            let argtype = last_of(&self.types.get_parameter_types_public(ctx, owntype));
            let is_reifiable = self.types.is_reifiable(ctx, syms, argtype);
            if !is_reifiable {
                let base = crate::code::symbol::base_symbol(ctx, sym);
                let trust_me_sym = ctx
                    .typ(syms.trust_me_type)
                    .tsym
                    .expect("trustMe type symbol");
                let has_trust_me = ctx
                    .sym(base)
                    .attribute(ctx, trust_me_sym)
                    .is_some();
                if !has_trust_me || !self.is_trust_me_allowed_on_method(ctx, sym) {
                    let env_tree = env.tree.expect("env tree");
                    self.warn_unchecked(
                        ctx.tree(env_tree).pos,
                        crate::resources::compiler_properties::warnings::unchecked_generic_array_creation(
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::comp::attr::type_display_of(ctx, argtype),
                            ),
                        ),
                    );
                }
            }
            let elemtype = self.types.elemtype(ctx, syms, argtype);
            let env_tree = env.tree.expect("env tree");
            crate::tree::tree_info::set_varargs_element(ctx, env_tree, Some(elemtype));
        }
        owntype
    }
}

/// List.last() helper (Java: List.last()).
fn last_of(ts: &List<TypeId>) -> TypeId {
    ts.last().expect("non-empty list")
}
impl Check {
    /// Java: void earlyRefError(DiagnosticPosition pos, Symbol sym)
    /// (Check.java:335-337).
    ///
    /// # Arguments
    ///   - pos: the diagnostic position;
    ///   - sym: the referenced symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:335-337.
    pub fn early_ref_error(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        log: &std::rc::Rc<std::cell::RefCell<crate::util::log::Log>>,
        pos: i32,
        sym: SymbolId,
    ) {
        let info = crate::resources::compiler_properties::errors::cant_ref_before_ctor_called(
            crate::resources::diagnostic_contract::DiagnosticArg::Text(
                ctx.sym(sym).name.to_java_string().to_string_lossy().to_string(),
            ),
        );
        crate::util::abstract_log::AbstractLog::error_at(
            &mut *log.borrow_mut(),
            pos,
            crate::util::jc_diagnostic::Error::from(info),
        );
    }

}

impl Check {
    /// Java: List<Type> checkRefTypes(List<JCExpression> trees, List<Type> types)
    /// (Check.java:801-812).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - trees: the original trees (for positions);
    ///   - types: the types to check.
    ///
    /// # Returns
    ///   - the checked types (error types replaced).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:801-812.
    pub fn check_ref_types(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        trees: crate::util::list::List<TreeId>,
        types: crate::util::list::List<TypeId>,
    ) -> crate::util::list::List<TypeId> {
        let mut result: crate::util::list::List<TypeId> = crate::util::list::List::nil();
        let mut tl = trees;
        let mut l = types;
        while l.non_empty() {
            let head = *l.head();
            let pos = if tl.non_empty() {
                ctx.tree(*tl.head()).pos
            } else {
                0
            };
            result = result.append(self.check_ref_type(ctx, syms, pos, head));
            if tl.non_empty() {
                tl = tl.tail();
            }
            l = l.tail();
        }
        result
    }
}
impl Check {
    /// Java: void warnStatic(DiagnosticPosition pos, Warning warnKey)
    /// (Check.java:275-282).
    ///
    /// # Arguments
    ///   - pos: the diagnostic position;
    ///   - warning: the warning key.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:275-282.
    pub fn warn_static(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        log: &std::rc::Rc<std::cell::RefCell<crate::util::log::Log>>,
        pos: i32,
        warning: crate::resources::diagnostic_contract::WarningInfo,
    ) {
        if self.lint.is_enabled(crate::code::lint::LintCategory::STATIC) {
            let warning = crate::util::jc_diagnostic::Warning::from(warning);
            crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                &mut *log.borrow_mut(),
                Some(crate::code::lint::LintCategory::STATIC),
                Some(std::rc::Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                )),
                warning,
            );
        }
        let _ = ctx;
    }
}

impl Check {
    /// Java: Type checkLocalVarType(DiagnosticPosition pos, Type t, Name name)
    /// (Check.java:954-977).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - t: the inferred initializer type;
    ///   - name: the variable name.
    ///
    /// # Returns
    ///   - the inferred local variable type (or an error type).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:954-977.
    pub fn check_local_var_type(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: TypeId,
        name: crate::util::name::Name,
    ) -> TypeId {
        // check that resulting type is not the null type
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::BOT) {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::local_cant_infer_null(),
            ));
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::cant_infer_local_var_type(
                        name,
                        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                            std::rc::Rc::new(fragment),
                        ),
                    ),
                ),
            );
            return self.types.create_error_type_original(ctx, Some(t), syms);
        } else if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::VOID) {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::local_cant_infer_void(),
            ));
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::cant_infer_local_var_type(
                        name,
                        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                            std::rc::Rc::new(fragment),
                        ),
                    ),
                ),
            );
            return self.types.create_error_type_original(ctx, Some(t), syms);
        }
        // upward project the initializer type
        let captures = self.types.captures(ctx, syms, t);
        let upward = self.types.upward(ctx, syms, t, &captures);
        self.types.base_type_of(ctx, upward)
    }
}

impl Check {
    /// Java: void checkSwitchCaseStructure(List<JCCase> cases) (Check.java:4262-4330).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - cases: the switch cases.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:4262-4330.
    pub fn check_switch_case_structure(&self, ctx: &mut crate::compiler_context::CompilerContext, cases: crate::util::list::List<TreeId>) {
        let mut was_constant = false;
        let mut was_default = false;
        let mut was_null_pattern = false;
        let mut was_pattern = false;
        let mut was_type_pattern = false;
        let mut was_non_empty_fall_through = false;
        let mut current = cases;
        while current.non_empty() {
            let case = *current.head();
            let (labels, case_kind, stats, completes_normally) = match &ctx.tree(case).kind {
                crate::tree::jc_tree::TreeKind::Case { labels, case_kind, stats, completes_normally, .. } => {
                    (labels.clone(), *case_kind, stats.clone(), *completes_normally)
                }
                _ => { current = current.tail(); continue; }
            };
            let mut label_current = labels;
            while label_current.non_empty() {
                let pat = *label_current.head();
                let is_expression = !ctx.tree(pat).has_tag(crate::tree::jc_tree::Tag::DEFAULTCASELABEL)
                    && !ctx.tree(pat).has_tag(crate::tree::jc_tree::Tag::BINDINGPATTERN)
                    && !ctx.tree(pat).has_tag(crate::tree::jc_tree::Tag::PARENTHESIZEDPATTERN)
                    && !ctx.tree(pat).has_tag(crate::tree::jc_tree::Tag::GUARDPATTERN);
                if is_expression {
                    let is_null = crate::tree::tree_info::is_null(ctx, pat);
                    if is_null {
                        if was_pattern && !was_type_pattern && !was_non_empty_fall_through {
                            self.report_flows_through_from_pattern(ctx, pat);
                        }
                        was_null_pattern = true;
                    } else {
                        if was_pattern && !was_non_empty_fall_through {
                            self.report_flows_through_from_pattern(ctx, pat);
                        }
                        was_constant = true;
                    }
                } else if ctx.tree(pat).has_tag(crate::tree::jc_tree::Tag::DEFAULTCASELABEL) {
                    if was_pattern && !was_non_empty_fall_through {
                        self.report_flows_through_from_pattern(ctx, pat);
                    }
                    was_default = true;
                } else {
                    let is_type_pattern = ctx.tree(pat).has_tag(crate::tree::jc_tree::Tag::BINDINGPATTERN);
                    if was_pattern || was_constant || was_default
                        || (was_null_pattern && (!is_type_pattern || was_non_empty_fall_through)) {
                        let info = crate::resources::compiler_properties::errors::flows_through_to_pattern();
                        crate::util::abstract_log::AbstractLog::error_at(
                            &mut *self.log.borrow_mut(),
                            ctx.tree(pat).pos,
                            crate::util::jc_diagnostic::Error::from(info),
                        );
                    }
                    was_pattern = true;
                    was_type_pattern = is_type_pattern;
                }
                label_current = label_current.tail();
            }
            let completes = case_kind == javac_source::tree::case_tree::CaseKind::STATEMENT && completes_normally;
            if !stats.is_empty() {
                was_constant = false;
                was_default = false;
                was_null_pattern = was_null_pattern && completes;
                was_pattern = was_pattern && completes;
                was_type_pattern = was_type_pattern && completes;
            }
            was_non_empty_fall_through = !stats.is_empty() && completes;
            current = current.tail();
        }
    }

    fn report_flows_through_from_pattern(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        pat: TreeId,
    ) {
        let info = crate::resources::compiler_properties::errors::flows_through_from_pattern();
        crate::util::abstract_log::AbstractLog::error_at(
            &mut *self.log.borrow_mut(),
            ctx.tree(pat).pos,
            crate::util::jc_diagnostic::Error::from(info),
        );
    }
}
impl Check {
    /// Java: private boolean isDeprecatedOverrideIgnorable(MethodSymbol m,
    /// ClassSymbol origin) (Check.java:1900-1920).
    fn is_deprecated_override_ignorable(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        m: SymbolId,
        origin: SymbolId,
    ) -> bool {
        let mc = crate::code::symbol::symbol_encl_class(ctx, m);
        let origin_type = ctx.sym(origin).ty.expect("origin type");
        let st = self.types.supertype(ctx, syms, origin_type);
        if !ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            return true;
        }
        let st_tsym = ctx.typ(st).tsym.expect("supertype symbol");
        let stimpl = crate::code::symbol::implementation(ctx, syms, &self.types, m, st_tsym, false);
        match mc {
            Some(mc) if ctx.sym(mc).flags_field & (crate::code::flags::INTERFACE as i64) != 0 => {
                let intfs = self.types.interfaces(ctx, syms, origin_type);
                let mc_type = ctx.sym(mc).ty.expect("enclosing class type");
                let contains = intfs.iter().any(|intf| self.types.is_same_type(ctx, syms, intf, mc_type));
                if contains {
                    false
                } else {
                    stimpl.is_some()
                }
            }
            _ => stimpl != Some(m),
        }
    }

    /// Java: List<Type> unhandled(List<Type> thrown, List<Type> handled)
    /// (Check.java:1680-1685).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - thrown: the thrown types;
    ///   - handled: the handled types.
    ///
    /// # Returns
    ///   - the thrown types not handled.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:1680-1685.
    pub fn unhandled(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        thrown: crate::util::list::List<TypeId>,
        handled: crate::util::list::List<TypeId>,
    ) -> crate::util::list::List<TypeId> {
        let mut unhandled: crate::util::list::List<TypeId> = crate::util::list::List::nil();
        let mut current = thrown;
        while current.non_empty() {
            let ty = *current.head();
            if !self.is_handled(ctx, syms, ty, &handled) {
                unhandled = unhandled.prepend(ty);
            }
            current = current.tail();
        }
        unhandled
    }

    /// Java: static int protection(long flags) (Check.java:1694-1705).
    pub fn protection(flags_value: i64) -> i32 {
        let access = flags_value & (crate::code::flags::AccessFlags as i64);
        let access = access as i16 as i64;
        if access == crate::code::flags::PRIVATE as i64 {
            3
        } else if access == crate::code::flags::PROTECTED as i64 {
            1
        } else if access == crate::code::flags::PUBLIC as i64 {
            0
        } else {
            2
        }
    }

    /// Java: Fragment cannotOverride(MethodSymbol m, MethodSymbol other)
    /// (Check.java:1709-1721).
    fn cannot_override(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        m: SymbolId,
        other: SymbolId,
    ) -> crate::resources::diagnostic_contract::FragmentInfo {
        let mloc = crate::code::symbol::symbol_location(ctx, m).unwrap_or(m);
        let oloc = crate::code::symbol::symbol_location(ctx, other).unwrap_or(other);
        let other_owner = ctx.sym(other).owner;
        let m_owner = ctx.sym(m).owner;
        let owner_is_interface = |sym: Option<SymbolId>| {
            sym.map(|sym| ctx.sym(sym).flags_field & (crate::code::flags::INTERFACE as i64) != 0)
                .unwrap_or(false)
        };
        let args = |ctx: &crate::compiler_context::CompilerContext| {
            (
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, m),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, mloc),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, other),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, oloc),
                ),
            )
        };
        let (a, b, c, d) = args(ctx);
        if !owner_is_interface(other_owner) {
            crate::resources::compiler_properties::fragments::cant_override(a, b, c, d)
        } else if !owner_is_interface(m_owner) {
            crate::resources::compiler_properties::fragments::cant_implement(a, b, c, d)
        } else {
            crate::resources::compiler_properties::fragments::clashes_with(a, b, c, d)
        }
    }

    /// Java: Fragment uncheckedOverrides(MethodSymbol m, MethodSymbol other)
    /// (Check.java:1726-1738).
    fn unchecked_overrides(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        m: SymbolId,
        other: SymbolId,
    ) -> crate::resources::diagnostic_contract::FragmentInfo {
        let mloc = crate::code::symbol::symbol_location(ctx, m).unwrap_or(m);
        let oloc = crate::code::symbol::symbol_location(ctx, other).unwrap_or(other);
        let other_owner = ctx.sym(other).owner;
        let m_owner = ctx.sym(m).owner;
        let owner_is_interface = |sym: Option<SymbolId>| {
            sym.map(|sym| ctx.sym(sym).flags_field & (crate::code::flags::INTERFACE as i64) != 0)
                .unwrap_or(false)
        };
        let arg = |ctx: &crate::compiler_context::CompilerContext| {
            (
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, m),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, mloc),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, other),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, oloc),
                ),
            )
        };
        let (a, b, c, d) = arg(ctx);
        if !owner_is_interface(other_owner) {
            crate::resources::compiler_properties::fragments::unchecked_override(a, b, c, d)
        } else if !owner_is_interface(m_owner) {
            crate::resources::compiler_properties::fragments::unchecked_implement(a, b, c, d)
        } else {
            crate::resources::compiler_properties::fragments::unchecked_clash_with(a, b, c, d)
        }
    }

    /// Java: Fragment varargsOverrides(MethodSymbol m, MethodSymbol other)
    /// (Check.java:1743-1755).
    fn varargs_overrides(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        m: SymbolId,
        other: SymbolId,
    ) -> crate::resources::diagnostic_contract::FragmentInfo {
        let mloc = crate::code::symbol::symbol_location(ctx, m).unwrap_or(m);
        let oloc = crate::code::symbol::symbol_location(ctx, other).unwrap_or(other);
        let other_owner = ctx.sym(other).owner;
        let m_owner = ctx.sym(m).owner;
        let owner_is_interface = |sym: Option<SymbolId>| {
            sym.map(|sym| ctx.sym(sym).flags_field & (crate::code::flags::INTERFACE as i64) != 0)
                .unwrap_or(false)
        };
        let arg = |ctx: &crate::compiler_context::CompilerContext| {
            (
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, m),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, mloc),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, other),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, oloc),
                ),
            )
        };
        let (a, b, c, d) = arg(ctx);
        if !owner_is_interface(other_owner) {
            crate::resources::compiler_properties::fragments::varargs_override(a, b, c, d)
        } else if !owner_is_interface(m_owner) {
            crate::resources::compiler_properties::fragments::varargs_implement(a, b, c, d)
        } else {
            crate::resources::compiler_properties::fragments::varargs_clash_with(a, b, c, d)
        }
    }
}
impl Check {
    /// Java: void checkOverride(JCTree tree, MethodSymbol m, MethodSymbol other,
    /// ClassSymbol origin) (Check.java:1776-1895).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the tree providing positions;
    ///   - m: the overriding method;
    ///   - other: the overridden method;
    ///   - origin: the class where checking started.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:1776-1895.
    #[allow(clippy::too_many_arguments, clippy::collapsible_if)]
    pub fn check_override(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        m: SymbolId,
        other: SymbolId,
        origin: SymbolId,
    ) {
        use crate::code::flags;
        use crate::resources::diagnostic_contract::{DiagnosticArg, SymbolArg, TypeArg};
        let m_flags = ctx.sym(m).flags_field;
        let other_flags = ctx.sym(other).flags_field;
        // Don't check overriding of synthetic methods or by bridge methods.
        if (m_flags & ((flags::SYNTHETIC as i64) | flags::BRIDGE)) != 0
            || (other_flags & (flags::SYNTHETIC as i64)) != 0
        {
            return;
        }
        let pos = crate::tree::tree_info::diagnostic_position_for(ctx, m, tree);
        // Error if static method overrides instance method (JLS 8.4.6.2).
        if (m_flags & (flags::STATIC as i64)) != 0 && (other_flags & (flags::STATIC as i64)) == 0 {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                self.cannot_override(ctx, m, other),
            ));
            let info = crate::resources::compiler_properties::errors::override_static(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
            ctx.sym_mut(m).flags_field |= flags::BAD_OVERRIDE;
            return;
        }
        // Error if instance method overrides static or final method (JLS 8.4.6.1).
        if (other_flags & (flags::FINAL as i64)) != 0
            || ((m_flags & (flags::STATIC as i64)) == 0
                && (other_flags & (flags::STATIC as i64)) != 0)
        {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                self.cannot_override(ctx, m, other),
            ));
            let info = crate::resources::compiler_properties::errors::override_meth(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
                self.as_flag_set_arg(other_flags & ((flags::FINAL as i64) | (flags::STATIC as i64))),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
            ctx.sym_mut(m).flags_field |= flags::BAD_OVERRIDE;
            return;
        }
        let m_owner = ctx.sym(m).owner.expect("method owner");
        if (ctx.sym(m_owner).flags_field & (flags::ANNOTATION as i64)) != 0 {
            // handled in validateAnnotationMethod
            return;
        }
        // Error if overriding method has weaker access (JLS 8.4.6.3).
        let access_flags = flags::AccessFlags as i64;
        if Self::protection(m_flags) > Self::protection(other_flags) {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                self.cannot_override(ctx, m, other),
            ));
            let access = if (other_flags & access_flags) == 0 {
                DiagnosticArg::from_display("package".to_string())
            } else {
                DiagnosticArg::from_display(
                    self.as_flag_set_arg(other_flags & access_flags)
                        .iter()
                        .map(|flag| flag.0.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            };
            let info = crate::resources::compiler_properties::errors::override_weaker_access(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
                access,
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
            ctx.sym_mut(m).flags_field |= flags::BAD_OVERRIDE;
            return;
        }
        let origin_type = ctx.sym(origin).ty.expect("origin type");
        let mt = self.types.member_type(ctx, syms, origin_type, m);
        let ot = self.types.member_type(ctx, syms, origin_type, other);
        let mtvars = ctx.typ(mt).get_type_arguments();
        let otvars = ctx.typ(ot).get_type_arguments();
        let mtres = self.types.get_return_type_public(ctx, mt);
        let otres_unsubst = self.types.get_return_type_public(ctx, ot);
        let otres_list = self.types.subst(ctx, syms, &crate::util::list::List::of(otres_unsubst), &otvars, &mtvars);
        let otres = *otres_list.head();
        let mut override_warner = crate::util::warner::Warner::new(None);
        let result_types_ok = self.types.return_type_substitutable_with(
            ctx,
            syms,
            mt,
            ot,
            otres,
            &mut override_warner,
        );
        if !result_types_ok {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                if (m_flags & (flags::STATIC as i64)) != 0 && (other_flags & (flags::STATIC as i64)) != 0 {
                    crate::resources::compiler_properties::fragments::cant_hide(
                        SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, m)),
                        SymbolArg::placeholder(
                            crate::comp::resolve::symbol_display_name(
                                ctx,
                                crate::code::symbol::symbol_location(ctx, m).unwrap_or(m),
                            ),
                        ),
                        SymbolArg::placeholder(
                            crate::comp::resolve::symbol_display_name(ctx, other),
                        ),
                        SymbolArg::placeholder(
                            crate::comp::resolve::symbol_display_name(
                                ctx,
                                crate::code::symbol::symbol_location(ctx, other).unwrap_or(other),
                            ),
                        ),
                    )
                } else {
                    self.cannot_override(ctx, m, other)
                },
            ));
            let info = crate::resources::compiler_properties::errors::override_incompatible_ret(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, mtres)),
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, otres)),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
            ctx.sym_mut(m).flags_field |= flags::BAD_OVERRIDE;
            return;
        } else if override_warner.has_non_silent_lint(crate::code::lint::LintCategory::UNCHECKED) {
            let warning = crate::resources::compiler_properties::warnings::override_unchecked_ret(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(
                    self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        self.unchecked_overrides(ctx, m, other),
                    )),
                )),
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, mtres)),
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, otres)),
            );
            let diags_arg = DiagnosticArg::from_display(
                warning.info().args.first().map(|arg| arg.to_text()).unwrap_or_default(),
            );
            let warning = crate::resources::compiler_properties::warnings::override_unchecked_ret(
                diags_arg,
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, mtres)),
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, otres)),
            );
            self.warn_unchecked(pos, warning);
        }
        // Error if overriding method throws an exception not reported by the
        // overridden method.
        let otthrown_unsubst = ctx.typ(ot).get_thrown_types(ctx);
        let otthrown = self.types.subst(ctx, syms, &otthrown_unsubst, &otvars, &mtvars);
        let mtthrown = ctx.typ(mt).get_thrown_types(ctx);
        let mut otthrown_erased: crate::util::list::List<TypeId> = crate::util::list::List::nil();
        let mut erased_current = otthrown.clone();
        while erased_current.non_empty() {
            let item = *erased_current.head();
            otthrown_erased = otthrown_erased.append(self.types.erasure(ctx, syms, item));
            erased_current = erased_current.tail();
        }
        let unhandled_erased = self.unhandled(ctx, syms, mtthrown.clone(), otthrown_erased);
        let unhandled_unerased = self.unhandled(ctx, syms, mtthrown, otthrown.clone());
        if unhandled_erased.non_empty() {
            let head = *unhandled_erased.head();
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                self.cannot_override(ctx, m, other),
            ));
            let info = crate::resources::compiler_properties::errors::override_meth_doesnt_throw(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, head)),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
            ctx.sym_mut(m).flags_field |= flags::BAD_OVERRIDE;
            return;
        } else if unhandled_unerased.non_empty() {
            let head = *unhandled_unerased.head();
            let warning = crate::resources::compiler_properties::warnings::override_unchecked_thrown(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(
                    self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        self.cannot_override(ctx, m, other),
                    )),
                )),
                TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, head)),
            );
            self.warn_unchecked(pos, warning);
            return;
        }
        // Optional warning if varargs don't agree.
        if ((m_flags ^ other_flags) & flags::VARARGS) != 0
            && self.lint.is_enabled(crate::code::lint::LintCategory::OVERRIDES)
        {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                self.varargs_overrides(ctx, m, other),
            ));
            let warning = if (m_flags & flags::VARARGS) != 0 {
                crate::resources::compiler_properties::warnings::override_varargs_missing(
                    DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
                )
            } else {
                crate::resources::compiler_properties::warnings::override_varargs_extra(
                    DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
                )
            };
            let warning = crate::util::jc_diagnostic::Warning::from(warning);
            crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                &mut *self.log.borrow_mut(),
                Some(crate::code::lint::LintCategory::OVERRIDES),
                Some(std::rc::Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                )),
                warning,
            );
        }
        // Warn if instance method overrides bridge method.
        if (other_flags & flags::BRIDGE) != 0 {
            let warning = crate::resources::compiler_properties::warnings::override_bridge(
                DiagnosticArg::JCDiagnostic(std::rc::Rc::new(
                    self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        self.unchecked_overrides(ctx, m, other),
                    )),
                )),
            );
            let warning = crate::util::jc_diagnostic::Warning::from(warning);
            crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                &mut *self.log.borrow_mut(),
                None,
                Some(std::rc::Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                )),
                warning,
            );
        }
        // Warn if a deprecated method is overridden by a non-deprecated one.
        if !self.is_deprecated_override_ignorable(ctx, syms, m, origin) {
            // TODO(Check): checkDeprecated is not ported yet (Check.java:3520-3560,
            // the deprecated warning family); registered.
        }
    }
}

impl Check {
    /// Java: void checkImplementations(JCClassDecl tree) (Check.java:2456-2460).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the class definition.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the class declaration symbol is required.
    ///
    /// Upstream: Check.java:2456-2460.
    pub fn check_implementations(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
    ) {
        let sym = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => return,
        };
        self.check_implementations_for(ctx, syms, tree, sym, sym);
    }

    /// Java: void checkImplementations(JCTree tree, ClassSymbol origin,
    /// ClassSymbol ic) (Check.java:2463-2490).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the tree for positions;
    ///   - origin: the origin class;
    ///   - ic: the class whose interfaces are checked.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2463-2490.
    #[allow(clippy::collapsible_if)]
    pub fn check_implementations_for(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        origin: SymbolId,
        ic: SymbolId,
    ) {
        let ic_type = ctx.sym(ic).ty.expect("checked class type");
        let mut closure = self.types.closure(ctx, syms, ic_type);
        while closure.non_empty() {
            let lc_type = *closure.head();
            let lc = ctx.typ(lc_type).tsym.expect("closure class symbol");
            if ctx.sym(lc).flags_field & (crate::code::flags::ABSTRACT as i64) != 0 {
                let members = ctx.sym(lc).members().expect("members");
                let symbols = ctx.scope_get_symbols(
                    members,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                for sym in symbols {
                    let flags_value = ctx.sym(sym).flags_field;
                    let abs_meth = ctx.sym(sym).kind == crate::code::kinds::Kind::MTH;
                    let abstract_static = flags_value
                        & ((crate::code::flags::STATIC as i64) | (crate::code::flags::ABSTRACT as i64));
                    if abs_meth && abstract_static == (crate::code::flags::ABSTRACT as i64) {
                        let impl_meth =
                            crate::code::symbol::implementation(ctx, syms, &self.types, sym, origin, false);
                        if let Some(impl_meth) = impl_meth {
                            if impl_meth != sym {
                                let impl_owner = ctx.sym(impl_meth).owner;
                                let origin_interface =
                                    ctx.sym(origin).flags_field & (crate::code::flags::INTERFACE as i64);
                                let impl_owner_interface = impl_owner
                                    .map(|owner| {
                                        ctx.sym(owner).flags_field
                                            & (crate::code::flags::INTERFACE as i64)
                                    })
                                    .unwrap_or(0);
                                if impl_owner_interface == origin_interface {
                                    // don't check if implmeth is in a class, yet origin is
                                    // an interface (Object case).
                                    self.check_override(ctx, syms, tree, impl_meth, sym, origin);
                                }
                            }
                        }
                    }
                }
            }
            closure = closure.tail();
        }
    }

    /// Java: void checkOverride(JCTree tree, Type site, ClassSymbol origin,
    /// MethodSymbol m) (Check.java:2150-2160).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the tree providing positions;
    ///   - site: the checked site type;
    ///   - origin: the origin class;
    ///   - m: the overriding method.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2150-2160.
    #[allow(clippy::collapsible_if)]
    pub fn check_override_at(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        site: TypeId,
        origin: SymbolId,
        m: SymbolId,
    ) {
        let c = ctx.typ(site).tsym.expect("site type symbol");
        let name = ctx.sym(m).name.clone();
        self.complete_for_members(ctx, syms, c);
        let members = ctx.sym(c).members().expect("site members");
        let symbols = ctx.scope_get_symbols_by_name(
            members,
            &name,
            crate::code::scope::LookupKind::RECURSIVE,
            &|_| true,
        );
        for sym in symbols {
            let overrides = crate::code::symbol::overrides(ctx, syms, &self.types, m, sym, origin, false);
            if overrides {
                if ctx.sym(sym).flags_field & (crate::code::flags::ABSTRACT as i64) == 0 {
                    self.check_override(ctx, syms, tree, m, sym, origin);
                }
            }
        }
    }
}

impl Check {
    /// Java: Type checkConstructorRefType(DiagnosticPosition pos, Type t)
    /// (Check.java:741-763).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - t: the constructor reference qualifier type.
    ///
    /// # Returns
    ///   - the checked type (error type on failure).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:741-763.
    pub fn check_constructor_ref_type(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: TypeId,
    ) -> TypeId {
        let t = self.check_class_or_array_type(ctx, syms, pos, t);
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            let flags_value = ctx.sym(tsym).flags_field;
            if flags_value
                & ((crate::code::flags::ABSTRACT as i64) | (crate::code::flags::INTERFACE as i64))
                != 0
            {
                let info = crate::resources::compiler_properties::errors::abstract_cant_be_instantiated(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        crate::comp::resolve::symbol_display_name(ctx, tsym),
                    ),
                );
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
                return self.types.create_error_type_original(ctx, Some(t), syms);
            } else if flags_value & (crate::code::flags::ENUM as i64) != 0 {
                let info = crate::resources::compiler_properties::errors::enum_cant_be_instantiated();
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
                return self.types.create_error_type_original(ctx, Some(t), syms);
            } else {
                return self.check_class_type_no_bounds(ctx, syms, pos, t, true);
            }
        } else if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::ARRAY) {
            let elemtype = match &ctx.typ(t).kind {
                crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => {
                    elemtype.expect("array element type")
                }
                _ => t,
            };
            if !self.types.is_reifiable(ctx, syms, elemtype) {
                let info = crate::resources::compiler_properties::errors::generic_array_creation();
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
                return self.types.create_error_type_original(ctx, Some(t), syms);
            }
        }
        t
    }
}
impl Check {
    /// Java: void checkRaw(JCTree tree, Env<AttrContext> env) (Check.java:1547-1560).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - env: the attribution environment;
    ///   - tree: the type tree.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:1547-1560.
    pub fn check_raw(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
    ) {
        if !self.lint.is_enabled(crate::code::lint::LintCategory::RAW) {
            return;
        }
        let tree_type = match ctx.tree(tree).ty {
            Some(t) => t,
            None => return,
        };
        if !ctx.typ(tree_type).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            return;
        }
        if crate::tree::tree_info::is_diamond(ctx, tree) {
            return;
        }
        let within_anon = {
            let encl_class = env.encl_class;
            let encl_method = env.encl_method;
            let anon = encl_class
                .map(|encl| match &ctx.tree(encl).kind {
                    crate::tree::jc_tree::TreeKind::ClassDecl { name, .. } => name.is_empty(),
                    _ => false,
                })
                .unwrap_or(false);
            let init_method = encl_method
                .map(|method| match &ctx.tree(method).kind {
                    crate::tree::jc_tree::TreeKind::MethodDecl { name, .. } => {
                        name == &self.names.init
                    }
                    _ => false,
                })
                .unwrap_or(false);
            anon && init_method
        };
        if !self.types.is_raw_type_public(ctx, tree_type) || within_anon {
            return;
        }
        let declared = ctx
            .typ(tree_type)
            .tsym
            .and_then(|tsym| ctx.sym(tsym).ty)
            .unwrap_or(tree_type);
        let warning = crate::resources::compiler_properties::warnings::raw_class_use(
            crate::resources::diagnostic_contract::TypeArg::placeholder(
                crate::comp::attr::type_display_of(ctx, tree_type),
            ),
            crate::resources::diagnostic_contract::TypeArg::placeholder(
                crate::comp::attr::type_display_of(ctx, declared),
            ),
        );
        let warning = crate::util::jc_diagnostic::Warning::from(warning);
        crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
            &mut *self.log.borrow_mut(),
            Some(crate::code::lint::LintCategory::RAW),
            Some(std::rc::Rc::new(
                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
            )),
            warning,
        );
    }
}
impl Check {
    /// Java: void checkAccessFromSerializableElement(JCTree tree, boolean isLambda)
    /// (Check.java:2745-2775).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the referenced tree;
    ///   - is_lambda: whether the access is from a serializable lambda.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the tree symbol kind is required to be a value/method.
    ///
    /// Upstream: Check.java:2745-2775.
    pub fn check_access_from_serializable_element(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        is_lambda: bool,
    ) {
        let serial_lint = self.lint.is_enabled(crate::code::lint::LintCategory::SERIAL);
        let serial_suppressed = self.lint.is_suppressed(crate::code::lint::LintCategory::SERIAL);
        if !(self.warn_on_any_access_to_members || (serial_lint && !serial_suppressed && is_lambda)) {
            return;
        }
        let Some(sym) = crate::tree::tree_info::symbol(ctx, tree) else {
            return;
        };
        let sym_kind = ctx.sym(sym).kind;
        if !sym_kind.matches(crate::code::kinds::KindSelector::VAL_MTH) {
            return;
        }
        if sym_kind == crate::code::kinds::Kind::VAR {
            let flags_value = ctx.sym(sym).flags_field;
            let name = ctx.sym(sym).name.clone();
            if (flags_value & crate::code::flags::PARAMETER) != 0
                || crate::code::symbol::Symbol::is_directly_or_indirectly_local(ctx, sym)
                || name == self.names._this
                || name == self.names._super
            {
                return;
            }
        }
        let owner = ctx.sym(sym).owner.expect("symbol owner");
        let owner_type = ctx.sym(owner).ty.expect("owner type");
        if !self.types.is_subtype(ctx, syms, owner_type, syms.serializable_type)
            && self.is_effectively_non_public(ctx, syms, sym)
        {
            let sym_arg = crate::resources::diagnostic_contract::SymbolArg::placeholder(
                crate::comp::resolve::symbol_display_name(ctx, sym),
            );
            if is_lambda {
                if self.belongs_to_restricted_package(ctx, sym) {
                    let warning = crate::resources::compiler_properties::warnings::access_to_member_from_serializable_lambda(sym_arg);
                    let warning = crate::util::jc_diagnostic::Warning::from(warning);
                    crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                        &mut *self.log.borrow_mut(),
                        Some(crate::code::lint::LintCategory::SERIAL),
                        Some(std::rc::Rc::new(
                            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
                        )),
                        warning,
                    );
                }
            } else {
                let warning = crate::resources::compiler_properties::warnings::access_to_member_from_serializable_element(sym_arg);
                let warning = crate::util::jc_diagnostic::Warning::from(warning);
                crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                    &mut *self.log.borrow_mut(),
                    None,
                    Some(std::rc::Rc::new(
                        crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
                    )),
                    warning,
                );
            }
        }
    }

    /// Java: private boolean isEffectivelyNonPublic(Symbol sym) (Check.java:2777-2790).
    fn is_effectively_non_public(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> bool {
        if crate::code::symbol::packge(ctx, sym) == Some(syms.root_package) {
            return false;
        }
        let mut current = sym;
        loop {
            if ctx.sym(current).kind == crate::code::kinds::Kind::PCK {
                break;
            }
            if (ctx.sym(current).flags_field & (crate::code::flags::PUBLIC as i64)) == 0 {
                return true;
            }
            match ctx.sym(current).owner {
                Some(owner) => current = owner,
                None => break,
            }
        }
        false
    }

    /// Java: private boolean belongsToRestrictedPackage(Symbol sym)
    /// (Check.java:2792-2798).
    fn belongs_to_restricted_package(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        sym: SymbolId,
    ) -> bool {
        let pkg = match crate::code::symbol::packge(ctx, sym) {
            Some(pkg) => pkg,
            None => return false,
        };
        let full_name = match &ctx.sym(pkg).kind_data {
            crate::code::symbol::SymbolKind::PackageSymbol { fullname, .. } => {
                fullname.to_java_string().to_string_lossy()
            }
            _ => ctx.sym(pkg).name.to_java_string().to_string_lossy(),
        };
        full_name.starts_with("java.")
            || full_name.starts_with("javax.")
            || full_name.starts_with("sun.")
            || full_name.contains(".internal.")
    }
}
impl Check {
    /// Java: void validate(JCTree tree, Env<AttrContext> env) (Check.java:1397-1399).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the attribution environment;
    ///   - tree: the type expression to validate.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:1397-1399.
    pub fn validate(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
    ) {
        self.validate_with(ctx, syms, env, tree, true);
    }

    /// Java: void validate(JCTree tree, Env<AttrContext> env, boolean checkRaw)
    /// (Check.java:1400-1402).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the attribution environment;
    ///   - tree: the type expression to validate;
    ///   - check_raw: whether the raw-type check is performed.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:1400-1402.
    pub fn validate_with(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
        check_raw: bool,
    ) {
        self.validate_tree(ctx, syms, env, tree, check_raw, true);
    }

    /// Java: Validator.validateTree(JCTree tree, boolean checkRaw, boolean isOuter)
    /// (Check.java:1513-1530).
    fn validate_tree(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
        check_raw: bool,
        is_outer: bool,
    ) {
        self.validate_visit(ctx, syms, env, tree, check_raw, is_outer);
        if check_raw {
            self.check_raw(ctx, env, tree);
        }
    }

    /// Java: Validator tree.accept(this) dispatch (Check.java:1424-1510).
    #[allow(clippy::collapsible_if)]
    fn validate_visit(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
        check_raw: bool,
        is_outer: bool,
    ) {
        match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ArrayTypeTree { elemtype: Some(elemtype) } => {
                let elemtype = *elemtype;
                self.validate_tree(ctx, syms, env, elemtype, check_raw, is_outer);
            }
            crate::tree::jc_tree::TreeKind::TypeApply { clazz, arguments } => {
                let clazz = *clazz.as_ref().expect("type apply clazz");
                let arguments = arguments.clone();
                let tree_type = match ctx.tree(tree).ty {
                    Some(t) => t,
                    None => return,
                };
                if ctx.typ(tree_type).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                    let tsym = ctx.typ(tree_type).tsym.expect("type apply symbol");
                    let tsym_ty = ctx.sym(tsym).ty.expect("type symbol type");
                    let mut forms = crate::code::types::get_type_arguments(ctx, tsym_ty);
                    let incompatible = self.first_incompatible_type_arg(ctx, syms, tree_type);
                    if let Some(incompatible) = incompatible {
                        let mut args_iter = arguments.clone();
                        let mut forms_iter = forms.clone();
                        while args_iter.non_empty() && forms_iter.non_empty() {
                            let arg = *args_iter.head();
                            if ctx.tree(arg).ty == Some(incompatible) {
                                let form = *forms_iter.head();
                                let info = crate::resources::compiler_properties::errors::not_within_bounds(
                                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                                        crate::comp::attr::type_display_of(ctx, incompatible),
                                    ),
                                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                                        crate::comp::attr::type_display_of(ctx, form),
                                    ),
                                );
                                crate::util::abstract_log::AbstractLog::error_at(
                                    &mut *self.log.borrow_mut(),
                                    ctx.tree(arg).pos,
                                    crate::util::jc_diagnostic::Error::from(info),
                                );
                            }
                            args_iter = args_iter.tail();
                            forms_iter = forms_iter.tail();
                        }
                    }
                    forms = crate::code::types::get_type_arguments(ctx, tsym_ty);
                    let flat_name = match &ctx.sym(tsym).kind_data {
                        crate::code::symbol::SymbolKind::ClassSymbol { flatname, .. } => {
                            flatname.clone()
                        }
                        _ => ctx.sym(tsym).name.clone(),
                    };
                    let is_java_lang_class = flat_name == self.names.java_lang_Class;
                    let mut args_iter = arguments.clone();
                    let mut forms_iter = forms;
                    while args_iter.non_empty() && forms_iter.non_empty() {
                        let arg = *args_iter.head();
                        self.validate_tree(
                            ctx,
                            syms,
                            env,
                            arg,
                            !(is_outer && is_java_lang_class),
                            false,
                        );
                        args_iter = args_iter.tail();
                        forms_iter = forms_iter.tail();
                    }
                    let enclosing = ctx.typ(tree_type).get_enclosing_type();
                    if let Some(enclosing) = enclosing {
                        if self.types.is_raw_type_public(ctx, enclosing) {
                            let info = crate::resources::compiler_properties::errors::improperly_formed_type_inner_raw_param();
                            crate::util::abstract_log::AbstractLog::error_at(
                                &mut *self.log.borrow_mut(),
                                ctx.tree(tree).pos,
                                crate::util::jc_diagnostic::Error::from(info),
                            );
                        }
                    }
                    if ctx.tree(clazz).tag == crate::tree::jc_tree::Tag::SELECT {
                        self.visit_select_internal(ctx, syms, env, clazz, check_raw, is_outer);
                    }
                }
            }
            crate::tree::jc_tree::TreeKind::TypeParameter { bounds, .. } => {
                let bounds = bounds.clone();
                let mut current = bounds;
                while current.non_empty() {
                    let bound = *current.head();
                    self.validate_tree(ctx, syms, env, bound, true, is_outer);
                    current = current.tail();
                }
                let tree_ty = match ctx.tree(tree).ty {
                    Some(t) => t,
                    None => return,
                };
                let mut seensofar: std::collections::HashMap<SymbolId, TypeId> =
                    std::collections::HashMap::new();
                self.check_class_bounds(ctx, syms, ctx.tree(tree).pos, &mut seensofar, tree_ty);
            }
            crate::tree::jc_tree::TreeKind::Wildcard { inner: Some(inner), .. } => {
                let inner = *inner;
                self.validate_tree(ctx, syms, env, inner, true, is_outer);
            }
            crate::tree::jc_tree::TreeKind::FieldAccess { selected, .. } => {
                let selected = *selected.as_ref().expect("select qualifier");
                let tree_ty = match ctx.tree(tree).ty {
                    Some(t) => t,
                    None => return,
                };
                if ctx.typ(tree_ty).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                    self.visit_select_internal(ctx, syms, env, tree, check_raw, is_outer);
                    let selected_ty = ctx.tree(selected).ty;
                    let tsym = ctx.typ(tree_ty).tsym.expect("select type symbol");
                    let tsym_ty = ctx.sym(tsym).ty.expect("type symbol type");
                    let typarams = crate::code::types::get_type_arguments(ctx, tsym_ty);
                    if selected_ty
                        .map(|selected_ty| self.types.is_parameterized_type_of(ctx, selected_ty))
                        .unwrap_or(false)
                        && !typarams.is_empty()
                    {
                        let info = crate::resources::compiler_properties::errors::improperly_formed_type_param_missing();
                        crate::util::abstract_log::AbstractLog::error_at(
                            &mut *self.log.borrow_mut(),
                            ctx.tree(tree).pos,
                            crate::util::jc_diagnostic::Error::from(info),
                        );
                    }
                }
            }
            crate::tree::jc_tree::TreeKind::AnnotatedType { underlying_type, .. } => {
                let underlying = *underlying_type.as_ref().expect("annotated type underlying");
                self.validate_visit(ctx, syms, env, underlying, check_raw, is_outer);
            }
            crate::tree::jc_tree::TreeKind::PrimitiveTypeTree { .. } => {
                if let Some(tree_ty) = ctx.tree(tree).ty {
                    if ctx.typ(tree_ty).has_tag(crate::code::type_tag::TypeTag::VOID) {
                        let info = crate::resources::compiler_properties::errors::void_not_allowed_here();
                        crate::util::abstract_log::AbstractLog::error_at(
                            &mut *self.log.borrow_mut(),
                            ctx.tree(tree).pos,
                            crate::util::jc_diagnostic::Error::from(info),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    /// Java: Validator.visitSelectInternal(JCFieldAccess tree)
    /// (Check.java:1490-1505).
    #[allow(clippy::collapsible_if)]
    fn visit_select_internal(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
        check_raw: bool,
        is_outer: bool,
    ) {
        let (selected, tree_ty) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::FieldAccess { selected, .. } => {
                (*selected.as_ref().expect("select qualifier"), ctx.tree(tree).ty)
            }
            _ => return,
        };
        let tree_ty = match tree_ty {
            Some(t) => t,
            None => return,
        };
        if !ctx.typ(tree_ty).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            return;
        }
        let tsym = ctx.typ(tree_ty).tsym.expect("select type symbol");
        let is_static = ctx.sym(tsym).is_static(ctx);
        let selected_parameterized = ctx.tree(selected)
            .ty
            .map(|selected_ty| self.types.is_parameterized_type_of(ctx, selected_ty))
            .unwrap_or(false);
        if is_static && selected_parameterized {
            let info = crate::resources::compiler_properties::errors::cant_select_static_class_from_param_type();
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(tree).pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        } else {
            self.validate_visit(ctx, syms, env, selected, check_raw, is_outer);
        }
    }

    /// Java: void checkClassBounds(DiagnosticPosition pos, Map<TypeSymbol,Type>
    /// seensofar, Type type) (Check.java:2810-2840).
    /// Adapter: public wrapper of checkClassBounds(DiagnosticPosition pos, Type type).
    pub fn check_class_bounds_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t: TypeId,
    ) {
        let mut seensofar: std::collections::HashMap<SymbolId, TypeId> =
            std::collections::HashMap::new();
        self.check_class_bounds(ctx, syms, pos, &mut seensofar, t);
    }

    #[allow(clippy::collapsible_if)]
    fn check_class_bounds(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        seensofar: &mut std::collections::HashMap<SymbolId, TypeId>,
        t: TypeId,
    ) {
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::ERROR) {
            return;
        }
        let mut current = self.types.interfaces(ctx, syms, t);
        while current.non_empty() {
            let it = *current.head();
            if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS)
                && !ctx.typ(it).has_tag(crate::code::type_tag::TypeTag::CLASS)
            {
                current = current.tail();
                continue;
            }
            let it_tsym = ctx.typ(it).tsym.expect("interface symbol");
            let old_it = seensofar.insert(it_tsym, it);
            if let Some(old_it) = old_it {
                let old_params = crate::code::types::allparams_of(ctx, old_it);
                let new_params = crate::code::types::allparams_of(ctx, it);
                let equivalent = self.types.contains_type_equivalent(ctx, syms, &old_params, &new_params);
                if !equivalent {
                    let info = crate::resources::compiler_properties::errors::cant_inherit_diff_arg(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(
                            crate::comp::resolve::symbol_display_name(ctx, it_tsym),
                        ),
                        crate::util::java_string::JavaString::from(
                            type_list_display(ctx, &old_params).as_str(),
                        ),
                        crate::util::java_string::JavaString::from(
                            type_list_display(ctx, &new_params).as_str(),
                        ),
                    );
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.log.borrow_mut(),
                        pos,
                        crate::util::jc_diagnostic::Error::from(info),
                    );
                }
            }
            self.check_class_bounds(ctx, syms, pos, seensofar, it);
            current = current.tail();
        }
        let st = self.types.supertype(ctx, syms, t);
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS)
            && !ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::CLASS)
        {
            return;
        }
        if !ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::NONE) {
            self.check_class_bounds(ctx, syms, pos, seensofar, st);
        }
    }
}

/// Java: Type.toString(List<Type>) display helper for the class-bound error.
fn type_list_display(ctx: &mut crate::compiler_context::CompilerContext, ts: &List<TypeId>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = ts.clone();
    while current.non_empty() {
        parts.push(crate::comp::attr::type_display_of(ctx, *current.head()));
        current = current.tail();
    }
    format!("<{}>", parts.join(","))
}
impl Check {
    /// Java: void checkNonCyclic(DiagnosticPosition pos, Type t) (Check.java:2374-2391).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position (None for the internal entry);
    ///   - t: the type to check.
    ///
    /// # Returns
    ///   - true when the check completed on all attributed classes.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Adapter: Symbol.isCompleted (Symbol.java:232, COMPLETE flag) is not
    /// represented; the completion test uses the UNATTRIBUTED flag (registered).
    /// Upstream: Check.java:2374-2391.
    pub fn check_non_cyclic(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: Option<i32>,
        t: TypeId,
    ) -> bool {
        self.check_non_cyclic_internal(ctx, syms, pos, t)
    }
}impl Check {
    /// Java: void checkNonCyclic(DiagnosticPosition pos, TypeVar t) (Check.java:2393-2395).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - t: the type variable to check.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2393-2410.
    pub fn check_non_cyclic_typevar(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: Option<i32>,
        t: TypeId,
    ) {
        self.check_non_cyclic1(ctx, syms, pos, t, List::nil());
    }

    /// Java: private void checkNonCyclic1(DiagnosticPosition pos, Type t,
    /// List<TypeVar> seen) (Check.java:2412-2428).
    fn check_non_cyclic1(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: Option<i32>,
        t: TypeId,
        seen: List<TypeId>,
    ) {
        let tsym = ctx.typ(t).tsym;
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::TYPEVAR)
            && tsym.map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::UNATTRIBUTED as i64) != 0).unwrap_or(false)
        {
            return;
        }
        let mut contains = false;
        let mut current = seen.clone();
        while current.non_empty() {
            if *current.head() == t {
                contains = true;
            }
            current = current.tail();
        }
        if contains {
            // Java: tv.setUpperBound(types.createErrorType(t));
            let error = self.types.create_error_type_original(ctx, Some(t), syms);
            if let crate::code::type_::TypeKindData::TypeVar { bound: slot, .. } = &mut ctx.typ_mut(t).kind {
                *slot = Some(error);
            }
            if let Some(pos) = pos {
                let info = crate::resources::compiler_properties::errors::cyclic_inheritance(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        crate::comp::attr::type_display_of(ctx, t),
                    ),
                );
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
            }
        } else if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
            let seen = seen.prepend(t);
            let bounds = self.types.get_bounds(ctx, syms, t);
            let mut current = bounds;
            while current.non_empty() {
                let bound = *current.head();
                self.check_non_cyclic1(ctx, syms, pos, bound, seen.clone());
                current = current.tail();
            }
        }
    }
}impl Check {
    /// Java: private boolean checkNonCyclicInternal(DiagnosticPosition pos, Type t)
    /// (Check.java:2430-2470).
    #[allow(clippy::collapsible_if)]
    fn check_non_cyclic_internal(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: Option<i32>,
        t: TypeId,
    ) -> bool {
        let mut complete = true;
        let c = match ctx.typ(t).tsym {
            Some(c) => c,
            None => return true,
        };
        if (ctx.sym(c).flags_field & crate::code::flags::ACYCLIC as i64) != 0 {
            return true;
        }
        if (ctx.sym(c).flags_field & crate::code::flags::LOCKED as i64) != 0 {
            self.note_cyclic(ctx, syms, pos, c);
        } else if !ctx.sym(c).ty.map(|c_ty| ctx.typ(c_ty).has_tag(crate::code::type_tag::TypeTag::ERROR)).unwrap_or(false) {
            ctx.sym_mut(c).flags_field |= crate::code::flags::LOCKED as i64;
            if let Some(c_ty) = ctx.sym(c).ty {
                if ctx.typ(c_ty).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                    let interfaces = self.types.interfaces(ctx, syms, c_ty);
                    let mut current = interfaces;
                    while current.non_empty() {
                        complete &= self.check_non_cyclic_internal(ctx, syms, pos, *current.head());
                        current = current.tail();
                    }
                    let (supertype_field, owner) = {
                        let supertype_field = match &ctx.typ(c_ty).kind {
                            crate::code::type_::TypeKindData::ClassType { supertype_field, .. } => *supertype_field,
                            _ => None,
                        };
                        (supertype_field, ctx.sym(c).owner)
                    };
                    if let Some(st) = supertype_field {
                        if ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                            complete &= self.check_non_cyclic_internal(ctx, syms, pos, st);
                        }
                    }
                    if let Some(owner) = owner {
                        if ctx.sym(owner).kind == crate::code::kinds::Kind::TYP {
                            if let Some(owner_ty) = ctx.sym(owner).ty {
                                complete &= self.check_non_cyclic_internal(ctx, syms, pos, owner_ty);
                            }
                        }
                    }
                }
            }
            ctx.sym_mut(c).flags_field &= !(crate::code::flags::LOCKED as i64);
        }
        if complete {
            // Adapter: isCompleted() (COMPLETE flag) is not represented; the
            // completion test uses the UNATTRIBUTED flag (registered).
            complete = (ctx.sym(c).flags_field & crate::code::flags::UNATTRIBUTED as i64) == 0;
        }
        if complete {
            ctx.sym_mut(c).flags_field |= crate::code::flags::ACYCLIC as i64;
        }
        complete
    }

    /// Java: private void noteCyclic(DiagnosticPosition pos, ClassSymbol c)
    /// (Check.java:2472-2490).
    #[allow(clippy::collapsible_if)]
    fn note_cyclic(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: Option<i32>,
        c: SymbolId,
    ) {
        if let Some(pos) = pos {
            let info = crate::resources::compiler_properties::errors::cyclic_inheritance(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    crate::comp::resolve::symbol_display_name(ctx, c),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
        let c_ty = ctx.sym(c).ty.expect("class symbol type");
        let interfaces = self.types.interfaces(ctx, syms, c_ty);
        let mut new_interfaces: List<TypeId> = List::nil();
        let mut current = interfaces;
        while current.non_empty() {
            let interface = *current.head();
            let error = self.types.create_error_type_original(ctx, Some(interface), syms);
            new_interfaces = new_interfaces.prepend(error);
            current = current.tail();
        }
        let supertype = self.types.supertype(ctx, syms, c_ty);
        let supertype_field_value = if ctx
            .typ(supertype)
            .has_tag(crate::code::type_tag::TypeTag::CLASS)
        {
            Some(self.types.create_error_type_original(ctx, Some(supertype), syms))
        } else {
            None
        };
        if let crate::code::type_::TypeKindData::ClassType {
            interfaces_field,
            supertype_field,
            ..
        } = &mut ctx.typ_mut(c_ty).kind
        {
            *interfaces_field = Some(new_interfaces);
            if supertype_field_value.is_some() {
                *supertype_field = supertype_field_value;
            }
        }
        let error = self.types.create_error_type_original(ctx, Some(c_ty), syms);
        ctx.sym_mut(c).ty = Some(error);
        ctx.sym_mut(c).flags_field |= crate::code::flags::ACYCLIC as i64;
    }
}impl Check {
    /// Java: void checkAllDefined(DiagnosticPosition pos, ClassSymbol c)
    /// (Check.java:2245-2265).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - envs: the type environments (Types.enter.getEnv);
    ///   - pos: the diagnostic position;
    ///   - c: the class symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2245-2265.
    pub fn check_all_defined(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        envs: &crate::comp::type_envs::TypeEnvs,
        pos: i32,
        c: SymbolId,
    ) {
        let undef = self.types.first_unimplemented_abstract(
            ctx,
            syms,
            envs,
            crate::code::source::Feature::DEFAULT_METHODS
                .allowed_in_source(crate::code::source::Source::JDK17),
            c,
        );
        let undef = match undef {
            Some(undef) => undef,
            None => return,
        };
        let c_ty = ctx.sym(c).ty.expect("class symbol type");
        let undef_flags = ctx.sym(undef).flags_field;
        let undef_name = ctx.sym(undef).name.clone();
        let undef_owner = ctx.sym(undef).owner;
        let member_ty = self.types.member_type(ctx, syms, c_ty, undef);
        let undef1 = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::MTH,
            undef_flags,
            undef_name,
            Some(member_ty),
            undef_owner,
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
                original_enclosing_method: None,
            },
        ));
        let location = ctx.sym(undef1).location();
        let info = crate::resources::compiler_properties::errors::does_not_override_abstract(
            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                crate::comp::resolve::symbol_display_name(ctx, c),
            ),
            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                crate::comp::resolve::symbol_display_name(ctx, undef1),
            ),
            crate::resources::diagnostic_contract::SymbolArg::placeholder(match location {
                Some(location) => crate::comp::resolve::symbol_display_name(ctx, location),
                None => String::new(),
            }),
        );
        crate::util::abstract_log::AbstractLog::error_at(
            &mut *self.log.borrow_mut(),
            pos,
            crate::util::jc_diagnostic::Error::from(info),
        );
    }
}impl Check {
    /// Java: void checkNonCyclicElements(JCClassDecl tree) (Check.java:3610-3623).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the annotation class declaration.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3610-3623.
    pub fn check_non_cyclic_elements(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        let sym = match crate::tree::tree_info::symbol(ctx, tree) {
            Some(sym) => sym,
            None => match &ctx.tree(tree).kind {
                crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                _ => return,
            },
        };
        if (ctx.sym(sym).flags_field & crate::code::flags::ANNOTATION as i64) == 0 {
            return;
        }
        assert!(
            (ctx.sym(sym).flags_field & crate::code::flags::LOCKED as i64) == 0,
            "AssertionError: annotation class already locked"
        );
        ctx.sym_mut(sym).flags_field |= crate::code::flags::LOCKED as i64;
        let defs = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => List::nil(),
        };
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            current = current.tail();
            if !ctx.tree(def).has_tag(crate::tree::jc_tree::Tag::METHODDEF) {
                continue;
            }
            let restype = match &ctx.tree(def).kind {
                crate::tree::jc_tree::TreeKind::MethodDecl { restype: Some(restype), .. } => match ctx.tree(*restype).ty {
                    Some(t) => t,
                    None => continue,
                },
                _ => continue,
            };
            self.check_annotation_res_type(ctx, ctx.tree(def).pos, restype);
        }
        ctx.sym_mut(sym).flags_field &= !(crate::code::flags::LOCKED as i64);
        ctx.sym_mut(sym).flags_field |= crate::code::flags::ACYCLIC_ANN;
    }

    /// Java: void checkNonCyclicElementsInternal(DiagnosticPosition pos,
    /// TypeSymbol tsym) (Check.java:3626-3648).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - tsym: the annotation type symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3626-3648.
    fn check_non_cyclic_elements_internal(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        pos: i32,
        tsym: SymbolId,
    ) {
        if (ctx.sym(tsym).flags_field & crate::code::flags::ACYCLIC_ANN) != 0 {
            return;
        }
        if (ctx.sym(tsym).flags_field & crate::code::flags::LOCKED as i64) != 0 {
            let info = crate::resources::compiler_properties::errors::cyclic_annotation_element(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, tsym),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
            return;
        }
        ctx.sym_mut(tsym).flags_field |= crate::code::flags::LOCKED as i64;
        let members = match &ctx.sym(tsym).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        if let Some(members) = members {
            let symbols = ctx.scope_get_symbols(
                members,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for member in symbols {
                if ctx.sym(member).kind != crate::code::kinds::Kind::MTH {
                    continue;
                }
                let member_ty = match ctx.sym(member).ty {
                    Some(t) => t,
                    None => continue,
                };
                let return_ty = self.types.get_return_type_public(ctx, member_ty);
                self.check_annotation_res_type(ctx, pos, return_ty);
            }
        }
        ctx.sym_mut(tsym).flags_field &= !(crate::code::flags::LOCKED as i64);
        ctx.sym_mut(tsym).flags_field |= crate::code::flags::ACYCLIC_ANN;
    }

    /// Java: void checkAnnotationResType(DiagnosticPosition pos, Type type)
    /// (Check.java:3650-3665).
    #[allow(clippy::collapsible_if)]
    fn check_annotation_res_type(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        pos: i32,
        ty: TypeId,
    ) {
        match ctx.typ(ty).get_tag() {
            crate::code::type_tag::TypeTag::CLASS => {
                if let Some(tsym) = ctx.typ(ty).tsym {
                    if (ctx.sym(tsym).flags_field & crate::code::flags::ANNOTATION as i64) != 0 {
                        self.check_non_cyclic_elements_internal(ctx, pos, tsym);
                    }
                }
            }
            crate::code::type_tag::TypeTag::ARRAY => {
                let elem = match &ctx.typ(ty).kind {
                    crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => *elemtype,
                    _ => None,
                };
                if let Some(elem) = elem {
                    self.check_annotation_res_type(ctx, pos, elem);
                }
            }
            _ => {}
        }
    }

    /// Java: void checkCyclicConstructors(JCClassDecl tree) (Check.java:3667-3690).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the class declaration.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3667-3690.
    pub fn check_cyclic_constructors(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        let defs = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => return,
        };
        let mut call_map: std::collections::HashMap<SymbolId, SymbolId> =
            std::collections::HashMap::new();
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            current = current.tail();
            let app = match crate::tree::tree_info::first_constructor_call(ctx, def) {
                Some(app) => app,
                None => continue,
            };
            let meth_sym = match &ctx.tree(def).kind {
                crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
                _ => continue,
            };
            let app_meth = match &ctx.tree(app).kind {
                crate::tree::jc_tree::TreeKind::MethodInvocation { meth: Some(meth), .. } => *meth,
                _ => continue,
            };
            let app_name = match crate::tree::tree_info::name(ctx, app_meth) {
                Some(name) => name,
                None => continue,
            };
            if app_name == self.names._this {
                if let Some(callee) = crate::tree::tree_info::symbol(ctx, app_meth) {
                    call_map.insert(meth_sym, callee);
                }
            } else {
                ctx.sym_mut(meth_sym).flags_field |= crate::code::flags::ACYCLIC as i64;
            }
        }
        let callers: Vec<SymbolId> = call_map.keys().copied().collect();
        for caller in callers {
            self.check_cyclic_constructor(ctx, tree, Some(caller), &mut call_map);
        }
    }

    /// Java: private void checkCyclicConstructor(JCClassDecl tree, Symbol ctor,
    /// Map<Symbol,Symbol> callMap) (Check.java:3695-3715).
    fn check_cyclic_constructor(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
        ctor: Option<SymbolId>,
        call_map: &mut std::collections::HashMap<SymbolId, SymbolId>,
    ) {
        let ctor = match ctor {
            Some(ctor) => ctor,
            None => return,
        };
        if (ctx.sym(ctor).flags_field & crate::code::flags::ACYCLIC as i64) == 0 {
            if (ctx.sym(ctor).flags_field & crate::code::flags::LOCKED as i64) != 0 {
                let pos = crate::tree::tree_info::diagnostic_position_for(ctx, ctor, tree);
                let info = crate::resources::compiler_properties::errors::recursive_ctor_invocation();
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
            } else {
                ctx.sym_mut(ctor).flags_field |= crate::code::flags::LOCKED as i64;
                let next = call_map.remove(&ctor);
                self.check_cyclic_constructor(ctx, tree, next, call_map);
                ctx.sym_mut(ctor).flags_field &= !(crate::code::flags::LOCKED as i64);
            }
            ctx.sym_mut(ctor).flags_field |= crate::code::flags::ACYCLIC as i64;
        }
    }
}impl Check {
    /// Java: void checkDeprecatedAnnotation(DiagnosticPosition pos, Symbol s)
    /// (Check.java:3530-3550).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - pos: the diagnostic position;
    ///   - s: the symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3530-3550.
    #[allow(clippy::collapsible_if)]
    pub fn check_deprecated_annotation(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        s: SymbolId,
    ) {
        let deprecatable = ctx.sym(s).is_deprecatable_via_annotation(ctx);
        let deprecated_type_erroneous = ctx
            .typ(syms.deprecated_type)
            .has_tag(crate::code::type_tag::TypeTag::ERROR);
        let deprecated_anno = match ctx.typ(syms.deprecated_type).tsym {
            Some(deprecated_tsym) => ctx.sym(s).attribute(ctx, deprecated_tsym),
            None => None,
        };
        if self.lint.is_enabled(crate::code::lint::LintCategory::DEP_ANN)
            && deprecatable
            && (ctx.sym(s).flags_field & crate::code::flags::DEPRECATED as i64) != 0
            && !deprecated_type_erroneous
            && deprecated_anno.is_none()
        {
            let warning = crate::util::jc_diagnostic::Warning::from(
                crate::resources::compiler_properties::warnings::missing_deprecated_annotation(),
            );
            crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                &mut *self.log.borrow_mut(),
                Some(crate::code::lint::LintCategory::DEP_ANN),
                Some(std::rc::Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                )),
                warning,
            );
        }
        if self.lint.is_enabled(crate::code::lint::LintCategory::DEPRECATION) && !deprecatable {
            if !deprecated_type_erroneous && deprecated_anno.is_some() {
                let kind = crate::code::kinds::kind_name(ctx.sym(s), ctx);
                let warning = crate::util::jc_diagnostic::Warning::from(
                    crate::resources::compiler_properties::warnings::deprecated_annotation_has_no_effect(
                        crate::resources::diagnostic_contract::KindNameArg::placeholder(
                            kind.to_string(),
                        ),
                    ),
                );
                crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                    &mut *self.log.borrow_mut(),
                    Some(crate::code::lint::LintCategory::DEPRECATION),
                    Some(std::rc::Rc::new(
                        crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                    )),
                    warning,
                );
            }
        }
    }

    /// Java: public void checkClassOverrideEqualsAndHashIfNeeded(DiagnosticPosition
    /// pos, ClassSymbol someClass) (Check.java:2164-2199).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - some_class: the class symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2164-2199.
    #[allow(clippy::collapsible_if)]
    pub fn check_class_override_equals_and_hash_if_needed(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        some_class: SymbolId,
    ) {
        let object_tsym = ctx.typ(syms.object_type).tsym.expect("Object symbol");
        if some_class == object_tsym
            || ctx.sym(some_class).is_interface()
            || ctx.sym(some_class).is_enum()
            || (ctx.sym(some_class).flags_field & crate::code::flags::ANNOTATION as i64) != 0
            || (ctx.sym(some_class).flags_field & crate::code::flags::ABSTRACT as i64) != 0
        {
            return;
        }
        if ctx.sym(some_class).is_anonymous() {
            let class_ty = ctx.sym(some_class).ty.expect("class type");
            let interfaces = self.types.interfaces(ctx, syms, class_ty);
            if interfaces.non_empty() {
                let comparator_tsym = ctx.typ(syms.comparator_type).tsym;
                let first_tsym = ctx.typ(*interfaces.head()).tsym;
                if first_tsym.is_some() && first_tsym == comparator_tsym {
                    return;
                }
            }
        }
        if !self.lint.is_enabled(crate::code::lint::LintCategory::OVERRIDES) {
            return;
        }
        let object_members = match &ctx.sym(object_tsym).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        let object_members = match object_members {
            Some(members) => members,
            None => return,
        };
        let equals_name = self.names.equals.clone();
        let hash_code_name = self.names.hashCode.clone();
        let equals_candidates = ctx.scope_get_symbols_by_name(
            object_members,
            &equals_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let hash_candidates = ctx.scope_get_symbols_by_name(
            object_members,
            &hash_code_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let equals_at_object = match equals_candidates.first() {
            Some(sym) => *sym,
            None => return,
        };
        let hash_code_at_object = match hash_candidates.first() {
            Some(sym) => *sym,
            None => return,
        };
        let equals_impl = self.types.implementation(
            ctx,
            syms,
            equals_at_object,
            some_class,
            false,
            crate::code::types::ImplementationFilterKind::EqualsHasCode,
        );
        let overrides_equals = equals_impl.map(|impl_sym| {
            ctx.sym(impl_sym).owner == Some(some_class)
        })
        .unwrap_or(false);
        let hash_impl = self.types.implementation(
            ctx,
            syms,
            hash_code_at_object,
            some_class,
            false,
            crate::code::types::ImplementationFilterKind::EqualsHasCode,
        );
        let overrides_hash_code = hash_impl != Some(hash_code_at_object);
        if overrides_equals && !overrides_hash_code {
            let warning = crate::util::jc_diagnostic::Warning::from(
                crate::resources::compiler_properties::warnings::override_equals_but_not_hashcode(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        crate::comp::resolve::symbol_display_name(ctx, some_class),
                    ),
                ),
            );
            crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                &mut *self.log.borrow_mut(),
                Some(crate::code::lint::LintCategory::OVERRIDES),
                Some(std::rc::Rc::new(
                    crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                )),
                warning,
            );
        }
    }

    /// Java: void checkFunctionalInterface(JCClassDecl tree, ClassSymbol cs)
    /// (Check.java:3975-3998).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the class declaration;
    ///   - cs: the class symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3975-3998.
    #[allow(clippy::collapsible_if)]
    pub fn check_functional_interface(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        cs: SymbolId,
    ) {
        let functional_tsym = match ctx.typ(syms.functional_interface_type).tsym {
            Some(tsym) => tsym,
            None => return,
        };
        let functional_anno = ctx.sym(cs).attribute(ctx, functional_tsym);
        if functional_anno.is_none() {
            return;
        }
        let diags = std::rc::Rc::clone(&self.diags);
        if let Err(ex) = self
            .types
            .find_descriptor_symbol(ctx, syms, &diags, self, cs)
        {
            let mut pos = ctx.tree(tree).pos;
            let mods = match &ctx.tree(tree).kind {
                crate::tree::jc_tree::TreeKind::ClassDecl { mods, .. } => *mods,
                _ => None,
            };
            if let Some(mods) = mods {
                let annotations = match &ctx.tree(mods).kind {
                    crate::tree::jc_tree::TreeKind::Modifiers { annotations, .. } => annotations.clone(),
                    _ => List::nil(),
                };
                let mut current = annotations;
                while current.non_empty() {
                    let anno = *current.head();
                    current = current.tail();
                    let anno_type_tree = match &ctx.tree(anno).kind {
                        crate::tree::jc_tree::TreeKind::Annotation {
                            annotation_type: Some(annotation_type),
                            ..
                        } => Some(*annotation_type),
                        _ => None,
                    };
                    if let Some(anno_type_tree) = anno_type_tree {
                        if ctx.tree(anno_type_tree).ty.and_then(|ty| ctx.typ(ty).tsym)
                            == Some(functional_tsym)
                        {
                            pos = ctx.tree(anno).pos;
                            break;
                        }
                    }
                }
            }
            let details = ex
                .diagnostic
                .map(|diagnostic| {
                    crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                        std::rc::Rc::new(diagnostic),
                    )
                })
                .unwrap_or_else(|| {
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(String::new())
                });
            let info = crate::resources::compiler_properties::errors::bad_functional_intf_anno1(details);
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }
}impl Check {
    /// Java: void validateRepeatable(ClassSymbol s, Attribute.Compound a,
    /// DiagnosticPosition pos) (Check.java:3100-3125).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - s: the annotation type symbol;
    ///   - a: the @Repeatable annotation attribute.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3100-3125.
    pub fn validate_repeatable(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        s: SymbolId,
        a: AttributeId,
    ) {
        let a_ty = ctx.attr(a).ty.expect("repeatable attribute type");
        assert!(
            self.types.is_same_type(ctx, syms, a_ty, syms.repeatable_type),
            "AssertionError: @Repeatable annotation type"
        );
        let values = match &ctx.attr(a).kind {
            crate::code::attribute::AttributeKind::Compound { values, .. } => values.clone(),
            _ => return,
        };
        let mut t: Option<TypeId> = None;
        if values.non_empty() {
            let head = values.head().clone();
            assert!(
                ctx.sym(head.fst).name == self.names.value,
                "AssertionError: @Repeatable value method"
            );
            t = match &ctx.attr(head.snd).kind {
                crate::code::attribute::AttributeKind::Class { class_type } => *class_type,
                _ => None,
            };
        }
        let t = match t {
            Some(t) => t,
            None => return,
        };
        let t_tsym = match ctx.typ(t).tsym {
            Some(tsym) => tsym,
            None => return,
        };
        self.validate_repeatable_value(ctx, syms, t_tsym, s, pos);
        self.validate_repeatable_retention(ctx, syms, t_tsym, s, pos);
        self.validate_repeatable_documented(ctx, syms, t_tsym, s, pos);
        self.validate_repeatable_inherited(ctx, syms, t_tsym, s, pos);
        self.validate_repeatable_target(ctx, syms, t_tsym, s, pos);
        self.validate_repeatable_default(ctx, t_tsym, pos);
    }

    /// Java: private void validateValue(TypeSymbol container, TypeSymbol
    /// contained, DiagnosticPosition pos) (Check.java:3127-3140).
    fn validate_repeatable_value(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        container: SymbolId,
        contained: SymbolId,
        pos: i32,
    ) {
        let members = match &ctx.sym(container).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        let value_name = self.names.value.clone();
        let sym = members.and_then(|members| {
            ctx.scope_get_symbols_by_name(
                members,
                &value_name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            )
            .first()
            .copied()
        });
        let sym = match sym {
            Some(sym) if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH => sym,
            _ => {
                let info = crate::resources::compiler_properties::errors::invalid_repeatable_annotation_no_value(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        crate::comp::resolve::symbol_display_name(ctx, container),
                    ),
                );
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
                return;
            }
        };
        let method_ty = ctx.sym(sym).ty.expect("value method type");
        let ret = self.types.get_return_type_public(ctx, method_ty);
        let contained_ty = ctx.sym(contained).ty.expect("contained annotation type");
        let expected_array = self.types.make_array_type(ctx, syms, contained_ty);
        let ok = ctx.typ(ret).has_tag(crate::code::type_tag::TypeTag::ARRAY)
            && {
                let elem = match &ctx.typ(ret).kind {
                    crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => *elemtype,
                    _ => None,
                };
                elem.map(|elem| self.types.is_same_type(ctx, syms, elem, contained_ty))
                    .unwrap_or(false)
            };
        if !ok {
            let info = crate::resources::compiler_properties::errors::invalid_repeatable_annotation_value_return(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    crate::comp::resolve::symbol_display_name(ctx, container),
                ),
                crate::resources::diagnostic_contract::TypeArg::placeholder(
                    crate::comp::attr::type_display_of(ctx, ret),
                ),
                crate::resources::diagnostic_contract::TypeArg::placeholder(
                    crate::comp::attr::type_display_of(ctx, expected_array),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

    /// Java: private void validateRetention(TypeSymbol container, TypeSymbol
    /// contained, DiagnosticPosition pos) (Check.java:3142-3170).
    #[allow(clippy::collapsible_match, clippy::if_same_then_else)]
    fn validate_repeatable_retention(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        container: SymbolId,
        contained: SymbolId,
        pos: i32,
    ) {
        use crate::code::attribute::RetentionPolicy;
        let container_retention = self.types.get_retention(ctx, syms, container);
        let contained_retention = self.types.get_retention(ctx, syms, contained);
        let mut error = false;
        match contained_retention {
            RetentionPolicy::RUNTIME => {
                if container_retention != RetentionPolicy::RUNTIME {
                    error = true;
                }
            }
            RetentionPolicy::CLASS => {
                if container_retention == RetentionPolicy::SOURCE {
                    error = true;
                }
            }
            _ => {}
        }
        if error {
            let info = crate::resources::compiler_properties::errors::invalid_repeatable_annotation_retention(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, container),
                ),
                crate::util::java_string::JavaString::from(retention_name(container_retention)),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, contained),
                ),
                crate::util::java_string::JavaString::from(retention_name(contained_retention)),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

    /// Java: private void validateDocumented(Symbol container, Symbol contained,
    /// DiagnosticPosition pos) (Check.java:3172-3178).
    fn validate_repeatable_documented(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        container: SymbolId,
        contained: SymbolId,
        pos: i32,
    ) {
        let Some(documented_tsym) = ctx.typ(syms.documented_type).tsym else {
            return;
        };
        if ctx.sym(contained).attribute(ctx, documented_tsym).is_some()
            && ctx.sym(container).attribute(ctx, documented_tsym).is_none()
        {
            let info = crate::resources::compiler_properties::errors::invalid_repeatable_annotation_not_documented(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, container),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, contained),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

    /// Java: private void validateInherited(Symbol container, Symbol contained,
    /// DiagnosticPosition pos) (Check.java:3180-3186).
    fn validate_repeatable_inherited(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        container: SymbolId,
        contained: SymbolId,
        pos: i32,
    ) {
        let Some(inherited_tsym) = ctx.typ(syms.inherited_type).tsym else {
            return;
        };
        if ctx.sym(contained).attribute(ctx, inherited_tsym).is_some()
            && ctx.sym(container).attribute(ctx, inherited_tsym).is_none()
        {
            let info = crate::resources::compiler_properties::errors::invalid_repeatable_annotation_not_inherited(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, container),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, contained),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

    /// Java: private void validateTarget(TypeSymbol container, TypeSymbol
    /// contained, DiagnosticPosition pos) (Check.java:3188-3230).
    fn validate_repeatable_target(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        container: SymbolId,
        contained: SymbolId,
        pos: i32,
    ) {
        let container_targets = match self.get_attribute_target_attribute(ctx, syms, container) {
            Ok(Some(arr)) => self.target_set_from_array(ctx, arr),
            _ => self.default_target_meta_info(),
        };
        let contained_targets = match self.get_attribute_target_attribute(ctx, syms, contained) {
            Ok(Some(arr)) => self.target_set_from_array(ctx, arr),
            _ => self.default_target_meta_info(),
        };
        if !is_target_subset_of(&container_targets, &contained_targets, ctx, syms, &self.names) {
            let info = crate::resources::compiler_properties::errors::invalid_repeatable_annotation_incompatible_target(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, container),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    crate::comp::resolve::symbol_display_name(ctx, contained),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

    /// Adapter: the ElementType names of an Attribute.Array target attribute.
    fn target_set_from_array(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        arr: AttributeId,
    ) -> Vec<crate::util::name::Name> {
        let values = match &ctx.attr(arr).kind {
            crate::code::attribute::AttributeKind::Array { values } => values.clone(),
            _ => return Vec::new(),
        };
        let mut targets: Vec<crate::util::name::Name> = Vec::new();
        for value in values {
            if let crate::code::attribute::AttributeKind::Enum { value: Some(enum_sym) } =
                &ctx.attr(value).kind
            {
                targets.push(ctx.sym(*enum_sym).name.clone());
            }
        }
        targets
    }

    /// Java: private void validateDefault(Symbol container, DiagnosticPosition pos)
    /// (Check.java:3270-3285).
    fn validate_repeatable_default(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        container: SymbolId,
        pos: i32,
    ) {
        let members = match &ctx.sym(container).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        let members = match members {
            Some(members) => members,
            None => return,
        };
        let symbols = ctx.scope_get_symbols(members, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
        for elm in symbols {
            if ctx.sym(elm).name != self.names.value && ctx.sym(elm).kind == crate::code::kinds::Kind::MTH {
                let default_value = match &ctx.sym(elm).kind_data {
                    crate::code::symbol::SymbolKind::MethodSymbol { default_value, .. } => *default_value,
                    _ => None,
                };
                if default_value.is_none() {
                    let info = crate::resources::compiler_properties::errors::invalid_repeatable_annotation_elem_nondefault(
                        crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                            crate::comp::resolve::symbol_display_name(ctx, container),
                        ),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(
                            crate::comp::resolve::symbol_display_name(ctx, elm),
                        ),
                    );
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.log.borrow_mut(),
                        pos,
                        crate::util::jc_diagnostic::Error::from(info),
                    );
                }
            }
        }
    }
}

/// Adapter: RetentionPolicy name for the diagnostic text.
fn retention_name(policy: crate::code::attribute::RetentionPolicy) -> &'static str {
    match policy {
        crate::code::attribute::RetentionPolicy::SOURCE => "SOURCE",
        crate::code::attribute::RetentionPolicy::CLASS => "CLASS",
        crate::code::attribute::RetentionPolicy::RUNTIME => "RUNTIME",
    }
}

/// Java: private boolean isTargetSubsetOf(Set<Name> s, Set<Name> t)
/// (Check.java:3245-3268).
fn is_target_subset_of(
    s: &[crate::util::name::Name],
    t: &[crate::util::name::Name],
    ctx: &mut crate::compiler_context::CompilerContext,
    _syms: &crate::code::symtab::Symtab,
    names: &crate::util::names::Names,
) -> bool {
    let _ = ctx;
    for n2 in s {
        let mut current_element_ok = false;
        for n1 in t {
            if n1 == n2
                || (n1 == &names.TYPE && n2 == &names.ANNOTATION_TYPE)
                || (n1 == &names.TYPE_USE
                    && (n2 == &names.TYPE
                        || n2 == &names.ANNOTATION_TYPE
                        || n2 == &names.TYPE_PARAMETER))
            {
                current_element_ok = true;
                break;
            }
        }
        if !current_element_ok {
            return false;
        }
    }
    true
}
impl Check {
    /// Java: public void checkCompatibleConcretes(DiagnosticPosition pos, Type site)
    /// (Check.java:1928-1980).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - site: the class type to check.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:1928-1980.
    #[allow(clippy::too_many_lines)]
    pub fn check_compatible_concretes(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        site: TypeId,
    ) {
        let sup = self.types.supertype(ctx, syms, site);
        if !ctx.typ(sup).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            return;
        }
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        let mut t1 = sup;
        while ctx.typ(t1).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            let t1_tsym = ctx.typ(t1).tsym.expect("t1 symbol");
            let t1_tsym_ty = ctx.sym(t1_tsym).ty.expect("t1 symbol type");
            if !self.types.is_parameterized_type_of(ctx, t1_tsym_ty) {
                break;
            }
            let t1_members = match &ctx.sym(t1_tsym).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
                _ => None,
            };
            if let Some(t1_members) = t1_members {
                let s1_symbols = ctx.scope_get_symbols(t1_members, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
                for s1 in s1_symbols {
                    let s1_flags = ctx.sym(s1).flags_field;
                    let skip_flags = crate::code::flags::STATIC as i64
                        | crate::code::flags::SYNTHETIC as i64
                        | crate::code::flags::BRIDGE;
                    if ctx.sym(s1).kind != crate::code::kinds::Kind::MTH
                        || (s1_flags & skip_flags) != 0
                        || !crate::code::symbol::is_inherited_in(ctx, syms, &self.types, s1, site_tsym)
                    {
                        continue;
                    }
                    let implementation = crate::code::symbol::implementation(ctx, syms, &self.types, s1, site_tsym, true);
                    if implementation != Some(s1) {
                        continue;
                    }
                    let st1 = self.types.member_type(ctx, syms, t1, s1);
                    let s1_args_length = list_length(self.types.get_parameter_types_public(ctx, st1));
                    if st1 == ctx.sym(s1).ty.expect("s1 type") {
                        continue;
                    }
                    let mut t2 = sup;
                    while ctx.typ(t2).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                        let t2_tsym = ctx.typ(t2).tsym.expect("t2 symbol");
                        let t2_members = match &ctx.sym(t2_tsym).kind_data {
                            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
                            _ => None,
                        };
                        if let Some(t2_members) = t2_members {
                            let s1_name = ctx.sym(s1).name.clone();
                            let s2_symbols = ctx.scope_get_symbols_by_name(t2_members, &s1_name, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
                            for s2 in s2_symbols {
                                let s2_flags = ctx.sym(s2).flags_field;
                                if s2 == s1
                                    || ctx.sym(s2).kind != crate::code::kinds::Kind::MTH
                                    || (s2_flags & skip_flags) != 0
                                {
                                    continue;
                                }
                                let s2_ty = ctx.sym(s2).ty.expect("s2 type");
                                if list_length(self.types.get_parameter_types_public(ctx, s2_ty)) != s1_args_length {
                                    continue;
                                }
                                if !crate::code::symbol::is_inherited_in(ctx, syms, &self.types, s2, site_tsym) {
                                    continue;
                                }
                                if crate::code::symbol::implementation(ctx, syms, &self.types, s2, site_tsym, true) != Some(s2) {
                                    continue;
                                }
                                let st2 = self.types.member_type(ctx, syms, t2, s2);
                                if self.types.override_equivalent(ctx, syms, st1, st2) {
                                    let info = crate::resources::compiler_properties::errors::concrete_inheritance_conflict(
                                        crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                            crate::comp::resolve::symbol_display_name(ctx, s1),
                                        ),
                                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                                            crate::comp::attr::type_display_of(ctx, t1),
                                        ),
                                        crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                            crate::comp::resolve::symbol_display_name(ctx, s2),
                                        ),
                                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                                            crate::comp::attr::type_display_of(ctx, t2),
                                        ),
                                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                                            crate::comp::attr::type_display_of(ctx, sup),
                                        ),
                                    );
                                    crate::util::abstract_log::AbstractLog::error_at(
                                        &mut *self.log.borrow_mut(),
                                        pos,
                                        crate::util::jc_diagnostic::Error::from(info),
                                    );
                                }
                            }
                        }
                        t2 = self.types.supertype(ctx, syms, t2);
                    }
                }
            }
            t1 = self.types.supertype(ctx, syms, t1);
        }
    }

    /// Java: public boolean checkCompatibleAbstracts(DiagnosticPosition pos,
    /// Type t1, Type t2, Type site) (Check.java:1988-1999).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - t1/t2: the supertypes;
    ///   - site: the most derived type.
    ///
    /// # Returns
    ///   - true when the abstract methods are compatible.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:1988-1999.
    pub fn check_compatible_abstracts(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t1: TypeId,
        t2: TypeId,
        site: TypeId,
    ) -> bool {
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        let site_flags = ctx.sym(site_tsym).flags_field;
        let (mut t1, mut t2) = (t1, t2);
        if (site_flags & crate::code::flags::COMPOUND as i64) != 0 {
            t1 = self.types.capture(ctx, syms, t1);
            t2 = self.types.capture(ctx, syms, t2);
        }
        self.first_incompatibility(ctx, syms, pos, t1, t2, site).is_none()
    }

    /// Java: private Symbol firstIncompatibility(DiagnosticPosition pos, Type t1,
    /// Type t2, Type site) (Check.java:2005-2020).
    fn first_incompatibility(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t1: TypeId,
        t2: TypeId,
        site: TypeId,
    ) -> Option<SymbolId> {
        let mut interfaces1: std::collections::HashMap<SymbolId, TypeId> = std::collections::HashMap::new();
        self.closure(ctx, syms, t1, &mut interfaces1);
        let mut interfaces2: std::collections::HashMap<SymbolId, TypeId> = std::collections::HashMap::new();
        if t1 != t2 {
            self.closure_skip(ctx, syms, t2, &interfaces1, &mut interfaces2);
        }
        let ifaces2: std::collections::HashMap<SymbolId, TypeId> = if t1 == t2 {
            interfaces1.clone()
        } else {
            interfaces2
        };
        let ifaces1: Vec<TypeId> = interfaces1.values().copied().collect();
        let ifaces2: Vec<TypeId> = ifaces2.values().copied().collect();
        for t3 in ifaces1 {
            for t4 in &ifaces2 {
                if let Some(sym) = self.first_direct_incompatibility(ctx, syms, pos, t3, *t4, site) {
                    return Some(sym);
                }
            }
        }
        None
    }

    /// Java: private void closure(Type t, Map<TypeSymbol,Type> typeMap)
    /// (Check.java:2022-2029).
    fn closure(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: TypeId,
        type_map: &mut std::collections::HashMap<SymbolId, TypeId>,
    ) {
        if !ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            return;
        }
        let tsym = ctx.typ(t).tsym.expect("closure symbol");
        if type_map.insert(tsym, t).is_none() {
            let sup = self.types.supertype(ctx, syms, t);
            self.closure(ctx, syms, sup, type_map);
            let interfaces = self.types.interfaces(ctx, syms, t);
            let mut current = interfaces;
            while current.non_empty() {
                self.closure(ctx, syms, *current.head(), type_map);
                current = current.tail();
            }
        }
    }

    /// Java: private void closure(Type t, Map<TypeSymbol,Type> typesSkip,
    /// Map<TypeSymbol,Type> typeMap) (Check.java:2032-2042).
    fn closure_skip(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: TypeId,
        types_skip: &std::collections::HashMap<SymbolId, TypeId>,
        type_map: &mut std::collections::HashMap<SymbolId, TypeId>,
    ) {
        if !ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            return;
        }
        let tsym = ctx.typ(t).tsym.expect("closure symbol");
        if types_skip.contains_key(&tsym) {
            return;
        }
        if type_map.insert(tsym, t).is_none() {
            let sup = self.types.supertype(ctx, syms, t);
            self.closure_skip(ctx, syms, sup, types_skip, type_map);
            let interfaces = self.types.interfaces(ctx, syms, t);
            let mut current = interfaces;
            while current.non_empty() {
                self.closure_skip(ctx, syms, *current.head(), types_skip, type_map);
                current = current.tail();
            }
        }
    }

    /// Java: private Symbol firstDirectIncompatibility(DiagnosticPosition pos,
    /// Type t1, Type t2, Type site) (Check.java:2107-2165).
    #[allow(clippy::too_many_lines, clippy::collapsible_if)]
    fn first_direct_incompatibility(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        t1: TypeId,
        t2: TypeId,
        site: TypeId,
    ) -> Option<SymbolId> {
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        let t1_tsym = ctx.typ(t1).tsym.expect("t1 symbol");
        let t2_tsym = ctx.typ(t2).tsym.expect("t2 symbol");
        let t1_members = match &ctx.sym(t1_tsym).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        let t1_symbols = match t1_members {
            Some(members) => ctx.scope_get_symbols(members, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true),
            None => Vec::new(),
        };
        for s1 in t1_symbols {
            let s1_flags = ctx.sym(s1).flags_field;
            if ctx.sym(s1).kind != crate::code::kinds::Kind::MTH
                || !crate::code::symbol::is_inherited_in(ctx, syms, &self.types, s1, site_tsym)
                || (s1_flags & crate::code::flags::SYNTHETIC as i64) != 0
            {
                continue;
            }
            let implementation = crate::code::symbol::implementation(ctx, syms, &self.types, s1, site_tsym, false);
            if let Some(impl_sym) = implementation {
                if (ctx.sym(impl_sym).flags_field & crate::code::flags::ABSTRACT as i64) == 0 {
                    continue;
                }
            }
            let mut st1: Option<TypeId> = None;
            let s1_name = ctx.sym(s1).name.clone();
            let t2_members = match &ctx.sym(t2_tsym).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
                _ => None,
            };
            if let Some(t2_members) = t2_members {
                let s2_symbols = ctx.scope_get_symbols_by_name(t2_members, &s1_name, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
                for s2 in s2_symbols {
                    if s1 == s2 {
                        continue;
                    }
                    let s2_flags = ctx.sym(s2).flags_field;
                    if ctx.sym(s2).kind != crate::code::kinds::Kind::MTH
                        || !crate::code::symbol::is_inherited_in(ctx, syms, &self.types, s2, site_tsym)
                        || (s2_flags & crate::code::flags::SYNTHETIC as i64) != 0
                    {
                        continue;
                    }
                    if st1.is_none() {
                        st1 = Some(self.types.member_type(ctx, syms, t1, s1));
                    }
                    let st1_value = st1.expect("st1");
                    let st2 = self.types.member_type(ctx, syms, t2, s2);
                    if self.types.override_equivalent(ctx, syms, st1_value, st2) {
                        let tvars1 = crate::code::types::get_type_arguments(ctx, st1_value);
                        let tvars2 = crate::code::types::get_type_arguments(ctx, st2);
                        let rt1 = self.types.get_return_type_public(ctx, st1_value);
                        let rt2_unsubst = self.types.get_return_type_public(ctx, st2);
                        let rt2 = {
                            let substituted = self.types.subst(ctx, syms, &List::of(rt2_unsubst), &tvars2, &tvars1);
                            *substituted.head()
                        };
                        let rt1_prim = ctx.typ(rt1).is_primitive_or_void();
                        let rt2_prim = ctx.typ(rt2).is_primitive_or_void();
                        let mut no_warnings = crate::util::warner::Warner::new(None);
                        let compat = self.types.is_same_type(ctx, syms, rt1, rt2)
                            || (!rt1_prim
                                && !rt2_prim
                                && (self.types.covariant_return_type(ctx, syms, rt1, rt2, &mut no_warnings)
                                    || self.types.covariant_return_type(ctx, syms, rt2, rt1, &mut no_warnings)))
                            || self.check_common_overrider_in(ctx, syms, s1, s2, site);
                        if !compat {
                            let params = self.types.get_parameter_types_public(ctx, st2);
                            let params_args: Vec<crate::resources::diagnostic_contract::TypeArg> = params
                                .iter()
                                .map(|p| crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, p)))
                                .collect();
                            let details = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                                crate::resources::compiler_properties::fragments::incompatible_diff_ret(
                                    ctx.sym(s2).name.clone(),
                                    params_args,
                                ),
                            ));
                            let info = crate::resources::compiler_properties::errors::types_incompatible(
                                crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, t1)),
                                crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, t2)),
                                crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(std::rc::Rc::new(details)),
                            );
                            crate::util::abstract_log::AbstractLog::error_at(&mut *self.log.borrow_mut(), pos, crate::util::jc_diagnostic::Error::from(info));
                            return Some(s2);
                        }
                    } else if self.check_name_clash(ctx, syms, site_tsym, s1, s2)
                        && !self.check_common_overrider_in(ctx, syms, s1, s2, site)
                    {
                        let st1_member = self.types.member_type(ctx, syms, site, s1);
                        let st2_member = self.types.member_type(ctx, syms, site, s2);
                        let params1: Vec<crate::resources::diagnostic_contract::TypeArg> = self.types.get_parameter_types_public(ctx, st1_member)
                            .iter().map(|p| crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, p))).collect();
                        let params2: Vec<crate::resources::diagnostic_contract::TypeArg> = self.types.get_parameter_types_public(ctx, st2_member)
                            .iter().map(|p| crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, p))).collect();
                        let loc1 = ctx.sym(s1).location();
                        let loc2 = ctx.sym(s2).location();
                        let info = crate::resources::compiler_properties::errors::name_clash_same_erasure_no_override(
                            ctx.sym(s1).name.clone(),
                            params1,
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(match loc1 {
                                Some(loc) => crate::comp::resolve::symbol_display_name(ctx, loc),
                                None => String::new(),
                            }),
                            ctx.sym(s2).name.clone(),
                            params2,
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(match loc2 {
                                Some(loc) => crate::comp::resolve::symbol_display_name(ctx, loc),
                                None => String::new(),
                            }),
                        );
                        crate::util::abstract_log::AbstractLog::error_at(&mut *self.log.borrow_mut(), pos, crate::util::jc_diagnostic::Error::from(info));
                        return Some(s2);
                    }
                }
            }
        }
        None
    }

    /// Java: boolean checkCommonOverriderIn(Symbol s1, Symbol s2, Type site)
    /// (Check.java:2072-2090).
    fn check_common_overrider_in(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s1: SymbolId,
        s2: SymbolId,
        site: TypeId,
    ) -> bool {
        let mut supertypes: std::collections::HashMap<SymbolId, TypeId> = std::collections::HashMap::new();
        let st1 = self.types.member_type(ctx, syms, site, s1);
        let st2 = self.types.member_type(ctx, syms, site, s2);
        self.closure(ctx, syms, site, &mut supertypes);
        let s1_name = ctx.sym(s1).name.clone();
        let values: Vec<TypeId> = supertypes.values().copied().collect();
        for t in values {
            let t_tsym = match ctx.typ(t).tsym {
                Some(tsym) => tsym,
                None => continue,
            };
            let members = match &ctx.sym(t_tsym).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
                _ => None,
            };
            let members = match members {
                Some(members) => members,
                None => continue,
            };
            let s3_symbols = ctx.scope_get_symbols_by_name(members, &s1_name, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
            for s3 in s3_symbols {
                let s3_flags = ctx.sym(s3).flags_field;
                if s3 == s1 || s3 == s2 || ctx.sym(s3).kind != crate::code::kinds::Kind::MTH
                    || (s3_flags & (crate::code::flags::BRIDGE | crate::code::flags::SYNTHETIC as i64)) != 0
                {
                    continue;
                }
                let st3 = self.types.member_type(ctx, syms, site, s3);
                if self.types.override_equivalent(ctx, syms, st3, st1)
                    && self.types.override_equivalent(ctx, syms, st3, st2)
                    && self.types.return_type_substitutable(ctx, syms, st3, st1)
                    && self.types.return_type_substitutable(ctx, syms, st3, st2)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Java: private boolean checkNameClash(ClassSymbol origin, Symbol s1, Symbol s2)
    /// (Check.java:2250-2256).
    fn check_name_clash(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        origin: SymbolId,
        s1: SymbolId,
        s2: SymbolId,
    ) -> bool {
        self.clash_filter_test(ctx, syms, origin, s1)
            && self.clash_filter_test(ctx, syms, origin, s2)
            && {
                let origin_ty = ctx.sym(origin).ty.expect("origin type");
                let e1 = self.types.erasure(ctx, syms, ctx.sym(s1).ty.expect("s1 type"));
                let e2 = self.types.erasure(ctx, syms, ctx.sym(s2).ty.expect("s2 type"));
                let _ = origin_ty;
                self.types.has_same_args(ctx, syms, e1, e2)
            }
    }

    /// Java: Check.ClashFilter.test(Symbol) (Check.java:2607-2635).
    fn clash_filter_test(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        site_tsym: SymbolId,
        s: SymbolId,
    ) -> bool {
        let flags_value = ctx.sym(s).flags_field;
        let should_skip = (flags_value & crate::code::flags::CLASH) != 0
            && ctx.sym(s).owner == Some(site_tsym);
        ctx.sym(s).kind == crate::code::kinds::Kind::MTH
            && (flags_value & crate::code::flags::SYNTHETIC as i64) == 0
            && !should_skip
            && crate::code::symbol::is_inherited_in(ctx, syms, &self.types, s, site_tsym)
            && !ctx.sym(s).is_constructor(&self.names)
    }
}
/// Adapter: List<T> length (the cons list has no len).
fn list_length<T: Clone>(list: List<T>) -> usize {
    let mut count = 0usize;
    let mut current = list;
    while current.non_empty() {
        count += 1;
        current = current.tail();
    }
    count
}
impl Check {
    /// Java: void checkDefaultMethodClashes(DiagnosticPosition pos, Type site)
    /// (Check.java:2640-2685).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - site: the class type to check.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2640-2685.
    pub fn check_default_method_clashes(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        site: TypeId,
    ) {
        // The filter needs the site symbol and the inherited check; iterate the
        // closure symbols and apply DefaultMethodClashFilter.test explicitly.
        let members = self.types.members_closure(ctx, syms, site);
        let symbols = ctx.scope_get_symbols(members, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        for m in symbols {
            let flags_value = ctx.sym(m).flags_field;
            let is_default = ctx.sym(m).kind == crate::code::kinds::Kind::MTH
                && (flags_value & crate::code::flags::DEFAULT) != 0
                && crate::code::symbol::is_inherited_in(ctx, syms, &self.types, m, site_tsym)
                && !ctx.sym(m).is_constructor(&self.names);
            if !is_default {
                continue;
            }
            let prov = self.types.interface_candidates(ctx, syms, site, m);
            if prov.len() > 1 {
                let mut defaults: Vec<SymbolId> = Vec::new();
                let mut abstracts: Vec<SymbolId> = Vec::new();
                for prov_sym in prov {
                    let prov_flags = ctx.sym(prov_sym).flags_field;
                    if (prov_flags & crate::code::flags::DEFAULT) != 0 {
                        defaults.push(prov_sym);
                    } else if (prov_flags & crate::code::flags::ABSTRACT as i64) != 0 {
                        abstracts.push(prov_sym);
                    }
                    if !defaults.is_empty() && defaults.len() + abstracts.len() >= 2 {
                        let s1 = defaults[0];
                        let (s2, diag_key, is_unrelated) = if defaults.len() > 1 {
                            (defaults[1], self.incompatible_defaults_fragment(ctx, syms, site, site_tsym, m, s1, defaults[1], true), true)
                        } else {
                            (abstracts[0], self.incompatible_defaults_fragment(ctx, syms, site, site_tsym, m, s1, abstracts[0], false), false)
                        };
                        let _ = is_unrelated;
                        let loc1 = ctx.sym(s1).location().unwrap_or(s1);
                        let loc2 = ctx.sym(s2).location().unwrap_or(s2);
                        let loc1_ty = ctx.sym(loc1).ty.unwrap_or_else(|| ctx.sym(s1).ty.expect("s1 type"));
                        let loc2_ty = ctx.sym(loc2).ty.unwrap_or_else(|| ctx.sym(s2).ty.expect("s2 type"));
                        let info = crate::resources::compiler_properties::errors::types_incompatible(
                            crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, loc1_ty)),
                            crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, loc2_ty)),
                            crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(std::rc::Rc::new(diag_key)),
                        );
                        crate::util::abstract_log::AbstractLog::error_at(&mut *self.log.borrow_mut(), pos, crate::util::jc_diagnostic::Error::from(info));
                        break;
                    }
                }
            }
        }
    }

    /// Adapter: the IncompatibleUnrelatedDefaults/IncompatibleAbstractDefault
    /// fragment (Fragments.java).
    #[allow(clippy::too_many_arguments)]
    fn incompatible_defaults_fragment(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        site: TypeId,
        site_tsym: SymbolId,
        m: SymbolId,
        s1: SymbolId,
        s2: SymbolId,
        unrelated: bool,
    ) -> crate::util::jc_diagnostic::JCDiagnostic {
        let kind = crate::code::kinds::kind_name(ctx.sym(site_tsym), ctx);
        let m_ty = self.types.member_type(ctx, syms, site, m);
        let params: Vec<crate::resources::diagnostic_contract::TypeArg> = self
            .types
            .get_parameter_types_public(ctx, m_ty)
            .iter()
            .map(|p| crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, p)))
            .collect();
        let s1_loc = ctx.sym(s1).location().unwrap_or(s1);
        let s2_loc = ctx.sym(s2).location().unwrap_or(s2);
        let s1_arg = crate::resources::diagnostic_contract::SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, s1_loc));
        let s2_arg = crate::resources::diagnostic_contract::SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, s2_loc));
        let info = if unrelated {
            crate::resources::compiler_properties::fragments::incompatible_unrelated_defaults(
                crate::resources::diagnostic_contract::KindNameArg::placeholder(kind.to_string()),
                crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, site)),
                ctx.sym(m).name.clone(),
                params,
                s1_arg,
                s2_arg,
            )
        } else {
            crate::resources::compiler_properties::fragments::incompatible_abstract_default(
                crate::resources::diagnostic_contract::KindNameArg::placeholder(kind.to_string()),
                crate::resources::diagnostic_contract::TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, site)),
                ctx.sym(m).name.clone(),
                params,
                s1_arg,
                s2_arg,
            )
        };
        self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(info))
    }
}impl Check {
    /// Java: void checkLeaksNotAccessible(Env<AttrContext> env, JCClassDecl check)
    /// (Check.java:4082-4210).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the attribution environment;
    ///   - check: the class declaration to scan.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the env toplevel must be a compilation unit.
    ///
    /// Upstream: Check.java:4082-4210.
    pub fn check_leaks_not_accessible(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &std::rc::Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        check: TreeId,
    ) {
        let toplevel = env.toplevel.expect("env toplevel");
        let (toplevel_modle, toplevel_packge) = match &ctx.tree(toplevel).kind {
            crate::tree::jc_tree::TreeKind::CompilationUnit { modle, packge, .. } => (*modle, *packge),
            _ => panic!("ClassCastException: JCCompilationUnit expected"),
        };
        let (module, packge) = match (toplevel_modle, toplevel_packge) {
            (Some(module), Some(packge)) => (module, packge),
            _ => return,
        };
        let unnamed = syms.unnamed_module;
        let no_module = syms.no_module;
        if module == unnamed || module == no_module {
            return;
        }
        let check_sym = match &ctx.tree(check).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => return,
        };
        if (ctx.sym(check_sym).flags_field & crate::code::flags::COMPOUND as i64) != 0 {
            return;
        }
        let current_export = find_export(ctx, packge);
        let in_export = match current_export {
            None => return,
            Some((_, Some(_))) => return,
            Some((p, None)) => (p, None),
        };
        let lint = env.info.borrow().lint.clone();
        let mut checker = LeaksChecker {
            chk: self,
            syms,
            in_package: packge,
            in_export: Some(in_export),
            lint,
            in_super_type: false,
            check_tree: check,
        };
        crate::tree::jc_tree::accept(ctx, check, &mut checker);
    }
}

/// Java: the TreeScanner used by checkLeaksNotAccessible (Check.java:4096-4205).
pub struct LeaksChecker<'a> {
    chk: &'a Check,
    syms: &'a crate::code::symtab::Symtab,
    in_package: SymbolId,
    in_export: Option<(Option<SymbolId>, Option<List<SymbolId>>)>,
    lint: Option<std::rc::Rc<crate::code::lint::Lint>>,
    in_super_type: bool,
    check_tree: TreeId,
}

impl<'a> crate::tree::tree_scanner::TreeScannerVisitor for LeaksChecker<'a> {
    /// Java: public void visitBlock(JCBlock tree) — no-op.
    fn visit_block(
        &mut self,
        _ctx: &mut crate::compiler_context::CompilerContext,
        _tree: TreeId,
    ) {
    }

    /// Java: public void visitMethodDef(JCMethodDecl tree) (Check.java:4108-4122).
    fn visit_method_def(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        let sym = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
            _ => return,
        };
        if !is_api_symbol(ctx, sym) {
            return;
        }
        let prev_lint = self.lint.clone();
        if let Some(lint) = prev_lint.clone() {
            let augmented = {
                let sym_ref = ctx.sym(sym);
                lint.augment(&*ctx, self.syms, sym_ref)
            };
            self.lint = Some(std::rc::Rc::new(augmented));
        }
        if self.exports_lint_enabled() {
            for child in crate::tree::tree_scanner::tree_children(ctx, tree) {
                self.scan(ctx, Some(child));
            }
        }
        self.lint = prev_lint;
    }

    /// Java: public void visitVarDef(JCVariableDecl tree) (Check.java:4124-4140).
    fn visit_var_def(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        let (sym, mods, vartype) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::VariableDecl {
                sym: Some(sym),
                mods,
                vartype,
                ..
            } => (*sym, *mods, *vartype),
            _ => return,
        };
        let owner_is_mth = ctx
            .sym(sym)
            .owner
            .map(|owner| ctx.sym(owner).kind == crate::code::kinds::Kind::MTH)
            .unwrap_or(false);
        if !is_api_symbol(ctx, sym) && !owner_is_mth {
            return;
        }
        let prev_lint = self.lint.clone();
        if let Some(lint) = prev_lint.clone() {
            let augmented = {
                let sym_ref = ctx.sym(sym);
                lint.augment(&*ctx, self.syms, sym_ref)
            };
            self.lint = Some(std::rc::Rc::new(augmented));
        }
        if self.exports_lint_enabled() {
            self.scan(ctx, mods);
            self.scan(ctx, vartype);
        }
        self.lint = prev_lint;
    }

    /// Java: public void visitClassDef(JCClassDecl tree) (Check.java:4142-4172).
    fn visit_class_def(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        if tree != self.check_tree {
            return;
        }
        let (sym, mods, typarams, extending, implementing, defs) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl {
                sym: Some(sym),
                mods,
                typarams,
                extending,
                implementing,
                defs,
                ..
            } => (
                *sym,
                *mods,
                typarams.clone(),
                *extending,
                implementing.clone(),
                defs.clone(),
            ),
            _ => return,
        };
        if !is_api_symbol(ctx, sym) {
            return;
        }
        let prev_lint = self.lint.clone();
        if let Some(lint) = prev_lint.clone() {
            let augmented = {
                let sym_ref = ctx.sym(sym);
                lint.augment(&*ctx, self.syms, sym_ref)
            };
            self.lint = Some(std::rc::Rc::new(augmented));
        }
        if self.exports_lint_enabled() {
            self.scan(ctx, mods);
            self.scan_list(ctx, &typarams);
            let old_in_super_type = self.in_super_type;
            self.in_super_type = true;
            self.scan(ctx, extending);
            self.scan_list(ctx, &implementing);
            self.in_super_type = old_in_super_type;
            self.scan_list(ctx, &defs);
        }
        self.lint = prev_lint;
    }

    /// Java: public void visitTypeApply(JCTypeApply tree) (Check.java:4174-4186).
    fn visit_type_apply(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => return,
        };
        self.scan(ctx, clazz);
        let old_in_super_type = self.in_super_type;
        self.in_super_type = false;
        self.scan_list(ctx, &arguments);
        self.in_super_type = old_in_super_type;
    }

    /// Java: public void visitIdent(JCIdent tree) (Check.java:4188-4194).
    fn visit_ident(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        let Some(sym) = crate::tree::tree_info::symbol(ctx, tree) else {
            return;
        };
        let is_type = ctx.sym(sym).kind == crate::code::kinds::Kind::TYP;
        let is_typevar = ctx
            .sym(sym)
            .ty
            .map(|ty| ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::TYPEVAR))
            .unwrap_or(false);
        if is_type && !is_typevar {
            self.check_visible(ctx, ctx.tree(tree).pos, sym);
        }
    }

    /// Java: public void visitSelect(JCFieldAccess tree) (Check.java:4196-4205).
    fn visit_select(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) {
        let (sym, selected) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::FieldAccess { sym, selected, .. } => (*sym, *selected),
            _ => return,
        };
        let Some(sym) = sym else {
            return;
        };
        let sitesym = selected.and_then(|selected| crate::tree::tree_info::symbol(ctx, selected));
        let sitesym_is_pck = sitesym
            .map(|s| ctx.sym(s).kind == crate::code::kinds::Kind::PCK)
            .unwrap_or(false);
        if ctx.sym(sym).kind == crate::code::kinds::Kind::TYP && sitesym_is_pck {
            self.check_visible(ctx, ctx.tree(tree).pos, sym);
        } else if let Some(selected) = selected {
            self.scan(ctx, Some(selected));
        }
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(LeaksChecker<'a>, 'a);

impl<'a> LeaksChecker<'a> {
    fn exports_lint_enabled(&self) -> bool {
        self.lint
            .as_ref()
            .map(|lint| lint.is_enabled(crate::code::lint::LintCategory::EXPORTS))
            .unwrap_or(false)
    }

    /// Java: private void checkVisible(DiagnosticPosition pos, Symbol what,
    /// PackageSymbol inPackage, boolean inSuperType) (Check.java:4230-4290).
    #[allow(clippy::collapsible_if)]
    fn check_visible(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        pos: i32,
        what: SymbolId,
    ) {
        let what_kind = crate::code::kinds::kind_name(ctx.sym(what), ctx).to_string();
        let what_display = crate::comp::resolve::symbol_display_name(ctx, what);
        let what_pkg = crate::code::symbol::packge(ctx, what);
        let what_module = what_pkg.and_then(|pkg| crate::code::symbol::Symbol::symbol_module(ctx, pkg));
        if !is_api_symbol(ctx, what) && !self.in_super_type {
            let warning = crate::util::jc_diagnostic::Warning::from(
                crate::resources::compiler_properties::warnings::leaks_not_accessible(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(what_kind.clone()),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(what_display.clone()),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(module_display(ctx, what_module)),
                ),
            );
            self.emit_warning(pos, warning);
            return;
        }
        let what_pkg = match what_pkg {
            Some(pkg) => pkg,
            None => return,
        };
        let what_export = find_export(ctx, what_pkg);
        let what_export = match what_export {
            Some(export) => export,
            None => {
                let warning = crate::util::jc_diagnostic::Warning::from(
                    crate::resources::compiler_properties::warnings::leaks_not_accessible_unexported(
                        crate::resources::diagnostic_contract::KindNameArg::placeholder(what_kind.clone()),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(what_display.clone()),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(module_display(ctx, what_module)),
                    ),
                );
                self.emit_warning(pos, warning);
                return;
            }
        };
        let (_, what_export_modules) = what_export;
        if let Some(what_export_modules) = what_export_modules {
            let in_modules = self.in_export.clone().and_then(|(_, modules)| modules);
            let contains_all = match (&what_export_modules, &in_modules) {
                (what_modules, Some(in_modules)) => {
                    let mut in_current = in_modules.clone();
                    let mut all = true;
                    while in_current.non_empty() {
                        let target = *in_current.head();
                        let mut what_current = what_modules.clone();
                        let mut found = false;
                        while what_current.non_empty() {
                            if *what_current.head() == target {
                                found = true;
                                break;
                            }
                            what_current = what_current.tail();
                        }
                        if !found {
                            all = false;
                            break;
                        }
                        in_current = in_current.tail();
                    }
                    all
                }
                (_, None) => false,
            };
            if !contains_all {
                let warning = crate::util::jc_diagnostic::Warning::from(
                    crate::resources::compiler_properties::warnings::leaks_not_accessible_unexported_qualified(
                        crate::resources::diagnostic_contract::KindNameArg::placeholder(what_kind.clone()),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(what_display.clone()),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(module_display(ctx, what_module)),
                    ),
                );
                self.emit_warning(pos, warning);
            }
        }
        let in_package_module = crate::code::symbol::Symbol::symbol_module(ctx, self.in_package);
        if what_module != in_package_module && what_module != Some(self.syms.java_base) {
            let target = match in_package_module {
                Some(module) => module,
                None => return,
            };
            let mut todo: List<SymbolId> = List::of(target);
            while todo.non_empty() {
                let current = *todo.head();
                todo = todo.tail();
                if Some(current) == what_module {
                    return;
                }
                if (ctx.sym(current).flags_field & crate::code::flags::AUTOMATIC_MODULE) != 0 {
                    continue;
                }
                let requires = match &ctx.sym(current).kind_data {
                    crate::code::symbol::SymbolKind::ModuleSymbol { requires, .. } => requires.clone(),
                    _ => List::nil(),
                };
                let mut current_requires = requires;
                while current_requires.non_empty() {
                    let directive = *current_requires.head();
                    current_requires = current_requires.tail();
                    if let crate::code::directive::Directive::Requires { module, flags } = ctx.directive(directive) {
                        if flags.contains(&crate::code::directive::RequiresFlag::TRANSITIVE) {
                            if let Some(module) = module {
                                todo = todo.prepend(*module);
                            }
                        }
                    }
                }
            }
            let warning = crate::util::jc_diagnostic::Warning::from(
                crate::resources::compiler_properties::warnings::leaks_not_accessible_not_required_transitive(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(what_kind.clone()),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(what_display.clone()),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(module_display(ctx, what_module)),
                ),
            );
            self.emit_warning(pos, warning);
        }
    }

    fn emit_warning(&self, pos: i32, warning: crate::util::jc_diagnostic::Warning) {
        crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
            &mut *self.chk.log.borrow_mut(),
            Some(crate::code::lint::LintCategory::EXPORTS),
            Some(std::rc::Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos))),
            warning,
        );
    }
}

/// Adapter: module display name for the leaks diagnostics.
fn module_display(
    ctx: &mut crate::compiler_context::CompilerContext,
    module: Option<SymbolId>,
) -> String {
    match module {
        Some(module) => crate::comp::resolve::symbol_display_name(ctx, module),
        None => String::new(),
    }
}

/// Java: private boolean isAPISymbol(Symbol sym) (Check.java:4207-4215).
#[allow(clippy::collapsible_if)]
fn is_api_symbol(ctx: &mut crate::compiler_context::CompilerContext, mut sym: SymbolId) -> bool {
    loop {
        if ctx.sym(sym).kind == crate::code::kinds::Kind::PCK {
            return true;
        }
        let flags_value = ctx.sym(sym).flags_field;
        let public_or_protected = (crate::code::flags::PUBLIC as i64) | (crate::code::flags::PROTECTED as i64);
        if (flags_value & public_or_protected) == 0 {
            return false;
        }
        match ctx.sym(sym).owner {
            Some(owner) => sym = owner,
            None => return true,
        }
    }
}

/// Java: private ExportsDirective findExport(PackageSymbol pack) (Check.java:4216-4225).
#[allow(clippy::collapsible_if)]
fn find_export(
    ctx: &mut crate::compiler_context::CompilerContext,
    pack: SymbolId,
) -> Option<(Option<SymbolId>, Option<List<SymbolId>>)> {
    let module = crate::code::symbol::Symbol::symbol_module(ctx, pack)?;
    let exports = match &ctx.sym(module).kind_data {
        crate::code::symbol::SymbolKind::ModuleSymbol { exports, .. } => exports.clone(),
        _ => return None,
    };
    let mut current = exports;
    while current.non_empty() {
        let directive = *current.head();
        current = current.tail();
        if let crate::code::directive::Directive::Exports { packge, modules, .. } = ctx.directive(directive) {
            if *packge == Some(pack) {
                return Some((*packge, modules.clone()));
            }
        }
    }
    None
}
impl Check {
    /// Java: public void validateTypeAnnotations(List<JCAnnotation> annotations,
    /// boolean isTypeParameter) (Check.java:2926-2930).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - annotations: the annotations to validate;
    ///   - is_type_parameter: whether the annotations are on a type parameter.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2926-2930.
    pub fn validate_type_annotations(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        annotations: &List<TreeId>,
        is_type_parameter: bool,
    ) {
        for annotation in annotations.iter() {
            self.validate_type_annotation(ctx, syms, annotation, is_type_parameter);
        }
    }

    /// Java: public void validateTypeAnnotation(JCAnnotation a, boolean
    /// isTypeParameter) (Check.java:3087-3098).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - a: the annotation tree;
    ///   - is_type_parameter: whether the annotation is on a type parameter.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3087-3098.
    pub fn validate_type_annotation(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
        is_type_parameter: bool,
    ) {
        assert!(ctx.tree(a).ty.is_some(), "AssertionError: annotation type");
        self.validate_annotation_tree(ctx, syms, a);
        let is_type_annotation_tag = ctx.tree(a).tag == crate::tree::jc_tree::Tag::TYPE_ANNOTATION;
        let annotation_type_erroneous = match &ctx.tree(a).kind {
            crate::tree::jc_tree::TreeKind::Annotation { annotation_type, .. } => {
                annotation_type
                    .and_then(|annotation_type| ctx.tree(annotation_type).ty)
                    .map(|ty| ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::ERROR))
                    .unwrap_or(false)
            }
            _ => false,
        };
        if is_type_annotation_tag
            && !annotation_type_erroneous
            && !self.is_type_annotation_tree(ctx, syms, a, is_type_parameter)
        {
            let annotation_ty = ctx.tree(a).ty.expect("annotation type");
            let info = crate::resources::compiler_properties::errors::annotation_type_not_applicable_to_type(
                crate::resources::diagnostic_contract::TypeArg::placeholder(
                    crate::comp::attr::type_display_of(ctx, annotation_ty),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(a).pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

    /// Java: boolean isTypeAnnotation(JCAnnotation a, boolean isTypeParameter)
    /// (Check.java:3290-3296).
    fn is_type_annotation_tree(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
        is_type_parameter: bool,
    ) -> bool {
        let annotation_type_ty = match &ctx.tree(a).kind {
            crate::tree::jc_tree::TreeKind::Annotation { annotation_type, .. } => *annotation_type,
            _ => return false,
        };
        let tsym = match annotation_type_ty
            .and_then(|annotation_type| ctx.tree(annotation_type).ty)
            .and_then(|ty| ctx.typ(ty).tsym)
        {
            Some(tsym) => tsym,
            None => return false,
        };
        let ta_rc = self.type_annotations.borrow().clone();
        let ta_rc = match ta_rc {
            Some(ta) => ta,
            None => return false,
        };
        let targets = ta_rc.annotation_targets(ctx, syms, tsym);
        let targets = match targets {
            Some(targets) => targets,
            None => return false,
        };
        let mut current = targets;
        while current.non_empty() {
            let attr = *current.head();
            if self.is_type_annotation_attr(ctx, attr, is_type_parameter) {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// Java: boolean isTypeAnnotation(Attribute a, boolean isTypeParameter)
    /// (Check.java:3297-3302).
    fn is_type_annotation_attr(
        &self,
        ctx: &mut CompilerContext,
        a: AttributeId,
        is_type_parameter: bool,
    ) -> bool {
        let enum_sym = match &ctx.attr(a).kind {
            crate::code::attribute::AttributeKind::Enum { value: Some(value) } => *value,
            _ => return false,
        };
        let name = ctx.sym(enum_sym).name.clone();
        name == self.names.TYPE_USE
            || (is_type_parameter && name == self.names.TYPE_PARAMETER)
    }
}impl Check {
    /// Java: void checkCompatibleSupertypes(DiagnosticPosition pos, Type c)
    /// (Check.java:2494-2507).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - c: the class type whose interfaces are checked.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:2494-2507.
    pub fn check_compatible_supertypes(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        c: TypeId,
    ) {
        let mut supertypes = self.types.interfaces(ctx, syms, c);
        let supertype = self.types.supertype(ctx, syms, c);
        if ctx.typ(supertype).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            let super_tsym = ctx.typ(supertype).tsym.expect("supertype symbol");
            if (ctx.sym(super_tsym).flags_field & crate::code::flags::ABSTRACT as i64) != 0 {
                supertypes = supertypes.prepend(supertype);
            }
        }
        let mut supertypes_vec: Vec<TypeId> = Vec::new();
        let mut current = supertypes;
        while current.non_empty() {
            supertypes_vec.push(*current.head());
            current = current.tail();
        }
        for index in 0..supertypes_vec.len() {
            let head = supertypes_vec[index];
            if !crate::code::types::get_type_arguments(ctx, head).is_empty()
                && !self.check_compatible_abstracts(ctx, syms, pos, head, head, c)
            {
                return;
            }
            for other in supertypes_vec.iter().take(index) {
                if !self.check_compatible_abstracts(ctx, syms, pos, head, *other, c) {
                    return;
                }
            }
        }
        self.check_compatible_concretes(ctx, syms, pos, c);
    }
}impl Check {
    /// Java: Type checkDiamond(JCNewClass tree, Type t) (Check.java:843-867).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the new class tree;
    ///   - t: the attributed class type.
    ///
    /// # Returns
    ///   - the checked (possibly error) type.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:843-867.
    pub fn check_diamond(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        t: TypeId,
    ) -> TypeId {
        let clazz = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::NewClass { clazz: Some(clazz), .. } => *clazz,
            _ => return t,
        };
        if !crate::tree::tree_info::is_diamond(ctx, tree)
            || ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::ERROR)
        {
            return self.check_class_type_no_bounds(ctx, syms, ctx.tree(clazz).pos, t, true);
        }
        let def = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::NewClass { def, .. } => *def,
            _ => None,
        };
        if def.is_some()
            && !crate::code::source::Feature::DIAMOND_WITH_ANONYMOUS_CLASS_CREATION
                .allowed_in_source(self.source)
        {
            let feature_fragment = self.diags.fragment(
                crate::code::source::Feature::DIAMOND_WITH_ANONYMOUS_CLASS_CREATION
                    .fragment(self.source.name()),
            );
            let info = crate::resources::compiler_properties::errors::cant_apply_diamond1(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    crate::comp::attr::type_display_of(ctx, t),
                ),
                crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                    std::rc::Rc::new(feature_fragment),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(clazz).pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
        let t_tsym = ctx.typ(t).tsym.expect("diamond class symbol");
        let t_tsym_ty = ctx.sym(t_tsym).ty.expect("diamond class symbol type");
        let type_arguments = crate::code::types::get_type_arguments(ctx, t_tsym_ty);
        if type_arguments.is_empty() {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::diamond_non_generic(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        crate::comp::attr::type_display_of(ctx, t),
                    ),
                ),
            ));
            let info = crate::resources::compiler_properties::errors::cant_apply_diamond1(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    crate::comp::attr::type_display_of(ctx, t),
                ),
                crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(clazz).pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
            self
                .types
                .create_error_type_original(ctx, Some(t), syms)
        } else {
            let typeargs_nonempty = match &ctx.tree(tree).kind {
                crate::tree::jc_tree::TreeKind::NewClass { typeargs, .. } => !typeargs.is_empty(),
                _ => false,
            };
            if typeargs_nonempty {
                let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::diamond_and_explicit_params(
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            crate::comp::attr::type_display_of(ctx, t),
                        ),
                    ),
                ));
                let info = crate::resources::compiler_properties::errors::cant_apply_diamond1(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        crate::comp::attr::type_display_of(ctx, t),
                    ),
                    crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(std::rc::Rc::new(fragment)),
                );
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    ctx.tree(clazz).pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
                return self
                    .types
                    .create_error_type_original(ctx, Some(t), syms);
            }
            t
        }
    }

    /// Java: List<Type> checkDiamondDenotable(ClassType t) (Check.java:874-884).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the inferred class type.
    ///
    /// # Returns
    ///   - the non-denotable types.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:874-884.
    pub fn check_diamond_denotable(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        t: TypeId,
    ) -> Vec<TypeId> {
        let allparams = crate::code::types::allparams_of(ctx, t);
        let mut result: Vec<TypeId> = Vec::new();
        for arg in allparams.iter() {
            if !self.check_denotable(ctx, arg) {
                result.push(arg);
            }
        }
        result
    }

    /// Java: public boolean checkDenotable(Type t) (Check.java:886-928).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the type to check.
    ///
    /// # Returns
    ///   - true when the type is denotable.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:886-928.
    pub fn check_denotable(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        t: TypeId,
    ) -> bool {
        use crate::code::type_::TypeKindData;
        match &ctx.typ(t).kind {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                if ctx.typ(t).is_union() || ctx.typ(t).is_intersection() {
                    return false;
                }
                let allparams = crate::code::types::allparams_of(ctx, t);
                for targ in allparams.iter() {
                    if !self.check_denotable(ctx, targ) {
                        return false;
                    }
                }
                true
            }
            TypeKindData::TypeVar { .. } => ctx
                .typ(t)
                .tsym
                .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::SYNTHETIC as i64) == 0)
                .unwrap_or(false),
            TypeKindData::CapturedType { .. } => false,
            TypeKindData::ArrayType { elemtype, .. } => elemtype
                .map(|elemtype| self.check_denotable(ctx, elemtype))
                .unwrap_or(true),
            TypeKindData::WildcardType { type_, .. } => type_
                .map(|type_| self.check_denotable(ctx, type_))
                .unwrap_or(true),
            _ => true,
        }
    }
}
impl Check {
    /// Java: public boolean importAccessible(Symbol sym, PackageSymbol packge)
    /// (Check.java:4042-4056).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - sym: the imported symbol;
    ///   - packge: the package doing the import.
    ///
    /// # Returns
    ///   - true when the symbol is accessible everywhere in the package.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:4042-4056.
    pub fn import_accessible(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
        packge: SymbolId,
    ) -> bool {
        // Java: sym.flags() completes ClassSymbol only (Symbol.java:1329-1332).
        if ctx.sym(sym).kind == crate::code::kinds::Kind::TYP
            && let Err(failure) = ctx.complete_class_symbol(syms, sym)
        {
            if failure.bad_class_file {
                // Java: catch (ClassFinder.BadClassFile err) { throw err; }
                panic!("BadClassFile: {}", failure.get_message().to_string_lossy());
            }
            // Java: catch (CompletionFailure ex) { return false; }
            return false;
        }
        let flags_value =
            (ctx.sym(sym).flags_field & crate::code::flags::AccessFlags as i64) as i32;
        match flags_value {
            0 => crate::code::symbol::symbol_packge(ctx, sym) == Some(packge),
            crate::code::flags::PROTECTED => {
                crate::code::symbol::symbol_packge(ctx, sym) == Some(packge)
            }
            crate::code::flags::PRIVATE => false,
            crate::code::flags::PUBLIC => true,
            _ => true,
        }
    }

    /// Java: void checkImportsUnique(JCCompilationUnit toplevel) (Check.java:3793-3820).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - toplevel: the compilation unit.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:3793-3820.
    pub fn check_imports_unique(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        toplevel: TreeId,
    ) {
        let (defs, packge, top_level_scope) = match &ctx.tree(toplevel).kind {
            TreeKind::CompilationUnit {
                defs,
                packge,
                toplevel_scope,
                ..
            } => (defs.clone(), *packge, *toplevel_scope),
            _ => return,
        };
        let Some(packge) = packge else {
            return;
        };
        let ordinally_imported_scope = ctx.scope_create(Some(packge));
        let statically_imported_scope = ctx.scope_create(Some(packge));
        let Some(top_level_scope) = top_level_scope else {
            return;
        };
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            current = current.tail();
            let import_scope = match &ctx.tree(def).kind {
                TreeKind::Import { import_scope, .. } => *import_scope,
                _ => continue,
            };
            let Some(import_scope) = import_scope else {
                continue;
            };
            let scope_snapshot = ctx.scope(import_scope).clone();
            let candidates = scope_snapshot.get_symbols(
                import_scope,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                ctx,
            );
            let symbols: Vec<SymbolId> = candidates
                .into_iter()
                .filter(|sym| ctx.sym(*sym).kind == crate::code::kinds::Kind::TYP)
                .collect();
            let is_static = match &ctx.tree(def).kind {
                TreeKind::Import { static_import, .. } => *static_import,
                _ => false,
            };
            for sym in symbols {
                self.check_unique_import(
                    ctx,
                    syms,
                    def,
                    ordinally_imported_scope,
                    statically_imported_scope,
                    top_level_scope,
                    sym,
                    is_static,
                );
                if is_static {
                    ctx.scope_enter(statically_imported_scope, sym);
                } else {
                    ctx.scope_enter(ordinally_imported_scope, sym);
                }
            }
            if let TreeKind::Import { import_scope, .. } = &mut ctx.tree_mut(def).kind {
                *import_scope = None;
            }
        }
    }

    /// Java: private boolean checkUniqueImport(...) (Check.java:3830-3860).
    #[allow(clippy::too_many_arguments)]
    fn check_unique_import(
        &self,
        ctx: &mut CompilerContext,
        _syms: &crate::code::symtab::Symtab,
        import_tree: TreeId,
        ordinally_imported_scope: ScopeId,
        statically_imported_scope: ScopeId,
        top_level_scope: ScopeId,
        sym: SymbolId,
        static_import: bool,
    ) -> bool {
        let symbol_name = ctx.sym(sym).name.clone();
        let candidates = ctx.scope_get_symbols_by_name(
            ordinally_imported_scope,
            &symbol_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let ordinary_clashing = candidates
            .into_iter()
            .find(|candidate| *candidate != sym && !erroneous_symbol(ctx, *candidate));
        let mut static_clashing = None;
        if ordinary_clashing.is_none() && !static_import {
            let candidates = ctx.scope_get_symbols_by_name(
                statically_imported_scope,
                &symbol_name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            static_clashing = candidates
                .into_iter()
                .find(|candidate| *candidate != sym && !erroneous_symbol(ctx, *candidate));
        }
        let pos = ctx.tree(import_tree).pos;
        if let Some(clashing) = ordinary_clashing {
            let message = crate::resources::compiler_properties::errors::already_defined_single_import(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_text(
                    ctx, clashing,
                )),
            );
            self.log
                .borrow_mut()
                .error_at(pos, crate::util::jc_diagnostic::Error::from(message));
            return false;
        }
        if let Some(clashing) = static_clashing {
            let message = crate::resources::compiler_properties::errors::already_defined_static_single_import(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_text(
                    ctx, clashing,
                )),
            );
            self.log
                .borrow_mut()
                .error_at(pos, crate::util::jc_diagnostic::Error::from(message));
            return false;
        }
        let candidates = ctx.scope_get_symbols_by_name(
            top_level_scope,
            &symbol_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let clashing = candidates
            .into_iter()
            .find(|candidate| *candidate != sym && !erroneous_symbol(ctx, *candidate));
        if let Some(clashing) = clashing {
            let message = crate::resources::compiler_properties::errors::already_defined_this_unit(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_text(
                    ctx, clashing,
                )),
            );
            self.log
                .borrow_mut()
                .error_at(pos, crate::util::jc_diagnostic::Error::from(message));
            return false;
        }
        true
    }

    /// Java: public void checkImportsResolvable(JCCompilationUnit toplevel)
    /// (Check.java:4002-4024).
    pub fn check_imports_resolvable(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        toplevel: TreeId,
    ) {
        let _ = syms;
        let (defs, packge) = match &ctx.tree(toplevel).kind {
            TreeKind::CompilationUnit { defs, packge, .. } => (defs.clone(), *packge),
            _ => return,
        };
        let Some(packge) = packge else {
            return;
        };
        let mut current = defs;
        while current.non_empty() {
            let imp = *current.head();
            current = current.tail();
            let (static_import, qualid) = match &ctx.tree(imp).kind {
                TreeKind::Import {
                    static_import,
                    qualid,
                    ..
                } => (*static_import, *qualid),
                _ => continue,
            };
            if !static_import {
                continue;
            }
            let Some(qualid) = qualid else {
                continue;
            };
            let TreeKind::FieldAccess {
                selected: Some(selected),
                name,
                ..
            } = &ctx.tree(qualid).kind
            else {
                continue;
            };
            let (selected, name) = (*selected, name.clone());
            if name == self.names.asterisk {
                continue;
            }
            let Some(origin) = ctx.tree(selected).ty else {
                continue;
            };
            let Some(site) = ctx.typ(origin).tsym else {
                continue;
            };
            if ctx.sym(site).kind != crate::code::kinds::Kind::TYP {
                continue;
            }
            let mut processed: Vec<SymbolId> = Vec::new();
            if !self.check_type_contains_importable_element(
                ctx,
                syms,
                site,
                site,
                packge,
                &name,
                &mut processed,
            ) {
                let message = crate::resources::compiler_properties::errors::cant_resolve_location(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder("STATIC"),
                    name.clone(),
                    (),
                    (),
                    crate::resources::diagnostic_contract::DiagnosticArg::Text(
                        symbol_text(ctx, site),
                    ),
                );
                self.log
                    .borrow_mut()
                    .error_at(ctx.tree(imp).pos, crate::util::jc_diagnostic::Error::from(message));
            }
        }
    }

    /// Java: private boolean checkTypeContainsImportableElement(...)
    /// (Check.java:4028-4040).
    fn check_type_contains_importable_element(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tsym: SymbolId,
        origin: SymbolId,
        packge: SymbolId,
        name: &Name,
        processed: &mut Vec<SymbolId>,
    ) -> bool {
        if processed.contains(&tsym) {
            return false;
        }
        processed.push(tsym);
        let Some(ty) = ctx.sym(tsym).ty else {
            return false;
        };
        let supertype = {
            let syms_ref: &crate::code::symtab::Symtab = syms;
            self.types.supertype(ctx, syms_ref, ty)
        };
        if let Some(super_tsym) = ctx.typ(supertype).tsym
            && super_tsym != tsym
            && self.check_type_contains_importable_element(
                ctx, syms, super_tsym, origin, packge, name, processed,
            )
        {
            return true;
        }
        let interfaces = {
            let syms_ref: &crate::code::symtab::Symtab = syms;
            self.types.interfaces(ctx, syms_ref, ty)
        };
        let mut interface_list = interfaces;
        while interface_list.non_empty() {
            let interface_ty = *interface_list.head();
            interface_list = interface_list.tail();
            if let Some(interface_tsym) = ctx.typ(interface_ty).tsym
                && interface_tsym != tsym
                && self.check_type_contains_importable_element(
                    ctx,
                    syms,
                    interface_tsym,
                    origin,
                    packge,
                    name,
                    processed,
                )
            {
                return true;
            }
        }
        let members = ctx.sym(tsym).members();
        let Some(members) = members else {
            return false;
        };
        let symbols =
            ctx.scope_get_symbols_by_name(members, name, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
        for sym in symbols {
            if ctx.sym(sym).is_static(ctx)
                && self.import_accessible(ctx, syms, sym, packge)
                && {
                    let syms_ref: &crate::code::symtab::Symtab = syms;
                    crate::code::symbol::is_member_of(ctx, syms_ref, &self.types, sym, origin)
                }
            {
                return true;
            }
        }
        false
    }

    /// Java: public void checkImportedPackagesObservable(JCCompilationUnit toplevel)
    /// (Check.java:4026-4039).
    pub fn check_imported_packages_observable(
        &self,
        ctx: &mut CompilerContext,
        toplevel: TreeId,
    ) {
        let defs = match &ctx.tree(toplevel).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => return,
        };
        let mut current = defs;
        while current.non_empty() {
            let imp = *current.head();
            current = current.tail();
            let (static_import, qualid) = match &ctx.tree(imp).kind {
                TreeKind::Import {
                    static_import,
                    qualid,
                    ..
                } => (*static_import, *qualid),
                _ => continue,
            };
            if static_import {
                continue;
            }
            let Some(qualid) = qualid else {
                continue;
            };
            let TreeKind::FieldAccess {
                selected: Some(selected),
                name,
                ..
            } = &ctx.tree(qualid).kind
            else {
                continue;
            };
            if name != &self.names.asterisk {
                continue;
            }
            let selected = *selected;
            let target = ctx
                .tree(selected)
                .ty
                .and_then(|ty| ctx.typ(ty).tsym);
            let Some(tsym) = target else {
                continue;
            };
            if ctx.sym(tsym).kind != crate::code::kinds::Kind::PCK {
                continue;
            }
            let empty = ctx
                .sym(tsym)
                .members()
                .map(|members| {
                    ctx.scope_get_symbols(
                        members,
                        crate::code::scope::LookupKind::NON_RECURSIVE,
                        &|_| true,
                    )
                    .is_empty()
                })
                .unwrap_or(true);
            let observable = crate::code::source::Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES
                .allowed_in_source(self.source)
                && symbol_exists_public(ctx, tsym);
            if empty && !observable {
                let message = crate::resources::compiler_properties::errors::doesnt_exist(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_text(
                        ctx, tsym,
                    )),
                );
                self.log
                    .borrow_mut()
                    .error_at(ctx.tree(imp).pos, crate::util::jc_diagnostic::Error::from(message));
            }
        }
    }
}

/// Return whether the symbol's type is erroneous.
fn erroneous_symbol(ctx: &mut CompilerContext, sym: SymbolId) -> bool {
    ctx.sym(sym)
        .ty
        .map(|ty| ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::ERROR))
        .unwrap_or(true)
}

/// Return the printable name of a symbol (diagnostic argument adapter).
fn symbol_text(ctx: &mut CompilerContext, sym: SymbolId) -> String {
    ctx.sym(sym)
        .name
        .to_java_string()
        .to_string_lossy()
        .to_string()
}

/// Return whether a package/class symbol exists (Symbol.exists()).
fn symbol_exists_public(ctx: &mut CompilerContext, sym: SymbolId) -> bool {
    (ctx.sym(sym).flags_field & crate::code::flags::EXISTS as i64) != 0
}

impl Check {
    /// Java: MethodSymbol setMethod(MethodSymbol newMethod) (Check.java:217-221).
    ///
    /// # Arguments
    ///   - new_method: the method now being checked.
    ///
    /// # Returns
    ///   - the previous method.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Check.java:217-221.
    pub fn set_method(&mut self, new_method: Option<SymbolId>) -> Option<SymbolId> {
        let prev = self.method;
        self.method = new_method;
        prev
    }

    /// Java: boolean isOverrider(Symbol s) (Check.java:3281-3302).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - s: the symbol to test.
    ///
    /// # Returns
    ///   - true when s overrides a method of a superclass.
    ///
    /// # Panics
    ///   - the symbol type of the owner is missing.
    ///
    /// Upstream: Check.java:3281-3302.
    pub fn is_overrider(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s: SymbolId,
    ) -> bool {
        if ctx.sym(s).kind != crate::code::kinds::Kind::MTH || ctx.sym(s).is_static(ctx) {
            return false;
        }
        let owner = ctx.sym(s).owner.expect("method owner");
        let owner_ty = ctx.sym(owner).ty.expect("owner type");
        let closure = self.types.closure(ctx, syms, owner_ty);
        let mut sup_types = closure;
        while sup_types.non_empty() {
            let sup = *sup_types.head();
            sup_types = sup_types.tail();
            if sup == owner_ty {
                continue;
            }
            let Some(sup_tsym) = ctx.typ(sup).tsym else {
                continue;
            };
            let Some(members) = ctx.sym(sup_tsym).members() else {
                continue;
            };
            let name = ctx.sym(s).name.clone();
            let symbols = ctx.scope_get_symbols_by_name(
                members,
                &name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for sym in symbols {
                if !ctx.sym(sym).is_static(ctx)
                    && crate::code::symbol::overrides(ctx, syms, &self.types, s, sym, owner, true)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Java: void checkOverride(Env<AttrContext> env, JCMethodDecl tree,
    /// MethodSymbol m) (Check.java:2103-2150).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the current environment;
    ///   - tree: the method declaration;
    ///   - m: the overriding method.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - a class invariant is violated.
    ///
    /// Upstream: Check.java:2103-2150.
    #[allow(clippy::too_many_lines)]
    pub fn check_override_env(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
        m: SymbolId,
    ) {
        use crate::code::flags;
        let origin = ctx.sym(m).owner.expect("method owner");
        if (ctx.sym(origin).flags_field & flags::ENUM as i64) != 0
            && ctx.sym(m).name == self.names.finalize
            && crate::code::symbol::overrides(
                ctx,
                syms,
                &self.types,
                m,
                syms.enum_final_finalize,
                origin,
                false,
            )
        {
            self.log.borrow_mut().error_at(
                ctx.tree(tree).pos,
                crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::enum_no_finalize(),
                ),
            );
            return;
        }
        if self.allow_records && ctx.sym(origin).is_record() {
            // A user-defined accessor is not checked for @Override.
            let components = match &ctx.sym(origin).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol {
                    record_components, ..
                } => record_components.clone(),
                _ => List::nil(),
            };
            let mut current = components;
            while current.non_empty() {
                let rc = *current.head();
                current = current.tail();
                if let crate::code::symbol::SymbolKind::RecordComponent {
                    accessor: Some(accessor),
                    ..
                } = ctx.sym(rc).kind_data
                    && accessor == m
                    && (ctx.sym(accessor).flags_field & flags::GENERATED_MEMBER as i64) == 0
                {
                    return;
                }
            }
        }
        let origin_ty = ctx.sym(origin).ty.expect("origin type");
        self.complete_for_members(ctx, syms, origin);
        let mut t = origin_ty;
        loop {
            if !ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                break;
            }
            if let Some(t_tsym) = ctx.typ(t).tsym {
                self.complete_for_members(ctx, syms, t_tsym);
            }
            if t != origin_ty {
                self.check_override_at(ctx, syms, tree, t, origin, m);
            }
            let interfaces = {
                let syms: &crate::code::symtab::Symtab = syms;
                self.types.interfaces(ctx, syms, t)
            };
            let mut list = interfaces;
            while list.non_empty() {
                let t2 = *list.head();
                list = list.tail();
                self.check_override_at(ctx, syms, tree, t2, origin, m);
            }
            let supertype = {
                let syms: &crate::code::symtab::Symtab = syms;
                self.types.supertype(ctx, syms, t)
            };
            if supertype == t {
                break;
            }
            t = supertype;
        }
        let override_tsym = ctx.typ(syms.override_type).tsym.expect("Override type");
        let explicit_override = ctx.sym(m).attribute(ctx, override_tsym).is_some();
        let anonymous_diamond = env.info.borrow().is_anonymous_diamond;
        let must_override = explicit_override
            || (anonymous_diamond
                && !ctx.sym(m).is_constructor(&self.names)
                && !ctx.sym(m).is_private());
        if must_override && !self.is_overrider(ctx, syms, m) {
            let pos = self.override_error_position(ctx, tree, override_tsym, explicit_override);
            let info = if explicit_override {
                if ctx.sym(m).is_static(ctx) {
                    crate::resources::compiler_properties::errors::static_methods_cannot_be_annotated_with_override()
                } else {
                    crate::resources::compiler_properties::errors::method_does_not_override_superclass()
                }
            } else {
                crate::resources::compiler_properties::errors::anonymous_diamond_method_does_not_override_superclass(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        crate::resources::compiler_properties::fragments::diamond_anonymous_methods_implicitly_override()
                            .info()
                            .key(),
                    ),
                )
            };
            self.log
                .borrow_mut()
                .error_at(pos, crate::util::jc_diagnostic::Error::from(info));
        }
    }

    /// The position of the @Override annotation (or the declaration).
    fn override_error_position(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        override_tsym: SymbolId,
        explicit_override: bool,
    ) -> i32 {
        if explicit_override
            && let crate::tree::jc_tree::TreeKind::MethodDecl { mods: Some(mods), .. } =
                &ctx.tree(tree).kind
            && let crate::tree::jc_tree::TreeKind::Modifiers { annotations, .. } =
                &ctx.tree(*mods).kind
        {
            let mut current = annotations.clone();
            while current.non_empty() {
                let anno = *current.head();
                let anno_tsym = ctx.tree(anno).ty.and_then(|ty| ctx.typ(ty).tsym);
                if anno_tsym == Some(override_tsym) {
                    return ctx.tree(anno).pos;
                }
                current = current.tail();
            }
        }
        ctx.tree(tree).pos
    }

    /// Java: void checkVarargsMethodDecl(Env<AttrContext> env, JCMethodDecl tree)
    /// (Check.java:938-990).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - env: the current environment;
    ///   - tree: the method declaration.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - a method invariant is violated.
    ///
    /// Upstream: Check.java:938-990.
    pub fn check_varargs_method_decl(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        _env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
    ) {
        use crate::code::flags;
        let (params, m) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MethodDecl {
                params,
                sym: Some(sym),
                ..
            } => (params.clone(), *sym),
            _ => return,
        };
        let trust_me_tsym = syms.trust_me_type;
        let trust_me_tsym = ctx.typ(trust_me_tsym).tsym.expect("SafeVarargs type");
        let has_trust_me = ctx.sym(m).attribute(ctx, trust_me_tsym).is_some();
        let is_var_args = (ctx.sym(m).flags_field & flags::VARARGS) != 0;
        let mut vararg_elem_type: Option<TypeId> = None;
        if is_var_args
            && let Some(last_param) = params.last()
            && let crate::tree::jc_tree::TreeKind::VariableDecl { vartype: Some(vartype), .. } =
                &ctx.tree(last_param).kind
            && let Some(param_ty) = ctx.tree(*vartype).ty
        {
            vararg_elem_type = Some(self.types.elemtype(ctx, syms, param_ty));
        }
        let private_safe = crate::code::source::Feature::PRIVATE_SAFE_VARARGS
            .allowed_in_source(self.source);
        if has_trust_me && !self.is_trust_me_allowed_on_method(ctx, m) {
            if let Some(elem) = vararg_elem_type {
                let fragment = if private_safe {
                    crate::resources::compiler_properties::fragments::varargs_trustme_on_virtual_varargs(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_name(ctx, m)),
                    )
                } else {
                    crate::resources::compiler_properties::fragments::varargs_trustme_on_virtual_varargs_final_only(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_name(ctx, m)),
                    )
                };
                let _ = elem;
                let info = crate::resources::compiler_properties::errors::varargs_invalid_trustme_anno(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_name(ctx, trust_me_tsym)),
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        fragment.info().key(),
                    ),
                );
                self.log
                    .borrow_mut()
                    .error_at(ctx.tree(tree).pos, crate::util::jc_diagnostic::Error::from(info));
            } else {
                let fragment =
                    crate::resources::compiler_properties::fragments::varargs_trustme_on_non_varargs_meth(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_name(ctx, m)),
                    );
                let info = crate::resources::compiler_properties::errors::varargs_invalid_trustme_anno(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_name(ctx, trust_me_tsym)),
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        fragment.info().key(),
                    ),
                );
                self.log
                    .borrow_mut()
                    .error_at(ctx.tree(tree).pos, crate::util::jc_diagnostic::Error::from(info));
            }
        } else if has_trust_me
            && let Some(elem) = vararg_elem_type
            && self.types.is_reifiable(ctx, syms, elem)
        {
            let fragment =
                crate::resources::compiler_properties::fragments::varargs_trustme_on_reifiable_varargs(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        type_name(ctx, elem),
                    ),
                );
            let warning = crate::resources::compiler_properties::warnings::varargs_redundant_trustme_anno(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_name(ctx, trust_me_tsym)),
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    fragment.info().key(),
                ),
            );
            self.warn_unsafe_vararg(ctx.tree(tree).pos, warning);
        } else if !has_trust_me
            && let Some(elem) = vararg_elem_type
            && !self.types.is_reifiable(ctx, syms, elem)
            && !params.is_empty()
        {
            let first = *params.head();
            let warning =
                crate::resources::compiler_properties::warnings::unchecked_varargs_non_reifiable_type(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(type_name(
                        ctx, elem,
                    )),
                );
            self.warn_unchecked(ctx.tree(first).pos, warning);
        }
    }

    /// Java: void warnUnsafeVararg(DiagnosticPosition pos, Warning warnKey)
    /// (Check.java:???).
    /// Java: Check.ConversionWarner.warn(LintCategory) VARARGS branch
    /// (Check.java:3961-3967).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services;
    ///   - pos: the diagnostic position of the checked expression.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub(crate) fn warn_unsafe_varargs_param(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        pos: i32,
    ) {
        let Some(method) = self.method else {
            return;
        };
        let Some(trust_me_tsym) = ctx.typ(syms.trust_me_type).tsym else {
            return;
        };
        if ctx.sym(method).attribute(ctx, trust_me_tsym).is_none() {
            return;
        }
        if !self.is_trust_me_allowed_on_method(ctx, method) {
            return;
        }
        let params = match &ctx.sym(method).kind_data {
            SymbolKind::MethodSymbol { params, .. } => params.clone(),
            _ => return,
        };
        let Some(last_param) = params.last() else {
            return;
        };
        let Some(last_type) = ctx.sym(last_param).ty else {
            return;
        };
        if self.types.is_reifiable(ctx, syms, last_type) {
            return;
        }
        self.warn_unsafe_vararg(
            pos,
            warnings::varargs_unsafe_use_varargs_param(SymbolArg::placeholder(symbol_name(
                ctx, last_param,
            ))),
        );
    }

    fn warn_unsafe_vararg(&self, pos: i32, warning: crate::resources::diagnostic_contract::WarningInfo) {
        if self.lint.is_enabled(crate::code::lint::LintCategory::VARARGS) {
            self.log
                .borrow_mut()
                .warning_at(pos, crate::util::jc_diagnostic::Warning::from(warning));
        }
    }
}

/// Return the printable name of a symbol (diagnostic argument adapter).
fn symbol_name(ctx: &mut CompilerContext, sym: SymbolId) -> String {
    ctx.sym(sym)
        .name
        .to_java_string()
        .to_string_lossy()
        .to_string()
}

/// Return the printable name of a type (diagnostic argument adapter).
fn type_name(ctx: &mut CompilerContext, ty: TypeId) -> String {
    let tsym = ctx.typ(ty).tsym;
    match tsym {
        Some(sym) => symbol_name(ctx, sym),
        None => format!("{ty:?}"),
    }
}

impl Check {
    /// Java: void checkOverrideClashes(DiagnosticPosition pos, Type site,
    /// MethodSymbol sym) (Check.java:2518-2586).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - site: the class whose methods are checked;
    ///   - sym: the method symbol to be checked.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - a class invariant is violated.
    ///
    /// Upstream: Check.java:2518-2586.
    #[allow(clippy::too_many_lines)]
    pub fn check_override_clashes(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        site: TypeId,
        sym: SymbolId,
    ) {
        use crate::code::flags;
        let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
        let site_name = ctx.sym(site_tsym).name.clone();
        let name = ctx.sym(sym).name.clone();
        let scope = self.types.members_closure(ctx, syms, site);
        let candidates = ctx.scope_get_symbols_by_name(
            scope,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let symbols: Vec<SymbolId> = candidates
            .into_iter()
            .filter(|candidate| {
                let flags_value = ctx.sym(*candidate).flags_field;
                if ctx.sym(*candidate).kind != crate::code::kinds::Kind::MTH
                    || (flags_value & flags::SYNTHETIC as i64) != 0
                {
                    return false;
                }
                let should_skip = (flags_value & flags::CLASH) != 0
                    && ctx.sym(*candidate).owner == Some(site_tsym);
                if should_skip || ctx.sym(*candidate).is_constructor(&self.names) {
                    return false;
                }
                let syms_ref: &crate::code::symtab::Symtab = &*syms;
                crate::code::symbol::is_inherited_in(
                    ctx,
                    syms_ref,
                    &self.types,
                    *candidate,
                    site_tsym,
                )
            })
            .collect();
        let mut potentially_ambiguous: Vec<SymbolId> = Vec::new();
        let mut overrides_any = false;
        for m1 in symbols.iter().copied() {
            let m1_overrides = crate::code::symbol::overrides(
                ctx, syms, &self.types, sym, m1, site_tsym, false,
            );
            if !m1_overrides {
                if m1 == sym {
                    continue;
                }
                if !overrides_any {
                    // Java: potentiallyAmbiguousList = potentiallyAmbiguousList.prepend(m1)
                    potentially_ambiguous.insert(0, m1);
                }
                continue;
            }
            if m1 != sym {
                overrides_any = true;
                potentially_ambiguous.clear();
            }
            for m2 in symbols.iter().copied() {
                if m2 == m1 {
                    continue;
                }
                let sym_ty = ctx.sym(sym).ty.expect("method type");
                let m2_member_ty = {
                    self.types.member_type(ctx, syms, site, m2)
                };
                let strict = crate::code::source::Feature::STRICT_METHOD_CLASH_CHECK
                    .allowed_in_source(self.source);
                let sub_signature = {
                    self.types.is_sub_signature_strict(
                        ctx,
                        syms,
                        sym_ty,
                        m2_member_ty,
                        strict,
                    )
                };
                let m2_erased = {
                    let m2_ty = ctx.sym(m2).ty.expect("method type");
                    self.types.erasure(ctx, syms, m2_ty)
                };
                let m1_erased = {
                    let m1_ty = ctx.sym(m1).ty.expect("method type");
                    self.types.erasure(ctx, syms, m1_ty)
                };
                let same_args = {
                    self.types
                        .has_same_args(ctx, syms, m2_erased, m1_erased)
                };
                if !sub_signature && same_args {
                    ctx.sym_mut(sym).flags_field |= flags::CLASH;
                    let m1_params = self.member_parameter_types(ctx, syms, site, m1);
                    let m2_params = self.member_parameter_types(ctx, syms, site, m2);
                    let m1_name = ctx.sym(m1).name.clone();
                    let m2_name = ctx.sym(m2).name.clone();
                    let info = if m1 == sym {
                        crate::resources::compiler_properties::errors::name_clash_same_erasure_no_override(
                            m1_name,
                            m1_params,
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                symbol_display_name(ctx, m1),
                            ),
                            m2_name,
                            m2_params,
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                symbol_display_name(ctx, m2),
                            ),
                        )
                    } else {
                        let is_interface = (ctx.sym(site_tsym).flags_field
                            & flags::INTERFACE as i64)
                            != 0;
                        let kind = if is_interface {
                            javac_strings::JavaString::from("interface")
                        } else {
                            javac_strings::JavaString::from("class")
                        };
                        crate::resources::compiler_properties::errors::name_clash_same_erasure_no_override1(
                            kind,
                            site_name.clone(),
                            m1_name,
                            m1_params,
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                symbol_display_name(ctx, m1),
                            ),
                            m2_name,
                            m2_params,
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                symbol_display_name(ctx, m2),
                            ),
                        )
                    };
                    self.log
                        .borrow_mut()
                        .error_at(pos, crate::util::jc_diagnostic::Error::from(info));
                    return;
                }
            }
        }
        if !overrides_any {
            for m in potentially_ambiguous {
                self.check_potentially_ambiguous_overloads(ctx, syms, pos, site, sym, m);
            }
        }
    }

    /// Return the parameter type arguments of a member type.
    fn member_parameter_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        site: TypeId,
        sym: SymbolId,
    ) -> Vec<crate::resources::diagnostic_contract::TypeArg> {
        let member_ty = {
            self.types.member_type(ctx, syms, site, sym)
        };
        let params = match &ctx.typ(member_ty).kind {
            crate::code::type_::TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
            crate::code::type_::TypeKindData::ForAll { qtype: Some(qtype), .. } => {
                match &ctx.typ(*qtype).kind {
                    crate::code::type_::TypeKindData::MethodType { argtypes, .. } => {
                        argtypes.clone()
                    }
                    _ => crate::util::list::List::nil(),
                }
            }
            _ => crate::util::list::List::nil(),
        };
        params
            .iter()
            .map(|ty| {
                crate::resources::diagnostic_contract::TypeArg::placeholder(type_display_name(
                    ctx, ty,
                ))
            })
            .collect()
    }

    /// Java: void checkHideClashes(DiagnosticPosition pos, Type site,
    /// MethodSymbol sym) (Check.java).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - site: the class whose methods are checked;
    ///   - sym: the method symbol to be checked.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - a class invariant is violated.
    ///
    /// Upstream: the corresponding Check.java method.
    pub fn check_hide_clashes(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        site: TypeId,
        sym: SymbolId,
    ) {
        use crate::code::flags;
        let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
        let name = ctx.sym(sym).name.clone();
        let scope = self.types.members_closure(ctx, syms, site);
        let candidates = ctx.scope_get_symbols_by_name(
            scope,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for candidate in candidates {
            let flags_value = ctx.sym(candidate).flags_field;
            if ctx.sym(candidate).kind != crate::code::kinds::Kind::MTH
                || (flags_value & flags::SYNTHETIC as i64) != 0
                || ctx.sym(candidate).is_constructor(&self.names)
            {
                continue;
            }
            // Java: MembersScope.combine excludes interface-owned members
            // (Types.java).
            if let Some(owner) = ctx.sym(candidate).owner
                && (ctx.sym(owner).flags_field & flags::INTERFACE as i64) != 0
            {
                continue;
            }
            let should_skip = (flags_value & flags::CLASH) != 0
                && ctx.sym(candidate).owner == Some(site_tsym);
            if should_skip {
                continue;
            }
            let inherited = {
                crate::code::symbol::is_inherited_in(
                    ctx,
                    syms,
                    &self.types,
                    candidate,
                    site_tsym,
                )
            };
            if !inherited {
                continue;
            }
            let sym_ty = ctx.sym(sym).ty.expect("method type");
            let candidate_member_ty = {
                self.types
                    .member_type(ctx, syms, site, candidate)
            };
            let strict = crate::code::source::Feature::STRICT_METHOD_CLASH_CHECK
                .allowed_in_source(self.source);
            let sub_signature = {
                self.types.is_sub_signature_strict(
                    ctx,
                    syms,
                    sym_ty,
                    candidate_member_ty,
                    strict,
                )
            };
            if sub_signature {
                continue;
            }
            let candidate_erased = {
                let candidate_ty = ctx.sym(candidate).ty.expect("method type");
                self.types.erasure(ctx, syms, candidate_ty)
            };
            let sym_erased = {
                self.types.erasure(ctx, syms, sym_ty)
            };
            let same_args = {
                self.types
                    .has_same_args(ctx, syms, candidate_erased, sym_erased)
            };
            if same_args {
                let info = crate::resources::compiler_properties::errors::name_clash_same_erasure_no_hide(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, sym),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, sym),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, candidate),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, candidate),
                    ),
                );
                self.log
                    .borrow_mut()
                    .error_at(pos, crate::util::jc_diagnostic::Error::from(info));
                return;
            }
            self.check_potentially_ambiguous_overloads(ctx, syms, pos, site, sym, candidate);
        }
    }

    /// Java: void checkPotentiallyAmbiguousOverloads(DiagnosticPosition pos,
    /// Type site, MethodSymbol msym1, MethodSymbol msym2) (Check.java).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - site: the class whose methods are checked;
    ///   - msym1/msym2: the two methods.
    ///
    /// # Returns
    ///   - none (warnings are reported).
    ///
    /// # Panics
    ///   - a method invariant is violated.
    ///
    /// Upstream: the corresponding Check.java method.
    #[allow(clippy::too_many_lines)]
    pub fn check_potentially_ambiguous_overloads(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        site: TypeId,
        msym1: SymbolId,
        msym2: SymbolId,
    ) {
        use crate::code::flags;
        if msym1 == msym2
            || !crate::code::source::Feature::DEFAULT_METHODS.allowed_in_source(self.source)
            || !self.lint.is_enabled(crate::code::lint::LintCategory::OVERLOADS)
            || (ctx.sym(msym1).flags_field & flags::POTENTIALLY_AMBIGUOUS) != 0
            || (ctx.sym(msym2).flags_field & flags::POTENTIALLY_AMBIGUOUS) != 0
        {
            return;
        }
        let mt1 = {
            self.types.member_type(ctx, syms, site, msym1)
        };
        let mut mt2 = {
            self.types.member_type(ctx, syms, site, msym2)
        };
        // if both generic methods, adjust type variables
        let (mt1_forall, mt2_forall) = (
            matches!(&ctx.typ(mt1).kind, crate::code::type_::TypeKindData::ForAll { .. }),
            matches!(&ctx.typ(mt2).kind, crate::code::type_::TypeKindData::ForAll { .. }),
        );
        if mt1_forall && mt2_forall {
            let same_bounds = {
                let tvars1 = match &ctx.typ(mt1).kind {
                    crate::code::type_::TypeKindData::ForAll { tvars, .. } => tvars.clone(),
                    _ => crate::util::list::List::nil(),
                };
                let tvars2 = match &ctx.typ(mt2).kind {
                    crate::code::type_::TypeKindData::ForAll { tvars, .. } => tvars.clone(),
                    _ => crate::util::list::List::nil(),
                };
                self.types
                    .has_same_bounds(ctx, syms, &tvars1, &tvars2)
            };
            if same_bounds {
                let (tvars1, tvars2) = (
                    match &ctx.typ(mt1).kind {
                        crate::code::type_::TypeKindData::ForAll { tvars, .. } => tvars.clone(),
                        _ => crate::util::list::List::nil(),
                    },
                    match &ctx.typ(mt2).kind {
                        crate::code::type_::TypeKindData::ForAll { tvars, .. } => tvars.clone(),
                        _ => crate::util::list::List::nil(),
                    },
                );
                mt2 = {
                    self.types
                        .subst_type(ctx, syms, mt2, &tvars2, &tvars1)
                };
            }
        }
        let params1 = match &ctx.typ(mt1).kind {
            crate::code::type_::TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
            _ => crate::util::list::List::nil(),
        };
        let params2 = match &ctx.typ(mt2).kind {
            crate::code::type_::TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
            _ => crate::util::list::List::nil(),
        };
        let max_length = params1.length().max(params2.length()) as usize;
        let (args1, args2) = {
            let args1 = crate::comp::resolve::adjust_args(
                ctx,
                syms,
                &self.types,
                params1,
                Some(msym1),
                max_length,
                true,
            );
            let args2 = crate::comp::resolve::adjust_args(
                ctx,
                syms,
                &self.types,
                params2,
                Some(msym2),
                max_length,
                true,
            );
            (args1, args2)
        };
        if args1.length() != args2.length() {
            return;
        }
        let mut potentially_ambiguous = false;
        let mut l1 = args1;
        let mut l2 = args2;
        while l1.non_empty() && l2.non_empty() {
            let s = *l1.head();
            let t = *l2.head();
            l1 = l1.tail();
            l2 = l2.tail();
            let (sub_t_s, sub_s_t) = {
                (
                    self.types.is_subtype(ctx, syms, t, s),
                    self.types.is_subtype(ctx, syms, s, t),
                )
            };
            if sub_t_s || sub_s_t {
                continue;
            }
            let s_tsym = ctx.typ(s).tsym;
            let t_tsym = ctx.typ(t).tsym;
            let (s_functional, t_functional) = match (s_tsym, t_tsym) {
                (Some(s_tsym), Some(t_tsym)) => {
                    let diags = Rc::clone(&self.diags);
                    let s_functional = self.types.is_functional_interface(
                        ctx,
                        syms,
                        &diags,
                        self,
                        s_tsym,
                    );
                    let t_functional = self.types.is_functional_interface(
                        ctx,
                        syms,
                        &self.diags,
                        self,
                        t_tsym,
                    );
                    (s_functional, t_functional)
                }
                _ => (false, false),
            };
            if s_functional && t_functional {
                let s_params = self.descriptor_parameter_count(ctx, syms, s);
                let t_params = self.descriptor_parameter_count(ctx, syms, t);
                if s_params > 0 && s_params == t_params {
                    potentially_ambiguous = true;
                    continue;
                }
            }
            return;
        }
        if potentially_ambiguous {
            ctx.sym_mut(msym1).flags_field |= flags::POTENTIALLY_AMBIGUOUS;
            ctx.sym_mut(msym2).flags_field |= flags::POTENTIALLY_AMBIGUOUS;
            let warning = crate::resources::compiler_properties::warnings::potentially_ambiguous_overload(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_display_name(
                    ctx, msym1,
                )),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_display_name(
                    ctx, msym1,
                )),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_display_name(
                    ctx, msym2,
                )),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_display_name(
                    ctx, msym2,
                )),
            );
            self.log.borrow_mut().warning(
                Some(crate::code::lint::LintCategory::OVERLOADS),
                Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
                warning_from_info(warning),
            );
        }
    }

    /// Return the parameter count of the functional descriptor of a type.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - ty: the functional interface type.
    ///
    /// # Returns
    ///   - the descriptor parameter count.
    ///
    /// # Panics
    ///   - the descriptor lookup failure is unreachable (guarded by
    ///     isFunctionalInterface); it propagates like the upstream exception.
    fn descriptor_parameter_count(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        ty: TypeId,
    ) -> usize {
        let diags = Rc::clone(&self.diags);
        // Java: types.findDescriptorType(s) may throw FunctionDescriptorLookupError
        // (uncaught upstream). The call site is guarded by isFunctionalInterface(s) &&
        // isFunctionalInterface(t), which proves a descriptor exists, so a failure
        // propagates as the upstream uncaught exception (panic).
        let descriptor = self
            .types
            .find_descriptor_type(ctx, syms, &diags, self, ty)
            .unwrap_or_else(|error| panic!("FunctionDescriptorLookupError: {error:?}"));
        match &ctx.typ(descriptor).kind {
            crate::code::type_::TypeKindData::MethodType { argtypes, .. } => {
                argtypes.length() as usize
            }
            _ => 0,
        }
    }
}

/// Return a printable name for a symbol (diagnostic argument adapter).
fn symbol_display_name(ctx: &mut CompilerContext, sym: SymbolId) -> String {
    ctx.sym(sym)
        .name
        .to_java_string()
        .to_string_lossy()
        .to_string()
}

/// Return a printable name for a type (diagnostic argument adapter).
fn type_display_name(ctx: &mut CompilerContext, ty: TypeId) -> String {
    match ctx.typ(ty).tsym {
        Some(sym) => symbol_display_name(ctx, sym),
        None => format!("{ty:?}"),
    }
}

impl Check {
    /// Validate the return type of an annotation method (the tree form).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - restype: the annotation method return type tree.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - none.
    pub fn validate_annotation_type_tree(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        restype: Option<TreeId>,
    ) {
        let Some(restype) = restype else {
            return;
        };
        let Some(ty) = ctx.tree(restype).ty else {
            return;
        };
        self.validate_annotation_type(ctx, syms, ctx.tree(restype).pos, ty);
    }

    /// Validate an annotation member type.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - ty: the annotation member type.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - none.
    pub fn validate_annotation_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        ty: TypeId,
    ) {
        use crate::code::flags;
        if ctx.typ(ty).is_primitive() {
            return;
        }
        if self.types.is_same_type(ctx, syms, ty, syms.string_type) {
            return;
        }
        if let Some(tsym) = ctx.typ(ty).tsym {
            let flags_value = ctx.sym(tsym).flags_field;
            if (flags_value & flags::ENUM as i64) != 0
                || (flags_value & flags::ANNOTATION as i64) != 0
            {
                return;
            }
        }
        let lower = crate::code::types::cvar_lower_bound(ctx, ty);
        if ctx.typ(lower).tsym == ctx.typ(syms.class_type).tsym {
            return;
        }
        if ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::ARRAY) {
            let elem = self.types.elemtype(ctx, syms, ty);
            if !ctx.typ(elem).has_tag(crate::code::type_tag::TypeTag::ARRAY) {
                self.validate_annotation_type(ctx, syms, pos, elem);
                return;
            }
        }
        self.log.borrow_mut().error_at(
            pos,
            crate::util::jc_diagnostic::Error::from(
                crate::resources::compiler_properties::errors::invalid_annotation_member_type(),
            ),
        );
    }

    /// Check that an annotation method does not clash with Object/Annotation.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - pos: the diagnostic position;
    ///   - m: the annotation method.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - a class invariant is violated.
    pub fn validate_annotation_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pos: i32,
        m: SymbolId,
    ) {
        use crate::code::flags;
        let mut sup = {
            syms.annotation_type
        };
        loop {
            if !ctx.typ(sup).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                break;
            }
            let Some(sup_tsym) = ctx.typ(sup).tsym else {
                break;
            };
            let Some(members) = ctx.sym(sup_tsym).members() else {
                break;
            };
            let name = ctx.sym(m).name.clone();
            let symbols = ctx.scope_get_symbols_by_name(
                members,
                &name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for sym in symbols {
                if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
                    && (ctx.sym(sym).flags_field
                        & (flags::PUBLIC as i64 | flags::PROTECTED as i64))
                        != 0
                {
                    let m_ty = ctx.sym(m).ty.expect("annotation method type");
                    let sym_ty = ctx.sym(sym).ty.expect("clashing method type");
                    let equivalent = self.types.override_equivalent(ctx, syms, m_ty, sym_ty);
                    if equivalent {
                        let info = crate::resources::compiler_properties::errors::intf_annotation_member_clash(
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                symbol_display_name(ctx, sym),
                            ),
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                type_display_name(ctx, sup),
                            ),
                        );
                        self.log
                            .borrow_mut()
                            .error_at(pos, crate::util::jc_diagnostic::Error::from(info));
                    }
                }
            }
            sup = {
                self.types.supertype(ctx, syms, sup)
            };
        }
    }
}

impl Check {
    /// Inject the Enter service (the upstream Check constructor receives it;
    /// the local Check is created before Enter, so the wiring installs it).
    ///
    /// # Arguments
    ///   - enter: the Enter service.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn set_enter(&self, enter: Rc<RefCell<Enter>>) {
        *self.enter.borrow_mut() = Some(enter);
    }

    /// Report a repeated interface.
    ///
    /// # Arguments
    ///   - pos: the diagnostic position;
    ///   - it: the interface type;
    ///   - its: the set of interfaces already seen.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - none.
    pub fn check_not_repeated(
        &self,
        pos: i32,
        it: TypeId,
        its: &mut std::collections::HashSet<TypeId>,
    ) {
        if its.contains(&it) {
            self.log.borrow_mut().error_at(
                pos,
                crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::repeated_interface(),
                ),
            );
        } else {
            its.insert(it);
        }
    }

    /// Check the class declarations of a compilation unit for cycles and mark
    /// acyclic classes.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the class declaration.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - a class invariant is violated.
    pub fn check_non_cyclic_decl(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
    ) {
        let mut cc = CycleChecker {
            seen_classes: std::collections::HashSet::new(),
            error_found: false,
            partial_check: false,
            check: self,
            syms,
        };
        crate::tree::jc_tree::accept(ctx, tree, &mut cc);
        if !cc.error_found && !cc.partial_check {
            let class_sym = match &ctx.tree(tree).kind {
                crate::tree::jc_tree::TreeKind::ClassDecl { sym, .. } => *sym,
                _ => None,
            };
            if let Some(sym) = class_sym {
                ctx.sym_mut(sym).flags_field |= crate::code::flags::ACYCLIC as i64;
            }
        }
        let _ = syms;
    }
}

/// Java: Check.CycleChecker — the cycle checker used by checkNonCyclicDecl.
struct CycleChecker<'a> {
    seen_classes: std::collections::HashSet<SymbolId>,
    error_found: bool,
    partial_check: bool,
    check: &'a Check,
    syms: &'a crate::code::symtab::Symtab,
}

impl CycleChecker<'_> {
    fn check_symbol(&mut self, ctx: &mut CompilerContext, pos: i32, sym: Option<SymbolId>) {
        let Some(sym) = sym else {
            self.partial_check = true;
            return;
        };
        if ctx.sym(sym).kind == crate::code::kinds::Kind::TYP {
            let enter = self.check.enter.borrow().clone();
            let class_env = enter.as_ref().and_then(|enter| enter.borrow().get_env(sym));
            if let Some(class_env) = class_env {
                let Some(toplevel) = class_env.toplevel else {
                    self.partial_check = true;
                    return;
                };
                let sourcefile = match &ctx.tree(toplevel).kind {
                    crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } => {
                        sourcefile.clone()
                    }
                    _ => None,
                };
                let prev = crate::util::abstract_log::AbstractLog::use_source(
                    &mut *self.check.log.borrow_mut(),
                    sourcefile,
                );
                let env_tree = class_env.encl_class.expect("class env tree");
                crate::tree::jc_tree::accept(ctx, env_tree, self);
                crate::util::abstract_log::AbstractLog::use_source(
                    &mut *self.check.log.borrow_mut(),
                    prev,
                );
            } else {
                self.check_class(ctx, Some(pos), sym, List::nil());
            }
        } else if ctx.sym(sym).kind != crate::code::kinds::Kind::PCK {
            self.partial_check = true;
        }
    }

    fn check_class(
        &mut self,
        ctx: &mut CompilerContext,
        pos: Option<i32>,
        c: SymbolId,
        supertypes: List<TreeId>,
    ) {
        use crate::code::flags;
        if (ctx.sym(c).flags_field & flags::ACYCLIC as i64) != 0 {
            return;
        }
        if self.seen_classes.contains(&c) {
            self.error_found = true;
            self.check.note_cyclic(ctx, self.syms, pos, c);
            return;
        }
        let c_ty = match ctx.sym(c).ty {
            Some(ty) => ty,
            None => return,
        };
        if ctx
            .typ(c_ty)
            .has_tag(crate::code::type_tag::TypeTag::ERROR)
        {
            return;
        }
        self.seen_classes.insert(c);
        if ctx.typ(c_ty).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            if supertypes.non_empty() {
                let mut current = supertypes;
                while current.non_empty() {
                    let supertree = *current.head();
                    current = current.tail();
                    crate::tree::jc_tree::accept(ctx, supertree, self);
                }
            } else {
                let (supertype_field, interfaces_field) = match &ctx.typ(c_ty).kind {
                    crate::code::type_::TypeKindData::ClassType {
                        supertype_field,
                        interfaces_field,
                        ..
                    } => (*supertype_field, interfaces_field.clone()),
                    _ => (None, None),
                };
                let (Some(supertype_field), Some(interfaces_field)) =
                    (supertype_field, interfaces_field)
                else {
                    self.partial_check = true;
                    self.seen_classes.remove(&c);
                    return;
                };
                let super_tsym = ctx.typ(supertype_field).tsym;
                self.check_symbol(ctx, pos.unwrap_or(crate::util::position::NOPOS), super_tsym);
                let mut current = interfaces_field;
                while current.non_empty() {
                    let intf = *current.head();
                    current = current.tail();
                    let intf_tsym = ctx.typ(intf).tsym;
                    self.check_symbol(ctx, pos.unwrap_or(crate::util::position::NOPOS), intf_tsym);
                }
            }
            if let Some(owner) = ctx.sym(c).owner
                && ctx.sym(owner).kind == crate::code::kinds::Kind::TYP
            {
                self.check_symbol(ctx, pos.unwrap_or(crate::util::position::NOPOS), Some(owner));
            }
        }
        self.seen_classes.remove(&c);
    }
}

impl crate::tree::tree_scanner::TreeScannerVisitor for CycleChecker<'_> {
    /// Java: public void visitSelect(JCFieldAccess tree)
    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selected, sym) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::FieldAccess { selected, sym, .. } => {
                (*selected, *sym)
            }
            _ => (None, None),
        };
        if let Some(selected) = selected {
            crate::tree::jc_tree::accept(ctx, selected, self);
        }
        let pos = ctx.tree(tree).pos;
        self.check_symbol(ctx, pos, sym);
    }

    /// Java: public void visitIdent(JCIdent tree)
    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Ident { sym, .. } => *sym,
            _ => None,
        };
        let pos = ctx.tree(tree).pos;
        self.check_symbol(ctx, pos, sym);
    }

    /// Java: public void visitTypeApply(JCTypeApply tree)
    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let crate::tree::jc_tree::TreeKind::TypeApply { clazz, .. } = &ctx.tree(tree).kind
            && let Some(clazz) = *clazz
        {
            crate::tree::jc_tree::accept(ctx, clazz, self);
        }
    }

    /// Java: public void visitTypeArray(JCArrayTypeTree tree)
    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let crate::tree::jc_tree::TreeKind::ArrayTypeTree { elemtype } = &ctx.tree(tree).kind
            && let Some(elemtype) = *elemtype
        {
            crate::tree::jc_tree::accept(ctx, elemtype, self);
        }
    }

    /// Java: public void visitClassDef(JCClassDecl tree)
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (extending, implementing, sym) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl {
                extending,
                implementing,
                sym,
                ..
            } => (*extending, implementing.clone(), *sym),
            _ => return,
        };
        let mut supertypes: List<TreeId> = List::nil();
        if let Some(extending) = extending {
            supertypes = supertypes.prepend(extending);
        }
        let mut current = implementing;
        while current.non_empty() {
            let intf = *current.head();
            current = current.tail();
            supertypes = supertypes.prepend(intf);
        }
        let Some(sym) = sym else {
            return;
        };
        let pos = ctx.tree(tree).pos;
        self.check_class(ctx, Some(pos), sym, supertypes);
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(CycleChecker<'_>);

impl Check {
    /// Check for a default constructor in an exported package (the
    /// missing-explicit-ctor lint).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - pos: the class declaration position;
    ///   - c: the class symbol.
    ///
    /// # Returns
    ///   - none (a deferred lint warning may be scheduled).
    ///
    /// # Panics
    ///   - none.
    pub fn check_default_constructor(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        c: SymbolId,
    ) {
        use crate::code::flags;
        if !self
            .lint
            .is_enabled(crate::code::lint::LintCategory::MISSING_EXPLICIT_CTOR)
        {
            return;
        }
        let c_flags = ctx.sym(c).flags_field;
        if (c_flags & (flags::ENUM as i64 | flags::RECORD)) != 0 {
            return;
        }
        if ctx.sym(c).is_anonymous() {
            return;
        }
        if (c_flags & (flags::PUBLIC as i64 | flags::PROTECTED as i64)) == 0 {
            return;
        }
        if !crate::code::source::Feature::MODULES.allowed_in_source(self.source) {
            return;
        }
        match ctx.sym(c).get_nesting_kind(ctx) {
            crate::code::kinds::NestingKind::Anonymous
            | crate::code::kinds::NestingKind::Local => return,
            crate::code::kinds::NestingKind::TopLevel => {}
            crate::code::kinds::NestingKind::Member => {
                let mut owner = ctx.sym(c).owner;
                while let Some(current) = owner
                    && ctx.sym(current).kind == crate::code::kinds::Kind::TYP
                {
                    if (ctx.sym(current).flags_field
                        & (flags::PUBLIC as i64 | flags::PROTECTED as i64))
                        == 0
                    {
                        return;
                    }
                    owner = ctx.sym(current).owner;
                }
            }
        }
        let Some(packge) = crate::code::symbol::packge(ctx, c) else {
            return;
        };
        let (pkg_fullname, pkg_modle) = match &ctx.sym(packge).kind_data {
            crate::code::symbol::SymbolKind::PackageSymbol {
                fullname, modle, ..
            } => (fullname.clone(), *modle),
            _ => return,
        };
        if pkg_fullname.is_empty() {
            return;
        }
        let Some(modle) = pkg_modle else {
            return;
        };
        let exports = match &ctx.sym(modle).kind_data {
            crate::code::symbol::SymbolKind::ModuleSymbol { exports, .. } => exports.clone(),
            _ => return,
        };
        let c_display = ctx
            .sym(c)
            .name
            .to_java_string()
            .to_string_lossy()
            .to_string();
        let pkg_display = pkg_fullname.to_java_string().to_string_lossy().to_string();
        let mod_display = match &ctx.sym(modle).kind_data {
            crate::code::symbol::SymbolKind::ModuleSymbol { .. } => ctx
                .sym(modle)
                .name
                .to_java_string()
                .to_string_lossy()
                .to_string(),
            _ => String::new(),
        };
        let mut current = exports;
        while current.non_empty() {
            let directive = *current.head();
            current = current.tail();
            if let crate::code::directive::Directive::Exports {
                packge: Some(exported_packge),
                modules,
                ..
            } = ctx.directive(directive)
            {
                if *exported_packge != packge {
                    continue;
                }
                let unqualified = modules.as_ref().map(|m| m.is_empty()).unwrap_or(true);
                if !unqualified {
                    return;
                }
                let log = Rc::clone(&self.log);
                let lint = Rc::clone(&self.lint);
                let c_display = c_display.clone();
                let pkg_display = pkg_display.clone();
                let mod_display = mod_display.clone();
                self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
                    if lint
                        .is_enabled(crate::code::lint::LintCategory::MISSING_EXPLICIT_CTOR)
                    {
                        let info = crate::resources::compiler_properties::warnings::missing_explicit_ctor(
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                c_display.clone(),
                            ),
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                pkg_display.clone(),
                            ),
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                mod_display.clone(),
                            ),
                        );
                        log.borrow_mut().warning(
                            Some(crate::code::lint::LintCategory::MISSING_EXPLICIT_CTOR),
                            Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
                            warning_from_info(info),
                        );
                    }
                }));
                return;
            }
        }
    }
}

impl Check {
    /// Check that a qualified name is in canonical form (for import
    /// declarations) and report ImportRequiresCanonical otherwise.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the qualified name tree.
    ///
    /// # Returns
    ///   - none (diagnostics are reported).
    ///
    /// # Panics
    ///   - none.
    pub fn check_canonical(&self, ctx: &mut CompilerContext, tree: TreeId) {
        if !self.is_canonical(ctx, tree) {
            let symbol = crate::tree::tree_info::symbol(ctx, tree);
            let display = symbol
                .map(|symbol| {
                    ctx.sym(symbol)
                        .name
                        .to_java_string()
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or_default();
            self.log.borrow_mut().error_at(
                ctx.tree(tree).pos,
                crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::import_requires_canonical(
                        SymbolArg::placeholder(display),
                    ),
                ),
            );
        }
    }

    /// Java: private boolean isCanonical(JCTree tree) (Check.java:3875-3888).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the qualified name tree.
    ///
    /// # Returns
    ///   - whether the qualified name is canonical.
    ///
    /// # Panics
    ///   - none.
    fn is_canonical(&self, ctx: &CompilerContext, tree: TreeId) -> bool {
        let mut current = tree;
        loop {
            let (selected, sym) = match &ctx.tree(current).kind {
                crate::tree::jc_tree::TreeKind::FieldAccess {
                    selected: Some(selected),
                    sym: Some(sym),
                    ..
                } => (*selected, *sym),
                _ => return true,
            };
            let owner_name = ctx
                .sym(sym)
                .owner
                .map(|owner| ctx.sym(owner).get_qualified_name().to_java_string());
            let selected_sym = crate::tree::tree_info::symbol(ctx, selected);
            let selected_name = selected_sym
                .map(|selected_sym| ctx.sym(selected_sym).get_qualified_name().to_java_string());
            if owner_name != selected_name {
                return false;
            }
            current = selected;
        }
    }
}

impl Check {
    /// Ensure a class symbol has been completed before its members are read.
    ///
    /// Adapter: upstream ClassSymbol.members() calls complete() implicitly
    /// (Symbol.java:1335-1338); the port expresses the completion at the
    /// accessor call sites because the local members() cannot take the
    /// compiler services. Completion failures are reported and the access
    /// continues with the (empty) members scope, mirroring the upstream
    /// error-recovery scope.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - sym: the symbol to complete (only class symbols are completed).
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    fn complete_for_members(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) {
        if ctx.sym(sym).kind != crate::code::kinds::Kind::TYP {
            return;
        }
        if ctx.sym(sym).members().is_some() {
            return;
        }
        if let Err(failure) = ctx.complete_class_symbol(syms, sym) {
            self.completion_error(ctx, crate::util::position::NOPOS, &failure, syms);
        }
    }
}

/// Java: String.valueOf(tree.clazz.type) for the RedundantCast warning argument.
fn type_display_str(ctx: &mut CompilerContext, ty: TypeId) -> String {
    crate::code::type_::type_to_string(ctx, ty)
}

/// Java: private boolean is292targetTypeCast(JCTypeCast tree) (Check.java:660-671).
fn is292_target_type_cast(ctx: &mut CompilerContext, tree: TreeId) -> bool {
    use crate::code::kinds::Kind;
    let expr = match &ctx.tree(tree).kind {
        crate::tree::jc_tree::TreeKind::TypeCast { expr, .. } => expr.expect("cast expression"),
        _ => panic!("ClassCastException: JCTypeCast expected"),
    };
    let expr = crate::tree::tree_info::skip_parens(ctx, expr);
    let crate::tree::jc_tree::TreeKind::MethodInvocation { meth, .. } = &ctx.tree(expr).kind else {
        return false;
    };
    let method = *meth;
    let Some(symbol) = crate::tree::tree_info::symbol(ctx, method.expect("method expression"))
    else {
        return false;
    };
    ctx.sym(symbol).kind == Kind::MTH
        && (ctx.sym(symbol).flags_field & crate::code::flags::HYPOTHETICAL) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::kinds::Kind;
    use crate::code::symtab::Symtab as TestSymtab;
    use crate::comp::infer::InferenceContext;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;

    struct FreeTypeCheckContext {
        deferred: Rc<DeferredAttrContext>,
        inference: Rc<InferenceContext>,
        compatible_calls: std::rc::Rc<std::cell::Cell<usize>>,
    }

    impl CheckContext for FreeTypeCheckContext {
        fn inference_context(&self) -> &Rc<InferenceContext> {
            &self.inference
        }

        fn compatible(
            &self,
            _ctx: &mut CompilerContext,
            _syms: &TestSymtab,
            _types: &Types,
            _found: TypeId,
            _req: TypeId,
            _warn: &mut Warner,
        ) -> bool {
            self.compatible_calls
                .set(self.compatible_calls.get() + 1);
            true
        }

        fn report(
            &self,
            _ctx: &mut CompilerContext,
            _pos: i32,
            _details: &JCDiagnostic,
        ) {
        }

        fn check_warner(
            &self,
            _ctx: &mut CompilerContext,
            _syms: &TestSymtab,
            _types: &Types,
            _pos: i32,
            _found: TypeId,
            _req: TypeId,
        ) -> Warner {
            Warner::new(None)
        }

        fn deferred_attr_context(&self) -> &Rc<DeferredAttrContext> {
            &self.deferred
        }
    }

    #[test]
    fn free_type_listener_rechecks_the_instantiated_types() {
        use crate::code::source::Source;
        use crate::code::type_::Type;
        use crate::code::type_::TypeKindData;
        use crate::code::types::Types;
        use crate::jvm::target::Target;

        let mut ctx = CompilerContext::new();
        let mut jcontext = Context::new();
        let names = Rc::new(crate::util::names::Names::new(false));
        let log = crate::util::log::Log::instance(&mut jcontext);
        let source = Source::JDK17;
        let syms = Rc::new(RefCell::new(TestSymtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::DEFAULT,
            source,
        )));
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        let messages = JavacMessages::instance(&mut jcontext);
        let diags = Rc::new(crate::util::jc_diagnostic::Factory::new(messages, "compiler"));
        let lint = crate::code::lint::Lint::instance(&mut jcontext);
        let deferred = Rc::new(RefCell::new(DeferredLintHandler::new()));
        let check = Rc::new(RefCell::new(Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            source,
            '$',
            Rc::clone(&types),
            Rc::clone(&lint),
            Rc::clone(&deferred),
            Rc::clone(&syms),
        )));
        crate::comp::check::attach_self(&check);
        attach_self(&check);
        let (object_type, string_type) = {
            let syms = syms.borrow();
            (syms.object_type, syms.string_type)
        };
        let tvar = {
            let name = names.from_utf(b"T");
            let symbol = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                Kind::TYP,
                0,
                name,
                None,
                Some(syms.borrow().no_symbol),
                crate::code::symbol::SymbolKind::TypeVariableSymbol,
            ));
            let ty = ctx.alloc_type(Type::new(
                TypeTag::TYPEVAR,
                Some(symbol),
                TypeKindData::TypeVar {
                    bound: Some(object_type),
                    lower: Some(syms.borrow().bot_type),
                },
            ));
            ctx.sym_mut(symbol).ty = Some(ty);
            ty
        };
        let infer = crate::comp::infer::Infer::new(
            &mut ctx,
            true,
            Rc::clone(&diags),
            Rc::clone(&names),
        );
        let syms_guard = syms.borrow_mut();
        let inference = InferenceContext::new(
            Rc::clone(&infer),
            &mut ctx,
            &syms_guard,
            &types,
            List::of(tvar),
        );
        drop(syms_guard);
        assert!(
            inference.free(&mut ctx, &syms.borrow(), &types, tvar),
            "the type variable is free in the context"
        );
        let calls = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let check_context: Rc<dyn CheckContext> = Rc::new(FreeTypeCheckContext {
            inference: Rc::clone(&inference),
            deferred: crate::comp::deferred_attr::empty_deferred_attr_context(),
            compatible_calls: Rc::clone(&calls),
        });
        // The listener is registered; the immediate check also consults the
        // context once.
        check.borrow().check_type(
            &mut ctx,
            &syms.borrow(),
            0,
            tvar,
            tvar,
            Rc::clone(&check_context),
        );
        assert_eq!(calls.get(), 1, "the immediate check runs");
        let uv = inference.as_undet_var(&mut ctx, &syms.borrow(), &types, tvar);
        types.undet_var_set_inst(&mut ctx, &syms.borrow(), uv, string_type);
        let syms_for_notify = syms.borrow_mut();
        inference
            .notify_change(&mut ctx, &syms_for_notify, &types)
            .expect("notify_change");
        assert_eq!(
            calls.get(),
            2,
            "the free type listener re-checks the instantiated types"
        );
    }
}

impl Check {
    /// Java: void checkModuleExists(final DiagnosticPosition pos,
    /// ModuleSymbol msym) (Check.java:4259-4266).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - pos: the diagnostic position;
    ///   - msym: the module symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn check_module_exists(&self, ctx: &mut CompilerContext, pos: i32, msym: SymbolId) {
        if ctx.sym(msym).kind != crate::code::kinds::Kind::MDL {
            let lint = Rc::clone(&self.lint);
            let log = Rc::clone(&self.log);
            let name = ctx.sym(msym).name.to_java_string().to_string_lossy();
            let warning = crate::util::jc_diagnostic::Warning::from(
                crate::resources::compiler_properties::warnings::module_not_found(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(name),
                ),
            );
            let position: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
                Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos));
            self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
                if lint.is_enabled(crate::code::lint::LintCategory::MODULE) {
                    log.borrow_mut().warning_lint_pos(
                        crate::code::lint::LintCategory::MODULE,
                        Some(Rc::clone(&position)),
                        warning.clone(),
                    );
                }
            }));
        }
    }
}

impl Check {
    /// Installs the Preview service (Java: final Preview preview; the ported
    /// Preview is created after Check by the bootstrap; registered adapter).
    ///
    /// # Arguments
    ///   - preview: the preview feature service.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn set_preview(&mut self, preview: Rc<Preview>) {
        *self.preview.borrow_mut() = Some(preview);
    }

    /// Installs the Resolve service (Java: private final Resolve rs;
    /// Check.java:87; created after Check by the ported wiring, registered
    /// adapter).
    ///
    /// # Arguments
    ///   - resolve: the shared resolve service.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn set_resolve(&self, resolve: Rc<RefCell<crate::comp::resolve::Resolve>>) {
        *self.resolve.borrow_mut() = Some(resolve);
    }

    /// Installs the file manager (Java: private final JavaFileManager
    /// fileManager; Check.java:95).
    ///
    /// # Arguments
    ///   - file_manager: the shared JavaFileManager service.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn set_file_manager(
        &self,
        file_manager: Rc<dyn javac_tools::tools::java_file_manager::JavaFileManager>,
    ) {
        *self.file_manager.borrow_mut() = Some(file_manager);
    }

    /// Installs the profile service (Java: private final Profile profile;
    /// Check.java:98).
    ///
    /// # Arguments
    ///   - profile: the shared target profile.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn set_profile(&self, profile: Rc<crate::jvm::profile::Profile>) {
        *self.profile.borrow_mut() = Some(profile);
    }

    /// Java: void warnDeprecated(DiagnosticPosition pos, Symbol sym)
    /// (Check.java:227-247), called from checkDeprecated's deferred report.
    fn warn_deprecated(
        &self,
        ctx: &CompilerContext,
        pos: i32,
        sym: SymbolId,
    ) {
        let removal = ctx.sym(sym).is_deprecated_for_removal();
        let is_module = ctx.sym(sym).kind == Kind::MDL;
        let sym_name = ctx.sym(sym).name.to_java_string().to_string_lossy();
        let loc_name = ctx
            .sym(sym)
            .location()
            .map(|loc| ctx.sym(loc).name.to_java_string().to_string_lossy())
            .unwrap_or_default();
        let lint = Rc::clone(&self.lint);
        let deprecation_handler = Rc::clone(&self.deprecation_handler);
        let removal_handler = Rc::clone(&self.removal_handler);
        let position: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
            Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos));
        self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
            if removal {
                if !lint.is_suppressed(LintCategory::REMOVAL) {
                    if is_module {
                        removal_handler.borrow_mut().report(
                            Some(Rc::clone(&position)),
                            crate::util::jc_diagnostic::Warning::from(
                                crate::resources::compiler_properties::warnings::
                                    has_been_deprecated_for_removal_module(
                                        SymbolArg::placeholder(sym_name.clone()),
                                    ),
                            ),
                        );
                    } else {
                        removal_handler.borrow_mut().report(
                            Some(Rc::clone(&position)),
                            crate::util::jc_diagnostic::Warning::from(
                                crate::resources::compiler_properties::warnings::
                                    has_been_deprecated_for_removal(
                                        SymbolArg::placeholder(sym_name.clone()),
                                        SymbolArg::placeholder(loc_name.clone()),
                                    ),
                            ),
                        );
                    }
                }
            } else if !lint.is_suppressed(LintCategory::DEPRECATION) {
                if is_module {
                    deprecation_handler.borrow_mut().report(
                        Some(Rc::clone(&position)),
                        crate::util::jc_diagnostic::Warning::from(
                            crate::resources::compiler_properties::warnings::
                                has_been_deprecated_module(SymbolArg::placeholder(sym_name.clone())),
                        ),
                    );
                } else {
                    deprecation_handler.borrow_mut().report(
                        Some(Rc::clone(&position)),
                        crate::util::jc_diagnostic::Warning::from(
                            crate::resources::compiler_properties::warnings::
                                has_been_deprecated(
                                    SymbolArg::placeholder(sym_name.clone()),
                                    SymbolArg::placeholder(loc_name.clone()),
                                ),
                        ),
                    );
                }
            }
        }));
    }

    /// Java: void warnPreviewAPI(DiagnosticPosition pos, Warning warnKey)
    /// (Check.java:249-253).
    fn warn_preview_api(
        &self,
        pos: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>,
        warning: crate::util::jc_diagnostic::Warning,
    ) {
        if !self.lint.is_suppressed(LintCategory::PREVIEW) {
            let preview = self
                .preview
                .borrow()
                .clone()
                .expect("AssertionError: preview service not installed");
            preview.report_preview_warning(pos.as_ref(), warning);
        }
    }

    /// Java: void warnDeclaredUsingPreview(DiagnosticPosition pos, Symbol sym)
    /// (Check.java:260-263).
    fn warn_declared_using_preview(
        &self,
        ctx: &CompilerContext,
        pos: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>,
        sym: SymbolId,
    ) {
        if !self.lint.is_suppressed(LintCategory::PREVIEW) {
            let preview = self
                .preview
                .borrow()
                .clone()
                .expect("AssertionError: preview service not installed");
            let kind_name = kind_name_of(ctx, sym);
            let sym_name = ctx.sym(sym).name.to_java_string().to_string_lossy();
            let warning = crate::util::jc_diagnostic::Warning::from(
                crate::resources::compiler_properties::warnings::declared_using_preview(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(kind_name),
                    SymbolArg::placeholder(sym_name),
                ),
            );
            preview.report_preview_warning(pos.as_ref(), warning);
        }
    }

    /// Java: void checkSunAPI(final DiagnosticPosition pos, final Symbol s)
    /// (Check.java:3566-3572).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - pos: the diagnostic position;
    ///   - s: the referenced symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn check_sun_api(&self, ctx: &CompilerContext, pos: i32, s: SymbolId) {
        if (ctx.sym(s).flags_field & crate::code::flags::PROPRIETARY) != 0 {
            let warning = warning_from_info(warnings::sun_proprietary(SymbolArg::placeholder(
                crate::comp::resolve::symbol_display_name(ctx, s),
            )));
            let log = Rc::clone(&self.log);
            let position: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
                Rc::new(SimpleDiagnosticPosition::new(pos));
            self.deferred_lint_handler
                .borrow_mut()
                .report(Box::new(move || {
                    AbstractLog::mandatory_warning_pos(
                        &mut *log.borrow_mut(),
                        Some(position.clone()),
                        warning.clone(),
                    );
                }));
        }
    }

    /// Java: void checkProfile(final DiagnosticPosition pos, final Symbol s)
    /// (Check.java:3574-3578).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - pos: the diagnostic position;
    ///   - s: the referenced symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the profile service must be installed by the wiring.
    pub fn check_profile(&self, ctx: &CompilerContext, pos: i32, s: SymbolId) {
        let profile = **self
            .profile
            .borrow()
            .as_ref()
            .expect("AssertionError: profile service not installed");
        if profile != crate::jvm::profile::Profile::DEFAULT
            && (ctx.sym(s).flags_field & crate::code::flags::NOT_IN_PROFILE) != 0
        {
            self.report_error(
                pos,
                errors::not_in_profile(
                    SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, s)),
                    DiagnosticArg::from_display(profile),
                ),
            );
        }
    }

    /// Java: void checkForBadAuxiliaryClassAccess(DiagnosticPosition pos,
    /// Env<AttrContext> env, ClassSymbol c) (Check.java:3877-3886).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services;
    ///   - pos: the diagnostic position;
    ///   - env: the attribution environment;
    ///   - c: the referenced class symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the resolve and file manager services must be installed by the wiring.
    pub fn check_for_bad_auxiliary_class_access(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        pos: i32,
        env: &Rc<Env<AttrContext>>,
        c: SymbolId,
    ) {
        if !self.lint.is_enabled(LintCategory::AUXILIARYCLASS)
            || (ctx.sym(c).flags_field & crate::code::flags::AUXILIARY) == 0
        {
            return;
        }
        let resolve = Rc::clone(
            self.resolve
                .borrow()
                .as_ref()
                .expect("AssertionError: resolve service not installed"),
        );
        if !resolve
            .borrow_mut()
            .is_accessible_class(ctx, syms, env, c, false)
        {
            return;
        }
        let c_sourcefile = match &ctx.sym(c).kind_data {
            SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile.clone(),
            _ => None,
        };
        let Some(c_sourcefile) = c_sourcefile else {
            return;
        };
        let env_sourcefile = env.toplevel.and_then(|toplevel| match &ctx.tree(toplevel).kind {
            TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
            _ => None,
        });
        let Some(env_sourcefile) = env_sourcefile else {
            return;
        };
        let file_manager = Rc::clone(
            self.file_manager
                .borrow()
                .as_ref()
                .expect("AssertionError: file manager service not installed"),
        );
        let a: javac_tools::tools::java_file_manager::FileObjectRef = c_sourcefile.clone();
        let b: javac_tools::tools::java_file_manager::FileObjectRef = env_sourcefile.clone();
        if !javac_tools::tools::java_file_manager::JavaFileManager::is_same_file(
            &*file_manager,
            &a,
            &b,
        ) {
            let warning = warning_from_info(
                warnings::auxiliary_class_accessed_from_outside_of_its_source_file(
                    DiagnosticArg::from_display(crate::comp::resolve::symbol_display_name(ctx, c)),
                    DiagnosticArg::from_display(c_sourcefile.get_name().to_string_lossy()),
                ),
            );
            self.log.borrow_mut().warning(
                None,
                Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
                warning,
            );
        }
    }

    /// Java: private void warnOnExplicitStrictfp(DiagnosticPosition pos)
    /// (Check.java:1326-1337).
    ///
    /// # Arguments
    ///   - pos: the diagnostic position of the explicit strictfp modifier.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    fn warn_on_explicit_strictfp(&self, pos: i32) {
        let position: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
            Rc::new(SimpleDiagnosticPosition::new(pos));
        let previous_pos = self
            .deferred_lint_handler
            .borrow_mut()
            .set_pos(Rc::clone(&position));
        let log = Rc::clone(&self.log);
        let lint = Rc::clone(&self.lint);
        self.deferred_lint_handler
            .borrow_mut()
            .report(Box::new(move || {
                if lint.is_enabled(LintCategory::STRICTFP) {
                    log.borrow_mut().warning(
                        Some(LintCategory::STRICTFP),
                        Some(position.clone()),
                        warning_from_info(warnings::strictfp()),
                    );
                }
            }));
        self.deferred_lint_handler
            .borrow_mut()
            .set_pos_state(previous_pos);
    }

    /// Java: void checkDeprecated(final DiagnosticPosition pos,
    /// final Symbol other, final Symbol s) (Check.java:3553-3564).
    pub fn check_deprecated(
        &self,
        ctx: &CompilerContext,
        pos: i32,
        other: SymbolId,
        s: SymbolId,
    ) {
        let s_outermost = outermost_class(ctx, s);
        let other_outermost = outermost_class(ctx, other);
        if (ctx.sym(s).is_deprecated_for_removal()
            || (ctx.sym(s).is_deprecated() && !ctx.sym(other).is_deprecated()))
            && (s_outermost != other_outermost || s_outermost.is_none())
            && ctx.sym(s).kind != Kind::PCK
        {
            self.warn_deprecated(ctx, pos, s);
        }
    }

    /// Java: void checkPreview(DiagnosticPosition pos, Symbol other, Symbol s)
    /// (Check.java:3580-3602).
    pub fn check_preview(
        &self,
        ctx: &CompilerContext,
        pos: i32,
        other: SymbolId,
        s: SymbolId,
    ) {
        let preview = self
            .preview
            .borrow()
            .clone()
            .expect("AssertionError: preview service not installed");
        let flags_field = ctx.sym(s).flags_field;
        let same_module = package_module_of(ctx, s) == package_module_of(ctx, other);
        let position: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
            Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos));
        if (flags_field & crate::code::flags::PREVIEW_API) != 0 && !same_module {
            if (flags_field & crate::code::flags::PREVIEW_REFLECTIVE) == 0 {
                if !preview.is_enabled() {
                    let sym_name = ctx.sym(s).name.to_java_string().to_string_lossy();
                    self.log.borrow_mut().error_at(
                        pos,
                        crate::util::jc_diagnostic::Error::from(
                            crate::resources::compiler_properties::errors::is_preview(
                                SymbolArg::placeholder(sym_name),
                            ),
                        ),
                    );
                } else {
                    preview.mark_uses_preview(position.as_ref());
                    let lint = Rc::clone(&self.lint);
                    let preview_for_warning = Rc::clone(&preview);
                    let pos_for_warning = Rc::clone(&position);
                    let sym_name = ctx.sym(s).name.to_java_string().to_string_lossy();
                    let warning = crate::util::jc_diagnostic::Warning::from(
                        crate::resources::compiler_properties::warnings::is_preview(
                            SymbolArg::placeholder(sym_name),
                        ),
                    );
                    self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
                        if !lint.is_suppressed(LintCategory::PREVIEW) {
                            preview_for_warning
                                .report_preview_warning(pos_for_warning.as_ref(), warning.clone());
                        }
                    }));
                }
            } else {
                let lint = Rc::clone(&self.lint);
                let preview_for_warning = Rc::clone(&preview);
                let pos_for_warning = Rc::clone(&position);
                let sym_name = ctx.sym(s).name.to_java_string().to_string_lossy();
                let warning = crate::util::jc_diagnostic::Warning::from(
                    crate::resources::compiler_properties::warnings::is_preview_reflective(
                        SymbolArg::placeholder(sym_name),
                    ),
                );
                self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
                    if !lint.is_suppressed(LintCategory::PREVIEW) {
                        preview_for_warning
                            .report_preview_warning(pos_for_warning.as_ref(), warning.clone());
                    }
                }));
            }
        }
        if preview.declared_using_preview_feature(ctx.sym(s))
            && preview.is_enabled() {
                preview.mark_uses_preview(position.as_ref());
                let lint = Rc::clone(&self.lint);
                let preview_for_warning = Rc::clone(&preview);
                let pos_for_warning = Rc::clone(&position);
                let kind_name = kind_name_of(ctx, s);
                let sym_name = ctx.sym(s).name.to_java_string().to_string_lossy();
                let warning = crate::util::jc_diagnostic::Warning::from(
                    crate::resources::compiler_properties::warnings::declared_using_preview(
                        crate::resources::diagnostic_contract::KindNameArg::placeholder(kind_name),
                        SymbolArg::placeholder(sym_name),
                    ),
                );
                self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
                    if !lint.is_suppressed(LintCategory::PREVIEW) {
                        preview_for_warning
                            .report_preview_warning(pos_for_warning.as_ref(), warning.clone());
                    }
                }));
            }
    }

    /// Java: void checkPackageExistsForOpens(final DiagnosticPosition pos,
    /// PackageSymbol packge) (Check.java:4268-4276).
    pub fn check_package_exists_for_opens(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        packge: SymbolId,
    ) {
        let members_empty = match ctx.sym(packge).members() {
            None => true,
            Some(scope) => ctx
                .scope_get_symbols(scope, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true)
                .is_empty(),
        };
        if members_empty && (ctx.sym(packge).flags_field & crate::code::flags::HAS_RESOURCE) == 0 {
            let lint = Rc::clone(&self.lint);
            let log = Rc::clone(&self.log);
            let sym_name = ctx.sym(packge).name.to_java_string().to_string_lossy();
            let warning = crate::util::jc_diagnostic::Warning::from(
                crate::resources::compiler_properties::warnings::package_empty_or_not_found(
                    SymbolArg::placeholder(sym_name),
                ),
            );
            let position: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
                Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos));
            self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
                if lint.is_enabled(LintCategory::OPENS) {
                    log.borrow_mut().warning_lint_pos(
                        LintCategory::OPENS,
                        Some(Rc::clone(&position)),
                        warning.clone(),
                    );
                }
            }));
        }
    }

    /// Java: void checkModuleRequires(final DiagnosticPosition pos,
    /// final RequiresDirective rd) (Check.java:4278-4288).
    pub fn check_module_requires(
        &self,
        ctx: &CompilerContext,
        pos: i32,
        rd: crate::ids::DirectiveId,
    ) {
        let (module, is_transitive) = match ctx.directive(rd) {
            crate::code::directive::Directive::Requires { module, flags } => {
                (*module, flags.contains(&crate::code::directive::RequiresFlag::TRANSITIVE))
            }
            _ => return,
        };
        let Some(module) = module else { return };
        if (ctx.sym(module).flags_field & crate::code::flags::AUTOMATIC_MODULE) != 0 {
            let lint = Rc::clone(&self.lint);
            let log = Rc::clone(&self.log);
            let position: Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
                Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos));
            self.deferred_lint_handler.borrow_mut().report(Box::new(move || {
                if is_transitive
                    && lint.is_enabled(LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC)
                {
                    log.borrow_mut().warning_lint_pos(
                        LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC,
                        Some(Rc::clone(&position)),
                        crate::util::jc_diagnostic::Warning::from(
                            crate::resources::compiler_properties::warnings::
                                requires_transitive_automatic(),
                        ),
                    );
                } else if lint.is_enabled(LintCategory::REQUIRES_AUTOMATIC) {
                    log.borrow_mut().warning_lint_pos(
                        LintCategory::REQUIRES_AUTOMATIC,
                        Some(Rc::clone(&position)),
                        crate::util::jc_diagnostic::Warning::from(
                            crate::resources::compiler_properties::warnings::requires_automatic(),
                        ),
                    );
                }
            }));
        }
    }
}

/// Java: Kinds.kindName(Symbol sym).toString(locale) — the localized kind
/// name used by Warnings.DeclaredUsingPreview (Kinds.java:272-317).
fn kind_name_of(ctx: &CompilerContext, sym: SymbolId) -> String {
    match ctx.sym(sym).get_kind(ctx) {
        crate::code::symbol::ElementKind::PACKAGE => "package".to_string(),
        crate::code::symbol::ElementKind::ENUM => "enum".to_string(),
        crate::code::symbol::ElementKind::ANNOTATION_TYPE
        | crate::code::symbol::ElementKind::CLASS => "class".to_string(),
        crate::code::symbol::ElementKind::RECORD => "record".to_string(),
        crate::code::symbol::ElementKind::INTERFACE => "interface".to_string(),
        crate::code::symbol::ElementKind::TYPE_PARAMETER => "type variable".to_string(),
        crate::code::symbol::ElementKind::BINDING_VARIABLE
        | crate::code::symbol::ElementKind::ENUM_CONSTANT
        | crate::code::symbol::ElementKind::PARAMETER
        | crate::code::symbol::ElementKind::LOCAL_VARIABLE
        | crate::code::symbol::ElementKind::EXCEPTION_PARAMETER
        | crate::code::symbol::ElementKind::RESOURCE_VARIABLE => "variable".to_string(),
        crate::code::symbol::ElementKind::FIELD => "variable".to_string(),
        crate::code::symbol::ElementKind::METHOD => "method".to_string(),
        crate::code::symbol::ElementKind::CONSTRUCTOR => "constructor".to_string(),
        crate::code::symbol::ElementKind::STATIC_INIT => "static initializer".to_string(),
        crate::code::symbol::ElementKind::INSTANCE_INIT => "instance initializer".to_string(),
        crate::code::symbol::ElementKind::MODULE => "module".to_string(),
        crate::code::symbol::ElementKind::RECORD_COMPONENT => "record component".to_string(),
        _ => panic!("AssertionError: Unexpected kind"),
    }
}

/// The module of the package enclosing the symbol (Check.checkPreview).
fn package_module_of(ctx: &CompilerContext, sym: SymbolId) -> Option<SymbolId> {
    let packge = crate::code::symbol::symbol_packge(ctx, sym)?;
    match &ctx.sym(packge).kind_data {
        SymbolKind::PackageSymbol { modle, .. } => *modle,
        _ => None,
    }
}

/// Java: Symbol.outermostClass() (Symbol.java:523-531).
fn outermost_class(ctx: &CompilerContext, sym: SymbolId) -> Option<SymbolId> {
    let mut sym = sym;
    let mut prev: Option<SymbolId> = None;
    loop {
        if ctx.sym(sym).kind == Kind::PCK {
            break;
        }
        prev = Some(sym);
        match ctx.sym(sym).owner {
            Some(owner) => sym = owner,
            None => break,
        }
    }
    prev
}

impl Check {
    /// Java: public void checkModuleName(JCModuleDecl tree) (Check.java:2204-2236).
    ///
    /// # Arguments
    ///   - ctx: the compiler context (tree arena);
    ///   - tree: the module declaration.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the declaration must carry a module symbol (set by Enter).
    pub fn check_module_name(&self, ctx: &CompilerContext, tree: TreeId) {
        let qual_id = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ModuleDecl { qual_id, sym, .. } => {
                assert!(sym.is_some(), "AssertionError: module symbol");
                *qual_id
            }
            _ => panic!("ClassCastException: JCModuleDecl expected"),
        };
        if !self.lint.is_enabled(LintCategory::MODULE) {
            return;
        }
        let mut current = qual_id;
        while let Some(node) = current {
            let (component, pos, next) = match &ctx.tree(node).kind {
                crate::tree::jc_tree::TreeKind::FieldAccess {
                    selected, name, ..
                } => (name.clone(), ctx.tree(node).pos, *selected),
                crate::tree::jc_tree::TreeKind::Ident { name, .. } => {
                    (name.clone(), ctx.tree(node).pos, None)
                }
                _ => panic!("AssertionError: Unexpected qualified identifier"),
            };
            let component_string = component.to_java_string().to_string_lossy();
            let last = component_string.chars().last();
            let ends_with_digit = last.map(|c| c.is_numeric()).unwrap_or(false);
            if ends_with_digit {
                let warning = crate::util::jc_diagnostic::Warning::from(
                    crate::resources::compiler_properties::warnings::
                        poor_choice_for_module_name(component),
                );
                self.log.borrow_mut().warning_lint_pos(
                    LintCategory::MODULE,
                    Some(Rc::new(
                        crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
                    )),
                    warning,
                );
            }
            current = next;
        }
    }
}

#[cfg(test)]
mod diagnostics_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    /// Compile the given sources through the wired MiniCompiler with a deferred
    /// diagnostic handler installed and return the captured diagnostics.
    ///
    /// # Arguments
    ///   - sources: (file name, source text) pairs;
    ///   - configure: option installer used before Lint is created.
    ///
    /// # Returns
    ///   - the compiler and the captured diagnostics.
    ///
    /// # Panics
    ///   - a filesystem or compiler invariant is violated.
    fn compile_with_diagnostics(
        sources: &[(&str, &str)],
        configure: impl FnOnce(&crate::util::options::Options),
    ) -> (
        crate::main::mini_compiler::MiniCompiler,
        Vec<crate::util::jc_diagnostic::JCDiagnostic>,
    ) {
        use javac_tools::tools::standard_java_file_manager::StandardJavaFileManager;
        let root = std::env::temp_dir().join(format!(
            "rust-javac-check-diag-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let out = root.join("out");
        std::fs::create_dir_all(&out).expect("out");
        let src = root.join("src");
        std::fs::create_dir_all(&src).expect("src");
        let mut files = Vec::new();
        for (name, text) in sources {
            let file = src.join(name);
            std::fs::write(&file, text).expect("write source");
            files.push(file);
        }
        let mut compiler =
            crate::main::mini_compiler::MiniCompiler::new_with_options(&out, configure);
        let sp = javac_tools::tools::standard_location::StandardLocation::location_for(
            &javac_strings::JavaString::from("SOURCE_PATH"),
        );
        compiler
            .file_manager
            .set_location(&*sp, Some(&[src]))
            .expect("source path");
        compiler
            .log
            .borrow_mut()
            .install_diagnostic_handler(crate::util::log::DiagnosticHandler::Deferred {
                prev: None,
                deferred: std::collections::VecDeque::new(),
                filter: None,
            });
        let _ = compiler.compile_files(&files);
        let diagnostics = compiler.log.borrow_mut().take_deferred_diagnostics();
        compiler.log.borrow_mut().pop_diagnostic_handler();
        (compiler, diagnostics)
    }

    #[test]
    fn safe_varargs_on_record_accessor_is_error() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[("R.java", "record R(@SafeVarargs int x) {}\n")],
            |_options| {},
        );
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes
                .iter()
                .any(|code| code == "compiler.err.varargs.invalid.trustme.anno"),
            "varargs.invalid.trustme.anno missing: {codes:?}"
        );
    }

    #[test]
    fn deprecated_field_annotation_compiles() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[("R.java", "class R { @Deprecated int x; }\n")],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn instanceof_pattern_no_subtype_is_error() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[("T.java", "class T { boolean test(Object o) { return o instanceof Object obj; } }\n")],
            |_options| {},
        );
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes.iter().any(|code| code == "compiler.err.instanceof.pattern.no.subtype"),
            "instanceof.pattern.no.subtype missing: {codes:?}"
        );
    }

    #[test]
    fn instanceof_reifiable_not_safe_is_error() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "import java.util.List;\nclass T { boolean test(Object o) { return o instanceof List<String> l; } }\n",
            )],
            |_options| {},
        );
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes.iter().any(|code| code == "compiler.err.instanceof.reifiable.not.safe"),
            "instanceof.reifiable.not.safe missing: {codes:?}"
        );
    }

    #[test]
    fn instanceof_pattern_legal_compiles() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { boolean test(Object o) { return o instanceof String s && s.length() > 0; } }\n",
            )],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn lambda_expression_compiles_with_indy() {
        let (compiler, diagnostics) = compile_with_diagnostics(
            &[("T.java", "class T { Runnable r = () -> {}; }\n")],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let class = compiler
            .last_compiled_classes
            .iter()
            .find(|class| class.name.contains("T"))
            .unwrap_or_else(|| panic!("generated class T; names={:?}", compiler.last_compiled_classes.iter().map(|class| class.name.clone()).collect::<Vec<_>>()));
        let bytes = &class.bytes;
        let text = String::from_utf8_lossy(bytes);
        assert!(text.contains("LambdaMetafactory"), "indy bootstrap missing");
    }

    #[test]
    fn method_reference_compiles_with_indy() {
        let (compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { Runnable r = T::m; static void m() {} }\n",
            )],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let class = compiler
            .last_compiled_classes
            .iter()
            .find(|class| class.name.contains("T"))
            .expect("generated class T");
        assert!(!class.bytes.is_empty());
    }

    #[test]
    fn lambda_capture_compiles() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f(int x) { Runnable r = () -> System.out.println(x); r.run(); } }\n",
            )],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    /// The nested diagnostic/fragment arguments of a diagnostic (ADR 0002:
    /// the original argument objects are preserved in the argument vector).
    fn nested_fragment_codes(
        diagnostic: &crate::util::jc_diagnostic::JCDiagnostic,
    ) -> Vec<String> {
        diagnostic
            .get_args()
            .iter()
            .filter_map(|arg| {
                arg.clone()
                    .downcast::<crate::util::jc_diagnostic::JCDiagnostic>()
                    .ok()
            })
            .map(|nested| nested.get_code_string())
            .collect()
    }

    #[test]
    fn guard_effectively_final_uses_guard_fragment() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f(Object o) { int x = 0; x = 1; switch (o) { case String s && x > 0 -> {} default -> {} } } }\n",
            )],
            |options| {
                options.put("preview", "true");
                // The deferred-handler compilation has no error count, so the
                // no-error stop policy applies; it stops after Flow (before
                // TransPatterns/Gen) for this erroneous tree.
                options.put("should-stop.ifNoError", "FLOW");
            },
        );
        let effectively_final = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.get_code_string() == "compiler.err.cant.ref.non.effectively.final.var"
            })
            .expect("cant.ref.non.effectively.final.var missing");
        let fragments = nested_fragment_codes(effectively_final);
        assert!(
            fragments.iter().any(|code| code == "compiler.misc.guard"),
            "guard fragment missing: {fragments:?}"
        );
    }

    #[test]
    fn verbose_resolution_reports_failure_note_and_candidates() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f(T t) {} void f(Runnable r) {} void g() { f(new Object()); } }\n",
            )],
            |options| {
                options.put("debug.verboseResolution", "all");
                options.put("should-stop.ifNoError", "FLOW");
            },
        );
        let note = diagnostics
            .iter()
            .find(|d| d.get_code_string() == "compiler.note.verbose.resolve.multi.1")
            .expect("verbose.resolve.multi.1 missing");
        let message = note.get_message(None).to_string_lossy();
        // Matches the JDK 17 diagnostic text for this source.
        assert_eq!(
            message.lines().next().unwrap_or_default(),
            "erroneous resolution for method f in type T"
        );
        assert!(message.contains("with actuals: java.lang.Object"));
        assert!(!note.get_subdiagnostics().is_empty());
    }

    fn collect_diagnostic_codes(
        diagnostic: &crate::util::jc_diagnostic::JCDiagnostic,
        codes: &mut Vec<String>,
    ) {
        codes.push(diagnostic.get_code_string());
        for arg in diagnostic.get_args() {
            if let Ok(nested) = arg.clone().downcast::<crate::util::jc_diagnostic::JCDiagnostic>() {
                collect_diagnostic_codes(&nested, codes);
            }
        }
        let mut subs = diagnostic.get_subdiagnostics();
        while subs.non_empty() {
            let sub = subs.head().clone();
            subs = subs.tail();
            collect_diagnostic_codes(&sub, codes);
        }
    }

    #[test]
    fn verbose_resolution_reports_applicable_and_partial_inst_sig() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { <G> void f(G g) {} void g() { f(\"s\"); } }\n",
            )],
            |options| {
                options.put("debug.verboseResolution", "all");
            },
        );
        let mut codes: Vec<String> = Vec::new();
        for diagnostic in &diagnostics {
            collect_diagnostic_codes(diagnostic, &mut codes);
        }
        assert!(codes.iter().any(|c| c == "compiler.misc.applicable.method.found.1"));
        assert!(codes.iter().any(|c| c.ends_with("partial.inst.sig")));
    }

    #[test]
    fn method_reference_dump_reports_bound_results() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f() {} void g() { Runnable r = this::f; r.run(); } }\n",
            )],
            |options| {
                options.put("debug.dumpMethodReferenceSearchResults", "true");
            },
        );
        let note = diagnostics
            .iter()
            .find(|d| d.get_code_string() == "compiler.note.method.ref.search.results.multi")
            .expect("method.ref.search.results.multi missing");
        assert!(!note.get_subdiagnostics().is_empty());
    }

    #[test]
    fn fatal_error_diagnostics_format_the_fragment() {
        let (compiler, _diagnostics) = compile_with_diagnostics(
            &[("T.java", "class T {}\n")],
            |_options| {},
        );
        let name = compiler
            .names
            .from_string(&javac_strings::JavaString::from("f"));
        let fragment = compiler.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::fatal_err_cant_locate_meth(name),
        ));
        let error = crate::util::fatal_error::FatalError::from_diagnostic(&fragment);
        assert_eq!(error.message, "Fatal Error: Unable to find method f");
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::util::fatal_error::fatal_error_diagnostic(&fragment);
        }));
        assert!(payload.is_err());
    }

    #[test]
    fn inapplicable_symbols_error_lists_candidate_details() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f(T t) {} void f(Runnable r) {} void g() { f(new Object()); } }\n",
            )],
            |options| {
                options.put("should-stop.ifNoError", "FLOW");
            },
        );
        let error = diagnostics
            .iter()
            .find(|d| d.get_code_string() == "compiler.err.cant.apply.symbols")
            .expect("cant.apply.symbols missing");
        let message = error.get_message(None).to_string_lossy();
        // Matches the JDK 17 diagnostic text for this source.
        assert_eq!(
            message.lines().next().unwrap_or_default(),
            "no suitable method found for f(java.lang.Object)"
        );
        assert!(message.contains("is not applicable"));
        // Candidate details come from the non-verbose inapplicable-method
        // path; the JDK 17 output for this source names the signatures.
        assert!(message.contains("method T.f(T) is not applicable"));
        assert!(message.contains("method T.f(java.lang.Runnable) is not applicable"));
        let mut sub_count = 0;
        let mut subs = error.get_subdiagnostics();
        while subs.non_empty() {
            assert_eq!(subs.head().get_code_string(), "compiler.misc.inapplicable.method");
            sub_count += 1;
            subs = subs.tail();
        }
        assert!(sub_count >= 2, "expected at least two inapplicable candidates");
    }

    fn collect_diagnostic_texts(
        diagnostic: &crate::util::jc_diagnostic::JCDiagnostic,
        texts: &mut Vec<String>,
    ) {
        texts.push(diagnostic.get_message(None).to_string_lossy());
        for arg in diagnostic.get_args() {
            if let Ok(nested) = arg.clone().downcast::<crate::util::jc_diagnostic::JCDiagnostic>() {
                collect_diagnostic_texts(&nested, texts);
            }
        }
        let mut subs = diagnostic.get_subdiagnostics();
        while subs.non_empty() {
            let sub = subs.head().clone();
            subs = subs.tail();
            collect_diagnostic_texts(&sub, texts);
        }
    }

    #[test]
    fn diagnostic_type_display_covers_all_type_forms() {
        let source = r#"class T<G> {
            static class In {}
            class NonStatic {}
            void f(int x) {}
            void f(java.util.List<String> x) {}
            void f(Object a, String b) {}
            void f(java.util.Map<String,String> x) {}
            void f(G x) {}
            void f(In x) {}
            void f(NonStatic x) {}
            void f(String... x) {}
            void arr(String[] x) {}
            void wild(java.util.List<? extends Number> x) {}
            void g(
                java.util.List<String> l,
                java.util.Map<String,String> m,
                java.util.List<? extends Number> w) {
                f(1);
                f(l);
                f("a", "b");
                f(m);
                wild(w);
                arr(new String[0]);
                f(new Object());
            }
        }"#;
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[("T.java", source)],
            |options| {
                options.put("debug.verboseResolution", "all");
                options.put("should-stop.ifNoError", "FLOW");
            },
        );
        let mut texts: Vec<String> = Vec::new();
        for diagnostic in &diagnostics {
            collect_diagnostic_texts(diagnostic, &mut texts);
        }
        let joined = texts.join("\n");
        for expected in [
            "with actuals: int",
            "with actuals: java.util.List<java.lang.String>",
            "with actuals: java.lang.String,java.lang.String",
            "with actuals: java.util.Map<java.lang.String,java.lang.String>",
            "with actuals: java.lang.String[]",
            "f(java.lang.String...)",
            "f(T<G>.NonStatic)",
            "f(T.In)",
            "f(G)",
            "f(java.util.Map<java.lang.String,java.lang.String>)",
            "f(java.lang.Object,java.lang.String)",
            "f(java.util.List<java.lang.String>)",
            "f(int)",
            "wild(java.util.List<? extends java.lang.Number>)",
        ] {
            assert!(joined.contains(expected), "missing {expected:?} in diagnostics:\n{joined}");
        }
        let error = diagnostics
            .iter()
            .find(|d| d.get_code_string() == "compiler.err.cant.apply.symbols")
            .expect("cant.apply.symbols missing");
        let message = error.get_message(None).to_string_lossy();
        assert_eq!(
            message.lines().next().unwrap_or_default(),
            "no suitable method found for f(java.lang.Object)"
        );
        for expected in [
            "method T.f(java.lang.String...) is not applicable",
            "method T.f(T<G>.NonStatic) is not applicable",
            "method T.f(T.In) is not applicable",
            "method T.f(G) is not applicable",
            "method T.f(java.util.Map<java.lang.String,java.lang.String>) is not applicable",
            "method T.f(java.lang.Object,java.lang.String) is not applicable",
            "method T.f(java.util.List<java.lang.String>) is not applicable",
            "method T.f(int) is not applicable",
        ] {
            assert!(message.contains(expected), "missing {expected:?} in error:\n{message}");
        }
    }
    #[test]
    fn verbose_resolution_reports_success_note() {
        let (compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f() {} void f(int x) {} void g() { f(); } }\n",
            )],
            |options| {
                options.put("debug.verboseResolution", "all");
            },
        );
        let note = diagnostics
            .iter()
            .find(|d| {
                d.get_code_string() == "compiler.note.verbose.resolve.multi"
                    && d.get_message(None)
                        .to_string_lossy()
                        .starts_with("resolving method f in type T")
            })
            .expect("verbose.resolve.multi for f missing");
        let message = note.get_message(None).to_string_lossy();
        // Matches the JDK 17 diagnostic text for this source.
        assert_eq!(
            message.lines().next().unwrap_or_default(),
            "resolving method f in type T to candidate 1"
        );
        assert!(message.contains("with actuals: no arguments"));
        assert_eq!(compiler.last_compiled_classes.len(), 1);
    }

    #[test]
    fn should_stop_if_no_error_at_attr_skips_attribution_and_generation() {
        let (compiler, diagnostics) = compile_with_diagnostics(
            &[("T.java", "class T {}\n")],
            |options| {
                options.put("should-stop.ifNoError", "ATTR");
            },
        );
        assert!(compiler.last_compiled_classes.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn should_stop_if_no_error_at_flow_skips_desugar_and_generation() {
        let (compiler, diagnostics) = compile_with_diagnostics(
            &[("T.java", "class T {}\n")],
            |options| {
                options.put("should-stop.ifNoError", "FLOW");
            },
        );
        assert!(compiler.last_compiled_classes.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn should_stop_default_policy_generates_the_class() {
        let (compiler, diagnostics) = compile_with_diagnostics(
            &[("T.java", "class T {}\n")],
            |_options| {},
        );
        assert_eq!(compiler.last_compiled_classes.len(), 1);
        assert!(diagnostics.is_empty());
    }
    #[test]
    fn lambda_effectively_final_uses_lambda_fragment() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f() { int x = 0; x = 1; Runnable r = () -> System.out.println(x); } }\n",
            )],
            |_options| {},
        );
        let effectively_final = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.get_code_string() == "compiler.err.cant.ref.non.effectively.final.var"
            })
            .expect("cant.ref.non.effectively.final.var missing");
        let fragments = nested_fragment_codes(effectively_final);
        assert!(
            fragments.iter().any(|code| code == "compiler.misc.lambda"),
            "lambda fragment missing: {fragments:?}"
        );
    }

    #[test]
    fn try_with_resources_compiles() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[
                (
                    "Res.java",
                    "class Res implements java.io.Closeable { public void close() {} }\n",
                ),
                (
                    "T.java",
                    "class T { void f() { try (Res r = new Res()) { } } }\n",
                ),
            ],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn try_with_multiple_resources_compiles() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[
                (
                    "Res.java",
                    "class Res implements java.io.Closeable { public void close() {} }\n",
                ),
                (
                    "T.java",
                    "class T { void f() { try (Res first = new Res(); Res second = new Res()) { } } }\n",
                ),
            ],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn try_resource_assignment_reports_error() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[
                (
                    "Res.java",
                    "class Res implements java.io.Closeable { public void close() {} }\n",
                ),
                (
                    "T.java",
                    "class T { void f() { try (Res r = new Res()) { r = new Res(); } } }\n",
                ),
            ],
            |_options| {},
        );
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes
                .iter()
                .any(|code| code == "compiler.err.try.resource.may.not.be.assigned"),
            "try.resource.may.not.be.assigned missing: {codes:?}"
        );
    }

    #[test]
    fn supertype_completion_registers_class_in_real_symtab() {
        // The supertype walk completes java.lang.Exception lazily (its
        // supertype_field is filled by the class reader on completion); the
        // completion must be written to the real Symtab tables, so lookups see
        // the completed class (no cloned symbol table).
        let (compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f() throws java.io.IOException { throw new java.io.IOException(); } }\n",
            )],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let syms = compiler.syms.borrow();
        let io_flat = compiler
            .names
            .from_string(&javac_strings::JavaString::from("java.io.IOException"));
        assert!(
            syms.get_class(syms.java_base, &io_flat).is_some(),
            "java.io.IOException missing from the real Symtab class table"
        );
        let exception_flat = compiler
            .names
            .from_string(&javac_strings::JavaString::from("java.lang.Exception"));
        let exception = syms
            .get_class(syms.java_base, &exception_flat)
            .expect("java.lang.Exception entered in the real Symtab class table");
        assert!(
            compiler.ctx.is_symbol_completed(exception),
            "java.lang.Exception was not completed by the supertype walk"
        );
    }

    #[test]
    fn multicatch_disjoint_types_compile() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f() throws java.io.IOException { try { throw new java.io.IOException(); } catch (java.io.IOException | RuntimeException e) { } } }\n",
            )],
            |_options| {},
        );
        let errors: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .filter(|code| code.starts_with("compiler.err."))
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn multicatch_related_types_report_disjoint_error() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[(
                "T.java",
                "class T { void f() throws java.io.IOException { try { throw new java.io.IOException(); } catch (java.io.IOException | java.io.FileNotFoundException e) { } } }\n",
            )],
            |_options| {},
        );
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes
                .iter()
                .any(|code| code == "compiler.err.multicatch.types.must.be.disjoint"),
            "multicatch.types.must.be.disjoint missing: {codes:?}"
        );
    }

    #[test]
    fn strictfp_warning_is_reported() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[("C.java", "strictfp class C { strictfp void m() {} }
")],
            |_options| {},
        );
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes
                .iter()
                .filter(|code| code.as_str() == "compiler.warn.strictfp")
                .count()
                >= 2,
            "strictfp warnings missing: {codes:?}"
        );
    }

    #[test]
    fn sun_proprietary_and_profile_diagnostics_are_reported() {
        use crate::code::source::Source;
        use crate::jvm::target::Target;
        use crate::util::context::Context;
        use crate::util::javac_messages::JavacMessages;

        let mut ctx = CompilerContext::new();
        let mut jcontext = Context::new();
        let names = Rc::new(crate::util::names::Names::new(false));
        let log = crate::util::log::Log::instance(&mut jcontext);
        let syms = Rc::new(RefCell::new(crate::code::symtab::Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::DEFAULT,
            Source::JDK17,
        )));
        let types = Rc::new(crate::code::types::Types::with_names(Rc::clone(&names)));
        let messages = JavacMessages::instance(&mut jcontext);
        let diags = Rc::new(crate::util::jc_diagnostic::Factory::new(messages, "compiler"));
        let lint = crate::code::lint::Lint::instance(&mut jcontext);
        let deferred = Rc::new(RefCell::new(DeferredLintHandler::new()));
        let check = Rc::new(RefCell::new(Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            diags,
            Source::JDK17,
            '$',
            Rc::clone(&types),
            lint,
            deferred,
            Rc::clone(&syms),
        )));
        attach_self(&check);
        check
            .borrow()
            .set_profile(Rc::new(crate::jvm::profile::Profile::COMPACT1));
        let object_type = syms.borrow().object_type;
        let symbol = ctx.typ(object_type).tsym.expect("object symbol");
        ctx.sym_mut(symbol).flags_field |= crate::code::flags::PROPRIETARY
            | crate::code::flags::NOT_IN_PROFILE;
        log.borrow_mut().install_diagnostic_handler(
            crate::util::log::DiagnosticHandler::Deferred {
                prev: None,
                deferred: std::collections::VecDeque::new(),
                filter: None,
            },
        );
        check.borrow().check_sun_api(&ctx, -1, symbol);
        check.borrow().check_profile(&ctx, -1, symbol);
        let diagnostics = log.borrow_mut().take_deferred_diagnostics();
        log.borrow_mut().pop_diagnostic_handler();
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes
                .iter()
                .any(|code| code == "compiler.warn.sun.proprietary"),
            "sun.proprietary warning missing: {codes:?}"
        );
        assert!(
            codes.iter().any(|code| code == "compiler.err.not.in.profile"),
            "not.in.profile error missing: {codes:?}"
        );
    }

    #[test]
    fn unsafe_use_of_varargs_param_is_reported() {
        let source = "class UnsafeUseOfVarargsParam {\n    static void testArr(Object[] o) {}\n    @SafeVarargs static <X> void m(X... x) {\n        testArr(x);\n    }\n}\n";
        let (_compiler, diagnostics) =
            compile_with_diagnostics(&[("UnsafeUseOfVarargsParam.java", source)], |options| {
                options.put("-Xlint:varargs", "true")
            });
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes
                .iter()
                .any(|code| code == "compiler.warn.varargs.unsafe.use.varargs.param"),
            "varargs.unsafe.use.varargs.param missing: {codes:?}"
        );
    }

    #[test]
    fn auxiliary_class_warning_is_reported() {
        let (_compiler, diagnostics) = compile_with_diagnostics(
            &[
                ("A.java", "class Aux {}
"),
                ("B.java", "class B { Aux aux; }
"),
            ],
            |options| options.put("-Xlint:auxiliaryclass", "true"),
        );
        let codes: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.get_code_string().to_string())
            .collect();
        assert!(
            codes.iter().any(|code| code
                == "compiler.warn.auxiliary.class.accessed.from.outside.of.its.source.file"),
            "auxiliary class warning missing: {codes:?}"
        );
    }
}
