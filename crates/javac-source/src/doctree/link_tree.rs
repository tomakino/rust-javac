use super::doc_tree::DocTree;
use super::inline_tag_tree::InlineTagTree;
use super::reference_tree::ReferenceTree;
/// A tree node for an {@code @link} or {@code @linkplain} inline tag.
///
/// <pre>
///    {&#064;link reference label}
///    {&#064;linkplain reference label}
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait LinkTree: InlineTagTree {
    /// Returns the reference of the link.
    /// @return the reference
    fn get_reference(&self) -> Rc<dyn ReferenceTree>;

    /// Returns the label, if any, of the link.
    /// @return the label
    fn get_label(&self) -> Vec<Rc<dyn DocTree>>;
}
