//! A path of tree nodes, typically used to represent the sequence of ancestor
//! nodes of a tree node up to the top-level CompilationUnitTree node.
//!
//! PORTING_RULES: Java concrete class -> Rust struct (values are immutable;
//! Java's reference identity is preserved through Rc).  `implements
//! Iterable<Tree>` -> `iterator()` returning `Vec<Rc<dyn Tree>>` (§14
//! collection mapping; Iterator.remove()'s UnsupportedOperationException is not
//! ported).  The Java fields hold a CompilationUnitTree reference; the Rust
//! struct keeps the root as `Rc<dyn Tree>` and exposes the compilation unit
//! through the object-conversion accessor (§13), because a concrete
//! `Rc<dyn CompilationUnitTree>` cannot be recovered from `&dyn Tree`; the two
//! static getPath searches rely on TreePathScanner (next batch) and are REGISTERED(P1).

use crate::tree::compilation_unit_tree::CompilationUnitTree;
use crate::tree::tree::{Kind, Tree};
use crate::util::tree_path_scanner::TreePathScanner;
use std::rc::Rc;

/// A path of tree nodes, typically used to represent the sequence of ancestor
/// nodes of a tree node up to the top-level {@code CompilationUnitTree} node.
///
/// @author Jonathan Gibbons
/// @since 1.6
pub struct TreePath {
    /// Java: private CompilationUnitTree compilationUnit.
    compilation_unit: Rc<dyn Tree>,
    /// Java: private TreePath parent.
    parent: Option<Rc<TreePath>>,
    /// Java: private Tree leaf.
    leaf: Rc<dyn Tree>,
}

impl TreePath {
    /// Returns a tree path for a tree node within a compilation unit,
    /// or {@code null} if the node is not found.
    /// @param unit the compilation unit to search
    /// @param target the node to locate
    /// @return the tree path
    ///
    /// PORTING_RULES: REGISTERED(P1) — the Java implementation searches with a
    /// TreePathScanner; it is filled in with the TreeScanner batch.
    pub fn get_path(
        unit: Rc<dyn CompilationUnitTree>,
        target: Rc<dyn Tree>,
    ) -> Option<Rc<TreePath>> {
        TreePath::get_path_in_path(Rc::new(TreePath::new_root(unit)), target)
    }

    /// Returns a tree path for a tree node within a subtree identified by a
    /// TreePath object.  Returns {@code null} if the node is not found.
    /// @param path the path in which to search
    /// @param target the node to locate
    /// @return the tree path of the target node
    ///
    /// PORTING_RULES: Implemented (see PORTING_RULES §14.5).
    pub fn get_path_in_path(path: Rc<TreePath>, target: Rc<dyn Tree>) -> Option<Rc<TreePath>> {
        if Rc::ptr_eq(&path.get_leaf(), &target) {
            return Some(path);
        }
        let mut finder = PathFinder { current: Some(path.clone()), target, result: None };
        let _ = finder.scan_path(path, ());
        finder.result
    }

    /// Java: new TreePath(null, node) for a compilation unit root.
    ///
    /// PORTING_RULES: used by TreePathScanner when the current path is null
    /// (Java's TreePath(null, tree) is only legal for a compilation unit).
    pub(crate) fn new_root_tree(tree: Rc<dyn Tree>) -> TreePath {
        debug_assert_eq!(tree.get_kind(), Kind::COMPILATION_UNIT);
        TreePath { compilation_unit: tree.clone(), parent: None, leaf: tree }
    }

    /// Creates a TreePath for a root node.
    /// @param node the root node
    pub fn new_root(node: Rc<dyn CompilationUnitTree>) -> TreePath {
        let leaf: Rc<dyn Tree> = node;
        TreePath { compilation_unit: leaf.clone(), parent: None, leaf }
    }

    /// Creates a TreePath for a child node.
    /// @param path the parent path
    /// @param tree the child node
    pub fn new_child(path: Rc<TreePath>, tree: Rc<dyn Tree>) -> TreePath {
        if tree.get_kind() == Kind::COMPILATION_UNIT {
            TreePath { compilation_unit: tree.clone(), parent: None, leaf: tree }
        } else {
            TreePath { compilation_unit: path.compilation_unit.clone(), parent: Some(path), leaf: tree }
        }
    }

    /// Returns the compilation unit associated with this path.
    /// @return the compilation unit
    ///
    /// PORTING_RULES: the Java field is typed CompilationUnitTree; the Rust
    /// struct stores the trait-object tree and converts through the §13
    /// accessor (a ClassCastException/panic if the invariant is violated).
    pub fn get_compilation_unit(&self) -> &dyn CompilationUnitTree {
        self.compilation_unit
            .as_compilation_unit_tree()
            .expect("ClassCastException: CompilationUnitTree")
    }

    /// Returns the leaf node for this path.
    /// @return the leaf node
    pub fn get_leaf(&self) -> Rc<dyn Tree> {
        self.leaf.clone()
    }

    /// Returns the path for the enclosing node, or {@code null} if there is no enclosing node.
    /// @return the path for the enclosing node
    pub fn get_parent_path(&self) -> Option<Rc<TreePath>> {
        self.parent.clone()
    }

    /// Iterates from leaves to root.
    pub fn iterator(&self) -> Vec<Rc<dyn Tree>> {
        let mut result = Vec::new();
        let mut next: Option<&TreePath> = Some(self);
        while let Some(current) = next {
            result.push(current.leaf.clone());
            next = current.parent.as_deref();
        }
        result
    }
}

