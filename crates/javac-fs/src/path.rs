// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.file.Path (the subset used by com.sun.tools.javac.file).
// Reference behavior: the default FileSystem's Path (WindowsPath on this
// machine), recorded through a development-time Java oracle (see the tests).
//
//! Path: a java.nio.file.Path equivalent.
//!
//!
//! * The path is parsed textually into (prefix, rooted, elements) like
//!   sun.nio.fs.WindowsPathPathParser, so Path.toString() keeps "." / ".."
//!   elements, collapses duplicate separators and drops the trailing
//!   separator (std::path would drop "." elements).
//! * Java Path methods keep their names in snake_case; the Java semantics are
//!   reproduced textually (only to_absolute_path uses the process working
//!   directory, like Java's user.dir).
//! * Windows semantics: '\' separator, case-insensitive comparisons
//!   (starts_with/ends_with/relativize/equality), drive-relative paths
//!   ("C:foo"), UNC roots, empty path.  Linux: '/' separator and
//!   case-sensitive comparisons; the Windows-only oracle tests are gated with
//!   #[cfg(windows)] and the differences are documented.
//! * InvalidPathException: NUL-containing input is rejected.

use crate::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// A java.nio.file.Path equivalent (platform-aware path syntax).
#[derive(Clone, Debug)]
pub struct Path {
    parsed: Parsed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Parsed {
    /// The root prefix: a drive ("C:") or a UNC authority ("\\server\share").
    prefix: Option<String>,
    /// A leading separator (or, on Windows, the separator after the drive).
    rooted: bool,
    /// The name elements, including "." and "..".
    elements: Vec<String>,
    /// The empty path (Java Path.of("")).
    empty: bool,
    /// The prefix is a UNC authority (its root keeps a trailing separator).
    unc: bool,
    /// The platform string form keeps a trailing separator (UNC root parents).
    trailing: bool,
}

/// Windows path separator.
#[cfg(windows)]
const SEP: char = '\\';
#[cfg(not(windows))]
const SEP: char = '/';

fn is_separator(ch: char) -> bool {
    if cfg!(windows) {
        ch == '\\' || ch == '/'
    } else {
        ch == '/'
    }
}

impl Parsed {
    fn empty_path() -> Parsed {
        Parsed { prefix: None, rooted: false, elements: Vec::new(), empty: true, unc: false, trailing: false }
    }

    fn from_elements(elements: Vec<String>) -> Parsed {
        Parsed { prefix: None, rooted: false, elements, empty: false, unc: false, trailing: false }
    }

    fn parse(text: &str) -> Result<Parsed, Error> {
        if text.contains('\0') {
            return Err(Error::InvalidPath { path: text.to_string() });
        }
        if text.is_empty() {
            return Ok(Parsed::empty_path());
        }
        let mut parsed = Parsed { prefix: None, rooted: false, elements: Vec::new(), empty: false, unc: false, trailing: false };
        let mut rest = text;
        if cfg!(windows) {
            let bytes = text.as_bytes();
            if bytes.len() >= 2 && is_separator(text.chars().next().unwrap()) && is_separator(text.chars().nth(1).unwrap()) {
                // UNC: \\server\share\...
                let segments: Vec<&str> = text.split(is_separator).filter(|s| !s.is_empty()).collect();
                let server = segments.first().copied().unwrap_or("");
                let share = segments.get(1).copied().unwrap_or("");
                parsed.prefix = Some(format!("{}{}{}{}{}", SEP, SEP, server, SEP, share));
                parsed.unc = true;
                parsed.rooted = true;
                // A UNC root spelled with a trailing separator keeps it in its
                // string form (the oracle records "\\server\share\").
                parsed.trailing = text.chars().last().map(is_separator).unwrap_or(false);
                let skip = 2;
                parsed.elements = segments
                    .iter()
                    .skip(skip)
                    .map(|segment| (*segment).to_string())
                    .collect();
                return Ok(parsed);
            }
            if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                parsed.prefix = Some(text[..2].to_string());
                rest = &text[2..];
                if rest.chars().next().map(is_separator).unwrap_or(false) {
                    parsed.rooted = true;
                }
                parsed.elements = rest
                    .split(is_separator)
                    .filter(|segment| !segment.is_empty())
                    .map(|segment| segment.to_string())
                    .collect();
                return Ok(parsed);
            }
            if bytes[0] == b'\\' || bytes[0] == b'/' {
                parsed.rooted = true;
                parsed.elements = text
                    .split(is_separator)
                    .filter(|segment| !segment.is_empty())
                    .map(|segment| segment.to_string())
                    .collect();
                return Ok(parsed);
            }
        } else if text.starts_with('/') {
            parsed.rooted = true;
        }
        parsed.elements = rest
            .split(is_separator)
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_string())
            .collect();
        Ok(parsed)
    }

    fn to_java_string(&self) -> String {
        let mut out = String::new();
        if let Some(prefix) = &self.prefix {
            out.push_str(prefix);
        }
        if self.rooted {
            if self.unc {
                if !self.elements.is_empty() || self.trailing {
                    out.push(SEP);
                }
            } else {
                out.push(SEP);
            }
        }
        let mut after_prefix_without_root = self.prefix.is_some() && !self.rooted;
        for element in &self.elements {
            push_element(&mut out, element, &mut after_prefix_without_root);
        }
        out
    }

    fn has_dot_or_dotdot(&self) -> bool {
        self.elements.iter().any(|element| element == "." || element == "..")
    }

    fn root_key(&self) -> Option<String> {
        let root = self.root_text();
        Some(if cfg!(windows) { root.to_ascii_lowercase() } else { root })
    }

    fn root_text(&self) -> String {
        if let Some(prefix) = &self.prefix {
            if self.rooted {
                format!("{}{}", prefix, SEP)
            } else {
                prefix.clone()
            }
        } else if self.rooted {
            SEP.to_string()
        } else {
            String::new()
        }
    }
}

