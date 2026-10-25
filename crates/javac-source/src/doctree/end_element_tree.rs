// Origin: com/sun/source/doctree/EndElementTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for the end of an HTML element.
///
/// <pre>
///    &lt;/ name &gt;
/// </pre>
///
/// @since 1.8
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::doc_tree::DocTree;

pub trait EndElementTree: DocTree {
    /// Returns the name of this element.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

}
