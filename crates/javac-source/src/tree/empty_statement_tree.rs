/// A tree node for an empty (skip) statement.
///
/// For example:
/// <pre>
///    ;
/// </pre>
///
/// @jls 14.6 The Empty Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use super::statement_tree::StatementTree;

pub trait EmptyStatementTree: StatementTree {
}
