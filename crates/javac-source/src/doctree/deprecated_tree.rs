/// A tree node for an {@code @deprecated} block tag.
///
/// <pre>
///    &#064;deprecated deprecated text
/// </pre>
///
/// @since 1.8
use std::rc::Rc;
use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;

pub trait DeprecatedTree: BlockTagTree {
    /// Returns the description explaining why an item is deprecated.
    /// @return the description
    fn get_body(&self) -> Vec<Rc<dyn DocTree>>;

}
