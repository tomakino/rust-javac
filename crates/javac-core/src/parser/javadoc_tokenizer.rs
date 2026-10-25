// Origin: com/sun/tools/javac/parser/JavadocTokenizer.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: dependent service batch (see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: ensure (JavadocTokenizer.java:398)
// End of §19 gap registration.

//! An extension to the base lexical analyzer (JavaTokenizer) that captures and
//! processes the contents of doc comments.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * extends JavaTokenizer -> composition (base: JavaTokenizer; the dispatch of
//!   processComment is the javadoc_comments flag of the base, registered);
//! * JavadocComment extends BasicComment -> a separate struct reusing the same
//!   reader/comment state plus the extraction buffer and the OffsetMap;
//! * the Java DEPRECATED_PATTERN regex
//!   "(?sm).*^\s*@deprecated( |$).*" is expressed with a line scan
//!   (equivalent for the @deprecated detection);
//! * Position.LineMap getLineMap() expands tabs (base get_line_map_expand_tabs).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use javac_strings::JavaString;

use crate::util::log::Log;
use crate::util::position::{LineMapImpl, NOPOS};

use super::java_tokenizer::{JavaTokenizer, Tokenizer};
use super::scanner_factory::ScannerFactory;
use super::tokens::{Comment, CommentStyle, Token};
use super::unicode_reader::UnicodeReader;

/// Java: public class JavadocTokenizer extends JavaTokenizer.
pub struct JavadocTokenizer {
    base: JavaTokenizer,
}

impl JavadocTokenizer {
    /// Java: protected JavadocTokenizer(ScannerFactory fac, char[] array,
    /// int length).
    pub(crate) fn new(
        factory: &ScannerFactory,
        buffer: Rc<[u16]>,
        length: i32,
    ) -> JavadocTokenizer {
        let mut base = JavaTokenizer::new(factory, buffer, length);
        base.javadoc_comments = true;
        JavadocTokenizer { base }
    }

}

impl Tokenizer for JavadocTokenizer {
    fn read_token(&mut self) -> Token {
        self.base.read_token()
    }

    fn err_pos(&self) -> i32 {
        self.base.err_pos()
    }

    fn set_err_pos(&mut self, pos: i32) {
        self.base.set_err_pos(pos);
    }

    fn get_line_map(&self) -> LineMapImpl {
        self.base.get_line_map_expand_tabs()
    }
}

/// Drop guard for the Java finally block that marks a comment as scanned.
struct ScannedGuard<'a>(&'a Cell<bool>);

impl Drop for ScannedGuard<'_> {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

/// Java: JavadocTokenizer.JavadocComment (extends BasicComment).
pub(crate) struct JavadocComment {
    /// The position-tracking reader over the comment characters.
    reader: RefCell<UnicodeReader>,
    /// Style of comment.
    cs: CommentStyle,
    /// true if comment contains @deprecated at beginning of a line.
    deprecated_flag: Cell<bool>,
    /// true if comment has been fully scanned.
    scanned: Cell<bool>,
    /// The relevant portion of the comment (Java: String docComment, null).
    doc_comment: RefCell<Option<JavaString>>,
    /// StringBuilder used to extract the relevant portion.
    sb: RefCell<Vec<u16>>,
    /// Map from the extracted comment offsets to the original source.
    offset_map: RefCell<OffsetMap>,
}

impl JavadocComment {
    /// Java: JavadocComment(CommentStyle cs, ScannerFactory sf, char[] array,
    /// int offset).
    pub(crate) fn new(
        cs: CommentStyle,
        log: Rc<RefCell<Log>>,
        buffer: Rc<[u16]>,
        offset: i32,
    ) -> JavadocComment {
        JavadocComment {
            reader: RefCell::new(UnicodeReader::new_position_tracking(log, buffer, offset)),
            cs,
            deprecated_flag: Cell::new(false),
            scanned: Cell::new(false),
            doc_comment: RefCell::new(None),
            sb: RefCell::new(Vec::new()),
            offset_map: RefCell::new(OffsetMap::new()),
        }
    }

    /// Java: protected void put(char ch) (JavadocComment override).
    fn put_unit(&self, unit: u16) {
        let offset = self.reader.borrow().offset_position();
        let length = self.sb.borrow().len() as i32;
        self.offset_map.borrow_mut().add(length, offset);
        self.sb.borrow_mut().push(unit);
    }