impl Path {
    /// Java: Paths.get(String) / Path.of(String).
    pub fn new(path: &str) -> Result<Path, Error> {
        Ok(Path { parsed: Parsed::parse(path)? })
    }

    /// Wraps an existing std path.
    pub fn from_path_buf(raw: PathBuf) -> Path {
        let text = raw.to_string_lossy().into_owned();
        Path {
            parsed: Parsed::parse(&text).unwrap_or_else(|_| Parsed::empty_path()),
        }
    }

    /// The underlying std path (built from the platform string form).
    pub fn as_path(&self) -> PathBuf {
        PathBuf::from(self.to_java_string())
    }

    /// The underlying std path buffer.
    pub fn to_path_buf(&self) -> PathBuf {
        self.as_path()
    }

    /// Java: Path.toString().
    pub fn to_java_string(&self) -> String {
        self.parsed.to_java_string()
    }

    /// The path is empty (Java Path.of("")).
    pub fn is_empty(&self) -> bool {
        self.parsed.empty
    }

    /// Java: Path.isAbsolute() — Windows: drive/UNC + root; a bare root ("/")
    /// is NOT absolute on Windows (the oracle records false).
    pub fn is_absolute(&self) -> bool {
        if cfg!(windows) {
            self.parsed.prefix.is_some() && self.parsed.rooted
        } else {
            self.parsed.rooted
        }
    }

    /// Java: Path.toAbsolutePath() — resolves against the working directory
    /// without normalizing.
    pub fn to_absolute_path(&self) -> Result<Path, Error> {
        if self.is_absolute() {
            return Ok(self.clone());
        }
        let cwd = std::env::current_dir()?;
        let cwd_text = cwd.to_string_lossy().into_owned();
        if self.parsed.empty {
            return Path::new(&cwd_text);
        }
        let relative = self.to_java_string();
        let text = format!("{}{}{}", cwd_text, SEP, relative);
        Path::new(&text)
    }

    /// Java: Path.normalize() — removes "." and resolves ".." textually.
    pub fn normalize(&self) -> Path {
        if self.parsed.empty {
            return self.clone();
        }
        let mut elements: Vec<String> = Vec::new();
        for element in &self.parsed.elements {
            match element.as_str() {
                "." => {}
                ".." => {
                    if matches!(elements.last(), Some(last) if last != "..") {
                        elements.pop();
                    } else if !self.parsed.rooted && self.parsed.prefix.is_none() {
                        elements.push("..".to_string());
                    } else if self.parsed.prefix.is_some() && !self.parsed.rooted {
                        // drive-relative: "C:.." keeps ".." (the oracle records
                        // C:..\a unchanged).
                        elements.push("..".to_string());
                    }
                    // An absolute path discards ".." that would escape the root.
                }
                other => elements.push(other.to_string()),
            }
        }
        Path {
            parsed: Parsed {
                prefix: self.parsed.prefix.clone(),
                rooted: self.parsed.rooted,
                elements,
                empty: false,
                unc: self.parsed.unc,
                trailing: self.parsed.trailing,
            },
        }
    }

    /// Java: Path.relativize(Path).
    ///
    /// as in sun.nio.fs.WindowsPath.relativize, paths that
    /// contain "." / ".." elements are normalized before the element
    /// comparison, and equal paths produce the empty path.
    pub fn relativize(&self, other: &Path) -> Result<Path, Error> {
        if self == other {
            return Ok(Path { parsed: Parsed::empty_path() });
        }
        if self.is_absolute() != other.is_absolute() {
            return Err(Error::IllegalArgument {
                message: "'other' is different type of Path".to_string(),
            });
        }
        if root_keys_differ(&self.parsed, &other.parsed) {
            return Err(Error::IllegalArgument {
                message: "'other' has different root".to_string(),
            });
        }
        if self.parsed.empty {
            return Ok(other.clone());
        }
        let base = if self.parsed.has_dot_or_dotdot() { self.normalize() } else { self.clone() };
        let child = if other.parsed.has_dot_or_dotdot() {
            other.normalize()
        } else {
            other.clone()
        };
        let base_elements = base.parsed.elements.clone();
        let child_elements = child.parsed.elements.clone();
        let mut common = 0;
        while common < base_elements.len()
            && common < child_elements.len()
            && elements_equal(&base_elements[common], &child_elements[common])
        {
            common += 1;
        }
        let mut elements = Vec::new();
        for _ in common..base_elements.len() {
            elements.push("..".to_string());
        }
        for element in &child_elements[common..] {
            elements.push(element.clone());
        }
        Ok(Path { parsed: Parsed::from_elements(elements) })
    }

