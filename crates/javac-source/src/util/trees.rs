//! Bridges JSR 199, JSR 269, and the Tree API.
//!
//! PORTING_RULES (§14.6):
//! * Java abstract class -> Rust trait; the implementation is provided by the
//!   P6 API layer (com.sun.tools.javac.api.JavacTrees).
//! * the two static `instance` factories use reflection over the internal
//!   JavacTrees class and ProcessingEnvironment; they are deferred to P6
//!   (REGISTERED(P6), not expressible without the javax.annotation.processing port).
//! * Java overloads of getTree/getPath/isAccessible become distinct Rust method
//!   names (documented per method; §14 overload rule).
//! * nullable returns -> Option; `String getDocComment` -> Option<JavaString>;
//!   `CharSequence msg` -> &JavaString; Diagnostic.Kind is the ported enum.
//!
//! @author Peter von der Ah&eacute;

use super::source_positions::SourcePositions;
use super::tree_path::TreePath;
use crate::tree::catch_tree::CatchTree;
use crate::tree::class_tree::ClassTree;
use crate::tree::compilation_unit_tree::CompilationUnitTree;
use crate::tree::method_tree::MethodTree;
use crate::tree::scope::Scope;
use crate::tree::tree::Tree;
use javac_strings::JavaString;
use javac_tools::lang::model::element::{
    AnnotationMirror, AnnotationValue, Element, ExecutableElement, TypeElement,
};
use javac_tools::lang::model::type_::{DeclaredType, ErrorType, TypeMirror};
use javac_tools::tools::diagnostic::Kind;
use std::rc::Rc;

/// Bridges JSR 199, JSR 269, and the Tree API.
///
/// @author Peter von der Ah&eacute;
pub trait Trees {
    /// Returns a utility object for obtaining source positions.
    /// @return the utility object for obtaining source positions
    fn get_source_positions(&self) -> Rc<dyn SourcePositions>;

    /// Returns the Tree node for a given Element.
    /// Returns {@code null} if the node can not be found.
    /// @param element the element
    /// @return the tree node
    fn get_tree(&self, element: &dyn Element) -> Option<Rc<dyn Tree>>;

    /// Returns the ClassTree node for a given TypeElement.
    /// Returns {@code null} if the node can not be found.
    /// (Java: getTree(TypeElement))
    /// @param element the element
    /// @return the class tree node
    fn get_tree_for_type(&self, element: &dyn TypeElement) -> Option<Rc<dyn ClassTree>>;

    /// Returns the MethodTree node for a given ExecutableElement.
    /// Returns {@code null} if the node can not be found.
    /// (Java: getTree(ExecutableElement))
    /// @param method the executable element
    /// @return the method tree node
    fn get_tree_for_method(&self, method: &dyn ExecutableElement) -> Option<Rc<dyn MethodTree>>;

    /// Returns the Tree node for an AnnotationMirror on a given Element.
    /// Returns {@code null} if the node can not be found.
    /// (Java: getTree(Element, AnnotationMirror))
    /// @param e the element
    /// @param a the annotation mirror
    /// @return the tree node
    fn get_tree_for_annotation(
        &self,
        e: &dyn Element,
        a: &dyn AnnotationMirror,
    ) -> Option<Rc<dyn Tree>>;

    /// Returns the Tree node for an AnnotationValue for an AnnotationMirror on a given Element.
    /// Returns {@code null} if the node can not be found.
    /// (Java: getTree(Element, AnnotationMirror, AnnotationValue))
    /// @param e the element
    /// @param a the annotation mirror
    /// @param v the annotation value
    /// @return the tree node
    fn get_tree_for_annotation_value(
        &self,
        e: &dyn Element,
        a: &dyn AnnotationMirror,
        v: &dyn AnnotationValue,
    ) -> Option<Rc<dyn Tree>>;

    /// Returns the path to tree node within the specified compilation unit.
    /// @param unit the compilation unit
    /// @param node the tree node
    /// @return the tree path
    fn get_path(
        &self,
        unit: &dyn CompilationUnitTree,
        node: &dyn Tree,
    ) -> Option<Rc<TreePath>>;

    /// Returns the TreePath node for a given Element.
    /// Returns {@code null} if the node can not be found.
    /// (Java: getPath(Element))
    /// @param e the element
    /// @return the tree path
    fn get_path_for_element(&self, e: &dyn Element) -> Option<Rc<TreePath>>;

