//! Development-time JDK oracle for the charset layer.
//!
//! The expectations were collected from the baseline JDK 17.0.19.10
//! java.nio.charset implementation through a Java program (Charset.forName,
//! CharsetDecoder/CharsetEncoder with CodingErrorAction.REPORT and .REPLACE)
//! and are embedded here so the tests run without a JVM.
//!
//! Reference implementation: java.nio.charset.Charset / CharsetDecoder /
//! CharsetEncoder of the default provider (UTF-8, US-ASCII, ISO-8859-1,
//! UTF-16, UTF-16BE, UTF-16LE).

use crate::charset::{Charset, MalformedAction};
use crate::error::{CharacterCodingError, CharsetError};
use javac_strings::JavaString;

#[derive(Clone, Copy)]
enum Outcome {
    Ok(&'static str),
    Malformed(usize),
}

#[derive(Clone, Copy)]
enum EncodeOutcome {
    Ok(&'static str),
    Malformed(usize),
    Unmappable(usize),
}

#[derive(Clone, Copy)]
struct DecodeCase {
    charset: &'static str,
    bytes: &'static str,
    report: Outcome,
    replace: Outcome,
}

#[derive(Clone, Copy)]
struct EncodeCase {
    charset: &'static str,
    units: &'static str,
    report: EncodeOutcome,
    replace: EncodeOutcome,
}

#[derive(Clone, Copy)]
enum NameOutcome {
    Ok(&'static str),
    IllegalCharsetName,
    UnsupportedCharset,
}

const DECODE_CASES: &[DecodeCase] = &[
    DecodeCase { charset: "UTF-8", bytes: "", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "UTF-8", bytes: "41 42 43", report: Outcome::Ok("ABC"), replace: Outcome::Ok("ABC") },
    DecodeCase { charset: "UTF-8", bytes: "C3 A9", report: Outcome::Ok("\\u00E9"), replace: Outcome::Ok("\\u00E9") },
    DecodeCase { charset: "UTF-8", bytes: "E4 B8 AD", report: Outcome::Ok("\\u4E2D"), replace: Outcome::Ok("\\u4E2D") },
    DecodeCase { charset: "UTF-8", bytes: "F0 9F 98 80", report: Outcome::Ok("\\uD83D\\uDE00"), replace: Outcome::Ok("\\uD83D\\uDE00") },
    DecodeCase { charset: "UTF-8", bytes: "80", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "C3", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "C3 28", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD(") },
    DecodeCase { charset: "UTF-8", bytes: "E0 80 80", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD\\uFFFD\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "ED A0 80", report: Outcome::Malformed(3), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "F4 90 80 80", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD\\uFFFD\\uFFFD\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "FF", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "C0 80", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "E4 B8", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "F0 9F 98", report: Outcome::Malformed(3), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "US-ASCII", bytes: "", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "US-ASCII", bytes: "41 42 43", report: Outcome::Ok("ABC"), replace: Outcome::Ok("ABC") },
    DecodeCase { charset: "US-ASCII", bytes: "80", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "US-ASCII", bytes: "FF", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "US-ASCII", bytes: "41 80 42", report: Outcome::Malformed(1), replace: Outcome::Ok("A\\uFFFDB") },
    DecodeCase { charset: "US-ASCII", bytes: "80 80", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD\\uFFFD") },
    DecodeCase { charset: "ISO-8859-1", bytes: "", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "ISO-8859-1", bytes: "41 42 43", report: Outcome::Ok("ABC"), replace: Outcome::Ok("ABC") },
    DecodeCase { charset: "ISO-8859-1", bytes: "80 41 FF", report: Outcome::Ok("\\u0080A\\u00FF"), replace: Outcome::Ok("\\u0080A\\u00FF") },
    DecodeCase { charset: "ISO-8859-1", bytes: "E9", report: Outcome::Ok("\\u00E9"), replace: Outcome::Ok("\\u00E9") },
    DecodeCase { charset: "UTF-16", bytes: "", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "UTF-16", bytes: "00 41 00 42", report: Outcome::Ok("AB"), replace: Outcome::Ok("AB") },
    DecodeCase { charset: "UTF-16", bytes: "FE FF 00 41", report: Outcome::Ok("A"), replace: Outcome::Ok("A") },
    DecodeCase { charset: "UTF-16", bytes: "FF FE 41 00", report: Outcome::Ok("A"), replace: Outcome::Ok("A") },
    DecodeCase { charset: "UTF-16", bytes: "00", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16", bytes: "D8 00 00 41", report: Outcome::Malformed(4), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16", bytes: "00 41 D8 00", report: Outcome::Malformed(2), replace: Outcome::Ok("A\\uFFFD") },
    DecodeCase { charset: "UTF-16", bytes: "DC 00", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16", bytes: "FE FF", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "UTF-16", bytes: "FF", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16", bytes: "FF FE", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "UTF-16", bytes: "D8 00 DC 00", report: Outcome::Ok("\\uD800\\uDC00"), replace: Outcome::Ok("\\uD800\\uDC00") },
    DecodeCase { charset: "UTF-16", bytes: "FE FF D8 00", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16BE", bytes: "", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "UTF-16BE", bytes: "00 41", report: Outcome::Ok("A"), replace: Outcome::Ok("A") },
    DecodeCase { charset: "UTF-16BE", bytes: "FE FF 00 41", report: Outcome::Ok("\\uFEFFA"), replace: Outcome::Ok("\\uFEFFA") },
    DecodeCase { charset: "UTF-16BE", bytes: "00", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16BE", bytes: "D8 00", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16BE", bytes: "DC 00", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16LE", bytes: "", report: Outcome::Ok(""), replace: Outcome::Ok("") },
    DecodeCase { charset: "UTF-16LE", bytes: "41 00", report: Outcome::Ok("A"), replace: Outcome::Ok("A") },
    DecodeCase { charset: "UTF-16LE", bytes: "FF FE 41 00", report: Outcome::Ok("\\uFEFFA"), replace: Outcome::Ok("\\uFEFFA") },
    DecodeCase { charset: "UTF-16LE", bytes: "41", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16LE", bytes: "00 D8", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16LE", bytes: "00 DC", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "E4", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "E1 80 41", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFDA") },
    DecodeCase { charset: "UTF-8", bytes: "F0 9F 41", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFDA") },
    DecodeCase { charset: "UTF-8", bytes: "F0 9F 98 41", report: Outcome::Malformed(3), replace: Outcome::Ok("\\uFFFDA") },
    DecodeCase { charset: "UTF-8", bytes: "ED A0", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-8", bytes: "ED A0 41", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFDA") },
    DecodeCase { charset: "UTF-8", bytes: "80 80 41", report: Outcome::Malformed(1), replace: Outcome::Ok("\\uFFFD\\uFFFDA") },
    DecodeCase { charset: "UTF-16", bytes: "D8 00 00 42 00 43", report: Outcome::Malformed(4), replace: Outcome::Ok("\\uFFFDC") },
    DecodeCase { charset: "UTF-16", bytes: "00 41 D8 00 DC 00", report: Outcome::Ok("A\\uD800\\uDC00"), replace: Outcome::Ok("A\\uD800\\uDC00") },
    DecodeCase { charset: "UTF-16", bytes: "DC 00 00 41", report: Outcome::Malformed(2), replace: Outcome::Ok("\\uFFFDA") },
    DecodeCase { charset: "UTF-16BE", bytes: "D8 00 00 41", report: Outcome::Malformed(4), replace: Outcome::Ok("\\uFFFD") },
    DecodeCase { charset: "UTF-16BE", bytes: "D8 00 DC 00", report: Outcome::Ok("\\uD800\\uDC00"), replace: Outcome::Ok("\\uD800\\uDC00") },
];

const ENCODE_CASES: &[EncodeCase] = &[
    EncodeCase { charset: "UTF-8", units: "0041 0042", report: EncodeOutcome::Ok("41 42"), replace: EncodeOutcome::Ok("41 42") },
    EncodeCase { charset: "UTF-8", units: "00E9", report: EncodeOutcome::Ok("C3 A9"), replace: EncodeOutcome::Ok("C3 A9") },
    EncodeCase { charset: "UTF-8", units: "4E2D", report: EncodeOutcome::Ok("E4 B8 AD"), replace: EncodeOutcome::Ok("E4 B8 AD") },
    EncodeCase { charset: "UTF-8", units: "D83D DE00", report: EncodeOutcome::Ok("F0 9F 98 80"), replace: EncodeOutcome::Ok("F0 9F 98 80") },
    EncodeCase { charset: "UTF-8", units: "D800", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "UTF-8", units: "DC00", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "UTF-8", units: "00FF", report: EncodeOutcome::Ok("C3 BF"), replace: EncodeOutcome::Ok("C3 BF") },
    EncodeCase { charset: "UTF-8", units: "0100", report: EncodeOutcome::Ok("C4 80"), replace: EncodeOutcome::Ok("C4 80") },
    EncodeCase { charset: "UTF-8", units: "0041", report: EncodeOutcome::Ok("41"), replace: EncodeOutcome::Ok("41") },
    EncodeCase { charset: "US-ASCII", units: "0041 0042", report: EncodeOutcome::Ok("41 42"), replace: EncodeOutcome::Ok("41 42") },
    EncodeCase { charset: "US-ASCII", units: "00E9", report: EncodeOutcome::Unmappable(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "US-ASCII", units: "4E2D", report: EncodeOutcome::Unmappable(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "US-ASCII", units: "D83D DE00", report: EncodeOutcome::Unmappable(2), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "US-ASCII", units: "D800", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "US-ASCII", units: "DC00", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "US-ASCII", units: "00FF", report: EncodeOutcome::Unmappable(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "US-ASCII", units: "0100", report: EncodeOutcome::Unmappable(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "US-ASCII", units: "0041", report: EncodeOutcome::Ok("41"), replace: EncodeOutcome::Ok("41") },
    EncodeCase { charset: "ISO-8859-1", units: "0041 0042", report: EncodeOutcome::Ok("41 42"), replace: EncodeOutcome::Ok("41 42") },
    EncodeCase { charset: "ISO-8859-1", units: "00E9", report: EncodeOutcome::Ok("E9"), replace: EncodeOutcome::Ok("E9") },
    EncodeCase { charset: "ISO-8859-1", units: "4E2D", report: EncodeOutcome::Unmappable(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "ISO-8859-1", units: "D83D DE00", report: EncodeOutcome::Unmappable(2), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "ISO-8859-1", units: "D800", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "ISO-8859-1", units: "DC00", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "ISO-8859-1", units: "00FF", report: EncodeOutcome::Ok("FF"), replace: EncodeOutcome::Ok("FF") },
    EncodeCase { charset: "ISO-8859-1", units: "0100", report: EncodeOutcome::Unmappable(1), replace: EncodeOutcome::Ok("3F") },
    EncodeCase { charset: "ISO-8859-1", units: "0041", report: EncodeOutcome::Ok("41"), replace: EncodeOutcome::Ok("41") },
    EncodeCase { charset: "UTF-16", units: "0041 0042", report: EncodeOutcome::Ok("FE FF 00 41 00 42"), replace: EncodeOutcome::Ok("FE FF 00 41 00 42") },
    EncodeCase { charset: "UTF-16", units: "00E9", report: EncodeOutcome::Ok("FE FF 00 E9"), replace: EncodeOutcome::Ok("FE FF 00 E9") },
    EncodeCase { charset: "UTF-16", units: "4E2D", report: EncodeOutcome::Ok("FE FF 4E 2D"), replace: EncodeOutcome::Ok("FE FF 4E 2D") },
    EncodeCase { charset: "UTF-16", units: "D83D DE00", report: EncodeOutcome::Ok("FE FF D8 3D DE 00"), replace: EncodeOutcome::Ok("FE FF D8 3D DE 00") },
    EncodeCase { charset: "UTF-16", units: "D800", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("FE FF FF FD") },
    EncodeCase { charset: "UTF-16", units: "DC00", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("FE FF FF FD") },
    EncodeCase { charset: "UTF-16", units: "00FF", report: EncodeOutcome::Ok("FE FF 00 FF"), replace: EncodeOutcome::Ok("FE FF 00 FF") },
    EncodeCase { charset: "UTF-16", units: "0100", report: EncodeOutcome::Ok("FE FF 01 00"), replace: EncodeOutcome::Ok("FE FF 01 00") },
    EncodeCase { charset: "UTF-16", units: "0041", report: EncodeOutcome::Ok("FE FF 00 41"), replace: EncodeOutcome::Ok("FE FF 00 41") },
    EncodeCase { charset: "UTF-16BE", units: "0041 0042", report: EncodeOutcome::Ok("00 41 00 42"), replace: EncodeOutcome::Ok("00 41 00 42") },
    EncodeCase { charset: "UTF-16BE", units: "00E9", report: EncodeOutcome::Ok("00 E9"), replace: EncodeOutcome::Ok("00 E9") },
    EncodeCase { charset: "UTF-16BE", units: "4E2D", report: EncodeOutcome::Ok("4E 2D"), replace: EncodeOutcome::Ok("4E 2D") },
    EncodeCase { charset: "UTF-16BE", units: "D83D DE00", report: EncodeOutcome::Ok("D8 3D DE 00"), replace: EncodeOutcome::Ok("D8 3D DE 00") },
    EncodeCase { charset: "UTF-16BE", units: "D800", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("FF FD") },
    EncodeCase { charset: "UTF-16BE", units: "DC00", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("FF FD") },
    EncodeCase { charset: "UTF-16BE", units: "00FF", report: EncodeOutcome::Ok("00 FF"), replace: EncodeOutcome::Ok("00 FF") },
    EncodeCase { charset: "UTF-16BE", units: "0100", report: EncodeOutcome::Ok("01 00"), replace: EncodeOutcome::Ok("01 00") },
    EncodeCase { charset: "UTF-16BE", units: "0041", report: EncodeOutcome::Ok("00 41"), replace: EncodeOutcome::Ok("00 41") },
    EncodeCase { charset: "UTF-16LE", units: "0041 0042", report: EncodeOutcome::Ok("41 00 42 00"), replace: EncodeOutcome::Ok("41 00 42 00") },
    EncodeCase { charset: "UTF-16LE", units: "00E9", report: EncodeOutcome::Ok("E9 00"), replace: EncodeOutcome::Ok("E9 00") },
    EncodeCase { charset: "UTF-16LE", units: "4E2D", report: EncodeOutcome::Ok("2D 4E"), replace: EncodeOutcome::Ok("2D 4E") },
    EncodeCase { charset: "UTF-16LE", units: "D83D DE00", report: EncodeOutcome::Ok("3D D8 00 DE"), replace: EncodeOutcome::Ok("3D D8 00 DE") },
    EncodeCase { charset: "UTF-16LE", units: "D800", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("FD FF") },
    EncodeCase { charset: "UTF-16LE", units: "DC00", report: EncodeOutcome::Malformed(1), replace: EncodeOutcome::Ok("FD FF") },
    EncodeCase { charset: "UTF-16LE", units: "00FF", report: EncodeOutcome::Ok("FF 00"), replace: EncodeOutcome::Ok("FF 00") },
    EncodeCase { charset: "UTF-16LE", units: "0100", report: EncodeOutcome::Ok("00 01"), replace: EncodeOutcome::Ok("00 01") },
    EncodeCase { charset: "UTF-16LE", units: "0041", report: EncodeOutcome::Ok("41 00"), replace: EncodeOutcome::Ok("41 00") },
    EncodeCase { charset: "UTF-8", units: "", report: EncodeOutcome::Ok(""), replace: EncodeOutcome::Ok("") },
    EncodeCase { charset: "US-ASCII", units: "", report: EncodeOutcome::Ok(""), replace: EncodeOutcome::Ok("") },
    EncodeCase { charset: "ISO-8859-1", units: "", report: EncodeOutcome::Ok(""), replace: EncodeOutcome::Ok("") },
    EncodeCase { charset: "UTF-16", units: "", report: EncodeOutcome::Ok(""), replace: EncodeOutcome::Ok("") },
    EncodeCase { charset: "UTF-16BE", units: "", report: EncodeOutcome::Ok(""), replace: EncodeOutcome::Ok("") },
    EncodeCase { charset: "UTF-16LE", units: "", report: EncodeOutcome::Ok(""), replace: EncodeOutcome::Ok("") },
];

const ALIASES: &[(&str, &str)] = &[
    ("UTF-8", "unicode-1-1-utf-8,UTF8"),
    ("US-ASCII", "ANSI_X3.4-1968,cp367,csASCII,iso-ir-6,ASCII,iso_646.irv:1983,ANSI_X3.4-1986,ascii7,default,ISO_646.irv:1991,ISO646-US,IBM367,646,us"),
    ("ISO-8859-1", "819,ISO8859-1,l1,ISO_8859-1:1987,ISO_8859-1,8859_1,iso-ir-100,latin1,cp819,ISO8859_1,IBM819,ISO_8859_1,IBM-819,csISOLatin1"),
    ("UTF-16", "UTF_16,unicode,utf16,UnicodeBig"),
    ("UTF-16BE", "X-UTF-16BE,UTF_16BE,ISO-10646-UCS-2,UnicodeBigUnmarked"),
    ("UTF-16LE", "UnicodeLittleUnmarked,UTF_16LE,X-UTF-16LE"),
];

// JDK-only charsets (e.g. GBK) are not part of this layer's support set; their
// explicit UnsupportedCharset behavior is asserted in
// unsupported_charsets_are_explicit_errors.
const NAME_CASES: &[(&str, NameOutcome)] = &[
    ("UTF-8", NameOutcome::Ok("UTF-8")),
    ("utf-8", NameOutcome::Ok("UTF-8")),
    ("UTF8", NameOutcome::Ok("UTF-8")),
    ("Utf-8", NameOutcome::Ok("UTF-8")),
    ("UTF_8", NameOutcome::UnsupportedCharset),
    ("utf_8", NameOutcome::UnsupportedCharset),
    ("unicode-1-1-utf-8", NameOutcome::Ok("UTF-8")),
    ("US-ASCII", NameOutcome::Ok("US-ASCII")),
    ("ascii", NameOutcome::Ok("US-ASCII")),
    ("ASCII", NameOutcome::Ok("US-ASCII")),
    ("ansi_x3.4-1968", NameOutcome::Ok("US-ASCII")),
    ("ISO-8859-1", NameOutcome::Ok("ISO-8859-1")),
    ("ISO8859-1", NameOutcome::Ok("ISO-8859-1")),
    ("ISO8859_1", NameOutcome::Ok("ISO-8859-1")),
    ("latin1", NameOutcome::Ok("ISO-8859-1")),
    ("ISO_8859-1", NameOutcome::Ok("ISO-8859-1")),
    ("8859_1", NameOutcome::Ok("ISO-8859-1")),
    ("UTF-16", NameOutcome::Ok("UTF-16")),
    ("UTF_16", NameOutcome::Ok("UTF-16")),
    ("utf16", NameOutcome::Ok("UTF-16")),
    ("UTF-16BE", NameOutcome::Ok("UTF-16BE")),
    ("UTF_16BE", NameOutcome::Ok("UTF-16BE")),
    ("UnicodeBigUnmarked", NameOutcome::Ok("UTF-16BE")),
    ("UTF-16LE", NameOutcome::Ok("UTF-16LE")),
    ("UTF_16LE", NameOutcome::Ok("UTF-16LE")),
    ("UnicodeLittleUnmarked", NameOutcome::Ok("UTF-16LE")),
    ("", NameOutcome::IllegalCharsetName),
    (" utf-8 ", NameOutcome::IllegalCharsetName),
    ("bad name!", NameOutcome::IllegalCharsetName),
    ("NoSuchCharset", NameOutcome::UnsupportedCharset),
];

fn charset(name: &str) -> Charset {
    Charset::for_name(&JavaString::from(name)).expect("supported charset")
}

fn parse_hex(text: &str) -> Vec<u8> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split(' ')
        .map(|part| u8::from_str_radix(part, 16).expect("hex byte"))
        .collect()
}

fn parse_units(text: &str) -> JavaString {
    if text.is_empty() {
        return JavaString::new();
    }
    let units = text
        .split(' ')
        .map(|part| u16::from_str_radix(part, 16).expect("hex code unit"))
        .collect::<Vec<u16>>();
    JavaString::from_units(units)
}

fn parse_escaped(text: &str) -> JavaString {
    let chars: Vec<char> = text.chars().collect();
    let mut units: Vec<u16> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '\\'
            && index + 5 < chars.len()
            && chars[index + 1] == 'u'
        {
            let hex: String = chars[index + 2..index + 6].iter().collect();
            units.push(u16::from_str_radix(&hex, 16).expect("escaped code unit"));
            index += 6;
        } else {
            let mut buffer = [0u16; 2];
            for unit in chars[index].encode_utf16(&mut buffer) {
                units.push(*unit);
            }
            index += 1;
        }
    }
    JavaString::from_units(units)
}

fn expect_decode(outcome: Outcome) -> Result<JavaString, CharacterCodingError> {
    match outcome {
        Outcome::Ok(text) => Ok(parse_escaped(text)),
        Outcome::Malformed(length) => Err(CharacterCodingError::MalformedInput { input_length: length }),
    }
}

fn expect_encode(outcome: EncodeOutcome) -> Result<Vec<u8>, CharacterCodingError> {
    match outcome {
        EncodeOutcome::Ok(text) => Ok(parse_hex(text)),
        EncodeOutcome::Malformed(length) => Err(CharacterCodingError::MalformedInput { input_length: length }),
        EncodeOutcome::Unmappable(length) => {
            Err(CharacterCodingError::UnmappableCharacter { input_length: length })
        }
    }
}

#[test]
fn aliases_and_names_match_the_jdk_oracle() {
    for (canonical, alias_list) in ALIASES {
        let parsed = charset(canonical);
        assert_eq!(parsed.name(), JavaString::from(*canonical), "name {}", canonical);
        let expected: Vec<JavaString> = if alias_list.is_empty() {
            Vec::new()
        } else {
            alias_list.split(',').map(JavaString::from).collect()
        };
        assert_eq!(parsed.aliases(), expected, "aliases {}", canonical);
        assert!(
            Charset::for_name(&JavaString::from(*canonical)).is_ok(),
            "canonical name {}", canonical
        );
        for alias in expected {
            assert_eq!(
                Charset::for_name(&alias).expect("alias resolves"),
                parsed,
                "alias {} of {}", alias, canonical
            );
        }
    }
    for (name, expected) in NAME_CASES {
        let result = Charset::for_name(&JavaString::from(*name));
        match expected {
            NameOutcome::Ok(canonical) => {
                let resolved = result.expect("name resolves");
                assert_eq!(resolved.name(), JavaString::from(*canonical), "forName({})", name);
            }
            NameOutcome::IllegalCharsetName => assert!(
                matches!(result, Err(CharsetError::IllegalCharsetName { .. })),
                "forName({}) should be an illegal name",
                name
            ),
            NameOutcome::UnsupportedCharset => assert!(
                matches!(result, Err(CharsetError::UnsupportedCharset { .. })),
                "forName({}) should be unsupported",
                name
            ),
        }
    }
}

#[test]
fn decode_matches_the_jdk_oracle() {
    for case in DECODE_CASES {
        let parsed = charset(case.charset);
        let bytes = parse_hex(case.bytes);
        assert_eq!(
            parsed.decode(&bytes, MalformedAction::Report),
            expect_decode(case.report),
            "decode REPORT {} [{}]",
            case.charset,
            case.bytes
        );
        assert_eq!(
            parsed.decode(&bytes, MalformedAction::Replace),
            expect_decode(case.replace),
            "decode REPLACE {} [{}]",
            case.charset,
            case.bytes
        );
    }
}

#[test]
fn encode_matches_the_jdk_oracle() {
    for case in ENCODE_CASES {
        let parsed = charset(case.charset);
        let units = parse_units(case.units);
        assert_eq!(
            parsed.encode(&units, MalformedAction::Report),
            expect_encode(case.report),
            "encode REPORT {} [{}]",
            case.charset,
            case.units
        );
        assert_eq!(
            parsed.encode(&units, MalformedAction::Replace),
            expect_encode(case.replace),
            "encode REPLACE {} [{}]",
            case.charset,
            case.units
        );
    }
}

#[test]
fn unsupported_charsets_are_explicit_errors() {
    assert!(matches!(
        Charset::for_name(&JavaString::from("GBK")),
        Err(CharsetError::UnsupportedCharset { .. })
    ));
    assert!(matches!(
        Charset::for_name(&JavaString::from("GB18030")),
        Err(CharsetError::UnsupportedCharset { .. })
    ));
    assert!(matches!(
        Charset::for_name(&JavaString::from("Big5")),
        Err(CharsetError::UnsupportedCharset { .. })
    ));
    assert!(matches!(
        Charset::for_name(&JavaString::from("Shift_JIS")),
        Err(CharsetError::UnsupportedCharset { .. })
    ));
    assert!(matches!(
        Charset::for_name(&JavaString::from("")),
        Err(CharsetError::IllegalCharsetName { .. })
    ));
}

#[test]
fn available_charsets_and_roundtrip() {
    let available = crate::available_charsets();
    assert_eq!(available.len(), 6);
    let text = parse_units("0041 00E9 4E2D D83D DE00");
    for name in ["UTF-8", "UTF-16", "UTF-16BE", "UTF-16LE"] {
        let parsed = charset(name);
        let bytes = parsed.encode(&text, MalformedAction::Report).expect("encode");
        let decoded = parsed.decode(&bytes, MalformedAction::Report).expect("decode");
        assert_eq!(decoded, text, "roundtrip {}", name);
    }
    let ascii = charset("US-ASCII");
    let hello = parse_units("0048 0069");
    let encoded = ascii.encode(&hello, MalformedAction::Report).expect("encode ascii");
    assert_eq!(ascii.decode(&encoded, MalformedAction::Report).expect("decode ascii"), hello);
}
