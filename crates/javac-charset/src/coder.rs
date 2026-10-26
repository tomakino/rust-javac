// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.charset.CharsetDecoder / CharsetEncoder for the
// charsets implemented by this crate (UTF-8, US-ASCII, ISO-8859-1, UTF-16,
// UTF-16BE, UTF-16LE).
// Why not 1:1: java.nio.charset is a service-provider framework whose
// decoders are Charset implementations; the port expresses the same
// incremental state machine over the implemented charsets:
// - decode(ByteBuffer, CharBuffer, endOfInput) returns the java.nio.charset
//   CoderResult states (UNDERFLOW / OVERFLOW / MALFORMED[n] / UNMAPPABLE[n]);
//   the input buffer is only advanced for consumed sequences, so the
//   BaseFileManager decode loop can report each malformed sequence exactly
//   like the upstream code;
// - onMalformedInput/onUnmappableCharacter select REPORT/REPLACE/IGNORE;
// - the replacement characters, average/max chars-per-byte and
//   bytes-per-char values are the ones recorded from the JDK 17.0.19 oracle;
// - incomplete trailing sequences stay in the input buffer and yield
//   UNDERFLOW when endOfInput is false (javac always uses endOfInput = true,
//   the false branch is kept for the java.nio contract).

//! Incremental java.nio.charset decoder/encoder semantics for the implemented
//! charsets.

use crate::charset::Charset;
use crate::nio::{ByteBuffer, CharBuffer, CoderResult, CodingErrorAction};
use javac_strings::JavaString;

/// The java CharacterCodingException replacement for a decoder is U+FFFD.
const DECODER_REPLACEMENT: u16 = 0xFFFD;

enum DecodeStep {
    Done { consumed: usize, units: Vec<u16> },
    Incomplete,
    Malformed { length: usize },
    Unmappable { length: usize },
}

enum EncodeStep {
    Done { consumed: usize, bytes: Vec<u8> },
    Incomplete,
    Malformed { length: usize },
    Unmappable { length: usize },
}

/// Java: java.nio.charset.CharsetDecoder.
#[derive(Clone, Debug)]
pub struct CharsetDecoder {
    charset: Charset,
    malformed_action: CodingErrorAction,
    unmappable_action: CodingErrorAction,
    /// Java: the UTF-16 BOM state (the decoder consumes the BOM on the first
    /// decode call).
    started: bool,
    endian_big: bool,
}

impl CharsetDecoder {
    /// Create the decoder of a charset (Java: Charset.newDecoder()).
    pub(crate) fn new(charset: Charset) -> CharsetDecoder {
        CharsetDecoder {
            charset,
            malformed_action: CodingErrorAction::Report,
            unmappable_action: CodingErrorAction::Report,
            started: false,
            endian_big: true,
        }
    }

    /// Java: public final Charset charset().
    pub fn charset(&self) -> Charset {
        self.charset
    }

    /// Java: public final CharsetDecoder onMalformedInput(CodingErrorAction
    /// newAction).
    pub fn on_malformed_input(&mut self, action: CodingErrorAction) -> &mut CharsetDecoder {
        self.malformed_action = action;
        self
    }

    /// Java: public final CharsetDecoder onUnmappableCharacter(CodingErrorAction
    /// newAction).
    pub fn on_unmappable_character(&mut self, action: CodingErrorAction) -> &mut CharsetDecoder {
        self.unmappable_action = action;
        self
    }

    /// Java: public final String replacement() (decoders replace with U+FFFD).
    pub fn replacement(&self) -> JavaString {
        JavaString::from_units(vec![DECODER_REPLACEMENT])
    }

    /// Java: public final float averageCharsPerByte().
    pub fn average_chars_per_byte(&self) -> f32 {
        match self.charset {
            Charset::Utf16 | Charset::Utf16Be | Charset::Utf16Le => 0.5,
            _ => 1.0,
        }
    }

    /// Java: public final float maxCharsPerByte().
    pub fn max_chars_per_byte(&self) -> f32 {
        1.0
    }

    /// Java: public final CharsetDecoder reset().
    pub fn reset(&mut self) -> &mut CharsetDecoder {
        self.started = false;
        self.endian_big = true;
        self
    }

    /// Java: public final CoderResult flush(CharBuffer out) — nothing to
    /// flush for the implemented decoders.
    pub fn flush(&mut self, _out: &mut CharBuffer) -> CoderResult {
        CoderResult::Underflow
    }

