// Origin: com/sun/tools/javac/tree/DocCommentTable.java @ 982d53c (jdk-17.0.19-ga)

//! A table giving the doc comment, if any, for any tree node.
//!
//! PORTING_RULES: Java JCTree keys -> arena TreeId (ADR 0001). The Comment and
//! CommentStyle types are the upstream parser/Tokens.Comment types; they now
//! live in parser/tokens.rs and are re-exported here for the tree layer.
//! The upstream getCommentTree(JCTree) result type DCDocComment is represented
//! as Rc<DCTree> whose DCKind is DCKind::DocComment (the port has one shared
//! node type; see tree/dc_tree.rs).

use std::rc::Rc;

use crate::ids::TreeId;

pub use crate::parser::tokens::{Comment, CommentStyle};
use crate::tree::dc_tree::DCTree;
use crate::util::java_string::JavaString;

/// Java: public interface DocCommentTable
pub trait DocCommentTable {
    /// Check if a tree node has a corresponding doc comment.
    fn has_comment(&self, tree: TreeId) -> bool;

    /// Get the Comment token containing the doc comment, if any, for a tree node.
    fn get_comment(&self, tree: TreeId) -> Option<Rc<dyn Comment>>;

    /// Get the plain text of the doc comment, if any, for a tree node.
    fn get_comment_text(&self, tree: TreeId) -> Option<JavaString>;

    /// Get the parsed form of the doc comment as a DocTree. If any errors
    /// are detected during parsing, they will be reported via ErroneousTree
    /// nodes within the resulting tree.
    ///
    /// Java: DCDocComment getCommentTree(JCTree tree) -- the result kind is
    /// DCKind::DocComment.
    fn get_comment_tree(&self, tree: TreeId) -> Option<Rc<DCTree>>;

    /// Set the Comment to be associated with a tree node.
    fn put_comment(&mut self, tree: TreeId, comment: Rc<dyn Comment>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokens::CommentStyle;
    use crate::tree::dc_tree::{DCDocComment, DCKind};
    use crate::util::list::List;
    use std::collections::HashMap;

    struct TestComment {
        text: Option<JavaString>,
        source_pos: i32,
        style: CommentStyle,
        deprecated: bool,
    }

    impl Comment for TestComment {
        fn get_text(&self) -> Option<JavaString> {
            self.text.clone()
        }

        fn get_source_pos(&self, index: i32) -> i32 {
            self.source_pos + index
        }

        fn get_style(&self) -> CommentStyle {
            self.style
        }

        fn is_deprecated(&self) -> bool {
            self.deprecated
        }
    }

    struct MapDocCommentTable {
        comments: HashMap<TreeId, Rc<dyn Comment>>,
        trees: HashMap<TreeId, Rc<DCTree>>,
    }

    impl DocCommentTable for MapDocCommentTable {
        fn has_comment(&self, tree: TreeId) -> bool {
            self.comments.contains_key(&tree)
        }

        fn get_comment(&self, tree: TreeId) -> Option<Rc<dyn Comment>> {
            self.comments.get(&tree).cloned()
        }

        fn get_comment_text(&self, tree: TreeId) -> Option<JavaString> {
            self.comments
                .get(&tree)
                .and_then(|comment| comment.get_text())
        }

        fn get_comment_tree(&self, tree: TreeId) -> Option<Rc<DCTree>> {
            self.trees.get(&tree).cloned()
        }

        fn put_comment(&mut self, tree: TreeId, comment: Rc<dyn Comment>) {
            self.comments.insert(tree, comment);
            self.trees.remove(&tree);
        }
    }

    fn table() -> MapDocCommentTable {
        MapDocCommentTable {
            comments: HashMap::new(),
            trees: HashMap::new(),
        }
    }

    fn comment(text: &str) -> Rc<dyn Comment> {
        Rc::new(TestComment {
            text: Some(JavaString::from(text)),
            source_pos: 5,
            style: CommentStyle::JAVADOC,
            deprecated: false,
        })
    }

    fn doc_comment_tree(comment: Rc<dyn Comment>, body: &str) -> Rc<DCTree> {
        let text = Rc::new(DCTree::new(
            0,
            DCKind::Text {
                text: JavaString::from(body),
            },
        ));
        Rc::new(DCTree::new(
            0,
            DCKind::DocComment(DCDocComment {
                comment,
                full_body: List::from_iterable([text.clone()]),
                first_sentence: List::of(text),
                body: List::nil(),
                tags: List::nil(),
                preamble: List::nil(),
                postamble: List::nil(),
            }),
        ))
    }

    #[test]
    fn missing_tree_has_no_comment_or_tree() {
        let table = table();
        let tree = TreeId::new(0);
        assert!(!table.has_comment(tree));
        assert!(table.get_comment(tree).is_none());
        assert!(table.get_comment_text(tree).is_none());
        assert!(table.get_comment_tree(tree).is_none());
    }

    #[test]
    fn put_and_get_comment_text_and_tree() {
        let mut table = table();
        let tree = TreeId::new(1);
        let comment = comment("doc");
        table.put_comment(tree, comment.clone());
        assert!(table.has_comment(tree));
        assert_eq!(table.get_comment_text(tree), Some(JavaString::from("doc")));
        assert_eq!(
            table.get_comment(tree).expect("comment").get_source_pos(2),
            7
        );
        assert!(table.get_comment_tree(tree).is_none());

        table.trees.insert(tree, doc_comment_tree(comment, "doc"));
        let parsed = table.get_comment_tree(tree).expect("parsed tree");
        assert_eq!(
            parsed.get_kind(),
            javac_source::doctree::doc_tree::Kind::DOC_COMMENT
        );
        assert_eq!(parsed.to_java_string().to_string_lossy(), "doc");
    }

    #[test]
    fn comment_style_and_deprecated_flag() {
        let mut comment = TestComment {
            text: Some(JavaString::from("deprecated doc")),
            source_pos: 0,
            style: CommentStyle::LINE,
            deprecated: true,
        };
        assert_eq!(comment.get_style(), CommentStyle::LINE);
        assert!(comment.is_deprecated());
        assert_eq!(comment.get_source_pos(9), 9);
        comment.style = CommentStyle::BLOCK;
        assert_eq!(comment.get_style(), CommentStyle::BLOCK);
        let facade: Rc<dyn Comment> = Rc::new(comment);
        assert_eq!(facade.get_text(), Some(JavaString::from("deprecated doc")));
    }
}
