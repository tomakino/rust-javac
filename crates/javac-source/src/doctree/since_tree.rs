use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
/// A tree node for an {@code @since} block tag.
///
/// <pre>
///    &#064;since since-text
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait SinceTree: BlockTagTree {
    /// Returns the text explaining the availability of the item being documented.
    /// @return the text
    fn get_body(&self) -> Vec<Rc<dyn DocTree>>;
}
