//! The `RCS1` container: ct.sym without the zip overhead.
//!
//! The JDK's archive is a zip of 15,710 entries. Only 11,594 payloads are distinct —
//! many classes are identical across releases — and roughly 3 MB of the file is zip
//! headers and names. `RCS1` keeps the entries, their order and their bytes, but drops
//! the redundancy:
//!
//! - a payload is stored once, however many entries point at it;
//! - names share one pool;
//! - distinct payloads are grouped into 256 KiB blocks and zlib-compressed;
//! - the index (entries, payloads, blocks, names) is one zlib stream.
//!
//! Blocking is what makes the image small: a per-entry stream restarts the dictionary
//! for every class, while a block lets the compressor see repetition across classes.
//! The baseline shrinks from 8.3 MB to 2.4 MB.
//!
//! # Layout (little endian)
//!
//! ```text
//! header (64 bytes)
//! zlib(index_compressed_size bytes)
//!   entry table     entry_count  * 16 bytes  (name offset, name length, payload id, flags)
//!   payload table   payload_count * 16 bytes (block, offset, length)
//!   block table     block_count  * 16 bytes  (data offset, compressed length, raw length)
//!   name pool       name_size bytes          (UTF-8, no terminators)
//! block data        block_count zlib streams of at most 256 KiB raw bytes
//! ```
//!
//! Entry order is preserved from the source archive: [`CtSym`](crate::CtSym) derives
//! the section order and module list from it, so the order is observable behaviour.
//!
//! # Reading
//!
//! Opening inflates the index once (about 2 MB). A payload read decompresses its
//! block, keeps it in a 64-block cache, and returns a slice; later reads of the same
//! block are memory operations. The cache is per instance, which makes `RcsArchive`
//! `!Sync` — use one per thread or share it behind a lock.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::rc::Rc;

/// File magic of an `RCS1` container.
pub const MAGIC: [u8; 4] = *b"RCS1";
/// Current container version.
pub const VERSION: u32 = 1;
const HEADER_SIZE: usize = 64;
const ENTRY_RECORD: usize = 16;
const UNIQUE_RECORD: usize = 16;
const BLOCK_RECORD: usize = 16;
const BLOCK_SIZE: usize = 256 * 1024;
const BLOCK_CACHE_SIZE: usize = 64;
const FLAG_DIRECTORY: u32 = 1;

/// Errors produced while opening or reading an RCS1 archive.
#[derive(Debug)]
pub enum RcsError {
    /// The file could not be read.
    Io(io::Error),
    /// The file does not start with the RCS1 magic.
    NotAnArchive(String),
    /// The container version is not supported by this reader.
    WrongVersion {
        /// Path of the archive, for the message.
        name: String,
        /// Version field found in the header.
        version: u32,
    },
    /// The index or block layout is truncated or inconsistent.
    Corrupted(String),
}

impl std::fmt::Display for RcsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RcsError::Io(error) => write!(f, "I/O error: {}", error),
            RcsError::NotAnArchive(name) => write!(f, "\"{}\" is not an RCS1 archive", name),
            RcsError::WrongVersion { name, version } => {
                write!(f, "\"{}\" has unsupported RCS1 version {}", name, version)
            }
            RcsError::Corrupted(message) => write!(f, "corrupted RCS1 archive: {}", message),
        }
    }
}

impl std::error::Error for RcsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RcsError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for RcsError {
    fn from(error: io::Error) -> RcsError {
        RcsError::Io(error)
    }
}

#[derive(Clone, Copy, Debug)]
struct Unique {
    block: u32,
    offset: u32,
    len: u32,
}

#[derive(Clone, Copy, Debug)]
struct BlockRecord {
    offset: u64,
    compressed_len: u32,
    uncompressed_len: u32,
}

#[derive(Debug)]
struct BlockCache {
    order: VecDeque<u32>,
    data: HashMap<u32, Rc<Vec<u8>>>,
}

