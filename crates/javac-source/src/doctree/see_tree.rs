// Origin: com/sun/source/doctree/SeeTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code @see} block tag.
///
/// <pre>
///    &#064;see "string"
///    &#064;see &lt;a href="URL#value"&gt; label &lt;/a&gt;
///    &#064;see reference
/// </pre>
///
/// @since 1.8
use std::rc::Rc;
use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;

pub trait SeeTree: BlockTagTree {
    /// Returns the reference.
    /// @return the reference
    fn get_reference(&self) -> Vec<Rc<dyn DocTree>>;

}
