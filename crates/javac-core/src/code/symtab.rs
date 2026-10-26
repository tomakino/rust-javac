// Origin: com/sun/tools/javac/code/Symtab.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration supplement (method-level audit):
// Dependency/phase: P3 semantic services (Enter/Attr/Resolve/Check or ClassReader; see docs/PLAN.md).
// - NOT PORTED: isTerminal (Symtab.java:324)
// - NOT PORTED: isTerminal (Symtab.java:358)
// - NOT PORTED: accept (Symtab.java:447)
// - NOT PORTED: getAllModules (Symtab.java:871)
// - NOT PORTED: getClassesForName (Symtab.java:875)
// - NOT PORTED: getPackagesForName (Symtab.java:879)
// End of §19 gap registration supplement.

//! A class that defines all predefined constants and operators as well as
//! special classes such as java.lang.Object, which need to be known to the
//! compiler. All symbols are held in instance fields. This makes it possible to
//! work in multiple concurrent projects, which might use different class files
//! for library classes.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs):
//! - the Java object fields become arena IDs (SymbolId/TypeId/DirectiveId/
//!   ScopeId); the class/package/module tables keep their Java shape (name ->
//!   module -> symbol maps, expressed as Vec rows because Name/SymbolId are not
//!   hashable by identity across name tables);
//! - the Java constructors that register the instance in the Context and the
//!   binding of completers are adapted: Symtab::new takes the CompilerContext
//!   and the ported Names explicitly (the Names.instance(Context) adapter is not
//!   ported yet); the initial/module completer bindings are installed by the
//!   bootstrap through set_initial_completer/set_module_completer (CompleterRef
//!   factories; registered adapter, no Context singletons);
//! - the anonymous ModuleSymbol/PackageSymbol subclasses that override toString
//!   (localized "unnamed module"/"unnamed package") cannot be expressed without
//!   the Symbol batch; the ported symbols are plain ModuleSymbol/PackageSymbol
//!   values (registered; the localized toString is a Symbol batch TODO);
//! - formFlatName/formFullName (Symbol.TypeSymbol/ClassSymbol statics) are not
//!   ported in symbol.rs yet; they are reproduced here as helpers and must move
//!   into symbol.rs with the Symbol batch (registered).
//!
//! NOT PORTED IN THIS STEP (dependencies not ported yet; no substitute logic):
//! - Modules (module completer): the bootstrap may pass the module completer
//!   explicitly; the java.base completer bootstrap (Symtab.java:637) is applied
//!   by set_module_completer; the anonymous Completers of
//!   synthesizeEmptyInterfaceIfMissing / synthesizeBoxTypeIfMissing /
//!   enterSyntheticAnnotation and the four synthetic annotation types
//!   (previewFeatureInternalType, valueBasedInternalType, proprietaryType,
//!   profileType) are therefore Option<TypeId> = None;
//! - MissingInfoHandler: RootPackageSymbol is created without the missing info
//!   handler;
//! - Types: the minimal prefix ported in types.rs (hashCode/isSameType/
//!   erasure/boxedClass) is used by classFields/getClassField; the remaining
//!   Types methods are NOT PORTED (TODO Types batch; see types.rs).
//! - ModuleSymbol.isNoModule(): no_module is a plain ModuleSymbol (the upstream
//!   anonymous override is a Symbol batch TODO);
//! - the JavacMessages-backed toString of the unnamed module/package (see the
//!   anonymous-subclass note above).

use std::collections::HashMap;
use crate::code::symbol::CompleterFactory;
use std::rc::Rc;

use crate::code::directive::{Directive, RequiresFlag};
use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::scope::Scope;
use crate::code::source::{Feature, Source};
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::type_::{Type, TypeKindData};
use crate::code::types::Types;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::{DirectiveId, ScopeId, SymbolId, TypeId};
use crate::jvm::target::Target;
use crate::util::convert;
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::names::Names;

/// The Java class/package/module tables: name -> module -> symbol.
struct SymtabMaps {
    classes: Vec<(Name, Vec<(SymbolId, SymbolId)>)>,
    packages: Vec<(Name, Vec<(SymbolId, SymbolId)>)>,
    modules: Vec<(Name, SymbolId)>,
}

impl SymtabMaps {
    fn new() -> SymtabMaps {
        SymtabMaps {
            classes: Vec::new(),
            packages: Vec::new(),
            modules: Vec::new(),
        }
    }
}

fn name_of(names: &Names, text: &str) -> Name {
    names.table.from_string(&JavaString::from(text))
}

fn append_char(base: &Name, ch: u8, suffix: &Name) -> Name {
    base.append_char(ch, suffix)
}

/// Java: Symbol.TypeSymbol.formFlatName(Name name, Symbol owner)
pub(crate) fn form_flat_name(ctx: &CompilerContext, name: &Name, owner: Option<SymbolId>) -> Name {
    let Some(owner) = owner else {
        return name.clone();
    };
    let owner_kind = ctx.symbol_kind(owner);
    if owner_kind == Kind::VAR
        || owner_kind == Kind::MTH
        || (owner_kind == Kind::TYP
            && ctx
                .sym(owner)
                .ty
                .map(|ty| ctx.typ(ty).has_tag(TypeTag::TYPEVAR))
                .unwrap_or(false))
    {
        return name.clone();
    }
    let sep = if owner_kind == Kind::TYP { b'$' } else { b'.' };
    let prefix = match &ctx.sym(owner).kind_data {
        SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
        // Java: PackageSymbol.flatName() returns the package fullname.
        SymbolKind::PackageSymbol { fullname, .. }
        | SymbolKind::RootPackageSymbol { fullname, .. } => fullname.clone(),
        _ => ctx.sym(owner).name.clone(),
    };
    if prefix.is_empty() {
        name.clone()
    } else {
        append_char(&prefix, sep, name)
    }
}

/// Java: Symbol.ClassSymbol.formFullName(Name name, Symbol owner)
pub(crate) fn form_full_name(ctx: &CompilerContext, name: &Name, owner: Option<SymbolId>) -> Name {
    let Some(owner) = owner else {
        return name.clone();
    };
    let owner_kind = ctx.symbol_kind(owner);
    if owner_kind != Kind::ERR
        && (owner_kind == Kind::VAR
            || owner_kind == Kind::MTH
            || (owner_kind == Kind::TYP
                && ctx
                    .sym(owner)
                    .ty
                    .map(|ty| ctx.typ(ty).has_tag(TypeTag::TYPEVAR))
                    .unwrap_or(false)))
    {
        return name.clone();
    }
    let prefix = match &ctx.sym(owner).kind_data {
        SymbolKind::ClassSymbol { fullname, .. }
        | SymbolKind::PackageSymbol { fullname, .. }
        | SymbolKind::RootPackageSymbol { fullname, .. } => fullname.clone(),
        _ => ctx.sym(owner).name.clone(),
    };
    if prefix.is_empty() {
        name.clone()
    } else {
        append_char(&prefix, b'.', name)
    }
}

/// Java: PackageSymbol.exists()
fn package_exists(ctx: &CompilerContext, pack: SymbolId) -> bool {
    (ctx.sym(pack).flags() & flags::EXISTS as i64) != 0
}

fn new_class_type(ctx: &mut CompilerContext, no_type: TypeId) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::CLASS,
        None,
        TypeKindData::ClassType {
            outer_field: Some(no_type),
            typarams_field: None,
            allparams_field: None,
            supertype_field: None,
            interfaces_field: None,
            all_interfaces_field: None,
        },
    ))
}

/// Java: new ClassSymbol(flags, name, owner) - the constructor that installs a
/// fresh ClassType (new ClassType(Type.noType, null, null)) as the symbol type.
pub(crate) fn new_class_symbol(
    ctx: &mut CompilerContext,
    flags_value: i64,
    name: Name,
    owner: Option<SymbolId>,
    no_type: TypeId,
) -> SymbolId {
    let fullname = form_full_name(ctx, &name, owner);
    let flatname = form_flat_name(ctx, &name, owner);
    let class_type = new_class_type(ctx, no_type);
    let symbol = ctx.alloc_symbol(Symbol::new(
        Kind::TYP,
        flags_value,
        name,
        Some(class_type),
        owner,
        SymbolKind::ClassSymbol {
            members_field: None,
            fullname,
            flatname,
            sourcefile: None,
            classfile: None,
            trans_local: List::nil(),
            record_components: List::nil(),
            permitted: List::nil(),
            is_permitted_explicit: false,
        },
    ));
    ctx.typ_mut(class_type).tsym = Some(symbol);
    symbol
}

/// Java: new ClassSymbol(flags, name, type, owner)
fn new_class_symbol_with_type(
    ctx: &mut CompilerContext,
    flags_value: i64,
    name: Name,
    ty: Option<TypeId>,
    owner: Option<SymbolId>,
) -> SymbolId {
    let fullname = form_full_name(ctx, &name, owner);
    let flatname = form_flat_name(ctx, &name, owner);
    ctx.alloc_symbol(Symbol::new(
        Kind::TYP,
        flags_value,
        name,
        ty,
        owner,
        SymbolKind::ClassSymbol {
            members_field: None,
            fullname,
            flatname,
            sourcefile: None,
            classfile: None,
            trans_local: List::nil(),
            record_components: List::nil(),
            permitted: List::nil(),
            is_permitted_explicit: false,
        },
    ))
}

