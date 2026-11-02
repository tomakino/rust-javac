//! A scanning visitor of program elements with default behavior appropriate for
//! the RELEASE_6 source version.
//!
//! PORTING_RULES — class mapping:
//! * Java concrete class -> Rust trait with the upstream default behavior.
//! * `Iterable<? extends Element>` -> `&[Rc<dyn Element>]`.
//! * the Java `public final R scan(Iterable, P)` becomes `scan_all` (overload
//!   mapping, docs/PORTING_RULES.md §3); it returns the result of the last scan
//!   or DEFAULT_VALUE for an empty iterable (Java null default -> panic, see
//!   `default_value`).
//! * `scan(Element, P)` dispatches through the element accept: Rust uses the
//!   kind-based dispatch of AbstractElementVisitor6 (same adapter contract).
//! * the single-argument convenience overload `scan(Element)` forwards null
//!   upstream and is not ported (registered difference).
//! * P must be Clone because a scanner passes the parameter to several
//!   children (Java passes the same reference).

use super::abstract_element_visitor6::AbstractElementVisitor6;
use crate::lang::model::element::{
    Element, ElementKind, ExecutableElement, PackageElement, TypeElement, TypeParameterElement,
    VariableElement,
};
use std::rc::Rc;

/// A scanning visitor of program elements with default behavior
/// appropriate for the RELEASE_6 source version.  The visitXyz methods in this
/// class scan their component elements by calling scan on
/// their enclosed elements, parameters, etc., as
/// indicated in the individual method specifications.  A subclass can
/// control the order elements are visited by overriding the
/// visitXyz methods.  Note that clients of a scanner
/// may get the desired behavior be invoking v.scan(e, p) rather
/// than v.visit(e, p) on the root objects of interest.
///
/// @param <R> the return type of this visitor's methods.  Use Void
///            for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use Void for visitors that do not need an
///            additional parameter.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
///
/// @see ElementScanner7
/// @see ElementScanner8
/// @see ElementScanner9
/// @see ElementScanner14
/// @since 1.6
pub trait ElementScanner6<R: Clone, P: Clone>: AbstractElementVisitor6<R, P> {
    /// The specified default value.
    ///
    /// Java: protected final R DEFAULT_VALUE; (null for the no-arg
    /// constructor, which has no Rust form; the accessor returns None for it).
    fn default_value(&self) -> Option<&R>;

    /// Iterates over the given elements and calls
    /// {@link #scan(Element, Object) scan(Element, P)} on each one.  Returns
    /// the result of the last call to scan or
    /// DEFAULT_VALUE for an empty iterable.
    ///
    /// @param iterable the elements to scan
    /// @param  p additional parameter
    /// @return the scan of the last element or DEFAULT_VALUE if no elements
    ///
    /// PORTING_RULES: overload `scan(Iterable, P)` -> `scan_all`.
    fn scan_all(&mut self, iterable: &[Rc<dyn Element>], p: P) -> R {
        let mut result: Option<R> = self.default_value().cloned();
        for e in iterable {
            result = Some(self.scan(e.as_ref(), p.clone()));
        }
        match result {
            Some(value) => value,
            None => panic!(
                "ElementScanner6.scan: DEFAULT_VALUE is null (Java); the Rust \
                 implementation must return Some(default value) from default_value()"
            ),
        }
    }

    /// Processes an element by calling e.accept(this, p);
    /// this method may be overridden by subclasses.
    ///
    /// @param e the element to scan
    /// @param p a scanner-specified parameter
    /// @return the result of visiting e.
    fn scan(&mut self, e: &dyn Element, p: P) -> R {
        // Java: return e.accept(this, p);
        // PORTING_RULES: Element::accept is generic and cannot be called on
        // &dyn Element; the Rust dispatch through the ElementVisitor trait
        // would bypass the scanner's default visit methods, so this base
        // implementation is not expressible.  Subclasses must override scan (a
        // scanner over concrete element types) — registered language-shape
        // limitation; javac core does not use the ElementScanner classes.
        let _ = (e, p);
        panic!(
            "AbstractMethodError: ElementScanner6.scan(Element, P) requires an override \
             (the Java implementation dispatches through Element.accept)"
        )
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the enclosed elements.
    fn visit_package(&mut self, e: &dyn PackageElement, p: P) -> R {
        self.scan_all(&e.get_enclosed_elements(), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the enclosed elements.
    /// Note that type parameters are <em>not</em> scanned by this
    /// implementation since type parameters are not considered to be
    /// enclosed elements of a type.
    fn visit_type(&mut self, e: &dyn TypeElement, p: P) -> R {
        self.scan_all(&e.get_enclosed_elements(), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the enclosed elements, unless the
    /// element is a RESOURCE_VARIABLE in which case visitUnknown is called.
    fn visit_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        if e.get_kind() != ElementKind::RESOURCE_VARIABLE {
            self.scan_all(&e.get_enclosed_elements(), p)
        } else {
            <Self as AbstractElementVisitor6<R, P>>::visit_unknown(self, e, p)
        }
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the parameters.
    /// Note that type parameters are <em>not</em> scanned by this
    /// implementation.
    fn visit_executable(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        let parameters: Vec<Rc<dyn Element>> = e
            .get_parameters()
            .into_iter()
            .map(|parameter| parameter as Rc<dyn Element>)
            .collect();
        self.scan_all(&parameters, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the enclosed elements.
    fn visit_type_parameter(&mut self, e: &dyn TypeParameterElement, p: P) -> R {
        self.scan_all(&e.get_enclosed_elements(), p)
    }
}

#[cfg(test)]
mod tests {
    use super::ElementScanner6;
    use crate::lang::model::element::{ElementKind, ElementVisitor};
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
    impl ElementScanner6<usize, ()> for CollectingScanner {
        fn default_value(&self) -> Option<&usize> {
            None
        }
        fn scan(&mut self, e: &dyn crate::lang::model::element::Element, _p: ()) -> usize {
            // PORTING_RULES: the base scan needs a concrete-type override; this
            // test records the traversal instead of dispatching.
            self.log.push(e.get_kind());
            0
        }
    }

    #[test]
    fn scanner_scans_enclosed_elements() {
        let package: Rc<dyn crate::lang::model::element::Element> =
            Rc::new(Fixture::new(ElementKind::PACKAGE, "p"));
        let method: Rc<dyn crate::lang::model::element::Element> =
            Rc::new(Fixture::new(ElementKind::METHOD, "m"));
        let class = Fixture::with_enclosed(ElementKind::CLASS, "C", vec![package, method]);
        let mut scanner = CollectingScanner::new();
        let result = ElementScanner6::visit_type(&mut scanner, &class, ());
        assert_eq!(result, 0);
        assert_eq!(scanner.log, vec![ElementKind::PACKAGE, ElementKind::METHOD]);
    }
}
