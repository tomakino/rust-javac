// Origin: javax/lang/model/type/IntersectionType.java @ 982d53c (jdk-17.0.19-ga)

//! Represents an intersection type.

use super::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents an intersection type.
///
/// <p>An intersection type can be either implicitly or explicitly
/// declared in a program. For example, the bound of the type parameter
/// &lt;T extends Number &amp; Runnable&gt; is an (implicit) intersection
/// type.  This is represented by an IntersectionType with
/// Number and Runnable as its bounds.
///
/// @implNote In the reference implementation an
/// IntersectionType is used to model the explicit target type of a
/// cast expression.
///
/// @since 1.8
pub trait IntersectionType: TypeMirror {
    /// {@return the bounds comprising this intersection type}
    fn get_bounds(&self) -> Vec<Rc<dyn TypeMirror>>;
}
