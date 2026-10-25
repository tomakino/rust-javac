// Origin: com/sun/source/tree/SwitchExpressionTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code switch} expression.
///
/// For example:
/// <pre>
///   switch ( <em>expression</em> ) {
///     <em>cases</em>
///   }
/// </pre>
///
/// @jls 15.29 Switch Expressions
///
/// @since 12
use std::rc::Rc;
use super::case_tree::CaseTree;
use super::expression_tree::ExpressionTree;

pub trait SwitchExpressionTree: ExpressionTree {
    /// Returns the expression for the {@code switch} expression.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the cases for the {@code switch} expression.
    /// @return the cases
    fn get_cases(&self) -> Vec<Rc<dyn CaseTree>>;

}
