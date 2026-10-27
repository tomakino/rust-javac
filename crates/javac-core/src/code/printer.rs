// Origin: com/sun/tools/javac/code/Printer.java @ 982d53c (jdk-17.0.19-ga)
//
// A combined type/symbol visitor for generating non-trivial localized string
// representation of types and symbols.
//
//! Port notes:
//! - the abstract localized-string/captured-id hooks are the JavacMessages
//!   service and the stable arena type id;
//! - the default locale is used (the formatter locale is not threaded through
//!   the ported diagnostic factory).

use std::collections::HashSet;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::SymbolKind;
use crate::ids::SymbolId;
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;
use crate::util::javac_messages::JavacMessages;
use crate::util::list::List;
use crate::util::names::Names;

const PRINTER_PRIME: u64 = 997;

fn localize(key: &str, args: &[String]) -> String {
    let messages = JavacMessages::new(crate::util::javac_messages::DEFAULT_BUNDLE_NAME);
    messages.get_localized_string(key, args)
}

/// The localized representation of a type (the diagnostic Type printer).
///
/// # Arguments
///   - ctx: the compiler context;
///   - ty: the type to render.
///
/// # Returns
///   - the localized type text.
///
/// # Panics
///   - a class type has no tsym (upstream invariant).
pub fn visit_type(ctx: &CompilerContext, ty: TypeId) -> String {
    let mut seen = HashSet::new();
    visit_type_inner(ctx, ty, &mut seen)
}

fn visit_type_inner(ctx: &CompilerContext, ty: TypeId, seen: &mut HashSet<TypeId>) -> String {
    let t = ctx.typ(ty);
    match &t.kind {
        TypeKindData::ClassType { .. }
        | TypeKindData::ErasedClassType { .. }
        | TypeKindData::UnionClassType { .. }
        | TypeKindData::IntersectionClassType { .. } => visit_class_type(ctx, ty, seen),
        TypeKindData::MethodType { argtypes, restype, .. }
        | TypeKindData::PartiallyInferredMethodType { argtypes, restype, .. } => {
            let args = print_method_args(ctx, argtypes, false, seen);
            let restype = restype.map(|restype| visit_type_inner(ctx, restype, seen)).unwrap_or_default();
            format!("({args}){restype}")
        }
        TypeKindData::WildcardType { type_, kind, .. } => {
            let mut text = kind.to_string();
            if let Some(type_) = type_ {
                text.push_str(&visit_type_inner(ctx, *type_, seen));
            }
            text
        }
        TypeKindData::ArrayType { .. } => {
            let mut root = ty;
            while let TypeKindData::ArrayType { elemtype: Some(inner), .. } = &ctx.typ(root).kind {
                root = *inner;
            }
            let mut text = visit_type_inner(ctx, root, seen);
            let mut current = ty;
            loop {
                text.push_str("[]");
                match &ctx.typ(current).kind {
                    TypeKindData::ArrayType { elemtype: Some(elemtype), .. } if ctx.typ(*elemtype).has_tag(TypeTag::ARRAY) => {
                        current = *elemtype;
                    }
                    _ => break,
                }
            }
            text
        }
        TypeKindData::CapturedType { wildcard, .. } => {
            let id = (stable_type_id(ty) & 0xFFFF_FFFF) % PRINTER_PRIME;
            if seen.contains(&ty) {
                localize("compiler.misc.type.captureof.1", &[id.to_string()])
            } else {
                seen.insert(ty);
                let wildcard = wildcard
                    .map(|wildcard| visit_type_inner(ctx, wildcard, seen))
                    .unwrap_or_default();
                seen.remove(&ty);
                localize("compiler.misc.type.captureof", &[id.to_string(), wildcard])
            }
        }
        TypeKindData::ForAll { tvars, qtype } => {
            let tvars = visit_types(ctx, tvars, seen);
            let qtype = qtype.map(|qtype| visit_type_inner(ctx, qtype, seen)).unwrap_or_default();
            format!("<{tvars}>{qtype}")
        }
        TypeKindData::UndetVar { qtype, inst, .. } => match inst {
            Some(inst) => visit_type_inner(ctx, *inst, seen),
            None => format!("{}?", visit_type_inner(ctx, *qtype, seen)),
        },
        TypeKindData::PackageType | TypeKindData::ModuleType => t
            .tsym
            .map(|tsym| {
                ctx.sym(tsym)
                    .get_qualified_name()
                    .to_java_string()
                    .to_string_lossy()
            })
            .unwrap_or_else(|| localize("compiler.misc.type.none", &[])),
        TypeKindData::ErrorType { .. } => visit_type_name(ctx, ty),
        TypeKindData::JCPrimitiveType
        | TypeKindData::JCVoidType
        | TypeKindData::BottomType
        | TypeKindData::UnknownType
        | TypeKindData::TypeVar { .. } => visit_type_name(ctx, ty),
        TypeKindData::JCNoType { .. } => visit_type_name(ctx, ty),
        TypeKindData::UninitializedType { qtype, .. } => qtype
            .map(|qtype| visit_type_inner(ctx, qtype, seen))
            .unwrap_or_else(|| localize("compiler.misc.type.none", &[])),
        TypeKindData::Deferred { .. } => "DeferredType".to_string(),
        TypeKindData::ProxyType { .. } => "<ProxyType>".to_string(),
    }
}

