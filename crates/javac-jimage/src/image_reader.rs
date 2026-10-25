// PORTING ADAPTER: jimage index reader (lib/modules).
// Reference implementation: java.base/jdk/internal/jimage/BasicImageReader.java
//   @ 982d53c (jdk-17.0.19-ga), extracted from the baseline JDK src.zip.

//! The jimage index (header, redirect table, offsets table, location attributes and string
//! table) is read into memory once; resource bytes are read from the file on demand. This keeps
//! opening cheap (the index of the baseline JDK 17 image is ~1.5 MB) and avoids a full scan per
//! lookup, without memory mapping.

use std::fs::File;
use std::io;
use std::path::Path;

use crate::image_header::{ImageHeader, HEADER_SIZE, MAGIC, MAJOR_VERSION, MINOR_VERSION};
use crate::image_location::{self, Attributes, ImageLocation, ATTRIBUTE_MODULE};
use crate::image_strings;
use crate::Error;

/// Java: public class BasicImageReader
#[derive(Debug)]
pub struct ImageIndex {
    file: File,
    header: ImageHeader,
    /// Java: private final long indexSize; — resource content starts after the index.
    index_size: u64,
    /// Java: private final IntBuffer redirect;
    redirect: Vec<i32>,
    /// Java: private final IntBuffer offsets;
    offsets: Vec<i32>,
    /// Java: private final ByteBuffer locations;
    locations: Vec<u8>,
    /// Java: private final ByteBuffer strings;
    strings: Vec<u8>,
}

impl ImageIndex {
    /// Java: BasicImageReader.open(Path) -> new BasicImageReader(path, ByteOrder.nativeOrder())
    pub fn open(path: &Path) -> Result<ImageIndex, Error> {
        let name = path.display().to_string();
        let file = File::open(path).map_err(Error::Io)?;
        let file_len = file.metadata().map_err(Error::Io)?.len();

        if file_len < HEADER_SIZE as u64 {
            return Err(Error::NotAnImage(name));
        }
        let mut header_bytes = [0u8; HEADER_SIZE];
        read_at(&file, &mut header_bytes, 0).map_err(Error::Io)?;
        let header = ImageHeader::read_from(&header_bytes)?;

        // Java: private ImageHeader readHeader(...) throws IOException when the magic is wrong.
        if header.magic != MAGIC {
            return Err(Error::NotAnImage(name));
        }
        if header.major_version != MAJOR_VERSION || header.minor_version != MINOR_VERSION {
            return Err(Error::WrongVersion {
                name,
                major: header.major_version,
                minor: header.minor_version,
            });
        }

        let index_size = header.get_index_size();
        // Java: if (memoryMap.capacity() < indexSize) throw new IOException("corrupted").
        if file_len < index_size {
            return Err(Error::Corrupted(format!(
                "image file is shorter than its index: {} < {}",
                file_len, index_size
            )));
        }

        let index_size_usize = usize::try_from(index_size)
            .map_err(|_| Error::Corrupted("image index does not fit in memory".to_string()))?;
        let table_length = usize::try_from(header.table_length)
            .map_err(|_| Error::Corrupted("negative image table length".to_string()))?;
        let locations_size = usize::try_from(header.get_locations_size())
            .map_err(|_| Error::Corrupted("negative image locations size".to_string()))?;
        let strings_size = usize::try_from(header.get_strings_size())
            .map_err(|_| Error::Corrupted("negative image strings size".to_string()))?;

        let mut index_bytes = vec![0u8; index_size_usize];
        read_at(&file, &mut index_bytes, 0).map_err(Error::Io)?;

        let redirect_offset = usize::try_from(header.get_redirect_offset())
            .map_err(|_| Error::Corrupted("bad redirect offset".to_string()))?;
        let offsets_offset = usize::try_from(header.get_offsets_offset())
            .map_err(|_| Error::Corrupted("bad offsets offset".to_string()))?;
        let locations_offset = usize::try_from(header.get_locations_offset())
            .map_err(|_| Error::Corrupted("bad locations offset".to_string()))?;
        let strings_offset = usize::try_from(header.get_strings_offset())
            .map_err(|_| Error::Corrupted("bad strings offset".to_string()))?;

        let redirect = read_i32_table(&index_bytes[redirect_offset..redirect_offset + table_length * 4]);
        let offsets = read_i32_table(&index_bytes[offsets_offset..offsets_offset + table_length * 4]);
        let locations = index_bytes[locations_offset..locations_offset + locations_size].to_vec();
        let strings = index_bytes[strings_offset..strings_offset + strings_size].to_vec();

        Ok(ImageIndex {
            file,
            header,
            index_size,
            redirect,
            offsets,
            locations,
            strings,
        })
    }

    /// Java: public ImageHeader getHeader()
    pub fn header(&self) -> &ImageHeader {
        &self.header
    }

