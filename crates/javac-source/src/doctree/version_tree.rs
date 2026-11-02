use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
/// A tree node for an {@code @version} block tag.
///
/// <pre>
///    &#064;version version-text
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait VersionTree: BlockTagTree {
    /// Returns the body of the tag.
    /// @return the body
    fn get_body(&self) -> Vec<Rc<dyn DocTree>>;
}
