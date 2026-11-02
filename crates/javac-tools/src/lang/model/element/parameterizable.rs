//! A mixin interface for an element that has type parameters.

use super::element::Element;
use super::type_parameter_element::TypeParameterElement;
use std::rc::Rc;

/// A mixin interface for an element that has type parameters.
///
/// @author Joseph D. Darcy
/// @since 1.7
pub trait Parameterizable: Element {
    /// Returns the formal type parameters of an element in
    /// declaration order.
    ///
    /// @return the formal type parameters, or an empty list
    /// if there are none
    fn get_type_parameters(&self) -> Vec<Rc<dyn TypeParameterElement>>;
}
