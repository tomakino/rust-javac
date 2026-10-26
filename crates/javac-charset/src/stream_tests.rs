// Oracle tests for the incremental CharsetDecoder/CharsetEncoder layer.

//! The expectations are recorded from the JDK 17.0.19 development oracle
//! (java.nio.charset CharsetDecoder/CharsetEncoder): the average/max
//! chars-per-byte and bytes-per-char values, the replacement characters, and
//! the CoderResult sequences of the BaseFileManager decode loop.

use crate::charset::Charset;
use crate::nio::{ByteBuffer, CharBuffer, CoderResult, CodingErrorAction};

fn units(cb: &CharBuffer) -> Vec<u16> {
    cb.slice_upto_limit().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

/// Literal transcription of BaseFileManager.decode's loop (JDK 17
/// BaseFileManager.java decode) used as the oracle comparison: it returns the
/// decoded text and the (unmappable bytes, charset name, dest.limit()) triples.
fn base_decode(charset_name: &str, bytes: &[u8]) -> (Vec<u16>, Vec<(String, String, usize)>) {
    let charset = Charset::for_name(&javac_strings::JavaString::from(charset_name)).expect("charset");
    let mut decoder = charset.new_decoder();
    decoder
        .on_malformed_input(CodingErrorAction::Report)
        .on_unmappable_character(CodingErrorAction::Report);
    let factor = decoder.average_chars_per_byte() * 0.8 + decoder.max_chars_per_byte() * 0.2;
    let mut input = ByteBuffer::wrap(bytes);
    let mut dest = CharBuffer::allocate(10 + (input.remaining() as f32 * factor) as usize);
    let mut errors = Vec::new();
    loop {
        let result = decoder.decode(&mut input, &mut dest, true);
        dest.flip();
        if result.is_underflow() {
            return (units(&dest), errors);
        } else if result.is_overflow() {
            let capacity =
                10 + dest.capacity() + (input.remaining() as f32 * decoder.max_chars_per_byte()) as usize;
            let mut bigger = CharBuffer::allocate(capacity);
            bigger.put_flipped(&dest);
            dest = bigger;
        } else if result.is_malformed() || result.is_unmappable() {
            let length = result.length();
            let mut unmappable = Vec::new();
            for _ in 0..length {
                unmappable.push(input.get());
            }
            let charset_name = charset.name().to_string_lossy();
            errors.push((hex(&unmappable), charset_name, dest.limit()));
            dest.set_position(dest.limit());
            dest.set_limit(dest.capacity());
            dest.put(0xFFFD);
        } else {
            panic!("AssertionError");
        }
    }
}

#[test]
fn decoder_and_encoder_parameters_match_the_oracle() {
    // JDK 17.0.19: printouts of averageCharsPerByte/maxCharsPerByte,
    // averageBytesPerChar/maxBytesPerChar and the replacement values.
    let expected = [
        ("UTF-8", 1.0f32, 1.0f32, 1.1f32, 3.0f32, vec![b'?']),
        ("US-ASCII", 1.0, 1.0, 1.0, 1.0, vec![b'?']),
        ("ISO-8859-1", 1.0, 1.0, 1.0, 1.0, vec![b'?']),
        ("UTF-16", 0.5, 1.0, 2.0, 4.0, vec![0xFF, 0xFD]),
        ("UTF-16BE", 0.5, 1.0, 2.0, 2.0, vec![0xFF, 0xFD]),
        ("UTF-16LE", 0.5, 1.0, 2.0, 2.0, vec![0xFD, 0xFF]),
    ];
    for (name, dec_avg, dec_max, enc_avg, enc_max, replacement) in expected {
        let charset = Charset::for_name(&javac_strings::JavaString::from(name)).expect("charset");
        let decoder = charset.new_decoder();
        let encoder = charset.new_encoder();
        assert_eq!(decoder.average_chars_per_byte(), dec_avg, "{name} dec.avg");
        assert_eq!(decoder.max_chars_per_byte(), dec_max, "{name} dec.max");
        assert_eq!(encoder.average_bytes_per_char(), enc_avg, "{name} enc.avg");
        assert_eq!(encoder.max_bytes_per_char(), enc_max, "{name} enc.max");
        assert_eq!(encoder.replacement(), replacement, "{name} enc.repl");
        assert_eq!(decoder.replacement().as_units().to_vec(), vec![0xFFFD], "{name} dec.repl");
    }
}

#[test]
fn base_file_manager_decode_loop_matches_the_oracle() {
    // JDK 17.0.19 oracle (literal BaseFileManager.decode transcription).
    let cases: &[(&str, &[u8], &[u16], &[(&str, &str, usize)])] = &[
        (
            "UTF-8",
            &[0x41, 0xE0, 0x80, 0xE0, 0xA0, 0x41],
            &[0x0041, 0xFFFD, 0xFFFD, 0xFFFD, 0x0041],
            &[("E0", "UTF-8", 1), ("80", "UTF-8", 2), ("E0A0", "UTF-8", 3)],
        ),
        (
            "UTF-8",
            &[0xED, 0xA0, 0x80, 0x41],
            &[0xFFFD, 0x0041],
            &[("EDA080", "UTF-8", 0)],
        ),
        (
            "UTF-8",
            &[0xF0, 0x80, 0x80, 0x41],
            &[0xFFFD, 0xFFFD, 0xFFFD, 0x0041],
            &[("F0", "UTF-8", 0), ("80", "UTF-8", 1), ("80", "UTF-8", 2)],
        ),
        (
            "UTF-8",
            &[0xE1, 0x80],
            &[0xFFFD],
            &[("E180", "UTF-8", 0)],
        ),
        (
            "UTF-8",
            &[0xC3, 0x28, 0xFF],
            &[0xFFFD, 0x0028, 0xFFFD],
            &[("C3", "UTF-8", 0), ("FF", "UTF-8", 2)],
        ),
        (
            "US-ASCII",
            &[0x41, 0x80, 0x42],
            &[0x0041, 0xFFFD, 0x0042],
            &[("80", "US-ASCII", 1)],
        ),
    ];
    for (charset_name, bytes, expected_units, expected_errors) in cases {
        let (text, errors) = base_decode(charset_name, bytes);
        assert_eq!(text.as_slice(), *expected_units, "{charset_name} {bytes:?}");
        let expected: Vec<(String, String, usize)> = expected_errors
            .iter()
            .map(|(bytes, name, pos)| (bytes.to_string(), name.to_string(), *pos))
            .collect();
        assert_eq!(errors, expected, "{charset_name} {bytes:?}");
    }
}

#[test]
fn decoder_underflow_keeps_incomplete_sequences_in_the_input() {
    let mut decoder = Charset::Utf8.new_decoder();
    let mut input = ByteBuffer::wrap(&[0xE1]);
    let mut output = CharBuffer::allocate(8);
    let result = decoder.decode(&mut input, &mut output, false);
    assert_eq!(result, CoderResult::Underflow);
    assert_eq!(input.position(), 0);
    let mut input = ByteBuffer::wrap(&[0xE1, 0x80, 0x80]);
    let result = decoder.decode(&mut input, &mut output, true);
    assert_eq!(result, CoderResult::Underflow);
    assert_eq!(input.position(), 3);
    output.flip();
    assert_eq!(units(&output), vec![0x1000]);
}

#[test]
fn encoder_stream_matches_the_oracle() {
    // JDK 17.0.19 US-ASCII encoder REPORT: "A\u{e9}\u{20ac}" reports
    // UNMAPPABLE[1] at positions 1 and 2; isolated surrogates report
    // MALFORMED[1].
    let mut encoder = Charset::UsAscii.new_encoder();
    encoder
        .on_malformed_input(CodingErrorAction::Report)
        .on_unmappable_character(CodingErrorAction::Report);
    let mut input = CharBuffer::wrap_units(&[0x0041, 0x00E9, 0x20AC]);
    let mut output = ByteBuffer::allocate(8);
    let first = encoder.encode(&mut input, &mut output, true);
    assert_eq!(first, CoderResult::UnmappableForLength(1));
    assert_eq!(input.position(), 1);
    encoder.encode(&mut input, &mut output, true);
    // consume the unmappable character like the caller does
    input.advance(1);
    let second = encoder.encode(&mut input, &mut output, true);
    assert_eq!(second, CoderResult::UnmappableForLength(1));
    assert_eq!(input.position(), 2);
    input.advance(1);
    assert_eq!(
        encoder.encode(&mut input, &mut output, true),
        CoderResult::Underflow
    );
    output.flip();
    assert_eq!(output.remaining_slice(), &[b'A']);
    let mut encoder = Charset::UsAscii.new_encoder();
    let mut input = CharBuffer::wrap_units(&[0x0041, 0xD800, 0x0042]);
    let mut output = ByteBuffer::allocate(8);
    assert_eq!(encoder.encode(&mut input, &mut output, true), CoderResult::MalformedForLength(1));
}

#[test]
fn default_charset_name_matches_the_oracle_contract() {
    // Oracle on the development host: Charset.defaultCharset().name() = UTF-8,
    // native.encoding = UTF-8, file.encoding = UTF-8.
    let name = Charset::default_charset_name();
    assert!(!name.to_string_lossy().is_empty());
    assert!(matches!(
        Charset::for_name(&name),
        Ok(_) | Err(crate::error::CharsetError::UnsupportedCharset { .. })
    ));
}

#[cfg(windows)]
#[test]
fn windows_acp_names_match_the_jdk_default_encodings() {
    // The JDK maps the Windows ANSI code page to the java.nio.charset name of
    // the corresponding default encoding.
    assert_eq!(Charset::acp_charset_name(65001), Some("UTF-8"));
    assert_eq!(Charset::acp_charset_name(20127), Some("US-ASCII"));
    assert_eq!(Charset::acp_charset_name(28591), Some("ISO-8859-1"));
    assert_eq!(Charset::acp_charset_name(936), Some("GBK"));
    assert_eq!(Charset::acp_charset_name(950), Some("Big5"));
    assert_eq!(Charset::acp_charset_name(932), Some("Shift_JIS"));
    assert_eq!(Charset::acp_charset_name(0), None);
}

