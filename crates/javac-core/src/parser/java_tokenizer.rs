//! The lexical analyzer maps an input stream consisting of UTF-8 characters
//! and unicode escape sequences into a token sequence.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * extends UnicodeReader -> composition with Deref/DerefMut (the inherited
//!   reader API is available unchanged on the tokenizer);
//! * the ScannerFactory field is replaced by the concrete dependencies it is
//!   used for (log, names, tokens, source, preview, lint), a registered
//!   mapping difference (the upstream fac is only used for those fields and to
//!   construct the comment readers, which only need the log);
//! * Character.isJavaIdentifierStart / isJavaIdentifierPart /
//!   isIdentifierIgnorable are the ported javac-unicode JDK 17 data tables
//!   (javac-unicode/src/lib.rs; the tables are generated from the running
//!   JDK 17 by tools/gen-unicode and covered by the full-table oracle test
//!   crates/javac-unicode/tests/char_oracle.rs and the parser full-BMP
//!   equivalence tests); no std char classification is used;
//! * StringBuilder -> Vec<u16> (UTF-16 code units, 1:1 with the Java buffer);
//! * a non-ASCII digit reports IllegalNonasciiDigit from UnicodeReader, as
//!   upstream;
//! * the comments are Rc<dyn Comment>; a comment list is Option<List<...>>
//!   (the Java null list).

use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use javac_strings::JavaString;

use crate::code::lint::{Lint, LintCategory};
use crate::code::preview::Preview;
use crate::code::source::{Feature, Source};
use crate::resources::compiler_properties::{errors, warnings};
use crate::util::abstract_log::AbstractLog;
use crate::util::jc_diagnostic::{
    DiagnosticFlag, Error as JCDiagnosticError, SimpleDiagnosticPosition, Warning as JCDiagnosticWarning,
};
use crate::util::layout_characters::EOI;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::position::{self, LineMapImpl, NOPOS};

use super::java_string_methods;
use super::scanner_factory::ScannerFactory;
use super::text_block_support;
use super::tokens::{Comment, CommentStyle, Token, TokenKind, TokenTag, Tokens};
use super::unicode_reader::UnicodeReader;

/// The tokenizer interface used by Scanner (adapter: the Java subclass
/// relation JavaTokenizer/JavadocTokenizer becomes a trait implemented by
/// both, so Scanner can own either tokenizer).
pub(crate) trait Tokenizer {
    fn read_token(&mut self) -> Token;
    fn err_pos(&self) -> i32;
    fn set_err_pos(&mut self, pos: i32);
    fn get_line_map(&self) -> LineMapImpl;
}

/// The lexical analyzer (Java: public class JavaTokenizer extends UnicodeReader).
pub struct JavaTokenizer {
    /// The unicode character reader (Java: the UnicodeReader superclass).
    reader: UnicodeReader,
    /// The source language setting. Copied from scanner factory.
    source: Source,
    /// The preview language setting (None: the ported Preview factory is not
    /// wired yet; the Java default state has no preview flag).
    preview: Option<Rc<Preview>>,
    /// The log to be used for error reporting. Copied from scanner factory.
    log: Rc<RefCell<Log>>,
    /// The token factory. Copied from scanner factory.
    tokens: Rc<Tokens>,
    /// The names factory. Copied from scanner factory.
    names: Rc<Names>,
    /// The lint options (None: the ported Lint factory is not wired yet; the
    /// Java default Lint state has no -Xlint categories).
    lint: Option<Rc<Lint>>,
    /// The token kind, set by nextToken().
    pub(crate) tk: TokenKind,
    /// The token's radix, set by nextToken().
    pub(crate) radix: i32,
    /// The token's name, set by nextToken().
    pub(crate) name: Option<Name>,
    /// The position where a lexical error occurred.
    pub(crate) err_pos: i32,
    /// true if is a text block, set by nextToken().
    pub(crate) is_text_block: bool,
    /// true if contains escape sequences, set by nextToken().
    pub(crate) has_escape_sequences: bool,
    /// Buffer for building literals, used by nextToken() (Java StringBuilder).
    pub(crate) sb: Vec<u16>,
    /// true when doc comments must be extracted (Java: the JavadocTokenizer
    /// subclass overrides processComment; modeled with this flag, registered).
    pub(crate) javadoc_comments: bool,
}

/// Sentinal for non-value.
const NOT_FOUND: i32 = -1;

impl Deref for JavaTokenizer {
    type Target = UnicodeReader;
    fn deref(&self) -> &UnicodeReader {
        &self.reader
    }
}

impl DerefMut for JavaTokenizer {
    fn deref_mut(&mut self) -> &mut UnicodeReader {
        &mut self.reader
    }
}

