/// A tree node used as the base class for the different types of
/// inline tags.
///
/// @since 1.8
use javac_strings::JavaString;
use super::doc_tree::DocTree;

pub trait InlineTagTree: DocTree {
    /// Returns the name of the tag.
    /// @return the name of the tag
    fn get_tag_name(&self) -> JavaString;

}
