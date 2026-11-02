// Rust: the explicit 'a lifetimes express the Java contract that the
// returned collections borrow from the argument (clippy::needless_lifetimes is
// intentionally allowed).
#![allow(clippy::needless_lifetimes)]


//! Filters for selecting just the elements of interest from a collection of
//! elements.
//!
//! — signature mapping:
//! * `Iterable<? extends Element>` / `Set<? extends Element>` arguments ->
//!   `&[Rc<dyn Element>]`; returns -> `Vec<&dyn X>` borrowing from the
//!   argument (Java returns new collections holding the same element
//!   references; the Rust borrow makes the lifetime explicit).
//! * `Iterable<? extends Directive>` -> `&[Rc<dyn Directive>]`; returns ->
//!   `Vec<&dyn X>`.
//! * Java `clazz.cast(e)` after the kind check needs instanceof: the
//!   Element/Directive object-conversion accessors (as_xxx) are used, in the
//!   same order as the upstream implementation.
//! * `Set<ElementKind>` constants -> `&[ElementKind]`; the Java Set overloads
//!   return a LinkedHashSet (input iteration order preserved), which maps to a
//!   Vec that keeps the input order; the Rust overloads therefore share the
//!   same filtering helper but keep the two Java methods with distinct names
//!   (docs/PORTING_RULES.md §3).
//! * `ElementFilter` is a non-instantiable utility class -> struct with a
//!   private field and associated functions.

use crate::lang::model::element::{
    Directive, DirectiveKind, Element, ElementKind, ExecutableElement, ModuleElement,
    PackageElement, RecordComponentElement, TypeElement, VariableElement,
};
use std::rc::Rc;

/// Filters for selecting just the elements of interest from a
/// collection of elements.  The returned sets and lists are new
/// collections and do use the argument as a backing store.  The
/// methods in this class do not make any attempts to guard against
/// concurrent modifications of the arguments.  The returned sets and
/// lists are mutable but unsafe for concurrent access.  A returned set
/// has the same iteration order as the argument set to a method.
///
/// <p>If iterables and sets containing null are passed as
/// arguments to methods in this class, a NullPointerException
/// will be thrown.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @author Martin Buchholz
/// @since 1.6
pub struct ElementFilter(());

impl ElementFilter {
    // Do not instantiate.

    const CONSTRUCTOR_KIND: &'static [ElementKind] = &[ElementKind::CONSTRUCTOR];

    const FIELD_KINDS: &'static [ElementKind] =
        &[ElementKind::FIELD, ElementKind::ENUM_CONSTANT];
    const METHOD_KIND: &'static [ElementKind] = &[ElementKind::METHOD];

    const PACKAGE_KIND: &'static [ElementKind] = &[ElementKind::PACKAGE];

    const MODULE_KIND: &'static [ElementKind] = &[ElementKind::MODULE];

