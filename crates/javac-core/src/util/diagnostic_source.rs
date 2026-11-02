//! A simple abstraction of a source file, as needed for use in a diagnostic
//! message. Provides access to the line and position in a line for any given
//! character offset.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - javax.tools.JavaFileObject -> Rc<dyn JavaFileObject>; the Java null file
//!   object (NO_SOURCE) maps to no_source(), a stateless instance (a Rust
//!   static cannot hold RefCell state; registered in PORTING_RULES section 9);
//! - char[] -> Vec<u16> (the ported JavaString uses UTF-16 code units);
//!   SoftReference<char[]> -> Rc<Vec<u16>> cache (the reference is kept alive
//!   for the lifetime of the DiagnosticSource; Rust has no soft references and
//!   no memory-pressure-based reclamation, registered section 9);
//! - the CharBuffer fast path of initBuf (JavacFileManager.toArray) cannot be
//!   expressed because java.nio.CharBuffer is not ported; the JavaString units
//!   are copied instead (the buffer content is identical, registered);
//! - the log used for the source.unavailable report is expressed by the
//!   DiagnosticSourceLog trait (see abstract_log.rs for the directError hook of
//!   AbstractLog); DiagnosticSource::new takes an Option of it;
//! - declared IOException -> Result<T, javac_tools::error::Error>, except in the
//!   protected initBuf whose Java IOException is caught by findLine (the Rust
//!   initBuf returns Result and findLine handles the error path).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use javac_tools::error::Error;
use javac_tools::tools::java_file_object::JavaFileObject;

use crate::tree::end_pos_table::EndPosTable;
use crate::util::java_string::JavaString;
use crate::util::layout_characters::{CR, LF, tabulate};
use crate::util::position::NOPOS;

/// The directError hook of AbstractLog that DiagnosticSource needs for
/// reporting a source.unavailable error.
///
/// PORTING_RULES: the Java code holds an AbstractLog reference; the port uses
/// this small trait so diagnostic_source has no dependency on abstract_log
/// (the Log implements it for the concrete log). direct_error takes
/// &self because the Java log owns mutable writers (the implementation is
/// responsible for interior mutability).
pub trait DiagnosticSourceLog {
    /// Java: AbstractLog.directError(String key, Object... args)
    fn direct_error(&self, key: &str, args: Vec<String>);
}

/// A simple abstraction of a source file.
///
/// Java: public class DiagnosticSource
pub struct DiagnosticSource {
    /// The underlying file object (Java: protected JavaFileObject fileObject;
    /// null -> None).
    file_object: Option<Rc<dyn JavaFileObject>>,
    /// The end position table (Java: protected EndPosTable endPosTable).
    end_pos_table: RefCell<Option<Rc<RefCell<dyn EndPosTable>>>>,
    /// A reference to the content of the file object (Java: protected
    /// SoftReference<char[]> refBuf).
    ref_buf: RefCell<Option<Rc<Vec<u16>>>>,
    /// The length of the content (Java: protected int bufLen).
    buf_len: Cell<usize>,
    /// The start of a line found by findLine (Java: protected int lineStart).
    line_start: Cell<usize>,
    /// The line number of a line found by findLine (Java: protected int line).
    line: Cell<i32>,
    /// A log for reporting errors, such as errors accessing the content
    /// (Java: protected AbstractLog log).
    log: Option<Rc<dyn DiagnosticSourceLog>>,
    /// True for the NO_SOURCE constant (the upstream anonymous subclass
    /// overrides findLine to return false).
    missing: bool,
}

impl DiagnosticSource {
    /// Java: public DiagnosticSource(JavaFileObject fo, AbstractLog log)
    pub fn new(
        file_object: Rc<dyn JavaFileObject>,
        log: Option<Rc<dyn DiagnosticSourceLog>>,
    ) -> DiagnosticSource {
        DiagnosticSource {
            file_object: Some(file_object),
            end_pos_table: RefCell::new(None),
            ref_buf: RefCell::new(None),
            buf_len: Cell::new(0),
            line_start: Cell::new(0),
            line: Cell::new(1),
            log,
            missing: false,
        }
    }

