// PORTING ADAPTER: JDK 17 java.lang.Character data and algorithms.
//!
//! The data tables in generated.rs are derived from the running JDK 17
//! (java.lang.Character) by tools/gen-unicode/GenUnicode.java via
//! "cargo xtask gen-unicode"; the repository checks in the result so that
//! building rust-javac never needs a JVM.
//!
//! PORTING_RULES: registered adapter (PORTING_RULES section 9). javac calls the static methods of
//! java.lang.Character; the port exposes them as free functions over the generated tables. The
//! observable semantics are those of the JDK 17 java.lang.Character int overloads:
//! - code points outside MIN_CODE_POINT..=MAX_CODE_POINT are not members of any property
//!   (JDK CharacterDataUndefined), so every predicate returns false and case conversion returns
//!   the input unchanged;
//! - Character.digit(cp, radix) == (d36 >= 0 && d36 < radix) ? d36 : -1, with d36 =
//!   Character.digit(cp, 36), and radices outside MIN_RADIX..=MAX_RADIX return -1. The generated
//!   DIGIT_VALUES table holds d36 for every code point with d36 >= 0, and GenUnicode.java
//!   verifies this contract against the running JDK before emitting the table;
//! - Character.forDigit keeps the Java check order: (digit >= radix || digit < 0) first, then
//!   the radix range, then 0x30+digit / 0x61-10+digit, so out-of-range inputs yield 0;
//! - surrogate arithmetic is the literal Java formula; int overflow wraps exactly as in Java.
//!
//! Every expected value in the tests below was produced by running the JDK 17 baseline
//! (Temurin 17.0.19+10) on the fixed probe list embedded in the test data.

pub mod generated;

use generated::{
    DEFINED, DIGIT, DIGIT_VALUES, IDENTIFIER_IGNORABLE, JAVA_ID_PART, JAVA_ID_START, LOWER_CASE,
    SPACE_CHAR, UNICODE_ID_PART, UNICODE_ID_START, UPPER_CASE, WHITESPACE,
};

/// Java: Character.MIN_VALUE == 0x0000 (the NUL char).
pub const MIN_VALUE: u16 = 0x0000;
/// Java: Character.MAX_VALUE == 0xFFFF.
pub const MAX_VALUE: u16 = 0xFFFF;

/// Java: Character.MIN_RADIX.
pub const MIN_RADIX: i32 = 2;
/// Java: Character.MAX_RADIX.
pub const MAX_RADIX: i32 = 36;

/// Java: Character.MIN_CODE_POINT.
pub const MIN_CODE_POINT: i32 = 0x000000;
/// Java: Character.MAX_CODE_POINT.
pub const MAX_CODE_POINT: i32 = 0x10FFFF;
/// Java: Character.MIN_SUPPLEMENTARY_CODE_POINT.
pub const MIN_SUPPLEMENTARY_CODE_POINT: i32 = 0x010000;

/// Java: Character.MIN_HIGH_SURROGATE == 0xD800.
pub const MIN_HIGH_SURROGATE: u16 = 0xD800;
/// Java: Character.MAX_HIGH_SURROGATE == 0xDBFF.
pub const MAX_HIGH_SURROGATE: u16 = 0xDBFF;
/// Java: Character.MIN_LOW_SURROGATE == 0xDC00.
pub const MIN_LOW_SURROGATE: u16 = 0xDC00;
/// Java: Character.MAX_LOW_SURROGATE == 0xDFFF.
pub const MAX_LOW_SURROGATE: u16 = 0xDFFF;
/// Java: Character.MIN_SURROGATE == 0xD800.
pub const MIN_SURROGATE: u16 = MIN_HIGH_SURROGATE;
/// Java: Character.MAX_SURROGATE == 0xDFFF.
pub const MAX_SURROGATE: u16 = MAX_LOW_SURROGATE;

/// Binary search over a sorted, disjoint list of closed ranges.
///
/// Java: the generated property tables only cover MIN_CODE_POINT..=MAX_CODE_POINT, and the JDK
/// returns false (CharacterDataUndefined) for every code point outside that range.
pub fn in_ranges(ranges: &[(u32, u32)], code_point: i32) -> bool {
    if !(MIN_CODE_POINT..=MAX_CODE_POINT).contains(&code_point) {
        return false;
    }
    let code_point = code_point as u32;
    let mut low = 0usize;
    let mut high = ranges.len();
    while low < high {
        let mid = (low + high) / 2;
        let (start, end) = ranges[mid];
        if code_point < start {
            high = mid;
        } else if code_point > end {
            low = mid + 1;
        } else {
            return true;
        }
    }
    false
}

