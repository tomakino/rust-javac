// Origin: com/sun/tools/javac/comp/Resolve.java @ 982d53c (jdk-17.0.19-ga)

//! Name/method resolution (Java: com.sun.tools.javac.comp.Resolve).
//!
//! PORTING_RULES:
//! * the resolution services are injected through the explicit-context adapter
//!   (ADR 0001): Symtab/Types are passed as parameters, not stored global
//!   singletons; the ported Resolve state keeps names/log/types/diags/source/
//!   options only.
//! * the Java overload sets become distinct Rust names (PORTING_RULES §3):
//!   isAccessible(Env, TypeSymbol) -> is_accessible_class,
//!   isAccessible(Env, Type) -> is_accessible_type,
//!   isAccessible(Env, Type, Symbol) -> is_accessible_member.
//! * Resolve.instance(Context) is registered for the Context registration batch.
//!
//! NOT YET PORTED IN THIS FILE (registered; each with unimplemented! + TODO):
//! - MethodResolutionContext / Candidate / method selection (resolveMethod,
//!   resolveConstructor, mostSpecific, ...): stage B of the Resolve batch.
//! - ResolveError hierarchy (ResolveError/SymbolNotFoundError/AccessError/
//!   InvalidSymbolError/InvisibleSymbolError/StaticError/AmbiguityError/
//!   BadRestrictedTypeError/...): error-class stage.
//! - findField/findVar/findMethodInScope/findMethod/findFun, findMemberType,
//!   findType/findTypeVar/findGlobalType, findIdent*, access*, logResolveError:
//!   following stages of the Resolve batch.
//! - verbose resolution diagnostics (VerboseResolutionMode): Options/Log batch.
//! - MethodSymbol.implementation(hiddenIn path): Symbol batch.
//! - Type.isUnion/UnionClassType: Types batch.

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
use crate::ids::{SymbolId, TreeId, TypeId};
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
        }
    }

    /// Java: public static Resolve instance(Context context) (Resolve.java:167)
    /// TODO(Context registration batch): needs the Context service registry.
    pub fn instance() -> Self {
        unimplemented!("TODO(Context registration batch): Resolve.instance")
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
            if self.allow_modules {
                let curr_module = env_toplevel_module(ctx, env).expect("toplevel module");
                // Java: currModule.complete() — the ported symbol complete is a
                // no-op for entered symbols (Completer batch registered).
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
                // Java: currModule.complete() — TODO(Completer batch): the
                // completer chain is not ported; entered modules are complete.
                is_accessible = package_module == Some(curr_module)
                    || visible
                    || package == syms.root_package
                    || (package_module == Some(syms.unnamed_module)
                        && package_module.map(|module| reads.contains(&module)).unwrap_or(false));
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
                    self.is_inner_sub_class(ctx, syms, encl_sym, owner)
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
                is_accessible && self.is_accessible_type(ctx, syms, env, enclosing, check_inner)
            }
        }
    }

    /// Java: private boolean isInnerSubClass(ClassSymbol c, Symbol base)
    /// (Resolve.java:376)
    fn is_inner_sub_class(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        c: SymbolId,
        base: SymbolId,
    ) -> bool {
        let mut current: Option<SymbolId> = Some(c);
        while let Some(class) = current {
            if symbol_is_sub_class(ctx, &self.types, syms, class, base) {
                return true;
            }
            current = ctx
                .sym(class)
                .owner
                .and_then(|owner| symbols_encl_class(ctx, owner));
        }
        false
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
        if ctx.typ(t).has_tag(TypeTag::ARRAY) {
            let elemtype = self.types.elemtype(ctx, syms, t);
            let bound = self.types.cvar_upper_bound(ctx, elemtype);
            return self.is_accessible_type(ctx, syms, env, bound, check_inner);
        }
        // TODO(Types batch): t.isUnion() / UnionClassType alternative types.
        let tsym = ctx.typ(t).tsym.expect("type symbol");
        self.is_accessible_class(ctx, syms, env, tsym, check_inner)
    }

    /// Java: public boolean isAccessible(Env<AttrContext> env, Type site,
    /// Symbol sym) and (Env, Type, Symbol, boolean checkInner)
    /// (Resolve.java:404-460)
    pub fn is_accessible_member(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
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
            let owner_owner = ctx.sym(owner).owner.expect("owner owner");
            (toplevel_package == Some(owner_owner) || toplevel_package == sym_package)
                && self.is_accessible_type(ctx, syms, env, site, check_inner)
                && ctx
                    .typ(site)
                    .tsym
                    .map(|site_tsym| symbol_is_inherited_in(ctx, &self.types, syms, sym, site_tsym))
                    .unwrap_or(false)
                && self.not_overridden_in(ctx, syms, site, sym)
        } else if access == flags::PROTECTED as i64 {
            let owner_owner = ctx.sym(owner).owner.expect("owner owner");
            let select_super = env.info.borrow().select_super;
            let sym_static = (ctx.sym(sym).flags_field & flags::STATIC as i64) != 0;
            let sym_is_type = ctx.sym(sym).kind == Kind::TYP;
            let encl_sym = env
                .encl_class
                .and_then(|class_tree| class_decl_symbol(ctx, class_tree))
                .expect("enclClass symbol");
            (toplevel_package == Some(owner_owner)
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
        _syms: &Symtab,
        site: TypeId,
        sym: SymbolId,
    ) -> bool {
        let is_method = ctx.sym(sym).kind == Kind::MTH;
        let is_constructor = ctx.sym(sym).is_constructor(&self.names);
        let is_static = ctx.sym(sym).is_static(ctx);
        if !is_method || is_constructor || is_static {
            return true;
        }
        // Java: ((MethodSymbol)sym).implementation(site.tsym, types, true)
        // TODO(Symbol batch): MethodSymbol.implementation (binaryImplementation/
        // overrides chain) is not ported yet.
        let _ = site;
        unimplemented!("TODO(Symbol batch): Resolve.notOverriddenIn implementation()")
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
                _ => unreachable!(),
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
        syms: &Symtab,
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
                if !self.is_accessible_class(ctx, syms, env, tsym, true) {
                    // Java: accessBase(new AccessError(env, null, t.tsym),
                    // env.tree.pos(), env.enclClass.sym, t, t.tsym.name, true);
                    unimplemented!(
                        "TODO(Resolve access stage): accessBase/logResolveError for AccessError"
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
            crate::code::symbol::ResolveErrorData::Invalid { sym } => {
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
        }
    }

    /// Java: abstract JCDiagnostic getDiagnostic(DiagnosticType dkind,
    /// DiagnosticPosition pos, Symbol location, Type site, Name name,
    /// List<Type> argtypes, List<Type> typeargtypes)
    #[allow(clippy::too_many_arguments)]
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
        _argtypes: Option<()>,
        _typeargtypes: Option<()>,
    ) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {
        use crate::resources::compiler_properties::{errors, fragments};
        let data = crate::code::symbol::symbol_resolve_error_data(ctx, error)
            .expect("resolve error symbol")
            .clone();
        match data {
            crate::code::symbol::ResolveErrorData::SymbolNotFound => {
                self.symbol_not_found_diagnostic(ctx, syms, dkind, pos, location, site, name)
            }
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
            crate::code::symbol::ResolveErrorData::Access { .. }
            | crate::code::symbol::ResolveErrorData::Invalid { .. }
            | crate::code::symbol::ResolveErrorData::Invisible { .. } => {
                // TODO(Resolve access stage): the full AccessError/InvalidSymbolError
                // diagnostics need the module read/exports reasons and the
                // InvisibleSymbolError SELECT-chain position logic.
                let info = fragments::inapplicable_method(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(
                        "kindname.method",
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, error),
                    ),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, error),
                    ),
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        "resolve error",
                    ),
                );
                Some(self.diags.fragment(info.into()))
            }
        }
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
                std::rc::Rc::new(javac_strings::JavaString::from(arg.0.as_str()))
                    as crate::util::jc_diagnostic::DiagnosticArg
            })
            .collect();
        // TODO(Log batch): source = log.currentSource().
        self.diags
            .create(dkind, None, pos, info.info().code, args)
    }

    /// Java: SymbolNotFoundError.getDiagnostic (Resolve.java:4022-4102)
    #[allow(clippy::too_many_arguments)]
    fn symbol_not_found_diagnostic(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &Symtab,
        dkind: crate::util::jc_diagnostic::DiagnosticType,
        pos: Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>>,
        location: Option<SymbolId>,
        site: Option<TypeId>,
        name: Option<&crate::util::name::Name>,
    ) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {
        let error_name = self.names.error.clone();
        if let Some(name) = name
            && *name == error_name
        {
            return None;
        }
        let location = location
            .or_else(|| site.and_then(|site| ctx.typ(site).tsym))
            .expect("location symbol");
        let location_name = ctx.sym(location).name.clone();
        if !location_name.is_empty()
            && ctx.sym(location).kind == Kind::PCK
            && !crate::code::symbol::symbol_exists(
                ctx,
                site.and_then(|site| ctx.typ(site).tsym).expect("site symbol"),
            )
            && location_name != self.names.java
        {
                let info = crate::resources::compiler_properties::errors::doesnt_exist(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_display_name(ctx, location),
                    ),
                );
            return Some(self.factory_error(dkind, pos, info));
        }
        // TODO(Resolve access stage): the cant.resolve.* dynamic keys with the
        // full location/args parameter set (the generated Errors accessors for
        // the dynamic key form do not exist; the key is built upstream by
        // getErrorKey and rendered through diags.create with the kindname/name/
        // typeargs/args/location arguments).
        let kindname = crate::code::kinds::KindName::CONSTRUCTOR;
        let _ = kindname;
        unimplemented!(
            "TODO(Resolve access stage): SymbolNotFoundError.getDiagnostic dynamic cant.resolve keys"
        )
    }
}

