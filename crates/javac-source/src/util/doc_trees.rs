// Origin: com/sun/source/util/DocTrees.java @ 982d53c (jdk-17.0.19-ga)

//! Provides access to syntax trees for doc comments.
//!
//! PORTING_RULES (§14.6):
//! * Java abstract class DocTrees extends Trees -> Rust trait DocTrees: Trees;
//!   the implementation is provided by the P6 API layer.
//! * the static instance factories use reflection / ProcessingEnvironment and
//!   are deferred to P6 (TODO(P6)).
//! * `java.text.BreakIterator` (getBreakIterator/setBreakIterator) is outside
//!   the ported surface (java.text) — omitted, registered (TODO(P6/P2)).
//! * `getDocTreeFactory()` returns the ported `Rc<dyn DocTreeFactory>`; the P6
//!   API layer provides the implementation together with the rest of DocTrees.
//! * Java overloads of getDocCommentTree are distinct Rust names (documented);
//!   nullable returns -> Option; `String relativePath` -> &JavaString; declared
//!   IOException -> Result<..., javac_tools::error::Error>.
//! * `getSourcePositions()` is re-declared in the sub-trait with the covariant
//!   DocSourcePositions return (Java override); as in the other sub-trait
//!   re-declarations, calls on a concrete type need the DocTrees:: form.
//!
//! @since 1.8

use super::doc_source_positions::DocSourcePositions;
use super::doc_tree_factory::DocTreeFactory;
use super::doc_tree_path::DocTreePath;
use super::tree_path::TreePath;
use super::trees::Trees;
use crate::doctree::doc_comment_tree::DocCommentTree;
use crate::doctree::doc_tree::DocTree;
use crate::doctree::entity_tree::EntityTree;
use crate::tree::compilation_unit_tree::CompilationUnitTree;
use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::lang::model::element::{Element, PackageElement};
use javac_tools::lang::model::type_::TypeMirror;
use javac_tools::tools::diagnostic::Kind;
use javac_tools::tools::file_object::FileObject;
use std::rc::Rc;

/// Provides access to syntax trees for doc comments.
///
/// @since 1.8
pub trait DocTrees: Trees {
    /// Returns the doc comment tree, if any, for the Tree node identified by a given TreePath.
    /// Returns {@code null} if no doc comment was found.
    /// @param path the path for the tree node
    /// @return the doc comment tree
    fn get_doc_comment_tree(&self, path: &TreePath) -> Option<Rc<dyn DocCommentTree>>;

    /// Returns the doc comment tree of the given element.
    /// Returns {@code null} if no doc comment was found.
    /// (Java: getDocCommentTree(Element))
    /// @param e an element whose documentation is required
    /// @return the doc comment tree
    ///
    /// @since 9
    fn get_doc_comment_tree_for_element(&self, e: &dyn Element) -> Option<Rc<dyn DocCommentTree>>;

    /// Returns the doc comment tree of the given file. The file must be
    /// an HTML file, in which case the doc comment tree represents the
    /// entire contents of the file.
    /// Returns {@code null} if no doc comment was found.
    /// (Java: getDocCommentTree(FileObject))
    ///
    /// @param file_object the content container
    /// @return the doc comment tree
    /// @since 9
    fn get_doc_comment_tree_for_file(
        &self,
        file_object: &dyn FileObject,
    ) -> Option<Rc<dyn DocCommentTree>>;

    /// Returns the doc comment tree of the given file whose path is
    /// specified relative to the given element.
    /// Returns {@code null} if no doc comment was found.
    /// (Java: getDocCommentTree(Element, String))
    ///
    /// @param e an element whose path is used as a reference
    /// @param relative_path the relative path from the Element
    /// @return the doc comment tree
    /// @throws java.io.IOException if an exception occurs
    /// @since 9
    fn get_doc_comment_tree_for_relative(
        &self,
        e: &dyn Element,
        relative_path: &JavaString,
    ) -> Result<Option<Rc<dyn DocCommentTree>>, Error>;

    /// Returns a doc tree path containing the doc comment tree of the given file.
    /// Returns {@code null} if no doc comment was found.
    /// @param file_object a file object encapsulating the HTML content
    /// @param package_element a package element to associate with the given file
    ///        object representing a legacy package.html, null otherwise
    /// @return a doc tree path containing the doc comment parsed from the given file
    /// @throws IllegalArgumentException if the fileObject is not an HTML file
    /// @since 9
    fn get_doc_tree_path(
        &self,
        file_object: &dyn FileObject,
        package_element: Option<&dyn PackageElement>,
    ) -> Option<Rc<DocTreePath>>;

    /// Returns the language model element referred to by the leaf node of the
    /// given DocTreePath, or {@code null} if unknown.
    /// (Java: getElement(DocTreePath) — a Java overload of Trees.getElement(TreePath);
    /// the Rust method is renamed to avoid the super-trait ambiguity, §14)
    /// @param path the path for the tree node
    /// @return the element
    fn get_element_for_doc_path(&self, path: &DocTreePath) -> Option<Rc<dyn Element>>;

    /// Returns the language model type referred to by the leaf node of the
    /// given DocTreePath, or {@code null} if unknown.
    /// (Java: getType(DocTreePath))
    /// @param path the path for the tree node
    /// @return the referenced type, or null
    /// @since 15
    fn get_type_for_doc_path(&self, path: &DocTreePath)
        -> Option<Rc<dyn TypeMirror>>;

    /// Returns the list of {@link DocTree} representing the first sentence of
    /// a comment.
    /// @param list the DocTree list to interrogate
    /// @return the first sentence
    /// @since 9
    fn get_first_sentence(&self, list: &[Rc<dyn DocTree>]) -> Vec<Rc<dyn DocTree>>;

    /// Returns a utility object for accessing the source positions
    /// of documentation tree nodes.
    /// @return the utility object
    ///
    /// Java: DocTrees overrides Trees.getSourcePositions() with the covariant
    /// DocSourcePositions return type; the Rust sub-trait re-declares it.
    fn get_source_positions_doc(&self) -> Rc<dyn DocSourcePositions>;

    /// Prints a message of the specified kind at the location of the
    /// tree within the provided compilation unit.
    /// @param kind the kind of message
    /// @param msg the message, or an empty string if none
    /// @param t the tree to use as a position hint
    /// @param c the doc comment tree to use as a position hint
    /// @param root the compilation unit that contains tree
    fn print_message_doc(
        &self,
        kind: Kind,
        msg: &JavaString,
        t: &dyn DocTree,
        c: &dyn DocCommentTree,
        root: &dyn CompilationUnitTree,
    );

    /// Returns a utility object for creating {@code DocTree} objects.
    /// @return a utility object for creating {@code DocTree} objects
    ///
    /// @since 9
    fn get_doc_tree_factory(&self) -> Rc<dyn DocTreeFactory>;

    /// Returns a string containing the characters for the entity in a given
    /// entity tree, or {@code null} if the tree does not represent a valid
    /// series of characters.
    /// @param tree the tree containing the entity
    /// @return a string containing the characters
    fn get_characters(&self, tree: &dyn EntityTree) -> Option<JavaString>;
}
