//! A mixin interface for an element that has a qualified name.

use super::element::Element;
use super::name::Name;
use std::rc::Rc;

/// A mixin interface for an element that has a qualified name.
///
/// @author Joseph D. Darcy
/// @since 1.7
pub trait QualifiedNameable: Element {
    /// {@return the fully qualified name of an element}
    fn get_qualified_name(&self) -> Rc<dyn Name>;
}
