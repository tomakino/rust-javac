// PORTING ADAPTER: minimal pure-Rust ZIP container reader for javac's file layer.
//!
//! javac reads jmod containers (JDK jmods/*.jmod), jar files on the class path and the ct.sym
//! archive through java.util.zip. This crate implements the read side of the ZIP format used by
//! those containers: end of central directory lookup, central directory parsing, entries by
//! name, stored (0) and deflate (8) payloads, UTF-8 / IBM437 entry names.
//!
//! PORTING_RULES: registered adapter (PORTING_RULES section 9). It replaces the zip crate 9.1.0
//! dependency: that crate pulls windows-sys/getrandom with raw-dylib, which requires dlltool when
//! building on the Windows GNU toolchain - a platform build prerequisite the project does not
//! accept. The only third-party dependency here is miniz_oxide (pure Rust) for deflate.
//!
//! zip64 archives are detected and reported as UnsupportedZip64; the JDK's jmod/jar files (and
//! ct.sym) never use zip64. Encrypted entries are not supported (they never occur in the
//! compiler's inputs).

pub mod cp437;

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Local file header signature.
const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4B50;
/// Central directory file header signature.
const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4B50;
/// End of central directory record signature.
const EOCD_SIGNATURE: u32 = 0x0605_4B50;
/// Zip64 end of central directory locator signature.
const ZIP64_EOCD_LOCATOR_SIGNATURE: u32 = 0x0706_4B50;

const LOCAL_HEADER_SIZE: usize = 30;
const CENTRAL_HEADER_SIZE: usize = 46;
const EOCD_SIZE: usize = 22;
const EOCD_MAX_COMMENT_SIZE: usize = 0xFFFF;
const ZIP64_EOCD_LOCATOR_SIZE: usize = 20;

const METHOD_STORED: u16 = 0;
const METHOD_DEFLATED: u16 = 8;

const FLAG_ENCRYPTED: u16 = 0x0001;
const FLAG_UTF8: u16 = 0x0800;

const ZIP64_SENTINEL_16: u16 = 0xFFFF;
const ZIP64_SENTINEL_32: u32 = 0xFFFF_FFFF;

/// A central directory entry.
///
/// Java: java.util.zip.ZipEntry
#[derive(Debug, Clone)]
pub struct Entry {
    name: String,
    compression_method: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    flags: u16,
    local_header_offset: u64,
}

impl Entry {
    /// Java: ZipEntry.getName()
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Java: ZipEntry.getMethod()
    pub fn compression_method(&self) -> u16 {
        self.compression_method
    }

    /// Java: ZipEntry.getCompressedSize()
    pub fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    /// Java: ZipEntry.getSize()
    pub fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    /// Java: ZipEntry.getCrc()
    pub fn crc32(&self) -> u32 {
        self.crc32
    }

    /// Java: ZipEntry.isDirectory()
    pub fn is_directory(&self) -> bool {
        self.name.ends_with('/')
    }
}

