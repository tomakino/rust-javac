/// A tree node for an {@code @serialData} block tag.
///
/// <pre>
///    &#064;serialData data-description
/// </pre>
///
/// @since 1.8
use std::rc::Rc;
use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;

pub trait SerialDataTree: BlockTagTree {
    /// Returns the description of the serial data.
    /// @return the description
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;

}
