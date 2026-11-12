//! The `RPI3` platform image: a flat, sorted model of the JDK's system classes.
//!
//! The compiler never reads method bodies, so the image stores each class file with
//! its `Code` attribute removed and its unreferenced constant-pool entries pruned.
//!
//! Layout (little endian):
//!
//! ```text
//! header (128 bytes)
//! zlib(index_compressed_size bytes)
//!   module table          module_count      * 24 bytes
//!   package table         package_count     * 16 bytes
//!   resource table        resource_count    * 32 bytes  (sorted by module, path)
//!   package -> module     package_module_count * 4 bytes
//!   module -> package     module_package_count * 4 bytes
//!   string pool           string_size bytes (UTF-8, no terminators)
//!   block table           block_count * 16 bytes
//! resource blocks          each one zlib stream of up to BLOCK_SIZE bytes
//! ```
//!
//! Table offsets in the header are relative to the decompressed index. A resource
//! is (block, offset, length); reading a resource decompresses its whole block and
//! caches it (LRU), so resources that share a block pay the decompression once.
//! Blocking the data lets zlib see repetition across classes instead of inside one
//! class file.

use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::rc::Rc;

use super::Error;

/// Magic of the ravac platform image.
pub const MAGIC: [u8; 4] = *b"RPI3";
/// Current format version.
pub const VERSION: u32 = 3;

const HEADER_SIZE: usize = 128;
const MODULE_RECORD: usize = 24;
const PACKAGE_RECORD: usize = 16;
const RESOURCE_RECORD: usize = 32;
const BLOCK_RECORD: usize = 16;
/// Uncompressed bytes per block.
const BLOCK_SIZE: usize = 256 * 1024;
/// Decompressed blocks kept in the reader cache. The whole image is ~120 blocks
/// (31 MB decompressed), so this keeps every block resident after first use and
/// turns repeated class loads into plain memory slices.
const BLOCK_CACHE_SIZE: usize = 256;

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
    block: u32,
    offset: u32,
    len: u32,
}