    /// Java: Path.resolve(Path) — an absolute other path is returned as is;
    /// otherwise the other path's elements are appended (no normalization).
    pub fn resolve(&self, other: &Path) -> Path {
        if other.is_absolute() {
            return other.clone();
        }
        if other.parsed.empty {
            return self.clone();
        }
        if self.parsed.empty && other.parsed.prefix.is_none() && !other.parsed.rooted {
            return other.clone();
        }
        let mut elements = self.parsed.elements.clone();
        elements.extend(other.parsed.elements.iter().cloned());
        let mut parsed = self.parsed.clone();
        parsed.elements = elements;
        parsed.empty = false;
        parsed.trailing = false;
        Path { parsed }
    }

    /// Java: Path.resolveSibling(Path).
    pub fn resolve_sibling(&self, other: &Path) -> Path {
        if other.is_absolute() {
            return other.clone();
        }
        match self.parent() {
            Some(parent) => parent.resolve(other),
            None => other.clone(),
        }
    }

    /// Java: Path.startsWith(Path).
    pub fn starts_with(&self, other: &Path) -> bool {
        if other.is_empty() {
            return false;
        }
        if other.is_absolute() {
            if !self.is_absolute() || root_keys_differ(&self.parsed, &other.parsed) {
                return false;
            }
        } else if self.is_absolute() {
            return false;
        }
        let this = &self.parsed.elements;
        let expected = &other.parsed.elements;
        expected.len() <= this.len()
            && this[..expected.len()]
                .iter()
                .zip(expected.iter())
                .all(|(left, right)| elements_equal(left, right))
    }

    /// Java: Path.endsWith(Path).
    pub fn ends_with(&self, other: &Path) -> bool {
        if other.is_empty() {
            return false;
        }
        if other.is_absolute() && (!self.is_absolute() || root_keys_differ(&self.parsed, &other.parsed)) {
            return false;
        }
        let this = &self.parsed.elements;
        let expected = &other.parsed.elements;
        if expected.len() > this.len() {
            return false;
        }
        this[this.len() - expected.len()..]
            .iter()
            .zip(expected.iter())
            .all(|(left, right)| elements_equal(left, right))
    }

    /// Java: Path.getFileName().
    pub fn file_name(&self) -> Option<Path> {
        if let Some(element) = self.parsed.elements.last() {
            return Some(Path { parsed: Parsed::from_elements(vec![element.clone()]) });
        }
        if self.parsed.empty {
            return Some(Path { parsed: Parsed::empty_path() });
        }
        None
    }

    /// Java: Path.getParent().
    pub fn parent(&self) -> Option<Path> {
        if self.parsed.elements.is_empty() {
            return None;
        }
        let elements = self.parsed.elements[..self.parsed.elements.len() - 1].to_vec();
        if elements.is_empty() && self.parsed.prefix.is_none() && !self.parsed.rooted {
            return None;
        }
        let trailing = elements.is_empty() && self.parsed.unc;
        Some(Path {
            parsed: Parsed {
                prefix: self.parsed.prefix.clone(),
                rooted: self.parsed.rooted,
                elements,
                empty: false,
                unc: self.parsed.unc,
                trailing,
            },
        })
    }

    /// Java: Path.getRoot().
    pub fn root(&self) -> Option<Path> {
        if self.parsed.prefix.is_none() && !self.parsed.rooted {
            return None;
        }
        let text = self.parsed.root_text();
        Some(Path {
            parsed: Parsed { prefix: Some(text), rooted: false, elements: Vec::new(), empty: false, unc: false, trailing: false },
        })
    }

    /// The file extension of the last element (a javac-fs helper).
    pub fn extension(&self) -> Option<String> {
        let name = self.file_name()?.to_java_string();
        let dot = name.rfind('.')?;
        if dot == 0 {
            return None;
        }
        Some(name[dot + 1..].to_string())
    }

    /// Java: Path.subpath(begin, end).
    pub fn subpath(&self, begin: usize, end: usize) -> Result<Path, Error> {
        let elements = &self.parsed.elements;
        if begin >= end || end > elements.len() {
            return Err(Error::IllegalArgument {
                message: format!(
                    "subpath({}, {}) out of range for {} elements",
                    begin,
                    end,
                    elements.len()
                ),
            });
        }
        Ok(Path { parsed: Parsed::from_elements(elements[begin..end].to_vec()) })
    }

    /// Java: Path.getNameCount().
    pub fn name_count(&self) -> usize {
        self.parsed.elements.len()
    }

    /// Java: Path.toUri().
    pub fn to_uri(&self) -> Result<String, Error> {
        let absolute = self.to_absolute_path()?;
        let text = absolute.to_java_string();
        if let Some(rest) = text.strip_prefix("\\\\") {
            let mut parts = rest.splitn(2, SEP);
            let server = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");
            let path = format!("/{}", path.replace(SEP, "/"));
            return Ok(format!("file://{}{}", server, encode_uri_path(&path)));
        }
        let path = text.replace(SEP, "/");
        Ok(format!("file:///{}", encode_uri_path(&path)))
    }