fn visit_type_name(ctx: &CompilerContext, ty: TypeId) -> String {
    match ctx.typ(ty).tsym {
        Some(tsym) => ctx.sym(tsym).name.to_java_string().to_string_lossy(),
        None => localize("compiler.misc.type.none", &[]),
    }
}

fn visit_class_type(ctx: &CompilerContext, ty: TypeId, seen: &mut HashSet<TypeId>) -> String {
    let t = ctx.typ(ty);
    let tsym = t.tsym.expect("ClassType.tsym");
    let (outer, typarams) = class_type_parts(ctx, ty);
    let mut text = String::new();
    let enclosing_is_class = outer
        .map(|outer| ctx.typ(outer).has_tag(TypeTag::CLASS))
        .unwrap_or(false)
        && ctx
            .sym(tsym)
            .owner
            .map(|owner| ctx.sym(owner).kind == Kind::TYP)
            .unwrap_or(false);
    if enclosing_is_class {
        text.push_str(&visit_type_inner(ctx, outer.expect("enclosing type"), seen));
        text.push('.');
        text.push_str(&class_name(ctx, ty, false));
    } else {
        text.push_str(&class_name(ctx, ty, true));
    }
    let arguments = typarams.unwrap_or_else(List::nil);
    if !arguments.is_empty() {
        text.push('<');
        text.push_str(&visit_types(ctx, &arguments, seen));
        text.push('>');
    }
    text
}

fn class_name(ctx: &CompilerContext, ty: TypeId, longform: bool) -> String {
    let t = ctx.typ(ty);
    let tsym = t.tsym.expect("ClassType.tsym");
    if ctx.sym(tsym).name.is_empty() && (ctx.sym(tsym).flags_field & flags::COMPOUND as i64) != 0 {
        let (supertype, interfaces) = class_type_fields(ctx, ty);
        let mut text = String::new();
        if let Some(supertype) = supertype {
            text.push_str(&visit_type(ctx, supertype));
        }
        let mut current = interfaces;
        while current.non_empty() {
            text.push('&');
            text.push_str(&visit_type(ctx, *current.head()));
            current = current.tail();
        }
        text
    } else if ctx.sym(tsym).name.is_empty() {
        let norm_ty = ctx.sym(tsym).ty;
        let arg = norm_ty.and_then(|norm_ty| {
            let (supertype, interfaces) = class_type_fields(ctx, norm_ty);
            if interfaces.non_empty() {
                Some(*interfaces.head())
            } else {
                supertype
            }
        });
        let arg_text = arg.map(|arg| visit_type(ctx, arg)).unwrap_or_default();
        localize("compiler.misc.anonymous.class", &[arg_text])
    } else if longform {
        ctx.sym(tsym)
            .get_qualified_name()
            .to_java_string()
            .to_string_lossy()
    } else {
        ctx.sym(tsym).name.to_java_string().to_string_lossy()
    }
}

fn class_type_parts(ctx: &CompilerContext, ty: TypeId) -> (Option<TypeId>, Option<List<TypeId>>) {
    match &ctx.typ(ty).kind {
        TypeKindData::ClassType { outer_field, typarams_field, .. }
        | TypeKindData::ErasedClassType { outer_field, typarams_field, .. }
        | TypeKindData::UnionClassType { outer_field, typarams_field, .. }
        | TypeKindData::IntersectionClassType { outer_field, typarams_field, .. } => {
            (*outer_field, typarams_field.clone())
        }
        _ => panic!("ClassCastException: ClassType expected"),
    }
}

