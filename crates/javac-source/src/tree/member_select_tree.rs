/// A tree node for a member access expression.
///
/// For example:
/// <pre>
///   <em>expression</em> . <em>identifier</em>
/// </pre>
///
/// @jls 6.5 Determining the Meaning of a Name
/// @jls 15.11 Field Access Expressions
/// @jls 15.12 Method Invocation Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::expression_tree::ExpressionTree;

pub trait MemberSelectTree: ExpressionTree {
    /// Returns the expression for which a member is to be selected.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the name of the member to be selected.
    /// @return the member
    fn get_identifier(&self) -> Rc<dyn Name>;

}
