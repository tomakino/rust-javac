/// A tree node for an annotation.
///
/// For example:
/// <pre>
///    {@code @}<em>annotationType</em>
///    {@code @}<em>annotationType</em> ( <em>arguments</em> )
/// </pre>
///
/// @jls 9.7 Annotations
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::tree::Tree;

pub trait AnnotationTree: ExpressionTree {
    /// Returns the annotation type.
    /// @return the annotation type
    fn get_annotation_type(&self) -> Rc<dyn Tree>;

    /// Returns the arguments, if any, for the annotation.
    /// @return the arguments for the annotation type
    fn get_arguments(&self) -> Vec<Rc<dyn ExpressionTree>>;

}