    /// Java: public final CoderResult decode(ByteBuffer in, CharBuffer out,
    /// boolean endOfInput).
    ///
    /// # Arguments
    ///   - input: the byte buffer (advanced only for consumed sequences);
    ///   - output: the char buffer (flipped by the caller like javac does);
    ///   - end_of_input: whether the input is complete.
    ///
    /// # Returns
    ///   - the CoderResult state of this decode step.
    ///
    /// # Panics
    ///   - none.
    pub fn decode(
        &mut self,
        input: &mut ByteBuffer,
        output: &mut CharBuffer,
        end_of_input: bool,
    ) -> CoderResult {
        loop {
            let remaining = input.remaining_slice();
            if remaining.is_empty() {
                return CoderResult::Underflow;
            }
            if !self.started {
                if self.charset == Charset::Utf16 {
                    if remaining.len() < 2 {
                        if !end_of_input {
                            return CoderResult::Underflow;
                        }
                        self.started = true;
                        return CoderResult::Underflow;
                    }
                    if remaining[0] == 0xFE && remaining[1] == 0xFF {
                        self.endian_big = true;
                        input.advance(2);
                    } else if remaining[0] == 0xFF && remaining[1] == 0xFE {
                        self.endian_big = false;
                        input.advance(2);
                    }
                }
                self.started = true;
                continue;
            }
            let step = if self.charset == Charset::Utf16 {
                decode_utf16_step(remaining, self.endian_big, end_of_input)
            } else {
                decode_step(self.charset, remaining, end_of_input)
            };
            match step {
                DecodeStep::Done { consumed, units } => {
                    if output.remaining() < units.len() {
                        return CoderResult::Overflow;
                    }
                    output.put_units(&units);
                    input.advance(consumed);
                }
                DecodeStep::Incomplete => return CoderResult::Underflow,
                DecodeStep::Malformed { length } => {
                    match self.malformed_action {
                        CodingErrorAction::Report => {
                            return CoderResult::MalformedForLength(length);
                        }
                        CodingErrorAction::Replace => {
                            if output.remaining() < 1 {
                                return CoderResult::Overflow;
                            }
                            output.put(DECODER_REPLACEMENT);
                            input.advance(length);
                        }
                        CodingErrorAction::Ignore => {
                            input.advance(length);
                        }
                    }
                }
                DecodeStep::Unmappable { length } => {
                    match self.unmappable_action {
                        CodingErrorAction::Report => {
                            return CoderResult::UnmappableForLength(length);
                        }
                        CodingErrorAction::Replace => {
                            if output.remaining() < 1 {
                                return CoderResult::Overflow;
                            }
                            output.put(DECODER_REPLACEMENT);
                            input.advance(length);
                        }
                        CodingErrorAction::Ignore => {
                            input.advance(length);
                        }
                    }
                }
            }
        }
    }
}

/// Java: java.nio.charset.CharsetEncoder.
#[derive(Clone, Debug)]
pub struct CharsetEncoder {
    charset: Charset,
    malformed_action: CodingErrorAction,
    unmappable_action: CodingErrorAction,
    /// Java: the UTF-16 encoder writes the BOM before the first character.
    bom_written: bool,
}

impl CharsetEncoder {
    /// Create the encoder of a charset (Java: Charset.newEncoder()).
    pub(crate) fn new(charset: Charset) -> CharsetEncoder {
        CharsetEncoder {
            charset,
            malformed_action: CodingErrorAction::Report,
            unmappable_action: CodingErrorAction::Report,
            bom_written: false,
        }
    }

    /// Java: public final Charset charset().
    pub fn charset(&self) -> Charset {
        self.charset
    }

    /// Java: public final CharsetEncoder onMalformedInput(CodingErrorAction
    /// newAction).
    pub fn on_malformed_input(&mut self, action: CodingErrorAction) -> &mut CharsetEncoder {
        self.malformed_action = action;
        self
    }

    /// Java: public final CharsetEncoder onUnmappableCharacter(CodingErrorAction
    /// newAction).
    pub fn on_unmappable_character(&mut self, action: CodingErrorAction) -> &mut CharsetEncoder {
        self.unmappable_action = action;
        self
    }

