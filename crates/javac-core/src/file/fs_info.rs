// Origin: com/sun/tools/javac/file/FSInfo.java @ 982d53c (jdk-17.0.19-ga)

//! Get meta-info about files. Default direct (non-caching) implementation.
//! @see CacheFSInfo
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * java.nio.file.Path -> javac-fs Path; Files.* -> javac_fs::Files.
//! * not ported dependencies (TODO(P2), reported): java.util.jar.JarFile/
//!   Manifest (getJarClassPath), java.net.URL (tryResolveFile),
//!   java.nio.file.spi.FileSystemProvider (getJarFSProvider), and the
//!   Context-based instance(Context)/factory registration (CacheFSInfo is a
//!   composition of FSInfo rather than a subclass, so the Context key type
//!   cannot hold both; registered).

use std::collections::HashMap;
use std::io;

use javac_fs::{Files, Path as FsPath};

/// The minimal java.nio.file.attribute.BasicFileAttributes representation used
/// by CacheFSInfo.
///
/// PORTING_RULES: java.nio.file.attribute is not ported; this struct carries
/// the attributes the caching FSInfo consults (registered adapter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileAttributes {
    /// Java: BasicFileAttributes.isDirectory().
    pub is_directory: bool,
    /// Java: BasicFileAttributes.isRegularFile().
    pub is_regular_file: bool,
    /// Java: lastModifiedTime().toMillis().
    pub last_modified_millis: i64,
    /// Java: size().
    pub size: u64,
}

/// Get meta-info about files. Default direct (non-caching) implementation.
///
/// @see CacheFSInfo
pub struct FSInfo;

/// Strip the Windows verbatim path prefix that std::fs::canonicalize adds
/// (\\?\C:\... -> C:\..., \\?\UNC\server\share -> \\server\share);
/// Java Path.toRealPath returns the plain form.
pub(crate) fn strip_verbatim_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
    if !cfg!(windows) {
        return path;
    }
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return std::path::PathBuf::from(format!(r"\\{}", rest));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' {
            return std::path::PathBuf::from(rest.to_string());
        }
    }
    path
}

impl FSInfo {
    /// Java: protected FSInfo() / protected FSInfo(Context context).
    ///
    /// PORTING_RULES: the Context constructor is deferred together with the
    /// Context-based instance() registration (see the file header).
    pub fn new() -> FSInfo {
        FSInfo
    }

    /// Java: public Path getCanonicalFile(Path file).
    pub fn get_canonical_file(&self, file: &FsPath) -> FsPath {
        // Java: try { return file.toRealPath(); } catch (IOException e) {
        // return file.toAbsolutePath().normalize(); }
        match std::fs::canonicalize(file.as_path()) {
            Ok(real) => FsPath::from_path_buf(strip_verbatim_prefix(real)),
            Err(_) => match file.to_absolute_path() {
                Ok(absolute) => absolute.normalize(),
                Err(_) => file.normalize(),
            },
        }
    }

    /// Java: public boolean exists(Path file).
    pub fn exists(&self, file: &FsPath) -> bool {
        Files::exists(file)
    }

    /// Java: public boolean isDirectory(Path file).
    pub fn is_directory(&self, file: &FsPath) -> bool {
        Files::is_directory(file)
    }

    /// Java: public boolean isFile(Path file).
    pub fn is_file(&self, file: &FsPath) -> bool {
        Files::is_regular_file(file)
    }

