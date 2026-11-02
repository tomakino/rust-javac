// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: tree (tree nodes/scanners for the corresponding upstream feature).
// Not yet ported upstream methods (no substitutes in this file):
// - REGISTERED P6: cast (DocTreeMaker.java:729) belongs to the DocTreeFactory
//   API bridge (the internal DCTree cast helper is not part of the core tree
//   factory); no substitute is introduced.
// End of Gap registration.

//! A factory for doc-comment trees.
//!
//! adapter notes:
//! - Java DocTreeFactory (com.sun.source.util.DocTreeFactory) is the P6 API view
//!   and is not implemented here (registered in STATUS: the DocTreeFactory
//!   implementation belongs to P6); the factory methods below take and return
//!   the internal DCTree nodes, exactly like the upstream implementation does
//!   with the concrete DCTree subclasses;
//! - Java String -> JavaString, javax.lang.model.element.Name -> util::Name,
//!   List<? extends DocTree> -> slices of Rc<DCTree> (collected into the ported
//!   immutable List<Rc<DCTree>>);
//! - ParserFactory/ReferenceParser are ported; newReferenceTree parses through
//!   the injected ReferenceParser (new_reference_tree) and newErroneousTree is
//!   implemented with the self-position object (DCErroneousPosition);
//! - REGISTERED P6 dependency: getSentenceBreak uses trees.getBreakIterator()
//!   upstream; JavacTrees/BreakIterator are not ported, so the upstream
//!   breakIterator == null path (defaultSentenceBreak) is used and registered
//!   as a difference (no fake break iterator);
//! - Java assert -> assert!, Java IllegalArgumentException -> panic.

use std::rc::Rc;

use javac_source::doctree::attribute_tree::ValueKind;
use javac_source::doctree::doc_tree::Kind;

use crate::ids::TreeId;
use crate::parser::java_string_methods::is_java_whitespace;
use crate::parser::tokens::{Comment, CommentStyle};
use crate::tree::dc_tree::{DCDocComment, DCKind, DCTree, assert_kind};
use crate::util::java_string::JavaString;
use crate::util::jc_diagnostic::{DiagnosticPosition, Factory, JCDiagnostic};
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::position::NOPOS;
use crate::util::string_utils;

/// A subset of block tags which act as sentence breakers, appearing anywhere
/// but the zero'th position in the first sentence.
///
/// Java: final Set<String> sentenceBreakTags = Set.of("H1", ..., "P")
const SENTENCE_BREAK_TAGS: [&str; 8] = ["H1", "H2", "H3", "H4", "H5", "H6", "PRE", "P"];

/// A factory for doc-comment trees.
///
/// Java: public class DocTreeMaker implements DocTreeFactory
pub struct DocTreeMaker {
    /// The position at which subsequent trees will be created.
    pub pos: i32,
    /// Access to the diag factory for ErroneousTrees.
    ///
    /// Used by newErroneousTree(diagSource, code, args); the accessor keeps
    /// the upstream field visible to P4/P6 code.
    diags: Rc<Factory>,
    /// Java: private final ReferenceParser referenceParser;
    /// injected explicitly (registered adapter; see new()).
    reference_parser: Option<Rc<crate::parser::reference_parser::ReferenceParser>>,
// REGISTERED P6: trees: JavacTrees (used by getSentenceBreak); ReferenceParser
// is injected (reference_parser) for newReferenceTree.
}

impl DocTreeMaker {
    /// Creates a doc tree maker with NOPOS as the initial position.
    ///
    /// upstream protected DocTreeMaker(Context context) obtains
    /// JCDiagnostic.Factory, JavacTrees and ReferenceParser from the context;
    /// the factory is passed in and the ReferenceParser is injected through
    /// set_reference_parser (registered adapter). JavacTrees remains a P6
    /// REGISTERED dependency for getSentenceBreak.
    pub fn new(diags: Rc<Factory>) -> DocTreeMaker {
        DocTreeMaker {
            pos: NOPOS,
            diags,
            reference_parser: None,
        }
    }

    /// The diagnostic factory supplied at construction (Java: private final
    /// JCDiagnostic.Factory diags).
    pub fn diags(&self) -> &Rc<Factory> {
        &self.diags
    }

    /// Installs the reference parser (Java: the field is created by the
    /// DocTreeMaker(ParserFactory) constructor; the ported constructor takes the
    /// diagnostic factory only, so the parser is injected explicitly).
    pub fn set_reference_parser(
        &mut self,
        parser: Rc<crate::parser::reference_parser::ReferenceParser>,
    ) {
        self.reference_parser = Some(parser);
    }

    /// Reassign current position.
    ///
    /// Java: public DocTreeMaker at(int pos)
    pub fn at(&mut self, pos: i32) -> &mut DocTreeMaker {
        self.pos = pos;
        self
    }

    /// Reassign current position.
    ///
    /// Java: public DocTreeMaker at(DiagnosticPosition pos)
    pub fn at_diagnostic_position(
        &mut self,
        pos: Option<&dyn DiagnosticPosition>,
    ) -> &mut DocTreeMaker {
        self.pos = pos.map_or(NOPOS, |position| position.get_start_position());
        self
    }

    fn node(&self, kind: DCKind) -> Rc<DCTree> {
        Rc::new(DCTree::new(self.pos, kind))
    }

    fn to_list(items: &[Rc<DCTree>]) -> List<Rc<DCTree>> {
        List::from_iterable(items.iter().cloned())
    }

