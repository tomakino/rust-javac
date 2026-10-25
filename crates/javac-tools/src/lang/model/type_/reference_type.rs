// Origin: javax/lang/model/type/ReferenceType.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a reference type.

use super::type_mirror::TypeMirror;

/// Represents a reference type.
/// These include class and interface types, array types, type variables,
/// and the null type.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @jls 4.3 Reference Types and Values
/// @since 1.6
pub trait ReferenceType: TypeMirror {}