/// Java: Character.isJavaIdentifierStart(int codePoint).
pub fn is_java_identifier_start(code_point: i32) -> bool {
    in_ranges(JAVA_ID_START, code_point)
}

/// Java: Character.isJavaIdentifierPart(int codePoint).
pub fn is_java_identifier_part(code_point: i32) -> bool {
    in_ranges(JAVA_ID_PART, code_point)
}

/// Java: Character.isUnicodeIdentifierStart(int codePoint).
pub fn is_unicode_identifier_start(code_point: i32) -> bool {
    in_ranges(UNICODE_ID_START, code_point)
}

/// Java: Character.isUnicodeIdentifierPart(int codePoint).
pub fn is_unicode_identifier_part(code_point: i32) -> bool {
    in_ranges(UNICODE_ID_PART, code_point)
}

/// Java: Character.isWhitespace(int codePoint).
pub fn is_whitespace(code_point: i32) -> bool {
    in_ranges(WHITESPACE, code_point)
}

/// Java: Character.isSpaceChar(int codePoint).
pub fn is_space_char(code_point: i32) -> bool {
    in_ranges(SPACE_CHAR, code_point)
}

/// Java: Character.isDefined(int codePoint).
pub fn is_defined(code_point: i32) -> bool {
    in_ranges(DEFINED, code_point)
}

/// Java: Character.isDigit(int codePoint).
pub fn is_digit(code_point: i32) -> bool {
    in_ranges(DIGIT, code_point)
}

/// Java: Character.isIdentifierIgnorable(int codePoint).
pub fn is_identifier_ignorable(code_point: i32) -> bool {
    in_ranges(IDENTIFIER_IGNORABLE, code_point)
}

/// Java: Character.isISOControl(int codePoint).
pub fn is_iso_control(code_point: i32) -> bool {
    // Java: (codePoint >= 0x0000 && codePoint <= 0x001F) ||
    //       (codePoint >= 0x007F && codePoint <= 0x009F)
    (0x0000..=0x001F).contains(&code_point) || (0x007F..=0x009F).contains(&code_point)
}

/// Java: Character.digit(int codePoint, int radix).
pub fn digit(code_point: i32, radix: i32) -> i32 {
    if !(MIN_RADIX..=MAX_RADIX).contains(&radix) {
        return -1;
    }
    // PORTING_RULES: GenUnicode.java verifies
    // Character.digit(cp, radix) == (d36 >= 0 && d36 < radix) ? d36 : -1 for every code point and
    // every radix 2..=36 against the running JDK; the generated DIGIT_VALUES table is that d36.
    let digit36 = digit36(code_point);
    if digit36 >= 0 && digit36 < radix {
        digit36
    } else {
        -1
    }
}

/// Java: Character.digit(codePoint, 36); DIGIT_VALUES holds d36 for every code point with
/// d36 >= 0 and is sorted by code point.
fn digit36(code_point: i32) -> i32 {
    if !(MIN_CODE_POINT..=MAX_CODE_POINT).contains(&code_point) {
        return -1;
    }
    match DIGIT_VALUES.binary_search_by_key(&(code_point as u32), |&(cp, _)| cp) {
        Ok(index) => DIGIT_VALUES[index].1 as i32,
        Err(_) => -1,
    }
}

/// Java: Character.forDigit(int digit, int radix).
pub fn for_digit(digit: i32, radix: i32) -> u16 {
    // Java: if ((digit >= radix) || (digit < 0)) return '\0';
    if digit >= radix || digit < 0 {
        return 0;
    }
    if !(MIN_RADIX..=MAX_RADIX).contains(&radix) {
        return 0;
    }
    if digit < 10 {
        // Java: return (char)('0' + digit);
        0x30 + digit as u16
    } else {
        // Java: return (char)('a' - 10 + digit);
        0x61 - 10 + digit as u16
    }
}

/// Java: Character.toLowerCase(int codePoint).
pub fn to_lower_case(code_point: i32) -> i32 {
    case_map(LOWER_CASE, code_point)
}

/// Java: Character.toUpperCase(int codePoint).
pub fn to_upper_case(code_point: i32) -> i32 {
    case_map(UPPER_CASE, code_point)
}

/// Java: CharacterDataUndefined.toLowerCase/toUpperCase returns the input; the generated maps
/// hold exactly the code points whose JDK 17 mapping differs from the input.
fn case_map(map: &[(u32, u32)], code_point: i32) -> i32 {
    if !(MIN_CODE_POINT..=MAX_CODE_POINT).contains(&code_point) {
        return code_point;
    }
    match map.binary_search_by_key(&(code_point as u32), |&(from, _)| from) {
        Ok(index) => map[index].1 as i32,
        Err(_) => code_point,
    }
}

