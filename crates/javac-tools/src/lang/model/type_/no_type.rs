// Origin: javax/lang/model/type/NoType.java @ 982d53c (jdk-17.0.19-ga)

//! A pseudo-type used where no actual type is appropriate.

use super::type_mirror::TypeMirror;

/// A pseudo-type used where no actual type is appropriate.
/// The kinds of NoType are:
/// <ul>
/// <li>{@link TypeKind#VOID VOID} - corresponds to the keyword void.
/// <li>{@link TypeKind#PACKAGE PACKAGE} - the pseudo-type of a package element.
/// <li>{@link TypeKind#MODULE MODULE} - the pseudo-type of a module element.
/// <li>{@link TypeKind#NONE NONE} - used in other cases
///   where no actual type is appropriate; for example, the superclass
///   of java.lang.Object.
/// </ul>
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see ExecutableElement#getReturnType()
/// @since 1.6
pub trait NoType: TypeMirror {}
