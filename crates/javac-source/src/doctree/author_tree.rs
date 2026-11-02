use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
/// A tree node for an {@code @author} block tag.
///
/// <pre>
///    &#064;author name-text
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait AuthorTree: BlockTagTree {
    /// Returns the name of the author.
    /// @return the name
    fn get_name(&self) -> Vec<Rc<dyn DocTree>>;
}
