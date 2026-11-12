//! The `RPI2` platform image: a flat, sorted model of the JDK's system classes.
//!
//! The compiler never reads method bodies, so the image stores each class file with
//! its `Code` attribute removed and its unreferenced constant-pool entries pruned.
//!
//! Layout (little endian):
//!
//! ```text
//! header (96 bytes)
//! zlib(index_compressed_size bytes)
//!   module table          module_count      * 24 bytes
//!   package table         package_count     * 16 bytes
//!   resource table        resource_count    * 32 bytes  (sorted by module, path)
//!   package -> module     package_module_count * 4 bytes
//!   module -> package     module_package_count * 4 bytes
//!   string pool           string_size bytes (UTF-8, no terminators)
//! resource data
//! ```
//!
//! Table offsets in the header are relative to the decompressed index. Strings are
//! addressed by (offset, length) from the string pool; resource data by an offset
//! relative to the start of the data area. A resource may be zlib compressed;
//! `flags & 1` says so and `uncompressed_len` is the decoded size.

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use super::Error;

/// Magic of the ravac platform image.
pub const MAGIC: [u8; 4] = *b"RPI2";
/// Current format version.
pub const VERSION: u32 = 2;

const HEADER_SIZE: usize = 96;
const MODULE_RECORD: usize = 24;
const PACKAGE_RECORD: usize = 16;
const RESOURCE_RECORD: usize = 32;
const RESOURCE_COMPRESSED: u32 = 1;

#[derive(Clone, Copy, Debug)]
struct ModuleRecord {
    name_off: u32,
    name_len: u32,
    package_start: u32,
    package_count: u32,
    resource_start: u32,
    resource_count: u32,
}

#[derive(Clone, Copy, Debug)]
struct PackageRecord {
    name_off: u32,
    name_len: u32,
    module_start: u32,
    module_count: u32,
}

#[derive(Clone, Copy, Debug)]
struct ResourceRecord {
    module: u32,
    path_off: u32,
    path_len: u32,
    flags: u32,
    data_off: u64,
    data_len: u32,
    uncompressed_len: u32,
}

/// A read-only view of an `RPI2` platform image.
#[derive(Debug)]
pub struct NativeImage {
    file: File,
    data_offset: u64,
    modules: Vec<ModuleRecord>,
    module_names: Vec<String>,
    packages: Vec<PackageRecord>,
    package_names: Vec<String>,
    resources: Vec<ResourceRecord>,
    package_modules: Vec<u32>,
    module_packages: Vec<u32>,
    strings: Vec<u8>,
    resource_names: OnceCell<Vec<String>>,
}

impl NativeImage {
    /// Opens and validates an `RPI2` image.
    pub fn open(path: &Path) -> Result<NativeImage, Error> {
        let name = path.display().to_string();
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        if file_len < HEADER_SIZE as u64 {
            return Err(Error::NotAnImage(name));
        }
        let mut header = [0u8; HEADER_SIZE];
        read_at(&file, &mut header, 0)?;
        if header[..4] != MAGIC {
            return Err(Error::NotAnImage(name));
        }
        let version = u32_at(&header, 4);
        if version != VERSION {
            return Err(Error::WrongVersion { name, version });
        }
        let module_count = u32_at(&header, 12) as usize;
        let package_count = u32_at(&header, 16) as usize;
        let resource_count = u32_at(&header, 20) as usize;
        let package_module_count = u32_at(&header, 24) as usize;
        let module_package_count = u32_at(&header, 28) as usize;
        let string_size = u32_at(&header, 32) as usize;
        let index_compressed_size = u32_at(&header, 36) as usize;
        let module_table_off = u64_at(&header, 40);
        let package_table_off = u64_at(&header, 48);
        let resource_table_off = u64_at(&header, 56);
        let package_module_off = u64_at(&header, 64);
        let module_package_off = u64_at(&header, 72);
        let string_pool_off = u64_at(&header, 80);
        let data_offset = u64_at(&header, 88);

        let compressed = read_table(
            &file,
            HEADER_SIZE as u64,
            index_compressed_size,
            file_len,
        )?;
        let index_bytes = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed)
            .map_err(|_| Error::Corrupted("index is not a zlib stream".to_string()))?;