fn new_type_symbol(
    ctx: &mut CompilerContext,
    kind: Kind,
    flags_value: i64,
    name: Name,
    ty: Option<TypeId>,
    owner: Option<SymbolId>,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        kind,
        flags_value,
        name,
        ty,
        owner,
        SymbolKind::TypeSymbol,
    ))
}

fn new_package_symbol(
    ctx: &mut CompilerContext,
    flags_value: i64,
    name: Name,
    owner: Option<SymbolId>,
) -> SymbolId {
    let fullname = form_full_name(ctx, &name, owner);
    ctx.alloc_symbol(Symbol::new(
        Kind::PCK,
        flags_value,
        name,
        None,
        owner,
        SymbolKind::PackageSymbol {
            members_field: None,
            fullname,
            package_info: None,
            modle: None,
            sourcefile: None,
        },
    ))
}

fn new_root_package_symbol(
    ctx: &mut CompilerContext,
    name: Name,
    allow_private_invoke_virtual: bool,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        Kind::PCK,
        0,
        name.clone(),
        None,
        None,
        SymbolKind::RootPackageSymbol {
            members_field: None,
            fullname: name,
            package_info: None,
            modle: None,
            sourcefile: None,
            allow_private_invoke_virtual,
        },
    ))
}

fn new_module_symbol(
    ctx: &mut CompilerContext,
    name: Name,
    requires: List<DirectiveId>,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        Kind::MDL,
        0,
        name.clone(),
        None,
        None,
        SymbolKind::ModuleSymbol {
            version: None,
            source_location: None,
            class_location: None,
            patch_location: None,
            patch_output_location: None,
            directives: List::nil(),
            requires,
            exports: List::nil(),
            opens: List::nil(),
            provides: List::nil(),
            uses: List::nil(),
            module_info: None,
            unnamed_package: None,
            visible_packages: HashMap::new(),
            read_modules: Vec::new(),
            enclosed_packages: List::nil(),
            flags: Vec::new(),
            resolution_flags: Vec::new(),
        },
    ))
}

fn new_requires_directive(
    ctx: &mut CompilerContext,
    module: SymbolId,
    requires_flags: Vec<RequiresFlag>,
) -> DirectiveId {
    ctx.alloc_directive(Directive::Requires {
        module: Some(module),
        flags: requires_flags,
    })
}

fn new_var_symbol(
    ctx: &mut CompilerContext,
    flags_value: i64,
    name: Name,
    ty: Option<TypeId>,
    owner: Option<SymbolId>,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        Kind::VAR,
        flags_value,
        name,
        ty,
        owner,
        SymbolKind::VarSymbol {
            pos: 0,
            adr: -1,
            data_kind: None,
            data: None,
        },
    ))
}

fn new_method_symbol(
    ctx: &mut CompilerContext,
    flags_value: i64,
    name: Name,
    ty: Option<TypeId>,
    owner: Option<SymbolId>,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        Kind::MTH,
        flags_value,
        name,
        ty,
        owner,
        SymbolKind::MethodSymbol {
            extra_params: List::nil(),
            captured_locals: List::nil(),
            params: List::nil(),
        },
    ))
}

fn new_method_type(
    ctx: &mut CompilerContext,
    argtypes: List<TypeId>,
    restype: Option<TypeId>,
    thrown: List<TypeId>,
    method_class: SymbolId,
) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::METHOD,
        Some(method_class),
        TypeKindData::MethodType {
            argtypes,
            restype,
            thrown,
            recvtype: None,
        },
    ))
}

fn set_members(ctx: &mut CompilerContext, symbol: SymbolId, scope: ScopeId) {
    match &mut ctx.sym_mut(symbol).kind_data {
        SymbolKind::ClassSymbol { members_field, .. }
        | SymbolKind::PackageSymbol { members_field, .. }
        | SymbolKind::RootPackageSymbol { members_field, .. } => *members_field = Some(scope),
        _ => panic!("symbol has no members field"),
    }
}

fn new_scope(ctx: &mut CompilerContext, owner: SymbolId) -> ScopeId {
    ctx.alloc_scope(Scope::new_scope_impl(Some(owner)))
}

fn new_error_scope(ctx: &mut CompilerContext, owner: SymbolId) -> ScopeId {
    ctx.alloc_scope(Scope::new_error_scope(Some(owner)))
}

fn set_module_unnamed_package(ctx: &mut CompilerContext, module: SymbolId, package: SymbolId) {
    if let SymbolKind::ModuleSymbol {
        unnamed_package, ..
    } = &mut ctx.sym_mut(module).kind_data
    {
        *unnamed_package = Some(package);
    }
}

fn append_enclosed_package(ctx: &mut CompilerContext, module: SymbolId, package: SymbolId) {
    if let SymbolKind::ModuleSymbol {
        enclosed_packages, ..
    } = &mut ctx.sym_mut(module).kind_data
    {
        *enclosed_packages = enclosed_packages.prepend(package);
    }
}

fn get_package_in(maps: &SymtabMaps, module: SymbolId, fullname: &Name) -> Option<SymbolId> {
    maps.packages
        .iter()
        .find(|(name, _)| name.to_java_string() == fullname.to_java_string())
        .and_then(|(_, modules)| {
            modules
                .iter()
                .find(|(candidate, _)| *candidate == module)
                .map(|(_, package)| *package)
        })
}

fn do_enter_package(
    ctx: &mut CompilerContext,
    maps: &mut SymtabMaps,
    module: SymbolId,
    package: SymbolId,
) {
    let fullname = {
        let node = ctx.sym(package);
        match &node.kind_data {
            SymbolKind::PackageSymbol { fullname, .. }
            | SymbolKind::RootPackageSymbol { fullname, .. } => fullname.clone(),
            _ => node.name.clone(),
        }
    };
    if let Some((_, modules)) = maps
        .packages
        .iter_mut()
        .find(|(name, _)| name.to_java_string() == fullname.to_java_string())
    {
        if let Some(entry) = modules.iter_mut().find(|(candidate, _)| *candidate == module) {
            entry.1 = package;
        } else {
            modules.push((module, package));
        }
    } else {
        maps.packages.push((fullname, vec![(module, package)]));
    }
    append_enclosed_package(ctx, module, package);
}

fn add_root_package_for(
    ctx: &mut CompilerContext,
    names: &Names,
    maps: &mut SymtabMaps,
    module: SymbolId,
    root_package: SymbolId,
) {
    do_enter_package(ctx, maps, module, root_package);
    let unnamed_package = new_package_symbol(ctx, 0, names.std.empty.clone(), Some(root_package));
    // Java: unnamedPackage.completer = s -> initialCompleter.complete(s);
    // NOT PORTED (Completer is not ported).
    if let SymbolKind::PackageSymbol { modle, .. } = &mut ctx.sym_mut(unnamed_package).kind_data {
        *modle = Some(module);
    }
    ctx.sym_mut(unnamed_package).flags_field |= flags::EXISTS as i64;
    set_module_unnamed_package(ctx, module, unnamed_package);
}

fn get_class_in(maps: &SymtabMaps, module: SymbolId, flat_name: &Name) -> Option<SymbolId> {
    maps.classes
        .iter()
        .find(|(name, _)| name.to_java_string() == flat_name.to_java_string())
        .and_then(|(_, modules)| {
            modules
                .iter()
                .find(|(candidate, _)| *candidate == module)
                .map(|(_, class)| *class)
        })
}

fn do_enter_class(maps: &mut SymtabMaps, module: SymbolId, class: SymbolId, flatname: Name) {
    if let Some((_, modules)) = maps
        .classes
        .iter_mut()
        .find(|(name, _)| name.to_java_string() == flatname.to_java_string())
    {
        if let Some(entry) = modules.iter_mut().find(|(candidate, _)| *candidate == module) {
            entry.1 = class;
        } else {
            modules.push((module, class));
        }
    } else {
        maps.classes.push((flatname, vec![(module, class)]));
    }
}

/// Java: private ClassSymbol defineClass(Name name, Symbol owner)
fn define_class(
    ctx: &mut CompilerContext,
    names: &Names,
    name: Name,
    owner: Option<SymbolId>,
) -> SymbolId {
    let no_type = no_type_id(ctx);
    // Java: ClassSymbol c = new ClassSymbol(0, name, owner);
    let class = new_class_symbol(ctx, 0, name, owner, no_type);
    // Java: c.completer = initialCompleter; - NOT PORTED (Completer).
    let _ = names;
    class
}