/// Java: Symbol.toString-like display for the diagnostic argument placeholders
/// (the generated SymbolArg is an opaque display placeholder before P4).
fn symbol_display_name(
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
        syms: &Symtab,
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
            let candidates = ctx.scope_get_symbols_by_name(
                members,
                name,
                crate::code::scope::LookupKind::RECURSIVE,
                &|_| true,
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
        syms: &Symtab,
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
                let candidates = ctx.scope_get_symbols_by_name(
                    scope,
                    name,
                    crate::code::scope::LookupKind::RECURSIVE,
                    &|_| true,
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
            let candidates = ctx.scope_get_symbols_by_name(
                import_scope,
                name,
                crate::code::scope::LookupKind::RECURSIVE,
                &|_| true,
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
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: &crate::util::name::Name,
        c: SymbolId,
    ) -> SymbolId {
        let members = ctx.sym(c).members();
        if let Some(members) = members {
            let candidates = ctx.scope_get_symbols_by_name(
                members,
                name,
                crate::code::scope::LookupKind::RECURSIVE,
                &|_| true,
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
        syms: &Symtab,
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
        syms: &Symtab,
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
        let candidates = ctx.scope_get_symbols_by_name(
            scope,
            name,
            crate::code::scope::LookupKind::RECURSIVE,
            &|_| true,
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
        _syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        site: Option<TypeId>,
        sym: SymbolId,
    ) -> SymbolId {
        let snapshot = resolve_env_snapshot(ctx, env);
        let data = crate::code::symbol::ResolveErrorData::Access {
            env: snapshot,
            site,
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
    /// no local consumer yet; registered §9 adapter).
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

/// Java: Symbol.name.isEmpty() comparison helper.
pub(crate) fn name_equals(
    _names: &crate::util::names::Names,
    left: &crate::util::name::Name,
    right: &crate::util::name::Name,
) -> bool {
    left == right
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
        let Some(finder) = self.finder.clone() else {
            unimplemented!(
                "TODO(ClassFinder batch): Resolve.loadClass needs the ClassFinder service"
            )
        };
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
                // TODO(ClassFinder batch): the BadClassFile distinction is not
                // representable in the local CompletionFailure; recovery path.
                match recovery {
                    RecoveryLoadClass::NoRecovery | RecoveryLoadClass::DoRecoveryLoadClass => {
                        // TODO(Modules batch): doRecoveryLoadClass needs
                        // syms.getClassesForName/lookupInvisibleSymbol/moduleFinder.
                        if recovery == RecoveryLoadClass::DoRecoveryLoadClass {
                            unimplemented!(
                                "TODO(Modules batch): Resolve.doRecoveryLoadClass"
                            )
                        }
                        self.type_not_found
                    }
                    RecoveryLoadClass::NamedImportScopeRecovery
                    | RecoveryLoadClass::StarImportScopeRecovery => {
                        self.recovery_import_scope_lookup(ctx, syms, env, name, recovery)
                    }
                }
            }
        }
    }

    /// Java: namedImportScopeRecovery/starImportScopeRecovery (Resolve.java
    /// 2087-2122): importScope.findFirst(Convert.shortName(name), predicate).
    fn recovery_import_scope_lookup(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &Symtab,
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
        let candidates = ctx.scope_get_symbols_by_name(
            import_scope,
            &short,
            crate::code::scope::LookupKind::RECURSIVE,
            &|_| true,
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
                // TODO(Modules batch): lookupInvisibleSymbol (module scanning +
                // syms.getAllModules/getPackagesForName) is not ported.
                unimplemented!("TODO(Modules batch): Resolve.lookupInvisibleSymbol")
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
                } else if pos.is_some() {
                    // TODO(Log batch): log.warning(pos, Warnings.IllegalRefTo
                    // RestrictedType(names.yield)) — the local Log has no
                    // warning(Position) adapter yet.
                    unimplemented!("TODO(Log batch): Resolve.checkRestrictedType warning")
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
        argtypes: Option<crate::util::list::List<TypeId>>,
        typeargtypes: Option<crate::util::list::List<TypeId>>,
        log_resolve_helper: LogResolveHelper,
    ) -> SymbolId {
        let _ = syms;
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
                // TODO(DeferredAttr batch): ResolveDeferredRecoveryMap needs the
                // DeferredAttr machinery.
                unimplemented!(
                    "TODO(DeferredAttr batch): LogResolveHelper.getArgumentTypes (ResolveDeferredRecoveryMap)"
                )
            }
            if self.resolve_diagnostic_needed(
                ctx,
                log_resolve_helper,
                site,
                argtypes.clone(),
                None,
            ) {
                self.log_resolve_error(ctx, syms, err_sym, pos, location, site, name, argtypes, None);
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
            argtypes.map(|_| ()),
            typeargtypes.map(|_| ()),
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
            None,
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
            LogResolveHelper::Basic,
        )
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

    /// Java: public Symbol mergeResults(Symbol prev, Symbol sym)
    /// TODO(Resolve batch B): the VARARITY override inspects the Kinds
    /// resolution-error kinds (WRONG_MTH/WRONG_MTHS/ABSENT_MTH), which are not
    /// ported yet.
    pub fn merge_results(self, _prev: SymbolId, _sym: SymbolId) -> SymbolId {
        unimplemented!("TODO(Resolve batch B): MethodResolutionPhase.mergeResults")
    }
}
