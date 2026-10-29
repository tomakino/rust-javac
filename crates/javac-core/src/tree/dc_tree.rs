// Origin: com/sun/tools/javac/tree/DCTree.java @ 982d53c (jdk-17.0.19-ga)

//! Doc tree node family (Java: DCTree and its nested node classes).
//!
//! PORTING_RULES: doc-tree adapter notes (same policy as JCTree in ADR 0005):
//! - the Java subclass hierarchy becomes one outer struct plus a DCKind enum
//!   (ADR 0005 section 1); the 35 concrete node classes are 35 variants;
//! - child nodes are Rc<DCTree> (doc trees are immutable after construction;
//!   PORTING_RULES section 4 allows Rc for immutable shared data). List<DCTree>
//!   stays util::List<Rc<DCTree>>, Java String becomes JavaString and
//!   javax.lang.model.element.Name stays util::Name;
//! - DCEndPosTree.endPos is a mutable (non-final) int field; in the variants
//!   that extend DCEndPosTree it becomes a Cell<i32> (upstream setEndPos);
//! - getKind() maps to the javac-source doctree Kind. The com.sun.source
//!   doctree interface implementations are the P6 API view and are not written
//!   here (same rule as JCTree, ADR 0005, registered PORTING_RULES section 9);
//! - the upstream Comment type is parser/Tokens.Comment (imported from
//!   crate::parser::tokens; the old placeholder in tree/doc_comment_table.rs
//!   now re-exports the same trait);
//! - DCErroneous's DiagnosticPosition is implemented by DCErroneousPosition
//!   (getTree null, start=pos, preferred=pos+len-1, end=pos+len), matching
//!   DCTree.java:375-428. The factory-based constructor is implemented by
//!   DocTreeMaker::new_erroneous_tree, which builds the JCDiagnostic with the
//!   DCErroneousPosition as the position (the Rust equivalent of the upstream
//!   "this" self reference); a caller-supplied JCDiagnostic constructor also
//!   exists.
//!
//! Ported 1:1 from DCTree.java: pos/getSourcePosition/pos(dc), DCEndPosTree
//! getEndPos/setEndPos, all node fields and getKind mappings.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use javac_source::doctree::attribute_tree::ValueKind;
use javac_source::doctree::doc_tree::Kind;

use crate::ids::TreeId;
use crate::parser::tokens::Comment;
use crate::util::java_string::JavaString;
use crate::util::jc_diagnostic::{JCDiagnostic, SimpleDiagnosticPosition};
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::position::NOPOS;

/// The base class of all doc tree nodes.
///
/// Java: public abstract class DCTree implements DocTree
pub struct DCTree {
    /// The position in the comment string.
    /// Use getSourcePosition to convert it to a position in the source file.
    pub pos: i32,
    /// The node specific fields (Java: the concrete DCTree subclasses).
    pub kind: DCKind,
}