/// Java: ClassSymbol.complete() error recovery: new ErrorType(this, Type.noType).
pub(crate) fn new_error_type_for_class(ctx: &mut CompilerContext, tsym: SymbolId) -> TypeId {
    let no_type = no_type_id(ctx);
    ctx.alloc_type(Type::new(
        TypeTag::ERROR,
        Some(tsym),
        TypeKindData::ErrorType {
            outer_field: None,
            typarams_field: None,
            allparams_field: None,
            supertype_field: None,
            interfaces_field: None,
            all_interfaces_field: None,
            original_type: Some(no_type),
        },
    ))
}

pub(crate) fn no_type_id(ctx: &mut CompilerContext) -> TypeId {
    ctx.alloc_type(Type::new(TypeTag::NONE, None, TypeKindData::JCNoType))
}

/// Java: public ClassSymbol enterClass(ModuleSymbol msym, Name name,
/// TypeSymbol owner)
fn enter_class_full(
    ctx: &mut CompilerContext,
    names: &Names,
    maps: &mut SymtabMaps,
    msym: SymbolId,
    name: Name,
    owner: Option<SymbolId>,
) -> SymbolId {
    let flatname = form_flat_name(ctx, &name, owner);
    if let Some(class) = get_class_in(maps, msym, &flatname) {
        // reassign fields of classes that might have been loaded with their
        // flat names.
        let needs_reassign = {
            let node = ctx.sym(class);
            let same_name = node.name.to_java_string() == name.to_java_string();
            let same_owner = node.owner == owner;
            let owner_is_type = owner
                .map(|owner| ctx.symbol_kind(owner) == Kind::TYP)
                .unwrap_or(false);
            let class_owner_is_package = node
                .owner
                .map(|class_owner| ctx.symbol_kind(class_owner) == Kind::PCK)
                .unwrap_or(false);
            (!same_name || !same_owner) && owner_is_type && class_owner_is_package
        };
        if needs_reassign {
            if let Some(class_owner) = ctx.sym(class).owner {
                let members = ensure_members_scope(ctx, class_owner).expect("owner members");
                ctx.scope_remove(members, class);
            }
            let fullname = form_full_name(ctx, &name, owner);
            let node = ctx.sym_mut(class);
            node.owner = owner;
            node.name = name;
            if let SymbolKind::ClassSymbol {
                fullname: slot, ..
            } = &mut node.kind_data
            {
                *slot = fullname;
            }
        }
        return class;
    }
    let class = {
        let no_type = no_type_id(ctx);
        new_class_symbol(ctx, 0, name, owner, no_type)
    };
    let class_name = ctx.sym(class).name.clone();
    let flatname = form_flat_name(ctx, &class_name, owner);
    do_enter_class(maps, msym, class, flatname);
    let _ = names;
    class
}

fn members_scope(ctx: &CompilerContext, symbol: SymbolId) -> Option<ScopeId> {
    match &ctx.sym(symbol).kind_data {
        SymbolKind::ClassSymbol { members_field, .. }
        | SymbolKind::PackageSymbol { members_field, .. }
        | SymbolKind::RootPackageSymbol { members_field, .. } => *members_field,
        _ => None,
    }
}

/// Java: PackageSymbol.complete() lazily creates members_field =
/// WriteableScope.create(this); the rest of complete() is not ported
/// (PackageSymbol completer TODO), the members scope is created here.
fn ensure_members_scope(ctx: &mut CompilerContext, symbol: SymbolId) -> Option<ScopeId> {
    if let Some(members) = members_scope(ctx, symbol) {
        return Some(members);
    }
    match &ctx.sym(symbol).kind_data {
        SymbolKind::PackageSymbol { .. } | SymbolKind::RootPackageSymbol { .. } => {
            let members = ctx.scope_create(Some(symbol));
            if let SymbolKind::PackageSymbol { members_field, .. }
            | SymbolKind::RootPackageSymbol { members_field, .. } =
                &mut ctx.sym_mut(symbol).kind_data
            {
                *members_field = Some(members);
            }
            Some(members)
        }
        _ => None,
    }
}
/// Java: private Type enterClass(String s) { return
/// enterClass(java_base, names.fromString(s)).type; }
fn enter_class_named(
    ctx: &mut CompilerContext,
    names: &Names,
    maps: &mut SymtabMaps,
    java_base: SymbolId,
    text: &str,
) -> TypeId {
    // Java: Symtab constructor uses enterClass(javaBase, flatname), which is
    // lookupPackage(msym, packagePart) + defineClass(shortName, package).
    // PORTING_RULES: the local enterClass(msym, flatname) overload is on the
    // Symtab value; this free helper is called from the constructor, so the
    // same two steps are performed here (owner = the entered package).
    let flat = name_of(names, text);
    let package = enter_package(ctx, names, maps, java_base, convert::package_part(&flat));
    let class = enter_class_full(
        ctx,
        names,
        maps,
        java_base,
        convert::short_name(&flat),
        Some(package),
    );
    ctx.sym(class).ty.expect("class symbol type")
}

/// Java: private PackageSymbol lookupPackage(ModuleSymbol msym, Name flatname,
/// boolean onlyExisting)
// The extra parameters are the ported Symtab state (unnamed/no module) passed
// explicitly because the Rust free function cannot read the Java instance
// fields of the caller (registered section 9).
#[allow(clippy::too_many_arguments)]
fn lookup_package_impl(
    ctx: &mut CompilerContext,
    names: &Names,
    maps: &mut SymtabMaps,
    module: SymbolId,
    flatname: &Name,
    only_existing: bool,
    unnamed_module: SymbolId,
    no_module: SymbolId,
) -> SymbolId {
    if flatname.is_empty() {
        return match &ctx.sym(module).kind_data {
            SymbolKind::ModuleSymbol {
                unnamed_package, ..
            } => unnamed_package.expect("module unnamed package"),
            _ => panic!("module expected"),
        };
    }
    if module == no_module {
        return enter_package(ctx, names, maps, module, flatname.clone());
    }
    // Java: msym.complete(); - NOT PORTED (Completer).
    if let Some(visible) = visible_package(ctx, module, flatname) {
        return visible;
    }
    let existing = get_package_in(maps, module, flatname);
    if let Some(package) = existing
        && (package_exists(ctx, package) || only_existing)
    {
        return package;
    }
    let depends_on_unnamed = module_reads_module(ctx, module, unnamed_module);
    if depends_on_unnamed {
        if let Some(unnamed_package) = get_package_in(maps, unnamed_module, flatname)
            && package_exists(ctx, unnamed_package)
        {
            set_visible_package(ctx, module, unnamed_package);
            return unnamed_package;
        }
        let package = enter_package(ctx, names, maps, module, flatname.clone());
        if package_exists(ctx, package) {
            return package;
        }
        let unnamed_package = enter_package(ctx, names, maps, unnamed_module, flatname.clone());
        if package_exists(ctx, unnamed_package) {
            set_visible_package(ctx, module, unnamed_package);
            return unnamed_package;
        }
        return package;
    }
    enter_package(ctx, names, maps, module, flatname.clone())
}

fn visible_package(ctx: &CompilerContext, module: SymbolId, flatname: &Name) -> Option<SymbolId> {
    match &ctx.sym(module).kind_data {
        SymbolKind::ModuleSymbol {
            visible_packages, ..
        } => visible_packages
            .iter()
            .find(|(name, _)| name.to_java_string() == flatname.to_java_string())
            .map(|(_, package)| *package),
        _ => None,
    }
}

fn set_visible_package(ctx: &mut CompilerContext, module: SymbolId, package: SymbolId) {
    let fullname = match &ctx.sym(package).kind_data {
        SymbolKind::PackageSymbol { fullname, .. }
        | SymbolKind::RootPackageSymbol { fullname, .. } => fullname.clone(),
        _ => ctx.sym(package).name.clone(),
    };
    if let SymbolKind::ModuleSymbol {
        visible_packages, ..
    } = &mut ctx.sym_mut(module).kind_data
    {
        visible_packages.insert(fullname, package);
    }
}

fn module_reads_module(ctx: &CompilerContext, module: SymbolId, other: SymbolId) -> bool {
    let requires = match &ctx.sym(module).kind_data {
        SymbolKind::ModuleSymbol { requires, .. } => requires.clone(),
        _ => return false,
    };
    for directive in requires.iter() {
        if let Directive::Requires { module: Some(read), .. } = ctx.directive(directive)
            && *read == other
        {
            return true;
        }
    }
    false
}

fn get_module_in(maps: &SymtabMaps, name: &Name) -> Option<SymbolId> {
    maps.modules
        .iter()
        .find(|(candidate, _)| candidate.to_java_string() == name.to_java_string())
        .map(|(_, module)| *module)
}

