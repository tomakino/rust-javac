// Origin: com/sun/source/tree/PackageTree.java @ 982d53c (jdk-17.0.19-ga)

/// Represents the package declaration.
///
/// @jls 7.3 Compilation Units
/// @jls 7.4 Package Declarations
///
/// @author Paul Govereau
/// @since 9
use std::rc::Rc;
use super::annotation_tree::AnnotationTree;
use super::expression_tree::ExpressionTree;
use super::tree::Tree;

pub trait PackageTree: Tree {
    /// Returns the annotations associated with this package declaration.
    /// @return the annotations
    fn get_annotations(&self) -> Vec<Rc<dyn AnnotationTree>>;

    /// Returns the name of the package being declared.
    /// @return the name
    fn get_package_name(&self) -> Rc<dyn ExpressionTree>;

}
