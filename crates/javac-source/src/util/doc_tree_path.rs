// Origin: com/sun/source/util/DocTreePath.java @ 982d53c (jdk-17.0.19-ga)

//! A path of tree nodes, typically used to represent the sequence of ancestor
//! nodes of a tree node up to the top-level DocCommentTree node.
//!
//! PORTING_RULES (§14.4/§14.6): Java concrete class -> Rust struct with Rc
//! identity; `implements Iterable<DocTree>` -> iterator() returning
//! Vec<Rc<dyn DocTree>> (leaf-to-root); the Java `Result extends Error` unwinding
//! in the static getPath search is ported in the DocTreePathScanner batch (the
//! two getPath methods are Implemented by the DocTreePathScanner batch (see PORTING_RULES §14.5).
//! TreePathScanner batch).

use super::tree_path::TreePath;
use crate::doctree::doc_comment_tree::DocCommentTree;
use crate::doctree::doc_tree::{DocTree, Kind};
use crate::util::doc_tree_path_scanner::DocTreePathScanner;
use std::rc::Rc;

/// A path of tree nodes, typically used to represent the sequence of ancestor
/// nodes of a tree node up to the top-level {@code DocCommentTree} node.
///
/// @since 1.8
pub struct DocTreePath {
    tree_path: Rc<TreePath>,
    doc_comment: Rc<dyn DocCommentTree>,
    leaf: Rc<dyn DocTree>,
    parent: Option<Rc<DocTreePath>>,
}

impl DocTreePath {
    /// Returns a documentation tree path for a tree node within a compilation unit,
    /// or {@code null} if the node is not found.
    /// @param tree_path the path for the node with which the doc comment is associated
    /// @param doc the doc comment associated with the node
    /// @param target a node within the doc comment
    /// @return a path identifying the target within the tree
    ///
    /// PORTING_RULES: Implemented by the DocTreePathScanner batch (see PORTING_RULES §14.5).
    /// DocTreePathScanner (next batch); filled in together with its port.
    pub fn get_path(
        tree_path: Rc<TreePath>,
        doc: Rc<dyn DocCommentTree>,
        target: Rc<dyn DocTree>,
    ) -> Option<Rc<DocTreePath>> {
        DocTreePath::get_path_in_path(Rc::new(DocTreePath::new(tree_path, doc)), target)
    }

    /// Returns a documentation tree path for a tree node within a subtree
    /// identified by a {@code DocTreePath} object, or {@code null} if the node is not found.
    /// @param path a path identifying a node within a doc comment tree
    /// @param target a node to be located within the given node
    /// @return a path identifying the target node
    ///
    /// PORTING_RULES: Implemented by the DocTreePathScanner batch (see PORTING_RULES §14.5).
    pub fn get_path_in_path(
        path: Rc<DocTreePath>,
        target: Rc<dyn DocTree>,
    ) -> Option<Rc<DocTreePath>> {
        if Rc::ptr_eq(&path.get_leaf(), &target) {
            return Some(path);
        }
        let mut finder = PathFinder { current: Some(path.clone()), target, result: None };
        let _ = finder.scan_path(path, ());
        finder.result
    }

    /// Creates a {@code DocTreePath} for a root node.
    ///
    /// @param tree_path the {@code TreePath} from which the root node was created
    /// @param t the {@code DocCommentTree} to create the path for
    pub fn new(tree_path: Rc<TreePath>, t: Rc<dyn DocCommentTree>) -> DocTreePath {
        DocTreePath { tree_path, doc_comment: t.clone(), leaf: t, parent: None }
    }

    /// Creates a {@code DocTreePath} for a child node.
    /// @param p the parent node
    /// @param t the child node
    pub fn new_child(p: Rc<DocTreePath>, t: Rc<dyn DocTree>) -> DocTreePath {
        if t.get_kind() == Kind::DOC_COMMENT {
            panic!(
                "IllegalArgumentException: Use DocTreePath(TreePath, DocCommentTree) to \
                 construct DocTreePath for a DocCommentTree."
            );
        }
        let tree_path = p.tree_path.clone();
        let doc_comment = p.doc_comment.clone();
        DocTreePath { tree_path, doc_comment, leaf: t, parent: Some(p) }
    }

    /// Returns the {@code TreePath} associated with this path.
    /// @return the {@code TreePath} for this {@code DocTreePath}
    pub fn get_tree_path(&self) -> Rc<TreePath> {
        self.tree_path.clone()
    }

    /// Returns the {@code DocCommentTree} associated with this path.
    /// @return the {@code DocCommentTree} for this {@code DocTreePath}
    pub fn get_doc_comment(&self) -> Rc<dyn DocCommentTree> {
        self.doc_comment.clone()
    }

    /// Returns the leaf node for this path.
    /// @return the {@code DocTree} for this {@code DocTreePath}
    pub fn get_leaf(&self) -> Rc<dyn DocTree> {
        self.leaf.clone()
    }

    /// Returns the path for the enclosing node, or {@code null} if there is no enclosing node.
    /// @return {@code DocTreePath} of parent
    pub fn get_parent_path(&self) -> Option<Rc<DocTreePath>> {
        self.parent.clone()
    }

    /// Java: iterator() — Iterates from leaves to root.
    pub fn iterator(&self) -> Vec<Rc<dyn DocTree>> {
        let mut result = Vec::new();
        let mut next: Option<&DocTreePath> = Some(self);
        while let Some(current) = next {
            result.push(current.leaf.clone());
            next = current.parent.as_deref();
        }
        result
    }
}

