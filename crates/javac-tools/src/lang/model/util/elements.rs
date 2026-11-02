//! Utility methods for operating on program elements.
//!
//! (§6/§14):
//! * Java interface -> Rust trait; Java default methods -> Rust default methods.
//! * Java overloads (getPackageElement/getTypeElement/getOrigin) -> distinct Rust
//!   names (documented per method; §14 overload rule).
//! * `Set<? extends X>` -> `Vec<Rc<dyn X>>` (input order kept); Java Map ->
//!   `Vec<(K, V)>`; `CharSequence`/`String` -> `JavaString`; nullable ->
//!   `Option<...>`; `Element...` -> `&[Rc<dyn Element>]`; `java.io.Writer` ->
//!   `crate::java_io::Writer`.
//! * `Object getConstantExpression(Object)` -> `Value` (the closed representation
//!   of javax.lang.model constant values; Java throws IllegalArgumentException
//!   for non-primitive/string values, the port panics with the same message).
//! * recordComponentFor's Java default implementation cannot be expressed: it
//!   returns an element from a temporary list through ElementFilter's borrowed
//!   references (§13), so the Rust default panics and implementors (P6) must
//!   override it (registered language-shape limitation, like ElementScanner6.scan).
//!
//! <p><b>Compatibility Note:</b> Methods may be added to this interface
//! in future releases of the platform.
//!
//! @author Joseph D. Darcy
//! @author Scott Seligman
//! @author Peter von der Ah&eacute;
//! @see javax.annotation.processing.ProcessingEnvironment#getElementUtils
//! @since 1.6

use super::element_filter::ElementFilter;
use crate::java_io::Writer;
use crate::lang::model::AnnotatedConstruct;
use crate::lang::model::element::{
    AnnotationMirror, AnnotationValue, Element, ExecutableElement, ModuleElement, Name,
    PackageElement, RecordComponentElement, TypeElement, Value,
};
use javac_strings::JavaString;
use std::rc::Rc;

