use super::expression_tree::ExpressionTree;
use super::tree::Tree;
/// A tree node to stand in for a malformed expression.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait ErroneousTree: ExpressionTree {
    /// Returns any trees that were saved in this node.
    /// @return the trees
    fn get_error_trees(&self) -> Vec<Rc<dyn Tree>>;
}