    /// Java: public final byte[] replacement() (the recorded JDK bytes).
    pub fn replacement(&self) -> Vec<u8> {
        match self.charset {
            Charset::Utf16Le => vec![0xFD, 0xFF],
            Charset::Utf16 | Charset::Utf16Be => vec![0xFF, 0xFD],
            _ => vec![b'?'],
        }
    }

    /// Java: public final float averageBytesPerChar().
    pub fn average_bytes_per_char(&self) -> f32 {
        match self.charset {
            Charset::Utf16 | Charset::Utf16Be | Charset::Utf16Le => 2.0,
            Charset::Utf8 => 1.1,
            _ => 1.0,
        }
    }

    /// Java: public final float maxBytesPerChar().
    pub fn max_bytes_per_char(&self) -> f32 {
        match self.charset {
            Charset::Utf8 => 3.0,
            Charset::Utf16 => 4.0,
            Charset::Utf16Be | Charset::Utf16Le => 2.0,
            _ => 1.0,
        }
    }

    /// Java: public final CharsetEncoder reset().
    pub fn reset(&mut self) -> &mut CharsetEncoder {
        self.bom_written = false;
        self
    }

    /// Java: public final CoderResult flush(ByteBuffer out) — nothing to flush
    /// for the implemented encoders.
    pub fn flush(&mut self, _out: &mut ByteBuffer) -> CoderResult {
        CoderResult::Underflow
    }

    /// Java: public final CoderResult encode(CharBuffer in, ByteBuffer out,
    /// boolean endOfInput).
    ///
    /// # Arguments
    ///   - input: the char buffer (advanced only for consumed characters);
    ///   - output: the byte buffer;
    ///   - end_of_input: whether the input is complete.
    ///
    /// # Returns
    ///   - the CoderResult state of this encode step.
    ///
    /// # Panics
    ///   - none.
    pub fn encode(
        &mut self,
        input: &mut CharBuffer,
        output: &mut ByteBuffer,
        end_of_input: bool,
    ) -> CoderResult {
        loop {
            let remaining = input.remaining_units();
            if remaining.is_empty() {
                return CoderResult::Underflow;
            }
            if self.charset == Charset::Utf16 && !self.bom_written && !remaining.is_empty() {
                if output.remaining() < 2 {
                    return CoderResult::Overflow;
                }
                output.put(0xFE);
                output.put(0xFF);
                self.bom_written = true;
                continue;
            }
            let step = encode_step(self.charset, remaining, end_of_input);
            match step {
                EncodeStep::Done { consumed, bytes } => {
                    if output.remaining() < bytes.len() {
                        return CoderResult::Overflow;
                    }
                    output.put_slice(&bytes);
                    input.advance(consumed);
                }
                EncodeStep::Incomplete => return CoderResult::Underflow,
                EncodeStep::Malformed { length } => match self.malformed_action {
                    CodingErrorAction::Report => {
                        return CoderResult::MalformedForLength(length);
                    }
                    CodingErrorAction::Replace => {
                        let replacement = self.replacement();
                        if output.remaining() < replacement.len() {
                            return CoderResult::Overflow;
                        }
                        output.put_slice(&replacement);
                        input.advance(length);
                    }
                    CodingErrorAction::Ignore => input.advance(length),
                },
                EncodeStep::Unmappable { length } => match self.unmappable_action {
                    CodingErrorAction::Report => {
                        return CoderResult::UnmappableForLength(length);
                    }
                    CodingErrorAction::Replace => {
                        let replacement = self.replacement();
                        if output.remaining() < replacement.len() {
                            return CoderResult::Overflow;
                        }
                        output.put_slice(&replacement);
                        input.advance(length);
                    }
                    CodingErrorAction::Ignore => input.advance(length),
                },
            }
        }
    }
}

fn utf8_second_range(first: u8) -> Option<(usize, u8, u8)> {
    match first {
        0xC2..=0xDF => Some((2, 0x80, 0xBF)),
        0xE0 => Some((3, 0xA0, 0xBF)),
        0xE1..=0xEC | 0xEE..=0xEF => Some((3, 0x80, 0xBF)),
        0xED => Some((3, 0x80, 0x9F)),
        0xF0 => Some((4, 0x90, 0xBF)),
        0xF1..=0xF3 => Some((4, 0x80, 0xBF)),
        0xF4 => Some((4, 0x80, 0x8F)),
        _ => None,
    }
}

fn is_continuation(byte: u8) -> bool {
    (0x80..=0xBF).contains(&byte)
}