/// Java: public ModuleSymbol enterModule(Name name)
fn enter_module(
    ctx: &mut CompilerContext,
    names: &Names,
    maps: &mut SymtabMaps,
    name: Name,
    root_package: SymbolId,
) -> SymbolId {
    if let Some(module) = get_module_in(maps, &name) {
        return module;
    }
    let module = new_module_symbol(ctx, name.clone(), List::nil());
    // Java: ModuleSymbol.create() also installs a module-info ClassSymbol.
    let module_info = new_class_symbol_with_type(
        ctx,
        flags::MODULE,
        names.std.module_info.clone(),
        None,
        Some(module),
    );
    if let SymbolKind::ModuleSymbol {
        module_info: slot, ..
    } = &mut ctx.sym_mut(module).kind_data
    {
        *slot = Some(module_info);
    }
    add_root_package_for(ctx, names, maps, module, root_package);
    // Java: msym.completer = s -> moduleCompleter.complete(s); - NOT PORTED.
    maps.modules.push((name, module));
    module
}

/// Java: public PackageSymbol enterPackage(ModuleSymbol currModule, Name
/// fullname)
fn enter_package(
    ctx: &mut CompilerContext,
    names: &Names,
    maps: &mut SymtabMaps,
    module: SymbolId,
    fullname: Name,
) -> SymbolId {
    if let Some(package) = get_package_in(maps, module, &fullname) {
        return package;
    }
    assert!(
        !fullname.is_empty(),
        "AssertionError: rootPackage missing!; currModule: {module:?}"
    );
    let short_name = convert::short_name(&fullname);
    let parent = convert::package_part(&fullname);
    let parent_package = enter_package(ctx, names, maps, module, parent);
    let package = new_package_symbol(ctx, 0, short_name, Some(parent_package));
    // Java: p.completer = initialCompleter; - NOT PORTED.
    if let SymbolKind::PackageSymbol { modle, .. } = &mut ctx.sym_mut(package).kind_data {
        *modle = Some(module);
    }
    let _ = names;
    do_enter_package(ctx, maps, module, package);
    package
}

/// A class that defines all predefined constants and operators as well as
/// special classes such as java.lang.Object.
///
/// Java: public class Symtab
pub struct Symtab {
    pub byte_type: TypeId,
    pub char_type: TypeId,
    pub short_type: TypeId,
    pub int_type: TypeId,
    pub long_type: TypeId,
    pub float_type: TypeId,
    pub double_type: TypeId,
    pub boolean_type: TypeId,
    pub bot_type: TypeId,
    pub void_type: TypeId,
    pub unnamed_module: SymbolId,
    pub err_module: SymbolId,
    pub no_module: SymbolId,
    pub root_package: SymbolId,
    pub no_symbol: SymbolId,
    pub err_symbol: SymbolId,
    pub unknown_symbol: SymbolId,
    pub err_type: TypeId,
    pub unknown_type: TypeId,
    /// Java: Type.stuckType (a NONE type used as the type of stuck trees)
    pub stuck_type: TypeId,
    /// Java: Type.recoveryType (a NONE type used by recovery attribution)
    pub recovery_type: TypeId,
    pub array_class: SymbolId,
    pub array_clone_method: SymbolId,
    pub bound_class: SymbolId,
    pub method_class: SymbolId,
    pub java_base: SymbolId,
    pub predef_class: SymbolId,
    pub object_type: TypeId,
    pub object_methods_type: TypeId,
    pub objects_type: TypeId,
    pub class_type: TypeId,
    pub class_loader_type: TypeId,
    pub string_type: TypeId,
    pub string_buffer_type: TypeId,
    pub string_builder_type: TypeId,
    pub cloneable_type: TypeId,
    pub serializable_type: TypeId,
    pub serialized_lambda_type: TypeId,
    pub var_handle_type: TypeId,
    pub method_handle_type: TypeId,
    pub method_handle_lookup_type: TypeId,
    pub method_type_type: TypeId,
    pub native_header_type: TypeId,
    pub throwable_type: TypeId,
    pub error_type: TypeId,
    pub interrupted_exception_type: TypeId,
    pub illegal_argument_exception_type: TypeId,
    pub exception_type: TypeId,
    pub runtime_exception_type: TypeId,
    pub class_not_found_exception_type: TypeId,
    pub no_class_def_found_error_type: TypeId,
    pub no_such_field_error_type: TypeId,
    pub assertion_error_type: TypeId,
    pub incompatible_class_change_error_type: TypeId,
    pub clone_not_supported_exception_type: TypeId,
    pub annotation_type: TypeId,
    pub enum_sym: SymbolId,
    pub list_type: TypeId,
    pub collections_type: TypeId,
    pub comparable_type: TypeId,
    pub comparator_type: TypeId,
    pub arrays_type: TypeId,
    pub iterable_type: TypeId,
    pub iterator_type: TypeId,
    pub annotation_target_type: TypeId,
    pub override_type: TypeId,
    pub retention_type: TypeId,
    pub deprecated_type: TypeId,
    pub suppress_warnings_type: TypeId,
    pub supplier_type: TypeId,
    pub inherited_type: TypeId,
    /// NOT PORTED: needs the anonymous Completer of the upstream synthesize
    /// step (see the module doc).
    pub profile_type: Option<TypeId>,
    /// NOT PORTED: see profile_type.
    pub proprietary_type: Option<TypeId>,
    /// Java: private final Completer initialCompleter (bound by the bootstrap)
    pub initial_completer: Option<CompleterFactory>,
    /// Java: private final Completer moduleCompleter (from Modules; not ported -> None)
    pub module_completer: Option<CompleterFactory>,
    pub system_type: TypeId,
    pub auto_closeable_type: TypeId,
    pub trust_me_type: TypeId,
    pub lambda_metafactory: TypeId,
    pub string_concat_factory: TypeId,
    pub repeatable_type: TypeId,
    pub documented_type: TypeId,
    pub element_type_type: TypeId,
    pub functional_interface_type: TypeId,
    pub preview_feature_type: TypeId,
    /// NOT PORTED: see profile_type.
    pub preview_feature_internal_type: Option<TypeId>,
    pub type_descriptor_type: TypeId,
    pub record_type: TypeId,
    pub switch_bootstraps_type: TypeId,
    pub value_based_type: TypeId,
    /// NOT PORTED: see profile_type.
    pub value_based_internal_type: Option<TypeId>,
    pub length_var: SymbolId,
    pub enum_final_finalize: SymbolId,
    pub auto_closeable_close: SymbolId,
    /// The predefined type that belongs to a tag (Java: final Type[]).
    pub type_of_tag: Vec<Option<TypeId>>,
    /// The name of the class that belongs to a basic type tag.
    pub boxed_name: Vec<Option<Name>>,
    /// The boxed class symbols for each primitive tag (pre-entered from
    /// boxed_name at construction; the read-only counterpart of
    /// Types.boxedClass, registered §9 adapter).
    pub boxed_class_ids: Vec<Option<SymbolId>>,
    /// Java: private final Map<Types.UniqueType, VarSymbol> classFields
    /// (Rust: UniqueType equivalence is expressed by the Types.hashCode value
    /// plus Types.isSameType, so the map key is the hash and each bucket holds
    /// the (TypeId, VarSymbol) pairs; registered section 9).
    class_fields: HashMap<i32, Vec<(TypeId, SymbolId)>>,
    names: Rc<Names>,
    maps: SymtabMaps,
}

impl Symtab {
    /// Constructor; enters all predefined identifiers and operators into the
    /// symbol table.
    ///
    /// PORTING_RULES: the Java protected Symtab(Context) constructor needs the
    /// CompilerContext (arena) and the ported Names; the Context registration
    /// (context.put(symtabKey, this)) is done by the caller until the
    /// Symtab/Context integration batch.
    /// PORTING_RULES: Target.instance(Context) and Source.instance(Context) are
    /// not ported yet (Options/Option integration TODO), so the caller passes
    /// them explicitly, like the Names parameter (registered section 9).
    pub fn new(
        ctx: &mut CompilerContext,
        names: Rc<Names>,
        target: Target,
        source: Source,
    ) -> Symtab {
        let mut maps = SymtabMaps::new();

        // Create the unknown type.
        let unknown_type = ctx.alloc_type(Type::new(TypeTag::UNKNOWN, None, TypeKindData::UnknownType));
        // Java: Type.stuckType / Type.recoveryType (Type.java:108-120): both are
        // JCNoType instances; their toString differs upstream (debug only).
        let stuck_type = ctx.alloc_type(Type::new(TypeTag::NONE, None, TypeKindData::JCNoType));
        let recovery_type = ctx.alloc_type(Type::new(TypeTag::NONE, None, TypeKindData::JCNoType));

        // TODO(MissingInfoHandler batch): MissingInfoHandler.instance(context) is
        // not ported; the root package is created without it.
        let root_package = new_root_package_symbol(
            ctx,
            names.std.empty.clone(),
            target.runtime_use_nest_access(),
        );

        // create the basic builtin symbols
        let java_base = enter_module(ctx, &names, &mut maps, names.std.java_base.clone(), root_package);
        let java_base_requires = new_requires_directive(ctx, java_base, vec![RequiresFlag::MANDATED]);
        let unnamed_module =
            new_module_symbol(ctx, names.std.empty.clone(), List::of(java_base_requires));
        add_root_package_for(ctx, &names, &mut maps, unnamed_module, root_package);
        let err_module = new_module_symbol(ctx, names.std.empty.clone(), List::of(java_base_requires));
        add_root_package_for(ctx, &names, &mut maps, err_module, root_package);
        // Java: noModule = new ModuleSymbol(...) { isNoModule() -> true }; the
        // anonymous override is not expressible (Symbol batch TODO).
        let no_module = new_module_symbol(ctx, names.std.empty.clone(), List::nil());
        add_root_package_for(ctx, &names, &mut maps, no_module, root_package);

        let no_type = no_type_id(ctx);
        let no_symbol = new_type_symbol(
            ctx,
            Kind::NIL,
            0,
            names.std.empty.clone(),
            Some(no_type),
            Some(root_package),
        );

        // create the error symbols
        let err_symbol = new_class_symbol(
            ctx,
            (flags::PUBLIC | flags::STATIC) as i64 | flags::ACYCLIC as i64,
            names.std.any.clone(),
            Some(root_package),
            no_type,
        );
        let err_type = ctx.alloc_type(Type::new(
            TypeTag::ERROR,
            Some(err_symbol),
            TypeKindData::ErrorType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
                original_type: Some(no_type),
            },
        ));
        let unknown_symbol = new_class_symbol_with_type(
            ctx,
            (flags::PUBLIC | flags::STATIC) as i64 | flags::ACYCLIC as i64,
            name_of(&names, "<any?>"),
            None,
            Some(root_package),
        );
        ctx.sym_mut(unknown_symbol).flags_field = (flags::PUBLIC | flags::STATIC) as i64 | flags::ACYCLIC as i64;
        let unknown_error_scope = new_error_scope(ctx, unknown_symbol);
        set_members(ctx, unknown_symbol, unknown_error_scope);
        ctx.sym_mut(unknown_symbol).ty = Some(unknown_type);

