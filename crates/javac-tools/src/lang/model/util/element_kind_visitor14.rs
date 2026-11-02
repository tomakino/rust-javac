//! A visitor of program elements based on their kind with default behavior
//! appropriate for the RELEASE_14 source version.
//!
//! PORTING_RULES: Java `ElementKindVisitor14 extends ElementKindVisitor9`
//! overrides `visitRecordComponent`, `visitTypeAsRecord` and
//! `visitVariableAsBindingVariable` (all call defaultAction).  The Rust trait is
//! a subtrait re-declaring those methods; calling them on a concrete type
//! implementing both requires a fully qualified call (registered language-shape
//! limitation).

use super::element_kind_visitor9::ElementKindVisitor9;
use crate::lang::model::element::{RecordComponentElement, TypeElement, VariableElement};

/// A visitor of program elements based on their kind with default behavior
/// appropriate for the RELEASE_14 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see ElementKindVisitor6
/// @see ElementKindVisitor9
/// @since 16
pub trait ElementKindVisitor14<R: Clone, P>: ElementKindVisitor9<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_record_component(&mut self, e: &dyn RecordComponentElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a RECORD type element.
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 16
    fn visit_type_as_record(&mut self, e: &dyn TypeElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a BINDING_VARIABLE variable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 14
    fn visit_variable_as_binding_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }
}