    const TYPE_KINDS: &'static [ElementKind] = &[
        ElementKind::CLASS,
        ElementKind::ENUM,
        ElementKind::INTERFACE,
        ElementKind::RECORD,
        ElementKind::ANNOTATION_TYPE,
    ];

    const RECORD_COMPONENT_KIND: &'static [ElementKind] = &[ElementKind::RECORD_COMPONENT];

    /// {@return a list of fields in elements}
    /// @param elements the elements to filter
    pub fn fields_in<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn VariableElement> {
        Self::list_filter(elements, Self::FIELD_KINDS, |e| e.as_variable_element())
    }

    /// {@return a set of fields in elements}
    /// @param elements the elements to filter
    ///
    /// overload `fieldsIn(Set<? extends Element>)`.
    pub fn fields_in_set<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn VariableElement> {
        Self::set_filter(elements, Self::FIELD_KINDS, |e| e.as_variable_element())
    }

    /// {@return a list of record components in elements}
    /// @param elements the elements to filter
    /// @since 16
    pub fn record_components_in<'a>(
        elements: &'a [Rc<dyn Element>],
    ) -> Vec<&'a dyn RecordComponentElement> {
        Self::list_filter(elements, Self::RECORD_COMPONENT_KIND, |e| {
            e.as_record_component_element()
        })
    }

    /// {@return a set of record components in elements}
    /// @param elements the elements to filter
    /// @since 16
    ///
    /// overload `recordComponentsIn(Set<? extends Element>)`.
    pub fn record_components_in_set<'a>(
        elements: &'a [Rc<dyn Element>],
    ) -> Vec<&'a dyn RecordComponentElement> {
        Self::set_filter(elements, Self::RECORD_COMPONENT_KIND, |e| {
            e.as_record_component_element()
        })
    }

    /// {@return a list of constructors in elements}
    /// @param elements the elements to filter
    pub fn constructors_in<'a>(
        elements: &'a [Rc<dyn Element>],
    ) -> Vec<&'a dyn ExecutableElement> {
        Self::list_filter(elements, Self::CONSTRUCTOR_KIND, |e| e.as_executable_element())
    }

    /// {@return a set of constructors in elements}
    /// @param elements the elements to filter
    ///
    /// overload `constructorsIn(Set<? extends Element>)`.
    pub fn constructors_in_set<'a>(
        elements: &'a [Rc<dyn Element>],
    ) -> Vec<&'a dyn ExecutableElement> {
        Self::set_filter(elements, Self::CONSTRUCTOR_KIND, |e| e.as_executable_element())
    }

    /// {@return a list of methods in elements}
    /// @param elements the elements to filter
    pub fn methods_in<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn ExecutableElement> {
        Self::list_filter(elements, Self::METHOD_KIND, |e| e.as_executable_element())
    }

    /// {@return a set of methods in elements}
    /// @param elements the elements to filter
    ///
    /// overload `methodsIn(Set<? extends Element>)`.
    pub fn methods_in_set<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn ExecutableElement> {
        Self::set_filter(elements, Self::METHOD_KIND, |e| e.as_executable_element())
    }

    /// {@return a list of classes and interfaces in elements}
    /// @param elements the elements to filter
    pub fn types_in<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn TypeElement> {
        Self::list_filter(elements, Self::TYPE_KINDS, |e| e.as_type_element())
    }

    /// {@return a set of types in elements}
    /// @param elements the elements to filter
    ///
    /// overload `typesIn(Set<? extends Element>)`.
    pub fn types_in_set<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn TypeElement> {
        Self::set_filter(elements, Self::TYPE_KINDS, |e| e.as_type_element())
    }

    /// {@return a list of packages in elements}
    /// @param elements the elements to filter
    pub fn packages_in<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn PackageElement> {
        Self::list_filter(elements, Self::PACKAGE_KIND, |e| e.as_package_element())
    }

    /// {@return a set of packages in elements}
    /// @param elements the elements to filter
    ///
    /// overload `packagesIn(Set<? extends Element>)`.
    pub fn packages_in_set<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn PackageElement> {
        Self::set_filter(elements, Self::PACKAGE_KIND, |e| e.as_package_element())
    }

    /// {@return a list of modules in elements}
    /// @param elements the elements to filter
    /// @since 9
    pub fn modules_in<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn ModuleElement> {
        Self::list_filter(elements, Self::MODULE_KIND, |e| e.as_module_element())
    }

    /// {@return a set of modules in elements}
    /// @param elements the elements to filter
    /// @since 9
    ///
    /// overload `modulesIn(Set<? extends Element>)`.
    pub fn modules_in_set<'a>(elements: &'a [Rc<dyn Element>]) -> Vec<&'a dyn ModuleElement> {
        Self::set_filter(elements, Self::MODULE_KIND, |e| e.as_module_element())
    }

    // Assumes targetKinds and X are sensible.
    //
    // Java: private static <E extends Element> List<E> listFilter(...)
    fn list_filter<'a, X: ?Sized>(
        elements: &'a [Rc<dyn Element>],
        target_kinds: &[ElementKind],
        select: impl Fn(&'a dyn Element) -> Option<&'a X>,
    ) -> Vec<&'a X> {
        let mut list: Vec<&'a X> = Vec::new();
        for element in elements {
            let element: &'a dyn Element = element.as_ref();
            if target_kinds.contains(&element.get_kind()) {
                list.push(cast(select(element), element));
            }
        }
        list
    }

    // Assumes targetKinds and X are sensible.
    //
    // Java: private static <E extends Element> Set<E> setFilter(...)
    // Rust: the Java LinkedHashSet preserves the input iteration order
    // and the input is a Set (no duplicates), which a Vec over the slice
    // reproduces exactly.
    fn set_filter<'a, X: ?Sized>(
        elements: &'a [Rc<dyn Element>],
        target_kinds: &[ElementKind],
        select: impl Fn(&'a dyn Element) -> Option<&'a X>,
    ) -> Vec<&'a X> {
        Self::list_filter(elements, target_kinds, select)
    }

    /// {@return a list of exports directives in directives}
    /// @param directives the directives to filter
    /// @since 9
    pub fn exports_in<'a>(
        directives: &'a [Rc<dyn Directive>],
    ) -> Vec<&'a dyn crate::lang::model::element::ExportsDirective> {
        Self::list_filter_directives(directives, DirectiveKind::EXPORTS, |d| {
            d.as_exports_directive()
        })
    }

    /// {@return a list of opens directives in directives}
    /// @param directives the directives to filter
    /// @since 9
    pub fn opens_in<'a>(
        directives: &'a [Rc<dyn Directive>],
    ) -> Vec<&'a dyn crate::lang::model::element::OpensDirective> {
        Self::list_filter_directives(directives, DirectiveKind::OPENS, |d| d.as_opens_directive())
    }

    /// {@return a list of provides directives in directives}
    /// @param directives the directives to filter
    /// @since 9
    pub fn provides_in<'a>(
        directives: &'a [Rc<dyn Directive>],
    ) -> Vec<&'a dyn crate::lang::model::element::ProvidesDirective> {
        Self::list_filter_directives(directives, DirectiveKind::PROVIDES, |d| {
            d.as_provides_directive()
        })
    }

    /// {@return a list of requires directives in directives}
    /// @param directives the directives to filter
    /// @since 9
    pub fn requires_in<'a>(
        directives: &'a [Rc<dyn Directive>],
    ) -> Vec<&'a dyn crate::lang::model::element::RequiresDirective> {
        Self::list_filter_directives(directives, DirectiveKind::REQUIRES, |d| {
            d.as_requires_directive()
        })
    }

    /// {@return a list of uses directives in directives}
    /// @param directives the directives to filter
    /// @since 9
    pub fn uses_in<'a>(
        directives: &'a [Rc<dyn Directive>],
    ) -> Vec<&'a dyn crate::lang::model::element::UsesDirective> {
        Self::list_filter_directives(directives, DirectiveKind::USES, |d| d.as_uses_directive())
    }

    // Assumes directiveKind and D are sensible.
    //
    // Java: private static <D extends Directive> List<D> listFilter(...)
    // Rust: overload `listFilter(Iterable of Directive, ...)`.
    fn list_filter_directives<'a, D: ?Sized>(
        directives: &'a [Rc<dyn Directive>],
        directive_kind: DirectiveKind,
        select: impl Fn(&'a dyn Directive) -> Option<&'a D>,
    ) -> Vec<&'a D> {
        let mut list: Vec<&'a D> = Vec::new();
        for directive in directives {
            let directive: &'a dyn Directive = directive.as_ref();
            if directive.get_kind() == directive_kind {
                list.push(cast_directive(select(directive), directive));
            }
        }
        list
    }
}

