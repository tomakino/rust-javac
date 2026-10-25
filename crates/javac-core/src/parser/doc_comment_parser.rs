// Origin: com/sun/tools/javac/parser/DocCommentParser.java @ 982d53c (jdk-17.0.19-ga)

//! A parser for doc comments, producing DocTree nodes.
//!
//! PORTING_RULES: adapter notes:
//! - upstream holds ParserFactory fac; ParserFactory is not ported yet
//!   (parser S7), so the three dependencies it provides are injected: Names,
//!   Rc<RefCell<DocTreeMaker>> (the upstream DocTreeMaker is a shared mutable
//!   object: its position changes while parsing; registered RefCell whitelist,
//!   PORTING_RULES section 9) and the DiagnosticSource that the caller takes
//!   from Log.currentSource() (TODO(parser S7));
//! - Java char[]/String buffers become Vec<u16>/JavaString (UTF-16 code unit
//!   semantics, PORTING_RULES section 7); EOI is LayoutCharacters.EOI;
//! - the anonymous TagParser subclasses of createTagParsers() become a
//!   TagParserSpec table plus parse_tag (one match arm per tag), registered as
//!   the Java anonymous-class adaptation;
//! - ParseException is a plain Rust error (Result); the upstream method-name
//!   overloads are disambiguated (inlineTag(ListBuffer) -> inline_tag,
//!   inlineTag() -> inline_tag_tree; entity(ListBuffer) -> entity_into,
//!   entity() -> entity_tree);
//! - TODO(tree batch): erroneous() calls DocTreeMaker.newErroneousTree(text,
//!   diagSource, code), which is still unimplemented! in tree/doc_tree_maker.rs
//!   (the upstream diagnostic position is the ErroneousTree itself). Malformed
//!   input panics with that TODO; well-formed comments do not take this path.
//! - TODO(parser S7): reference() delegates to ReferenceParser, whose fragment
//!   parsing needs JavacParser; comments with references panic with the
//!   ReferenceParser unimplemented! marker until parser S7 lands.
//! - Java Character.isUnicodeIdentifierStart/Part and isJavaIdentifierStart/Part
//!   are approximated with std char classification, consistent with the tokenizer
//!   GAP registered in parser/mod.rs; Character.isWhitespace uses the ported
//!   java_string_methods::is_java_whitespace (same registered GAP).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use javac_source::doctree::attribute_tree::ValueKind;
use javac_source::doctree::doc_tree::Kind;

use crate::parser::java_string_methods::is_java_whitespace;
use crate::parser::reference_parser::{ParseException as ReferenceParseException, ReferenceParser};
use crate::parser::tokens::Comment;
use crate::tree::dc_tree::DCTree;
use crate::tree::doc_tree_maker::DocTreeMaker;
use crate::util::diagnostic_source::DiagnosticSource;
use crate::util::java_string::JavaString;
use crate::util::layout_characters::EOI;
use crate::util::list::List;
use crate::util::list_buffer::ListBuffer;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::position::NOPOS;

/// Java: DocCommentParser.ParseException
#[derive(Debug, Clone)]
pub struct ParseException {
    /// The diagnostic resource key (Java: super(key)).
    pub key: String,
}

impl ParseException {
    pub fn new(key: impl Into<String>) -> ParseException {
        ParseException { key: key.into() }
    }
}

/// Java: private enum Phase
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preamble,
    Body,
    Postamble,
}

/// Java: TagParser.Kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Inline,
    Block,
    Either,
}

/// Java: private enum WhitespaceRetentionPolicy
#[derive(Clone, Copy, PartialEq, Eq)]
enum WhitespaceRetentionPolicy {
    RetainAll,
    RemoveFirstSpace,
    RemoveAll,
}

/// A tag parser entry (Java: an anonymous TagParser subclass registered in
/// createTagParsers()).
#[derive(Clone, Copy)]
struct TagParserSpec {
    kind: TagKind,
    tree_kind: Kind,
    retain_white_space: bool,
}

impl TagParserSpec {
    fn allows_block(&self) -> bool {
        self.kind != TagKind::Inline
    }

    fn allows_inline(&self) -> bool {
        self.kind != TagKind::Block
    }
}

/// A parser for doc comments: consumes the comment text and builds DocTree
/// nodes through DocTreeMaker.
///
/// Java: public class DocCommentParser
pub struct DocCommentParser {
    /// Java: private final ParserFactory fac (names + docTreeMaker injected)
    names: Rc<Names>,
    /// Java: private final DiagnosticSource diagSource (nullable)
    diag_source: Option<Rc<DiagnosticSource>>,
    /// Java: private final Comment comment
    comment: Rc<dyn Comment>,
    /// Java: private final DocTreeMaker m
    m: Rc<RefCell<DocTreeMaker>>,
    /// Java: private final boolean isFileContent
    is_file_content: bool,

    /// The input buffer (Java: char[] buf), index of most recent character read,
    /// index of one past last character in buffer.
    buf: Vec<u16>,
    bp: i32,
    buflen: i32,

    /// The current character.
    ch: u16,

    text_start: i32,
    last_non_white: i32,
    newline: bool,

    /// Java: private final Map<Name, TagParser> tagParsers
    tag_parsers: HashMap<Name, TagParserSpec>,
}

impl DocCommentParser {
    /// PORTING_RULES: upstream DocCommentParser(ParserFactory fac,
    /// DiagnosticSource diagSource, Comment comment, boolean isFileContent);
    /// the ParserFactory dependencies are injected until parser S7.
    pub fn new(
        names: Rc<Names>,
        m: Rc<RefCell<DocTreeMaker>>,
        diag_source: Option<Rc<DiagnosticSource>>,
        comment: Rc<dyn Comment>,
        is_file_content: bool,
    ) -> DocCommentParser {
        let mut parser = DocCommentParser {
            names,
            diag_source,
            comment,
            m,
            is_file_content,
            buf: Vec::new(),
            bp: 0,
            buflen: 0,
            ch: 0,
            text_start: -1,
            last_non_white: -1,
            newline: true,
            tag_parsers: HashMap::new(),
        };
        parser.tag_parsers = parser.create_tag_parsers();
        parser
    }

    /// Java: public DocCommentParser(ParserFactory fac, DiagnosticSource
    /// diagSource, Comment comment)
    pub fn new_for_comment(
        names: Rc<Names>,
        m: Rc<RefCell<DocTreeMaker>>,
        diag_source: Option<Rc<DiagnosticSource>>,
        comment: Rc<dyn Comment>,
    ) -> DocCommentParser {
        DocCommentParser::new(names, m, diag_source, comment, false)
    }

