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

    /// Java: public List<Path> getJarClassPath(Path file) throws IOException.
    ///
    /// TODO(P2): needs java.util.jar.JarFile/Manifest and java.net.URL, which
    /// are not ported yet.
    pub fn get_jar_class_path(&self, _file: &FsPath) -> Result<Vec<FsPath>, javac_fs::Error> {
        unimplemented!("TODO(P2): FSInfo.getJarClassPath needs java.util.jar.JarFile/Manifest")
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

    #[test]
    #[should_panic(expected = "TODO(P2)")]
    fn jar_class_path_is_deferred() {
        let info = FSInfo::new();
        let _ = info.get_jar_class_path(&FsPath::new("x.jar").expect("path"));
    }
}
