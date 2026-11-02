//! A collection of utilities for String manipulation.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: Java String arguments/returns are JavaString (UTF-16).
//! REGISTERED(P0): verify case mapping against JDK 17 String.toLowerCase(Locale.US)
//! conditional mapping rules; the initial implementation uses the simple Unicode
//! mapping, which is sufficient for the ASCII inputs used by javac itself.

use crate::util::java_string::JavaString;

/// Converts the given String to lower case using the US Locale. The result is
/// independent of the default Locale in the current JVM instance.
pub fn to_lower_case(source: &JavaString) -> JavaString {
    let mut out: Vec<u16> = Vec::with_capacity(source.len());
    for r in char::decode_utf16(source.as_units().iter().copied()) {
        match r {
            Ok(c) => {
                for lc in c.to_lowercase() {
                    push_utf16(&mut out, lc);
                }
            }
            Err(e) => out.push(e.unpaired_surrogate()),
        }
    }
    JavaString::from_units(out)
}

/// Converts the given String to upper case using the US Locale. The result is
/// independent of the default Locale in the current JVM instance.
pub fn to_upper_case(source: &JavaString) -> JavaString {
    let mut out: Vec<u16> = Vec::with_capacity(source.len());
    for r in char::decode_utf16(source.as_units().iter().copied()) {
        match r {
            Ok(c) => {
                for uc in c.to_uppercase() {
                    push_utf16(&mut out, uc);
                }
            }
            Err(e) => out.push(e.unpaired_surrogate()),
        }
    }
    JavaString::from_units(out)
}

fn push_utf16(out: &mut Vec<u16>, c: char) {
    let mut buf = [0u16; 2];
    let encoded = c.encode_utf16(&mut buf);
    out.extend_from_slice(encoded);
}

/// Case insensitive version of String.indexOf(String).
pub fn index_of_ignore_case(text: &JavaString, s: &JavaString) -> i32 {
    index_of_ignore_case_from(text, s, 0)
}

/// Case insensitive version of String.indexOf(String, int).
///
/// Java implementation:
///   Pattern.compile(Pattern.quote(str), Pattern.CASE_INSENSITIVE).matcher(text)
///          .find(startIndex) ? m.start() : -1
/// Pattern.CASE_INSENSITIVE only maps US-ASCII letters (no UNICODE_CASE flag),
/// so non-ASCII code units are compared exactly.
pub fn index_of_ignore_case_from(text: &JavaString, s: &JavaString, start_index: i32) -> i32 {
    let t = text.as_units();
    let p = s.as_units();
    if start_index < 0 {
        // Java Matcher.find(int) throws IndexOutOfBoundsException for negative start.
        panic!("IndexOutOfBoundsException: startIndex = {}", start_index);
    }
    let start = start_index as usize;
    if start > t.len() {
        panic!("IndexOutOfBoundsException: startIndex = {}", start_index);
    }
    let mut i = start;
    loop {
        if i + p.len() > t.len() {
            break;
        }
        if p.is_empty() {
            return i as i32;
        }
        if units_eq_ignore_case_ascii(&t[i..i + p.len()], p) {
            return i as i32;
        }
        i += 1;
    }
    -1
}

fn units_eq_ignore_case_ascii(a: &[u16], b: &[u16]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (&x, &y) in a.iter().zip(b.iter()) {
        if x < 128 && y < 128 {
            // Pattern.CASE_INSENSITIVE maps US-ASCII letters only.
            if !(x as u8).eq_ignore_ascii_case(&(y as u8)) {
                return false;
            }
        } else if x != y {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    #[test]
    fn lower_upper() {
        assert_eq!(to_lower_case(&js("AbC")).to_string_lossy(), "abc");
        assert_eq!(to_upper_case(&js("AbC")).to_string_lossy(), "ABC");
    }

    #[test]
    fn ignore_case_index() {
        assert_eq!(index_of_ignore_case(&js("HelloWorld"), &js("low")), 3);
        assert_eq!(index_of_ignore_case(&js("HelloWorld"), &js("LOW")), 3);
        assert_eq!(index_of_ignore_case(&js("abc"), &js("z")), -1);
        assert_eq!(index_of_ignore_case_from(&js("abcabc"), &js("abc"), 1), 3);
        assert_eq!(index_of_ignore_case(&js("abc"), &js("")), 0);
    }
}
