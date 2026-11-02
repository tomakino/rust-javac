/// An identifier in a documentation comment.
///
/// <pre>
///    name
/// </pre>
///
/// @since 1.8
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::doc_tree::DocTree;

pub trait IdentifierTree: DocTree {
    /// Returns the name of the identifier.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

}