    /// The constant DiagnosticSource used when sourcefile is missing
    /// (Java: public static final DiagnosticSource NO_SOURCE).
    ///
    /// PORTING_RULES: a stateless constructor instead of a static instance
    /// (registered section 9).
    pub fn no_source() -> DiagnosticSource {
        DiagnosticSource {
            file_object: None,
            end_pos_table: RefCell::new(None),
            ref_buf: RefCell::new(None),
            buf_len: Cell::new(0),
            line_start: Cell::new(0),
            line: Cell::new(1),
            log: None,
            missing: true,
        }
    }

    /// Return the underlying file object handled by this DiagnosticSource
    /// object (Java: public JavaFileObject getFile(); null -> None).
    pub fn get_file(&self) -> Option<Rc<dyn JavaFileObject>> {
        self.file_object.clone()
    }

    /// Return the one-based line number associated with a given pos for the
    /// current source file. Zero is returned if no line exists for the given
    /// position.
    pub fn get_line_number(&self, pos: i32) -> i32 {
        if self.find_line(pos) {
            return self.line.get();
        }
        0
    }

    /// Return the one-based column number associated with a given pos for the
    /// current source file. Zero is returned if no column exists for the given
    /// position.
    pub fn get_column_number(&self, pos: i32, expand_tabs: bool) -> i32 {
        if !self.find_line(pos) {
            return 0;
        }
        let buf = self
            .ref_buf
            .borrow()
            .clone()
            .expect("findLine initialized the buffer");
        let buf_len = self.buf_len.get();
        let mut column = 0i32;
        let mut bp = self.line_start.get();
        while bp < pos as usize {
            if bp >= buf_len {
                return 0;
            }
            if buf[bp] == '\t' as u16 && expand_tabs {
                column = tabulate(column);
            } else {
                column += 1;
            }
            bp += 1;
        }
        column + 1 // positions are one-based
    }

    /// Return the content of the line containing a given pos (Java returns
    /// String; null -> None; the content is a JavaString because javac text is
    /// UTF-16).
    pub fn get_line(&self, pos: i32) -> Option<JavaString> {
        if !self.find_line(pos) {
            return None;
        }
        let buf = self
            .ref_buf
            .borrow()
            .clone()
            .expect("findLine initialized the buffer");
        let buf_len = self.buf_len.get();
        let mut line_end = self.line_start.get();
        while line_end < buf_len && buf[line_end] != CR as u16 && buf[line_end] != LF as u16 {
            line_end += 1;
        }
        if line_end - self.line_start.get() == 0 {
            return None;
        }
        Some(JavaString::from(
            &buf[self.line_start.get()..line_end],
        ))
    }

    /// Java: public EndPosTable getEndPosTable()
    pub fn get_end_pos_table(&self) -> Option<Rc<RefCell<dyn EndPosTable>>> {
        self.end_pos_table.borrow().clone()
    }

    /// Java: public void setEndPosTable(EndPosTable t)
    pub fn set_end_pos_table(&self, t: Rc<RefCell<dyn EndPosTable>>) {
        let mut slot = self.end_pos_table.borrow_mut();
        if let Some(existing) = slot.as_ref()
            && !Rc::ptr_eq(existing, &t)
        {
            panic!("endPosTable already set");
        }
        *slot = Some(t);
    }