/// Errors produced while opening or reading a zip archive.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io(io::Error),
    /// The file is not a zip archive (no usable end of central directory record).
    NotAZip(String),
    /// The archive structure is damaged or truncated.
    Corrupted(String),
    /// An entry uses a compression method other than stored (0) or deflate (8).
    UnsupportedCompression(u16),
    /// The archive uses the zip64 extension, which the JDK containers never need.
    UnsupportedZip64(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::NotAZip(message) => write!(f, "not a zip archive: {}", message),
            Error::Corrupted(message) => write!(f, "corrupted zip archive: {}", message),
            Error::UnsupportedCompression(method) => {
                write!(f, "unsupported zip compression method: {}", method)
            }
            Error::UnsupportedZip64(message) => write!(f, "unsupported zip64 archive: {}", message),
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

/// A read-only view of a zip archive.
pub struct ZipArchive {
    file: File,
    /// Offset of the zip data inside the file. jmod files prepend a 4-byte header, so the
    /// central directory offsets (which are relative to the zip data) need this base.
    archive_offset: u64,
    entries: Vec<Entry>,
    entry_index: HashMap<String, usize>,
}

impl fmt::Debug for ZipArchive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ZipArchive")
            .field("archive_offset", &self.archive_offset)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl ZipArchive {
    /// Opens a zip archive and reads its central directory.
    ///
    /// Prepended data (the jmod header) is tolerated: the archive offset is derived from the end
    /// of central directory record position exactly like java.util.zip.ZipFile locates it.
    pub fn open(path: &Path) -> Result<ZipArchive, Error> {
        let file = File::open(path).map_err(Error::Io)?;
        let file_len = file.metadata().map_err(Error::Io)?.len();
        if file_len < EOCD_SIZE as u64 {
            return Err(Error::NotAZip(
                "file is too short to hold a zip end of central directory record".to_string(),
            ));
        }

        let mut archive = ZipArchive {
            file,
            archive_offset: 0,
            entries: Vec::new(),
            entry_index: HashMap::new(),
        };

        // Search the end of central directory record in the trailing window (it can be followed
        // by at most 64KB of archive comment).
        let tail_len = file_len.min((EOCD_SIZE + EOCD_MAX_COMMENT_SIZE) as u64) as usize;
        let tail_offset = file_len - tail_len as u64;
        let tail = archive.read_at(tail_offset, tail_len)?;
        let mut eocd_index = None;
        let mut index = tail_len - EOCD_SIZE;
        loop {
            if read_u32(&tail, index) == Some(EOCD_SIGNATURE) {
                let comment_len = read_u16(&tail, index + 20).unwrap_or(0) as usize;
                if index + EOCD_SIZE + comment_len == tail_len {
                    eocd_index = Some(index);
                    break;
                }
            }
            if index == 0 {
                break;
            }
            index -= 1;
        }
        let eocd_index = eocd_index
            .ok_or_else(|| Error::NotAZip("no end of central directory record".to_string()))?;
        let eocd = &tail[eocd_index..eocd_index + EOCD_SIZE];

        let disk_number = require_u16(eocd, 4)?;
        let cd_disk = require_u16(eocd, 6)?;
        let entries_on_disk = require_u16(eocd, 8)?;
        let total_entries = require_u16(eocd, 10)?;
        let cd_size = require_u32(eocd, 12)?;
        let cd_offset = require_u32(eocd, 16)?;

        if total_entries == ZIP64_SENTINEL_16
            || cd_size == ZIP64_SENTINEL_32
            || cd_offset == ZIP64_SENTINEL_32
        {
            return Err(Error::UnsupportedZip64(
                "zip64 end of central directory fields".to_string(),
            ));
        }
        if disk_number != 0 || cd_disk != 0 || entries_on_disk != total_entries {
            return Err(Error::Corrupted(
                "multi-disk zip archives are not supported".to_string(),
            ));
        }
        if eocd_index >= ZIP64_EOCD_LOCATOR_SIZE
            && read_u32(&tail, eocd_index - ZIP64_EOCD_LOCATOR_SIZE) == Some(ZIP64_EOCD_LOCATOR_SIGNATURE)
        {
            return Err(Error::UnsupportedZip64(
                "zip64 end of central directory locator".to_string(),
            ));
        }

        // The central directory ends where the EOCD begins; deriving the archive offset from that
        // position is what lets jmod files (with their 4-byte header) resolve correctly.
        let eocd_file_pos = tail_offset + eocd_index as u64;
        let cd_start = eocd_file_pos.checked_sub(u64::from(cd_size)).ok_or_else(|| {
            Error::Corrupted("central directory size exceeds its position".to_string())
        })?;
        let archive_offset = cd_start.checked_sub(u64::from(cd_offset)).ok_or_else(|| {
            Error::Corrupted(
                "central directory offset does not match the record position".to_string(),
            )
        })?;
        let cd_size_usize = usize::try_from(cd_size)
            .map_err(|_| Error::Corrupted("central directory is too large".to_string()))?;
        let central = archive.read_at(cd_start, cd_size_usize)?;

        archive.archive_offset = archive_offset;
        archive.parse_central_directory(&central, usize::from(total_entries))?;
        Ok(archive)
    }

    /// The entries in central directory order (the order java.util.zip.ZipFile enumerates).
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Java: ZipFile.getEntry(String) followed by reading the entry.
    ///
    /// Returns None when the entry does not exist or cannot be read (unsupported compression,
    /// encryption, damaged data). Use read_entry for the error.
    pub fn get_entry(&self, name: &str) -> Option<Vec<u8>> {
        let index = *self.entry_index.get(name)?;
        self.read_entry(index).ok()
    }

    /// Reads the entry at the given central directory index.
    pub fn get_entry_of(&self, index: usize) -> Option<Vec<u8>> {
        self.read_entry(index).ok()
    }

    /// Reads and decompresses the entry at the given central directory index.
    pub fn read_entry(&self, index: usize) -> Result<Vec<u8>, Error> {
        let entry = self.entries.get(index).ok_or_else(|| {
            Error::Corrupted(format!("entry index {} is out of range", index))
        })?;
        if entry.flags & FLAG_ENCRYPTED != 0 {
            return Err(Error::Corrupted(
                "encrypted zip entries are not supported".to_string(),
            ));
        }

        let local_header_offset = self.archive_offset + entry.local_header_offset;
        let header = self.read_at(local_header_offset, LOCAL_HEADER_SIZE)?;
        if require_u32(&header, 0)? != LOCAL_HEADER_SIGNATURE {
            return Err(Error::Corrupted(format!(
                "bad local file header signature for entry {}",
                entry.name
            )));
        }
        let name_len = u64::from(require_u16(&header, 26)?);
        let extra_len = u64::from(require_u16(&header, 28)?);
        let data_offset =
            local_header_offset + LOCAL_HEADER_SIZE as u64 + name_len + extra_len;
        let compressed_len = usize::try_from(entry.compressed_size).map_err(|_| {
            Error::Corrupted("entry compressed size does not fit in memory".to_string())
        })?;
        let data = self.read_at(data_offset, compressed_len)?;

        match entry.compression_method {
            METHOD_STORED => {
                if data.len() as u64 != entry.uncompressed_size {
                    return Err(Error::Corrupted(format!(
                        "stored entry {} has inconsistent sizes",
                        entry.name
                    )));
                }
                Ok(data)
            }
            METHOD_DEFLATED => {
                let limit = usize::try_from(entry.uncompressed_size).map_err(|_| {
                    Error::Corrupted("entry size does not fit in memory".to_string())
                })?;
                let payload = miniz_oxide::inflate::decompress_to_vec_with_limit(&data, limit)
                    .map_err(|err| {
                        Error::Corrupted(format!("deflate error in {}: {:?}", entry.name, err))
                    })?;
                if payload.len() as u64 != entry.uncompressed_size {
                    return Err(Error::Corrupted(format!(
                        "deflated entry {} has an inconsistent size",
                        entry.name
                    )));
                }
                Ok(payload)
            }
            other => Err(Error::UnsupportedCompression(other)),
        }
    }

    /// The offset of the zip data inside the file (0 for plain zip files, 4 for jmod files).
    pub fn archive_offset(&self) -> u64 {
        self.archive_offset
    }

    /// Parses the central directory file headers.
    fn parse_central_directory(
        &mut self,
        central: &[u8],
        total_entries: usize,
    ) -> Result<(), Error> {
        let mut offset = 0usize;
        for index in 0..total_entries {
            if offset + CENTRAL_HEADER_SIZE > central.len() {
                return Err(Error::Corrupted(
                    "truncated central directory".to_string(),
                ));
            }
            if require_u32(central, offset)? != CENTRAL_HEADER_SIGNATURE {
                return Err(Error::Corrupted(
                    "bad central directory file header signature".to_string(),
                ));
            }
            let flags = require_u16(central, offset + 8)?;
            let compression_method = require_u16(central, offset + 10)?;
            let crc32 = require_u32(central, offset + 16)?;
            let compressed_size = require_u32(central, offset + 20)?;
            let uncompressed_size = require_u32(central, offset + 24)?;
            let name_len = usize::from(require_u16(central, offset + 28)?);
            let extra_len = usize::from(require_u16(central, offset + 30)?);
            let comment_len = usize::from(require_u16(central, offset + 32)?);
            let disk_start = require_u16(central, offset + 34)?;
            let local_header_offset = require_u32(central, offset + 42)?;

            if disk_start == ZIP64_SENTINEL_16
                || compressed_size == ZIP64_SENTINEL_32
                || uncompressed_size == ZIP64_SENTINEL_32
                || local_header_offset == ZIP64_SENTINEL_32
            {
                return Err(Error::UnsupportedZip64(format!(
                    "zip64 central directory fields for entry {}",
                    index
                )));
            }

            let name_start = offset + CENTRAL_HEADER_SIZE;
            let name_end = name_start + name_len;
            if name_end > central.len() {
                return Err(Error::Corrupted(
                    "truncated central directory entry name".to_string(),
                ));
            }
            let name = decode_entry_name(&central[name_start..name_end], flags)?;

            self.entry_index.entry(name.clone()).or_insert(index);
            self.entries.push(Entry {
                name,
                compression_method,
                compressed_size: u64::from(compressed_size),
                uncompressed_size: u64::from(uncompressed_size),
                crc32,
                flags,
                local_header_offset: u64::from(local_header_offset),
            });

            offset = name_end + extra_len + comment_len;
        }
        Ok(())
    }

    /// Reads exactly len bytes at the file offset.
    fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        let mut buffer = vec![0u8; len];
        let mut file = self.file.try_clone().map_err(Error::Io)?;
        file.seek(SeekFrom::Start(offset)).map_err(Error::Io)?;
        file.read_exact(&mut buffer).map_err(|err| match err.kind() {
            io::ErrorKind::UnexpectedEof => Error::Corrupted(format!(
                "zip data is truncated at offset {}",
                offset
            )),
            _ => Error::Io(err),
        })?;
        Ok(buffer)
    }
}

