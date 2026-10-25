// Origin: com/sun/source/tree/ParameterizedTypeTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a type expression involving type parameters.
///
/// For example:
/// <pre>
///   <em>type</em> &lt; <em>typeArguments</em> &gt;
/// </pre>
///
/// @jls 4.5.1 Type Arguments of Parameterized Types
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::tree::Tree;

pub trait ParameterizedTypeTree: Tree {
    /// Returns the base type.
    /// @return the base type
    fn get_type(&self) -> Rc<dyn Tree>;

    /// Returns the type arguments.
    /// @return the type arguments
    fn get_type_arguments(&self) -> Vec<Rc<dyn Tree>>;

}
