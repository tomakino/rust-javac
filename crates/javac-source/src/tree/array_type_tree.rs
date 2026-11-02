use super::tree::Tree;
/// A tree node for an array type.
///
/// For example:
/// <pre>
///   <em>type</em> []
/// </pre>
///
/// @jls 10.1 Array Types
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait ArrayTypeTree: Tree {
    /// Returns the element type of this array type.
    /// @return the element type
    fn get_type(&self) -> Rc<dyn Tree>;
}
