use super::tree::Tree;
/// A tree node for a wildcard type argument.
/// Use {@link #getKind getKind} to determine the kind of bound.
///
/// For example:
/// <pre>
///   ?
///
///   ? extends <em>bound</em>
///
///   ? super <em>bound</em>
/// </pre>
///
/// @jls 4.5.1 Type Arguments of Parameterized Types
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait WildcardTree: Tree {
    /// Returns the bound of the wildcard.
    /// @return the bound
    fn get_bound(&self) -> Rc<dyn Tree>;
}
