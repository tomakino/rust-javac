/// A tree node for a basic {@code for} loop statement.
///
/// For example:
/// <pre>
///   for ( <em>initializer</em> ; <em>condition</em> ; <em>update</em> )
///       <em>statement</em>
/// </pre>
///
/// @jls 14.14.1 The basic for Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_statement_tree::ExpressionStatementTree;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;

pub trait ForLoopTree: StatementTree {
    /// Returns any initializers of the {@code for} statement.
    /// The result will be an empty list if there are
    /// no initializers
    /// @return the initializers
    fn get_initializer(&self) -> Vec<Rc<dyn StatementTree>>;

    /// Returns the condition of the {@code for} statement.
    /// May be {@code null} if there is no condition.
    /// @return the condition
    fn get_condition(&self) -> Rc<dyn ExpressionTree>;

    /// Returns any update expressions of the {@code for} statement.
    /// @return the update expressions
    fn get_update(&self) -> Vec<Rc<dyn ExpressionStatementTree>>;

    /// Returns the body of the {@code for} statement.
    /// @return the body
    fn get_statement(&self) -> Rc<dyn StatementTree>;

}
