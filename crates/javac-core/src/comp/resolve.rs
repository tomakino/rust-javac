// PORTING_RULES: the upstream-shaped method-check code keeps large error
// enums and nested conditionals; the lints are allowed locally (registered).
#![allow(clippy::result_large_err, clippy::collapsible_if)]
// Origin: com/sun/tools/javac/comp/Resolve.java @ 982d53c (jdk-17.0.19-ga)

//! Name/method resolution (Java: com.sun.tools.javac.comp.Resolve).
//!
//! PORTING_RULES:
//! * the resolution services are injected through the explicit-context adapter
//!   (ADR 0001): Symtab/Types are passed as parameters, not stored global
//!   singletons; the ported Resolve state keeps names/log/types/diags/source/
//!   options only.
//! * the Java overload sets become distinct Rust names (PORTING_RULES Porting rules):
//!   isAccessible(Env, TypeSymbol) -> is_accessible_class,
//!   isAccessible(Env, Type) -> is_accessible_type,
//!   isAccessible(Env, Type, Symbol) -> is_accessible_member.
//! * Resolve.instance(Context) is registered for the Context registration.
//!
//! Ported in the Resolve B (this file, Porting rules rows in the report):
//! - Resolve.methodResolutionSteps/MethodResolutionPhase (BASIC/BOX/VARARITY,
//!   mergeResults), MethodResolutionContext/Candidate, InapplicableMethodException,
//!   MethodCheckDiag, LookupFilter, InterfaceLookupPhase, superclasses;
//! - rawInstantiate/checkMethod/instantiate (Resolve.java:568-733);
//! - MethodCheck implementations: NilMethodCheck/ArityMethodCheck/
//!   ResolveMethodCheck/MostSpecificCheck/MethodReferenceCheck (class + entry),
//!   AbstractMethodCheck.argumentsAcceptable, MethodCheckContext, MethodResultInfo,
//!   adjustArgs (varargs branch), varargsAccessible (registered access check);
//! - selectBest/mostSpecific/signatureMoreSpecific/ambiguityError/
//!   findMethodInScope/findMethod/findFun/findConstructor/findDiamond;
//! - lookupMethod/LookupHelper/BasicLookupHelper and the resolve* family:
//!   resolveMethod/resolveQualifiedMethod/resolveInternalMethod/
//!   resolveConstructor/resolveInternalConstructor/resolveDiamond;
//! - ResolveError data + diagnostics: InapplicableSymbolError/
//!   InapplicableSymbolsError/DiamondError/BadVarargsMethod added to the
//!   existing SymbolNotFound/Access/Invisible/Static/Ambiguity/BadRestrictedType
//!   hierarchy (Resolve.java:3904-4720);
//! - access errors/ambiguity helpers (new_access_error/new_ambiguity_error/
//!   new_static_error/resolve_error_access/resolve_error_diagnostic).
//!
//! NOT YET PORTED IN THIS FILE (registered; each with a not-yet-ported marker + TODO):
//! - member-reference resolution: makeReferenceLookupHelper/
//!   MethodReferenceLookupHelper/UnboundMethodReferenceLookupHelper/
//!   ArrayConstructorReferenceLookupHelper/ConstructorReferenceLookupHelper/
//!   ReferenceLookupResult/getMemberReference/resolveMemberReference/
//!   ReferenceLookupHelper (Resolve.java:3000-3650; needs
//!   Types.findDescriptorType + Attr member-reference wiring, FunctionDescriptor
//!   );
//! - MethodReferenceCheck.argumentsAcceptable body (Resolve.java:1000-1070,
//!   same dependency);
//! - selectBest inference-exception propagation (Resolve.java:1592-1593) and
//!   AmbiguityError.mergeAbstracts (Types);
//! - varargsAccessible isAccessible/reportMC diagnostics (Check);
//! - verbose resolution diagnostics (reportVerboseResolutionDiagnostic/
//!   VerboseResolutionMode): Log/Options;
//! - MethodSymbol.implementation(hiddenIn path): Symbol;
//! - Type.isUnion/UnionClassType and Types.overrideEquivalent/
//!   Types.mergeAbstracts: Types.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{
    package_fullname, package_module, symbol_is_inherited_in, symbol_is_sub_class,
    symbol_location_site, symbol_outermost_class, symbol_packge,
};
use crate::code::types::get_parameter_types;
use crate::code::source::{Feature, Source};
use crate::code::symtab::Symtab;
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;
use crate::ids::{ScopeId, SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::TreeKind;
use crate::util::jc_diagnostic::Factory;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::names::Names;
use crate::util::options::Options;

/// Java: public class Resolve (comp/Resolve.java:93)
pub struct Resolve {
    /// Java: private final SymbolNotFoundError varNotFound (ABSENT_VAR).
    pub var_not_found: SymbolId,
    /// Java: private final SymbolNotFoundError methodNotFound (ABSENT_MTH).
    pub method_not_found: SymbolId,
    /// Java: private final SymbolNotFoundError typeNotFound (ABSENT_TYP).
    pub type_not_found: SymbolId,
    /// Java: private final ClassFinder finder.
    pub finder: Option<Rc<crate::code::class_finder::ClassFinder>>,
    /// Java: public final boolean allowFunctionalInterfaceMostSpecific.
    pub allow_functional_interface_most_specific: bool,
    /// Java: private final boolean allowLocalVariableTypeInference.
    pub allow_local_variable_type_inference: bool,
    /// Java: private final boolean allowYieldStatement.
    pub allow_yield_statement: bool,
    /// Java: public final boolean checkVarargsAccessAfterResolution.
    pub check_varargs_access_after_resolution: bool,
    /// Java: public final boolean allowRecords.
    pub allow_records: bool,
    /// Java: "private final Names names" (the ported dependency set).
    pub names: Rc<Names>,
    /// Java: "private final Log log".
    pub log: Rc<std::cell::RefCell<Log>>,
    /// Java: "private final Types types".
    pub types: Rc<Types>,
    /// Java: "private final JCDiagnostic.Factory diags".
    pub diags: Rc<Factory>,
    /// Java: "final Source source".
    pub source: Source,
    /// Java: "private final boolean allowModules" (new Options()).
    pub allow_modules: bool,
    /// Java: "private final Options options" — kept for the verbose resolution
    /// flags (not ported yet).
    pub options: Rc<Options>,
    /// Java: private MethodCheck resolveMethodCheck (default method check)
    pub default_method_check: Rc<RefCell<dyn MethodCheck>>,
    /// Java: WriteableScope polymorphicSignatureScope (Resolve.java:118; lazily
    /// created on first use because the port Resolve has no Symtab field, Porting rules).
    pub polymorphic_signature_scope: Option<ScopeId>,
    /// Java: MethodResolutionContext currentResolutionContext (Resolve.java:5005):
    /// the step read by accessInternal for the deferred argument-type mapping
    /// (the resolution context is otherwise passed explicitly; registered adapter:
    /// the port stores the immutable step while a method check runs).
    pub current_resolution_step: Option<MethodResolutionPhase>,
    /// The Attr service used by the deferred argument-type mapping in
    /// accessInternal (Java: methodLogResolveHelper/DeferredAttr; the port wires
    /// the Attr cell after construction — registered adapter).
    pub attr_cell: Rc<RefCell<Option<Rc<RefCell<crate::comp::attr::Attr>>>>>, 
    /// Java: private final ModuleFinder moduleFinder (Resolve.java:105) — the
    /// module finder service (wired after construction; registered adapter
    /// cell like attr_cell).
    pub module_finder: Rc<RefCell<Option<Rc<RefCell<crate::code::module_finder::ModuleFinder>>>>>, 
}

impl Resolve {
    /// Java: protected Resolve(Context context) (Resolve.java:120).
    /// The explicit-context adapter takes the ported dependency set directly.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: &mut crate::compiler_context::CompilerContext,
        names: Rc<Names>,
        log: Rc<std::cell::RefCell<Log>>,
        types: Rc<Types>,
        diags: Rc<Factory>,
        source: Source,
        options: Rc<Options>,
        finder: Option<Rc<crate::code::class_finder::ClassFinder>>,
    ) -> Self {
        let allow_modules = Feature::MODULES.allowed_in_source(source);
        let var_not_found = crate::code::symbol::symbol_new_resolve_error(
            ctx,
            Kind::ABSENT_VAR,
            names.empty.clone(),
            "symbol not found error",
            crate::code::symbol::ResolveErrorData::SymbolNotFound,
        );
        let method_not_found = crate::code::symbol::symbol_new_resolve_error(
            ctx,
            Kind::ABSENT_MTH,
            names.empty.clone(),
            "symbol not found error",
            crate::code::symbol::ResolveErrorData::SymbolNotFound,
        );
        let type_not_found = crate::code::symbol::symbol_new_resolve_error(
            ctx,
            Kind::ABSENT_TYP,
            names.empty.clone(),
            "symbol not found error",
            crate::code::symbol::ResolveErrorData::SymbolNotFound,
        );
        let default_method_check: Rc<RefCell<dyn MethodCheck>> =
            Rc::new(RefCell::new(ResolveMethodCheck {
                diags: Rc::clone(&diags),
                check_varargs_access_after_resolution:
                    Feature::POST_APPLICABILITY_VARARGS_ACCESS_CHECK.allowed_in_source(source),
                allow_modules,
            }));
        Resolve {
            var_not_found,
            method_not_found,
            type_not_found,
            finder,
            allow_functional_interface_most_specific: Feature::FUNCTIONAL_INTERFACE_MOST_SPECIFIC
                .allowed_in_source(source),
            allow_local_variable_type_inference: Feature::LOCAL_VARIABLE_TYPE_INFERENCE
                .allowed_in_source(source),
            allow_yield_statement: Feature::SWITCH_EXPRESSION.allowed_in_source(source),
            check_varargs_access_after_resolution: Feature::POST_APPLICABILITY_VARARGS_ACCESS_CHECK
                .allowed_in_source(source),
            allow_records: Feature::RECORDS.allowed_in_source(source),
            names,
            log,
            types,
            diags,
            source,
            allow_modules,
            options,
            default_method_check,
            polymorphic_signature_scope: None,
            current_resolution_step: None,
            attr_cell: Rc::new(RefCell::new(None)),
            module_finder: Rc::new(RefCell::new(None)),
        }
    }

    /// Java: public static Resolve instance(Context context) (Resolve.java:167)
    ///
    /// PORTING_RULES: the ported Resolve is created by the service wiring with
    /// its explicit dependencies; the Context registration makes the instance
    /// reachable like the Java Context singleton (registered adapter: the
    /// instance must be registered by the compiler entry point).
    pub fn instance(
        context: &mut crate::util::context::Context,
    ) -> Rc<RefCell<Resolve>> {
        match context.get::<RefCell<Resolve>>() {
            Some(instance) => instance,
            None => panic!("AssertionError: Resolve is not registered in the Context"),
        }
    }

    /// Java: protected static boolean isStatic(Env<AttrContext> env)
    /// (Resolve.java:289)
    pub fn is_static(&self, env: &Rc<Env<AttrContext>>) -> bool {
        let outer = env.outer.as_ref().expect("env.outer");
        env.info.borrow().static_level > outer.info.borrow().static_level
    }

    /// Java: static boolean isInitializer(Env<AttrContext> env)
    /// (Resolve.java:296)
    pub fn is_initializer(&self, ctx: &crate::compiler_context::CompilerContext, env: &Rc<Env<AttrContext>>) -> bool {
        let scope = env.info.borrow().scope.expect("env.info.scope");
        let owner = ctx.scope(scope).owner.expect("scope owner");
        let owner_kind = ctx.sym(owner).kind;
        let owner_flags = ctx.sym(owner).flags_field;
        let owner_owner_kind = ctx
            .sym(owner)
            .owner
            .map(|owner| ctx.sym(owner).kind)
            .expect("symbol owner");
        ctx.sym(owner).is_constructor(&self.names)
            || (owner_owner_kind == Kind::TYP
                && (owner_kind == Kind::VAR
                    || (owner_kind == Kind::MTH && (owner_flags & flags::BLOCK as i64) != 0))
                && (owner_flags & flags::STATIC as i64) == 0)
    }

    /// Java: public boolean isAccessible(Env<AttrContext> env, TypeSymbol c)
    /// and isAccessible(Env, TypeSymbol, boolean checkInner) (Resolve.java:309-375)
    pub fn is_accessible_class(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        c: SymbolId,
        check_inner: bool,
    ) -> bool {
        is_accessible_class_free(&self.types, self.allow_modules, ctx, syms, env, c, check_inner)
    }

    /// Java: private boolean isInnerSubClass(ClassSymbol c, Symbol base)
    /// (Resolve.java:376)
    pub fn is_inner_sub_class(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        c: SymbolId,
        base: SymbolId,
    ) -> bool {
        is_inner_sub_class_free(&self.types, ctx, syms, c, base)
    }

    /// Java: boolean isAccessible(Env<AttrContext> env, Type t)
    /// and (Env, Type, boolean checkInner) (Resolve.java:383-395)
    pub fn is_accessible_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        t: TypeId,
        check_inner: bool,
    ) -> bool {
        is_accessible_type_free(&self.types, self.allow_modules, ctx, syms, env, t, check_inner)
    }

    /// Java: public boolean isAccessible(Env<AttrContext> env, Type site,
    /// Symbol sym) and (Env, Type, Symbol, boolean checkInner)
    /// (Resolve.java:404-460)
    pub fn is_accessible_member(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        sym: SymbolId,
        check_inner: bool,
    ) -> bool {
        let init_name = self.names.init.clone();
        let sym_name = ctx.sym(sym).name.clone();
        let site_tsym = ctx.typ(site).tsym;
        if sym_name.content_equals(&init_name.to_java_string()) && ctx.sym(sym).owner != site_tsym {
            return false;
        }

        // 15.9.5.1: anonymous constructor signature accessibility.
        if env
            .encl_method
            .map(|encl_method| method_decl_flags(ctx, encl_method))
            .is_some_and(|flags_value| (flags_value & flags::ANONCONSTR as i64) != 0)
        {
            return true;
        }

        if env.info.borrow().visiting_service_implementation {
            let toplevel_module = env_toplevel_module(ctx, env);
            let sym_module = package_module(ctx, symbol_packge(ctx, sym).expect("symbol package"));
            if toplevel_module.is_some() && toplevel_module == sym_module {
                return true;
            }
        }

        let access = ctx.sym(sym).flags_field & flags::AccessFlags as i64;
        let toplevel_package = env_toplevel_package(ctx, env);
        let sym_package = symbol_packge(ctx, sym);
        let owner = ctx.sym(sym).owner.expect("symbol owner");
        if access == flags::PRIVATE as i64 {
            let encl_sym = env
                .encl_class
                .and_then(|class_tree| class_decl_symbol(ctx, class_tree))
                .expect("enclClass symbol");
            let same_outermost = symbol_outermost_class(ctx, encl_sym)
                == symbol_outermost_class(ctx, owner);
            let same_owner = encl_sym == owner;
            let inherited = ctx
                .typ(site)
                .tsym
                .map(|site_tsym| symbol_is_inherited_in(ctx, &self.types, syms, sym, site_tsym))
                .unwrap_or(false);
            (same_owner || same_outermost) && inherited
        } else if access == 0 {
            // Java: env.toplevel.packge == sym.owner.owner || ...; the ported
            // package owner may be unset (the Modules bootstrap adapter), in which case the
            // first comparison is false (a null owner does not match a package).
            let owner_owner_matches = ctx
                .sym(owner)
                .owner
                .map(|owner_owner| toplevel_package == Some(owner_owner))
                .unwrap_or(false);
            (owner_owner_matches || toplevel_package == sym_package)
                && self.is_accessible_type(ctx, syms, env, site, check_inner)
                && ctx
                    .typ(site)
                    .tsym
                    .map(|site_tsym| symbol_is_inherited_in(ctx, &self.types, syms, sym, site_tsym))
                    .unwrap_or(false)
                && self.not_overridden_in(ctx, syms, site, sym)
        } else if access == flags::PROTECTED as i64 {
            let owner_owner_matches = ctx
                .sym(owner)
                .owner
                .map(|owner_owner| toplevel_package == Some(owner_owner))
                .unwrap_or(false);
            let select_super = env.info.borrow().select_super;
            let sym_static = (ctx.sym(sym).flags_field & flags::STATIC as i64) != 0;
            let sym_is_type = ctx.sym(sym).kind == Kind::TYP;
            let encl_sym = env
                .encl_class
                .and_then(|class_tree| class_decl_symbol(ctx, class_tree))
                .expect("enclClass symbol");
            (owner_owner_matches
                || toplevel_package == sym_package
                || self.is_protected_accessible(ctx, syms, sym, encl_sym, site)
                || (select_super && !sym_static && !sym_is_type))
                && self.is_accessible_type(ctx, syms, env, site, check_inner)
                && self.not_overridden_in(ctx, syms, site, sym)
        } else {
            // this case includes erroneous combinations as well
            self.is_accessible_type(ctx, syms, env, site, check_inner)
                && self.not_overridden_in(ctx, syms, site, sym)
        }
    }

    /// Java: private boolean notOverriddenIn(Type site, Symbol sym)
    /// (Resolve.java:467-474)
    fn not_overridden_in(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        site: TypeId,
        sym: SymbolId,
    ) -> bool {
        let is_method = ctx.sym(sym).kind == Kind::MTH;
        let is_constructor = ctx.sym(sym).is_constructor(&self.names);
        let is_static = ctx.sym(sym).is_static(ctx);
        if !is_method || is_constructor || is_static {
            return true;
        }
        // Java: ((MethodSymbol)sym).implementation(site.tsym, types, true) == sym
        // (Resolve.java:467-474; Symbol.java:2168-2187).
        let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
        crate::code::symbol::implementation(ctx, syms, &self.types, sym, site_tsym, true)
            == Some(sym)
    }

    /// Java: private boolean isProtectedAccessible(Symbol sym, ClassSymbol c,
    /// Type site) (Resolve.java:484-498)
    fn is_protected_accessible(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
        c: SymbolId,
        site: TypeId,
    ) -> bool {
        let mut new_site = site;
        if ctx.typ(site).has_tag(TypeTag::TYPEVAR) {
            new_site = match &ctx.typ(site).kind {
                TypeKindData::TypeVar { bound, .. } => (*bound).expect("type variable bound"),
                _ => crate::util::assert::error(),
            };
        }
        let mut current = Some(c);
        while let Some(class) = current {
            let sym_owner = ctx.sym(sym).owner.expect("symbol owner");
            if symbol_is_sub_class(ctx, &self.types, syms, class, sym_owner) {
                let class_is_interface =
                    (ctx.sym(class).flags_field & flags::INTERFACE as i64) != 0;
                // JLS 2e 6.6.2.1: the subclass restriction applies only to
                // instance fields and methods; types are excluded regardless of
                // static.
                let static_or_type = (ctx.sym(sym).flags_field & flags::STATIC as i64) != 0
                    || ctx.sym(sym).kind == Kind::TYP;
                let site_subclass = ctx
                    .typ(new_site)
                    .tsym
                    .map(|new_site_tsym| {
                        symbol_is_sub_class(ctx, &self.types, syms, new_site_tsym, class)
                    })
                    .unwrap_or(false);
                if !class_is_interface && (static_or_type || site_subclass) {
                    return true;
                }
            }
            // c = c.owner.enclClass()
            let owner = ctx.sym(class).owner.expect("class owner");
            if ctx.sym(owner).kind == Kind::TYP {
                current = Some(owner);
            } else if ctx.sym(owner).kind == Kind::MTH {
                current = symbols_encl_class(ctx, class);
            } else {
                current = None;
            }
        }
        false
    }

    /// Java: void checkAccessibleType(Env<AttrContext> env, Type t)
    /// (Resolve.java:501) and the AccessibilityChecker SimpleVisitor
    /// (Resolve.java:506-551).
    pub fn check_accessible_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        t: TypeId,
    ) {
        match ctx.typ(t).get_tag() {
            TypeTag::ARRAY => {
                let elemtype = match &ctx.typ(t).kind {
                    TypeKindData::ArrayType { elemtype, .. } => *elemtype,
                    _ => None,
                }
                .expect("array element type");
                self.check_accessible_type(ctx, syms, env, elemtype);
            }
            TypeTag::CLASS => {
                let arguments = crate::comp::attr::type_arguments(ctx, t);
                let mut current = arguments;
                while current.non_empty() {
                    let argument = *current.head();
                    self.check_accessible_type(ctx, syms, env, argument);
                    current = current.tail();
                }
                let tsym = ctx.typ(t).tsym.expect("class type symbol");
                // Java: Resolve.java:528-534 (AccessibilityChecker.visitClassType):
                //   if (!isAccessible(env, t, true))
                //       accessBase(new AccessError(env, null, t.tsym),
                //                  env.tree.pos(), env.enclClass.sym, t, t.tsym.name, true);
                if !self.is_accessible_type(ctx, &*syms, env, t, true) {
                    let error = self.new_access_error(ctx, syms, env, None, tsym);
                    let pos_tree = env.tree.expect("env.tree");
                    let pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>> =
                        Some(Rc::new(
                            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(
                                ctx.tree(pos_tree).pos,
                            ),
                        ));
                    let location = env.encl_class.map(|tree| {
                        match &ctx.tree(tree).kind {
                            crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => {
                                *sym
                            }
                            _ => panic!("ClassCastException: JCClassDecl expected"),
                        }
                    });
                    let name = ctx.sym(tsym).name.clone();
                    self.access_base(
                        ctx, syms, error, pos, location, Some(t), &name, true,
                    );
                }
            }
            TypeTag::WILDCARD => {
                let type_ = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { type_, .. } => *type_,
                    _ => None,
                }
                .expect("wildcard type");
                self.check_accessible_type(ctx, syms, env, type_);
            }
            TypeTag::METHOD => {
                let parameter_types = get_parameter_types(ctx, t);
                let mut current = parameter_types;
                while current.non_empty() {
                    let parameter = *current.head();
                    self.check_accessible_type(ctx, syms, env, parameter);
                    current = current.tail();
                }
                let return_type = match &ctx.typ(t).kind {
                    TypeKindData::MethodType { restype, .. } => *restype,
                    _ => None,
                }
                .expect("method return type");
                self.check_accessible_type(ctx, syms, env, return_type);
                let thrown_types = match &ctx.typ(t).kind {
                    TypeKindData::MethodType { thrown, .. } => thrown.clone(),
                    _ => List::nil(),
                };
                let mut current = thrown_types;
                while current.non_empty() {
                    let thrown = *current.head();
                    self.check_accessible_type(ctx, syms, env, thrown);
                    current = current.tail();
                }
            }
            // SimpleVisitor.visitType (default: no children).
            _ => {}
        }
    }

}

/// The enclosing-class symbol of a class member (Java: Symbol.enclClass,
/// Symbol.java:1245): the enclosing class-or-interface.
pub(crate) fn symbols_encl_class(
    ctx: &crate::compiler_context::CompilerContext,
    symbol: SymbolId,
) -> Option<SymbolId> {
    let owner = ctx.sym(symbol).owner?;
    if ctx.sym(owner).kind == Kind::TYP {
        return Some(owner);
    }
    if ctx.sym(owner).kind == Kind::PCK || ctx.sym(owner).kind == Kind::MDL {
        return None;
    }
    symbols_encl_class(ctx, owner)
}

/// Java: ClassSymbol of a class declaration tree (JCTree.JCClassDecl.sym)
pub(crate) fn class_decl_symbol(
    ctx: &crate::compiler_context::CompilerContext,
    class_tree: TreeId,
) -> Option<SymbolId> {
    match &ctx.tree(class_tree).kind {
        TreeKind::ClassDecl { sym, .. } => *sym,
        _ => None,
    }
}

/// Java: JCCompilationUnit.packge (the package symbol of the compilation unit).
pub(crate) fn env_toplevel_package(
    ctx: &crate::compiler_context::CompilerContext,
    env: &Rc<Env<AttrContext>>,
) -> Option<SymbolId> {
    let toplevel = env.toplevel.expect("env.toplevel");
    match &ctx.tree(toplevel).kind {
        TreeKind::CompilationUnit { packge, .. } => *packge,
        _ => None,
    }
}

/// Java: JCCompilationUnit.modle
pub(crate) fn env_toplevel_module(
    ctx: &crate::compiler_context::CompilerContext,
    env: &Rc<Env<AttrContext>>,
) -> Option<SymbolId> {
    let toplevel = env.toplevel.expect("env.toplevel");
    match &ctx.tree(toplevel).kind {
        TreeKind::CompilationUnit { modle, .. } => *modle,
        _ => None,
    }
}

/// Java: JCMethodDecl.mods.flags
pub(crate) fn method_decl_flags(
    ctx: &crate::compiler_context::CompilerContext,
    method: TreeId,
) -> i64 {
    let mods = match &ctx.tree(method).kind {
        TreeKind::MethodDecl { mods, .. } => *mods,
        _ => None,
    };
    match mods {
        Some(mods) => match &ctx.tree(mods).kind {
            TreeKind::Modifiers { flags, .. } => *flags,
            _ => 0,
        },
        None => 0,
    }
}


// ---------------------------------------------------------------------------
// ResolveError hierarchy (Resolve.java:3909-4700). The Java error classes are
// Symbols; the port stores the payload in SymbolKind::ResolveError and keeps
// the diagnostic logic in these methods.
// ---------------------------------------------------------------------------
impl Resolve {
    /// Java: R? no direct counterpart: creates a ResolveError symbol.
    pub fn new_resolve_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        kind: Kind,
        debug_name: &str,
        data: crate::code::symbol::ResolveErrorData,
    ) -> SymbolId {
        let name = self.names.empty.clone();
        crate::code::symbol::symbol_new_resolve_error(ctx, kind, name, debug_name, data)
    }

    /// Java: ResolveError.exists() / InvalidSymbolError.exists()
    pub fn resolve_error_exists(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        error: SymbolId,
    ) -> bool {
        crate::code::symbol::symbol_exists(ctx, error)
    }

    /// Java: protected Symbol access(Name name, TypeSymbol location) — the
    /// per-error-class access implementation (ResolveError/InvalidSymbolError/
    /// AmbiguityError overrides).
/// # Arguments
///   - ctx/syms: explicit services
///   - error: the resolve-error symbol
///   - name/location: the access target
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:4110-4160 (ResolveError.access overrides).
///
    pub fn resolve_error_access(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        error: SymbolId,
        name: &crate::util::name::Name,
        location: SymbolId,
    ) -> SymbolId {
        let data = crate::code::symbol::symbol_resolve_error_data(ctx, error)
            .expect("resolve error symbol")
            .clone();
        match data {
            crate::code::symbol::ResolveErrorData::Access { sym, .. }
            | crate::code::symbol::ResolveErrorData::Invisible { sym, .. }
            | crate::code::symbol::ResolveErrorData::Static { sym } => {
                // InvalidSymbolError.access
                if ctx.sym(sym).kind.is_resolution_error()
                    || !ctx.sym(sym).kind.matches(crate::code::kinds::KindSelector::TYP)
                {
                    sym
                } else {
                    let original = ctx.sym(sym).ty;
                    let error_type = self.types.create_error_type(
                        ctx,
                        name.clone(),
                        location,
                        original,
                    );
                    ctx.typ(error_type).tsym.expect("error type symbol")
                }
            }
            crate::code::symbol::ResolveErrorData::Ambiguity { syms: ambiguous } => {
                let first = *ambiguous.last().expect("ambiguous symbols");
                if ctx.sym(first).kind == Kind::TYP {
                    let original = ctx.sym(first).ty;
                    let error_type = self.types.create_error_type(
                        ctx,
                        name.clone(),
                        location,
                        original,
                    );
                    ctx.typ(error_type).tsym.expect("error type symbol")
                } else {
                    first
                }
            }
            crate::code::symbol::ResolveErrorData::SymbolNotFound
            | crate::code::symbol::ResolveErrorData::BadRestrictedType { .. } => {
                // ResolveError.access
                let original = ctx.sym(syms.err_symbol).ty;
                let error_type =
                    self.types
                        .create_error_type(ctx, name.clone(), location, original);
                ctx.typ(error_type).tsym.expect("error type symbol")
            }
            crate::code::symbol::ResolveErrorData::Inapplicable { sym, .. }
            | crate::code::symbol::ResolveErrorData::Diamond { sym, .. } => {
                // Java: InapplicableSymbolError.access -> createErrorType with the
                // candidate's type; attrRecover.wrongMethodSymbolCandidate is
                // registered with the Attr.
                let original = ctx.sym(sym).ty;
                let error_type =
                    self.types
                        .create_error_type(ctx, name.clone(), location, original);
                ctx.typ(error_type).tsym.expect("error type symbol")
            }
            crate::code::symbol::ResolveErrorData::InapplicableSymbols { candidates, .. } => {
                let original = candidates
                    .first()
                    .and_then(|(sym, _)| ctx.sym(*sym).ty)
                    .unwrap_or_else(|| ctx.sym(syms.err_symbol).ty.expect("err symbol type"));
                let error_type =
                    self.types
                        .create_error_type(ctx, name.clone(), location, Some(original));
                ctx.typ(error_type).tsym.expect("error type symbol")
            }
            crate::code::symbol::ResolveErrorData::BadMethodReference { sym, .. }
            | crate::code::symbol::ResolveErrorData::BadConstructorReference { sym } => {
                // Java: InvalidSymbolError.access
                if ctx.sym(sym).kind.is_resolution_error()
                    || !ctx.sym(sym).kind.matches(crate::code::kinds::KindSelector::TYP)
                {
                    sym
                } else {
                    let original = ctx.sym(sym).ty;
                    let error_type = self.types.create_error_type(
                        ctx,
                        name.clone(),
                        location,
                        original,
                    );
                    ctx.typ(error_type).tsym.expect("error type symbol")
                }
            }
            crate::code::symbol::ResolveErrorData::BadVarargs { delegated } => {
                // Java: BadVarargsMethod.access -> delegatedError.access
                self.resolve_error_access(ctx, syms, delegated, name, location)
            }
        }
    }

    /// Java: abstract JCDiagnostic getDiagnostic(DiagnosticType dkind,
    /// DiagnosticPosition pos, Symbol location, Type site, Name name,
    /// List<Type> argtypes, List<Type> typeargtypes)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms: explicit services
