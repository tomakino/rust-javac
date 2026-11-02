// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.charset Charset/CharsetDecoder/CharsetEncoder (the
// subset used by com.sun.tools.javac.file.BaseFileManager and PathFileObject).
// The Java decoder/encoder state machines are replaced by whole-input
// decode/encode operations; the observable conversions, the REPORT/REPLACE
// semantics, the replacement characters and the exception roles
// (MalformedInputException / UnmappableCharacterException with
// getInputLength()) are reproduced from the recorded JDK 17.0.19 oracle.
//
//! Charset: names/aliases plus Java-compatible decode/encode.

use crate::error::{CharacterCodingError, CharsetError};
use std::option::Option as StdOption;

use javac_strings::JavaString;

/// Java: java.nio.charset.CodingErrorAction (the subset javac uses).
///
/// BaseFileManager selects REPLACE when ignoreEncodingErrors is
/// true and REPORT otherwise; the port keeps the two actions (IGNORE is not
/// needed by javac and is not modeled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MalformedAction {
    /// Java: CodingErrorAction.REPORT.
    Report,
    /// Java: CodingErrorAction.REPLACE.
    Replace,
}

/// The supported charsets (the JDK names are the canonical Java names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    Utf8,
    UsAscii,
    Iso8859_1,
    Utf16,
    Utf16Be,
    Utf16Le,
}

/// The recorded JDK aliases, in java.nio.charset order.
const SUPPORTED: &[(Charset, &str, &[&str])] = &[
    (
        Charset::Utf8,
        "UTF-8",
        &["unicode-1-1-utf-8", "UTF8"],
    ),
    (
        Charset::UsAscii,
        "US-ASCII",
        &[
            "ANSI_X3.4-1968",
            "cp367",
            "csASCII",
            "iso-ir-6",
            "ASCII",
            "iso_646.irv:1983",
            "ANSI_X3.4-1986",
            "ascii7",
            "default",
            "ISO_646.irv:1991",
            "ISO646-US",
            "IBM367",
            "646",
            "us",
        ],
    ),
    (
        Charset::Iso8859_1,
        "ISO-8859-1",
        &[
            "819",
            "ISO8859-1",
            "l1",
            "ISO_8859-1:1987",
            "ISO_8859-1",
            "8859_1",
            "iso-ir-100",
            "latin1",
            "cp819",
            "ISO8859_1",
            "IBM819",
            "ISO_8859_1",
            "IBM-819",
            "csISOLatin1",
        ],
    ),
    (
        Charset::Utf16,
        "UTF-16",
        &["UTF_16", "unicode", "utf16", "UnicodeBig"],
    ),
    (
        Charset::Utf16Be,
        "UTF-16BE",
        &[
            "X-UTF-16BE",
            "UTF_16BE",
            "ISO-10646-UCS-2",
            "UnicodeBigUnmarked",
        ],
    ),
    (
        Charset::Utf16Le,
        "UTF-16LE",
        &["UnicodeLittleUnmarked", "UTF_16LE", "X-UTF-16LE"],
    ),
];

impl Charset {
    /// Java: Charset.forName(String) — see the name/alias table recorded from
    /// the baseline JDK.  Invalid names report IllegalCharsetNameException and
    /// names that are legal but not implemented report
    /// UnsupportedCharsetException (e.g. GBK: no silent substitution).
    pub fn for_name(name: &JavaString) -> Result<Charset, CharsetError> {
        let name = name.to_string_lossy();
        if !is_legal_charset_name(&name) {
            return Err(CharsetError::IllegalCharsetName { name });
        }
        for (charset, canonical, aliases) in SUPPORTED {
            if name.eq_ignore_ascii_case(canonical)
                || aliases.iter().any(|alias| name.eq_ignore_ascii_case(alias))
            {
                return Ok(*charset);
            }
        }
        Err(CharsetError::UnsupportedCharset { name })
    }

    /// Java: Charset.name().
    pub fn name(&self) -> JavaString {
        JavaString::from(self.canonical_name())
    }

    /// Java: Charset.aliases().
    pub fn aliases(&self) -> Vec<JavaString> {
        self.alias_names()
            .iter()
            .map(|alias| JavaString::from(*alias))
            .collect()
    }

