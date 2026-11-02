//! Adapter for the `java.lang.Character` digit helpers used by the ported
//! `java.math.BigInteger` (the Unicode classification lives in the in-workspace
//! `javac-unicode` crate; this module keeps the exact JDK signatures).

/// Java: public static int Character.digit(char ch, int radix) — the port uses
/// the Unicode code point of the `char`.
///
/// # Arguments
///   - c: the character;
///   - radix: the radix (2..=36).
///
/// # Returns
///   - the digit value, or -1 when the character is not a digit in the radix.
///
/// # Panics
///   - none.
///
/// Upstream: Character.java:public static int digit(char ch, int radix).
pub(crate) fn digit(c: u16, radix: i32) -> i32 {
    javac_unicode::digit(c as i32, radix)
}

/// Java: public static char Character.forDigit(int digit, int radix).
///
/// # Arguments
///   - digit: the digit value;
///   - radix: the radix (2..=36).
///
/// # Returns
///   - the character for the digit (\u0000 when invalid).
///
/// # Panics
///   - none.
///
/// Upstream: Character.java:public static char forDigit(int digit, int radix).
pub(crate) fn for_digit(digit: i32, radix: i32) -> char {
    let c = javac_unicode::for_digit(digit, radix);
    if c == 0 {
        '\u{0}'
    } else {
        char::from_u32(c as u32).unwrap_or('\u{0}')
    }
}