impl BlockCache {
    fn new() -> BlockCache {
        BlockCache {
            order: VecDeque::new(),
            data: HashMap::new(),
        }
    }

    fn get(&mut self, index: u32) -> Option<Rc<Vec<u8>>> {
        let data = self.data.get(&index).cloned()?;
        if let Some(position) = self.order.iter().position(|&candidate| candidate == index) {
            self.order.remove(position);
        }
        self.order.push_back(index);
        Some(data)
    }

    fn insert(&mut self, index: u32, data: Rc<Vec<u8>>) {
        if self.data.contains_key(&index) {
            return;
        }
        if self.order.len() >= BLOCK_CACHE_SIZE
            && let Some(oldest) = self.order.pop_front()
        {
            self.data.remove(&oldest);
        }
        self.data.insert(index, data);
        self.order.push_back(index);
    }
}

/// One entry of the archive, in the stored order.
#[derive(Debug, Clone)]
pub struct RcsEntry {
    name: String,
    is_directory: bool,
    unique: u32,
}

impl RcsEntry {
    /// The entry name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this is a directory entry.
    pub fn is_directory(&self) -> bool {
        self.is_directory
    }
}

/// A read-only view of an `RCS1` ct.sym archive.
#[derive(Debug)]
pub struct RcsArchive {
    file: File,
    data_offset: u64,
    entries: Vec<RcsEntry>,
    entry_index: HashMap<String, usize>,
    unique: Vec<Unique>,
    blocks: Vec<BlockRecord>,
    block_cache: RefCell<BlockCache>,
}

impl RcsArchive {
    /// Opens and validates an `RCS1` archive.
    ///
    /// The complete index is read and inflated up front; payload blocks stay on disk and are
    /// decompressed on first access.
    ///
    /// # Errors
    ///
    /// - [`RcsError::Io`] when the file cannot be read;
    /// - [`RcsError::NotAnArchive`] when the file is too short or the magic is not `RCS1`;
    /// - [`RcsError::WrongVersion`] when the header version is not supported;
    /// - [`RcsError::Corrupted`] when a table, an entry reference, or a block extent is
    ///   inconsistent with the index or the file size.
    pub fn open(path: &Path) -> Result<RcsArchive, RcsError> {
        let name = path.display().to_string();
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        if file_len < HEADER_SIZE as u64 {
            return Err(RcsError::NotAnArchive(name));
        }
        let mut header = [0u8; HEADER_SIZE];
        read_at(&file, &mut header, 0)?;
        if header[..4] != MAGIC {
            return Err(RcsError::NotAnArchive(name));
        }
        let version = u32_at(&header, 4);
        if version != VERSION {
            return Err(RcsError::WrongVersion { name, version });
        }
        let entry_count = u32_at(&header, 8) as usize;
        let unique_count = u32_at(&header, 12) as usize;
        let block_count = u32_at(&header, 16) as usize;
        let name_size = u32_at(&header, 20) as usize;
        let index_compressed_size = u32_at(&header, 24) as usize;
        let data_offset = u64_at(&header, 32);

        let compressed = read_table(
            &file,
            HEADER_SIZE as u64,
            index_compressed_size,
            file_len,
        )?;
        let index_bytes = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed)
            .map_err(|_| RcsError::Corrupted("index is not a zlib stream".to_string()))?;

        let entry_table_end = entry_count
            .checked_mul(ENTRY_RECORD)
            .ok_or_else(|| RcsError::Corrupted("entry table is too large".to_string()))?;
        let unique_table_end = entry_table_end
            .checked_add(
                unique_count
                    .checked_mul(UNIQUE_RECORD)
                    .ok_or_else(|| RcsError::Corrupted("unique table is too large".to_string()))?,
            )
            .ok_or_else(|| RcsError::Corrupted("index size overflow".to_string()))?;
        let block_table_end = unique_table_end
            .checked_add(
                block_count
                    .checked_mul(BLOCK_RECORD)
                    .ok_or_else(|| RcsError::Corrupted("block table is too large".to_string()))?,
            )
            .ok_or_else(|| RcsError::Corrupted("index size overflow".to_string()))?;
        let name_pool_end = block_table_end
            .checked_add(name_size)
            .ok_or_else(|| RcsError::Corrupted("name pool overflow".to_string()))?;
        if name_pool_end > index_bytes.len() {
            return Err(RcsError::Corrupted(
                "index tables describe more bytes than the index holds".to_string(),
            ));
        }

