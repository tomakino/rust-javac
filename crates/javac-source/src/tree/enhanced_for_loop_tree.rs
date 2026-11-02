use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;
use super::variable_tree::VariableTree;
/// A tree node for an "enhanced" {@code for} loop statement.
///
/// For example:
/// <pre>
///   for ( <em>variable</em> : <em>expression</em> )
///       <em>statement</em>
/// </pre>
///
/// @jls 14.14.2 The enhanced for statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait EnhancedForLoopTree: StatementTree {
    /// Returns the control variable for the loop.
    /// @return the control variable
    fn get_variable(&self) -> Rc<dyn VariableTree>;

    /// Returns the expression yielding the values for the control variable.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the body of the loop.
    /// @return the body of the loop
    fn get_statement(&self) -> Rc<dyn StatementTree>;
}
