/// A tree node for a {@code @provides} block tag.
///
/// <pre>
///    &#064;provides service-type description
/// </pre>
///
/// @since 9
use std::rc::Rc;
use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
use super::reference_tree::ReferenceTree;

pub trait ProvidesTree: BlockTagTree {
    /// Returns the name of the service type being documented.
    /// @return the name of the service type
    fn get_service_type(&self) -> Rc<dyn ReferenceTree>;

    /// Returns a description of the service type being provided by the module.
    /// @return the description
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;

}
