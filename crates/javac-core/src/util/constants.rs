// Origin: com/sun/tools/javac/util/Constants.java @ 982d53c (jdk-17.0.19-ga)

//! Utilities for operating on constant values.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the Java Object constant value/parameter maps to the closed
//! ConstantValue enum (the Java primitive wrapper types and String; the Java
//! null value is not representable inside the enum and is documented at the
//! call sites, registered in PORTING_RULES section 9); Type stays the ported
//! code::type_::Type.
//!
//! PORTING_RULES Porting rules.17: formatFloat/formatDouble use the JDK
//! java.lang.FloatingDecimal semantics (Float.toString/Double.toString), ported
//! 1:1 from jdk/internal/math/FloatingDecimal.java @ 982d53c in the
//! javac-floating adapter crate (registered in docs/DEPENDENCIES.md).

use crate::code::type_::Type;
use crate::code::type_tag::TypeTag;
use crate::util::convert;
use crate::util::java_string::JavaString;

/// The Java Object representation of a constant value (the primitive wrapper
/// types plus String; see the module doc for the null note).
#[derive(Clone, Debug, PartialEq)]
pub enum ConstantValue {
    /// Java Integer
    Integer(i32),
    /// Java Byte (standard wrapped representation)
    Byte(i8),
    /// Java Short (standard wrapped representation)
    Short(i16),
    /// Java Long
    Long(i64),
    /// Java Float
    Float(f32),
    /// Java Double
    Double(f64),
    /// Java Character (UTF-16 code unit)
    Character(u16),
    /// Java Boolean
    Boolean(bool),
    /// Java String
    String(JavaString),
}

impl ConstantValue {
    /// Return the Java display form of this constant value.
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - the display string.
    ///
    /// # Panics
    ///   - none.
    pub fn display(&self) -> String {
        self.to_display()
    }

    /// Java: Object.toString() / String.valueOf(Object) for the wrapped value.
    fn to_display(&self) -> String {
        match self {
            ConstantValue::Integer(value) => value.to_string(),
            ConstantValue::Byte(value) => value.to_string(),
            ConstantValue::Short(value) => value.to_string(),
            ConstantValue::Long(value) => value.to_string(),
            ConstantValue::Float(value) => javac_floating::to_java_string_float(*value),
            ConstantValue::Double(value) => javac_floating::to_java_string_double(*value),
            ConstantValue::Character(value) => {
                JavaString::from_units(vec![*value]).to_string_lossy()
            }
            ConstantValue::Boolean(value) => value.to_string(),
            ConstantValue::String(value) => value.to_string_lossy(),
        }
    }
}

/// Converts a constant in internal representation (in which boolean, char,
/// byte, short, and int are each represented by an Integer) into standard
/// representation. Other values are returned unchanged.
///
/// Java: public static Object decode(Object value, Type type)
pub fn decode(value: ConstantValue, ty: &Type) -> ConstantValue {
    if let ConstantValue::Integer(int_val) = value {
        match ty.get_tag() {
            TypeTag::BOOLEAN => return ConstantValue::Boolean(int_val != 0),
            TypeTag::CHAR => return ConstantValue::Character(int_val as u16),
            TypeTag::BYTE => return ConstantValue::Byte(int_val as i8),
            TypeTag::SHORT => return ConstantValue::Short(int_val as i16),
            _ => {}
        }
    }
    value
}

/// Returns a string representation of a constant value (given in internal
/// representation), quoted and formatted as in Java source.
///
/// Java: public static String format(Object value, Type type)
pub fn format(value: ConstantValue, ty: &Type) -> String {
    let value = decode(value, ty);
    match ty.get_tag() {
        TypeTag::BYTE => return format_byte(expect_byte(&value)),
        TypeTag::LONG => return format_long(expect_long(&value)),
        TypeTag::FLOAT => return format_float(expect_float(&value)),
        TypeTag::DOUBLE => return format_double(expect_double(&value)),
        TypeTag::CHAR => return format_char(expect_character(&value)),
        _ => {}
    }
    if let ConstantValue::String(text) = &value {
        return format_string(text);
    }
    value.to_display()
}

