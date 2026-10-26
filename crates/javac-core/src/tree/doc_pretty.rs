// Origin: com/sun/tools/javac/tree/DocPretty.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: tree (tree nodes/scanners for the corresponding upstream feature).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: println (DocPretty.java:104)
// - NOT PORTED: visitComment (DocPretty.java:172)
// - NOT PORTED: visitDeprecated (DocPretty.java:182)
// - NOT PORTED: visitDocRoot (DocPretty.java:211)
// - NOT PORTED: visitDocType (DocPretty.java:223)
// - NOT PORTED: visitErroneous (DocPretty.java:257)
// - NOT PORTED: visitHidden (DocPretty.java:267)
// - NOT PORTED: visitInheritDoc (DocPretty.java:309)
// - NOT PORTED: visitText (DocPretty.java:547)
// End of Gap registration.

//! Prints out a doc-comment tree.
//!
//! PORTING_RULES: adapter notes (same policy as Pretty in this package):
//! - Java Writer/IOException/UncheckedIOException become infallible: the output
//!   is an in-memory JavaString buffer (registered section 9);
//! - the upstream JCTree-style visitor dispatch (DocTreeVisitor) is reproduced
//!   by DocPretty::dispatch (kind -> visit method); the visitor interface itself
//!   belongs to the P6 API view and is not implemented here (ADR 0005);
//! - lineSep is "\n" (the platform newline is normalized for oracle comparison;
//!   registered section 9), matching Pretty;
//! - print(Object) escapes non-ASCII characters with Convert.escapeUnicode,
//!   exactly like the upstream method;
//! - List<DCTree> is iterated with util::List order (the upstream
//!   java.util.List iteration order).

use std::rc::Rc;

use javac_source::doctree::attribute_tree::ValueKind;

use crate::parser::java_string_methods::is_java_whitespace;
use crate::tree::dc_tree::{DCDocComment, DCKind, DCTree};
use crate::util::convert;
use crate::util::java_string::JavaString;
use crate::util::list::List;

/// Prints out a doc-comment tree.
///
/// Java: public class DocPretty implements DocTreeVisitor<Void,Void>
pub struct DocPretty {
    /// The output buffer (Java: Writer out).
    out: JavaString,
}

impl Default for DocPretty {
    fn default() -> DocPretty {
        DocPretty::new()
    }
}

impl DocPretty {
    /// Java: public DocPretty(Writer out)
    pub fn new() -> DocPretty {
        DocPretty {
            out: JavaString::from(""),
        }
    }

    /// The accumulated output (Java: the Writer contents).
    pub fn get_out(&self) -> JavaString {
        self.out.clone()
    }

    /// Visitor method: print a doc tree (Java: null -> "/*missing*/").
    pub fn print(&mut self, tree: Option<&DCTree>) {
        match tree {
            None => self.print_str("/*missing*/"),
            Some(tree) => self.dispatch(tree),
        }
    }

    /// Print a list of trees (Java: public void print(List<? extends DocTree>)).
    pub fn print_list(&mut self, list: &List<Rc<DCTree>>) {
        for tree in list.to_vec() {
            self.print(Some(&tree));
        }
    }

    /// Print a list with separators.
    fn print_list_with_sep(&mut self, list: &List<Rc<DCTree>>, sep: &str) {
        if list.is_empty() {
            return;
        }
        let mut first = true;
        for tree in list.to_vec() {
            if !first {
                self.print_str(sep);
            }
            self.print(Some(&tree));
            first = false;
        }
    }

    /// Java: protected void printTagName(DocTree node)
    fn print_tag_name(&mut self, tree: &DCTree) {
        let tag_name = tree
            .get_kind()
            .tag_name()
            .unwrap_or_else(|| panic!("doc tree kind has no tag name (Java: Writer.write(null))"));
        self.print_str("@");
        self.print_java_string(&tag_name);
    }

    /// Print a Java string escaping non-ascii characters (Java: print(Object)).
    fn print_java_string(&mut self, text: &JavaString) {
        let escaped = convert::escape_unicode(text);
        self.out = self.out.concat(&escaped);
    }

