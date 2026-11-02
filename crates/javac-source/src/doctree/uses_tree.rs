use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
use super::reference_tree::ReferenceTree;
/// A tree node for an {@code @uses} block tag.
///
/// <pre>
///    &#064;uses service-type description
/// </pre>
///
/// @since 9
use std::rc::Rc;

pub trait UsesTree: BlockTagTree {
    /// Returns the name of the service type being documented.
    /// @return the name of the service type
    fn get_service_type(&self) -> Rc<dyn ReferenceTree>;

    /// Returns a description of the use of service type within the module.
    /// @return the description
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;
}
