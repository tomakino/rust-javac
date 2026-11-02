use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
/// A tree node for an {@code @hidden} block tag.
///
/// <pre>
///    &#064;hidden
/// </pre>
///
/// @since 9
use std::rc::Rc;

pub trait HiddenTree: BlockTagTree {
    /// Returns the description explaining why an item is hidden.
    /// @return the description
    fn get_body(&self) -> Vec<Rc<dyn DocTree>>;
}