    /// Java: CharsetDecoder.decode(ByteBuffer) with onMalformedInput(action)
    /// (whole input, end of input true).
    ///
    /// REPORT reports MalformedInputException; REPLACE substitutes U+FFFD using
    /// the recorded Java replacement boundaries and continues.
    pub fn decode(
        &self,
        bytes: &[u8],
        action: MalformedAction,
    ) -> Result<JavaString, CharacterCodingError> {
        match self {
            Charset::Utf8 => decode_utf8(bytes, action),
            Charset::UsAscii => decode_ascii(bytes, action),
            Charset::Iso8859_1 => {
                Ok(JavaString::from_units(bytes.iter().map(|byte| u16::from(*byte)).collect()))
            }
            Charset::Utf16 => decode_utf16(bytes, Utf16Endian::AutoBom, action),
            Charset::Utf16Be => decode_utf16(bytes, Utf16Endian::Big, action),
            Charset::Utf16Le => decode_utf16(bytes, Utf16Endian::Little, action),
        }
    }

    /// Java: CharsetEncoder.encode(CharBuffer) with onMalformedInput / onUnmappableCharacter
    /// actions (whole input).
    ///
    /// The replacement bytes are the recorded Java ones: '?' for
    /// UTF-8/US-ASCII/ISO-8859-1 and U+FFFD for the UTF-16 family.  UTF-16
    /// (with BOM) writes the big-endian BOM when the input is not empty.
    pub fn encode(
        &self,
        text: &JavaString,
        action: MalformedAction,
    ) -> Result<Vec<u8>, CharacterCodingError> {
        match self {
            Charset::Utf8 => encode_utf8(text.as_units(), action),
            Charset::UsAscii => encode_single_byte(text.as_units(), 0x7F, action),
            Charset::Iso8859_1 => encode_single_byte(text.as_units(), 0xFF, action),
            Charset::Utf16 => encode_utf16(text.as_units(), Utf16Endian::Big, true, action),
            Charset::Utf16Be => encode_utf16(text.as_units(), Utf16Endian::Big, false, action),
            Charset::Utf16Le => encode_utf16(text.as_units(), Utf16Endian::Little, false, action),
        }
    }

    fn canonical_name(&self) -> &'static str {
        SUPPORTED
            .iter()
            .find(|(charset, _, _)| charset == self)
            .map(|(_, canonical, _)| *canonical)
            .expect("all charsets are in the table")
    }

    fn alias_names(&self) -> &'static [&'static str] {
        SUPPORTED
            .iter()
            .find(|(charset, _, _)| charset == self)
            .map(|(_, _, aliases)| *aliases)
            .expect("all charsets are in the table")
    }
/// Java: Charset.newDecoder() — the incremental decoder of this charset.
///
/// # Arguments
///   - none.
///
/// # Returns
///   - a CharsetDecoder in the REPORT state (callers select the action).
///
/// # Panics
///   - none.
pub fn new_decoder(self) -> crate::coder::CharsetDecoder {
    crate::coder::CharsetDecoder::new(self)
}

/// Java: Charset.newEncoder() — the incremental encoder of this charset.
///
/// # Arguments
///   - none.
///
/// # Returns
///   - a CharsetEncoder in the REPORT state (callers select the action).
///
/// # Panics
///   - none.
pub fn new_encoder(self) -> crate::coder::CharsetEncoder {
    crate::coder::CharsetEncoder::new(self)
}

/// Java: Charset.defaultCharset() — the platform default charset adapter.
///
/// the JVM default comes from the launcher's file.encoding /
/// platform locale; the port takes the launcher-injected
/// JAVAC_DEFAULT_ENCODING variable first (registered adapter), then the POSIX
/// locale (LC_ALL/LC_CTYPE/LANG), and falls back to UTF-8 (the recorded
/// JDK 17 oracle on the development host is UTF-8: file.encoding=UTF-8,
/// native.encoding=UTF-8, sun.jnu.encoding=UTF-8).
///
/// # Arguments
///   - none.
///
/// # Returns
///   - the default charset.
///
/// # Panics
///   - none.
pub fn default_charset_name() -> JavaString {
    if let Ok(name) = std::env::var("JAVAC_DEFAULT_ENCODING") {
        return JavaString::from(name.as_str());
    }
    for key in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            let upper = value.to_ascii_uppercase();
            if upper.contains("UTF-8") || upper.contains("UTF8") {
                return JavaString::from("UTF-8");
            }
            if upper.contains("ISO-8859-1") || upper.contains("LATIN1") {
                return JavaString::from("ISO-8859-1");
            }
        }
    }
    #[cfg(windows)]
    if let Ok(acp) = Self::windows_acp() {
        if let Some(name) = Self::acp_charset_name(acp) {
            return JavaString::from(name);
        }
        return JavaString::from(format!("x-windows-{acp}").as_str());
    }
    JavaString::from("UTF-8")
}

