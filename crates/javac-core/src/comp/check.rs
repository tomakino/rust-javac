// Origin: com/sun/tools/javac/comp/Check.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES §19 registration: this file ports the subset of Check that is
// required by the Enter batch (class enter symbol/flags/duplicate checks).
// NOT PORTED IN THIS STEP (registered; dependencies: Resolve/DeferredAttr/Infer/
// TypeAnnotations/Attr/Preview/MandatoryWarningHandler/DeferredLintHandler and
// the whole type-checking body of Check):
// - the Check constructor dependencies (Resolve/DeferredAttr/Infer/
//   TypeAnnotations/Preview/lint handlers) and the constructors themselves;
// - every other Check method (type checking, annotations checks, lambda/method
//   checks, warnings, sunApiHandler, ...).
// The ported methods keep the upstream bodies and error keys; where a dependency
// is not ported the branch is marked TODO and not executed (no substitute).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::code::symbol::{
    symbol_encl_class, symbol_is_directly_or_indirectly_local, symbol_is_static_or_instance_init,
    symbol_location, symbol_packge, SymbolKind,
};
use crate::code::symtab::Symtab;
use crate::code::types::Types;
use crate::comp::deferred_attr::DeferredAttrContext;
use crate::comp::infer::InferenceContext;
use crate::ids::TypeId;
use crate::util::jc_diagnostic::JCDiagnostic;
use crate::util::warner::Warner;
use crate::compiler_context::CompilerContext;
use crate::ids::{ScopeId, SymbolId, TreeId};
use crate::resources::compiler_properties::errors;
use crate::resources::diagnostic_contract::{DiagnosticArg, FlagArg, KindNameArg, SymbolArg};
use crate::resources::diagnostic_contract::ErrorInfo;
use crate::util::jc_diagnostic::{Error as JCDiagnosticError, Factory};
use crate::util::abstract_log::AbstractLog;
use crate::util::log::Log;
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
}

impl Check {
    /// Java: protected Check(Context context) — the ported dependency subset.
    pub fn new(
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        diags: Rc<Factory>,
        source: crate::code::source::Source,
        synthetic_name_char: char,
        types: Rc<Types>,
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
        }
    }


    // ------------------------------------------------------------------
    // Exception checking set utilities (Check.java:1578-1685). Ported here
    // because the Flow batch (FlowAnalyzer) depends on them (registered §19).
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
                    // TODO(Check batch): warnOnExplicitStrictfp uses the
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
                    // TODO(Check batch): warnOnExplicitStrictfp (DeferredLintHandler)
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
            let same_kind_check = if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH {
                // TODO(Check/Types batch): types.hasSameArgs / erasure checks
                // are not ported (Types batch); registered.
                unimplemented!("TODO(Check/Types batch): checkUnique MTH hasSameArgs")
            } else {
                true
            };
            if same_kind_check {
                // The remaining branches are all MTH-specific upstream.
                if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH {
                    unimplemented!("TODO(Check/Types batch): checkUnique MTH varargs/erasure branches")
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
    /// TODO(Check/Types + printing batch): the upstream body logs
    /// Errors.TypeFoundReq(found, required) — the Object found is a Type or a
    /// JCDiagnostic fragment and the diagnostic argument mapping needs
    /// Type.toString (Types/Symbol printing batch) — and returns
    /// types.createErrorType(found instanceof Type ? type : syms.errType).
    /// Registered; no substitute diagnostic text is produced.
    fn type_tag_error(
        &self,
        _ctx: &mut CompilerContext,
        _pos: i32,
        _required: JCDiagnostic,
        _found: DiagnosticArg,
    ) -> crate::ids::TypeId {
        unimplemented!("TODO(Check/Types + printing batch): Check.typeTagError")
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
            let found = if tag == crate::code::type_tag::TypeTag::TYPEVAR {
                DiagnosticArg::from_display("type parameter")
            } else {
                DiagnosticArg::from_display("type")
            };
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
                        return self.type_tag_error(
                            ctx,
                            pos,
                            required,
                            DiagnosticArg::from_display("type"),
                        );
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
            // TODO(Infer batch): inferenceContext.addFreeTypeListener(List.of(req,
            //     found), solvedContext -> checkType(pos, solvedContext.asInstType
            //     (found), solvedContext.asInstType(req), checkContext)); the
            //     listener needs the mutable Attr/Check recursion; registered.
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
            // TODO(Check/Types + printing batch): checkContext.report(pos,
            //     diags.fragment(Fragments.PossibleLossOfPrecision(found, req)))
            //     needs the Type argument printing (DiagnosticArg.TypeArg) and
            //     types.createErrorType(found); registered, no substitute text.
            unimplemented!("TODO(Check/Types + printing batch): Check.checkType possible loss of precision");
        }
        // TODO(Check/Types + printing batch): checkContext.report(pos,
        //     diags.fragment(Fragments.InconvertibleTypes(found, req))) + the
        //     error type; registered, no substitute text.
        unimplemented!("TODO(Check/Types + printing batch): Check.checkType inconvertible types")
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
        self.type_tag_error(ctx, pos, required, DiagnosticArg::from_display("type"))
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
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &Types,
        _found: TypeId,
        _req: TypeId,
        _warn: &mut Warner,
    ) -> bool {
        // TODO(Types batch): types.isAssignable(found, req, warn) is not ported.
        unimplemented!("TODO(Types batch): Check.basicHandler compatible (Types.isAssignable)")
    }

    fn report(&self, _ctx: &mut CompilerContext, _pos: i32, _details: &JCDiagnostic) {
        // TODO(Check/Types + printing batch): log.error(pos,
        //     Errors.ProbFoundReq(details)) needs the JCDiagnostic diagnostic
        //     argument mapping (DiagnosticArg has no JCDiagnostic variant in the
        //     port yet).
        unimplemented!("TODO(Check/Types + printing batch): Check.basicHandler report")
    }

    fn check_warner(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &Types,
        _pos: i32,
        _found: TypeId,
        _req: TypeId,
    ) -> Warner {
        // TODO(Check batch): convertWarner(pos, found, req) is not ported.
        unimplemented!("TODO(Check batch): Check.basicHandler checkWarner (convertWarner)")
    }

    fn inference_context(&self) -> &Rc<InferenceContext> {
        &self.infer_empty
    }

    fn deferred_attr_context(&self) -> &Rc<DeferredAttrContext> {
        &self.deferred_empty
    }
}
