// Origin: javax/lang/model/util/ElementKindVisitor7.java @ 982d53c (jdk-17.0.19-ga)

//! A visitor of program elements based on their kind with default behavior
//! appropriate for the RELEASE_7 source version.
//!
//! PORTING_RULES: Java `ElementKindVisitor7 extends ElementKindVisitor6` only
//! overrides `visitVariableAsResourceVariable` (calls defaultAction instead of
//! visitUnknown).  The Rust trait is a subtrait re-declaring that method;
//! calling it on a concrete type implementing both requires a fully qualified
//! call (registered language-shape limitation).

use super::element_kind_visitor6::ElementKindVisitor6;
use crate::lang::model::element::VariableElement;

/// A visitor of program elements based on their kind with default behavior
/// appropriate for the RELEASE_7 source version.
///
/// @param <R> the return type of this visitor's methods.  Use Void
///            for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use Void for visitors that do not need an
///            additional parameter.
///
/// @see ElementKindVisitor6
/// @see ElementKindVisitor8
/// @see ElementKindVisitor9
/// @see ElementKindVisitor14
/// @since 1.7
pub trait ElementKindVisitor7<R: Clone, P>: ElementKindVisitor6<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 1.7
    fn visit_variable_as_resource_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }
}
