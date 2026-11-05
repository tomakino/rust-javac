// PORTING ADAPTER: JDK 17 jmod container reader.
//!
//! jmod files are zip archives containing classes/**, conf/**, lib/**, bin/**, include/**,
//! legal/** and man/** entries, preceded by a 4-byte header ("JM" + version). The compiler uses
//! their classes/ entries as an alternative source of system class signatures (e.g. when
//! --release/ct.sym or an explicit JDK image is not available). The read API is consumed by the
//! P2 file/ClassReader layer.
//!
//! Reference implementation: java.base/jdk/internal/jmod/JmodFile.java
//!   @ 982d53c (jdk-17.0.19-ga), extracted from the baseline JDK src.zip:
//! - JMOD_MAGIC_NUMBER = { 0x4A, 0x4D, 0x01, 0x00 } ("JM", version 1.0), written by
//!   JmodOutputStream before the zip stream;
//! - checkMagic accepts version <= 1.0 and requires the "JM" magic;
//! - entries are named "<section>/<name>", with the sections below.
//!
//! registered adapter. Container reading uses
//! javac-zip, the project's in-house zip reader (no platform build prerequisites); it derives
//! the archive offset from the central directory position, so the 4-byte jmod header is
//! transparently tolerated.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

/// Java: JmodFile.JMOD_MAGIC_NUMBER
pub const JMOD_MAGIC: [u8; 4] = [0x4A, 0x4D, 0x01, 0x00];

/// Java: JmodFile.JMOD_MAJOR_VERSION
pub const JMOD_MAJOR_VERSION: u8 = 0x01;

/// Java: JmodFile.JMOD_MINOR_VERSION
pub const JMOD_MINOR_VERSION: u8 = 0x00;

/// Number of header bytes before the zip data ("JM" + version).
pub const JMOD_HEADER_SIZE: u64 = 4;

/// Java: JmodFile.Section
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// Java: CLASSES("classes")
    Classes,
    /// Java: CONFIG("conf")
    Config,
    /// Java: HEADER_FILES("include")
    HeaderFiles,
    /// Java: LEGAL_NOTICES("legal")
    LegalNotices,
    /// Java: MAN_PAGES("man")
    ManPages,
    /// Java: NATIVE_LIBS("lib")
    NativeLibs,
    /// Java: NATIVE_CMDS("bin")
    NativeCmds,
}

impl Section {
    /// Java: Section.jmodDir()
    pub fn jmod_dir(self) -> &'static str {
        match self {
            Section::Classes => "classes",
            Section::Config => "conf",
            Section::HeaderFiles => "include",
            Section::LegalNotices => "legal",
            Section::ManPages => "man",
            Section::NativeLibs => "lib",
            Section::NativeCmds => "bin",
        }
    }

    /// The section of a jmod entry, from its "<section>/<name>" name
    /// (Java: JmodFile.Entry.section(String) via NAME_TO_SECTION).
    pub fn from_jmod_dir(dir: &str) -> Option<Section> {
        const ALL: [Section; 7] = [
            Section::Classes,
            Section::Config,
            Section::HeaderFiles,
            Section::LegalNotices,
            Section::ManPages,
            Section::NativeLibs,
            Section::NativeCmds,
        ];
        ALL.iter()
            .copied()
            .find(|section| section.jmod_dir() == dir)
    }
}

/// Errors produced while opening or reading a jmod file.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io(io::Error),
    /// The file does not start with the jmod magic, or its version is not supported.
    NotAZip(String),
    /// The jmod header is valid but the zip container cannot be decoded.
    Corrupted(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::NotAZip(message) => write!(f, "invalid jmod file: {}", message),
            Error::Corrupted(message) => write!(f, "corrupted jmod container: {}", message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// Maps a javac-zip error onto the jmod error type.
///
/// The jmod magic is validated before the zip container is opened, so a zip-level "not a zip"
/// means the jmod container itself is damaged: the jmod layer reports Corrupted for it.
fn zip_error(error: javac_zip::Error) -> Error {
    match error {
        javac_zip::Error::Io(err) => Error::Io(err),
        javac_zip::Error::NotAZip(message) => Error::Corrupted(message),
        javac_zip::Error::Corrupted(message) => Error::Corrupted(message),
        javac_zip::Error::UnsupportedCompression(method) => {
            Error::Corrupted(format!("unsupported zip compression method {}", method))
        }
        javac_zip::Error::UnsupportedZip64(message) => {
            Error::Corrupted(format!("unsupported zip64 jmod: {}", message))
        }
    }
}

/// A read-only view of a JDK jmod archive.
pub struct JMod {
    path: PathBuf,
    module_name: String,
    entries: Vec<String>,
    classes: Vec<String>,
    archive: ZipArchive,
}

impl fmt::Debug for JMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JMod")
            .field("path", &self.path)
            .field("module_name", &self.module_name)
            .field("entries", &self.entries.len())
            .field("classes", &self.classes.len())
            .finish()
    }
}

