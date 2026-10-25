// Origin: com/sun/source/tree/ExpressionStatementTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an expression statement.
///
/// For example:
/// <pre>
///   <em>expression</em> ;
/// </pre>
///
/// @jls 14.8 Expression Statements
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait ExpressionStatementTree: StatementTree {
    /// Returns the expression constituting this statement.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

}