/// Java: `clazz.cast(e)` — the element was already selected by kind, so a
/// missing conversion is an implementation contract violation (never silently
/// dropped): panic instead of returning a wrong result.
fn cast<'a, X: ?Sized>(selected: Option<&'a X>, element: &dyn Element) -> &'a X {
    selected.unwrap_or_else(|| {
        panic!(
            "ClassCastException: element of kind {} does not provide the requested \
             subinterface (PORTING_RULES: concrete Element implementations must \
             override the as_xxx accessor for their kind)",
            element.get_kind()
        )
    })
}

/// Java: `clazz.cast(d)` for directives — see `cast`.
fn cast_directive<'a, D: ?Sized>(selected: Option<&'a D>, directive: &dyn Directive) -> &'a D {
    selected.unwrap_or_else(|| {
        panic!(
            "ClassCastException: directive of kind {} does not provide the requested \
             subinterface (PORTING_RULES: concrete Directive implementations must \
             override the as_xxx accessor for their kind)",
            directive.get_kind()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::ElementFilter;
    use crate::lang::model::element::{
        Directive, DirectiveKind, Element, ElementKind, ExecutableElement, ExportsDirective,
        Modifier, ModuleElement, Name, NestingKind, PackageElement, Parameterizable,
        QualifiedNameable, RecordComponentElement, TypeElement, TypeParameterElement,
        VariableElement,
    };
    use crate::lang::model::type_::TypeMirror;
    use crate::lang::model::AnnotatedConstruct;
    use crate::jdk::{Annotation, Class};
    use javac_strings::JavaString;
    use std::fmt;
    use std::rc::Rc;

    struct FixtureName;

    impl Name for FixtureName {
        fn length(&self) -> usize {
            0
        }
        fn char_at(&self, _index: usize) -> u16 {
            panic!("empty name")
        }
        fn sub_sequence(&self, _start: usize, _end: usize) -> JavaString {
            JavaString::new()
        }
        fn to_java_string(&self) -> JavaString {
            JavaString::new()
        }
        fn content_equals(&self, cs: &JavaString) -> bool {
            cs.is_empty()
        }
    }

    struct Fixture {
        kind: ElementKind,
    }

    impl fmt::Display for Fixture {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "fixture[{}]", self.kind)
        }
    }

    impl AnnotatedConstruct for Fixture {
        fn get_annotation_mirrors(&self) -> Vec<Rc<dyn crate::lang::model::element::AnnotationMirror>> {
            Vec::new()
        }
        fn get_annotation<A: Annotation>(&self, _annotation_type: Class<A>) -> Option<A> {
            None
        }
        fn get_annotations_by_type<A: Annotation>(&self, _annotation_type: Class<A>) -> Vec<A> {
            Vec::new()
        }
    }

    impl Element for Fixture {
        fn as_type(&self) -> Rc<dyn TypeMirror> {
            panic!("UnsupportedOperationException: as_type: fixture has no type mirror")
        }
        fn get_kind(&self) -> ElementKind {
            self.kind
        }
        fn get_modifiers(&self) -> Vec<Modifier> {
            Vec::new()
        }
        fn get_simple_name(&self) -> Rc<dyn Name> {
            Rc::new(FixtureName)
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
            panic!("UnsupportedOperationException: accept: fixture is not visited")
        }
        fn as_package_element(&self) -> Option<&dyn PackageElement> {
            (self.kind == ElementKind::PACKAGE).then_some(self as &dyn PackageElement)
        }
        fn as_module_element(&self) -> Option<&dyn ModuleElement> {
            (self.kind == ElementKind::MODULE).then_some(self as &dyn ModuleElement)
        }
        fn as_type_element(&self) -> Option<&dyn TypeElement> {
            (self.kind.is_class() || self.kind.is_interface())
                .then_some(self as &dyn TypeElement)
        }
        fn as_variable_element(&self) -> Option<&dyn VariableElement> {
            matches!(
                self.kind,
                ElementKind::FIELD
                    | ElementKind::ENUM_CONSTANT
                    | ElementKind::PARAMETER
                    | ElementKind::LOCAL_VARIABLE
                    | ElementKind::EXCEPTION_PARAMETER
                    | ElementKind::RESOURCE_VARIABLE
                    | ElementKind::BINDING_VARIABLE
            )
            .then_some(self as &dyn VariableElement)
        }
        fn as_executable_element(&self) -> Option<&dyn ExecutableElement> {
            matches!(
                self.kind,
                ElementKind::METHOD
                    | ElementKind::CONSTRUCTOR
                    | ElementKind::STATIC_INIT
                    | ElementKind::INSTANCE_INIT
            )
            .then_some(self as &dyn ExecutableElement)
        }
        fn as_type_parameter_element(&self) -> Option<&dyn TypeParameterElement> {
            (self.kind == ElementKind::TYPE_PARAMETER).then_some(self as &dyn TypeParameterElement)
        }
        fn as_record_component_element(&self) -> Option<&dyn RecordComponentElement> {
            (self.kind == ElementKind::RECORD_COMPONENT)
                .then_some(self as &dyn RecordComponentElement)
        }
    }

    impl Parameterizable for Fixture {
        fn get_type_parameters(&self) -> Vec<Rc<dyn TypeParameterElement>> {
            Vec::new()
        }
    }

    impl QualifiedNameable for Fixture {
        fn get_qualified_name(&self) -> Rc<dyn Name> {
            Rc::new(FixtureName)
        }
    }

    impl VariableElement for Fixture {
        fn get_constant_value(&self) -> Option<crate::lang::model::element::ConstantValue> {
            None
        }
    }

    impl ExecutableElement for Fixture {
        fn get_parameters(&self) -> Vec<Rc<dyn VariableElement>> {
            Vec::new()
        }
        fn get_return_type(&self) -> Rc<dyn TypeMirror> {
            panic!("UnsupportedOperationException: get_return_type: fixture has no type mirror")
        }
        fn get_receiver_type(&self) -> Rc<dyn TypeMirror> {
            panic!("UnsupportedOperationException: get_receiver_type: fixture has no type mirror")
        }
        fn is_var_args(&self) -> bool {
            false
        }
        fn is_default(&self) -> bool {
            false
        }
        fn get_thrown_types(&self) -> Vec<Rc<dyn TypeMirror>> {
            Vec::new()
        }
        fn get_default_value(
            &self,
        ) -> Option<Rc<dyn crate::lang::model::element::AnnotationValue>> {
            None
        }
    }

    impl TypeElement for Fixture {
        fn get_nesting_kind(&self) -> NestingKind {
            NestingKind::TOP_LEVEL
        }
        fn get_superclass(&self) -> Rc<dyn TypeMirror> {
            panic!("UnsupportedOperationException: get_superclass: fixture has no type mirror")
        }
        fn get_interfaces(&self) -> Vec<Rc<dyn TypeMirror>> {
            Vec::new()
        }
    }

    impl PackageElement for Fixture {
        fn is_unnamed(&self) -> bool {
            false
        }
    }

    impl ModuleElement for Fixture {
        fn is_open(&self) -> bool {
            false
        }
        fn is_unnamed(&self) -> bool {
            false
        }
        fn get_directives(&self) -> Vec<Rc<dyn Directive>> {
            Vec::new()
        }
    }

    impl TypeParameterElement for Fixture {
        fn get_generic_element(&self) -> Rc<dyn Element> {
            panic!("UnsupportedOperationException: get_generic_element: fixture")
        }
        fn get_bounds(&self) -> Vec<Rc<dyn TypeMirror>> {
            Vec::new()
        }
    }

    impl RecordComponentElement for Fixture {
        fn get_accessor(&self) -> Rc<dyn ExecutableElement> {
            panic!("UnsupportedOperationException: get_accessor: fixture")
        }
    }

    fn element(kind: ElementKind) -> Rc<dyn Element> {
        Rc::new(Fixture { kind })
    }

    #[test]
    fn filters_fields_and_methods_in_order() {
        let elements = vec![
            element(ElementKind::METHOD),
            element(ElementKind::FIELD),
            element(ElementKind::ENUM_CONSTANT),
            element(ElementKind::CLASS),
            element(ElementKind::PACKAGE),
        ];
        let fields = ElementFilter::fields_in(&elements);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].get_kind(), ElementKind::FIELD);
        assert_eq!(fields[1].get_kind(), ElementKind::ENUM_CONSTANT);
        let methods = ElementFilter::methods_in(&elements);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].get_kind(), ElementKind::METHOD);
        let types = ElementFilter::types_in(&elements);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].get_kind(), ElementKind::CLASS);
        let packages = ElementFilter::packages_in(&elements);
        assert_eq!(packages.len(), 1);
        // the Java Set overloads keep the input iteration order as well.
        let fields_set = ElementFilter::fields_in_set(&elements);
        assert_eq!(fields_set.len(), 2);
    }

    #[test]
    fn filters_constructors_and_modules() {
        let elements = vec![
            element(ElementKind::CONSTRUCTOR),
            element(ElementKind::STATIC_INIT),
            element(ElementKind::MODULE),
        ];
        assert_eq!(ElementFilter::constructors_in(&elements).len(), 1);
        assert_eq!(ElementFilter::constructors_in_set(&elements).len(), 1);
        assert_eq!(ElementFilter::modules_in(&elements).len(), 1);
        assert_eq!(ElementFilter::modules_in_set(&elements).len(), 1);
        assert!(ElementFilter::record_components_in(&elements).is_empty());
    }

    struct FixtureDirective {
        kind: DirectiveKind,
    }

    impl fmt::Display for FixtureDirective {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "fixture-directive[{}]", self.kind)
        }
    }

    impl Directive for FixtureDirective {
        fn get_kind(&self) -> DirectiveKind {
            self.kind
        }
        fn accept<R, P>(
            &self,
            v: &mut dyn crate::lang::model::element::DirectiveVisitor<R, P>,
            p: P,
        ) -> R {
            v.visit_unknown(self, p)
        }
        fn as_exports_directive(&self) -> Option<&dyn ExportsDirective> {
            (self.kind == DirectiveKind::EXPORTS).then_some(self as &dyn ExportsDirective)
        }
    }

    impl ExportsDirective for FixtureDirective {
        fn get_package(&self) -> Rc<dyn PackageElement> {
            panic!("UnsupportedOperationException: get_package: fixture")
        }
        fn get_target_modules(&self) -> Option<Vec<Rc<dyn ModuleElement>>> {
            None
        }
    }

    #[test]
    fn filters_directives() {
        let directives: Vec<Rc<dyn Directive>> = vec![
            Rc::new(FixtureDirective { kind: DirectiveKind::REQUIRES }),
            Rc::new(FixtureDirective { kind: DirectiveKind::EXPORTS }),
            Rc::new(FixtureDirective { kind: DirectiveKind::EXPORTS }),
        ];
        let exports = ElementFilter::exports_in(&directives);
        assert_eq!(exports.len(), 2);
        assert_eq!(exports[0].get_kind(), DirectiveKind::EXPORTS);
        // The other directive filters use empty inputs: a directive of a
        // different kind is never selected, and a selected directive must
        // provide its as_xxx accessor (object-conversion contract).
        let empty: [Rc<dyn Directive>; 0] = [];
        assert!(ElementFilter::opens_in(&empty).is_empty());
        assert!(ElementFilter::provides_in(&empty).is_empty());
        assert!(ElementFilter::requires_in(&empty).is_empty());
        assert!(ElementFilter::uses_in(&empty).is_empty());
    }
}