/// The concrete doc tree node classes (Java: the 35 nested DCTree subclasses).
///
/// PORTING_RULES: Java subclass fields become variant fields; the variants that
/// extend DCEndPosTree carry the upstream endPos as Cell<i32>.
#[allow(clippy::large_enum_variant)]
pub enum DCKind {
    /// Java: DCDocComment
    DocComment(DCDocComment),
    /// Java: DCAttribute
    Attribute {
        name: Name,
        vkind: ValueKind,
        value: Option<List<Rc<DCTree>>>,
    },
    /// Java: DCAuthor
    Author { name: List<Rc<DCTree>> },
    /// Java: DCComment
    Comment { body: JavaString },
    /// Java: DCDeprecated
    Deprecated { body: List<Rc<DCTree>> },
    /// Java: DCDocRoot
    DocRoot { end_pos: Cell<i32> },
    /// Java: DCDocType
    DocType { text: JavaString },
    /// Java: DCEndElement
    EndElement { name: Name, end_pos: Cell<i32> },
    /// Java: DCEntity
    Entity { name: Name },
    /// Java: DCErroneous
    Erroneous {
        body: JavaString,
        diag: Rc<JCDiagnostic>,
    },
    /// Java: DCHidden
    Hidden { body: List<Rc<DCTree>> },
    /// Java: DCIdentifier
    Identifier { name: Name },
    /// Java: DCIndex
    Index {
        term: Rc<DCTree>,
        description: List<Rc<DCTree>>,
        end_pos: Cell<i32>,
    },
    /// Java: DCInheritDoc
    InheritDoc { end_pos: Cell<i32> },
    /// Java: DCLink
    Link {
        /// Java: final Kind kind (LINK or LINK_PLAIN)
        kind: Kind,
        /// Java: final DCReference ref
        reference: Rc<DCTree>,
        label: List<Rc<DCTree>>,
        end_pos: Cell<i32>,
    },
    /// Java: DCLiteral
    Literal {
        /// Java: final Kind kind (CODE or LITERAL)
        kind: Kind,
        /// Java: final DCText body
        body: Rc<DCTree>,
        end_pos: Cell<i32>,
    },
    /// Java: DCParam
    Param {
        is_type_parameter: bool,
        /// Java: final DCIdentifier name
        name: Rc<DCTree>,
        description: List<Rc<DCTree>>,
    },
    /// Java: DCProvides
    Provides {
        /// Java: final DCReference serviceType
        service_type: Rc<DCTree>,
        description: List<Rc<DCTree>>,
    },
    /// Java: DCReference
    Reference {
        signature: JavaString,
        /// Java: final JCTree.JCExpression moduleName (null -> None)
        module_name: Option<TreeId>,
        /// Java: final JCTree qualifierExpression (null -> None)
        qualifier_expression: Option<TreeId>,
        /// Java: final Name memberName (null -> None)
        member_name: Option<Name>,
        /// Java: final List<JCTree> paramTypes (null -> None)
        param_types: Option<List<TreeId>>,
        end_pos: Cell<i32>,
    },
    /// Java: DCReturn
    Return {
        inline: bool,
        description: List<Rc<DCTree>>,
        end_pos: Cell<i32>,
    },
    /// Java: DCSee
    See { reference: List<Rc<DCTree>> },
    /// Java: DCSerial
    Serial { description: List<Rc<DCTree>> },
    /// Java: DCSerialData
    SerialData { description: List<Rc<DCTree>> },
    /// Java: DCSerialField
    SerialField {
        /// Java: final DCIdentifier name
        name: Rc<DCTree>,
        /// Java: final DCReference type
        type_: Rc<DCTree>,
        description: List<Rc<DCTree>>,
    },
    /// Java: DCSince
    Since { body: List<Rc<DCTree>> },
    /// Java: DCStartElement
    StartElement {
        name: Name,
        attrs: List<Rc<DCTree>>,
        self_closing: bool,
        end_pos: Cell<i32>,
    },
    /// Java: DCSummary
    Summary {
        summary: List<Rc<DCTree>>,
        end_pos: Cell<i32>,
    },
    /// Java: DCSystemProperty
    SystemProperty {
        property_name: Name,
        end_pos: Cell<i32>,
    },
    /// Java: DCText
    Text { text: JavaString },
    /// Java: DCThrows
    Throws {
        /// Java: final Kind kind (EXCEPTION or THROWS)
        kind: Kind,
        /// Java: final DCReference name
        name: Rc<DCTree>,
        description: List<Rc<DCTree>>,
    },
    /// Java: DCUnknownBlockTag
    UnknownBlockTag {
        name: Name,
        content: List<Rc<DCTree>>,
    },
    /// Java: DCUnknownInlineTag
    UnknownInlineTag {
        name: Name,
        content: List<Rc<DCTree>>,
        end_pos: Cell<i32>,
    },
    /// Java: DCUses
    Uses {
        /// Java: final DCReference serviceType
        service_type: Rc<DCTree>,
        description: List<Rc<DCTree>>,
    },
    /// Java: DCValue
    Value {
        /// Java: final DCReference ref
        reference: Rc<DCTree>,
        end_pos: Cell<i32>,
    },
    /// Java: DCVersion
    Version { body: List<Rc<DCTree>> },
}

/// Java: public static class DCDocComment extends DCTree implements DocCommentTree
pub struct DCDocComment {
    /// Java: public final Comment comment (required for the implicit source pos table)
    pub comment: Rc<dyn Comment>,
    pub full_body: List<Rc<DCTree>>,
    pub first_sentence: List<Rc<DCTree>>,
    pub body: List<Rc<DCTree>>,
    pub tags: List<Rc<DCTree>>,
    pub preamble: List<Rc<DCTree>>,
    pub postamble: List<Rc<DCTree>>,
}

impl DCTree {
    /// Creates a node with the node fields supplied by the factory.
    ///
    /// Java: the package private constructors of the DCTree subclasses plus
    /// tree.pos = pos in DocTreeMaker.
    pub fn new(pos: i32, kind: DCKind) -> DCTree {
        DCTree { pos, kind }
    }