    /// Returns the TreePath node for an AnnotationMirror on a given Element.
    /// Returns {@code null} if the node can not be found.
    /// (Java: getPath(Element, AnnotationMirror))
    /// @param e the element
    /// @param a the annotation mirror
    /// @return the tree path
    fn get_path_for_annotation(
        &self,
        e: &dyn Element,
        a: &dyn AnnotationMirror,
    ) -> Option<Rc<TreePath>>;

    /// Returns the TreePath node for an AnnotationValue for an AnnotationMirror
    /// on a given Element.
    /// Returns {@code null} if the node can not be found.
    /// (Java: getPath(Element, AnnotationMirror, AnnotationValue))
    /// @param e the element
    /// @param a the annotation mirror
    /// @param v the annotation value
    /// @return the tree path
    fn get_path_for_annotation_value(
        &self,
        e: &dyn Element,
        a: &dyn AnnotationMirror,
        v: &dyn AnnotationValue,
    ) -> Option<Rc<TreePath>>;

    /// Returns the Element for the Tree node identified by a given TreePath.
    /// Returns {@code null} if the element is not available.
    /// @param path the tree path
    /// @return the element
    /// @throws IllegalArgumentException is the TreePath does not identify
    /// a Tree node that might have an associated Element.
    fn get_element(&self, path: &TreePath) -> Option<Rc<dyn Element>>;

    /// Returns the TypeMirror for the Tree node identified by a given TreePath.
    /// Returns {@code null} if the TypeMirror is not available.
    /// @param path the tree path
    /// @return the type mirror
    /// @throws IllegalArgumentException is the TreePath does not identify
    /// a Tree node that might have an associated TypeMirror.
    fn get_type_mirror(&self, path: &TreePath) -> Option<Rc<dyn TypeMirror>>;

    /// Returns the Scope for the Tree node identified by a given TreePath.
    /// Returns {@code null} if the Scope is not available.
    /// @param path the tree path
    /// @return the scope
    fn get_scope(&self, path: &TreePath) -> Option<Rc<dyn Scope>>;

    /// Returns the doc comment, if any, for the Tree node identified by a given TreePath.
    /// Returns {@code null} if no doc comment was found.
    /// @see DocTrees#getDocCommentTree(TreePath)
    /// @param path the tree path
    /// @return the doc comment
    fn get_doc_comment(&self, path: &TreePath) -> Option<JavaString>;

    /// Checks whether a given type is accessible in a given scope.
    /// @param scope the scope to be checked
    /// @param type the type to be checked
    /// @return true if {@code type} is accessible
    fn is_accessible(&self, scope: &dyn Scope, type_: &dyn TypeElement) -> bool;

    /// Checks whether the given element is accessible as a member of the given
    /// type in a given scope.
    /// (Java: isAccessible(Scope, Element, DeclaredType))
    /// @param scope the scope to be checked
    /// @param member the member to be checked
    /// @param type the type for which to check if the member is accessible
    /// @return true if {@code member} is accessible in {@code type}
    fn is_accessible_member(
        &self,
        scope: &dyn Scope,
        member: &dyn Element,
        type_: &dyn DeclaredType,
    ) -> bool;

    /// Returns the original type from the ErrorType object.
    /// @param error_type the errorType for which we want to get the original type
    /// @return the type mirror corresponding to the original type, replaced by the ErrorType
    fn get_original_type(&self, error_type: &dyn ErrorType) -> Rc<dyn TypeMirror>;

    /// Prints a message of the specified kind at the location of the
    /// tree within the provided compilation unit
    ///
    /// @param kind the kind of message
    /// @param msg  the message, or an empty string if none
    /// @param t    the tree to use as a position hint
    /// @param root the compilation unit that contains tree
    fn print_message(
        &self,
        kind: Kind,
        msg: &JavaString,
        t: &dyn Tree,
        root: &dyn CompilationUnitTree,
    );

    /// Returns the lub of an exception parameter declared in a catch clause.
    /// @param tree the tree for the catch clause
    /// @return the lub of the exception parameter
    fn get_lub(&self, tree: &dyn CatchTree) -> Rc<dyn TypeMirror>;
}

