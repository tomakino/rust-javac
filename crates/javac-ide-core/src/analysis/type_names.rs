//! Human-readable type names for hover and completion.

use crate::code::bound_kind::BoundKind;
use crate::code::type_::TypeKindData;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;

pub(crate) fn type_name(ctx: &CompilerContext, ty: TypeId) -> String {
    let current = ctx.typ(ty);
    let base_name = || {
        current
            .tsym
            .map(|sym| ctx.sym(sym).name.to_java_string().to_string_lossy())
            .unwrap_or_else(|| "?".to_string())
    };
    match &current.kind {
        TypeKindData::ClassType { typarams_field, .. } => match typarams_field {
            Some(params) if !params.is_empty() => {
                let args = params
                    .iter()
                    .map(|arg| type_name(ctx, arg))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{}>", base_name(), args)
            }
            _ => base_name(),
        },
        // Erased types stay raw: "List", never "List<T>".
        TypeKindData::ErasedClassType { .. } => base_name(),
        TypeKindData::ArrayType { elemtype, .. } => elemtype
            .map(|elem| format!("{}[]", type_name(ctx, elem)))
            .unwrap_or_else(|| "?[]".to_string()),
        TypeKindData::MethodType {
            argtypes, restype, ..
        } => {
            let args = argtypes
                .iter()
                .map(|arg| type_name(ctx, arg))
                .collect::<Vec<_>>()
                .join(", ");
            let ret = restype
                .map(|ret| type_name(ctx, ret))
                .unwrap_or_else(|| "void".to_string());
            format!("({args}){ret}")
        }
        TypeKindData::TypeVar { .. } => base_name(),
        TypeKindData::WildcardType { type_, kind, .. } => match type_ {
            Some(bound) if *kind != BoundKind::UNBOUND => {
                format!("{}{}", kind.name(), type_name(ctx, *bound))
            }
            _ => kind.name().to_string(),
        },
        TypeKindData::ForAll { qtype, .. } => qtype
            .map(|qtype| type_name(ctx, qtype))
            .unwrap_or_else(|| "?".to_string()),
        TypeKindData::JCPrimitiveType => match current.tag {
            crate::code::type_tag::TypeTag::BYTE => "byte",
            crate::code::type_tag::TypeTag::CHAR => "char",
            crate::code::type_tag::TypeTag::SHORT => "short",
            crate::code::type_tag::TypeTag::LONG => "long",
            crate::code::type_tag::TypeTag::FLOAT => "float",
            crate::code::type_tag::TypeTag::INT => "int",
            crate::code::type_tag::TypeTag::DOUBLE => "double",
            crate::code::type_tag::TypeTag::BOOLEAN => "boolean",
            _ => "?",
        }
        .to_string(),
        TypeKindData::JCVoidType => "void".to_string(),
        TypeKindData::BottomType => "null".to_string(),
        _ => "?".to_string(),
    }
}