    /// Java: protected void putCodePoint(int codePoint).
    fn put_code_point(&self, code_point: i32) {
        if let Some(ch) = char::from_u32(code_point as u32) {
            let mut buffer = [0u16; 2];
            for unit in ch.encode_utf16(&mut buffer) {
                self.put_unit(*unit);
            }
        }
    }

    /// Java: protected void put().
    fn put(&self) {
        let (surrogate, code_point, unit) = {
            let reader = self.reader.borrow();
            (reader.is_surrogate(), reader.get_codepoint(), reader.get())
        };
        if surrogate {
            self.put_code_point(code_point);
        } else {
            self.put_unit(unit);
        }
    }

    /// Java: public String getText().
    fn get_text_value(&self) -> Option<JavaString> {
        if !self.scanned.get() && self.cs == CommentStyle::JAVADOC {
            self.scan_doc_comment();
        }
        self.doc_comment.borrow().clone()
    }

    /// Java: public int getSourcePos(int pos).
    fn get_source_pos_value(&self, pos: i32) -> i32 {
        if pos == NOPOS {
            return NOPOS;
        }
        let doc_comment = self.doc_comment.borrow();
        let Some(text) = doc_comment.as_ref() else {
            panic!("java.lang.NullPointerException");
        };
        if pos < 0 || pos > text.len() as i32 {
            panic!("java.lang.StringIndexOutOfBoundsException: {}", pos);
        }
        self.offset_map.borrow().get_source_pos(pos)
    }

    /// Java: protected void scanDocComment() (JavadocComment version).
    pub(crate) fn scan_doc_comment(&self) {
        let _guard = ScannedGuard(&self.scanned);
        let mut first_line = true;
        {
            let mut reader = self.reader.borrow_mut();
            // Skip over /*
            reader.accept_string("/*");
            // Consume any number of stars
            reader.skip('*');
            // Is the comment in the form /**/, /***/, /****/, etc. ?
            if reader.is('/') {
                *self.doc_comment.borrow_mut() = Some(JavaString::new());
                return;
            }
            // Skip line terminator on the first line of the comment.
            if reader.is_one_of('\n', '\r') {
                reader.accept('\r');
                reader.accept('\n');
                first_line = false;
            }
        }
        'outer_loop: while self.reader.borrow().is_available() {
            let begin_pos = self.reader.borrow().position();
            {
                let mut reader = self.reader.borrow_mut();
                // Consume whitespace from the beginning of each line.
                reader.skip_whitespace();
                // Are there stars here?  If so, consume them all
                // and check for the end of comment.
                if reader.is('*') {
                    reader.skip('*');
                    if reader.accept('/') {
                        // We're done with the Javadoc comment
                        break 'outer_loop;
                    }
                } else if !first_line {
                    // The current line does not begin with a '*' so we will
                    // treat it as comment
                    reader.reset(begin_pos);
                }
            }
            // The text loop processes the rest of the characters on the line.
            loop {
                let mut reader = self.reader.borrow_mut();
                if !reader.is_available() {
                    break;
                }
                if reader.accept_string("*/") {
                    // This is the end of the comment, return the contents.
                    break 'outer_loop;
                } else if reader.is_one_of('\n', '\r') {
                    // We've seen a newline.  Add it to our buffer.
                    // (put_unit reads the position-tracking reader; release the
                    // mutable borrow first so the RefCell adapter is not re-entered.)
                    drop(reader);
                    self.put_unit('\n' as u16);
                    let mut reader = self.reader.borrow_mut();
                    reader.accept('\r');
                    reader.accept('\n');
                    break;
                } else if reader.is('\u{0C}') {
                    reader.next();
                    break; // treat as end of line
                } else {
                    // Add the character to our buffer.
                    drop(reader);
                    self.put();
                    self.reader.borrow_mut().next();
                }
            }
            first_line = false;
        }
        // If extraction buffer is not empty.
        {
            let mut sb = self.sb.borrow_mut();
            if !sb.is_empty() {
                // Remove trailing asterisks.
                let mut index = sb.len() as i32 - 1;
                while index > -1 && sb[index as usize] == '*' as u16 {
                    index -= 1;
                }
                sb.truncate((index + 1) as usize);
                *self.doc_comment.borrow_mut() = Some(JavaString::from_units(sb.clone()));
            } else {
                *self.doc_comment.borrow_mut() = Some(JavaString::new());
            }
        }
        // Java finally: check if the comment contains the @deprecated tag.
        let deprecated = self
            .doc_comment
            .borrow()
            .as_ref()
            .map(contains_deprecated_tag)
            .unwrap_or(false);
        if deprecated {
            self.deprecated_flag.set(true);
        }
    }
}

