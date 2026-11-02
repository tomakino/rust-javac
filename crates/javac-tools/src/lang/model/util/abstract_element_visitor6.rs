//! A skeletal visitor of program elements with default behavior appropriate
//! for the RELEASE_6 source version.
//!
//! PORTING_RULES — class mapping:
//! * Java abstract class -> Rust trait with the upstream default behavior.
//! * the Java `final R visit(Element, P)` forwards to `e.accept(this, p)`;
//!   Rust cannot call the generic `Element::accept` on `&dyn Element` (see the
//!   Element trait PORTING_RULES), so this default panics and callers dispatch
//!   through a concrete element type (registered language-shape limitation).
//! * the Java constructors and @SupportedSourceVersion annotations have no Rust
//!   form.
//! * `visitUnknown` throws `UnknownElementException` upstream; the Rust default
//!   panics with the same exception message.

use crate::lang::model::element::{
    Element, ElementKind, ElementVisitor, ModuleElement, RecordComponentElement,
};

/// A skeletal visitor of program elements with default behavior
/// appropriate for the RELEASE_6 source version.
///
/// <p id=note_for_subclasses><strong>WARNING:</strong> The ElementVisitor
/// interface implemented by this class may have methods added to it in the
/// future to accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.  Therefore, methods whose
/// names begin with "visit" may be added to this class in the future; to avoid
/// incompatibilities, classes and subclasses which extend this class should not
/// declare any instance methods with names beginning with "visit".
///
/// <p>When such a new visit method is added, the default
/// implementation in this class will be to directly or indirectly call
/// the {@link #visitUnknown visitUnknown} method.  A new abstract
/// element visitor class will also be introduced to correspond to the
/// new language level; this visitor will have different default
/// behavior for the visit method in question.
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
/// @see AbstractElementVisitor7
/// @see AbstractElementVisitor8
/// @see AbstractElementVisitor9
/// @see AbstractElementVisitor14
/// @since 1.6
pub trait AbstractElementVisitor6<R, P>: ElementVisitor<R, P> {
    /// Visits any program element as if by passing itself to that
    /// element's {@link Element#accept accept} method.  The invocation
    /// v.visit(elem, p) is equivalent to elem.accept(v, p).
    ///
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    ///
    /// PORTING_RULES: see the file header; the Java implementation is
    /// `return e.accept(this, p);`.
    fn visit(&mut self, e: &dyn Element, p: P) -> R {
        dispatch_element(self, e, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec The default implementation of this method in
    /// AbstractElementVisitor6 will always throw
    /// new UnknownElementException(e, p).
    /// This behavior is not required of a subclass.
    ///
    /// @param e {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  {@inheritDoc}
    /// @throws UnknownElementException
    ///          a visitor implementation may optionally throw this exception
    fn visit_unknown(&mut self, e: &dyn Element, p: P) -> R {
        let _ = p;
        // Java: throw new UnknownElementException(e, p);
        panic!("Unknown element: \"{}\"", e)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec Visits a ModuleElement by calling visitUnknown.
    ///
    /// @param e  {@inheritDoc}
    /// @param p  {@inheritDoc}
    /// @return   {@inheritDoc}
    ///
    /// @since 9
    fn visit_module(&mut self, e: &dyn ModuleElement, p: P) -> R {
        // Use implementation from interface default method
        <Self as AbstractElementVisitor6<R, P>>::visit_unknown(self, e, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec Visits a RecordComponentElement by calling visitUnknown.
    ///
    /// @param e  {@inheritDoc}
    /// @param p  {@inheritDoc}
    /// @return   {@inheritDoc}
    ///
    /// @since 14
    fn visit_record_component(&mut self, e: &dyn RecordComponentElement, p: P) -> R {
        // Use implementation from interface default method
        <Self as AbstractElementVisitor6<R, P>>::visit_unknown(self, e, p)
    }
}

/// Java: the element's own `accept(visitor, p)` method performs the dispatch to
/// the most specific `visitXyz` method.
///
/// PORTING_RULES: `Element::accept` is generic and therefore not callable on
/// `&dyn Element`; the dispatch is reproduced from the element kind and the
/// object-conversion accessors (`Element::as_xxx`), which is exactly the
/// information the Java dispatch uses.  Concrete Element implementations must
/// override the accessor for their kind (registered adapter contract); a
/// missing accessor panics instead of silently mis-dispatching.
pub(crate) fn dispatch_element<R, P, V: ElementVisitor<R, P> + ?Sized>(
    v: &mut V,
    e: &dyn Element,
    p: P,
) -> R {
    match e.get_kind() {
        ElementKind::PACKAGE => match e.as_package_element() {
            Some(element) => v.visit_package(element, p),
            None => missing_dispatch(e),
        },
        ElementKind::MODULE => match e.as_module_element() {
            Some(element) => v.visit_module(element, p),
            None => missing_dispatch(e),
        },
        ElementKind::ANNOTATION_TYPE
        | ElementKind::CLASS
        | ElementKind::ENUM
        | ElementKind::INTERFACE
        | ElementKind::RECORD => match e.as_type_element() {
            Some(element) => v.visit_type(element, p),
            None => missing_dispatch(e),
        },
        ElementKind::ENUM_CONSTANT
        | ElementKind::FIELD
        | ElementKind::PARAMETER
        | ElementKind::LOCAL_VARIABLE
        | ElementKind::EXCEPTION_PARAMETER
        | ElementKind::RESOURCE_VARIABLE
        | ElementKind::BINDING_VARIABLE => match e.as_variable_element() {
            Some(element) => v.visit_variable(element, p),
            None => missing_dispatch(e),
        },
        ElementKind::METHOD
        | ElementKind::CONSTRUCTOR
        | ElementKind::STATIC_INIT
        | ElementKind::INSTANCE_INIT => match e.as_executable_element() {
            Some(element) => v.visit_executable(element, p),
            None => missing_dispatch(e),
        },
        ElementKind::TYPE_PARAMETER => match e.as_type_parameter_element() {
            Some(element) => v.visit_type_parameter(element, p),
            None => missing_dispatch(e),
        },
        ElementKind::RECORD_COMPONENT => match e.as_record_component_element() {
            Some(element) => v.visit_record_component(element, p),
            None => missing_dispatch(e),
        },
        ElementKind::OTHER => v.visit_unknown(e, p),
    }
}

fn missing_dispatch(e: &dyn Element) -> ! {
    panic!(
        "ClassCastException: element of kind {} does not provide its subinterface \
         (PORTING_RULES: concrete Element implementations must override the as_xxx \
         accessor for their kind)",
        e.get_kind()
    )
}

#[cfg(test)]
mod tests {
    use super::AbstractElementVisitor6;
    use crate::lang::model::element::{
        Element, ElementKind, ElementVisitor, PackageElement, TypeElement,
    };
    use crate::lang::model::util::test_support::Fixture;

    struct TestVisitor {
        visited: Vec<ElementKind>,
    }

    impl TestVisitor {
        fn new() -> TestVisitor {
            TestVisitor { visited: Vec::new() }
        }
    }

    impl AbstractElementVisitor6<(), ()> for TestVisitor {}

    impl ElementVisitor<(), ()> for TestVisitor {
        fn visit_package(&mut self, e: &dyn PackageElement, _p: ()) {
            self.visited.push(e.get_kind());
        }
        fn visit_type(&mut self, e: &dyn TypeElement, _p: ()) {
            self.visited.push(e.get_kind());
        }
        fn visit_unknown(&mut self, e: &dyn Element, _p: ()) {
            self.visited.push(e.get_kind());
        }
    }

    #[test]
    fn dispatch_uses_the_element_kind() {
        let mut v = TestVisitor::new();
        let class = Fixture::new(ElementKind::CLASS, "C");
        AbstractElementVisitor6::visit(&mut v, &class, ());
        let package = Fixture::new(ElementKind::PACKAGE, "p");
        AbstractElementVisitor6::visit(&mut v, &package, ());
        let other = Fixture::new(ElementKind::OTHER, "x");
        AbstractElementVisitor6::visit(&mut v, &other, ());
        assert_eq!(
            v.visited,
            vec![ElementKind::CLASS, ElementKind::PACKAGE, ElementKind::OTHER]
        );
    }

    #[test]
    fn visitor_is_object_safe() {
        let v: &mut dyn AbstractElementVisitor6<(), ()> = &mut TestVisitor::new();
        let class = Fixture::new(ElementKind::CLASS, "C");
        // The visit default is callable through dyn (dispatch through get_kind).
        AbstractElementVisitor6::visit(v, &class, ());
    }
}