        let names = index_bytes
            .get(block_table_end..name_pool_end)
            .ok_or_else(|| RcsError::Corrupted("name pool out of bounds".to_string()))?;
        let mut entries = Vec::with_capacity(entry_count);
        let mut entry_index = HashMap::with_capacity(entry_count);
        for index in 0..entry_count {
            let base = index * ENTRY_RECORD;
            let name_off = u32_at(&index_bytes, base) as usize;
            let name_len = u32_at(&index_bytes, base + 4) as usize;
            let unique = u32_at(&index_bytes, base + 8);
            let flags = u32_at(&index_bytes, base + 12);
            let end = name_off
                .checked_add(name_len)
                .ok_or_else(|| RcsError::Corrupted("name offset overflow".to_string()))?;
            let bytes = names
                .get(name_off..end)
                .ok_or_else(|| RcsError::Corrupted("entry name out of bounds".to_string()))?;
            let name = std::str::from_utf8(bytes)
                .map_err(|_| RcsError::Corrupted("name pool is not UTF-8".to_string()))?
                .to_string();
            entry_index.insert(name.clone(), entries.len());
            entries.push(RcsEntry {
                name,
                is_directory: flags & FLAG_DIRECTORY != 0,
                unique,
            });
        }

        let mut unique = Vec::with_capacity(unique_count);
        for index in 0..unique_count {
            let base = entry_table_end + index * UNIQUE_RECORD;
            unique.push(Unique {
                block: u32_at(&index_bytes, base),
                offset: u32_at(&index_bytes, base + 4),
                len: u32_at(&index_bytes, base + 8),
            });
        }
        let mut blocks = Vec::with_capacity(block_count);
        for index in 0..block_count {
            let base = unique_table_end + index * BLOCK_RECORD;
            blocks.push(BlockRecord {
                offset: u64_at(&index_bytes, base),
                compressed_len: u32_at(&index_bytes, base + 8),
                uncompressed_len: u32_at(&index_bytes, base + 12),
            });
        }

        for record in &blocks {
            let end = data_offset
                .checked_add(record.offset)
                .and_then(|start| start.checked_add(record.compressed_len as u64))
                .ok_or_else(|| RcsError::Corrupted("block offset overflow".to_string()))?;
            if end > file_len {
                return Err(RcsError::Corrupted(format!(
                    "block data ends at {end}, past the file ({file_len})"
                )));
            }
        }
        for record in &entries {
            let unique = unique.get(record.unique as usize).ok_or_else(|| {
                RcsError::Corrupted(format!(
                    "entry refers to unique content {} of {}",
                    record.unique,
                    unique.len()
                ))
            })?;
            let block = blocks.get(unique.block as usize).ok_or_else(|| {
                RcsError::Corrupted(format!(
                    "unique content refers to block {} of {}",
                    unique.block,
                    blocks.len()
                ))
            })?;
            let end = unique
                .offset
                .checked_add(unique.len)
                .ok_or_else(|| RcsError::Corrupted("unique range overflow".to_string()))?;
            if end > block.uncompressed_len {
                return Err(RcsError::Corrupted(format!(
                    "unique content {end} exceeds block size {}",
                    block.uncompressed_len
                )));
            }
        }