    /// Java: indexSize — content offsets are relative to the end of the index.
    pub fn index_size(&self) -> u64 {
        self.index_size
    }

    /// Java: public int getLocationIndex(String name)
    pub fn get_location_index(&self, units: &[u16]) -> Option<usize> {
        let count = self.header.table_length;
        if count <= 0 {
            return None;
        }
        let hash = image_strings::hash_code(units);
        let slot = (hash % count) as usize;
        let index = *self.redirect.get(slot)?;
        if index < 0 {
            // index is twos complement of location attributes index.
            let location_index = (-index - 1) as usize;
            if location_index < self.offsets.len() {
                Some(location_index)
            } else {
                None
            }
        } else if index > 0 {
            // index is hash seed needed to compute location attributes index.
            let location_index = (image_strings::hash_code_seeded(units, index) % count) as usize;
            if location_index < self.offsets.len() {
                Some(location_index)
            } else {
                None
            }
        } else {
            // No entry.
            None
        }
    }

    /// Java: public ImageLocation findLocation(String name) — name is the full name with the
    /// leading slash, e.g. "/java.base/java/lang/Object.class".
    pub fn find_location(&self, name: &str) -> Option<ImageLocation> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let index = self.get_location_index(&units)?;
        let attribute_offset = self.offsets[index];
        if attribute_offset < 0 {
            return None;
        }
        let attributes = self.get_attributes(attribute_offset as usize).ok()?;
        // Java: if (!ImageLocation.verify(name, attributes, stringsReader)) return null;
        if !image_location::verify(&units, &attributes, &self.strings) {
            return None;
        }
        ImageLocation::from_attributes(&self.strings, attributes).ok()
    }

    /// Java: public long[] getAttributes(int offset)
    pub fn get_attributes(&self, offset: usize) -> Result<Attributes, Error> {
        if offset >= self.locations.len() {
            return Err(Error::OffsetOutOfBounds("locations", offset));
        }
        image_location::decompress(&self.locations, offset)
    }

    /// Java: public String getString(int offset)
    pub fn get_string(&self, offset: usize) -> Result<String, Error> {
        if offset >= self.strings.len() {
            return Err(Error::OffsetOutOfBounds("strings", offset));
        }
        image_strings::string_from_bytes(&self.strings, offset)
    }

    /// Java: ImageLocation getLocation(int offset) { return ImageLocation.readFrom(this, offset); }
    pub fn get_location(&self, offset: usize) -> Result<ImageLocation, Error> {
        let attributes = self.get_attributes(offset)?;
        ImageLocation::from_attributes(&self.strings, attributes)
    }

    /// Java: public String[] getEntryNames() — sorted full names, including the synthetic
    /// /modules and /packages tree nodes.
    pub fn get_entry_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for &offset in &self.offsets {
            if offset == 0 {
                continue;
            }
            if let Ok(location) = self.get_location(offset as usize) {
                names.push(location.full_name());
            }
        }
        names.sort();
        names
    }

    /// Java: ImageReader.getModuleNames() — the names of the children of the synthetic
    /// /modules node, in image order.
    pub fn modules(&self) -> Result<Vec<String>, Error> {
        let modules_location = self
            .find_location("/modules")
            .ok_or_else(|| Error::Corrupted("image has no /modules node".to_string()))?;
        let content = self
            .get_resource(&modules_location)
            .ok_or_else(|| Error::Corrupted("cannot read the /modules node".to_string()))?;
        let mut modules = Vec::new();
        for chunk in content.as_chunks::<4>().0 {
            let attribute_offset = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if attribute_offset <= 0 {
                continue;
            }
            // Java: ImageLocation pkgLoc = getLocation(offset);
            let location = self.get_location(attribute_offset as usize)?;
            // Java: Node.getNameString() of a /modules child is "/modules/<module>".
            let full_name = location.full_name();
            let module = match full_name.strip_prefix("/modules/") {
                Some(module) => module,
                None => full_name.as_str(),
            };
            modules.push(module.to_string());
        }
        Ok(modules)
    }

    /// The flat list of real module resources, named "<module>/<path>" like jimage list shows
    /// them. Tree nodes whose module attribute is the synthetic "modules"/"packages" marker (or
    /// empty for the /modules and /packages roots) are skipped by matching against the module
    /// list read from the /modules node.
    pub fn resource_names(&self, modules: &[String]) -> Result<Vec<String>, Error> {
        let module_set: std::collections::HashSet<&str> =
            modules.iter().map(String::as_str).collect();
        let mut names = Vec::new();
        for &attribute_offset in &self.offsets {
            if attribute_offset <= 0 {
                continue;
            }
            let attributes = match self.get_attributes(attribute_offset as usize) {
                Ok(attributes) => attributes,
                Err(_) => continue,
            };
            let module = match self.get_string(attributes[ATTRIBUTE_MODULE] as usize) {
                Ok(module) => module,
                Err(_) => continue,
            };
            if !module_set.contains(module.as_str()) {
                continue;
            }
            let location = ImageLocation::from_attributes(&self.strings, attributes)?;
            let full_name = location.full_name();
            let resource = match full_name.strip_prefix('/') {
                Some(resource) => resource,
                None => full_name.as_str(),
            };
            names.push(resource.to_string());
        }
        names.sort();
        Ok(names)
    }

    /// Java: ImageReader.handlePackages("/packages") — the package names of the synthetic
    /// /packages tree, in image order (the resource of /packages is a table of offsets to the
    /// /packages/<name> locations; the name is the base.ext part, ImageReader.getBaseExt).
    pub fn packages(&self) -> Result<Vec<String>, Error> {
        let packages_location = self
            .find_location("/packages")
            .ok_or_else(|| Error::Corrupted("image has no /packages node".to_string()))?;
        let content = self
            .get_resource(&packages_location)
            .ok_or_else(|| Error::Corrupted("cannot read the /packages node".to_string()))?;
        let mut packages = Vec::new();
        for chunk in content.as_chunks::<4>().0 {
            let attribute_offset = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if attribute_offset <= 0 {
                continue;
            }
            // Java: ImageLocation pkgLoc = getLocation(offset);
            let location = self.get_location(attribute_offset as usize)?;
            let full_name = location.full_name();
            // Java: getBaseExt(loc) joins the location's base and extension attributes; for
            // tree nodes the resulting name equals the last component of the full name.
            let package = match full_name.strip_prefix("/packages/") {
                Some(package) => package,
                None => full_name.as_str(),
            };
            packages.push(package.to_string());
        }
        Ok(packages)
    }

    /// Java: ImageReader.visitPackageLocation(loc) — the modules that contain the package,
    /// in resource order. The resource content is an int table of (empty-state, string-offset)
    /// pairs; the string-offset points at the module name in the strings table.
    ///
    /// A package that is not present in the /packages tree yields an empty list (Java's
    /// buildNode returns null for it).
    pub fn modules_of_package(&self, package: &str) -> Result<Vec<String>, Error> {
        let name = format!("/packages/{}", package);
        let Some(location) = self.find_location(&name) else {
            return Ok(Vec::new());
        };
        let Some(content) = self.get_resource(&location) else {
            return Ok(Vec::new());
        };
        let mut modules = Vec::new();
        for chunk in content.as_chunks::<8>().0 {
            // intBuffer.get(i) — skip the empty state; intBuffer.get(i + 1) is the string offset.
            let string_offset = i32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            if string_offset <= 0 {
                continue;
            }
            modules.push(self.get_string(string_offset as usize)?);
        }
        Ok(modules)
    }

    /// The packages of a module: the reverse of the /packages tree (the same packages whose
    /// module list contains this module, in /packages order).
    pub fn packages_of_module(&self, module: &str) -> Result<Vec<String>, Error> {
        let mut packages = Vec::new();
        for package in self.packages()? {
            if self
                .modules_of_package(&package)?
                .iter()
                .any(|candidate| candidate == module)
            {
                packages.push(package);
            }
        }
        Ok(packages)
    }

    /// Java: public byte[] getResource(ImageLocation loc) / getResourceBuffer(loc)
    pub fn get_resource(&self, location: &ImageLocation) -> Option<Vec<u8>> {
        // Java: long offset = loc.getContentOffset() + indexSize;
        let offset = location.content_offset() + self.index_size;
        let compressed_size = location.compressed_size();
        let uncompressed_size = location.uncompressed_size();

        if compressed_size > i32::MAX as u64 || uncompressed_size > i32::MAX as u64 {
            return None;
        }
        if compressed_size == 0 {
            let mut buffer = vec![0u8; uncompressed_size as usize];
            read_at(&self.file, &mut buffer, offset).ok()?;
            Some(buffer)
        } else {
            // Java decompresses here (Decompressor / ResourceDecompressorRepository). The
            // baseline JDK 17 lib/modules contains no compressed resource (checked through
            // BasicImageReader), so a compressed resource is reported as unavailable instead of
            // silently returning wrong bytes.
            None
        }
    }
}

/// Java: IntBuffer.get(int index) over a little-endian int table.
fn read_i32_table(bytes: &[u8]) -> Vec<i32> {
    bytes
        .as_chunks::<4>().0.iter()
        .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Reads exactly buf.len() bytes at offset without disturbing a shared file cursor.
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        return file.read_exact_at(buf, offset);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut position = offset;
        let mut filled = 0usize;
        while filled < buf.len() {
            let read = file.seek_read(&mut buf[filled..], position)?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "short jimage read",
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
        clone.read_exact(buf)
    }
}