///   - error: the resolve-error symbol
///   - dkind/pos/location/site/name/argtypes/typeargtypes: the diagnostic context
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:4160-4370 (getDiagnostic overrides).
///
    pub fn resolve_error_diagnostic(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        error: SymbolId,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: Option<&crate::util::name::Name>,
        argtypes: Option<crate::util::list::List<TypeId>>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {
        use crate::resources::compiler_properties::errors;
        let data = crate::code::symbol::symbol_resolve_error_data(ctx, error)
            .expect("resolve error symbol")
            .clone();
        match data {
            crate::code::symbol::ResolveErrorData::SymbolNotFound => self
                .symbol_not_found_diagnostic(
                    ctx, syms, error, dkind, pos, location, site, name, argtypes, typeargtypes,
                ),
            crate::code::symbol::ResolveErrorData::Static { sym } => {
                let kind = ctx.sym(sym).kind;
                let end_sym = if kind == Kind::TYP
                    && ctx
                        .sym(sym)
                        .ty
                        .map(|ty| ctx.typ(ty).has_tag(TypeTag::CLASS))
                        .unwrap_or(false)
                {
                    let erased = self.types.erasure(ctx, syms, ctx.sym(sym).ty.expect("type"));
                    ctx.typ(erased).tsym.unwrap_or(sym)
                } else {
                    sym
                };
                let info = errors::non_static_cant_be_ref(
                    crate::resources::diagnostic_contract::SymbolKindArg::placeholder(
                        crate::code::kinds::kind_name(ctx.sym(sym), ctx).name(),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, end_sym),
                    ),
                );
                let diagnostic = self.factory_error(dkind, pos, info);
                Some(diagnostic)
            }
            crate::code::symbol::ResolveErrorData::BadMethodReference { sym, unbound_lookup } => {
                // Java: BadMethodReferenceError.getDiagnostic (Resolve.java:4655-4675)
                // adapter: the fragment constructors are used (the generated resource
                // helper is the Fragment form of the upstream diags.create key).
                if ctx.sym(sym).kind.is_resolution_error() {
                    return self.resolve_error_diagnostic(
                        ctx, syms, sym, dkind, pos, location, site, name, argtypes, typeargtypes,
                    );
                }
                let kind_name = crate::code::kinds::kind_name(ctx.sym(sym), ctx).name();
                let sym_display = symbol_display_name(ctx, sym);
                let info = if !unbound_lookup {
                    crate::resources::compiler_properties::fragments::bad_static_method_in_bound_lookup(
                        crate::resources::diagnostic_contract::SymbolKindArg::placeholder(kind_name),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(sym_display),
                    )
                } else if ctx.sym(sym).flags_field & (crate::code::flags::STATIC as i64) != 0 {
                    crate::resources::compiler_properties::fragments::bad_static_method_in_unbound_lookup(
                        crate::resources::diagnostic_contract::SymbolKindArg::placeholder(kind_name),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(sym_display),
                    )
                } else {
                    crate::resources::compiler_properties::fragments::bad_instance_method_in_unbound_lookup(
                        crate::resources::diagnostic_contract::SymbolKindArg::placeholder(kind_name),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(sym_display),
                    )
                };
                Some(self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(info)))
            }
            crate::code::symbol::ResolveErrorData::BadConstructorReference { sym } => {
                // Java: BadConstructorReferenceError.getDiagnostic (Resolve.java:4680-4690)
                let _ = sym;
                let site_tsym = site.and_then(|site| ctx.typ(site).tsym);
                let site_name = site_tsym
                    .map(|tsym| symbol_display_name(ctx, tsym))
                    .unwrap_or_default();
                let type_args: Vec<crate::resources::diagnostic_contract::TypeArg> = argtypes
                    .clone()
                    .unwrap_or_else(crate::util::list::List::nil)
                    .iter()
                    .map(|t| {
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            crate::comp::attr::type_display_of(ctx, t),
                        )
                    })
                    .collect();
                let enclosing = site
                    .and_then(|site| ctx.typ(site).get_enclosing_type())
                    .map(|encl| {
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            crate::comp::attr::type_display_of(ctx, encl),
                        )
                    })
                    .unwrap_or_else(|| {
                        crate::resources::diagnostic_contract::TypeArg::placeholder("(no enclosing type)")
                    });
                let info =
                    crate::resources::compiler_properties::fragments::cant_access_inner_cls_constr(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(site_name),
                        type_args,
                        enclosing,
                    );
                Some(self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(info)))
            }
            crate::code::symbol::ResolveErrorData::Ambiguity { syms: ambiguous } => {
                let mut reversed = ambiguous.clone();
                reversed.reverse();
                let s1 = reversed[0];
                let s2 = reversed[1];
                let mut sname = ctx.sym(s1).name.clone();
                if sname == self.names.init {
                    sname = ctx
                        .sym(ctx.sym(s1).owner.expect("ambiguous owner"))
                        .name
                        .clone();
                }
                let site_ty = site.expect("ambiguous site");
                let loc1 = symbol_location_site(
                    ctx,
                    &self.types,
                    syms,
                    s1,
                    site_ty,
                );
                let loc2 = symbol_location_site(
                    ctx,
                    &self.types,
                    syms,
                    s2,
                    site_ty,
                );
                let info = errors::ref_ambiguous(
                    sname,
                    crate::resources::diagnostic_contract::SymbolKindArg::placeholder(
                        crate::code::kinds::kind_name(ctx.sym(s1), ctx).name(),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, s1),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        loc1.map(|loc| symbol_display_name(ctx, loc)).unwrap_or_default(),
                    ),
                    crate::resources::diagnostic_contract::SymbolKindArg::placeholder(
                        crate::code::kinds::kind_name(ctx.sym(s2), ctx).name(),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, s2),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        loc2.map(|loc| symbol_display_name(ctx, loc)).unwrap_or_default(),
                    ),
                );
                Some(self.factory_error(dkind, pos, info))
            }
            crate::code::symbol::ResolveErrorData::BadRestrictedType { type_name } => {
                let info = errors::illegal_ref_to_restricted_type(type_name);
                Some(self.factory_error(dkind, pos, info))
            }
            crate::code::symbol::ResolveErrorData::Access {
                env,
                site: access_site,
                site_inaccessible,
                sym,
            } => self.access_diagnostic(
                ctx,
                syms,
                dkind,
                pos,
                location,
                site,
                name,
                &env,
                access_site,
                site_inaccessible,
                sym,
            ),
            crate::code::symbol::ResolveErrorData::Invisible {
                env,
                suppress_error,
                sym,
            } => self.invisible_diagnostic(ctx, syms, dkind, pos, &env, suppress_error, sym),
            crate::code::symbol::ResolveErrorData::Inapplicable { kind: _, sym, details } => {
                // Java: InapplicableSymbolError.getDiagnostic (Resolve.java:4124-4152)
                if name.map(|name| *name == self.names.error).unwrap_or(false) {
                    return None;
                }
                Some(self.inapplicable_symbol_diagnostic(ctx, syms, dkind, pos, site, argtypes.clone(), sym, details))
            }
            crate::code::symbol::ResolveErrorData::InapplicableSymbols { kind: _, candidates } => {
                // Java: InapplicableSymbolsError.getDiagnostic (Resolve.java:4195-4255)
                if candidates.is_empty() {
                    return self.symbol_not_found_diagnostic(
                        ctx,
                        syms,
                        self.method_not_found,
                        dkind,
                        pos,
                        location,
                        site,
                        name,
                        argtypes,
                        typeargtypes,
                    );
                }
                if candidates.len() == 1 {
                    let (sym, details) = candidates[0].clone();
                    return Some(self.inapplicable_symbol_diagnostic(
                        ctx, syms, dkind, pos, site, argtypes.clone(), sym, details,
                    ));
                }
                // TODO(Resolve diagnostics detail): the upstream creates a
                // MultilineDiagnostic with per-candidate InapplicableMethod
                // fragments (JCDiagnostic.MultilineDiagnostic is registered
                // with the JCDiagnostic).
                let kind_name = if name
                    .map(|name| *name == self.names.init)
                    .unwrap_or(false)
                {
                    crate::code::kinds::KindName::CONSTRUCTOR
                } else {
                    crate::code::kinds::Kind::WRONG_MTH.absent_kind()
                };
                let simple_name = name
                    .cloned()
                    .unwrap_or_else(|| self.names.init.clone());
                let found_args: Vec<crate::resources::diagnostic_contract::TypeArg> = argtypes
                    .map(|argtypes| {
                        let mut out = Vec::new();
                        let mut current = argtypes;
                        while current.non_empty() {
                            out.push(crate::resources::diagnostic_contract::TypeArg::placeholder(
                                type_display_str(ctx, *current.head()),
                            ));
                            current = current.tail();
                        }
                        out
                    })
                    .unwrap_or_default();
                let info = errors::cant_apply_symbols(
                    crate::resources::diagnostic_contract::SymbolKindArg::placeholder(
                        kind_name.name(),
                    ),
                    simple_name,
                    found_args,
                );
                Some(self.factory_error(dkind, pos, info))
            }
            crate::code::symbol::ResolveErrorData::Diamond { kind: _, sym: _, details } => {
                // Java: DiamondError.getDiagnostic (Resolve.java:4330-4350)
                let site_tsym = site
                    .and_then(|site| ctx.typ(site).tsym)
                    .map(|tsym| symbol_display_name(ctx, tsym))
                    .unwrap_or_default();
                let info = errors::cant_apply_diamond(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(site_tsym),
                    (),
                );
                let _ = details;
                Some(self.factory_error(dkind, pos, info))
            }
            crate::code::symbol::ResolveErrorData::BadVarargs { delegated } => self
                .resolve_error_diagnostic(
                    ctx,
                    syms,
                    delegated,
                    dkind,
                    pos,
                    location,
                    site,
                    name,
                    argtypes,
                    typeargtypes,
                ),
        }
    }

    /// Java: InapplicableSymbolError.getDiagnostic (Resolve.java:4124-4152) —
    /// the methodArguments formatting keeps the ADR 0002 P4 display placeholder.
    #[allow(clippy::too_many_arguments)]
    fn inapplicable_symbol_diagnostic(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &Symtab,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        site: Option<TypeId>,
        argtypes: Option<crate::util::list::List<TypeId>>,
        ws: SymbolId,
        details: Option<crate::util::jc_diagnostic::JCDiagnostic>,
    ) -> crate::util::jc_diagnostic::JCDiagnostic {
        use crate::resources::compiler_properties::errors;
        use crate::resources::diagnostic_contract::{
            DiagnosticArg, SymbolKindArg, TypeArg,
        };
        let ws_name = ctx.sym(ws).name.clone();
        let diagnostic_name = if ws_name == self.names.init {
            ctx.sym(ctx.sym(ws).owner.expect("owner")).name.clone()
        } else {
            ws_name
        };
        let ws_owner = ctx.sym(ws).owner.expect("owner");
        let ws_type = ctx.sym(ws).ty.expect("symbol type");
        let required_display = method_arguments_display(ctx, types_parameter_types(ctx, ws_type));
        let found_display = method_arguments_display(ctx, argtypes);
        let owner_type = ctx.sym(ws_owner).ty;
        let info = errors::cant_apply_symbol(
            SymbolKindArg::placeholder(crate::code::kinds::kind_name(ctx.sym(ws), ctx).name()),
            diagnostic_name,
            DiagnosticArg::from_display(required_display),
            DiagnosticArg::from_display(found_display),
            SymbolKindArg::placeholder(
                crate::code::kinds::kind_name(ctx.sym(ws_owner), ctx).name(),
            ),
            TypeArg::placeholder(
                owner_type
                    .map(|t| type_display_str(ctx, t))
                    .unwrap_or_default(),
            ),
            DiagnosticArg::from_display(
                details
                    .map(|d| format!("{d:?}"))
                    .unwrap_or_default(),
            ),
        );
        let _ = site;
        self.factory_error(dkind, pos, info)
    }

    fn factory_error(
        &self,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        info: crate::resources::diagnostic_contract::ErrorInfo,
    ) -> crate::util::jc_diagnostic::JCDiagnostic {
        // The generated resource accessors keep the ADR 0002 P4 placeholder
        // arguments (display text); JCDiagnostic arguments are Rc<dyn Any>.
        let args: Vec<crate::util::jc_diagnostic::DiagnosticArg> = info
            .info()
            .args
            .iter()
            .map(|arg| {
                std::rc::Rc::new(javac_strings::JavaString::from(arg.to_text().as_str()))
                    as crate::util::jc_diagnostic::DiagnosticArg
            })
            .collect();
        let source = self.log.borrow().current_source();
        self.diags
            .create(dkind, source, pos, info.info().code, args)
    }

    /// Java: SymbolNotFoundError.getDiagnostic (Resolve.java:4022-4102)
    #[allow(clippy::too_many_arguments)]
    fn symbol_not_found_diagnostic(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        absent_error: SymbolId,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: Option<&crate::util::name::Name>,
        argtypes: Option<crate::util::list::List<TypeId>>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {

        let error_name = self.names.error.clone();
        let Some(name) = name else {
            // The upstream callers always pass a non-null name; a null name
            // cannot produce the cant.resolve diagnostics.
            return None;
        };
        if *name == error_name {
            return None;
        }
        let mut location = location;
        if location.is_none() {
            location = site.and_then(|site| ctx.typ(site).tsym);
        }
        let location = location.expect("location symbol");
        let location_name = ctx.sym(location).name.clone();
        let mut has_location = false;
        if !location_name.is_empty() {
            if ctx.sym(location).kind == Kind::PCK
                && !crate::code::symbol::symbol_exists(
                    ctx,
                    site.and_then(|site| ctx.typ(site).tsym).expect("site symbol"),
                )
                && location_name != self.names.java
            {
                return Some(self.factory_error(
                    dkind,
                    pos,
                    crate::resources::compiler_properties::errors::doesnt_exist(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(
                            symbol_display_name(ctx, location),
                        ),
                    ),
                ));
            }
            has_location = location_name != self.names._this && location_name != self.names._super;
        }
        let is_constructor = *name == self.names.init;
        let kindname = if is_constructor {
            crate::code::kinds::KindName::CONSTRUCTOR
        } else {
            ctx.sym(absent_error).kind.absent_kind()
        };
        let idname = if is_constructor {
            ctx.sym(site.and_then(|site| ctx.typ(site).tsym).expect("site symbol"))
                .name
                .clone()
        } else {
            name.clone()
        };
        let has_type_args = typeargtypes
            .as_ref()
            .map(|typeargs| typeargs.non_empty())
            .unwrap_or(false);
        let err_key = symbol_not_found_error_key(kindname, has_type_args, has_location);
        let mut contract_args: Vec<crate::resources::diagnostic_contract::DiagnosticArg> = vec![
            crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                kindname.name(),
            ),
            crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                idname.to_java_string(),
            ),
            crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                type_args_place(ctx, typeargtypes),
            ),
            crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                method_arguments_place(ctx, argtypes),
            ),
        ];
        if has_location {
            contract_args.push(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    self.location_diag(ctx, location, site)
                        .get_diagnostic_info()
                        .code
                        .clone(),
                ),
            );
        }
        Some(self.diag_with_contract_args(dkind, pos, &err_key, contract_args))
    }

    fn diag_with_contract_args(
        &self,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        key: &str,
        contract_args: Vec<crate::resources::diagnostic_contract::DiagnosticArg>,
    ) -> crate::util::jc_diagnostic::JCDiagnostic {
        let args: Vec<crate::util::jc_diagnostic::DiagnosticArg> = contract_args
            .iter()
            .map(|arg| {
                std::rc::Rc::new(javac_strings::JavaString::from(arg.to_text().as_str()))
                    as crate::util::jc_diagnostic::DiagnosticArg
            })
            .collect();
        let source = self.log.borrow().current_source();
        self.diags.create(dkind, source, pos, key, args)
    }

    /// Java: SymbolNotFoundError.getLocationDiag (Resolve.java:4009-4017)
    fn location_diag(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        location: SymbolId,
        site: Option<TypeId>,
    ) -> crate::util::jc_diagnostic::JCDiagnostic {
        if ctx.sym(location).kind == Kind::VAR {
            let location_type = ctx.sym(location).ty;
            let info = crate::resources::compiler_properties::fragments::location1(
                kind_name_arg(ctx, location),
                symbol_place(ctx, location),
                type_place(ctx, location_type),
            );
            self.diags.fragment(info.into())
        } else {
            let site_ty = site.expect("location diag site");
            let info = crate::resources::compiler_properties::fragments::location(
                crate::resources::diagnostic_contract::KindNameArg::placeholder(
                    crate::code::kinds::type_kind_name(ctx.typ(site_ty), ctx).name(),
                ),
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    type_place_text(ctx, Some(site_ty)),
                ),
                (),
            );
            self.diags.fragment(info.into())
        }
    }

    /// Java: AccessError.getDiagnostic (Resolve.java:4362-4420)
    #[allow(clippy::too_many_arguments)]
    fn access_diagnostic(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: Option<&crate::util::name::Name>,
        env_snapshot: &crate::code::symbol::ResolveEnvSnapshot,
        access_site: Option<TypeId>,
        site_inaccessible: bool,
        sym: SymbolId,
    ) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {
        let site_tsym = site.and_then(|site| ctx.typ(site).tsym);
        if ctx.sym(sym).name == self.names.init && ctx.sym(sym).owner != site_tsym {
            return self.symbol_not_found_diagnostic(
                ctx,
                syms,
                self.method_not_found,
                dkind,
                pos,
                location,
                site,
                name,
                None,
                None,
            );
        }
        let access = ctx.sym(sym).flags_field & flags::PUBLIC as i64;
        let is_public = access != 0;
        if is_public || (access_site.is_some() && site_inaccessible) {
            let sym_location = crate::code::symbol::symbol_location(ctx, sym);
            let owner = ctx.sym(sym).owner.expect("symbol owner");
            if ctx.sym(owner).kind == Kind::PCK {
                let package = crate::code::symbol::symbol_packge(ctx, sym).expect("package");
                let reason = self.inaccessible_package_reason(ctx, syms, env_snapshot, package);
                return Some(self.factory_error(
                    dkind,
                    pos,
                    crate::resources::compiler_properties::errors::not_def_access_package_cant_access(
                        symbol_place(ctx, sym),
                        symbol_place_opt(ctx, sym_location),
                        reason_place(&reason),
                    ),
                ));
            } else if crate::code::symbol::symbol_packge(ctx, sym) != Some(syms.root_package)
                && !symbol_package_visible(ctx, env_snapshot, sym)
            {
                let package = crate::code::symbol::symbol_packge(ctx, sym).expect("package");
                let reason = self.inaccessible_package_reason(ctx, syms, env_snapshot, package);
                let location_package = sym_location
                    .and_then(|location| crate::code::symbol::symbol_packge(ctx, location));
                return Some(self.factory_error(
                    dkind,
                    pos,
                    crate::resources::compiler_properties::errors::not_def_access_class_intf_cant_access_reason(
                        symbol_place(ctx, sym),
                        symbol_place_opt(ctx, sym_location),
                        symbol_place_opt(ctx, location_package),
                        reason_place(&reason),
                    ),
                ));
            } else {
                return Some(self.factory_error(
                    dkind,
                    pos,
                    crate::resources::compiler_properties::errors::not_def_access_class_intf_cant_access(
                        symbol_place(ctx, sym),
                        symbol_place_opt(ctx, sym_location),
                    ),
                ));
            }
        }
        let access_flags = ctx.sym(sym).flags_field & (flags::PRIVATE | flags::PROTECTED) as i64;
        let sym_location = crate::code::symbol::symbol_location(ctx, sym);
        if access_flags != 0 {
            let modifiers = modifier_args(access_flags);
            return Some(self.factory_error(
                dkind,
                pos,
                crate::resources::compiler_properties::errors::report_access(
                    symbol_place(ctx, sym),
                    modifiers,
                    symbol_place_opt(ctx, sym_location),
                ),
            ));
        }
        Some(self.factory_error(
            dkind,
            pos,
            crate::resources::compiler_properties::errors::not_def_public_cant_access(
                symbol_place(ctx, sym),
                symbol_place_opt(ctx, sym_location),
            ),
        ))
    }

    /// Java: InvisibleSymbolError.getDiagnostic (Resolve.java:4430-4480)
    #[allow(clippy::too_many_arguments)]
    fn invisible_diagnostic(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env_snapshot: &crate::code::symbol::ResolveEnvSnapshot,
        suppress_error: bool,
        sym: SymbolId,
    ) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {
        if suppress_error {
            return None;
        }
        let package = crate::code::symbol::symbol_packge(ctx, sym).expect("symbol package");
        let reason = self.inaccessible_package_reason(ctx, syms, env_snapshot, package);
        if ctx.sym(sym).kind == Kind::PCK {
            return Some(self.factory_error(
                dkind,
                pos,
                crate::resources::compiler_properties::errors::package_not_visible(
                    symbol_place(ctx, sym),
                    reason_place(&reason),
                ),
            ));
        }
        if let Some(position) = pos.as_ref()
            && let Some(mut tree) = position.get_tree()
        {
            let mut o = sym;
            while ctx.sym(o).kind != Kind::PCK
                && ctx.tree(tree).has_tag(crate::tree::jc_tree::Tag::SELECT)
            {
                let owner = ctx.sym(o).owner.expect("symbol owner");
                o = owner;
                tree = match &ctx.tree(tree).kind {
                    TreeKind::FieldAccess {
                        selected: Some(selected),
                        ..
                    } => *selected,
                    _ => break,
                };
            }
            if ctx.sym(o).kind == Kind::PCK {
                let new_pos: std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition> =
                    std::rc::Rc::new(
                        crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(
                            ctx.tree(tree).pos,
                        ),
                    );
                return Some(self.factory_error(
                    dkind,
                    Some(new_pos),
                    crate::resources::compiler_properties::errors::package_not_visible(
                        symbol_place(ctx, o),
                        reason_place(&reason),
                    ),
                ));
            }
        }
        Some(self.factory_error(
            dkind,
            pos,
            crate::resources::compiler_properties::errors::not_def_access_package_cant_access(
                symbol_place(ctx, sym),
                symbol_place_opt(ctx, Some(package)),
                reason_place(&reason),
            ),
        ))
    }

    /// Java: Resolve.inaccessiblePackageReason(Env, PackageSymbol)
    /// (Resolve.java:4425-4494)
    fn inaccessible_package_reason(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        env_snapshot: &crate::code::symbol::ResolveEnvSnapshot,
        package: SymbolId,
    ) -> crate::resources::diagnostic_contract::FragmentInfo {
        use crate::resources::compiler_properties::fragments;
        let env_module = env_snapshot.toplevel_module.expect("toplevel module");
        let package_module = crate::code::symbol::package_module(ctx, package)
            .expect("package module");
        let env_reads = module_read_modules(ctx, env_module);
        if !env_reads.contains(&package_module) {
            if package_module != syms.unnamed_module {
                if env_module != syms.unnamed_module {
                    fragments::not_def_access_does_not_read(
                        symbol_place(ctx, env_module),
                        symbol_place(ctx, package),
                        symbol_place(ctx, package_module),
                    )
                } else {
                    fragments::not_def_access_does_not_read_from_unnamed(
                        symbol_place(ctx, package),
                        symbol_place(ctx, package_module),
                    )
                }
            } else {
                fragments::not_def_access_does_not_read_unnamed(
                    symbol_place(ctx, package),
                    symbol_place(ctx, env_module),
                )
            }
        } else {
            let exported = module_exports_package(ctx, package_module, package);
            if exported {
                if env_module != syms.unnamed_module {
                    fragments::not_def_access_not_exported_to_module(
                        symbol_place(ctx, package),
                        symbol_place(ctx, package_module),
                        symbol_place(ctx, env_module),
                    )
                } else {
                    fragments::not_def_access_not_exported_to_module_from_unnamed(
                        symbol_place(ctx, package),
                        symbol_place(ctx, package_module),
                    )
                }
            } else if env_module != syms.unnamed_module {
                fragments::not_def_access_not_exported(
                    symbol_place(ctx, package),
                    symbol_place(ctx, package_module),
                )
            } else {
                fragments::not_def_access_not_exported_from_unnamed(
                    symbol_place(ctx, package),
                    symbol_place(ctx, package_module),
                )
            }
        }
    }
}

/// Java: Symbol.toString-like display for the diagnostic argument placeholders
/// (the generated SymbolArg is an opaque display placeholder before P4).
pub(crate) fn symbol_display_name(
    ctx: &crate::compiler_context::CompilerContext,
    symbol: SymbolId,
) -> String {
    match &ctx.sym(symbol).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { fullname, .. } => {
            fullname.to_java_string().to_string_lossy()
        }
        crate::code::symbol::SymbolKind::PackageSymbol { fullname, .. }
        | crate::code::symbol::SymbolKind::RootPackageSymbol { fullname, .. } => {
            fullname.to_java_string().to_string_lossy()
        }
        _ => ctx.sym(symbol).name.to_java_string().to_string_lossy(),
    }
}


// ---------------------------------------------------------------------------
// Resolve A: name lookup (Resolve.java:1432-2530). Each method follows the
// upstream body; unimplemented dependencies are explicit TODO registrations.
// ---------------------------------------------------------------------------

/// Java: interface Resolve.RecoveryLoadClass (Resolve.java:2067)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryLoadClass {
    NoRecovery,
    NamedImportScopeRecovery,
    StarImportScopeRecovery,
    DoRecoveryLoadClass,
}

/// Java: private static Symbol bestOf(Symbol s1, Symbol s2) (Resolve.java:174)
fn best_of(ctx: &crate::compiler_context::CompilerContext, s1: SymbolId, s2: SymbolId) -> SymbolId {
    if ctx.sym(s1).kind.better_than(ctx.sym(s2).kind) {
        s1
    } else {
        s2
    }
}

impl Resolve {
    /// Java: public boolean isAccessible(Env, TypeSymbol) — the loadClass call
    /// site uses the one-argument overload.
    fn class_accessible(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        c: SymbolId,
    ) -> bool {
        self.is_accessible_class(ctx, syms, env, c, false)
    }

    /// Java: Symbol findField(Env, Type site, Name name, TypeSymbol c)
    /// (Resolve.java:1432-1471)
    fn find_field(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        mut c: SymbolId,
    ) -> SymbolId {
        // while (c.type.hasTag(TYPEVAR)) c = c.type.getUpperBound().tsym;
        loop {
            let c_ty = ctx.sym(c).ty.expect("type symbol type");
            if !ctx.typ(c_ty).has_tag(TypeTag::TYPEVAR) {
                break;
            }
            let bound = match &ctx.typ(c_ty).kind {
                TypeKindData::TypeVar { bound, .. } => *bound,
                _ => None,
            }
            .expect("type variable bound");
            c = ctx.typ(bound).tsym.expect("type variable bound symbol");
        }
        let mut best_so_far = self.var_not_found;
        let c_type = ctx.sym(c).ty.expect("type symbol type");
        let members = ctx.sym(c).members();
        if let Some(members) = members {
            let candidates = ctx.scope_get_symbols_by_name_full(
                members,
                name,
                crate::code::scope::LookupKind::RECURSIVE,
                &|_| true,
                &self.types,
                syms,
            );
            for candidate in candidates {
                if ctx.sym(candidate).kind == Kind::VAR
                    && (ctx.sym(candidate).flags_field & flags::SYNTHETIC as i64) == 0
                {
                    return if self.is_accessible_member(ctx, syms, env, site, candidate, false) {
                        candidate
                    } else {
                        self.new_access_error(ctx, syms, env, Some(site), candidate)
                    };
                }
            }
        }
        let st = self.types.supertype(ctx, syms, c_type);
        if ctx.typ(st).has_tag(TypeTag::CLASS) || ctx.typ(st).has_tag(TypeTag::TYPEVAR) {
            let st_tsym = ctx.typ(st).tsym.expect("supertype symbol");
            let sym = self.find_field(ctx, syms, env, site, name, st_tsym);
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        let interfaces = self.types.interfaces(ctx, syms, c_type);
        let mut current = interfaces;
        while ctx.sym(best_so_far).kind != Kind::AMBIGUOUS && current.non_empty() {
            let interface = *current.head();
            let interface_tsym = ctx.typ(interface).tsym.expect("interface symbol");
            let sym = self.find_field(ctx, syms, env, site, name, interface_tsym);
            if crate::code::symbol::symbol_exists(ctx, best_so_far)
                && crate::code::symbol::symbol_exists(ctx, sym)
                && ctx.sym(sym).owner != ctx.sym(best_so_far).owner
            {
                best_so_far = self.new_ambiguity_error(ctx, best_so_far, sym);
            } else {
                best_so_far = best_of(ctx, best_so_far, sym);
            }
            current = current.tail();
        }
        best_so_far
    }

    /// Java: Symbol findVar(Env, Name) (Resolve.java:1484-1560)
    fn find_var(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
    ) -> SymbolId {
        let mut best_so_far = self.var_not_found;
        let mut env1 = Rc::clone(env);
        let mut static_only = false;
        loop {
            if env1.outer.is_none() {
                break;
            }
            let mut sym: Option<SymbolId> = None;
            let scope = env1.info.borrow().scope;
            if let Some(scope) = scope {
                let candidates = ctx.scope_get_symbols_by_name_full(
                    scope,
                    name,
                    crate::code::scope::LookupKind::RECURSIVE,
                    &|_| true,
                    &self.types,
                    syms,
                );
                for candidate in candidates {
                    if ctx.sym(candidate).kind == Kind::VAR
                        && (ctx.sym(candidate).flags_field & flags::SYNTHETIC as i64) == 0
                    {
                        if static_only {
                            return self.new_static_error(ctx, candidate);
                        }
                        sym = Some(candidate);
                        break;
                    }
                }
            }
            if self.is_static(&env1) {
                static_only = true;
            }
            let encl_class = env1.encl_class.expect("enclClass");
            let encl_sym = class_decl_symbol(ctx, encl_class).expect("enclClass symbol");
            let encl_type = ctx.sym(encl_sym).ty.expect("enclClass type");
            let sym = match sym {
                Some(sym) => sym,
                None => self.find_field(ctx, syms, &env1, encl_type, name, encl_sym),
            };
            if crate::code::symbol::symbol_exists(ctx, sym) {
                if static_only
                    && ctx.sym(sym).kind == Kind::VAR
                    && ctx.sym(ctx.sym(sym).owner.expect("owner")).kind == Kind::TYP
                    && (ctx.sym(sym).flags_field & flags::STATIC as i64) == 0
                {
                    return self.new_static_error(ctx, sym);
                }
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
            if (ctx.sym(encl_sym).flags_field & flags::STATIC as i64) != 0 {
                static_only = true;
            }
            let outer = env1.outer.clone().expect("env.outer");
            env1 = outer;
        }

        let predef_class = syms.predef_class;
        let predef_type = ctx.sym(predef_class).ty.expect("predefClass type");
        let sym = self.find_field(ctx, syms, env, predef_type, name, predef_class);
        if crate::code::symbol::symbol_exists(ctx, sym) {
            return sym;
        }
        if crate::code::symbol::symbol_exists(ctx, best_so_far) {
            return best_so_far;
        }

        let mut origin: Option<SymbolId> = None;
        let toplevel = env.toplevel.expect("env.toplevel");
        let (named_import_scope, star_import_scope) = match &ctx.tree(toplevel).kind {
            TreeKind::CompilationUnit {
                named_import_scope,
                star_import_scope,
                ..
            } => (*named_import_scope, *star_import_scope),
            _ => (None, None),
        };
        for import_scope in [named_import_scope, star_import_scope] {
            let Some(import_scope) = import_scope else {
                continue;
            };
            let candidates = ctx.scope_get_symbols_by_name_full(
                import_scope,
                name,
                crate::code::scope::LookupKind::RECURSIVE,
                &|_| true,
                &self.types,
                syms,
            );
            for current_symbol in candidates {
                if ctx.sym(current_symbol).kind != Kind::VAR {
                    continue;
                }
                if !ctx.sym(best_so_far).kind.is_resolution_error()
                    && ctx.sym(current_symbol).owner != ctx.sym(best_so_far).owner
                {
                    return self.new_ambiguity_error(ctx, best_so_far, current_symbol);
                } else if !ctx.sym(best_so_far).kind.better_than(Kind::VAR) {
                    let import_scope_snapshot = ctx.scope(import_scope).clone();
                    let origin_scope = import_scope_snapshot.get_origin_full(
                        import_scope,
                        current_symbol,
                        &self.types,
                        syms,
                        ctx,
                    );
                    let origin_symbol = origin_scope
                        .and_then(|scope| ctx.scope(scope).owner)
                        .expect("import scope origin owner");
                    origin = Some(origin_symbol);
                    let origin_type = ctx.sym(origin_symbol).ty.expect("origin type");
                    best_so_far =
                        if self.is_accessible_member(ctx, syms, env, origin_type, current_symbol, false)
                        {
                            current_symbol
                        } else {
                            self.new_access_error(ctx, syms, env, Some(origin_type), current_symbol)
                        };
                }
            }
            if crate::code::symbol::symbol_exists(ctx, best_so_far) {
                break;
            }
        }
        if ctx.sym(best_so_far).kind == Kind::VAR {
            let best_owner = ctx.sym(best_so_far).owner.expect("best owner");
            let origin = origin.expect("import origin");
            let best_owner_type = ctx.sym(best_owner).ty;
            let origin_type = ctx.sym(origin).ty;
            if best_owner_type != origin_type {
                return self.clone_symbol_with_owner(ctx, best_so_far, origin);
            }
        }
        best_so_far
    }
}


impl Resolve {
    /// Java: Symbol findImmediateMemberType(...) (Resolve.java:2221-2235)
    fn find_immediate_member_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        c: SymbolId,
    ) -> SymbolId {
        let members = ctx.sym(c).members();
        if let Some(members) = members {
            let candidates = ctx.scope_get_symbols_by_name_full(
                members,
                name,
                crate::code::scope::LookupKind::RECURSIVE,
                &|_| true,
                &self.types,
                syms,
            );
            for candidate in candidates {
                if ctx.sym(candidate).kind == Kind::TYP {
                    return if self.is_accessible_member(ctx, syms, env, site, candidate, false) {
                        candidate
                    } else {
                        self.new_access_error(ctx, syms, env, Some(site), candidate)
                    };
                }
            }
        }
        self.type_not_found
    }

    /// Java: Symbol findInheritedMemberType(...) (Resolve.java:2244-2274)
    fn find_inherited_member_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        c: SymbolId,
    ) -> SymbolId {
        let mut best_so_far = self.type_not_found;
        let c_type = ctx.sym(c).ty.expect("type symbol type");
        let st = self.types.supertype(ctx, syms, c_type);
        if ctx.typ(st).has_tag(TypeTag::CLASS) {
            let st_tsym = ctx.typ(st).tsym.expect("supertype symbol");
            let sym = self.find_member_type(ctx, syms, env, site, name, st_tsym);
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        let interfaces = self.types.interfaces(ctx, syms, c_type);
        let mut current = interfaces;
        while ctx.sym(best_so_far).kind != Kind::AMBIGUOUS && current.non_empty() {
            let interface = *current.head();
            let interface_tsym = ctx.typ(interface).tsym.expect("interface symbol");
            let sym = self.find_member_type(ctx, syms, env, site, name, interface_tsym);
            if !ctx.sym(best_so_far).kind.is_resolution_error()
                && !ctx.sym(sym).kind.is_resolution_error()
                && ctx.sym(sym).owner != ctx.sym(best_so_far).owner
            {
                best_so_far = self.new_ambiguity_error(ctx, best_so_far, sym);
            } else {
                best_so_far = best_of(ctx, best_so_far, sym);
            }
            current = current.tail();
        }
        best_so_far
    }

    /// Java: Symbol findMemberType(...) (Resolve.java:2281-2294)
    fn find_member_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        c: SymbolId,
    ) -> SymbolId {
        let sym = self.find_immediate_member_type(ctx, syms, env, site, name, c);
        if sym != self.type_not_found {
            return sym;
        }
        self.find_inherited_member_type(ctx, syms, env, site, name, c)
    }

    /// Java: Symbol findGlobalType(Env, Scope, Name, RecoveryLoadClass)
    /// (Resolve.java:2296-2307)
    fn find_global_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        scope: Option<crate::ids::ScopeId>,
        name: &crate::util::name::Name,
        recovery: RecoveryLoadClass,
    ) -> SymbolId {
        let mut best_so_far = self.type_not_found;
        let Some(scope) = scope else {
            return best_so_far;
        };
        let candidates = ctx.scope_get_symbols_by_name_full(
            scope,
            name,
            crate::code::scope::LookupKind::RECURSIVE,
            &|_| true,
            &self.types,
            syms,
        );
        for candidate in candidates {
            let flat_name = symbol_flat_name(ctx, candidate);
            let sym = self.load_class(ctx, syms, env, &flat_name, recovery);
            if ctx.sym(best_so_far).kind == Kind::TYP
                && ctx.sym(sym).kind == Kind::TYP
                && best_so_far != sym
            {
                return self.new_ambiguity_error(ctx, best_so_far, sym);
            }
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        best_so_far
    }

    /// Java: Symbol findTypeVar(Env, Name, boolean staticOnly)
    /// (Resolve.java:2309-2327)
    fn find_type_var(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
        static_only: bool,
    ) -> SymbolId {
        let scope = env.info.borrow().scope;
        let Some(scope) = scope else {
            return self.type_not_found;
        };
        let candidates = ctx.scope_get_symbols_by_name(
            scope,
            name,
            crate::code::scope::LookupKind::RECURSIVE,
            &|_| true,
        );
        for candidate in candidates {
            if ctx.sym(candidate).kind == Kind::TYP {
                let candidate_type = ctx.sym(candidate).ty.expect("type symbol type");
                let candidate_owner = ctx.sym(candidate).owner.expect("type symbol owner");
                if ctx.typ(candidate_type).has_tag(TypeTag::TYPEVAR)
                    && (static_only
                        || (self.is_static(env) && ctx.sym(candidate_owner).kind == Kind::TYP))
                {
                    return self.new_static_error(ctx, candidate);
                }
                return candidate;
            }
        }
        self.type_not_found
    }

    /// Java: Symbol findType(Env, Name) (Resolve.java:2329-2400)
    fn find_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
    ) -> SymbolId {
        let empty = &self.names.empty;
        if name == empty {
            return self.type_not_found;
        }
        let mut best_so_far = self.type_not_found;
        let mut static_only = false;
        let mut env1 = Rc::clone(env);
        loop {
            if env1.outer.is_none() {
                break;
            }
            let tyvar = self.find_type_var(ctx, &env1, name, static_only);
            if self.is_static(&env1) {
                static_only = true;
            }
            let encl_class = env1.encl_class.expect("enclClass");
            let encl_sym = class_decl_symbol(ctx, encl_class).expect("enclClass symbol");
            let encl_type = ctx.sym(encl_sym).ty.expect("enclClass type");
            let mut sym =
                self.find_immediate_member_type(ctx, syms, &env1, encl_type, name, encl_sym);
            if tyvar != self.type_not_found {
                let tyvar_kind = ctx.sym(tyvar).kind;
                let tyvar_owner = ctx.sym(tyvar).owner.expect("tyvar owner");
                if env.base_clause
                    || sym == self.type_not_found
                    || (tyvar_kind == Kind::TYP
                        && crate::code::symbol::symbol_exists(ctx, tyvar)
                        && ctx.sym(tyvar_owner).kind == Kind::MTH)
                {
                    return tyvar;
                }
            }
            if sym == self.type_not_found {
                sym = self.find_inherited_member_type(ctx, syms, &env1, encl_type, name, encl_sym);
            }
            if static_only && ctx.sym(sym).kind == Kind::TYP {
                let sym_type = ctx.sym(sym).ty.expect("type symbol");
                let no_type = crate::code::symtab::no_type_id(ctx);
                let sym_enclosing = ctx.typ(sym_type).get_enclosing_type().unwrap_or(no_type);
                if ctx.typ(sym_type).has_tag(TypeTag::CLASS)
                    && ctx.typ(sym_enclosing).has_tag(TypeTag::CLASS)
                    && type_is_parameterized(ctx, &self.types, syms, encl_type)
                    && type_is_parameterized(ctx, &self.types, syms, sym_enclosing)
                {
                    return self.new_static_error(ctx, sym);
                }
            }
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
            let encl = if env1.base_clause {
                env1.tree.expect("env.tree")
            } else {
                env1.encl_class.expect("env.enclClass")
            };
            let encl_sym = class_decl_symbol(ctx, encl).expect("enclosing class symbol");
            if (ctx.sym(encl_sym).flags_field & flags::STATIC as i64) != 0 {
                static_only = true;
            }
            let outer = env1.outer.clone().expect("env.outer");
            env1 = outer;
        }
        let toplevel = env.toplevel.expect("env.toplevel");
        let tree_is_import = env
            .tree
            .map(|tree| ctx.tree(tree).has_tag(crate::tree::jc_tree::Tag::IMPORT))
            .unwrap_or(false);
        if !tree_is_import {
            let (named_scope, toplevel_scope, package_members, star_scope) =
                match &ctx.tree(toplevel).kind {
                    TreeKind::CompilationUnit {
                        named_import_scope,
                        toplevel_scope,
                        packge,
                        star_import_scope,
                        ..
                    } => {
                        let package_members = packge.and_then(|p| ctx.sym(p).members());
                        (
                            *named_import_scope,
                            *toplevel_scope,
                            package_members,
                            *star_import_scope,
                        )
                    }
                    _ => (None, None, None, None),
                };
            let sym = self.find_global_type(
                ctx,
                syms,
                env,
                named_scope,
                name,
                RecoveryLoadClass::NamedImportScopeRecovery,
            );
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
            let sym = self.find_global_type(
                ctx,
                syms,
                env,
                toplevel_scope,
                name,
                RecoveryLoadClass::NoRecovery,
            );
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
            let sym = self.find_global_type(
                ctx,
                syms,
                env,
                package_members,
                name,
                RecoveryLoadClass::NoRecovery,
            );
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
            let sym = self.find_global_type(
                ctx,
                syms,
                env,
                star_scope,
                name,
                RecoveryLoadClass::StarImportScopeRecovery,
            );
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        best_so_far
    }
}

