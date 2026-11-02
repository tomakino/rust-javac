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
//! * REGISTERED: preRegister(Context) (Context factory for
//!   the FSInfo type cannot hold a CacheFSInfo with the composition mapping),
//!   and super.getJarClassPath (java.util.jar.JarFile/Manifest are REGISTERED as unavailable).

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

    /// Java: public List<Path> getJarClassPath(Path file) throws IOException —
    /// cached (CacheFSInfo.java:85-93).
    ///
    /// # Arguments
    ///   - file: the jar file.
    ///
    /// # Returns
    ///   - the Class-Path entries of the jar manifest.
    ///
    /// # Panics
    ///   - none.
    pub fn get_jar_class_path(&self, file: &FsPath) -> Result<Vec<FsPath>, javac_fs::Error> {
        if let Some(cached) = self.jar_class_path_cache.borrow().get(file) {
            return Ok(cached.clone());
        }
        let jar_class_path = self.base.get_jar_class_path(file)?;
        self.jar_class_path_cache
            .borrow_mut()
            .insert(file.clone(), jar_class_path.clone());
        Ok(jar_class_path)
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
        // catch (IOException e) { return Optional.empty(); }
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

}
