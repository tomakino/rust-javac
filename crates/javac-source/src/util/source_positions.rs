//! Provides methods to obtain the position of a Tree within a CompilationUnit.
//!
//! PORTING_RULES: Java interface -> Rust trait; `long` -> i64 (the Java
//! NOPOS/-1 convention is preserved by the implementations).

use crate::tree::compilation_unit_tree::CompilationUnitTree;
use crate::tree::tree::Tree;

/// Provides methods to obtain the position of a Tree within a CompilationUnit.
/// A position is defined as a simple character offset from the start of a
/// CompilationUnit where the first character is at offset 0.
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait SourcePositions {
    /// Returns the starting position of tree within file.  If tree is not found within
    /// file, or if the starting position is not available,
    /// returns {@link javax.tools.Diagnostic#NOPOS}.
    /// The returned position must be at the start of the yield of this tree, that
    /// is for any sub-tree of this tree, the following must hold:
    ///
    /// <p>
    /// {@code tree.getStartPosition() <= subtree.getStartPosition()} or <br>
    /// {@code tree.getStartPosition() == NOPOS} or <br>
    /// {@code subtree.getStartPosition() == NOPOS}
    /// </p>
    ///
    /// @param file CompilationUnit in which to find tree
    /// @param tree tree for which a position is sought
    /// @return the start position of tree
    fn get_start_position(&self, file: &dyn CompilationUnitTree, tree: &dyn Tree) -> i64;

    /// Returns the ending position of tree within file.  If tree is not found within
    /// file, or if the ending position is not available,
    /// returns {@link javax.tools.Diagnostic#NOPOS}.
    /// The returned position must be at the end of the yield of this tree, that
    /// is for any sub-tree of this tree, the following must hold:
    ///
    /// <p>
    /// {@code tree.getEndPosition() >= subtree.getEndPosition()} or <br>
    /// {@code tree.getEndPosition() == NOPOS} or <br>
    /// {@code subtree.getEndPosition() == NOPOS}
    /// </p>
    ///
    /// In addition, the following must hold:
    ///
    /// <p>
    /// {@code tree.getStartPosition() <= tree.getEndPosition()} or <br>
    /// {@code tree.getStartPosition() == NOPOS} or <br>
    /// {@code tree.getEndPosition() == NOPOS}
    /// </p>
    ///
    /// @param file CompilationUnit in which to find tree
    /// @param tree tree for which a position is sought
    /// @return the end position of tree
    fn get_end_position(&self, file: &dyn CompilationUnitTree, tree: &dyn Tree) -> i64;
}

#[cfg(test)]
mod tests {
    use super::SourcePositions;
    use crate::util::test_support::{IdentTree, NoPositions, UnitTree};

    #[test]
    fn source_positions_is_object_safe() {
        let positions = NoPositions;
        let pos: &dyn SourcePositions = &positions;
        let unit = UnitTree;
        let ident = IdentTree;
        assert_eq!(pos.get_start_position(&unit, &ident), -1);
        assert_eq!(pos.get_end_position(&unit, &ident), -1);
    }
}
