//! The zip archive reader: rawzip parses the central directory and local
//! file headers; zlib-rs inflates DEFLATE payloads.

use std::collections::HashMap;
use std::fs::File;
use std::hash::{BuildHasherDefault, Hasher};
use std::io::Read;
use std::path::Path;

use crate::decompress;
use crate::entry::{decode_entry_name, Entry, FLAG_ENCRYPTED};
use crate::error::Error;
use crate::{EOCD_MAX_COMMENT_SIZE, EOCD_SIGNATURE, EOCD_SIZE, read_u16, read_u32};

const METHOD_STORED: u16 = 0;
const METHOD_DEFLATED: u16 = 8;

/// A fast, non-cryptographic hasher for archive entry names.
#[derive(Default)]
struct FxHasher {
    hash: u64,
}

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.hash
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.hash = (self.hash.rotate_left(5) ^ u64::from(byte))
                .wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
}

type FxBuildHasher = BuildHasherDefault<FxHasher>;

/// Java: the ZipEntry compression method codes.
fn compression_method_code(method: rawzip::CompressionMethod) -> u16 {
    if method == rawzip::CompressionMethod::STORE {
        METHOD_STORED
    } else if method == rawzip::CompressionMethod::DEFLATE {
        METHOD_DEFLATED
    } else {
        u16::MAX
    }
}

/// Map a rawzip structural error to the ported zip error type.
fn corrupted(error: rawzip::Error) -> Error {
    Error::Corrupted(format!("zip: {error}"))
}

/// A read-only view of a zip archive.
pub struct ZipArchive {
    raw: rawzip::ZipArchive<rawzip::FileReader>,
    entries: Vec<Entry>,
    entry_index: HashMap<String, usize, FxBuildHasher>,
    wayfinders: Vec<rawzip::ZipArchiveEntryWayfinder>,
    archive_offset: u64,
}

impl std::fmt::Debug for ZipArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZipArchive")
            .field("archive_offset", &self.archive_offset)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl ZipArchive {
    /// Opens a zip archive and reads its central directory.
    pub fn open(path: &Path) -> Result<ZipArchive, Error> {
        let file = File::open(path).map_err(Error::Io)?;
        let archive_offset = compute_archive_offset(path);
        // The central directory is parsed once per archive; a large buffer lets
        // rawzip read it in a few positioned reads instead of per-entry reads.
        let mut buffer = vec![0u8; 1024 * 1024];
        let mut raw = rawzip::ZipArchive::<()>::from_file(file, &mut buffer)
            .map_err(|error| Error::NotAZip(format!("zip: {error}")))?;
        let mut entries = Vec::new();
        let mut entry_index: HashMap<String, usize, FxBuildHasher> = HashMap::default();
        let mut wayfinders = Vec::new();
        {
            let mut iter = raw.entries(&mut buffer);
            while let Some(header) = iter.next_entry().map_err(corrupted)? {
                let name = decode_entry_name(header.file_path().as_ref(), header_flags(header.flags()))?;
                let index = entries.len();
                entries.push(Entry {
                    name: name.clone(),
                    compression_method: compression_method_code(header.compression_method()),
                    compressed_size: header.compressed_size_hint(),
                    uncompressed_size: header.uncompressed_size_hint(),
                    crc32: header.crc32(),
                    flags: header_flags(header.flags()),
                });
                wayfinders.push(header.wayfinder());
                entry_index.insert(name, index);
            }
        }
        Ok(ZipArchive {
            raw,
            entries,
            entry_index,
            wayfinders,
            archive_offset,
        })
    }

    /// The entries in central directory order (the order java.util.zip.ZipFile enumerates).
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Java: ZipFile.getEntry(String) followed by reading the entry.
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
        let wayfinder = *self.wayfinders.get(index).ok_or_else(|| {
            Error::Corrupted(format!("entry index {} has no locator", index))
        })?;
        let raw_entry = self.raw.get_entry(wayfinder).map_err(corrupted)?;
        let mut data = Vec::with_capacity(entry.compressed_size as usize);
        raw_entry
            .reader()
            .read_to_end(&mut data)
            .map_err(Error::Io)?;
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
                let size = usize::try_from(entry.uncompressed_size).map_err(|_| {
                    Error::Corrupted("entry size does not fit in memory".to_string())
                })?;
                let payload = decompress::inflate(&data, size, &entry.name)?;
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
}

/// rawzip exposes the raw general-purpose bits (encryption, UTF-8 names,
/// data descriptors, ...); keep them all so name decoding and the entry API
/// match the ported central-directory flags.
fn header_flags(flags: rawzip::EntryFlags) -> u16 {
    flags.bits()
}

/// Derives the offset of the zip data inside the file from the EOCD position
/// (jmod files prepend a 4-byte header). Returns 0 when the record cannot be
/// located; rawzip remains the authority for the actual parse.
fn compute_archive_offset(path: &Path) -> u64 {
    let Ok(file) = File::open(path) else {
        return 0;
    };
    let Ok(len) = file.metadata().map(|meta| meta.len()) else {
        return 0;
    };
    if len < EOCD_SIZE as u64 {
        return 0;
    }
    let tail_len = len.min((EOCD_SIZE + EOCD_MAX_COMMENT_SIZE) as u64) as usize;
    let mut tail = vec![0u8; tail_len];
    let Ok(mut reader) = file.try_clone() else {
        return 0;
    };
    use std::io::Seek;
    if reader.seek(std::io::SeekFrom::Start(len - tail_len as u64)).is_err()
        || reader.read_exact(&mut tail).is_err()
    {
        return 0;
    }
    let mut index = tail_len - EOCD_SIZE;
    loop {
        if read_u32(&tail, index) == Some(EOCD_SIGNATURE) {
            let comment_len = read_u16(&tail, index + 20).unwrap_or(0) as usize;
            if index + EOCD_SIZE + comment_len == tail_len {
                let eocd_file_pos = (len - tail_len as u64) + index as u64;
                let cd_size = read_u32(&tail, index + 12).unwrap_or(0) as u64;
                let cd_offset = read_u32(&tail, index + 16).unwrap_or(0) as u64;
                let Some(cd_start) = eocd_file_pos.checked_sub(cd_size) else {
                    return 0;
                };
                return cd_start.checked_sub(cd_offset).unwrap_or(0);
            }
        }
        if index == 0 {
            break;
        }
        index -= 1;
    }
    0
}