// ---------------------------------------------------------------------------
// ResolveError construction helpers (the Java error class constructors) and the
// remaining Resolve A name lookup / access-check methods.
// ---------------------------------------------------------------------------

impl Resolve {
    /// Java: new AccessError(env, site, sym) (Resolve.java:4351-4360)
    fn new_access_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: Option<TypeId>,
        sym: SymbolId,
    ) -> SymbolId {
        let snapshot = resolve_env_snapshot(ctx, env);
        let site_inaccessible = match site {
            Some(site) => !self.is_accessible_type(ctx, _syms, env, site, false),
            None => false,
        };
        let data = crate::code::symbol::ResolveErrorData::Access {
            env: snapshot,
            site,
            site_inaccessible,
            sym,
        };
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            Kind::HIDDEN,
            self.names.empty.clone(),
            "access error",
            data,
        )
    }

    /// Java: new AmbiguityError(sym1, sym2) (Resolve.java:4548-4577)
    fn new_ambiguity_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        sym1: SymbolId,
        sym2: SymbolId,
    ) -> SymbolId {
        let mut syms = flatten_ambiguous(ctx, sym2);
        syms.extend(flatten_ambiguous(ctx, sym1));
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            Kind::AMBIGUOUS,
            self.names.empty.clone(),
            "ambiguity error",
            crate::code::symbol::ResolveErrorData::Ambiguity { syms },
        )
    }

    /// Java: new StaticError(sym) (Resolve.java:4521-4528)
    fn new_static_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        sym: SymbolId,
    ) -> SymbolId {
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            Kind::STATICERR,
            self.names.empty.clone(),
            "static error",
            crate::code::symbol::ResolveErrorData::Static { sym },
        )
    }

    /// Java: Symbol.clone(Symbol newOwner) for the VarSymbol case
    /// (Symbol.java:1676-1700). The upstream anonymous subclass only overrides
    /// baseSymbol/poolKey; the port copies the fields (baseSymbol/poolKey have
    /// no local consumer yet; registered Porting rules adapter).
    fn clone_symbol_with_owner(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        sym: SymbolId,
        new_owner: SymbolId,
    ) -> SymbolId {
        let original = ctx.sym(sym).clone();
        let mut copy = original;
        copy.owner = Some(new_owner);
        ctx.alloc_symbol(copy)
    }
}

/// Java: AmbiguityError.flatten(Symbol sym) (Resolve.java:4566-4572)
fn flatten_ambiguous(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> Vec<SymbolId> {
    if ctx.sym(sym).kind == Kind::AMBIGUOUS {
        match crate::code::symbol::symbol_resolve_error_data(ctx, sym) {
            Some(crate::code::symbol::ResolveErrorData::Ambiguity { syms }) => syms.clone(),
            _ => panic!("ambiguity error symbol expected"),
        }
    } else {
        vec![sym]
    }
}

/// Java: Type.isParameterized() (Type.java:579/1118): a class type is
/// parameterized when it has more than one type parameter (allparams().tail).
pub(crate) fn type_is_parameterized(
    ctx: &mut crate::compiler_context::CompilerContext,
    _types: &Types,
    _syms: &Symtab,
    t: TypeId,
) -> bool {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType {
            allparams_field, ..
        } => allparams_field
            .as_ref()
            .map(|params| params.tail().non_empty())
            .unwrap_or(false),
        _ => false,
    }
}

/// Java: Type.isErroneous(List<Type>) (Type.java:568-572)
pub(crate) fn type_list_is_erroneous(
    ctx: &crate::compiler_context::CompilerContext,
    types: crate::util::list::List<TypeId>,
) -> bool {
    let mut current = types;
    while current.non_empty() {
        let t = *current.head();
        if ctx.typ(t).has_tag(TypeTag::ERROR) {
            return true;
        }
        current = current.tail();
    }
    false
}

/// Java: Symbol.flatName() — reads the ClassSymbol flat name (the only symbols
/// passed to loadClass are type symbols).
pub(crate) fn symbol_flat_name(
    ctx: &crate::compiler_context::CompilerContext,
    symbol: SymbolId,
) -> crate::util::name::Name {
    match &ctx.sym(symbol).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
        _ => ctx.sym(symbol).name.clone(),
    }
}

/// Java: Resolve.Env snapshot for the resolution-error payloads.
pub(crate) fn resolve_env_snapshot(
    ctx: &crate::compiler_context::CompilerContext,
    env: &Rc<Env<AttrContext>>,
) -> crate::code::symbol::ResolveEnvSnapshot {
    let encl_class = env.encl_class;
    let encl_class_sym = encl_class.and_then(|tree| class_decl_symbol(ctx, tree));
    crate::code::symbol::ResolveEnvSnapshot {
        tree: env.tree,
        toplevel: env.toplevel,
        encl_class,
        encl_class_sym,
        toplevel_package: env_toplevel_package(ctx, env),
        toplevel_module: env_toplevel_module(ctx, env),
        select_super: env.info.borrow().select_super,
        visiting_service_implementation: env.info.borrow().visiting_service_implementation,
    }
}

impl Resolve {
    /// Java: Symbol loadClass(Env, Name, RecoveryLoadClass)
    /// (Resolve.java:2049-2064)
    fn load_class(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
        recovery: RecoveryLoadClass,
    ) -> SymbolId {
        // Java: Resolve.loadClass uses the Context ClassFinder service
        // (Resolve.java:2049-2064); the ported wiring installs it for every
        // compilation, so a missing service is an assembly assertion (the
        // Option keeps the test constructors that do not exercise class loading).
        let finder = self
            .finder
            .clone()
            .expect("AssertionError: ClassFinder service is not installed");
        let module = env_toplevel_module(ctx, env).expect("toplevel module");
        match finder.load_class(ctx, syms, module, name.clone()) {
            Ok(c) => {
                if self.class_accessible(ctx, syms, env, c) {
                    c
                } else {
                    self.new_access_error(ctx, syms, env, None, c)
                }
            }
            Err(_failure) => {
                // TODO(ClassFinder): the BadClassFile distinction is not
                // representable in the local CompletionFailure; recovery path.
                match recovery {
                    RecoveryLoadClass::NoRecovery => self.type_not_found,
                    RecoveryLoadClass::DoRecoveryLoadClass => {
                        self.do_recovery_load_class(ctx, syms, env, name)
                    }
                    RecoveryLoadClass::NamedImportScopeRecovery
                    | RecoveryLoadClass::StarImportScopeRecovery => {
                        self.recovery_import_scope_lookup(ctx, syms, env, name, recovery)
                    }
                }
            }
        }
    }


    /// Java: doRecoveryLoadClass (Resolve.java:2073-2091).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler context and symbol table;
    ///   - env: the attribution environment;
    ///   - name: the class name.
    ///
    /// # Returns
    ///   - the recovered class symbol, or typeNotFound.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:2073-2091.
    fn do_recovery_load_class(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
    ) -> SymbolId {
        let finder = self
            .finder
            .clone()
            .expect("AssertionError: ClassFinder service is not installed");
        let mut candidates = crate::util::convert::class_candidates(name);
        let candidate_names: Vec<crate::util::name::Name> = {
            let mut result = Vec::new();
            while candidates.non_empty() {
                result.push(candidates.head().clone());
                candidates = candidates.tail();
            }
            result
        };
        // Java: n -> () -> createCompoundIterator(candidates,
        //     c -> syms.getClassesForName(c).iterator()).
        let get = |syms: &Symtab, _name: &crate::util::name::Name| {
            let mut result = Vec::new();
            for candidate in &candidate_names {
                result.extend(syms.get_classes_for_name(candidate));
            }
            result
        };
        // Java: (ms, n) -> { for (Name candidate : candidates) { try {
        //     return finder.loadClass(ms, candidate); } catch (CompletionFailure cf)
        //     { } } return null; }
        let mut load = |ctx: &mut crate::compiler_context::CompilerContext,
                        syms: &mut Symtab,
                        module: SymbolId,
                        _name: &crate::util::name::Name|
        -> Option<SymbolId> {
            for candidate in &candidate_names {
                if let Ok(found) = finder.load_class(ctx, syms, module, candidate.clone()) {
                    return Some(found);
                }
            }
            None
        };
        // Java: sym -> sym.kind == Kind.TYP.
        let mut validate = |ctx: &mut crate::compiler_context::CompilerContext,
                            _syms: &mut Symtab,
                            sym: SymbolId|
        -> bool { ctx.sym(sym).kind == Kind::TYP };
        self.lookup_invisible_symbol(
            ctx,
            syms,
            env,
            name,
            &get,
            &mut load,
            &mut validate,
            self.type_not_found,
        )
    }

    /// Java: private <S extends Symbol> Symbol lookupInvisibleSymbol(Env, Name,
    /// Function<Name, Iterable<S>> get, BiFunction<ModuleSymbol, Name, S> load,
    /// Predicate<S> validate, Symbol defaultResult) (Resolve.java:2152-2193).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler context and symbol table;
    ///   - env: the attribution environment;
    ///   - name: the lookup name;
    ///   - get: the name -> candidate symbols function;
    ///   - load: the module/name -> symbol loader;
    ///   - validate: the candidate predicate;
    ///   - default_result: the fallback symbol.
    ///
    /// # Returns
    ///   - the invisible-symbol error, or defaultResult.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:2152-2193.
    #[allow(clippy::too_many_arguments)]
    fn lookup_invisible_symbol(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
        get: &dyn Fn(&Symtab, &crate::util::name::Name) -> Vec<SymbolId>,
        load: &mut dyn FnMut(
            &mut crate::compiler_context::CompilerContext,
            &mut Symtab,
            SymbolId,
            &crate::util::name::Name,
        ) -> Option<SymbolId>,
        validate: &mut dyn FnMut(
            &mut crate::compiler_context::CompilerContext,
            &mut Symtab,
            SymbolId,
        ) -> bool,
        default_result: SymbolId,
    ) -> SymbolId {
        // even if a class/package cannot be found in the current module and
        // among packages in modules it depends on that are exported for any or
        // this module, the class/package may exist internally in some of these
        // modules, or may exist in a module on which this module does not depend.
        let candidates = get(syms, name);
        for sym in candidates {
            if validate(ctx, syms, sym) {
                return self.create_invisible_symbol_error(ctx, syms, env, sym);
            }
        }
        let env_module = env_toplevel_module(ctx, env).expect("toplevel module");
        let mut recoverable_modules = syms.get_all_modules();
        recoverable_modules.push(syms.unnamed_module);
        recoverable_modules.retain(|module| *module != env_module);
        for original in recoverable_modules {
            // avoid overly eager completing classes from source-based modules, as
            // those may not be completable with the current compiler settings:
            if resolve_module_source_location(ctx, original).is_some() {
                continue;
            }
            let mut module = original;
            if resolve_module_class_location(ctx, module).is_none() {
                let finder = Rc::clone(
                    self.module_finder
                        .borrow()
                        .as_ref()
                        .expect("AssertionError: ModuleFinder service is not installed"),
                );
                module = finder.borrow_mut().find_module(ctx, syms, module);
            }
            if ctx.sym(module).kind != Kind::ERR {
                if let Some(sym) = load(ctx, syms, module, name) {
                    if validate(ctx, syms, sym) {
                        return self.create_invisible_symbol_error(ctx, syms, env, sym);
                    }
                }
            }
        }
        default_result
    }

    /// Java: private Symbol createInvisibleSymbolError(Env, Symbol)
    /// (Resolve.java:2195-2201).
    fn create_invisible_symbol_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        sym: SymbolId,
    ) -> SymbolId {
        let snapshot = resolve_env_snapshot(ctx, env);
        if symbol_package_visible(ctx, &snapshot, sym) {
            self.new_access_error(ctx, syms, env, None, sym)
        } else {
            crate::code::symbol::symbol_new_resolve_error(
                ctx,
                Kind::HIDDEN,
                self.names.empty.clone(),
                "invisible class error",
                crate::code::symbol::ResolveErrorData::Invisible {
                    env: snapshot,
                    suppress_error: false,
                    sym,
                },
            )
        }
    }
    /// Java: namedImportScopeRecovery/starImportScopeRecovery (Resolve.java
    /// 2087-2122): importScope.findFirst(Convert.shortName(name), predicate).
    fn recovery_import_scope_lookup(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
        recovery: RecoveryLoadClass,
    ) -> SymbolId {
        let toplevel = env.toplevel.expect("env.toplevel");
        let import_scope = match &ctx.tree(toplevel).kind {
            TreeKind::CompilationUnit {
                named_import_scope,
                star_import_scope,
                ..
            } => match recovery {
                RecoveryLoadClass::NamedImportScopeRecovery => *named_import_scope,
                _ => *star_import_scope,
            },
            _ => None,
        };
        let Some(import_scope) = import_scope else {
            return self.type_not_found;
        };
        let short = crate::util::convert::short_name(name);
        let candidates = ctx.scope_get_symbols_by_name_full(
            import_scope,
            &short,
            crate::code::scope::LookupKind::RECURSIVE,
            &|_| true,
            &self.types,
            _syms,
        );
        for existing in candidates {
            if ctx.sym(existing).kind == Kind::TYP && symbol_flat_name(ctx, existing) == *name {
                let snapshot = resolve_env_snapshot(ctx, env);
                return crate::code::symbol::symbol_new_resolve_error(
                    ctx,
                    Kind::HIDDEN,
                    self.names.empty.clone(),
                    "invisible class error",
                    crate::code::symbol::ResolveErrorData::Invisible {
                        env: snapshot,
                        suppress_error: true,
                        sym: existing,
                    },
                );
            }
        }
        self.type_not_found
    }

    /// Java: Symbol lookupPackage(Env, Name) (Resolve.java:2124-2150)
    fn lookup_package(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
    ) -> SymbolId {
        let module = env_toplevel_module(ctx, env).expect("toplevel module");
        let pack = syms.lookup_package(ctx, module, name);
        if self.allow_modules && self.import_on_demand(ctx, env, name) {
            let members_empty = ctx.sym(pack).members().is_none()
                || ctx
                    .sym(pack)
                    .members()
                    .map(|scope| ctx.scope_get_symbols(scope, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true).is_empty())
                    .unwrap_or(true);
            if members_empty {
                // Java: return lookupInvisibleSymbol(env, name,
                //     syms::getPackagesForName, syms::enterPackage, sym -> {
                //         sym.complete(); return !sym.members().isEmpty(); }, pack)
                //     (Resolve.java:2127-2130).
                let get = |syms: &Symtab, name: &crate::util::name::Name| {
                    syms.get_packages_for_name(name)
                };
                let mut load = |ctx: &mut crate::compiler_context::CompilerContext,
                                syms: &mut Symtab,
                                module: SymbolId,
                                name: &crate::util::name::Name|
                -> Option<SymbolId> { Some(syms.enter_package(ctx, module, name.clone())) };
                let mut validate = |ctx: &mut crate::compiler_context::CompilerContext,
                                    syms: &mut Symtab,
                                    sym: SymbolId|
                -> bool {
                    // Java: sym.complete(); return !sym.members().isEmpty();
                    let _ = ctx.complete_symbol(syms, sym);
                    let members_empty = ctx.sym(sym).members().is_none()
                        || ctx
                            .sym(sym)
                            .members()
                            .map(|scope| {
                                ctx.scope_get_symbols(
                                    scope,
                                    crate::code::scope::LookupKind::NON_RECURSIVE,
                                    &|_| true,
                                )
                                .is_empty()
                            })
                            .unwrap_or(true);
                    !members_empty
                };
                return self.lookup_invisible_symbol(
                    ctx,
                    syms,
                    env,
                    name,
                    &get,
                    &mut load,
                    &mut validate,
                    pack,
                );
            }
        }
        pack
    }

    /// Java: private boolean isImportOnDemand(Env, Name) (Resolve.java:2152-2164)
    fn import_on_demand(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
    ) -> bool {
        let Some(tree) = env.tree else {
            return false;
        };
        if !ctx.tree(tree).has_tag(crate::tree::jc_tree::Tag::IMPORT) {
            return false;
        }
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => return false,
        };
        let Some(qualid) = qualid else {
            return false;
        };
        if !ctx.tree(qualid).has_tag(crate::tree::jc_tree::Tag::SELECT) {
            return false;
        }
        let tree_name = crate::tree::tree_info::name(ctx, qualid);
        if tree_name != Some(self.names.asterisk.clone()) {
            return false;
        }
        let selected = match &ctx.tree(qualid).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => return false,
        };
        let Some(selected) = selected else {
            return false;
        };
        let full_name = crate::tree::tree_info::full_name(ctx, selected);
        full_name.as_ref() == Some(name)
    }

    /// Java: private Symbol checkRestrictedType(pos, bestSoFar, name)
    /// (Resolve.java:2515-2532)
    fn check_restricted_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        mut best_so_far: SymbolId,
        name: &crate::util::name::Name,
    ) -> SymbolId {
        use crate::util::mandatory_warning_handler::MandatoryWarningLog;
        let kind = ctx.sym(best_so_far).kind;
        if kind == Kind::TYP || kind == Kind::ABSENT_TYP {
            if self.allow_local_variable_type_inference && *name == self.names.var {
                best_so_far = crate::code::symbol::symbol_new_resolve_error(
                    ctx,
                    Kind::BAD_RESTRICTED_TYPE,
                    self.names.empty.clone(),
                    "bad var use",
                    crate::code::symbol::ResolveErrorData::BadRestrictedType {
                        type_name: self.names.var.clone(),
                    },
                );
            } else if *name == self.names.r#yield {
                if self.allow_yield_statement {
                    best_so_far = crate::code::symbol::symbol_new_resolve_error(
                        ctx,
                        Kind::BAD_RESTRICTED_TYPE,
                        self.names.empty.clone(),
                        "bad var use",
                        crate::code::symbol::ResolveErrorData::BadRestrictedType {
                            type_name: self.names.r#yield.clone(),
                        },
                    );
                } else if let Some(pos) = pos {
                    // Java: Resolve.java:2503-2505
                    //   else if (pos != null)
                    //       log.warning(pos, Warnings.IllegalRefToRestrictedType(names.yield));
                    let info =
                        crate::resources::compiler_properties::warnings::illegal_ref_to_restricted_type(
                            self.names.r#yield.clone(),
                        );
                    self.log.borrow_mut().warning(
                        None,
                        Some(pos),
                        crate::comp::check::warning_from_info(info),
                    );
                }
            }
        }
        best_so_far
    }
}


impl Resolve {
    /// Java: Symbol findIdent(pos, Env, Name, KindSelector) (Resolve.java:2401)
    pub fn find_ident(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
        kind: crate::code::kinds::KindSelector,
    ) -> SymbolId {
        let found = self.find_ident_internal(ctx, syms, env, name, kind);
        self.check_restricted_type(ctx, pos, found, name)
    }

    /// Java: Symbol findIdentInternal(Env, Name, KindSelector)
    /// (Resolve.java:2405-2423)
    fn find_ident_internal(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        name: &crate::util::name::Name,
        kind: crate::code::kinds::KindSelector,
    ) -> SymbolId {
        use crate::code::kinds::KindSelector;
        let mut best_so_far = self.type_not_found;
        if kind.contains(KindSelector::VAL) {
            let sym = self.find_var(ctx, syms, env, name);
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        if kind.contains(KindSelector::TYP) {
            let sym = self.find_type(ctx, syms, env, name);
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        if kind.contains(KindSelector::PCK) {
            self.lookup_package(ctx, syms, env, name)
        } else {
            best_so_far
        }
    }

    /// Java: Symbol findIdentInPackage(pos, Env, TypeSymbol pck, Name,
    /// KindSelector) (Resolve.java:2435-2440)
    #[allow(clippy::too_many_arguments)]
    pub fn find_ident_in_package(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        pck: SymbolId,
        name: &crate::util::name::Name,
        kind: crate::code::kinds::KindSelector,
    ) -> SymbolId {
        let found = self.find_ident_in_package_internal(ctx, syms, env, pck, name, kind);
        self.check_restricted_type(ctx, pos, found, name)
    }

    /// Java: Symbol findIdentInPackageInternal(Env, TypeSymbol pck, Name,
    /// KindSelector) (Resolve.java:2441-2470)
    #[allow(clippy::too_many_arguments)]
    fn find_ident_in_package_internal(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        pck: SymbolId,
        name: &crate::util::name::Name,
        kind: crate::code::kinds::KindSelector,
    ) -> SymbolId {
        use crate::code::kinds::KindSelector;
        let fullname = form_full_name(ctx, name, pck);
        let mut best_so_far = self.type_not_found;
        if kind.contains(KindSelector::TYP) {
            let speculative = env.info.borrow().attribution_mode.is_speculative();
            let recovery = if self.allow_modules
                && !kind.contains(KindSelector::PCK)
                && !crate::code::symbol::symbol_exists(ctx, pck)
                && !speculative
            {
                RecoveryLoadClass::DoRecoveryLoadClass
            } else {
                RecoveryLoadClass::NoRecovery
            };
            let sym = self.load_class(ctx, syms, env, &fullname, recovery);
            if crate::code::symbol::symbol_exists(ctx, sym) {
                // don't allow programs to use flatnames
                if ctx.sym(sym).name == *name {
                    return sym;
                }
            } else {
                best_so_far = best_of(ctx, best_so_far, sym);
            }
        }
        if kind.contains(KindSelector::PCK) {
            return self.lookup_package(ctx, syms, env, &fullname);
        }
        best_so_far
    }

    /// Java: Symbol findIdentInType(pos, Env, Type site, Name, KindSelector)
    /// (Resolve.java:2472-2477)
    #[allow(clippy::too_many_arguments)]
    pub fn find_ident_in_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        kind: crate::code::kinds::KindSelector,
    ) -> SymbolId {
        let found = self.find_ident_in_type_internal(ctx, syms, env, site, name, kind);
        self.check_restricted_type(ctx, pos, found, name)
    }

    /// Java: Symbol findIdentInTypeInternal(Env, Type site, Name, KindSelector)
    /// (Resolve.java:2478-2513)
    #[allow(clippy::too_many_arguments)]
    fn find_ident_in_type_internal(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        kind: crate::code::kinds::KindSelector,
    ) -> SymbolId {
        use crate::code::kinds::KindSelector;
        let mut best_so_far = self.type_not_found;
        if kind.contains(KindSelector::VAL) {
            let site_tsym = ctx.typ(site).tsym.expect("site symbol");
            let sym = self.find_field(ctx, syms, env, site, name, site_tsym);
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        if kind.contains(KindSelector::TYP) {
            let site_tsym = ctx.typ(site).tsym.expect("site symbol");
            let sym = self.find_member_type(ctx, syms, env, site, name, site_tsym);
            if crate::code::symbol::symbol_exists(ctx, sym) {
                return sym;
            }
            best_so_far = best_of(ctx, best_so_far, sym);
        }
        best_so_far
    }
}

/// Java: TypeSymbol.formFullName(Name name, Symbol owner): joins the package
/// owner and the simple name.
pub(crate) fn form_full_name(
    ctx: &crate::compiler_context::CompilerContext,
    name: &crate::util::name::Name,
    owner: SymbolId,
) -> crate::util::name::Name {
    if ctx.sym(owner).kind == Kind::PCK && !name.is_empty() {
        let owner_flat = symbol_flat_name(ctx, owner);
        owner_flat.append_char(b'.', name)
    } else {
        name.clone()
    }
}

/// Java: interface Resolve.LogResolveHelper (Resolve.java:2605-2635)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogResolveHelper {
    Basic,
    Silent,
    Method,
}

impl Resolve {
    /// Java: LogResolveHelper.resolveDiagnosticNeeded
    fn resolve_diagnostic_needed(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        helper: LogResolveHelper,
        site: Option<TypeId>,
        argtypes: Option<crate::util::list::List<TypeId>>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> bool {
        match helper {
            LogResolveHelper::Basic => site
                .map(|site| !ctx.typ(site).has_tag(TypeTag::ERROR))
                .unwrap_or(true),
            LogResolveHelper::Silent => false,
            LogResolveHelper::Method => {
                let site_ok = site
                    .map(|site| !ctx.typ(site).has_tag(TypeTag::ERROR))
                    .unwrap_or(true);
                let args_ok = argtypes
                    .map(|argtypes| !type_list_is_erroneous(ctx, argtypes))
                    .unwrap_or(true);
                let typeargs_ok = typeargtypes
                    .map(|typeargtypes| !type_list_is_erroneous(ctx, typeargtypes))
                    .unwrap_or(true);
                site_ok && args_ok && typeargs_ok
            }
        }
    }

    /// Java: Symbol accessInternal(...) (Resolve.java:2535-2554)
    #[allow(clippy::too_many_arguments)]
    pub fn access_internal(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: &crate::util::name::Name,
        qualified: bool,
        mut argtypes: Option<crate::util::list::List<TypeId>>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        attr: Option<&mut crate::comp::attr::Attr>,
        log_resolve_helper: LogResolveHelper,
    ) -> SymbolId {

        if ctx.sym(sym).kind.is_resolution_error() {
            let err_sym = sym;
            let access_location = if qualified {
                site.and_then(|site| ctx.typ(site).tsym).expect("qualified site symbol")
            } else {
                // syms.noSymbol
                syms.no_symbol
            };
            let sym = self.resolve_error_access(ctx, syms, err_sym, name, access_location);
            let _ = sym;
            // Java: argtypes = logResolveHelper.getArgumentTypes(errSym, sym,
            // name, argtypes)
            if log_resolve_helper == LogResolveHelper::Method {
                // Java: argtypes = logResolveHelper.getArgumentTypes(errSym, sym,
                // name, argtypes) with methodLogResolveHelper mapping each argument
                // through new ResolveDeferredRecoveryMap(SPECULATIVE, accessedSym,
                // currentResolutionContext.step) (Resolve.java:2615-2636).
                if let Some(current_argtypes) = argtypes.clone() {
                    let step = self
                        .current_resolution_step
                        .expect("AssertionError: currentResolutionContext is null");
                    // The caller may already hold the mutable Attr borrow (the
                    // attribution pass); otherwise the installed service cell is
                    // used (Resolve.DeferredAttr is a Context service upstream).
                    let mapped = if let Some(attr) = attr {
                        attr.map_resolve_deferred_argtypes(
                            ctx,
                            syms,
                            current_argtypes,
                            Some(sym),
                            step,
                        )
                    } else {
                        let attr_rc = Rc::clone(
                            self.attr_cell
                                .borrow()
                                .as_ref()
                                .expect("AssertionError: Attr service must be installed"),
                        );
                        let mapped = attr_rc.borrow_mut().map_resolve_deferred_argtypes(
                            ctx,
                            syms,
                            current_argtypes,
                            Some(sym),
                            step,
                        );
                        mapped
                    };
                    argtypes = Some(mapped);
                }
            }
            if self.resolve_diagnostic_needed(
                ctx,
                log_resolve_helper,
                site,
                argtypes.clone(),
                typeargtypes.clone(),
            ) {
                self.log_resolve_error(
                    ctx,
                    syms,
                    err_sym,
                    pos,
                    location,
                    site,
                    name,
                    argtypes,
                    typeargtypes,
                );
            }
            return sym;
        }
        sym
    }

    /// Java: private void logResolveError(ResolveError error, ...)
    /// (Resolve.java:3866-3880)
    #[allow(clippy::too_many_arguments)]
    fn log_resolve_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        error: SymbolId,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: &crate::util::name::Name,
        argtypes: Option<crate::util::list::List<TypeId>>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) {
        let diagnostic = self.resolve_error_diagnostic(
            ctx,
            syms,
            error,
            crate::util::jc_diagnostic::DiagnosticType::ERROR,
            pos,
            location,
            site,
            Some(name),
            argtypes,
            typeargtypes,
        );
        if let Some(mut diagnostic) = diagnostic {
            diagnostic.set_flag(crate::util::jc_diagnostic::DiagnosticFlag::RESOLVE_ERROR);
            // Java: log.report(d)
            let log = std::rc::Rc::clone(&self.log);
            log.borrow_mut().report(diagnostic);
        }
    }

    /// Java: Symbol accessMethod(sym, pos, location, site, name, qualified,
    /// argtypes, typeargtypes) (Resolve.java:2559-2583)
    #[allow(clippy::too_many_arguments)]
    pub fn access_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: &crate::util::name::Name,
        qualified: bool,
        argtypes: Option<crate::util::list::List<TypeId>>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> SymbolId {
        self.access_internal(
            ctx,
            syms,
            sym,
            pos,
            location,
            site,
            name,
            qualified,
            argtypes,
            typeargtypes,
            attr,
            LogResolveHelper::Method,
        )
    }

    /// Java: Symbol accessBase(sym, pos, location, site, name, qualified)
    /// (Resolve.java:2586-2602)
    #[allow(clippy::too_many_arguments)]
    pub fn access_base(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: &crate::util::name::Name,
        qualified: bool,
    ) -> SymbolId {
        self.access_internal(
            ctx,
            syms,
            sym,
            pos,
            location,
            site,
            name,
            qualified,
            None,
            None,
            None,
            LogResolveHelper::Basic,
        )
    }
}


// ---------------------------------------------------------------------------
// Diagnostic argument helpers (the ADR 0002 P4 display placeholders) and the
// module access-reason queries used by the ResolveError diagnostics.
// ---------------------------------------------------------------------------

/// Java: SymbolNotFoundError.getErrorKey (Resolve.java:4079-4095)
fn symbol_not_found_error_key(
    kindname: crate::code::kinds::KindName,
    has_type_args: bool,
    has_location: bool,
) -> String {
    let key = "cant.resolve";
    let mut suffix = String::new();
    if has_location {
        suffix.push_str(".location");
    }
    if matches!(
        kindname,
        crate::code::kinds::KindName::METHOD | crate::code::kinds::KindName::CONSTRUCTOR
    ) {
        suffix.push_str(".args");
        if has_type_args {
            suffix.push_str(".params");
        }
    }
    format!("{key}{suffix}")
}

/// Java: SymbolNotFoundError.args(List<Type>) (Resolve.java:4072-4074)
fn type_args_place(
    ctx: &crate::compiler_context::CompilerContext,
    typeargs: Option<crate::util::list::List<TypeId>>,
) -> String {
    let Some(typeargs) = typeargs else {
        return String::new();
    };
    if typeargs.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    let mut current = typeargs;
    while current.non_empty() {
        let t = *current.head();
        parts.push(type_place_text(ctx, Some(t)));
        current = current.tail();
    }
    parts.join(",")
}

/// Java: Resolve.methodArguments(List<Type>) (Resolve.java:3884-3900): the
/// no-argument case renders compiler.misc.no.args; argument types use the P4
/// display placeholders.
fn method_arguments_place(
    ctx: &crate::compiler_context::CompilerContext,
    argtypes: Option<crate::util::list::List<TypeId>>,
) -> String {
    let Some(argtypes) = argtypes else {
        return "compiler.misc.no.args".to_string();
    };
    if argtypes.is_empty() {
        return "compiler.misc.no.args".to_string();
    }
    let mut parts = Vec::new();
    let mut current = argtypes;
    while current.non_empty() {
        let t = *current.head();
        parts.push(type_place_text(ctx, Some(t)));
        current = current.tail();
    }
    parts.join(",")
}

/// Java: KindName display (the compiler.misc.kindname.* resource key).
fn kind_name_arg(
    ctx: &crate::compiler_context::CompilerContext,
    symbol: SymbolId,
) -> crate::resources::diagnostic_contract::KindNameArg {
    crate::resources::diagnostic_contract::KindNameArg::placeholder(
        crate::code::kinds::kind_name(ctx.sym(symbol), ctx).name(),
    )
}

/// Display placeholder for a Type argument (P4). The ported Type.toString is
/// not available yet; the placeholder uses the type symbol display, which is
/// the rendered text for the common declared-type case (registered Porting rules).
fn type_place_text(
    ctx: &crate::compiler_context::CompilerContext,
    ty: Option<TypeId>,
) -> String {
    let Some(ty) = ty else {
        return String::new();
    };
    if let Some(tsym) = ctx.typ(ty).tsym {
        return symbol_display_name(ctx, tsym);
    }
    format!("{:?}", ctx.typ(ty).get_tag())
}

fn type_place(
    ctx: &crate::compiler_context::CompilerContext,
    ty: Option<TypeId>,
) -> crate::resources::diagnostic_contract::TypeArg {
    crate::resources::diagnostic_contract::TypeArg::placeholder(type_place_text(ctx, ty))
}

/// Display placeholder for a Symbol argument (P4).
fn symbol_place(
    ctx: &crate::compiler_context::CompilerContext,
    symbol: SymbolId,
) -> crate::resources::diagnostic_contract::SymbolArg {
    crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_display_name(ctx, symbol))
}

fn symbol_place_opt(
    ctx: &crate::compiler_context::CompilerContext,
    symbol: Option<SymbolId>,
) -> crate::resources::diagnostic_contract::SymbolArg {
    crate::resources::diagnostic_contract::SymbolArg::placeholder(
        symbol
            .map(|symbol| symbol_display_name(ctx, symbol))
            .unwrap_or_default(),
    )
}

/// Display placeholder for a diagnostic fragment argument (P4).
fn reason_place(
    reason: &crate::resources::diagnostic_contract::FragmentInfo,
) -> crate::resources::diagnostic_contract::DiagnosticArg {
    crate::resources::diagnostic_contract::DiagnosticArg::from_display(reason.info().key())
}

/// Java: Flags.asFlagSet(sym.flags() & (PRIVATE | PROTECTED))
fn modifier_args(
    access_flags: i64,
) -> Vec<crate::resources::diagnostic_contract::ModifierArg> {
    let mut result = Vec::new();
    if (access_flags & flags::PRIVATE as i64) != 0 {
        result.push(
            crate::resources::diagnostic_contract::ModifierArg::placeholder("private"),
        );
    }
    if (access_flags & flags::PROTECTED as i64) != 0 {
        result.push(
            crate::resources::diagnostic_contract::ModifierArg::placeholder("protected"),
        );
    }
    result
}

