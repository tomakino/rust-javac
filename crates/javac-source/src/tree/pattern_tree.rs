// Origin: com/sun/source/tree/PatternTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node used as the base class for the different kinds of
/// patterns.
///
/// @since 16
use super::case_label_tree::CaseLabelTree;
use super::tree::Tree;

pub trait PatternTree: Tree + CaseLabelTree {
}
