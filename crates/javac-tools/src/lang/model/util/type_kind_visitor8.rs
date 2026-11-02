//! A visitor of types based on their kind with default behavior appropriate for
//! the RELEASE_8 source version.
//!
//! PORTING_RULES: Java `TypeKindVisitor8 extends TypeKindVisitor7` only
//! overrides `visitIntersection` (calls defaultAction).  The Rust trait is a
//! subtrait re-declaring that method; calling it on a concrete type
//! implementing both requires a fully qualified call (registered language-shape
//! limitation).

use super::type_kind_visitor7::TypeKindVisitor7;
use crate::lang::model::type_::IntersectionType;

/// A visitor of types based on their kind with default behavior appropriate for
/// the RELEASE_8 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see TypeKindVisitor6
/// @see TypeKindVisitor7
/// @see TypeKindVisitor9
/// @see TypeKindVisitor14
/// @since 1.8
pub trait TypeKindVisitor8<R: Clone, P>: TypeKindVisitor7<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 1.8
    fn visit_intersection(&mut self, t: &dyn IntersectionType, p: P) -> R {
        self.default_action(t, p)
    }
}
