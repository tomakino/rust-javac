//! Represents a method, constructor, or initializer (static or instance) of a
//! class or interface, including annotation type elements.
//!
//!
//! * `List<? extends X>` -> `Vec<Rc<dyn X>>`;
//!   nullable `AnnotationValue getDefaultValue()` -> `Option<...>`.
//! * `getTypeParameters` and `getSimpleName` are the same methods inherited
//!   from `Parameterizable`/`Element` (the Java interface re-declares them with
//!   `@Override`); they are not repeated in this trait.

use super::annotation_value::AnnotationValue;
use super::element::Element;
use super::parameterizable::Parameterizable;
use super::variable_element::VariableElement;
use crate::lang::model::type_::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents a method, constructor, or initializer (static or
/// instance) of a class or interface, including annotation type
/// elements.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see ExecutableType
/// @since 1.6
pub trait ExecutableElement: Element + Parameterizable {
    /// Returns the formal parameters of this executable.
    /// They are returned in declaration order.
    ///
    /// @return the formal parameters,
    /// or an empty list if there are none
    fn get_parameters(&self) -> Vec<Rc<dyn VariableElement>>;

    /// {@return the return type of this executable}
    /// Returns a {@link NoType} with kind {@link TypeKind#VOID VOID}
    /// if this executable is not a method, or is a method that does not
    /// return a value.
    fn get_return_type(&self) -> Rc<dyn TypeMirror>;

    /// Returns the receiver type of this executable,
    /// or {@link javax.lang.model.type.NoType NoType} with
    /// kind {@link javax.lang.model.type.TypeKind#NONE NONE}
    /// if the executable has no receiver type.
    ///
    /// An executable which is an instance method, or a constructor of an
    /// inner class, has a receiver type derived from the declaring type.
    ///
    /// An executable which is a static method, or a constructor of a
    /// non-inner class, or an initializer (static or instance), has no
    /// receiver type.
    ///
    /// @return the receiver type of this executable
    /// @since 1.8
    ///
    /// @jls 8.4 Method Declarations
    /// @jls 8.4.1 Formal Parameters
    /// @jls 8.8 Constructor Declarations
    fn get_receiver_type(&self) -> Rc<dyn TypeMirror>;

    /// {@return true if this method or constructor accepts a variable
    /// number of arguments and returns false otherwise}
    fn is_var_args(&self) -> bool;

    /// {@return true if this method is a default method and
    /// returns false otherwise}
    /// @since 1.8
    fn is_default(&self) -> bool;

    /// Returns the exceptions and other throwables listed in this
    /// method or constructor's throws clause in declaration
    /// order.
    ///
    /// @return the exceptions and other throwables listed in the
    /// throws clause, or an empty list if there are none
    fn get_thrown_types(&self) -> Vec<Rc<dyn TypeMirror>>;

    /// Returns the default value if this executable is an annotation
    /// interface element.  Returns null if this method is not
    /// an annotation interface element, or if it is an annotation
    /// interface element with no default value.
    ///
    /// @return the default value, or null if none
    fn get_default_value(&self) -> Option<Rc<dyn AnnotationValue>>;
}
