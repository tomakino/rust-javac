// Origin: javax/lang/model/util/ElementKindVisitor6.java @ 982d53c (jdk-17.0.19-ga)

//! A visitor of program elements based on their kind with default behavior
//! appropriate for the RELEASE_6 source version.
//!
//! PORTING_RULES — class mapping:
//! * Java concrete class -> Rust trait with the upstream default behavior.
//! * the Java `assert e.getKind() == PACKAGE` statements (disabled by default in
//!   Java) -> `debug_assert_eq!` (enabled in debug builds); the switch default
//!   arms that throw AssertionError unconditionally -> panic with the upstream
//!   message.
//! * visit methods shared with SimpleElementVisitor6 are re-declared by this
//!   subtrait with the kind-dispatching behavior; calling them on a concrete
//!   type implementing both traits requires a fully qualified call (registered
//!   language-shape limitation).

use super::abstract_element_visitor6::AbstractElementVisitor6;
use super::simple_element_visitor6::SimpleElementVisitor6;
use crate::lang::model::element::{
    ElementKind, ExecutableElement, PackageElement, TypeElement, TypeParameterElement,
    VariableElement,
};

/// A visitor of program elements based on their {@linkplain
/// ElementKind kind} with default behavior appropriate for the
/// RELEASE_6 source version.  For elements Xyz that may have more than one
/// kind, the visitXyz methods in this class delegate
/// to the visitXyzAsKind method corresponding to the
/// first argument's kind.  The visitXyzAsKind methods
/// call {@link #defaultAction defaultAction}, passing their arguments
/// to defaultAction's corresponding parameters.
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
/// @see ElementKindVisitor7
/// @see ElementKindVisitor8
/// @see ElementKindVisitor9
/// @see ElementKindVisitor14
/// @since 1.6
pub trait ElementKindVisitor6<R: Clone, P>: SimpleElementVisitor6<R, P> {
    /// {@inheritDoc}
    ///
    /// The element argument has kind PACKAGE.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_package(&mut self, e: &dyn PackageElement, p: P) -> R {
        debug_assert_eq!(
            e.get_kind(),
            ElementKind::PACKAGE,
            "Bad kind on PackageElement"
        );
        self.default_action(e, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation dispatches to the visit method for the
    /// specific kind of type, ANNOTATION_TYPE, CLASS, ENUM, or
    /// INTERFACE.
    fn visit_type(&mut self, e: &dyn TypeElement, p: P) -> R {
        let k = e.get_kind();
        match k {
            ElementKind::ANNOTATION_TYPE => self.visit_type_as_annotation_type(e, p),
            ElementKind::CLASS => self.visit_type_as_class(e, p),
            ElementKind::ENUM => self.visit_type_as_enum(e, p),
            ElementKind::INTERFACE => self.visit_type_as_interface(e, p),
            ElementKind::RECORD => self.visit_type_as_record(e, p),
            _ => panic!("AssertionError: Bad kind {k} for TypeElement {e}"),
        }
    }

    /// Visits an ANNOTATION_TYPE type element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type_as_annotation_type(&mut self, e: &dyn TypeElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a CLASS type element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type_as_class(&mut self, e: &dyn TypeElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits an ENUM type element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type_as_enum(&mut self, e: &dyn TypeElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits an INTERFACE type element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type_as_interface(&mut self, e: &dyn TypeElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a RECORD type element.
    ///
    /// @implSpec This implementation calls visitUnknown.
    ///
    /// @since 16
    fn visit_type_as_record(&mut self, e: &dyn TypeElement, p: P) -> R {
        <Self as AbstractElementVisitor6<R, P>>::visit_unknown(self, e, p)
    }

    /// Visits a variable element
    ///
    /// @implSpec This implementation dispatches to the visit method for
    /// the specific kind of variable, ENUM_CONSTANT, EXCEPTION_PARAMETER,
    /// FIELD, LOCAL_VARIABLE, PARAMETER, or RESOURCE_VARIABLE.
    fn visit_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        let k = e.get_kind();
        match k {
            ElementKind::ENUM_CONSTANT => self.visit_variable_as_enum_constant(e, p),
            ElementKind::EXCEPTION_PARAMETER => self.visit_variable_as_exception_parameter(e, p),
            ElementKind::FIELD => self.visit_variable_as_field(e, p),
            ElementKind::LOCAL_VARIABLE => self.visit_variable_as_local_variable(e, p),
            ElementKind::PARAMETER => self.visit_variable_as_parameter(e, p),
            ElementKind::RESOURCE_VARIABLE => self.visit_variable_as_resource_variable(e, p),
            ElementKind::BINDING_VARIABLE => self.visit_variable_as_binding_variable(e, p),
            _ => panic!("AssertionError: Bad kind {k} for VariableElement {e}"),
        }
    }