/// Java: Character.charCount(int codePoint).
pub fn char_count(code_point: i32) -> i32 {
    // Java: return codePoint >= MIN_SUPPLEMENTARY_CODE_POINT ? 2 : 1;
    // (no validity check, exactly like Java)
    if code_point >= MIN_SUPPLEMENTARY_CODE_POINT {
        2
    } else {
        1
    }
}

/// Java: Character.highSurrogate(int codePoint).
pub fn high_surrogate(code_point: i32) -> u16 {
    // Java: return (char) ((codePoint >>> 10)
    //     + (MIN_HIGH_SURROGATE - (MIN_SUPPLEMENTARY_CODE_POINT >>> 10)));
    ((code_point as u32 >> 10)
        + (MIN_HIGH_SURROGATE as u32 - (MIN_SUPPLEMENTARY_CODE_POINT as u32 >> 10)))
        as u16
}

/// Java: Character.lowSurrogate(int codePoint).
pub fn low_surrogate(code_point: i32) -> u16 {
    // Java: return (char) ((codePoint & 0x3ff) + MIN_LOW_SURROGATE);
    ((code_point & 0x3FF) + MIN_LOW_SURROGATE as i32) as u16
}

/// Java: Character.toCodePoint(char high, char low).
pub fn to_code_point(high: u16, low: u16) -> i32 {
    // Java: return ((high << 10) + low) + (MIN_SUPPLEMENTARY_CODE_POINT
    //     - (MIN_HIGH_SURROGATE << 10)
    //     - MIN_LOW_SURROGATE);
    ((high as i32) << 10)
        + (low as i32)
        + (MIN_SUPPLEMENTARY_CODE_POINT
            - ((MIN_HIGH_SURROGATE as i32) << 10)
            - (MIN_LOW_SURROGATE as i32))
}

/// Java: Character.isHighSurrogate(char ch).
pub fn is_high_surrogate(ch: u16) -> bool {
    (MIN_HIGH_SURROGATE..=MAX_HIGH_SURROGATE).contains(&ch)
}

/// Java: Character.isLowSurrogate(char ch).
pub fn is_low_surrogate(ch: u16) -> bool {
    (MIN_LOW_SURROGATE..=MAX_LOW_SURROGATE).contains(&ch)
}

/// Java: Character.isBmpCodePoint(int codePoint).
fn is_bmp_code_point(code_point: i32) -> bool {
    (code_point as u32) >> 16 == 0
}

/// Java: Character.isValidCodePoint(int codePoint).
fn is_valid_code_point(code_point: i32) -> bool {
    (code_point as u32) >> 16 < 0x11
}

