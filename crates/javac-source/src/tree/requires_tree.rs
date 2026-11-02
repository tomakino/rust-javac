use super::directive_tree::DirectiveTree;
use super::expression_tree::ExpressionTree;
/// A tree node for a 'requires' directive in a module declaration.
///
/// For example:
/// <pre>
///    requires <em>module-name</em>;
///    requires static <em>module-name</em>;
///    requires transitive <em>module-name</em>;
/// </pre>
///
/// @since 9
use std::rc::Rc;

pub trait RequiresTree: DirectiveTree {
    /// Returns true if this is a "requires static" directive.
    /// @return true if this is a "requires static" directive
    fn is_static(&self) -> bool;

    /// Returns true if this is a "requires transitive" directive.
    /// @return true if this is a "requires transitive" directive
    fn is_transitive(&self) -> bool;

    /// Returns the name of the module that is required.
    /// @return the name of the module that is required
    fn get_module_name(&self) -> Rc<dyn ExpressionTree>;
}