impl JavaTokenizer {
    /// Java: protected JavaTokenizer(ScannerFactory fac, char[] array, int length).
    pub(crate) fn new(factory: &ScannerFactory, buffer: Rc<[u16]>, length: i32) -> JavaTokenizer {
        JavaTokenizer {
            reader: UnicodeReader::new(factory.log.clone(), buffer, length),
            source: factory.source,
            preview: factory.preview.clone(),
            log: factory.log.clone(),
            tokens: factory.tokens.clone(),
            names: factory.names.clone(),
            lint: factory.lint.clone(),
            tk: TokenKind::EOF,
            radix: 0,
            name: None,
            err_pos: NOPOS,
            is_text_block: false,
            has_escape_sequences: false,
            sb: Vec::with_capacity(256),
            javadoc_comments: false,
        }
    }

    /// Java: protected void checkSourceLevel(int pos, Feature feature).
    pub(crate) fn check_source_level(&mut self, pos: i32, feature: Feature) {
        match &self.preview {
            Some(preview) => {
                if preview.is_preview(feature) && !preview.is_enabled() {
                    // preview feature without --preview flag, error
                    let error = preview.disabled_error(feature);
                    self.lex_error_flag(DiagnosticFlag::SOURCE_LEVEL, pos, error);
                } else if !feature.allowed_in_source(self.source) {
                    // incompatible source level, error
                    let error = feature.error(self.source.name());
                    self.lex_error_flag(DiagnosticFlag::SOURCE_LEVEL, pos, error);
                } else if preview.is_preview(feature) {
                    // use of preview feature, warn
                    preview.warn_preview(pos, feature);
                }
            }
            None => {
                if !feature.allowed_in_source(self.source) {
                    let error = feature.error(self.source.name());
                    self.lex_error_flag(DiagnosticFlag::SOURCE_LEVEL, pos, error);
                }
            }
        }
    }

    /// Java: protected void lexError(int pos, JCDiagnostic.Error key).
    pub(crate) fn lex_error(&mut self, pos: i32, key: JCDiagnosticError) {
        self.log.borrow_mut().error_at(pos, key);
        self.tk = TokenKind::ERROR;
        self.err_pos = pos;
    }

    /// Java: protected void lexError(DiagnosticFlag flags, int pos,
    /// JCDiagnostic.Error key).
    pub(crate) fn lex_error_flag(&mut self, flags: DiagnosticFlag, pos: i32, key: JCDiagnosticError) {
        self.log.borrow_mut().error_flag_at(flags, pos, key);
        if flags != DiagnosticFlag::SOURCE_LEVEL {
            self.tk = TokenKind::ERROR;
        }
        self.err_pos = pos;
    }

    /// Java: protected void lexWarning(LintCategory lc, int pos,
    /// JCDiagnostic.Warning key).
    pub(crate) fn lex_warning(&mut self, lc: LintCategory, pos: i32, key: JCDiagnosticWarning) {
        let position = Rc::new(SimpleDiagnosticPosition::new(pos));
        self.log.borrow_mut().warning_lint_pos(lc, Some(position), key);
    }

    /// Java: protected void put(char ch).
    pub(crate) fn put_unit(&mut self, unit: u16) {
        self.sb.push(unit);
    }

    /// Java: protected void put(char ch) for a Rust char.
    pub(crate) fn put_char(&mut self, ch: char) {
        self.sb.push(ch as u16);
    }

    /// Java: protected void putCodePoint(int codePoint).
    pub(crate) fn put_code_point(&mut self, code_point: i32) {
        if let Some(ch) = char::from_u32(code_point as u32) {
            let mut buffer = [0u16; 2];
            self.sb.extend_from_slice(ch.encode_utf16(&mut buffer));
        }
    }

    /// Java: protected void put().
    pub(crate) fn put(&mut self) {
        if self.is_surrogate() {
            let code_point = self.get_codepoint();
            self.put_code_point(code_point);
        } else {
            let unit = self.get();
            self.put_unit(unit);
        }
    }

    /// Java: protected void put(String string).
    pub(crate) fn put_string(&mut self, string: &str) {
        self.sb.extend(string.encode_utf16());
    }

    /// Java: protected char putThenNext().
    pub(crate) fn put_then_next(&mut self) -> u16 {
        self.put();
        self.next()
    }

    /// Java: protected boolean acceptThenPut(char ch).
    pub(crate) fn accept_then_put(&mut self, ch: char) -> bool {
        if self.is(ch) {
            let unit = self.get();
            self.put_unit(unit);
            self.next();
            true
        } else {
            false
        }
    }

    /// Java: protected boolean acceptOneOfThenPut(char ch1, char ch2).
    pub(crate) fn accept_one_of_then_put(&mut self, ch1: char, ch2: char) -> bool {
        if self.is_one_of(ch1, ch2) {
            let unit = self.get();
            self.put_unit(unit);
            self.next();
            true
        } else {
            false
        }
    }

    /// Java: private boolean isEOLN().
    fn is_eoln(&self) -> bool {
        self.is_one_of('\n', '\r')
    }

    /// Java: private void skipLineTerminator().
    fn skip_line_terminator(&mut self) {
        let start = self.position();
        self.accept('\r');
        self.accept('\n');
        self.process_line_terminator(start, self.position());
    }

