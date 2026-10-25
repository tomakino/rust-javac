// Origin: javax/lang/model/type/TypeVariable.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a type variable.

use super::reference_type::ReferenceType;
use super::type_mirror::TypeMirror;
use crate::lang::model::element::Element;
use std::rc::Rc;

/// Represents a type variable.
/// A type variable may be explicitly declared by a
/// type parameter of a
/// type, method, or constructor.
/// A type variable may also be declared implicitly, as by
/// the capture conversion of a wildcard type argument
/// (see chapter {@jls 5} of
/// <cite>The Java Language Specification</cite>).
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see TypeParameterElement
/// @jls 4.4 Type Variables
/// @since 1.6
pub trait TypeVariable: ReferenceType {
    /// {@return the element corresponding to this type variable}
    fn as_element(&self) -> Rc<dyn Element>;

    /// {@return the upper bound of this type variable}
    ///
    /// <p> If this type variable was declared with no explicit
    /// upper bounds, the result is java.lang.Object.
    /// If it was declared with multiple upper bounds,
    /// the result is an {@linkplain IntersectionType intersection type};
    /// individual bounds can be found by examining the result's
    /// {@linkplain IntersectionType#getBounds() bounds}.
    ///
    /// @jls 4.9 Intersection Types
    fn get_upper_bound(&self) -> Rc<dyn TypeMirror>;

    /// {@return the lower bound of this type variable}  While a type
    /// parameter cannot include an explicit lower bound declaration,
    /// capture conversion can produce a type variable with a
    /// non-trivial lower bound.  Type variables otherwise have a
    /// lower bound of {@link NullType}.
    ///
    /// @jls 18.1.3. Bounds
    fn get_lower_bound(&self) -> Rc<dyn TypeMirror>;
}
