//! Interface for determining locally available program elements, such as local
//! variables and imports.
//!
//! Java interface -> Rust trait.  `Scope` is not a Tree
//! subinterface (no getKind/accept).  The Java documentation does not state
//! nullable returns for the enclosing scope/class/method; the port uses Option
//! because the outermost scope has no enclosing scope (registered).

use javac_tools::lang::model::element::{Element, ExecutableElement, TypeElement};
use std::rc::Rc;

/// Interface for determining locally available program elements, such as
/// local variables and imports.
/// Upon creation, a Scope is associated with a given program position;
/// for example, a {@linkplain Tree tree node}. This position may be used to
/// infer an enclosing method and/or class.
///
/// <p>A Scope does not itself contain the details of the elements corresponding
/// to the parameters, methods and fields of the methods and classes containing
/// its position. However, these elements can be determined from the enclosing
/// elements.
///
/// <p>Scopes may be contained in an enclosing scope. The outermost scope contains
/// those elements available via "star import" declarations; the scope within that
/// contains the top level elements of the compilation unit, including any named
/// imports.
///
/// @since 1.6
pub trait Scope {
    /// Returns the enclosing scope.
    /// @return the enclosing scope
    fn get_enclosing_scope(&self) -> Option<Rc<dyn Scope>>;

    /// Returns the innermost type element containing the position of this scope.
    /// @return the innermost enclosing type element
    fn get_enclosing_class(&self) -> Option<Rc<dyn TypeElement>>;

    /// Returns the innermost executable element containing the position of this scope.
    /// @return the innermost enclosing method declaration
    fn get_enclosing_method(&self) -> Option<Rc<dyn ExecutableElement>>;

    /// Returns the elements directly contained in this scope.
    /// @return the elements contained in this scope
    fn get_local_elements(&self) -> Vec<Rc<dyn Element>>;
}
