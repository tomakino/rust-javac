//! Utility class for static conversion methods between numbers and strings in
//! various formats.
//!
//! <p>Note regarding UTF-8: the JVMS defines its own version of the UTF-8 format
//! so that it contains no zero bytes (modified UTF-8). This is not actually the
//! same as Charset.forName("UTF-8"). The methods here support modified UTF-8.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! Java overloads map to distinct Rust function names
//! (utf2chars / utf2chars_into / utf2chars_range, chars2utf / chars2utf_into /
//!  chars2utf_range, quote / quote_char, short_name / short_name_str, ...).
//! REGISTERED(P1): Unicode digits in Character.digit; the parser only feeds ASCII
//! literals, but command line parsing may accept Unicode digits in javac.

use crate::util::java_string::JavaString;
use crate::util::name::Name;

/// Java: NumberFormatException marker for string2int/string2long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberFormatException;

/// Convert string to integer.
pub fn string2int(s: &JavaString, radix: i32) -> Result<i32, NumberFormatException> {
    if radix == 10 {
        return parse_int(s, radix);
    }
    let cs = s.as_units();
    let limit = i32::MAX / (radix / 2);
    let mut n: i32 = 0;
    for &c in cs {
        let d = digit(c, radix);
        if n < 0 || n > limit || n.wrapping_mul(radix) > i32::MAX.wrapping_sub(d) {
            return Err(NumberFormatException);
        }
        // Java int arithmetic wraps here; hexadecimal literals such as
        // 0xFFFFFFFF rely on the wrap to produce -1.
        n = n.wrapping_mul(radix).wrapping_add(d);
    }
    Ok(n)
}

/// Convert string to long integer.
pub fn string2long(s: &JavaString, radix: i32) -> Result<i64, NumberFormatException> {
    if radix == 10 {
        return parse_long(s, radix);
    }
    let cs = s.as_units();
    // Java: Convert.string2long (jdk.compiler) uses limit = MAX/(radix/2) and
    // lets n * radix wrap like Java's long arithmetic, so hexadecimal literals
    // such as 0xFFFF_FFFF_FFFF_FFFFL decode to -1 instead of failing.
    let limit = i64::MAX / (radix as i64 / 2);
    let mut n: i64 = 0;
    for &c in cs {
        let d = digit(c, radix) as i64;
        if n < 0 || n > limit || n.wrapping_mul(radix as i64) > i64::MAX.wrapping_sub(d) {
            return Err(NumberFormatException);
        }
        n = n.wrapping_mul(radix as i64).wrapping_add(d);
    }
    Ok(n)
}

/// Java: Integer.parseInt(s, radix) — ported from java.lang.Integer#parseInt.
fn parse_int(s: &JavaString, radix: i32) -> Result<i32, NumberFormatException> {
    let cs = s.as_units();
    if cs.is_empty() {
        return Err(NumberFormatException);
    }
    let mut i = 0usize;
    let negative = cs[0] == b'-' as u16;
    if negative || cs[0] == b'+' as u16 {
        i = 1;
        if cs.len() == 1 {
            return Err(NumberFormatException);
        }
    }
    let limit = if negative { i32::MIN } else { -i32::MAX };
    let multmin = limit / radix;
    let mut result: i32 = 0;
    while i < cs.len() {
        let d = digit(cs[i], radix);
        if d < 0 || result < multmin {
            return Err(NumberFormatException);
        }
        result *= radix;
        if result < limit + d {
            return Err(NumberFormatException);
        }
        result -= d;
        i += 1;
    }
    Ok(if negative { result } else { -result })
}

/// Java: Long.parseLong(s, radix) — ported from java.lang.Long#parseLong.
fn parse_long(s: &JavaString, radix: i32) -> Result<i64, NumberFormatException> {
    let cs = s.as_units();
    if cs.is_empty() {
        return Err(NumberFormatException);
    }
    let mut i = 0usize;
    let negative = cs[0] == b'-' as u16;
    if negative || cs[0] == b'+' as u16 {
        i = 1;
        if cs.len() == 1 {
            return Err(NumberFormatException);
        }
    }
    let limit = if negative { i64::MIN } else { -i64::MAX };
    let multmin = limit / radix as i64;
    let mut result: i64 = 0;
    while i < cs.len() {
        let d = digit(cs[i], radix) as i64;
        if d < 0 || result < multmin {
            return Err(NumberFormatException);
        }
        result *= radix as i64;
        if result < limit + d {
            return Err(NumberFormatException);
        }
        result -= d;
        i += 1;
    }
    Ok(if negative { result } else { -result })
}

