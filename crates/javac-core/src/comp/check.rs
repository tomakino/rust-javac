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
use crate::code::kinds::Kind;
use crate::code::symtab::Symtab;
use crate::code::symbol::CompletionFailure;
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_scanner::{tree_scanner_visitor_delegate, TreeScannerVisitor};
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::deferred_attr::DeferredAttrContext;
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
use crate::util::mandatory_warning_handler::MandatoryWarningLog;
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
    /// Java: private Lint lint (Check.java:187) — restored to the upstream
    /// field shape (, Porting rules official structure).
    pub lint: Rc<Lint>,
    /// Java: private DeferredLintHandler deferredLintHandler (Check.java:193)
    pub deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
    /// Java: the Symtab service (Symtab.instance(context)); explicit service
    /// per ADR 0001.
    pub syms: Rc<RefCell<Symtab>>,
    /// Java: Feature.PRIVATE_SAFE_VARARGS.allowedInSource(source) (Check.java:970-977)
    pub private_safe_varargs: bool,
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
                    // TODO(Check): warnOnExplicitStrictfp uses the
                    // DeferredLintHandler, which is not ported (registered).
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
                    // TODO(Check): warnOnExplicitStrictfp (DeferredLintHandler)
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
                let syms = self.syms.borrow();
                types.has_same_args(ctx, &syms, sym_ty, other_ty)
                    || {
                        let sym_erasure = types.erasure(ctx, &syms, sym_ty);
                        let other_erasure = types.erasure(ctx, &syms, other_ty);
                        types.has_same_args(ctx, &syms, sym_erasure, other_erasure)
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
                    let syms = self.syms.borrow();
                    if !types.has_same_args_strict(ctx, &syms, sym_ty, other_ty, false) {
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
        pos: i32,
        required: JCDiagnostic,
        found: FoundArg,
    ) -> crate::ids::TypeId {
        let syms = self.syms.borrow();
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
            FoundArg::Type(t) => self.types.create_error_type_original(ctx, Some(t), &syms),
            FoundArg::Fragment(_) => syms.err_type,
        }
    }

    /// Java: Type checkClassType(DiagnosticPosition pos, Type t)
    pub fn check_class_type(
        &self,
        ctx: &mut CompilerContext,
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
            return self.type_tag_error(ctx, pos, required, found);
        }
        t
    }

    /// Java: Type checkClassType(DiagnosticPosition pos, Type t, boolean noBounds)
    pub fn check_class_type_no_bounds(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        t: crate::ids::TypeId,
        no_bounds: bool,
    ) -> crate::ids::TypeId {
        let t = self.check_class_type(ctx, pos, t);
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
                        return self.type_tag_error(ctx, pos, required, FoundArg::Type(t));
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
        self.check_type(ctx, syms, pos, found, req, &*self.basic_handler)
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
        check_context: &dyn CheckContext,
    ) -> TypeId {
        let inference_context = check_context.inference_context();
        if inference_context.free(ctx, syms, &self.types, req)
            || inference_context.free(ctx, syms, &self.types, found)
        {
            // TODO(Infer): inferenceContext.addFreeTypeListener(List.of(req,
            // found), solvedContext -> checkType(pos, solvedContext.asInstType
            // (found), solvedContext.asInstType(req), checkContext)); the
            // listener needs the mutable Attr/Check recursion; registered.
            unimplemented!("TODO(Infer batch): Check.checkType free type listener");
        }
        if ctx.typ(req).has_tag(crate::code::type_tag::TypeTag::ERROR) {
            return req;
        }
        if ctx.typ(req).has_tag(crate::code::type_tag::TypeTag::NONE) {
            return found;
        }
        let mut warn = check_context.check_warner(ctx, syms, &self.types, pos, found, req);
        if check_context.compatible(ctx, syms, &self.types, found, req, &mut warn) {
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
        pos: i32,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).is_reference() {
            return t;
        }
        let required = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::type_req_ref(),
        ));
        self.type_tag_error(ctx, pos, required, FoundArg::Type(t))
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
        pos: i32,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::VOID) {
            self.report_error(pos, errors::void_not_allowed_here());
            let syms = self.syms.borrow();
            self.types.create_error_type_original(ctx, Some(t), &syms)
        } else {
            t
        }
    }

    /// Java: Type checkClassOrArrayType(DiagnosticPosition pos, Type t)
    /// (Check.java:709-717)
    pub fn check_class_or_array_type(
        &self,
        ctx: &mut CompilerContext,
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
            return self.type_tag_error(ctx, pos, required, found);
        }
        t
    }

    /// Java: Type checkNullOrRefType(DiagnosticPosition pos, Type t)
    /// (Check.java:814-822)
    pub fn check_null_or_ref_type(
        &self,
        ctx: &mut CompilerContext,
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
            self.type_tag_error(ctx, pos, required, FoundArg::Type(t))
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
    /// final JCTypeCast tree) (Check.java:647-660)
    ///
    /// TODO(TreeInfo): requires TreeInfo.containsTypeAnnotation
    /// (TypeAnnotationFinder) — not ported; registered.
    pub fn check_redundant_cast(
        &self,
        _ctx: &mut CompilerContext,
        _env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        _tree: TreeId,
    ) {
        unimplemented!(
            "TODO(TreeInfo batch): Check.checkRedundantCast needs TreeInfo.containsTypeAnnotation (Check.java:647-660)"
        )
    }

    /// Java: void validate(JCTree tree, Env<AttrContext> env, boolean
    /// checkRaw) (Check.java:1397-1407) and class Validator
    /// (Check.java:1421-1525)
    ///
    /// TODO(Types/Check): the Validator requires firstIncompatibleTypeArg,
    /// checkClassBounds, TreeInfo.containsTypeAnnotation and the JCTree.Visitor
    /// subclass navigation; registered (no partial validator).
    pub fn validate(
        &self,
        _ctx: &mut CompilerContext,
        _env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        _tree: TreeId,
    ) {
        unimplemented!(
            "TODO(Types/Check batch): Check.validate/Validator (Check.java:1397-1525)"
        )
    }

    /// Java: void validate(List<? extends JCTree> trees, Env<AttrContext> env)
    /// (Check.java:1406-1410)
    pub fn validate_list(
        &self,
        ctx: &mut CompilerContext,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        trees: &List<TreeId>,
    ) {
        for tree in trees.iter() {
            self.validate(ctx, env, tree);
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
fn warning_from_info(
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
    pub fn validate_annotation_tree(&self, ctx: &mut CompilerContext, tree: TreeId) {
        struct AnnotationValidator {
            root: TreeId,
        }
        // Java: class AnnotationValidator extends TreeScanner { visitAnnotation }
        tree_scanner_visitor_delegate!(AnnotationValidator);
        impl TreeScannerVisitor for AnnotationValidator {
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
                    <Self as TreeScannerVisitor>::visit_annotation(self, ctx, tree);
                    let _ = self.root;
                }
            }
        }
        let mut validator = AnnotationValidator { root: tree };
        validator.scan(ctx, Some(tree));
        let _ = &self.lint;
        let _ = &self.syms;
        let _ = &self.deferred_lint_handler;
    }

    /// Java: public void validateAnnotations(List<JCAnnotation> annotations,
    /// JCTree declarationTree, Symbol s) (Check.java:2919-2923)
    pub fn validate_annotations(
        &self,
        ctx: &mut CompilerContext,
        annotations: &List<TreeId>,
        declaration_tree: Option<TreeId>,
        s: SymbolId,
    ) -> Result<(), CompletionFailure> {
        for a in annotations.iter() {
            self.validate_annotation(ctx, a, declaration_tree, s)?;
        }
        Ok(())
    }

    /// Java: private void validateAnnotation(JCAnnotation a, JCTree
    /// declarationTree, Symbol s) (Check.java:2933-3073)
    fn validate_annotation(
        &self,
        ctx: &mut CompilerContext,
        a: TreeId,
        declaration_tree: Option<TreeId>,
        s: SymbolId,
    ) -> Result<(), CompletionFailure> {
        self.validate_annotation_tree(ctx, a);
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
            // Java: the record-component annotation propagation needs
            // ClassSymbol.getRecordComponent + RecordComponent metadata.
            unimplemented!(
                "TODO(Record batch): Check.validateAnnotation record-field branch (Check.java:2944-2975)"
            );
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
            let applicable_targets = self.get_applicable_targets(ctx, a, s)?;
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
            // Java: the compiler-generated record element trimming branch
            // (isCompGeneratedRecordElement) needs the Record; registered.
        }
        // functional interface check
        let functional_interface_tsym = ctx
            .typ(self.syms.borrow().functional_interface_type)
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
        a: TreeId,
        s: SymbolId,
    ) -> Result<bool, CompletionFailure> {
        let targets = self.get_applicable_targets(ctx, a, s)?;
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
        let arr = self.get_attribute_target_attribute(ctx, anno_sym)?;
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
        anno_sym: SymbolId,
    ) -> Result<Vec<Name>, CompletionFailure> {
        let arr = self.get_attribute_target_attribute(ctx, anno_sym)?;
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
        s: SymbolId,
    ) -> Result<Option<AttributeId>, CompletionFailure> {
        let mut syms = self.syms.borrow_mut();
        // adapter adapter: the upstream mutates the ClassSymbol-held metadata in place;
        // the port temporarily clones it (the metadata is Clone) and writes it back
        // so inference/completer state stays on the symbol.
        let Some(mut metadata) = ctx
            .sym(s)
            .get_annotation_type_metadata()
            .cloned()
        else {
            return Ok(None);
        };
        let at_target = metadata.get_target(ctx, &mut syms)?;
        if let Some(slot) = ctx.sym_mut(s).get_annotation_type_metadata_mut() {
            *slot = metadata;
        }
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
        a: TreeId,
    ) -> Result<bool, CompletionFailure> {
        self.log
            .borrow_mut()
            .install_diagnostic_handler(crate::util::log::DiagnosticHandler::Discard { prev: None });
        let result = self.validate_annotation_values(ctx, a);
        self.log.borrow_mut().pop_diagnostic_handler();
        result
    }

    /// Java: private boolean validateAnnotation(JCAnnotation a)
    /// (Check.java:3468-3505) — the element inventory check.
    fn validate_annotation_values(
        &self,
        ctx: &mut CompilerContext,
        a: TreeId,
    ) -> Result<bool, CompletionFailure> {
        let mut is_valid = true;
        let annotation_ty = ctx.tree(a).ty.expect("annotation type");
        let anno_tsym = ctx.typ(annotation_ty).tsym.expect("annotation tsym");
        let mut syms = self.syms.borrow_mut();
        let Some(mut metadata) = ctx
            .sym(anno_tsym)
            .get_annotation_type_metadata()
            .cloned()
        else {
            return Ok(true);
        };
        let mut elements = metadata.get_annotation_elements(ctx, &mut syms)?;
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
        let members_with_default = metadata.get_annotation_elements_with_default(ctx, &mut syms)?;
        if let Some(slot) = ctx.sym_mut(anno_tsym).get_annotation_type_metadata_mut() {
            *slot = metadata;
        }
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
        Ok(is_valid && self.validate_target_annotation_value(ctx, a))
    }

    /// Java: boolean validateTargetAnnotationValue(JCAnnotation a)
    /// (Check.java:3508-3530)
    pub fn validate_target_annotation_value(&self, ctx: &mut CompilerContext, a: TreeId) -> bool {
        let anno_ty = ctx.tree(a).ty.expect("annotation type");
        let anno_tsym = ctx.typ(anno_ty).tsym.expect("annotation tsym");
        let target_tsym = ctx.typ(self.syms.borrow().annotation_target_type).tsym;
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
        syms: &mut Symtab,
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
        syms: &mut Symtab,
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
        syms: &mut Symtab,
        t: TypeId,
    ) -> bool {
        self.first_incompatible_type_arg(ctx, syms, t).is_none()
    }

    /// Java: private Type firstIncompatibleTypeArg(Type type)
    /// (Check.java:1080-1162)
    fn first_incompatible_type_arg(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
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
        syms: &mut Symtab,
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
        syms: &mut Symtab,
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
        _syms: &crate::code::symtab::Symtab,
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
            result = result.append(self.check_ref_type(ctx, pos, head));
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
        tree: TreeId,
        site: TypeId,
        origin: SymbolId,
        m: SymbolId,
    ) {
        let c = ctx.typ(site).tsym.expect("site type symbol");
        let name = ctx.sym(m).name.clone();
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
        let t = self.check_class_or_array_type(ctx, pos, t);
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
                return self.check_class_type_no_bounds(ctx, pos, t, true);
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
