
// Source: com/sun/tools/javac/jvm/UninitializedType.java

//! Pseudo-types that appear in verifier tables to indicate objects that have
//! been allocated but not yet constructed.

use crate::code::type_::{Type, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;

/// Create the pseudo-type of a not-yet-constructed this reference.
///
/// The type wraps the qualified type of the constructed object and carries the
/// UNINITIALIZED_THIS tag; the offset of the allocation site is -1.
pub fn uninitialized_this(ctx: &mut CompilerContext, qtype: TypeId) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::UNINITIALIZED_THIS,
        None,
        TypeKindData::UninitializedType {
            qtype: Some(qtype),
            offset: -1,
        },
    ))
}

/// Create the pseudo-type of a not-yet-constructed object allocated at the
/// given code offset.
pub fn uninitialized_object(
    ctx: &mut CompilerContext,
    qtype: TypeId,
    offset: i32,
) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::UNINITIALIZED_OBJECT,
        None,
        TypeKindData::UninitializedType {
            qtype: Some(qtype),
            offset,
        },
    ))
}

/// Return the qualified type wrapped by an uninitialized pseudo-type.
pub fn initialized_type(ctx: &CompilerContext, ty: TypeId) -> Option<TypeId> {
    match &ctx.typ(ty).kind {
        TypeKindData::UninitializedType { qtype, .. } => *qtype,
        _ => None,
    }
}

/// Return the allocation offset of an uninitialized object pseudo-type.
pub fn offset(ctx: &CompilerContext, ty: TypeId) -> Option<i32> {
    match &ctx.typ(ty).kind {
        TypeKindData::UninitializedType { offset, .. } => Some(*offset),
        _ => None,
    }
}