    /// Java: public List<Path> getJarClassPath(Path file) throws IOException
    /// (FSInfo.java:100-137).
    ///
    /// PORTING_RULES adapter: java.util.jar.JarFile/Manifest and
    /// java.net.URL are not ported as public types; the jar manifest main
    /// attributes are parsed by [parse_manifest_main_attributes] (the JDK 17
    /// oracle records the format rules) and the URL resolution of each
    /// Class-Path element is implemented by [try_resolve_file].
    ///
    /// # Arguments
    ///   - file: the jar file.
    ///
    /// # Returns
    ///   - the Class-Path entries as files (empty when the manifest has no
    ///     Class-Path attribute).
    ///
    /// # Panics
    ///   - none.
    pub fn get_jar_class_path(&self, file: &FsPath) -> Result<Vec<FsPath>, javac_fs::Error> {
        let archive =
            javac_zip::ZipArchive::open(&file.as_path()).map_err(zip_error_to_fs_error)?;
        let Some(bytes) = archive.get_entry("META-INF/MANIFEST.MF") else {
            return Ok(Vec::new());
        };
        let Some(class_path) = parse_manifest_main_attributes(&bytes)?.get("class-path").cloned()
        else {
            return Ok(Vec::new());
        };
        let mut list = Vec::new();
        let base = file.to_uri()?;
        for element in class_path.split(|unit: char| {
            matches!(unit, ' ' | '\t' | '\n' | '\r' | '\u{000C}')
        }) {
            if element.is_empty() {
                continue;
            }
            if let Some(url) = try_resolve_file(&base, element)? {
                list.push(file_url_to_path(&url)?);
            }
        }
        Ok(list)
    }

    // TODO(P2): the upstream getJarFSProvider() (FileSystemProvider lookup) and
    // the static tryResolveFile(URL, String) are not ported: their java.nio.file
    // provider / java.net.URL dependencies do not exist in the port yet.
}

impl Default for FSInfo {
    fn default() -> FSInfo {
        FSInfo::new()
    }
}


/// Map a zip read error to the java.nio.file.IOException equivalent used by
/// FSInfo.getJarClassPath.
fn zip_error_to_fs_error(error: javac_zip::Error) -> javac_fs::Error {
    match error {
        javac_zip::Error::Io(io_error) => javac_fs::Error::Io(io_error),
        other => javac_fs::Error::Io(io::Error::new(io::ErrorKind::InvalidData, other.to_string())),
    }
}

/// An IOException for the manifest/URL text errors of the upstream code.
fn io_error(message: String) -> javac_fs::Error {
    javac_fs::Error::Io(io::Error::new(io::ErrorKind::InvalidData, message))
}

/// Java: the main attributes of java.util.jar.Manifest (JarFile.getManifest),
/// read from META-INF/MANIFEST.MF. The JDK 17 oracle records the format rules:
/// - lines end with CRLF, LF or CR; the main section ends at the first empty line;
/// - a header is "Name: value" (the colon must be followed by a space);
/// - a line starting with a space continues the previous header, its leading
///   space removed and the text appended without a separator
///   ("Class-Path: a.jar" + " b.jar" -> "a.jarb.jar");
/// - header names are case-insensitive.
fn parse_manifest_main_attributes(bytes: &[u8]) -> Result<HashMap<String, String>, javac_fs::Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| io_error(format!("invalid manifest UTF-8: {error}")))?;
    let mut attributes: HashMap<String, String> = HashMap::new();
    let mut main_section = true;
    let mut current_name: Option<String> = None;
    let mut line_number = 0;
    for line in manifest_lines(text) {
        line_number += 1;
        if !main_section {
            break;
        }
        if line.is_empty() {
            main_section = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix(' ') {
            let Some(name) = &current_name else {
                return Err(io_error(format!(
                    "invalid manifest format (line {line_number})"
                )));
            };
            let value = attributes.entry(name.clone()).or_default();
            value.push_str(rest);
            continue;
        }
        let Some(colon) = line.find(':') else {
            return Err(io_error(format!(
                "invalid header field (line {line_number})"
            )));
        };
        let name = &line[..colon];
        let valid_name = !name.is_empty()
            && name
                .chars()
                .all(|unit| unit.is_ascii_alphanumeric() || unit == '-' || unit == '_');
        let Some(value) = line[colon + 1..].strip_prefix(' ') else {
            return Err(io_error(format!(
                "invalid header field (line {line_number})"
            )));
        };
        if !valid_name {
            return Err(io_error(format!(
                "invalid header field (line {line_number})"
            )));
        }
        attributes.insert(name.to_ascii_lowercase(), value.to_string());
        current_name = Some(name.to_ascii_lowercase());
    }
    Ok(attributes)
}