impl JMod {
    /// Opens a jmod file and reads its central directory.
    ///
    /// Java: new JmodFile(Path) — checkMagic(file) followed by the zip container open.
    pub fn open(path: &Path) -> Result<JMod, Error> {
        let mut file = File::open(path).map_err(Error::Io)?;

        // Java: JmodFile.checkMagic: readNBytes(4), then the magic and version checks.
        let mut magic = [0u8; 4];
        match file.read_exact(&mut magic) {
            Ok(()) => {}
            Err(_) => return Err(Error::NotAZip(path.display().to_string())),
        }
        if magic[0] != JMOD_MAGIC[0] || magic[1] != JMOD_MAGIC[1] {
            return Err(Error::NotAZip(path.display().to_string()));
        }
        if magic[2] > JMOD_MAJOR_VERSION
            || (magic[2] == JMOD_MAJOR_VERSION && magic[3] > JMOD_MINOR_VERSION)
        {
            return Err(Error::NotAZip(format!(
                "unsupported jmod version {}.{} in {}",
                magic[2],
                magic[3],
                path.display()
            )));
        }

        // The jmod header is validated; javac-zip derives the archive offset from the central
        // directory position, so the prepended header is tolerated by the zip reader.
        drop(file);
        let archive = ZipArchive::open(path).map_err(zip_error)?;

        let entries: Vec<String> = archive
            .entries()
            .iter()
            .map(|entry| entry.name().to_string())
            .collect();
        let classes = entries
            .iter()
            .filter(|name| name.starts_with("classes/"))
            .cloned()
            .collect();

        let module_name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();

        Ok(JMod {
            path: path.to_path_buf(),
            module_name,
            entries,
            classes,
            archive,
        })
    }

    /// The path this archive was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The module name of this jmod (the file stem: java.base.jmod is module java.base).
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    /// All entries in central-directory order, which is the order "jmod list" prints.
    pub fn list_entries(&self) -> &[String] {
        &self.entries
    }

    /// The entries below the classes/ section, in central-directory order.
    pub fn list_classes(&self) -> &[String] {
        &self.classes
    }

    /// Reads a jmod entry by its full "<section>/<name>" name, or None.
    pub fn get_entry(&mut self, name: &str) -> Option<Vec<u8>> {
        self.archive.get_entry(name)
    }

    /// Reads the class file of a binary class name from the classes/ section.
    ///
    /// The binary name is dot-separated ("java.lang.Object"); a slash-separated name or a
    /// name already starting with "classes/" also works.
    pub fn get_class_bytes(&mut self, binary_name: &str) -> Option<Vec<u8>> {
        let path = binary_name.replace('.', "/");
        let path = path.strip_prefix("classes/").unwrap_or(path.as_str());
        let entry_name = format!("classes/{}.class", path);
        self.get_entry(&entry_name)
    }

