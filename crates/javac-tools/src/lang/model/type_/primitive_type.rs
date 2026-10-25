// Origin: javax/lang/model/type/PrimitiveType.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a primitive type.

use super::type_mirror::TypeMirror;

/// Represents a primitive type.  These include
/// boolean, byte, short, int,
/// long, char, float, and double.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @jls 4.2 Primitive Types and Values
/// @since 1.6
pub trait PrimitiveType: TypeMirror {}
