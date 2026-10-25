// Origin: com/sun/source/tree/UsesTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a 'uses' directive in a module declaration.
///
/// For example:
/// <pre>
///    uses <em>service-name</em>;
/// </pre>
///
/// @since 9
use std::rc::Rc;
use super::directive_tree::DirectiveTree;
use super::expression_tree::ExpressionTree;

pub trait UsesTree: DirectiveTree {
    /// Returns the name of the service type.
    /// @return  the name of the service type
    fn get_service_name(&self) -> Rc<dyn ExpressionTree>;

}