/// Java: Charset.defaultCharset() — the launcher/platform default charset.
///
/// the JVM default comes from file.encoding/native.encoding;
/// the port resolves the name via default_charset_name() (launcher override,
/// POSIX locale, Windows ANSI code page) and maps it through the implemented
/// charset set; a name outside the implemented set falls back to UTF-8 (the
/// oracle on the development host is UTF-8; registered adapter).
///
/// # Arguments
///   - none.
///
/// # Returns
///   - the default charset.
///
/// # Panics
///   - none.
pub fn default_charset() -> Charset {
    let name = Self::default_charset_name();
    Charset::for_name(&name).unwrap_or(Charset::Utf8)
}

/// Windows ANSI code page (Java: the native encoding used for
/// Charset.defaultCharset on Windows).
#[cfg(windows)]
pub(crate) fn windows_acp() -> Result<u32, ()> {
    unsafe extern "system" {
        fn GetACP() -> u32;
    }
    // SAFETY: GetACP is a Win32 API with no preconditions.
    Ok(unsafe { GetACP() })
}

/// Map a Windows ANSI code page to the JDK charset name (the JVM uses the
/// equivalent java.nio.charset name; the table covers the JDK 17 default
/// encodings; other code pages map to the JVM-style x-windows-N name).
#[cfg(windows)]
pub(crate) fn acp_charset_name(acp: u32) -> StdOption<&'static str> {
    match acp {
        65001 => StdOption::Some("UTF-8"),
        20127 => StdOption::Some("US-ASCII"),
        28591 => StdOption::Some("ISO-8859-1"),
        932 => StdOption::Some("Shift_JIS"),
        936 => StdOption::Some("GBK"),
        949 => StdOption::Some("x-windows-949"),
        950 => StdOption::Some("Big5"),
        1252 => StdOption::Some("windows-1252"),
        437 => StdOption::Some("IBM437"),
        _ => StdOption::None,
    }
}
}

/// The canonical names of the implemented charsets (the JDK also has charsets
/// that are not implemented here; see the crate docs).
pub fn available_charsets() -> Vec<JavaString> {
    SUPPORTED
        .iter()
        .map(|(_, canonical, _)| JavaString::from(*canonical))
        .collect()
}


/// Java: Charset.checkName / IllegalCharsetNameException rules (first character
/// alphanumeric; the rest alphanumeric or '-', '+', '.', ':', '_').
fn is_legal_charset_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '+' | '.' | ':' | '_'))
}

fn malformed_utf8(
    action: MalformedAction,
    units: &mut Vec<u16>,
    index: &mut usize,
    length: usize,
) -> Result<(), CharacterCodingError> {
    match action {
        MalformedAction::Report => Err(CharacterCodingError::MalformedInput { input_length: length }),
        MalformedAction::Replace => {
            units.push(0xFFFD);
            *index += length;
            Ok(())
        }
    }
}

fn is_continuation(byte: u8) -> bool {
    (0x80..=0xBF).contains(&byte)
}