        // initialize builtin types
        let mut type_of_tag: Vec<Option<TypeId>> =
            vec![None; TypeTag::get_type_tag_count() as usize];
        let mut boxed_name: Vec<Option<Name>> = vec![None; TypeTag::get_type_tag_count() as usize];

        let byte_type = ctx.alloc_type(Type::new(TypeTag::BYTE, None, TypeKindData::JCPrimitiveType));
        let char_type = ctx.alloc_type(Type::new(TypeTag::CHAR, None, TypeKindData::JCPrimitiveType));
        let short_type = ctx.alloc_type(Type::new(TypeTag::SHORT, None, TypeKindData::JCPrimitiveType));
        let int_type = ctx.alloc_type(Type::new(TypeTag::INT, None, TypeKindData::JCPrimitiveType));
        let long_type = ctx.alloc_type(Type::new(TypeTag::LONG, None, TypeKindData::JCPrimitiveType));
        let float_type = ctx.alloc_type(Type::new(TypeTag::FLOAT, None, TypeKindData::JCPrimitiveType));
        let double_type = ctx.alloc_type(Type::new(TypeTag::DOUBLE, None, TypeKindData::JCPrimitiveType));
        let boolean_type = ctx.alloc_type(Type::new(TypeTag::BOOLEAN, None, TypeKindData::JCPrimitiveType));
        let bot_type = ctx.alloc_type(Type::new(TypeTag::BOT, None, TypeKindData::BottomType));
        let void_type = ctx.alloc_type(Type::new(TypeTag::VOID, None, TypeKindData::JCVoidType));

