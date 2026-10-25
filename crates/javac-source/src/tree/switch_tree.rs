// Origin: com/sun/source/tree/SwitchTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code switch} statement.
///
/// For example:
/// <pre>
///   switch ( <em>expression</em> ) {
///     <em>cases</em>
///   }
/// </pre>
///
/// @jls 14.11 The switch Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::case_tree::CaseTree;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait SwitchTree: StatementTree {
    /// Returns the expression for the {@code switch} statement.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the cases for the {@code switch} statement.
    /// @return the cases
    fn get_cases(&self) -> Vec<Rc<dyn CaseTree>>;

}