/// Returns a string representation of a constant value (given in standard
/// wrapped representation), quoted and formatted as in Java source.
///
/// Java: public static String format(Object value)
pub fn format_value(value: &ConstantValue) -> String {
    match value {
        ConstantValue::Byte(byte_val) => format_byte(*byte_val),
        ConstantValue::Short(short_val) => format_short(*short_val),
        ConstantValue::Long(long_val) => format_long(*long_val),
        ConstantValue::Float(float_val) => format_float(*float_val),
        ConstantValue::Double(double_val) => format_double(*double_val),
        ConstantValue::Character(char_val) => format_char(*char_val),
        ConstantValue::String(str_val) => format_string(str_val),
        ConstantValue::Integer(_) | ConstantValue::Boolean(_) => value.to_display(),
    }
}

fn expect_byte(value: &ConstantValue) -> i8 {
    match value {
        ConstantValue::Byte(value) => *value,
        _ => panic!("unexpected constant value, expected Byte"),
    }
}

fn expect_float(value: &ConstantValue) -> f32 {
    match value {
        ConstantValue::Float(value) => *value,
        _ => panic!("unexpected constant value, expected Float"),
    }
}

fn expect_double(value: &ConstantValue) -> f64 {
    match value {
        ConstantValue::Double(value) => *value,
        _ => panic!("unexpected constant value, expected Double"),
    }
}

// Java: private static String formatFloat(float f)
fn format_float(f: f32) -> String {
    if f.is_nan() {
        "0.0f/0.0f".to_string()
    } else if f.is_infinite() {
        if f < 0.0 {
            "-1.0f/0.0f".to_string()
        } else {
            "1.0f/0.0f".to_string()
        }
    } else {
        format!("{}f", javac_floating::to_java_string_float(f))
    }
}

// Java: private static String formatDouble(double d)
fn format_double(d: f64) -> String {
    if d.is_nan() {
        "0.0/0.0".to_string()
    } else if d.is_infinite() {
        if d < 0.0 {
            "-1.0/0.0".to_string()
        } else {
            "1.0/0.0".to_string()
        }
    } else {
        javac_floating::to_java_string_double(d)
    }
}

fn expect_long(value: &ConstantValue) -> i64 {
    match value {
        ConstantValue::Long(value) => *value,
        _ => panic!("unexpected constant value, expected Long"),
    }
}

fn expect_character(value: &ConstantValue) -> u16 {
    match value {
        ConstantValue::Character(value) => *value,
        _ => panic!("unexpected constant value, expected Character"),
    }
}

/// Java: private static String formatByte(byte b) {
///           return String.format("(byte)0x%02x", b); }
fn format_byte(byte_val: i8) -> String {
    // String.format %02x prints the unsigned byte value (oracle: -1 -> ff,
    // -128 -> 80).
    format!("(byte)0x{:02x}", byte_val as u8)
}

/// Java: private static String formatShort(short s) {
///           return String.format("(short)%d", s); }
fn format_short(short_val: i16) -> String {
    format!("(short){short_val}")
}

/// Java: private static String formatLong(long lng) { return lng + "L"; }
fn format_long(long_val: i64) -> String {
    format!("{long_val}L")
}

/// Java: private static String formatChar(char c) {
///           return '\'' + Convert.quote(c) + '\''; }
fn format_char(char_val: u16) -> String {
    format!("'{}'", convert::quote_char(char_val).to_string_lossy())
}