    /// Java: each subclass getKind()
    pub fn get_kind(&self) -> Kind {
        match &self.kind {
            DCKind::DocComment(_) => Kind::DOC_COMMENT,
            DCKind::Attribute { .. } => Kind::ATTRIBUTE,
            DCKind::Author { .. } => Kind::AUTHOR,
            DCKind::Comment { .. } => Kind::COMMENT,
            DCKind::Deprecated { .. } => Kind::DEPRECATED,
            DCKind::DocRoot { .. } => Kind::DOC_ROOT,
            DCKind::DocType { .. } => Kind::DOC_TYPE,
            DCKind::EndElement { .. } => Kind::END_ELEMENT,
            DCKind::Entity { .. } => Kind::ENTITY,
            DCKind::Erroneous { .. } => Kind::ERRONEOUS,
            DCKind::Hidden { .. } => Kind::HIDDEN,
            DCKind::Identifier { .. } => Kind::IDENTIFIER,
            DCKind::Index { .. } => Kind::INDEX,
            DCKind::InheritDoc { .. } => Kind::INHERIT_DOC,
            DCKind::Link { kind, .. } => *kind,
            DCKind::Literal { kind, .. } => *kind,
            DCKind::Param { .. } => Kind::PARAM,
            DCKind::Provides { .. } => Kind::PROVIDES,
            DCKind::Reference { .. } => Kind::REFERENCE,
            DCKind::Return { .. } => Kind::RETURN,
            DCKind::See { .. } => Kind::SEE,
            DCKind::Serial { .. } => Kind::SERIAL,
            DCKind::SerialData { .. } => Kind::SERIAL_DATA,
            DCKind::SerialField { .. } => Kind::SERIAL_FIELD,
            DCKind::Since { .. } => Kind::SINCE,
            DCKind::StartElement { .. } => Kind::START_ELEMENT,
            DCKind::Summary { .. } => Kind::SUMMARY,
            DCKind::SystemProperty { .. } => Kind::SYSTEM_PROPERTY,
            DCKind::Text { .. } => Kind::TEXT,
            DCKind::Throws { kind, .. } => *kind,
            DCKind::UnknownBlockTag { .. } => Kind::UNKNOWN_BLOCK_TAG,
            DCKind::UnknownInlineTag { .. } => Kind::UNKNOWN_INLINE_TAG,
            DCKind::Uses { .. } => Kind::USES,
            DCKind::Value { .. } => Kind::VALUE,
            DCKind::Version { .. } => Kind::VERSION,
        }
    }

    /// Java: public long getSourcePosition(DCDocComment dc)
    pub fn get_source_position(&self, dc: &DCDocComment) -> i64 {
        i64::from(dc.comment.get_source_pos(self.pos))
    }

    /// Java: public JCDiagnostic.DiagnosticPosition pos(DCDocComment dc)
    pub fn pos_of(&self, dc: &DCDocComment) -> SimpleDiagnosticPosition {
        SimpleDiagnosticPosition::new(dc.comment.get_source_pos(self.pos))
    }

    /// Whether this node extends DCEndPosTree upstream.
    pub fn is_end_pos_tree(&self) -> bool {
        self.end_pos_cell().is_some()
    }

    /// Java: public int getEndPos(DCDocComment dc) (DCEndPosTree)
    pub fn get_end_pos(&self, dc: &DCDocComment) -> i32 {
        let end_pos = self.end_pos_value();
        dc.comment.get_source_pos(end_pos)
    }

    /// Java: public T setEndPos(int endPos) (DCEndPosTree)
    ///
    /// PORTING_RULES: the upstream method returns this; the Rust caller keeps
    /// the &Self reference. Panics for nodes that do not extend DCEndPosTree
    /// (the Java cast would throw ClassCastException).
    pub fn set_end_pos(&self, end_pos: i32) -> &Self {
        let cell = self
            .end_pos_cell()
            .unwrap_or_else(|| panic!("DCEndPosTree.setEndPos on a non-DCEndPosTree node"));
        cell.set(end_pos);
        self
    }

    /// The raw (unmapped) endPos value of a DCEndPosTree node.
    pub fn end_pos_value(&self) -> i32 {
        self.end_pos_cell().map_or(NOPOS, |cell| cell.get())
    }

    fn end_pos_cell(&self) -> Option<&Cell<i32>> {
        match &self.kind {
            DCKind::DocRoot { end_pos }
            | DCKind::EndElement { end_pos, .. }
            | DCKind::Index { end_pos, .. }
            | DCKind::InheritDoc { end_pos }
            | DCKind::Link { end_pos, .. }
            | DCKind::Literal { end_pos, .. }
            | DCKind::Reference { end_pos, .. }
            | DCKind::Return { end_pos, .. }
            | DCKind::StartElement { end_pos, .. }
            | DCKind::Summary { end_pos, .. }
            | DCKind::SystemProperty { end_pos, .. }
            | DCKind::UnknownInlineTag { end_pos, .. }
            | DCKind::Value { end_pos, .. } => Some(end_pos),
            _ => None,
        }
    }

