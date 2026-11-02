//! A simple visitor of program elements with default behavior appropriate for
//! the RELEASE_14 source version.
//!
//! Java `SimpleElementVisitor14 extends SimpleElementVisitor9`
//! only overrides `visitRecordComponent` (calling defaultAction).  The Rust
//! trait is a subtrait of `SimpleElementVisitor9` that re-declares that method;
//! calling it on a concrete type that implements both requires a fully
//! qualified call (registered language-shape limitation).

use super::simple_element_visitor9::SimpleElementVisitor9;
use crate::lang::model::element::RecordComponentElement;

/// A simple visitor of program elements with default behavior
/// appropriate for the RELEASE_14 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleElementVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleElementVisitor9
/// @since 16
pub trait SimpleElementVisitor14<R: Clone, P>: SimpleElementVisitor9<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_record_component(&mut self, e: &dyn RecordComponentElement, p: P) -> R {
        self.default_action(e, p)
    }
}