        let modules_raw = index_table(
            &index_bytes,
            module_table_off,
            module_count
                .checked_mul(MODULE_RECORD)
                .ok_or_else(|| Error::Corrupted("module table is too large".to_string()))?,
        )?;
        let packages_raw = index_table(
            &index_bytes,
            package_table_off,
            package_count
                .checked_mul(PACKAGE_RECORD)
                .ok_or_else(|| Error::Corrupted("package table is too large".to_string()))?,
        )?;
        let resources_raw = index_table(
            &index_bytes,
            resource_table_off,
            resource_count
                .checked_mul(RESOURCE_RECORD)
                .ok_or_else(|| Error::Corrupted("resource table is too large".to_string()))?,
        )?;
        let package_modules_raw = index_table(
            &index_bytes,
            package_module_off,
            package_module_count
                .checked_mul(4)
                .ok_or_else(|| Error::Corrupted("package->module table is too large".to_string()))?,
        )?;
        let module_packages_raw = index_table(
            &index_bytes,
            module_package_off,
            module_package_count
                .checked_mul(4)
                .ok_or_else(|| Error::Corrupted("module->package table is too large".to_string()))?,
        )?;
        let strings = index_table(&index_bytes, string_pool_off, string_size)?.to_vec();

        let mut modules = Vec::with_capacity(module_count);
        for index in 0..module_count {
            let base = index * MODULE_RECORD;
            modules.push(ModuleRecord {
                name_off: u32_at(&modules_raw, base),
                name_len: u32_at(&modules_raw, base + 4),
                package_start: u32_at(&modules_raw, base + 8),
                package_count: u32_at(&modules_raw, base + 12),
                resource_start: u32_at(&modules_raw, base + 16),
                resource_count: u32_at(&modules_raw, base + 20),
            });
        }
        let mut packages = Vec::with_capacity(package_count);
        for index in 0..package_count {
            let base = index * PACKAGE_RECORD;
            packages.push(PackageRecord {
                name_off: u32_at(&packages_raw, base),
                name_len: u32_at(&packages_raw, base + 4),
                module_start: u32_at(&packages_raw, base + 8),
                module_count: u32_at(&packages_raw, base + 12),
            });
        }
        let mut resources = Vec::with_capacity(resource_count);
        for index in 0..resource_count {
            let base = index * RESOURCE_RECORD;
            resources.push(ResourceRecord {
                module: u32_at(&resources_raw, base),
                path_off: u32_at(&resources_raw, base + 4),
                path_len: u32_at(&resources_raw, base + 8),
                flags: u32_at(&resources_raw, base + 12),
                data_off: u64_at(&resources_raw, base + 16),
                data_len: u32_at(&resources_raw, base + 24),
                uncompressed_len: u32_at(&resources_raw, base + 28),
            });
        }
        let package_modules: Vec<u32> = package_modules_raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();
        let module_packages: Vec<u32> = module_packages_raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();

        let module_names = modules
            .iter()
            .map(|record| decode_string(&strings, record.name_off, record.name_len))
            .collect::<Result<Vec<String>, Error>>()?;
        let package_names = packages
            .iter()
            .map(|record| decode_string(&strings, record.name_off, record.name_len))
            .collect::<Result<Vec<String>, Error>>()?;

        for record in &resources {
            decode_string(&strings, record.path_off, record.path_len)?;
            if record.module as usize >= module_names.len() {
                return Err(Error::Corrupted(format!(
                    "resource refers to module {} of {}",
                    record.module,
                    module_names.len()
                )));
            }
            let end = data_offset
                .checked_add(record.data_off)
                .and_then(|start| start.checked_add(record.data_len as u64))
                .ok_or_else(|| Error::Corrupted("resource offset overflow".to_string()))?;
            if end > file_len {
                return Err(Error::Corrupted(format!(
                    "resource data ends at {end}, past the file ({file_len})"
                )));
            }
        }