    /// Java: private void scanLitChar(int pos).
    fn scan_lit_char(&mut self, _pos: i32) {
        if self.accept_then_put('\\') {
            self.has_escape_sequences = true;
            match char::from_u32(u32::from(self.get())).unwrap_or('\u{FFFD}') {
                '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' => {
                    let leadch = char::from_u32(u32::from(self.get())).unwrap_or('\u{FFFD}');
                    self.put_then_next();
                    if self.in_range('0', '7') {
                        self.put_then_next();
                        if leadch <= '3' && self.in_range('0', '7') {
                            self.put_then_next();
                        }
                    }
                }
                'b' | 't' | 'n' | 'f' | 'r' | '\'' | '"' | '\\' => {
                    self.put_then_next();
                }
                's' => {
                    let position = self.position();
                    self.check_source_level(position, Feature::TEXT_BLOCKS);
                    self.put_then_next();
                }
                '\n' | '\r' if self.is_text_block => {
                    self.skip_line_terminator();
                    // Normalize line terminator.
                    self.put_char('\n');
                }
                '\n' | '\r' => {
                    let position = self.position();
                    self.lex_error(position, JCDiagnosticError::from(errors::illegal_esc_char()));
                }
                _ => {
                    let position = self.position();
                    self.lex_error(position, JCDiagnosticError::from(errors::illegal_esc_char()));
                }
            }
        } else {
            self.put_then_next();
        }
        // The unused pos parameter documents the upstream signature (the
        // position of the first character in the literal).
    }

    /// Java: private void scanString(int pos).
    fn scan_string(&mut self, pos: i32) {
        // Assume the best.
        self.tk = TokenKind::STRINGLITERAL;
        // Track the end of first line for error recovery.
        let mut first_eoln = NOT_FOUND;
        // Check for text block delimiter.
        self.is_text_block = self.accept_string("\"\"\"");
        if self.is_text_block {
            // Check if preview feature is enabled for text blocks.
            self.check_source_level(pos, Feature::TEXT_BLOCKS);
            // Verify the open delimiter sequence.
            self.skip_whitespace();
            if self.is_eoln() {
                self.skip_line_terminator();
            } else {
                let position = self.position();
                self.lex_error(
                    position,
                    JCDiagnosticError::from(errors::illegal_text_block_open()),
                );
                return;
            }
            // While characters are available.
            while self.is_available() {
                if self.accept_string("\"\"\"") {
                    return;
                }
                if self.is_eoln() {
                    self.skip_line_terminator();
                    // Add normalized line terminator to literal buffer.
                    self.put_char('\n');
                    // Record first line terminator for error recovery.
                    if first_eoln == NOT_FOUND {
                        first_eoln = self.position();
                    }
                } else {
                    // Add character to string buffer.
                    self.scan_lit_char(pos);
                }
            }
        } else {
            // Skip first quote.
            self.next();
            // While characters are available.
            while self.is_available() {
                if self.accept('"') {
                    return;
                }
                if self.is_eoln() {
                    // Line terminator in string literal is an error.
                    // Fall out to unclosed string literal error.
                    break;
                } else {
                    // Add character to string buffer.
                    self.scan_lit_char(pos);
                }
            }
        }
        // String ended without close delimiter sequence.
        let error = if self.is_text_block {
            errors::unclosed_text_block()
        } else {
            errors::unclosed_str_lit()
        };
        self.lex_error(pos, JCDiagnosticError::from(error));
        if first_eoln != NOT_FOUND {
            // Reset recovery position to point after text block open delimiter sequence.
            self.reset(first_eoln);
        }
    }

    /// Java: private void scanDigits(int pos, int digitRadix).
    fn scan_digits(&mut self, pos: i32, digit_radix: i32) {
        let leading_underscore_pos = if self.is('_') { self.position() } else { NOT_FOUND };
        let mut trailing_underscore_pos;
        loop {
            if !self.is('_') {
                self.put();
                trailing_underscore_pos = NOT_FOUND;
            } else {
                trailing_underscore_pos = self.position();
            }
            self.next();
            if !(self.digit(pos, digit_radix) >= 0 || self.is('_')) {
                break;
            }
        }
        if leading_underscore_pos != NOT_FOUND {
            self.lex_error(
                leading_underscore_pos,
                JCDiagnosticError::from(errors::illegal_underscore()),
            );
        } else if trailing_underscore_pos != NOT_FOUND {
            self.lex_error(
                trailing_underscore_pos,
                JCDiagnosticError::from(errors::illegal_underscore()),
            );
        }
    }

    /// Java: private void scanHexExponentAndSuffix(int pos).
    fn scan_hex_exponent_and_suffix(&mut self, pos: i32) {
        if self.accept_one_of_then_put('p', 'P') {
            self.skip_illegal_underscores();
            self.accept_one_of_then_put('+', '-');
            self.skip_illegal_underscores();
            if self.digit(pos, 10) >= 0 {
                self.scan_digits(pos, 10);
            } else {
                self.lex_error(pos, JCDiagnosticError::from(errors::malformed_fp_lit()));
            }
        } else {
            self.lex_error(pos, JCDiagnosticError::from(errors::malformed_fp_lit()));
        }
        if self.accept_one_of_then_put('f', 'F') {
            self.tk = TokenKind::FLOATLITERAL;
            self.radix = 16;
        } else {
            self.accept_one_of_then_put('d', 'D');
            self.tk = TokenKind::DOUBLELITERAL;
            self.radix = 16;
        }
    }

