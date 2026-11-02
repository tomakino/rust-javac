use super::doc_tree::DocTree;
use super::inline_tag_tree::InlineTagTree;
/// A tree node for an {@code @summary} inline tag.
///
/// <pre>
///    {&#064;summary text}
/// </pre>
///
/// @since 10
use std::rc::Rc;

pub trait SummaryTree: InlineTagTree {
    /// Returns the summary or the first line of a comment.
    /// @return the summary text
    fn get_summary(&self) -> Vec<Rc<dyn DocTree>>;
}