        let byte_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "byte"), Some(byte_type), Some(root_package));
        let short_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "short"), Some(short_type), Some(root_package));
        let char_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "char"), Some(char_type), Some(root_package));
        let int_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "int"), Some(int_type), Some(root_package));
        let long_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "long"), Some(long_type), Some(root_package));
        let float_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "float"), Some(float_type), Some(root_package));
        let double_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "double"), Some(double_type), Some(root_package));
        let boolean_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "boolean"), Some(boolean_type), Some(root_package));
        let void_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "void"), Some(void_type), Some(root_package));
        let bot_class = new_class_symbol_with_type(
            ctx, flags::PUBLIC as i64, name_of(&names, "<nulltype>"), Some(bot_type), Some(root_package));

        for (ty, class) in [
            (byte_type, byte_class),
            (short_type, short_class),
            (char_type, char_class),
            (int_type, int_class),
            (long_type, long_class),
            (float_type, float_class),
            (double_type, double_class),
            (boolean_type, boolean_class),
            (void_type, void_class),
            (bot_type, bot_class),
        ] {
            ctx.typ_mut(ty).tsym = Some(class);
            type_of_tag[tag_ordinal(ctx.typ(ty).get_tag())] = Some(ty);
        }
        ctx.typ_mut(err_type).tsym = Some(err_symbol);
        type_of_tag[tag_ordinal(TypeTag::ERROR)] = Some(err_type);
        ctx.typ_mut(unknown_type).tsym = Some(unknown_symbol);
        type_of_tag[tag_ordinal(TypeTag::UNKNOWN)] = Some(unknown_type);
        boxed_name[TypeTag::BYTE as usize] = Some(name_of(&names, "java.lang.Byte"));
        boxed_name[TypeTag::SHORT as usize] = Some(name_of(&names, "java.lang.Short"));
        boxed_name[TypeTag::CHAR as usize] = Some(name_of(&names, "java.lang.Character"));
        boxed_name[TypeTag::INT as usize] = Some(name_of(&names, "java.lang.Integer"));
        boxed_name[TypeTag::LONG as usize] = Some(name_of(&names, "java.lang.Long"));
        boxed_name[TypeTag::FLOAT as usize] = Some(name_of(&names, "java.lang.Float"));
        boxed_name[TypeTag::DOUBLE as usize] = Some(name_of(&names, "java.lang.Double"));
        boxed_name[TypeTag::BOOLEAN as usize] = Some(name_of(&names, "java.lang.Boolean"));
        boxed_name[TypeTag::VOID as usize] = Some(name_of(&names, "java.lang.Void"));
        boxed_name[TypeTag::BOT as usize] = Some(name_of(&names, "<nulltype>"));

        // the builtin class of all arrays
        let array_class = new_class_symbol(
            ctx,
            (flags::PUBLIC | flags::ACYCLIC) as i64,
            names.std.Array.clone(),
            Some(no_symbol),
            no_type,
        );
        // VGJ
        let bound_class = new_class_symbol(
            ctx,
            (flags::PUBLIC | flags::ACYCLIC) as i64,
            names.std.Bound.clone(),
            Some(no_symbol),
            no_type,
        );
        let bound_scope = new_error_scope(ctx, bound_class);
        set_members(ctx, bound_class, bound_scope);
        // the builtin class of all methods
        let method_class = new_class_symbol(
            ctx,
            (flags::PUBLIC | flags::ACYCLIC) as i64,
            names.std.Method.clone(),
            Some(no_symbol),
            no_type,
        );
        // Java: methodClass.members_field = new Scope.ErrorScope(boundClass);
        let method_scope = new_error_scope(ctx, bound_class);
        set_members(ctx, method_class, method_scope);

        // Create class to hold all predefined constants and operations.
        let predef_class = new_class_symbol(
            ctx,
            (flags::PUBLIC | flags::ACYCLIC) as i64,
            names.std.empty.clone(),
            Some(root_package),
            no_type,
        );
        let predef_scope = new_scope(ctx, predef_class);
        set_members(ctx, predef_class, predef_scope);

        // Java: initialCompleter = ClassFinder.instance(context).getCompleter();
        // NOT PORTED (ClassFinder/Completer).
        let root_scope = new_scope(ctx, root_package);
        set_members(ctx, root_package, root_scope);

        // Enter symbols for basic types.
        for class in [
            byte_class, short_class, char_class, int_class, long_class, float_class,
            double_class, boolean_class, err_symbol,
        ] {
            ctx.scope_enter(predef_scope, class);
        }

        let java_base = if Feature::MODULES.allowed_in_source(source) {
            let java_base = enter_module(ctx, &names, &mut maps, names.std.java_base.clone(), root_package);
            // Java: avoid completing java.base during the Symtab initialization
            // (java_base.completer = Completer.NULL_COMPLETER) - NOT PORTED.
            java_base
        } else {
            no_module
        };
        // Java: moduleCompleter = Modules.instance(context).getCompleter();
        // NOT PORTED (Modules/Completer).

        // Enter predefined classes. All are assumed to be in the java.base module.
        let object_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Object");
        let object_methods_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.runtime.ObjectMethods");
        let objects_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.util.Objects");
        let class_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Class");
        let string_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.String");
        let string_buffer_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.StringBuffer");
        let string_builder_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.StringBuilder");
        let cloneable_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Cloneable");
        let throwable_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Throwable");
        let serializable_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.io.Serializable");
        let serialized_lambda_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.SerializedLambda");
        let var_handle_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.VarHandle");
        let method_handle_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.MethodHandle");
        let method_handle_lookup_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.MethodHandles$Lookup");
        let method_type_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.MethodType");
        let error_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Error");
        let illegal_argument_exception_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.IllegalArgumentException");
        let interrupted_exception_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.InterruptedException");
        let exception_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Exception");
        let runtime_exception_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.RuntimeException");
        let class_not_found_exception_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.ClassNotFoundException");
        let no_class_def_found_error_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.NoClassDefFoundError");
        let no_such_field_error_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.NoSuchFieldError");
        let assertion_error_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.AssertionError");
        let incompatible_class_change_error_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.IncompatibleClassChangeError");
        let clone_not_supported_exception_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.CloneNotSupportedException");
        let annotation_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.Annotation");
        let class_loader_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.ClassLoader");
        let enum_sym =
            enter_class_full(ctx, &names, &mut maps, java_base, names.std.java_lang_Enum.clone(), None);
        let finalize_type =
            new_method_type(ctx, List::nil(), Some(void_type), List::nil(), method_class);
        let enum_final_finalize = new_method_symbol(
            ctx,
            (flags::PROTECTED | flags::FINAL) as i64 | flags::HYPOTHETICAL,
            names.std.finalize.clone(),
            Some(finalize_type),
            Some(enum_sym),
        );
        let list_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.util.List");
        let collections_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.util.Collections");
        let comparable_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Comparable");
        let comparator_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.util.Comparator");
        let arrays_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.util.Arrays");
        let iterable_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Iterable");
        let iterator_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.util.Iterator");
        let annotation_target_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.Target");
        let override_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Override");
        let retention_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.Retention");
        let deprecated_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Deprecated");
        let suppress_warnings_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.SuppressWarnings");
        let supplier_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.util.function.Supplier");
        let inherited_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.Inherited");
        let repeatable_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.Repeatable");
        let documented_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.Documented");
        let element_type_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.ElementType");
        let system_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.System");
        let auto_closeable_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.AutoCloseable");
        let auto_closeable_tsym = ctx.typ(auto_closeable_type).tsym.expect("AutoCloseable symbol");
        let close_type = new_method_type(
            ctx,
            List::of(exception_type),
            Some(void_type),
            List::of(exception_type),
            method_class,
        );
        let auto_closeable_close = new_method_symbol(
            ctx,
            flags::PUBLIC as i64,
            names.std.close.clone(),
            Some(close_type),
            Some(auto_closeable_tsym),
        );
        let trust_me_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.SafeVarargs");
        let native_header_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.annotation.Native");
        let lambda_metafactory = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.LambdaMetafactory");
        let string_concat_factory = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.StringConcatFactory");
        let functional_interface_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.FunctionalInterface");
        let preview_feature_type = enter_class_named(ctx, &names, &mut maps, java_base, "jdk.internal.javac.PreviewFeature");
        // NOT PORTED: previewFeatureInternalType = enterSyntheticAnnotation(...)
        let type_descriptor_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.invoke.TypeDescriptor");
        let record_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.Record");
        let switch_bootstraps_type = enter_class_named(ctx, &names, &mut maps, java_base, "java.lang.runtime.SwitchBootstraps");
        let value_based_type = enter_class_named(ctx, &names, &mut maps, java_base, "jdk.internal.ValueBased");
        // NOT PORTED: valueBasedInternalType = enterSyntheticAnnotation(...)

        // NOT PORTED: synthesizeEmptyInterfaceIfMissing(...) and
        // synthesizeBoxTypeIfMissing(...) need the anonymous Completers.

        // Enter a class for arrays.
        {
            let array_type = ctx.sym(array_class).ty.expect("array class type");
            if let TypeKindData::ClassType {
                supertype_field,
                interfaces_field,
                ..
            } = &mut ctx.typ_mut(array_type).kind
            {
                *supertype_field = Some(object_type);
                *interfaces_field = Some(List::of2(cloneable_type, serializable_type));
            }
            let array_scope = new_scope(ctx, array_class);
            set_members(ctx, array_class, array_scope);
            let length_var = new_var_symbol(
                ctx,
                (flags::PUBLIC | flags::FINAL) as i64,
                names.std.length.clone(),
                Some(int_type),
                Some(array_class),
            );
            ctx.scope_enter(array_scope, length_var);
            let clone_type =
                new_method_type(ctx, List::nil(), Some(object_type), List::nil(), method_class);
            let array_clone_method = new_method_symbol(
                ctx,
                flags::PUBLIC as i64,
                names.std.clone.clone(),
                Some(clone_type),
                Some(array_class),
            );
            ctx.scope_enter(array_scope, array_clone_method);

            // Java: Types.boxedClass(t) enters syms.enterClass(java_base,
    // boxedName[tag]); the port pre-enters the same symbols at startup so
    // that the read-only relation is available (registered §9 adapter).
    let mut boxed_class_ids: Vec<Option<SymbolId>> =
        vec![None; TypeTag::get_type_tag_count() as usize];
    for (tag, boxed) in boxed_name.iter().enumerate() {
        if let Some(name) = boxed {
            let class = enter_class_full(
                ctx,
                &names,
                &mut maps,
                java_base,
                name.clone(),
                Some(no_symbol),
            );
            boxed_class_ids[tag] = Some(class);
        }
    }

    Symtab {
                byte_type,
                char_type,
                short_type,
                int_type,
                long_type,
                float_type,
                double_type,
                boolean_type,
                bot_type,
                void_type,
                unnamed_module,
                err_module,
                no_module,
                root_package,
                no_symbol,
                err_symbol,
                unknown_symbol,
                err_type,
                unknown_type,
                stuck_type,
                recovery_type,
                array_class,
                array_clone_method,
                bound_class,
                method_class,
                java_base,
                predef_class,
                object_type,
                object_methods_type,
                objects_type,
                class_type,
                class_loader_type,
                string_type,
                string_buffer_type,
                string_builder_type,
                cloneable_type,
                serializable_type,
                serialized_lambda_type,
                var_handle_type,
                method_handle_type,
                method_handle_lookup_type,
                method_type_type,
                native_header_type,
                throwable_type,
                error_type,
                interrupted_exception_type,
                illegal_argument_exception_type,
                exception_type,
                runtime_exception_type,
                class_not_found_exception_type,
                no_class_def_found_error_type,
                no_such_field_error_type,
                assertion_error_type,
                incompatible_class_change_error_type,
                clone_not_supported_exception_type,
                annotation_type,
                enum_sym,
                list_type,
                collections_type,
                comparable_type,
                comparator_type,
                arrays_type,
                iterable_type,
                iterator_type,
                annotation_target_type,
                override_type,
                retention_type,
                deprecated_type,
                suppress_warnings_type,
                supplier_type,
                inherited_type,
                profile_type: None,
                proprietary_type: None,
                system_type,
                auto_closeable_type,
                trust_me_type,
                lambda_metafactory,
                string_concat_factory,
                repeatable_type,
                documented_type,
                element_type_type,
                functional_interface_type,
                preview_feature_type,
                preview_feature_internal_type: None,
                type_descriptor_type,
                record_type,
                switch_bootstraps_type,
                value_based_type,
                value_based_internal_type: None,
                length_var,
                enum_final_finalize,
                auto_closeable_close,
                type_of_tag,
                boxed_name,
        boxed_class_ids,
                class_fields: HashMap::new(),
                names: Rc::clone(&names),
                maps,
                initial_completer: None,
                module_completer: None,
            }
        }
    }

    /// Java: public ClassSymbol getClass(ModuleSymbol msym, Name flatName)
    pub fn get_class(&self, msym: SymbolId, flatname: &Name) -> Option<SymbolId> {
        get_class_in(&self.maps, msym, flatname)
    }

    /// Java: public void removeClass(ModuleSymbol msym, Name flatName)
    pub fn remove_class(&mut self, msym: SymbolId, flatname: &Name) {
        if let Some((_, modules)) = self
            .maps
            .classes
            .iter_mut()
            .find(|(name, _)| name.to_java_string() == flatname.to_java_string())
        {
            modules.retain(|(candidate, _)| *candidate != msym);
        }
    }

    /// Java: public Iterable<ClassSymbol> getAllClasses()
    pub fn get_all_classes(&self) -> Vec<SymbolId> {
        let mut result = Vec::new();
        for (_, modules) in &self.maps.classes {
            for (_, class) in modules {
                result.push(*class);
            }
        }
        result
    }

    /// Java: public PackageSymbol getPackage(ModuleSymbol module, Name fullname)
    pub fn get_package(&self, module: SymbolId, fullname: &Name) -> Option<SymbolId> {
        get_package_in(&self.maps, module, fullname)
    }

    /// Java: public ModuleSymbol getModule(Name name)
    pub fn get_module(&self, name: &Name) -> Option<SymbolId> {
        get_module_in(&self.maps, name)
    }

    /// Java: public ModuleSymbol inferModule(Name packageName)
    pub fn infer_module(&self, ctx: &CompilerContext, package_name: &Name) -> Option<SymbolId> {
        if package_name.is_empty() {
            return Some(if self.java_base == self.no_module {
                self.no_module
            } else {
                self.unnamed_module
            });
        }
        let modules = self
            .maps
            .packages
            .iter()
            .find(|(name, _)| name.to_java_string() == package_name.to_java_string())
            .map(|(_, modules)| modules.clone())?;
        let mut found: Option<SymbolId> = None;
        for (module, package) in modules {
            let members = members_scope(ctx, package);
            let non_empty = members
                .map(|scope| !ctx.scope_is_empty(scope))
                .unwrap_or(false);
            if non_empty {
                if found.is_none() {
                    found = Some(module);
                } else {
                    return None;
                }
            }
        }
        found
    }

    /// Java: public List<ModuleSymbol> listPackageModules(Name packageName)
    pub fn list_package_modules(&self, package_name: &Name) -> List<SymbolId> {
        if package_name.is_empty() {
            return List::nil();
        }
        let mut result = List::nil();
        if let Some((_, modules)) = self
            .maps
            .packages
            .iter()
            .find(|(name, _)| name.to_java_string() == package_name.to_java_string())
        {
            for (module, _) in modules.iter().rev() {
                result = result.prepend(*module);
            }
        }
        result
    }

    /// Java: public void initType(Type type, ClassSymbol c)
    pub fn init_type_class(&mut self, ctx: &mut CompilerContext, ty: TypeId, class: SymbolId) {
        ctx.typ_mut(ty).tsym = Some(class);
        let index = tag_index(ctx, ty);
        if let Some(slot) = self.type_of_tag.get_mut(index) {
            *slot = Some(ty);
        }
    }

    /// Java: public void initType(Type type, String name)
    pub fn init_type_named(
        &mut self,
        ctx: &mut CompilerContext,
        ty: TypeId,
        name: Name,
        class_flags: i64,
    ) -> SymbolId {
        let class = new_class_symbol_with_type(ctx, class_flags, name, Some(ty), Some(self.root_package));
        self.init_type_class(ctx, ty, class);
        class
    }

    /// Java: public void initType(Type type, String name, String bname)
    pub fn init_type_boxed(
        &mut self,
        ctx: &mut CompilerContext,
        ty: TypeId,
        name: Name,
        bname: &str,
        class_flags: i64,
    ) -> SymbolId {
        let class = self.init_type_named(ctx, ty, name, class_flags);
        let index = tag_index(ctx, ty);
        if let Some(slot) = self.boxed_name.get_mut(index) {
            *slot = Some(name_of(&self.names, bname));
        }
        class
    }

    /// Java: public ClassSymbol defineClass(Name name, Symbol owner)
    pub fn define_class(&mut self, ctx: &mut CompilerContext, name: Name, owner: Option<SymbolId>) -> SymbolId {
        define_class(ctx, &self.names, name, owner)
    }

    /// Java: public ClassSymbol enterClass(ModuleSymbol msym, Name name,
    /// TypeSymbol owner)
    pub fn enter_class(
        &mut self,
        ctx: &mut CompilerContext,
        msym: SymbolId,
        name: Name,
        owner: Option<SymbolId>,
    ) -> SymbolId {
        let class = enter_class_full(ctx, &self.names, &mut self.maps, msym, name, owner);
        if let Some(factory) = &self.initial_completer {
            ctx.sym_mut(class).completer = Some(factory());
        }
        class
    }

    /// Java: public ClassSymbol enterClass(ModuleSymbol msym, Name flatname)
    pub fn enter_class_flat(
        &mut self,
        ctx: &mut CompilerContext,
        msym: SymbolId,
        flatname: Name,
    ) -> SymbolId {
        let ps = self.lookup_package(ctx, msym, &convert::package_part(&flatname));
        let package_module = match &ctx.sym(ps).kind_data {
            SymbolKind::PackageSymbol { modle, .. } => modle.expect("package module"),
            _ => panic!("package expected"),
        };
        if let Some(class) = get_class_in(&self.maps, package_module, &flatname) {
            return class;
        }
        let class = define_class(ctx, &self.names, convert::short_name(&flatname), Some(ps));
        do_enter_class(
            &mut self.maps,
            package_module,
            class,
            form_flat_name(ctx, &ctx.sym(class).name.clone(), Some(ps)),
        );
        if let Some(factory) = &self.initial_completer {
            ctx.sym_mut(class).completer = Some(factory());
        }
        class
    }

    /// Java: public PackageSymbol lookupPackage(ModuleSymbol msym, Name
    /// flatname)
    pub fn lookup_package(&mut self, ctx: &mut CompilerContext, msym: SymbolId, flatname: &Name) -> SymbolId {
        lookup_package_impl(
            ctx,
            &self.names,
            &mut self.maps,
            msym,
            flatname,
            false,
            self.unnamed_module,
            self.no_module,
        )
    }

    /// Java: private PackageSymbol lookupPackage(ModuleSymbol msym, Name
    /// flatname, boolean onlyExisting)
    pub fn lookup_package_only_existing(
        &mut self,
        ctx: &mut CompilerContext,
        msym: SymbolId,
        flatname: &Name,
    ) -> SymbolId {
        lookup_package_impl(
            ctx,
            &self.names,
            &mut self.maps,
            msym,
            flatname,
            true,
            self.unnamed_module,
            self.no_module,
        )
    }

    /// Java: public boolean packageExists(ModuleSymbol msym, Name fullname)
    pub fn package_exists(&mut self, ctx: &mut CompilerContext, msym: SymbolId, fullname: &Name) -> bool {
        let package = self.lookup_package_only_existing(ctx, msym, fullname);
        package_exists(ctx, package)
    }

    /// Java: public PackageSymbol enterPackage(ModuleSymbol currModule, Name
    /// fullname)
    pub fn enter_package(
        &mut self,
        ctx: &mut CompilerContext,
        module: SymbolId,
        fullname: Name,
    ) -> SymbolId {
        let package = enter_package(ctx, &self.names, &mut self.maps, module, fullname);
        if let Some(factory) = &self.initial_completer
            && ctx.sym(package).completer.is_none() {
                ctx.sym_mut(package).completer = Some(factory());
            }
        package
    }

    /// Java: public ModuleSymbol enterModule(Name name)
    pub fn enter_module(&mut self, ctx: &mut CompilerContext, name: Name) -> SymbolId {
        let module = enter_module(ctx, &self.names, &mut self.maps, name, self.root_package);
        if let Some(factory) = &self.module_completer
            && ctx.sym(module).completer.is_none() {
                ctx.sym_mut(module).completer = Some(factory());
            }
        module
    }

    /// Java: Symtab constructor binds initialCompleter from
    /// ClassFinder.instance(context).getCompleter(); the port binds it after the
    /// bootstrap creates the ClassFinder (registered adapter) and applies it to
    /// the symbols created before that point.
    pub fn set_initial_completer(&mut self, ctx: &mut CompilerContext, factory: CompleterFactory) {
        self.initial_completer = Some(Rc::clone(&factory));
        let mut classes: Vec<SymbolId> = Vec::new();
        for (_, per_module) in &self.maps.classes {
            for (_, symbol) in per_module {
                classes.push(*symbol);
            }
        }
        for symbol in classes {
            if symbol != self.java_base {
                ctx.sym_mut(symbol).completer = Some(factory());
            }
        }
        let mut packages: Vec<SymbolId> = Vec::new();
        for (_, per_module) in &self.maps.packages {
            for (_, symbol) in per_module {
                packages.push(*symbol);
            }
        }
        for symbol in packages {
            if ctx.sym(symbol).completer.is_none() {
                ctx.sym_mut(symbol).completer = Some(factory());
            }
        }
    }

    /// Java: Symtab constructor binds moduleCompleter from
    /// Modules.instance(context).getCompleter() (Modules is not ported: the
    /// bootstrap may pass the module completer explicitly; registered adapter).
    pub fn set_module_completer(&mut self, ctx: &mut CompilerContext, factory: CompleterFactory) {
        self.module_completer = Some(Rc::clone(&factory));
        // Java: java_base.completer = moduleCompleter::complete (bootstrap issues)
        ctx.sym_mut(self.java_base).completer = Some(factory());
        let modules: Vec<SymbolId> = self.maps.modules.iter().map(|(_, m)| *m).collect();
        for module in modules {
            if ctx.sym(module).completer.is_none() {
                ctx.sym_mut(module).completer = Some(factory());
            }
        }
    }

    /// The Names instance used by this symbol table (Symtab keeps its own
    /// reference like the upstream Symtab field).
    pub fn names(&self) -> &Names {
        self.names.as_ref()
    }

    /// Java: public VarSymbol getClassField(Type type, Types types)
    pub fn get_class_field(
        &mut self,
        ctx: &mut CompilerContext,
        types: &Types,
        t: TypeId,
    ) -> SymbolId {
        let hash = types.hash_code(ctx, t);
        if let Some(bucket) = self.class_fields.get(&hash) {
            for (existing, field) in bucket {
                if types.is_same_type(ctx, self, *existing, t) {
                    return *field;
                }
            }
        }
        let tag = ctx.typ(t).get_tag();
        let arg = if tag == TypeTag::ARRAY || tag == TypeTag::CLASS {
            types.erasure(ctx, self, t)
        } else if ctx.typ(t).is_primitive_or_void() {
            // Java: types.boxedClass(type).type
            let boxed = types.boxed_class(ctx, self, t);
            ctx.sym(boxed).ty.expect("boxed class type")
        } else {
            panic!("AssertionError: getClassField {tag:?}");
        };
        let class_tsym = ctx.typ(self.class_type).tsym;
        let enclosing = match &ctx.typ(self.class_type).kind {
            TypeKindData::ClassType { outer_field, .. } => *outer_field,
            _ => panic!("classType is not a ClassType"),
        };
        let field_type = ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            class_tsym,
            TypeKindData::ClassType {
                outer_field: enclosing,
                typarams_field: Some(List::of(arg)),
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        let var = new_var_symbol(
            ctx,
            (flags::STATIC | flags::PUBLIC | flags::FINAL) as i64,
            self.names.std._class.clone(),
            Some(field_type),
            ctx.typ(t).tsym,
        );
        self.class_fields.entry(hash).or_default().push((t, var));
        var
    }
}

