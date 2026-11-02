use super::block_tree::BlockTree;
use super::catch_tree::CatchTree;
use super::statement_tree::StatementTree;
use super::tree::Tree;
/// A tree node for a {@code try} statement.
///
/// For example:
/// <pre>
///   try
///       <em>block</em>
///   <em>catches</em>
///   finally
///       <em>finallyBlock</em>
/// </pre>
///
/// @jls 14.20 The try statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait TryTree: StatementTree {
    /// Returns the block of the {@code try} statement.
    /// @return the block
    fn get_block(&self) -> Rc<dyn BlockTree>;

    /// Returns any catch blocks provided in the {@code try} statement.
    /// The result will be an empty list if there are no
    /// catch blocks.
    /// @return the catch blocks
    fn get_catches(&self) -> Vec<Rc<dyn CatchTree>>;

    /// Returns the finally block provided in the {@code try} statement,
    /// or {@code null} if there is none.
    /// @return the finally block
    fn get_finally_block(&self) -> Option<Rc<dyn BlockTree>>;

    /// Returns any resource declarations provided in the {@code try} statement.
    /// The result will be an empty list if there are no
    /// resource declarations.
    /// @return the resource declarations
    fn get_resources(&self) -> Vec<Rc<dyn Tree>>;
}