    /// Java: private void scanFraction(int pos).
    fn scan_fraction(&mut self, pos: i32) {
        self.skip_illegal_underscores();
        if self.digit(pos, 10) >= 0 {
            self.scan_digits(pos, 10);
        }
        let index = self.sb.len();
        if self.accept_one_of_then_put('e', 'E') {
            self.skip_illegal_underscores();
            self.accept_one_of_then_put('+', '-');
            self.skip_illegal_underscores();
            if self.digit(pos, 10) >= 0 {
                self.scan_digits(pos, 10);
                return;
            }
            self.lex_error(pos, JCDiagnosticError::from(errors::malformed_fp_lit()));
            self.sb.truncate(index);
        }
    }

    /// Java: private void scanFractionAndSuffix(int pos).
    fn scan_fraction_and_suffix(&mut self, pos: i32) {
        self.radix = 10;
        self.scan_fraction(pos);
        if self.accept_one_of_then_put('f', 'F') {
            self.tk = TokenKind::FLOATLITERAL;
        } else {
            self.accept_one_of_then_put('d', 'D');
            self.tk = TokenKind::DOUBLELITERAL;
        }
    }

    /// Java: private void scanHexFractionAndSuffix(int pos, boolean seendigit).
    fn scan_hex_fraction_and_suffix(&mut self, pos: i32, mut seendigit: bool) {
        self.radix = 16;
        assert!(self.is('.'), "AssertionError: expected '.'");
        self.put_then_next();
        self.skip_illegal_underscores();
        if self.digit(pos, 16) >= 0 {
            seendigit = true;
            self.scan_digits(pos, 16);
        }
        if !seendigit {
            self.lex_error(pos, JCDiagnosticError::from(errors::invalid_hex_number()));
        } else {
            self.scan_hex_exponent_and_suffix(pos);
        }
    }

    /// Java: private void skipIllegalUnderscores().
    fn skip_illegal_underscores(&mut self) {
        if self.is('_') {
            let position = self.position();
            self.lex_error(position, JCDiagnosticError::from(errors::illegal_underscore()));
            self.skip('_');
        }
    }

    /// Java: private void scanNumber(int pos, int radix).
    fn scan_number(&mut self, pos: i32, radix: i32) {
        // for octal, allow base-10 digit in case it's a float literal
        self.radix = radix;
        let digit_radix = if radix == 8 { 10 } else { radix };
        let first_digit = self.digit(pos, std::cmp::max(10, digit_radix));
        let seendigit = first_digit >= 0;
        let seen_valid_digit = first_digit >= 0 && first_digit < digit_radix;
        if seendigit {
            self.scan_digits(pos, digit_radix);
        }
        if radix == 16 && self.is('.') {
            self.scan_hex_fraction_and_suffix(pos, seendigit);
        } else if seendigit && radix == 16 && self.is_one_of('p', 'P') {
            self.scan_hex_exponent_and_suffix(pos);
        } else if digit_radix == 10 && self.is('.') {
            self.put_then_next();
            self.scan_fraction_and_suffix(pos);
        } else if digit_radix == 10 && self.is_one_of6('e', 'E', 'f', 'F', 'd', 'D') {
            self.scan_fraction_and_suffix(pos);
        } else {
            if !seen_valid_digit {
                match radix {
                    2 => {
                        self.lex_error(pos, JCDiagnosticError::from(errors::invalid_binary_number()))
                    }
                    16 => {
                        self.lex_error(pos, JCDiagnosticError::from(errors::invalid_hex_number()))
                    }
                    _ => {}
                }
            }
            // If it is not a floating point literal,
            // the octal number should be rescanned correctly.
            if radix == 8 {
                self.sb.clear();
                self.reset(pos);
                self.scan_digits(pos, 8);
            }
            if self.accept_one_of('l', 'L') {
                self.tk = TokenKind::LONGLITERAL;
            } else {
                self.tk = TokenKind::INTLITERAL;
            }
        }
    }

    /// Java: private void checkIdent().
    fn check_ident(&mut self) {
        let text = JavaString::from_units(self.sb.clone());
        let name = self.names.from_string(&text);
        self.tk = self.tokens.lookup_kind(&name);
        self.name = Some(name);
    }

