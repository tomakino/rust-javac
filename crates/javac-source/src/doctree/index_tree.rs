// Origin: com/sun/source/doctree/IndexTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code @index} inline tag.
///
/// <pre>
///    {&#064;index keyword optional description}
/// </pre>
///
/// @since 9
use std::rc::Rc;
use super::doc_tree::DocTree;
use super::inline_tag_tree::InlineTagTree;

pub trait IndexTree: InlineTagTree {
    /// Returns the specified search term.
    /// @return the search term
    fn get_search_term(&self) -> Rc<dyn DocTree>;

    /// Returns the description, if any.
    /// @return the description
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;

}
