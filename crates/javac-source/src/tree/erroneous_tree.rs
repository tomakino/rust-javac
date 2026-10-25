// Origin: com/sun/source/tree/ErroneousTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node to stand in for a malformed expression.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::tree::Tree;

pub trait ErroneousTree: ExpressionTree {
    /// Returns any trees that were saved in this node.
    /// @return the trees
    fn get_error_trees(&self) -> Vec<Rc<dyn Tree>>;

}
