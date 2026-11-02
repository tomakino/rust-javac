/// An embedded HTML comment.
///
/// <pre>
///    &lt;!-- text --&gt;
/// </pre>
///
/// @since 1.8
use javac_strings::JavaString;
use super::doc_tree::DocTree;

pub trait CommentTree: DocTree {
    /// Returns the text of the comment.
    /// @return the comment text
    fn get_body(&self) -> JavaString;

}
