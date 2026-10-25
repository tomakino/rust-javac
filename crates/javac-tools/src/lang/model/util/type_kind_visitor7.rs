// Origin: javax/lang/model/util/TypeKindVisitor7.java @ 982d53c (jdk-17.0.19-ga)

//! A visitor of types based on their kind with default behavior appropriate for
//! the RELEASE_7 source version.
//!
//! PORTING_RULES: Java `TypeKindVisitor7 extends TypeKindVisitor6` only
//! overrides `visitUnion` (calls defaultAction).  The Rust trait is a subtrait
//! re-declaring that method; calling it on a concrete type implementing both
//! requires a fully qualified call (registered language-shape limitation).

use super::type_kind_visitor6::TypeKindVisitor6;
use crate::lang::model::type_::UnionType;

/// A visitor of types based on their kind with default behavior appropriate for
/// the RELEASE_7 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see TypeKindVisitor6
/// @see TypeKindVisitor8
/// @see TypeKindVisitor9
/// @see TypeKindVisitor14
/// @since 1.7
pub trait TypeKindVisitor7<R: Clone, P>: TypeKindVisitor6<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 1.7
    fn visit_union(&mut self, t: &dyn UnionType, p: P) -> R {
        self.default_action(t, p)
    }
}
