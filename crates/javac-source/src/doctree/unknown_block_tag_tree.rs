use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
/// A tree node for an unrecognized block tag.
///
/// <pre>
///    &#064;name content
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait UnknownBlockTagTree: BlockTagTree {
    /// Returns the content of an unrecognized block tag.
    /// @return the content
    fn get_content(&self) -> Vec<Rc<dyn DocTree>>;
}
