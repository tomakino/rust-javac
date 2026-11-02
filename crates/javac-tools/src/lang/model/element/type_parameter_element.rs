//! Represents a formal type parameter of a generic class, interface, method,
//! or constructor element.
//!
//! PORTING_RULES:
//! * `List<? extends TypeMirror>` -> `Vec<Rc<dyn TypeMirror>>`.
//! * the Java `@Override` re-declarations of `asType` and
//!   `getEnclosingElement` only narrow their documentation and return type in
//!   the subinterface; Rust inherits both from `Element` and cannot narrow the
//!   `Option` of `get_enclosing_element` (upstream guarantees non-null here),
//!   so they are not repeated (registered difference).

use super::element::Element;
use crate::lang::model::type_::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents a formal type parameter of a generic class, interface, method,
/// or constructor element.
/// A type parameter declares a {@link TypeVariable}.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see TypeVariable
/// @since 1.6
pub trait TypeParameterElement: Element {
    /// {@return the generic class, interface, method, or constructor that is
    /// parameterized by this type parameter}
    fn get_generic_element(&self) -> Rc<dyn Element>;

    /// Returns the bounds of this type parameter.
    /// These are the types given by the extends clause
    /// used to declare this type parameter.
    /// If no explicit extends clause was used,
    /// then java.lang.Object is considered to be the sole bound.
    ///
    /// @return the bounds of this type parameter, or an empty list if
    /// there are none
    fn get_bounds(&self) -> Vec<Rc<dyn TypeMirror>>;
}
