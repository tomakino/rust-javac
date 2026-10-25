// Origin: com/sun/source/tree/ExpressionTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node used as the base class for the different types of
/// expressions.
///
/// @jls 15 Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use super::case_label_tree::CaseLabelTree;
use super::tree::Tree;

pub trait ExpressionTree: Tree + CaseLabelTree {
}
