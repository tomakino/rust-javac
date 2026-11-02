/// A tree node for a parenthesized expression.  Note: parentheses
/// not be preserved by the parser.
///
/// For example:
/// <pre>
///   ( <em>expression</em> )
/// </pre>
///
/// @jls 15.8.5 Parenthesized Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;

pub trait ParenthesizedTree: ExpressionTree {
    /// Returns the expression within the parentheses.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

}