    /// Java: public DCDocComment parse()
    ///
    /// PORTING_RULES: returns the single DCTree node whose kind is
    /// DCKind::DocComment (the port has one node type; see tree/dc_tree.rs).
    pub fn parse(&mut self) -> Rc<DCTree> {
        let text = self.comment.get_text().unwrap_or_else(|| {
            // Java: comment.getText() is assumed non-null; a null text is an NPE.
            panic!("doc comment without text (Java: NullPointerException)")
        });
        let mut buf = text.as_units().to_vec();
        buf.push(u16::from(EOI));
        self.buf = buf;
        self.buflen = self.buf.len() as i32 - 1;
        self.bp = -1;
        self.next_char();

        let preamble = if self.is_file_content {
            self.block_content(Phase::Preamble)
        } else {
            List::nil()
        };
        let body = self.block_content(Phase::Body);
        let tags = self.block_tags();
        let postamble = if self.is_file_content {
            self.block_content(Phase::Postamble)
        } else {
            List::nil()
        };

        let mut pos = NOPOS;
        if !preamble.is_empty() {
            pos = preamble.head().pos;
        } else if !body.is_empty() {
            pos = body.head().pos;
        } else if !tags.is_empty() {
            pos = tags.head().pos;
        } else if !postamble.is_empty() {
            pos = postamble.head().pos;
        }

        let preamble_items = preamble.to_vec();
        let body_items = body.to_vec();
        let tag_items = tags.to_vec();
        let postamble_items = postamble.to_vec();
        self.m.borrow_mut().at(pos).new_doc_comment_tree_with_comment(
            Rc::clone(&self.comment),
            &body_items,
            &tag_items,
            &preamble_items,
            &postamble_items,
        )
    }

    /// Java: void nextChar()
    fn next_char(&mut self) {
        let bp = if self.bp < self.buflen {
            self.bp + 1
        } else {
            self.buflen
        };
        self.bp = bp;
        self.ch = self.buf[bp as usize];
        if self.ch == 0x0c || self.ch == b'\n' as u16 || self.ch == b'\r' as u16 {
            self.newline = true;
        }
    }

    /// Java: protected List<DCTree> blockContent()
    fn block_content_default(&mut self) -> List<Rc<DCTree>> {
        self.block_content(Phase::Body)
    }

    /// Read block content, consisting of text, html and inline tags.
    /// Terminated by the end of input, or the beginning of the next block tag:
    /// that is, @ as the first non-whitespace character on a line.
    fn block_content(&mut self, phase: Phase) -> List<Rc<DCTree>> {
        let mut trees = ListBuffer::new();
        self.text_start = -1;

        'outer: while self.bp < self.buflen {
            match char::from_u32(u32::from(self.ch)) {
                Some('\n') | Some('\r') | Some('\x0c') => {
                    self.newline = true;
                    self.next_char();
                }
                Some(' ') | Some('\t') => {
                    self.next_char();
                }
                Some('&') => {
                    self.entity_into(&mut trees);
                }
                Some('<') => {
                    self.newline = false;
                    if self.is_file_content {
                        match phase {
                            Phase::Preamble => {
                                if self.is_end_preamble() {
                                    trees.add(self.html());
                                    if self.text_start == -1 {
                                        self.text_start = self.bp;
                                        self.last_non_white = -1;
                                    }
                                    // mark this as the start, for processing purposes
                                    self.newline = true;
                                    break 'outer;
                                }
                            }
                            Phase::Body => {
                                if self.is_end_body() {
                                    self.add_pending_text(&mut trees, self.last_non_white);
                                    break 'outer;
                                }
                            }
                            Phase::Postamble => {}
                        }
                    }
                    self.add_pending_text(&mut trees, self.bp - 1);
                    trees.add(self.html());

                    if phase == Phase::Preamble || phase == Phase::Postamble {
                        // Ignore newlines after html tags, in the meta content
                        continue 'outer;
                    }
                    if self.text_start == -1 {
                        self.text_start = self.bp;
                        self.last_non_white = -1;
                    }
                }
                Some('{') => {
                    self.inline_tag(&mut trees);
                }
                Some('@') => {
                    if self.newline {
                        self.add_pending_text(&mut trees, self.last_non_white);
                        break 'outer;
                    }
                    self.newline = false;
                    if self.text_start == -1 {
                        self.text_start = self.bp;
                    }
                    self.last_non_white = self.bp;
                    self.next_char();
                }
                _ => {
                    self.newline = false;
                    if self.text_start == -1 {
                        self.text_start = self.bp;
                    }
                    self.last_non_white = self.bp;
                    self.next_char();
                }
            }
        }

        if self.last_non_white != -1 {
            self.add_pending_text(&mut trees, self.last_non_white);
        }

