// PORTING ADAPTER: minimal pure-Rust ZIP container reader for javac's file layer.
//!
//! javac reads jmod containers (JDK jmods/*.jmod), jar files on the class path
//! and the ct.sym archive through java.util.zip. This crate provides the read
//! side of the ZIP format used by those containers: EOCD / central directory /
//! local headers (parsed by rawzip), entries by name, stored (0) and deflate
//! (8) payloads (inflated by zlib-rs), UTF-8 / IBM437 entry names.
//!
//! PORTING_RULES: registered adapter (PORTING_RULES section 9). The zip reader
//! is implemented on pure-Rust dependencies (rawzip + zlib-rs); rawzip also
//! accepts zip64 archives, which java.util.zip.ZipFile supports.

pub mod cp437;
mod archive;
mod decompress;
mod entry;
mod error;

pub use archive::ZipArchive;
pub use entry::Entry;
pub use error::Error;

/// Local file header signature.
pub(crate) const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4B50;
/// Central directory file header signature.
pub(crate) const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4B50;
/// End of central directory record signature.
pub(crate) const EOCD_SIGNATURE: u32 = 0x0605_4B50;
/// Zip64 end of central directory locator signature.
pub(crate) const ZIP64_EOCD_LOCATOR_SIGNATURE: u32 = 0x0706_4B50;

pub(crate) const EOCD_SIZE: usize = 22;
pub(crate) const EOCD_MAX_COMMENT_SIZE: usize = 0xFFFF;
pub(crate) const ZIP64_EOCD_LOCATOR_SIZE: usize = 20;

pub(crate) use entry::FLAG_UTF8;

