// PORTING ADAPTER: JDK 17 jimage container (lib/modules) reader.
// Reference implementation: java.base/jdk/internal/jimage/ImageHeader.java
//   @ 982d53c (jdk-17.0.19-ga), extracted from the baseline JDK src.zip.
//!
//! The jimage header is seven native-order ints at the start of the file. The JDK reads them
//! with ByteBuffer.order(ByteOrder.nativeOrder()); the shipped JDK images are little-endian, so
//! this reader interprets the whole index as little-endian and rejects anything else via the
//! magic check.

use crate::Error;

/// Java: ImageHeader.MAGIC (0xCAFEDADA, a negative int literal in Java).
pub const MAGIC: u32 = 0xCAFE_DADA;

/// Java: ImageHeader.MAJOR_VERSION.
pub const MAJOR_VERSION: i32 = 1;

/// Java: ImageHeader.MINOR_VERSION.
pub const MINOR_VERSION: i32 = 0;

/// Java: ImageHeader.HEADER_SLOTS.
const HEADER_SLOTS: usize = 7;

/// Java: ImageHeader.getHeaderSize() == HEADER_SLOTS * 4.
pub const HEADER_SIZE: usize = HEADER_SLOTS * 4;

/// Java: final class ImageHeader.
#[derive(Debug, Clone, Copy)]
pub struct ImageHeader {
    /// Java: private final int magic; (compared as an unsigned bit pattern here)
    pub magic: u32,
    /// Java: private final int majorVersion;
    pub major_version: i32,
    /// Java: private final int minorVersion;
    pub minor_version: i32,
    /// Java: private final int flags;
    pub flags: i32,
    /// Java: private final int resourceCount;
    pub resource_count: i32,
    /// Java: private final int tableLength;
    pub table_length: i32,
    /// Java: private final int locationsSize;
    pub locations_size: i32,
    /// Java: private final int stringsSize;
    pub strings_size: i32,
}

impl ImageHeader {
    /// Java: static ImageHeader readFrom(IntBuffer buffer)
    ///
    /// the JDK reads an IntBuffer in native byte order; the baseline images are
    /// little-endian, which is what this reader decodes.
    pub fn read_from(bytes: &[u8]) -> Result<ImageHeader, Error> {
        if bytes.len() < HEADER_SIZE {
            return Err(Error::NotAnImage("short image header".to_string()));
        }
        let word = |index: usize| -> i32 {
            i32::from_le_bytes([
                bytes[index * 4],
                bytes[index * 4 + 1],
                bytes[index * 4 + 2],
                bytes[index * 4 + 3],
            ])
        };
        let magic = word(0) as u32;
        let version = word(1);
        let major_version = version >> 16;
        let minor_version = version & 0xFFFF;
        let flags = word(2);
        let resource_count = word(3);
        let table_length = word(4);
        let locations_size = word(5);
        let strings_size = word(6);

        // Rust: the JDK would fail later (negative capacities / huge index) for corrupt
        // headers; rejecting the nonsensical values here keeps the arithmetic checked.
        if table_length < 0 || locations_size < 0 || strings_size < 0 || resource_count < 0 {
            return Err(Error::Corrupted("negative image header field".to_string()));
        }

        Ok(ImageHeader {
            magic,
            major_version,
            minor_version,
            flags,
            resource_count,
            table_length,
            locations_size,
            strings_size,
        })
    }

    /// Java: int getRedirectSize() { return tableLength * 4; }
    pub fn get_redirect_size(&self) -> u64 {
        self.table_length as u64 * 4
    }

    /// Java: int getOffsetsSize() { return tableLength * 4; }
    pub fn get_offsets_size(&self) -> u64 {
        self.table_length as u64 * 4
    }

    /// Java: int getLocationsSize()
    pub fn get_locations_size(&self) -> u64 {
        self.locations_size as u64
    }

    /// Java: int getStringsSize()
    pub fn get_strings_size(&self) -> u64 {
        self.strings_size as u64
    }

    /// Java: int getRedirectOffset() { return getHeaderSize(); }
    pub fn get_redirect_offset(&self) -> u64 {
        HEADER_SIZE as u64
    }

    /// Java: int getOffsetsOffset()
    pub fn get_offsets_offset(&self) -> u64 {
        self.get_redirect_offset() + self.get_redirect_size()
    }

    /// Java: int getLocationsOffset()
    pub fn get_locations_offset(&self) -> u64 {
        self.get_offsets_offset() + self.get_offsets_size()
    }

    /// Java: int getStringsOffset()
    pub fn get_strings_offset(&self) -> u64 {
        self.get_locations_offset() + self.get_locations_size()
    }

    /// Java: int getIndexSize()
    pub fn get_index_size(&self) -> u64 {
        HEADER_SIZE as u64
            + self.get_redirect_size()
            + self.get_offsets_size()
            + self.get_locations_size()
            + self.get_strings_size()
    }
}
