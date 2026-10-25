// Origin: com/sun/source/tree/BlockTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a statement block.
///
/// For example:
/// <pre>
///   { }
///
///   { <em>statements</em> }
///
///   static { <em>statements</em> }
/// </pre>
///
/// @jls 14.2 Blocks
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::statement_tree::StatementTree;

pub trait BlockTree: StatementTree {
    /// Returns true if and only if this is a static initializer block.
    /// @return true if this is a static initializer block
    fn is_static(&self) -> bool;

    /// Returns the statements comprising this block.
    /// @return the statements
    fn get_statements(&self) -> Vec<Rc<dyn StatementTree>>;

}
