//! File-URI encoding shared by LSP document links and hover links.
//!
//! One implementation keeps every `file://` link consistent: all bytes except
//! URI path characters are percent-encoded, so Chinese names, spaces, `#` and
//! `%` cannot break the link or be mistaken for a fragment delimiter.

use std::path::Path;

/// Percent-encode a path as a `file:///` URI (no fragment).
pub(crate) fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file://");
    // POSIX absolute paths already start with `/`; Windows paths need the
    // separator after the empty authority (`file:///C:/...`).
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~');
        if keep {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::file_uri;
    use url::Url;

    #[test]
    fn file_uri_percent_encodes_unicode_space_hash_percent() {
        assert_eq!(
            file_uri(std::path::Path::new("C:/中文 目录/a#b%.java")),
            "file:///C:/%E4%B8%AD%E6%96%87%20%E7%9B%AE%E5%BD%95/a%23b%25.java"
        );
    }

    #[test]
    fn encoded_uri_parses_and_keeps_the_fragment() {
        let uri = format!(
            "{}#L4",
            file_uri(std::path::Path::new("C:/中文 目录/a#b%.java"))
        );
        let parsed = Url::parse(&uri).expect("valid URI");
        assert_eq!(parsed.scheme(), "file");
        assert_eq!(parsed.fragment(), Some("L4"));
        assert!(parsed.as_str().contains("%E4%B8%AD%E6%96%87%20%E7%9B%AE%E5%BD%95"));
        assert!(parsed.as_str().contains("a%23b%25.java#L4"));
    }

    #[test]
    fn posix_absolute_path_has_three_slashes() {
        assert_eq!(
            file_uri(std::path::Path::new("/tmp/a.java")),
            "file:///tmp/a.java"
        );
        let parsed = Url::parse(&file_uri(std::path::Path::new("/tmp/a.java")))
            .expect("valid URI");
        assert_eq!(parsed.path(), "/tmp/a.java");
    }
}