/// Java: ModuleSymbol.readModules
fn module_read_modules(
    ctx: &crate::compiler_context::CompilerContext,
    module: SymbolId,
) -> Vec<SymbolId> {
    match &ctx.sym(module).kind_data {
        crate::code::symbol::SymbolKind::ModuleSymbol { read_modules, .. } => {
            read_modules.clone()
        }
        _ => Vec::new(),
    }
}

/// Java: ModuleSymbol.exports contains the package (the ExportsDirective list).
fn module_exports_package(
    ctx: &crate::compiler_context::CompilerContext,
    module: SymbolId,
    package: SymbolId,
) -> bool {
    let exports = match &ctx.sym(module).kind_data {
        crate::code::symbol::SymbolKind::ModuleSymbol { exports, .. } => exports.clone(),
        _ => return false,
    };
    let mut current = exports;
    while current.non_empty() {
        let directive = *current.head();
        match ctx.directive(directive) {
            crate::code::directive::Directive::Exports {
                packge: Some(exported),
                ..
            } if *exported == package => return true,
            _ => {}
        }
        current = current.tail();
    }
    false
}

/// Java: ModuleSymbol.visiblePackages.containsKey(symPack.fullname)
fn module_visible_package(
    ctx: &crate::compiler_context::CompilerContext,
    module: SymbolId,
    fullname: &crate::util::name::Name,
) -> bool {
    match &ctx.sym(module).kind_data {
        crate::code::symbol::SymbolKind::ModuleSymbol {
            visible_packages, ..
        } => visible_packages.contains_key(fullname),
        _ => false,
    }
}

/// Java: Resolve.symbolPackageVisible(Env, Symbol) (Resolve.java:2205-2210)
fn symbol_package_visible(
    ctx: &crate::compiler_context::CompilerContext,
    env_snapshot: &crate::code::symbol::ResolveEnvSnapshot,
    sym: SymbolId,
) -> bool {
    let Some(env_module) = env_snapshot.toplevel_module else {
        return false;
    };
    let Some(package) = crate::code::symbol::symbol_packge(ctx, sym) else {
        return false;
    };
    let package_module = crate::code::symbol::package_module(ctx, package);
    env_module == package_module.unwrap_or(env_module)
        || crate::code::symbol::package_fullname(ctx, package)
            .map(|fullname| module_visible_package(ctx, env_module, &fullname))
            .unwrap_or(false)
}

/// The module source location of a module symbol (Java: ms.sourceLocation).
fn resolve_module_source_location(
    ctx: &crate::compiler_context::CompilerContext,
    msym: SymbolId,
) -> Option<Rc<dyn javac_tools::tools::java_file_manager::Location>> {
    match &ctx.sym(msym).kind_data {
        crate::code::symbol::SymbolKind::ModuleSymbol {
            source_location, ..
        } => source_location.clone(),
        _ => None,
    }
}

/// The module class location of a module symbol (Java: ms.classLocation).
fn resolve_module_class_location(
    ctx: &crate::compiler_context::CompilerContext,
    msym: SymbolId,
) -> Option<Rc<dyn javac_tools::tools::java_file_manager::Location>> {
    match &ctx.sym(msym).kind_data {
        crate::code::symbol::SymbolKind::ModuleSymbol { class_location, .. } => {
            class_location.clone()
        }
        _ => None,
    }
}

/// Java: enum Resolve.MethodResolutionPhase
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MethodResolutionPhase {
    #[default]
    BASIC,
    BOX,
    VARARITY,
}

impl MethodResolutionPhase {
    /// Java: public boolean isBoxingRequired()
    pub fn is_boxing_required(self) -> bool {
        matches!(
            self,
            MethodResolutionPhase::BOX | MethodResolutionPhase::VARARITY
        )
    }

    /// Java: public boolean isVarargsRequired()
    pub fn is_varargs_required(self) -> bool {
        matches!(self, MethodResolutionPhase::VARARITY)
    }

    /// Java: enum ordinal (BASIC < BOX < VARARITY).
    pub fn ordinal(self) -> usize {
        match self {
            MethodResolutionPhase::BASIC => 0,
            MethodResolutionPhase::BOX => 1,
            MethodResolutionPhase::VARARITY => 2,
        }
    }

    /// Java: MethodResolutionPhase.values()
    pub fn values() -> Vec<MethodResolutionPhase> {
        vec![
            MethodResolutionPhase::BASIC,
            MethodResolutionPhase::BOX,
            MethodResolutionPhase::VARARITY,
        ]
    }

    /// Java: boolean shouldStop(Symbol sym, MethodResolutionPhase phase) is on
    /// LookupHelper; phase comparison uses the ordinal.
    pub fn is_after(self, other: MethodResolutionPhase) -> bool {
        self.ordinal() > other.ordinal()
    }

    /// Java: public Symbol mergeResults(Symbol prev, Symbol sym)
    /// (Resolve.java:4867-4925; VARARITY overrides the base behavior)
    pub fn merge_results(
        self,
        ctx: &crate::compiler_context::CompilerContext,
        prev: SymbolId,
        sym: SymbolId,
    ) -> SymbolId {
        if self != MethodResolutionPhase::VARARITY {
            return sym;
        }
        let prev_kind = ctx.sym(prev).kind;
        let sym_kind = ctx.sym(sym).kind;
        // Java: Assert.check(bestSoFar.kind.isResolutionError() &&
        // bestSoFar.kind != AMBIGUOUS) (Resolve.java:4872-4874)
        assert!(
            prev_kind.is_resolution_error() && prev_kind != Kind::AMBIGUOUS,
            "AssertionError: mergeResults invariant"
        );
        if !sym_kind.is_resolution_error() {
            // varargs resolution successful
            return sym;
        }
        match prev_kind {
            Kind::WRONG_MTH | Kind::WRONG_MTHS => match sym_kind {
                Kind::WRONG_MTH => {
                    if prev_kind == Kind::WRONG_MTHS {
                        prev
                    } else {
                        sym
                    }
                }
                Kind::ABSENT_MTH => prev,
                _ => sym,
            },
            _ => prev,
        }
    }

}

/// Java: final List<MethodResolutionPhase> methodResolutionSteps
/// (Resolve.java:4927)
pub fn method_resolution_steps() -> crate::util::list::List<MethodResolutionPhase> {
    crate::util::list::List::of3(
        MethodResolutionPhase::BASIC,
        MethodResolutionPhase::BOX,
        MethodResolutionPhase::VARARITY,
    )
}

/// Java: Resolve.MethodResolutionContext.Candidate (Resolve.java:4978-5002)
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Java: final MethodResolutionPhase step
    pub step: MethodResolutionPhase,
    /// Java: final Symbol sym
    pub sym: SymbolId,
    /// Java: final JCDiagnostic details
    pub details: Option<crate::util::jc_diagnostic::JCDiagnostic>,
    /// Java: final Type mtype
    pub mtype: Option<TypeId>,
}

impl Candidate {
    /// Java: boolean isApplicable()
    pub fn is_applicable(&self) -> bool {
        self.mtype.is_some()
    }
}

/// Java: Resolve.MethodResolutionContext (Resolve.java:4938-5006)
pub struct MethodResolutionContext {
    /// Java: private List<Candidate> candidates
    pub(crate) candidates: Vec<Candidate>,
    /// Java: MethodResolutionPhase step (null -> None)
    pub step: Option<MethodResolutionPhase>,
    /// Java: private boolean internalResolution
    pub(crate) internal_resolution: bool,
    /// Java: private DeferredAttr.AttrMode attrMode (SPECULATIVE by default)
    pub(crate) attr_mode: crate::comp::deferred_attr::AttrMode,
    /// Java: MethodCheck methodCheck (resolveMethodCheck by default; the
    /// explicit-context adapter sets it from Resolve.defaultMethodCheck)
    pub(crate) method_check: Option<Rc<RefCell<dyn MethodCheck>>>,
}

impl Default for MethodResolutionContext {
    fn default() -> MethodResolutionContext {
        MethodResolutionContext::new()
    }
}

impl MethodResolutionContext {
/// /// Java: MethodResolutionContext() (Resolve.java:4920-4940).
///
/// # Panics
///   - none.
///
/// Upstream: Resolve.java:4920-4940.
///
    pub fn new() -> MethodResolutionContext {
        MethodResolutionContext {
            candidates: Vec::new(),
            step: None,
            internal_resolution: false,
            attr_mode: crate::comp::deferred_attr::AttrMode::SPECULATIVE,
            method_check: None,
        }
    }

    /// Java: void addInapplicableCandidate(Symbol sym, JCDiagnostic details)
    pub fn add_inapplicable_candidate(
        &mut self,
        sym: SymbolId,
        details: Option<crate::util::jc_diagnostic::JCDiagnostic>,
    ) {
        let step = self.step.expect("resolution step");
        self.candidates.push(Candidate {
            step,
            sym,
            details,
            mtype: None,
        });
    }

    /// Java: void addApplicableCandidate(Symbol sym, Type mtype)
    pub fn add_applicable_candidate(&mut self, sym: SymbolId, mtype: TypeId) {
        let step = self.step.expect("resolution step");
        self.candidates.push(Candidate {
            step,
            sym,
            details: None,
            mtype: Some(mtype),
        });
    }

    /// Java: DeferredAttrContext deferredAttrContext(Symbol sym,
    /// InferenceContext inferenceContext, ResultInfo pendingResult,
    /// Warner warn)
    pub fn deferred_attr_context(
        &self,
        sym: SymbolId,
        inference_context: std::rc::Rc<crate::comp::infer::InferenceContext>,
        pending_result: Option<&crate::comp::attr::ResultInfo>,
        warn: Option<crate::util::warner::Warner>,
    ) -> crate::comp::deferred_attr::DeferredAttrContext {
        let parent: Option<std::rc::Rc<crate::comp::deferred_attr::DeferredAttrContext>> =
            match pending_result {
                None => None,
                Some(pending_result) => {
                    Some(std::rc::Rc::clone(pending_result.check_context.deferred_attr_context()))
                }
            };
        let phase = self.step.expect("resolution step");
        crate::comp::deferred_attr::DeferredAttrContext::new(
            self.attr_mode,
            Some(sym),
            phase,
            inference_context,
            parent,
            warn,
        )
    }

    /// Java: DeferredAttr.AttrMode attrMode()
    pub fn attr_mode(&self) -> crate::comp::deferred_attr::AttrMode {
        self.attr_mode
    }

    /// Java: boolean internal()
    pub fn internal(&self) -> bool {
        self.internal_resolution
    }

    /// The inapplicable candidates, in insertion order (upstream iterates
    /// resolveContext.candidates and skips the applicable ones).
    pub fn inapplicable_candidates(&self) -> Vec<Candidate> {
        self.candidates
            .iter()
            .filter(|candidate| !candidate.is_applicable())
            .cloned()
            .collect()
    }

    /// Java: InapplicableSymbolError.errCandidate(): the last inapplicable
    /// candidate of the resolution context.
    pub fn err_candidate(&self) -> Option<Candidate> {
        self.inapplicable_candidates().into_iter().last()
    }
}

thread_local! {
    /// Diagnostics for the inapplicable-method panics currently unwinding on
    /// this thread (the Java exception payload is not Send, so the diagnostic
    /// travels in a thread-local stack and the panic payload is a marker;
    /// registered adapter for the throw/catch control flow).
    static INAPPLICABLE_DIAGNOSTICS: std::cell::RefCell<Vec<crate::util::jc_diagnostic::JCDiagnostic>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Inference exceptions currently unwinding on this thread (same adapter).
    static INFERENCE_EXCEPTIONS: std::cell::RefCell<Vec<crate::comp::infer::InferenceException>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The panic payload of a Java InapplicableMethodException throw (Resolve.java).
#[derive(Debug)]
pub struct InapplicableMethodExceptionPayload;

/// The panic payload of a Java InferenceException throw (Infer.java).
#[derive(Debug)]
pub struct InferenceExceptionPayload;

/// Java: throw new InapplicableMethodException(details) (Resolve.java:1040-1042).
pub fn throw_inapplicable_method_exception(details: &crate::util::jc_diagnostic::JCDiagnostic) -> ! {
    INAPPLICABLE_DIAGNOSTICS.with(|stack| stack.borrow_mut().push(details.clone()));
    std::panic::panic_any(InapplicableMethodExceptionPayload);
}

/// Java: throw new InferenceException(...) (Infer.java): carries the exception
/// through the thread-local stack for the inference failure throw.
pub fn throw_inference_exception(exception: crate::comp::infer::InferenceException) -> ! {
    INFERENCE_EXCEPTIONS.with(|stack| stack.borrow_mut().push(exception));
    std::panic::panic_any(InferenceExceptionPayload);
}

/// Java catch (InapplicableMethodException ex): runs `body` and converts the
/// inapplicable-method panic into the ported error value; other panics are
/// resumed unchanged.
pub fn catch_inapplicable_method_exception<R>(
    body: impl FnOnce() -> R,
) -> Result<R, MethodCheckFailure> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(value) => Ok(value),
        Err(payload) => {
            if payload.is::<InapplicableMethodExceptionPayload>() {
                let diagnostic = INAPPLICABLE_DIAGNOSTICS
                    .with(|stack| stack.borrow_mut().pop())
                    .expect("inapplicable diagnostic payload");
                Err(MethodCheckFailure::Inapplicable(
                    InapplicableMethodException::new(diagnostic),
                ))
            } else {
                std::panic::resume_unwind(payload)
            }
        }
    }
}

/// Java catch (InferenceException ex): runs `body` and converts the inference
/// panic into the ported error value; other panics are resumed unchanged.
pub fn catch_inference_exception<R>(
    body: impl FnOnce() -> R,
) -> Result<R, crate::comp::infer::InferenceException> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(value) => Ok(value),
        Err(payload) => {
            if payload.is::<InferenceExceptionPayload>() {
                let exception = INFERENCE_EXCEPTIONS
                    .with(|stack| stack.borrow_mut().pop())
                    .expect("inference exception payload");
                Err(exception)
            } else {
                std::panic::resume_unwind(payload)
            }
        }
    }
}

/// Java: public static class InapplicableMethodException extends
/// RuntimeException (Resolve.java:1398-1411)
#[derive(Clone, Debug)]
pub struct InapplicableMethodException {
    diagnostic: crate::util::jc_diagnostic::JCDiagnostic,
}

impl InapplicableMethodException {
/// /// Java: InapplicableMethodException(JCDiagnostic) (Resolve.java:1398-1411).
///
/// # Panics
///   - none.
///
/// Upstream: Resolve.java:1398-1411.
///
    pub fn new(diagnostic: crate::util::jc_diagnostic::JCDiagnostic) -> InapplicableMethodException {
        InapplicableMethodException { diagnostic }
    }

    /// Java: public JCDiagnostic getDiagnostic()
    pub fn get_diagnostic(&self) -> &crate::util::jc_diagnostic::JCDiagnostic {
        &self.diagnostic
    }
}

/// Java: enum Resolve.MethodCheckDiag (Resolve.java:717-751)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodCheckDiag {
    ArityMismatch,
    ArgMismatch,
    VarargMismatch,
    InaccessibleVarargs,
}

impl MethodCheckDiag {
    /// Java: private final String basicKey
    pub fn basic_key(self) -> &'static str {
        match self {
            MethodCheckDiag::ArityMismatch => "arg.length.mismatch",
            MethodCheckDiag::ArgMismatch => "no.conforming.assignment.exists",
            MethodCheckDiag::VarargMismatch => "varargs.argument.mismatch",
            MethodCheckDiag::InaccessibleVarargs => "inaccessible.varargs.type",
        }
    }

    /// Java: private final String inferKey
    pub fn infer_key(self) -> &'static str {
        match self {
            MethodCheckDiag::ArityMismatch => "infer.arg.length.mismatch",
            MethodCheckDiag::ArgMismatch => "infer.no.conforming.assignment.exists",
            MethodCheckDiag::VarargMismatch => "infer.varargs.argument.mismatch",
            MethodCheckDiag::InaccessibleVarargs => "inaccessible.varargs.type",
        }
    }

    /// Java: String regex()
    pub fn regex(self) -> String {
        format!("compiler\\.(?:err|misc)\\.{}.*", self.basic_key())
    }
}

/// Java: interface Resolve.MethodCheck (Resolve.java:694-711).
///
/// PORTING_RULES: the upstream methods throw InapplicableMethodException (and
/// InferenceException from Infer); the port models both with MethodCheckFailure
/// (Porting rules adapter registered).
pub trait MethodCheck {
    /// Java: void argumentsAcceptable(Env, DeferredAttrContext, List<Type>,
    /// List<Type>, Warner)
    #[allow(clippy::too_many_arguments)]
    fn arguments_acceptable(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        env: &Rc<Env<AttrContext>>,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        argtypes: crate::util::list::List<TypeId>,
        formals: crate::util::list::List<TypeId>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure>;

    /// Java: MethodCheck mostSpecificCheck(List<Type> actuals)
    fn most_specific_check(&self, actuals: crate::util::list::List<TypeId>) -> Rc<dyn MethodCheck>;
}


/// Java: class Resolve.NoMethodCheck (nilMethodCheck, Resolve.java:755-765)
pub struct NilMethodCheck;

impl MethodCheck for NilMethodCheck {
    fn arguments_acceptable(
        &mut self,
        _ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &mut crate::code::symtab::Symtab,
        _types: &crate::code::types::Types,
        _attr: &mut crate::comp::attr::Attr,
        _chk: &crate::comp::check::Check,
        _diags: &crate::util::jc_diagnostic::Factory,
        _env: &Rc<Env<AttrContext>>,
        _deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        _argtypes: crate::util::list::List<TypeId>,
        _formals: crate::util::list::List<TypeId>,
        _warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        // do nothing - method always applicable regardless of actuals
        Ok(())
    }

    fn most_specific_check(&self, _actuals: crate::util::list::List<TypeId>) -> Rc<dyn MethodCheck> {
        Rc::new(NilMethodCheck)
    }
}

/// Java: class Resolve.AbstractMethodCheck (Resolve.java:767-860)
///
/// PORTING_RULES: the flyweight SharedInapplicableMethodException is modelled by
/// returning MethodCheckFailure values (the callers propagate them unchanged).
pub trait AbstractMethodCheck: MethodCheck {
    /// Java: abstract void checkArg(DiagnosticPosition pos, boolean varargs,
    /// Type actual, Type formal, DeferredAttrContext, Warner)
    #[allow(clippy::too_many_arguments)]
    fn check_arg(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        pos: Option<crate::ids::TreeId>,
        varargs: bool,
        actual: TypeId,
        formal: TypeId,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure>;
}

/// Java: List.head on the empty list yields null; the ported comparison
/// "formals.head != varargsFormal" is None == None for the empty list.
fn formals_head_is_not_varargs(
    formals: &crate::util::list::List<TypeId>,
    varargs_formal: Option<TypeId>,
) -> bool {
    if formals.is_empty() {
        varargs_formal.is_some()
    } else {
        Some(*formals.head()) != varargs_formal
    }
}

/// The ported AbstractMethodCheck.argumentsAcceptable body, shared by the
/// resolveMethodCheck/MethodReferenceCheck/arityMethodCheck implementations.
#[allow(clippy::too_many_arguments)]
pub fn abstract_method_check_arguments_acceptable(
    this: &mut dyn AbstractMethodCheck,
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    attr: &mut crate::comp::attr::Attr,
    chk: &crate::comp::check::Check,
    diags: &crate::util::jc_diagnostic::Factory,
    env: &Rc<Env<AttrContext>>,
    deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
    argtypes: crate::util::list::List<TypeId>,
    formals: crate::util::list::List<TypeId>,
    warn: &mut crate::util::warner::Warner,
) -> Result<(), MethodCheckFailure> {
    let use_varargs = deferred_attr_context.phase.is_varargs_required();
    let call_tree = {
        let info = env.info.borrow();
        info.preferred_tree_for_diagnostics.or(env.tree)
    };
    let mut trees = call_tree
        .and_then(|tree| crate::tree::tree_info::args(ctx, tree))
        .unwrap_or_else(crate::util::list::List::nil);
    let varargs_formal = if use_varargs {
        formals.last()
    } else {
        None
    };
    let mut argtypes = argtypes;
    let mut formals = formals;
    // Java: if (varargsFormal == null && argtypes.size() != formals.size())
    //     reportMC(callTree, MethodCheckDiag.ARITY_MISMATCH, inferenceContext);
    if varargs_formal.is_none() {
        let mut arg_count = 0usize;
        let mut current = argtypes.clone();
        while current.non_empty() {
            arg_count += 1;
            current = current.tail();
        }
        let mut formal_count = 0usize;
        let mut current = formals.clone();
        while current.non_empty() {
            formal_count += 1;
            current = current.tail();
        }
        if arg_count != formal_count {
            return Err(MethodCheckFailure::Inapplicable(
                InapplicableMethodException::new(method_check_diag(diags, "arity mismatch")),
            ));
        }
    }
    // Java: formals.head (null for the empty list) != varargsFormal.
    while argtypes.non_empty() && formals_head_is_not_varargs(&formals, varargs_formal) {
        let pos = if trees.non_empty() {
            Some(*trees.head())
        } else {
            None
        };
        this.check_arg(
            ctx,
            syms,
            types,
            attr,
            chk,
            pos,
            false,
            *argtypes.head(),
            *formals.head(),
            Rc::clone(&deferred_attr_context),
            warn,
        )?;
        argtypes = argtypes.tail();
        formals = formals.tail();
        if trees.non_empty() {
            trees = trees.tail();
        }
    }
    if formals_head_is_not_varargs(&formals, varargs_formal) {
        return Err(MethodCheckFailure::Inapplicable(InapplicableMethodException::new(
            method_check_diag(diags, "arity mismatch"),
        )));
    }
    if use_varargs {
        let elt = types.elemtype(ctx, syms, varargs_formal.expect("varargs formal"));
        while argtypes.non_empty() {
            let pos = if trees.non_empty() {
                Some(*trees.head())
            } else {
                None
            };
            this.check_arg(
                ctx,
                syms,
                types,
                attr,
                chk,
                pos,
                true,
                *argtypes.head(),
                elt,
                Rc::clone(&deferred_attr_context),
                warn,
            )?;
            argtypes = argtypes.tail();
            if trees.non_empty() {
                trees = trees.tail();
            }
        }
    }
    let _ = warn;
    Ok(())
}

fn method_check_diag(
    diags: &crate::util::jc_diagnostic::Factory,
    text: &str,
) -> crate::util::jc_diagnostic::JCDiagnostic {
    // TODO(Check diagnostics , ADR 0002 P4): the upstream reportMC builds
    // a fragment with the MethodCheckDiag key and argument list; until the
    // diagnostic argument mapping is complete the fragment uses the
    // no.conforming.assignment.exists key with the placeholder text argument.
    let info = crate::resources::diagnostic_contract::FragmentInfo::new(
        "no.conforming.assignment.exists",
        vec![crate::resources::diagnostic_contract::DiagnosticArg::from_display(text)],
    );
    diags.fragment(info.into())
}

/// Java: class Resolve.MethodCheckContext (Resolve.java:1021-1064)
pub struct MethodCheckContext {
    /// Java: boolean strict
    pub strict: bool,
    /// Java: DeferredAttrContext deferredAttrContext
    pub deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
    /// Java: Warner rsWarner
    pub rs_warner: Rc<std::cell::RefCell<crate::util::warner::Warner>>,
    /// Java: MethodReferenceCheck's anonymous MethodCheckContext override keeps
    /// the pending inference context (Porting rules: the override is a field + branch in
    /// compatible below).
    pub pending_inference: Option<Rc<crate::comp::infer::InferenceContext>>,
}

impl crate::comp::check::CheckContext for MethodCheckContext {
    fn compatible(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        found: TypeId,
        req: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        let _ = warn;
        let mut found = found;
        let mut req = req;
        if let Some(pending) = &self.pending_inference {
            // Java: found = pendingInferenceContext.asUndetVar(found);
            found = pending.as_undet_var(ctx, syms, types, found);
            // Java: if (found.hasTag(UNDETVAR) && req.isPrimitive()) req = boxedClass(req).type;
            if ctx.typ(found).has_tag(crate::code::type_tag::TypeTag::UNDETVAR)
                && ctx.typ(req).is_primitive()
            {
                let boxed = types.boxed_class_ro(ctx, syms, req);
                req = ctx.sym(boxed).ty.expect("boxed type");
            }
        }
        let inference_context = &self.deferred_attr_context.inference_context;
        let found = inference_context.as_undet_var(ctx, syms, types, found);
        let req = inference_context.as_undet_var(ctx, syms, types, req);
        if self.strict {
            types.is_subtype_unchecked(ctx, syms, found, req)
        } else {
            types.is_convertible(ctx, syms, found, req)
        }
    }

    fn report(
        &self,
        _ctx: &mut crate::compiler_context::CompilerContext,
        _pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        // Java: throw new InapplicableMethodException(details) (Resolve.java:1040-1042).
        throw_inapplicable_method_exception(details);
    }

    fn check_warner(
        &self,
        _ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &crate::code::symtab::Symtab,
        _types: &crate::code::types::Types,
        _pos: i32,
        _found: TypeId,
        _req: TypeId,
    ) -> crate::util::warner::Warner {
        // TODO(Lint/Check): the upstream returns the shared rsWarner; the
        // ported Warner is not Clone, so a fresh Warner is returned and the
        // warning state is not shared (registered).
        let _ = &self.rs_warner;
        crate::util::warner::Warner::new(None)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        &self.deferred_attr_context.inference_context
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        &self.deferred_attr_context
    }
}

/// Java: class Resolve.MethodResultInfo (Resolve.java:1066-1120)
pub struct MethodResultInfo {
    /// Java: final Type pt
    pub pt: TypeId,
    /// Java: final CheckContext checkContext
    pub check_context: Rc<dyn crate::comp::check::CheckContext>,
}

impl MethodResultInfo {
    /// Java: Type check(DiagnosticPosition pos, Type found)
    #[allow(clippy::too_many_arguments)]
    pub fn check(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        pos: Option<crate::ids::TreeId>,
        found: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<TypeId, MethodCheckFailure> {
        let _ = chk;
        if ctx.typ(found).has_tag(crate::code::type_tag::TypeTag::DEFERRED) {
            let result_info = crate::comp::attr::ResultInfo::new(
                crate::code::kinds::KindSelector::VAL,
                Some(self.pt),
                Rc::clone(&self.check_context),
            );
            return crate::comp::deferred_attr::deferred_type_check(
                ctx, syms, types, attr, found, &result_info,
            )
            .map_err(|failure| match failure {
                crate::comp::deferred_attr::DeferredAttrFailure::Inference(ex) => {
                    MethodCheckFailure::Inference(ex)
                }
                crate::comp::deferred_attr::DeferredAttrFailure::Completion(_) => {
                    // Java: CompletionFailure propagates out of check; the
                    // ported method-check path only reports inference failures
                    // (the CompletionFailure path is Attr, registered).
                    MethodCheckFailure::Inapplicable(InapplicableMethodException::new(
                        method_check_diag(diags, "completion failure"),
                    ))
                }
            });
        }
        let u_result = if found == self.pt {
            found
        } else {
            types.cvar_upper_bound_of(ctx, found)
        };
        let captured_type = match pos {
            None => types.capture(ctx, syms, u_result),
            Some(tree) => self
                .check_context
                .inference_context()
                .cached_capture(ctx, syms, types, tree, u_result, true),
        };
        if !self.check_context.compatible(ctx, syms, types, captured_type, self.pt, warn) {
            return Err(MethodCheckFailure::Inapplicable(
                InapplicableMethodException::new(method_check_diag(diags, "no conforming assignment")),
            ));
        }
        // Java: super.check(pos, chk.checkNonVoid(pos, capturedType)) — the
        // checkType compatibility re-check is performed above; checkNonVoid is
        // registered (Check).
        Ok(captured_type)
    }
}


/// The two upstream checked exceptions of the method-check path.
#[derive(Debug)]
pub enum MethodCheckFailure {
    Inapplicable(InapplicableMethodException),
    Inference(crate::comp::infer::InferenceException),
}

impl From<crate::comp::infer::InferenceException> for MethodCheckFailure {
    fn from(value: crate::comp::infer::InferenceException) -> MethodCheckFailure {
        MethodCheckFailure::Inference(value)
    }
}

impl From<InapplicableMethodException> for MethodCheckFailure {
    fn from(value: InapplicableMethodException) -> MethodCheckFailure {
        MethodCheckFailure::Inapplicable(value)
    }
}




// ---------------------------------------------------------------------------
// Resolve B core: rawInstantiate / checkMethod / instantiate
// (Resolve.java:568-733) — the method applicability entry points.
// ---------------------------------------------------------------------------

/// Java: class Resolve.MethodReferenceCheck (Resolve.java:1000-1070).
///
/// PORTING_RULES: the reference-specific check context (pending inference
/// variables in argtypes) depends on the member-reference resolution
/// (Resolve.java:3000-3200 + FunctionDescriptor): the port registers the class
/// and the argumentsAcceptable entry, which is reached only from
/// checkMethod for JCTree.Tag.REFERENCE trees.
pub struct MethodReferenceCheck {
    /// Java: final InferenceContext pendingInferenceContext
    pub pending_inference_context: Rc<crate::comp::infer::InferenceContext>,
    /// Java: JCDiagnostic.Factory diags (Resolve field; explicit Porting rules parameter
    /// holder so check_arg can build the failure diagnostic).
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
}

impl AbstractMethodCheck for MethodReferenceCheck {
    #[allow(clippy::too_many_arguments)]
    fn check_arg(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        pos: Option<crate::ids::TreeId>,
        _varargs: bool,
        actual: TypeId,
        formal: TypeId,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        // Java: MethodReferenceCheck.methodCheckResult (Resolve.java:1030-1050)
        let mut check_context = method_check_context(
            !deferred_attr_context.phase.is_boxing_required(),
            Rc::clone(&deferred_attr_context),
            crate::util::warner::Warner::new(warn.pos()),
        );
        check_context.pending_inference = Some(Rc::clone(&self.pending_inference_context));
        let mresult = MethodResultInfo {
            pt: formal,
            check_context: Rc::new(check_context),
        };
        let diags = Rc::clone(&self.diags);
        mresult
            .check(ctx, syms, types, attr, chk, &diags, pos, actual, warn)
            .map(|_| ())
    }
}

impl MethodCheck for MethodReferenceCheck {
    #[allow(clippy::too_many_arguments)]
    fn arguments_acceptable(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        env: &Rc<Env<AttrContext>>,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        argtypes: crate::util::list::List<TypeId>,
        formals: crate::util::list::List<TypeId>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        abstract_method_check_arguments_acceptable(
            self,
            ctx,
            syms,
            types,
            attr,
            chk,
            diags,
            env,
            Rc::clone(&deferred_attr_context),
            argtypes,
            formals,
            warn,
        )
    }