fn class_type_fields(ctx: &CompilerContext, ty: TypeId) -> (Option<TypeId>, List<TypeId>) {
    match &ctx.typ(ty).kind {
        TypeKindData::ClassType { supertype_field, interfaces_field, .. }
        | TypeKindData::ErasedClassType { supertype_field, interfaces_field, .. }
        | TypeKindData::UnionClassType { supertype_field, interfaces_field, .. }
        | TypeKindData::IntersectionClassType { supertype_field, interfaces_field, .. } => {
            (*supertype_field, interfaces_field.clone().unwrap_or_else(List::nil))
        }
        _ => panic!("ClassCastException: ClassType expected"),
    }
}

fn visit_types(ctx: &CompilerContext, types: &List<TypeId>, seen: &mut HashSet<TypeId>) -> String {
    let mut text = String::new();
    let mut current = types.clone();
    while current.non_empty() {
        if !text.is_empty() {
            text.push(',');
        }
        text.push_str(&visit_type_inner(ctx, *current.head(), seen));
        current = current.tail();
    }
    text
}

fn print_method_args(ctx: &CompilerContext, args: &List<TypeId>, varargs: bool, seen: &mut HashSet<TypeId>) -> String {
    if !varargs {
        return visit_types(ctx, args, seen);
    }
    let mut text = String::new();
    let mut current = args.clone();
    while current.tail().non_empty() {
        text.push_str(&visit_type_inner(ctx, *current.head(), seen));
        text.push(',');
        current = current.tail();
    }
    if current.non_empty() {
        let head = *current.head();
        if ctx.typ(head).has_tag(TypeTag::ARRAY) {
            if let TypeKindData::ArrayType { elemtype: Some(elemtype), .. } = &ctx.typ(head).kind {
                text.push_str(&visit_type_inner(ctx, *elemtype, seen));
            }
            text.push_str("...");
        } else {
            text.push_str(&visit_type_inner(ctx, head, seen));
        }
    }
    text
}

/// The localized representation of a method symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - names: the name table (constructor detection);
///   - sym: the symbol to render.
///
/// # Returns
///   - the localized symbol text.
///
/// # Panics
///   - none beyond the upstream invariants.
pub fn visit_symbol(ctx: &CompilerContext, names: &Names, sym: SymbolId) -> String {
    if crate::code::symbol::symbol_is_static_or_instance_init(ctx, sym) {
        return ctx
            .sym(sym)
            .owner
            .map(|owner| ctx.sym(owner).name.to_java_string().to_string_lossy())
            .unwrap_or_default();
    }
    let symbol = ctx.sym(sym);
    if !matches!(symbol.kind_data, SymbolKind::MethodSymbol { .. }) {
        return symbol.name.to_java_string().to_string_lossy();
    }
    let mut text = if symbol.is_constructor(names) {
        ctx.sym(symbol.owner.expect("constructor owner"))
            .name
            .to_java_string()
            .to_string_lossy()
    } else {
        symbol.name.to_java_string().to_string_lossy()
    };
    if let Some(ty) = symbol.ty {
        let (type_vars, method_type) = match &ctx.typ(ty).kind {
            TypeKindData::ForAll { tvars, qtype } => (Some(tvars.clone()), *qtype),
            _ => (None, Some(ty)),
        };
        if let Some(type_vars) = type_vars {
            let mut seen = HashSet::new();
            text = format!("<{}>{}", visit_types(ctx, &type_vars, &mut seen), text);
        }
        if let Some(method_type) = method_type {
            let params = method_type_parameter_types(ctx, method_type);
            let varargs = (symbol.flags_field & flags::VARARGS) != 0;
            let mut seen = HashSet::new();
            text.push('(');
            text.push_str(&print_method_args(ctx, &params, varargs, &mut seen));
            text.push(')');
        }
    }
    text
}

fn method_type_parameter_types(ctx: &CompilerContext, ty: TypeId) -> List<TypeId> {
    match &ctx.typ(ty).kind {
        TypeKindData::MethodType { argtypes, .. }
        | TypeKindData::PartiallyInferredMethodType { argtypes, .. } => argtypes.clone(),
        TypeKindData::ForAll { qtype: Some(qtype), .. } => method_type_parameter_types(ctx, *qtype),
        _ => List::nil(),
    }
}

fn stable_type_id(ty: TypeId) -> u64 {
    ty.index() as u64
}