    /// Print a Rust literal (convenience for the upstream String literals).
    fn print_str(&mut self, text: &str) {
        self.print_java_string(&JavaString::from(text));
    }

    /// Visitor dispatch (Java: DocTree.accept -> visitXxx).
    ///
    /// Porting note: the upstream visitOther(DocTree, Void) handles non-DCTree
    /// implementations ("(UNKNOWN: ...)" + newline); dispatch covers every
    /// DCKind and DCTree is the only doc tree type, so visitOther has no
    /// reachable input in the port (registered section 9).
    fn dispatch(&mut self, tree: &DCTree) {
        match &tree.kind {
            DCKind::DocComment(dc) => self.visit_doc_comment(dc),
            DCKind::Attribute { .. } => self.visit_attribute(tree),
            DCKind::Author { .. } => self.visit_author(tree),
            DCKind::Comment { body } => {
                let body = body.clone();
                self.print_java_string(&body);
            }
            DCKind::Deprecated { body } => {
                let body = body.to_vec();
                self.print_tag_name(tree);
                if !body.is_empty() {
                    self.print_str(" ");
                    self.print_list_values(&body);
                }
            }
            DCKind::DocRoot { .. } => {
                self.print_str("{");
                self.print_tag_name(tree);
                self.print_str("}");
            }
            DCKind::DocType { text } => {
                let text = text.clone();
                self.print_java_string(&text);
            }
            DCKind::EndElement { .. } => self.visit_end_element(tree),
            DCKind::Entity { .. } => self.visit_entity(tree),
            DCKind::Erroneous { body, .. } => {
                let body = body.clone();
                self.print_java_string(&body);
            }
            DCKind::Hidden { body } => {
                let body = body.to_vec();
                self.print_tag_name(tree);
                if !body.is_empty() {
                    self.print_str(" ");
                    self.print_list_values(&body);
                }
            }
            DCKind::Identifier { .. } => self.visit_identifier(tree),
            DCKind::Index { .. } => self.visit_index(tree),
            DCKind::InheritDoc { .. } => {
                self.print_str("{");
                self.print_tag_name(tree);
                self.print_str("}");
            }
            DCKind::Link { .. } => self.visit_link(tree),
            DCKind::Literal { .. } => self.visit_literal(tree),
            DCKind::Param { .. } => self.visit_param(tree),
            DCKind::Provides { .. } => self.visit_provides(tree),
            DCKind::Reference { .. } => self.visit_reference(tree),
            DCKind::Return { .. } => self.visit_return(tree),
            DCKind::See { .. } => self.visit_see(tree),
            DCKind::Serial { .. } => self.visit_serial(tree),
            DCKind::SerialData { .. } => self.visit_serial_data(tree),
            DCKind::SerialField { .. } => self.visit_serial_field(tree),
            DCKind::Since { .. } => self.visit_since(tree),
            DCKind::StartElement { .. } => self.visit_start_element(tree),
            DCKind::Summary { .. } => self.visit_summary(tree),
            DCKind::SystemProperty { .. } => self.visit_system_property(tree),
            DCKind::Text { text } => {
                let text = text.clone();
                self.print_java_string(&text);
            }
            DCKind::Throws { .. } => self.visit_throws(tree),
            DCKind::UnknownBlockTag { .. } => self.visit_unknown_block_tag(tree),
            DCKind::UnknownInlineTag { .. } => self.visit_unknown_inline_tag(tree),
            DCKind::Uses { .. } => self.visit_uses(tree),
            DCKind::Value { .. } => self.visit_value(tree),
            DCKind::Version { .. } => self.visit_version(tree),
        }
    }

    fn print_list_values(&mut self, list: &[Rc<DCTree>]) {
        for tree in list {
            self.print(Some(tree));
        }
    }

