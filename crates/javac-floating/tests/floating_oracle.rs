// Origin: oracle expectations generated with JDK 17 (Temurin 17.0.19) from
// jdk/internal/math/FloatingDecimal.java @ 982d53c (jdk-17.0.19-ga).

use javac_floating::{parse_double, parse_float, to_java_string_double, to_java_string_float};

fn oracle() -> &'static str {
    include_str!("data/floating-oracle.txt")
}

fn parse_hex_bits(bits: &str) -> u64 {
    u64::from_str_radix(bits, 16).expect("bits")
}

fn parse_hex_bits32(bits: &str) -> u32 {
    u32::from_str_radix(bits, 16).expect("bits")
}

#[test]
fn oracle_double_parse_and_format() {
    for line in oracle().lines() {
        let mut parts = line.split('|');
        let kind = parts.next().expect("kind");
        match kind {
            "D" => {
                let input = parts.next().expect("input");
                let bits = parts.next().expect("bits");
                let text = parts.next().expect("text");
                if bits == "NaN" {
                    continue;
                }
                let expected_bits = parse_hex_bits(bits);
                let parsed = parse_double(input);
                assert_eq!(
                    parsed.to_bits(),
                    expected_bits,
                    "parse_double({input:?}) bits (expected text {text:?})"
                );
                let formatted = to_java_string_double(parsed);
                assert_eq!(
                    formatted, text,
                    "to_java_string_double({input:?})"
                );
                // round-trip through the Java string form
                let reparsed = parse_double(&formatted);
                assert_eq!(
                    reparsed.to_bits(),
                    expected_bits,
                    "round-trip parse_double({formatted:?})"
                );
            }
            "F" => {
                let input = parts.next().expect("input");
                let bits = parts.next().expect("bits");
                let text = parts.next().expect("text");
                if bits == "NaN" {
                    continue;
                }
                let expected_bits = parse_hex_bits32(bits);
                let parsed = parse_float(input);
                assert_eq!(
                    parsed.to_bits(),
                    expected_bits,
                    "parse_float({input:?}) bits (expected text {text:?})"
                );
                let formatted = to_java_string_float(parsed);
                assert_eq!(formatted, text, "to_java_string_float({input:?})");
                let reparsed = parse_float(&formatted);
                assert_eq!(
                    reparsed.to_bits(),
                    expected_bits,
                    "round-trip parse_float({formatted:?})"
                );
            }
            _ => {}
        }
    }
}

#[test]
fn oracle_double_format_bits() {
    for line in oracle().lines() {
        let mut parts = line.split('|');
        let kind = parts.next().expect("kind");
        match kind {
            "DT" => {
                let bits = parts.next().expect("bits");
                let text = parts.next().expect("text");
                let value = f64::from_bits(parse_hex_bits(bits));
                assert_eq!(to_java_string_double(value), text, "bits {bits}");
            }
            "FT" => {
                let bits = parts.next().expect("bits");
                let text = parts.next().expect("text");
                let value = f32::from_bits(parse_hex_bits32(bits));
                assert_eq!(to_java_string_float(value), text, "bits {bits}");
            }
            _ => {}
        }
    }
}

#[test]
fn round_trip_all_representable_specials() {
    for value in [
        0.0f64,
        -0.0,
        1.0,
        -1.0,
        0.1,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let text = to_java_string_double(value);
        let back = parse_double(&text);
        if value.is_nan() {
            assert!(back.is_nan());
        } else {
            assert_eq!(back.to_bits(), value.to_bits(), "double {value} via {text:?}");
        }
    }
    for value in [
        0.0f32,
        -0.0,
        1.0,
        -1.0,
        0.1,
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        let text = to_java_string_float(value);
        let back = parse_float(&text);
        if value.is_nan() {
            assert!(back.is_nan());
        } else {
            assert_eq!(back.to_bits(), value.to_bits(), "float {value} via {text:?}");
        }
    }
}