        Ok(NativeImage {
            file,
            data_offset,
            modules,
            module_names,
            packages,
            package_names,
            resources,
            package_modules,
            module_packages,
            strings,
            resource_names: OnceCell::new(),
        })
    }

    /// The module names in image order.
    pub fn modules(&self) -> &[String] {
        &self.module_names
    }

    /// All module resources, named `<module>/<path>`, sorted.
    pub fn resources(&self) -> &[String] {
        self.resource_names.get_or_init(|| {
            self.resources
                .iter()
                .map(|record| {
                    let path = self.resource_path(record);
                    format!("{}/{}", self.module_names[record.module as usize], path)
                })
                .collect()
        })
    }

    /// Returns the bytes of the named resource, or None when it does not exist.
    pub fn find_resource(&self, name: &str) -> Option<Vec<u8>> {
        let name = name.strip_prefix('/').unwrap_or(name);
        let (module, path) = name.split_once('/')?;
        let module = self.module_index(module)?;
        let record = &self.modules[module as usize];
        let start = record.resource_start as usize;
        let end = start + record.resource_count as usize;
        let records = self.resources.get(start..end)?;
        let key = path.as_bytes();
        let mut low = 0usize;
        let mut high = records.len();
        while low < high {
            let mid = (low + high) / 2;
            match self.resource_path(&records[mid]).as_bytes().cmp(key) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return self.read_resource(&records[mid]),
            }
        }
        None
    }

    /// The modules that contain the package (empty for an unknown package).
    pub fn modules_of_package(&self, package: &str) -> Result<Vec<String>, Error> {
        let Ok(index) = self
            .package_names
            .binary_search_by(|name| name.as_str().cmp(package))
        else {
            return Ok(Vec::new());
        };
        let record = &self.packages[index];
        let start = record.module_start as usize;
        let end = start + record.module_count as usize;
        let modules = self
            .package_modules
            .get(start..end)
            .ok_or_else(|| Error::Corrupted("package->module range out of bounds".to_string()))?;
        Ok(modules
            .iter()
            .filter_map(|module| self.module_names.get(*module as usize).cloned())
            .collect())
    }

    /// The packages of a module (empty for an unknown module).
    pub fn packages_of_module(&self, module: &str) -> Result<Vec<String>, Error> {
        let Some(index) = self.module_index(module) else {
            return Ok(Vec::new());
        };
        let record = &self.modules[index as usize];
        let start = record.package_start as usize;
        let end = start + record.package_count as usize;
        let packages = self
            .module_packages
            .get(start..end)
            .ok_or_else(|| Error::Corrupted("module->package range out of bounds".to_string()))?;
        Ok(packages
            .iter()
            .filter_map(|package| self.package_names.get(*package as usize).cloned())
            .collect())
    }

    fn module_index(&self, name: &str) -> Option<u32> {
        self.module_names
            .iter()
            .position(|candidate| candidate == name)
            .map(|index| index as u32)
    }

    fn resource_path(&self, record: &ResourceRecord) -> &str {
        let start = record.path_off as usize;
        let end = start + record.path_len as usize;
        std::str::from_utf8(&self.strings[start..end]).expect("validated resource path")
    }

    fn read_resource(&self, record: &ResourceRecord) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; record.data_len as usize];
        read_at(&self.file, &mut buffer, self.data_offset + record.data_off).ok()?;
        if record.flags & RESOURCE_COMPRESSED != 0 {
            let decoded = miniz_oxide::inflate::decompress_to_vec_zlib(&buffer).ok()?;
            if decoded.len() != record.uncompressed_len as usize {
                return None;
            }
            Some(decoded)
        } else {
            Some(buffer)
        }
    }
}

/// Builds an `RPI2` image from module resources and writes it to a file.
#[derive(Default)]
pub struct ImageBuilder {
    modules: Vec<BuildModule>,
    by_name: HashMap<String, usize>,
}

struct BuildModule {
    name: String,
    resources: Vec<(String, Vec<u8>)>,
}

impl ImageBuilder {
    /// Creates an empty builder.
    pub fn new() -> ImageBuilder {
        ImageBuilder::default()
    }

    /// Registers a module. Modules are kept in insertion order.
    pub fn add_module(&mut self, name: &str) {
        if self.by_name.contains_key(name) {
            return;
        }
        let index = self.modules.len();
        self.modules.push(BuildModule {
            name: name.to_string(),
            resources: Vec::new(),
        });
        self.by_name.insert(name.to_string(), index);
    }

    /// Adds a resource (`<module>/<path>` naming, path without a leading slash).
    pub fn add_resource(&mut self, module: &str, path: &str, bytes: Vec<u8>) {
        self.add_module(module);
        let index = self.by_name[module];
        self.modules[index]
            .resources
            .push((path.to_string(), bytes));
    }