/// Java: Character.digit(char, radix); ASCII-only for now (see REGISTERED above).
fn digit(c: u16, radix: i32) -> i32 {
    if (c >= b'0' as u16) && (c <= b'9' as u16) {
        let d = (c - b'0' as u16) as i32;
        if d < radix {
            return d;
        }
        return -1;
    }
    if (c >= b'a' as u16) && (c <= b'z' as u16) {
        let d = (c - b'a' as u16) as i32 + 10;
        if d < radix {
            return d;
        }
        return -1;
    }
    if (c >= b'A' as u16) && (c <= b'Z' as u16) {
        let d = (c - b'A' as u16) as i32 + 10;
        if d < radix {
            return d;
        }
        return -1;
    }
    -1
}

// ------------------------------------------------------------------------
// Conversion routines between names, strings, and byte arrays in Utf8 format
// ------------------------------------------------------------------------

/// Convert len bytes from utf8 to characters.
/// Parameters are as in System.arraycopy. Return first index in dst past the
/// last copied char.
pub fn utf2chars_into(src: &[u8], sindex: i32, dst: &mut [u16], dindex: i32, len: i32) -> i32 {
    let mut i = sindex as usize;
    let limit = (sindex + len) as usize;
    let mut j = dindex as usize;
    while i < limit {
        // ASCII runs are copied in one tight loop; the branchy decoder below
        // only runs for the (rare) non-ASCII characters.
        let run_start = i;
        while i < limit && src[i] < 0x80 {
            i += 1;
        }
        if i > run_start {
            let slots = &mut dst[j..j + (i - run_start)];
            for (slot, &byte) in slots.iter_mut().zip(&src[run_start..i]) {
                *slot = byte as u16;
            }
            j += i - run_start;
        }
        if i >= limit {
            break;
        }
        let mut b = src[i] as i32 & 0xFF;
        i += 1;
        if b >= 0xE0 {
            b = (b & 0x0F) << 12;
            b |= (src[i] as i32 & 0x3F) << 6;
            i += 1;
            b |= src[i] as i32 & 0x3F;
            i += 1;
        } else if b >= 0xC0 {
            b = (b & 0x1F) << 6;
            b |= src[i] as i32 & 0x3F;
            i += 1;
        }
        dst[j] = b as u16;
        j += 1;
    }
    j as i32
}

/// Return bytes in Utf8 representation as an array of characters.
pub fn utf2chars_range(src: &[u8], sindex: i32, len: i32) -> Vec<u16> {
    let mut i = sindex as usize;
    let limit = (sindex + len) as usize;
    let mut out: Vec<u16> = Vec::with_capacity(len as usize);
    while i < limit {
        let run_start = i;
        while i < limit && src[i] < 0x80 {
            i += 1;
        }
        if i > run_start {
            out.extend(src[run_start..i].iter().map(|&byte| byte as u16));
        }
        if i >= limit {
            break;
        }
        let mut b = src[i] as i32 & 0xFF;
        i += 1;
        if b >= 0xE0 {
            b = (b & 0x0F) << 12;
            b |= (src[i] as i32 & 0x3F) << 6;
            i += 1;
            b |= src[i] as i32 & 0x3F;
            i += 1;
        } else if b >= 0xC0 {
            b = (b & 0x1F) << 6;
            b |= src[i] as i32 & 0x3F;
            i += 1;
        }
        out.push(b as u16);
    }
    out
}

/// Return all bytes of a given array in Utf8 representation as an array of characters.
pub fn utf2chars(src: &[u8]) -> Vec<u16> {
    utf2chars_range(src, 0, src.len() as i32)
}

/// Return bytes in Utf8 representation as a string.
pub fn utf2string_range(src: &[u8], sindex: i32, len: i32) -> JavaString {
    JavaString::from_units(utf2chars_range(src, sindex, len))
}

/// Return all bytes of a given array in Utf8 representation as a string.
pub fn utf2string(src: &[u8]) -> JavaString {
    utf2string_range(src, 0, src.len() as i32)
}

