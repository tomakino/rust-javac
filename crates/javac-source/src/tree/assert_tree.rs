// Origin: com/sun/source/tree/AssertTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code assert} statement.
///
/// For example:
/// <pre>
///   assert <em>condition</em> ;
///
///   assert <em>condition</em> : <em>detail</em> ;
/// </pre>
///
/// @jls 14.10 The assert Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait AssertTree: StatementTree {
    /// Returns the condition being asserted.
    /// @return the condition
    fn get_condition(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the detail expression.
    /// @return the detail expression
    fn get_detail(&self) -> Rc<dyn ExpressionTree>;

}