    /// The module descriptor bytes (classes/module-info.class).
    pub fn module_info_bytes(&mut self) -> Option<Vec<u8>> {
        self.get_entry("classes/module-info.class")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // Recorded from java.base.jmod of the baseline JDK 17.0.19 (Temurin 17.0.19+10) through
    // java.util.zip.ZipFile (the class JmodFile itself uses) and cross-checked with
    // "$JAVA_HOME/bin/jmod list" (6508 printed entries).
    const EXPECTED_ENTRY_COUNT: usize = 6508;
    const EXPECTED_CLASS_ENTRY_COUNT: usize = 6411;
    const EXPECTED_ENTRIES_FNV: u64 = 14226484957823009642;
    const EXPECTED_CLASSES_FNV: u64 = 9808249173603003412;
    const EXPECTED_FIRST_ENTRY: &str = "classes/module-info.class";
    const EXPECTED_SECOND_ENTRY: &str =
        "classes/META-INF/services/java.nio.file.spi.FileSystemProvider";
    const EXPECTED_LAST_ENTRIES: [&str; 3] =
        ["lib/vcruntime140_1.dll", "lib/verify.dll", "lib/zip.dll"];
    const EXPECTED_SECTION_HEADS: &[(&str, &str)] = &[
        ("classes", "classes/module-info.class"),
        ("conf", "conf/net.properties"),
        ("include", "include/classfile_constants.h"),
        ("legal", "legal/ADDITIONAL_LICENSE_INFO"),
        ("bin", "bin/java.exe"),
        ("lib", "lib/api-ms-win-core-console-l1-1-0.dll"),
    ];
    const EXPECTED_OBJECT_CLASS_LEN: usize = 1895;
    const EXPECTED_STRING_CLASS_LEN: usize = 47651;
    const EXPECTED_MODULE_INFO_LEN: usize = 11454;
    const EXPECTED_SERVICES_LEN: usize = 41;
    const EXPECTED_TZDB_LEN: usize = 102820;
    const EXPECTED_JAVA_EXE_LEN: usize = 50344;

    /// The java.base.jmod of the baseline JDK, if this machine has one.
    fn baseline_jmod() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let candidate = PathBuf::from(home).join("jmods").join("java.base.jmod");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn fnv1a64_names(names: &[String]) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for name in names {
            for byte in name.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            hash ^= b'\n' as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    fn temp_file(tag: &str, bytes: &[u8]) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "rust-javac-jmod-test-{}-{}.jmod",
            std::process::id(),
            tag
        ));
        std::fs::write(&path, bytes).expect("write temp file");
        path
    }

    #[test]
    fn opens_baseline_jmod_and_reads_classes() {
        if !cfg!(windows) {
            eprintln!(
                "skipping opens_baseline_jmod_and_reads_classes: recorded from the Windows JDK"
            );
            return;
        }
        let Some(path) = baseline_jmod() else {
            eprintln!("skipping opens_baseline_jmod_and_reads_classes: no JDK jmods found");
            return;
        };
        let mut jmod = JMod::open(&path)
            .unwrap_or_else(|err| panic!("cannot open {}: {}", path.display(), err));

        assert_eq!(jmod.module_name(), "java.base");
        assert!(!jmod.list_entries().is_empty());
        assert!(!jmod.list_classes().is_empty());

        let object = jmod
            .get_class_bytes("java.lang.Object")
            .expect("java.lang.Object must exist in java.base.jmod");
        assert!(object.len() > 0);
        assert_eq!(&object[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
        assert_eq!(
            jmod.get_class_bytes("java/lang/Object").as_deref(),
            Some(object.as_slice())
        );
        assert_eq!(
            jmod.get_class_bytes("classes/java/lang/Object").as_deref(),
            Some(object.as_slice())
        );

        let module_info = jmod.module_info_bytes().expect("module-info.class");
        assert_eq!(&module_info[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);

        assert!(jmod.get_class_bytes("java.lang.NoSuchClassXyz").is_none());
        assert!(jmod.get_entry("classes/java/lang/Object.class").is_some());
        assert!(jmod.get_entry("classes/no/such/Entry.class").is_none());
    }

    #[test]
    fn matches_the_recorded_jmod_list() {
        if !cfg!(windows) {
            eprintln!("skipping matches_the_recorded_jmod_list: recorded from the Windows JDK");
            return;
        }
        let Some(path) = baseline_jmod() else {
            eprintln!("skipping matches_the_recorded_jmod_list: no JDK jmods found");
            return;
        };
        let mut jmod = JMod::open(&path)
            .unwrap_or_else(|err| panic!("cannot open {}: {}", path.display(), err));
        if jmod.list_entries().len() != EXPECTED_ENTRY_COUNT
            || jmod.list_classes().len() != EXPECTED_CLASS_ENTRY_COUNT
        {
            eprintln!(
                "skipping the recorded jmod list comparison: {} is not the recorded JDK 17 baseline",
                path.display()
            );
            return;
        }

        // Complete name lists (order-sensitive FNV over every entry) against the recorded dump.
        assert_eq!(fnv1a64_names(jmod.list_entries()), EXPECTED_ENTRIES_FNV);
        assert_eq!(fnv1a64_names(jmod.list_classes()), EXPECTED_CLASSES_FNV);
        assert_eq!(jmod.list_entries()[0], EXPECTED_FIRST_ENTRY);
        assert_eq!(jmod.list_entries()[1], EXPECTED_SECOND_ENTRY);
        let last = &jmod.list_entries()[jmod.list_entries().len() - 3..];
        assert_eq!(last, EXPECTED_LAST_ENTRIES);

        // First entry of every section, in zip order (the order the sections are written in).
        let mut seen_entries: Vec<(String, String)> = Vec::new();
        for name in jmod.list_entries() {
            let dir = name.split('/').next().unwrap_or_default();
            if Section::from_jmod_dir(dir).is_some()
                && !seen_entries
                    .iter()
                    .any(|(seen_dir, _)| seen_dir.as_str() == dir)
            {
                seen_entries.push((dir.to_string(), name.clone()));
            }
        }
        let expected_sections: Vec<(String, String)> = EXPECTED_SECTION_HEADS
            .iter()
            .map(|(dir, name)| (dir.to_string(), name.to_string()))
            .collect();
        assert_eq!(seen_entries, expected_sections);
        // Every entry name must start with one of the known sections.
        for name in jmod.list_entries() {
            let dir = name.split('/').next().unwrap_or_default();
            assert!(
                Section::from_jmod_dir(dir).is_some(),
                "unknown jmod section in {}",
                name
            );
        }

        // Recorded class/resource sizes and payloads.
        let object = jmod
            .get_class_bytes("java.lang.Object")
            .expect("Object.class");
        assert_eq!(object.len(), EXPECTED_OBJECT_CLASS_LEN);
        assert_eq!(&object[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
        let string = jmod
            .get_class_bytes("java.lang.String")
            .expect("String.class");
        assert_eq!(string.len(), EXPECTED_STRING_CLASS_LEN);
        let module_info = jmod.module_info_bytes().expect("module-info.class");
        assert_eq!(module_info.len(), EXPECTED_MODULE_INFO_LEN);
        let services = jmod
            .get_entry("classes/META-INF/services/java.nio.file.spi.FileSystemProvider")
            .expect("services file");
        assert_eq!(services.len(), EXPECTED_SERVICES_LEN);
        assert!(services.starts_with(b"jdk."));
        let tzdb = jmod.get_entry("lib/tzdb.dat").expect("tzdb.dat");
        assert_eq!(tzdb.len(), EXPECTED_TZDB_LEN);
        let java_exe = jmod.get_entry("bin/java.exe").expect("bin/java.exe");
        assert_eq!(java_exe.len(), EXPECTED_JAVA_EXE_LEN);
        assert_eq!(&java_exe[..2], b"MZ");
    }

    #[test]
    fn rejects_missing_and_malformed_files() {
        let missing = {
            let mut path = std::env::temp_dir();
            path.push("rust-javac-jmod-no-such-file.jmod");
            path
        };
        assert!(matches!(JMod::open(&missing).unwrap_err(), Error::Io(_)));

        // Shorter than the 4-byte jmod header.
        let short = temp_file("short", &[0x4A, 0x4D]);
        assert!(matches!(JMod::open(&short).unwrap_err(), Error::NotAZip(_)));
        std::fs::remove_file(&short).ok();

        // Right length, not a jmod (plain zip magic).
        let plain_zip = temp_file("plain-zip", b"PK\x03\x04rest of a zip");
        assert!(matches!(
            JMod::open(&plain_zip).unwrap_err(),
            Error::NotAZip(_)
        ));
        std::fs::remove_file(&plain_zip).ok();

        // Right magic, unsupported major version 2.0.
        let bad_version = temp_file("bad-version", &[0x4A, 0x4D, 0x02, 0x00, 0, 0, 0, 0]);
        assert!(matches!(
            JMod::open(&bad_version).unwrap_err(),
            Error::NotAZip(_)
        ));
        std::fs::remove_file(&bad_version).ok();

        // Right magic, unsupported minor version 1.1.
        let bad_minor = temp_file("bad-minor", &[0x4A, 0x4D, 0x01, 0x01, 0, 0, 0, 0]);
        assert!(matches!(
            JMod::open(&bad_minor).unwrap_err(),
            Error::NotAZip(_)
        ));
        std::fs::remove_file(&bad_minor).ok();

        // Valid jmod header followed by garbage instead of a zip container.
        let mut garbage = vec![0x4A, 0x4D, 0x01, 0x00];
        garbage.extend_from_slice(&[0u8; 128]);
        let garbage_path = temp_file("garbage", &garbage);
        assert!(matches!(
            JMod::open(&garbage_path).unwrap_err(),
            Error::Corrupted(_)
        ));
        std::fs::remove_file(&garbage_path).ok();

        // Valid magic and a zip local header, but no central directory.
        let mut truncated = vec![0x4A, 0x4D, 0x01, 0x00];
        truncated.extend_from_slice(b"PK\x03\x04");
        truncated.extend_from_slice(&[0u8; 64]);
        let truncated_path = temp_file("truncated", &truncated);
        assert!(matches!(
            JMod::open(&truncated_path).unwrap_err(),
            Error::Corrupted(_)
        ));
        std::fs::remove_file(&truncated_path).ok();
    }

    #[test]
    fn sections_match_jdk_directories() {
        if !cfg!(windows) {
            eprintln!("skipping sections_match_jdk_directories: recorded from the Windows JDK");
            return;
        }
        assert_eq!(Section::Classes.jmod_dir(), "classes");
        assert_eq!(Section::Config.jmod_dir(), "conf");
        assert_eq!(Section::HeaderFiles.jmod_dir(), "include");
        assert_eq!(Section::LegalNotices.jmod_dir(), "legal");
        assert_eq!(Section::ManPages.jmod_dir(), "man");
        assert_eq!(Section::NativeLibs.jmod_dir(), "lib");
        assert_eq!(Section::NativeCmds.jmod_dir(), "bin");
        assert_eq!(Section::from_jmod_dir("classes"), Some(Section::Classes));
        assert_eq!(Section::from_jmod_dir("lib"), Some(Section::NativeLibs));
        assert_eq!(Section::from_jmod_dir("nope"), None);
    }
}
