use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
use super::identifier_tree::IdentifierTree;
use super::reference_tree::ReferenceTree;
/// A tree node for an {@code @serialField} block tag.
///
/// <pre>
///    &#064;serialField field-name field-type field-description
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait SerialFieldTree: BlockTagTree {
    /// Returns the name of the serial field.
    /// @return the name of the serial field
    fn get_name(&self) -> Rc<dyn IdentifierTree>;

    /// Returns the type of the serial field.
    /// @return the type of the serial field
    fn get_type(&self) -> Rc<dyn ReferenceTree>;

    /// Returns the description of the serial field.
    /// @return the description of the serial field
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;
}