    /// Writes the image.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        // Resources sorted by path inside each module; module order is insertion order,
        // so the global (module, path) order is sorted too and lookup can binary search.
        let mut order: Vec<Vec<usize>> = Vec::with_capacity(self.modules.len());
        for module in &self.modules {
            let mut indices: Vec<usize> = (0..module.resources.len()).collect();
            indices.sort_by(|&a, &b| module.resources[a].0.cmp(&module.resources[b].0));
            order.push(indices);
        }

        // Packages: one row per package name, with the modules that contain it.
        let mut package_modules: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for (module_index, module) in self.modules.iter().enumerate() {
            for &resource_index in &order[module_index] {
                if let Some(package) = package_of(&module.resources[resource_index].0) {
                    package_modules
                        .entry(package)
                        .or_default()
                        .insert(module_index as u32);
                }
            }
        }
        let package_names: Vec<String> = package_modules.keys().cloned().collect();
        let mut package_module_flat: Vec<u32> = Vec::new();
        let mut package_module_spans: Vec<(u32, u32)> = Vec::with_capacity(package_names.len());
        for package in &package_names {
            let start = package_module_flat.len() as u32;
            package_module_flat.extend(package_modules[package].iter().copied());
            package_module_spans.push((start, package_module_flat.len() as u32 - start));
        }

        let mut module_packages: Vec<u32> = Vec::new();
        let mut module_package_spans: Vec<(u32, u32)> = Vec::with_capacity(self.modules.len());
        for (module_index, _module) in self.modules.iter().enumerate() {
            let start = module_packages.len() as u32;
            for (package_index, package) in package_names.iter().enumerate() {
                if package_modules[package].contains(&(module_index as u32)) {
                    module_packages.push(package_index as u32);
                }
            }
            module_package_spans.push((start, module_packages.len() as u32 - start));
        }

        let mut strings: Vec<u8> = Vec::new();
        let mut interned: HashMap<String, (u32, u32)> = HashMap::new();
        let mut intern = |text: &str| -> (u32, u32) {
            if let Some(&span) = interned.get(text) {
                return span;
            }
            let offset = strings.len() as u32;
            strings.extend_from_slice(text.as_bytes());
            let span = (offset, text.len() as u32);
            interned.insert(text.to_string(), span);
            span
        };
        let module_name_spans: Vec<(u32, u32)> =
            self.modules.iter().map(|module| intern(&module.name)).collect();
        let package_name_spans: Vec<(u32, u32)> =
            package_names.iter().map(|package| intern(package)).collect();

        let mut resource_path_spans: Vec<Vec<(u32, u32)>> = Vec::with_capacity(self.modules.len());
        for (module_index, module) in self.modules.iter().enumerate() {
            resource_path_spans.push(
                order[module_index]
                    .iter()
                    .map(|&resource_index| intern(&module.resources[resource_index].0))
                    .collect(),
            );
        }

        // Resource data, with a per-resource zlib stream when that is smaller.
        let total_bytes: usize = self
            .modules
            .iter()
            .map(|module| module.resources.iter().map(|(_, bytes)| bytes.len()).sum::<usize>())
            .sum();
        let mut data: Vec<u8> = Vec::with_capacity(total_bytes);
        let mut resources: Vec<ResourceRecord> = Vec::new();
        for (module_index, module) in self.modules.iter().enumerate() {
            for (position, &resource_index) in order[module_index].iter().enumerate() {
                let bytes = &module.resources[resource_index].1;
                let (payload, flags) = compress(bytes);
                let path_span = resource_path_spans[module_index][position];
                resources.push(ResourceRecord {
                    module: module_index as u32,
                    path_off: path_span.0,
                    path_len: path_span.1,
                    flags,
                    data_off: data.len() as u64,
                    data_len: payload.len() as u32,
                    uncompressed_len: bytes.len() as u32,
                });
                data.extend_from_slice(&payload);
            }
        }

