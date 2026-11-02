use super::doc_tree::DocTree;
/// A tree node used as the base class for the different types of
/// block tags.
///
/// @since 1.8
use javac_strings::JavaString;

pub trait BlockTagTree: DocTree {
    /// Returns the name of the tag.
    /// @return the name of the tag
    fn get_tag_name(&self) -> JavaString;
}
