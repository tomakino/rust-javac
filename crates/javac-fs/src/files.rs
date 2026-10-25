// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.file.Files (the subset used by javac.file.*: exists,
// isDirectory, isRegularFile, readAllBytes, list, size, getLastModifiedTime).
// PORTING_RULES: the Java static methods are Rust associated functions;
// declared IOException -> Result<_, Error>; the Java "false on error" behavior
// of Files.exists/isDirectory/isRegularFile is preserved.

use crate::error::Error;
use crate::path::Path;
use std::time::UNIX_EPOCH;

/// The java.nio.file.Files query subset.
pub struct Files;

impl Files {
    /// Java: Files.exists(Path) — follows links; false when the status cannot
    /// be determined (missing file, permission error).
    pub fn exists(path: &Path) -> bool {
        std::fs::metadata(path.as_path()).is_ok()
    }

    /// Java: Files.isDirectory(Path) — false for missing paths or errors.
    pub fn is_directory(path: &Path) -> bool {
        std::fs::metadata(path.as_path()).map(|meta| meta.is_dir()).unwrap_or(false)
    }

    /// Java: Files.isRegularFile(Path) — false for missing paths or errors.
    pub fn is_regular_file(path: &Path) -> bool {
        std::fs::metadata(path.as_path()).map(|meta| meta.is_file()).unwrap_or(false)
    }

    /// Java: Files.readAllBytes(Path).
    pub fn read_all_bytes(path: &Path) -> Result<Vec<u8>, Error> {
        Ok(std::fs::read(path.as_path())?)
    }

    /// Java: Files.list(Path) / newDirectoryStream(Path) — the directory
    /// entries in the platform's directory order (Java stream order).
    pub fn list_directory(path: &Path) -> Result<Vec<Path>, Error> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path.as_path())? {
            let entry = entry?;
            entries.push(Path::from_path_buf(entry.path()));
        }
        Ok(entries)
    }

    /// Java: Files.size(Path).
    pub fn size(path: &Path) -> Result<u64, Error> {
        Ok(std::fs::metadata(path.as_path())?.len())
    }

    /// Java: Files.getLastModifiedTime(Path).toMillis() — milliseconds since
    /// the epoch; a pre-epoch time is reported as a negative value.
    pub fn last_modified_millis(path: &Path) -> Result<i64, Error> {
        let modified = std::fs::metadata(path.as_path())?.modified()?;
        let millis = match modified.duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_millis() as i64,
            Err(err) => -(err.duration().as_millis() as i64),
        };
        Ok(millis)
    }
}

#[cfg(test)]
mod tests {
    use super::Files;
    use crate::path::Path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("javac-fs-{}-{}-{}", tag, std::process::id(), unique))
    }

    #[test]
    fn queries_on_a_temp_directory() {
        let dir = temp_dir("query");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let dir_path = Path::from_path_buf(dir.clone());
        assert!(Files::exists(&dir_path));
        assert!(Files::is_directory(&dir_path));
        assert!(!Files::is_regular_file(&dir_path));

        let a = dir.join("a.txt");
        std::fs::write(&a, b"abc").expect("write a.txt");
        let a_path = Path::from_path_buf(a.clone());
        assert!(Files::exists(&a_path));
        assert!(Files::is_regular_file(&a_path));
        assert!(!Files::is_directory(&a_path));
        assert_eq!(Files::read_all_bytes(&a_path).expect("readAllBytes"), b"abc");
        assert_eq!(Files::size(&a_path).expect("size"), 3);
        assert!(Files::last_modified_millis(&a_path).expect("lastModified") > 0);

        std::fs::write(dir.join("b.txt"), b"x").expect("write b.txt");
        std::fs::create_dir(dir.join("sub")).expect("create sub");
        let entries = Files::list_directory(&dir_path).expect("list");
        let names = || -> Vec<String> {
            entries
                .iter()
                .map(|entry| entry.file_name().expect("file name").to_java_string())
                .collect()
        };
        // Recorded oracle (Java Files.newDirectoryStream on the baseline JDK
        // Windows): a.txt, b.txt, c.txt, sub for insertion order b,a,sub,c.
        // Here we created a,b,sub in order, and NTFS enumerates them in name
        // order; on other platforms only the set is asserted.
        #[cfg(windows)]
        {
            assert_eq!(names(), ["a.txt", "b.txt", "sub"]);
        }
        #[cfg(not(windows))]
        {
            let mut sorted = names();
            sorted.sort();
            assert_eq!(sorted, ["a.txt", "b.txt", "sub"]);
        }
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn missing_paths_report_false_or_errors() {
        let missing = Path::new("no/such/javac-fs-file").expect("path");
        assert!(!Files::exists(&missing));
        assert!(!Files::is_directory(&missing));
        assert!(!Files::is_regular_file(&missing));
        assert!(Files::read_all_bytes(&missing).is_err());
        assert!(Files::size(&missing).is_err());
        assert!(Files::last_modified_millis(&missing).is_err());
        assert!(Files::list_directory(&missing).is_err());
    }
}
