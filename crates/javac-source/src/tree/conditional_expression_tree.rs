use super::expression_tree::ExpressionTree;
/// A tree node for the conditional operator {@code ? :}.
///
/// For example:
/// <pre>
///   <em>condition</em> ? <em>trueExpression</em> : <em>falseExpression</em>
/// </pre>
///
/// @jls 15.25 Conditional Operator ? :
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait ConditionalExpressionTree: ExpressionTree {
    /// Returns the condition.
    /// @return the condition
    fn get_condition(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the expression to be evaluated if the condition is true.
    /// @return the expression to be evaluated if the condition is true
    fn get_true_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the expression to be evaluated if the condition is false.
    /// @return the expression to be evaluated if the condition is false
    fn get_false_expression(&self) -> Rc<dyn ExpressionTree>;
}