/// Decodes a zip entry name (Java: ZipCoder). Bit 11 of the flags means UTF-8, otherwise IBM437.
fn decode_entry_name(bytes: &[u8], flags: u16) -> Result<String, Error> {
    if flags & FLAG_UTF8 != 0 {
        String::from_utf8(bytes.to_vec())
            .map_err(|_| Error::Corrupted("invalid UTF-8 zip entry name".to_string()))
    } else {
        Ok(cp437::decode_name(bytes))
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn require_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    read_u16(bytes, offset).ok_or_else(|| {
        Error::Corrupted(format!("truncated zip structure at offset {}", offset))
    })
}

fn require_u32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    read_u32(bytes, offset).ok_or_else(|| {
        Error::Corrupted(format!("truncated zip structure at offset {}", offset))
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

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

        // Every recorded entry uses a supported method and has a non-zero size.
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
        // A plain zip archive has no prepended data.
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
        // "caf" + IBM437 0x82 (e acute) + ".txt"
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
    fn rejects_zip64_archives() {
        let data = b"x".to_vec();
        let entries = [TestEntry {
            name: &b"a.txt"[..],
            method: 0,
            data: &data,
            utf8_flag: false,
        }];

        // Saturate the EOCD total-entries field.
        let mut bytes = build_zip(&[], &entries);
        let len = bytes.len();
        bytes[len - 12..len - 10].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let path = write_temp("zip64-eocd", &bytes);
        assert!(matches!(
            ZipArchive::open(&path),
            Err(Error::UnsupportedZip64(_))
        ));
        std::fs::remove_file(&path).ok();

        // Saturate the central directory local header offset.
        let mut bytes = build_zip(&[], &entries);
        let len = bytes.len();
        let eocd = len - 22;
        let cd_size = read_u32(&bytes, eocd + 12).expect("cd size") as usize;
        let cd_start = eocd - cd_size;
        bytes[cd_start + 42..cd_start + 46].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let path = write_temp("zip64-central", &bytes);
        assert!(matches!(
            ZipArchive::open(&path),
            Err(Error::UnsupportedZip64(_))
        ));
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
            Err(Error::UnsupportedCompression(99))
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

        // Truncated: the end of central directory record is cut off.
        let mut bytes = build_zip(&[], &entries);
        let len = bytes.len();
        bytes.truncate(len - 10);
        let truncated = write_temp("truncated", &bytes);
        assert!(matches!(
            ZipArchive::open(&truncated),
            Err(Error::NotAZip(_))
        ));
        std::fs::remove_file(&truncated).ok();

        // Corrupted central directory signature.
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

        // Corrupted local header: the archive opens but the payload cannot be read.
        let mut bytes = build_zip(&[], &entries);
        bytes[0] ^= 0xFF;
        let broken_local = write_temp("corrupted-local", &bytes);
        let archive = ZipArchive::open(&broken_local).expect("central directory is intact");
        assert!(archive.get_entry("a.txt").is_none());
        assert!(archive.read_entry(0).is_err());
        std::fs::remove_file(&broken_local).ok();
    }
}

