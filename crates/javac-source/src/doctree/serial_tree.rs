/// A tree node for an {@code @serial} block tag.
///
/// <pre>
///    &#064;serial field-description | include | exclude
/// </pre>
///
/// @since 1.8
use std::rc::Rc;
use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;

pub trait SerialTree: BlockTagTree {
    /// Returns the description of the field, or the word
    /// "include" or "exclude".
    /// @return the description of the field
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;

}