impl Comment for JavadocComment {
    fn get_text(&self) -> Option<JavaString> {
        self.get_text_value()
    }

    fn get_source_pos(&self, pos: i32) -> i32 {
        self.get_source_pos_value(pos)
    }

    fn get_style(&self) -> CommentStyle {
        self.cs
    }

    fn is_deprecated(&self) -> bool {
        if !self.scanned.get() && self.cs == CommentStyle::JAVADOC {
            self.scan_doc_comment();
        }
        self.deprecated_flag.get()
    }
}

/// Java: DEPRECATED_PATTERN = (?sm).*^\s*@deprecated( |$).*
fn contains_deprecated_tag(text: &JavaString) -> bool {
    let units = text.as_units();
    let mut line_start = 0usize;
    let mut index = 0usize;
    while index <= units.len() {
        let at_end = index == units.len();
        let unit = if at_end { 0 } else { units[index] };
        if at_end || unit == '\n' as u16 || unit == '\r' as u16 {
            let mut start = line_start;
            while start < index
                && char::from_u32(u32::from(units[start]))
                    .map(|ch| ch.is_whitespace())
                    .unwrap_or(false)
            {
                start += 1;
            }
            let prefix = " @deprecated".as_bytes();
            let tag: Vec<u16> = "@deprecated".encode_utf16().collect();
            if start + tag.len() <= index && units[start..start + tag.len()] == tag[..] {
                let after = start + tag.len();
                if after == index || units[after] == ' ' as u16 {
                    return true;
                }
            }
            let _ = prefix;
            line_start = index + 1;
        }
        index += 1;
    }
    false
}

/// Java: JavadocTokenizer.OffsetMap — maps extracted comment positions back to
/// the original source buffer.
/// Map entry offset for comment offset member of pair.
const SB_OFFSET: usize = 0;
/// Map entry offset of input offset member of pair.
const POS_OFFSET: usize = 1;
/// Number of elements in each entry.
const NOFFSETS: usize = 2;

struct OffsetMap {
    /// Array storing entries in map (pairs flattened).
    map: Vec<i32>,
}

impl OffsetMap {
    fn new() -> OffsetMap {
        OffsetMap { map: Vec::with_capacity(128) }
    }

    /// Java: boolean shouldAdd(int sbOffset, int posOffset).
    fn should_add(&self, sb_offset: i32, pos_offset: i32) -> bool {
        sb_offset - self.last_sb_offset() != pos_offset - self.last_pos_offset()
    }

    /// Java: void add(int sbOffset, int posOffset).
    fn add(&mut self, sb_offset: i32, pos_offset: i32) {
        let size = self.map.len() as i32;
        if size == 0 || self.should_add(sb_offset, pos_offset) {
            self.map.push(sb_offset);
            self.map.push(pos_offset);
        }
    }

    /// Java: private int lastSBOffset().
    fn last_sb_offset(&self) -> i32 {
        if self.map.is_empty() {
            0
        } else {
            let size = self.map.len();
            self.map[size - NOFFSETS + SB_OFFSET]
        }
    }

    /// Java: private int lastPosOffset().
    fn last_pos_offset(&self) -> i32 {
        if self.map.is_empty() {
            0
        } else {
            let size = self.map.len();
            self.map[size - NOFFSETS + POS_OFFSET]
        }
    }

    /// Java: int getSourcePos(int pos) (binary search).
    fn get_source_pos(&self, pos: i32) -> i32 {
        let size = self.map.len() as i32;
        if size == 0 {
            return NOPOS;
        }
        let mut start = 0i32;
        let mut end = size / NOFFSETS as i32;
        while start < end - 1 {
            let index = (start + end) / 2;
            let index_scaled = (index as usize) * NOFFSETS;
            let sb_offset = self.map[index_scaled + SB_OFFSET];
            if sb_offset < pos {
                start = index;
            } else if sb_offset == pos {
                return self.map[index_scaled + POS_OFFSET];
            } else {
                end = index;
            }
        }
        let start_scaled = (start as usize) * NOFFSETS;
        self.map[start_scaled + POS_OFFSET] + (pos - self.map[start_scaled + SB_OFFSET])
    }
}
