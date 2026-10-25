// Origin: com/sun/tools/javac/parser/UnicodeReader.java @ 982d53c (jdk-17.0.19-ga)

//! The unicode character reader used by the javac/javadoc lexer/tokenizer,
//! returns characters one by one as contained in the input stream, handling
//! unicode escape sequences accordingly.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b></p>
//!
//! PORTING_RULES:
//! * the Java char[] buffer becomes Rc<[u16]> (shared with the
//!   JavadocTokenizer position-tracking readers; no borrow lifetime);
//! * the ScannerFactory constructor parameter is narrowed to the log it is used
//!   for (Rc<RefCell<Log>>), registered as a mapping difference;
//! * the Java protected methods stay pub(crate) (parser-internal API);
//! * the nested PositionTrackingReader subclass becomes the optional tracking
//!   state of the same struct (next() updates the column, offsetPosition() uses
//!   the offset) - the observable behavior is kept;
//! * Character.digit covers the ASCII digit/letter subset only (the full
//!   Unicode digit tables are not ported; registered GAP, the Java corpus does
//!   not exercise it); LayoutCharacters EOI/tabulate are the ported ones.

use std::cell::RefCell;
use std::rc::Rc;

use crate::resources::compiler_properties::errors;
use crate::util::abstract_log::AbstractLog;
use crate::util::jc_diagnostic::Error as JCDiagnosticError;
use crate::util::layout_characters::{tabulate, EOI};
use crate::util::log::Log;

/// The unicode character reader (Java: UnicodeReader).
pub struct UnicodeReader {
    /// Buffer containing characters from source file.
    buffer: Rc<[u16]>,
    /// Length of meaningful content in buffer.
    length: i32,
    /// Character buffer index of character currently being observed.
    position: i32,
    /// Number of characters combined to provide the character currently observed.
    width: i32,
    /// Character currently being observed (high member for a surrogate pair).
    character: u16,
    /// Codepoint of the character currently being observed.
    codepoint: i32,
    /// true if the last character was a backslash.
    was_backslash: bool,
    /// true if the last character was derived from an unicode escape sequence.
    was_unicode_escape: bool,
    /// Log for error reporting.
    log: Rc<RefCell<Log>>,
    /// Position-tracking state (Java: the nested PositionTrackingReader subclass).
    tracking: Option<Tracking>,
}

/// Java: UnicodeReader.PositionTrackingReader state.
struct Tracking {
    /// Offset from the beginning of the original reader buffer.
    offset: i32,
    /// Current column in the comment.
    column: i32,
}

enum UnicodeEscapeResult {
    Backslash,
    ValidEscape,
    BrokenEscape,
}

/// Java: Character.isHighSurrogate
pub(crate) fn is_high_surrogate(ch: u16) -> bool {
    (0xD800..=0xDBFF).contains(&ch)
}

/// Java: Character.isLowSurrogate
pub(crate) fn is_low_surrogate(ch: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&ch)
}

/// Java: Character.toCodePoint
pub(crate) fn to_code_point(high: u16, low: u16) -> i32 {
    0x10000 + (((high as i32) - 0xD800) << 10) + ((low as i32) - 0xDC00)
}

impl UnicodeReader {
    /// Java: protected UnicodeReader(ScannerFactory sf, char[] array, int length).
    pub(crate) fn new(log: Rc<RefCell<Log>>, buffer: Rc<[u16]>, length: i32) -> UnicodeReader {
        let mut reader = UnicodeReader {
            buffer,
            length,
            position: 0,
            width: 0,
            character: 0,
            codepoint: 0,
            was_backslash: false,
            was_unicode_escape: false,
            log,
            tracking: None,
        };
        reader.next_code_point();
        reader
    }

    /// Java: protected PositionTrackingReader(ScannerFactory sf, char[] array, int offset).
    pub(crate) fn new_position_tracking(
        log: Rc<RefCell<Log>>,
        buffer: Rc<[u16]>,
        offset: i32,
    ) -> UnicodeReader {
        let length = buffer.len() as i32;
        let mut reader = UnicodeReader {
            buffer,
            length,
            position: 0,
            width: 0,
            character: 0,
            codepoint: 0,
            was_backslash: false,
            was_unicode_escape: false,
            log,
            tracking: Some(Tracking { offset, column: 0 }),
        };
        reader.next_code_point();
        reader
    }