    /// Find the line in the buffer that contains the current position.
    /// @param pos Character offset into the buffer
    pub fn find_line(&self, pos: i32) -> bool {
        if self.missing || pos == NOPOS {
            return false;
        }
        // try and recover buffer from the cache
        let mut buf = self.ref_buf.borrow().clone();
        let rebuild = match &buf {
            None => true,
            Some(_) => self.line_start.get() > pos as usize,
        };
        if rebuild {
            let Some(file_object) = self.file_object.clone() else {
                return false;
            };
            match self.init_buf(file_object.as_ref()) {
                Ok(new_buf) => {
                    self.buf_len.set(new_buf.len());
                    self.ref_buf.replace(Some(Rc::clone(&new_buf)));
                    self.line_start.set(0);
                    self.line.set(1);
                    buf = Some(new_buf);
                }
                Err(_) => {
                    // Java: catch (IOException e) { log.directError("source.unavailable"); buf = new char[0]; return false; }
                    if let Some(log) = &self.log {
                        log.direct_error("source.unavailable", Vec::new());
                    }
                    self.ref_buf.replace(Some(Rc::new(Vec::new())));
                    self.buf_len.set(0);
                    return false;
                }
            }
        }
        let buf = buf.expect("buffer initialized above");
        let buf_len = self.buf_len.get();
        let mut bp = self.line_start.get();
        while bp < buf_len && bp < pos as usize {
            let c = buf[bp];
            bp += 1;
            if c == CR as u16 {
                if bp < buf_len && buf[bp] == LF as u16 {
                    bp += 1;
                }
                self.line.set(self.line.get() + 1);
                self.line_start.set(bp);
            } else if c == LF as u16 {
                self.line.set(self.line.get() + 1);
                self.line_start.set(bp);
            }
        }
        bp <= buf_len
    }