    /// Java: public Void visitAttribute(AttributeTree node, Void p)
    fn visit_attribute(&mut self, tree: &DCTree) {
        let DCKind::Attribute { name, vkind, value } = &tree.kind else {
            unreachable!("visitAttribute on a non-Attribute node")
        };
        let name = name.to_java_string();
        let vkind = *vkind;
        let value = value.clone();
        self.print_java_string(&name);
        let quote = match vkind {
            ValueKind::EMPTY => None,
            ValueKind::UNQUOTED => Some(""),
            ValueKind::SINGLE => Some("'"),
            ValueKind::DOUBLE => Some("\""),
        };
        if let Some(quote) = quote {
            let mut text = JavaString::from("=");
            text = text.concat(&JavaString::from(quote));
            self.print_java_string(&text);
            if let Some(value) = &value {
                self.print_list(value);
            }
            self.print_str(quote);
        }
    }

    /// Java: public Void visitAuthor(AuthorTree node, Void p)
    fn visit_author(&mut self, tree: &DCTree) {
        let DCKind::Author { name } = &tree.kind else {
            unreachable!("visitAuthor on a non-Author node")
        };
        let name = name.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print_list(&name);
    }

    /// Java: public Void visitDocComment(DocCommentTree node, Void p)
    fn visit_doc_comment(&mut self, dc: &DCDocComment) {
        let body = dc.full_body.clone();
        let tags = dc.tags.clone();
        self.print_list(&body);
        if !body.is_empty() && !tags.is_empty() {
            self.print_str("\n");
        }
        self.print_list_with_sep(&tags, "\n");
    }

    /// Java: public Void visitEndElement(EndElementTree node, Void p)
    fn visit_end_element(&mut self, tree: &DCTree) {
        let DCKind::EndElement { name, .. } = &tree.kind else {
            unreachable!("visitEndElement on a non-EndElement node")
        };
        let name = name.to_java_string();
        self.print_str("</");
        self.print_java_string(&name);
        self.print_str(">");
    }

    /// Java: public Void visitEntity(EntityTree node, Void p)
    fn visit_entity(&mut self, tree: &DCTree) {
        let DCKind::Entity { name } = &tree.kind else {
            unreachable!("visitEntity on a non-Entity node")
        };
        let name = name.to_java_string();
        self.print_str("&");
        self.print_java_string(&name);
        self.print_str(";");
    }

    /// Java: public Void visitIdentifier(IdentifierTree node, Void p)
    fn visit_identifier(&mut self, tree: &DCTree) {
        let DCKind::Identifier { name } = &tree.kind else {
            unreachable!("visitIdentifier on a non-Identifier node")
        };
        let name = name.to_java_string();
        self.print_java_string(&name);
    }

    /// Java: public Void visitIndex(IndexTree node, Void p)
    fn visit_index(&mut self, tree: &DCTree) {
        let DCKind::Index {
            term, description, ..
        } = &tree.kind
        else {
            unreachable!("visitIndex on a non-Index node")
        };
        let term = term.clone();
        let description = description.clone();
        self.print_str("{");
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print(Some(&term));
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
        self.print_str("}");
    }

    /// Java: public Void visitLink(LinkTree node, Void p)
    fn visit_link(&mut self, tree: &DCTree) {
        let DCKind::Link {
            reference, label, ..
        } = &tree.kind
        else {
            unreachable!("visitLink on a non-Link node")
        };
        let reference = reference.clone();
        let label = label.clone();
        self.print_str("{");
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print(Some(&reference));
        if !label.is_empty() {
            self.print_str(" ");
            self.print_list(&label);
        }
        self.print_str("}");
    }

    /// Java: public Void visitLiteral(LiteralTree node, Void p)
    fn visit_literal(&mut self, tree: &DCTree) {
        let DCKind::Literal { body, .. } = &tree.kind else {
            unreachable!("visitLiteral on a non-Literal node")
        };
        let body = body.clone();
        self.print_str("{");
        self.print_tag_name(tree);
        let DCKind::Text { text } = &body.kind else {
            unreachable!("DCLiteral body must be a DCText (Java: cast)")
        };
        let text = text.clone();
        if !text.is_empty() && !is_java_whitespace(text.char_at(0)) {
            self.print_str(" ");
        }
        self.print(Some(&body));
        self.print_str("}");
    }

