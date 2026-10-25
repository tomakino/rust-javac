// Origin: javax/lang/model/type/UnionType.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a union type.

use super::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents a union type.
///
/// Union types can appear as the type of a multi-catch exception
/// parameter.
///
/// @jls 14.20 The try statement
/// @since 1.7
pub trait UnionType: TypeMirror {
    /// {@return the alternatives comprising this union type}
    fn get_alternatives(&self) -> Vec<Rc<dyn TypeMirror>>;
}
