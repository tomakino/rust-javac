// Origin: com/sun/source/tree/SynchronizedTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code synchronized} statement.
///
/// For example:
/// <pre>
///   synchronized ( <em>expression</em> )
///       <em>block</em>
/// </pre>
///
/// @jls 14.19 The synchronized Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::block_tree::BlockTree;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait SynchronizedTree: StatementTree {
    /// Returns the expression on which to synchronize.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the block of the {@code synchronized} statement.
    /// @return the block
    fn get_block(&self) -> Rc<dyn BlockTree>;

}
