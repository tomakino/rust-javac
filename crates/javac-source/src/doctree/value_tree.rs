use super::inline_tag_tree::InlineTagTree;
use super::reference_tree::ReferenceTree;
/// A tree node for an {@code @value} inline tag.
///
/// <pre>
///    {&#064;value reference}
/// </pre>
///
/// @since 1.8
use std::rc::Rc;

pub trait ValueTree: InlineTagTree {
    /// Returns the reference to the value.
    /// @return the reference
    fn get_reference(&self) -> Rc<dyn ReferenceTree>;
}