        Ok(RcsArchive {
            file,
            data_offset,
            entries,
            entry_index,
            unique,
            blocks,
            block_cache: RefCell::new(BlockCache::new()),
        })
    }

    /// All entries, in the stored order.
    pub fn entries(&self) -> &[RcsEntry] {
        &self.entries
    }

    /// The number of entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the bytes of the named entry, or None when it does not exist.
    ///
    /// # Returns
    ///
    /// The exact payload stored for the entry (an empty vector for a directory entry). `None`
    /// when the name is absent or the owning block cannot be decompressed.
    ///
    /// # Performance
    ///
    /// A hash-map probe, plus one block decompression (at most 256 KiB) on the first access to
    /// that block; cached blocks are returned as slices.
    pub fn get_entry(&self, name: &str) -> Option<Vec<u8>> {
        let index = *self.entry_index.get(name)?;
        let unique = self.unique.get(self.entries[index].unique as usize)?;
        let block = self.read_block(unique.block)?;
        let start = unique.offset as usize;
        let end = start.checked_add(unique.len as usize)?;
        block.get(start..end).map(|slice| slice.to_vec())
    }

    fn read_block(&self, index: u32) -> Option<Rc<Vec<u8>>> {
        if let Some(data) = self.block_cache.borrow_mut().get(index) {
            return Some(data);
        }
        let record = self.blocks.get(index as usize)?;
        let mut compressed = vec![0u8; record.compressed_len as usize];
        read_at(&self.file, &mut compressed, self.data_offset + record.offset).ok()?;
        let mut output = vec![0u8; record.uncompressed_len as usize];
        let mut inflate = zlib_rs::Inflate::new(true, 15);
        let status = inflate
            .decompress(&compressed, &mut output, zlib_rs::InflateFlush::Finish)
            .ok()?;
        if status != zlib_rs::Status::StreamEnd {
            return None;
        }
        if inflate.total_out() as usize != output.len() {
            return None;
        }
        let data = Rc::new(output);
        self.block_cache
            .borrow_mut()
            .insert(index, Rc::clone(&data));
        Some(data)
    }
}

/// Builds and writes an `RCS1` archive.
#[derive(Debug)]
pub struct RcsBuilder {
    entries: Vec<(String, bool, Vec<u8>)>,
}

impl Default for RcsBuilder {
    fn default() -> Self {
        RcsBuilder::new()
    }
}

impl RcsBuilder {
    /// Creates an empty builder.
    pub fn new() -> RcsBuilder {
        RcsBuilder {
            entries: Vec::new(),
        }
    }

    /// Adds an entry; order is preserved (it must match the zip central directory).
    pub fn add_entry(&mut self, name: &str, is_directory: bool, data: Vec<u8>) {
        self.entries
            .push((name.to_string(), is_directory, data));
    }

    /// Writes the archive.
    ///
    /// Entry order is preserved; identical payloads (including the empty directory payload) are
    /// stored once.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the output file cannot be created or written.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        // Deduplicate payloads (directory entries share the empty payload).
        let mut unique_payloads: Vec<Vec<u8>> = Vec::new();
        let mut unique_by_content: HashMap<Vec<u8>, u32> = HashMap::new();
        let mut entry_unique: Vec<u32> = Vec::with_capacity(self.entries.len());
        for (_, _, data) in &self.entries {
            let id = match unique_by_content.get(data) {
                Some(&id) => id,
                None => {
                    let id = unique_payloads.len() as u32;
                    unique_payloads.push(data.clone());
                    unique_by_content.insert(data.clone(), id);
                    id
                }
            };
            entry_unique.push(id);
        }

