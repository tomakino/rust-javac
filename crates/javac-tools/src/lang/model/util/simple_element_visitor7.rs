//! A simple visitor of program elements with default behavior appropriate for
//! the RELEASE_7 source version.
//!
//! Java `SimpleElementVisitor7 extends SimpleElementVisitor6`
//! only overrides `visitVariable` (it calls defaultAction for every variable,
//! dropping the RELEASE_6 RESOURCE_VARIABLE special case).  The Rust trait is a
//! subtrait of `SimpleElementVisitor6` that re-declares that one method with the
//! RELEASE_7 behavior.  Calling it on a concrete type that implements both
//! traits requires a fully qualified call (registered language-shape
//! limitation).

use super::simple_element_visitor6::SimpleElementVisitor6;
use crate::lang::model::element::VariableElement;

/// A simple visitor of program elements with default behavior
/// appropriate for the RELEASE_7 source version.
///
/// Visit methods corresponding to RELEASE_7 language constructs call
/// {@link #defaultAction defaultAction}, passing their arguments to
/// defaultAction's corresponding parameters.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleElementVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleElementVisitor6
/// @see SimpleElementVisitor8
/// @see SimpleElementVisitor9
/// @see SimpleElementVisitor14
/// @since 1.7
pub trait SimpleElementVisitor7<R: Clone, P>: SimpleElementVisitor6<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleElementVisitor7;
    use crate::lang::model::element::ElementKind;
    use crate::lang::model::util::test_support::Fixture;

    struct DefaultVisitor {
        value: usize,
    }

    impl crate::lang::model::element::ElementVisitor<usize, ()> for DefaultVisitor {}
    impl crate::lang::model::util::abstract_element_visitor6::AbstractElementVisitor6<usize, ()>
        for DefaultVisitor
    {
    }
    impl crate::lang::model::util::simple_element_visitor6::SimpleElementVisitor6<usize, ()>
        for DefaultVisitor
    {
        fn default_value(&self) -> Option<&usize> {
            Some(&self.value)
        }
    }
    impl SimpleElementVisitor7<usize, ()> for DefaultVisitor {}

    #[test]
    fn resource_variable_uses_default_action_in_release7() {
        // RELEASE_7 drops the RELEASE_6 RESOURCE_VARIABLE special case.
        let mut v = DefaultVisitor { value: 7 };
        let rv = Fixture::new(ElementKind::RESOURCE_VARIABLE, "r");
        assert_eq!(SimpleElementVisitor7::visit_variable(&mut v, &rv, ()), 7);
    }
}
