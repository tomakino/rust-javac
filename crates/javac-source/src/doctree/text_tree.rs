/// A tree node for plain text.
///
/// @since 1.8
use javac_strings::JavaString;
use super::doc_tree::DocTree;

pub trait TextTree: DocTree {
    /// Returns the text.
    /// @return the text
    fn get_body(&self) -> JavaString;

}
