// Origin: com/sun/source/doctree/EntityTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an HTML entity.
///
/// <pre>
///    &amp;name;
///    &amp;#digits;
///    &amp;#Xhex-digits;
/// </pre>
///
/// @since 1.8
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::doc_tree::DocTree;

pub trait EntityTree: DocTree {
    /// Returns the name or value of the entity.
    /// @return the name or value of the entity
    fn get_name(&self) -> Rc<dyn Name>;

}
