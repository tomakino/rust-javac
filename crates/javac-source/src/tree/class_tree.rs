// Origin: com/sun/source/tree/ClassTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a class, interface, enum, record, or annotation
/// type declaration.
///
/// For example:
/// <pre>
///   <em>modifiers</em> class <em>simpleName</em> <em>typeParameters</em>
///       extends <em>extendsClause</em>
///       implements <em>implementsClause</em>
///   {
///       <em>members</em>
///   }
/// </pre>
///
/// @jls 8.1 Class Declarations
/// @jls 8.9 Enum Types
/// @jls 8.10 Record Types
/// @jls 9.1 Interface Declarations
/// @jls 9.6 Annotation Types
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;
use super::modifiers_tree::ModifiersTree;
use super::statement_tree::StatementTree;
use super::tree::Tree;
use super::type_parameter_tree::TypeParameterTree;

pub trait ClassTree: StatementTree {
    /// Returns the modifiers, including any annotations,
    /// for this type declaration.
    /// @return the modifiers
    fn get_modifiers(&self) -> Rc<dyn ModifiersTree>;

    /// Returns the simple name of this type declaration.
    /// @return the simple name
    fn get_simple_name(&self) -> Rc<dyn Name>;

    /// Returns any type parameters of this type declaration.
    /// @return the type parameters
    fn get_type_parameters(&self) -> Vec<Rc<dyn TypeParameterTree>>;

    /// Returns the supertype of this type declaration,
    /// or {@code null} if none is provided.
    /// @return the supertype
    fn get_extends_clause(&self) -> Option<Rc<dyn Tree>>;

    /// Returns the interfaces implemented by this type declaration.
    /// @return the interfaces
    fn get_implements_clause(&self) -> Vec<Rc<dyn Tree>>;

    /// Returns the members declared in this type declaration.
    /// @return the members
    fn get_members(&self) -> Vec<Rc<dyn Tree>>;


    /// Returns the permits clause of this class declaration.
    /// @return the permits clause of this class declaration
    /// @since 17
    fn get_permits_clause(&self) -> Vec<Rc<dyn Tree>> {
        Vec::new()
    }
}