    /// Java: protected char[] initBuf(JavaFileObject fileObject) throws IOException
    pub fn init_buf(
        &self,
        file_object: &dyn JavaFileObject,
    ) -> Result<Rc<Vec<u16>>, Error> {
        let content = file_object.get_char_content(true)?;
        // Java: cs.toString().toCharArray() (the CharBuffer fast path cannot be
        // expressed; registered in the module doc).
        let content = match content {
            Some(content) => content,
            None => panic!("source content is null"),
        };
        Ok(Rc::new(content.as_units().to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::java_string::JavaString;
    use javac_tools::error::Error;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use javac_tools::uri::URI;
    use std::cell::RefCell;
    use std::io::{Read, Write};

    struct InMemoryFileObject {
        content: Option<JavaString>,
        fail: bool,
    }

    impl FileObject for InMemoryFileObject {
        fn to_uri(&self) -> URI {
            URI::create(&JavaString::from("mem:///Test.java"))
        }

        fn get_name(&self) -> JavaString {
            JavaString::from("Test.java")
        }

        fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_char_content(&self, _ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
            if self.fail {
                return Err(Error::io(JavaString::from("cannot read")));
            }
            Ok(self.content.clone())
        }

        fn open_reader(&self, _ignore_encoding_errors: bool) -> Result<Box<dyn javac_tools::java_io::Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn open_writer(&self) -> Result<Box<dyn javac_tools::java_io::Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_last_modified(&self) -> i64 {
            0
        }

        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for InMemoryFileObject {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }

        fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
            kind == Kind::SOURCE
                && simple_name.to_string_lossy() == "Test"
        }

        fn get_nesting_kind(&self) -> Option<javac_tools::lang::model::element::nesting_kind::NestingKind> {
            None
        }

        fn get_access_level(&self) -> Option<javac_tools::lang::model::element::modifier::Modifier> {
            None
        }
    }

    fn file_object(content: &str) -> Rc<dyn JavaFileObject> {
        Rc::new(InMemoryFileObject {
            content: Some(JavaString::from(content)),
            fail: false,
        })
    }

    #[derive(Default)]
    struct RecordingLog {
        errors: RefCell<Vec<String>>,
    }

    impl DiagnosticSourceLog for RecordingLog {
        fn direct_error(&self, key: &str, _args: Vec<String>) {
            self.errors.borrow_mut().push(key.to_string());
        }
    }

    #[test]
    fn line_and_column_numbers() {
        let source = DiagnosticSource::new(file_object("abc\ndef\tX\r\nghi"), None);
        assert_eq!(source.get_line_number(0), 1);
        assert_eq!(source.get_line_number(2), 1);
        assert_eq!(source.get_line_number(3), 1);
        assert_eq!(source.get_line_number(4), 2);
        assert_eq!(source.get_line_number(8), 2);
        assert_eq!(source.get_line_number(9), 2);
        assert_eq!(source.get_line_number(11), 3);
        assert_eq!(source.get_line_number(13), 3);
        assert_eq!(source.get_line_number(NOPOS), 0);
        assert_eq!(source.get_column_number(0, false), 1);
        assert_eq!(source.get_column_number(2, false), 3);
        assert_eq!(source.get_column_number(8, false), 5);
        // LayoutCharacters.tabulate advances to the next tab stop (TabInc = 8):
        // "def" occupies columns 1-3, the tab moves to column 8, and the one-based
        // column of the character after it is 9.
        assert_eq!(source.get_column_number(8, true), 9);
        assert_eq!(source.get_column_number(NOPOS, false), 0);
    }

    #[test]
    fn tab_expansion_in_columns() {
        let source = DiagnosticSource::new(file_object("a\tbc"), None);
        // pos 3 ('c') is preceded by a, tab, b: three columns without tab
        // expansion (one-based 4), and a (1) + tab stop 8 + b (9) with expansion.
        assert_eq!(source.get_column_number(3, false), 4);
        assert_eq!(source.get_column_number(3, true), 10);
    }

    #[test]
    fn line_content_and_blank_lines() {
        let source = DiagnosticSource::new(file_object("a\n\nb"), None);
        assert_eq!(source.get_line(0).map(|s| s.to_string_lossy()), Some("a".to_string()));
        assert_eq!(source.get_line(2), None);
        assert_eq!(source.get_line(3).map(|s| s.to_string_lossy()), Some("b".to_string()));
        assert_eq!(source.get_line(NOPOS), None);
    }

    #[test]
    fn no_source_instance() {
        let source = DiagnosticSource::no_source();
        assert!(source.get_file().is_none());
        assert_eq!(source.get_line_number(0), 0);
        assert_eq!(source.get_column_number(0, false), 0);
        assert_eq!(source.get_line(0), None);
        assert!(!source.find_line(0));
        assert!(source.get_end_pos_table().is_none());
    }

    #[test]
    fn io_error_reports_source_unavailable() {
        let log = Rc::new(RecordingLog::default());
        let failing = Rc::new(InMemoryFileObject {
            content: None,
            fail: true,
        });
        let source = DiagnosticSource::new(failing, Some(log.clone()));
        assert_eq!(source.get_line_number(0), 0);
        assert_eq!(log.errors.borrow().as_slice(), ["source.unavailable"]);
    }

    #[test]
    fn end_pos_table_can_be_set_once() {
        let source = DiagnosticSource::new(file_object("x"), None);
        let table: Rc<RefCell<dyn EndPosTable>> = Rc::new(RefCell::new(MapEndPosTable));
        source.set_end_pos_table(Rc::clone(&table));
        assert!(source.get_end_pos_table().is_some());
        // Setting the same table again is allowed (Java: endPosTable == t).
        source.set_end_pos_table(Rc::clone(&table));
        let other: Rc<RefCell<dyn EndPosTable>> = Rc::new(RefCell::new(MapEndPosTable));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source.set_end_pos_table(other);
        }));
        assert!(result.is_err());
    }

    struct MapEndPosTable;

    impl EndPosTable for MapEndPosTable {
        fn get_end_pos(&self, _tree: crate::ids::TreeId) -> i32 {
            NOPOS
        }

        fn store_end(&mut self, _tree: crate::ids::TreeId, _end_pos: i32) {}

        fn replace_tree(&mut self, _old_tree: crate::ids::TreeId, _new_tree: crate::ids::TreeId) -> i32 {
            NOPOS
        }
    }
}
