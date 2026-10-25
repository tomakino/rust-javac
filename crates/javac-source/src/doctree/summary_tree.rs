// Origin: com/sun/source/doctree/SummaryTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code @summary} inline tag.
///
/// <pre>
///    {&#064;summary text}
/// </pre>
///
/// @since 10
use std::rc::Rc;
use super::doc_tree::DocTree;
use super::inline_tag_tree::InlineTagTree;

pub trait SummaryTree: InlineTagTree {
    /// Returns the summary or the first line of a comment.
    /// @return the summary text
    fn get_summary(&self) -> Vec<Rc<dyn DocTree>>;

}