    fn most_specific_check(&self, actuals: crate::util::list::List<TypeId>) -> Rc<dyn MethodCheck> {
        Rc::new(MostSpecificCheck { actuals })
    }
}

impl Resolve {
    /// Java: Type rawInstantiate(Env<AttrContext> env, Type site, Symbol m,
    /// ResultInfo resultInfo, List<Type> argtypes, List<Type> typeargtypes,
    /// boolean allowBoxing, boolean useVarargs, Warner warn)
    /// (Resolve.java:568-637)
    #[allow(clippy::too_many_arguments)]
    pub fn raw_instantiate(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        m: SymbolId,
        result_info: Option<&crate::comp::attr::ResultInfo>,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<TypeId, MethodCheckFailure> {
        let mut mt = types.member_type(ctx, syms, site, m);
        let mut tvars: crate::util::list::List<TypeId> = crate::util::list::List::nil();
        let typeargtypes = typeargtypes.unwrap_or_else(crate::util::list::List::nil);
        if !ctx.typ(mt).has_tag(crate::code::type_tag::TypeTag::FORALL)
            && typeargtypes.non_empty()
        {
            // This is not a polymorphic method, but typeargs are supplied
            // which is fine, see JLS 15.12.2.1
        } else if ctx.typ(mt).has_tag(crate::code::type_tag::TypeTag::FORALL)
            && typeargtypes.non_empty()
        {
            let (pmt_tvars, pmt_qtype) = match &ctx.typ(mt).kind {
                crate::code::type_::TypeKindData::ForAll { tvars, qtype } => {
                    (tvars.clone(), qtype.expect("for-all qtype"))
                }
                _ => crate::util::assert::error(),
            };
            if typeargtypes.length() != pmt_tvars.length() {
                let diag = self.diags.fragment(
                    crate::resources::compiler_properties::fragments::wrong_number_type_args(
                        {
                            let length = pmt_tvars.length().to_string();
                            crate::util::java_string::JavaString::from(length.as_str())
                        },
                    )
                    .into(),
                );
                return Err(MethodCheckFailure::Inapplicable(
                    InapplicableMethodException::new(diag),
                ));
            }
            let mut formals = pmt_tvars.clone();
            let mut actuals = typeargtypes.clone();
            while formals.non_empty() && actuals.non_empty() {
                let formal = *formals.head();
                let declared_bounds = types.get_bounds(ctx, syms, formal);
                let bounds = types.subst(ctx, syms, &declared_bounds, &pmt_tvars, &typeargtypes);
                let mut bounds = bounds;
                while bounds.non_empty() {
                    if !types.is_subtype_unchecked(ctx, syms, *actuals.head(), *bounds.head()) {
                        let diag = self.diags.fragment(
                            crate::resources::compiler_properties::fragments::explicit_param_do_not_conform_to_bounds(
                                crate::resources::diagnostic_contract::TypeArg::placeholder("type"),
                                vec![crate::resources::diagnostic_contract::TypeArg::placeholder("type")],
                            )
                            .into(),
                        );
                        return Err(MethodCheckFailure::Inapplicable(
                            InapplicableMethodException::new(diag),
                        ));
                    }
                    bounds = bounds.tail();
                }
                formals = formals.tail();
                actuals = actuals.tail();
            }
            mt = types.subst_type(ctx, syms, pmt_qtype, &pmt_tvars, &typeargtypes);
        } else if ctx.typ(mt).has_tag(crate::code::type_tag::TypeTag::FORALL) {
            let (pmt_tvars, pmt_qtype) = match &ctx.typ(mt).kind {
                crate::code::type_::TypeKindData::ForAll { tvars, qtype } => {
                    (tvars.clone(), qtype.expect("for-all qtype"))
                }
                _ => crate::util::assert::error(),
            };
            let tvars1 = types.new_instances(ctx, syms, &pmt_tvars);
            let mut appended = crate::util::list::List::nil();
            let mut current = tvars1.clone();
            while current.non_empty() {
                appended = appended.append(*current.head());
                current = current.tail();
            }
            tvars = appended;
            mt = types.subst_type(ctx, syms, pmt_qtype, &pmt_tvars, &tvars1);
        }
        // find out whether we need to go the slow route via infer
        let mut inst_needed = !tvars.is_empty();
        let mut current = argtypes.clone();
        while current.non_empty() {
            if ctx
                .typ(*current.head())
                .has_tag(crate::code::type_tag::TypeTag::FORALL)
            {
                inst_needed = true;
            }
            current = current.tail();
        }
        if inst_needed {
            return infer
                .instantiate_method(
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    env,
                    tvars,
                    mt,
                    result_info,
                    m,
                    argtypes,
                    allow_boxing,
                    use_varargs,
                    resolve_context,
                    warn,
                )
                .map_err(MethodCheckFailure::Inference);
        }
        let deferred_context = Rc::new(resolve_context.deferred_attr_context(
            m,
            crate::comp::infer::empty_context(),
            result_info,
            Some(crate::util::warner::Warner::new(warn.pos())),
        ));
        let method_check = resolve_context
            .method_check
            .clone()
            .unwrap_or_else(|| Rc::new(RefCell::new(NilMethodCheck)));
        {
            let mut check = method_check.borrow_mut();
            check.arguments_acceptable(
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
                warn,
            )?;
        }
        deferred_context
            .complete(ctx, syms, types, attr, infer)
            .map_err(|failure| match failure {
                crate::comp::deferred_attr::DeferredAttrFailure::Inference(ex) => {
                    MethodCheckFailure::Inference(ex)
                }
                crate::comp::deferred_attr::DeferredAttrFailure::Completion(_) => {
                    // TODO(Attr): the CompletionFailure channel of
                    // DeferredAttr.complete is registered with the Attr.
                    MethodCheckFailure::Inference(crate::comp::infer::InferenceException::new())
                }
            })?;
        Ok(mt)
    }

    /// Java: Type checkMethod(Env<AttrContext> env, Type site, Symbol m,
    /// ResultInfo resultInfo, List<Type> argtypes, List<Type> typeargtypes,
    /// Warner warn) (Resolve.java:640-668)
    #[allow(clippy::too_many_arguments)]
    pub fn check_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        m: SymbolId,
        result_info: &crate::comp::attr::ResultInfo,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<TypeId, MethodCheckFailure> {
        let mut resolution_context = MethodResolutionContext::new();
        resolution_context.method_check = Some(Rc::clone(&self.default_method_check));
        resolution_context.attr_mode = if result_info.pt == Some(infer.any_poly) {
            crate::comp::deferred_attr::AttrMode::SPECULATIVE
        } else {
            crate::comp::deferred_attr::AttrMode::CHECK
        };
        if let Some(tree) = env.tree {
            if ctx.tree(tree).has_tag(crate::tree::jc_tree::Tag::REFERENCE) {
                resolution_context.method_check = Some(Rc::new(RefCell::new(
                    MethodReferenceCheck {
                        pending_inference_context: Rc::clone(
                            result_info.check_context.inference_context(),
                        ),
                        diags: Rc::clone(&self.diags),
                    },
                )));
            }
        }
        let step = env
            .info
            .borrow()
            .pending_resolution_phase
            .expect("pending resolution phase");
        resolution_context.step = Some(step);
        let previous_step = self.current_resolution_step;
        self.current_resolution_step = Some(step);
        // Java: finally { currentResolutionContext = prevContext; } (Resolve.java:647-665).
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.raw_instantiate(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                site,
                m,
                Some(result_info),
                argtypes,
                typeargtypes,
                step.is_boxing_required(),
                step.is_varargs_required(),
                &mut resolution_context,
                warn,
            )
        }));
        self.current_resolution_step = previous_step;
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Java: Type instantiate(Env<AttrContext> env, Type site, Symbol m,
    /// ResultInfo resultInfo, List<Type> argtypes, List<Type> typeargtypes,
    /// boolean allowBoxing, boolean useVarargs, Warner warn)
    /// (Resolve.java:673-690): same as rawInstantiate but returns null
    /// (None) instead of throwing InapplicableMethodException.
    #[allow(clippy::too_many_arguments)]
    pub fn instantiate(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        m: SymbolId,
        result_info: Option<&crate::comp::attr::ResultInfo>,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
        warn: &mut crate::util::warner::Warner,
    ) -> Option<TypeId> {
        let raw_result = catch_inapplicable_method_exception(|| self.raw_instantiate(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            site,
            m,
            result_info,
            argtypes,
            typeargtypes,
            allow_boxing,
            use_varargs,
            resolve_context,
            warn,
        ));
        let inner_result = match raw_result {
            Ok(inner) => inner,
            Err(failure) => Err(failure),
        };
        match inner_result {
            Ok(t) => Some(t),
            Err(MethodCheckFailure::Inapplicable(_)) => None,
            Err(MethodCheckFailure::Inference(_)) => None,
        }
    }
}


/// Java: class Resolve.LookupFilter (Resolve.java:1940-1955): filters out
/// non-abstract methods unless abstractok.
pub struct LookupFilter {
    pub abstractok: bool,
}

impl LookupFilter {
/// /// Java: LookupFilter.test(Symbol) (Resolve.java:1880-1895).
///
/// # Panics
///   - none.
///
/// Upstream: Resolve.java:1880-1895.
///
    pub fn test(&self, ctx: &crate::compiler_context::CompilerContext, sym: SymbolId) -> bool {
        let flags = ctx.sym(sym).flags_field;
        (flags & crate::code::flags::ABSTRACT as i64) != 0 || self.abstractok
    }
}

impl Resolve {
    /// Java: Symbol selectBest(Env<AttrContext> env, Type site, List<Type>
    /// argtypes, List<Type> typeargtypes, Symbol sym, Symbol bestSoFar,
    /// boolean allowBoxing, boolean useVarargs)
    /// (Resolve.java:1562-1665)
    #[allow(clippy::too_many_arguments)]
    pub fn select_best(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        sym: SymbolId,
        best_so_far: SymbolId,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let mut best_so_far = best_so_far;
        let site_tsym = ctx.typ(site).tsym;
        let inherited_ok = match site_tsym {
            Some(site_tsym) => {
                ctx.sym(sym).owner == Some(site_tsym)
                    || crate::code::symbol::symbol_is_inherited_in(
                        ctx, types, syms, sym, site_tsym,
                    )
            }
            None => true,
        };
        if ctx.sym(sym).kind == crate::code::kinds::Kind::ERR
            || !inherited_ok
            || !self.not_overridden_in(ctx, syms, site, sym)
        {
            return best_so_far;
        } else if use_varargs && (ctx.sym(sym).flags_field & crate::code::flags::VARARGS) == 0 {
            if ctx.sym(best_so_far).kind.is_resolution_error() {
                let base = base_symbol_id(ctx, best_so_far);
                return self.new_bad_varargs_method(ctx, base);
            } else {
                return best_so_far;
            }
        }
        assert!(
            !ctx.sym(sym).kind.is_resolution_error(),
            "AssertionError: !sym.kind.isResolutionError()"
        );
        let mut warn = crate::util::warner::Warner::new(None);
        let raw_result = catch_inapplicable_method_exception(|| self.raw_instantiate(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            site,
            sym,
            None,
            argtypes.clone(),
            typeargtypes.clone(),
            allow_boxing,
            use_varargs,
            resolve_context,
            &mut warn,
        ));
        let inner_result = match raw_result {
            Ok(inner) => inner,
            Err(failure) => Err(failure),
        };
        match inner_result {
            Ok(mt) => resolve_context.add_applicable_candidate(sym, mt),
            Err(MethodCheckFailure::Inapplicable(ex)) => {
                resolve_context.add_inapplicable_candidate(sym, Some(ex.get_diagnostic().clone()));
                // Currently, an InapplicableMethodException occurs.
                // If bestSoFar.kind was ABSENT_MTH, return an
                // InapplicableSymbolError(kind is WRONG_MTH).
                // If bestSoFar.kind was HIDDEN(AccessError)/WRONG_MTH/WRONG_MTHS,
                // return an InapplicableSymbolsError(kind is WRONG_MTHS).
                // (Resolve.java:1595-1632)
                match ctx.sym(best_so_far).kind {
                    crate::code::kinds::Kind::ABSENT_MTH => {
                        return self.new_inapplicable_symbol_error(
                            ctx,
                            resolve_context,
                            crate::code::kinds::Kind::WRONG_MTH,
                        );
                    }
                    crate::code::kinds::Kind::HIDDEN => {
                        let prev = crate::code::symbol::symbol_resolve_error_data(ctx, best_so_far);
                        let access_sym = match prev {
                            Some(crate::code::symbol::ResolveErrorData::Access { sym, .. }) => {
                                Some(*sym)
                            }
                            _ => None,
                        };
                        if let Some(access_sym) = access_sym {
                            let previous_diagnostic = self.resolve_error_diagnostic(
                                ctx,
                                syms,
                                best_so_far,
                                crate::util::jc_diagnostic::DiagnosticType::FRAGMENT,
                                None,
                                None,
                                Some(site),
                                None,
                                Some(argtypes.clone()),
                                typeargtypes.clone(),
                            );
                            resolve_context
                                .add_inapplicable_candidate(access_sym, previous_diagnostic);
                            return self.new_inapplicable_symbols_error(
                                ctx,
                                resolve_context,
                                crate::code::kinds::Kind::WRONG_MTHS,
                            );
                        }
                        return best_so_far;
                    }
                    crate::code::kinds::Kind::WRONG_MTH => {
                        return self.new_inapplicable_symbols_error(
                            ctx,
                            resolve_context,
                            crate::code::kinds::Kind::WRONG_MTHS,
                        );
                    }
                    _ => return best_so_far,
                }
            }
Err(MethodCheckFailure::Inference(exception)) => {
                // Java: InferenceException propagates out of selectBest (Resolve.java:1580-1609
                // catches only InapplicableMethodException); the port throws the
                // inference exception through the panic channel and Infer catches it.
                throw_inference_exception(exception);
            }
        }
        if !self.is_accessible(ctx, syms, types, env, site, sym) {
            let access_error = self.new_access_error(ctx, syms, env, Some(site), sym);
            let cur_diagnostic = self.resolve_error_diagnostic(
                ctx,
                syms,
                access_error,
                crate::util::jc_diagnostic::DiagnosticType::FRAGMENT,
                None,
                None,
                Some(site),
                None,
                Some(argtypes.clone()),
                typeargtypes.clone(),
            );
            // Currently, an AccessError occurs. (Resolve.java:1638-1665)
            match ctx.sym(best_so_far).kind {
                crate::code::kinds::Kind::ABSENT_MTH => {
                    best_so_far = access_error;
                }
                crate::code::kinds::Kind::WRONG_MTH => {
                    resolve_context
                        .add_inapplicable_candidate(sym, cur_diagnostic.clone());
                    best_so_far = self.new_inapplicable_symbols_error(
                        ctx,
                        resolve_context,
                        crate::code::kinds::Kind::WRONG_MTHS,
                    );
                }
                crate::code::kinds::Kind::WRONG_MTHS => {
                    resolve_context.add_inapplicable_candidate(sym, cur_diagnostic);
                }
                crate::code::kinds::Kind::HIDDEN => {
                    let prev = crate::code::symbol::symbol_resolve_error_data(ctx, best_so_far);
                    let access_sym = match prev {
                        Some(crate::code::symbol::ResolveErrorData::Access { sym, .. }) => {
                            Some(*sym)
                        }
                        _ => None,
                    };
                    if let Some(access_sym) = access_sym {
                        let previous_diagnostic = self.resolve_error_diagnostic(
                            ctx,
                            syms,
                            best_so_far,
                            crate::util::jc_diagnostic::DiagnosticType::FRAGMENT,
                            None,
                            None,
                            Some(site),
                            None,
                            Some(argtypes.clone()),
                            typeargtypes.clone(),
                        );
                        resolve_context
                            .add_inapplicable_candidate(access_sym, previous_diagnostic);
                        resolve_context.add_inapplicable_candidate(sym, cur_diagnostic);
                        best_so_far = self.new_inapplicable_symbols_error(
                            ctx,
                            resolve_context,
                            crate::code::kinds::Kind::WRONG_MTHS,
                        );
                    }
                }
                _ => {}
            }
            return best_so_far;
        }
        if ctx.sym(best_so_far).kind.is_resolution_error()
            && ctx.sym(best_so_far).kind != crate::code::kinds::Kind::AMBIGUOUS
        {
            sym
        } else {
            let mut note_warner = crate::util::warner::Warner::new(None);
            self.most_specific(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                site,
                argtypes,
                sym,
                best_so_far,
                use_varargs,
                &mut note_warner,
            )
        }
    }

    /// Java: boolean isAccessible(Env<AttrContext> env, Type site, Symbol sym)
    /// (Resolve.java:1042-1060) — the ported subset delegates to the ported
    /// accessibility check for members (Resolve A); the full upstream body adds
    /// the protected/private owner walk registered with the Attr.
    fn is_accessible(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        sym: SymbolId,
    ) -> bool {
        let _ = types;
        self.is_accessible_member(ctx, syms, env, site, sym, true)
    }

    /// Java: Symbol mostSpecific(List<Type> argtypes, Symbol m1, Symbol m2,
    /// Env<AttrContext> env, Type site, boolean useVarargs)
    /// (Resolve.java:1668-1740): the MTH branch is complete except for
    /// Types.overrideEquivalent/Symbol.overrides (registered Symbol).
    #[allow(clippy::too_many_arguments)]
    fn most_specific(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        m1: SymbolId,
        m2: SymbolId,
        use_varargs: bool,
        note_warner: &mut crate::util::warner::Warner,
    ) -> SymbolId {
        let m2_kind = ctx.sym(m2).kind;
        match m2_kind {
            crate::code::kinds::Kind::MTH => {
                if m1 == m2 {
                    return m1;
                }
                let m1_signature_more_specific = self.signature_more_specific(
                    ctx, syms, types, attr, chk, infer, env, site, argtypes.clone(), m1, m2,
                    use_varargs, note_warner,
                );
                let m2_signature_more_specific = self.signature_more_specific(
                    ctx, syms, types, attr, chk, infer, env, site, argtypes.clone(), m2, m1,
                    use_varargs, note_warner,
                );
                if m1_signature_more_specific && m2_signature_more_specific {
                    // Java: Resolve.java:1664-1705.
                    let mt1 = types.member_type(ctx, syms, site, m1);
                    let mt2 = types.member_type(ctx, syms, site, m2);
                    if !types.override_equivalent(ctx, syms, mt1, mt2) {
                        return self.ambiguity_error(ctx, m1, m2);
                    }
                    let m1_flags = ctx.sym(m1).flags_field;
                    let m2_flags = ctx.sym(m2).flags_field;
                    let m1_bridge = (m1_flags & crate::code::flags::BRIDGE) != 0;
                    let m2_bridge = (m2_flags & crate::code::flags::BRIDGE) != 0;
                    if m1_bridge != m2_bridge {
                        return if m1_bridge { m2 } else { m1 };
                    }
                    if crate::code::symbol::base_symbol(ctx, m1)
                        == crate::code::symbol::base_symbol(ctx, m2)
                    {
                        return m1;
                    }
                    let m1_owner = ctx.sym(m1).owner.expect("method owner");
                    let m2_owner = ctx.sym(m2).owner.expect("method owner");
                    if m1_owner != m2_owner {
                        let m1_owner_ty = ctx.sym(m1_owner).ty.expect("owner type");
                        let m2_owner_ty = ctx.sym(m2_owner).ty.expect("owner type");
                        let m1_owner_interface =
                            (ctx.sym(m1_owner).flags_field
                                & crate::code::flags::INTERFACE as i64)
                                != 0;
                        let m2_owner_interface =
                            (ctx.sym(m2_owner).flags_field
                                & crate::code::flags::INTERFACE as i64)
                                != 0;
                        if types
                            .as_super(ctx, &*syms, m1_owner_ty, m2_owner)
                            .is_some()
                            && (!m1_owner_interface || m2_owner_interface)
                            && crate::code::symbol::overrides(
                                ctx, syms, types, m1, m2, m1_owner, false,
                            )
                        {
                            return m1;
                        }
                        if types
                            .as_super(ctx, &*syms, m2_owner_ty, m1_owner)
                            .is_some()
                            && (!m2_owner_interface || m1_owner_interface)
                            && crate::code::symbol::overrides(
                                ctx, syms, types, m2, m1, m2_owner, false,
                            )
                        {
                            return m2;
                        }
                    }
                    let m1_abstract =
                        (m1_flags & crate::code::flags::ABSTRACT as i64) != 0;
                    let m2_abstract =
                        (m2_flags & crate::code::flags::ABSTRACT as i64) != 0;
                    if m1_abstract && !m2_abstract {
                        return m2;
                    }
                    if m2_abstract && !m1_abstract {
                        return m1;
                    }
                    return self.ambiguity_error(ctx, m1, m2);
                }
                if m1_signature_more_specific {
                    return m1;
                }
                if m2_signature_more_specific {
                    return m2;
                }
                self.ambiguity_error(ctx, m1, m2)
            }
            crate::code::kinds::Kind::AMBIGUOUS => {
                // compare m1 to ambiguous methods in m2
                let ambiguous = crate::code::symbol::symbol_resolve_error_data(ctx, m2);
                let mut ambiguous_syms: Vec<SymbolId> = Vec::new();
                if let Some(crate::code::symbol::ResolveErrorData::Ambiguity {
                    syms: syms_list,
                }) = ambiguous
                {
                    ambiguous_syms = syms_list.clone();
                }
                let mut m1_more_specific_than_any = true;
                let mut all_more_specific_than_m1 = true;
                for s in ambiguous_syms {
                    let more_specific = self.most_specific(
                        ctx,
                        syms,
                        types,
                        attr,
                        chk,
                        infer,
                        env,
                        site,
                        argtypes.clone(),
                        m1,
                        s,
                        use_varargs,
                        note_warner,
                    );
                    m1_more_specific_than_any &= more_specific == m1;
                    all_more_specific_than_m1 &= more_specific == s;
                }
                if m1_more_specific_than_any {
                    return m1;
                }
                // Java: Resolve.java:1710-1726: AmbiguityError e = (AmbiguityError)m2.baseSymbol();
                // if (!allAmbiguousMoreSpecificThanM1) e.addAmbiguousSymbol(m1); return e;
                let error = base_symbol_id(ctx, m2);
                if !all_more_specific_than_m1 {
                    // Java: addAmbiguousSymbol prepends m1 to the ambiguity list
                    // (Resolve.java:4571-4574) and returns the error symbol.
                    if let crate::code::symbol::SymbolKind::ResolveError { data, .. } =
                        &mut ctx.sym_mut(error).kind_data
                        && let crate::code::symbol::ResolveErrorData::Ambiguity { syms } =
                            &mut **data
                        && !syms.contains(&m1)
                    {
                        syms.insert(0, m1);
                    }
                }
                error
            }
            _ => panic!("AssertionError: unexpected most-specific candidate kind"),
        }
    }

    /// Java: private boolean signatureMoreSpecific(List<Type> actuals,
    /// Env<AttrContext> env, Type site, Symbol m1, Symbol m2,
    /// boolean useVarargs) (Resolve.java:1745-1770)
    #[allow(clippy::too_many_arguments)]
    fn signature_more_specific(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        actuals: crate::util::list::List<TypeId>,
        m1: SymbolId,
        m2: SymbolId,
        use_varargs: bool,
        note_warner: &mut crate::util::warner::Warner,
    ) -> bool {
        let mut resolution_context = MethodResolutionContext::new();
        let m1_type = types.member_type(ctx, syms, site, m1);
        let m2_type = types.member_type(ctx, syms, site, m2);
        let m1_params = types.get_parameter_types_public(ctx, m1_type);
        let m2_params = types.get_parameter_types_public(ctx, m2_type);
        let max_length = m1_params.length().max(actuals.length()).max(m2_params.length());
        let adjusted = adjust_args(
            ctx,
            syms,
            types,
            m1_params,
            Some(m1),
            max_length as usize,
            use_varargs,
        );
        let _ = (&attr, &chk, &infer, &env);
        let instantiated = self.instantiate(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            site,
            m2,
            None,
            adjusted,
            None,
            false,
            use_varargs,
            &mut resolution_context,
            note_warner,
        );
        instantiated.is_some()
            && !note_warner.has_lint(crate::code::lint::LintCategory::UNCHECKED)
    }

    /// Java: Symbol ambiguityError(Symbol m1, Symbol m2)
    /// (Resolve.java:1772-1780)
    fn ambiguity_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        m1: SymbolId,
        m2: SymbolId,
    ) -> SymbolId {
        let clash_mask = crate::code::flags::CLASH;
        if ((ctx.sym(m1).flags_field | ctx.sym(m2).flags_field) & clash_mask) != 0 {
            if (ctx.sym(m1).flags_field & clash_mask) == 0 {
                m1
            } else {
                m2
            }
        } else {
            self.new_ambiguity_error(ctx, m1, m2)
        }
    }

    /// Java: Symbol findMethodInScope(Env<AttrContext> env, Type site, Name
    /// name, List<Type> argtypes, List<Type> typeargtypes, Scope sc,
    /// Symbol bestSoFar, boolean allowBoxing, boolean useVarargs,
    /// boolean abstractok) (Resolve.java:1777-1800)
    #[allow(clippy::too_many_arguments)]
    pub fn find_method_in_scope(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        scope: crate::ids::ScopeId,
        best_so_far: SymbolId,
        allow_boxing: bool,
        use_varargs: bool,
        abstractok: bool,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let filter = LookupFilter { abstractok };
        // TODO(Scope/Resolve): upstream getSymbolsByName applies the
        // LookupFilter inside the import-scope lookup; the ported filter is
        // applied after the lookup because the filter callback cannot borrow
        // the context while the scope query takes it (registered adapter; the
        // FilterImportScope static-import filtering stays in the Scope).
        let symbols = {
            let scope_ref = ctx.scope(scope).clone();
            scope_ref.get_symbols_by_name_full(
                scope,
                &name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                types,
                syms,
                ctx,
            )
        };
        let mut best_so_far = best_so_far;
        for sym in symbols {
            if !filter.test(ctx, sym) {
                continue;
            }
            best_so_far = self.select_best(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                site,
                argtypes.clone(),
                typeargtypes.clone(),
                sym,
                best_so_far,
                allow_boxing,
                use_varargs,
                resolve_context,
            );
        }
        best_so_far
    }
}




/// Java: class Resolve.ArityMethodCheck (arityMethodCheck, Resolve.java:805-818)
pub struct ArityMethodCheck;

impl AbstractMethodCheck for ArityMethodCheck {
    fn check_arg(
        &mut self,
        _ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &mut crate::code::symtab::Symtab,
        _types: &crate::code::types::Types,
        _attr: &mut crate::comp::attr::Attr,
        _chk: &crate::comp::check::Check,
        _pos: Option<crate::ids::TreeId>,
        _varargs: bool,
        _actual: TypeId,
        _formal: TypeId,
        _deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        _warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        // do nothing - actual always compatible to formals
        Ok(())
    }
}

impl MethodCheck for ArityMethodCheck {
    #[allow(clippy::too_many_arguments)]
    fn arguments_acceptable(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        env: &Rc<Env<AttrContext>>,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        argtypes: crate::util::list::List<TypeId>,
        formals: crate::util::list::List<TypeId>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        abstract_method_check_arguments_acceptable(
            self,
            ctx,
            syms,
            types,
            attr,
            chk,
            diags,
            env,
            deferred_attr_context,
            argtypes,
            formals,
            warn,
        )
    }

    fn most_specific_check(&self, _actuals: crate::util::list::List<TypeId>) -> Rc<dyn MethodCheck> {
        Rc::new(NilMethodCheck)
    }
}

/// Build the MethodCheckContext used by resolveMethodCheck / MethodReferenceCheck
/// (Resolve.java:975-1015, MethodCheckContext + anonymous subclasses).
fn method_check_context(
    strict: bool,
    deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
    rs_warner: crate::util::warner::Warner,
) -> MethodCheckContext {
    MethodCheckContext {
        strict,
        deferred_attr_context,
        rs_warner: Rc::new(RefCell::new(rs_warner)),
        pending_inference: None,
    }
}

/// Java: class Resolve.ResolveMethodCheck (resolveMethodCheck, Resolve.java:820-920)
pub struct ResolveMethodCheck {
    /// Java: JCDiagnostic.Factory diags (passed explicitly in the port)
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    /// Java: Resolve.checkVarargsAccessAfterResolution
    pub check_varargs_access_after_resolution: bool,
    /// Java: outer Resolve.allowModules.
    pub allow_modules: bool,
}

impl AbstractMethodCheck for ResolveMethodCheck {
    #[allow(clippy::too_many_arguments)]
    fn check_arg(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        pos: Option<crate::ids::TreeId>,
        _varargs: bool,
        actual: TypeId,
        formal: TypeId,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        let check_context = Rc::new(method_check_context(
            !deferred_attr_context.phase.is_boxing_required(),
            Rc::clone(&deferred_attr_context),
            crate::util::warner::Warner::new(warn.pos()),
        ));
        let mresult = MethodResultInfo {
            pt: formal,
            check_context,
        };
        let _ = self.diags.clone();
        mresult
            .check(ctx, syms, types, attr, chk, &self.diags, pos, actual, warn)
            .map(|_| ())
    }
}

impl MethodCheck for ResolveMethodCheck {
    #[allow(clippy::too_many_arguments)]
    fn arguments_acceptable(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        env: &Rc<Env<AttrContext>>,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        argtypes: crate::util::list::List<TypeId>,
        formals: crate::util::list::List<TypeId>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        let self_diags = Rc::clone(&self.diags);
        let _ = diags;
        abstract_method_check_arguments_acceptable(
            self,
            ctx,
            syms,
            types,
            attr,
            chk,
            &self_diags,
            env,
            Rc::clone(&deferred_attr_context),
            argtypes,
            formals.clone(),
            warn,
        )?;
        // should we check varargs element type accessibility?
        if deferred_attr_context.phase.is_varargs_required()
            && (deferred_attr_context.mode == crate::comp::deferred_attr::AttrMode::CHECK
                || !self.check_varargs_access_after_resolution)
        {
            let elt = types.elemtype(
                ctx,
                syms,
                formals.last().expect("varargs formal"),
            );
            varargs_accessible(
                ctx,
                syms,
                types,
                env,
                self.allow_modules,
                &deferred_attr_context.inference_context,
                elt,
                &self.diags,
            )?;
        }
        Ok(())
    }

    fn most_specific_check(&self, actuals: crate::util::list::List<TypeId>) -> Rc<dyn MethodCheck> {
        Rc::new(MostSpecificCheck { actuals })
    }
}

/// Java: ResolveMethodCheck.varargsAccessible(Env, Type, InferenceContext)
/// (Resolve.java:880-900)
fn varargs_accessible(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    env: &Rc<Env<AttrContext>>,
    allow_modules: bool,
    inference_context: &crate::comp::infer::InferenceContext,
    t: TypeId,
    diags: &Rc<crate::util::jc_diagnostic::Factory>,
) -> Result<(), MethodCheckFailure> {
    // Java: ResolveMethodCheck.varargsAccessible (Resolve.java:880-900)
    if inference_context.free(ctx, syms, types, t) {
        struct VarargsAccessibilityListener {
            env: Rc<Env<AttrContext>>,
            t: TypeId,
            diags: Rc<crate::util::jc_diagnostic::Factory>,
            allow_modules: bool,
        }
        impl crate::comp::infer::FreeTypeListener for VarargsAccessibilityListener {
            fn types_inferred(
                &self,
                ctx: &mut crate::compiler_context::CompilerContext,
                syms: &mut crate::code::symtab::Symtab,
                types: &crate::code::types::Types,
                inference_context: &crate::comp::infer::InferenceContext,
            ) -> Result<(), crate::comp::infer::InferenceException> {
                match varargs_accessible(
                    ctx,
                    syms,
                    types,
                    &self.env,
                    self.allow_modules,
                    inference_context,
                    self.t,
                    &self.diags,
                ) {
                    Ok(()) => Ok(()),
                    Err(MethodCheckFailure::Inference(exception)) => Err(exception),
                    Err(MethodCheckFailure::Inapplicable(exception)) => {
                        throw_inapplicable_method_exception(exception.get_diagnostic())
                    }
                }
            }
        }
        inference_context.add_free_type_listener(
            ctx,
            syms,
            types,
            crate::util::list::List::of(t),
            Rc::new(VarargsAccessibilityListener {
                env: Rc::clone(env),
                t,
                diags: Rc::clone(diags),
                allow_modules,
            }),
        );
        Ok(())
    } else {
        let erasure = types.erasure(ctx, syms, t);
        if is_accessible_type_free(types, allow_modules, ctx, syms, env, erasure, true) {
            return Ok(());
        }
        // Java: reportMC(env.tree, MethodCheckDiag.INACCESSIBLE_VARARGS,
        // inferenceContext, t, Kinds.kindName(location), location)
        let location = env
            .encl_class
            .and_then(|tree| class_decl_symbol(ctx, tree))
            .expect("enclClass symbol");
        let kind = crate::code::kinds::kind_name(ctx.sym(location), ctx);
        let kind_text = crate::util::javac_messages::JavacMessages::new(
            crate::util::javac_messages::DEFAULT_BUNDLE_NAME,
        )
        .get_localized_string(&format!("compiler.misc.{}", kind.name()), &[]);
        let fragment = crate::resources::compiler_properties::fragments::inaccessible_varargs_type(
            crate::resources::diagnostic_contract::TypeArg::placeholder(
                crate::comp::attr::type_display_of(ctx, t),
            ),
            crate::resources::diagnostic_contract::SymbolKindArg::placeholder(kind_text),
            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                crate::comp::resolve::symbol_display_name(ctx, location),
            ),
        );
        let pos = env.tree.map(|tree| ctx.tree(tree).pos).unwrap_or(crate::util::position::NOPOS);
        let info = crate::util::jc_diagnostic::Fragment::from(fragment).into_info();
        let diag = diags.create(
            crate::util::jc_diagnostic::DiagnosticType::FRAGMENT,
            None,
            Some(Rc::new(
                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
            )),
            &info.code,
            info.args,
        );
        Err(MethodCheckFailure::Inapplicable(
            InapplicableMethodException::new(diag),
        ))
    }
}

/// Java: class Resolve.MostSpecificCheck (Resolve.java:1122-1200): the
/// argumentsAcceptable body is complete; the functional-interface specific
/// branches depend on Types.findDescriptorType (FunctionDescriptor,
/// registered inside the context below).
pub struct MostSpecificCheck {
    /// Java: List<Type> actuals
    pub actuals: crate::util::list::List<TypeId>,
}

impl MethodCheck for MostSpecificCheck {
    #[allow(clippy::too_many_arguments)]
    fn arguments_acceptable(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        _env: &Rc<Env<AttrContext>>,
        deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
        formals1: crate::util::list::List<TypeId>,
        formals2: crate::util::list::List<TypeId>,
        warn: &mut crate::util::warner::Warner,
    ) -> Result<(), MethodCheckFailure> {
        let msym = deferred_attr_context.msym;
        let formals2 = adjust_args(
            ctx,
            syms,
            types,
            formals2,
            msym,
            formals1.length() as usize,
            deferred_attr_context.phase.is_varargs_required(),
        );
        let mut actuals = self.actuals.clone();
        let mut formals1 = formals1;
        let mut formals2 = formals2;
        while formals2.non_empty() {
            let check_context = Rc::new(MethodCheckContext {
                strict: true,
                deferred_attr_context: Rc::clone(&deferred_attr_context),
                rs_warner: Rc::new(RefCell::new(crate::util::warner::Warner::new(warn.pos()))),
                        pending_inference: None,
        });
            let mresult = MethodResultInfo {
                pt: *formals2.head(),
                check_context,
            };
            let actual = if actuals.non_empty() {
                Some(*actuals.head())
            } else {
                None
            };
            let _ = actual;
            mresult.check(
                ctx,
                syms,
                types,
                attr,
                chk,
                diags,
                None,
                *formals1.head(),
                warn,
            )?;
            formals1 = formals1.tail();
            formals2 = formals2.tail();
            if actuals.non_empty() {
                actuals = actuals.tail();
            }
        }
        Ok(())
    }

    fn most_specific_check(&self, _actuals: crate::util::list::List<TypeId>) -> Rc<dyn MethodCheck> {
        // Java: Assert.error("Cannot get here!")
        panic!("AssertionError: Cannot get here!");
    }
}

/// Java: List<Type> adjustArgs(List<Type> args, Symbol msym, int length,
/// boolean allowVarargs) (Resolve.java:1740-1765)
pub fn adjust_args(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    args: crate::util::list::List<TypeId>,
    msym: Option<SymbolId>,
    length: usize,
    allow_varargs: bool,
) -> crate::util::list::List<TypeId> {
    let is_varargs = msym
        .map(|msym| {
            (ctx.sym(msym).flags_field & crate::code::flags::VARARGS) != 0
        })
        .unwrap_or(false);
    if is_varargs && allow_varargs {
        let last = args.last().expect("varargs formal");
        let varargs_elem = types.elemtype(ctx, syms, last);
        // Java: args.reverse().tail.prepend(varargsElem).reverse(), then pad
        let mut items: Vec<TypeId> = Vec::new();
        let mut current = args.clone();
        while current.non_empty() {
            items.push(*current.head());
            current = current.tail();
        }
        items.reverse();
        items.pop();
        items.insert(0, varargs_elem);
        items.reverse();
        while items.len() < length {
            let last = *items.last().expect("non-empty args");
            items.push(last);
        }
        let mut result: crate::util::list::List<TypeId> = crate::util::list::List::nil();
        for item in items.into_iter().rev() {
            result = result.prepend(item);
        }
        result
    } else {
        args
    }
}


impl Resolve {
    /// Java: new InapplicableSymbolError(currentResolutionContext)
    /// (Resolve.java:4104-4183)
    fn new_inapplicable_symbol_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        resolve_context: &MethodResolutionContext,
        kind: crate::code::kinds::Kind,
    ) -> SymbolId {
        let candidate = resolve_context
            .err_candidate()
            .expect("AssertionError: bestSoFar must be non-null");
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            kind,
            self.names.empty.clone(),
            "inapplicable symbol error",
            crate::code::symbol::ResolveErrorData::Inapplicable {
                kind,
                sym: candidate.sym,
                details: candidate.details,
            },
        )
    }

    /// Java: new InapplicableSymbolsError(currentResolutionContext)
    /// (Resolve.java:4183-4300)
    fn new_inapplicable_symbols_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        resolve_context: &MethodResolutionContext,
        kind: crate::code::kinds::Kind,
    ) -> SymbolId {
        let candidates: Vec<(
            SymbolId,
            Option<crate::util::jc_diagnostic::JCDiagnostic>,
        )> = resolve_context
            .inapplicable_candidates()
            .into_iter()
            .map(|candidate| (candidate.sym, candidate.details))
            .collect();
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            kind,
            self.names.empty.clone(),
            "inapplicable symbols",
            crate::code::symbol::ResolveErrorData::InapplicableSymbols { kind, candidates },
        )
    }

    /// Java: new DiamondError(sym, currentResolutionContext)
    /// (Resolve.java:4313-4350)
    fn new_diamond_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        sym: SymbolId,
    ) -> SymbolId {
        let kind = ctx.sym(sym).kind;
        let details = if kind == crate::code::kinds::Kind::WRONG_MTH {
            match crate::code::symbol::symbol_resolve_error_data(ctx, sym) {
                Some(crate::code::symbol::ResolveErrorData::Inapplicable { details, .. }) => {
                    details.clone()
                }
                _ => None,
            }
        } else {
            None
        };
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            kind,
            self.names.empty.clone(),
            "diamondError",
            crate::code::symbol::ResolveErrorData::Diamond { kind, sym, details },
        )
    }

    /// Java: new BadVarargsMethod((ResolveError)bestSoFar.baseSymbol())
    /// (Resolve.java:4617-4650)
    fn new_bad_varargs_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        delegated: SymbolId,
    ) -> SymbolId {
        let kind = ctx.sym(delegated).kind;
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            kind,
            self.names.empty.clone(),
            "badVarargs",
            crate::code::symbol::ResolveErrorData::BadVarargs { delegated },
        )
    }
}

