// Origin: com/sun/source/tree/ProvidesTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a 'provides' directive in a module declaration.
///
/// For example:
/// <pre>
///    provides <em>service-name</em> with <em>implementation-name</em>;
/// </pre>
///
/// @since 9
use std::rc::Rc;
use super::directive_tree::DirectiveTree;
use super::expression_tree::ExpressionTree;

pub trait ProvidesTree: DirectiveTree {
    /// Returns the name of the service type being provided.
    /// @return the name of the service type being provided
    fn get_service_name(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the names of the implementation types being provided.
    /// @return the names of the implementation types being provided
    fn get_implementation_names(&self) -> Vec<Rc<dyn ExpressionTree>>;

}
