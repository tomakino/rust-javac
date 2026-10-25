// Origin: com/sun/source/tree/OpensTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an 'opens' directive in a module declaration.
///
/// For example:
/// <pre>
///    opens   <em>package-name</em>;
///    opens   <em>package-name</em> to <em>module-name</em>;
/// </pre>
///
/// @since 9
use std::rc::Rc;
use super::directive_tree::DirectiveTree;
use super::expression_tree::ExpressionTree;

pub trait OpensTree: DirectiveTree {
    /// Returns the name of the package to be opened.
    /// @return  the name of the package to be opened
    fn get_package_name(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the names of the modules to which the package is opened,
    /// or null, if the package is opened to all modules.
    ///
    /// @return the names of the modules to which the package is opened, or null
    fn get_module_names(&self) -> Vec<Rc<dyn ExpressionTree>>;

}
