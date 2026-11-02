// PORTING ADAPTER (not a 1:1 port): Java String/char semantics.
//! Java String/char semantics: text is a sequence of UTF-16 code units.
//!
//! javac is written against Java strings; the observable behavior (identifier
//! rules, String.length(), source positions, class-file modified UTF-8) is
//! defined in terms of UTF-16 code units. Rust String is UTF-8, so the compiler
//! uses JavaString internally.
//!
//! This crate is the bottom-most layer shared by javac-tools and javac-core.
//! See docs/porting adapter.md ("String semantics").

use std::fmt;

/// A Java String: a sequence of UTF-16 code units.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JavaString {
    units: Vec<u16>,
}

impl JavaString {
    pub fn new() -> Self {
        JavaString { units: Vec::new() }
    }

    pub fn from_units(units: Vec<u16>) -> Self {
        JavaString { units }
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn as_units(&self) -> &[u16] {
        &self.units
    }

    pub fn as_units_mut(&mut self) -> &mut Vec<u16> {
        &mut self.units
    }

    pub fn push(&mut self, unit: u16) {
        self.units.push(unit);
    }

    /// Java: charAt(index) — panics outside 0..len like Java throws.
    pub fn char_at(&self, index: usize) -> u16 {
        self.units[index]
    }

    /// Lossy UTF-8 view, for Rust-side logging/tests only.
    /// Unpaired surrogates are replaced with U+FFFD.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.units)
    }
}

impl From<&str> for JavaString {
    fn from(s: &str) -> Self {
        JavaString {
            units: s.encode_utf16().collect(),
        }
    }
}

impl From<&[u16]> for JavaString {
    fn from(units: &[u16]) -> Self {
        JavaString {
            units: units.to_vec(),
        }
    }
}

impl fmt::Debug for JavaString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.to_string_lossy())
    }
}

impl fmt::Display for JavaString {
    // NOTE: final output encoding (platform charset) is handled by the Log/output
    // layer; this Display is for UTF-16 -> UTF-8 rendering.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

// ---------------------------------------------------------------------------
// Java String API subset used by the interface layer and compiler port.
// §7: all operations are defined on UTF-16 code units exactly
// like java.lang.String; they must not be reimplemented ad hoc in dependents.
// ---------------------------------------------------------------------------

impl JavaString {
    /// Java: public String concat(String str) — appends the units unchanged.
    pub fn concat(&self, other: &JavaString) -> JavaString {
        let mut units = Vec::with_capacity(self.units.len() + other.units.len());
        units.extend_from_slice(&self.units);
        units.extend_from_slice(&other.units);
        JavaString { units }
    }

    /// Java: public boolean startsWith(String prefix)
    pub fn starts_with(&self, prefix: &JavaString) -> bool {
        self.units.starts_with(&prefix.units)
    }

    /// Java: public boolean endsWith(String suffix)
    pub fn ends_with(&self, suffix: &JavaString) -> bool {
        self.units.ends_with(&suffix.units)
    }

    /// Java: public boolean contains(CharSequence s) — true for the empty string.
    pub fn contains(&self, other: &JavaString) -> bool {
        if other.units.is_empty() {
            return true;
        }
        self.units
            .windows(other.units.len())
            .any(|w| w == other.units.as_slice())
    }

    /// Java: String.split("<literal sep>", -1) for a single literal code unit.
    ///
    /// Semantics of String.split with a negative limit: no trailing empty strings
    /// are discarded, and the empty input yields a single empty element. The
    /// separator is a literal unit here (call sites must quote regex metacharacters
    /// in Java; passing a literal unit is the exact equivalent for those sites).
    pub fn split_char(&self, sep: u16) -> Vec<JavaString> {
        if self.units.is_empty() {
            return vec![JavaString::new()];
        }
        let mut out = Vec::new();
        let mut start = 0usize;
        for i in 0..self.units.len() {
            if self.units[i] == sep {
                out.push(JavaString::from_units(self.units[start..i].to_vec()));
                start = i + 1;
            }
        }
        out.push(JavaString::from_units(self.units[start..].to_vec()));
        out
    }

    /// Java: public String substring(int beginIndex, int endIndex)
    pub fn substring(&self, start: usize, end: usize) -> JavaString {
        assert!(
            start <= end,
            "StringIndexOutOfBoundsException: begin {start}, end {end}"
        );
        assert!(
            end <= self.units.len(),
            "StringIndexOutOfBoundsException: end {end}, length {}",
            self.units.len()
        );
        JavaString::from_units(self.units[start..end].to_vec())
    }

    /// Java: public int codePointAt(int index) — reads UTF-16 code units and
    /// combines a valid surrogate pair, exactly like java.lang.String.
    pub fn code_point_at(&self, index: usize) -> i32 {
        let u = self.units[index];
        if (0xD800..=0xDBFF).contains(&u) && index + 1 < self.units.len() {
            let n = self.units[index + 1];
            if (0xDC00..=0xDFFF).contains(&n) {
                return 0x10000 + (((u - 0xD800) as i32) << 10) + (n - 0xDC00) as i32;
            }
        }
        u as i32
    }

