/// A tree node for the modifiers, including annotations, for a declaration.
///
/// For example:
/// <pre>
///   <em>flags</em>
///
///   <em>flags</em> <em>annotations</em>
/// </pre>
///
/// @jls 8.1.1 Class Modifiers
/// @jls 8.3.1 Field Modifiers
/// @jls 8.4.3 Method Modifiers
/// @jls 8.5.1 Static Member Type Declarations
/// @jls 8.8.3 Constructor Modifiers
/// @jls 9.1.1 Interface Modifiers
/// @jls 9.7 Annotations
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Modifier;
use std::rc::Rc;
use super::annotation_tree::AnnotationTree;
use super::tree::Tree;

pub trait ModifiersTree: Tree {
    /// Returns the flags in this modifiers tree.
    /// @return the flags
    fn get_flags(&self) -> Vec<Modifier>;

    /// Returns the annotations in this modifiers tree.
    /// @return the annotations
    fn get_annotations(&self) -> Vec<Rc<dyn AnnotationTree>>;

}
