use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;
/// A tree node for an {@code if} statement.
///
/// For example:
/// <pre>
///   if ( <em>condition</em> )
///      <em>thenStatement</em>
///
///   if ( <em>condition</em> )
///       <em>thenStatement</em>
///   else
///       <em>elseStatement</em>
/// </pre>
///
/// @jls 14.9 The if Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait IfTree: StatementTree {
    /// Returns the condition of the if-statement.
    /// @return the condition
    fn get_condition(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the statement to be executed if the condition is true
    /// @return the statement to be executed if the condition is true
    fn get_then_statement(&self) -> Rc<dyn StatementTree>;

    /// Returns the statement to be executed if the condition is false,
    /// or {@code null} if there is no such statement.
    /// @return the statement to be executed if the condition is false
    fn get_else_statement(&self) -> Option<Rc<dyn StatementTree>>;
}
