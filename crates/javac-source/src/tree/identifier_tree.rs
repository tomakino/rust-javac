/// A tree node for an identifier expression.
///
/// For example:
/// <pre>
///   <em>name</em>
/// </pre>
///
/// @jls 6.5.6.1 Simple Expression Names
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::expression_tree::ExpressionTree;

pub trait IdentifierTree: ExpressionTree {
    /// Returns the name of the identifier.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

}