/// Java: Manifest.read line splitting (CRLF, LF and CR are line terminators).
fn manifest_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                lines.push(&text[start..index]);
                index += if bytes.get(index + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                start = index;
            }
            b'\n' => {
                lines.push(&text[start..index]);
                index += 1;
                start = index;
            }
            _ => index += 1,
        }
    }
    if start < bytes.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Java: FSInfo.tryResolveFile(URL base, String input) — returns null for an
/// absolute URL whose scheme is not "file" (ignoring case).
fn try_resolve_file(base: &str, input: &str) -> Result<Option<String>, javac_fs::Error> {
    let resolved = resolve_url(base, input)?;
    if input.contains(':') && !url_protocol(&resolved).eq_ignore_ascii_case("file") {
        return Ok(None);
    }
    Ok(Some(resolved))
}

/// Java: new URL(base, input) for a file base URL. Unknown absolute schemes
/// other than "file" resolve to their own URL (the caller drops them); a
/// one-letter scheme (a Windows drive letter) reports the
/// MalformedURLException ("unknown protocol: c") that the upstream code
/// propagates as an IOException.
fn resolve_url(base: &str, input: &str) -> Result<String, javac_fs::Error> {
    if let Some(colon) = input.find(':') {
        let candidate = &input[..colon];
        let scheme_like = !candidate.is_empty()
            && candidate.chars().next().is_some_and(|unit| unit.is_ascii_alphabetic())
            && candidate
                .chars()
                .all(|unit| unit.is_ascii_alphanumeric() || unit == '+' || unit == '-' || unit == '.');
        if scheme_like {
            if candidate.len() == 1 {
                return Err(io_error(format!(
                    "unknown protocol: {}",
                    candidate.to_ascii_lowercase()
                )));
            }
            let scheme = candidate.to_ascii_lowercase();
            if scheme == "file" {
                return Ok(input.to_string());
            }
            return Ok(format!("{candidate}:{}", &input[colon + 1..]));
        }
    }
    let (authority, base_path) = split_file_url(base)?;
    let mut relative = input.replace('\\', "/");
    if let Some(rest) = relative.strip_prefix("//") {
        // A UNC reference: the first segment is the host.
        let (host, path) = match rest.find('/') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        let path = normalize_url_path(if path.is_empty() { "/" } else { path });
        return Ok(format!("file://{host}{path}"));
    }
    let path = if relative.starts_with('/') {
        relative
    } else {
        let directory = match base_path.rfind('/') {
            Some(index) => &base_path[..=index],
            None => "/",
        };
        format!("{directory}{relative}")
    };
    let path = normalize_url_path(&path);
    Ok(format!("file://{authority}{path}"))
}

/// Split a file URL into authority and path ("file:///x" -> ("", "/x"),
/// "file://host/share" -> ("host", "/share")).
fn split_file_url(url: &str) -> Result<(String, String), javac_fs::Error> {
    let Some(rest) = url.strip_prefix("file:") else {
        return Err(io_error(format!("not a file URL: {url}")));
    };
    if let Some(after) = rest.strip_prefix("//") {
        match after.find('/') {
            Some(index) => Ok((after[..index].to_string(), after[index..].to_string())),
            None => Ok((after.to_string(), "/".to_string())),
        }
    } else if rest.starts_with('/') {
        Ok((String::new(), rest.to_string()))
    } else {
        Err(io_error(format!("not a hierarchical file URL: {url}")))
    }
}

/// Java: URL/URI path normalization for the relative resolution: "." segments
/// are removed and ".." pops the previous segment.
fn normalize_url_path(path: &str) -> String {
    let rooted = path.starts_with('/');
    let mut output: Vec<&str> = Vec::new();
    let segments: Vec<&str> = path.split('/').collect();
    for (index, segment) in segments.iter().enumerate() {
        match *segment {
            "." => {
                if index == segments.len() - 1 {
                    output.push("");
                }
            }
            ".." => {
                // Java's URL path normalization removes exactly one parent
                // segment per "..".
                if let Some(last) = output.last()
                    && !last.is_empty()
                {
                    output.pop();
                }
                if index == segments.len() - 1 {
                    output.push("");
                }
            }
            other => {
                if other.is_empty() && !output.is_empty() {
                    continue;
                }
                output.push(other);
            }
        }
    }
    let joined = output.join("/");
    if rooted && !joined.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
}

