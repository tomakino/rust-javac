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
    pub fn new(
        names: Rc<Names>,
        log: Rc<std::cell::RefCell<Log>>,
        types: Rc<Types>,
        diags: Rc<Factory>,
        source: Source,
        options: Rc<Options>,
    ) -> Self {
        let allow_modules = Feature::MODULES.allowed_in_source(source);
        Resolve {
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