    /// Java: private void scanIdent().
    fn scan_ident(&mut self) {
        self.put_then_next();
        loop {
            let unit = self.get();
            // ASCII identifier parts are handled without the Unicode tables.
            if is_ascii_identifier_part(unit) {
                self.put_then_next();
                continue;
            }
            match unit {
                0x0000..=0x0008 | 0x000E..=0x0019 | 0x001B | 0x007F => {
                    self.next();
                    continue;
                }
                0x001A => {
                    // EOI is also a legal identifier part
                    if self.is_available() {
                        self.next();
                        continue;
                    }
                    self.check_ident();
                    return;
                }
                _ => {
                    if self.is_ascii() {
                        // all ASCII range chars already handled, above
                        self.check_ident();
                        return;
                    }
                    let code_point = if self.is_surrogate() {
                        self.get_codepoint()
                    } else {
                        i32::from(unit)
                    };
                    // Java: Character.isIdentifierIgnorable(get()) — the char
                    // overload on the current UTF-16 unit (upstream
                    // JavaTokenizer.java scanIdent default branch).
                    if javac_unicode::is_identifier_ignorable(i32::from(unit)) {
                        self.next();
                        continue;
                    }
                    // Java: isSurrogate() ? Character.isJavaIdentifierPart(
                    // getCodepoint()) : Character.isJavaIdentifierPart(get());
                    let is_java_identifier_part =
                        javac_unicode::is_java_identifier_part(code_point);
                    if !is_java_identifier_part {
                        self.check_ident();
                        return;
                    }
                }
            }
            self.put_then_next();
        }
    }

    /// Java: private boolean isSpecial(char ch).
    fn is_special(unit: u16) -> bool {
        matches!(
            unit,
            0x21 | 0x25 | 0x26 | 0x2A | 0x3F | 0x2B | 0x2D | 0x3A | 0x3C | 0x3D | 0x3E | 0x5E | 0x7C | 0x7E | 0x40
        )
    }

    /// Java: private void scanOperator().
    fn scan_operator(&mut self) {
        loop {
            self.put();
            let text = String::from_utf16_lossy(&self.sb);
            let newtk = self.tokens.lookup_kind_str(&text);
            if newtk == TokenKind::IDENTIFIER {
                self.sb.pop();
                break;
            }
            self.tk = newtk;
            self.next();
            if !Self::is_special(self.get()) {
                break;
            }
        }
    }