fn decode_utf8_step(bytes: &[u8], end_of_input: bool) -> DecodeStep {
    let first = bytes[0];
    if first < 0x80 {
        return DecodeStep::Done {
            consumed: 1,
            units: vec![u16::from(first)],
        };
    }
    let Some((expected, second_min, second_max)) = utf8_second_range(first) else {
        return DecodeStep::Malformed { length: 1 };
    };
    if first == 0xED && bytes.len() > 1 && (0xA0..=0xBF).contains(&bytes[1]) {
        // Java special case: a UTF-8-encoded surrogate prefix is malformed;
        // the consumed prefix is reported as one malformed sequence.
        let length = if bytes.len() >= 3 {
            if is_continuation(bytes[2]) { 3 } else { 2 }
        } else {
            2
        };
        return DecodeStep::Malformed { length };
    }
    let mut consumed = 1usize;
    while consumed < expected {
        if consumed >= bytes.len() {
            if !end_of_input {
                return DecodeStep::Incomplete;
            }
            return DecodeStep::Malformed {
                length: consumed.max(1),
            };
        }
        let byte = bytes[consumed];
        let (min, max) = if consumed == 1 {
            (second_min, second_max)
        } else {
            (0x80, 0xBF)
        };
        if byte < min || byte > max {
            return DecodeStep::Malformed {
                length: consumed.max(1),
            };
        }
        consumed += 1;
    }
    let code_point: u32 = match expected {
        2 => (u32::from(first & 0x1F) << 6) | u32::from(bytes[1] & 0x3F),
        3 => {
            (u32::from(first & 0x0F) << 12)
                | (u32::from(bytes[1] & 0x3F) << 6)
                | u32::from(bytes[2] & 0x3F)
        }
        _ => {
            (u32::from(first & 0x07) << 18)
                | (u32::from(bytes[1] & 0x3F) << 12)
                | (u32::from(bytes[2] & 0x3F) << 6)
                | u32::from(bytes[3] & 0x3F)
        }
    };
    let units = if code_point >= 0x10000 {
        let value = code_point - 0x10000;
        vec![0xD800 + (value >> 10) as u16, 0xDC00 + (value & 0x3FF) as u16]
    } else {
        vec![code_point as u16]
    };
    DecodeStep::Done { consumed, units }
}

fn decode_step(charset: Charset, bytes: &[u8], end_of_input: bool) -> DecodeStep {
    match charset {
        Charset::Utf8 => decode_utf8_step(bytes, end_of_input),
        Charset::UsAscii => {
            if bytes[0] < 0x80 {
                DecodeStep::Done {
                    consumed: 1,
                    units: vec![u16::from(bytes[0])],
                }
            } else {
                DecodeStep::Malformed { length: 1 }
            }
        }
        Charset::Iso8859_1 => DecodeStep::Done {
            consumed: 1,
            units: vec![u16::from(bytes[0])],
        },
        Charset::Utf16 | Charset::Utf16Be | Charset::Utf16Le => {
            decode_utf16_step(bytes, true, end_of_input)
        }
    }
}

fn read_utf16_unit(bytes: &[u8], index: usize, big: bool) -> u16 {
    if big {
        (u16::from(bytes[index]) << 8) | u16::from(bytes[index + 1])
    } else {
        (u16::from(bytes[index + 1]) << 8) | u16::from(bytes[index])
    }
}

fn decode_utf16_step(bytes: &[u8], big: bool, end_of_input: bool) -> DecodeStep {
    if bytes.len() < 2 {
        if !end_of_input {
            return DecodeStep::Incomplete;
        }
        return DecodeStep::Malformed { length: 1 };
    }
    let unit = read_utf16_unit(bytes, 0, big);
    if (0xD800..=0xDBFF).contains(&unit) {
        let remaining = bytes.len();
        if remaining < 4 {
            if !end_of_input {
                return DecodeStep::Incomplete;
            }
            return DecodeStep::Malformed { length: remaining };
        }
        let next = read_utf16_unit(bytes, 2, big);
        if (0xDC00..=0xDFFF).contains(&next) {
            return DecodeStep::Done {
                consumed: 4,
                units: vec![unit, next],
            };
        }
        return DecodeStep::Malformed { length: 4 };
    }
    if (0xDC00..=0xDFFF).contains(&unit) {
        return DecodeStep::Malformed { length: 2 };
    }
    DecodeStep::Done {
        consumed: 2,
        units: vec![unit],
    }
}

