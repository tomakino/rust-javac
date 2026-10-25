// Origin: com/sun/source/doctree/DocCommentTree.java @ 982d53c (jdk-17.0.19-ga)

//! The top-level representation of a documentation comment.
//!
//! PORTING_RULES: Java interface -> Rust trait; the 3 Java `default` methods
//! are expressible as Rust default methods (getFullBody concatenates
//! firstSentence and body; getPreamble/getPostamble return an empty list).

use super::doc_tree::DocTree;
use std::rc::Rc;

/// The top-level representation of a documentation comment.
///
/// <pre>
///    first-sentence body block-tags
/// </pre>
///
/// @since 1.8
pub trait DocCommentTree: DocTree {
    /// Returns the first sentence of a documentation comment.
    /// @return the first sentence of a documentation comment
    fn get_first_sentence(&self) -> Vec<Rc<dyn DocTree>>;

    /// Returns the entire body of a documentation comment, appearing
    /// before any block tags, including the first sentence.
    /// @return body of a documentation comment first sentence inclusive
    ///
    /// @since 9
    fn get_full_body(&self) -> Vec<Rc<dyn DocTree>> {
        let mut body_list: Vec<Rc<dyn DocTree>> = Vec::new();
        body_list.extend(self.get_first_sentence());
        body_list.extend(self.get_body());
        body_list
    }

    /// Returns the body of a documentation comment,
    /// appearing after the first sentence, and before any block tags.
    /// @return the body of a documentation comment
    fn get_body(&self) -> Vec<Rc<dyn DocTree>>;

    /// Returns the block tags for a documentation comment.
    /// @return the block tags of a documentation comment
    fn get_block_tags(&self) -> Vec<Rc<dyn DocTree>>;

    /// Returns a list of trees containing the content (if any) preceding
    /// the content of the documentation comment.
    /// When the {@code DocCommentTree} has been read from a documentation
    /// comment in a Java source file, the list will be empty.
    /// When the {@code DocCommentTree} has been read from an HTML file, this
    /// represents the content from the beginning of the file up to and
    /// including the {@code <body>} tag.
    ///
    /// @implSpec This implementation returns an empty list.
    ///
    /// @return the list of trees
    /// @since 10
    fn get_preamble(&self) -> Vec<Rc<dyn DocTree>> {
        Vec::new()
    }

    /// Returns a list of trees containing the content (if any) following the
    /// content of the documentation comment.
    /// When the {@code DocCommentTree} has been read from a documentation
    /// comment in a Java source file, the list will be empty.
    /// When {@code DocCommentTree} has been read from an HTML file, this
    /// represents the content from the {@code </body>} tag to the end of file.
    ///
    /// @implSpec This implementation returns an empty list.
    ///
    /// @return the list of trees
    /// @since 10
    fn get_postamble(&self) -> Vec<Rc<dyn DocTree>> {
        Vec::new()
    }
}
