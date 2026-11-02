use super::doc_tree::DocTree;
/// A tree node for the end of an HTML element.
///
/// <pre>
///    &lt;/ name &gt;
/// </pre>
///
/// @since 1.8
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

pub trait EndElementTree: DocTree {
    /// Returns the name of this element.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;
}