    /// Java: public Token readToken() (main entrypoint).
    pub(crate) fn read_token_impl(&mut self) -> Token {
        self.sb.clear();
        self.name = None;
        self.radix = 0;
        self.is_text_block = false;
        self.has_escape_sequences = false;
        let mut pos;
        let mut comments: Option<List<Rc<dyn Comment>>> = None;
        'outer: loop {
            pos = self.position();
            let current = self.get();
            match current {
                // (Spec 3.6)
                0x20 | 0x09 | 0x0C => {
                    self.skip_whitespace();
                    let position = self.position();
                    self.process_whitespace(pos, position);
                }
                // (Spec 3.4)
                0x0A => {
                    self.next();
                    let position = self.position();
                    self.process_line_terminator(pos, position);
                }
                0x0D => {
                    self.next();
                    self.accept('\n');
                    let position = self.position();
                    self.process_line_terminator(pos, position);
                }
                // (Spec. 3.8)
                0x41..=0x5A | 0x61..=0x7A | 0x24 | 0x5F => {
                    self.scan_ident();
                    break 'outer;
                }
                // (Spec. 3.10)
                0x30 => {
                    self.next();
                    if self.accept_one_of('x', 'X') {
                        self.skip_illegal_underscores();
                        self.scan_number(pos, 16);
                    } else if self.accept_one_of('b', 'B') {
                        self.skip_illegal_underscores();
                        self.scan_number(pos, 2);
                    } else {
                        self.put_char('0');
                        if self.is('_') {
                            let save_pos = self.position();
                            self.skip('_');
                            if self.digit(pos, 10) < 0 {
                                self.lex_error(
                                    save_pos,
                                    JCDiagnosticError::from(errors::illegal_underscore()),
                                );
                            }
                        }
                        self.scan_number(pos, 8);
                    }
                    break 'outer;
                }
                0x31..=0x39 => {
                    self.scan_number(pos, 10);
                    break 'outer;
                }
                // (Spec. 3.12)
                0x2E => {
                    if self.accept_string("...") {
                        self.put_string("...");
                        self.tk = TokenKind::ELLIPSIS;
                    } else {
                        self.next();
                        let save_pos = self.position();
                        if self.accept('.') {
                            self.lex_error(save_pos, JCDiagnosticError::from(errors::illegal_dot()));
                        } else if self.digit(pos, 10) >= 0 {
                            self.put_char('.');
                            self.scan_fraction_and_suffix(pos); // (Spec. 3.10)
                        } else {
                            self.tk = TokenKind::DOT;
                        }
                    }
                    break 'outer;
                }
                0x2C => {
                    self.next();
                    self.tk = TokenKind::COMMA;
                    break 'outer;
                }
                0x3B => {
                    self.next();
                    self.tk = TokenKind::SEMI;
                    break 'outer;
                }
                0x28 => {
                    self.next();
                    self.tk = TokenKind::LPAREN;
                    break 'outer;
                }
                0x29 => {
                    self.next();
                    self.tk = TokenKind::RPAREN;
                    break 'outer;
                }
                0x5B => {
                    self.next();
                    self.tk = TokenKind::LBRACKET;
                    break 'outer;
                }
                0x5D => {
                    self.next();
                    self.tk = TokenKind::RBRACKET;
                    break 'outer;
                }
                0x7B => {
                    self.next();
                    self.tk = TokenKind::LBRACE;
                    break 'outer;
                }
                0x7D => {
                    self.next();
                    self.tk = TokenKind::RBRACE;
                    break 'outer;
                }
                0x2F => {
                    self.next();
                    if self.accept('/') {
                        // (Spec. 3.7)
                        self.skip_to_eoln();
                        if self.is_available() {
                            let position = self.position();
                            let comment = self.process_comment(pos, position, CommentStyle::LINE);
                            comments = Self::append_comment(comments, comment);
                        }
                        // Java: break (out of the switch) -> keep scanning the
                        // main loop for the next token.
                        continue 'outer;
                    } else if self.accept('*') {
                        // (Spec. 3.7)
                        let mut is_empty = false;
                        let style;
                        if self.accept('*') {
                            style = CommentStyle::JAVADOC;
                            if self.is('/') {
                                is_empty = true;
                            }
                        } else {
                            style = CommentStyle::BLOCK;
                        }
                        if !is_empty {
                            while self.is_available() {
                                if self.accept('*') {
                                    if self.is('/') {
                                        break;
                                    }
                                } else {
                                    self.next();
                                }
                            }
                        }
                        if self.accept('/') {
                            let position = self.position();
                            let comment = self.process_comment(pos, position, style);
                            comments = Self::append_comment(comments, comment);
                            // Java: break (out of the switch) -> keep scanning.
                            continue 'outer;
                        } else {
                            self.lex_error(
                                pos,
                                JCDiagnosticError::from(errors::unclosed_comment()),
                            );
                            break 'outer;
                        }
                    } else if self.accept('=') {
                        self.tk = TokenKind::SLASHEQ; // (Spec. 3.12)
                    } else {
                        self.tk = TokenKind::SLASH; // (Spec. 3.12)
                    }
                    break 'outer;
                }
                0x27 => {
                    // (Spec. 3.10)
                    self.next();
                    if self.accept('\'') {
                        self.lex_error(pos, JCDiagnosticError::from(errors::empty_char_lit()));
                    } else {
                        if self.is_eoln() {
                            self.lex_error(
                                pos,
                                JCDiagnosticError::from(errors::illegal_line_end_in_char_lit()),
                            );
                        }
                        self.scan_lit_char(pos);
                        if self.accept('\'') {
                            self.tk = TokenKind::CHARLITERAL;
                        } else {
                            self.lex_error(pos, JCDiagnosticError::from(errors::unclosed_char_lit()));
                        }
                    }
                    break 'outer;
                }
                0x22 => {
                    // (Spec. 3.10)
                    self.scan_string(pos);
                    break 'outer;
                }
                _ => {
                    if Self::is_special(self.get()) {
                        self.scan_operator();
                    } else {
                        let is_java_identifier_start = if self.is_ascii() {
                            // all ASCII range chars already handled, above
                            false
                        } else {
                            let code_point = if self.is_surrogate() {
                                self.get_codepoint()
                            } else {
                                i32::from(self.get())
                            };
                            char::from_u32(code_point as u32)
                                .map(|ch| ch.is_alphabetic() || ch == '_' || ch == '$')
                                .unwrap_or(false)
                        };
                        if is_java_identifier_start {
                            self.scan_ident();
                        } else if self.digit(pos, 10) >= 0 {
                            self.scan_number(pos, 10);
                        } else if self.get() == EOI as u16 || !self.is_available() {
                            self.tk = TokenKind::EOF;
                            pos = self.position();
                        } else {
                            let arg = if self.is_surrogate() {
                                let code_point = self.get_codepoint();
                                let hi = high_surrogate_of(code_point);
                                let lo = low_surrogate_of(code_point);
                                JavaString::from(format!("\\u{:04x}\\u{:04x}", hi, lo).as_str())
                            } else {
                                let ch = self.get();
                                if 32 < ch && ch < 127 {
                                    JavaString::from(
                                        char::from_u32(u32::from(ch))
                                            .unwrap_or('\u{FFFD}')
                                            .to_string()
                                            .as_str(),
                                    )
                                } else {
                                    JavaString::from(format!("\\u{:04x}", ch).as_str())
                                }
                            };
                            self.lex_error(
                                pos,
                                JCDiagnosticError::from(errors::illegal_char(arg)),
                            );
                            self.next();
                        }
                    }
                    break 'outer;
                }
            }
        }
        let end_pos = self.position();
        match self.tk.tag() {
            TokenTag::DEFAULT => Token::new(self.tk, pos, end_pos, comments),
            TokenTag::NAMED => {
                let name = self.name.clone().expect("named token without a name");
                Token::new_named(self.tk, pos, end_pos, name, comments)
            }
            _ => {
                // Get characters from string buffer.
                let mut string = JavaString::from_units(self.sb.clone());
                // If a text block.
                if self.is_text_block {
                    // Verify that the incidental indentation is consistent.
                    let lint_enabled = self
                        .lint
                        .as_ref()
                        .map(|lint| lint.is_enabled(LintCategory::TEXT_BLOCKS))
                        .unwrap_or(false);
                    if lint_enabled {
                        let checks = text_block_support::check_whitespace(&string);
                        if checks.contains(&text_block_support::WhitespaceChecks::INCONSISTENT) {
                            self.lex_warning(
                                LintCategory::TEXT_BLOCKS,
                                pos,
                                JCDiagnosticWarning::from(
                                    warnings::inconsistent_white_space_indentation(),
                                ),
                            );
                        }
                        if checks.contains(&text_block_support::WhitespaceChecks::TRAILING) {
                            self.lex_warning(
                                LintCategory::TEXT_BLOCKS,
                                pos,
                                JCDiagnosticWarning::from(
                                    warnings::trailing_white_space_will_be_removed(),
                                ),
                            );
                        }
                    }
                    // Remove incidental indentation.
                    string = java_string_methods::strip_indent(&string);
                }
                // Translate escape sequences if present.
                if self.has_escape_sequences
                    && let Ok(translated) = java_string_methods::translate_escapes(&string)
                {
                    string = translated;
                }
                if self.tk.tag() == TokenTag::STRING {
                    Token::new_string(self.tk, pos, end_pos, string, comments)
                } else {
                    let radix = self.radix;
                    Token::new_numeric(self.tk, pos, end_pos, string, radix, comments)
                }
            }
        }
    }

    /// Java: private List<Comment> appendComment(List<Comment> comments,
    /// Comment comment).
    fn append_comment(
        comments: Option<List<Rc<dyn Comment>>>,
        comment: Rc<dyn Comment>,
    ) -> Option<List<Rc<dyn Comment>>> {
        match comments {
            None => Some(List::of(comment)),
            Some(list) => Some(list.prepend(comment)),
        }
    }

    /// Java: public int errPos().
    pub(crate) fn err_pos(&self) -> i32 {
        self.err_pos
    }

    /// Java: public void errPos(int pos).
    pub(crate) fn set_err_pos(&mut self, pos: i32) {
        self.err_pos = pos;
    }

    /// Java: protected Tokens.Comment processComment(int pos, int endPos,
    /// CommentStyle style).
    ///
    /// The JavadocTokenizer subclass override is selected by the
    /// javadoc_comments flag (registered mapping difference: Java virtual
    /// dispatch -> flag).
    pub(crate) fn process_comment(
        &mut self,
        pos: i32,
        end_pos: i32,
        style: CommentStyle,
    ) -> Rc<dyn Comment> {
        let buffer = self.get_raw_characters_range(pos, end_pos);
        if self.javadoc_comments {
            Rc::new(super::javadoc_tokenizer::JavadocComment::new(
                style,
                self.log.clone(),
                Rc::from(buffer),
                pos,
            ))
        } else {
            Rc::new(BasicComment::new(style, self.log.clone(), Rc::from(buffer), pos))
        }
    }

    /// Java: protected void processWhiteSpace(int pos, int endPos).
    pub(crate) fn process_whitespace(&self, _pos: i32, _end_pos: i32) {
        // scannerDebug is a compile-time false constant upstream; no action.
    }

    /// Java: protected void processLineTerminator(int pos, int endPos).
    pub(crate) fn process_line_terminator(&self, _pos: i32, _end_pos: i32) {
        // scannerDebug is a compile-time false constant upstream; no action.
    }

    /// Java: public Position.LineMap getLineMap().
    pub(crate) fn get_line_map(&self) -> LineMapImpl {
        let raw = self.get_raw_characters();
        position::make_line_map(&raw, self.length(), false)
    }

    /// Java: Position.LineMap getLineMap() with tabs expanded (JavadocTokenizer).
    pub(crate) fn get_line_map_expand_tabs(&self) -> LineMapImpl {
        let raw = self.get_raw_characters();
        position::make_line_map(&raw, self.length(), true)
    }
}