#[derive(Clone, Copy, Debug)]
struct BlockRecord {
    /// Offset of the compressed block relative to the start of the data area.
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

/// A read-only view of an `RPI3` platform image.
#[derive(Debug)]
pub struct NativeImage {
    file: File,
    data_offset: u64,
    modules: Vec<ModuleRecord>,
    module_names: Vec<String>,
    packages: Vec<PackageRecord>,
    package_names: Vec<String>,
    resources: Vec<ResourceRecord>,
    blocks: Vec<BlockRecord>,
    package_modules: Vec<u32>,
    module_packages: Vec<u32>,
    strings: Vec<u8>,
    block_cache: RefCell<BlockCache>,
    resource_names: OnceCell<Vec<String>>,
}

impl NativeImage {
    /// Opens and validates an `RPI3` image.
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
        let block_count = u32_at(&header, 32) as usize;
        let string_size = u32_at(&header, 36) as usize;
        let index_compressed_size = u32_at(&header, 40) as usize;
        let module_table_off = u64_at(&header, 48);
        let package_table_off = u64_at(&header, 56);
        let resource_table_off = u64_at(&header, 64);
        let package_module_off = u64_at(&header, 72);
        let module_package_off = u64_at(&header, 80);
        let string_pool_off = u64_at(&header, 88);
        let block_table_off = u64_at(&header, 96);
        let data_offset = u64_at(&header, 104);

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
        let blocks_raw = index_table(
            &index_bytes,
            block_table_off,
            block_count
                .checked_mul(BLOCK_RECORD)
                .ok_or_else(|| Error::Corrupted("block table is too large".to_string()))?,
        )?;

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
                block: u32_at(&resources_raw, base + 12),
                offset: u32_at(&resources_raw, base + 16),
                len: u32_at(&resources_raw, base + 20),
            });
        }
        let mut blocks = Vec::with_capacity(block_count);
        for index in 0..block_count {
            let base = index * BLOCK_RECORD;
            blocks.push(BlockRecord {
                offset: u64_at(&blocks_raw, base),
                compressed_len: u32_at(&blocks_raw, base + 8),
                uncompressed_len: u32_at(&blocks_raw, base + 12),
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

        for record in &blocks {
            let end = data_offset
                .checked_add(record.offset)
                .and_then(|start| start.checked_add(record.compressed_len as u64))
                .ok_or_else(|| Error::Corrupted("block offset overflow".to_string()))?;
            if end > file_len {
                return Err(Error::Corrupted(format!(
                    "block data ends at {end}, past the file ({file_len})"
                )));
            }
        }
        for record in &resources {
            decode_string(&strings, record.path_off, record.path_len)?;
            if record.module as usize >= module_names.len() {
                return Err(Error::Corrupted(format!(
                    "resource refers to module {} of {}",
                    record.module,
                    module_names.len()
                )));
            }
            let block = blocks.get(record.block as usize).ok_or_else(|| {
                Error::Corrupted(format!(
                    "resource refers to block {} of {}",
                    record.block,
                    blocks.len()
                ))
            })?;
            let end = record
                .offset
                .checked_add(record.len)
                .ok_or_else(|| Error::Corrupted("resource range overflow".to_string()))?;
            if end > block.uncompressed_len {
                return Err(Error::Corrupted(format!(
                    "resource {end} exceeds block size {}",
                    block.uncompressed_len
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
            blocks,
            package_modules,
            module_packages,
            strings,
            block_cache: RefCell::new(BlockCache::new()),
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
        let block = self.read_block(record.block)?;
        let start = record.offset as usize;
        let end = start.checked_add(record.len as usize)?;
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

/// Builds an `RPI3` image from module resources and writes it to a file.
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
        // Resources sorted by path inside each module; module order is insertion
        // order, so the global (module, path) order is sorted too.
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

        // Group resources into blocks, then compress each block.
        let mut block_bytes: Vec<Vec<u8>> = Vec::new();
        let mut blocks: Vec<BlockRecord> = Vec::new();
        let mut current: Vec<u8> = Vec::with_capacity(BLOCK_SIZE);
        let mut resources: Vec<ResourceRecord> = Vec::new();

        let flush = |current: &mut Vec<u8>,
                         blocks: &mut Vec<BlockRecord>,
                         block_bytes: &mut Vec<Vec<u8>>| {
            let uncompressed = current.len();
            let bound = zlib_rs::compress_bound(uncompressed);
            let mut output = vec![0u8; bound];
            let (compressed, _code) = zlib_rs::compress_slice(
                &mut output,
                current,
                zlib_rs::DeflateConfig::new(9),
            );
            let compressed = compressed.to_vec();
            let data_offset: u64 = block_bytes.iter().map(|block| block.len() as u64).sum();
            blocks.push(BlockRecord {
                offset: data_offset,
                compressed_len: compressed.len() as u32,
                uncompressed_len: uncompressed as u32,
            });
            block_bytes.push(compressed);
            current.clear();
        };

        for (module_index, module) in self.modules.iter().enumerate() {
            for (position, &resource_index) in order[module_index].iter().enumerate() {
                let bytes = &module.resources[resource_index].1;
                if bytes.len() >= BLOCK_SIZE {
                    if !current.is_empty() {
                        flush(&mut current, &mut blocks, &mut block_bytes);
                    }
                    let block = blocks.len() as u32;
                    current.extend_from_slice(bytes);
                    flush(&mut current, &mut blocks, &mut block_bytes);
                    let path_span = resource_path_spans[module_index][position];
                    resources.push(ResourceRecord {
                        module: module_index as u32,
                        path_off: path_span.0,
                        path_len: path_span.1,
                        block,
                        offset: 0,
                        len: bytes.len() as u32,
                    });
                    continue;
                }
                if current.len() + bytes.len() > BLOCK_SIZE && !current.is_empty() {
                    flush(&mut current, &mut blocks, &mut block_bytes);
                }
                let block = blocks.len() as u32;
                let offset = current.len() as u32;
                current.extend_from_slice(bytes);
                let path_span = resource_path_spans[module_index][position];
                resources.push(ResourceRecord {
                    module: module_index as u32,
                    path_off: path_span.0,
                    path_len: path_span.1,
                    block,
                    offset,
                    len: bytes.len() as u32,
                });
            }
        }
        if !current.is_empty() {
            flush(&mut current, &mut blocks, &mut block_bytes);
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

        // The complete index (tables, string pool, block table) is one zlib stream.
        let module_table_off = 0u64;
        let package_table_off = module_table_off + (modules.len() * MODULE_RECORD) as u64;
        let resource_table_off = package_table_off + (packages.len() * PACKAGE_RECORD) as u64;
        let package_module_off = resource_table_off + (resources.len() * RESOURCE_RECORD) as u64;
        let module_package_off = package_module_off + (package_module_flat.len() * 4) as u64;
        let string_pool_off = module_package_off + (module_packages.len() * 4) as u64;
        let block_table_off = string_pool_off + strings.len() as u64;
        let index_size = block_table_off + (blocks.len() * BLOCK_RECORD) as u64;

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
        for block in &blocks {
            index.extend_from_slice(&block.offset.to_le_bytes());
            index.extend_from_slice(&block.compressed_len.to_le_bytes());
            index.extend_from_slice(&block.uncompressed_len.to_le_bytes());
        }
        debug_assert_eq!(index.len() as u64, index_size);
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&index, 9);
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
        push_u32(&mut header, blocks.len() as u32);
        push_u32(&mut header, strings.len() as u32);
        push_u32(&mut header, compressed.len() as u32);
        push_u32(&mut header, 0);
        push_u64(&mut header, module_table_off);
        push_u64(&mut header, package_table_off);
        push_u64(&mut header, resource_table_off);
        push_u64(&mut header, package_module_off);
        push_u64(&mut header, module_package_off);
        push_u64(&mut header, string_pool_off);
        push_u64(&mut header, block_table_off);
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
        out.write_all(&record.block.to_le_bytes())?;
        out.write_all(&record.offset.to_le_bytes())?;
        out.write_all(&record.len.to_le_bytes())?;
        out.write_all(&0u32.to_le_bytes())?;
        out.write_all(&0u32.to_le_bytes())?;
    }
    Ok(())
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
        dir.join(format!("ravac-platform-{}-{name}", std::process::id()))
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
