//! Represents a wildcard type argument.
//!
//! nullable bounds -> `Option<Rc<dyn TypeMirror>>`.

use super::type_mirror::TypeMirror;
use std::rc::Rc;

/// Represents a wildcard type argument.
/// Examples include:    <pre><code>
///   ?
///   ? extends Number
///   ? super T
/// </code></pre>
///
/// <p> A wildcard may have its upper bound explicitly set by an
/// extends clause, its lower bound explicitly set by a
/// super clause, or neither (but not both).
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @jls 4.5.1 Type Arguments of Parameterized Types
/// @since 1.6
pub trait WildcardType: TypeMirror {
    /// {@return the upper bound of this wildcard}
    /// If no upper bound is explicitly declared,
    /// null is returned.
    fn get_extends_bound(&self) -> Option<Rc<dyn TypeMirror>>;

    /// {@return the lower bound of this wildcard}
    /// If no lower bound is explicitly declared,
    /// null is returned.
    fn get_super_bound(&self) -> Option<Rc<dyn TypeMirror>>;
}