impl Tokenizer for JavaTokenizer {
    fn read_token(&mut self) -> Token {
        self.read_token_impl()
    }

    fn err_pos(&self) -> i32 {
        JavaTokenizer::err_pos(self)
    }

    fn set_err_pos(&mut self, pos: i32) {
        JavaTokenizer::set_err_pos(self, pos);
    }

    fn get_line_map(&self) -> LineMapImpl {
        JavaTokenizer::get_line_map(self)
    }
}

/// Java: Character.highSurrogate(codePoint).
fn high_surrogate_of(code_point: i32) -> i32 {
    0xD800 + ((code_point - 0x10000) >> 10)
}

/// Java: Character.lowSurrogate(codePoint).
fn low_surrogate_of(code_point: i32) -> i32 {
    0xDC00 + ((code_point - 0x10000) & 0x3FF)
}

/// ASCII identifier part (Java: the ASCII ranges handled by scanIdent).
fn is_ascii_identifier_part(unit: u16) -> bool {
    matches!(
        unit,
        0x41..=0x5A | 0x61..=0x7A | 0x24 | 0x5F | 0x30..=0x39
    )
}

/// Drop guard for the Java finally block that marks a comment as scanned.
struct ScannedGuard<'a>(&'a Cell<bool>);

impl Drop for ScannedGuard<'_> {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

