// Origin: com/sun/source/tree/YieldTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code yield} statement.
///
/// For example:
/// <pre>
///   yield <em>expression</em> ;
/// </pre>
///
/// @jls 14.21 The yield Statement
///
/// @since 13
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait YieldTree: StatementTree {
    /// Returns the expression for this {@code yield} statement.
    ///
    /// @return the expression
    fn get_value(&self) -> Rc<dyn ExpressionTree>;

}
