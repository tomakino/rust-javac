//! Represents a record component.
//!
//! the Java `@Override` re-declarations of
//! `getEnclosingElement` and `getSimpleName` only narrow their documentation;
//! Rust inherits both from `Element` (with the `Option` form of
//! `get_enclosing_element`), so they are not repeated (registered difference).

use super::element::Element;
use super::executable_element::ExecutableElement;
use std::rc::Rc;

/// Represents a record component.
///
/// @since 16
pub trait RecordComponentElement: Element {
    /// Returns the executable element for the accessor associated with the
    /// given record component.
    ///
    /// @return the record component accessor.
    fn get_accessor(&self) -> Rc<dyn ExecutableElement>;
}
