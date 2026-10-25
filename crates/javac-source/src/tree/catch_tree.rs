// Origin: com/sun/source/tree/CatchTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code catch} block in a {@code try} statement.
///
/// For example:
/// <pre>
///   catch ( <em>parameter</em> )
///       <em>block</em>
/// </pre>
///
/// @jls 14.20 The try statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::block_tree::BlockTree;
use super::tree::Tree;
use super::variable_tree::VariableTree;

pub trait CatchTree: Tree {
    /// Returns the catch variable.
    /// A multi-catch variable will have a
    /// {@link UnionTypeTree UnionTypeTree}
    /// as the type of the variable.
    /// @return the catch variable
    fn get_parameter(&self) -> Rc<dyn VariableTree>;

    /// Returns the catch block.
    /// @return the catch block
    fn get_block(&self) -> Rc<dyn BlockTree>;

}