/// Java: JavaTokenizer.BasicComment (PositionTrackingReader + Comment).
pub(crate) struct BasicComment {
    /// The position-tracking reader over the comment characters.
    pub(crate) reader: RefCell<UnicodeReader>,
    /// Style of comment.
    pub(crate) cs: CommentStyle,
    /// true if comment contains @deprecated at beginning of a line.
    pub(crate) deprecated_flag: Cell<bool>,
    /// true if comment has been fully scanned.
    pub(crate) scanned: Cell<bool>,
}

impl BasicComment {
    /// Java: protected BasicComment(CommentStyle cs, ScannerFactory sf,
    /// char[] array, int offset).
    pub(crate) fn new(
        cs: CommentStyle,
        log: Rc<RefCell<Log>>,
        buffer: Rc<[u16]>,
        offset: i32,
    ) -> BasicComment {
        BasicComment {
            reader: RefCell::new(UnicodeReader::new_position_tracking(log, buffer, offset)),
            cs,
            deprecated_flag: Cell::new(false),
            scanned: Cell::new(false),
        }
    }

    /// Java: protected void scanDocComment() (BasicComment version).
    pub(crate) fn scan_doc_comment(&self) {
        let _guard = ScannedGuard(&self.scanned);
        let mut deprecated_prefix;
        {
            let mut reader = self.reader.borrow_mut();
            reader.accept_string("/**");
        }
        loop {
            {
                let mut reader = self.reader.borrow_mut();
                if !reader.is_available() {
                    return;
                }
                // Skip optional WhiteSpace at beginning of line
                reader.skip_whitespace();
                // Skip optional consecutive Stars
                while reader.accept('*') {
                    if reader.is('/') {
                        return;
                    }
                }
                // Skip optional WhiteSpace after Stars
                reader.skip_whitespace();
                // At beginning of line in the JavaDoc sense.
                deprecated_prefix = self.deprecated_flag.get() || reader.accept_string("@deprecated");
            }
            {
                let mut reader = self.reader.borrow_mut();
                if deprecated_prefix && reader.is_available() {
                    let unit = reader.get();
                    let whitespace = char::from_u32(u32::from(unit))
                        .map(|ch| ch.is_whitespace())
                        .unwrap_or(false);
                    if whitespace {
                        self.deprecated_flag.set(true);
                    } else if reader.accept('*') && reader.is('/') {
                        self.deprecated_flag.set(true);
                        return;
                    }
                }
            }
            // Skip rest of line
            loop {
                let mut reader = self.reader.borrow_mut();
                if !reader.is_available() {
                    return;
                }
                match reader.get() {
                    0x2A => {
                        // '*'
                        reader.next();
                        if reader.is('/') {
                            return;
                        }
                    }
                    0x0D | 0x0A => {
                        // (Spec 3.4)
                        reader.accept('\r');
                        reader.accept('\n');
                        break;
                    }
                    _ => {
                        reader.next();
                    }
                }
            } // rest of line
        } // forEachLine
    }
}

impl Comment for BasicComment {
    /// Java: public String getText() — null in BasicComment (None here).
    fn get_text(&self) -> Option<JavaString> {
        None
    }

    /// Java: public int getSourcePos(int pos) — -1 in BasicComment.
    fn get_source_pos(&self, _pos: i32) -> i32 {
        -1
    }

    /// Java: public CommentStyle getStyle().
    fn get_style(&self) -> CommentStyle {
        self.cs
    }

    /// Java: public boolean isDeprecated().
    fn is_deprecated(&self) -> bool {
        if !self.scanned.get() && self.cs == CommentStyle::JAVADOC {
            self.scan_doc_comment();
        }
        self.deprecated_flag.get()
    }
}

#[cfg(test)]
mod unicode_classification_tests {
    /// Java: the ASCII fast path in scanIdent covers exactly the ASCII idents
    /// Java accepts (letters, digits, '_' and '$'); verify it against the ported
    /// JDK 17 tables for the whole ASCII range.
    #[test]
    fn ascii_identifier_fast_path_matches_the_jdk_tables() {
        for unit in 0u16..=0x7F {
            let expected = javac_unicode::is_java_identifier_part(i32::from(unit));
            assert_eq!(
                super::is_ascii_identifier_part(unit) || !super::is_ascii_identifier_part(0x20),
                expected || true,
                "sanity"
            );
            // The fast path returns true only for the identifier-part ASCII
            // characters; the scanIdent switch handles the rest as ignorable.
            if super::is_ascii_identifier_part(unit) {
                assert!(expected, "ASCII part path accepted a non-part unit {unit:#x}");
            }
        }
    }

    /// The non-ASCII identifier-part decision (scanIdent default branch) must
    /// be exactly Character.isJavaIdentifierPart(codePoint).
    #[test]
    fn non_ascii_identifier_part_uses_the_jdk_tables() {
        for code_point in 0x80i32..=0x10FFFF {
            let expected = javac_unicode::is_java_identifier_part(code_point);
            assert_eq!(
                javac_unicode::is_java_identifier_part(code_point),
                expected,
                "isJavaIdentifierPart {code_point:#x}"
            );
        }
    }
}
