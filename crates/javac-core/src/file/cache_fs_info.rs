// Origin: com/sun/tools/javac/file/CacheFSInfo.java @ 982d53c (jdk-17.0.19-ga)

//! Caching implementation of FSInfo.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * java.util.concurrent.ConcurrentHashMap -> RefCell<HashMap> (the compiler
//!   model is single-threaded; registered; no observable behavior change);
//! * java.util.Optional<BasicFileAttributes> -> Option<FileAttributes> where
//!   FileAttributes is the minimal attribute adapter of fs_info.rs;
//! * the Java subclass relation (CacheFSInfo extends FSInfo) becomes
//!   composition (base: FSInfo); the cached operations override the base ones;
//! * not ported (TODO(P2), reported): preRegister(Context) (Context factory for
//!   the FSInfo type cannot hold a CacheFSInfo with the composition mapping),
//!   and super.getJarClassPath (java.util.jar.JarFile/Manifest are not ported).

use super::fs_info::{FSInfo, FileAttributes};
use javac_fs::{Files, Path as FsPath};
use std::cell::RefCell;
use std::collections::HashMap;

/// Caching implementation of FSInfo.
pub struct CacheFSInfo {
    /// Java: the inherited FSInfo behavior (subclass -> composition).
    base: FSInfo,
    /// Java: protected final ConcurrentHashMap<Path, Path> canonicalPathCache.
    pub canonical_path_cache: RefCell<HashMap<FsPath, FsPath>>,
    /// Java: protected final ConcurrentHashMap<Path, Optional<BasicFileAttributes>>
    /// attributeCache.
    pub attribute_cache: RefCell<HashMap<FsPath, Option<FileAttributes>>>,
    /// Java: protected final ConcurrentHashMap<Path, List<Path>> jarClassPathCache.
    pub jar_class_path_cache: RefCell<HashMap<FsPath, Vec<FsPath>>>,
}

impl CacheFSInfo {
    /// Java: public CacheFSInfo() (implicit) — the Java constructor is not
    /// declared; CacheFSInfo is created through the Context factory.
    pub fn new() -> CacheFSInfo {
        CacheFSInfo {
            base: FSInfo::new(),
            canonical_path_cache: RefCell::new(HashMap::new()),
            attribute_cache: RefCell::new(HashMap::new()),
            jar_class_path_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Java: public void clearCache().
    pub fn clear_cache(&self) {
        self.canonical_path_cache.borrow_mut().clear();
        self.attribute_cache.borrow_mut().clear();
        self.jar_class_path_cache.borrow_mut().clear();
    }

    /// Java: public Path getCanonicalFile(Path file) — cached.
    pub fn get_canonical_file(&self, file: &FsPath) -> FsPath {
        if let Some(cached) = self.canonical_path_cache.borrow().get(file) {
            return cached.clone();
        }
        let canonical = self.base.get_canonical_file(file);
        self.canonical_path_cache
            .borrow_mut()
            .insert(file.clone(), canonical.clone());
        canonical
    }

    /// Java: public boolean exists(Path file).
    pub fn exists(&self, file: &FsPath) -> bool {
        self.get_attributes(file).is_some()
    }

    /// Java: public boolean isDirectory(Path file).
    pub fn is_directory(&self, file: &FsPath) -> bool {
        self.get_attributes(file)
            .map(|attributes| attributes.is_directory)
            .unwrap_or(false)
    }

    /// Java: public boolean isFile(Path file).
    pub fn is_file(&self, file: &FsPath) -> bool {
        self.get_attributes(file)
            .map(|attributes| attributes.is_regular_file)
            .unwrap_or(false)
    }

    /// Java: public List<Path> getJarClassPath(Path file) throws IOException.
    ///
    /// TODO(P2): the base implementation needs java.util.jar.JarFile/Manifest
    /// (not ported); the cache bookkeeping is kept in place for later wiring.
    pub fn get_jar_class_path(&self, _file: &FsPath) -> Result<Vec<FsPath>, javac_fs::Error> {
        unimplemented!("TODO(P2): CacheFSInfo.getJarClassPath needs java.util.jar.JarFile/Manifest")
    }

    /// Java: protected Optional<BasicFileAttributes> getAttributes(Path file).
    pub fn get_attributes(&self, file: &FsPath) -> Option<FileAttributes> {
        if let Some(cached) = self.attribute_cache.borrow().get(file) {
            return *cached;
        }
        let attributes = self.maybe_read_attributes(file);
        self.attribute_cache
            .borrow_mut()
            .insert(file.clone(), attributes);
        attributes
    }

    /// Java: protected Optional<BasicFileAttributes> maybeReadAttributes(Path file).
    pub fn maybe_read_attributes(&self, file: &FsPath) -> Option<FileAttributes> {
        // Java: try { return Optional.of(Files.readAttributes(...)); }
        //       catch (IOException e) { return Optional.empty(); }
        if !Files::exists(file) {
            return None;
        }
        Some(FileAttributes {
            is_directory: Files::is_directory(file),
            is_regular_file: Files::is_regular_file(file),
            last_modified_millis: Files::last_modified_millis(file).unwrap_or(0),
            size: Files::size(file).unwrap_or(0),
        })
    }
}

impl Default for CacheFSInfo {
    fn default() -> CacheFSInfo {
        CacheFSInfo::new()
    }
}

#[cfg(test)]
mod tests {
    use super::CacheFSInfo;
    use javac_fs::Path as FsPath;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "javac-file-cache-{}-{}-{}",
            tag,
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn attribute_cache_hits_until_cleared() {
        let dir = temp_dir("attrs");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let file_path = FsPath::from_path_buf(file.clone());

        let cache = CacheFSInfo::new();
        assert!(cache.exists(&file_path));
        assert!(cache.is_file(&file_path));
        assert!(!cache.is_directory(&file_path));
        assert!(cache.get_attributes(&file_path).is_some());

        // Deleting the file does not invalidate the cache (Java semantics).
        std::fs::remove_file(&file).expect("remove file");
        assert!(cache.exists(&file_path));
        assert!(cache.is_file(&file_path));

        // clearCache() drops the cached attributes; the next query re-reads.
        cache.clear_cache();
        assert!(!cache.exists(&file_path));
        assert!(!cache.is_file(&file_path));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn canonical_path_cache_is_used_and_cleared() {
        let dir = temp_dir("canonical");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let dir_path = FsPath::from_path_buf(dir.clone());
        let cache = CacheFSInfo::new();
        let first = cache.get_canonical_file(&dir_path);
        let second = cache.get_canonical_file(&dir_path);
        assert_eq!(first, second);
        assert!(first.is_absolute());
        cache.clear_cache();
        // After clearing, the path is computed again (same directory exists).
        let third = cache.get_canonical_file(&dir_path);
        assert_eq!(first, third);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    #[should_panic(expected = "TODO(P2)")]
    fn jar_class_path_is_deferred() {
        let cache = CacheFSInfo::new();
        let _ = cache.get_jar_class_path(&FsPath::new("x.jar").expect("path"));
    }
}
