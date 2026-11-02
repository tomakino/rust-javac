//! A simple visitor of program elements with default behavior appropriate for
//! the RELEASE_6 source version.
//!
//! PORTING_RULES — class mapping:
//! * Java concrete class -> Rust trait with the upstream default behavior.
//! * the Java `protected final R DEFAULT_VALUE` field (whose null default is one
//!   of the two upstream constructors) -> the object-safe accessor
//!   `default_value() -> Option<&R>`; a None value reproduces the Java null
//!   default, and `default_action` panics if it would have returned null (Java
//!   returns null, which then fails at the use site).
//! * visit methods take `&mut self` and dispatch through `default_action`.
//! * visit methods for constructs newer than RELEASE_6 are inherited and call
//!   `visit_unknown`, exactly as upstream.

use super::abstract_element_visitor6::AbstractElementVisitor6;
use crate::lang::model::element::{
    Element, ElementKind, ExecutableElement, PackageElement, TypeElement, TypeParameterElement,
    VariableElement,
};

/// A simple visitor of program elements with default behavior
/// appropriate for the RELEASE_6 source version.
///
/// Visit methods corresponding to RELEASE_6 language
/// constructs call {@link #defaultAction defaultAction}, passing their
/// arguments to defaultAction's corresponding parameters.
///
/// For constructs introduced in RELEASE_7 and later,
/// visitUnknown is called instead.
///
/// <p id=note_for_subclasses><strong>WARNING:</strong> The ElementVisitor
/// interface implemented by this class may have methods added to it in the
/// future to accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.  Therefore, methods whose
/// names begin with "visit" may be added to this class in the future; to avoid
/// incompatibilities, classes and subclasses which extend this class should not
/// declare any instance methods with names beginning with "visit".
///
/// @param <R> the return type of this visitor's methods.  Use Void
///             for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's methods.
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
///
/// @see SimpleElementVisitor7
/// @see SimpleElementVisitor8
/// @see SimpleElementVisitor9
/// @see SimpleElementVisitor14
/// @since 1.6
pub trait SimpleElementVisitor6<R: Clone, P>: AbstractElementVisitor6<R, P> {
    /// Default value to be returned; {@link #defaultAction
    /// defaultAction} returns this value unless the method is
    /// overridden.
    ///
    /// Java: protected final R DEFAULT_VALUE; (null for the no-arg
    /// constructor, which has no Rust form; the accessor returns None for it).
    fn default_value(&self) -> Option<&R>;

    /// The default action for visit methods.
    ///
    /// @implSpec The implementation in this class just returns {@link
    /// #DEFAULT_VALUE}; subclasses will commonly override this method.
    ///
    /// @param e the element to process
    /// @param p a visitor-specified parameter
    /// @return DEFAULT_VALUE unless overridden
    fn default_action(&mut self, _e: &dyn Element, _p: P) -> R {
        match self.default_value() {
            Some(value) => value.clone(),
            None => panic!(
                "SimpleElementVisitor6.defaultAction: DEFAULT_VALUE is null (Java); the Rust \
                 implementation must return Some(default value) from default_value()"
            ),
        }
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_package(&mut self, e: &dyn PackageElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type(&mut self, e: &dyn TypeElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction, unless the
    /// element is a RESOURCE_VARIABLE in which case visitUnknown is called.
    fn visit_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        if e.get_kind() != ElementKind::RESOURCE_VARIABLE {
            self.default_action(e, p)
        } else {
            <Self as AbstractElementVisitor6<R, P>>::visit_unknown(self, e, p)
        }
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_executable(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type_parameter(&mut self, e: &dyn TypeParameterElement, p: P) -> R {
        self.default_action(e, p)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleElementVisitor6;
    use crate::lang::model::element::{ElementKind, ElementVisitor};
    use crate::lang::model::util::test_support::Fixture;

    struct DefaultVisitor {
        value: usize,
    }

    impl ElementVisitor<usize, ()> for DefaultVisitor {}
    impl crate::lang::model::util::abstract_element_visitor6::AbstractElementVisitor6<usize, ()>
        for DefaultVisitor
    {
    }

    impl SimpleElementVisitor6<usize, ()> for DefaultVisitor {
        fn default_value(&self) -> Option<&usize> {
            Some(&self.value)
        }
    }

    #[test]
    fn visit_calls_default_action() {
        let mut v = DefaultVisitor { value: 7 };
        let package = Fixture::new(ElementKind::PACKAGE, "p");
        assert_eq!(SimpleElementVisitor6::visit_package(&mut v, &package, ()), 7);
        let type_ = Fixture::new(ElementKind::CLASS, "C");
        assert_eq!(SimpleElementVisitor6::visit_type(&mut v, &type_, ()), 7);
    }

    #[test]
    #[should_panic(expected = "Unknown element")]
    fn resource_variable_uses_visit_unknown_in_release6() {
        let mut v = DefaultVisitor { value: 7 };
        let rv = Fixture::new(ElementKind::RESOURCE_VARIABLE, "r");
        SimpleElementVisitor6::visit_variable(&mut v, &rv, ());
    }
}
