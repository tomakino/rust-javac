use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;
/// A tree node for a {@code return} statement.
///
/// For example:
/// <pre>
///   return;
///   return <em>expression</em>;
/// </pre>
///
/// @jls 14.17 The return Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait ReturnTree: StatementTree {
    /// Returns the expression to be returned.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;
}
