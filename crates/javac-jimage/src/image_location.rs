// PORTING ADAPTER: jimage location attributes and name verification.
// Reference implementation: java.base/jdk/internal/jimage/ImageLocation.java
//   @ 982d53c (jdk-17.0.19-ga), extracted from the baseline JDK src.zip.

//! A jimage location is a variable-length attribute record. Attribute values are strings-table
//! offsets for module/parent/base/extension and content offsets/sizes for the resource data. The
//! full name of a location is reassembled exactly as ImageLocation.getFullName() does it.

use crate::Error;
use crate::image_strings;

/// Java: ImageLocation.ATTRIBUTE_END.
pub const ATTRIBUTE_END: usize = 0;
/// Java: ImageLocation.ATTRIBUTE_MODULE.
pub const ATTRIBUTE_MODULE: usize = 1;
/// Java: ImageLocation.ATTRIBUTE_PARENT.
pub const ATTRIBUTE_PARENT: usize = 2;
/// Java: ImageLocation.ATTRIBUTE_BASE.
pub const ATTRIBUTE_BASE: usize = 3;
/// Java: ImageLocation.ATTRIBUTE_EXTENSION.
pub const ATTRIBUTE_EXTENSION: usize = 4;
/// Java: ImageLocation.ATTRIBUTE_OFFSET.
pub const ATTRIBUTE_OFFSET: usize = 5;
/// Java: ImageLocation.ATTRIBUTE_COMPRESSED.
pub const ATTRIBUTE_COMPRESSED: usize = 6;
/// Java: ImageLocation.ATTRIBUTE_UNCOMPRESSED.
pub const ATTRIBUTE_UNCOMPRESSED: usize = 7;
/// Java: ImageLocation.ATTRIBUTE_COUNT.
pub const ATTRIBUTE_COUNT: usize = 8;

/// Java: protected final long[] attributes;
pub type Attributes = [u64; ATTRIBUTE_COUNT];

/// Java: static long[] decompress(ByteBuffer bytes, int offset)
pub fn decompress(bytes: &[u8], offset: usize) -> Result<Attributes, Error> {
    let mut attributes: Attributes = [0; ATTRIBUTE_COUNT];

    let limit = bytes.len();
    let mut index = offset;
    while index < limit {
        let data = bytes[index] as usize;
        index += 1;
        if data <= 0x7 {
            // ATTRIBUTE_END
            break;
        }
        let kind = data >> 3;
        if kind >= ATTRIBUTE_COUNT {
            return Err(Error::InvalidAttributeKind(kind));
        }

        let length = (data & 0x7) + 1;
        let mut value: u64 = 0;
        for _ in 0..length {
            value <<= 8;
            if index >= limit {
                return Err(Error::MissingAttributeData);
            }
            value |= bytes[index] as u64;
            index += 1;
        }
        attributes[kind] = value;
    }
    Ok(attributes)
}

/// Java: public class ImageLocation
#[derive(Debug, Clone)]
pub struct ImageLocation {
    attributes: Attributes,
    module: String,
    parent: String,
    base: String,
    extension: String,
}

impl ImageLocation {
    /// Java: static ImageLocation readFrom(BasicImageReader reader, int offset) — the reader part
    /// is split: attributes are decoded by the caller, strings are resolved here.
    pub fn from_attributes(strings: &[u8], attributes: Attributes) -> Result<ImageLocation, Error> {
        Ok(ImageLocation {
            module: attribute_string(strings, &attributes, ATTRIBUTE_MODULE)?,
            parent: attribute_string(strings, &attributes, ATTRIBUTE_PARENT)?,
            base: attribute_string(strings, &attributes, ATTRIBUTE_BASE)?,
            extension: attribute_string(strings, &attributes, ATTRIBUTE_EXTENSION)?,
            attributes,
        })
    }

    /// Java: public long[] / protected final long[] attributes;
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// Java: public int getModuleOffset()
    pub fn module_offset(&self) -> u64 {
        self.attributes[ATTRIBUTE_MODULE]
    }

    /// Java: public int getParentOffset()
    pub fn parent_offset(&self) -> u64 {
        self.attributes[ATTRIBUTE_PARENT]
    }

    /// Java: public int getBaseOffset()
    pub fn base_offset(&self) -> u64 {
        self.attributes[ATTRIBUTE_BASE]
    }

    /// Java: public int getExtensionOffset()
    pub fn extension_offset(&self) -> u64 {
        self.attributes[ATTRIBUTE_EXTENSION]
    }

