use super::expression_tree::ExpressionTree;
/// A tree node for postfix and unary expressions.
/// Use {@link #getKind getKind} to determine the kind of operator.
///
/// For example:
/// <pre>
///   <em>operator</em> <em>expression</em>
///
///   <em>expression</em> <em>operator</em>
/// </pre>
///
/// @jls 15.14 Postfix Expressions
/// @jls 15.15 Unary Operators
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait UnaryTree: ExpressionTree {
    /// Returns the expression that is the operand of the unary operator.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;
}