    /// Java: public DCAttribute newAttributeTree(Name name, ValueKind vkind,
    /// List<? extends DocTree> value)
    pub fn new_attribute_tree(
        &mut self,
        name: Name,
        vkind: ValueKind,
        value: Option<&[Rc<DCTree>]>,
    ) -> Rc<DCTree> {
        // Java: Assert.check((vkind == ValueKind.EMPTY) ? (value == null) : (value != null));
        assert!(
            if vkind == ValueKind::EMPTY {
                value.is_none()
            } else {
                value.is_some()
            },
            "DCAttribute value kind/value mismatch (Java: Assert.check)"
        );
        let value = value.map(Self::to_list);
        self.node(DCKind::Attribute { name, vkind, value })
    }

    /// Java: public DCAuthor newAuthorTree(List<? extends DocTree> name)
    pub fn new_author_tree(&mut self, name: &[Rc<DCTree>]) -> Rc<DCTree> {
        let name = Self::to_list(name);
        self.node(DCKind::Author { name })
    }

    /// Java: public DCLiteral newCodeTree(TextTree text)
    pub fn new_code_tree(&mut self, text: &Rc<DCTree>) -> Rc<DCTree> {
        assert_kind(text, "DCLiteral body", Kind::TEXT);
        self.node(DCKind::Literal {
            kind: Kind::CODE,
            body: text.clone(),
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCComment newCommentTree(String text)
    pub fn new_comment_tree(&mut self, text: JavaString) -> Rc<DCTree> {
        self.node(DCKind::Comment { body: text })
    }

    /// Java: public DCDeprecated newDeprecatedTree(List<? extends DocTree> text)
    pub fn new_deprecated_tree(&mut self, text: &[Rc<DCTree>]) -> Rc<DCTree> {
        let body = Self::to_list(text);
        self.node(DCKind::Deprecated { body })
    }

    /// Java: public DCDocComment newDocCommentTree(List<? extends DocTree>
    /// fullBody, List<? extends DocTree> tags)
    pub fn new_doc_comment_tree(
        &mut self,
        full_body: &[Rc<DCTree>],
        tags: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        self.new_doc_comment_tree_with_preamble(full_body, tags, &[], &[])
    }

    /// Java: public DCDocComment newDocCommentTree(Comment comment,
    /// List<? extends DocTree> fullBody, List<? extends DocTree> tags,
    /// List<? extends DocTree> preamble, List<? extends DocTree> postamble)
    pub fn new_doc_comment_tree_with_comment(
        &mut self,
        comment: Rc<dyn Comment>,
        full_body: &[Rc<DCTree>],
        tags: &[Rc<DCTree>],
        preamble: &[Rc<DCTree>],
        postamble: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        let (first_sentence, body) = self.split_body(full_body);
        let dc = DCDocComment {
            comment,
            full_body: Self::to_list(full_body),
            first_sentence,
            body,
            tags: Self::to_list(tags),
            preamble: Self::to_list(preamble),
            postamble: Self::to_list(postamble),
        };
        self.node(DCKind::DocComment(dc))
    }

    /// Java: public DCDocComment newDocCommentTree(List<? extends DocTree>
    /// fullBody, List<? extends DocTree> tags, List<? extends DocTree> preamble,
    /// List<? extends DocTree> postamble)
    ///
    /// upstream uses an anonymous dummy Comment and does not set
    /// tree.pos on this path (the Java field keeps its 0 default; reproduced by
    /// creating the node at position 0).
    pub fn new_doc_comment_tree_with_preamble(
        &mut self,
        full_body: &[Rc<DCTree>],
        tags: &[Rc<DCTree>],
        preamble: &[Rc<DCTree>],
        postamble: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        let saved_pos = self.pos;
        let (first_sentence, body) = self.split_body(full_body);
        self.pos = saved_pos;
        // A dummy comment to keep the diagnostics logic happy.
        let comment: Rc<dyn Comment> = Rc::new(DummyComment);
        let dc = DCDocComment {
            comment,
            full_body: Self::to_list(full_body),
            first_sentence,
            body,
            tags: Self::to_list(tags),
            preamble: Self::to_list(preamble),
            postamble: Self::to_list(postamble),
        };
        Rc::new(DCTree::new(0, DCKind::DocComment(dc)))
    }

    /// Java: public DCDocRoot newDocRootTree()
    pub fn new_doc_root_tree(&mut self) -> Rc<DCTree> {
        self.node(DCKind::DocRoot {
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCDocType newDocTypeTree(String text)
    pub fn new_doc_type_tree(&mut self, text: JavaString) -> Rc<DCTree> {
        self.node(DCKind::DocType { text })
    }

    /// Java: public DCEndElement newEndElementTree(Name name)
    pub fn new_end_element_tree(&mut self, name: Name) -> Rc<DCTree> {
        self.node(DCKind::EndElement {
            name,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCEntity newEntityTree(Name name)
    pub fn new_entity_tree(&mut self, name: Name) -> Rc<DCTree> {
        self.node(DCKind::Entity { name })
    }

    /// Java: public DCErroneous newErroneousTree(String text, Diagnostic<JavaFileObject> diag)
    pub fn new_erroneous_tree_with_diagnostic(
        &mut self,
        text: JavaString,
        diag: Rc<JCDiagnostic>,
    ) -> Rc<DCTree> {
        self.node(DCKind::Erroneous {
            body: text,
            diag,
        })
    }

    /// Java: public DCErroneous newErroneousTree(String text,
    /// DiagnosticSource diagSource, String code, Object... args)
    /// (DocTreeMaker.java:292-296 / DCTree.java:375-428).
    ///
    /// Adapter: the upstream DCErroneous is its own
    /// JCDiagnostic.DiagnosticPosition; the ported position object carries the
    /// same observable values (getTree null, start = pos, preferred =
    /// pos + body.length() - 1, end = pos + body.length()) without an Rc cycle.
    pub fn new_erroneous_tree(
        &mut self,
        text: JavaString,
        diag_source: Option<Rc<crate::util::diagnostic_source::DiagnosticSource>>,
        code: &str,
        args: Vec<crate::util::jc_diagnostic::DiagnosticArg>,
    ) -> Rc<DCTree> {
        let body_len = text.as_units().len() as i32;
        let position: Rc<dyn DiagnosticPosition> =
            Rc::new(DCErroneousPosition::new(self.pos, body_len));
        let diag = self.diags.create(
            crate::util::jc_diagnostic::DiagnosticType::ERROR,
            diag_source,
            Some(position),
            code,
            args,
        );
        self.node(DCKind::Erroneous {
            body: text,
            diag: Rc::new(diag),
        })
    }

    /// Java: public DCThrows newExceptionTree(ReferenceTree name,
    /// List<? extends DocTree> description)
    pub fn new_exception_tree(
        &mut self,
        name: &Rc<DCTree>,
        description: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        assert_kind(name, "DCThrows.name", Kind::REFERENCE);
        let description = Self::to_list(description);
        self.node(DCKind::Throws {
            kind: Kind::EXCEPTION,
            name: name.clone(),
            description,
        })
    }

    /// Java: public DCHidden newHiddenTree(List<? extends DocTree> text)
    pub fn new_hidden_tree(&mut self, text: &[Rc<DCTree>]) -> Rc<DCTree> {
        let body = Self::to_list(text);
        self.node(DCKind::Hidden { body })
    }

    /// Java: public DCIdentifier newIdentifierTree(Name name)
    pub fn new_identifier_tree(&mut self, name: Name) -> Rc<DCTree> {
        self.node(DCKind::Identifier { name })
    }

    /// Java: public DCIndex newIndexTree(DocTree term,
    /// List<? extends DocTree> description)
    pub fn new_index_tree(&mut self, term: &Rc<DCTree>, description: &[Rc<DCTree>]) -> Rc<DCTree> {
        let description = Self::to_list(description);
        self.node(DCKind::Index {
            term: term.clone(),
            description,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCInheritDoc newInheritDocTree()
    pub fn new_inherit_doc_tree(&mut self) -> Rc<DCTree> {
        self.node(DCKind::InheritDoc {
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCLink newLinkTree(ReferenceTree ref,
    /// List<? extends DocTree> label)
    pub fn new_link_tree(&mut self, ref_: &Rc<DCTree>, label: &[Rc<DCTree>]) -> Rc<DCTree> {
        assert_kind(ref_, "DCLink.ref", Kind::REFERENCE);
        let label = Self::to_list(label);
        self.node(DCKind::Link {
            kind: Kind::LINK,
            reference: ref_.clone(),
            label,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCLink newLinkPlainTree(ReferenceTree ref,
    /// List<? extends DocTree> label)
    pub fn new_link_plain_tree(&mut self, ref_: &Rc<DCTree>, label: &[Rc<DCTree>]) -> Rc<DCTree> {
        assert_kind(ref_, "DCLink.ref", Kind::REFERENCE);
        let label = Self::to_list(label);
        self.node(DCKind::Link {
            kind: Kind::LINK_PLAIN,
            reference: ref_.clone(),
            label,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCLiteral newLiteralTree(TextTree text)
    pub fn new_literal_tree(&mut self, text: &Rc<DCTree>) -> Rc<DCTree> {
        assert_kind(text, "DCLiteral body", Kind::TEXT);
        self.node(DCKind::Literal {
            kind: Kind::LITERAL,
            body: text.clone(),
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCParam newParamTree(boolean isTypeParameter,
    /// IdentifierTree name, List<? extends DocTree> description)
    pub fn new_param_tree(
        &mut self,
        is_type_parameter: bool,
        name: &Rc<DCTree>,
        description: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        assert_kind(name, "DCParam.name", Kind::IDENTIFIER);
        let description = Self::to_list(description);
        self.node(DCKind::Param {
            is_type_parameter,
            name: name.clone(),
            description,
        })
    }

    /// Java: public DCProvides newProvidesTree(ReferenceTree name,
    /// List<? extends DocTree> description)
    pub fn new_provides_tree(
        &mut self,
        name: &Rc<DCTree>,
        description: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        assert_kind(name, "DCProvides.serviceType", Kind::REFERENCE);
        let description = Self::to_list(description);
        self.node(DCKind::Provides {
            service_type: name.clone(),
            description,
        })
    }

    /// Java: public DCReference newReferenceTree(String signature)
    ///
    /// Uses the injected parser.ReferenceParser; a parse error corresponds to
    /// the upstream IllegalArgumentException (panic channel).
    pub fn new_reference_tree(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        signature: JavaString,
    ) -> Rc<DCTree> {
        let reference_parser = self
            .reference_parser
            .as_ref()
            .expect("AssertionError: DocTreeMaker.referenceParser")
            .clone();
        match reference_parser.parse(ctx, &signature) {
            Ok(reference) => self.new_reference_tree_with_parts(
                signature,
                reference.module_name,
                reference.qual_expr,
                reference.member,
                reference.param_types,
            ),
            Err(error) => panic!(
                "IllegalArgumentException: invalid signature: {}",
                error.get_message()
            ),
        }
    }

    /// Java: public DCReference newReferenceTree(String signature,
    /// JCTree.JCExpression moduleName, JCTree qualExpr, Name member,
    /// List<JCTree> paramTypes)
    pub fn new_reference_tree_with_parts(
        &mut self,
        signature: JavaString,
        module_name: Option<TreeId>,
        qual_expr: Option<TreeId>,
        member: Option<Name>,
        param_types: Option<List<TreeId>>,
    ) -> Rc<DCTree> {
        self.node(DCKind::Reference {
            signature,
            module_name,
            qualifier_expression: qual_expr,
            member_name: member,
            param_types,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCReturn newReturnTree(List<? extends DocTree> description)
    pub fn new_return_tree(&mut self, description: &[Rc<DCTree>]) -> Rc<DCTree> {
        self.new_return_tree_with_inline(false, description)
    }

    /// Java: public DCReturn newReturnTree(boolean isInline,
    /// List<? extends DocTree> description)
    pub fn new_return_tree_with_inline(
        &mut self,
        is_inline: bool,
        description: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        let description = Self::to_list(description);
        self.node(DCKind::Return {
            inline: is_inline,
            description,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCSee newSeeTree(List<? extends DocTree> reference)
    pub fn new_see_tree(&mut self, reference: &[Rc<DCTree>]) -> Rc<DCTree> {
        let reference = Self::to_list(reference);
        self.node(DCKind::See { reference })
    }

    /// Java: public DCSerial newSerialTree(List<? extends DocTree> description)
    pub fn new_serial_tree(&mut self, description: &[Rc<DCTree>]) -> Rc<DCTree> {
        let description = Self::to_list(description);
        self.node(DCKind::Serial { description })
    }

    /// Java: public DCSerialData newSerialDataTree(
    /// List<? extends DocTree> description)
    pub fn new_serial_data_tree(&mut self, description: &[Rc<DCTree>]) -> Rc<DCTree> {
        let description = Self::to_list(description);
        self.node(DCKind::SerialData { description })
    }

    /// Java: public DCSerialField newSerialFieldTree(IdentifierTree name,
    /// ReferenceTree type, List<? extends DocTree> description)
    pub fn new_serial_field_tree(
        &mut self,
        name: &Rc<DCTree>,
        type_: &Rc<DCTree>,
        description: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        assert_kind(name, "DCSerialField.name", Kind::IDENTIFIER);
        assert_kind(type_, "DCSerialField.type", Kind::REFERENCE);
        let description = Self::to_list(description);
        self.node(DCKind::SerialField {
            name: name.clone(),
            type_: type_.clone(),
            description,
        })
    }

    /// Java: public DCSince newSinceTree(List<? extends DocTree> text)
    pub fn new_since_tree(&mut self, text: &[Rc<DCTree>]) -> Rc<DCTree> {
        let body = Self::to_list(text);
        self.node(DCKind::Since { body })
    }

    /// Java: public DCStartElement newStartElementTree(Name name,
    /// List<? extends DocTree> attrs, boolean selfClosing)
    pub fn new_start_element_tree(
        &mut self,
        name: Name,
        attrs: &[Rc<DCTree>],
        self_closing: bool,
    ) -> Rc<DCTree> {
        let attrs = Self::to_list(attrs);
        self.node(DCKind::StartElement {
            name,
            attrs,
            self_closing,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCSummary newSummaryTree(List<? extends DocTree> text)
    pub fn new_summary_tree(&mut self, text: &[Rc<DCTree>]) -> Rc<DCTree> {
        let summary = Self::to_list(text);
        self.node(DCKind::Summary {
            summary,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCSystemProperty newSystemPropertyTree(Name propertyName)
    pub fn new_system_property_tree(&mut self, property_name: Name) -> Rc<DCTree> {
        self.node(DCKind::SystemProperty {
            property_name,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCText newTextTree(String text)
    pub fn new_text_tree(&mut self, text: JavaString) -> Rc<DCTree> {
        self.node(DCKind::Text { text })
    }

    /// Java: public DCThrows newThrowsTree(ReferenceTree name,
    /// List<? extends DocTree> description)
    pub fn new_throws_tree(&mut self, name: &Rc<DCTree>, description: &[Rc<DCTree>]) -> Rc<DCTree> {
        assert_kind(name, "DCThrows.name", Kind::REFERENCE);
        let description = Self::to_list(description);
        self.node(DCKind::Throws {
            kind: Kind::THROWS,
            name: name.clone(),
            description,
        })
    }

    /// Java: public DCUnknownBlockTag newUnknownBlockTagTree(Name name,
    /// List<? extends DocTree> content)
    pub fn new_unknown_block_tag_tree(&mut self, name: Name, content: &[Rc<DCTree>]) -> Rc<DCTree> {
        let content = Self::to_list(content);
        self.node(DCKind::UnknownBlockTag { name, content })
    }

    /// Java: public DCUnknownInlineTag newUnknownInlineTagTree(Name name,
    /// List<? extends DocTree> content)
    pub fn new_unknown_inline_tag_tree(
        &mut self,
        name: Name,
        content: &[Rc<DCTree>],
    ) -> Rc<DCTree> {
        let content = Self::to_list(content);
        self.node(DCKind::UnknownInlineTag {
            name,
            content,
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCUses newUsesTree(ReferenceTree name,
    /// List<? extends DocTree> description)
    pub fn new_uses_tree(&mut self, name: &Rc<DCTree>, description: &[Rc<DCTree>]) -> Rc<DCTree> {
        assert_kind(name, "DCUses.serviceType", Kind::REFERENCE);
        let description = Self::to_list(description);
        self.node(DCKind::Uses {
            service_type: name.clone(),
            description,
        })
    }

    /// Java: public DCValue newValueTree(ReferenceTree ref)
    pub fn new_value_tree(&mut self, ref_: &Rc<DCTree>) -> Rc<DCTree> {
        assert_kind(ref_, "DCValue.ref", Kind::REFERENCE);
        self.node(DCKind::Value {
            reference: ref_.clone(),
            end_pos: std::cell::Cell::new(NOPOS),
        })
    }

    /// Java: public DCVersion newVersionTree(List<? extends DocTree> text)
    pub fn new_version_tree(&mut self, text: &[Rc<DCTree>]) -> Rc<DCTree> {
        let body = Self::to_list(text);
        self.node(DCKind::Version { body })
    }

    /// Java: public List<DocTree> getFirstSentence(List<? extends DocTree> list)
    pub fn get_first_sentence(&mut self, list: &[Rc<DCTree>]) -> Vec<Rc<DCTree>> {
        let (first_sentence, _body) = self.split_body(list);
        first_sentence.to_vec()
    }

    /*
     * Breaks up the body tags into the first sentence and its successors.
     * The first sentence is determined with the presence of a period,
     * block tag, or a sentence break, as returned by the BreakIterator.
     * Trailing whitespaces are trimmed.
     *
     * Rust: the upstream ListIterator loop is reproduced with an index
     * (hasPrevious/next/nextIndex visits map to index 0 / index / index + 1);
     * the consumed adjacent tree in the TEXT case maps to index += 2.
     */
    fn split_body(&mut self, list: &[Rc<DCTree>]) -> (List<Rc<DCTree>>, List<Rc<DCTree>>) {
        // pos is modified as we create trees, therefore we save the pos and restore it later.
        let saved_pos = self.pos;
        let mut body: Vec<Rc<DCTree>> = Vec::new();
        let mut first_sentence: Vec<Rc<DCTree>> = Vec::new();
        if list.is_empty() {
            self.pos = saved_pos;
            return (List::nil(), List::nil());
        }
        let mut found_first_sentence = false;
        let mut index = 0usize;
        while index < list.len() {
            let is_first = index == 0;
            let tree = list[index].clone();
            let spos = tree.pos;
            if found_first_sentence {
                body.push(tree);
                index += 1;
                continue;
            }
            match &tree.kind {
                DCKind::Return { .. } => {
                    found_first_sentence = true;
                }
                DCKind::Summary { .. } => {
                    found_first_sentence = true;
                }
                DCKind::Text { text } => {
                    let s = text.clone();
                    let peeked_next = list.get(index + 1);
                    let sbreak = self.get_sentence_break(&s, peeked_next);
                    if sbreak > 0 {
                        let s = strip_trailing(&s.substring(0, sbreak as usize));
                        self.pos = spos;
                        let text_node = self.new_text_tree(s);
                        first_sentence.push(text_node);
                        found_first_sentence = true;
                        let nw_pos = skip_white_space(text, sbreak);
                        if nw_pos > 0 {
                            self.pos = spos + nw_pos;
                            let text2 =
                                self.new_text_tree(text.substring(nw_pos as usize, text.len()));
                            body.push(text2);
                        }
                        self.pos = saved_pos;
                        index += 1;
                        continue;
                    } else if let Some(next) = peeked_next {
                        // if the next doctree is a break, remove trailing spaces
                        if self.is_sentence_break(next, false) {
                            let next = next.clone();
                            let s = strip_trailing(&s);
                            self.pos = spos;
                            let text_node = self.new_text_tree(s);
                            first_sentence.push(text_node);
                            body.push(next);
                            self.pos = saved_pos;
                            found_first_sentence = true;
                            index += 2;
                            continue;
                        }
                    }
                }
                _ => {
                    if self.is_sentence_break(&tree, is_first) {
                        body.push(tree);
                        found_first_sentence = true;
                        index += 1;
                        continue;
                    }
                }
            }
            first_sentence.push(tree);
            index += 1;
        }
        self.pos = saved_pos;
        (
            List::from_iterable(first_sentence),
            List::from_iterable(body),
        )
    }

    #[allow(dead_code)]
    fn is_text_tree(tree: &DCTree) -> bool {
        tree.get_kind() == Kind::TEXT
    }

    /*
     * Computes the first sentence break, a simple dot-space algorithm.
     */
    fn default_sentence_break(&self, s: &JavaString) -> i32 {
        // scan for period followed by whitespace
        let mut period = -1i32;
        for i in 0..s.len() {
            match s.char_at(i) {
                unit if unit == '.' as u16 => {
                    period = i as i32;
                }
                unit if unit == ' ' as u16
                    || unit == 0x0c
                    || unit == '\n' as u16
                    || unit == '\r' as u16
                    || unit == '\t' as u16 =>
                {
                    if period >= 0 {
                        return i as i32;
                    }
                }
                _ => {
                    period = -1;
                }
            }
        }
        -1
    }

    /*
     * Computes the first sentence, if using a default breaker,
     * the break is returned, if not then a -1, indicating that
     * more doctree elements are required to be examined.
     *
     * REGISTERED P6: upstream consults trees.getBreakIterator(); JavacTrees and
     * BreakIterator are not ported, so the upstream breakIterator == null path
     * (defaultSentenceBreak) is used (no fake break iterator).
     */
    fn get_sentence_break(&self, s: &JavaString, _dt: Option<&Rc<DCTree>>) -> i32 {
        self.default_sentence_break(s)
    }

    fn is_sentence_break_name(&self, tag_name: &Name) -> bool {
        let upper = string_utils::to_upper_case(&tag_name.to_java_string());
        SENTENCE_BREAK_TAGS.contains(&upper.to_string_lossy().as_str())
    }

    fn is_sentence_break(&self, tree: &Rc<DCTree>, is_first_doc_tree: bool) -> bool {
        match &tree.kind {
            DCKind::StartElement { name, .. } => {
                !is_first_doc_tree && tree.pos > 1 && self.is_sentence_break_name(name)
            }
            DCKind::EndElement { name, .. } => {
                !is_first_doc_tree && tree.pos > 1 && self.is_sentence_break_name(name)
            }
            _ => false,
        }
    }
}

/// Java: private int skipWhiteSpace(String s, int start)
fn skip_white_space(s: &JavaString, start: i32) -> i32 {
    let mut i = start as usize;
    while i < s.len() {
        let unit = s.char_at(i);
        if !is_java_whitespace(unit) {
            return i as i32;
        }
        i += 1;
    }
    -1
}

/// Java: String.stripTrailing() -- removes trailing white space as defined by
/// Character.isWhitespace.
///
/// JavaString has no stripTrailing yet; the port keeps the helper
/// here (to be moved to javac-strings when its API is extended; registered).
fn strip_trailing(s: &JavaString) -> JavaString {
    let units = s.as_units();
    let mut end = units.len();
    while end > 0 && is_java_whitespace(units[end - 1]) {
        end -= 1;
    }
    JavaString::from_units(units[..end].to_vec())
}

/// The anonymous dummy Comment used by newDocCommentTree(fullBody, tags,
/// preamble, postamble).
struct DummyComment;

impl Comment for DummyComment {
    fn get_text(&self) -> Option<JavaString> {
        None
    }

    fn get_source_pos(&self, _index: i32) -> i32 {
        NOPOS
    }

    fn get_style(&self) -> CommentStyle {
        CommentStyle::JAVADOC
    }

    fn is_deprecated(&self) -> bool {
        false
    }
}

/// Java: DCErroneous implements JCDiagnostic.DiagnosticPosition (DCTree.java:375-428):
/// getTree returns null, getStartPosition is pos, getPreferredPosition is
/// pos + body.length() - 1 and getEndPosition is pos + body.length().
struct DCErroneousPosition {
    pos: i32,
    body_len: i32,
}

impl DCErroneousPosition {
    fn new(pos: i32, body_len: i32) -> DCErroneousPosition {
        DCErroneousPosition { pos, body_len }
    }
}

impl DiagnosticPosition for DCErroneousPosition {
    fn get_tree(&self) -> Option<TreeId> {
        None
    }

    fn get_start_position(&self) -> i32 {
        self.pos
    }

    fn get_preferred_position(&self) -> i32 {
        self.pos + self.body_len - 1
    }

    fn get_end_position(
        &self,
        _end_pos_table: Option<&dyn crate::tree::end_pos_table::EndPosTable>,
    ) -> i32 {
        self.pos + self.body_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::javac_messages::{DEFAULT_BUNDLE_NAME, JavacMessages};
    use crate::util::names::Names;

    fn maker() -> DocTreeMaker {
        let messages = Rc::new(JavacMessages::new(DEFAULT_BUNDLE_NAME));
        DocTreeMaker::new(Rc::new(Factory::new(messages, "compiler")))
    }

    fn name(text: &str) -> Name {
        let names = Names::new(false);
        names.table.from_string(&JavaString::from(text))
    }

    fn text(maker: &mut DocTreeMaker, value: &str) -> Rc<DCTree> {
        maker.new_text_tree(JavaString::from(value))
    }

    fn contents(tree: &Rc<DCTree>) -> String {
        tree.to_java_string().to_string_lossy()
    }

    #[test]
    fn at_sets_the_position_of_new_nodes() {
        let mut maker = maker();
        assert_eq!(maker.pos, NOPOS);
        let node = maker.at(42).new_doc_root_tree();
        assert_eq!(node.pos, 42);
        assert_eq!(node.get_kind(), Kind::DOC_ROOT);
    }

    #[test]
    fn at_diagnostic_position_maps_null_to_nopos() {
        let mut maker = maker();
        let position = crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(7);
        let node = maker
            .at_diagnostic_position(Some(&position))
            .new_identifier_tree(name("x"));
        assert_eq!(node.pos, 7);
        let node = maker
            .at_diagnostic_position(None)
            .new_identifier_tree(name("y"));
        assert_eq!(node.pos, NOPOS);
    }

    #[test]
    fn attribute_asserts_value_kind_consistency() {
        let mut maker = maker();
        let value = maker.new_attribute_tree(name("checked"), ValueKind::UNQUOTED, Some(&[]));
        assert_eq!(value.get_kind(), Kind::ATTRIBUTE);
        // EMPTY requires a null value and a value requires a non-null list.
        maker.new_attribute_tree(name("disabled"), ValueKind::EMPTY, None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            maker.new_attribute_tree(name("bad"), ValueKind::EMPTY, Some(&[]));
        }));
        assert!(result.is_err(), "DCAttribute assertion must fire");
    }

    #[test]
    fn factory_methods_build_the_expected_nodes() {
        let mut maker = maker();
        let text_node = text(&mut maker, "hello");
        assert_eq!(text_node.get_kind(), Kind::TEXT);

        let code = maker.new_code_tree(&text_node);
        assert_eq!(code.get_kind(), Kind::CODE);
        assert_eq!(contents(&code), "{@code hello}");

        let ident = maker.new_identifier_tree(name("arg"));
        let description = [text(&mut maker, "the arg")];
        let param = maker.new_param_tree(false, &ident, &description);
        assert_eq!(contents(&param), "@param arg the arg");

        let reference = maker.new_reference_tree_with_parts(
            JavaString::from("java.lang.String"),
            None,
            None,
            None,
            None,
        );
        let label = [text(&mut maker, "String")];
        let link = maker.new_link_tree(&reference, &label);
        assert_eq!(contents(&link), "{@link java.lang.String String}");

        let throws = maker.new_throws_tree(&reference, &[]);
        assert_eq!(contents(&throws), "@throws java.lang.String");

        let exception = maker.new_exception_tree(&reference, &[]);
        assert_eq!(contents(&exception), "@exception java.lang.String");

        let value = maker.new_value_tree(&reference);
        assert_eq!(contents(&value), "{@value java.lang.String}");

        let field = maker.new_serial_field_tree(&ident, &reference, &[]);
        assert_eq!(contents(&field), "@serialField arg java.lang.String");

        let start = maker.new_start_element_tree(name("a"), &[], false);
        assert_eq!(contents(&start), "<a>");
        let end = maker.new_end_element_tree(name("a"));
        assert_eq!(contents(&end), "</a>");
        let entity = maker.new_entity_tree(name("nbsp"));
        assert_eq!(contents(&entity), "&nbsp;");

        let author_name = [text(&mut maker, "Alice")];
        let authored = maker.new_author_tree(&author_name);
        assert_eq!(contents(&authored), "@author Alice");
        let since_text = [text(&mut maker, "1.0")];
        let since = maker.new_since_tree(&since_text);
        assert_eq!(contents(&since), "@since 1.0");
        let version_text = [text(&mut maker, "2.0")];
        let version = maker.new_version_tree(&version_text);
        assert_eq!(contents(&version), "@version 2.0");
        let deprecated_text = [text(&mut maker, "old")];
        let deprecated = maker.new_deprecated_tree(&deprecated_text);
        assert_eq!(contents(&deprecated), "@deprecated old");
        let hidden = maker.new_hidden_tree(&[]);
        assert_eq!(contents(&hidden), "@hidden");
        let serial = maker.new_serial_tree(&[]);
        assert_eq!(contents(&serial), "@serial");
        let serial_data = maker.new_serial_data_tree(&[]);
        assert_eq!(contents(&serial_data), "@serialData");
        let summary_text = [text(&mut maker, "short")];
        let summary = maker.new_summary_tree(&summary_text);
        assert_eq!(contents(&summary), "{@summary short}");
        let system_property = maker.new_system_property_tree(name("user.home"));
        assert_eq!(contents(&system_property), "{@systemProperty user.home}");
        let inline_content = [text(&mut maker, "x")];
        let unknown_inline = maker.new_unknown_inline_tag_tree(name("custom"), &inline_content);
        assert_eq!(contents(&unknown_inline), "{@custom x}");
        let block_content = [text(&mut maker, "x")];
        let unknown_block = maker.new_unknown_block_tag_tree(name("custom"), &block_content);
        assert_eq!(contents(&unknown_block), "@custom x");
        let see_tail = [text(&mut maker, "and")];
        let see = maker.new_see_tree(&[reference.clone(), see_tail[0].clone()]);
        assert_eq!(contents(&see), "@see java.lang.String and");
        let provides = maker.new_provides_tree(&reference, &[]);
        assert_eq!(contents(&provides), "@provides java.lang.String");
        let uses = maker.new_uses_tree(&reference, &[]);
        assert_eq!(contents(&uses), "@uses java.lang.String");
        let index = maker.new_index_tree(&text_node, &[]);
        assert_eq!(contents(&index), "{@index hello}");
        let inherit = maker.new_inherit_doc_tree();
        assert_eq!(contents(&inherit), "{@inheritDoc}");
        let doc_root = maker.new_doc_root_tree();
        assert_eq!(contents(&doc_root), "{@docRoot}");
        let comment = maker.new_comment_tree(JavaString::from("body"));
        assert_eq!(comment.get_kind(), Kind::COMMENT);
    }

    #[test]
    fn return_overloads_set_the_inline_flag() {
        let mut maker = maker();
        let block = maker.new_return_tree(&[]);
        // Upstream visitReturn always prints the separator after the tag name.
        assert_eq!(contents(&block), "@return ");
        let inline_text = [text(&mut maker, "x")];
        let inline = maker.new_return_tree_with_inline(true, &inline_text);
        assert_eq!(contents(&inline), "{@return x}");
        assert_eq!(inline.get_kind(), Kind::RETURN);
    }

    #[test]
    fn split_body_finds_the_first_sentence_and_keeps_the_rest() {
        let mut maker = maker();
        let body = [text(&mut maker, "Hello world. Next sentence.")];
        let first = maker.get_first_sentence(&body);
        assert_eq!(first.len(), 1);
        assert_eq!(contents(&first[0]), "Hello world.");

        // The doc comment itself has firstSentence and body split like upstream.
        let comment = maker.new_doc_comment_tree_with_preamble(&body, &[], &[], &[]);
        let DCKind::DocComment(dc) = &comment.kind else {
            panic!("expected a doc comment node");
        };
        assert_eq!(dc.full_body.length(), 1);
        assert_eq!(dc.first_sentence.length(), 1);
        assert_eq!(contents(dc.first_sentence.head()), "Hello world.");
        assert_eq!(dc.body.length(), 1);
        assert_eq!(contents(dc.body.head()), "Next sentence.");
        // The position is restored after splitBody.
        assert_eq!(maker.pos, NOPOS);
    }

    #[test]
    fn split_body_handles_sentence_break_tags_and_return_summary() {
        {
            let mut maker = maker();
            let leading = [maker.new_text_tree(JavaString::from("Intro"))];
            let start = maker.at(2).new_start_element_tree(name("P"), &[], false);
            let rest = maker.new_text_tree(JavaString::from("Rest."));
            let body = [leading[0].clone(), start.clone(), rest.clone()];
            let first = maker.get_first_sentence(&body);
            assert_eq!(first.len(), 1);
            assert_eq!(contents(&first[0]), "Intro");
        }

        let mut second = maker();
        let summary_text = second.new_text_tree(JavaString::from("sum"));
        let summary = second.new_summary_tree(&[summary_text]);
        let rest = second.new_text_tree(JavaString::from("rest"));
        let body = [summary, rest];
        let first = second.get_first_sentence(&body);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].get_kind(), Kind::SUMMARY);
    }

    #[test]
    fn strip_trailing_removes_java_whitespace() {
        assert_eq!(
            strip_trailing(&JavaString::from("abc  \n")).to_string_lossy(),
            "abc"
        );
        assert_eq!(
            strip_trailing(&JavaString::from("abc")).to_string_lossy(),
            "abc"
        );
        assert_eq!(
            strip_trailing(&JavaString::from("   ")).to_string_lossy(),
            ""
        );
    }

    #[test]
    fn empty_list_has_no_first_sentence() {
        let mut maker = maker();
        let first = maker.get_first_sentence(&[]);
        assert!(first.is_empty());
        let comment = maker.new_doc_comment_tree_with_preamble(&[], &[], &[], &[]);
        let DCKind::DocComment(dc) = &comment.kind else {
            panic!("expected a doc comment node");
        };
        assert!(dc.full_body.is_empty());
        assert!(dc.first_sentence.is_empty());
        assert!(dc.body.is_empty());
    }

    #[test]
    #[should_panic(expected = "DocTreeMaker.referenceParser")]
    fn reference_parsing_needs_reference_parser() {
        let mut maker = maker();
        let mut ctx = crate::compiler_context::CompilerContext::new();
        maker.new_reference_tree(&mut ctx, JavaString::from("java.lang.String"));
    }

    #[test]
    fn reference_tree_parses_the_signature() {
        let mut maker = maker();
        let mut context = crate::util::context::Context::new();
        let names = Rc::new(crate::util::names::Names::new(false));
        let source = crate::code::source::Source::JDK17;
        let scanner_factory = crate::parser::scanner_factory::ScannerFactory::new(
            &mut context,
            Rc::clone(&names),
            source,
        );
        let factory = Rc::new(crate::parser::parser_factory::ParserFactory::new(
            &mut context,
            names,
            source,
            scanner_factory,
            None,
        ));
        maker.set_reference_parser(Rc::new(crate::parser::reference_parser::ReferenceParser::new(
            factory,
        )));
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let tree = maker.new_reference_tree(&mut ctx, JavaString::from("java.lang.String"));
        assert_eq!(tree.get_kind(), Kind::REFERENCE);
    }

    #[test]
    #[should_panic(expected = "ClassCastException")]
    fn factory_checks_reference_argument_kinds() {
        let mut maker = maker();
        let text_node = maker.new_text_tree(JavaString::from("not a ref"));
        maker.new_link_tree(&text_node, &[]);
    }
    #[test]
    fn erroneous_tree_has_the_self_position_and_diagnostic() {
        use crate::util::diagnostic_source::DiagnosticSource;
        let mut maker = maker();
        maker.at(5);
        let tree = maker.new_erroneous_tree(
            JavaString::from("bad"),
            Some(Rc::new(DiagnosticSource::no_source())),
            "compiler.err.test",
            Vec::new(),
        );
        assert_eq!(tree.pos, 5);
        match &tree.kind {
            DCKind::Erroneous { body, diag } => {
                assert_eq!(body.to_string_lossy(), "bad");
                // Java: DCErroneous.getStartPosition() = pos,
                // getPreferredPosition() = pos + body.length() - 1,
                // getEndPosition() = pos + body.length().
                assert_eq!(diag.get_start_position(), 5);
                assert_eq!(diag.get_position(), 7);
                assert_eq!(diag.get_end_position(), 8);
            }
            _ => panic!("expected an erroneous doc tree"),
        }
    }

}
