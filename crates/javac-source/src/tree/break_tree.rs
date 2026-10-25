// Origin: com/sun/source/tree/BreakTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code break} statement.
///
/// For example:
/// <pre>
///   break;
///
///   break <em>label</em> ;
/// </pre>
///
/// @jls 14.15 The break Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::statement_tree::StatementTree;

pub trait BreakTree: StatementTree {
    /// Returns the label for this {@code break} statement.
    /// @return the label
    fn get_label(&self) -> Rc<dyn Name>;

}
