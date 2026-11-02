/// A tree node for a method or annotation type element declaration.
///
/// For example:
/// <pre>
///   <em>modifiers</em> <em>typeParameters</em> <em>type</em> <em>name</em>
///      ( <em>parameters</em> )
///      <em>body</em>
///
///   <em>modifiers</em> <em>type</em> <em>name</em> () default <em>defaultValue</em>
/// </pre>
///
/// @jls 8.4 Method Declarations
/// @jls 8.6 Instance Initializers
/// @jls 8.7 Static Initializers
/// @jls 9.4 Method Declarations
/// @jls 9.6.1 Annotation Type Elements
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::block_tree::BlockTree;
use super::expression_tree::ExpressionTree;
use super::modifiers_tree::ModifiersTree;
use super::tree::Tree;
use super::type_parameter_tree::TypeParameterTree;
use super::variable_tree::VariableTree;

pub trait MethodTree: Tree {
    /// Returns the modifiers, including any annotations for the method being declared.
    /// @return the modifiers
    fn get_modifiers(&self) -> Rc<dyn ModifiersTree>;

    /// Returns the name of the method being declared.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

    /// Returns the return type of the method being declared.
    /// Returns {@code null} for a constructor.
    /// @return the return type
    fn get_return_type(&self) -> Rc<dyn Tree>;

    /// Returns the type parameters of the method being declared.
    /// @return the type parameters
    fn get_type_parameters(&self) -> Vec<Rc<dyn TypeParameterTree>>;

    /// Returns the parameters of the method being declared.
    /// @return the parameters
    fn get_parameters(&self) -> Vec<Rc<dyn VariableTree>>;

    /// Return an explicit receiver parameter ("this" parameter),
    /// or {@code null} if none.
    ///
    /// @return an explicit receiver parameter ("this" parameter)
    /// @since 1.8
    fn get_receiver_parameter(&self) -> Option<Rc<dyn VariableTree>>;

    /// Returns the exceptions listed as being thrown by this method.
    /// @return the exceptions
    fn get_throws(&self) -> Vec<Rc<dyn ExpressionTree>>;

    /// Returns the method body, or {@code null} if this is an abstract or native method.
    /// @return the method body
    fn get_body(&self) -> Option<Rc<dyn BlockTree>>;

    /// Returns the default value, if this is an element within
    /// an annotation type declaration.
    /// Returns {@code null} otherwise.
    /// @return the default value
    fn get_default_value(&self) -> Rc<dyn Tree>;

}