    /// Java: protected int length().
    pub(crate) fn length(&self) -> i32 {
        self.length
    }

    /// Java: protected boolean isAvailable().
    pub(crate) fn is_available(&self) -> bool {
        self.position < self.length
    }

    /// Java: private void nextCodeUnit().
    fn next_code_unit(&mut self) {
        // Index of next character in buffer.
        let index = self.position + self.width;
        if self.length <= index {
            // End of file is marked with EOI.
            self.character = EOI as u16;
        } else {
            self.character = self.buffer[index as usize];
            self.width += 1;
        }
    }

    /// Java: private void nextUnicodeInputCharacter().
    fn next_unicode_input_character(&mut self) {
        self.position += self.width;
        self.width = 0;
        self.next_code_unit();
        if self.character == '\\' as u16 && (!self.was_backslash || self.was_unicode_escape) {
            match self.unicode_escape() {
                UnicodeEscapeResult::Backslash => {
                    self.was_unicode_escape = false;
                    self.was_backslash = !self.was_backslash;
                }
                UnicodeEscapeResult::ValidEscape => {
                    self.was_unicode_escape = true;
                    self.was_backslash = self.character == '\\' as u16 && !self.was_backslash;
                }
                UnicodeEscapeResult::BrokenEscape => self.next_unicode_input_character(),
            }
        } else {
            self.was_backslash = false;
            self.was_unicode_escape = false;
        }
        self.codepoint = i32::from(self.character);
    }

    /// Java: private void nextCodePoint().
    fn next_code_point(&mut self) {
        self.next_unicode_input_character();
        if self.is_ascii() || !is_high_surrogate(self.character) {
            return;
        }
        // Capture high surrogate and position.
        let hi = self.character;
        let save_position = self.position;
        let save_width = self.width;
        // Get potential low surrogate.
        self.next_unicode_input_character();
        let lo = self.character;
        if is_low_surrogate(lo) {
            // Start codepoint at start of high surrogate.
            self.position = save_position;
            self.width += save_width;
            self.codepoint = to_code_point(hi, lo);
        } else {
            // Restore to treat high surrogate as just a character.
            self.position = save_position;
            self.width = save_width;
            self.character = hi;
            self.codepoint = i32::from(hi);
        }
    }

    /// Java: private UnicodeEscapeResult unicodeEscape().
    fn unicode_escape(&mut self) -> UnicodeEscapeResult {
        // Start of unicode escape (past backslash).
        let start = self.position + self.width;
        // Default to backslash result, unless proven otherwise.
        self.character = '\\' as u16;
        self.width = 1;
        // Skip multiple 'u'.
        let mut index = start;
        while index < self.length {
            if self.buffer[index as usize] != 'u' as u16 {
                break;
            }
            index += 1;
        }
        // Needs to have been at least one u.
        if index == start {
            return UnicodeEscapeResult::Backslash;
        }
        let mut code: i32 = 0;
        for _ in 0..4 {
            // Translate and merge digit.
            let digit = if index < self.length {
                char::from_u32(u32::from(self.buffer[index as usize]))
                    .and_then(|ch| ch.to_digit(16))
                    .map(|digit| digit as i32)
                    .unwrap_or(-1)
            } else {
                -1
            };
            code = (code << 4) | digit;
            // If invalid digit.
            if code < 0 {
                break;
            }
            // On to next character.
            index += 1;
        }
        // Skip digits even if error.
        self.width = index - self.position;
        if code >= 0 {
            self.character = code as u16;
            UnicodeEscapeResult::ValidEscape
        } else {
            self.log.borrow_mut().error_at(
                index,
                JCDiagnosticError::from(errors::illegal_unicode_esc()),
            );
            UnicodeEscapeResult::BrokenEscape
        }
    }

    /// Java: protected int position().
    pub(crate) fn position(&self) -> i32 {
        self.position
    }

