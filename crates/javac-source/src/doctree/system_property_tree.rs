use super::inline_tag_tree::InlineTagTree;
/// A tree node for an {@code @systemProperty} inline tag.
///
/// <pre>
///    {&#064;systemProperty property-name}
/// </pre>
///
/// @since 12
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

pub trait SystemPropertyTree: InlineTagTree {
    /// Returns the specified system property name.
    /// @return the system property name
    fn get_property_name(&self) -> Rc<dyn Name>;
}