    /// Java: public String getModule()
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Java: public String getParent()
    pub fn parent(&self) -> &str {
        &self.parent
    }

    /// Java: public String getBase()
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Java: public String getExtension()
    pub fn extension(&self) -> &str {
        &self.extension
    }

    /// Java: public long getContentOffset()
    pub fn content_offset(&self) -> u64 {
        self.attributes[ATTRIBUTE_OFFSET]
    }

    /// Java: public long getCompressedSize()
    pub fn compressed_size(&self) -> u64 {
        self.attributes[ATTRIBUTE_COMPRESSED]
    }

    /// Java: public long getUncompressedSize()
    pub fn uncompressed_size(&self) -> u64 {
        self.attributes[ATTRIBUTE_UNCOMPRESSED]
    }

    /// Java: public String getFullName() { return getFullName(false); }
    pub fn full_name(&self) -> String {
        let mut builder = String::new();

        if self.module_offset() != 0 {
            builder.push('/');
            builder.push_str(&self.module);
            builder.push('/');
        }

        if self.parent_offset() != 0 {
            builder.push_str(&self.parent);
            builder.push('/');
        }

        builder.push_str(&self.base);

        if self.extension_offset() != 0 {
            builder.push('.');
            builder.push_str(&self.extension);
        }

        builder
    }
}

/// Java: String getAttributeString(int kind) — the JDK asks ImageStrings.get((int) attributes[kind]).
fn attribute_string(strings: &[u8], attributes: &Attributes, kind: usize) -> Result<String, Error> {
    image_strings::string_from_bytes(strings, attributes[kind] as usize)
}

/// Java: static boolean verify(String name, long[] attributes, ImageStrings strings)
///
/// The name is passed as UTF-16 code units because the JDK indexes it with String.charAt.
pub fn verify(name: &[u16], attributes: &Attributes, strings: &[u8]) -> bool {
    const SLASH: u16 = '/' as u16;

    let length = name.len();
    let mut index = 0usize;
    let module_offset = attributes[ATTRIBUTE_MODULE] as usize;
    if module_offset != 0 && length >= 1 {
        let module_len = match image_strings::match_string(strings, module_offset, name, 1) {
            Some(len) => len,
            None => return false,
        };
        index = module_len + 1;
        if length <= index || name[0] != SLASH || name[index] != SLASH {
            return false;
        }
        // Java: name.charAt(index++)
        index += 1;
    }
    verify_name(
        None,
        name,
        index,
        length,
        0,
        attributes[ATTRIBUTE_PARENT] as usize,
        attributes[ATTRIBUTE_BASE] as usize,
        attributes[ATTRIBUTE_EXTENSION] as usize,
        strings,
    )
}

/// Java: private static boolean verifyName(String module, String name, int index, int length,
///         int moduleOffset, int parentOffset, int baseOffset, int extOffset, ImageStrings strings)
// 1:1 port of the upstream JDK signature; the argument count is part of the format logic.
#[allow(clippy::too_many_arguments)]
fn verify_name(
    module: Option<&[u16]>,
    name: &[u16],
    mut index: usize,
    length: usize,
    module_offset: usize,
    parent_offset: usize,
    base_offset: usize,
    ext_offset: usize,
    strings: &[u8],
) -> bool {
    const SLASH: u16 = '/' as u16;
    const DOT: u16 = '.' as u16;

    if module_offset != 0 {
        let module = match module {
            Some(module) => module,
            None => return false,
        };
        match image_strings::match_string(strings, module_offset, module, 0) {
            Some(len) if len == module.len() => {}
            _ => return false,
        }
    }
    if parent_offset != 0 {
        let parent_len = match image_strings::match_string(strings, parent_offset, name, index) {
            Some(len) => len,
            None => return false,
        };
        index += parent_len;
        if length <= index || name[index] != SLASH {
            return false;
        }
        // Java: name.charAt(index++)
        index += 1;
    }
    let base_len = match image_strings::match_string(strings, base_offset, name, index) {
        Some(len) => len,
        None => return false,
    };
    index += base_len;
    if ext_offset != 0 {
        if length <= index || name[index] != DOT {
            return false;
        }
        // Java: name.charAt(index++)
        index += 1;
        let ext_len = match image_strings::match_string(strings, ext_offset, name, index) {
            Some(len) => len,
            None => return false,
        };
        index += ext_len;
    }
    length == index
}
