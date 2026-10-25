// Origin: com/sun/source/util/DocSourcePositions.java @ 982d53c (jdk-17.0.19-ga)

//! Provides methods to obtain the position of a DocTree within a javadoc
//! comment.
//!
//! PORTING_RULES: Java interface -> Rust trait; the two overloaded methods are
//! re-declared in the sub-trait with the DocTree parameters (the same Java
//! method name with different signatures cannot be expressed by a single Rust
//! method; calling them on a concrete implementation requires the fully
//! qualified `DocSourcePositions::` form because the super-trait has same-named
//! methods — registered language-shape limitation, §14).

use super::source_positions::SourcePositions;
use crate::doctree::doc_comment_tree::DocCommentTree;
use crate::doctree::doc_tree::DocTree;
use crate::tree::compilation_unit_tree::CompilationUnitTree;

/// Provides methods to obtain the position of a DocTree within a javadoc comment.
/// A position is defined as a simple character offset from the start of a
/// CompilationUnit where the first character is at offset 0.
///
/// @since 1.8
pub trait DocSourcePositions: SourcePositions {
    /// Returns the starting position of the tree within the comment within the file.
    /// If tree is not found within file, or if the starting position is not available,
    /// returns {@link javax.tools.Diagnostic#NOPOS}.
    ///
    /// @param file compilation unit in which to find tree
    /// @param comment the comment tree that encloses the tree for which the
    ///                position is being sought
    /// @param tree tree for which a position is sought
    /// @return the start position of tree
    fn get_start_position(
        &self,
        file: &dyn CompilationUnitTree,
        comment: &dyn DocCommentTree,
        tree: &dyn DocTree,
    ) -> i64;

    /// Returns the ending position of the tree within the comment within the file.
    /// If tree is not found within file, or if the ending position is not available,
    /// returns {@link javax.tools.Diagnostic#NOPOS}.
    ///
    /// @param file compilation unit in which to find tree
    /// @param comment the comment tree that encloses the tree for which the
    ///                position is being sought
    /// @param tree tree for which a position is sought
    /// @return the end position of tree
    fn get_end_position(
        &self,
        file: &dyn CompilationUnitTree,
        comment: &dyn DocCommentTree,
        tree: &dyn DocTree,
    ) -> i64;
}

#[cfg(test)]
mod tests {
    use super::DocSourcePositions;
    use crate::util::source_positions::SourcePositions;
    use crate::util::test_support::{DocCommentLeaf, DocLeaf, DocNoPositions, IdentTree, UnitTree};

    #[test]
    fn doc_source_positions_is_object_safe() {
        let positions = DocNoPositions;
        let pos: &dyn DocSourcePositions = &positions;
        let unit = UnitTree;
        let comment = DocCommentLeaf;
        let doc = DocLeaf;
        // The sub-trait re-declares the same Java method names with DocTree
        // parameters; a fully qualified call selects the DocSourcePositions one.
        assert_eq!(
            DocSourcePositions::get_start_position(pos, &unit, &comment, &doc),
            -1
        );
        assert_eq!(
            DocSourcePositions::get_end_position(pos, &unit, &comment, &doc),
            -1
        );
        // The inherited SourcePositions methods stay callable.
        let ident = IdentTree;
        assert_eq!(SourcePositions::get_start_position(pos, &unit, &ident), -1);
    }
}