/// Java: the PathFinder class inside DocTreePath.getPath(DocTreePath, DocTree).
///
/// PORTING_RULES: the Java implementation throws a private Error to unwind the
/// scan when the target is found; Rust records the result and short-circuits
/// every subsequent scan (same observable result, no unwinding) — the same
/// approach as TreePath's PathFinder.
struct PathFinder {
    current: Option<Rc<DocTreePath>>,
    target: Rc<dyn DocTree>,
    result: Option<Rc<DocTreePath>>,
}

impl crate::doctree::doc_tree_visitor::DocTreeVisitor<Option<Rc<DocTreePath>>, ()> for PathFinder {}

impl crate::util::doc_tree_path_scanner::DocTreePathScanner<Rc<DocTreePath>, ()> for PathFinder {
    fn get_current_path(&self) -> Option<Rc<DocTreePath>> {
        self.current.clone()
    }

    fn set_current_path(&mut self, path: Option<Rc<DocTreePath>>) {
        self.current = path;
    }

    fn scan(&mut self, tree: Rc<dyn DocTree>, p: ()) -> Option<Rc<DocTreePath>> {
        if let Some(result) = &self.result {
            return Some(result.clone());
        }
        if Rc::ptr_eq(&tree, &self.target) {
            let current = self.current.as_ref().expect("PathFinder: no current path");
            let path = Rc::new(DocTreePath::new_child(current.clone(), tree.clone()));
            self.result = Some(path.clone());
            return Some(path);
        }
        let prev = self.get_current_path();
        let path = match &prev {
            Some(prev) => Rc::new(DocTreePath::new_child(prev.clone(), tree.clone())),
            None => panic!("NullPointerException: DocTreePathScanner.scan without a current path"),
        };
        self.set_current_path(Some(path));
        let result = crate::util::dispatch::dispatch_doc_tree_path_scanner(self, tree.as_ref(), p);
        self.set_current_path(prev);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::DocTreePath;
    use crate::doctree::doc_comment_tree::DocCommentTree;
    use crate::doctree::doc_tree::{DocTree, Kind};
    use crate::util::test_support::{DocCommentLeaf, DocCommentWithBody, DocLeaf, UnitTree};
    use crate::util::tree_path::TreePath;
    use std::rc::Rc;

    fn root_path() -> Rc<TreePath> {
        let unit: Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree> =
            Rc::new(UnitTree);
        Rc::new(TreePath::new_root(unit))
    }

    #[test]
    fn doc_tree_path_chain_and_iterator() {
        let doc: Rc<dyn DocCommentTree> = Rc::new(DocCommentLeaf);
        let root = Rc::new(DocTreePath::new(root_path(), doc.clone()));
        let doc_tree: Rc<dyn DocTree> = doc.clone();
        assert!(Rc::ptr_eq(&root.get_leaf(), &doc_tree));
        assert!(root.get_parent_path().is_none());
        let leaf: Rc<dyn DocTree> = Rc::new(DocLeaf);
        let child = Rc::new(DocTreePath::new_child(root.clone(), leaf.clone()));
        assert!(Rc::ptr_eq(&child.get_parent_path().unwrap(), &root));
        assert!(Rc::ptr_eq(&child.get_leaf(), &leaf));
        let iter = child.iterator();
        assert_eq!(iter.len(), 2);
        assert_eq!(iter[0].get_kind(), Kind::TEXT);
        assert_eq!(iter[1].get_kind(), Kind::DOC_COMMENT);
    }

    #[test]
    #[should_panic(expected = "IllegalArgumentException")]
    fn doc_comment_child_is_rejected() {
        let doc: Rc<dyn DocCommentTree> = Rc::new(DocCommentLeaf);
        let root = Rc::new(DocTreePath::new(root_path(), doc.clone()));
        let _ = DocTreePath::new_child(root, doc);
    }

    #[test]
    fn get_path_finds_a_node_in_the_doc_comment_tree() {
        let leaf: Rc<dyn DocTree> = Rc::new(DocLeaf);
        let doc: Rc<dyn DocCommentTree> = Rc::new(DocCommentWithBody { body: vec![leaf.clone()] });
        let doc_tree: Rc<dyn DocTree> = doc.clone();
        let path = DocTreePath::get_path(root_path(), doc, leaf.clone()).expect("target found");
        assert!(Rc::ptr_eq(&path.get_leaf(), &leaf));
        let parent = path.get_parent_path().expect("parent path");
        assert!(Rc::ptr_eq(&parent.get_leaf(), &doc_tree));
        assert_eq!(path.iterator().len(), 2);
    }

    #[test]
    fn get_path_returns_none_for_an_absent_node() {
        let doc: Rc<dyn DocCommentTree> = Rc::new(DocCommentWithBody { body: Vec::new() });
        let target: Rc<dyn DocTree> = Rc::new(DocLeaf);
        assert!(DocTreePath::get_path(root_path(), doc, target).is_none());
    }

    #[test]
    fn get_path_in_path_searches_a_subtree() {
        let leaf: Rc<dyn DocTree> = Rc::new(DocLeaf);
        let doc: Rc<dyn DocCommentTree> = Rc::new(DocCommentWithBody { body: vec![leaf.clone()] });
        let root = Rc::new(DocTreePath::new(root_path(), doc));
        let path = DocTreePath::get_path_in_path(root.clone(), leaf.clone()).expect("found");
        assert!(Rc::ptr_eq(&path.get_leaf(), &leaf));
        let same = DocTreePath::get_path_in_path(root.clone(), root.get_leaf()).expect("self");
        assert!(Rc::ptr_eq(&same, &root));
    }
}