        let mut resource_start = 0u32;
        let modules: Vec<ModuleRecord> = self
            .modules
            .iter()
            .enumerate()
            .map(|(module_index, module)| {
                let (package_start, package_count) = module_package_spans[module_index];
                let record = ModuleRecord {
                    name_off: module_name_spans[module_index].0,
                    name_len: module_name_spans[module_index].1,
                    package_start,
                    package_count,
                    resource_start,
                    resource_count: module.resources.len() as u32,
                };
                resource_start += module.resources.len() as u32;
                record
            })
            .collect();
        let packages: Vec<PackageRecord> = package_names
            .iter()
            .enumerate()
            .map(|(package_index, _package)| PackageRecord {
                name_off: package_name_spans[package_index].0,
                name_len: package_name_spans[package_index].1,
                module_start: package_module_spans[package_index].0,
                module_count: package_module_spans[package_index].1,
            })
            .collect();

        // The complete index (tables plus string pool) is one zlib stream.
        let module_table_off = 0u64;
        let package_table_off = module_table_off + (modules.len() * MODULE_RECORD) as u64;
        let resource_table_off = package_table_off + (packages.len() * PACKAGE_RECORD) as u64;
        let package_module_off = resource_table_off + (resources.len() * RESOURCE_RECORD) as u64;
        let module_package_off = package_module_off + (package_module_flat.len() * 4) as u64;
        let string_pool_off = module_package_off + (module_packages.len() * 4) as u64;
        let index_size = string_pool_off + strings.len() as u64;

        let mut index = Vec::with_capacity(index_size as usize);
        write_modules(&mut index, &modules)?;
        write_packages(&mut index, &packages)?;
        write_resources(&mut index, &resources)?;
        for value in &package_module_flat {
            index.extend_from_slice(&value.to_le_bytes());
        }
        for value in &module_packages {
            index.extend_from_slice(&value.to_le_bytes());
        }
        index.extend_from_slice(&strings);
        debug_assert_eq!(index.len() as u64, index_size);
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&index, 6);
        let data_off = HEADER_SIZE as u64 + compressed.len() as u64;

        let mut header = Vec::with_capacity(HEADER_SIZE);
        header.extend_from_slice(&MAGIC);
        push_u32(&mut header, VERSION);
        push_u32(&mut header, 0);
        push_u32(&mut header, modules.len() as u32);
        push_u32(&mut header, packages.len() as u32);
        push_u32(&mut header, resources.len() as u32);
        push_u32(&mut header, package_module_flat.len() as u32);
        push_u32(&mut header, module_packages.len() as u32);
        push_u32(&mut header, strings.len() as u32);
        push_u32(&mut header, compressed.len() as u32);
        push_u64(&mut header, module_table_off);
        push_u64(&mut header, package_table_off);
        push_u64(&mut header, resource_table_off);
        push_u64(&mut header, package_module_off);
        push_u64(&mut header, module_package_off);
        push_u64(&mut header, string_pool_off);
        push_u64(&mut header, data_off);
        debug_assert_eq!(header.len(), HEADER_SIZE);

        let mut out = BufWriter::with_capacity(1 << 20, File::create(path)?);
        out.write_all(&header)?;
        out.write_all(&compressed)?;
        out.write_all(&data)?;
        out.flush()
    }
}

fn write_modules(out: &mut impl Write, records: &[ModuleRecord]) -> io::Result<()> {
    for record in records {
        out.write_all(&record.name_off.to_le_bytes())?;
        out.write_all(&record.name_len.to_le_bytes())?;
        out.write_all(&record.package_start.to_le_bytes())?;
        out.write_all(&record.package_count.to_le_bytes())?;
        out.write_all(&record.resource_start.to_le_bytes())?;
        out.write_all(&record.resource_count.to_le_bytes())?;
    }
    Ok(())
}

fn write_packages(out: &mut impl Write, records: &[PackageRecord]) -> io::Result<()> {
    for record in records {
        out.write_all(&record.name_off.to_le_bytes())?;
        out.write_all(&record.name_len.to_le_bytes())?;
        out.write_all(&record.module_start.to_le_bytes())?;
        out.write_all(&record.module_count.to_le_bytes())?;
    }
    Ok(())
}

fn write_resources(out: &mut impl Write, records: &[ResourceRecord]) -> io::Result<()> {
    for record in records {
        out.write_all(&record.module.to_le_bytes())?;
        out.write_all(&record.path_off.to_le_bytes())?;
        out.write_all(&record.path_len.to_le_bytes())?;
        out.write_all(&record.flags.to_le_bytes())?;
        out.write_all(&record.data_off.to_le_bytes())?;
        out.write_all(&record.data_len.to_le_bytes())?;
        out.write_all(&record.uncompressed_len.to_le_bytes())?;
    }
    Ok(())
}

