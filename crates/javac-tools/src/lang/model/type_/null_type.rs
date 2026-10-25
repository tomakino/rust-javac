// Origin: javax/lang/model/type/NullType.java @ 982d53c (jdk-17.0.19-ga)

//! Represents the null type.

use super::reference_type::ReferenceType;

/// Represents the null type.
/// This is the type of the expression null,
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @jls 3.10.7 The Null Literal
/// @jls 4.1 The Kinds of Types and Values
/// @since 1.6
pub trait NullType: ReferenceType {}