/// The TypeTag ordinal (Java: type.getTag().ordinal()).
fn tag_index(ctx: &CompilerContext, ty: TypeId) -> usize {
    ctx.typ(ty).get_tag() as usize
}

/// The TypeTag ordinal helper for a type tag.
fn tag_ordinal(tag: TypeTag) -> usize {
    tag as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::kinds::KindName;
    use crate::util::names::Names;

    fn build() -> (CompilerContext, Symtab) {
        let mut ctx = CompilerContext::new();
        let names = Rc::new(Names::new(false));
        let symtab = Symtab::new(&mut ctx, names, Target::DEFAULT, Source::JDK17);
        (ctx, symtab)
    }

    #[test]
    fn primitive_types_are_registered_in_tag_order() {
        let (ctx, symtab) = build();
        for (tag, ty) in [
            (TypeTag::BYTE, symtab.byte_type),
            (TypeTag::CHAR, symtab.char_type),
            (TypeTag::SHORT, symtab.short_type),
            (TypeTag::INT, symtab.int_type),
            (TypeTag::LONG, symtab.long_type),
            (TypeTag::FLOAT, symtab.float_type),
            (TypeTag::DOUBLE, symtab.double_type),
            (TypeTag::BOOLEAN, symtab.boolean_type),
            (TypeTag::VOID, symtab.void_type),
        ] {
            assert_eq!(ctx.typ(ty).get_tag(), tag);
            assert_eq!(symtab.type_of_tag[tag_ordinal(tag)], Some(ty));
            let tsym = ctx.typ(ty).tsym.expect("primitive tsym");
            assert_eq!(KindName::CLASS, crate::code::kinds::kind_name(ctx.sym(tsym), &ctx));
        }
        assert_eq!(symtab.type_of_tag[tag_ordinal(TypeTag::BOT)], Some(symtab.bot_type));
        assert_eq!(symtab.type_of_tag[tag_ordinal(TypeTag::ERROR)], Some(symtab.err_type));
        assert_eq!(symtab.type_of_tag[tag_ordinal(TypeTag::UNKNOWN)], Some(symtab.unknown_type));
        assert_eq!(
            symtab.type_of_tag.len(),
            TypeTag::get_type_tag_count() as usize
        );
    }

    #[test]
    fn boxed_names_match_the_primitive_tags() {
        let (ctx, symtab) = build();
        for (tag, expected) in [
            (TypeTag::BYTE, "java.lang.Byte"),
            (TypeTag::SHORT, "java.lang.Short"),
            (TypeTag::CHAR, "java.lang.Character"),
            (TypeTag::INT, "java.lang.Integer"),
            (TypeTag::LONG, "java.lang.Long"),
            (TypeTag::FLOAT, "java.lang.Float"),
            (TypeTag::DOUBLE, "java.lang.Double"),
            (TypeTag::BOOLEAN, "java.lang.Boolean"),
            (TypeTag::VOID, "java.lang.Void"),
        ] {
            let name = symtab.boxed_name[tag_ordinal(tag)].clone().expect("boxed name");
            assert_eq!(name.to_java_string().to_string_lossy(), expected);
        }
        assert_eq!(symtab.boxed_name.len(), TypeTag::get_type_tag_count() as usize);
        let _ = ctx;
    }

    #[test]
    fn predefined_classes_have_the_expected_names() {
        let (ctx, symtab) = build();
        for (ty, expected) in [
            (symtab.object_type, "java.lang.Object"),
            (symtab.class_type, "java.lang.Class"),
            (symtab.string_type, "java.lang.String"),
            (symtab.throwable_type, "java.lang.Throwable"),
            (symtab.exception_type, "java.lang.Exception"),
            (symtab.record_type, "java.lang.Record"),
            (symtab.list_type, "java.util.List"),
        ] {
            let tsym = ctx.typ(ty).tsym.expect("predefined tsym");
            let fullname = match &ctx.sym(tsym).kind_data {
                SymbolKind::ClassSymbol { fullname, .. } => fullname.clone(),
                _ => panic!("class expected"),
            };
            assert_eq!(fullname.to_java_string().to_string_lossy(), expected);
        }
        let object_class = ctx.typ(symtab.object_type).tsym.expect("object tsym");
        assert_eq!(symtab.get_class(symtab.java_base, &name_of(symtab.names(), "java.lang.Object")), Some(object_class));
    }

    #[test]
    fn modules_packages_and_tables_are_consistent() {
        let (mut ctx, mut symtab) = build();
        // getModule(java.base) returns the java.base module.
        let java_base_name = symtab.names().std.java_base.clone();
        assert_eq!(symtab.get_module(&java_base_name), Some(symtab.java_base));
        // the java.base module has an unnamed package and the root package.
        let unnamed_package = match &ctx.sym(symtab.java_base).kind_data {
            SymbolKind::ModuleSymbol { unnamed_package, .. } => unnamed_package.expect("unnamed package"),
            _ => panic!("module expected"),
        };
        assert_eq!(ctx.symbol_kind(unnamed_package), Kind::PCK);
        // enterPackage / lookupPackage / packageExists.
        let package_name = name_of(symtab.names(), "com.example.test");
        let package = symtab.enter_package(&mut ctx, symtab.java_base, package_name.clone());
        assert_eq!(package, symtab.lookup_package(&mut ctx, symtab.java_base, &package_name));
        assert_eq!(symtab.get_package(symtab.java_base, &package_name), Some(package));
        // enterClass(msym, flatname) uses the package of the flat name.
        let flat = name_of(symtab.names(), "com.example.test.Foo");
        let class = symtab.enter_class_flat(&mut ctx, symtab.java_base, flat.clone());
        assert_eq!(symtab.get_class(symtab.java_base, &flat), Some(class));
        // The packages table includes the package (root package is entered for
        // every module: java.base/unnamed/err/noModule).
        let all_classes = symtab.get_all_classes();
        assert!(!all_classes.is_empty());
        assert!(all_classes.contains(&class));
        // removeClass removes it.
        symtab.remove_class(symtab.java_base, &flat);
        assert_eq!(symtab.get_class(symtab.java_base, &flat), None);
        // initType with a new primitive-like class symbol keeps tag order.
        let new_type = ctx.alloc_type(Type::new(TypeTag::NONE, None, TypeKindData::JCNoType));
        let class = symtab.init_type_named(&mut ctx, new_type, name_of(symtab.names(), "testtype"), 0);
        assert_eq!(symtab.type_of_tag[tag_ordinal(TypeTag::NONE)], Some(new_type));
        let _ = class;
    }

    #[test]
    fn array_class_members_are_entered() {
        let (ctx, symtab) = build();
        let array_scope = match &ctx.sym(symtab.array_class).kind_data {
            SymbolKind::ClassSymbol { members_field, .. } => members_field.expect("array members"),
            _ => panic!("class expected"),
        };
        let length = name_of(symtab.names(), "length");
        let clone = name_of(symtab.names(), "clone");
        assert!(ctx.scope_lookup(array_scope, &length).is_some());
        assert!(ctx.scope_lookup(array_scope, &clone).is_some());
        // arrayClass supertype is Object and its interfaces are Cloneable and
        // Serializable.
        let array_type = ctx.sym(symtab.array_class).ty.expect("array type");
        match &ctx.typ(array_type).kind {
            TypeKindData::ClassType {
                supertype_field,
                interfaces_field,
                ..
            } => {
                assert_eq!(*supertype_field, Some(symtab.object_type));
                let interfaces = interfaces_field.as_ref().expect("interfaces");
                assert_eq!(interfaces.length(), 2);
                assert_eq!(interfaces.get(0), symtab.cloneable_type);
                assert_eq!(interfaces.get(1), symtab.serializable_type);
            }
            _ => panic!("class type expected"),
        }
    }
}
