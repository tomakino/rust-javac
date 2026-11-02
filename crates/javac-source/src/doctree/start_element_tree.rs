use super::doc_tree::DocTree;
/// A tree node for the start of an HTML element.
///
/// <pre>
///    &lt; name [attributes] [/]&gt;
/// </pre>
///
/// @since 1.8
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

pub trait StartElementTree: DocTree {
    /// Returns the name of the element.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

    /// Returns any attributes defined by this element.
    /// @return the attributes
    fn get_attributes(&self) -> Vec<Rc<dyn DocTree>>;

    /// Returns {@code true} if this is a self-closing element,
    /// as indicated by a {@code "/"} before the closing {@code ">"}.
    /// @return {@code true} if this is a self-closing element
    fn is_self_closing(&self) -> bool;
}
