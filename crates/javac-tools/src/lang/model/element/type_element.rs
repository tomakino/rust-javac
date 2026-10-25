// Origin: javax/lang/model/element/TypeElement.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a class or interface program element.
//!
//! PORTING_RULES:
//! * `List<? extends X>` -> `Vec<Rc<dyn X>>`.
//! * the `default` methods `getRecordComponents`/`getPermittedSubclasses` that
//!   return `List.of()` upstream become trait default methods returning an
//!   empty Vec.
//! * the Java `@Override` re-declarations of `asType`, `getEnclosedElements`,
//!   `getQualifiedName`, `getSimpleName`, `getTypeParameters` and
//!   `getEnclosingElement` only narrow their documentation; the signatures are
//!   inherited from `Element`/`Parameterizable`/`QualifiedNameable` and are not
//!   repeated (registered difference).

use super::element::Element;
use super::nesting_kind::NestingKind;
use super::parameterizable::Parameterizable;
use super::qualified_nameable::QualifiedNameable;
use super::record_component_element::RecordComponentElement;
use crate::lang::model::type_::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents a class or interface program element.  Provides access
/// to information about the class or interface and its members.  Note
/// that an enum class and a record class are specialized kinds of
/// classes and an annotation interface is a specialized kind of
/// interface.
///
/// <p> While a TypeElement represents a class or interface
/// <i>element</i>, a {@link DeclaredType} represents a class
/// or interface <i>type</i>, the latter being a use
/// (or <i>invocation</i>) of the former.
/// The distinction is most apparent with generic types,
/// for which a single element can define a whole
/// family of types.  For example, the element
/// java.util.Set corresponds to the parameterized types
/// java.util.Set&lt;String&gt; and java.util.Set&lt;Number&gt;
/// (and many others), and to the raw type java.util.Set.
///
/// <p> Each method of this interface that returns a list of elements
/// will return them in the order that is natural for the underlying
/// source of program information.  For example, if the underlying
/// source of information is Java source code, then the elements will be
/// returned in source code order.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see DeclaredType
/// @since 1.6
pub trait TypeElement: Element + Parameterizable + QualifiedNameable {
    /// Returns the <i>nesting kind</i> of this class or interface element.
    ///
    /// @return the nesting kind of this class or interface element
    fn get_nesting_kind(&self) -> NestingKind;

    /// Returns the direct superclass of this class or interface element.
    /// If this class or interface element represents an interface or the class
    /// java.lang.Object, then a {@link NoType}
    /// with kind {@link TypeKind#NONE NONE} is returned.
    ///
    /// @return the direct superclass, or a NoType if there is none
    fn get_superclass(&self) -> Rc<dyn TypeMirror>;

    /// Returns the interface types directly implemented by this class
    /// or extended by this interface.
    ///
    /// @return the interface types directly implemented by this class
    /// or extended by this interface, or an empty list if there are none
    fn get_interfaces(&self) -> Vec<Rc<dyn TypeMirror>>;

    /// Returns the record components of this class or interface
    /// element in declaration order.
    ///
    /// @implSpec The default implementations of this method returns an
    /// empty and unmodifiable list.
    ///
    /// @return the record components, or an empty list if there are
    /// none
    ///
    /// @since 16
    fn get_record_components(&self) -> Vec<Rc<dyn RecordComponentElement>> {
        Vec::new()
    }

    /// Returns the permitted classes of this class or interface
    /// element in declaration order.
    ///
    /// @implSpec The default implementations of this method returns an
    /// empty and unmodifiable list.
    ///
    /// @return the permitted classes, or an empty list if there are none
    ///
    /// @since 17
    fn get_permitted_subclasses(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}