/// Java: Symbol.baseSymbol() (the delegated symbol chain root).
fn base_symbol_id(ctx: &crate::compiler_context::CompilerContext, sym: SymbolId) -> SymbolId {
    let mut current = sym;
    loop {
        match &ctx.sym(current).kind_data {
            crate::code::symbol::SymbolKind::DelegatedSymbol { other: Some(other) } => {
                current = *other
            }
            _ => return current,
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic display helpers (ADR 0002 P4: the argument values are display
// placeholders until the typed JCDiagnostic argument mapping lands).
// ---------------------------------------------------------------------------

fn type_display_str(ctx: &crate::compiler_context::CompilerContext, ty: TypeId) -> String {
    match &ctx.typ(ty).kind {
        crate::code::type_::TypeKindData::ClassType { .. }
        | crate::code::type_::TypeKindData::ErasedClassType { .. } => ctx
            .typ(ty)
            .tsym
            .map(|tsym| symbol_display_name(ctx, tsym))
            .unwrap_or_default(),
        _ => format!("{:?}", ctx.typ(ty).get_tag()),
    }
}

fn types_parameter_types(
    ctx: &crate::compiler_context::CompilerContext,
    t: TypeId,
) -> Option<crate::util::list::List<TypeId>> {
    let mut current = t;
    loop {
        match &ctx.typ(current).kind {
            crate::code::type_::TypeKindData::MethodType { argtypes, .. } => {
                return Some(argtypes.clone())
            }
            crate::code::type_::TypeKindData::ForAll { qtype, .. } => {
                current = (*qtype)?;
            }
            _ => return None,
        }
    }
}

///  the public wrapper of method_arguments_display used by
/// Attr.checkMethod (upstream Resolve.methodArguments is package-visible).
pub(crate) fn method_arguments_display_pub(
    ctx: &crate::compiler_context::CompilerContext,
    argtypes: Option<crate::util::list::List<TypeId>>,
) -> String {
    method_arguments_display(ctx, argtypes)
}

fn method_arguments_display(
    ctx: &crate::compiler_context::CompilerContext,
    argtypes: Option<crate::util::list::List<TypeId>>,
) -> String {
    match argtypes {
        None => "(no arguments)".to_string(),
        Some(argtypes) if argtypes.is_empty() => "(no arguments)".to_string(),
        Some(argtypes) => {
            let mut parts: Vec<String> = Vec::new();
            let mut current = argtypes;
            while current.non_empty() {
                parts.push(type_display_str(ctx, *current.head()));
                current = current.tail();
            }
            format!("({})", parts.join(","))
        }
    }
}


// ---------------------------------------------------------------------------
// Resolve B: LookupHelper / BasicLookupHelper / lookupMethod / findMethod /
// findFun / superclasses (Resolve.java:1777-2100, 3390-3510, 3688-3710)
// ---------------------------------------------------------------------------

/// Java: enum Resolve.InterfaceLookupPhase (Resolve.java:1897-1920)
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceLookupPhase {
    ABSTRACT_OK,
    DEFAULT_OK,
}

impl InterfaceLookupPhase {
    /// Java: enum ordinal (ABSTRACT_OK < DEFAULT_OK).
    pub fn ordinal(self) -> usize {
        match self {
            InterfaceLookupPhase::ABSTRACT_OK => 0,
            InterfaceLookupPhase::DEFAULT_OK => 1,
        }
    }

    /// Java: InterfaceLookupPhase.values()
    pub fn values() -> Vec<InterfaceLookupPhase> {
        vec![
            InterfaceLookupPhase::ABSTRACT_OK,
            InterfaceLookupPhase::DEFAULT_OK,
        ]
    }

    /// Java: abstract InterfaceLookupPhase update(Symbol s, Resolve rs)
    fn update(self, ctx: &crate::compiler_context::CompilerContext, s: SymbolId) -> InterfaceLookupPhase {
        match self {
            InterfaceLookupPhase::ABSTRACT_OK => {
                let mask = (crate::code::flags::ABSTRACT
                    | crate::code::flags::INTERFACE
                    | crate::code::flags::ENUM) as i64;
                if (ctx.sym(s).flags_field & mask) != 0 {
                    InterfaceLookupPhase::ABSTRACT_OK
                } else {
                    InterfaceLookupPhase::DEFAULT_OK
                }
            }
            InterfaceLookupPhase::DEFAULT_OK => InterfaceLookupPhase::DEFAULT_OK,
        }
    }
}

/// Java: List<Type> LookupHelper field access (the ported trait mirrors the
/// upstream abstract class fields through accessors).
pub trait LookupHelper {
    fn name(&self) -> &crate::util::name::Name;
    fn site(&self) -> TypeId;
    fn argtypes(&self) -> &crate::util::list::List<TypeId>;
    fn typeargtypes(&self) -> &Option<crate::util::list::List<TypeId>>;
    fn max_phase(&self) -> MethodResolutionPhase;

    /// Java: final boolean shouldStop(Symbol sym, MethodResolutionPhase phase)
    fn should_stop(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        sym: SymbolId,
        phase: MethodResolutionPhase,
    ) -> bool {
        phase.is_after(self.max_phase())
            || !ctx.sym(sym).kind.is_resolution_error()
            || ctx.sym(sym).kind == crate::code::kinds::Kind::AMBIGUOUS
            || ctx.sym(sym).kind == crate::code::kinds::Kind::STATICERR
    }

    /// Java: abstract Symbol lookup(Env<AttrContext> env, MethodResolutionPhase
    /// phase)
    #[allow(clippy::too_many_arguments)]
    fn lookup(
        &mut self,
        rs: &mut Resolve,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        phase: MethodResolutionPhase,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId;

    /// Java: void debug(DiagnosticPosition pos, Symbol sym) — the upstream
    /// body reports a verbose resolution diagnostic; verbose resolution is
    /// registered with the Log/diagnostics, so the default is empty.
    fn debug(&mut self, _ctx: &crate::compiler_context::CompilerContext, _sym: SymbolId) {}

    /// Java: abstract Symbol access(Env<AttrContext> env, DiagnosticPosition
    /// pos, Symbol location, Symbol sym)
    #[allow(clippy::too_many_arguments)]
    fn access(
        &mut self,
        rs: &mut Resolve,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        sym: SymbolId,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId;
}

/// Java: abstract class Resolve.BasicLookupHelper.doLookup implementations
/// (the anonymous subclasses in resolveMethod etc.)
#[derive(Clone, Debug)]
pub enum BasicLookupFn {
    FindFun,
    FindMethod { site: TypeId },
    FindConstructor { site: TypeId },
    FindDiamond { site: TypeId },
}

/// Java: the access() overrides of the BasicLookupHelper anonymous subclasses.
#[derive(Clone, Debug)]
pub enum BasicAccessKind {
    Default,
    QualifiedMethod,
    Diamond,
}

/// Java: abstract class Resolve.BasicLookupHelper (Resolve.java:3442-3480)
pub struct BasicLookupHelper {
    pub name: crate::util::name::Name,
    pub site: TypeId,
    pub argtypes: crate::util::list::List<TypeId>,
    pub typeargtypes: Option<crate::util::list::List<TypeId>>,
    pub max_phase: MethodResolutionPhase,
    pub do_lookup: BasicLookupFn,
    pub access_kind: BasicAccessKind,
}

impl BasicLookupHelper {
    pub fn new(
        name: crate::util::name::Name,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        do_lookup: BasicLookupFn,
        access_kind: BasicAccessKind,
    ) -> BasicLookupHelper {
        BasicLookupHelper {
            name,
            site,
            argtypes,
            typeargtypes,
            max_phase: MethodResolutionPhase::VARARITY,
            do_lookup,
            access_kind,
        }
    }
}

impl LookupHelper for BasicLookupHelper {
    fn name(&self) -> &crate::util::name::Name {
        &self.name
    }

    fn site(&self) -> TypeId {
        self.site
    }

    fn argtypes(&self) -> &crate::util::list::List<TypeId> {
        &self.argtypes
    }

    fn typeargtypes(&self) -> &Option<crate::util::list::List<TypeId>> {
        &self.typeargtypes
    }

    fn max_phase(&self) -> MethodResolutionPhase {
        self.max_phase
    }

    #[allow(clippy::too_many_arguments)]
    fn lookup(
        &mut self,
        rs: &mut Resolve,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        phase: MethodResolutionPhase,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let sym = match &self.do_lookup {
            BasicLookupFn::FindFun => rs.find_fun(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                self.name.clone(),
                self.argtypes.clone(),
                self.typeargtypes.clone(),
                phase.is_boxing_required(),
                phase.is_varargs_required(),
                resolve_context,
            ),
            BasicLookupFn::FindMethod { site } => rs.find_method(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                *site,
                self.name.clone(),
                self.argtypes.clone(),
                self.typeargtypes.clone(),
                phase.is_boxing_required(),
                phase.is_varargs_required(),
                resolve_context,
            ),
            BasicLookupFn::FindConstructor { site } => rs.find_constructor(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                *site,
                self.argtypes.clone(),
                self.typeargtypes.clone(),
                phase.is_boxing_required(),
                phase.is_varargs_required(),
                resolve_context,
            ),
            BasicLookupFn::FindDiamond { site } => rs.find_diamond(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                *site,
                self.argtypes.clone(),
                self.typeargtypes.clone(),
                phase.is_boxing_required(),
                phase.is_varargs_required(),
                resolve_context,
            ),
        };
        // Java: Resolve.java:3453-3459 and 4603-4606:
        // if (sym.kind == AMBIGUOUS) { AmbiguityError a_err = (AmbiguityError)sym.baseSymbol();
        //     sym = a_err.mergeAbstracts(site); }
        let sym = if ctx.sym(sym).kind == crate::code::kinds::Kind::AMBIGUOUS {
            let error = base_symbol_id(ctx, sym);
            let ambiguous_in_order = match crate::code::symbol::symbol_resolve_error_data(ctx, error) {
                Some(crate::code::symbol::ResolveErrorData::Ambiguity { syms }) => {
                    let mut symbols = syms.clone();
                    symbols.reverse();
                    symbols
                }
                _ => panic!("ClassCastException: AmbiguityError expected"),
            };
            match types.merge_abstracts(ctx, syms, chk, &ambiguous_in_order, self.site, true) {
                Some(result) => result.symbol,
                None => error,
            }
        } else {
            sym
        };
        sym
    }

    #[allow(clippy::too_many_arguments)]
    fn access(
        &mut self,
        rs: &mut Resolve,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        sym: SymbolId,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        match &self.access_kind {
            BasicAccessKind::Default => {
                if ctx.sym(sym).kind.is_resolution_error() {
                    // if nothing is found return the 'first' error
                    rs.access_method(
                        ctx,
                        syms,
                        sym,
                        pos,
                        location,
                        Some(self.site),
                        &self.name,
                        true,
                        Some(self.argtypes.clone()),
                        self.typeargtypes.clone(),
                        Some(attr),
                    )
                } else {
                    sym
                }
            }
            BasicAccessKind::QualifiedMethod => {
                if ctx.sym(sym).kind.is_resolution_error() {
                    rs.access_method(
                        ctx,
                        syms,
                        sym,
                        pos,
                        location,
                        Some(self.site),
                        &self.name,
                        true,
                        Some(self.argtypes.clone()),
                        self.typeargtypes.clone(),
                        Some(attr),
                    )
                } else if (ctx.sym(sym).flags_field & crate::code::flags::SIGNATURE_POLYMORPHIC) != 0 {
                    // Java: env.info.pendingResolutionPhase = BASIC;
                    // findPolymorphicSignatureInstance(env, sym, argtypes)
                    env.info.borrow_mut().pending_resolution_phase =
                        Some(MethodResolutionPhase::BASIC);
                    // Java: Resolve.java:2748-2749 (the access path):
                    //   env.info.pendingResolutionPhase = BASIC;
                    //   return findPolymorphicSignatureInstance(env, sym, argtypes);
                    let diags = Rc::clone(&rs.diags);
                    rs.find_polymorphic_signature_instance(
                        ctx,
                        syms,
                        types,
                        attr,
                        chk,
                        &diags,
                        infer,
                        env,
                        sym,
                        self.argtypes.clone(),
                    )
                } else {
                    sym
                }
            }
            BasicAccessKind::Diamond => {
                if ctx.sym(sym).kind.is_resolution_error() {
                    let kind = ctx.sym(sym).kind;
                    if kind != crate::code::kinds::Kind::WRONG_MTH
                        && kind != crate::code::kinds::Kind::WRONG_MTHS
                    {
                        rs.access_method(
                            ctx,
                            syms,
                            sym,
                            pos,
                            location,
                            Some(self.site),
                            &self.name,
                            true,
                            Some(self.argtypes.clone()),
                            self.typeargtypes.clone(),
                            Some(attr),
                        )
                    } else {
                        // final JCDiagnostic details =...
                        let sym = rs.new_diamond_error(ctx, sym);
                        let sym = rs.access_method(
                            ctx,
                            syms,
                            sym,
                            pos,
                            Some(self.site).and_then(|site| ctx.typ(site).tsym),
                            Some(self.site),
                            &rs.names.init.clone(),
                            true,
                            Some(self.argtypes.clone()),
                            self.typeargtypes.clone(),
                            Some(attr),
                        );
                        env.info.borrow_mut().pending_resolution_phase = resolve_context.step;
                        sym
                    }
                } else {
                    sym
                }
            }
        }
    }
}

/// Java: Symbol.clone(Symbol newOwner) helper for the import-scope lookups
/// (Symbol.java clone; the ported Symbol is a plain struct).
fn symbol_clone_with_owner(
    ctx: &mut crate::compiler_context::CompilerContext,
    sym: SymbolId,
    new_owner: SymbolId,
) -> SymbolId {
    let original = ctx.sym(sym).clone();
    let mut cloned = crate::code::symbol::Symbol::new(
        original.kind,
        original.flags_field,
        original.name.clone(),
        original.ty,
        Some(new_owner),
        original.kind_data.clone(),
    );
    cloned.erasure_field = original.erasure_field;
    ctx.alloc_symbol(cloned)
}

/// Java: Resolve.superclasses(intype).symbolFor(Type t) (Resolve.java:1956-1975)
fn symbol_for(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    seen: &mut Vec<SymbolId>,
    t: TypeId,
) -> Option<SymbolId> {
    if !ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS)
        && !ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::TYPEVAR)
    {
        return None;
    }
    let t = skip_type_vars(ctx, syms, t);
    let tsym = ctx.typ(t).tsym?;
    if seen.contains(&tsym) {
        return None;
    }
    seen.push(tsym);
    Some(tsym)
}

/// Java: Types.skipTypeVars(Type t, boolean extendBound) for extendBound=false
/// (follows the upper bound of a type variable).
fn skip_type_vars(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    t: TypeId,
) -> TypeId {

    let mut current = t;
    loop {
        if ctx.typ(current).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
            if let Some(upper) = ctx.typ(current).get_upper_bound() {
                current = upper;
                continue;
            }
        }
        return current;
    }
}

/// Java: MethodType Type.asMethodType() (the ForAll case strips the type
/// variables; the ported type model keeps the qtype MethodType).
fn as_method_type(
    ctx: &crate::compiler_context::CompilerContext,
    t: TypeId,
) -> TypeId {
    let mut current = t;
    loop {
        match &ctx.typ(current).kind {
            crate::code::type_::TypeKindData::MethodType { .. } => return current,
            crate::code::type_::TypeKindData::ForAll { qtype, .. } => match qtype {
                Some(qtype) => current = *qtype,
                None => panic!("AssertionError: for-all without qtype"),
            },
            _ => panic!("ClassCastException: MethodType expected"),
        }
    }
}

impl Resolve {
    /// Java: Symbol findConstructor(DiagnosticPosition pos, Env<AttrContext>
    /// env, Type site, List<Type> argtypes, List<Type> typeargtypes,
    /// boolean allowBoxing, boolean useVarargs) (Resolve.java:2881-2895)
    #[allow(clippy::too_many_arguments)]
    pub fn find_constructor(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let sym = self.find_method(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            site,
            self.names.init.clone(),
            argtypes,
            typeargtypes,
            allow_boxing,
            use_varargs,
            resolve_context,
        );
        // TODO(Check): FindConstructor also runs
        // chk.checkDeprecated(pos, env.info.scope.owner, sym) and
        // chk.checkPreview(pos, env.info.scope.owner, sym).
        let _ = (chk, env);
        sym
    }

    /// Java: private Symbol findDiamond(Env<AttrContext> env, Type site,
    /// List<Type> argtypes, List<Type> typeargtypes, boolean allowBoxing,
    /// boolean useVarargs) (Resolve.java:2920-2990)
    #[allow(clippy::too_many_arguments)]
    pub fn find_diamond(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let mut best_so_far = self.method_not_found;
        let site_tsym = ctx.typ(site).tsym.expect("diamond site symbol");
        let tsym = if (ctx.sym(site_tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0 {
            ctx.typ(syms.object_type).tsym.expect("object symbol")
        } else {
            site_tsym
        };
        let members = ctx.sym(tsym).members().expect("diamond class members");
        let symbols = {
            let scope_ref = ctx.scope(members).clone();
            
            scope_ref.get_symbols_by_name(
                members,
                &self.names.init.clone(),
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                ctx,
            )
        };
        let site_type_args = types.get_type_arguments_public(
            ctx,
            ctx.sym(site_tsym).ty.expect("diamond site type"),
        );
        for sym in symbols {
            if ctx.sym(sym).kind != crate::code::kinds::Kind::MTH
                || (ctx.sym(sym).flags_field & crate::code::flags::SYNTHETIC as i64) != 0
            {
                continue;
            }
            let old_params = match &ctx.typ(ctx.sym(sym).ty.expect("constructor type")).kind {
                crate::code::type_::TypeKindData::ForAll { tvars, .. } => tvars.clone(),
                _ => crate::util::list::List::nil(),
            };
            let mut tvars = site_type_args.clone();
            let mut current = old_params;
            while current.non_empty() {
                tvars = tvars.append(*current.head());
                current = current.tail();
            }
            let sym_mt = as_method_type(ctx, ctx.sym(sym).ty.expect("constructor type"));
            let constr_mt = types.create_method_type_with_return(ctx, sym_mt, site);
            let constr_type = ctx.alloc_type(crate::code::type_::Type::new(
                crate::code::type_tag::TypeTag::FORALL,
                None,
                crate::code::type_::TypeKindData::ForAll {
                    tvars,
                    qtype: Some(constr_mt),
                },
            ));
            let new_constr = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                crate::code::kinds::Kind::MTH,
                ctx.sym(sym).flags_field,
                self.names.init.clone(),
                Some(constr_type),
                Some(ctx.typ(site).tsym.expect("diamond site symbol")),
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: crate::util::list::List::nil(),
                    captured_locals: crate::util::list::List::nil(),
                    params: crate::util::list::List::nil(),
                    default_value: None,
                },
            ));
            best_so_far = self.select_best(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                site,
                argtypes.clone(),
                typeargtypes.clone(),
                new_constr,
                best_so_far,
                allow_boxing,
                use_varargs,
                resolve_context,
            );
        }
        best_so_far
    }

    /// Java: Symbol resolveMethod(DiagnosticPosition pos, Env<AttrContext> env,
    /// Name name, List<Type> argtypes, List<Type> typeargtypes)
    /// (Resolve.java:2696-2715)
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> SymbolId {
        let encl_tree = env.encl_class.expect("resolving method outside class");
        // Java: env.enclClass.sym.type (Resolve.java:1520-1530).
        let encl_class = match &ctx.tree(encl_tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("enclosing class"),
        };
        let site = ctx.sym(encl_class).ty.expect("enclosing class type");
        let mut helper = BasicLookupHelper::new(
            name,
            site,
            argtypes,
            typeargtypes,
            BasicLookupFn::FindFun,
            BasicAccessKind::Default,
        );
        let method_check = Rc::clone(&self.default_method_check);
        self.lookup_method_with_check(
            ctx, syms, types, attr, chk, infer, env, pos, encl_class, method_check, &mut helper,
        )
    }

    /// Java: Symbol resolveQualifiedMethod(DiagnosticPosition pos,
    /// Env<AttrContext> env, Type site, Name name, List<Type> argtypes,
    /// List<Type> typeargtypes) (Resolve.java:2729-2745)
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_qualified_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        location: SymbolId,
        site: TypeId,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> SymbolId {
        let mut helper = BasicLookupHelper::new(
            name,
            site,
            argtypes,
            typeargtypes,
            BasicLookupFn::FindMethod { site },
            BasicAccessKind::QualifiedMethod,
        );
        let method_check = Rc::clone(&self.default_method_check);
        self.lookup_method_with_check(
            ctx, syms, types, attr, chk, infer, env, pos, location, method_check, &mut helper,
        )
    }

    /// Java: public MethodSymbol resolveInternalMethod(DiagnosticPosition pos,
    /// Env<AttrContext> env, Type site, Name name, List<Type> argtypes,
    /// List<Type> typeargtypes) (Resolve.java:2822-2835)
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_internal_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> SymbolId {
        let _ = (typeargtypes, types);
        let mut resolve_context = MethodResolutionContext::new();
        resolve_context.internal_resolution = true;
        resolve_context.method_check = Some(Rc::clone(&self.default_method_check));
        let location = ctx.typ(site).tsym.expect("internal method site symbol");
        let mut helper = BasicLookupHelper::new(
            name,
            site,
            argtypes,
            None,
            BasicLookupFn::FindMethod { site },
            BasicAccessKind::QualifiedMethod,
        );
        let sym = self.lookup_method(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            pos,
            location,
            &mut resolve_context,
            &mut helper,
        );
        if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH {
            sym
        } else {
            // Java: throw new FatalError(diags.fragment(Fragments.FatalErrCantLocateMeth(name)))
            panic!("FatalError: cannot locate method")
        }
    }

    /// Java: Symbol resolveConstructor(DiagnosticPosition pos, Env<AttrContext>
    /// env, Type site, List<Type> argtypes, List<Type> typeargtypes)
    /// (Resolve.java:2844-2860)
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_constructor(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> SymbolId {
        let mut resolve_context = MethodResolutionContext::new();
        resolve_context.method_check = Some(Rc::clone(&self.default_method_check));
        let location = ctx.typ(site).tsym.expect("constructor site symbol");
        let mut helper = BasicLookupHelper::new(
            self.names.init.clone(),
            site,
            argtypes,
            typeargtypes,
            BasicLookupFn::FindConstructor { site },
            BasicAccessKind::Default,
        );
        self.lookup_method(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            pos,
            location,
            &mut resolve_context,
            &mut helper,
        )
    }

    /// Java: public MethodSymbol resolveInternalConstructor(DiagnosticPosition
    /// pos, Env<AttrContext> env, Type site, List<Type> argtypes,
    /// List<Type> typeargtypes) (Resolve.java:2868-2880)
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_internal_constructor(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> SymbolId {
        let mut resolve_context = MethodResolutionContext::new();
        resolve_context.internal_resolution = true;
        resolve_context.method_check = Some(Rc::clone(&self.default_method_check));
        let location = ctx.typ(site).tsym.expect("constructor site symbol");
        let mut helper = BasicLookupHelper::new(
            self.names.init.clone(),
            site,
            argtypes,
            typeargtypes,
            BasicLookupFn::FindConstructor { site },
            BasicAccessKind::Default,
        );
        let sym = self.lookup_method(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            pos,
            location,
            &mut resolve_context,
            &mut helper,
        );
        if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH {
            sym
        } else {
            // Java: throw new FatalError(diags.fragment(Fragments.FatalErrCantLocateCtor(site)))
            panic!("FatalError: cannot locate constructor")
        }
    }

    /// Java: Symbol resolveDiamond(DiagnosticPosition pos, Env<AttrContext> env,
    /// Type site, List<Type> argtypes, List<Type> typeargtypes)
    /// (Resolve.java:2914-2932)
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_diamond(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
    ) -> SymbolId {
        let location = ctx.typ(site).tsym.expect("diamond site symbol");
        let mut helper = BasicLookupHelper::new(
            self.names.init.clone(),
            site,
            argtypes,
            typeargtypes,
            BasicLookupFn::FindDiamond { site },
            BasicAccessKind::Diamond,
        );
        let method_check = Rc::clone(&self.default_method_check);
        self.lookup_method_with_check(
            ctx, syms, types, attr, chk, infer, env, pos, location, method_check, &mut helper,
        )
    }
}

impl Resolve {
    /// Java: Symbol lookupMethod(Env<AttrContext> env, DiagnosticPosition pos,
    /// Symbol location, MethodCheck methodCheck, LookupHelper lookupHelper)
    /// (Resolve.java:3688-3692)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/types/attr/chk/infer: explicit services
///   - env/pos/site: the lookup context
///   - method_check: the applicability check
///   - helper: the lookup helper
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:4300-4330 (lookupMethod with a helper).
///
    pub fn lookup_method_with_check(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: SymbolId,
        method_check: Rc<RefCell<dyn MethodCheck>>,
        helper: &mut dyn LookupHelper,
    ) -> SymbolId {
        let mut resolve_context = MethodResolutionContext::new();
        resolve_context.method_check = Some(method_check);
        self.lookup_method(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            pos,
            location,
            &mut resolve_context,
            helper,
        )
    }

    /// Java: Symbol lookupMethod(Env<AttrContext> env, DiagnosticPosition pos,
    /// Symbol location, MethodResolutionContext resolveContext,
    /// LookupHelper lookupHelper) (Resolve.java:3694-3710)
    #[allow(clippy::too_many_arguments)]
    pub fn lookup_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: SymbolId,
        resolve_context: &mut MethodResolutionContext,
        helper: &mut dyn LookupHelper,
    ) -> SymbolId {
        // Java: MethodResolutionContext prevResolutionContext =
        // currentResolutionContext; try { currentResolutionContext =
        // resolveContext; ... } finally { currentResolutionContext =
        // prevResolutionContext; } (Resolve.java:3696-3714).
        let previous_step = self.current_resolution_step;
        self.current_resolution_step = resolve_context.step;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut best_so_far = self.method_not_found;
            let steps = method_resolution_steps();
            let mut current = steps;
            while current.non_empty() {
                let phase = *current.head();
                if helper.should_stop(ctx, best_so_far, phase) {
                    break;
                }
                let prev_phase = resolve_context.step;
                let prev_best = best_so_far;
                resolve_context.step = Some(phase);
                self.current_resolution_step = Some(phase);
                let sym = helper.lookup(
                    self,
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    infer,
                    env,
                    phase,
                    resolve_context,
                );
                helper.debug(ctx, sym);
                best_so_far = phase.merge_results(ctx, best_so_far, sym);
                env.info.borrow_mut().pending_resolution_phase =
                    if prev_best == best_so_far { prev_phase } else { Some(phase) };
                current = current.tail();
            }
            self.current_resolution_step = resolve_context.step;
            helper.access(
                self,
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                pos,
                Some(location),
                best_so_far,
                resolve_context,
            )
        }));
        self.current_resolution_step = previous_step;
        match result {
            Ok(sym) => sym,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Java: Symbol findMethod(Env<AttrContext> env, Type site, Name name,
    /// List<Type> argtypes, List<Type> typeargtypes, boolean allowBoxing,
    /// boolean useVarargs) (Resolve.java:1824-1844)
    #[allow(clippy::too_many_arguments)]
    pub fn find_method(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let best_so_far = self.method_not_found;
        let intype = ctx.typ(site).tsym.and_then(|tsym| ctx.sym(tsym).ty);
        match intype {
            Some(intype) => self.find_method_full(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                site,
                name,
                argtypes,
                typeargtypes,
                intype,
                best_so_far,
                allow_boxing,
                use_varargs,
                resolve_context,
            ),
            None => best_so_far,
        }
    }

    /// Java: private Symbol findMethod(Env, Type, Name, List<Type>,
    /// List<Type>, Type intype, Symbol bestSoFar, boolean, boolean)
    /// (Resolve.java:1844-1900)
    #[allow(clippy::too_many_arguments)]
    fn find_method_full(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        intype: TypeId,
        best_so_far: SymbolId,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let mut itypes: [crate::util::list::List<TypeId>; 2] =
            [crate::util::list::List::nil(), crate::util::list::List::nil()];
        let mut iphase: Option<InterfaceLookupPhase> = Some(InterfaceLookupPhase::ABSTRACT_OK);
        let mut seen: Vec<SymbolId> = Vec::new();
        let mut best_so_far = best_so_far;
        let mut current = symbol_for(ctx, syms, &mut seen, intype);
        while let Some(s) = current {
            if let Some(members) = ctx.sym(s).members() {
                best_so_far = self.find_method_in_scope(
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    infer,
                    env,
                    site,
                    name.clone(),
                    argtypes.clone(),
                    typeargtypes.clone(),
                    members,
                    best_so_far,
                    allow_boxing,
                    use_varargs,
                    true,
                    resolve_context,
                );
            }
            if name == self.names.init {
                return best_so_far;
            }
            iphase = iphase.map(|phase| phase.update(ctx, s));
            if let Some(phase) = iphase {
                let interfaces = types.interfaces(ctx, syms, ctx.sym(s).ty.expect("class type"));
                let mut itype_current = interfaces;
                while itype_current.non_empty() {
                    let itype = *itype_current.head();
                    let closed = types.closure(ctx, syms, itype);
                    let index = phase.ordinal();
                    itypes[index] = types.closure_union(ctx, syms, closed, itypes[index].clone());
                    itype_current = itype_current.tail();
                }
            }
            // next superclass
            let st = ctx.sym(s).ty.expect("class type");
            let supertype = types.supertype(ctx, syms, st);
            current = symbol_for(ctx, syms, &mut seen, supertype);
        }
        let concrete_kind = ctx.sym(best_so_far).kind;
        let concrete = if concrete_kind.is_valid()
            && (ctx.sym(best_so_far).flags_field & crate::code::flags::ABSTRACT as i64) == 0
        {
            best_so_far
        } else {
            self.method_not_found
        };
        for phase2 in InterfaceLookupPhase::values() {
            let mut itype_current = itypes[phase2.ordinal()].clone();
            while itype_current.non_empty() {
                let itype = *itype_current.head();
                // skip j.l.Object (included by Types.closure())
                let is_interface = ctx
                    .typ(itype)
                    .tsym
                    .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                    .unwrap_or(false);
                if is_interface {
                    let tsym = ctx.typ(itype).tsym.expect("interface symbol");
                    let default_ok = phase2 == InterfaceLookupPhase::DEFAULT_OK;
                    let has_default =
                        (ctx.sym(tsym).flags_field & crate::code::flags::DEFAULT) != 0;
                    if !(default_ok && !has_default) {
                        if let Some(members) = ctx.sym(tsym).members() {
                            best_so_far = self.find_method_in_scope(
                                ctx,
                                syms,
                                types,
                                attr,
                                chk,
                                infer,
                                env,
                                site,
                                name.clone(),
                                argtypes.clone(),
                                typeargtypes.clone(),
                                members,
                                best_so_far,
                                allow_boxing,
                                use_varargs,
                                true,
                                resolve_context,
                            );
                            if concrete != best_so_far
                                && ctx.sym(concrete).kind.is_valid()
                                && ctx.sym(best_so_far).kind.is_valid()
                                && types.is_sub_signature(
                                    ctx,
                                    syms,
                                    ctx.sym(concrete).ty.expect("concrete type"),
                                    ctx.sym(best_so_far).ty.expect("best type"),
                                )
                            {
                                // this is a hack - as javac does not do full
                                // membership checks (CR 6178365)
                                best_so_far = concrete;
                            }
                        }
                    }
                }
                itype_current = itype_current.tail();
            }
        }
        best_so_far
    }

    /// Java: Symbol findFun(Env<AttrContext> env, Name name, List<Type>
    /// argtypes, List<Type> typeargtypes, boolean allowBoxing,
    /// boolean useVarargs) (Resolve.java:1978-2100)
    #[allow(clippy::too_many_arguments)]
    pub fn find_fun(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        allow_boxing: bool,
        use_varargs: bool,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let mut best_so_far = self.method_not_found;
        let mut env1 = Rc::clone(env);
        let mut static_only = false;
        loop {
            if env1.outer.is_none() {
                break;
            }
            if self.is_static(&env1) {
                static_only = true;
            }
            let tree = env.tree;
            env1.info.borrow_mut().preferred_tree_for_diagnostics = tree;
            let encl_tree = env1.encl_class.expect("enclosing class tree");
            // Java: env1.enclClass.sym.type (Resolve.java:findFun).
            let encl_class = match &ctx.tree(encl_tree).kind {
                crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                _ => panic!("enclosing class symbol"),
            };
            let site = ctx.sym(encl_class).ty.expect("enclosing class type");
            let sym = self.find_method(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                &env1,
                site,
                name.clone(),
                argtypes.clone(),
                typeargtypes.clone(),
                allow_boxing,
                use_varargs,
                resolve_context,
            );
            env1.info.borrow_mut().preferred_tree_for_diagnostics = None;
            if crate::code::symbol::symbol_exists(ctx, sym) {
                if static_only
                    && ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
                    && ctx.sym(ctx.sym(sym).owner.expect("owner")).kind
                        == crate::code::kinds::Kind::TYP
                    && (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) == 0
                {
                    return self.new_static_error(ctx, sym);
                } else {
                    return sym;
                }
            } else {
                best_so_far = best_of(ctx, best_so_far, sym);
            }
            if (ctx.sym(encl_class).flags_field & crate::code::flags::STATIC as i64) != 0 {
                static_only = true;
            }
            let next = env1
                .outer
                .as_ref()
                .map(Rc::clone)
                .expect("outer env");
            env1 = next;
        }
        let predef_type = ctx.sym(syms.predef_class).ty.expect("predef class type");
        let sym = self.find_method(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            env,
            predef_type,
            name.clone(),
            argtypes.clone(),
            typeargtypes.clone(),
            allow_boxing,
            use_varargs,
            resolve_context,
        );
        if crate::code::symbol::symbol_exists(ctx, sym) {
            return sym;
        }
        // Java: env.toplevel.namedImportScope.getSymbolsByName(name)
        if let Some(toplevel) = env.toplevel {
            let (named_scope, star_scope) = match &ctx.tree(toplevel).kind {
                crate::tree::jc_tree::TreeKind::CompilationUnit {
                    named_import_scope,
                    star_import_scope,
                    ..
                } => (*named_import_scope, *star_import_scope),
                _ => (None, None),
            };
            if let Some(named_scope) = named_scope {
                best_so_far = self.find_fun_in_import_scope(
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    infer,
                    env,
                    named_scope,
                    &name,
                    argtypes.clone(),
                    typeargtypes.clone(),
                    allow_boxing,
                    use_varargs,
                    best_so_far,
                    resolve_context,
                );
            }
            if crate::code::symbol::symbol_exists(ctx, best_so_far) {
                return best_so_far;
            }
            if let Some(star_scope) = star_scope {
                best_so_far = self.find_fun_in_import_scope(
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    infer,
                    env,
                    star_scope,
                    &name,
                    argtypes,
                    typeargtypes,
                    allow_boxing,
                    use_varargs,
                    best_so_far,
                    resolve_context,
                );
            }
        }
        best_so_far
    }

    /// The shared body of the two import-scope loops in findFun
    /// (Resolve.java:2030-2095).
    #[allow(clippy::too_many_arguments)]
    fn find_fun_in_import_scope(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        scope: crate::ids::ScopeId,
        name: &crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        allow_boxing: bool,
        use_varargs: bool,
        mut best_so_far: SymbolId,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let symbols = {
            let scope_ref = ctx.scope(scope).clone();
            
            scope_ref.get_symbols_by_name(
                scope,
                name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                ctx,
            )
        };
        for current_sym in symbols {
            if ctx.sym(current_sym).kind != crate::code::kinds::Kind::MTH {
                continue;
            }
            let origin_scope = {
                let scope_ref = ctx.scope(scope).clone();
                scope_ref.get_origin_full(scope, current_sym, types, syms, ctx)
            };
            let origin = origin_scope
                .and_then(|origin_scope| ctx.scope(origin_scope).owner)
                .or_else(|| ctx.sym(current_sym).owner);
            let Some(origin) = origin else {
                continue;
            };
            let current_sym = {
                let owner_type = ctx.sym(ctx.sym(current_sym).owner.expect("owner")).ty;
                let origin_type = ctx.sym(origin).ty;
                if owner_type != origin_type {
                    symbol_clone_with_owner(ctx, current_sym, origin)
                } else {
                    current_sym
                }
            };
            let current_sym = {
                let origin_type = ctx.sym(origin).ty.expect("origin type");
                if !self.is_accessible(ctx, syms, types, env, origin_type, current_sym) {
                    self.new_access_error(ctx, syms, env, Some(origin_type), current_sym)
                } else {
                    current_sym
                }
            };
            best_so_far = self.select_best(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                ctx.sym(origin).ty.expect("origin type"),
                argtypes.clone(),
                typeargtypes.clone(),
                current_sym,
                best_so_far,
                allow_boxing,
                use_varargs,
                resolve_context,
            );
        }
        best_so_far
    }
}

impl Resolve {
    /// Java: WriteableScope polymorphicSignatureScope (Resolve.java:118/152)
    fn polymorphic_signature_scope_init(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
    ) -> ScopeId {
        if let Some(scope) = self.polymorphic_signature_scope {
            return scope;
        }
        let scope = ctx.scope_create(Some(syms.no_symbol));
        self.polymorphic_signature_scope = Some(scope);
        scope
    }

    /// Java: Symbol findPolymorphicSignatureInstance(Env<AttrContext> env,
    /// Symbol spMethod, List<Type> argtypes) (Resolve.java:2764-2770)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/types/attr/chk/diags/infer: explicit services
///   - env: the attribution environment
///   - sp_method/argtypes: the signature polymorphic method and actuals
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:2764-2770.
///
    pub fn find_polymorphic_signature_instance(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        sp_method: SymbolId,
        argtypes: crate::util::list::List<TypeId>,
    ) -> SymbolId {
        let mtype = infer.instantiate_polymorphic_signature_instance(
            ctx, syms, types, attr, chk, diags, env, Some(sp_method), argtypes,
        );
        self.find_polymorphic_signature_instance_with_type(ctx, syms, types, sp_method, mtype)
    }

    /// Java: Symbol findPolymorphicSignatureInstance(final Symbol spMethod,
    /// Type mtype) (Resolve.java:2772-2812)
/// # Arguments
///   - ctx/syms/types: explicit services
///   - sp_method/mtype: the method and its synthetic type
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:2772-2812.
///
    pub fn find_polymorphic_signature_instance_with_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        sp_method: SymbolId,
        mtype: TypeId,
    ) -> SymbolId {
        let scope = self.polymorphic_signature_scope_init(ctx, syms);
        let sp_name = ctx.sym(sp_method).name.clone();
        let sp_owner = ctx.sym(sp_method).owner;
        let mut existing: Option<SymbolId> = None;
        for sym in ctx.scope_get_symbols_by_name_full(
            scope,
            &sp_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            &self.types,
            syms,
        ) {
            let sym_type = ctx.sym(sym).ty;
            if let Some(sym_type) = sym_type {
                if types.is_same_type(ctx, syms, mtype, sym_type)
                    && sp_owner == ctx.sym(sym).owner
                {
                    existing = Some(sym);
                    break;
                }
            }
        }
        if let Some(sym) = existing {
            return sym;
        }
        let sp_type = ctx.sym(sp_method).ty.expect("signature polymorphic method type");
        let sp_return_type = types.get_return_type_public(ctx, sp_type);
        let mut mtype = mtype;
        if types.is_same_type(ctx, syms, sp_return_type, syms.object_type) {
            // Polymorphic return, pass through mtype
        } else {
            let m_return_type = types.get_return_type_public(ctx, mtype);
            if !types.is_same_type(ctx, syms, sp_return_type, m_return_type) {
                let params = types.get_parameter_types_public(ctx, mtype);
                let thrown = ctx.typ(mtype).get_thrown_types(ctx);
                mtype = ctx.alloc_type(crate::code::type_::Type::new_method_type(
                    params,
                    sp_return_type,
                    thrown,
                    syms.method_class,
                ));
            }
        }
        let sp_flags = ctx.sym(sp_method).flags_field;
        let flags = (crate::code::flags::ABSTRACT as i64)
            | crate::code::flags::HYPOTHETICAL
            | (sp_flags
                & ((crate::code::flags::PUBLIC
                    | crate::code::flags::PROTECTED
                    | crate::code::flags::PRIVATE
                    | crate::code::flags::STATIC) as i64));
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::MTH,
            flags,
            sp_name,
            Some(mtype),
            sp_owner,
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params: crate::util::list::List::nil(),
                captured_locals: crate::util::list::List::nil(),
                params: crate::util::list::List::nil(),
                default_value: None,
            },
        ));
        ctx.sym_mut(sym).base_symbol_override = Some(sp_method);
        if !types.is_erroneous_type(ctx, syms, mtype) {
            ctx.scope_enter(scope, sym);
        }
        sym
    }
}