/// Utility methods for operating on program elements.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait Elements {
    /// Returns a package given its fully qualified name if the package is uniquely
    /// determinable in the environment.
    ///
    /// @param name fully qualified package name,
    ///             or an empty string for an unnamed package
    /// @return the specified package,
    ///         or {@code null} if no package can be uniquely determined.
    fn get_package_element(&self, name: &JavaString) -> Option<Rc<dyn PackageElement>>;

    /// Returns a package given its fully qualified name, as seen from the given module.
    ///
    /// @implSpec The default implementation of this method returns {@code null}.
    /// (Java: getPackageElement(ModuleElement, CharSequence))
    ///
    /// @param name  fully qualified package name, or an empty string for an unnamed package
    /// @param module module relative to which the lookup should happen
    /// @return the specified package, or {@code null} if it cannot be found
    /// @since 9
    fn get_package_element_in_module(
        &self,
        module: &dyn ModuleElement,
        name: &JavaString,
    ) -> Option<Rc<dyn PackageElement>> {
        let _ = (module, name);
        None
    }

    /// Returns all package elements with the given canonical name.
    ///
    /// @implSpec The default implementation of this method calls
    /// getAllModuleElements and stores the result. If the set of modules is
    /// empty, getPackageElement(name) is called passing through the name
    /// argument. If it is null, an empty set of package elements is returned;
    /// otherwise, a single-element set with the found package element is
    /// returned. If the set of modules is nonempty, the modules are iterated
    /// over and any non-null results of getPackageElement(module, name) are
    /// accumulated into a set.
    ///
    /// @param name  the canonical name
    /// @return the package elements, or an empty set if no package with the name can be found
    /// @since 9
    fn get_all_package_elements(&self, name: &JavaString) -> Vec<Rc<dyn PackageElement>> {
        let modules = self.get_all_module_elements();
        if modules.is_empty() {
            match self.get_package_element(name) {
                Some(package_elt) => vec![package_elt],
                None => Vec::new(),
            }
        } else {
            let mut result: Vec<Rc<dyn PackageElement>> = Vec::new();
            for module in &modules {
                if let Some(package_elt) = self.get_package_element_in_module(module.as_ref(), name)
                {
                    result.push(package_elt);
                }
            }
            result
        }
    }

    /// Returns a type element given its canonical name if the type element is uniquely
    /// determinable in the environment.
    ///
    /// @param name the canonical name
    /// @return the named type element,
    ///         or {@code null} if no type element can be uniquely determined.
    fn get_type_element(&self, name: &JavaString) -> Option<Rc<dyn TypeElement>>;

    /// Returns a type element given its canonical name, as seen from the given module.
    ///
    /// @implSpec The default implementation of this method returns {@code null}.
    /// (Java: getTypeElement(ModuleElement, CharSequence))
    ///
    /// @param name  the canonical name
    /// @param module module relative to which the lookup should happen
    /// @return the named type element, or {@code null} if it cannot be found
    /// @since 9
    fn get_type_element_in_module(
        &self,
        module: &dyn ModuleElement,
        name: &JavaString,
    ) -> Option<Rc<dyn TypeElement>> {
        let _ = (module, name);
        None
    }

    /// Returns all type elements with the given canonical name.
    ///
    /// @implSpec The default implementation follows the same two-stage search as
    /// getAllPackageElements: if there are no modules, getTypeElement(name) is
    /// used (single-element or empty result), otherwise the modules are iterated
    /// and non-null getTypeElement(module, name) results are accumulated.
    ///
    /// @param name  the canonical name
    /// @return the type elements, or an empty set if no type with the name can be found
    /// @since 9
    fn get_all_type_elements(&self, name: &JavaString) -> Vec<Rc<dyn TypeElement>> {
        let modules = self.get_all_module_elements();
        if modules.is_empty() {
            match self.get_type_element(name) {
                Some(type_elt) => vec![type_elt],
                None => Vec::new(),
            }
        } else {
            let mut result: Vec<Rc<dyn TypeElement>> = Vec::new();
            for module in &modules {
                if let Some(type_elt) = self.get_type_element_in_module(module.as_ref(), name) {
                    result.push(type_elt);
                }
            }
            result
        }
    }

    /// Returns a module element given its fully qualified name.
    ///
    /// @implSpec The default implementation of this method returns {@code null}.
    ///
    /// @param name  the name, or an empty string for an unnamed module
    /// @return the named module element, or {@code null} if it cannot be found
    /// @since 9
    fn get_module_element(&self, name: &JavaString) -> Option<Rc<dyn ModuleElement>> {
        let _ = name;
        None
    }

    /// Returns all module elements in the current environment.
    ///
    /// @implSpec The default implementation of this method returns an empty set.
    ///
    /// @return the known module elements, or an empty set if there are no modules
    /// @since 9
    fn get_all_module_elements(&self) -> Vec<Rc<dyn ModuleElement>> {
        Vec::new()
    }

    /// Returns the values of an annotation's elements, including defaults.
    ///
    /// @see AnnotationMirror#getElementValues()
    /// @param a  annotation to examine
    fn get_element_values_with_defaults(
        &self,
        a: &dyn AnnotationMirror,
    ) -> Vec<(Rc<dyn ExecutableElement>, Rc<dyn AnnotationValue>)>;

    /// Returns the text of the documentation ("Javadoc") comment of an element.
    ///
    /// @param e  the element being examined
    /// @return the documentation comment of the element, or {@code null}
    ///          if there is none
    /// @jls 3.6 White Space
    fn get_doc_comment(&self, e: &dyn Element) -> Option<JavaString>;

    /// Returns {@code true} if the element is deprecated, {@code false} otherwise.
    ///
    /// @param e  the element being examined
    fn is_deprecated(&self, e: &dyn Element) -> bool;

    /// Returns the origin of the given element.
    ///
    /// @implSpec The default implementation of this method returns {@code EXPLICIT}.
    ///
    /// @param e  the element being examined
    /// @since 9
    fn get_origin(&self, e: &dyn Element) -> Origin {
        let _ = e;
        Origin::EXPLICIT
    }

    /// Returns the origin of the given annotation mirror.
    ///
    /// @implSpec The default implementation of this method returns {@code EXPLICIT}.
    /// (Java: getOrigin(AnnotatedConstruct, AnnotationMirror))
    ///
    /// @param c the construct the annotation mirror modifies
    /// @param a the annotation mirror being examined
    /// @since 9
    fn get_origin_for_construct(
        &self,
        c: &dyn AnnotatedConstruct,
        a: &dyn AnnotationMirror,
    ) -> Origin {
        let _ = (c, a);
        Origin::EXPLICIT
    }

    /// Returns the origin of the given module directive.
    ///
    /// @implSpec The default implementation of this method returns {@code EXPLICIT}.
    /// (Java: getOrigin(ModuleElement, ModuleElement.Directive))
    ///
    /// @param m the module of the directive
    /// @param directive  the module directive being examined
    /// @since 9
    fn get_origin_for_directive(
        &self,
        m: &dyn ModuleElement,
        directive: &dyn crate::lang::model::element::Directive,
    ) -> Origin {
        let _ = (m, directive);
        Origin::EXPLICIT
    }

    /// Returns {@code true} if the executable element is a bridge method,
    /// {@code false} otherwise.
    ///
    /// @implSpec The default implementation of this method returns {@code false}.
    ///
    /// @param e  the executable being examined
    /// @since 9
    fn is_bridge(&self, e: &dyn ExecutableElement) -> bool {
        let _ = e;
        false
    }

    /// Returns the binary name of a type element.
    ///
    /// @param type  the type element being examined
    /// @see TypeElement#getQualifiedName
    /// @jls 13.1 The Form of a Binary
    fn get_binary_name(&self, type_: &dyn TypeElement) -> Rc<dyn Name>;

    /// Returns the package of an element.  The package of a package is itself.
    /// The package of a module is {@code null}.
    ///
    /// @param e the element being examined
    fn get_package_of(&self, e: &dyn Element) -> Option<Rc<dyn PackageElement>>;

    /// Returns the module of an element.  The module of a module is itself.
    ///
    /// @implSpec The default implementation of this method returns {@code null}.
    ///
    /// @param e the element being examined
    /// @since 9
    fn get_module_of(&self, e: &dyn Element) -> Option<Rc<dyn ModuleElement>> {
        let _ = e;
        None
    }

    /// Returns all members of a type element, whether inherited or declared
    /// directly.
    ///
    /// @param type  the type being examined
    /// @return all members of the type
    /// @see Element#getEnclosedElements
    fn get_all_members(&self, type_: &dyn TypeElement) -> Vec<Rc<dyn Element>>;

    /// Returns all annotations present on an element, whether directly present
    /// or present via inheritance.
    ///
    /// @param e  the element being examined
    /// @return all annotations of the element
    /// @see Element#getAnnotationMirrors
    /// @see javax.lang.model.AnnotatedConstruct
    fn get_all_annotation_mirrors(&self, e: &dyn Element) -> Vec<Rc<dyn AnnotationMirror>>;

    /// Tests whether one type, method, or field hides another.
    ///
    /// @param hider   the first element
    /// @param hidden  the second element
    /// @return {@code true} if and only if the first element hides the second
    /// @jls 8.4.8 Inheritance, Overriding, and Hiding
    fn hides(&self, hider: &dyn Element, hidden: &dyn Element) -> bool;

    /// Tests whether one method, as a member of a given class or interface,
    /// overrides another method.
    ///
    /// @param overrider  the first method, possible overrider
    /// @param overridden  the second method, possibly being overridden
    /// @param type   the class or interface of which the first method is a member
    /// @return {@code true} if and only if the first method overrides the second
    /// @jls 8.4.8 Inheritance, Overriding, and Hiding
    /// @jls 9.4.1 Inheritance and Overriding
    fn overrides(
        &self,
        overrider: &dyn ExecutableElement,
        overridden: &dyn ExecutableElement,
        type_: &dyn TypeElement,
    ) -> bool;

    /// Returns the text of a constant expression representing a primitive value
    /// or a string.  The text returned is in a form suitable for representing
    /// the value in source code.
    ///
    /// @param value  a primitive value or string
    /// @return the text of a constant expression
    /// @throws IllegalArgumentException if the argument is not a primitive value or string
    /// @see VariableElement#getConstantValue()
    fn get_constant_expression(&self, value: &Value) -> JavaString;

    /// Prints a representation of the elements to the given writer in the
    /// specified order.
    ///
    /// @param w the writer to print the output to
    /// @param elements the elements to print
    fn print_elements(&self, w: &mut dyn Writer, elements: &[Rc<dyn Element>]);

    /// Returns a name with the same sequence of characters as the argument.
    ///
    /// @param cs the character sequence to return as a name
    fn get_name(&self, cs: &JavaString) -> Rc<dyn Name>;

    /// Returns {@code true} if the type element is a functional interface,
    /// {@code false} otherwise.
    ///
    /// @param type the type element being examined
    /// @jls 9.8 Functional Interfaces
    /// @since 1.8
    fn is_functional_interface(&self, type_: &dyn TypeElement) -> bool;

    /// Returns {@code true} if the module element is an automatic module,
    /// {@code false} otherwise.
    ///
    /// @implSpec The default implementation of this method returns {@code false}.
    ///
    /// @param module the module element being examined
    /// @jls 7.7.1 Dependences
    /// @since 17
    fn is_automatic_module(&self, module: &dyn ModuleElement) -> bool {
        let _ = module;
        false
    }

    /// Returns the record component for the given accessor. Returns null if the
    /// given method is not a record component accessor.
    ///
    /// the Java default implementation returns a component from a
    /// temporary list through ElementFilter's borrowed references (§13), which
    /// cannot produce an owned Rc; the Rust default panics and implementors (P6)
    /// must override it (registered language-shape limitation).
    ///
    /// @param accessor the method for which the record component should be found.
    /// @return the record component, or null if the given method is not an record
    /// component accessor
    /// @since 16
    fn record_component_for(
        &self,
        accessor: &dyn ExecutableElement,
    ) -> Option<Rc<dyn RecordComponentElement>> {
        let _ = (accessor, ElementFilter::record_components_in(&[]));
        panic!(
            "AbstractMethodError: record_component_for must be overridden (the Java default \
             uses borrowed ElementFilter results and cannot return an owned element)"
        )
    }
}

