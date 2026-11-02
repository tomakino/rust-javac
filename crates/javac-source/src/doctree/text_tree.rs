use super::doc_tree::DocTree;
/// A tree node for plain text.
///
/// @since 1.8
use javac_strings::JavaString;

pub trait TextTree: DocTree {
    /// Returns the text.
    /// @return the text
    fn get_body(&self) -> JavaString;
}