        // Group unique payloads into blocks, compressing each block.
        let mut block_bytes: Vec<Vec<u8>> = Vec::new();
        let mut blocks: Vec<BlockRecord> = Vec::new();
        let mut current: Vec<u8> = Vec::with_capacity(BLOCK_SIZE);
        let mut unique: Vec<Unique> = Vec::with_capacity(unique_payloads.len());
        for payload in &unique_payloads {
            if payload.is_empty() {
                unique.push(Unique {
                    block: 0,
                    offset: 0,
                    len: 0,
                });
                continue;
            }
            if payload.len() >= BLOCK_SIZE {
                if !current.is_empty() {
                    flush_block(&mut current, &mut blocks, &mut block_bytes);
                }
                let block = blocks.len() as u32;
                current.extend_from_slice(payload);
                flush_block(&mut current, &mut blocks, &mut block_bytes);
                unique.push(Unique {
                    block,
                    offset: 0,
                    len: payload.len() as u32,
                });
                continue;
            }
            if current.len() + payload.len() > BLOCK_SIZE && !current.is_empty() {
                flush_block(&mut current, &mut blocks, &mut block_bytes);
            }
            let block = blocks.len() as u32;
            let offset = current.len() as u32;
            current.extend_from_slice(payload);
            unique.push(Unique {
                block,
                offset,
                len: payload.len() as u32,
            });
        }
        if !current.is_empty() {
            flush_block(&mut current, &mut blocks, &mut block_bytes);
        }
        // Empty payloads point at block 0 offset 0 len 0; block 0 exists unless the
        // archive is empty, which the caller must not produce.
        if blocks.is_empty() {
            blocks.push(BlockRecord {
                offset: 0,
                compressed_len: 0,
                uncompressed_len: 0,
            });
            block_bytes.push(Vec::new());
        }

        // Name pool plus entry table.
        let mut names: Vec<u8> = Vec::new();
        let mut name_spans: Vec<(u32, u32)> = Vec::with_capacity(self.entries.len());
        for (name, _, _) in &self.entries {
            let offset = names.len() as u32;
            names.extend_from_slice(name.as_bytes());
            name_spans.push((offset, name.len() as u32));
        }
        let name_size = names.len();

        let entry_table_end = self.entries.len() * ENTRY_RECORD;
        let unique_table_end = entry_table_end + unique.len() * UNIQUE_RECORD;
        let block_table_end = unique_table_end + blocks.len() * BLOCK_RECORD;
        let index_size = block_table_end + name_size;

        let mut index = Vec::with_capacity(index_size);
        for (position, (_, is_directory, _)) in self.entries.iter().enumerate() {
            let (offset, length) = name_spans[position];
            index.extend_from_slice(&offset.to_le_bytes());
            index.extend_from_slice(&length.to_le_bytes());
            index.extend_from_slice(&entry_unique[position].to_le_bytes());
            index.extend_from_slice(
                &(if *is_directory { FLAG_DIRECTORY } else { 0 }).to_le_bytes(),
            );
        }
        for record in &unique {
            index.extend_from_slice(&record.block.to_le_bytes());
            index.extend_from_slice(&record.offset.to_le_bytes());
            index.extend_from_slice(&record.len.to_le_bytes());
            index.extend_from_slice(&0u32.to_le_bytes());
        }
        for record in &blocks {
            index.extend_from_slice(&record.offset.to_le_bytes());
            index.extend_from_slice(&record.compressed_len.to_le_bytes());
            index.extend_from_slice(&record.uncompressed_len.to_le_bytes());
        }
        index.extend_from_slice(&names);
        debug_assert_eq!(index.len(), index_size);

        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&index, 9);
        let data_off = HEADER_SIZE as u64 + compressed.len() as u64;

        let mut header = Vec::with_capacity(HEADER_SIZE);
        header.extend_from_slice(&MAGIC);
        push_u32(&mut header, VERSION);
        push_u32(&mut header, self.entries.len() as u32);
        push_u32(&mut header, unique.len() as u32);
        push_u32(&mut header, blocks.len() as u32);
        push_u32(&mut header, name_size as u32);
        push_u32(&mut header, compressed.len() as u32);
        push_u32(&mut header, 0);
        push_u64(&mut header, data_off);
        header.resize(HEADER_SIZE, 0);
        debug_assert_eq!(header.len(), HEADER_SIZE);

        let mut out = BufWriter::with_capacity(1 << 20, File::create(path)?);
        out.write_all(&header)?;
        out.write_all(&compressed)?;
        for block in &block_bytes {
            out.write_all(block)?;
        }
        out.flush()
    }
}

