use super::statement_tree::StatementTree;
/// A tree node for a labeled statement.
///
/// For example:
/// <pre>
///   <em>label</em> : <em>statement</em>
/// </pre>
///
/// @jls 14.7 Labeled Statements
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

pub trait LabeledStatementTree: StatementTree {
    /// Returns the label.
    /// @return the label
    fn get_label(&self) -> Rc<dyn Name>;

    /// Returns the statement that is labeled.
    /// @return the statement
    fn get_statement(&self) -> Rc<dyn StatementTree>;
}
