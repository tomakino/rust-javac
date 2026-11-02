//! The zip archive reader: rawzip parses the central directory and local
//! file headers; zlib-rs inflates DEFLATE payloads.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::path::Path;

use crate::decompress;
use crate::entry::{Entry, FLAG_ENCRYPTED, decode_entry_name};
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
            self.hash =
                (self.hash.rotate_left(5) ^ u64::from(byte)).wrapping_mul(0x517c_c1b7_2722_0a95);
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

/// A read-only view of a zip archive. The whole file is resident in memory
/// (`ZipSliceArchive`), so entry reads never touch the file system again: the
/// compiler opens each jar once and then reads classes at random.
pub struct ZipArchive {
    raw: rawzip::ZipSliceArchive<Vec<u8>>,
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
        let data = std::fs::read(path).map_err(Error::Io)?;
        let archive_offset = compute_archive_offset(&data);
        let raw = rawzip::ZipArchive::<()>::from_slice(data)
            .map_err(|error| Error::NotAZip(format!("zip: {error}")))?;
        let mut entries = Vec::new();
        let mut entry_index: HashMap<String, usize, FxBuildHasher> = HashMap::default();
        let mut wayfinders = Vec::new();
        {
            let mut iter = raw.entries();
            while let Some(header) = iter.next_entry().map_err(corrupted)? {
                let name =
                    decode_entry_name(header.file_path().as_ref(), header_flags(header.flags()))?;
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
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| Error::Corrupted(format!("entry index {} is out of range", index)))?;
        if entry.flags & FLAG_ENCRYPTED != 0 {
            return Err(Error::Corrupted(
                "encrypted zip entries are not supported".to_string(),
            ));
        }
        let wayfinder = *self
            .wayfinders
            .get(index)
            .ok_or_else(|| Error::Corrupted(format!("entry index {} has no locator", index)))?;
        let raw_entry = self.raw.get_entry(wayfinder).map_err(corrupted)?;
        let data = raw_entry.data();
        match entry.compression_method {
            METHOD_STORED => {
                if data.len() as u64 != entry.uncompressed_size {
                    return Err(Error::Corrupted(format!(
                        "stored entry {} has inconsistent sizes",
                        entry.name
                    )));
                }
                Ok(data.to_vec())
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
fn compute_archive_offset(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    if len < EOCD_SIZE {
        return 0;
    }
    let tail_len = len.min(EOCD_SIZE + EOCD_MAX_COMMENT_SIZE);
    let tail = &bytes[len - tail_len..];
    let mut index = tail_len - EOCD_SIZE;
    loop {
        if read_u32(tail, index) == Some(EOCD_SIGNATURE) {
            let comment_len = read_u16(tail, index + 20).unwrap_or(0) as usize;
            if index + EOCD_SIZE + comment_len == tail_len {
                let eocd_file_pos = (len - tail_len + index) as u64;
                let cd_size = read_u32(tail, index + 12).unwrap_or(0) as u64;
                let cd_offset = read_u32(tail, index + 16).unwrap_or(0) as u64;
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