    /// Java: public int indexOf(int ch, int fromIndex) for a single code unit.
    /// Returns the UTF-16 index or -1; fromIndex<0 behaves like 0.
    pub fn index_of_char(&self, needle: u16, from_index: usize) -> i32 {
        let start = from_index.min(self.units.len());
        for (i, &u) in self.units.iter().enumerate().skip(start) {
            if u == needle {
                return i as i32;
            }
        }
        -1
    }

    /// Java: public String replace(char oldChar, char newChar)
    pub fn replace_char(&self, old: u16, new: u16) -> JavaString {
        JavaString {
            units: self
                .units
                .iter()
                .map(|&u| if u == old { new } else { u })
                .collect(),
        }
    }

    /// Decode UTF-8 bytes to a JavaString with replacement of malformed input.
    ///
    /// REGISTERED(P2): match java.nio.charset.StandardCharsets.UTF_8 CharsetDecoder
    /// REPLACE semantics byte-for-byte (Java emits U+FFFD with its own malformed
    /// sequence boundaries); Rust from_utf8_lossy is used until the file layer
    /// differential tests can compare against javac.
    pub fn from_utf8_lossy(bytes: &[u8]) -> JavaString {
        JavaString {
            units: String::from_utf8_lossy(bytes).encode_utf16().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JavaString;

    #[test]
    fn utf16_units() {
        let s = JavaString::from("a\u{1F600}b");
        // Java String.length() counts the surrogate pair as two chars.
        assert_eq!(s.len(), 4);
        assert_eq!(s.char_at(0), 0x61);
        assert_eq!(s.to_string_lossy(), "a\u{1F600}b");
    }

    #[test]
    fn unpaired_surrogate_roundtrip_lossy() {
        let s = JavaString::from_units(vec![0xD800]);
        assert_eq!(s.len(), 1);
    }
}

#[cfg(test)]
mod string_ops_tests {
    use super::*;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    #[test]
    fn concat_and_starts_ends() {
        let a = js("java.");
        let b = js("lang");
        assert_eq!(a.concat(&b).to_string_lossy(), "java.lang");
        assert!(js("java.lang").starts_with(&a));
        assert!(js("java.lang").ends_with(&b));
        assert!(!js("java.lang").starts_with(&b));
    }

    #[test]
    fn contains_and_split() {
        assert!(js("java.lang").contains(&js("a.l")));
        assert!(js("java.lang").contains(&js("")));
        assert!(!js("java.lang").contains(&js("x")));
        let parts = js("java.lang.String").split_char(b'.' as u16);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2].to_string_lossy(), "String");
        let trailing = js("a.").split_char(b'.' as u16);
        assert_eq!(trailing.len(), 2);
        assert!(trailing[1].is_empty());
        let empty = js("").split_char(b'.' as u16);
        assert_eq!(empty.len(), 1);
        assert!(empty[0].is_empty());
    }

    #[test]
    fn substring_and_code_points() {
        let s = js("a\u{1F600}b");
        assert_eq!(s.substring(0, 1).to_string_lossy(), "a");
        assert_eq!(s.substring(1, 3).to_string_lossy(), "\u{1F600}");
        assert_eq!(s.code_point_at(0), 0x61);
        assert_eq!(s.code_point_at(1), 0x1F600);
        // index 2 is the low surrogate itself: Java returns its unit value
        assert_eq!(s.code_point_at(2), 0xDE00);
        assert_eq!(s.code_point_at(3), 0x62);
    }

    #[test]
    fn from_utf8_lossy_basic() {
        assert_eq!(JavaString::from_utf8_lossy(b"abc").to_string_lossy(), "abc");
        let replacement = JavaString::from_utf8_lossy(&[0xFF]);
        assert_eq!(replacement.to_string_lossy(), "\u{FFFD}");
    }
}

#[cfg(test)]
mod index_replace_tests {
    use super::*;

    #[test]
    fn index_of_char_semantics() {
        let s = JavaString::from("a.b.C");
        assert_eq!(s.index_of_char(b'.' as u16, 0), 1);
        assert_eq!(s.index_of_char(b'.' as u16, 2), 3);
        assert_eq!(s.index_of_char(b'z' as u16, 0), -1);
        assert_eq!(s.index_of_char(b'C' as u16, s.len()), -1);
        assert_eq!(s.index_of_char(b'a' as u16, 100), -1);
    }

    #[test]
    fn replace_char_semantics() {
        let s = JavaString::from("a.b.C").replace_char(b'.' as u16, b'$' as u16);
        assert_eq!(s.to_string_lossy(), "a$b$C");
    }
}
