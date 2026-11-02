//! A visitor of program elements based on their kind with default behavior
//! appropriate for source versions RELEASE_9 through RELEASE_14.
//!
//! Java `ElementKindVisitor9 extends ElementKindVisitor8` only
//! overrides `visitModule` (calls defaultAction).  The Rust trait is a subtrait
//! re-declaring that method; calling it on a concrete type implementing both
//! requires a fully qualified call (registered language-shape limitation).

use super::element_kind_visitor8::ElementKindVisitor8;
use crate::lang::model::element::ModuleElement;

/// A visitor of program elements based on their kind with default behavior
/// appropriate for source versions RELEASE_9 through RELEASE_14.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see ElementKindVisitor6
/// @see ElementKindVisitor7
/// @see ElementKindVisitor8
/// @since 9
pub trait ElementKindVisitor9<R: Clone, P>: ElementKindVisitor8<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 9
    fn visit_module(&mut self, e: &dyn ModuleElement, p: P) -> R {
        self.default_action(e, p)
    }
}
