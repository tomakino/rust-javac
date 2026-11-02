//! A tree node for a primitive type.

use super::tree::Tree;
use javac_tools::lang::model::type_::TypeKind;

/// A tree node for a primitive type.
///
/// For example:
/// <pre>
///   <em>primitiveTypeKind</em>
/// </pre>
///
/// @jls 4.2 Primitive Types and Values
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
pub trait PrimitiveTypeTree: Tree {
    /// Returns the kind of this primitive type.
    /// @return the kind of the type
    fn get_primitive_type_kind(&self) -> TypeKind;
}
