/// A tree node used as the base class for the different kinds of
/// statements.
///
/// @jls 14 Blocks and Statements
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use super::tree::Tree;

pub trait StatementTree: Tree {}