/// Copy characters in source array to bytes in target array, converting them to
/// Utf8 representation. The target array must be large enough. Returns first index
/// in dst past the last copied byte.
pub fn chars2utf_into(src: &[u16], sindex: i32, dst: &mut [u8], dindex: i32, len: i32) -> i32 {
    let mut j = dindex;
    let limit = sindex + len;
    let mut i = sindex;
    while i < limit {
        let ch = src[i as usize];
        if (1..=0x7F).contains(&ch) {
            dst[j as usize] = ch as u8;
            j += 1;
        } else if ch <= 0x7FF {
            dst[j as usize] = 0xC0 | ((ch >> 6) as u8);
            dst[j as usize + 1] = 0x80 | ((ch & 0x3F) as u8);
            j += 2;
        } else {
            dst[j as usize] = 0xE0 | ((ch >> 12) as u8);
            dst[j as usize + 1] = 0x80 | (((ch >> 6) & 0x3F) as u8);
            dst[j as usize + 2] = 0x80 | ((ch & 0x3F) as u8);
            j += 3;
        }
        i += 1;
    }
    j
}

/// Return characters as an array of bytes in Utf8 representation.
pub fn chars2utf_range(src: &[u16], sindex: i32, len: i32) -> Vec<u8> {
    let mut dst = vec![0u8; (len * 3) as usize];
    let len1 = chars2utf_into(src, sindex, &mut dst, 0, len);
    dst.truncate(len1 as usize);
    dst
}

/// Return all characters in given array as an array of bytes in Utf8 representation.
pub fn chars2utf(src: &[u16]) -> Vec<u8> {
    chars2utf_range(src, 0, src.len() as i32)
}

/// Return string as an array of bytes in Utf8 representation.
pub fn string2utf(s: &JavaString) -> Vec<u8> {
    chars2utf(s.as_units())
}

/// Escapes each character in a string that has an escape sequence or is
/// non-printable ASCII. Leaves non-ASCII characters alone.
pub fn quote(s: &JavaString) -> JavaString {
    let mut out = JavaString::new();
    let units = s.as_units().to_vec();
    for u in units {
        let q = quote_char(u);
        out.as_units_mut().extend_from_slice(q.as_units());
    }
    out
}

/// Escapes a character if it has an escape sequence or is non-printable ASCII.
/// Leaves non-ASCII characters alone.
pub fn quote_char(ch: u16) -> JavaString {
    let s: &str = match ch {
        0x08 => "\\b",
        0x0C => "\\f",
        0x0A => "\\n",
        0x0D => "\\r",
        0x09 => "\\t",
        0x27 => "\\'",
        0x22 => "\\\"",
        0x5C => "\\\\",
        _ => {
            if is_printable_ascii(ch) {
                return JavaString::from_units(vec![ch]);
            }
            return JavaString::from(format!("\\u{:04x}", ch).as_str());
        }
    };
    JavaString::from(s)
}

fn is_printable_ascii(ch: u16) -> bool {
    ch >= b' ' as u16 && ch <= b'~' as u16
}

/// Escape all unicode characters in string.
pub fn escape_unicode(s: &JavaString) -> JavaString {
    let len = s.len();
    let mut i = 0usize;
    let mut result = s.clone();
    while i < len {
        let ch = result.char_at(i);
        if ch > 255 {
            let mut buf = JavaString::new();
            buf.as_units_mut()
                .extend_from_slice(&result.as_units()[0..i]);
            while i < len {
                let ch = result.char_at(i);
                if ch > 255 {
                    buf.as_units_mut().push(b'\\' as u16);
                    buf.as_units_mut().push(b'u' as u16);
                    for shift in [12, 8, 4, 0] {
                        buf.as_units_mut()
                            .push(hex_digit(((ch >> shift) % 16) as i32));
                    }
                } else {
                    buf.push(ch);
                }
                i += 1;
            }
            result = buf;
            break;
        }
        i += 1;
    }
    result
}

fn hex_digit(d: i32) -> u16 {
    if d < 10 {
        b'0' as u16 + d as u16
    } else {
        b'a' as u16 + (d - 10) as u16
    }
}

// ------------------------------------------------------------------------
// Conversion routines for qualified name splitting
// ------------------------------------------------------------------------

/// Return the last part of a qualified name.
pub fn short_name(name: &Name) -> Name {
    let start = name.last_index_of(b'.') + 1;
    let end = name.get_byte_length();
    if start == 0 && end as usize == name.length() {
        return name.clone();
    }
    name.sub_name(name.last_index_of(b'.') + 1, name.get_byte_length())
}