    /// Java: protected void reset(int pos).
    pub(crate) fn reset(&mut self, pos: i32) {
        self.position = pos;
        self.width = 0;
        self.was_backslash = false;
        self.was_unicode_escape = false;
        self.next_code_point();
    }

    /// Java: protected char get().
    pub(crate) fn get(&self) -> u16 {
        self.character
    }

    /// Java: protected int getCodepoint().
    pub(crate) fn get_codepoint(&self) -> i32 {
        self.codepoint
    }

    /// Java: protected boolean isSurrogate().
    pub(crate) fn is_surrogate(&self) -> bool {
        0xFFFF < self.codepoint
    }

    /// Java: protected boolean isASCII().
    pub(crate) fn is_ascii(&self) -> bool {
        self.character <= 0x7F
    }

    /// Java: protected char next() (the PositionTrackingReader override folds
    /// the column update into this method).
    pub(crate) fn next(&mut self) -> u16 {
        self.next_code_point();
        if let Some(tracking) = &mut self.tracking {
            let character = self.character;
            if character == '\n' as u16 || character == '\r' as u16 || character == '\u{0C}' as u16 {
                tracking.column = 0;
            } else if character == '\t' as u16 {
                tracking.column = tabulate(tracking.column);
            } else {
                tracking.column += 1;
            }
        }
        self.character
    }

    /// Java: protected boolean is(char ch).
    pub(crate) fn is(&self, ch: char) -> bool {
        let mut buffer = [0u16; 2];
        let units = ch.encode_utf16(&mut buffer);
        if units.len() != 1 {
            return false;
        }
        self.character == units[0]
    }

    /// Java: protected boolean isOneOf(char ch1, char ch2).
    pub(crate) fn is_one_of(&self, ch1: char, ch2: char) -> bool {
        self.is(ch1) || self.is(ch2)
    }

    /// Java: protected boolean isOneOf(char ch1, char ch2, char ch3).
    pub(crate) fn is_one_of3(&self, ch1: char, ch2: char, ch3: char) -> bool {
        self.is(ch1) || self.is(ch2) || self.is(ch3)
    }

    /// Java: protected boolean isOneOf(char ch1, char ch2, char ch3, char ch4,
    /// char ch5, char ch6).
    pub(crate) fn is_one_of6(
        &self,
        ch1: char,
        ch2: char,
        ch3: char,
        ch4: char,
        ch5: char,
        ch6: char,
    ) -> bool {
        self.is(ch1) || self.is(ch2) || self.is(ch3) || self.is(ch4) || self.is(ch5) || self.is(ch6)
    }

    /// Java: protected boolean inRange(char lo, char hi).
    pub(crate) fn in_range(&self, lo: char, hi: char) -> bool {
        let mut low = [0u16; 2];
        let mut high = [0u16; 2];
        let low = lo.encode_utf16(&mut low)[0];
        let high = hi.encode_utf16(&mut high)[0];
        low <= self.character && self.character <= high
    }

    /// Java: protected boolean accept(char ch).
    pub(crate) fn accept(&mut self, ch: char) -> bool {
        if self.is(ch) {
            self.next();
            true
        } else {
            false
        }
    }

    /// Java: protected boolean acceptOneOf(char ch1, char ch2).
    pub(crate) fn accept_one_of(&mut self, ch1: char, ch2: char) -> bool {
        if self.is_one_of(ch1, ch2) {
            self.next();
            true
        } else {
            false
        }
    }

    /// Java: protected boolean acceptOneOf(char ch1, char ch2, char ch3).
    pub(crate) fn accept_one_of3(&mut self, ch1: char, ch2: char, ch3: char) -> bool {
        if self.is_one_of3(ch1, ch2, ch3) {
            self.next();
            true
        } else {
            false
        }
    }

    /// Java: protected void skip(char ch).
    pub(crate) fn skip(&mut self, ch: char) {
        while self.accept(ch) {}
    }

    /// Java: protected void skipWhitespace().
    pub(crate) fn skip_whitespace(&mut self) {
        while self.accept_one_of3(' ', '\t', '\u{0C}') {}
    }

