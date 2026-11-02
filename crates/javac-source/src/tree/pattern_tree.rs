/// A tree node used as the base class for the different kinds of
/// patterns.
///
/// @since 16
use super::case_label_tree::CaseLabelTree;
use super::tree::Tree;

pub trait PatternTree: Tree + CaseLabelTree {}