pub(crate) fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Recorded from the baseline JDK 17.0.19 (Temurin 17.0.19+10) through
    // java.util.zip.ZipFile (the class java.util.jar.JarFile builds on): entry names in
    // enumeration order hashed with FNV-1a 64 over their UTF-8 bytes plus one newline each.
    const JMOD_ENTRY_COUNT: usize = 6508;
    const JMOD_ENTRIES_FNV: u64 = 14226484957823009642;
    const JMOD_FIRST_ENTRY: &str = "classes/module-info.class";
    const JMOD_SECOND_ENTRY: &str =
        "classes/META-INF/services/java.nio.file.spi.FileSystemProvider";
    const JMOD_LAST_ENTRIES: [&str; 3] =
        ["lib/vcruntime140_1.dll", "lib/verify.dll", "lib/zip.dll"];
    const JMOD_OBJECT_CLASS_LEN: usize = 1895;
    const JMOD_MODULE_INFO_LEN: usize = 11454;

    const JRTFS_ENTRY_COUNT: usize = 61;
    const JRTFS_ENTRIES_FNV: u64 = 978841351141325665;
    const JRTFS_FIRST_ENTRIES: [&str; 5] = [
        "META-INF/",
        "META-INF/MANIFEST.MF",
        "META-INF/services/java.nio.file.spi.FileSystemProvider",
        "jdk/internal/jimage/BasicImageReader$1.class",
        "jdk/internal/jimage/BasicImageReader$2.class",
    ];
    const JRTFS_LAST_ENTRIES: [&str; 3] = [
        "jdk/internal/jrtfs/SystemImage$1.class",
        "jdk/internal/jrtfs/SystemImage$2.class",
        "jdk/internal/jrtfs/SystemImage.class",
    ];
    const JRTFS_MANIFEST_LEN: usize = 310;
    const JRTFS_SERVICES_LEN: usize = 41;
    const JRTFS_BASIC_IMAGE_READER_LEN: usize = 14875;

    fn java_home() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let home = PathBuf::from(home);
            if home.is_dir() {
                return Some(home);
            }
        }
        let fallback = PathBuf::from(
            r"C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot",
        );
        if fallback.is_dir() {
            Some(fallback)
        } else {
            None
        }
    }

    fn jmod_path() -> Option<PathBuf> {
        let candidate = java_home()?.join("jmods").join("java.base.jmod");
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    }

    fn jrt_fs_jar_path() -> Option<PathBuf> {
        let candidate = java_home()?.join("lib").join("jrt-fs.jar");
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    }

    fn fnv1a64_names(archive: &ZipArchive) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for entry in archive.entries() {
            for byte in entry.name().as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            hash ^= b'\n' as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    #[test]
    fn matches_recorded_jmod_archive() {
        let Some(path) = jmod_path() else {
            eprintln!("skipping matches_recorded_jmod_archive: no JDK jmods found");
            return;
        };
        let archive = ZipArchive::open(&path)
            .unwrap_or_else(|err| panic!("cannot open {}: {}", path.display(), err));
        if archive.entries().len() != JMOD_ENTRY_COUNT {
            eprintln!(
                "skipping the recorded jmod comparison: {} is not the recorded JDK 17 baseline",
                path.display()
            );
            return;
        }

        assert_eq!(fnv1a64_names(&archive), JMOD_ENTRIES_FNV);
        assert_eq!(archive.entries()[0].name(), JMOD_FIRST_ENTRY);
        assert_eq!(archive.entries()[1].name(), JMOD_SECOND_ENTRY);
        let count = archive.entries().len();
        let last: Vec<&str> = archive.entries()[count - 3..]
            .iter()
            .map(Entry::name)
            .collect();
        assert_eq!(last, JMOD_LAST_ENTRIES);
        // jmod files prepend a 4-byte header ("JM" + version) before the zip data.
        assert_eq!(archive.archive_offset(), 4);

        let object = archive
            .get_entry("classes/java/lang/Object.class")
            .expect("Object.class");
        assert_eq!(object.len(), JMOD_OBJECT_CLASS_LEN);
        assert_eq!(&object[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
        let module_info = archive
            .get_entry("classes/module-info.class")
            .expect("module-info.class");
        assert_eq!(module_info.len(), JMOD_MODULE_INFO_LEN);
        assert_eq!(&module_info[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
        assert!(archive.get_entry("classes/no/such/Entry.class").is_none());

        for entry in archive.entries() {
            assert!(
                entry.compression_method() == 0 || entry.compression_method() == 8,
                "unexpected method for {}",
                entry.name()
            );
            assert!(entry.uncompressed_size() > 0 || entry.is_directory());
        }
    }

    #[test]
    fn matches_recorded_jrt_fs_jar() {
        let Some(path) = jrt_fs_jar_path() else {
            eprintln!("skipping matches_recorded_jrt_fs_jar: no JDK lib/jrt-fs.jar found");
            return;
        };
        let archive = ZipArchive::open(&path)
            .unwrap_or_else(|err| panic!("cannot open {}: {}", path.display(), err));

        assert_eq!(archive.entries().len(), JRTFS_ENTRY_COUNT);
        assert_eq!(fnv1a64_names(&archive), JRTFS_ENTRIES_FNV);
        assert_eq!(archive.archive_offset(), 0);

        let first: Vec<&str> = archive.entries().iter().take(5).map(Entry::name).collect();
        assert_eq!(first, JRTFS_FIRST_ENTRIES);
        let last: Vec<&str> = archive.entries()[JRTFS_ENTRY_COUNT - 3..]
            .iter()
            .map(Entry::name)
            .collect();
        assert_eq!(last, JRTFS_LAST_ENTRIES);
        assert!(archive.entries()[0].is_directory());

        let manifest = archive
            .get_entry("META-INF/MANIFEST.MF")
            .expect("MANIFEST.MF");
        assert_eq!(manifest.len(), JRTFS_MANIFEST_LEN);
        assert!(manifest.starts_with(b"Mani"));
        let services = archive
            .get_entry("META-INF/services/java.nio.file.spi.FileSystemProvider")
            .expect("services file");
        assert_eq!(services.len(), JRTFS_SERVICES_LEN);
        assert!(services.starts_with(b"jdk."));
        let reader = archive
            .get_entry("jdk/internal/jimage/BasicImageReader.class")
            .expect("BasicImageReader.class");
        assert_eq!(reader.len(), JRTFS_BASIC_IMAGE_READER_LEN);
        assert_eq!(&reader[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
        assert!(archive.get_entry("no/such/entry.txt").is_none());

        let services_entry = &archive.entries()[2];
        assert_eq!(
            services_entry.name(),
            "META-INF/services/java.nio.file.spi.FileSystemProvider"
        );
        assert_eq!(services_entry.compression_method(), 8);
        assert_eq!(services_entry.uncompressed_size(), JRTFS_SERVICES_LEN as u64);
    }

    struct TestEntry<'a> {
        name: &'a [u8],
        method: u16,
        data: &'a [u8],
        utf8_flag: bool,
    }

    fn push_u16(out: &mut Vec<u8>, value: u16) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    /// Builds a minimal zip archive around the given entries, with optional prepended data.
    fn build_zip(prefix: &[u8], entries: &[TestEntry<'_>]) -> Vec<u8> {
        let mut out = prefix.to_vec();
        let mut central = Vec::new();
        for entry in entries {
            let payload: Vec<u8> = match entry.method {
                0 => entry.data.to_vec(),
                8 => miniz_oxide::deflate::compress_to_vec(entry.data, 6),
                _ => entry.data.to_vec(),
            };
            let name_len = entry.name.len() as u16;
            let local_offset = (out.len() - prefix.len()) as u32;
            let flags = if entry.utf8_flag { FLAG_UTF8 } else { 0 };

            push_u32(&mut out, LOCAL_HEADER_SIGNATURE);
            push_u16(&mut out, 20);
            push_u16(&mut out, flags);
            push_u16(&mut out, entry.method);
            push_u16(&mut out, 0);
            push_u16(&mut out, 0);
            push_u32(&mut out, 0);
            push_u32(&mut out, payload.len() as u32);
            push_u32(&mut out, entry.data.len() as u32);
            push_u16(&mut out, name_len);
            push_u16(&mut out, 0);
            out.extend_from_slice(entry.name);
            out.extend_from_slice(&payload);

            push_u32(&mut central, CENTRAL_HEADER_SIGNATURE);
            push_u16(&mut central, 20);
            push_u16(&mut central, 20);
            push_u16(&mut central, flags);
            push_u16(&mut central, entry.method);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u32(&mut central, 0);
            push_u32(&mut central, payload.len() as u32);
            push_u32(&mut central, entry.data.len() as u32);
            push_u16(&mut central, name_len);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u32(&mut central, 0);
            push_u32(&mut central, local_offset);
            central.extend_from_slice(entry.name);
        }
        let cd_offset = (out.len() - prefix.len()) as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);

        push_u32(&mut out, EOCD_SIGNATURE);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, entries.len() as u16);
        push_u16(&mut out, entries.len() as u16);
        push_u32(&mut out, cd_size);
        push_u32(&mut out, cd_offset);
        push_u16(&mut out, 0);
        out
    }

    fn write_temp(tag: &str, bytes: &[u8]) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "rust-javac-zip-test-{}-{}.zip",
            std::process::id(),
            tag
        ));
        std::fs::write(&path, bytes).expect("write temp zip");
        path
    }

    #[test]
    fn reads_stored_and_deflated_entries() {
        let stored_data = b"hello stored payload".to_vec();
        let deflated_data = b"hello deflated payload hello deflated payload".to_vec();
        let entries = [
            TestEntry {
                name: &b"stored.txt"[..],
                method: 0,
                data: &stored_data,
                utf8_flag: false,
            },
            TestEntry {
                name: &b"dir/deflated.txt"[..],
                method: 8,
                data: &deflated_data,
                utf8_flag: false,
            },
        ];
        let path = write_temp("roundtrip", &build_zip(&[], &entries));
        let archive = ZipArchive::open(&path).expect("open minimal archive");
        assert_eq!(archive.entries().len(), 2);
        assert_eq!(
            archive.get_entry("stored.txt").as_deref(),
            Some(stored_data.as_slice())
        );
        assert_eq!(
            archive.get_entry("dir/deflated.txt").as_deref(),
            Some(deflated_data.as_slice())
        );
        assert_eq!(archive.get_entry_of(0).as_deref(), Some(stored_data.as_slice()));
        assert!(archive.get_entry_of(2).is_none());
        assert_eq!(archive.entries()[0].compression_method(), 0);
        assert_eq!(archive.entries()[1].compression_method(), 8);
        assert_eq!(
            archive.entries()[1].uncompressed_size(),
            deflated_data.len() as u64
        );
        assert!(archive.entries()[1].compressed_size() > 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn tolerates_prepended_data() {
        let data = b"prefixed payload".to_vec();
        let entries = [TestEntry {
            name: &b"a.txt"[..],
            method: 0,
            data: &data,
            utf8_flag: false,
        }];
        let path = write_temp("prefix", &build_zip(&[0x4A, 0x4D, 0x01, 0x00], &entries));
        let archive = ZipArchive::open(&path).expect("open prefixed archive");
        assert_eq!(archive.archive_offset(), 4);
        assert_eq!(archive.get_entry("a.txt").as_deref(), Some(data.as_slice()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn decodes_cp437_and_utf8_names() {
        let cp437_name: &[u8] = &[b'c', b'a', b'f', 0x82, b'.', b't', b'x', b't'];
        let utf8_name = "中文.txt".as_bytes();
        let data = b"x".to_vec();
        let entries = [
            TestEntry {
                name: cp437_name,
                method: 0,
                data: &data,
                utf8_flag: false,
            },
            TestEntry {
                name: utf8_name,
                method: 0,
                data: &data,
                utf8_flag: true,
            },
        ];
        let path = write_temp("names", &build_zip(&[], &entries));
        let archive = ZipArchive::open(&path).expect("open archive");
        assert_eq!(archive.entries()[0].name(), "caf\u{00E9}.txt");
        assert_eq!(archive.entries()[1].name(), "中文.txt");
        assert_eq!(
            archive.get_entry("caf\u{00E9}.txt").as_deref(),
            Some(data.as_slice())
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn supports_zip64_central_directory_fields() {
        let data = b"x".to_vec();
        let entries = [TestEntry {
            name: &b"a.txt"[..],
            method: 0,
            data: &data,
            utf8_flag: false,
        }];

        // Saturate the EOCD total-entries field: rawzip reads the real central
        // directory (java.util.zip.ZipFile accepts these zip64 sentinels too).
        let mut bytes = build_zip(&[], &entries);
        let len = bytes.len();
        bytes[len - 12..len - 10].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let path = write_temp("zip64-eocd", &bytes);
        let archive = ZipArchive::open(&path).expect("open with saturated EOCD entry count");
        assert_eq!(archive.entries().len(), 1);
        assert_eq!(archive.get_entry("a.txt").as_deref(), Some(data.as_slice()));
        std::fs::remove_file(&path).ok();

        // Saturate the central directory local header offset: the archive opens
        // but the entry data cannot be located.
        let mut bytes = build_zip(&[], &entries);
        let len = bytes.len();
        let eocd = len - 22;
        let cd_size = read_u32(&bytes, eocd + 12).expect("cd size") as usize;
        let cd_start = eocd - cd_size;
        bytes[cd_start + 42..cd_start + 46].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let path = write_temp("zip64-central", &bytes);
        let archive = ZipArchive::open(&path).expect("open with saturated local offset");
        assert!(archive.read_entry(0).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reports_unsupported_compression() {
        let data = b"raw".to_vec();
        let entries = [TestEntry {
            name: &b"weird.bin"[..],
            method: 99,
            data: &data,
            utf8_flag: false,
        }];
        let path = write_temp("method99", &build_zip(&[], &entries));
        let archive = ZipArchive::open(&path).expect("open archive");
        assert!(archive.get_entry("weird.bin").is_none());
        assert!(matches!(
            archive.read_entry(0),
            Err(Error::UnsupportedCompression(u16::MAX))
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_malformed_archives() {
        let empty = write_temp("empty", &[]);
        assert!(matches!(ZipArchive::open(&empty), Err(Error::NotAZip(_))));
        std::fs::remove_file(&empty).ok();

        let junk = write_temp("junk", b"this is not a zip archive at all");
        assert!(matches!(ZipArchive::open(&junk), Err(Error::NotAZip(_))));
        std::fs::remove_file(&junk).ok();

        let data = b"x".to_vec();
        let entries = [TestEntry {
            name: &b"a.txt"[..],
            method: 0,
            data: &data,
            utf8_flag: false,
        }];

        let mut bytes = build_zip(&[], &entries);
        let len = bytes.len();
        bytes.truncate(len - 10);
        let truncated = write_temp("truncated", &bytes);
        assert!(matches!(
            ZipArchive::open(&truncated),
            Err(Error::NotAZip(_))
        ));
        std::fs::remove_file(&truncated).ok();

        let mut bytes = build_zip(&[], &entries);
        let len = bytes.len();
        let eocd = len - 22;
        let cd_size = read_u32(&bytes, eocd + 12).expect("cd size") as usize;
        let cd_start = eocd - cd_size;
        bytes[cd_start] ^= 0xFF;
        let corrupted = write_temp("corrupted-central", &bytes);
        assert!(matches!(
            ZipArchive::open(&corrupted),
            Err(Error::Corrupted(_))
        ));
        std::fs::remove_file(&corrupted).ok();

        let mut bytes = build_zip(&[], &entries);
        bytes[0] ^= 0xFF;
        let broken_local = write_temp("corrupted-local", &bytes);
        let archive = ZipArchive::open(&broken_local).expect("central directory is intact");
        assert!(archive.get_entry("a.txt").is_none());
        assert!(archive.read_entry(0).is_err());
        std::fs::remove_file(&broken_local).ok();
    }
}