/// UTF-8 decoder with the recorded Java malformed boundaries.
fn decode_utf8(bytes: &[u8], action: MalformedAction) -> Result<JavaString, CharacterCodingError> {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let first = bytes[index];
        if first < 0x80 {
            units.push(u16::from(first));
            index += 1;
            continue;
        }
        let (expected, second_min, second_max) = match first {
            0xC2..=0xDF => (2usize, 0x80u8, 0xBFu8),
            0xE0 => (3, 0xA0, 0xBF),
            0xE1..=0xEC | 0xEE..=0xEF => (3, 0x80, 0xBF),
            0xED => (3, 0x80, 0x9F),
            0xF0 => (4, 0x90, 0xBF),
            0xF1..=0xF3 => (4, 0x80, 0xBF),
            0xF4 => (4, 0x80, 0x8F),
            _ => {
                malformed_utf8(action, &mut units, &mut index, 1)?;
                continue;
            }
        };
        // Java special case: an ED lead byte followed by A0..BF can start a
        // UTF-8-encoded surrogate; the decoder consumes the available
        // continuation and reports the whole consumed prefix as malformed.
        if first == 0xED && index + 1 < bytes.len() && (0xA0..=0xBF).contains(&bytes[index + 1]) {
            let consumed = if index + 2 < bytes.len() && is_continuation(bytes[index + 2]) {
                3
            } else {
                2
            };
            malformed_utf8(action, &mut units, &mut index, consumed)?;
            continue;
        }
        let mut consumed = 1usize;
        let mut valid = true;
        while consumed < expected {
            if index + consumed >= bytes.len() {
                valid = false;
                break;
            }
            let byte = bytes[index + consumed];
            let (min, max) = if consumed == 1 {
                (second_min, second_max)
            } else {
                (0x80, 0xBF)
            };
            if byte < min || byte > max {
                valid = false;
                break;
            }
            consumed += 1;
        }
        if !valid {
            malformed_utf8(action, &mut units, &mut index, consumed.max(1))?;
            continue;
        }
        let code_point: u32 = match expected {
            2 => (u32::from(first & 0x1F) << 6) | u32::from(bytes[index + 1] & 0x3F),
            3 => {
                (u32::from(first & 0x0F) << 12)
                    | (u32::from(bytes[index + 1] & 0x3F) << 6)
                    | u32::from(bytes[index + 2] & 0x3F)
            }
            _ => {
                (u32::from(first & 0x07) << 18)
                    | (u32::from(bytes[index + 1] & 0x3F) << 12)
                    | (u32::from(bytes[index + 2] & 0x3F) << 6)
                    | u32::from(bytes[index + 3] & 0x3F)
            }
        };
        if code_point >= 0x10000 {
            let value = code_point - 0x10000;
            units.push(0xD800 + (value >> 10) as u16);
            units.push(0xDC00 + (value & 0x3FF) as u16);
        } else {
            units.push(code_point as u16);
        }
        index += expected;
    }
    Ok(JavaString::from_units(units))
}

/// US-ASCII decoder (any byte >= 0x80 is malformed, length 1).
fn decode_ascii(bytes: &[u8], action: MalformedAction) -> Result<JavaString, CharacterCodingError> {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    for byte in bytes {
        if *byte < 0x80 {
            units.push(u16::from(*byte));
        } else {
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => units.push(0xFFFD),
            }
        }
    }
    Ok(JavaString::from_units(units))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Utf16Endian {
    AutoBom,
    Big,
    Little,
}

fn decode_utf16(
    bytes: &[u8],
    endian: Utf16Endian,
    action: MalformedAction,
) -> Result<JavaString, CharacterCodingError> {
    let mut big = match endian {
        Utf16Endian::Big => true,
        Utf16Endian::Little => false,
        Utf16Endian::AutoBom => true,
    };
    let mut index = 0usize;
    if endian == Utf16Endian::AutoBom && bytes.len() >= 2 {
        if bytes[0] == 0xFE && bytes[1] == 0xFF {
            big = true;
            index = 2;
        } else if bytes[0] == 0xFF && bytes[1] == 0xFE {
            big = false;
            index = 2;
        }
    }
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len() / 2 + 1);
    while index < bytes.len() {
        if bytes.len() - index < 2 {
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => {
                    units.push(0xFFFD);
                    index += 1;
                    continue;
                }
            }
        }
        let unit = read_utf16_unit(bytes, index, big);
        if (0xD800..=0xDBFF).contains(&unit) {
            let remaining = bytes.len() - index;
            if remaining == 2 {
                match action {
                    MalformedAction::Report => {
                        return Err(CharacterCodingError::MalformedInput { input_length: 2 });
                    }
                    MalformedAction::Replace => {
                        units.push(0xFFFD);
                        index += 2;
                        continue;
                    }
                }
            }
            if remaining == 3 {
                match action {
                    MalformedAction::Report => {
                        return Err(CharacterCodingError::MalformedInput { input_length: 3 });
                    }
                    MalformedAction::Replace => {
                        units.push(0xFFFD);
                        index += 3;
                        continue;
                    }
                }
            }
            let next = read_utf16_unit(bytes, index + 2, big);
            if (0xDC00..=0xDFFF).contains(&next) {
                units.push(unit);
                units.push(next);
                index += 4;
                continue;
            }
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 4 });
                }
                MalformedAction::Replace => {
                    units.push(0xFFFD);
                    index += 4;
                    continue;
                }
            }
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 2 });
                }
                MalformedAction::Replace => {
                    units.push(0xFFFD);
                    index += 2;
                    continue;
                }
            }
        }
        units.push(unit);
        index += 2;
    }
    Ok(JavaString::from_units(units))
}

fn read_utf16_unit(bytes: &[u8], index: usize, big: bool) -> u16 {
    if big {
        (u16::from(bytes[index]) << 8) | u16::from(bytes[index + 1])
    } else {
        (u16::from(bytes[index + 1]) << 8) | u16::from(bytes[index])
    }
}

