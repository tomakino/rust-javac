// Origin: com/sun/source/tree/AnnotatedTypeTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an annotated type.
///
/// For example:
/// <pre>
///    {@code @}<em>annotationType String</em>
///    {@code @}<em>annotationType</em> ( <em>arguments</em> ) <em>Date</em>
/// </pre>
///
/// @see "JSR 308: Annotations on Java Types"
///
/// @author Mahmood Ali
/// @since 1.8
use std::rc::Rc;
use super::annotation_tree::AnnotationTree;
use super::expression_tree::ExpressionTree;

pub trait AnnotatedTypeTree: ExpressionTree {
    /// Returns the annotations associated with this type expression.
    /// @return the annotations
    fn get_annotations(&self) -> Vec<Rc<dyn AnnotationTree>>;

    /// Returns the underlying type with which the annotations are associated.
    /// @return the underlying type
    fn get_underlying_type(&self) -> Rc<dyn ExpressionTree>;

}
