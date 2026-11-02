/// A tree node for a {@code throw} statement.
///
/// For example:
/// <pre>
///   throw <em>expression</em>;
/// </pre>
///
/// @jls 14.18 The throw Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait ThrowTree: StatementTree {
    /// Returns the expression to be thrown.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

}
