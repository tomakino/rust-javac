//! Specifies the methods to access a mappings of syntax trees to end positions.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own
//! risk.  This code and its internal interfaces are subject to change
//! or deletion without notice.</b></p>
//!
//! Java JCTree keys -> arena TreeId (ADR 0001); positions are
//! the encoded int positions of util/Position.

use crate::ids::TreeId;

/// Java: public interface EndPosTable
pub trait EndPosTable {
    /// This method will return the end position of a given tree, otherwise a
    /// Positions.NOPOS will be returned.
    /// @param tree JCTree
    /// @return position of the source tree or Positions.NOPOS for non-existent mapping
    fn get_end_pos(&self, tree: TreeId) -> i32;

    /// Store ending position for a tree, the value of which is the greater of
    /// last error position and the given ending position.
    /// @param tree The tree.
    /// @param end_pos The ending position to associate with the tree.
    fn store_end(&self, tree: TreeId, end_pos: i32);

    /// Give an old tree and a new tree, the old tree will be replaced with
    /// the new tree, the position of the new tree will be that of the old
    /// tree.
    /// @param old_tree a JCTree to be replaced
    /// @param new_tree a JCTree to be replaced with
    /// @return position of the old tree or Positions.NOPOS for non-existent mapping
    fn replace_tree(&self, old_tree: TreeId, new_tree: TreeId) -> i32;
}

/// Fillable end-position table (production use).
///
/// The parser records ends through [`EndPosTable::store_end`]; the tree-layer
/// text fill in [`crate::tree::end_position`] records computed ends here so
/// later lookups hit the table (single fill implementation).
pub(crate) struct MapEndPosTable {
    positions: std::cell::RefCell<std::collections::HashMap<TreeId, i32>>,
    /// Expression-level ends: subtree max (token/closure), without the
    /// trailing `;`/`}` of the enclosing statement or declaration.
    expression: std::cell::RefCell<std::collections::HashMap<TreeId, i32>>,
}

impl MapEndPosTable {
    pub(crate) fn new() -> MapEndPosTable {
        MapEndPosTable {
            positions: std::cell::RefCell::new(std::collections::HashMap::new()),
            expression: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    /// End of the expression/statement content of `tree` (no trailing `;`).
    pub(crate) fn get_expression_end(&self, tree: TreeId) -> i32 {
        self.expression.borrow().get(&tree).copied().unwrap_or(-1)
    }

    pub(crate) fn store_expression_end(&self, tree: TreeId, end: i32) {
        self.expression.borrow_mut().insert(tree, end);
    }
}

impl EndPosTable for MapEndPosTable {
    fn get_end_pos(&self, tree: TreeId) -> i32 {
        self.positions.borrow().get(&tree).copied().unwrap_or(-1)
    }

    fn store_end(&self, tree: TreeId, end_pos: i32) {
        self.positions.borrow_mut().insert(tree, end_pos);
    }

    fn replace_tree(&self, old_tree: TreeId, new_tree: TreeId) -> i32 {
        let removed = self.positions.borrow_mut().remove(&old_tree);
        match removed {
            Some(position) => {
                self.positions.borrow_mut().insert(new_tree, position);
                position
            }
            None => -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn table() -> MapEndPosTable {
        MapEndPosTable::new()
    }

    #[test]
    fn missing_tree_returns_nopos() {
        let table = table();
        assert_eq!(table.get_end_pos(TreeId::new(0)), -1);
    }

    #[test]
    fn store_and_get_end_pos() {
        let mut table = table();
        let tree = TreeId::new(3);
        table.store_end(tree, 42);
        assert_eq!(table.get_end_pos(tree), 42);
        table.store_end(tree, 43);
        assert_eq!(table.get_end_pos(tree), 43);
    }

    #[test]
    fn replace_tree_moves_the_end_pos() {
        let mut table = table();
        let old_tree = TreeId::new(1);
        let new_tree = TreeId::new(2);
        table.store_end(old_tree, 10);
        assert_eq!(table.replace_tree(old_tree, new_tree), 10);
        assert_eq!(table.get_end_pos(old_tree), -1);
        assert_eq!(table.get_end_pos(new_tree), 10);
    }

    #[test]
    fn replace_tree_missing_returns_nopos() {
        let mut table = table();
        assert_eq!(table.replace_tree(TreeId::new(4), TreeId::new(5)), -1);
    }
}
