//! Human-readable type names for hover.

use crate::code::type_::TypeKindData;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;

pub(crate) fn type_name(ctx: &CompilerContext, ty: TypeId) -> String {
    let current = ctx.typ(ty);
    match &current.kind {
        TypeKindData::ClassType { .. } => current
            .tsym
            .map(|sym| ctx.sym(sym).name.to_java_string().to_string_lossy())
            .unwrap_or_else(|| "?".to_string()),
        TypeKindData::ArrayType { elemtype, .. } => elemtype
            .map(|elem| format!("{}[]", type_name(ctx, elem)))
            .unwrap_or_else(|| "?[]".to_string()),
        TypeKindData::MethodType { argtypes, restype, .. } => {
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
        TypeKindData::TypeVar { .. } => current
            .tsym
            .map(|sym| ctx.sym(sym).name.to_java_string().to_string_lossy())
            .unwrap_or_else(|| "?".to_string()),
        TypeKindData::ForAll { qtype, .. } => qtype
            .map(|qtype| type_name(ctx, qtype))
            .unwrap_or_else(|| "?".to_string()),
        TypeKindData::JCVoidType => "void".to_string(),
        TypeKindData::BottomType => "null".to_string(),
        _ => "?".to_string(),
    }
}