/// The scheme of a URL text, if any.
fn url_protocol(url: &str) -> &str {
    match url.find(':') {
        Some(index) => &url[..index],
        None => "",
    }
}

/// Java: Path.of(url.toURI()) for the resolved file URL.
fn file_url_to_path(url: &str) -> Result<FsPath, javac_fs::Error> {
    if !url_protocol(url).eq_ignore_ascii_case("file") {
        return Err(javac_fs::Error::UnsupportedUri {
            uri: url.to_string(),
        });
    }
    FsPath::from_uri(url)
}

#[cfg(test)]
mod tests {
    use super::FSInfo;
    use javac_fs::Path as FsPath;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "javac-file-info-{}-{}-{}",
            tag,
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn queries_on_a_temp_directory() {
        let dir = temp_dir("query");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let dir_path = FsPath::from_path_buf(dir.clone());
        let info = FSInfo::new();
        assert!(info.exists(&dir_path));
        assert!(info.is_directory(&dir_path));
        assert!(!info.is_file(&dir_path));

        let file = dir.join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let file_path = FsPath::from_path_buf(file.clone());
        assert!(info.exists(&file_path));
        assert!(info.is_file(&file_path));
        assert!(!info.is_directory(&file_path));
        assert!(!info.exists(&FsPath::new("no/such/javac-file").expect("path")));

        let canonical = info.get_canonical_file(&dir_path);
        assert!(canonical.is_absolute());
        assert_eq!(canonical, info.get_canonical_file(&dir_path));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for byte in bytes {
            crc ^= *byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    fn write_stored_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let mut data: Vec<u8> = Vec::new();
        let mut central: Vec<u8> = Vec::new();
        for (name, content) in entries {
            let offset = data.len() as u32;
            let name_bytes = name.as_bytes();
            let crc = crc32(content);
            let size = content.len() as u32;
            data.extend(0x0403_4B50u32.to_le_bytes());
            data.extend(20u16.to_le_bytes());
            data.extend(0u16.to_le_bytes());
            data.extend(0u16.to_le_bytes());
            data.extend(0u16.to_le_bytes());
            data.extend(0u16.to_le_bytes());
            data.extend(crc.to_le_bytes());
            data.extend(size.to_le_bytes());
            data.extend(size.to_le_bytes());
            data.extend((name_bytes.len() as u16).to_le_bytes());
            data.extend(0u16.to_le_bytes());
            data.extend(name_bytes);
            data.extend(*content);

            central.extend(0x0201_4B50u32.to_le_bytes());
            central.extend(20u16.to_le_bytes());
            central.extend(20u16.to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(crc.to_le_bytes());
            central.extend(size.to_le_bytes());
            central.extend(size.to_le_bytes());
            central.extend((name_bytes.len() as u16).to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(0u16.to_le_bytes());
            central.extend(0u32.to_le_bytes());
            central.extend(offset.to_le_bytes());
            central.extend(name_bytes);
        }
        let central_offset = data.len() as u32;
        let central_size = central.len() as u32;
        data.extend(central);
        data.extend(0x0605_4B50u32.to_le_bytes());
        data.extend(0u16.to_le_bytes());
        data.extend(0u16.to_le_bytes());
        data.extend((entries.len() as u16).to_le_bytes());
        data.extend((entries.len() as u16).to_le_bytes());
        data.extend(central_size.to_le_bytes());
        data.extend(central_offset.to_le_bytes());
        data.extend(0u16.to_le_bytes());
        std::fs::write(path, data).expect("write zip");
    }


    #[test]
    fn jar_class_path_parses_the_manifest_and_resolves_relative_urls() {
        let dir = temp_dir("jarcp");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let jar = dir.join("y.jar");
        write_stored_zip(
            &jar,
            &[
                (
                    "META-INF/MANIFEST.MF",
                    b"Manifest-Version: 1.0\r\nClass-Path: lib/a.jar ../lib2/b.jar http://host/c.jar\r\n\r\n",
                ),
                ("lib/a.jar", b""),
            ],
        );
        let info = FSInfo::new();
        let jar_path = FsPath::from_path_buf(jar.clone());
        let class_path = info.get_jar_class_path(&jar_path).expect("class path");
        assert_eq!(
            class_path,
            vec![
                FsPath::from_path_buf(dir.join("lib").join("a.jar")),
                FsPath::from_path_buf(dir.parent().expect("parent").join("lib2").join("b.jar")),
            ]
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn manifest_main_attributes_follow_the_oracle() {
        let parse = |text: &str| {
            super::parse_manifest_main_attributes(text.as_bytes())
                .map(|attributes| attributes.get("class-path").cloned())
        };
        assert_eq!(
            parse("Manifest-Version: 1.0\r\nClass-Path: a.jar b.jar\r\n\r\n").unwrap(),
            Some("a.jar b.jar".to_string())
        );
        assert_eq!(
            parse("Manifest-Version: 1.0\r\nClass-Path: a.jar\r\n b.jar\r\n\r\n").unwrap(),
            Some("a.jarb.jar".to_string())
        );
        assert_eq!(
            parse("Manifest-Version: 1.0\r\nClass-Path: a.jar b.ja\r\n r\r\n\r\n").unwrap(),
            Some("a.jar b.jar".to_string())
        );
        assert_eq!(
            parse("Manifest-Version: 1.0\r\n\r\nName: x\nClass-Path: not-main.jar\r\n\r\n").unwrap(),
            None
        );
        assert_eq!(
            parse("Manifest-Version: 1.0\r\nclass-path: lower.jar\r\n\r\n").unwrap(),
            Some("lower.jar".to_string())
        );
        assert_eq!(
            parse("Manifest-Version: 1.0\r\nClass-Path: \r\n\r\n").unwrap(),
            Some(String::new())
        );
        assert_eq!(
            parse("Manifest-Version: 1.0\nClass-Path: lf.jar\n\n").unwrap(),
            Some("lf.jar".to_string())
        );
        assert!(parse("Manifest-Version: 1.0\r\nClass-Path:a.jar\r\n\r\n").is_err());
    }

    #[test]
    fn class_path_url_resolution_follows_the_oracle() {
        let base = "file:///tmp/classes/lib/y.jar";
        assert_eq!(
            super::try_resolve_file(base, "a.jar").unwrap(),
            Some("file:///tmp/classes/lib/a.jar".to_string())
        );
        assert_eq!(
            super::try_resolve_file(base, "sub/b.jar").unwrap(),
            Some("file:///tmp/classes/lib/sub/b.jar".to_string())
        );
        assert_eq!(
            super::try_resolve_file(base, "/abs/c.jar").unwrap(),
            Some("file:///abs/c.jar".to_string())
        );
        assert_eq!(
            super::try_resolve_file(base, "file:///other/d.jar").unwrap(),
            Some("file:///other/d.jar".to_string())
        );
        assert_eq!(super::try_resolve_file(base, "http://h/e.jar").unwrap(), None);
        assert!(super::try_resolve_file(base, "C:/win/f.jar").is_err());
        assert!(super::try_resolve_file(base, "C:\\win\\g.jar").is_err());
        assert_eq!(
            super::try_resolve_file(base, "../up/h.jar").unwrap(),
            Some("file:///tmp/classes/up/h.jar".to_string())
        );
        assert_eq!(
            super::try_resolve_file(base, "file:/tmp/z.jar").unwrap(),
            Some("file:/tmp/z.jar".to_string())
        );
        assert_eq!(
            super::try_resolve_file(base, "\\\\srv\\share\\i.jar").unwrap(),
            Some("file://srv/share/i.jar".to_string())
        );
        let url = super::try_resolve_file(base, "a.jar?x=1").unwrap().expect("url");
        assert!(super::file_url_to_path(&url).is_err());
    }

}
