// Origin: com/sun/source/doctree/ThrowsTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code @exception} or {@code @throws} block tag.
/// {@code @exception} is a synonym for {@code @throws}.
///
/// <pre>
///    &#064;exception class-name description
///    &#064;throws class-name description
/// </pre>
///
/// @since 1.8
use std::rc::Rc;
use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
use super::reference_tree::ReferenceTree;

pub trait ThrowsTree: BlockTagTree {
    /// Returns the name of the exception being documented.
    /// @return the name of the exception
    fn get_exception_name(&self) -> Rc<dyn ReferenceTree>;

    /// Returns a description of the reasons why the
    /// exception may be thrown.
    /// @return the description
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;

}
