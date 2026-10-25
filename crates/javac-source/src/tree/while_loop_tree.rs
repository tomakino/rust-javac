// Origin: com/sun/source/tree/WhileLoopTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code while} loop statement.
///
/// For example:
/// <pre>
///   while ( <em>condition</em> )
///     <em>statement</em>
/// </pre>
///
///
/// @jls 14.12 The while Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait WhileLoopTree: StatementTree {
    /// Returns the condition of the loop.
    /// @return the condition
    fn get_condition(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the body of the loop.
    /// @return the body of the loop
    fn get_statement(&self) -> Rc<dyn StatementTree>;

}
