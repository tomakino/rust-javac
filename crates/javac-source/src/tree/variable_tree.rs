use super::expression_tree::ExpressionTree;
use super::modifiers_tree::ModifiersTree;
use super::statement_tree::StatementTree;
use super::tree::Tree;
/// A tree node for a variable declaration.
///
/// For example:
/// <pre>
///   <em>modifiers</em> <em>type</em> <em>name</em> <em>initializer</em> ;
///   <em>modifiers</em> <em>type</em> <em>qualified-name</em>.this
/// </pre>
///
/// @jls 8.3 Field Declarations
/// @jls 14.4 Local Variable Declaration Statements
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

pub trait VariableTree: StatementTree {
    /// Returns the modifiers, including any annotations, on the declaration.
    /// @return the modifiers
    fn get_modifiers(&self) -> Rc<dyn ModifiersTree>;

    /// Returns the name of the variable being declared.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

    /// Returns the qualified identifier for the name being "declared".
    /// This is only used in certain cases for the receiver of a
    /// method declaration. Returns {@code null} in all other cases.
    /// @return the qualified identifier of a receiver declaration
    fn get_name_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the type of the variable being declared.
    /// @return the type
    fn get_type(&self) -> Rc<dyn Tree>;

    /// Returns the initializer for the variable, or {@code null} if none.
    /// @return the initializer
    fn get_initializer(&self) -> Option<Rc<dyn ExpressionTree>>;
}