// ---------------------------------------------------------------------------
// Resolve member-reference resolution family (Resolve.java:3000-3650).
// adapter adapters: the upstream anonymous ReferenceChooser subclasses are the
// ReferenceChooserKind enum + Resolve methods; the HashMap identity maps are
// Vec<(SymbolId, JCDiagnostic)>; the generated diagnostic constructors are used
// where the upstream creates fragments.
// ---------------------------------------------------------------------------

/// Java: enum Resolve.ReferenceLookupResult.StaticKind (Resolve.java:3178-3205)
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticKind {
    STATIC,
    NON_STATIC,
    BOTH,
    UNDEFINED,
}

impl StaticKind {
    /// Java: static StaticKind from(Symbol s)
    fn from(ctx: &crate::compiler_context::CompilerContext, sym: SymbolId) -> StaticKind {
        if ctx.sym(sym).flags_field & (crate::code::flags::STATIC as i64) != 0 {
            StaticKind::STATIC
        } else {
            StaticKind::NON_STATIC
        }
    }

    /// Java: static StaticKind reduce(StaticKind sk1, StaticKind sk2)
    fn reduce(sk1: StaticKind, sk2: StaticKind) -> StaticKind {
        if sk1 == StaticKind::UNDEFINED {
            sk2
        } else if sk2 == StaticKind::UNDEFINED || sk1 == sk2 {
            sk1
        } else {
            StaticKind::BOTH
        }
    }
}

/// Java: static class Resolve.ReferenceLookupResult (Resolve.java:3174-3290)
#[derive(Clone, Copy)]
pub struct ReferenceLookupResult {
    pub static_kind: StaticKind,
    pub sym: SymbolId,
}

impl ReferenceLookupResult {
    /// Java: ReferenceLookupResult(Symbol sym, MethodResolutionContext ctx)
    pub fn new(
        ctx: &crate::compiler_context::CompilerContext,
        sym: SymbolId,
        resolve_context: &MethodResolutionContext,
    ) -> ReferenceLookupResult {
        let static_kind = match ctx.sym(sym).kind {
            crate::code::kinds::Kind::MTH | crate::code::kinds::Kind::AMBIGUOUS => {
                let mut kind = StaticKind::UNDEFINED;
                for candidate in &resolve_context.candidates {
                    if candidate.is_applicable()
                        && Some(candidate.step) == resolve_context.step
                    {
                        kind = StaticKind::reduce(kind, StaticKind::from(ctx, candidate.sym));
                    }
                }
                kind
            }
            _ => StaticKind::UNDEFINED,
        };
        ReferenceLookupResult { static_kind, sym }
    }

    fn with_kind(sym: SymbolId, static_kind: StaticKind) -> ReferenceLookupResult {
        ReferenceLookupResult { static_kind, sym }
    }

    /// Java: boolean isSuccess()
    pub fn is_success(&self) -> bool {
        self.static_kind != StaticKind::UNDEFINED
    }

    /// Java: boolean hasKind(StaticKind sk)
    pub fn has_kind(&self, sk: StaticKind) -> bool {
        self.static_kind == sk
    }

    /// Java: boolean canIgnore() (Resolve.java:3236-3260)
    pub fn can_ignore(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
    ) -> bool {
        match ctx.sym(self.sym).kind {
            crate::code::kinds::Kind::ABSENT_MTH => true,
            crate::code::kinds::Kind::WRONG_MTH => {
                let base = crate::code::symbol::base_symbol(ctx, self.sym);
                let data = crate::code::symbol::symbol_resolve_error_data(ctx, base).cloned();
                match data {
                    Some(crate::code::symbol::ResolveErrorData::Inapplicable { details, .. }) => {
                        details
                            .map(|details| {
                                resolve_matches_method_check_diag(
                                    &details.get_code_string(),
                                    false,
                                )
                            })
                            .unwrap_or(false)
                    }
                    _ => false,
                }
            }
            crate::code::kinds::Kind::WRONG_MTHS => {
                let base = crate::code::symbol::base_symbol(ctx, self.sym);
                let data = crate::code::symbol::symbol_resolve_error_data(ctx, base).cloned();
                match data {
                    Some(crate::code::symbol::ResolveErrorData::InapplicableSymbols {
                        candidates,
                        ..
                    }) => {
                        let mapped = resolve_map_candidates(ctx, syms, types, &candidates);
                        resolve_filter_candidates(&mapped).is_empty()
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Java: static ReferenceLookupResult error(Symbol sym)
    pub fn error(sym: SymbolId) -> ReferenceLookupResult {
        ReferenceLookupResult::with_kind(sym, StaticKind::UNDEFINED)
    }
}

/// Java: MethodCheckDiag.ARITY_MISMATCH.regex() matching (Porting rules: the Template
/// regex is expressed as a suffix test on the diagnostic code).
fn resolve_matches_method_check_diag(code: &str, _unused: bool) -> bool {
    code.ends_with("arg.length.mismatch") || code.ends_with("infer.arg.length.mismatch")
}

/// Java: InapplicableSymbolsError.MostSpecificMap.put(Candidate) (Porting rules: Vec map).
fn resolve_map_candidates(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut Symtab,
    types: &crate::code::types::Types,
    candidates: &[(SymbolId, Option<crate::util::jc_diagnostic::JCDiagnostic>)],
) -> Vec<(SymbolId, Option<crate::util::jc_diagnostic::JCDiagnostic>)> {
    let mut result: Vec<(SymbolId, Option<crate::util::jc_diagnostic::JCDiagnostic>)> =
        Vec::new();
    for (cand_sym, cand_details) in candidates {
        let mut overridden: Vec<SymbolId> = Vec::new();
        let mut skip = false;
        for (existing, _) in &result {
            if existing == cand_sym {
                continue;
            }
            let existing_owner = ctx.sym(*existing).owner.expect("candidate owner");
            if crate::code::symbol::overrides(
                ctx, syms, types, *cand_sym, *existing, existing_owner, false,
            ) {
                overridden.push(*existing);
            } else {
                let cand_owner = ctx.sym(*cand_sym).owner.expect("candidate owner");
                if crate::code::symbol::overrides(
                    ctx, syms, types, *existing, *cand_sym, cand_owner, false,
                ) {
                    skip = true;
                    break;
                }
            }
        }
        if skip {
            continue;
        }
        result.retain(|(sym, _)| !overridden.contains(sym));
        result.push((*cand_sym, cand_details.clone()));
    }
    result
}

/// Java: InapplicableSymbolsError.filterCandidates (Porting rules).
fn resolve_filter_candidates(
    candidates: &[(SymbolId, Option<crate::util::jc_diagnostic::JCDiagnostic>)],
) -> Vec<(SymbolId, Option<crate::util::jc_diagnostic::JCDiagnostic>)> {
    let mut result: Vec<(SymbolId, Option<crate::util::jc_diagnostic::JCDiagnostic>)> =
        Vec::new();
    for (sym, details) in candidates {
        let is_arity = details
            .as_ref()
            .map(|d| resolve_matches_method_check_diag(&d.get_code_string(), false))
            .unwrap_or(false);
        if !is_arity {
            result.push((*sym, details.clone()));
        }
    }
    result
}

/// Java: Resolve.ReferenceChooser subclasses (basicReferenceChooser /
/// structuralReferenceChooser) — Porting rules enum dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceChooserKind {
    Basic,
    Structural,
}

// ---------------------------------------------------------------------------
// Resolve member-reference lookup helpers (Resolve.java:3480-3700)
// and Resolve.getMemberReference (Resolve.java:3000-3016).
// ---------------------------------------------------------------------------

/// Java: ReferenceLookupHelper subclass discriminator (Porting rules).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceHelperKind {
    Method,
    UnboundMethod,
    ArrayConstructor,
    Constructor,
}

/// Java: abstract class Resolve.ReferenceLookupHelper (Resolve.java:3485-3520)
/// and its four concrete subclasses (Porting rules: one struct + kind dispatch).
#[derive(Clone)]
pub struct ReferenceLookupHelper {
    pub reference_tree: TreeId,
    pub name: crate::util::name::Name,
    pub site: TypeId,
    pub argtypes: crate::util::list::List<TypeId>,
    pub typeargtypes: Option<crate::util::list::List<TypeId>>,
    pub max_phase: MethodResolutionPhase,
    pub kind: ReferenceHelperKind,
    /// Java: MethodReferenceLookupHelper.originalSite
    pub original_site: TypeId,
    /// Java: ConstructorReferenceLookupHelper.needsInference
    pub needs_inference: bool,
    pub method_not_found: SymbolId,
}

impl ReferenceLookupHelper {
    #[allow(clippy::too_many_arguments)]
/// /// Java: new MethodReferenceLookupHelper(...) (Resolve.java:3528-3540).
///
/// # Panics
///   - none.
///
/// Upstream: Resolve.java:3528-3540.
///
    pub fn new_method(
        reference_tree: TreeId,
        name: crate::util::name::Name,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        max_phase: MethodResolutionPhase,
    ) -> ReferenceLookupHelper {
        ReferenceLookupHelper {
            reference_tree,
            name,
            site,
            argtypes,
            typeargtypes,
            max_phase,
            kind: ReferenceHelperKind::Method,
            original_site: site,
            needs_inference: false,
            method_not_found: crate::ids::SymbolId::new(0),
        }
    }

    #[allow(clippy::too_many_arguments)]
/// /// Java: new UnboundMethodReferenceLookupHelper(...) (Resolve.java:3585-3600).
///
/// # Panics
///   - none.
///
/// Upstream: Resolve.java:3585-3600.
///
    pub fn new_unbound_method(
        reference_tree: TreeId,
        name: crate::util::name::Name,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        max_phase: MethodResolutionPhase,
        original_site: TypeId,
    ) -> ReferenceLookupHelper {
        ReferenceLookupHelper {
            kind: ReferenceHelperKind::UnboundMethod,
            original_site,
            ..ReferenceLookupHelper::new_method(
                reference_tree, name, site, argtypes, typeargtypes, max_phase,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_array_constructor(
        reference_tree: TreeId,
        name: crate::util::name::Name,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        max_phase: MethodResolutionPhase,
    ) -> ReferenceLookupHelper {
        ReferenceLookupHelper {
            kind: ReferenceHelperKind::ArrayConstructor,
            ..ReferenceLookupHelper::new_method(
                reference_tree, name, site, argtypes, typeargtypes, max_phase,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_constructor(
        reference_tree: TreeId,
        name: crate::util::name::Name,
        site: TypeId,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        max_phase: MethodResolutionPhase,
        needs_inference: bool,
    ) -> ReferenceLookupHelper {
        ReferenceLookupHelper {
            kind: ReferenceHelperKind::Constructor,
            needs_inference,
            ..ReferenceLookupHelper::new_method(
                reference_tree, name, site, argtypes, typeargtypes, max_phase,
            )
        }
    }

    /// Java: ReferenceLookupHelper unboundLookup(InferenceContext)
    pub fn unbound_lookup(
        &self,
        rs: &Resolve,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        inference_context: &Rc<crate::comp::infer::InferenceContext>,
    ) -> Option<ReferenceLookupHelper> {
        match self.kind {
            ReferenceHelperKind::Method => {
                let expr = match &ctx.tree(self.reference_tree).kind {
                    crate::tree::jc_tree::TreeKind::MemberReference { expr, .. } => *expr,
                    _ => panic!("ClassCastException: JCMemberReference expected"),
                };
                if crate::tree::tree_info::is_static_selector(ctx, expr, &rs.names) {
                    let head = if self.argtypes.is_empty() {
                        None
                    } else {
                        Some(*self.argtypes.head())
                    };
                    let has_none = head
                        .map(|t| ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::NONE))
                        .unwrap_or(false);
                    let first_subtype = head
                        .map(|t| {
                            let undet = inference_context.as_undet_var(ctx, syms, types, t);
                            types.is_subtype_unchecked(ctx, syms, undet, self.original_site)
                        })
                        .unwrap_or(false);
                    if has_none || first_subtype {
                        Some(ReferenceLookupHelper::new_unbound_method(
                            self.reference_tree,
                            self.name.clone(),
                            self.original_site,
                            self.argtypes.clone(),
                            self.typeargtypes.clone(),
                            self.max_phase,
                            self.original_site,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ReferenceHelperKind::UnboundMethod => Some(ReferenceLookupHelper::new_unbound_method(
                self.reference_tree,
                self.name.clone(),
                self.site,
                self.argtypes.clone(),
                self.typeargtypes.clone(),
                self.max_phase,
                self.original_site,
            )),
            _ => None,
        }
    }

    /// Java: JCMemberReference.ReferenceKind referenceKind(Symbol sym)
    pub fn reference_kind(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        sym: SymbolId,
    ) -> crate::tree::jc_tree::ReferenceKind {
        use crate::tree::jc_tree::ReferenceKind;
        match self.kind {
            ReferenceHelperKind::Method | ReferenceHelperKind::UnboundMethod => {
                if self.kind == ReferenceHelperKind::UnboundMethod {
                    return ReferenceKind::UNBOUND;
                }
                if ctx.sym(sym).flags_field & (crate::code::flags::STATIC as i64) != 0 {
                    ReferenceKind::STATIC
                } else {
                    let qualifier = match &ctx.tree(self.reference_tree).kind {
                        crate::tree::jc_tree::TreeKind::MemberReference { expr, .. } => *expr,
                        _ => None,
                    };
                    let sel_name = qualifier.and_then(|q| crate::tree::tree_info::name(ctx, q));
                    if sel_name
                        .as_ref()
                        .map(|n| n.to_java_string().to_string_lossy() == "_super")
                        .unwrap_or(false)
                    {
                        ReferenceKind::SUPER
                    } else {
                        ReferenceKind::BOUND
                    }
                }
            }
            ReferenceHelperKind::ArrayConstructor => ReferenceKind::ARRAY_CTOR,
            ReferenceHelperKind::Constructor => {
                let encl = ctx.typ(self.original_site).get_enclosing_type();
                let none = encl
                    .map(|e| ctx.typ(e).has_tag(crate::code::type_tag::TypeTag::NONE))
                    .unwrap_or(false);
                if none {
                    ReferenceKind::TOPLEVEL
                } else {
                    ReferenceKind::IMPLICIT_INNER
                }
            }
        }
    }
}

impl LookupHelper for ReferenceLookupHelper {
    fn name(&self) -> &crate::util::name::Name {
        &self.name
    }
    fn site(&self) -> TypeId {
        self.site
    }
    fn argtypes(&self) -> &crate::util::list::List<TypeId> {
        &self.argtypes
    }
    fn typeargtypes(&self) -> &Option<crate::util::list::List<TypeId>> {
        &self.typeargtypes
    }
    fn max_phase(&self) -> MethodResolutionPhase {
        self.max_phase
    }

    fn lookup(
        &mut self,
        rs: &mut Resolve,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        phase: MethodResolutionPhase,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        match self.kind {
            ReferenceHelperKind::Method | ReferenceHelperKind::UnboundMethod => rs.find_method(
                ctx,
                syms,
                types,
                attr,
                chk,
                infer,
                env,
                self.site,
                self.name.clone(),
                self.argtypes.clone(),
                self.typeargtypes.clone(),
                phase.is_boxing_required(),
                phase.is_varargs_required(),
                resolve_context,
            ),
            ReferenceHelperKind::ArrayConstructor => {
                // Java: WriteableScope sc = WriteableScope.create(syms.arrayClass);
                // MethodSymbol arrayConstr = new MethodSymbol(PUBLIC, name, null, site.tsym);
                // arrayConstr.type = new MethodType(List.of(syms.intType), site, nil, syms.methodClass);
                let scope = ctx.scope_create(Some(syms.array_class));
                let site_tsym = ctx.typ(self.site).tsym.expect("array constructor site");
                let array_constr = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                    crate::code::kinds::Kind::MTH,
                    crate::code::flags::PUBLIC as i64,
                    self.name.clone(),
                    None,
                    Some(site_tsym),
                    crate::code::symbol::SymbolKind::MethodSymbol {
                        extra_params: crate::util::list::List::nil(),
                        captured_locals: crate::util::list::List::nil(),
                        params: crate::util::list::List::nil(),
                        default_value: None,
                    },
                ));
                let ctor_type = ctx.alloc_type(crate::code::type_::Type::new_method_type(
                    crate::util::list::List::of(syms.int_type),
                    self.site,
                    crate::util::list::List::nil(),
                    syms.method_class,
                ));
                ctx.sym_mut(array_constr).ty = Some(ctor_type);
                ctx.scope_enter(scope, array_constr);
                rs.find_method_in_scope(
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    infer,
                    env,
                    self.site,
                    self.name.clone(),
                    self.argtypes.clone(),
                    self.typeargtypes.clone(),
                    scope,
                    rs.method_not_found,
                    phase.is_boxing_required(),
                    phase.is_varargs_required(),
                    false,
                    resolve_context,
                )
            }
            ReferenceHelperKind::Constructor => {
                let sym = if self.needs_inference {
                    rs.find_diamond(
                        ctx,
                        syms,
                        types,
                        attr,
                        chk,
                        infer,
                        env,
                        self.site,
                        self.argtypes.clone(),
                        self.typeargtypes.clone(),
                        phase.is_boxing_required(),
                        phase.is_varargs_required(),
                        resolve_context,
                    )
                } else {
                    rs.find_method(
                        ctx,
                        syms,
                        types,
                        attr,
                        chk,
                        infer,
                        env,
                        self.site,
                        self.name.clone(),
                        self.argtypes.clone(),
                        self.typeargtypes.clone(),
                        phase.is_boxing_required(),
                        phase.is_varargs_required(),
                        resolve_context,
                    )
                };
                if rs.enclosing_instance_missing(ctx, syms, types, env, self.site) {
                    rs.new_resolve_error(
                        ctx,
                        crate::code::kinds::Kind::MISSING_ENCL,
                        "BadConstructorReferenceError",
                        crate::code::symbol::ResolveErrorData::BadConstructorReference { sym },
                    )
                } else {
                    sym
                }
            }
        }
    }

    fn access(
        &mut self,
        _rs: &mut Resolve,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        sym: SymbolId,
        resolve_context: &mut MethodResolutionContext,
    ) -> SymbolId {
        let _ = (attr, infer, env, pos);
        let mut sym = sym;
        if ctx.sym(sym).kind == crate::code::kinds::Kind::AMBIGUOUS {
            let base = crate::code::symbol::base_symbol(ctx, sym);
            let data = crate::code::symbol::symbol_resolve_error_data(ctx, base).cloned();
            if let Some(crate::code::symbol::ResolveErrorData::Ambiguity { syms: syms_vec }) = data {
                if let Some(merged) = types.merge_abstracts(
                    ctx,
                    syms,
                    chk,
                    syms_vec.as_slice(),
                    self.site,
                    true,
                ) {
                    sym = merged.symbol;
                }
            }
        }
        let _ = resolve_context;
        let _ = location;
        sym
    }
}

impl Resolve {
    /// Java: boolean enclosingInstanceMissing(Env<AttrContext> env, Type type)
    /// (Resolve.java:3746-3752)
/// # Arguments
///   - ctx/syms/types: explicit services
///   - env: the attribution environment
///   - site: the constructor site type
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:3746-3752.
///
    pub fn enclosing_instance_missing(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
    ) -> bool {
        let is_class = ctx.typ(site).has_tag(crate::code::type_tag::TypeTag::CLASS);
        let enclosing_is_class = ctx
            .typ(site)
            .get_enclosing_type()
            .map(|encl| ctx.typ(encl).has_tag(crate::code::type_tag::TypeTag::CLASS))
            .unwrap_or(false);
        if !(is_class && enclosing_is_class) {
            return false;
        }
        let tsym = ctx.typ(site).tsym.expect("class type symbol");
        let encl = self.resolve_self_containing_internal(ctx, syms, types, env, tsym, false);
        match encl {
            None => true,
            Some(encl) => ctx.sym(encl).kind.is_resolution_error(),
        }
    }

    /// Java: private Symbol resolveSelfContainingInternal(Env<AttrContext> env,
    /// Symbol member, boolean isSuperCall) (Resolve.java:3760-3800)
/// # Arguments
///   - ctx/syms/types: explicit services
///   - env: the attribution environment
///   - member: the member symbol
///   - is_super_call: the super-call context
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:3760-3800.
///
    pub fn resolve_self_containing_internal(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        env: &Rc<Env<AttrContext>>,
        member: SymbolId,
        is_super_call: bool,
    ) -> Option<SymbolId> {
        let name = self.names._this.clone();
        let mut env1 = if is_super_call {
            env.outer.clone()
        } else {
            Some(Rc::clone(env))
        };
        let mut static_only = false;
        while let Some(current) = env1.clone() {
            if current.outer.is_none() {
                break;
            }
            if self.is_static(&current) {
                static_only = true;
            }
            if let Some(encl_tree) = current.encl_class {
                if let Some(encl_sym) = class_decl_symbol(ctx, encl_tree) {
                    let member_owner = ctx.sym(member).owner.expect("member owner");
                    let member_encl = crate::code::symbol::symbol_encl_class(ctx, member_owner)
                        .expect("member enclosing class");
                    if crate::code::symbol::is_sub_class(
                        ctx,
                        syms,
                        types,
                        encl_sym,
                        member_encl,
                    ) {
                        let scope = current.info.borrow().scope;
                        if let Some(scope) = scope {
                            if let Some(sym) = ctx.scope_find_first(scope, &name, &|_| true) {
                                if static_only {
                                    return Some(self.new_static_error(ctx, sym));
                                }
                                return Some(sym);
                            }
                        }
                    }
                    if ctx.sym(encl_sym).flags_field & (crate::code::flags::STATIC as i64) != 0 {
                        static_only = true;
                    }
                }
            }
            env1 = current.outer.clone();
        }
        None
    }

}


impl Resolve {
    /// Java: ReferenceLookupHelper makeReferenceLookupHelper(JCMemberReference,
    /// Type site, Name name, List<Type> argtypes, List<Type> typeargtypes,
    /// MethodResolutionPhase maxPhase) (Resolve.java:3027-3045)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/types: explicit services
///   - reference_tree/site/name: the reference site
///   - argtypes/typeargtypes/max_phase: the lookup input
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:3027-3045.
///
    pub fn make_reference_lookup_helper(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        reference_tree: TreeId,
        site: TypeId,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        max_phase: MethodResolutionPhase,
    ) -> ReferenceLookupHelper {
        use crate::code::type_tag::TypeTag;
        if name != self.names.init {
            let skipped = self.skip_type_vars_capture(ctx, syms, types, site);
            let mut helper = ReferenceLookupHelper::new_method(
                reference_tree,
                name,
                skipped,
                argtypes,
                typeargtypes,
                max_phase,
            );
            helper.original_site = site;
            helper.method_not_found = self.method_not_found;
            helper
        } else if ctx.typ(site).has_tag(TypeTag::ARRAY) {
            let mut helper = ReferenceLookupHelper::new_array_constructor(
                reference_tree,
                name,
                site,
                argtypes,
                typeargtypes,
                max_phase,
            );
            helper.method_not_found = self.method_not_found;
            helper
        } else {
            let site_tsym = ctx.typ(site).tsym.expect("constructor reference site");
            let is_raw = types.is_raw_type_public(ctx, site);
            let (site2, needs_inference) = if is_raw {
                let enclosing = ctx.typ(site).get_enclosing_type();
                let tsym_type = ctx.sym(site_tsym).ty.expect("site type symbol type");
                let enclosing_is_raw = enclosing
                    .map(|encl| types.is_raw_type_public(ctx, encl))
                    .unwrap_or(false);
                let inner = ctx.sym(site_tsym).owner.map(|owner| {
                    ctx.sym(owner).kind == crate::code::kinds::Kind::TYP
                        && (ctx.sym(site_tsym).flags_field
                            & (crate::code::flags::INTERFACE as i64))
                            == 0
                });
                let type_args = if !(inner.unwrap_or(false) && enclosing_is_raw) {
                    types.get_type_arguments_public(ctx, tsym_type)
                } else {
                    crate::util::list::List::nil()
                };
                let (outer_field, typarams_field, allparams_field, supertype_field,
                    interfaces_field, all_interfaces_field) = match &ctx.typ(site).kind {
                    crate::code::type_::TypeKindData::ClassType {
                        outer_field,
                        typarams_field,
                        allparams_field,
                        supertype_field,
                        interfaces_field,
                        all_interfaces_field,
                        ..
                    } => (
                        *outer_field,
                        typarams_field.clone(),
                        allparams_field.clone(),
                        *supertype_field,
                        interfaces_field.clone(),
                        all_interfaces_field.clone(),
                    ),
                    _ => panic!("ClassCastException: ClassType expected"),
                };
                let _ = typarams_field;
                (
                    ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::CLASS,
                        Some(site_tsym),
                        crate::code::type_::TypeKindData::ClassType {
                            outer_field,
                            typarams_field: Some(type_args),
                            allparams_field,
                            supertype_field,
                            interfaces_field,
                            all_interfaces_field,
                        },
                    )),
                    true,
                )
            } else {
                (site, false)
            };
            let mut helper = ReferenceLookupHelper::new_constructor(
                reference_tree,
                self.names.init.clone(),
                site2,
                argtypes,
                typeargtypes,
                max_phase,
                needs_inference,
            );
            helper.original_site = site;
            helper.method_not_found = self.method_not_found;
            helper
        }
    }

    /// Java: Types.skipTypeVars(site, capture=true).
    fn skip_type_vars_capture(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        site: TypeId,
    ) -> TypeId {
        let mut site = site;
        while ctx.typ(site).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
            site = ctx.typ(site).get_upper_bound().expect("type var upper bound");
        }
        types.capture(ctx, syms, site)
    }

    /// Java: Symbol getMemberReference(DiagnosticPosition pos,
    /// Env<AttrContext> env, JCMemberReference referenceTree, Type site,
    /// Name name) (Resolve.java:3000-3016)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/types/attr/chk/infer: explicit services
///   - pos/env/reference_tree: the reference context
///   - site/name: the qualifier type and name
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:3000-3016.
///
    pub fn get_member_reference(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        pos: i32,
        env: &Rc<Env<AttrContext>>,
        reference_tree: TreeId,
        site: TypeId,
        name: crate::util::name::Name,
    ) -> SymbolId {
        let site = types.capture(ctx, syms, site);
        let mut helper = self.make_reference_lookup_helper(
            ctx,
            syms,
            types,
            reference_tree,
            site,
            name,
            crate::util::list::List::nil(),
            None,
            MethodResolutionPhase::VARARITY,
        );
        let new_env = env.dup(env.tree, env.info.borrow().dup(None));
        let nil_check: Rc<RefCell<dyn MethodCheck>> = Rc::new(RefCell::new(NilMethodCheck));
        let sym = self.lookup_method_with_check(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            &new_env,
            Some(Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos))),
            site_tsym_or_err(ctx, site),
            nil_check,
            &mut helper,
        );
        env.info.borrow_mut().pending_resolution_phase =
            new_env.info.borrow().pending_resolution_phase;
        sym
    }

    /// Java: Pair<Symbol, ReferenceLookupHelper> resolveMemberReference(
    /// Env<AttrContext>, JCMemberReference, Type site, Name name, List<Type>
    /// argtypes, List<Type> typeargtypes, Type descriptor, MethodCheck,
    /// InferenceContext, ReferenceChooser) (Resolve.java:3071-3130)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms/types/attr/chk/infer: explicit services
///   - env/pos/reference_tree: the reference context
///   - site/name/argtypes/typeargtypes/descriptor: the lookup input
///   - method_check/inference_context/reference_chooser: the resolution round
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Resolve.java:3071-3130.
///
    pub fn resolve_member_reference(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        attr: &mut crate::comp::attr::Attr,
        chk: &crate::comp::check::Check,
        infer: &Rc<crate::comp::infer::Infer>,
        env: &Rc<Env<AttrContext>>,
        pos: i32,
        reference_tree: TreeId,
        site: TypeId,
        name: crate::util::name::Name,
        argtypes: crate::util::list::List<TypeId>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        descriptor: TypeId,
        method_check: Rc<RefCell<dyn MethodCheck>>,
        inference_context: &Rc<crate::comp::infer::InferenceContext>,
        reference_chooser: ReferenceChooserKind,
    ) -> (SymbolId, Option<ReferenceLookupHelper>) {
        let bound_helper = self.make_reference_lookup_helper(
            ctx,
            syms,
            types,
            reference_tree,
            site,
            name,
            argtypes.clone(),
            typeargtypes.clone(),
            MethodResolutionPhase::VARARITY,
        );
        let bound_env = env.dup(env.tree, env.info.borrow().dup(None));
        let mut bound_context = MethodResolutionContext::new();
        bound_context.method_check = Some(Rc::clone(&method_check));
        let mut bound_helper = bound_helper;
        let bound_sym = self.lookup_method(
            ctx,
            syms,
            types,
            attr,
            chk,
            infer,
            &bound_env,
            Some(Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos))),
            site_tsym_or_err(ctx, site),
            &mut bound_context,
            &mut bound_helper,
        );
        let bound_res = ReferenceLookupResult::new(ctx, bound_sym, &bound_context);
        let unbound_env = env.dup(env.tree, env.info.borrow().dup(None));
        let unbound_helper = bound_helper.unbound_lookup(self, ctx, syms, types, inference_context);
        let unbound_res = match &unbound_helper {
            None => None,
            Some(_) => {
                let mut unbound_context = MethodResolutionContext::new();
                unbound_context.method_check = Some(Rc::clone(&method_check));
                let mut helper = unbound_helper.clone().expect("unbound helper");
                let unbound_sym = self.lookup_method(
                    ctx,
                    syms,
                    types,
                    attr,
                    chk,
                    infer,
                    &unbound_env,
                    Some(Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos))),
                    site_tsym_or_err(ctx, site),
                    &mut unbound_context,
                    &mut helper,
                );
                Some(ReferenceLookupResult::new(ctx, unbound_sym, &unbound_context))
            }
        };
        let best_res = self.reference_chooser_result(
            ctx,
            syms,
            types,
            reference_chooser,
            &bound_res,
            unbound_res.as_ref(),
        );
        let unbound_selected = match &unbound_res {
            Some(unbound) => best_res.sym == unbound.sym && best_res.static_kind == unbound.static_kind,
            None => false,
        };
        env.info.borrow_mut().pending_resolution_phase = if unbound_selected {
            unbound_env.info.borrow().pending_resolution_phase
        } else {
            bound_env.info.borrow().pending_resolution_phase
        };
        let mut result_sym = best_res.sym;
        if !ctx.sym(result_sym).kind.is_resolution_error()
            && ctx.sym(result_sym).flags_field & crate::code::flags::SIGNATURE_POLYMORPHIC != 0
        {
            env.info.borrow_mut().pending_resolution_phase = Some(MethodResolutionPhase::BASIC);
            result_sym = self.find_polymorphic_signature_instance_with_type(
                ctx,
                syms,
                types,
                result_sym,
                descriptor,
            );
        }
        let chosen_helper = if unbound_selected { unbound_helper } else { Some(bound_helper) };
        (result_sym, chosen_helper)
    }

    /// Java: ReferenceChooser.result(boundRes, unboundRes)
    #[allow(clippy::too_many_arguments)]
    fn reference_chooser_result(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        kind: ReferenceChooserKind,
        bound: &ReferenceLookupResult,
        unbound: Option<&ReferenceLookupResult>,
    ) -> ReferenceLookupResult {
        match unbound {
            Some(unbound) => self.reference_chooser_unbound_result(ctx, syms, types, kind, bound, unbound),
            None => self.reference_chooser_bound_result(ctx, syms, types, kind, bound),
        }
    }

    fn reference_chooser_bound_result(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        kind: ReferenceChooserKind,
        bound: &ReferenceLookupResult,
    ) -> ReferenceLookupResult {
        let error = match kind {
            ReferenceChooserKind::Basic => {
                if !bound.is_success() || bound.has_kind(StaticKind::NON_STATIC) {
                    return *bound;
                }
                ReferenceLookupResult::error(self.bad_method_reference_error(ctx, bound.sym, false))
            }
            ReferenceChooserKind::Structural => {
                if !bound.is_success() || !bound.has_kind(StaticKind::STATIC) {
                    return *bound;
                }
                ReferenceLookupResult::error(self.bad_method_reference_error(ctx, bound.sym, false))
            }
        };
        let _ = (syms, types);
        error
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_chooser_unbound_result(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        kind: ReferenceChooserKind,
        bound: &ReferenceLookupResult,
        unbound: &ReferenceLookupResult,
    ) -> ReferenceLookupResult {
        match kind {
            ReferenceChooserKind::Basic => {
                if bound.is_success()
                    && ctx.sym(bound.sym).flags_field & (crate::code::flags::STATIC as i64) != 0
                    && (!unbound.is_success() || unbound.has_kind(StaticKind::STATIC))
                {
                    return *bound;
                }
                if unbound.is_success()
                    && ctx.sym(unbound.sym).flags_field & (crate::code::flags::STATIC as i64) == 0
                    && (!bound.is_success() || bound.has_kind(StaticKind::NON_STATIC))
                {
                    return *unbound;
                }
                if bound.is_success() && unbound.is_success() {
                    return ReferenceLookupResult::error(
                        self.ambiguity_error(ctx, bound.sym, unbound.sym),
                    );
                }
                if bound.is_success() || unbound.is_success() {
                    return ReferenceLookupResult::error(self.bad_method_reference_error(
                        ctx,
                        if bound.is_success() { bound.sym } else { unbound.sym },
                        true,
                    ));
                }
                if bound.can_ignore(ctx, syms, types) && !unbound.can_ignore(ctx, syms, types) {
                    *unbound
                } else {
                    *bound
                }
            }
            ReferenceChooserKind::Structural => {
                if bound.is_success() && !bound.has_kind(StaticKind::NON_STATIC) {
                    return *bound;
                }
                if unbound.is_success() && !unbound.has_kind(StaticKind::STATIC) {
                    return *unbound;
                }
                if bound.is_success() || unbound.is_success() {
                    return ReferenceLookupResult::error(self.bad_method_reference_error(
                        ctx,
                        if bound.is_success() { bound.sym } else { unbound.sym },
                        true,
                    ));
                }
                if bound.can_ignore(ctx, syms, types) && !unbound.can_ignore(ctx, syms, types) {
                    *unbound
                } else {
                    *bound
                }
            }
        }
    }

    fn bad_method_reference_error(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        sym: SymbolId,
        unbound_lookup: bool,
    ) -> SymbolId {
        self.new_resolve_error(
            ctx,
            crate::code::kinds::Kind::STATICERR,
            "BadMethodReferenceError",
            crate::code::symbol::ResolveErrorData::BadMethodReference { sym, unbound_lookup },
        )
    }
}

/// Porting rules helper: the member-reference lookup location is the site type symbol.
fn site_tsym_or_err(
    ctx: &crate::compiler_context::CompilerContext,
    site: TypeId,
) -> SymbolId {
    ctx.typ(site).tsym.expect("site type symbol")
}

impl Resolve {
    /// Java: Symbol resolveIdent(DiagnosticPosition pos, Env<AttrContext> env,
    /// Name name, KindSelector kind) (Resolve.java:2676-2690)
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - env: the attribution environment;
    ///   - name: the identifier;
    ///   - kind: the expected kind selector.
    ///
    /// # Returns
    ///   - the resolved (and access-checked) symbol.
    ///
    /// # Panics
    ///   - the environment must have an enclosing class (upstream cast).
    ///
    /// Upstream: Resolve.java:2676-2690.
    pub fn resolve_ident(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        name: crate::util::name::Name,
        kind: crate::code::kinds::KindSelector,
    ) -> SymbolId {
        // Java: return accessBase(findIdent(pos, env, name, kind), pos,
        // env.enclClass.sym.type, name, false);
        let found = self.find_ident(ctx, syms, pos.clone(), env, &name, kind);
        let encl_tree = env.encl_class.expect("resolving ident outside class");
        // Java: env.enclClass.sym.type (Resolve.java:2683).
        let encl_sym = match &ctx.tree(encl_tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("enclosing class"),
        };
        let site = ctx.sym(encl_sym).ty;
        let location = env
            .info
            .borrow()
            .scope
            .and_then(|scope| ctx.scope(scope).owner);
        self.access_base(ctx, syms, found, pos, location, site, &name, false)
    }

}

impl Resolve {
    /// Java: Symbol resolveSelf(DiagnosticPosition pos, Env<AttrContext> env,
    /// TypeSymbol c, Name name) (Resolve.java:3724-3775).
    ///
    /// # Arguments
    ///   - ctx/syms/types: explicit services;
    ///   - pos: the diagnostic position;
    ///   - env: the attribution environment;
    ///   - c: the qualifier type symbol;
    ///   - name: this or super.
    ///
    /// # Returns
    ///   - the this/super symbol (or a resolve error).
    ///
    /// # Panics
    ///   - the environment chain is asserted as in the upstream code.
    ///
    /// Upstream: Resolve.java:3724-3775.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_self(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        c: SymbolId,
        name: crate::util::name::Name,
    ) -> SymbolId {
        let mut env1 = Rc::clone(env);
        let mut static_only = false;
        while env1.outer.is_some() {
            if self.is_static(&env1) {
                static_only = true;
            }
            if let Some(encl_tree) = env1.encl_class {
                if let Some(encl_sym) = crate::tree::tree_info::symbol(ctx, encl_tree) {
                    if encl_sym == c {
                        let scope = env1.info.borrow().scope;
                        if let Some(scope) = scope {
                            if let Some(sym) = ctx.scope_find_first(scope, &name, &|_| true) {
                                let sym = if static_only {
                                    self.new_static_error(ctx, sym)
                                } else {
                                    sym
                                };
                                let encl_sym = crate::tree::tree_info::symbol(ctx, env.encl_class.expect("enclClass"))
                                    .expect("enclClass sym");
                                let encl_type = ctx.sym(encl_sym).ty;
                                return self.access_base(
                                    ctx, syms, sym, pos, None, encl_type, &name, true,
                                );
                            }
                        }
                    }
                    if ctx.sym(encl_sym).flags_field & (crate::code::flags::STATIC as i64) != 0 {
                        static_only = true;
                    }
                }
            }
            env1 = Rc::clone(env1.outer.as_ref().expect("env outer"));
        }
        let encl_class = crate::tree::tree_info::symbol(ctx, env.encl_class.expect("enclClass"))
            .expect("enclClass sym");
        let is_interface = ctx.sym(c).flags_field & (crate::code::flags::INTERFACE as i64) != 0;
        if is_interface
            && name == self.names._super
            && !self.is_static(env)
            && types.is_direct_super_interface(ctx, syms, c, encl_class)
        {
            let encl_type = ctx.sym(encl_class).ty.expect("encl class type");
            let mut pruned = self.prune_interfaces(ctx, syms, types, encl_type);
            while pruned.non_empty() {
                let t = *pruned.head();
                if ctx.typ(t).tsym == Some(c) {
                    env.info.borrow_mut().default_super_call_site = Some(t);
                    let super_type = types.as_super(ctx, syms, encl_type, c);
                    return ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                        crate::code::kinds::Kind::VAR,
                        0,
                        self.names._super.clone(),
                        super_type,
                        Some(encl_class),
                        crate::code::symbol::SymbolKind::VarSymbol {
                            pos: 0,
                            adr: 0,
                            data_kind: None,
                            data: None,
                        },
                    ));
                }
                pruned = pruned.tail();
            }
            let mut supers = types.direct_supertypes(ctx, syms, encl_type);
            while supers.non_empty() {
                let i = *supers.head();
                let i_tsym = ctx.typ(i).tsym.expect("direct super type symbol");
                if crate::code::symbol::is_sub_class(ctx, syms, types, i_tsym, c) && i_tsym != c {
                    let redundant = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        crate::resources::compiler_properties::fragments::redundant_supertype(
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                ctx.sym(c).name.to_java_string().to_string_lossy().to_string(),
                            ),
                            crate::resources::diagnostic_contract::DiagnosticArg::Text(
                                ctx.typ(i).tsym.map(|tsym| ctx.sym(tsym).name.to_java_string().to_string_lossy().to_string()).unwrap_or_default(),
                            ),
                        ),
                    ));
                    let info = crate::resources::compiler_properties::errors::illegal_default_super_call(
                        crate::resources::diagnostic_contract::DiagnosticArg::Text(
                            ctx.sym(c).name.to_java_string().to_string_lossy().to_string(),
                        ),
                        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                            std::rc::Rc::new(redundant),
                        ),
                    );
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.log.borrow_mut(),
                        pos.as_ref().map(|pos| pos.get_start_position()).unwrap_or(0),
                        crate::util::jc_diagnostic::Error::from(info),
                    );
                    return syms.err_symbol;
                }
                supers = supers.tail();
            }
            panic!("AssertionError");
        }
        let info = crate::resources::compiler_properties::errors::not_encl_class(
            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                ctx.sym(c).name.to_java_string().to_string_lossy().to_string(),
            ),
        );
        crate::util::abstract_log::AbstractLog::error_at(
            &mut *self.log.borrow_mut(),
            pos.as_ref().map(|pos| pos.get_start_position()).unwrap_or(0),
            crate::util::jc_diagnostic::Error::from(info),
        );
        syms.err_symbol
    }

    /// Java: private List<Type> pruneInterfaces(Type t) (Resolve.java:3777-3790).
    ///
    /// # Arguments
    ///   - ctx/syms/types: explicit services;
    ///   - t: the class type whose interfaces are pruned.
    ///
    /// # Returns
    ///   - the direct interfaces not overridden by a direct supertype.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:3777-3790.
    fn prune_interfaces(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        t: TypeId,
    ) -> crate::util::list::List<TypeId> {
        let mut result: crate::util::list::List<TypeId> = crate::util::list::List::nil();
        let mut interfaces = types.interfaces(ctx, syms, t);
        while interfaces.non_empty() {
            let t1 = *interfaces.head();
            let mut should_add = true;
            let mut supers = types.direct_supertypes(ctx, syms, t);
            while supers.non_empty() {
                let t2 = *supers.head();
                if t1 != t2
                    && !ctx.typ(t2).has_tag(crate::code::type_tag::TypeTag::ERROR)
                    && types.is_subtype_no_capture(ctx, syms, t2, t1)
                {
                    should_add = false;
                }
                supers = supers.tail();
            }
            if should_add {
                result = result.append(t1);
            }
            interfaces = interfaces.tail();
        }
        result
    }
}


    /// Porting rules helper: public wrapper of findMemberType for Attr.attribIdent
    /// (Resolve.java:2461-2480).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the attribution environment;
    ///   - site: the qualifier type;
    ///   - name: the member type name;
    ///   - c: the qualifier class symbol.
    ///
    /// # Returns
    ///   - the member type symbol or a resolve error.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:2461-2480.