    /// Java: Paths.get(URI) for "file" URIs.
    pub fn from_uri(uri: &str) -> Result<Path, Error> {
        // Java: Path.of(URI) for "file" URIs: accepts "file:/x", "file:///x"
        // and "file://host/share/x"; a non-hierarchical URI or a URI with a
        // query/fragment reports IllegalArgumentException.
        let Some(rest) = uri.strip_prefix("file:") else {
            return Err(Error::UnsupportedUri {
                uri: uri.to_string(),
            });
        };
        let (authority, path) = if let Some(after) = rest.strip_prefix("//") {
            match after.find('/') {
                Some(index) => (&after[..index], &after[index..]),
                None => (after, ""),
            }
        } else if rest.starts_with('/') {
            ("", rest)
        } else {
            return Err(Error::IllegalArgument {
                message: format!("URI is not hierarchical: {}", uri),
            });
        };
        if path.contains('?') {
            return Err(Error::IllegalArgument {
                message: "URI has a query component".to_string(),
            });
        }
        if path.contains('#') {
            return Err(Error::IllegalArgument {
                message: "URI has a fragment component".to_string(),
            });
        }
        let mut decoded = decode_uri_path(path)?;
        if cfg!(windows) && authority.is_empty() {
            let bytes = decoded.as_bytes();
            if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
                decoded = decoded[1..].to_string();
            }
        }
        let mut text = decoded.replace('/', &SEP.to_string());
        if !authority.is_empty() {
            text = format!("{}{}{}{}", SEP, SEP, authority, text);
        }
        Path::new(&text)
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Path) -> bool {
        prefixes_equal(&self.parsed, &other.parsed)
            && self.parsed.rooted == other.parsed.rooted
            && elements_eq(&self.parsed.elements, &other.parsed.elements)
            && self.parsed.empty == other.parsed.empty
    }
}

fn prefixes_equal(left: &Parsed, right: &Parsed) -> bool {
    match (&left.prefix, &right.prefix) {
        (Some(left), Some(right)) => {
            if cfg!(windows) {
                left.eq_ignore_ascii_case(right)
            } else {
                left == right
            }
        }
        (None, None) => true,
        _ => false,
    }
}

impl Eq for Path {}

impl Hash for Path {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(prefix) = &self.parsed.prefix {
            if cfg!(windows) {
                prefix.to_ascii_lowercase().hash(state);
            } else {
                prefix.hash(state);
            }
        }
        self.parsed.rooted.hash(state);
        self.parsed.empty.hash(state);
        for element in &self.parsed.elements {
            if cfg!(windows) {
                element.to_ascii_lowercase().hash(state);
            } else {
                element.hash(state);
            }
        }
    }
}

impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Path) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Path {
    fn cmp(&self, other: &Path) -> std::cmp::Ordering {
        let key = |path: &Path| {
            let mut key: Vec<String> = Vec::new();
            key.push(path.parsed.root_text());
            key.extend(path.parsed.elements.iter().cloned());
            if cfg!(windows) {
                key.iter().map(|part| part.to_ascii_lowercase()).collect::<Vec<_>>()
            } else {
                key
            }
        };
        key(self).cmp(&key(other))
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_java_string())
    }
}

fn push_element(out: &mut String, element: &str, after_prefix_without_root: &mut bool) {
    if !out.is_empty() && !out.ends_with(SEP) && !*after_prefix_without_root {
        out.push(SEP);
    }
    out.push_str(element);
    *after_prefix_without_root = false;
}

fn elements_equal(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

fn elements_eq(left: &[String], right: &[String]) -> bool {
    left.len() == right.len() && left.iter().zip(right.iter()).all(|(l, r)| elements_equal(l, r))
}

fn root_keys_differ(left: &Parsed, right: &Parsed) -> bool {
    // Java compares the root components when present; absent roots compare equal.
    match (left.prefix.is_some() || left.rooted, right.prefix.is_some() || right.rooted) {
        (false, false) => false,
        _ => left.root_key() != right.root_key(),
    }
}

/// Java: java.net.URI path percent-encoding as produced by Path.toUri().
fn encode_uri_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for ch in path.chars() {
        let keep = ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '/' | ':' | '-' | '_' | '.' | '~' | '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=' | '@'
            );
        if keep || !ch.is_ascii() {
            encoded.push(ch);
        } else {
            let mut buffer = [0u8; 4];
            for byte in ch.encode_utf8(&mut buffer).as_bytes() {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Decodes the percent-encoded path component of a file URI.
fn decode_uri_path(path: &str) -> Result<String, Error> {
    let bytes = path.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(Error::InvalidPath { path: path.to_string() });
            }
            let hex = &path[index + 1..index + 3];
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| Error::InvalidPath { path: path.to_string() })?;
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|_| Error::InvalidPath { path: path.to_string() })
}

#[cfg(all(windows, test))]
mod oracle {
    //! The development-time java.nio.file oracle (baseline JDK 17.0.19.10,
    //! default Windows FileSystem).  The expectations were collected by a Java
    //! program (Paths.get / normalize / resolve / relativize / startsWith /
    //! endsWith / toUri / equals) and are embedded here so the tests run
    //! without a JVM.  Linux differences: '/' separator and case-sensitive
    //! comparisons (the Windows oracle is gated with #[cfg(windows)]).

    use super::Path;
    use crate::error::Error;

    struct PathCase {
        input: &'static str,
        display: &'static str,
        absolute: bool,
        file_name: Option<&'static str>,
        parent: Option<&'static str>,
        root: Option<&'static str>,
        normalized: &'static str,
    }

    struct ResolveCase {
        base: &'static str,
        other: &'static str,
        result: &'static str,
    }

    struct RelativizeCase {
        base: &'static str,
        other: &'static str,
        result: &'static str,
    }

