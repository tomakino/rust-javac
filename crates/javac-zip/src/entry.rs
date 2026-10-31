//! A central directory entry (java.util.zip.ZipEntry).

use crate::cp437;
use crate::error::Error;

/// Bit 11 of the general purpose flags: the entry name is UTF-8.
pub(crate) const FLAG_UTF8: u16 = 0x0800;
/// Bit 0: the entry is encrypted.
pub(crate) const FLAG_ENCRYPTED: u16 = 0x0001;

/// A central directory entry.
#[derive(Debug, Clone)]
pub struct Entry {
    pub(crate) name: String,
    pub(crate) compression_method: u16,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) crc32: u32,
    pub(crate) flags: u16,
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

/// Decodes a zip entry name (Java: ZipCoder). Bit 11 of the flags means
/// UTF-8, otherwise IBM437.
pub(crate) fn decode_entry_name(bytes: &[u8], flags: u16) -> Result<String, Error> {
    if flags & FLAG_UTF8 != 0 {
        String::from_utf8(bytes.to_vec())
            .map_err(|_| Error::Corrupted("invalid UTF-8 zip entry name".to_string()))
    } else {
        Ok(cp437::decode_name(bytes))
    }
}