        trees.to_list()
    }

    /// Read a series of block tags, including their content.
    fn block_tags(&mut self) -> List<Rc<DCTree>> {
        let mut tags = ListBuffer::new();
        while self.bp < self.buflen && self.ch == b'@' as u16 {
            tags.add(self.block_tag());
        }
        tags.to_list()
    }

    /// Read a single block tag, including its content.
    fn block_tag(&mut self) -> Rc<DCTree> {
        let p = self.bp;
        match self.block_tag_inner(p) {
            Ok(tree) => tree,
            Err(error) => {
                self.block_content_default();
                self.erroneous(&error.key, p)
            }
        }
    }

    fn block_tag_inner(&mut self, p: i32) -> Result<Rc<DCTree>, ParseException> {
        self.next_char();
        if Self::is_identifier_start(self.ch) {
            let name = self.read_tag_name();
            let spec = self.tag_parsers.get(&name).copied();
            if let Some(spec) = spec {
                if spec.allows_block() {
                    return self.parse_tag(spec, p, TagKind::Block);
                }
                return Ok(self.erroneous("dc.bad.inline.tag", p));
            }
            let content = self.block_content_default();
            let items = content.to_vec();
            return Ok(self
                .m
                .borrow_mut()
                .at(p)
                .new_unknown_block_tag_tree(name, &items));
        }

        self.block_content_default();
        Ok(self.erroneous("dc.no.tag.name", p))
    }

    /// Java: protected void inlineTag(ListBuffer<DCTree> list)
    fn inline_tag(&mut self, list: &mut ListBuffer<Rc<DCTree>>) {
        self.newline = false;
        self.next_char();
        if self.ch == b'@' as u16 {
            self.add_pending_text(list, self.bp - 2);
            list.add(self.inline_tag_tree());
            self.text_start = self.bp;
            self.last_non_white = -1;
        } else {
            if self.text_start == -1 {
                self.text_start = self.bp - 1;
            }
            self.last_non_white = self.bp;
            self.next_char();
        }
    }

    /// Read a single inline tag, including its content (Java: inlineTag()).
    fn inline_tag_tree(&mut self) -> Rc<DCTree> {
        let p = self.bp - 1;
        match self.inline_tag_inner(p) {
            Ok(tree) => tree,
            Err(error) => self.erroneous(&error.key, p),
        }
    }

    fn inline_tag_inner(&mut self, p: i32) -> Result<Rc<DCTree>, ParseException> {
        self.next_char();
        if !Self::is_identifier_start(self.ch) {
            return Ok(self.erroneous("dc.no.tag.name", p));
        }
        let name = self.read_tag_name();
        let spec = self.tag_parsers.get(&name).copied();
        match spec {
            None => {
                self.skip_whitespace();
                let text = self.inline_text(WhitespaceRetentionPolicy::RemoveAll)?;
                self.next_char();
                let tree = self
                    .m
                    .borrow_mut()
                    .at(p)
                    .new_unknown_inline_tag_tree(name, &[Rc::clone(&text)]);
                tree.set_end_pos(self.bp);
                Ok(Rc::clone(&tree))
            }
            Some(spec) => {
                if !spec.retain_white_space {
                    self.skip_whitespace();
                }
                if spec.allows_inline() {
                    let tree = self.parse_tag(spec, p, TagKind::Inline)?;
                    tree.set_end_pos(self.bp);
                    Ok(tree)
                } else {
                    // handle block tags (for example, @see) in inline content
                    let text = self.inline_text(WhitespaceRetentionPolicy::RemoveAll)?;
                    self.next_char();
                    let tree = self.m.borrow_mut().at(p).new_unknown_inline_tag_tree(
                        name,
                        &[Rc::clone(&text)],
                    );
                    tree.set_end_pos(self.bp);
                    Ok(Rc::clone(&tree))
                }
            }
        }
    }

    /// Read plain text content of an inline tag.
    fn inline_text(
        &mut self,
        whitespace_policy: WhitespaceRetentionPolicy,
    ) -> Result<Rc<DCTree>, ParseException> {
        match whitespace_policy {
            WhitespaceRetentionPolicy::RemoveAll => self.skip_whitespace(),
            WhitespaceRetentionPolicy::RemoveFirstSpace => {
                if self.ch == b' ' as u16 {
                    self.next_char();
                }
            }
            WhitespaceRetentionPolicy::RetainAll => {}
        }
        let pos = self.bp;
        let mut depth = 1;

        while self.bp < self.buflen {
            match char::from_u32(u32::from(self.ch)) {
                Some('\n') | Some('\r') | Some('\x0c') => {
                    self.newline = true;
                }
                Some(' ') | Some('\t') => {}
                Some('{') => {
                    self.newline = false;
                    self.last_non_white = self.bp;
                    depth += 1;
                }
                Some('}') => {
                    depth -= 1;
                    if depth == 0 {
                        let text = self.new_string(pos, self.bp);
                        return Ok(self.m.borrow_mut().at(pos).new_text_tree(text));
                    }
                    self.newline = false;
                    self.last_non_white = self.bp;
                }
                _ => {
                    self.newline = false;
                    self.last_non_white = self.bp;
                }
            }
            self.next_char();
        }
        Err(ParseException::new("dc.unterminated.inline.tag"))
    }

    /// Read general text content of an inline tag, including HTML entities and
    /// elements. Matching pairs of { } are skipped.
    fn inline_content(&mut self) -> List<Rc<DCTree>> {
        let mut trees = ListBuffer::new();

        self.skip_whitespace();
        let pos = self.bp;
        let mut depth = 1;
        self.text_start = -1;

        'outer: while self.bp < self.buflen {
            match char::from_u32(u32::from(self.ch)) {
                Some('\n') | Some('\r') | Some('\x0c') => {
                    self.newline = true;
                    self.next_char();
                }
                Some(' ') | Some('\t') => {
                    self.next_char();
                }
                Some('&') => {
                    self.entity_into(&mut trees);
                }
                Some('<') => {
                    self.newline = false;
                    self.add_pending_text(&mut trees, self.bp - 1);
                    trees.add(self.html());
                    self.text_start = self.bp;
                    self.last_non_white = -1;
                }
                Some('{') => {
                    if self.text_start == -1 {
                        self.text_start = self.bp;
                    }
                    self.newline = false;
                    self.next_char();
                    if self.ch == b'@' as u16 {
                        self.add_pending_text(&mut trees, self.bp - 2);
                        trees.add(self.inline_tag_tree());
                        self.text_start = self.bp;
                        self.last_non_white = -1;
                    } else {
                        depth += 1;
                        self.next_char();
                    }
                }
                Some('}') => {
                    self.newline = false;
                    depth -= 1;
                    if depth == 0 {
                        self.add_pending_text(&mut trees, self.bp - 1);
                        self.next_char();
                        return trees.to_list();
                    }
                    self.next_char();
                }
                Some('@') => {
                    if self.newline {
                        break 'outer;
                    }
                    if self.text_start == -1 {
                        self.text_start = self.bp;
                    }
                    self.next_char();
                }
                _ => {
                    if self.text_start == -1 {
                        self.text_start = self.bp;
                    }
                    self.next_char();
                }
            }
        }

        List::of(self.erroneous("dc.unterminated.inline.tag", pos))
    }

    /// Java: protected void entity(ListBuffer<DCTree> list)
    fn entity_into(&mut self, list: &mut ListBuffer<Rc<DCTree>>) {
        self.newline = false;
        self.add_pending_text(list, self.bp - 1);
        list.add(self.entity_tree());
        if self.text_start == -1 {
            self.text_start = self.bp;
            self.last_non_white = -1;
        }
    }

    /// Read an HTML entity: an identifier or digits or hex digits form.
    fn entity_tree(&mut self) -> Rc<DCTree> {
        let p = self.bp;
        self.next_char();
        let mut name: Option<Name> = None;
        if self.ch == b'#' as u16 {
            let namep = self.bp;
            self.next_char();
            if self.is_decimal_digit(self.ch) {
                self.next_char();
                while self.bp < self.buflen && self.is_decimal_digit(self.ch) {
                    self.next_char();
                }
                name = Some(self.names.from_chars(&self.buf, namep, self.bp - namep));
            } else if self.ch == b'x' as u16 || self.ch == b'X' as u16 {
                self.next_char();
                if self.is_hex_digit(self.ch) {
                    self.next_char();
                    while self.bp < self.buflen && self.is_hex_digit(self.ch) {
                        self.next_char();
                    }
                    name = Some(self.names.from_chars(&self.buf, namep, self.bp - namep));
                }
            }
        } else if Self::is_identifier_start(self.ch) {
            name = Some(self.read_identifier());
        }

        match name {
            None => self.erroneous("dc.bad.entity", p),
            Some(name) => {
                if self.ch != b';' as u16 {
                    return self.erroneous("dc.missing.semicolon", p);
                }
                self.next_char();
                self.m.borrow_mut().at(p).new_entity_tree(name)
            }
        }
    }

    /// Returns whether this is the end of the preamble of an HTML file.
    fn is_end_preamble(&mut self) -> bool {
        let saved_pos = self.bp;
        let saved_ch = self.ch;
        let result = self.is_end_preamble_inner();
        self.bp = saved_pos;
        self.ch = saved_ch;
        result
    }

    fn is_end_preamble_inner(&mut self) -> bool {
        if self.ch == b'<' as u16 {
            self.next_char();
        }

        if Self::is_identifier_start(self.ch) {
            let identifier = self.read_identifier();
            let mut name = self.lowercase(identifier);
            if name == "body" {
                // Check if also followed by <main>
                // 1. skip rest of <body>
                while self.bp < self.buflen && self.ch != b'>' as u16 {
                    self.next_char();
                }
                if self.ch == b'>' as u16 {
                    self.next_char();
                }
                // 2. skip any whitespace
                while self.bp < self.buflen && self.is_whitespace(self.ch) {
                    self.next_char();
                }
                // 3. check if looking at "<main..."
                if self.ch == b'<' as u16 {
                    self.next_char();
                    if Self::is_identifier_start(self.ch) {
                        let identifier = self.read_identifier();
                        name = self.lowercase(identifier);
                        if name == "main" {
                            return false;
                        }
                    }
                }
                // if <body> is not followed by <main> then this is the end of
                // the preamble
                return true;
            } else if name == "main" {
                // <main> is unconditionally the end of the preamble
                return true;
            }
        }
        false
    }

    /// Returns whether this is the end of the main body of the content in a
    /// standalone HTML file.
    fn is_end_body(&mut self) -> bool {
        let saved_pos = self.bp;
        let saved_ch = self.ch;
        let result = self.is_end_body_inner();
        self.bp = saved_pos;
        self.ch = saved_ch;
        result
    }

    fn is_end_body_inner(&mut self) -> bool {
        if self.ch == b'<' as u16 {
            self.next_char();
        }
        if self.ch == b'/' as u16 {
            self.next_char();
            if Self::is_identifier_start(self.ch) {
                let identifier = self.read_identifier();
                let name = self.lowercase(identifier);
                return name == "body" || name == "main";
            }
        }
        false
    }

    fn peek(&mut self, s: &str) -> bool {
        let saved_pos = self.bp;
        let saved_ch = self.ch;
        let result = self.peek_inner(s);
        self.bp = saved_pos;
        self.ch = saved_ch;
        result
    }

    fn peek_inner(&mut self, s: &str) -> bool {
        let mut s = s.to_string();
        if self.ch == b'<' as u16 {
            self.next_char();
        }
        if self.ch == b'/' as u16 {
            if !s.starts_with('/') {
                return false;
            }
            s = s[1..].to_string();
            self.next_char();
        }
        if Self::is_identifier_start(self.ch) {
            let name = self.read_identifier();
            return self.lowercase(name) == s;
        }
        false
    }

    /// Read the start or end of an HTML tag, or an HTML comment.
    fn html(&mut self) -> Rc<DCTree> {
        let p = self.bp;
        self.next_char();
        if Self::is_identifier_start(self.ch) {
            let name = self.read_identifier();
            let attrs = self.html_attrs();
            let mut self_closing = false;
            if self.ch == b'/' as u16 {
                self.next_char();
                self_closing = true;
            }
            if self.ch == b'>' as u16 {
                self.next_char();
                let tree = self.m.borrow_mut().at(p).new_start_element_tree(
                    name,
                    &attrs.to_vec(),
                    self_closing,
                );
                tree.set_end_pos(self.bp);
                return Rc::clone(&tree);
            }
        } else if self.ch == b'/' as u16 {
            self.next_char();
            if Self::is_identifier_start(self.ch) {
                let name = self.read_identifier();
                self.skip_whitespace();
                if self.ch == b'>' as u16 {
                    self.next_char();
                    let tree = self.m.borrow_mut().at(p).new_end_element_tree(name);
                    tree.set_end_pos(self.bp);
                    return Rc::clone(&tree);
                }
            }
        } else if self.ch == b'!' as u16 {
            self.next_char();
            if self.ch == b'-' as u16 {
                self.next_char();
                if self.ch == b'-' as u16 {
                    self.next_char();
                    while self.bp < self.buflen {
                        let mut dash = 0;
                        while self.bp < self.buflen && self.ch == b'-' as u16 {
                            dash += 1;
                            self.next_char();
                        }
                        // Strictly speaking, a comment should not contain two
                        // consecutive dashes, so dash > 2 is an error and
                        // dash == 2 implies ch is the closing bracket.
                        if dash >= 2 && self.ch == b'>' as u16 {
                            self.next_char();
                            let text = self.new_string(p, self.bp);
                            return self.m.borrow_mut().at(p).new_comment_tree(text);
                        }
                        self.next_char();
                    }
                }
            } else if Self::is_identifier_start(self.ch) && self.peek("doctype") {
                self.read_identifier();
                self.next_char();
                self.skip_whitespace();
                let d = self.bp;
                while self.bp < self.buflen {
                    if self.ch == b'>' as u16 {
                        let mark = self.bp;
                        self.next_char();
                        let text = self.new_string(d, mark);
                        return self.m.borrow_mut().at(d).new_doc_type_tree(text);
                    }
                    self.next_char();
                }
            }
        }

        self.bp = p + 1;
        self.ch = self.buf[self.bp as usize];
        self.erroneous("dc.malformed.html", p)
    }

    /// Read a series of HTML attributes, terminated by the closing bracket.
    fn html_attrs(&mut self) -> List<Rc<DCTree>> {
        let mut attrs = ListBuffer::new();
        self.skip_whitespace();

        'outer: while self.bp < self.buflen && Self::is_identifier_start(self.ch) {
            let name_pos = self.bp;
            let name = self.read_attribute_name();
            self.skip_whitespace();
            let mut value: Option<List<Rc<DCTree>>> = None;
            let mut vkind = ValueKind::EMPTY;
            if self.ch == b'=' as u16 {
                let mut v = ListBuffer::new();
                self.next_char();
                self.skip_whitespace();
                if self.ch == b'\'' as u16 || self.ch == b'"' as u16 {
                    self.newline = false;
                    vkind = if self.ch == b'\'' as u16 {
                        ValueKind::SINGLE
                    } else {
                        ValueKind::DOUBLE
                    };
                    let quote = self.ch;
                    self.next_char();
                    self.text_start = self.bp;
                    while self.bp < self.buflen && self.ch != quote {
                        if self.newline && self.ch == b'@' as u16 {
                            attrs.add(self.erroneous("dc.unterminated.string", name_pos));
                            // No point trying to read more: all attrs get discarded
                            // by the caller and superseded by a malformed.html node.
                            break 'outer;
                        }
                        self.attr_value_char(&mut v);
                    }
                    self.add_pending_text(&mut v, self.bp - 1);
                    self.next_char();
                } else {
                    vkind = ValueKind::UNQUOTED;
                    self.text_start = self.bp;
                    while self.bp < self.buflen
                        && !Self::is_unquoted_attr_value_terminator(self.ch)
                    {
                        self.attr_value_char(&mut v);
                    }
                    self.add_pending_text(&mut v, self.bp - 1);
                }
                self.skip_whitespace();
                value = Some(v.to_list());
            }
            let attr = {
                let mut m = self.m.borrow_mut();
                let value_slice = value.as_ref().map(|list| list.to_vec());
                m.at(name_pos)
                    .new_attribute_tree(name, vkind, value_slice.as_deref())
            };
            attrs.add(attr);
        }

        attrs.to_list()
    }

    fn attr_value_char(&mut self, list: &mut ListBuffer<Rc<DCTree>>) {
        match char::from_u32(u32::from(self.ch)) {
            Some('&') => self.entity_into(list),
            Some('{') => self.inline_tag(list),
            _ => self.next_char(),
        }
    }

    fn add_pending_text(&mut self, list: &mut ListBuffer<Rc<DCTree>>, text_end: i32) {
        if self.text_start != -1 {
            if self.text_start <= text_end {
                let text = self.new_string(self.text_start, text_end + 1);
                let tree = self.m.borrow_mut().at(self.text_start).new_text_tree(text);
                list.add(tree);
            }
            self.text_start = -1;
        }
    }

    /// Java: protected DCErroneous erroneous(String code, int pos)
    fn erroneous(&mut self, code: &str, pos: i32) -> Rc<DCTree> {
        let mut i = self.bp - 1;
        'outer: while i > pos {
            match self.buf[i as usize] {
                0x0c | 0x0a | 0x0d => {
                    self.newline = true;
                }
                0x09 | 0x20 => {}
                _ => break 'outer,
            }
            i -= 1;
        }
        self.text_start = -1;
        let text = self.new_string(pos, i + 1);
        // TODO(tree batch): DocTreeMaker.newErroneousTree(text, diagSource, code)
        // is still unimplemented! (the upstream diagnostic position is the node
        // itself); malformed input panics there. Well-formed comments do not
        // reach this path.
        let diag_source = self.diag_source.clone().unwrap_or_else(|| {
            panic!("erroneous doc tree without a DiagnosticSource (Java: null diagSource)")
        });
        self.m
            .borrow_mut()
            .at(pos)
            .new_erroneous_tree(text, diag_source, code, Vec::new())
    }

    /// Read a Java class name, possibly followed by a member.
    fn reference(&mut self, _allow_member: bool) -> Option<Rc<DCTree>> {
        let pos = self.bp;
        let mut depth = 0;

        // scan to find the end of the signature, by looking for the first
        // whitespace not enclosed in parentheses or angle brackets, or the end
        // of the tag
        'outer: while self.bp < self.buflen {
            match char::from_u32(u32::from(self.ch)) {
                Some('\n') | Some('\r') | Some('\x0c') => {
                    self.newline = true;
                    if depth == 0 {
                        break 'outer;
                    }
                }
                Some(' ') | Some('\t') => {
                    if depth == 0 {
                        break 'outer;
                    }
                }
                Some('(') | Some('<') => {
                    self.newline = false;
                    depth += 1;
                }
                Some(')') | Some('>') => {
                    self.newline = false;
                    depth -= 1;
                }
                Some('}') => {
                    if self.bp == pos {
                        return None;
                    }
                    self.newline = false;
                    break 'outer;
                }
                Some('@') => {
                    if self.newline {
                        break 'outer;
                    }
                    self.newline = false;
                }
                _ => {
                    self.newline = false;
                }
            }
            self.next_char();
        }

        // a negative depth will be caught and reported by ReferenceParser::parse
        if depth > 0 {
            return Some(self.erroneous("dc.unterminated.signature", pos));
        }

        let sig = self.new_string(pos, self.bp);
        match ReferenceParser::new().parse(&sig) {
            Ok(reference) => {
                let tree = self.m.borrow_mut().at(pos).new_reference_tree_with_parts(
                    sig,
                    reference.module_name,
                    reference.qual_expr,
                    reference.member,
                    reference.param_types,
                );
                tree.set_end_pos(self.bp);
                Some(Rc::clone(&tree))
            }
            Err(ReferenceParseException { key }) => Some(self.erroneous(&key, pos)),
        }
    }

    /// Read Java identifier. Matching pairs of braces are skipped.
    fn identifier(&mut self) -> Result<Rc<DCTree>, ParseException> {
        self.skip_whitespace();
        let pos = self.bp;

        if self.is_java_identifier_start(self.ch) {
            let name = self.read_java_identifier();
            return Ok(self.m.borrow_mut().at(pos).new_identifier_tree(name));
        }

        Err(ParseException::new("dc.identifier.expected"))
    }

    /// Read a quoted string.
    fn quoted_string(&mut self) -> Option<Rc<DCTree>> {
        let pos = self.bp;
        self.next_char();

        'outer: while self.bp < self.buflen {
            match char::from_u32(u32::from(self.ch)) {
                Some('\n') | Some('\r') | Some('\x0c') => {
                    self.newline = true;
                }
                Some(' ') | Some('\t') => {}
                Some('"') => {
                    self.next_char();
                    // trim trailing white-space?
                    let text = self.new_string(pos, self.bp);
                    return Some(self.m.borrow_mut().at(pos).new_text_tree(text));
                }
                Some('@') => {
                    if self.newline {
                        break 'outer;
                    }
                }
                _ => {}
            }
            self.next_char();
        }
        None
    }

    /// Read a term (that is, one word).
    fn inline_word(&mut self) -> Option<Rc<DCTree>> {
        let pos = self.bp;
        let mut depth = 0;
        'outer: while self.bp < self.buflen {
            match char::from_u32(u32::from(self.ch)) {
                Some('\n') => {
                    self.newline = true;
                    let text = self.new_string(pos, self.bp);
                    return Some(self.m.borrow_mut().at(pos).new_text_tree(text));
                }
                Some('\r') | Some('\x0c') | Some(' ') | Some('\t') => {
                    let text = self.new_string(pos, self.bp);
                    return Some(self.m.borrow_mut().at(pos).new_text_tree(text));
                }
                Some('@') => {
                    if self.newline {
                        break 'outer;
                    }
                }
                Some('{') => {
                    depth += 1;
                }
                Some('}') => {
                    depth -= 1;
                    if depth == 0 {
                        let text = self.new_string(pos, self.bp);
                        return Some(self.m.borrow_mut().at(pos).new_text_tree(text));
                    }
                }
                _ => {}
            }
            self.newline = false;
            self.next_char();
        }
        None
    }

    fn is_identifier_start(unit: u16) -> bool {
        // Java: Character.isUnicodeIdentifierStart (approximation, GAP: see the
        // file header and parser/mod.rs).
        char::from_u32(u32::from(unit))
            .map(|ch| ch.is_alphabetic() || ch == '_')
            .unwrap_or(false)
    }

    fn is_unicode_identifier_part(unit: u16) -> bool {
        // Java: Character.isUnicodeIdentifierPart (approximation, GAP).
        char::from_u32(u32::from(unit))
            .map(|ch| ch.is_alphanumeric() || ch == '_')
            .unwrap_or(false)
    }

    fn read_identifier(&mut self) -> Name {
        let start = self.bp;
        self.next_char();
        while self.bp < self.buflen && Self::is_unicode_identifier_part(self.ch) {
            self.next_char();
        }
        self.names.from_chars(&self.buf, start, self.bp - start)
    }

    fn read_attribute_name(&mut self) -> Name {
        let start = self.bp;
        self.next_char();
        while self.bp < self.buflen
            && (Self::is_unicode_identifier_part(self.ch) || self.ch == b'-' as u16)
        {
            self.next_char();
        }
        self.names.from_chars(&self.buf, start, self.bp - start)
    }

    fn read_tag_name(&mut self) -> Name {
        let start = self.bp;
        self.next_char();
        while self.bp < self.buflen
            && (Self::is_unicode_identifier_part(self.ch)
                || self.ch == b'.' as u16
                || self.ch == b'-' as u16
                || self.ch == b':' as u16)
        {
            self.next_char();
        }
        self.names.from_chars(&self.buf, start, self.bp - start)
    }

    fn is_java_identifier_start(&self, unit: u16) -> bool {
        // Java: Character.isJavaIdentifierStart (approximation, GAP).
        char::from_u32(u32::from(unit))
            .map(|ch| ch.is_alphabetic() || ch == '_' || ch == '$')
            .unwrap_or(false)
    }

    fn is_java_identifier_part(unit: u16) -> bool {
        // Java: Character.isJavaIdentifierPart (approximation, GAP).
        char::from_u32(u32::from(unit))
            .map(|ch| ch.is_alphanumeric() || ch == '_' || ch == '$')
            .unwrap_or(false)
    }

    fn read_java_identifier(&mut self) -> Name {
        let start = self.bp;
        self.next_char();
        while self.bp < self.buflen && Self::is_java_identifier_part(self.ch) {
            self.next_char();
        }
        self.names.from_chars(&self.buf, start, self.bp - start)
    }

    fn read_system_property_name(&mut self) -> Name {
        let pos = self.bp;
        self.next_char();
        while self.bp < self.buflen
            && (Self::is_unicode_identifier_part(self.ch) || self.ch == b'.' as u16)
        {
            self.next_char();
        }
        self.names.from_chars(&self.buf, pos, self.bp - pos)
    }

    fn is_decimal_digit(&self, unit: u16) -> bool {
        (b'0' as u16) <= unit && unit <= (b'9' as u16)
    }

    fn is_hex_digit(&self, unit: u16) -> bool {
        ((b'0' as u16) <= unit && unit <= (b'9' as u16))
            || ((b'a' as u16) <= unit && unit <= (b'f' as u16))
            || ((b'A' as u16) <= unit && unit <= (b'F' as u16))
    }

    fn is_unquoted_attr_value_terminator(unit: u16) -> bool {
        let space = b' ' as u16;
        let dquote = b'"' as u16;
        let squote = b'\'' as u16;
        let backtick = b'\x60' as u16;
        let equal = b'=' as u16;
        let lt = b'<' as u16;
        let gt = b'>' as u16;
        unit == 0x0c
            || unit == 0x0a
            || unit == 0x0d
            || unit == 0x09
            || unit == space
            || unit == dquote
            || unit == squote
            || unit == backtick
            || unit == equal
            || unit == lt
            || unit == gt
    }

    fn is_whitespace(&self, unit: u16) -> bool {
        // Java: Character.isWhitespace (ported helper, same GAP as the tokenizer).
        is_java_whitespace(unit)
    }

    fn skip_whitespace(&mut self) {
        while self.bp < self.buflen && self.is_whitespace(self.ch) {
            self.next_char();
        }
    }

    /// Java: String newString(int start, int end)
    fn new_string(&self, start: i32, end: i32) -> JavaString {
        JavaString::from_units(self.buf[start as usize..end as usize].to_vec())
    }

    fn lowercase(&self, name: Name) -> String {
        crate::util::string_utils::to_lower_case(&name.to_java_string()).to_string_lossy()
    }

    /// Java: DCTree parse(int pos, Kind kind) dispatch for the anonymous
    /// TagParser subclasses.
    fn parse_tag(
        &mut self,
        spec: TagParserSpec,
        pos: i32,
        kind: TagKind,
    ) -> Result<Rc<DCTree>, ParseException> {
        if kind != spec.kind && spec.kind != TagKind::Either {
            // Java: throw new IllegalArgumentException(kind.toString())
            panic!("illegal TagParser kind {kind:?}");
        }
        match spec.tree_kind {
            Kind::AUTHOR => {
                let name = self.block_content_default();
                let items = name.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_author_tree(&items))
            }
            Kind::CODE => {
                let text = self.inline_text(WhitespaceRetentionPolicy::RemoveFirstSpace)?;
                self.next_char();
                Ok(self.m.borrow_mut().at(pos).new_code_tree(&text))
            }
            Kind::DEPRECATED => {
                let reason = self.block_content_default();
                let items = reason.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_deprecated_tree(&items))
            }
            Kind::DOC_ROOT => {
                if self.ch == b'}' as u16 {
                    self.next_char();
                    return Ok(self.m.borrow_mut().at(pos).new_doc_root_tree());
                }
                self.inline_text(WhitespaceRetentionPolicy::RemoveAll)?;
                self.next_char();
                Err(ParseException::new("dc.unexpected.content"))
            }
            Kind::EXCEPTION => {
                self.skip_whitespace();
                let reference = self.reference_required(false);
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_exception_tree(&reference, &items))
            }
            Kind::HIDDEN => {
                let reason = self.block_content_default();
                let items = reason.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_hidden_tree(&items))
            }
            Kind::INDEX => {
                self.skip_whitespace();
                if self.ch == b'}' as u16 {
                    return Err(ParseException::new("dc.no.content"));
                }
                let term = if self.ch == b'"' as u16 {
                    self.quoted_string()
                } else {
                    self.inline_word()
                };
                let Some(term) = term else {
                    return Err(ParseException::new("dc.no.content"));
                };
                self.skip_whitespace();
                let description = if self.ch != b'}' as u16 {
                    self.inline_content()
                } else {
                    self.next_char();
                    List::nil()
                };
                let items = description.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_index_tree(&term, &items))
            }
            Kind::INHERIT_DOC => {
                if self.ch == b'}' as u16 {
                    self.next_char();
                    return Ok(self.m.borrow_mut().at(pos).new_inherit_doc_tree());
                }
                self.inline_text(WhitespaceRetentionPolicy::RemoveAll)?;
                self.next_char();
                Err(ParseException::new("dc.unexpected.content"))
            }
            Kind::LINK => {
                let reference = self.reference_required(true);
                let label = self.inline_content();
                let items = label.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_link_tree(&reference, &items))
            }
            Kind::LINK_PLAIN => {
                let reference = self.reference_required(true);
                let label = self.inline_content();
                let items = label.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_link_plain_tree(&reference, &items))
            }
            Kind::LITERAL => {
                let text = self.inline_text(WhitespaceRetentionPolicy::RemoveFirstSpace)?;
                self.next_char();
                Ok(self.m.borrow_mut().at(pos).new_literal_tree(&text))
            }
            Kind::PARAM => {
                self.skip_whitespace();
                let mut typaram = false;
                if self.ch == b'<' as u16 {
                    typaram = true;
                    self.next_char();
                }
                let id = self.identifier()?;
                if typaram {
                    if self.ch != b'>' as u16 {
                        return Err(ParseException::new("dc.gt.expected"));
                    }
                    self.next_char();
                }
                self.skip_whitespace();
                let desc = self.block_content_default();
                let items = desc.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_param_tree(typaram, &id, &items))
            }
            Kind::PROVIDES => {
                self.skip_whitespace();
                let reference = self.reference_required(true);
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_provides_tree(&reference, &items))
            }
            Kind::RETURN => {
                let description = match kind {
                    TagKind::Block => self.block_content_default(),
                    TagKind::Inline => self.inline_content(),
                    TagKind::Either => {
                        panic!("illegal TagParser kind Either for @return")
                    }
                };
                let items = description.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_return_tree_with_inline(kind == TagKind::Inline, &items))
            }
            Kind::SEE => {
                self.skip_whitespace();
                match char::from_u32(u32::from(self.ch)) {
                    Some('"') => {
                        let string = self.quoted_string();
                        if let Some(string) = string {
                            self.skip_whitespace();
                            if self.ch == b'@' as u16
                                || (self.ch == u16::from(EOI)
                                    && self.bp == self.buf.len() as i32 - 1)
                            {
                                return Ok(self
                                    .m
                                    .borrow_mut()
                                    .at(pos)
                                    .new_see_tree(&[string]));
                            }
                        }
                    }
                    Some('<') => {
                        let html = self.block_content_default();
                        if !html.is_empty() {
                            return Ok(self
                                .m
                                .borrow_mut()
                                .at(pos)
                                .new_see_tree(&html.to_vec()));
                        }
                    }
                    Some('@') => {
                        if self.newline {
                            return Err(ParseException::new("dc.no.content"));
                        }
                    }
                    _ => {
                        if self.ch == u16::from(EOI) && self.bp == self.buf.len() as i32 - 1 {
                            return Err(ParseException::new("dc.no.content"));
                        }
                        if self.is_java_identifier_start(self.ch) || self.ch == b'#' as u16 {
                            let reference = self.reference_required(true);
                            let description = self.block_content_default();
                            let mut items = description.to_vec();
                            items.insert(0, reference);
                            return Ok(self
                                .m
                                .borrow_mut()
                                .at(pos)
                                .new_see_tree(&items));
                        }
                    }
                }
                Err(ParseException::new("dc.unexpected.content"))
            }
            Kind::SERIAL_DATA => {
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_serial_data_tree(&items))
            }
            Kind::SERIAL_FIELD => {
                self.skip_whitespace();
                let name = self.identifier()?;
                self.skip_whitespace();
                let type_ = self.reference_required(false);
                let description = if self.is_whitespace(self.ch) {
                    self.skip_whitespace();
                    Some(self.block_content_default())
                } else {
                    None
                };
                // Java: a null description list is passed through; the ported
                // factory takes a slice, so a null description becomes empty
                // (registered TODO(tree): null description fields).
                let items = description.map(|list| list.to_vec()).unwrap_or_default();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_serial_field_tree(&name, &type_, &items))
            }
            Kind::SERIAL => {
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_serial_tree(&items))
            }
            Kind::SINCE => {
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_since_tree(&items))
            }
            Kind::SUMMARY => {
                let summary = self.inline_content();
                let items = summary.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_summary_tree(&items))
            }
            Kind::SYSTEM_PROPERTY => {
                self.skip_whitespace();
                if self.ch == b'}' as u16 {
                    return Err(ParseException::new("dc.no.content"));
                }
                let property_name = self.read_system_property_name();
                self.skip_whitespace();
                if self.ch != b'}' as u16 {
                    self.next_char();
                    return Err(ParseException::new("dc.unexpected.content"));
                }
                self.next_char();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_system_property_tree(property_name))
            }
            Kind::THROWS => {
                self.skip_whitespace();
                let reference = self.reference_required(false);
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_throws_tree(&reference, &items))
            }
            Kind::USES => {
                self.skip_whitespace();
                let reference = self.reference_required(true);
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self
                    .m
                    .borrow_mut()
                    .at(pos)
                    .new_uses_tree(&reference, &items))
            }
            Kind::VALUE => {
                let reference = self.reference_required(true);
                self.skip_whitespace();
                if self.ch == b'}' as u16 {
                    self.next_char();
                    return Ok(self.m.borrow_mut().at(pos).new_value_tree(&reference));
                }
                self.next_char();
                Err(ParseException::new("dc.unexpected.content"))
            }
            Kind::VERSION => {
                let description = self.block_content_default();
                let items = description.to_vec();
                Ok(self.m.borrow_mut().at(pos).new_version_tree(&items))
            }
            other => panic!("unexpected tag tree kind {other:?}"),
        }
    }

    fn reference_required(&mut self, allow_member: bool) -> Rc<DCTree> {
        // Java: a null reference is only produced for empty tag content (for
        // example an empty throws tag); the ported DocTreeMaker takes a
        // non-null reference node (registered TODO(tree): null reference fields).
        self.reference(allow_member)
            .expect("null reference node is not representable in the port (TODO)")
    }

    /// Java: private Map<Name, TagParser> createTagParsers()
    fn create_tag_parsers(&self) -> HashMap<Name, TagParserSpec> {
        let specs = [
            // @author name-text
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::AUTHOR,
                retain_white_space: false,
            },
            // @code text
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::CODE,
                retain_white_space: true,
            },
            // @deprecated deprecated-text
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::DEPRECATED,
                retain_white_space: false,
            },
            // @docRoot
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::DOC_ROOT,
                retain_white_space: false,
            },
            // @exception class-name description
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::EXCEPTION,
                retain_white_space: false,
            },
            // @hidden hidden-text
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::HIDDEN,
                retain_white_space: false,
            },
            // @index search-term options-description
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::INDEX,
                retain_white_space: false,
            },
            // @inheritDoc
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::INHERIT_DOC,
                retain_white_space: false,
            },
            // @link package.class#member label
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::LINK,
                retain_white_space: false,
            },
            // @linkplain package.class#member label
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::LINK_PLAIN,
                retain_white_space: false,
            },
            // @literal text
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::LITERAL,
                retain_white_space: true,
            },
            // @param parameter-name description
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::PARAM,
                retain_white_space: false,
            },
            // @provides service-name description
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::PROVIDES,
                retain_white_space: false,
            },
            // @return description or inline return
            TagParserSpec {
                kind: TagKind::Either,
                tree_kind: Kind::RETURN,
                retain_white_space: false,
            },
            // @see reference or quoted-string or HTML
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::SEE,
                retain_white_space: false,
            },
            // @serialData data-description
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::SERIAL_DATA,
                retain_white_space: false,
            },
            // @serialField field-name field-type description
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::SERIAL_FIELD,
                retain_white_space: false,
            },
            // @serial field-description or include or exclude
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::SERIAL,
                retain_white_space: false,
            },
            // @since since-text
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::SINCE,
                retain_white_space: false,
            },
            // @summary summary-text
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::SUMMARY,
                retain_white_space: false,
            },
            // @systemProperty property-name
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::SYSTEM_PROPERTY,
                retain_white_space: false,
            },
            // @throws class-name description
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::THROWS,
                retain_white_space: false,
            },
            // @uses service-name description
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::USES,
                retain_white_space: false,
            },
            // @value package.class#field
            TagParserSpec {
                kind: TagKind::Inline,
                tree_kind: Kind::VALUE,
                retain_white_space: false,
            },
            // @version version-text
            TagParserSpec {
                kind: TagKind::Block,
                tree_kind: Kind::VERSION,
                retain_white_space: false,
            },
        ];
        let mut tag_parsers = HashMap::new();
        for spec in specs {
            let tag_name = spec
                .tree_kind
                .tag_name()
                .unwrap_or_else(|| panic!("doc tree kind without tag name: {:?}", spec.tree_kind));
            tag_parsers.insert(self.names.from_string(&tag_name), spec);
        }
        tag_parsers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokens::CommentStyle;
    use crate::tree::dc_tree::{DCDocComment, DCKind};
    use crate::tree::doc_pretty;
    use crate::util::jc_diagnostic::Factory;
    use crate::util::javac_messages::{DEFAULT_BUNDLE_NAME, JavacMessages};

    struct TestComment {
        text: JavaString,
    }

    impl Comment for TestComment {
        fn get_text(&self) -> Option<JavaString> {
            Some(self.text.clone())
        }

        fn get_source_pos(&self, index: i32) -> i32 {
            index
        }

        fn get_style(&self) -> CommentStyle {
            CommentStyle::JAVADOC
        }

        fn is_deprecated(&self) -> bool {
            false
        }
    }

    struct Env {
        names: Rc<Names>,
        maker: Rc<RefCell<DocTreeMaker>>,
    }

    fn env() -> Env {
        let names = Rc::new(Names::new(false));
        let messages = Rc::new(JavacMessages::new(DEFAULT_BUNDLE_NAME));
        let maker = DocTreeMaker::new(Rc::new(Factory::new(messages, "compiler")));
        Env {
            names,
            maker: Rc::new(RefCell::new(maker)),
        }
    }

    fn parse(text: &str) -> Rc<DCTree> {
        let env = env();
        DocCommentParser::new_for_comment(
            env.names,
            env.maker,
            None,
            Rc::new(TestComment {
                text: JavaString::from(text),
            }),
        )
        .parse()
    }

    fn render(text: &str) -> String {
        doc_pretty::to_java_string(&parse(text)).to_string_lossy()
    }

    fn doc_comment(tree: &DCTree) -> &DCDocComment {
        let DCKind::DocComment(dc) = &tree.kind else {
            panic!("not a doc comment node");
        };
        dc
    }

    #[test]
    fn parses_plain_text_and_first_sentence() {
        let tree = parse("First sentence. Second sentence.");
        assert_eq!(tree.get_kind(), Kind::DOC_COMMENT);
        let dc = doc_comment(&tree);
        assert!(dc.tags.is_empty());
        assert_eq!(
            doc_pretty::to_java_string(dc.first_sentence.head()).to_string_lossy(),
            "First sentence."
        );
        assert_eq!(
            doc_pretty::to_java_string(dc.body.head()).to_string_lossy(),
            "Second sentence."
        );
        assert_eq!(render("Hello world."), "Hello world.");
    }

    #[test]
    fn parses_html_elements_entities_and_comments() {
        assert_eq!(
            render("<p>Hello <b>world</b> &amp; more</p>"),
            "<p>Hello <b>world</b> &amp; more</p>"
        );
        assert_eq!(render("a <br/> b"), "a <br/> b");
        assert_eq!(render("<!-- comment -->x"), "<!-- comment -->x");
    }

    #[test]
    fn parses_inline_tags_and_unknown_tags() {
        assert_eq!(render("{@code x + y}"), "{@code x + y}");
        assert_eq!(render("{@literal <T>}"), "{@literal <T>}");
        assert_eq!(render("{@unknown body}"), "{@unknown body}");
        assert_eq!(render("@unknown block"), "@unknown block");
        assert_eq!(render("{@docRoot}"), "{@docRoot}");
        assert_eq!(render("{@inheritDoc}"), "{@inheritDoc}");
        assert_eq!(render("{@summary short}"), "{@summary short}");
        assert_eq!(
            render("{@systemProperty user.home}"),
            "{@systemProperty user.home}"
        );
    }

    #[test]
    fn parses_block_tags() {
        let text = "@author Alice\n@deprecated old\n@since 1.0\n@version 2.0\n@hidden\n@serial";
        assert_eq!(
            render(text),
            "@author Alice\n@deprecated old\n@since 1.0\n@version 2.0\n@hidden\n@serial"
        );
    }

    #[test]
    fn parses_param_and_return_tags() {
        let text = "@param <T> the type\n@param arg the arg\n@return the result";
        assert_eq!(
            render(text),
            "@param <T> the type\n@param arg the arg\n@return the result"
        );
        assert_eq!(render("{@return the result}"), "{@return the result}");
    }

    #[test]
    fn parses_html_attributes() {
        assert_eq!(
            render("<a href=\"http://example.com\" name=value>text</a>"),
            "<a href=\"http://example.com\" name=value>text</a>"
        );
    }

    #[test]
    fn parses_file_content_phases() {
        // Java: isFileContent parses preamble/body/postamble; a standalone HTML
        // file's preamble ends at body followed by main.
        let env = env();
        let mut parser = DocCommentParser::new(
            env.names,
            env.maker,
            None,
            Rc::new(TestComment {
                text: JavaString::from("<html><body><main>Hello</main></body></html>"),
            }),
            true,
        );
        let tree = parser.parse();
        let dc = doc_comment(&tree);
        assert!(!dc.preamble.is_empty());
        assert!(!dc.postamble.is_empty());
        assert_eq!(
            doc_pretty::to_java_string(dc.preamble.head()).to_string_lossy(),
            "<html>"
        );
    }

    #[test]
    fn nested_braces_in_inline_content_are_counted() {
        assert_eq!(render("{@summary a {b} c}"), "{@summary a {b} c}");
    }

    #[test]
    #[should_panic(expected = "TODO(parser S7)")]
    fn references_need_javac_parser() {
        // ReferenceParser fragment parsing depends on JavacParser (parser S7).
        let _ = parse("{@link java.lang.String}");
    }

    #[test]
    fn tag_parsers_cover_the_upstream_table() {
        let env = env();
        let parser = DocCommentParser::new_for_comment(
            env.names,
            env.maker,
            None,
            Rc::new(TestComment {
                text: JavaString::from("doc"),
            }),
        );
        assert_eq!(parser.tag_parsers.len(), 25);
        let mut kinds: Vec<String> = parser
            .tag_parsers
            .keys()
            .map(|name| name.to_java_string().to_string_lossy())
            .collect();
        kinds.sort();
        assert!(kinds.contains(&"author".to_string()));
        assert!(kinds.contains(&"code".to_string()));
        assert!(kinds.contains(&"exception".to_string()));
        assert!(kinds.contains(&"linkplain".to_string()));
        assert!(kinds.contains(&"systemProperty".to_string()));
        assert!(kinds.contains(&"version".to_string()));
    }
}
