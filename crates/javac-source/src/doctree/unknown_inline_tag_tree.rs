use super::doc_tree::DocTree;
use super::inline_tag_tree::InlineTagTree;
/// A tree node for an unrecognized inline tag.
///
/// <pre>
///    {&#064;name content}
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait UnknownInlineTagTree: InlineTagTree {
    /// Returns the content of an unrecognized inline tag.
    /// @return the content
    fn get_content(&self) -> Vec<Rc<dyn DocTree>>;
}
