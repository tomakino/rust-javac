// Origin: javax/lang/model/type/ArrayType.java @ 982d53c (jdk-17.0.19-ga)

//! Represents an array type.

use super::reference_type::ReferenceType;
use super::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents an array type.
/// A multidimensional array type is represented as an array type
/// whose component type is also an array type.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait ArrayType: ReferenceType {
    /// {@return the component type of this array type}
    fn get_component_type(&self) -> Rc<dyn TypeMirror>;
}
