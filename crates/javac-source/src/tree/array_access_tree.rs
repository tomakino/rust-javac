use super::expression_tree::ExpressionTree;
/// A tree node for an array access expression.
///
/// For example:
/// <pre>
///   <em>expression</em> [ <em>index</em> ]
/// </pre>
///
/// @jls 15.10.3 Array Access Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait ArrayAccessTree: ExpressionTree {
    /// Returns the expression for the array being accessed.
    /// @return the array
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the expression for the index.
    /// @return the index
    fn get_index(&self) -> Rc<dyn ExpressionTree>;
}
