// PORTING ADAPTER (not a 1:1 port)
// Adapter for: java.net.URI (java.base).
// Why not 1:1: java.base is not part of the sparse reference checkout
//              (reference/ contains java.compiler, jdk.compiler,
//              jdk.internal.opt only) and javax.tools only observes a minimal
//              subset of java.net.URI; the complete port is deferred to P2
//              (com.sun.tools.javac.file), see the TODOs below.
//! Minimal mapping of `java.net.URI` (java.base) for the `javax.tools` layer.
//!
//! PORTING_RULES: only the parts of `java.net.URI` that `javax.tools` observes
//! are implemented here:
//!
//! * creation with syntax validation (`URI.create` / `new URI(String)`),
//! * `getScheme()`, `isAbsolute()`, `isOpaque()`,
//! * `getPath()` — percent-decoded as UTF-8 with malformed input replaced,
//!   which is what the JDK decoder used by `java.net.URI` does,
//! * the string form (`toString`), used by diagnostics and messages.
//!
//! All text is a Java String (`javac_strings::JavaString`): the URI string, the
//! scheme and the decoded path are Java Strings in the JDK.
//!
//! TODO(P2): this is not the complete `java.net.URI` port.
//! `normalize()`, `resolve()`, `relativize()`, `toURL()`, component-wise
//! validation and component-wise equality are added together with
//! `com.sun.tools.javac.file`, which is the first package that observes them.
//! Until then `PartialEq`/`Hash` compare the URI string form, whereas
//! `java.net.URI.equals` normalizes percent-encoding and the case of the
//! scheme/host components — registered as a known limitation of this adapter.

use javac_strings::JavaString;
use std::fmt;

/// Mapping of `java.net.URI`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct URI {
    /// The URI string as given (Java: the `input` field).
    raw: JavaString,
    /// Parsed scheme, `None` for a relative URI.
    scheme: Option<JavaString>,
    /// Decoded path component (`getPath()`); `None` for an opaque URI.
    path: Option<JavaString>,
    /// Whether the URI is opaque.
    opaque: bool,
}

impl URI {
    /// Java: `URI.create(String)`.
    ///
    /// PORTING_RULES: an invalid URI raises `IllegalArgumentException`
    /// upstream, which is API misuse and maps to `panic!` (see crate::error).
    pub fn create(s: &JavaString) -> URI {
        match parse(s) {
            Ok(uri) => uri,
            Err(message) => panic!("{message}"),
        }
    }

    /// Java: `URI.getScheme()`.
    pub fn get_scheme(&self) -> Option<&JavaString> {
        self.scheme.as_ref()
    }

    /// Java: `URI.isAbsolute()`.
    pub fn is_absolute(&self) -> bool {
        self.scheme.is_some()
    }

    /// Java: `URI.isOpaque()`.
    pub fn is_opaque(&self) -> bool {
        self.opaque
    }

    /// Java: `URI.getPath()`.
    ///
    /// Returns the decoded path, or `None` for an opaque URI.
    pub fn get_path(&self) -> Option<JavaString> {
        self.path.clone()
    }

    /// Java: `URI.toString()`.
    pub fn to_uri_string(&self) -> JavaString {
        self.raw.clone()
    }
}

impl fmt::Display for URI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw.to_string_lossy())
    }
}

impl fmt::Debug for URI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "URI({:?})", self.raw)
    }
}