    /// Java: protected void skipToEOLN().
    pub(crate) fn skip_to_eoln(&mut self) {
        while self.is_available() {
            if self.is_one_of('\r', '\n') {
                break;
            }
            self.next();
        }
    }

    /// Java: protected boolean accept(String string).
    pub(crate) fn accept_string(&mut self, string: &str) -> bool {
        let units: Vec<u16> = string.encode_utf16().collect();
        if units.is_empty() || self.character != units[0] {
            return false;
        }
        let saved_position = self.position;
        self.next_code_point();
        for unit in &units[1..] {
            if self.character != *unit {
                self.reset(saved_position);
                return false;
            }
            self.next_code_point();
        }
        true
    }

    /// Java: protected int digit(int pos, int digitRadix).
    pub(crate) fn digit(&mut self, _pos: i32, digit_radix: i32) -> i32 {
        // Just an ASCII digit.
        if self.in_range('0', '9') {
            let result = i32::from(self.character) - '0' as i32;
            return if result < digit_radix { result } else { -1 };
        }
        // Handle other digits (the full Unicode digit tables are not ported).
        let code_point = if self.is_surrogate() {
            self.codepoint as u32
        } else {
            u32::from(self.character)
        };
        let result = char::from_u32(code_point)
            .and_then(|ch| ch.to_digit(digit_radix as u32))
            .map(|digit| digit as i32)
            .unwrap_or(-1);
        if result >= 0 && !self.is_ascii() {
            self.log.borrow_mut().error_at(
                self.position(),
                JCDiagnosticError::from(errors::illegal_nonascii_digit()),
            );
            self.character = "0123456789abcdef".as_bytes()[result as usize] as u16;
        }
        result
    }

    /// Java: public char[] getRawCharacters().
    pub(crate) fn get_raw_characters(&self) -> Vec<u16> {
        self.buffer[..self.length as usize].to_vec()
    }

    /// Java: public char[] getRawCharacters(int beginIndex, int endIndex).
    pub(crate) fn get_raw_characters_range(&self, begin_index: i32, end_index: i32) -> Vec<u16> {
        if begin_index < 0 || end_index > self.length || begin_index > end_index {
            panic!("java.lang.ArrayIndexOutOfBoundsException");
        }
        self.buffer[begin_index as usize..end_index as usize].to_vec()
    }

    /// Java: PositionTrackingReader.column().
    ///
    /// PORTING_RULES: the method is kept 1:1 but is only exercised by the
    /// UnicodeReader tests until the Javadoc position-tracking wiring uses it.
    #[cfg(test)]
    pub(crate) fn column(&self) -> i32 {
        self.tracking.as_ref().map(|tracking| tracking.column).unwrap_or(0)
    }

    /// Java: PositionTrackingReader.offsetPosition().
    pub(crate) fn offset_position(&self) -> i32 {
        self.position() + self.tracking.as_ref().map(|tracking| tracking.offset).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_high_surrogate, is_low_surrogate, to_code_point, UnicodeReader};
    use crate::util::abstract_log::AbstractLog;
    use crate::util::context::Context;
    use crate::util::log::Log;
    use javac_strings::JavaString;
    use javac_tools::tools::java_file_object::Kind;
    use javac_tools::tools::simple_java_file_object::SimpleJavaFileObject;
    use javac_tools::uri::URI;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn log() -> Rc<RefCell<Log>> {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let dummy = Rc::new(SimpleJavaFileObject::new(
            URI::create(&JavaString::from("file:///Dummy.java")),
            Kind::SOURCE,
        ));
        log.borrow_mut().use_source(Some(dummy));
        log
    }

    fn make_reader(text: &str) -> UnicodeReader {
        let units: Vec<u16> = text.encode_utf16().collect();
        let length = units.len() as i32;
        UnicodeReader::new(log(), Rc::from(units), length)
    }

    fn make_position_reader(text: &str, offset: i32) -> UnicodeReader {
        let units: Vec<u16> = text.encode_utf16().collect();
        UnicodeReader::new_position_tracking(log(), Rc::from(units), offset)
    }