/// Return the last part of a qualified name from its string representation.
pub fn short_name_str(name: &JavaString) -> JavaString {
    let units = name.as_units();
    let mut idx = -1i32;
    for (i, &u) in units.iter().enumerate() {
        if u == b'.' as u16 {
            idx = i as i32;
        }
    }
    JavaString::from_units(units[(idx + 1) as usize..].to_vec())
}

/// Return the package name of a class name, excluding the trailing '.',
/// "" if not existent.
pub fn package_part(classname: &Name) -> Name {
    classname.sub_name(0, classname.last_index_of(b'.'))
}

/// String version of package_part.
pub fn package_part_str(classname: &JavaString) -> JavaString {
    let units = classname.as_units();
    let mut last_dot = -1i32;
    for (i, &u) in units.iter().enumerate() {
        if u == b'.' as u16 {
            last_dot = i as i32;
        }
    }
    if last_dot < 0 {
        JavaString::new()
    } else {
        JavaString::from_units(units[0..last_dot as usize].to_vec())
    }
}

/// Return all enclosing candidates of a class name, e.g. for a$b$c return
/// [a, a$b].
pub fn enclosing_candidates(name: &Name) -> crate::util::list::List<Name> {
    let mut names: crate::util::list::List<Name> = crate::util::list::List::nil();
    let mut name = name.clone();
    loop {
        let index = name.last_index_of(b'$');
        if index <= 0 {
            break;
        }
        name = name.sub_name(0, index);
        names = names.prepend(name.clone());
    }
    names
}

/// Return all candidates for a class name, converting dots to dollars.
pub fn class_candidates(name: &Name) -> crate::util::list::List<Name> {
    let mut names: crate::util::list::List<Name> = crate::util::list::List::nil();
    let name_str = name.to_java_string();
    let mut index: i32 = -1;
    loop {
        index = name_str.index_of_char(b'.' as u16, (index + 1) as usize);
        if index <= 0 {
            break;
        }
        let pack = name_str.substring(0, (index + 1) as usize);
        let clz = name_str
            .substring((index + 1) as usize, name_str.len())
            .replace_char(b'.' as u16, b'$' as u16);
        let candidate = name.table.from_string(&pack.concat(&clz));
        names = names.prepend(candidate);
    }
    names.reverse()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf_roundtrip_ascii() {
        let j = JavaString::from("abc");
        let utf = string2utf(&j);
        assert_eq!(utf, b"abc");
        assert_eq!(utf2string(&utf).to_string_lossy(), "abc");
    }

    #[test]
    fn utf_roundtrip_non_ascii() {
        let j = JavaString::from("\u{00e9}\u{4e2d}");
        let utf = string2utf(&j);
        assert_eq!(utf2string(&utf), j);
    }

    #[test]
    fn quote_chars() {
        assert_eq!(quote_char(0x0A).to_string_lossy(), "\\n");
        assert_eq!(quote_char(b'a' as u16).to_string_lossy(), "a");
        assert_eq!(quote_char(0x01).to_string_lossy(), "\\u0001");
    }

    #[test]
    fn parse_ints() {
        assert_eq!(string2int(&JavaString::from("123"), 10).unwrap(), 123);
        assert_eq!(string2int(&JavaString::from("-7"), 10).unwrap(), -7);
        assert_eq!(string2int(&JavaString::from("ff"), 16).unwrap(), 255);
        assert_eq!(
            string2int(&JavaString::from("abc"), 10).unwrap_err(),
            NumberFormatException
        );
        assert_eq!(
            string2long(&JavaString::from("9223372036854775807"), 10).unwrap(),
            i64::MAX
        );
        assert_eq!(
            string2long(&JavaString::from("9223372036854775808"), 10).unwrap_err(),
            NumberFormatException
        );
    }

    #[test]
    fn escape_unicode_case() {
        let s = JavaString::from("a\u{0100}b");
        assert_eq!(escape_unicode(&s).to_string_lossy(), "a\\u0100b");
    }

    #[test]
    fn package_split_str() {
        assert_eq!(
            short_name_str(&JavaString::from("a.b.C")).to_string_lossy(),
            "C"
        );
        assert_eq!(
            package_part_str(&JavaString::from("a.b.C")).to_string_lossy(),
            "a.b"
        );
        assert_eq!(
            package_part_str(&JavaString::from("C")).to_string_lossy(),
            ""
        );
    }
}