fn decode_utf16_step_endian(charset: Charset) -> bool {
    !matches!(charset, Charset::Utf16Le)
}

fn encode_step(charset: Charset, units: &[u16], end_of_input: bool) -> EncodeStep {
    match charset {
        Charset::Utf8 => encode_utf8_step(units, end_of_input),
        Charset::UsAscii => encode_single_byte_step(units, 0x7F, end_of_input),
        Charset::Iso8859_1 => encode_single_byte_step(units, 0xFF, end_of_input),
        Charset::Utf16 => encode_utf16_step(units, decode_utf16_step_endian(charset), end_of_input),
        Charset::Utf16Be => encode_utf16_step(units, true, end_of_input),
        Charset::Utf16Le => encode_utf16_step(units, false, end_of_input),
    }
}

fn encode_utf8_step(units: &[u16], end_of_input: bool) -> EncodeStep {
    let unit = units[0];
    if (0xD800..=0xDBFF).contains(&unit) {
        if units.len() >= 2 && (0xDC00..=0xDFFF).contains(&units[1]) {
            let code_point =
                0x10000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(units[1]) - 0xDC00);
            return EncodeStep::Done {
                consumed: 2,
                bytes: vec![
                    0xF0 | ((code_point >> 18) as u8),
                    0x80 | (((code_point >> 12) & 0x3F) as u8),
                    0x80 | (((code_point >> 6) & 0x3F) as u8),
                    0x80 | ((code_point & 0x3F) as u8),
                ],
            };
        }
        if !end_of_input && units.len() == 1 {
            return EncodeStep::Incomplete;
        }
        return EncodeStep::Malformed { length: 1 };
    }
    if (0xDC00..=0xDFFF).contains(&unit) {
        return EncodeStep::Malformed { length: 1 };
    }
    let value = u32::from(unit);
    let bytes = if value < 0x80 {
        vec![value as u8]
    } else if value < 0x800 {
        vec![0xC0 | (value >> 6) as u8, 0x80 | ((value & 0x3F) as u8)]
    } else {
        vec![
            0xE0 | (value >> 12) as u8,
            0x80 | (((value >> 6) & 0x3F) as u8),
            0x80 | ((value & 0x3F) as u8),
        ]
    };
    EncodeStep::Done { consumed: 1, bytes }
}

fn encode_single_byte_step(units: &[u16], max: u16, end_of_input: bool) -> EncodeStep {
    let unit = units[0];
    if (0xD800..=0xDBFF).contains(&unit) {
        if units.len() >= 2 && (0xDC00..=0xDFFF).contains(&units[1]) {
            return EncodeStep::Unmappable { length: 2 };
        }
        if !end_of_input && units.len() == 1 {
            return EncodeStep::Incomplete;
        }
        return EncodeStep::Malformed { length: 1 };
    }
    if (0xDC00..=0xDFFF).contains(&unit) {
        return EncodeStep::Malformed { length: 1 };
    }
    if unit <= max {
        EncodeStep::Done {
            consumed: 1,
            bytes: vec![unit as u8],
        }
    } else {
        EncodeStep::Unmappable { length: 1 }
    }
}

fn encode_utf16_step(units: &[u16], big: bool, end_of_input: bool) -> EncodeStep {
    let unit = units[0];
    if (0xD800..=0xDBFF).contains(&unit) {
        if units.len() >= 2 && (0xDC00..=0xDFFF).contains(&units[1]) {
            return EncodeStep::Done {
                consumed: 2,
                bytes: utf16_bytes(&[unit, units[1]], big),
            };
        }
        if !end_of_input && units.len() == 1 {
            return EncodeStep::Incomplete;
        }
        return EncodeStep::Malformed { length: 1 };
    }
    if (0xDC00..=0xDFFF).contains(&unit) {
        return EncodeStep::Malformed { length: 1 };
    }
    EncodeStep::Done {
        consumed: 1,
        bytes: utf16_bytes(&[unit], big),
    }
}

fn utf16_bytes(units: &[u16], big: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(units.len() * 2);
    for unit in units {
        let high = (unit >> 8) as u8;
        let low = (unit & 0xFF) as u8;
        if big {
            bytes.push(high);
            bytes.push(low);
        } else {
            bytes.push(low);
            bytes.push(high);
        }
    }
    bytes
}