/// The origin of an element or other language model item.
///
/// Java nested enum Elements.Origin -> elements::Origin.
///
/// @jls 13.1 The Form of a Binary
/// @since 9
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Describes a construct explicitly declared in source code.
    EXPLICIT,
    /// A mandated construct is one that is not explicitly declared in the source
    /// code, but whose presence is mandated by the specification.
    MANDATED,
    /// A synthetic construct is one that is neither implicitly nor explicitly
    /// declared in the source code.
    SYNTHETIC,
}

impl Origin {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Origin::EXPLICIT => "EXPLICIT",
            Origin::MANDATED => "MANDATED",
            Origin::SYNTHETIC => "SYNTHETIC",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [Origin; 3] {
        [Origin::EXPLICIT, Origin::MANDATED, Origin::SYNTHETIC]
    }

    /// Returns {@code true} for values corresponding to constructs that are
    /// implicitly or explicitly declared, {@code false} otherwise.
    /// @return {@code true} for {@link #EXPLICIT} and {@link #MANDATED},
    ///         {@code false} otherwise.
    pub fn is_declared(&self) -> bool {
        *self != Origin::SYNTHETIC
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::{Elements, Origin};
    use crate::java_io::Writer;
    use crate::lang::model::AnnotatedConstruct;
    use crate::lang::model::element::{
        AnnotationMirror, AnnotationValue, Element, ExecutableElement, Name, PackageElement,
        TypeElement, Value,
    };
    use javac_strings::JavaString;
    use std::rc::Rc;

    struct FakeElements;

    impl Elements for FakeElements {
        fn get_package_element(&self, _name: &JavaString) -> Option<Rc<dyn PackageElement>> {
            None
        }
        fn get_type_element(&self, _name: &JavaString) -> Option<Rc<dyn TypeElement>> {
            None
        }
        fn get_element_values_with_defaults(
            &self,
            _a: &dyn AnnotationMirror,
        ) -> Vec<(Rc<dyn ExecutableElement>, Rc<dyn AnnotationValue>)> {
            Vec::new()
        }
        fn get_doc_comment(&self, _e: &dyn Element) -> Option<JavaString> {
            None
        }
        fn is_deprecated(&self, _e: &dyn Element) -> bool {
            false
        }
        fn get_binary_name(&self, _type_: &dyn TypeElement) -> Rc<dyn Name> {
            panic!("UnsupportedOperationException: get_binary_name: test fixture")
        }
        fn get_package_of(&self, _e: &dyn Element) -> Option<Rc<dyn PackageElement>> {
            None
        }
        fn get_all_members(&self, _type_: &dyn TypeElement) -> Vec<Rc<dyn Element>> {
            Vec::new()
        }
        fn get_all_annotation_mirrors(&self, _e: &dyn Element) -> Vec<Rc<dyn AnnotationMirror>> {
            Vec::new()
        }
        fn hides(&self, _hider: &dyn Element, _hidden: &dyn Element) -> bool {
            false
        }
        fn overrides(
            &self,
            _overrider: &dyn ExecutableElement,
            _overridden: &dyn ExecutableElement,
            _type_: &dyn TypeElement,
        ) -> bool {
            false
        }
        fn get_constant_expression(&self, _value: &Value) -> JavaString {
            panic!("UnsupportedOperationException: get_constant_expression: test fixture")
        }
        fn print_elements(&self, _w: &mut dyn Writer, _elements: &[Rc<dyn Element>]) {}
        fn get_name(&self, cs: &JavaString) -> Rc<dyn Name> {
            panic!("UnsupportedOperationException: get_name")
        }
        fn is_functional_interface(&self, _type_: &dyn TypeElement) -> bool {
            false
        }
    }

    #[test]
    fn elements_is_object_safe_with_java_defaults() {
        let elements: &dyn Elements = &FakeElements;
        let name = JavaString::from("java.lang");
        assert!(elements.get_package_element(&name).is_none());
        assert!(elements.get_type_element(&name).is_none());
        // Java default methods keep their upstream behavior.
        assert!(elements.get_module_element(&name).is_none());
        assert!(elements.get_all_module_elements().is_empty());
        assert!(elements.get_all_package_elements(&name).is_empty());
        assert!(elements.get_all_type_elements(&name).is_empty());
        assert!(elements.get_module_of(&DummyElement).is_none());
        assert_eq!(elements.get_origin(&DummyElement), Origin::EXPLICIT);
        assert!(
            elements
                .get_origin_for_construct(&DummyConstruct, &DummyAnnotation)
                .eq(&Origin::EXPLICIT)
        );
        let mut out = NullWriter;
        elements.print_elements(&mut out, &[]);
    }

    #[test]
    fn origin_enum_shape() {
        assert_eq!(Origin::values().len(), 3);
        assert_eq!(Origin::EXPLICIT.name().to_string_lossy(), "EXPLICIT");
        assert_eq!(Origin::SYNTHETIC.to_string(), "SYNTHETIC");
        assert!(Origin::EXPLICIT.is_declared());
        assert!(Origin::MANDATED.is_declared());
        assert!(!Origin::SYNTHETIC.is_declared());
    }

    // Fixtures: only the trait-object identity matters for the default methods.
    struct NullWriter;

    impl Writer for NullWriter {
        fn write(&mut self, _cbuf: &[u16]) -> Result<(), crate::error::Error> {
            Ok(())
        }
        fn flush(&mut self) -> Result<(), crate::error::Error> {
            Ok(())
        }
        fn close(&mut self) -> Result<(), crate::error::Error> {
            Ok(())
        }
    }

    struct DummyElement;
    struct DummyAnnotation;
    struct DummyConstruct;

    impl std::fmt::Display for DummyElement {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("dummy")
        }
    }
    impl std::fmt::Display for DummyAnnotation {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("dummy")
        }
    }
    impl std::fmt::Display for DummyConstruct {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("dummy")
        }
    }

    impl AnnotatedConstruct for DummyConstruct {
        fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
            Vec::new()
        }
        fn get_annotation<A: crate::jdk::Annotation>(
            &self,
            _annotation_type: crate::jdk::Class<A>,
        ) -> Option<A> {
            None
        }
        fn get_annotations_by_type<A: crate::jdk::Annotation>(
            &self,
            _annotation_type: crate::jdk::Class<A>,
        ) -> Vec<A> {
            Vec::new()
        }
    }

    impl AnnotatedConstruct for DummyElement {
        fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
            Vec::new()
        }
        fn get_annotation<A: crate::jdk::Annotation>(
            &self,
            _annotation_type: crate::jdk::Class<A>,
        ) -> Option<A> {
            None
        }
        fn get_annotations_by_type<A: crate::jdk::Annotation>(
            &self,
            _annotation_type: crate::jdk::Class<A>,
        ) -> Vec<A> {
            Vec::new()
        }
    }

    impl Element for DummyElement {
        fn as_type(&self) -> Rc<dyn crate::lang::model::type_::TypeMirror> {
            panic!("UnsupportedOperationException: as_type: test fixture")
        }
        fn get_kind(&self) -> crate::lang::model::element::ElementKind {
            crate::lang::model::element::ElementKind::OTHER
        }
        fn get_modifiers(&self) -> Vec<crate::lang::model::element::Modifier> {
            Vec::new()
        }
        fn get_simple_name(&self) -> Rc<dyn Name> {
            panic!("UnsupportedOperationException: get_simple_name: test fixture")
        }
        fn get_enclosing_element(&self) -> Option<Rc<dyn Element>> {
            None
        }
        fn get_enclosed_elements(&self) -> Vec<Rc<dyn Element>> {
            Vec::new()
        }
        fn accept<R, P>(
            &self,
            _v: &mut dyn crate::lang::model::element::ElementVisitor<R, P>,
            _p: P,
        ) -> R {
            panic!("UnsupportedOperationException: accept: test fixture")
        }
    }

    impl AnnotationMirror for DummyAnnotation {
        fn get_annotation_type(&self) -> Rc<dyn crate::lang::model::type_::DeclaredType> {
            panic!("UnsupportedOperationException: get_annotation_type: test fixture")
        }
        fn get_element_values(&self) -> Vec<(Rc<dyn ExecutableElement>, Rc<dyn AnnotationValue>)> {
            Vec::new()
        }
    }
}