fn encode_utf8(
    units: &[u16],
    action: MalformedAction,
) -> Result<Vec<u8>, CharacterCodingError> {
    let mut out: Vec<u8> = Vec::with_capacity(units.len());
    let mut index = 0usize;
    while index < units.len() {
        let unit = units[index];
        if (0xD800..=0xDBFF).contains(&unit) {
            if index + 1 < units.len() && (0xDC00..=0xDFFF).contains(&units[index + 1]) {
                let code_point = 0x10000
                    + ((u32::from(unit) - 0xD800) << 10)
                    + (u32::from(units[index + 1]) - 0xDC00);
                out.push(0xF0 | ((code_point >> 18) as u8));
                out.push(0x80 | (((code_point >> 12) & 0x3F) as u8));
                out.push(0x80 | (((code_point >> 6) & 0x3F) as u8));
                out.push(0x80 | ((code_point & 0x3F) as u8));
                index += 2;
                continue;
            }
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => {
                    out.push(b'?');
                    index += 1;
                    continue;
                }
            }
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => {
                    out.push(b'?');
                    index += 1;
                    continue;
                }
            }
        }
        let value = u32::from(unit);
        if value < 0x80 {
            out.push(value as u8);
        } else if value < 0x800 {
            out.push(0xC0 | (value >> 6) as u8);
            out.push(0x80 | ((value & 0x3F) as u8));
        } else {
            out.push(0xE0 | (value >> 12) as u8);
            out.push(0x80 | (((value >> 6) & 0x3F) as u8));
            out.push(0x80 | ((value & 0x3F) as u8));
        }
        index += 1;
    }
    Ok(out)
}

fn encode_single_byte(
    units: &[u16],
    max: u16,
    action: MalformedAction,
) -> Result<Vec<u8>, CharacterCodingError> {
    let mut out: Vec<u8> = Vec::with_capacity(units.len());
    let mut index = 0usize;
    while index < units.len() {
        let unit = units[index];
        if (0xD800..=0xDBFF).contains(&unit) {
            if index + 1 < units.len() && (0xDC00..=0xDFFF).contains(&units[index + 1]) {
                match action {
                    MalformedAction::Report => {
                        return Err(CharacterCodingError::UnmappableCharacter { input_length: 2 });
                    }
                    MalformedAction::Replace => {
                        out.push(b'?');
                        index += 2;
                        continue;
                    }
                }
            }
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => {
                    out.push(b'?');
                    index += 1;
                    continue;
                }
            }
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => {
                    out.push(b'?');
                    index += 1;
                    continue;
                }
            }
        }
        if unit <= max {
            out.push(unit as u8);
        } else {
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::UnmappableCharacter { input_length: 1 });
                }
                MalformedAction::Replace => out.push(b'?'),
            }
        }
        index += 1;
    }
    Ok(out)
}

fn encode_utf16(
    units: &[u16],
    endian: Utf16Endian,
    with_bom: bool,
    action: MalformedAction,
) -> Result<Vec<u8>, CharacterCodingError> {
    let big = endian == Utf16Endian::Big;
    let mut out: Vec<u8> = Vec::with_capacity(units.len() * 2 + 2);
    if with_bom && !units.is_empty() {
        out.extend_from_slice(&[0xFE, 0xFF]);
    }
    let mut index = 0usize;
    while index < units.len() {
        let unit = units[index];
        if (0xD800..=0xDBFF).contains(&unit) {
            if index + 1 < units.len() && (0xDC00..=0xDFFF).contains(&units[index + 1]) {
                write_utf16_unit(&mut out, unit, big);
                write_utf16_unit(&mut out, units[index + 1], big);
                index += 2;
                continue;
            }
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => {
                    write_utf16_unit(&mut out, 0xFFFD, big);
                    index += 1;
                    continue;
                }
            }
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            match action {
                MalformedAction::Report => {
                    return Err(CharacterCodingError::MalformedInput { input_length: 1 });
                }
                MalformedAction::Replace => {
                    write_utf16_unit(&mut out, 0xFFFD, big);
                    index += 1;
                    continue;
                }
            }
        }
        write_utf16_unit(&mut out, unit, big);
        index += 1;
    }
    Ok(out)
}

fn write_utf16_unit(out: &mut Vec<u8>, unit: u16, big: bool) {
    let high = (unit >> 8) as u8;
    let low = (unit & 0xFF) as u8;
    if big {
        out.push(high);
        out.push(low);
    } else {
        out.push(low);
        out.push(high);
    }
}
