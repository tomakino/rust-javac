use super::expression_tree::ExpressionTree;
/// A tree node for an assignment expression.
///
/// For example:
/// <pre>
///   <em>variable</em> = <em>expression</em>
/// </pre>
///
/// @jls 15.26.1 Simple Assignment Operator =
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait AssignmentTree: ExpressionTree {
    /// Returns the variable being assigned to.
    /// @return the variable
    fn get_variable(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the expression being assigned to the variable.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;
}