    #[test]
    fn reads_ascii_units_and_eoi() {
        let mut reader = make_reader("ab");
        assert_eq!(reader.length(), 2);
        assert_eq!(reader.get(), 'a' as u16);
        assert_eq!(reader.position(), 0);
        assert!(reader.is_available());
        reader.next();
        assert_eq!(reader.get(), 'b' as u16);
        reader.next();
        assert_eq!(reader.get(), crate::util::layout_characters::EOI as u16);
        assert!(!reader.is_available());
        assert_eq!(reader.get_raw_characters(), vec!['a' as u16, 'b' as u16]);
        assert_eq!(reader.get_raw_characters_range(1, 2), vec!['b' as u16]);
    }

    #[test]
    fn translates_unicode_escapes() {
        let mut reader = make_reader("a\\u0042c");
        assert_eq!(reader.get(), 'a' as u16);
        reader.next();
        assert_eq!(reader.get(), 'B' as u16);
        assert_eq!(reader.position(), 1);
        reader.next();
        assert_eq!(reader.get(), 'c' as u16);
        // Multiple u's are allowed.
        let reader = make_reader("\\uuu0041");
        assert_eq!(reader.get(), 'A' as u16);
        // A backslash that does not start an escape is kept.
        let mut reader = make_reader("\\\\u0041");
        assert_eq!(reader.get(), '\\' as u16);
        reader.next();
        assert_eq!(reader.get(), '\\' as u16);
    }

    #[test]
    fn accepts_strings_and_resets() {
        let mut reader = make_reader("....x");
        assert!(reader.accept_string("..."));
        assert_eq!(reader.get(), '.' as u16);
        let position = reader.position();
        reader.reset(0);
        assert_eq!(reader.get(), '.' as u16);
        assert_ne!(position, reader.position());
        assert!(reader.accept('.'));
        assert!(reader.accept_one_of('.', ','));
        assert!(reader.accept_one_of3('.', ',', ';'));
        assert!(reader.accept('.'));
        assert!(!reader.accept('.'));
        assert_eq!(reader.get(), 'x' as u16);
        assert!(reader.is_one_of('x', 'y'));
        assert!(reader.is_one_of3('x', 'y', 'z'));
        assert!(reader.is_one_of6('x', 'y', 'z', 'w', 'v', 'u'));
        assert!(reader.in_range('a', 'z'));
    }

    #[test]
    fn digits_use_the_given_radix() {
        let mut reader = make_reader("7");
        assert_eq!(reader.digit(0, 10), 7);
        assert_eq!(reader.digit(0, 8), 7);
        let mut reader = make_reader("8");
        assert_eq!(reader.digit(0, 8), -1);
        let mut reader = make_reader("a");
        assert_eq!(reader.digit(0, 16), 10);
        let mut reader = make_reader("g");
        assert_eq!(reader.digit(0, 16), -1);
    }

    #[test]
    fn tracks_columns_and_offsets() {
        let mut reader = make_position_reader("ab\tc", 100);
        assert_eq!(reader.column(), 0);
        assert_eq!(reader.offset_position(), 100);
        reader.next();
        assert_eq!(reader.column(), 1);
        assert_eq!(reader.offset_position(), 101);
        reader.next();
        assert_eq!(reader.column(), 8);
        assert_eq!(reader.offset_position(), 102);
        reader.next();
        assert_eq!(reader.get(), 'c' as u16);
        assert_eq!(reader.column(), 9);
        assert_eq!(reader.offset_position(), 103);
    }

    #[test]
    fn surrogate_helpers_match_the_jdk() {
        assert!(is_high_surrogate(0xD800));
        assert!(is_high_surrogate(0xDBFF));
        assert!(!is_high_surrogate(0xDC00));
        assert!(is_low_surrogate(0xDC00));
        assert!(is_low_surrogate(0xDFFF));
        assert!(!is_low_surrogate(0xD800));
        assert_eq!(to_code_point(0xD83D, 0xDE00), 0x1F600);
        let reader = make_reader("\u{1F600}");
        assert!(reader.is_surrogate());
        assert_eq!(reader.get_codepoint(), 0x1F600);
        // Java keeps the low surrogate in the character field once a pair is
        // combined (the codepoint carries the full value).
        assert_eq!(reader.get(), 0xDE00);
    }
}
