// Origin: com/sun/source/tree/TypeCastTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a type cast expression.
///
/// For example:
/// <pre>
///   ( <em>type</em> ) <em>expression</em>
/// </pre>
///
/// @jls 15.16 Cast Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::tree::Tree;

pub trait TypeCastTree: ExpressionTree {
    /// Returns the target type of the cast.
    /// @return the cast
    fn get_type(&self) -> Rc<dyn Tree>;

    /// Returns the expression being cast.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

}
