// Origin: javax/lang/model/element/Element.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a program element such as a module, package, class, or method.
//!
//! PORTING_RULES — signature mapping:
//! * `List<? extends Element>` -> `Vec<Rc<dyn Element>>`;
//!   `Set<Modifier>` -> `Vec<Modifier>` (upstream returns an `EnumSet`, whose
//!   iteration order is the declaration order; the port keeps that order and
//!   no duplicates);
//!   `null` returns -> `Option<...>`.
//! * `equals(Object)`/`hashCode()` are Object methods; element identity is
//!   carried by `Rc<dyn Element>` references (Java: implementations define
//!   equality).  This keeps the trait object safe.
//! * The generic `<R, P> R accept(ElementVisitor<R, P>, P)` method keeps
//!   `where Self: Sized` because a generic method cannot be part of a trait
//!   object; `dyn Element` therefore cannot call accept (language shape
//!   limitation, registered).  Concrete implementations call it directly.
//! * The AnnotatedConstruct methods are inherited as a Rust supertrait.
//! * Java `Object.toString()` is expressed as the `std::fmt::Display`
//!   supertrait (every Java object has a toString; the ported element
//!   implementations print the same text as upstream).

use super::element_kind::ElementKind;
use super::element_visitor::ElementVisitor;
use super::executable_element::ExecutableElement;
use super::modifier::Modifier;
use super::module_element::ModuleElement;
use super::name::Name;
use super::package_element::PackageElement;
use super::record_component_element::RecordComponentElement;
use super::type_element::TypeElement;
use super::type_parameter_element::TypeParameterElement;
use super::variable_element::VariableElement;
use crate::lang::model::AnnotatedConstruct;
use crate::lang::model::type_::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents a program element such as a module, package, class, or method.
/// Each element represents a compile-time language-level construct
/// (and not, for example, a runtime construct of the virtual machine).
///
/// <p> Elements should be compared using the equals(Object)
/// method.  There is no guarantee that any particular element will
/// always be represented by the same object.
///
/// <p> To implement operations based on the class of an Element object,
/// either use a visitor or use the result of the getKind method.
/// Using instanceof is <em>not</em> necessarily a reliable idiom for
/// determining the effective class of an object in this modeling
/// hierarchy since an implementation may choose to have a single object
/// implement multiple Element subinterfaces.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see Elements
/// @see TypeMirror
/// @since 1.6
pub trait Element: AnnotatedConstruct + std::fmt::Display {
    /// {@return the type defined by this element}
    ///
    /// @see Types
    /// @see ExecutableElement#asType
    /// @see ModuleElement#asType
    /// @see PackageElement#asType
    /// @see TypeElement#asType
    /// @see TypeParameterElement#asType
    /// @see VariableElement#asType
    fn as_type(&self) -> Rc<dyn TypeMirror>;

    /// {@return the kind of this element}
    ///
    /// <ul>
    ///
    /// <li> The kind of a package is PACKAGE.
    ///
    /// <li> The kind of a module is MODULE.
    ///
    /// <li> The kind of a type element is one
    /// of ANNOTATION_TYPE, CLASS, ENUM, INTERFACE, or RECORD.
    ///
    /// <li> The kind of a variable is one
    /// of ENUM_CONSTANT, EXCEPTION_PARAMETER, FIELD, LOCAL_VARIABLE,
    /// PARAMETER, RESOURCE_VARIABLE, or BINDING_VARIABLE.
    ///
    /// <li> The kind of an executable is one of CONSTRUCTOR, INSTANCE_INIT,
    /// METHOD, or STATIC_INIT.
    ///
    /// <li> The kind of a type parameter is TYPE_PARAMETER.
    ///
    /// <li> The kind of a record component is RECORD_COMPONENT.
    ///
    /// </ul>
    fn get_kind(&self) -> ElementKind;

    /// Returns the modifiers of this element, excluding annotations.
    /// Implicit modifiers, such as the public and static
    /// modifiers of interface members, are included.
    ///
    /// @return the modifiers of this element, or an empty set if there are none
    fn get_modifiers(&self) -> Vec<Modifier>;

