// Origin: com/sun/source/tree/NewArrayTree.java @ 982d53c (jdk-17.0.19-ga)

//! A tree node for an expression to create a new instance of an array.
//!
//! PORTING_RULES: nested generic lists map to nested Vecs;
//! `List<? extends List<? extends AnnotationTree>>` ->
//! `Vec<Vec<Rc<dyn AnnotationTree>>>`.

use super::annotation_tree::AnnotationTree;
use super::expression_tree::ExpressionTree;
use super::tree::Tree;
use std::rc::Rc;

/// A tree node for an expression to create a new instance of an array.
///
/// For example:
/// <pre>
///   new <em>type</em> <em>dimensions</em> <em>initializers</em>
///
///   new <em>type</em> <em>dimensions</em> [ ] <em>initializers</em>
/// </pre>
///
/// @jls 15.10.1 Array Creation Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
pub trait NewArrayTree: ExpressionTree {
    /// Returns the base type of the expression.
    /// May be {@code null} for an array initializer expression.
    /// @return the base type
    fn get_type(&self) -> Option<Rc<dyn Tree>>;

    /// Returns the dimension expressions for the type.
    ///
    /// @return the dimension expressions
    fn get_dimensions(&self) -> Vec<Rc<dyn ExpressionTree>>;

    /// Returns the initializer expressions.
    ///
    /// @return the initializer expressions
    fn get_initializers(&self) -> Vec<Rc<dyn ExpressionTree>>;

    /// Returns the annotations on the base type.
    /// @return the annotations
    fn get_annotations(&self) -> Vec<Rc<dyn AnnotationTree>>;

    /// Returns the annotations on each of the dimension
    /// expressions.
    /// @return the annotations on the dimensions expressions
    fn get_dim_annotations(&self) -> Vec<Vec<Rc<dyn AnnotationTree>>>;
}
