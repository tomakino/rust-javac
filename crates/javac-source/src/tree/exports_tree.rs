use super::directive_tree::DirectiveTree;
use super::expression_tree::ExpressionTree;
/// A tree node for an 'exports' directive in a module declaration.
///
/// For example:
/// <pre>
///    exports <em>package-name</em>;
///    exports <em>package-name</em> to <em>module-name</em>;
/// </pre>
///
/// @since 9
use std::rc::Rc;

pub trait ExportsTree: DirectiveTree {
    /// Returns the name of the package to be exported.
    /// @return  the name of the package to be exported
    fn get_package_name(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the names of the modules to which the package is exported,
    /// or null, if the package is exported to all modules.
    ///
    /// @return the names of the modules to which the package is exported, or null
    fn get_module_names(&self) -> Vec<Rc<dyn ExpressionTree>>;
}