fn compress(bytes: &[u8]) -> (Vec<u8>, u32) {
    if bytes.is_empty() {
        return (Vec::new(), 0);
    }
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(bytes, 6);
    if packed.len() < bytes.len() {
        (packed, RESOURCE_COMPRESSED)
    } else {
        (bytes.to_vec(), 0)
    }
}

fn package_of(path: &str) -> Option<String> {
    let (directory, _file) = path.rsplit_once('/')?;
    if directory.is_empty() {
        None
    } else {
        Some(directory.replace('/', "."))
    }
}

fn push_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

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

fn decode_string(strings: &[u8], offset: u32, length: u32) -> Result<String, Error> {
    let start = offset as usize;
    let end = start
        .checked_add(length as usize)
        .ok_or_else(|| Error::Corrupted("string offset overflow".to_string()))?;
    let bytes = strings
        .get(start..end)
        .ok_or_else(|| Error::Corrupted(format!("string {start}..{end} out of bounds")))?;
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| Error::Corrupted("string pool is not UTF-8".to_string()))
}

/// A slice of the decompressed index at `offset` with `size` bytes.
fn index_table(index: &[u8], offset: u64, size: usize) -> Result<&[u8], Error> {
    let start = usize::try_from(offset)
        .map_err(|_| Error::Corrupted("table offset does not fit in memory".to_string()))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| Error::Corrupted("table size overflow".to_string()))?;
    index
        .get(start..end)
        .ok_or_else(|| Error::Corrupted(format!("table {start}..{end} out of bounds")))
}

fn read_table(file: &File, offset: u64, size: usize, file_len: u64) -> Result<Vec<u8>, Error> {
    let end = offset
        .checked_add(size as u64)
        .ok_or_else(|| Error::Corrupted("table offset overflow".to_string()))?;
    if end > file_len {
        return Err(Error::Corrupted(format!(
            "table at {offset} with size {size} ends past the file ({file_len})"
        )));
    }
    let mut buffer = vec![0u8; size];
    read_at(file, &mut buffer, offset)?;
    Ok(buffer)
}

/// Reads exactly `buffer.len()` bytes at `offset` without disturbing a shared file cursor.
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
                    "short platform image read",
                ));
            }
            filled += read;
            position += read as u64;
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let mut clone = file.try_clone()?;
        clone.seek(SeekFrom::Start(offset))?;
        clone.read_exact(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageBuilder, NativeImage};
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("target")
            .join("test-tmp");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("ravac-platform-{}-{name}", std::process::id()));
        path
    }

    #[test]
    fn round_trips_tables_strings_and_resources() {
        let path = temp_path("round-trip.bin");
        let mut builder = ImageBuilder::new();
        builder.add_module("java.base");
        builder.add_resource(
            "java.base",
            "java/lang/Object.class",
            b"object class bytes".to_vec(),
        );
        builder.add_resource("java.base", "module-info.class", b"module".to_vec());
        builder.add_module("other.mod");
        builder.add_resource("other.mod", "pkg/A.class", b"other class".to_vec());
        builder.write(&path).expect("write image");

        let image = NativeImage::open(&path).expect("open image");
        assert_eq!(
            image.modules().iter().map(String::as_str).collect::<Vec<_>>(),
            ["java.base", "other.mod"]
        );
        assert_eq!(
            image.find_resource("java.base/java/lang/Object.class"),
            Some(b"object class bytes".to_vec())
        );
        assert_eq!(
            image.find_resource("/other.mod/pkg/A.class"),
            Some(b"other class".to_vec())
        );
        assert_eq!(image.find_resource("java.base/missing"), None);
        assert_eq!(
            image
                .packages_of_module("java.base")
                .expect("packages")
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["java.lang"]
        );
        assert_eq!(
            image
                .modules_of_package("java.lang")
                .expect("modules")
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["java.base"]
        );
        assert_eq!(
            image.resources().iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "java.base/java/lang/Object.class",
                "java.base/module-info.class",
                "other.mod/pkg/A.class"
            ]
        );
        assert!(image
            .modules_of_package("no.such.package")
            .expect("empty")
            .is_empty());
        assert!(image.find_resource("other.mod/path/that/does/not/exist").is_none());
        let _ = std::fs::remove_file(&path);
    }
}