    /// {@return the simple (unqualified) name of this element} The
    /// name of a generic class or interface does not include any
    /// reference to its formal type parameters.
    ///
    /// If this element represents an unnamed package or module, an empty name
    /// is returned.
    ///
    /// If it represents a constructor, the name "&lt;init&gt;" is returned.  If it
    /// represents a static initializer, the name "&lt;clinit&gt;" is returned.
    ///
    /// If it represents an anonymous class or instance initializer, an empty
    /// name is returned.
    ///
    /// @see PackageElement#getSimpleName
    /// @see ExecutableElement#getSimpleName
    /// @see TypeElement#getSimpleName
    /// @see VariableElement#getSimpleName
    /// @see ModuleElement#getSimpleName
    /// @see RecordComponentElement#getSimpleName
    /// @revised 9
    fn get_simple_name(&self) -> Rc<dyn Name>;

    /// Returns the innermost element
    /// within which this element is, loosely speaking, enclosed.
    /// <ul>
    /// <li> If this element is one whose declaration is lexically enclosed
    /// immediately within the declaration of another element, that other
    /// element is returned.
    ///
    /// <li> If this is a top-level class or interface, its package is returned.
    ///
    /// <li> If this is a package, its module is
    /// returned if such a module exists. Otherwise, null is returned.
    ///
    /// <li> If this is a type parameter,
    /// the generic element of the type parameter is returned.
    ///
    /// <li> If this is a method or constructor
    /// parameter, the executable element which declares the parameter is
    /// returned.
    ///
    /// <li> If this is a record component,
    /// the record class which declares the
    /// record component is returned.
    ///
    /// <li> If this is a module, null is returned.
    ///
    /// </ul>
    ///
    /// @return the enclosing element, or null if there is none
    /// @see Elements#getPackageOf
    /// @revised 9
    fn get_enclosing_element(&self) -> Option<Rc<dyn Element>>;

    /// Returns the elements that are, loosely speaking, directly
    /// enclosed by this element.
    ///
    /// A class or interface is considered
    /// to enclose the fields, methods,
    /// constructors, record components, and member classes and interfaces that
    /// it directly declares.
    ///
    /// A package encloses the top-level classes and interfaces within it, but
    /// is not considered to enclose subpackages.
    ///
    /// A module encloses packages within it.
    ///
    /// Enclosed elements may include implicitly declared mandated elements.
    ///
    /// Other kinds of elements are not currently considered to enclose
    /// any elements; however, that may change as this API or the
    /// programming language evolves.
    ///
    /// @apiNote Elements of certain kinds can be isolated using
    /// methods in {@link ElementFilter}.
    ///
    /// @return the enclosed elements, or an empty list if none
    /// @see TypeElement#getEnclosedElements
    /// @see PackageElement#getEnclosedElements
    /// @see ModuleElement#getEnclosedElements
    /// @see Elements#getAllMembers
    /// @jls 8.8.9 Default Constructor
    /// @jls 8.9 Enum Classes
    /// @revised 9
    fn get_enclosed_elements(&self) -> Vec<Rc<dyn Element>>;

    /// Applies a visitor to this element.
    ///
    /// @param <R> the return type of the visitor's methods
    /// @param <P> the type of the additional parameter to the visitor's methods
    /// @param v   the visitor operating on this element
    /// @param p   additional parameter to the visitor
    /// @return a visitor-specified result
    ///
    /// PORTING_RULES: `where Self: Sized` keeps Element object safe; see the
    /// file header.
    fn accept<R, P>(&self, v: &mut dyn ElementVisitor<R, P>, p: P) -> R
    where
        Self: Sized;

    // PORTING_RULES: Object conversion contract.  Java code (for example
    // ElementFilter) uses `clazz.cast(element)` / `instanceof` to obtain a
    // subinterface reference from an Element after checking getKind().  Rust has
    // no instanceof and `&dyn Element` cannot be upcast downwards, so the
    // Element trait exposes one object-safe accessor per subinterface; the
    // default is None and a concrete implementation type that also implements a
    // subinterface overrides the applicable accessor.  This is an adapter-level
    // extension of the Java interface (registered contract).
    fn as_package_element(&self) -> Option<&dyn PackageElement> {
        None
    }

    fn as_module_element(&self) -> Option<&dyn ModuleElement> {
        None
    }

    fn as_type_element(&self) -> Option<&dyn TypeElement> {
        None
    }

    fn as_variable_element(&self) -> Option<&dyn VariableElement> {
        None
    }

    fn as_executable_element(&self) -> Option<&dyn ExecutableElement> {
        None
    }

    fn as_type_parameter_element(&self) -> Option<&dyn TypeParameterElement> {
        None
    }

    fn as_record_component_element(&self) -> Option<&dyn RecordComponentElement> {
        None
    }
}