    const PATH_CASES: &[PathCase] = &[
    PathCase { input: r#"C:\foo\bar"#, display: r#"C:\foo\bar"#, absolute: true, file_name: Some(r#"bar"#), parent: Some(r#"C:\foo"#), root: Some(r#"C:\"#), normalized: r#"C:\foo\bar"# },
    PathCase { input: r#"C:/foo/bar/"#, display: r#"C:\foo\bar"#, absolute: true, file_name: Some(r#"bar"#), parent: Some(r#"C:\foo"#), root: Some(r#"C:\"#), normalized: r#"C:\foo\bar"# },
    PathCase { input: r#"foo/bar"#, display: r#"foo\bar"#, absolute: false, file_name: Some(r#"bar"#), parent: Some(r#"foo"#), root: None, normalized: r#"foo\bar"# },
    PathCase { input: r#"foo"#, display: r#"foo"#, absolute: false, file_name: Some(r#"foo"#), parent: None, root: None, normalized: r#"foo"# },
    PathCase { input: r#""#, display: r#""#, absolute: false, file_name: Some(r#""#), parent: None, root: None, normalized: r#""# },
    PathCase { input: r#"C:\"#, display: r#"C:\"#, absolute: true, file_name: None, parent: None, root: Some(r#"C:\"#), normalized: r#"C:\"# },
    PathCase { input: r#"/"#, display: r#"\"#, absolute: false, file_name: None, parent: None, root: Some(r#"\"#), normalized: r#"\"# },
    PathCase { input: r#"C:\foo\..\bar"#, display: r#"C:\foo\..\bar"#, absolute: true, file_name: Some(r#"bar"#), parent: Some(r#"C:\foo\.."#), root: Some(r#"C:\"#), normalized: r#"C:\bar"# },
    PathCase { input: r#"C:\foo\.\bar"#, display: r#"C:\foo\.\bar"#, absolute: true, file_name: Some(r#"bar"#), parent: Some(r#"C:\foo\."#), root: Some(r#"C:\"#), normalized: r#"C:\foo\bar"# },
    PathCase { input: r#"a\b\..\..\c"#, display: r#"a\b\..\..\c"#, absolute: false, file_name: Some(r#"c"#), parent: Some(r#"a\b\..\.."#), root: None, normalized: r#"c"# },
    PathCase { input: r#"..\a"#, display: r#"..\a"#, absolute: false, file_name: Some(r#"a"#), parent: Some(r#".."#), root: None, normalized: r#"..\a"# },
    PathCase { input: r#".\a"#, display: r#".\a"#, absolute: false, file_name: Some(r#"a"#), parent: Some(r#"."#), root: None, normalized: r#"a"# },
    PathCase { input: r#"C:\foo\bar.txt"#, display: r#"C:\foo\bar.txt"#, absolute: true, file_name: Some(r#"bar.txt"#), parent: Some(r#"C:\foo"#), root: Some(r#"C:\"#), normalized: r#"C:\foo\bar.txt"# },
    PathCase { input: r#"C:\foo\bar.tar.gz"#, display: r#"C:\foo\bar.tar.gz"#, absolute: true, file_name: Some(r#"bar.tar.gz"#), parent: Some(r#"C:\foo"#), root: Some(r#"C:\"#), normalized: r#"C:\foo\bar.tar.gz"# },
    PathCase { input: r#"C:\foo\.hidden"#, display: r#"C:\foo\.hidden"#, absolute: true, file_name: Some(r#".hidden"#), parent: Some(r#"C:\foo"#), root: Some(r#"C:\"#), normalized: r#"C:\foo\.hidden"# },
    PathCase { input: r#"C:\foo\bar."#, display: r#"C:\foo\bar."#, absolute: true, file_name: Some(r#"bar."#), parent: Some(r#"C:\foo"#), root: Some(r#"C:\"#), normalized: r#"C:\foo\bar."# },
    PathCase { input: r#"C:\A\B"#, display: r#"C:\A\B"#, absolute: true, file_name: Some(r#"B"#), parent: Some(r#"C:\A"#), root: Some(r#"C:\"#), normalized: r#"C:\A\B"# },
    PathCase { input: r#"c:\a\b\"#, display: r#"c:\a\b"#, absolute: true, file_name: Some(r#"b"#), parent: Some(r#"c:\a"#), root: Some(r#"c:\"#), normalized: r#"c:\a\b"# },
    PathCase { input: r#"C:\foo\\bar"#, display: r#"C:\foo\bar"#, absolute: true, file_name: Some(r#"bar"#), parent: Some(r#"C:\foo"#), root: Some(r#"C:\"#), normalized: r#"C:\foo\bar"# },
    PathCase { input: r#"\\server\share\dir"#, display: r#"\\server\share\dir"#, absolute: true, file_name: Some(r#"dir"#), parent: Some(r#"\\server\share\"#), root: Some(r#"\\server\share\"#), normalized: r#"\\server\share\dir"# },
    PathCase { input: r#"C:foo"#, display: r#"C:foo"#, absolute: false, file_name: Some(r#"foo"#), parent: Some(r#"C:"#), root: Some(r#"C:"#), normalized: r#"C:foo"# },
    PathCase { input: r#"C:\a b\c.txt"#, display: r#"C:\a b\c.txt"#, absolute: true, file_name: Some(r#"c.txt"#), parent: Some(r#"C:\a b"#), root: Some(r#"C:\"#), normalized: r#"C:\a b\c.txt"# },
    PathCase { input: r#"C:\\u4e2d\u6587\x.txt"#, display: r#"C:\u4e2d\u6587\x.txt"#, absolute: true, file_name: Some(r#"x.txt"#), parent: Some(r#"C:\u4e2d\u6587"#), root: Some(r#"C:\"#), normalized: r#"C:\u4e2d\u6587\x.txt"# },
    PathCase { input: r#"C:\..\x"#, display: r#"C:\..\x"#, absolute: true, file_name: Some(r#"x"#), parent: Some(r#"C:\.."#), root: Some(r#"C:\"#), normalized: r#"C:\x"# },
    PathCase { input: r#"C:\a\..\..\b"#, display: r#"C:\a\..\..\b"#, absolute: true, file_name: Some(r#"b"#), parent: Some(r#"C:\a\..\.."#), root: Some(r#"C:\"#), normalized: r#"C:\b"# },
    PathCase { input: r#"\\server\share\..\x"#, display: r#"\\server\share\..\x"#, absolute: true, file_name: Some(r#"x"#), parent: Some(r#"\\server\share\.."#), root: Some(r#"\\server\share\"#), normalized: r#"\\server\share\x"# },
    PathCase { input: r#"C:..\a"#, display: r#"C:..\a"#, absolute: false, file_name: Some(r#"a"#), parent: Some(r#"C:.."#), root: Some(r#"C:"#), normalized: r#"C:..\a"# },
    PathCase { input: r#"."#, display: r#"."#, absolute: false, file_name: Some(r#"."#), parent: None, root: None, normalized: r#""# },
    PathCase { input: r#".."#, display: r#".."#, absolute: false, file_name: Some(r#".."#), parent: None, root: None, normalized: r#".."# },
    PathCase { input: r#"C:"#, display: r#"C:"#, absolute: false, file_name: None, parent: None, root: Some(r#"C:"#), normalized: r#"C:"# },
    PathCase { input: r#"\\server\share\"#, display: r#"\\server\share\"#, absolute: true, file_name: None, parent: None, root: Some(r#"\\server\share\"#), normalized: r#"\\server\share\"# },
    PathCase { input: r#"C:/"#, display: r#"C:\"#, absolute: true, file_name: None, parent: None, root: Some(r#"C:\"#), normalized: r#"C:\"# },
    PathCase { input: r#"C:\a\\..\b"#, display: r#"C:\a\..\b"#, absolute: true, file_name: Some(r#"b"#), parent: Some(r#"C:\a\.."#), root: Some(r#"C:\"#), normalized: r#"C:\b"# },
    PathCase { input: r#"foo\..\bar"#, display: r#"foo\..\bar"#, absolute: false, file_name: Some(r#"bar"#), parent: Some(r#"foo\.."#), root: None, normalized: r#"bar"# },
    PathCase { input: r#"C:\a\b\.."#, display: r#"C:\a\b\.."#, absolute: true, file_name: Some(r#".."#), parent: Some(r#"C:\a\b"#), root: Some(r#"C:\"#), normalized: r#"C:\a"# },
    ];

    const RESOLVE_CASES: &[ResolveCase] = &[
    ResolveCase { base: r#"C:\a\b"#, other: r#"c"#, result: r#"C:\a\b\c"# },
    ResolveCase { base: r#"C:\a\b"#, other: r#"C:\x"#, result: r#"C:\x"# },
    ResolveCase { base: r#"C:\a"#, other: r#"..\x"#, result: r#"C:\a\..\x"# },
    ResolveCase { base: r#"foo"#, other: r#"bar"#, result: r#"foo\bar"# },
    ResolveCase { base: r#"foo"#, other: r#""#, result: r#"foo"# },
    ResolveCase { base: r#"C:\a\b"#, other: r#"c\d"#, result: r#"C:\a\b\c\d"# },
    ResolveCase { base: r#"C:\"#, other: r#"a"#, result: r#"C:\a"# },
    ];

    const RELATIVIZE_CASES: &[RelativizeCase] = &[
    RelativizeCase { base: r#"C:\a\b\c"#, other: r#"C:\a\b\d"#, result: r#"..\d"# },
    RelativizeCase { base: r#"C:\a\b"#, other: r#"C:\a\b"#, result: r#""# },
    RelativizeCase { base: r#"C:\a"#, other: r#"C:\a\b\c"#, result: r#"b\c"# },
    RelativizeCase { base: r#"C:\a\b\c"#, other: r#"C:\a"#, result: r#"..\.."# },
    RelativizeCase { base: r#"C:\a"#, other: r#"D:\b"#, result: r#"IllegalArgumentException"# },
    RelativizeCase { base: r#"a\b"#, other: r#"a\b\c"#, result: r#"c"# },
    RelativizeCase { base: r#"C:\A\B"#, other: r#"c:\a\b\c"#, result: r#"c"# },
    RelativizeCase { base: r#"foo"#, other: r#"bar"#, result: r#"..\bar"# },
    RelativizeCase { base: r#"C:foo"#, other: r#"C:bar"#, result: r#"..\bar"# },
    RelativizeCase { base: r#"C:foo"#, other: r#"C:foo\bar"#, result: r#"bar"# },
    RelativizeCase { base: r#"C:\a\b"#, other: r#"C:\a\b\c\"#, result: r#"c"# },
    RelativizeCase { base: r#"C:\a\b"#, other: r#"a\b"#, result: r#"IllegalArgumentException"# },
    RelativizeCase { base: r#"\\server\share\a"#, other: r#"\\SERVER\share\b"#, result: r#"..\b"# },
    ];

    const STARTS_WITH_CASES: &[(&str, &str, bool)] = &[
    (r#"C:\A\B"#, r#"c:\a"#, true),
    (r#"C:\A\B"#, r#"C:\a\b"#, true),
    (r#"C:\A\B"#, r#"B"#, false),
    (r#"C:\A\B"#, r#"c:\a\b\"#, true),
    (r#"C:\foo\bar.txt"#, r#"bar.txt"#, false),
    (r#"C:\foo\bar"#, r#"b"#, false),
    (r#"C:\A\B"#, r#""#, false),
    (r#"C:\A\B"#, r#"A"#, false),
    (r#"C:\A\B"#, r#"C:\a\b\c"#, false),
    (r#"C:\A\B"#, r#"C:\a\b"#, true),
    ];

    const ENDS_WITH_CASES: &[(&str, &str, bool)] = &[
    (r#"C:\A\B"#, r#"c:\a"#, false),
    (r#"C:\A\B"#, r#"C:\a\b"#, true),
    (r#"C:\A\B"#, r#"B"#, true),
    (r#"C:\A\B"#, r#"c:\a\b\"#, true),
    (r#"C:\foo\bar.txt"#, r#"bar.txt"#, true),
    (r#"C:\foo\bar"#, r#"b"#, false),
    (r#"C:\A\B"#, r#""#, false),
    (r#"C:\A\B"#, r#"A"#, false),
    (r#"C:\A\B"#, r#"C:\a\b\c"#, false),
    (r#"C:\A\B"#, r#"C:\a\b"#, true),
    ];

    const TO_URI_CASES: &[(&str, &str)] = &[
    (r#"C:\a b\c.txt"#, r#"file:///C:/a%20b/c.txt"#),
    (r#"C:\中文\x.txt"#, r#"file:///C:/中文/x.txt"#),
    (r#"C:\a#b\x.txt"#, r#"file:///C:/a%23b/x.txt"#),
    (r#"C:\a%%20b\x.txt"#, r#"file:///C:/a%25%2520b/x.txt"#),
    (r#"C:\foo\bar.txt"#, r#"file:///C:/foo/bar.txt"#),
    (r#"C:\a b\c.txt"#, r#"file:///C:/a%20b/c.txt"#),
    (r#"\\server\share\dir"#, r#"file://server/share/dir"#),
    (r#"C:\"#, r#"file:///C:/"#),
    (r#"C:\a\b%c\x.txt"#, r#"file:///C:/a/b%25c/x.txt"#),
    ];

    const EQUALS_CASES: &[(&str, &str, bool)] = &[
    (r#"C:\A\B"#, r#"c:\a\b"#, true),
    (r#"C:\a\b"#, r#"C:\a\b\"#, true),
    (r#"C:\a"#, r#"C:\a"#, true),
    (r#"C:\a\b"#, r#"C:\a\b\c"#, false),
    (r#"C:\a"#, r#"C:\\"#, false),
    ];

    const FROM_URI_CASES: &[(&str, &str)] = &[
    (r#"file:///C:/foo/bar.txt"#, r#"C:\foo\bar.txt"#),
    (r#"file:///C:/a%20b/c.txt"#, r#"C:\a b\c.txt"#),
    (r#"file:///C:/%E4%B8%AD%E6%96%87/x.txt"#, r#"C:\中文\x.txt"#),
    ];

    fn path(input: &str) -> Path {
        Path::new(input).expect("valid path")
    }

    #[test]
    fn path_cases_match_the_java_oracle() {
        for case in PATH_CASES {
            let p = path(case.input);
            assert_eq!(p.to_java_string(), case.display, "toString {}", case.input);
            assert_eq!(p.is_absolute(), case.absolute, "isAbsolute {}", case.input);
            assert_eq!(
                p.file_name().map(|name| name.to_java_string()).as_deref(),
                case.file_name,
                "getFileName {}",
                case.input
            );
            assert_eq!(
                p.parent().map(|parent| parent.to_java_string()).as_deref(),
                case.parent,
                "getParent {}",
                case.input
            );
            assert_eq!(
                p.root().map(|root| root.to_java_string()).as_deref(),
                case.root,
                "getRoot {}",
                case.input
            );
            assert_eq!(
                p.normalize().to_java_string(),
                case.normalized,
                "normalize {}",
                case.input
            );
        }
    }

    #[test]
    fn resolve_cases_match_the_java_oracle() {
        for case in RESOLVE_CASES {
            let result = path(case.base).resolve(&path(case.other));
            assert_eq!(result.to_java_string(), case.result, "resolve {} {}", case.base, case.other);
        }
    }

    #[test]
    fn relativize_cases_match_the_java_oracle() {
        for case in RELATIVIZE_CASES {
            if case.result == "IllegalArgumentException" {
                assert!(
                    matches!(
                        path(case.base).relativize(&path(case.other)),
                        Err(Error::IllegalArgument { .. })
                    ),
                    "relativize {} {} should fail",
                    case.base,
                    case.other
                );
            } else {
                let result = path(case.base)
                    .relativize(&path(case.other))
                    .expect("relativize");
                assert_eq!(
                    result.to_java_string(),
                    case.result,
                    "relativize {} {}",
                    case.base,
                    case.other
                );
            }
        }
    }

    #[test]
    fn starts_with_and_ends_with_cases_match_the_java_oracle() {
        for (base, other, expected) in STARTS_WITH_CASES {
            assert_eq!(path(base).starts_with(&path(other)), *expected, "startsWith {} {}", base, other);
        }
        for (base, other, expected) in ENDS_WITH_CASES {
            assert_eq!(path(base).ends_with(&path(other)), *expected, "endsWith {} {}", base, other);
        }
    }

    #[test]
    fn uri_cases_match_the_java_oracle() {
        for (input, expected) in TO_URI_CASES {
            assert_eq!(path(input).to_uri().expect("to_uri"), *expected, "toUri {}", input);
        }
        for (uri, expected) in FROM_URI_CASES {
            assert_eq!(Path::from_uri(uri).expect("from_uri").to_java_string(), *expected, "fromUri {}", uri);
        }
        assert!(matches!(
            Path::from_uri("http://example.com/x"),
            Err(Error::UnsupportedUri { .. })
        ));
        // Java oracle: Path.of(URI) rejects non-hierarchical and query/fragment URIs.
        assert!(matches!(
            Path::from_uri("file:relative/x"),
            Err(Error::IllegalArgument { .. })
        ));
        assert!(matches!(
            Path::from_uri("file:/tmp/x?a"),
            Err(Error::IllegalArgument { .. })
        ));
        assert!(matches!(
            Path::from_uri("file:/tmp/x#f"),
            Err(Error::IllegalArgument { .. })
        ));
        if cfg!(windows) {
            // Java oracle: "file:/tmp/x" -> \tmp\x; "file://host/share/x" -> \\host\share\x.
            assert_eq!(
                Path::from_uri("file:/tmp/x").expect("single slash").to_java_string(),
                "\\tmp\\x"
            );
            assert_eq!(
                Path::from_uri("file://host/share/x")
                    .expect("unc")
                    .to_java_string(),
                "\\\\host\\share\\x"
            );
        }
    }

    #[test]
    fn equals_cases_match_the_java_oracle() {
        for (left, right, expected) in EQUALS_CASES {
            assert_eq!(path(left) == path(right), *expected, "equals {} {}", left, right);
        }
    }
}

#[cfg(test)]
mod syntax_tests {
    use super::Path;
    use crate::error::Error;

    fn sep() -> char {
        std::path::MAIN_SEPARATOR
    }

    #[test]
    fn nul_is_rejected() {
        assert!(matches!(Path::new("a\0b"), Err(Error::InvalidPath { .. })));
    }

    #[test]
    fn extension_rules() {
        assert_eq!(Path::new("a/b.txt").unwrap().extension().as_deref(), Some("txt"));
        assert_eq!(Path::new("a/b.tar.gz").unwrap().extension().as_deref(), Some("gz"));
        assert_eq!(Path::new(".hidden").unwrap().extension(), None);
        assert_eq!(Path::new("a/b.").unwrap().extension().as_deref(), Some(""));
        assert_eq!(Path::new("a/b").unwrap().extension(), None);
    }

    #[test]
    fn name_count_and_subpath() {
        let p = Path::new("a/b/c").unwrap();
        assert_eq!(p.name_count(), 3);
        assert_eq!(
            p.subpath(1, 3).unwrap().to_java_string(),
            format!("b{}c", sep())
        );
        assert!(matches!(p.subpath(3, 3), Err(Error::IllegalArgument { .. })));
        assert!(matches!(p.subpath(0, 4), Err(Error::IllegalArgument { .. })));
    }

    #[test]
    fn resolve_and_sibling() {
        let p = Path::new("a/b").unwrap();
        assert_eq!(
            p.resolve(&Path::new("c/d").unwrap()).to_java_string(),
            format!("a{s}b{s}c{s}d", s = sep())
        );
        assert_eq!(
            p.resolve_sibling(&Path::new("d").unwrap()).to_java_string(),
            format!("a{}d", sep())
        );
        // An empty other path is a no-op (Java resolve("") behavior).
        assert_eq!(p.resolve(&Path::new("").unwrap()), p);
    }

    #[test]
    fn normalize_removes_dot_elements() {
        let p = Path::new("a/./b/../c").unwrap();
        assert_eq!(p.to_java_string(), format!("a{s}.{s}b{s}..{s}c", s = sep()));
        assert_eq!(p.normalize().to_java_string(), format!("a{}c", sep()));
        assert_eq!(
            Path::new("./a").unwrap().normalize().to_java_string(),
            "a"
        );
    }

    #[test]
    fn relativize_relative_paths() {
        let base = Path::new("a/b/c").unwrap();
        let child = Path::new("a/b/d").unwrap();
        assert_eq!(
            base.relativize(&child).unwrap().to_java_string(),
            format!("..{}d", sep())
        );
        assert_eq!(
            base.relativize(&base).unwrap().to_java_string(),
            ""
        );
        assert_eq!(
            Path::new("a").unwrap().relativize(&child).unwrap().to_java_string(),
            format!("b{}d", sep())
        );
    }

    #[test]
    fn to_absolute_path_resolves_against_the_working_directory() {
        let relative = Path::new("target/debug").unwrap();
        let absolute = relative.to_absolute_path().expect("absolute path");
        assert!(absolute.is_absolute());
        // The result keeps the relative tail unchanged (Java does not normalize).
        assert!(absolute.to_java_string().ends_with(&format!("target{}debug", sep())));
    }
}