    /// Java: DCTree.toString() -- new DocPretty().print(this)
    pub fn to_java_string(&self) -> JavaString {
        crate::tree::doc_pretty::to_java_string(self)
    }
}

impl fmt::Display for DCTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_java_string().to_string_lossy())
    }
}

/// Asserts the runtime node kind of a reference field, mirroring the upstream
/// casts such as (DCReference) ref (ClassCastException -> panic).
pub(crate) fn assert_kind(tree: &Rc<DCTree>, role: &str, expected: Kind) {
    let actual = tree.get_kind();
    assert!(
        actual == expected,
        "{}: expected {:?} but found {:?} (Java: ClassCastException)",
        role,
        expected,
        actual
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::java_string::JavaString;
    use crate::util::jc_diagnostic::{DiagnosticPosition, DiagnosticType};
    use crate::util::names::Names;
    use std::rc::Rc;

    struct TestComment {
        text: Option<JavaString>,
    }

    impl Comment for TestComment {
        fn get_text(&self) -> Option<JavaString> {
            self.text.clone()
        }

        fn get_source_pos(&self, index: i32) -> i32 {
            // Comment text offsets are shifted by 100 in the source file.
            index + 100
        }

        fn get_style(&self) -> crate::parser::tokens::CommentStyle {
            crate::parser::tokens::CommentStyle::JAVADOC
        }

        fn is_deprecated(&self) -> bool {
            false
        }
    }

    fn name(text: &str) -> Name {
        let names = Names::new(false);
        names.table.from_string(&JavaString::from(text))
    }

    fn diagnostic() -> Rc<JCDiagnostic> {
        let messages = Rc::new(crate::util::javac_messages::JavacMessages::new(
            crate::util::javac_messages::DEFAULT_BUNDLE_NAME,
        ));
        let factory = crate::util::jc_diagnostic::Factory::new(messages, "compiler");
        Rc::new(factory.create(DiagnosticType::ERROR, None, None, "test.error", Vec::new()))
    }

    fn nil() -> List<Rc<DCTree>> {
        List::nil()
    }

    fn text_node(text: &str) -> Rc<DCTree> {
        Rc::new(DCTree::new(
            0,
            DCKind::Text {
                text: JavaString::from(text),
            },
        ))
    }

    fn doc_comment() -> DCDocComment {
        DCDocComment {
            comment: Rc::new(TestComment {
                text: Some(JavaString::from("doc")),
            }),
            full_body: nil(),
            first_sentence: nil(),
            body: nil(),
            tags: nil(),
            preamble: nil(),
            postamble: nil(),
        }
    }

    fn one_node_for_each_kind() -> Vec<DCTree> {
        let n = || name("x");
        let text = || text_node("t");
        let cell = || Cell::new(NOPOS);
        vec![
            DCTree::new(0, DCKind::DocComment(doc_comment())),
            DCTree::new(
                0,
                DCKind::Attribute {
                    name: n(),
                    vkind: ValueKind::EMPTY,
                    value: None,
                },
            ),
            DCTree::new(0, DCKind::Author { name: nil() }),
            DCTree::new(
                0,
                DCKind::Comment {
                    body: JavaString::from("c"),
                },
            ),
            DCTree::new(0, DCKind::Deprecated { body: nil() }),
            DCTree::new(0, DCKind::DocRoot { end_pos: cell() }),
            DCTree::new(
                0,
                DCKind::DocType {
                    text: JavaString::from("html"),
                },
            ),
            DCTree::new(
                0,
                DCKind::EndElement {
                    name: n(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(0, DCKind::Entity { name: n() }),
            DCTree::new(
                0,
                DCKind::Erroneous {
                    body: JavaString::from("bad"),
                    diag: diagnostic(),
                },
            ),
            DCTree::new(0, DCKind::Hidden { body: nil() }),
            DCTree::new(0, DCKind::Identifier { name: n() }),
            DCTree::new(
                0,
                DCKind::Index {
                    term: text(),
                    description: nil(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(0, DCKind::InheritDoc { end_pos: cell() }),
            DCTree::new(
                0,
                DCKind::Link {
                    kind: Kind::LINK,
                    reference: text(),
                    label: nil(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Literal {
                    kind: Kind::CODE,
                    body: text(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Param {
                    is_type_parameter: false,
                    name: text(),
                    description: nil(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Provides {
                    service_type: text(),
                    description: nil(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Reference {
                    signature: JavaString::from("java.lang.String"),
                    module_name: None,
                    qualifier_expression: None,
                    member_name: None,
                    param_types: None,
                    end_pos: cell(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Return {
                    inline: false,
                    description: nil(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(0, DCKind::See { reference: nil() }),
            DCTree::new(0, DCKind::Serial { description: nil() }),
            DCTree::new(0, DCKind::SerialData { description: nil() }),
            DCTree::new(
                0,
                DCKind::SerialField {
                    name: text(),
                    type_: text(),
                    description: nil(),
                },
            ),
            DCTree::new(0, DCKind::Since { body: nil() }),
            DCTree::new(
                0,
                DCKind::StartElement {
                    name: n(),
                    attrs: nil(),
                    self_closing: true,
                    end_pos: cell(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Summary {
                    summary: nil(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(
                0,
                DCKind::SystemProperty {
                    property_name: n(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Text {
                    text: JavaString::from("t"),
                },
            ),
            DCTree::new(
                0,
                DCKind::Throws {
                    kind: Kind::THROWS,
                    name: text(),
                    description: nil(),
                },
            ),
            DCTree::new(
                0,
                DCKind::UnknownBlockTag {
                    name: n(),
                    content: nil(),
                },
            ),
            DCTree::new(
                0,
                DCKind::UnknownInlineTag {
                    name: n(),
                    content: nil(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Uses {
                    service_type: text(),
                    description: nil(),
                },
            ),
            DCTree::new(
                0,
                DCKind::Value {
                    reference: text(),
                    end_pos: cell(),
                },
            ),
            DCTree::new(0, DCKind::Version { body: nil() }),
        ]
    }

    #[test]
    fn every_node_class_has_a_kind() {
        let nodes = one_node_for_each_kind();
        assert_eq!(nodes.len(), 35, "DCTree.java has 35 concrete node classes");
        let mut kinds = std::collections::BTreeSet::new();
        for node in &nodes {
            kinds.insert(node.get_kind() as u8);
        }
        // Each node class has its own getKind(); the samples choose LINK, CODE
        // and THROWS for the three nodes that carry a kind field.
        assert_eq!(kinds.len(), 35);
    }

    #[test]
    fn source_positions_are_mapped_through_the_comment() {
        let dc = doc_comment();
        let node = DCTree::new(
            7,
            DCKind::Text {
                text: JavaString::from("abc"),
            },
        );
        assert_eq!(node.get_source_position(&dc), 107);
        let position = node.pos_of(&dc);
        assert_eq!(position.get_start_position(), 107);
        assert_eq!(position.get_preferred_position(), 107);
    }

    #[test]
    fn end_pos_trees_store_and_map_the_end_position() {
        let dc = doc_comment();
        let root = DCTree::new(
            0,
            DCKind::DocRoot {
                end_pos: Cell::new(NOPOS),
            },
        );
        assert!(root.is_end_pos_tree());
        assert_eq!(root.end_pos_value(), NOPOS);
        root.set_end_pos(12);
        assert_eq!(root.end_pos_value(), 12);
        assert_eq!(root.get_end_pos(&dc), 112);

        let text = text_node("t");
        assert!(!text.is_end_pos_tree());
        assert_eq!(text.end_pos_value(), NOPOS);
    }

    #[test]
    #[should_panic(expected = "non-DCEndPosTree")]
    fn set_end_pos_on_a_non_end_pos_tree_panics() {
        let node = text_node("t");
        node.set_end_pos(1);
    }

    #[test]
    #[should_panic(expected = "ClassCastException")]
    fn reference_fields_are_checked_like_upstream_casts() {
        let node = text_node("t");
        assert_kind(&node, "DCLink.ref", Kind::REFERENCE);
    }

    #[test]
    fn link_and_literal_and_throws_kinds_come_from_the_node_field() {
        let cell = || Cell::new(NOPOS);
        let link_plain = DCTree::new(
            0,
            DCKind::Link {
                kind: Kind::LINK_PLAIN,
                reference: text_node("ref"),
                label: nil(),
                end_pos: cell(),
            },
        );
        assert_eq!(link_plain.get_kind(), Kind::LINK_PLAIN);
        let literal = DCTree::new(
            0,
            DCKind::Literal {
                kind: Kind::LITERAL,
                body: text_node("lit"),
                end_pos: cell(),
            },
        );
        assert_eq!(literal.get_kind(), Kind::LITERAL);
        let exception = DCTree::new(
            0,
            DCKind::Throws {
                kind: Kind::EXCEPTION,
                name: text_node("java.io.IOException"),
                description: nil(),
            },
        );
        assert_eq!(exception.get_kind(), Kind::EXCEPTION);
    }
}