/// Java: `new URI(String)`, i.e. the parse performed by the `java.net.URI`
/// constructor.
fn parse(s: &JavaString) -> Result<URI, JavaString> {
    check_chars(s)?;

    // Split off the scheme, if any.  A colon before the first '/', '?' or '#'
    // starts an absolute URI only when the preceding text is a legal scheme;
    // otherwise java.net.URI reports an illegal character in the path.
    let mut scheme_end: Option<usize> = None;
    let mut is_colon = false;
    for i in 0..s.len() {
        let c = s.char_at(i);
        if c == b':' as u16 {
            scheme_end = Some(i);
            is_colon = true;
            break;
        }
        if c == b'/' as u16 || c == b'?' as u16 || c == b'#' as u16 {
            scheme_end = Some(i);
            break;
        }
    }
    let scheme = match scheme_end {
        Some(i) if is_colon => {
            let candidate = s.substring(0, i);
            if !is_scheme(&candidate) {
                // Java: "Illegal character in path at index 0" for a relative
                // URI whose first segment contains a colon.
                return Err(illegal_char_in_path(s, i));
            }
            Some(candidate)
        }
        _ => None,
    };

    let rest = match scheme.as_ref() {
        Some(name) => s.substring(name.len() + 1, s.len()),
        None => s.clone(),
    };

    // Java: "A URI is opaque if it is absolute and its scheme-specific part
    // does not begin with a slash ('/')."
    let opaque = scheme.is_some() && (rest.is_empty() || rest.char_at(0) != b'/' as u16);

    if opaque {
        return Ok(URI { raw: s.clone(), scheme, path: None, opaque: true });
    }

    // Hierarchical URI: optional authority, then the path up to '?' or '#'.
    let after_authority = if rest.starts_with(&JavaString::from("//")) {
        let ssp = rest.substring(2, rest.len());
        match find_first_of(&ssp, b"/?#") {
            Some(i) => ssp.substring(i, ssp.len()),
            None => JavaString::new(),
        }
    } else {
        rest
    };
    let path_end = find_first_of(&after_authority, b"?#").unwrap_or(after_authority.len());
    let raw_path = after_authority.substring(0, path_end);
    let path = decode(&raw_path);

    Ok(URI { raw: s.clone(), scheme, path: Some(path), opaque: false })
}

/// Java: the character checks performed while parsing a URI string.  The URI
/// syntax is ASCII-based; non-ASCII characters must be percent-encoded.  The
/// set of characters rejected here is the one reported by `java.net.URI`
/// ("Illegal character in ...") plus malformed escape pairs.
///
/// TODO(P2): `java.net.URI` validates per component (for example '[' and ']'
/// are legal only in an IPv6 authority); component-wise validation is added
/// with the complete URI port.
fn check_chars(s: &JavaString) -> Result<(), JavaString> {
    for i in 0..s.len() {
        let b = s.char_at(i);
        // The URI syntax is ASCII; control characters and space are illegal.
        if !(0x20..0x7F).contains(&b) || b == b' ' as u16 {
            return Err(illegal_char_in_component(s, i));
        }
        if b == b'<' as u16
            || b == b'>' as u16
            || b == b'"' as u16
            || b == b'{' as u16
            || b == b'}' as u16
            || b == b'|' as u16
            || b == b'\\' as u16
            || b == b'^' as u16
            || b == b'`' as u16
        {
            return Err(illegal_char_in_component(s, i));
        }
        if b == b'%' as u16 {
            let ok = i + 2 < s.len()
                && hex_value(s.char_at(i + 1)).is_some()
                && hex_value(s.char_at(i + 2)).is_some();
            if !ok {
                return Err(JavaString::from(
                    format!("Malformed escape pair at index {i}").as_str(),
                ));
            }
        }
    }
    Ok(())
}

fn illegal_char_in_path(s: &JavaString, index: usize) -> JavaString {
    let _ = s;
    JavaString::from(format!("Illegal character in path at index {index}").as_str())
}

fn illegal_char_in_component(s: &JavaString, index: usize) -> JavaString {
    let _ = s;
    JavaString::from(format!("Illegal character in ... at index {index}").as_str())
}

/// Returns the index of the first code unit equal to one of `targets`.
fn find_first_of(s: &JavaString, targets: &[u8]) -> Option<usize> {
    for i in 0..s.len() {
        let c = s.char_at(i);
        if targets.iter().any(|t| *t as u16 == c) {
            return Some(i);
        }
    }
    None
}

/// Java: the `scheme` production of RFC 3986.
fn is_scheme(candidate: &JavaString) -> bool {
    if candidate.is_empty() {
        return false;
    }
    // The URI string was validated as ASCII by check_chars, so a u8 view of a
    // code unit is exact here.
    let first = candidate.char_at(0) as u8;
    if !first.is_ascii_alphabetic() {
        return false;
    }
    for i in 1..candidate.len() {
        let b = candidate.char_at(i) as u8;
        if !(b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.') {
            return false;
        }
    }
    true
}

