// Origin: com/sun/source/tree/CompoundAssignmentTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for compound assignment operator.
/// Use {@link #getKind getKind} to determine the kind of operator.
///
/// For example:
/// <pre>
///   <em>variable</em> <em>operator</em> <em>expression</em>
/// </pre>
///
/// @jls 15.26.2 Compound Assignment Operators
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;

pub trait CompoundAssignmentTree: ExpressionTree {
    /// Returns the variable on the left hand side of the compound assignment.
    /// @return the variable
    fn get_variable(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the expression on the right hand side of the compound assignment.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

}