    /// Java: public Void visitParam(ParamTree node, Void p)
    fn visit_param(&mut self, tree: &DCTree) {
        let DCKind::Param {
            is_type_parameter,
            name,
            description,
        } = &tree.kind
        else {
            unreachable!("visitParam on a non-Param node")
        };
        let is_type_parameter = *is_type_parameter;
        let name = name.clone();
        let description = description.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        if is_type_parameter {
            self.print_str("<");
        }
        self.print(Some(&name));
        if is_type_parameter {
            self.print_str(">");
        }
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
    }

    /// Java: public Void visitProvides(ProvidesTree node, Void p)
    fn visit_provides(&mut self, tree: &DCTree) {
        let DCKind::Provides {
            service_type,
            description,
        } = &tree.kind
        else {
            unreachable!("visitProvides on a non-Provides node")
        };
        let service_type = service_type.clone();
        let description = description.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print(Some(&service_type));
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
    }

    /// Java: public Void visitReference(ReferenceTree node, Void p)
    fn visit_reference(&mut self, tree: &DCTree) {
        let DCKind::Reference { signature, .. } = &tree.kind else {
            unreachable!("visitReference on a non-Reference node")
        };
        let signature = signature.clone();
        self.print_java_string(&signature);
    }

    /// Java: public Void visitReturn(ReturnTree node, Void p)
    fn visit_return(&mut self, tree: &DCTree) {
        let DCKind::Return {
            inline,
            description,
            ..
        } = &tree.kind
        else {
            unreachable!("visitReturn on a non-Return node")
        };
        let inline = *inline;
        let description = description.clone();
        if inline {
            self.print_str("{");
        }
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print_list(&description);
        if inline {
            self.print_str("}");
        }
    }

    /// Java: public Void visitSee(SeeTree node, Void p)
    fn visit_see(&mut self, tree: &DCTree) {
        let DCKind::See { reference } = &tree.kind else {
            unreachable!("visitSee on a non-See node")
        };
        let reference = reference.to_vec();
        self.print_tag_name(tree);
        let mut first = true;
        let mut need_sep = true;
        for item in reference {
            if need_sep {
                self.print_str(" ");
            }
            need_sep = first && item.get_kind() == javac_source::doctree::doc_tree::Kind::REFERENCE;
            first = false;
            self.print(Some(&item));
        }
    }

    /// Java: public Void visitSerial(SerialTree node, Void p)
    fn visit_serial(&mut self, tree: &DCTree) {
        let DCKind::Serial { description } = &tree.kind else {
            unreachable!("visitSerial on a non-Serial node")
        };
        let description = description.clone();
        self.print_tag_name(tree);
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
    }

    /// Java: public Void visitSerialData(SerialDataTree node, Void p)
    fn visit_serial_data(&mut self, tree: &DCTree) {
        let DCKind::SerialData { description } = &tree.kind else {
            unreachable!("visitSerialData on a non-SerialData node")
        };
        let description = description.clone();
        self.print_tag_name(tree);
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
    }

    /// Java: public Void visitSerialField(SerialFieldTree node, Void p)
    fn visit_serial_field(&mut self, tree: &DCTree) {
        let DCKind::SerialField {
            name,
            type_,
            description,
        } = &tree.kind
        else {
            unreachable!("visitSerialField on a non-SerialField node")
        };
        let name = name.clone();
        let type_ = type_.clone();
        let description = description.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print(Some(&name));
        self.print_str(" ");
        self.print(Some(&type_));
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
    }

    /// Java: public Void visitSince(SinceTree node, Void p)
    fn visit_since(&mut self, tree: &DCTree) {
        let DCKind::Since { body } = &tree.kind else {
            unreachable!("visitSince on a non-Since node")
        };
        let body = body.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print_list(&body);
    }

    /// Java: public Void visitStartElement(StartElementTree node, Void p)
    fn visit_start_element(&mut self, tree: &DCTree) {
        let DCKind::StartElement {
            name,
            attrs,
            self_closing,
            ..
        } = &tree.kind
        else {
            unreachable!("visitStartElement on a non-StartElement node")
        };
        let name = name.to_java_string();
        let attrs = attrs.clone();
        let self_closing = *self_closing;
        self.print_str("<");
        self.print_java_string(&name);
        if !attrs.is_empty() {
            self.print_str(" ");
            self.print_list_with_sep(&attrs, " ");
            let last = attrs.last().expect("non-empty attrs");
            if self_closing
                && let DCKind::Attribute { vkind, .. } = &last.kind
                && *vkind == ValueKind::UNQUOTED
            {
                self.print_str(" ");
            }
        }
        if self_closing {
            self.print_str("/");
        }
        self.print_str(">");
    }