/// Java: private static String formatString(String s) {
///           return '"' + Convert.quote(s) + '"'; }
fn format_string(text: &JavaString) -> String {
    format!("\"{}\"", convert::quote(text).to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::java_string::JavaString;

    fn ty(tag: TypeTag) -> Type {
        Type::new(tag, None, crate::code::type_::TypeKindData::JCNoType { kind: crate::code::type_::JCNoTypeKind::Base })
    }

    #[test]
    fn decode_converts_integer_to_the_standard_representation() {
        let int_value = ConstantValue::Integer(1);
        assert_eq!(
            decode(int_value.clone(), &ty(TypeTag::BOOLEAN)),
            ConstantValue::Boolean(true)
        );
        assert_eq!(
            decode(ConstantValue::Integer(0), &ty(TypeTag::BOOLEAN)),
            ConstantValue::Boolean(false)
        );
        assert_eq!(
            decode(ConstantValue::Integer(65), &ty(TypeTag::CHAR)),
            ConstantValue::Character(65)
        );
        assert_eq!(
            decode(ConstantValue::Integer(0x1234), &ty(TypeTag::BYTE)),
            ConstantValue::Byte(0x34)
        );
        assert_eq!(
            decode(ConstantValue::Integer(70000), &ty(TypeTag::SHORT)),
            ConstantValue::Short(70000i32 as i16)
        );
        // Unknown/non-primitive tags return the value unchanged.
        assert_eq!(
            decode(int_value.clone(), &ty(TypeTag::INT)),
            int_value
        );
        assert_eq!(
            decode(ConstantValue::String(JavaString::from("x")), &ty(TypeTag::CHAR)),
            ConstantValue::String(JavaString::from("x"))
        );
    }

    #[test]
    fn format_uses_the_java_source_representation() {
        assert_eq!(
            format(ConstantValue::Integer(5), &ty(TypeTag::INT)),
            "5"
        );
        assert_eq!(
            format(ConstantValue::Integer(1), &ty(TypeTag::BOOLEAN)),
            "true"
        );
        assert_eq!(
            format(ConstantValue::Integer(-1), &ty(TypeTag::BYTE)),
            "(byte)0xff"
        );
        assert_eq!(
            format(ConstantValue::Integer(-128), &ty(TypeTag::BYTE)),
            "(byte)0x80"
        );
        assert_eq!(
            format(ConstantValue::Integer(200), &ty(TypeTag::SHORT)),
            "200"
        );
        assert_eq!(
            format(ConstantValue::Long(42), &ty(TypeTag::LONG)),
            "42L"
        );
        assert_eq!(
            format(ConstantValue::Integer(65), &ty(TypeTag::CHAR)),
            "'A'"
        );
        assert_eq!(
            format(ConstantValue::Integer(10), &ty(TypeTag::CHAR)),
            "'\\n'"
        );
        assert_eq!(
            format(
                ConstantValue::String(JavaString::from("a\"b")),
                &ty(TypeTag::CLASS)
            ),
            "\"a\\\"b\""
        );
    }

    #[test]
    fn format_value_uses_the_standard_wrapped_representation() {
        assert_eq!(format_value(&ConstantValue::Byte(-1)), "(byte)0xff");
        assert_eq!(format_value(&ConstantValue::Short(-5)), "(short)-5");
        assert_eq!(format_value(&ConstantValue::Long(7)), "7L");
        assert_eq!(format_value(&ConstantValue::Character(9)), "'\\t'");
        assert_eq!(
            format_value(&ConstantValue::String(JavaString::from("s"))),
            "\"s\""
        );
        assert_eq!(format_value(&ConstantValue::Integer(-3)), "-3");
        assert_eq!(format_value(&ConstantValue::Boolean(true)), "true");
    }

    #[test]
    fn float_and_double_formatting_matches_floating_decimal() {
        // Java: Constants.formatFloat / formatDouble semantics
        assert_eq!(format(ConstantValue::Float(1.0), &ty(TypeTag::FLOAT)), "1.0f");
        assert_eq!(
            format(ConstantValue::Float(f32::from_bits(1)), &ty(TypeTag::FLOAT)),
            "1.4E-45f"
        );
        assert_eq!(format(ConstantValue::Double(1.0), &ty(TypeTag::DOUBLE)), "1.0");
        assert_eq!(
            format(ConstantValue::Double(f64::from_bits(1)), &ty(TypeTag::DOUBLE)),
            "4.9E-324"
        );
        assert_eq!(
            format_value(&ConstantValue::Float(f32::NAN)),
            "0.0f/0.0f"
        );
        assert_eq!(
            format_value(&ConstantValue::Float(f32::INFINITY)),
            "1.0f/0.0f"
        );
        assert_eq!(
            format_value(&ConstantValue::Double(f64::NEG_INFINITY)),
            "-1.0/0.0"
        );
        assert_eq!(format_value(&ConstantValue::Double(1.0)), "1.0");
    }
}