/// Java: `java.net.URI.decode(String)` — decode every `%XX` escape and
/// interpret the resulting byte sequence as UTF-8, replacing malformed input
/// (the JDK decoder used by `java.net.URI` is configured with
/// `CodingErrorAction.REPLACE`).
fn decode(s: &JavaString) -> JavaString {
    if !s.contains(&JavaString::from("%")) {
        return s.clone();
    }
    let mut decoded: Vec<u8> = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s.char_at(i) == b'%' as u16 {
            let hi = hex_value(s.char_at(i + 1)).expect("URI escape was validated");
            let lo = hex_value(s.char_at(i + 2)).expect("URI escape was validated");
            decoded.push(hi << 4 | lo);
            i += 3;
        } else {
            decoded.push(s.char_at(i) as u8);
            i += 1;
        }
    }
    JavaString::from_utf8_lossy(&decoded)
}

fn hex_value(b: u16) -> Option<u8> {
    if (b'0' as u16..=b'9' as u16).contains(&b) {
        Some((b - b'0' as u16) as u8)
    } else if (b'a' as u16..=b'f' as u16).contains(&b) {
        Some((b - b'a' as u16 + 10) as u8)
    } else if (b'A' as u16..=b'F' as u16).contains(&b) {
        Some((b - b'A' as u16 + 10) as u8)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::URI;
    use javac_strings::JavaString;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    #[test]
    fn string_uri_path() {
        // java.net.URI.create("string:///Foo.java").getPath() == "/Foo.java"
        let uri = URI::create(&js("string:///Foo.java"));
        assert_eq!(uri.get_scheme(), Some(&js("string")));
        assert!(uri.is_absolute());
        assert!(!uri.is_opaque());
        assert_eq!(uri.get_path(), Some(js("/Foo.java")));
    }

    #[test]
    fn file_uri_path() {
        let uri = URI::create(&js("file:/C:/tmp/Foo.java"));
        assert_eq!(uri.get_scheme(), Some(&js("file")));
        assert_eq!(uri.get_path(), Some(js("/C:/tmp/Foo.java")));
    }

    #[test]
    fn empty_path() {
        let uri = URI::create(&js("file:///"));
        assert_eq!(uri.get_path(), Some(js("/")));
        let uri = URI::create(&js("http://example.com"));
        assert_eq!(uri.get_path(), Some(js("")));
    }

    #[test]
    fn opaque_uri_has_no_path() {
        // java.net.URI.getPath() returns null for opaque URIs.
        let uri = URI::create(&js("jar:file:/tmp/x.jar!/a/B.class"));
        assert!(uri.is_opaque());
        assert_eq!(uri.get_path(), None);
        let uri = URI::create(&js("mailto:a@b.c"));
        assert_eq!(uri.get_path(), None);
    }

    #[test]
    fn relative_uri_path() {
        let uri = URI::create(&js("a/b/Foo.java"));
        assert!(!uri.is_absolute());
        assert_eq!(uri.get_path(), Some(js("a/b/Foo.java")));
        let uri = URI::create(&js("//host/a"));
        assert_eq!(uri.get_path(), Some(js("/a")));
    }

    #[test]
    fn percent_decoding() {
        let uri = URI::create(&js("file:/tmp/a%20b/Foo.java"));
        assert_eq!(uri.get_path(), Some(js("/tmp/a b/Foo.java")));
        let uri = URI::create(&js("file:/tmp/%C3%BC/Foo.java"));
        assert_eq!(uri.get_path(), Some(js("/tmp/ü/Foo.java")));
    }

    #[test]
    fn empty_uri() {
        let uri = URI::create(&js(""));
        assert_eq!(uri.get_scheme(), None);
        assert!(!uri.is_absolute());
        assert_eq!(uri.get_path(), Some(js("")));
    }

    #[test]
    fn uri_string_form() {
        let uri = URI::create(&js("string:///Foo.java"));
        assert_eq!(uri.to_uri_string(), js("string:///Foo.java"));
        assert_eq!(uri.to_string(), "string:///Foo.java");
    }

    #[test]
    #[should_panic]
    fn space_is_illegal() {
        // java.net.URI throws IllegalArgumentException for a space.
        URI::create(&js("string:///Foo Bar.java"));
    }

    #[test]
    #[should_panic]
    fn malformed_escape_is_illegal() {
        URI::create(&js("file:/tmp/%ZZ"));
    }
}