    /// Visits an ENUM_CONSTANT variable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_variable_as_enum_constant(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits an EXCEPTION_PARAMETER variable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_variable_as_exception_parameter(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a FIELD variable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_variable_as_field(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a LOCAL_VARIABLE variable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_variable_as_local_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a PARAMETER variable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_variable_as_parameter(&mut self, e: &dyn VariableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a RESOURCE_VARIABLE variable element.
    ///
    /// @implSpec This implementation calls visitUnknown.
    ///
    /// @since 1.7
    fn visit_variable_as_resource_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        <Self as AbstractElementVisitor6<R, P>>::visit_unknown(self, e, p)
    }

    /// Visits a BINDING_VARIABLE variable element.
    ///
    /// @implSpec This implementation calls visitUnknown.
    ///
    /// @since 14
    fn visit_variable_as_binding_variable(&mut self, e: &dyn VariableElement, p: P) -> R {
        <Self as AbstractElementVisitor6<R, P>>::visit_unknown(self, e, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation dispatches to the visit method
    /// for the specific kind of executable,
    /// CONSTRUCTOR, INSTANCE_INIT, METHOD, or STATIC_INIT.
    fn visit_executable(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        let k = e.get_kind();
        match k {
            ElementKind::CONSTRUCTOR => self.visit_executable_as_constructor(e, p),
            ElementKind::INSTANCE_INIT => self.visit_executable_as_instance_init(e, p),
            ElementKind::METHOD => self.visit_executable_as_method(e, p),
            ElementKind::STATIC_INIT => self.visit_executable_as_static_init(e, p),
            _ => panic!("AssertionError: Bad kind {k} for ExecutableElement {e}"),
        }
    }

    /// Visits a CONSTRUCTOR executable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_executable_as_constructor(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits an INSTANCE_INIT executable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_executable_as_instance_init(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a METHOD executable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_executable_as_method(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// Visits a STATIC_INIT executable element.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_executable_as_static_init(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        self.default_action(e, p)
    }

    /// {@inheritDoc}
    ///
    /// The element argument has kind TYPE_PARAMETER.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type_parameter(&mut self, e: &dyn TypeParameterElement, p: P) -> R {
        debug_assert_eq!(
            e.get_kind(),
            ElementKind::TYPE_PARAMETER,
            "Bad kind on TypeParameterElement"
        );
        self.default_action(e, p)
    }
}

#[cfg(test)]
mod tests {
    use super::ElementKindVisitor6;
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
    impl ElementKindVisitor6<usize, ()> for DefaultVisitor {}

    #[test]
    fn visit_type_dispatches_by_kind() {
        let mut v = DefaultVisitor { value: 7 };
        let class = Fixture::new(ElementKind::CLASS, "C");
        assert_eq!(ElementKindVisitor6::visit_type(&mut v, &class, ()), 7);
        let interface = Fixture::new(ElementKind::INTERFACE, "I");
        assert_eq!(ElementKindVisitor6::visit_type(&mut v, &interface, ()), 7);
    }

    #[test]
    fn visit_variable_dispatches_by_kind() {
        let mut v = DefaultVisitor { value: 7 };
        let field = Fixture::new(ElementKind::FIELD, "f");
        assert_eq!(ElementKindVisitor6::visit_variable(&mut v, &field, ()), 7);
        let parameter = Fixture::new(ElementKind::PARAMETER, "p");
        assert_eq!(
            ElementKindVisitor6::visit_variable(&mut v, &parameter, ()),
            7
        );
        let binding = Fixture::new(ElementKind::BINDING_VARIABLE, "b");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ElementKindVisitor6::visit_variable(&mut v, &binding, ())
        }));
        assert!(result.is_err());
    }
}
