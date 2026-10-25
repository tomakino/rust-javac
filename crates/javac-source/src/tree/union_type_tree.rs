// Origin: com/sun/source/tree/UnionTypeTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a union type expression in a multicatch
/// variable declaration.
///
/// @author Maurizio Cimadamore
///
/// @since 1.7
use std::rc::Rc;
use super::tree::Tree;

pub trait UnionTypeTree: Tree {
    /// Returns the alternative type expressions.
    /// @return the alternative type expressions
    fn get_type_alternatives(&self) -> Vec<Rc<dyn Tree>>;

}
