/// A tree node for an {@code @literal} or {@code @code} inline tag.
///
/// <pre>
///    {&#064;literal text}
///    {&#064;code text}
/// </pre>
///
/// @since 1.8
use std::rc::Rc;
use super::inline_tag_tree::InlineTagTree;
use super::text_tree::TextTree;

pub trait LiteralTree: InlineTagTree {
    /// Returns the body of the {@code @literal} or {@code @code} tag.
    /// @return the body of the tag
    fn get_body(&self) -> Rc<dyn TextTree>;

}
