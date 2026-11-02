//! Represents the type of an executable.
//!
//! PORTING_RULES: `List<? extends X>` -> `Vec<Rc<dyn X>>`.

use super::type_mirror::TypeMirror;
use super::type_variable::TypeVariable;
use std::rc::Rc;

/// Represents the type of an executable.  An <i>executable</i>
/// is a method, constructor, or initializer.
///
/// <p> The executable is
/// represented as when viewed as a method (or constructor or
/// initializer) of some reference type.
/// If that reference type is parameterized, then its actual
/// type arguments are substituted into any types returned by the methods of
/// this interface.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see ExecutableElement
/// @since 1.6
pub trait ExecutableType: TypeMirror {
    /// Returns the type variables declared by the formal type parameters
    /// of this executable.
    ///
    /// @return the type variables declared by the formal type parameters,
    ///          or an empty list if there are none
    fn get_type_variables(&self) -> Vec<Rc<dyn TypeVariable>>;

    /// {@return the return type of this executable}
    /// Returns a {@link NoType} with kind {@link TypeKind#VOID VOID}
    /// if this executable is not a method, or is a method that does not
    /// return a value.
    fn get_return_type(&self) -> Rc<dyn TypeMirror>;

    /// Returns the types of this executable's formal parameters.
    ///
    /// @return the types of this executable's formal parameters,
    ///          or an empty list if there are none
    fn get_parameter_types(&self) -> Vec<Rc<dyn TypeMirror>>;

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

    /// Returns the exceptions and other throwables listed in this
    /// executable's throws clause.
    ///
    /// @return the exceptions and other throwables listed in this
    ///          executable's throws clause,
    ///          or an empty list if there are none.
    fn get_thrown_types(&self) -> Vec<Rc<dyn TypeMirror>>;
}
