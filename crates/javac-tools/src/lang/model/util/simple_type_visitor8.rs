//! A simple visitor of types with default behavior appropriate for the
//! RELEASE_8 source version.
//!
//! Java `SimpleTypeVisitor8 extends SimpleTypeVisitor7` only
//! overrides `visitIntersection` (calls defaultAction).  The Rust trait is a
//! subtrait re-declaring that method; calling it on a concrete type
//! implementing both requires a fully qualified call (registered language-shape
//! limitation).

use super::simple_type_visitor7::SimpleTypeVisitor7;
use crate::lang::model::type_::IntersectionType;

/// A simple visitor of types with default behavior appropriate for the
/// RELEASE_8 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleTypeVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleTypeVisitor6
/// @see SimpleTypeVisitor7
/// @see SimpleTypeVisitor9
/// @see SimpleTypeVisitor14
/// @since 1.8
pub trait SimpleTypeVisitor8<R: Clone, P>: SimpleTypeVisitor7<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 1.8
    fn visit_intersection(&mut self, t: &dyn IntersectionType, p: P) -> R {
        self.default_action(t, p)
    }
}
