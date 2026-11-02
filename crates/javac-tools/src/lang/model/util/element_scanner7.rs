//! A scanning visitor of program elements with default behavior appropriate for
//! the RELEASE_7 source version.
//!
//! PORTING_RULES: Java `ElementScanner7 extends ElementScanner6` only overrides
//! `visitVariable` (it scans the enclosed elements for every variable, dropping
//! the RELEASE_6 RESOURCE_VARIABLE special case).  The Rust trait is a subtrait
//! re-declaring that method; calling it on a concrete type implementing both
//! requires a fully qualified call (registered language-shape limitation).

use super::element_scanner6::ElementScanner6;
use crate::lang::model::element::VariableElement;

/// A scanning visitor of program elements with default behavior
/// appropriate for the RELEASE_7 source version.
///
/// @param <R> the return type of this visitor's methods.  Use Void
///            for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use Void for visitors that do not need an
///            additional parameter.
///
/// @see <a href="ElementScanner6.html#note_for_subclasses"><strong>Compatibility
/// note for subclasses</strong></a>
/// @see ElementScanner6
/// @see ElementScanner8
/// @see ElementScanner9
/// @see ElementScanner14
/// @since 1.7
pub trait ElementScanner7<R: Clone, P: Clone>: ElementScanner6<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the enclosed elements.
    fn visit_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.scan_all(&e.get_enclosed_elements(), p)
    }
}

#[cfg(test)]
mod tests {
    use super::ElementScanner7;
    use crate::lang::model::element::{Element, ElementKind, ElementVisitor};
    use crate::lang::model::util::test_support::Fixture;
    use std::rc::Rc;

    struct CollectingScanner {
        log: Vec<ElementKind>,
    }

    impl CollectingScanner {
        fn new() -> CollectingScanner {
            CollectingScanner { log: Vec::new() }
        }
    }

    impl ElementVisitor<usize, ()> for CollectingScanner {}
    impl crate::lang::model::util::abstract_element_visitor6::AbstractElementVisitor6<usize, ()>
        for CollectingScanner
    {
    }
    impl crate::lang::model::util::element_scanner6::ElementScanner6<usize, ()>
        for CollectingScanner
    {
        fn default_value(&self) -> Option<&usize> {
            None
        }
        fn scan(&mut self, e: &dyn Element, _p: ()) -> usize {
            // PORTING_RULES: the base scan needs a concrete-type override; this
            // test records the traversal instead of dispatching.
            self.log.push(e.get_kind());
            0
        }
    }
    impl ElementScanner7<usize, ()> for CollectingScanner {}

    #[test]
    fn release7_scans_resource_variables() {
        // RELEASE_7 drops the RELEASE_6 RESOURCE_VARIABLE special case.
        let child: Rc<dyn Element> = Rc::new(Fixture::new(ElementKind::PACKAGE, "p"));
        let rv = Fixture::with_enclosed(ElementKind::RESOURCE_VARIABLE, "r", vec![child]);
        let mut scanner = CollectingScanner::new();
        let result = <CollectingScanner as super::ElementScanner7<usize, ()>>::visit_variable(
            &mut scanner,
            &rv,
            (),
        );
        assert_eq!(result, 0);
        assert_eq!(scanner.log, vec![ElementKind::PACKAGE]);
    }
}