/// Java: the PathFinder class inside TreePath.getPath(TreePath, Tree).
///
/// PORTING_RULES: the Java implementation throws a private Error to unwind the
/// scan when the target is found; Rust records the result and short-circuits
/// every subsequent scan (same observable result, no unwinding).
struct PathFinder {
    current: Option<Rc<TreePath>>,
    target: Rc<dyn Tree>,
    result: Option<Rc<TreePath>>,
}

impl crate::tree::tree_visitor::TreeVisitor<Option<Rc<TreePath>>, ()> for PathFinder {}

impl crate::util::tree_path_scanner::TreePathScanner<Rc<TreePath>, ()> for PathFinder {
    fn get_current_path(&self) -> Option<Rc<TreePath>> {
        self.current.clone()
    }

    fn set_current_path(&mut self, path: Option<Rc<TreePath>>) {
        self.current = path;
    }

    fn scan(&mut self, tree: Rc<dyn Tree>, p: ()) -> Option<Rc<TreePath>> {
        if let Some(result) = &self.result {
            return Some(result.clone());
        }
        if Rc::ptr_eq(&tree, &self.target) {
            let current = self.current.as_ref().expect("PathFinder: no current path");
            let path = Rc::new(TreePath::new_child(current.clone(), tree.clone()));
            self.result = Some(path.clone());
            return Some(path);
        }
        let prev = self.get_current_path();
        let current = match &prev {
            Some(prev) => Rc::new(TreePath::new_child(prev.clone(), tree.clone())),
            None => {
                if tree.get_kind() != Kind::COMPILATION_UNIT {
                    panic!("NullPointerException: TreePathScanner.scan without a current path");
                }
                Rc::new(TreePath::new_root_tree(tree.clone()))
            }
        };
        self.set_current_path(Some(current));
        let result =
            crate::util::dispatch::dispatch_tree_path_scanner(self, tree.as_ref(), p);
        self.set_current_path(prev);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::TreePath;
    use crate::util::test_support::{IdentTree, UnitTree, UnitTreeWithDecls};
    use std::rc::Rc;

    #[test]
    fn tree_path_chain_and_iterator() {
        let unit: Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree> =
            Rc::new(UnitTree);
        let root = Rc::new(TreePath::new_root(unit));
        assert!(root.get_parent_path().is_none());
        let leaf: Rc<dyn crate::tree::tree::Tree> = Rc::new(IdentTree);
        let child = Rc::new(TreePath::new_child(root.clone(), leaf.clone()));
        assert!(Rc::ptr_eq(&child.get_parent_path().unwrap(), &root));
        assert!(Rc::ptr_eq(&child.get_leaf(), &leaf));
        assert_eq!(
            child.get_compilation_unit().get_kind(),
            crate::tree::tree::Kind::COMPILATION_UNIT
        );
        let path = child.iterator();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].get_kind(), crate::tree::tree::Kind::IDENTIFIER);
        assert_eq!(path[1].get_kind(), crate::tree::tree::Kind::COMPILATION_UNIT);
    }

    #[test]
    fn compilation_unit_child_starts_a_new_root() {
        let unit: Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree> =
            Rc::new(UnitTree);
        let root = Rc::new(TreePath::new_root(unit));
        let leaf: Rc<dyn crate::tree::tree::Tree> = Rc::new(UnitTree);
        let nested = TreePath::new_child(root, leaf);
        assert!(nested.get_parent_path().is_none());
        assert_eq!(nested.iterator().len(), 1);
    }

    #[test]
    fn get_path_finds_a_node_in_a_compilation_unit() {
        let ident: Rc<dyn crate::tree::tree::Tree> = Rc::new(IdentTree);
        let unit: Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree> =
            Rc::new(UnitTreeWithDecls { decls: vec![ident.clone()] });
        let unit_tree: Rc<dyn crate::tree::tree::Tree> = unit.clone();
        let path = TreePath::get_path(unit, ident.clone()).expect("target found");
        assert!(Rc::ptr_eq(&path.get_leaf(), &ident));
        let parent = path.get_parent_path().expect("parent path");
        assert!(Rc::ptr_eq(&parent.get_leaf(), &unit_tree));
        assert_eq!(path.iterator().len(), 2);
    }

    #[test]
    fn get_path_returns_none_for_an_absent_node() {
        let ident: Rc<dyn crate::tree::tree::Tree> = Rc::new(IdentTree);
        let unit: Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree> =
            Rc::new(UnitTreeWithDecls { decls: Vec::new() });
        assert!(TreePath::get_path(unit, ident).is_none());
    }

    #[test]
    fn get_path_in_path_searches_a_subtree() {
        let ident: Rc<dyn crate::tree::tree::Tree> = Rc::new(IdentTree);
        let unit: Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree> =
            Rc::new(UnitTreeWithDecls { decls: vec![ident.clone()] });
        let root = Rc::new(TreePath::new_root(unit));
        let path = TreePath::get_path_in_path(root.clone(), ident.clone()).expect("found");
        assert!(Rc::ptr_eq(&path.get_leaf(), &ident));
        // the Java implementation returns the path itself when the leaf matches
        let same = TreePath::get_path_in_path(root.clone(), root.get_leaf()).expect("self");
        assert!(Rc::ptr_eq(&same, &root));
    }
}