/// Compresses the current block, records its extent in `blocks`, and clears it.
fn flush_block(current: &mut Vec<u8>, blocks: &mut Vec<BlockRecord>, block_bytes: &mut Vec<Vec<u8>>) {
    let uncompressed = current.len();
    let bound = zlib_rs::compress_bound(uncompressed);
    let mut output = vec![0u8; bound];
    let (compressed, _code) =
        zlib_rs::compress_slice(&mut output, current, zlib_rs::DeflateConfig::new(9));
    let compressed = compressed.to_vec();
    let offset: u64 = block_bytes.iter().map(|block| block.len() as u64).sum();
    blocks.push(BlockRecord {
        offset,
        compressed_len: compressed.len() as u32,
        uncompressed_len: uncompressed as u32,
    });
    block_bytes.push(compressed);
    current.clear();
}

/// Appends a little-endian `u32`.
fn push_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

/// Appends a little-endian `u64`.
fn push_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

/// Reads a little-endian `u32`; the caller guarantees the bounds.
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Reads a little-endian `u64`; the caller guarantees the bounds.
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

/// Reads `size` bytes at `offset`, rejecting ranges that end past `file_len`.
fn read_table(file: &File, offset: u64, size: usize, file_len: u64) -> Result<Vec<u8>, RcsError> {
    let end = offset
        .checked_add(size as u64)
        .ok_or_else(|| RcsError::Corrupted("table offset overflow".to_string()))?;
    if end > file_len {
        return Err(RcsError::Corrupted(format!(
            "table at {offset} with size {size} ends past the file ({file_len})"
        )));
    }
    let mut buffer = vec![0u8; size];
    read_at(file, &mut buffer, offset)?;
    Ok(buffer)
}

/// Reads exactly `buffer.len()` bytes at `offset` without moving the shared file cursor.
fn read_at(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        return file.read_exact_at(buffer, offset);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut position = offset;
        let mut filled = 0usize;
        while filled < buffer.len() {
            let read = file.seek_read(&mut buffer[filled..], position)?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "short RCS1 read",
                ));
            }
            filled += read;
            position += read as u64;
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        use std::io::{Read, Seek, SeekFrom};
        let mut clone = file.try_clone()?;
        clone.seek(SeekFrom::Start(offset))?;
        clone.read_exact(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::{RcsArchive, RcsBuilder};
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("target")
            .join("test-tmp");
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("ravac-ctsym-{}-{name}", std::process::id()))
    }

    #[test]
    fn round_trips_entries_and_deduplicates() {
        let path = temp_path("round-trip.rcs");
        let mut builder = RcsBuilder::new();
        builder.add_entry("C/", true, Vec::new());
        builder.add_entry("C/java.base/", true, Vec::new());
        builder.add_entry("C/java.base/java/lang/Object.sig", false, b"object".to_vec());
        builder.add_entry("D/java.base/java/lang/Object.sig", false, b"object".to_vec());
        builder.add_entry("D/java.base/java/lang/String.sig", false, b"string".to_vec());
        builder.write(&path).expect("write");

        let archive = RcsArchive::open(&path).expect("open");
        assert_eq!(archive.entry_count(), 5);
        assert_eq!(archive.entries()[0].name(), "C/");
        assert!(archive.entries()[0].is_directory());
        assert_eq!(
            archive.get_entry("C/java.base/java/lang/Object.sig"),
            Some(b"object".to_vec())
        );
        assert_eq!(
            archive.get_entry("D/java.base/java/lang/Object.sig"),
            Some(b"object".to_vec())
        );
        assert_eq!(
            archive.get_entry("D/java.base/java/lang/String.sig"),
            Some(b"string".to_vec())
        );
        assert_eq!(archive.get_entry("missing"), None);
        let _ = std::fs::remove_file(&path);
    }
}