#[cfg(test)]
mod tests {
    use super::Trees;
    use crate::tree::class_tree::ClassTree;
    use crate::tree::compilation_unit_tree::CompilationUnitTree;
    use crate::tree::method_tree::MethodTree;
    use crate::tree::scope::Scope;
    use crate::tree::tree::Tree;
    use crate::util::source_positions::SourcePositions;
    use crate::util::test_support::{IdentTree, NoPositions, UnitTree};
    use crate::util::tree_path::TreePath;
    use javac_strings::JavaString;
    use javac_tools::lang::model::element::{
        AnnotationMirror, AnnotationValue, Element, ExecutableElement, TypeElement,
    };
    use javac_tools::lang::model::type_::{DeclaredType, ErrorType, TypeMirror};
    use javac_tools::tools::diagnostic::Kind;
    use std::rc::Rc;

    struct FakeTrees;

    impl Trees for FakeTrees {
        fn get_source_positions(&self) -> Rc<dyn SourcePositions> {
            Rc::new(NoPositions)
        }
        fn get_tree(&self, _element: &dyn Element) -> Option<Rc<dyn Tree>> {
            None
        }
        fn get_tree_for_type(&self, _element: &dyn TypeElement) -> Option<Rc<dyn ClassTree>> {
            None
        }
        fn get_tree_for_method(
            &self,
            _method: &dyn ExecutableElement,
        ) -> Option<Rc<dyn MethodTree>> {
            None
        }
        fn get_tree_for_annotation(
            &self,
            _e: &dyn Element,
            _a: &dyn AnnotationMirror,
        ) -> Option<Rc<dyn Tree>> {
            None
        }
        fn get_tree_for_annotation_value(
            &self,
            _e: &dyn Element,
            _a: &dyn AnnotationMirror,
            _v: &dyn AnnotationValue,
        ) -> Option<Rc<dyn Tree>> {
            None
        }
        fn get_path(
            &self,
            _unit: &dyn CompilationUnitTree,
            _node: &dyn Tree,
        ) -> Option<Rc<TreePath>> {
            None
        }
        fn get_path_for_element(&self, _e: &dyn Element) -> Option<Rc<TreePath>> {
            None
        }
        fn get_path_for_annotation(
            &self,
            _e: &dyn Element,
            _a: &dyn AnnotationMirror,
        ) -> Option<Rc<TreePath>> {
            None
        }
        fn get_path_for_annotation_value(
            &self,
            _e: &dyn Element,
            _a: &dyn AnnotationMirror,
            _v: &dyn AnnotationValue,
        ) -> Option<Rc<TreePath>> {
            None
        }
        fn get_element(&self, _path: &TreePath) -> Option<Rc<dyn Element>> {
            None
        }
        fn get_type_mirror(&self, _path: &TreePath) -> Option<Rc<dyn TypeMirror>> {
            None
        }
        fn get_scope(&self, _path: &TreePath) -> Option<Rc<dyn Scope>> {
            None
        }
        fn get_doc_comment(&self, _path: &TreePath) -> Option<JavaString> {
            None
        }
        fn is_accessible(&self, _scope: &dyn Scope, _type_: &dyn TypeElement) -> bool {
            false
        }
        fn is_accessible_member(
            &self,
            _scope: &dyn Scope,
            _member: &dyn Element,
            _type_: &dyn DeclaredType,
        ) -> bool {
            false
        }
        fn get_original_type(&self, _error_type: &dyn ErrorType) -> Rc<dyn TypeMirror> {
            panic!("UnsupportedOperationException: get_original_type: test fixture")
        }
        fn print_message(
            &self,
            _kind: Kind,
            _msg: &JavaString,
            _t: &dyn Tree,
            _root: &dyn CompilationUnitTree,
        ) {
        }
        fn get_lub(&self, _tree: &dyn crate::tree::catch_tree::CatchTree) -> Rc<dyn TypeMirror> {
            panic!("UnsupportedOperationException: get_lub: test fixture")
        }
    }

    #[test]
    fn trees_is_object_safe() {
        let trees: &dyn Trees = &FakeTrees;
        let positions = trees.get_source_positions();
        let unit = UnitTree;
        let ident = IdentTree;
        assert_eq!(positions.get_start_position(&unit, &ident), -1);
        let path = Rc::new(TreePath::new_root(Rc::new(UnitTree)));
        assert!(trees.get_doc_comment(&path).is_none());
        assert!(trees.get_element(&path).is_none());
        assert!(trees.get_type_mirror(&path).is_none());
        assert!(trees.get_scope(&path).is_none());
        trees.print_message(Kind::NOTE, &JavaString::from(""), &ident, &unit);
    }
}
