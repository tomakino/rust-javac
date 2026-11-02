use super::tree::Tree;
/// A tree node for an intersection type in a cast expression.
///
/// @author Maurizio Cimadamore
///
/// @since 1.8
use std::rc::Rc;

pub trait IntersectionTypeTree: Tree {
    /// Returns the bounds of the type.
    /// @return the bounds
    fn get_bounds(&self) -> Vec<Rc<dyn Tree>>;
}