impl Resolve {
    pub(crate) fn find_member_type_pub(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        c: SymbolId,
    ) -> SymbolId {
        self.find_member_type(ctx, syms, env, site, name, c)
    }
}

impl Resolve {
    /// Java: Symbol resolveSelfContaining(DiagnosticPosition pos, Env<AttrContext>
    /// env, Symbol member, boolean isSuperCall) (Resolve.java:3793-3803).
    ///
    /// # Arguments
    ///   - ctx/syms/types: explicit services;
    ///   - pos: the diagnostic position;
    ///   - env: the attribution environment;
    ///   - member: the member symbol;
    ///   - is_super_call: the super-call context.
    ///
    /// # Returns
    ///   - the enclosing this symbol or syms.errSymbol.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:3793-3803.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_self_containing(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        member: SymbolId,
        is_super_call: bool,
    ) -> SymbolId {
        let sym = self.resolve_self_containing_internal(ctx, syms, types, env, member, is_super_call);
        let Some(sym) = sym else {
            let info = crate::resources::compiler_properties::errors::encl_class_required(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    ctx.sym(member).name.to_java_string().to_string_lossy().to_string(),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos.as_ref().map(|pos| pos.get_start_position()).unwrap_or(0),
                crate::util::jc_diagnostic::Error::from(info),
            );
            return syms.err_symbol;
        };
        let encl_sym = crate::tree::tree_info::symbol(ctx, env.encl_class.expect("enclClass"))
            .expect("enclClass sym");
        let encl_type = ctx.sym(encl_sym).ty;
        let name = ctx.sym(sym).name.clone();
        self.access_base(ctx, syms, sym, pos, None, encl_type, &name, true)
    }

    /// Java: Type resolveImplicitThis(DiagnosticPosition pos, Env<AttrContext> env,
    /// Type t, boolean isSuperCall) (Resolve.java:3845-3852).
    ///
    /// # Arguments
    ///   - ctx/syms/types: explicit services;
    ///   - pos: the diagnostic position;
    ///   - env: the attribution environment;
    ///   - t: the type whose container this is resolved;
    ///   - is_super_call: the super-call context.
    ///
    /// # Returns
    ///   - the implicit this type.
    ///
    /// # Panics
    ///   - the type must have an enclosing type (upstream cast).
    ///
    /// Upstream: Resolve.java:3845-3852.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_implicit_this(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        types: &crate::code::types::Types,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        env: &Rc<Env<AttrContext>>,
        t: TypeId,
        is_super_call: bool,
    ) -> TypeId {
        let t_tsym = ctx.typ(t).tsym.expect("AssertionError: type symbol");
        let owner = ctx.sym(t_tsym).owner.expect("type owner");
        let owner_kind = ctx.sym(owner).kind;
        let vals_mth = crate::code::kinds::KindSelector::VAL_MTH.contains(owner_kind.to_selector());
        let this_sym = if vals_mth {
            let encl = ctx.typ(t).get_enclosing_type().expect("enclosing type");
            let encl_tsym = ctx.typ(encl).tsym.expect("enclosing type symbol");
            self.resolve_self(ctx, syms, types, pos.clone(), env, encl_tsym, self.names._this.clone())
        } else {
            self.resolve_self_containing(ctx, syms, types, pos.clone(), env, t_tsym, is_super_call)
        };
        let this_type = ctx.sym(this_sym).ty.expect("this type");
        let encl_sym = crate::tree::tree_info::symbol(ctx, env.encl_class.expect("enclClass"))
            .expect("enclClass sym");
        if env.info.borrow().is_self_call
            && ctx.typ(this_type).tsym == Some(encl_sym)
        {
            // Java: log.error(pos, Errors.CantRefBeforeCtorCalled("this"));
            let info = crate::resources::compiler_properties::errors::cant_ref_before_ctor_called(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display("this"),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos.as_ref().map(|pos| pos.get_start_position()).unwrap_or(0),
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
        this_type
    }
}

impl Resolve {
    /// Java: void checkNonAbstract(DiagnosticPosition pos, Symbol sym)
    /// (Resolve.java:2655-2667).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - pos: the diagnostic position;
    ///   - sym: the selected symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:2655-2667.
    pub fn check_non_abstract(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        sym: SymbolId,
    ) {
        let flags = ctx.sym(sym).flags_field;
        if (flags & (crate::code::flags::ABSTRACT as i64)) != 0
            && (flags & crate::code::flags::DEFAULT) == 0
        {
            let kind = crate::code::kinds::kind_name(ctx.sym(sym), ctx);
            let enclosing = crate::code::symbol::symbol_encl_class(ctx, sym);
            let info = crate::resources::compiler_properties::errors::abstract_cant_be_accessed_directly(
                crate::resources::diagnostic_contract::KindNameArg::placeholder(kind.to_string()),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    ctx.sym(sym).name.to_java_string().to_string_lossy().to_string(),
                ),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    enclosing
                        .map(|encl| ctx.sym(encl).name.to_java_string().to_string_lossy().to_string())
                        .unwrap_or_default(),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos.as_ref().map(|pos| pos.get_start_position()).unwrap_or(0),
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

    /// Porting rules helper: public wrapper of new StaticError(sym).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - sym: the symbol to wrap.
    ///
    /// # Returns
    ///   - the StaticError symbol.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:4520-4530.
    pub fn new_static_error_pub(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        sym: SymbolId,
    ) -> SymbolId {
        self.new_static_error(ctx, sym)
    }
}


impl Resolve {
    /// Adapter: public wrapper of the ported isAccessible(Env, TypeSymbol)
    /// subset (Resolve.java:1042-1060).
    ///
    /// # Arguments
    ///   - ctx/syms/types: explicit services;
    ///   - env: the attribution environment;
    ///   - site: the qualifier type;
    ///   - sym: the referenced symbol.
    ///
    /// # Returns
    ///   - true when the symbol is accessible.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Resolve.java:1042-1060.
    pub fn is_accessible_pub(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        sym: SymbolId,
    ) -> bool {
        self.is_accessible(ctx, syms, types, env, site, sym)
    }
}


impl Resolve {
    /// Java: public void logAccessErrorInternal(Env<AttrContext> env, JCTree tree,
    /// Type type) (Resolve.java:3861-3866) — used by TransTypes when checking
    /// the target type of a synthetic cast.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the attribution environment;
    ///   - tree: the cast tree;
    ///   - target: the cast target type.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the environment must belong to a class.
    ///
    /// Upstream: Resolve.java:3861-3866.
    pub fn log_access_error_internal(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
        target: TypeId,
    ) {
        let encl_tree = env.encl_class.expect("enclClass");
        let encl_sym = match &ctx.tree(encl_tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("enclosing class"),
        };
        let encl_ty = ctx.sym(encl_sym).ty.expect("enclClass type");
        let target_tsym = ctx.typ(target).tsym.expect("target type symbol");
        let error = self.new_access_error(ctx, syms, env, Some(encl_ty), target_tsym);
        let pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>> = Some(Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
        ));
        let empty_name = self.names.empty.clone();
        self.log_resolve_error(
            ctx,
            syms,
            error,
            pos,
            Some(encl_sym),
            Some(encl_ty),
            &empty_name,
            None,
            None,
        );
    }
}
#[cfg(test)]
mod tests {

    #[test]
    #[should_panic(expected = "Resolve is not registered in the Context")]
    fn instance_requires_registration() {
        let mut context = crate::util::context::Context::new();
        let _ = Resolve::instance(&mut context);
    }
    use super::*;

    fn names() -> crate::util::names::Names {
        crate::util::names::Names::new(false)
    }

    fn error_symbol(
        ctx: &mut crate::compiler_context::CompilerContext,
        kind: Kind,
    ) -> SymbolId {
        crate::code::symbol::symbol_new_resolve_error(
            ctx,
            kind,
            names().from_utf(b"e"),
            "test error",
            crate::code::symbol::ResolveErrorData::SymbolNotFound,
        )
    }

    #[test]
    fn method_resolution_phase_merge_results() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let wrong_mth = error_symbol(&mut ctx, Kind::WRONG_MTH);
        let wrong_mths = error_symbol(&mut ctx, Kind::WRONG_MTHS);
        let absent_mth = error_symbol(&mut ctx, Kind::ABSENT_MTH);
        // non-VARARITY phases return the new symbol unchanged.
        assert_eq!(
            MethodResolutionPhase::BASIC.merge_results(&ctx, wrong_mths, wrong_mth),
            wrong_mth
        );
        assert_eq!(
            MethodResolutionPhase::BOX.merge_results(&ctx, wrong_mths, wrong_mth),
            wrong_mth
        );
        // VARARITY: successful symbol wins.
        let ok = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            Kind::MTH,
            0,
            names().from_utf(b"m"),
            None,
            None,
            crate::code::symbol::SymbolKind::TypeSymbol,
        ));
        assert_eq!(
            MethodResolutionPhase::VARARITY.merge_results(&ctx, wrong_mths, ok),
            ok
        );
        // VARARITY: WRONG_MTHS + WRONG_MTH keeps the previous (multi) error.
        assert_eq!(
            MethodResolutionPhase::VARARITY.merge_results(&ctx, wrong_mths, wrong_mth),
            wrong_mths
        );
        // VARARITY: WRONG_MTH + WRONG_MTH prefers the current symbol.
        assert_eq!(
            MethodResolutionPhase::VARARITY.merge_results(&ctx, wrong_mth, wrong_mth),
            wrong_mth
        );
        // VARARITY: ABSENT_MTH does not override the previous error.
        assert_eq!(
            MethodResolutionPhase::VARARITY.merge_results(&ctx, wrong_mth, absent_mth),
            wrong_mth
        );
    }

    #[test]
    #[should_panic(expected = "mergeResults invariant")]
    fn method_resolution_phase_merge_results_checks_the_invariant() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        // A non-resolution-error symbol as bestSoFar violates the upstream
        // Assert.check (Resolve.java:4872).
        let ok = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            Kind::MTH,
            0,
            names().from_utf(b"ok"),
            None,
            None,
            crate::code::symbol::SymbolKind::TypeSymbol,
        ));
        let absent = error_symbol(&mut ctx, Kind::ABSENT_MTH);
        let _ = MethodResolutionPhase::VARARITY.merge_results(&ctx, ok, absent);
    }

    #[test]
    #[should_panic(expected = "mergeResults invariant")]
    fn method_resolution_phase_merge_results_rejects_ambiguous_best() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let ambiguous = error_symbol(&mut ctx, Kind::AMBIGUOUS);
        let absent = error_symbol(&mut ctx, Kind::ABSENT_MTH);
        let _ = MethodResolutionPhase::VARARITY.merge_results(&ctx, ambiguous, absent);
    }

    #[test]
    fn method_resolution_context_candidates() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            Kind::MTH,
            0,
            names().from_utf(b"m"),
            None,
            None,
            crate::code::symbol::SymbolKind::TypeSymbol,
        ));
        let mut context = MethodResolutionContext::new();
        context.step = Some(MethodResolutionPhase::BASIC);
        context.add_applicable_candidate(sym, TypeId::new(0));
        context.add_inapplicable_candidate(sym, None);
        assert_eq!(context.candidates.len(), 2);
        assert!(context.candidates[0].is_applicable());
        assert!(!context.candidates[1].is_applicable());
        let deferred = context.deferred_attr_context(
            sym,
            crate::comp::infer::empty_context(),
            None,
            None,
        );
        assert_eq!(deferred.phase, MethodResolutionPhase::BASIC);
        assert!(!context.internal());
        assert_eq!(context.attr_mode(), crate::comp::deferred_attr::AttrMode::SPECULATIVE);
    }

    #[test]
    fn method_resolution_steps_and_check_diag() {
        let steps = method_resolution_steps();
        assert_eq!(steps.length(), 3);
        assert_eq!(*steps.head(), MethodResolutionPhase::BASIC);
        assert_eq!(*steps.tail().head(), MethodResolutionPhase::BOX);
        assert_eq!(*steps.tail().tail().head(), MethodResolutionPhase::VARARITY);
        assert_eq!(MethodCheckDiag::ArityMismatch.basic_key(), "arg.length.mismatch");
        assert_eq!(
            MethodCheckDiag::ArgMismatch.infer_key(),
            "infer.no.conforming.assignment.exists"
        );
        assert!(MethodCheckDiag::VarargMismatch.regex().contains("varargs.argument.mismatch"));
    }

    #[test]
    fn inapplicable_method_exception_keeps_the_diagnostic() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let diags = crate::util::jc_diagnostic::Factory::new(
            crate::util::javac_messages::JavacMessages::instance(&mut crate::util::context::Context::new()),
            "compiler",
        );
        let fragment = diags.fragment(crate::resources::compiler_properties::fragments::wrong_number_type_args(
            javac_strings::JavaString::from("1"),
        ).into());
        let ex = InapplicableMethodException::new(fragment.clone());
        assert_eq!(ex.get_diagnostic().get_type(), crate::util::jc_diagnostic::DiagnosticType::FRAGMENT);
        let _ = &mut ctx;
    }
    #[test]
    fn method_resolution_phase_ordinal_and_requirements() {
        assert_eq!(MethodResolutionPhase::BASIC.ordinal(), 0);
        assert_eq!(MethodResolutionPhase::BOX.ordinal(), 1);
        assert_eq!(MethodResolutionPhase::VARARITY.ordinal(), 2);
        assert!(!MethodResolutionPhase::BASIC.is_boxing_required());
        assert!(MethodResolutionPhase::BOX.is_boxing_required());
        assert!(!MethodResolutionPhase::BOX.is_varargs_required());
        assert!(MethodResolutionPhase::VARARITY.is_boxing_required());
        assert!(MethodResolutionPhase::VARARITY.is_varargs_required());
        assert_eq!(
            MethodResolutionPhase::values(),
            vec![
                MethodResolutionPhase::BASIC,
                MethodResolutionPhase::BOX,
                MethodResolutionPhase::VARARITY
            ]
        );
        assert!(!MethodResolutionPhase::BASIC.is_after(MethodResolutionPhase::BASIC));
        assert!(MethodResolutionPhase::VARARITY.is_after(MethodResolutionPhase::BASIC));
    }

    #[test]
    fn interface_lookup_phase_values_and_ordinal() {
        assert_eq!(InterfaceLookupPhase::ABSTRACT_OK.ordinal(), 0);
        assert_eq!(InterfaceLookupPhase::DEFAULT_OK.ordinal(), 1);
        assert_eq!(
            InterfaceLookupPhase::values(),
            vec![
                InterfaceLookupPhase::ABSTRACT_OK,
                InterfaceLookupPhase::DEFAULT_OK
            ]
        );
    }

    #[test]
    fn adjust_args_non_varargs_is_identity() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let names = std::rc::Rc::new(crate::util::names::Names::new(false));
        let syms = crate::code::symtab::Symtab::new(
            &mut ctx,
            std::rc::Rc::clone(&names),
            crate::jvm::target::Target::DEFAULT,
            crate::code::source::Source::JDK17,
        );
        let types = crate::code::types::Types::with_names(std::rc::Rc::clone(&names));
        let args = crate::util::list::List::of(syms.int_type)
            .prepend(syms.boolean_type);
        let adjusted = adjust_args(&mut ctx, &syms, &types, args.clone(), None, 2, true);
        assert_eq!(adjusted.length(), args.length());
        assert_eq!(*adjusted.head(), *args.head());
    }

    #[test]
    fn lookup_filter_abstract_ok() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let names = crate::util::names::Names::new(false);
        let abstract_sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::MTH,
            crate::code::flags::ABSTRACT as i64,
            names.from_utf(b"m"),
            None,
            None,
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params: crate::util::list::List::nil(),
                captured_locals: crate::util::list::List::nil(),
                params: crate::util::list::List::nil(),
                default_value: None,
            },
        ));
        let concrete_sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::MTH,
            0,
            names.from_utf(b"n"),
            None,
            None,
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params: crate::util::list::List::nil(),
                captured_locals: crate::util::list::List::nil(),
                params: crate::util::list::List::nil(),
                default_value: None,
            },
        ));
        let strict = LookupFilter { abstractok: false };
        assert!(strict.test(&ctx, abstract_sym));
        assert!(!strict.test(&ctx, concrete_sym));
        let lenient = LookupFilter { abstractok: true };
        assert!(lenient.test(&ctx, concrete_sym));
    }

}

#[cfg(test)]
mod check_channel_tests {
    use super::*;
    use crate::util::javac_messages::{DEFAULT_BUNDLE_NAME, JavacMessages};

    fn diagnostic() -> crate::util::jc_diagnostic::JCDiagnostic {
        let messages = Rc::new(JavacMessages::new(DEFAULT_BUNDLE_NAME));
        let factory = crate::util::jc_diagnostic::Factory::new(messages, "compiler");
        factory.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::stat_expr_expected(),
        ))
    }

    #[test]
    fn inapplicable_panic_is_caught_at_the_boundary() {
        let diag = diagnostic();
        let result =
            catch_inapplicable_method_exception(|| throw_inapplicable_method_exception(&diag));
        match result {
            Err(MethodCheckFailure::Inapplicable(exception)) => {
                assert_eq!(
                    exception.get_diagnostic().get_message(None).to_string_lossy(),
                    diag.get_message(None).to_string_lossy()
                );
            }
            _ => panic!("expected the inapplicable failure"),
        }
    }

    #[test]
    fn non_inapplicable_panic_propagates_unchanged() {
        let outer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            catch_inapplicable_method_exception(|| panic!("unrelated bug"))
        }));
        let payload = outer.err().expect("the unrelated panic must propagate");
        assert!(
            payload.downcast_ref::<&str>().is_some() || payload.downcast_ref::<String>().is_some()
        );
    }
}



    fn is_accessible_class_free(
        types: &crate::code::types::Types, allow_modules: bool,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        c: SymbolId,
        check_inner: bool,
    ) -> bool {
        // 15.9.5.1: the signature of the anonymous constructor can refer to an
        // inaccessible type.
        if env
            .encl_method
            .map(|encl_method| method_decl_flags(ctx, encl_method))
            .is_some_and(|flags_value| (flags_value & flags::ANONCONSTR as i64) != 0)
        {
            return true;
        }

        if env.info.borrow().visiting_service_implementation {
            let toplevel_module = env_toplevel_module(ctx, env);
            let class_module = package_module(ctx, symbol_packge(ctx, c).expect("class package"));
            if toplevel_module.is_some() && toplevel_module == class_module {
                return true;
            }
        }

        let access = ctx.sym(c).flags_field & flags::AccessFlags as i64;
        let is_accessible;
        if access == flags::PRIVATE as i64 {
            let encl_sym = env
                .encl_class
                .and_then(|class_tree| class_decl_symbol(ctx, class_tree))
                .expect("enclClass symbol");
            is_accessible = symbol_outermost_class(ctx, encl_sym)
                == ctx
                    .sym(c)
                    .owner
                    .and_then(|owner| symbol_outermost_class(ctx, owner));
        } else if access == 0 {
            let toplevel_package = env_toplevel_package(ctx, env);
            is_accessible = toplevel_package == ctx.sym(c).owner
                || toplevel_package == symbol_packge(ctx, c);
        } else if access == flags::PUBLIC as i64 {
            if allow_modules {
                let curr_module = env_toplevel_module(ctx, env).expect("toplevel module");
                // Java: currModule.complete() — the ported symbol complete is a
                // no-op for entered symbols (Completer registered).
                let package = symbol_packge(ctx, c).expect("class package");
                let package_module = package_module(ctx, package);
                let visible = match &ctx.sym(curr_module).kind_data {
                    crate::code::symbol::SymbolKind::ModuleSymbol {
                        visible_packages, ..
                    } => package_fullname(ctx, package)
                        .and_then(|fullname| visible_packages.get(&fullname).copied())
                        == Some(package),
                    _ => false,
                };
                let reads = match &ctx.sym(curr_module).kind_data {
                    crate::code::symbol::SymbolKind::ModuleSymbol { read_modules, .. } => {
                        read_modules.clone()
                    }
                    _ => Vec::new(),
                };
                // Java: currModule.complete() — TODO(Completer): the
                // completer chain is not ported; entered modules are complete.
                is_accessible = package_module == Some(curr_module)
                    || visible
                    || package == syms.root_package
                    || (package_module == Some(syms.unnamed_module)
                        && reads.contains(&syms.unnamed_module));
            } else {
                is_accessible = true;
            }
        } else if access == flags::PROTECTED as i64 {
            let toplevel_package = env_toplevel_package(ctx, env);
            let owner = ctx.sym(c).owner.expect("class owner");
            is_accessible = toplevel_package == Some(owner)
                || toplevel_package == symbol_packge(ctx, c)
                || {
                    let encl_sym = env
                        .encl_class
                        .and_then(|class_tree| {
                            class_decl_symbol(ctx, class_tree)
                        })
                        .expect("enclClass symbol");
                    is_inner_sub_class_free(types, ctx, syms, encl_sym, owner)
                };
        } else {
            // error recovery
            is_accessible = true;
        }

        let class_type = ctx.sym(c).ty.expect("class symbol type");
        let no_type = crate::code::symtab::no_type_id(ctx);
        let enclosing = ctx.typ(class_type).get_enclosing_type();
        if !check_inner {
            return is_accessible;
        }
        match enclosing {
            None => is_accessible,
            Some(enclosing) if enclosing == no_type => is_accessible,
            Some(enclosing) => {
                is_accessible && is_accessible_type_free(types, allow_modules, ctx, syms, env, enclosing, check_inner)
            }
        }
    }

    fn is_inner_sub_class_free(
        types: &crate::code::types::Types,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        c: SymbolId,
        base: SymbolId,
    ) -> bool {
        let mut current: Option<SymbolId> = Some(c);
        while let Some(class) = current {
            if symbol_is_sub_class(ctx, &types, syms, class, base) {
                return true;
            }
            current = ctx
                .sym(class)
                .owner
                .and_then(|owner| symbols_encl_class(ctx, owner));
        }
        false
    }

    fn is_accessible_type_free(
        types: &crate::code::types::Types, allow_modules: bool,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        t: TypeId,
        check_inner: bool,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ARRAY) {
            let elemtype = types.elemtype(ctx, syms, t);
            let bound = types.cvar_upper_bound(ctx, elemtype);
            return is_accessible_type_free(types, allow_modules, ctx, syms, env, bound, check_inner);
        }
        // TODO(Types): t.isUnion() / UnionClassType alternative types.
        // Java: isAccessible(env, (Symbol) t.tsym, checkInner) returns true for
        // a null symbol (e.g. the no-type has no tsym).
        let tsym = match ctx.typ(t).tsym {
            Some(tsym) => tsym,
            None => return true,
        };
        is_accessible_class_free(types, allow_modules, ctx, syms, env, tsym, check_inner)
    }