/// Java: Character.toChars(int codePoint).
pub fn to_chars(code_point: i32) -> Vec<u16> {
    if is_bmp_code_point(code_point) {
        vec![code_point as u16]
    } else if is_valid_code_point(code_point) {
        vec![high_surrogate(code_point), low_surrogate(code_point)]
    } else {
        // Java: throw new IllegalArgumentException();
        panic!("java.lang.IllegalArgumentException");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{
        DEFINED, DIGIT, DIGIT_VALUES, IDENTIFIER_IGNORABLE, JAVA_ID_PART, JAVA_ID_START, LOWER_CASE,
        SPACE_CHAR, UNICODE_ID_PART, UNICODE_ID_START, UPPER_CASE, WHITESPACE,
    };

    /// Produced by running the JDK 17 baseline (Temurin 17.0.19+10) over the fixed probe list.
    /// Sections: ATTR (character properties), DIGIT, FORDIGIT, SUR, TOCODEPOINT, SURFLAG,
    /// TOCHARS, TOCHARSERR.
    const JDK17_EXPECTED: &str = r#"#ATTR
-5 0 0 0 0 0 0 0 0 0 0 1 -1 -1 -1 -5 -5
-1 0 0 0 0 0 0 0 0 0 0 1 -1 -1 -1 -1 -1
0 0 1 0 1 0 0 1 0 1 1 1 -1 -1 -1 0 0
9 0 0 0 0 1 0 1 0 0 1 1 -1 -1 -1 9 9
13 0 0 0 0 1 0 1 0 0 1 1 -1 -1 -1 13 13
28 0 0 0 0 1 0 1 0 0 1 1 -1 -1 -1 28 28
31 0 0 0 0 1 0 1 0 0 1 1 -1 -1 -1 31 31
32 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 32 32
36 1 1 0 0 0 0 1 0 0 0 1 -1 -1 -1 36 36
48 0 1 0 1 0 0 1 1 0 0 1 0 0 0 48 48
57 0 1 0 1 0 0 1 1 0 0 1 9 9 9 57 57
65 1 1 1 1 0 0 1 0 0 0 1 -1 10 10 97 65
90 1 1 1 1 0 0 1 0 0 0 1 -1 35 -1 122 90
95 1 1 0 1 0 0 1 0 0 0 1 -1 -1 -1 95 95
97 1 1 1 1 0 0 1 0 0 0 1 -1 10 10 97 65
122 1 1 1 1 0 0 1 0 0 0 1 -1 35 -1 122 90
127 0 1 0 1 0 0 1 0 1 1 1 -1 -1 -1 127 127
128 0 1 0 1 0 0 1 0 1 1 1 -1 -1 -1 128 128
159 0 1 0 1 0 0 1 0 1 1 1 -1 -1 -1 159 159
160 0 0 0 0 0 1 1 0 0 0 1 -1 -1 -1 160 160
170 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 170 170
181 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 181 924
186 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 186 186
192 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 224 192
214 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 246 214
216 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 248 216
246 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 246 214
248 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 248 216
304 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 105 304
305 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 305 73
705 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 705 705
710 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 710 710
721 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 721 721
736 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 736 736
768 0 1 0 1 0 0 1 0 0 0 1 -1 -1 -1 768 768
880 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 881 880
884 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 884 884
886 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 887 886
887 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 887 886
890 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 890 890
1632 0 1 0 1 0 0 1 1 0 0 1 0 0 0 1632 1632
1633 0 1 0 1 0 0 1 1 0 0 1 1 1 1 1633 1633
1641 0 1 0 1 0 0 1 1 0 0 1 9 9 9 1641 1641
1642 0 0 0 0 0 0 1 0 0 0 1 -1 -1 -1 1642 1642
5760 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 5760 5760
8192 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 8192 8192
8198 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 8198 8198
8199 0 0 0 0 0 1 1 0 0 0 1 -1 -1 -1 8199 8199
8200 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 8200 8200
8202 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 8202 8202
8203 0 1 0 1 0 0 1 0 1 0 1 -1 -1 -1 8203 8203
8232 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 8232 8232
8233 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 8233 8233
8287 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 8287 8287
8486 1 1 1 1 0 0 1 0 0 0 1 -1 -1 -1 969 8486
12288 0 0 0 0 1 1 1 0 0 0 1 -1 -1 -1 12288 12288
55295 0 0 0 0 0 0 0 0 0 0 1 -1 -1 -1 55295 55295
55296 0 0 0 0 0 0 1 0 0 0 1 -1 -1 -1 55296 55296
56319 0 0 0 0 0 0 1 0 0 0 1 -1 -1 -1 56319 56319
56320 0 0 0 0 0 0 1 0 0 0 1 -1 -1 -1 56320 56320
57343 0 0 0 0 0 0 1 0 0 0 1 -1 -1 -1 57343 57343
57344 0 0 0 0 0 0 1 0 0 0 1 -1 -1 -1 57344 57344
65296 0 1 0 1 0 0 1 1 0 0 1 0 0 0 65296 65296
65313 1 1 1 1 0 0 1 0 0 0 1 -1 10 10 65345 65313
65533 0 0 0 0 0 0 1 0 0 0 1 -1 -1 -1 65533 65533
65535 0 0 0 0 0 0 0 0 0 0 1 -1 -1 -1 65535 65535
65536 1 1 1 1 0 0 1 0 0 0 2 -1 -1 -1 65536 65536
66560 1 1 1 1 0 0 1 0 0 0 2 -1 -1 -1 66600 66560
66600 1 1 1 1 0 0 1 0 0 0 2 -1 -1 -1 66600 66560
119808 1 1 1 1 0 0 1 0 0 0 2 -1 -1 -1 119808 119808
128512 0 0 0 0 0 0 1 0 0 0 2 -1 -1 -1 128512 128512
1114109 0 0 0 0 0 0 1 0 0 0 2 -1 -1 -1 1114109 1114109
1114111 0 0 0 0 0 0 0 0 0 0 2 -1 -1 -1 1114111 1114111
1114112 0 0 0 0 0 0 0 0 0 0 2 -1 -1 -1 1114112 1114112
#DIGIT
48 -1 -1
49 -1 -1
57 -1 -1
65 -1 -1
90 -1 -1
97 -1 -1
122 -1 -1
1632 -1 -1
1633 -1 -1
1641 -1 -1
1642 -1 -1
120792 -1 -1
65296 -1 -1
65313 -1 -1
-1 -1 -1
1114112 -1 -1
48 0 -1
49 0 -1
57 0 -1
65 0 -1
90 0 -1
97 0 -1
122 0 -1
1632 0 -1
1633 0 -1
1641 0 -1
1642 0 -1
120792 0 -1
65296 0 -1
65313 0 -1
-1 0 -1
1114112 0 -1
48 1 -1
49 1 -1
57 1 -1
65 1 -1
90 1 -1
97 1 -1
122 1 -1
1632 1 -1
1633 1 -1
1641 1 -1
1642 1 -1
120792 1 -1
65296 1 -1
65313 1 -1
-1 1 -1
1114112 1 -1
48 2 0
49 2 1
57 2 -1
65 2 -1
90 2 -1
97 2 -1
122 2 -1
1632 2 0
1633 2 1
1641 2 -1
1642 2 -1
120792 2 0
65296 2 0
65313 2 -1
-1 2 -1
1114112 2 -1
48 3 0
49 3 1
57 3 -1
65 3 -1
90 3 -1
97 3 -1
122 3 -1
1632 3 0
1633 3 1
1641 3 -1
1642 3 -1
120792 3 0
65296 3 0
65313 3 -1
-1 3 -1
1114112 3 -1
48 9 0
49 9 1
57 9 -1
65 9 -1
90 9 -1
97 9 -1
122 9 -1
1632 9 0
1633 9 1
1641 9 -1
1642 9 -1
120792 9 0
65296 9 0
65313 9 -1
-1 9 -1
1114112 9 -1
48 10 0
49 10 1
57 10 9
65 10 -1
90 10 -1
97 10 -1
122 10 -1
1632 10 0
1633 10 1
1641 10 9
1642 10 -1
120792 10 0
65296 10 0
65313 10 -1
-1 10 -1
1114112 10 -1
48 11 0
49 11 1
57 11 9
65 11 10
90 11 -1
97 11 10
122 11 -1
1632 11 0
1633 11 1
1641 11 9
1642 11 -1
120792 11 0
65296 11 0
65313 11 10
-1 11 -1
1114112 11 -1
48 16 0
49 16 1
57 16 9
65 16 10
90 16 -1
97 16 10
122 16 -1
1632 16 0
1633 16 1
1641 16 9
1642 16 -1
120792 16 0
65296 16 0
65313 16 10
-1 16 -1
1114112 16 -1
48 35 0
49 35 1
57 35 9
65 35 10
90 35 -1
97 35 10
122 35 -1
1632 35 0
1633 35 1
1641 35 9
1642 35 -1
120792 35 0
65296 35 0
65313 35 10
-1 35 -1
1114112 35 -1
48 36 0
49 36 1
57 36 9
65 36 10
90 36 35
97 36 10
122 36 35
1632 36 0
1633 36 1
1641 36 9
1642 36 -1
120792 36 0
65296 36 0
65313 36 10
-1 36 -1
1114112 36 -1
48 37 -1
49 37 -1
57 37 -1
65 37 -1
90 37 -1
97 37 -1
122 37 -1
1632 37 -1
1633 37 -1
1641 37 -1
1642 37 -1
120792 37 -1
65296 37 -1
65313 37 -1
-1 37 -1
1114112 37 -1
48 40 -1
49 40 -1
57 40 -1
65 40 -1
90 40 -1
97 40 -1
122 40 -1
1632 40 -1
1633 40 -1
1641 40 -1
1642 40 -1
120792 40 -1
65296 40 -1
65313 40 -1
-1 40 -1
1114112 40 -1
#FORDIGIT
-2 -1 0
-1 -1 0
0 -1 0
1 -1 0
5 -1 0
9 -1 0
10 -1 0
11 -1 0
35 -1 0
36 -1 0
40 -1 0
-2 0 0
-1 0 0
0 0 0
1 0 0
5 0 0
9 0 0
10 0 0
11 0 0
35 0 0
36 0 0
40 0 0
-2 1 0
-1 1 0
0 1 0
1 1 0
5 1 0
9 1 0
10 1 0
11 1 0
35 1 0
36 1 0
40 1 0
-2 2 0
-1 2 0
0 2 48
1 2 49
5 2 0
9 2 0
10 2 0
11 2 0
35 2 0
36 2 0
40 2 0
-2 3 0
-1 3 0
0 3 48
1 3 49
5 3 0
9 3 0
10 3 0
11 3 0
35 3 0
36 3 0
40 3 0
-2 10 0
-1 10 0
0 10 48
1 10 49
5 10 53
9 10 57
10 10 0
11 10 0
35 10 0
36 10 0
40 10 0
-2 11 0
-1 11 0
0 11 48
1 11 49
5 11 53
9 11 57
10 11 97
11 11 0
35 11 0
36 11 0
40 11 0
-2 16 0
-1 16 0
0 16 48
1 16 49
5 16 53
9 16 57
10 16 97
11 16 98
35 16 0
36 16 0
40 16 0
-2 35 0
-1 35 0
0 35 48
1 35 49
5 35 53
9 35 57
10 35 97
11 35 98
35 35 0
36 35 0
40 35 0
-2 36 0
-1 36 0
0 36 48
1 36 49
5 36 53
9 36 57
10 36 97
11 36 98
35 36 122
36 36 0
40 36 0
-2 37 0
-1 37 0
0 37 0
1 37 0
5 37 0
9 37 0
10 37 0
11 37 0
35 37 0
36 37 0
40 37 0
-2 40 0
-1 40 0
0 40 0
1 40 0
5 40 0
9 40 0
10 40 0
11 40 0
35 40 0
36 40 0
40 40 0
#SUR
-1 55231 57343 1
0 55232 56320 1
65 55232 56385 1
55295 55285 57343 1
55296 55286 56320 1
56319 55286 57343 1
56320 55287 56320 1
57343 55287 57343 1
57344 55288 56320 1
65535 55295 57343 1
65536 55296 56320 2
128512 55357 56832 2
1114111 56319 57343 2
1114112 56320 56320 2
#TOCODEPOINT
55357 56832 128512
55296 56320 65536
56319 57343 1114111
65 66 -56547262
#SURFLAG
65 0 0
55295 0 0
55296 1 0
56319 1 0
56320 0 1
57343 0 1
57344 0 0
65535 0 0
#TOCHARS
65 1 65
55296 1 55296
65535 1 65535
65536 2 55296 56320
128512 2 55357 56832
1114111 2 56319 57343
#TOCHARSERR
-1 1
1114112 1
-100 1
#END"#;

    fn fields(line: &str) -> Vec<i64> {
        line.split_whitespace()
            .map(|v| v.parse::<i64>().expect("integer field"))
            .collect()
    }

    #[test]
    fn matches_jdk17_character() {
        let mut section = "";
        for line in JDK17_EXPECTED.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('#') {
                section = name;
                continue;
            }
            if section == "END" {
                break;
            }
            let f = fields(line);
            match section {
                "ATTR" => {
                    let cp = f[0] as i32;
                    assert_eq!(is_java_identifier_start(cp), f[1] == 1, "isJavaIdentifierStart {}", cp);
                    assert_eq!(is_java_identifier_part(cp), f[2] == 1, "isJavaIdentifierPart {}", cp);
                    assert_eq!(is_unicode_identifier_start(cp), f[3] == 1, "isUnicodeIdentifierStart {}", cp);
                    assert_eq!(is_unicode_identifier_part(cp), f[4] == 1, "isUnicodeIdentifierPart {}", cp);
                    assert_eq!(is_whitespace(cp), f[5] == 1, "isWhitespace {}", cp);
                    assert_eq!(is_space_char(cp), f[6] == 1, "isSpaceChar {}", cp);
                    assert_eq!(is_defined(cp), f[7] == 1, "isDefined {}", cp);
                    assert_eq!(is_digit(cp), f[8] == 1, "isDigit {}", cp);
                    assert_eq!(is_identifier_ignorable(cp), f[9] == 1, "isIdentifierIgnorable {}", cp);
                    assert_eq!(is_iso_control(cp), f[10] == 1, "isISOControl {}", cp);
                    assert_eq!(char_count(cp) as i64, f[11], "charCount {}", cp);
                    assert_eq!(digit(cp, 10) as i64, f[12], "digit({}, 10)", cp);
                    assert_eq!(digit(cp, 36) as i64, f[13], "digit({}, 36)", cp);
                    assert_eq!(digit(cp, 16) as i64, f[14], "digit({}, 16)", cp);
                    assert_eq!(to_lower_case(cp) as i64, f[15], "toLowerCase {}", cp);
                    assert_eq!(to_upper_case(cp) as i64, f[16], "toUpperCase {}", cp);
                }
                "DIGIT" => {
                    let (cp, radix, expected) = (f[0] as i32, f[1] as i32, f[2] as i32);
                    assert_eq!(digit(cp, radix), expected, "digit({}, {})", cp, radix);
                }
                "FORDIGIT" => {
                    let (d, radix, expected) = (f[0] as i32, f[1] as i32, f[2] as u16);
                    assert_eq!(for_digit(d, radix), expected, "forDigit({}, {})", d, radix);
                }
                "SUR" => {
                    let cp = f[0] as i32;
                    assert_eq!(high_surrogate(cp) as i64, f[1], "highSurrogate {}", cp);
                    assert_eq!(low_surrogate(cp) as i64, f[2], "lowSurrogate {}", cp);
                    assert_eq!(char_count(cp) as i64, f[3], "charCount {}", cp);
                }
                "TOCODEPOINT" => {
                    let (high, low, expected) = (f[0] as u16, f[1] as u16, f[2] as i32);
                    assert_eq!(to_code_point(high, low), expected, "toCodePoint({}, {})", high, low);
                }
                "SURFLAG" => {
                    let (ch, high, low) = (f[0] as u16, f[1] == 1, f[2] == 1);
                    assert_eq!(is_high_surrogate(ch), high, "isHighSurrogate {}", ch);
                    assert_eq!(is_low_surrogate(ch), low, "isLowSurrogate {}", ch);
                }
                "TOCHARS" => {
                    let cp = f[0] as i32;
                    let mut expected = Vec::new();
                    for v in &f[2..] {
                        expected.push(*v as u16);
                    }
                    assert_eq!(expected.len() as i64, f[1], "toChars length {}", cp);
                    assert_eq!(to_chars(cp), expected, "toChars {}", cp);
                }
                "TOCHARSERR" => {
                    let cp = f[0] as i32;
                    assert_eq!(f[1], 1, "expected IllegalArgumentException for {}", cp);
                    let result = std::panic::catch_unwind(|| to_chars(cp));
                    assert!(result.is_err(), "toChars({}) must panic", cp);
                }
                other => panic!("unknown expectation section {}", other),
            }
        }
    }

    fn check_table(name: &str, table: &[(u32, u32)]) {
        assert!(!table.is_empty(), "{} is empty", name);
        for pair in table.windows(2) {
            assert!(
                pair[0].1 < pair[1].0,
                "{} must be sorted and disjoint around {:?}",
                name,
                pair
            );
        }
        for &(start, end) in table {
            assert!(in_ranges(table, start as i32), "{} start {}", name, start);
            assert!(in_ranges(table, end as i32), "{} end {}", name, end);
            if start > 0 && !in_ranges(table, start as i32 - 1) {
                // fine
            } else if start > 0 {
                panic!("{} contains {} just before range start", name, start - 1);
            }
            if end < MAX_CODE_POINT as u32 && in_ranges(table, end as i32 + 1) {
                panic!("{} contains {} just after range end", name, end + 1);
            }
        }
        assert!(!in_ranges(table, -1), "{} must not contain -1", name);
        assert!(!in_ranges(table, MAX_CODE_POINT + 1), "{} must not contain 0x110000", name);
        assert!(!in_ranges(table, i32::MIN), "{} must not contain i32::MIN", name);
    }

    #[test]
    fn property_tables_are_consistent_with_binary_search() {
        check_table("JAVA_ID_START", JAVA_ID_START);
        check_table("JAVA_ID_PART", JAVA_ID_PART);
        check_table("UNICODE_ID_START", UNICODE_ID_START);
        check_table("UNICODE_ID_PART", UNICODE_ID_PART);
        check_table("WHITESPACE", WHITESPACE);
        check_table("SPACE_CHAR", SPACE_CHAR);
        check_table("DEFINED", DEFINED);
        check_table("DIGIT", DIGIT);
        check_table("IDENTIFIER_IGNORABLE", IDENTIFIER_IGNORABLE);
    }

    #[test]
    fn digit_contract_holds_for_generated_table() {
        for &(cp, d36) in DIGIT_VALUES {
            let cp = cp as i32;
            let d36 = d36 as i32;
            for radix in MIN_RADIX..=MAX_RADIX {
                let expected = if d36 < radix { d36 } else { -1 };
                assert_eq!(digit(cp, radix), expected, "digit({}, {})", cp, radix);
            }
            assert_eq!(digit(cp, 1), -1, "radix below MIN_RADIX for {}", cp);
            assert_eq!(digit(cp, MAX_RADIX + 1), -1, "radix above MAX_RADIX for {}", cp);
        }
        assert_eq!(digit(-1, 36), -1);
        assert_eq!(digit(MAX_CODE_POINT + 1, 36), -1);
        assert_eq!(digit(MAX_CODE_POINT + 1, 2), -1);
    }

    #[test]
    fn digit_table_is_sorted_and_unique() {
        for pair in DIGIT_VALUES.windows(2) {
            assert!(pair[0].0 < pair[1].0, "DIGIT_VALUES must be sorted");
        }
        for pair in LOWER_CASE.windows(2) {
            assert!(pair[0].0 < pair[1].0, "LOWER_CASE must be sorted");
        }
        for pair in UPPER_CASE.windows(2) {
            assert!(pair[0].0 < pair[1].0, "UPPER_CASE must be sorted");
        }
    }

    #[test]
    fn case_maps_are_consistent() {
        for &(from, to) in LOWER_CASE {
            assert_eq!(to_lower_case(from as i32), to as i32, "toLowerCase {}", from);
            assert_ne!(from, to);
        }
        for &(from, to) in UPPER_CASE {
            assert_eq!(to_upper_case(from as i32), to as i32, "toUpperCase {}", from);
            assert_ne!(from, to);
        }
        // code points without a mapping (and invalid ones) are returned unchanged
        assert_eq!(to_lower_case(0x20), 0x20);
        assert_eq!(to_upper_case(0x20), 0x20);
        assert_eq!(to_lower_case(-5), -5);
        assert_eq!(to_upper_case(-5), -5);
        assert_eq!(to_lower_case(MAX_CODE_POINT + 1), MAX_CODE_POINT + 1);
        assert_eq!(to_upper_case(MAX_CODE_POINT + 1), MAX_CODE_POINT + 1);
    }

    #[test]
    fn constants_match_java() {
        assert_eq!(MIN_VALUE, 0);
        assert_eq!(MAX_VALUE, 0xFFFF);
        assert_eq!(MIN_RADIX, 2);
        assert_eq!(MAX_RADIX, 36);
        assert_eq!(MIN_CODE_POINT, 0);
        assert_eq!(MAX_CODE_POINT, 0x10FFFF);
        assert_eq!(MIN_SUPPLEMENTARY_CODE_POINT, 0x10000);
        assert_eq!((MIN_HIGH_SURROGATE, MAX_HIGH_SURROGATE), (0xD800, 0xDBFF));
        assert_eq!((MIN_LOW_SURROGATE, MAX_LOW_SURROGATE), (0xDC00, 0xDFFF));
        assert_eq!(MIN_SURROGATE, 0xD800);
        assert_eq!(MAX_SURROGATE, 0xDFFF);
    }

    #[test]
    fn predicates_reject_invalid_code_points() {
        assert!(!is_java_identifier_start(-5));
        assert!(!is_java_identifier_part(-5));
        assert!(!is_unicode_identifier_start(i32::MIN));
        assert!(!is_unicode_identifier_part(i32::MAX));
        assert!(!is_whitespace(-1));
        assert!(!is_space_char(MAX_CODE_POINT + 1));
        assert!(!is_defined(-1));
        assert!(!is_defined(MAX_CODE_POINT + 1));
        assert!(!is_digit(-1));
        assert!(!is_identifier_ignorable(MAX_CODE_POINT + 1));
        assert!(!is_iso_control(i32::MIN));
        assert!(!is_iso_control(i32::MAX));
    }

    #[test]
    fn ascii_baselines() {
        assert!(is_java_identifier_start('a' as i32));
        assert!(is_java_identifier_start('$' as i32));
        assert!(is_java_identifier_start('_' as i32));
        assert!(!is_java_identifier_start('1' as i32));
        assert!(is_java_identifier_part('1' as i32));
        assert!(is_whitespace(' ' as i32));
        assert!(is_whitespace('\t' as i32));
        assert!(!is_whitespace(0x00A0)); // NBSP is a space char but not whitespace
        assert!(is_space_char(0x00A0));
        assert_eq!(digit('0' as i32, 10), 0);
        assert_eq!(digit('9' as i32, 10), 9);
        assert_eq!(digit('a' as i32, 16), 10);
        assert_eq!(digit('B' as i32, 16), 11);
        assert_eq!(digit('a' as i32, 10), -1);
        assert_eq!(for_digit(0, 10), '0' as u16);
        assert_eq!(for_digit(35, 36), 'z' as u16);
        assert_eq!(for_digit(10, 10), 0);
        assert_eq!(for_digit(-1, 10), 0);
        assert_eq!(char_count(0xFFFF), 1);
        assert_eq!(char_count(0x10000), 2);
        assert_eq!(high_surrogate(0x1F600), 0xD83D);
        assert_eq!(low_surrogate(0x1F600), 0xDE00);
        assert_eq!(to_code_point(0xD83D, 0xDE00), 0x1F600);
        assert!(is_high_surrogate(0xD800));
        assert!(is_high_surrogate(0xDBFF));
        assert!(!is_high_surrogate(0xDC00));
        assert!(is_low_surrogate(0xDFFF));
        assert_eq!(to_chars(0x1F600), vec![0xD83D, 0xDE00]);
        assert_eq!(to_chars(0x41), vec![0x41]);
    }
}
