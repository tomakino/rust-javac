use super::annotation_tree::AnnotationTree;
use super::tree::Tree;
/// A tree node for a type parameter.
///
/// For example:
/// <pre>
///   <em>name</em>
///
///   <em>name</em> extends <em>bounds</em>
///
///   <em>annotations</em> <em>name</em>
/// </pre>
///
/// @jls 4.4 Type Variables
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

pub trait TypeParameterTree: Tree {
    /// Returns the name of the type parameter.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

    /// Returns the bounds of the type parameter.
    /// @return the bounds
    fn get_bounds(&self) -> Vec<Rc<dyn Tree>>;

    /// Returns annotations on the type parameter declaration.
    ///
    /// Annotations need Target meta-annotations of
    /// {@link java.lang.annotation.ElementType#TYPE_PARAMETER} or
    /// {@link java.lang.annotation.ElementType#TYPE_USE}
    /// to appear in this position.
    ///
    /// @return annotations on the type parameter declaration
    /// @since 1.8
    fn get_annotations(&self) -> Vec<Rc<dyn AnnotationTree>>;
}
