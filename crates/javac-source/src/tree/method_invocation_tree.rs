use super::expression_tree::ExpressionTree;
use super::tree::Tree;
/// A tree node for a method invocation expression.
///
/// For example:
/// <pre>
///   <em>identifier</em> ( <em>arguments</em> )
///
///   this . <em>typeArguments</em> <em>identifier</em> ( <em>arguments</em> )
/// </pre>
///
/// @jls 15.12 Method Invocation Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;

pub trait MethodInvocationTree: ExpressionTree {
    /// Returns the type arguments for this method invocation.
    /// @return the type arguments
    fn get_type_arguments(&self) -> Vec<Rc<dyn Tree>>;

    /// Returns the expression identifying the method to be invoked.
    /// @return the method selection expression
    fn get_method_select(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the arguments for the method invocation.
    /// @return the arguments
    fn get_arguments(&self) -> Vec<Rc<dyn ExpressionTree>>;
}