    /// Java: public Void visitSummary(SummaryTree node, Void p)
    fn visit_summary(&mut self, tree: &DCTree) {
        let DCKind::Summary { summary, .. } = &tree.kind else {
            unreachable!("visitSummary on a non-Summary node")
        };
        let summary = summary.clone();
        self.print_str("{");
        self.print_tag_name(tree);
        if !summary.is_empty() {
            self.print_str(" ");
            self.print_list(&summary);
        }
        self.print_str("}");
    }

    /// Java: public Void visitSystemProperty(SystemPropertyTree node, Void p)
    fn visit_system_property(&mut self, tree: &DCTree) {
        let DCKind::SystemProperty { property_name, .. } = &tree.kind else {
            unreachable!("visitSystemProperty on a non-SystemProperty node")
        };
        let property_name = property_name.to_java_string();
        self.print_str("{");
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print_java_string(&property_name);
        self.print_str("}");
    }

    /// Java: public Void visitThrows(ThrowsTree node, Void p)
    fn visit_throws(&mut self, tree: &DCTree) {
        let DCKind::Throws {
            name, description, ..
        } = &tree.kind
        else {
            unreachable!("visitThrows on a non-Throws node")
        };
        let name = name.clone();
        let description = description.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print(Some(&name));
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
    }

    /// Java: public Void visitUnknownBlockTag(UnknownBlockTagTree node, Void p)
    fn visit_unknown_block_tag(&mut self, tree: &DCTree) {
        let DCKind::UnknownBlockTag { name, content } = &tree.kind else {
            unreachable!("visitUnknownBlockTag on a non-UnknownBlockTag node")
        };
        let name = name.to_java_string();
        let content = content.clone();
        self.print_str("@");
        self.print_java_string(&name);
        self.print_str(" ");
        self.print_list(&content);
    }

    /// Java: public Void visitUnknownInlineTag(UnknownInlineTagTree node, Void p)
    fn visit_unknown_inline_tag(&mut self, tree: &DCTree) {
        let DCKind::UnknownInlineTag { name, content, .. } = &tree.kind else {
            unreachable!("visitUnknownInlineTag on a non-UnknownInlineTag node")
        };
        let name = name.to_java_string();
        let content = content.clone();
        self.print_str("{");
        self.print_str("@");
        self.print_java_string(&name);
        self.print_str(" ");
        self.print_list(&content);
        self.print_str("}");
    }

    /// Java: public Void visitUses(UsesTree node, Void p)
    fn visit_uses(&mut self, tree: &DCTree) {
        let DCKind::Uses {
            service_type,
            description,
        } = &tree.kind
        else {
            unreachable!("visitUses on a non-Uses node")
        };
        let service_type = service_type.clone();
        let description = description.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print(Some(&service_type));
        if !description.is_empty() {
            self.print_str(" ");
            self.print_list(&description);
        }
    }

    /// Java: public Void visitValue(ValueTree node, Void p)
    fn visit_value(&mut self, tree: &DCTree) {
        let DCKind::Value { reference, .. } = &tree.kind else {
            unreachable!("visitValue on a non-Value node")
        };
        let reference = reference.clone();
        self.print_str("{");
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print(Some(&reference));
        self.print_str("}");
    }

    /// Java: public Void visitVersion(VersionTree node, Void p)
    fn visit_version(&mut self, tree: &DCTree) {
        let DCKind::Version { body } = &tree.kind else {
            unreachable!("visitVersion on a non-Version node")
        };
        let body = body.clone();
        self.print_tag_name(tree);
        self.print_str(" ");
        self.print_list(&body);
    }
}

