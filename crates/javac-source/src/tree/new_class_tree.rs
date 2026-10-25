// Origin: com/sun/source/tree/NewClassTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node to declare a new instance of a class.
///
/// For example:
/// <pre>
///   new <em>identifier</em> ( )
///
///   new <em>identifier</em> ( <em>arguments</em> )
///
///   new <em>typeArguments</em> <em>identifier</em> ( <em>arguments</em> )
///       <em>classBody</em>
///
///   <em>enclosingExpression</em>.new <em>identifier</em> ( <em>arguments</em> )
/// </pre>
///
/// @jls 15.9 Class Instance Creation Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::class_tree::ClassTree;
use super::expression_tree::ExpressionTree;
use super::tree::Tree;

pub trait NewClassTree: ExpressionTree {
    /// Returns the enclosing expression, or {@code null} if none.
    /// @return the enclosing expression
    fn get_enclosing_expression(&self) -> Option<Rc<dyn ExpressionTree>>;

    /// Returns the type arguments for the object being created.
    /// @return the type arguments
    fn get_type_arguments(&self) -> Vec<Rc<dyn Tree>>;

    /// Returns the name of the class being instantiated.
    /// @return the name
    fn get_identifier(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the arguments for the constructor to be invoked.
    /// @return the arguments
    fn get_arguments(&self) -> Vec<Rc<dyn ExpressionTree>>;

    /// Returns the class body if an anonymous class is being
    /// instantiated, and {@code null} otherwise.
    /// @return the class body
    fn get_class_body(&self) -> Rc<dyn ClassTree>;

}
