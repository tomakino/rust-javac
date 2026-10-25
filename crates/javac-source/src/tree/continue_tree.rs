// Origin: com/sun/source/tree/ContinueTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code continue} statement.
///
/// For example:
/// <pre>
///   continue;
///   continue <em>label</em> ;
/// </pre>
///
/// @jls 14.16 The continue Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::statement_tree::StatementTree;

pub trait ContinueTree: StatementTree {
    /// Returns the label for this {@code continue} statement.
    /// @return the label
    fn get_label(&self) -> Rc<dyn Name>;

}