/// Java: DCTree.toString() -- a fresh DocPretty prints the tree.
pub fn to_java_string(tree: &DCTree) -> JavaString {
    let mut pretty = DocPretty::new();
    pretty.print(Some(tree));
    pretty.get_out()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::dc_tree::{DCKind, DCTree};
    use crate::util::java_string::JavaString;
    use crate::util::list::List;
    use crate::util::names::Names;
    use std::cell::Cell;
    use std::rc::Rc;

    struct TestComment;

    impl crate::parser::tokens::Comment for TestComment {
        fn get_text(&self) -> Option<JavaString> {
            None
        }

        fn get_source_pos(&self, index: i32) -> i32 {
            index
        }

        fn get_style(&self) -> crate::parser::tokens::CommentStyle {
            crate::parser::tokens::CommentStyle::JAVADOC
        }

        fn is_deprecated(&self) -> bool {
            false
        }
    }

    fn name(text: &str) -> crate::util::name::Name {
        let names = Names::new(false);
        names.table.from_string(&JavaString::from(text))
    }

    fn nil() -> List<Rc<DCTree>> {
        List::nil()
    }

    fn text(text: &str) -> Rc<DCTree> {
        Rc::new(DCTree::new(
            0,
            DCKind::Text {
                text: JavaString::from(text),
            },
        ))
    }

    fn reference(signature: &str) -> Rc<DCTree> {
        Rc::new(DCTree::new(
            0,
            DCKind::Reference {
                signature: JavaString::from(signature),
                module_name: None,
                qualifier_expression: None,
                member_name: None,
                param_types: None,
                end_pos: Cell::new(-1),
            },
        ))
    }

    fn list(items: Vec<Rc<DCTree>>) -> List<Rc<DCTree>> {
        List::from_iterable(items)
    }

    fn render(tree: &DCTree) -> String {
        to_java_string(tree).to_string_lossy()
    }

    #[test]
    fn prints_literal_and_link_family() {
        let code = DCTree::new(
            0,
            DCKind::Literal {
                kind: javac_source::doctree::doc_tree::Kind::CODE,
                body: text("x + y"),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&code), "{@code x + y}");

        // A leading space in the body does not add a separator (upstream rule).
        let spaced = DCTree::new(
            0,
            DCKind::Literal {
                kind: javac_source::doctree::doc_tree::Kind::CODE,
                body: text(" indented"),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&spaced), "{@code indented}");

        let link = DCTree::new(
            0,
            DCKind::Link {
                kind: javac_source::doctree::doc_tree::Kind::LINK,
                reference: reference("java.lang.String"),
                label: list(vec![text("String")]),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&link), "{@link java.lang.String String}");

        let plain = DCTree::new(
            0,
            DCKind::Link {
                kind: javac_source::doctree::doc_tree::Kind::LINK_PLAIN,
                reference: reference("java.util.List"),
                label: nil(),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&plain), "{@linkplain java.util.List}");
    }

    #[test]
    fn prints_block_tags_and_comment_body() {
        let author = DCTree::new(
            0,
            DCKind::Author {
                name: list(vec![text("Alice")]),
            },
        );
        assert_eq!(render(&author), "@author Alice");

        let param = DCTree::new(
            0,
            DCKind::Param {
                is_type_parameter: true,
                name: text("T"),
                description: list(vec![text("the type")]),
            },
        );
        assert_eq!(render(&param), "@param <T> the type");

        let see = DCTree::new(
            0,
            DCKind::See {
                reference: list(vec![reference("java.lang.String"), text("and more")]),
            },
        );
        assert_eq!(render(&see), "@see java.lang.String and more");

        let throws = DCTree::new(
            0,
            DCKind::Throws {
                kind: javac_source::doctree::doc_tree::Kind::EXCEPTION,
                name: reference("java.io.IOException"),
                description: list(vec![text("when it fails")]),
            },
        );
        assert_eq!(
            render(&throws),
            "@exception java.io.IOException when it fails"
        );

        let doc_comment = DCTree::new(
            0,
            DCKind::DocComment(DCDocComment {
                comment: Rc::new(TestComment),
                full_body: list(vec![text("First sentence.")]),
                first_sentence: list(vec![text("First sentence.")]),
                body: nil(),
                tags: list(vec![text("@author Alice")]),
                preamble: nil(),
                postamble: nil(),
            }),
        );
        assert_eq!(render(&doc_comment), "First sentence.\n@author Alice");
    }

    #[test]
    fn prints_html_elements_attributes_and_entities() {
        let attr = Rc::new(DCTree::new(
            0,
            DCKind::Attribute {
                name: name("href"),
                vkind: ValueKind::DOUBLE,
                value: Some(list(vec![text("http://example.com")])),
            },
        ));
        assert_eq!(render(&attr), "href=\"http://example.com\"");

        let attr_unquoted = Rc::new(DCTree::new(
            0,
            DCKind::Attribute {
                name: name("checked"),
                vkind: ValueKind::UNQUOTED,
                value: Some(list(vec![text("checked")])),
            },
        ));
        assert_eq!(render(&attr_unquoted), "checked=checked");

        let attr_empty = DCTree::new(
            0,
            DCKind::Attribute {
                name: name("disabled"),
                vkind: ValueKind::EMPTY,
                value: None,
            },
        );
        assert_eq!(render(&attr_empty), "disabled");

        let start = DCTree::new(
            0,
            DCKind::StartElement {
                name: name("a"),
                attrs: list(vec![attr]),
                self_closing: false,
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&start), "<a href=\"http://example.com\">");

        let start_self_closing = DCTree::new(
            0,
            DCKind::StartElement {
                name: name("br"),
                attrs: nil(),
                self_closing: true,
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&start_self_closing), "<br/>");

        let start_unquoted_self_closing = DCTree::new(
            0,
            DCKind::StartElement {
                name: name("input"),
                attrs: list(vec![attr_unquoted]),
                self_closing: true,
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(
            render(&start_unquoted_self_closing),
            "<input checked=checked />"
        );

        let end = DCTree::new(
            0,
            DCKind::EndElement {
                name: name("a"),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&end), "</a>");

        let entity = DCTree::new(0, DCKind::Entity { name: name("nbsp") });
        assert_eq!(render(&entity), "&nbsp;");
    }

    #[test]
    fn escapes_like_convert_escape_unicode() {
        // Convert.escapeUnicode escapes characters above 0xFF only (a Latin-1
        // character such as e-acute is written as is, like upstream).
        let text_node = DCTree::new(
            0,
            DCKind::Text {
                text: JavaString::from("a\u{00E9}\u{4E2D}"),
            },
        );
        assert_eq!(render(&text_node), "a\u{00E9}\\u4e2d");
    }

    #[test]
    fn prints_unknown_tags_and_summary_family() {
        let unknown_block = DCTree::new(
            0,
            DCKind::UnknownBlockTag {
                name: name("custom"),
                content: list(vec![text("body")]),
            },
        );
        assert_eq!(render(&unknown_block), "@custom body");

        let unknown_inline = DCTree::new(
            0,
            DCKind::UnknownInlineTag {
                name: name("custom"),
                content: list(vec![text("body")]),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&unknown_inline), "{@custom body}");

        let summary = DCTree::new(
            0,
            DCKind::Summary {
                summary: list(vec![text("short")]),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&summary), "{@summary short}");

        let system_property = DCTree::new(
            0,
            DCKind::SystemProperty {
                property_name: name("user.home"),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&system_property), "{@systemProperty user.home}");

        let value = DCTree::new(
            0,
            DCKind::Value {
                reference: reference("MAX_VALUE"),
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&value), "{@value MAX_VALUE}");

        let inherit = DCTree::new(
            0,
            DCKind::InheritDoc {
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&inherit), "{@inheritDoc}");

        let doc_root = DCTree::new(
            0,
            DCKind::DocRoot {
                end_pos: Cell::new(-1),
            },
        );
        assert_eq!(render(&doc_root), "{@docRoot}");
    }

    #[test]
    fn missing_tree_prints_the_upstream_marker() {
        let mut pretty = DocPretty::new();
        pretty.print(None);
        assert_eq!(pretty.get_out().to_string_lossy(), "/*missing*/");
    }
}
