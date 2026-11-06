//! On-disk index format.
//!
//! A self-describing, dependency-free binary format: a header with every hash
//! that must match, a string table, then flat tables that are re-indexed into
//! the in-memory maps on load. Any mismatch simply discards the cache; there is
//! no migration across schema versions.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::key::{
    FileId, IndexSymbolKind, NameId, StringInterner, SymbolId, SymbolKey,
};
use super::model::*;
use super::build::hash_text;

pub const MAGIC: &[u8; 4] = b"RAVI";
pub const SCHEMA_VERSION: u32 = 2;

/// Directory that holds persisted indexes.
pub fn cache_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("ravac").join("index"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(|base| PathBuf::from(base).join("ravac").join("index"))
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|base| PathBuf::from(base).join(".cache").join("ravac").join("index"))
            })
    }
}

pub fn cache_path(workspace_hash: u64) -> Option<PathBuf> {
    cache_dir().map(|dir| dir.join(format!("{workspace_hash:016x}-v{SCHEMA_VERSION}.bin")))
}

/// Hash of the workspace root; part of the cache file name.
pub fn workspace_hash(model: &super::super::model::WorkspaceModel) -> u64 {
    super::super::model::hash_paths(&[model.root.clone()])
}

pub fn save(index: &ProjectIndex, path: &Path) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    write_index(index, &mut bytes);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Atomic write: a temp file in the same directory, then rename over.
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&temp, path)?;
    Ok(())
}

/// Load an index if, and only if, the schema and workspace fingerprint match
/// and every recorded file hash still matches the file on disk.
pub fn load(model: &super::super::model::WorkspaceModel) -> Option<ProjectIndex> {
    let path = cache_path(workspace_hash(model))?;
    let bytes = std::fs::read(path).ok()?;
    let index = read_index(&bytes)?;
    let fingerprint = model.fingerprint();
    if index.workspace != fingerprint {
        return None;
    }
    if index.files.len() > 0 && !file_hashes_match(&index) {
        return None;
    }
    Some(index)
}

fn file_hashes_match(index: &ProjectIndex) -> bool {
    index.files.iter().all(|record| {
        std::fs::read_to_string(&record.path)
            .map(|text| hash_text(&text) == record.hash)
            .unwrap_or(false)
    })
}

fn write_index(index: &ProjectIndex, out: &mut Vec<u8>) {
    out.extend_from_slice(MAGIC);
    write_u32(out, SCHEMA_VERSION);
    // Keep the root path as a string header entry for diagnostics.
    write_string(out, &index.workspace.root.to_string_lossy());
    write_u64(out, index.workspace.classpath_hash);
    write_u64(out, index.workspace.toolchain_hash);
    write_string(out, &index.workspace.source_level);
    write_opt_string(out, index.workspace.jdk_home.as_deref().map(|p| p.to_string_lossy()).as_deref());
    write_opt_string(
        out,
        index
            .workspace
            .platform_dir
            .as_deref()
            .map(|p| p.to_string_lossy())
            .as_deref(),
    );
    write_u64(out, index.generation);

    write_u32(out, index.files.len() as u32);
    for file in &index.files {
        write_string(out, &file.path.to_string_lossy());
        write_u64(out, file.hash);
        out.push(file.source_set.tag());
        out.push(match file.status {
            FileStatus::Indexed => 1,
            FileStatus::Failed => 2,
            FileStatus::Skipped => 3,
        });
    }

    // Rebuild a string table from the interner plus every symbol's detail and
    // flat name, in a deterministic order.
    let mut strings = StringInterner::default();
    let mut name_ids = Vec::with_capacity(index.symbols.len());
    let mut detail_ids = Vec::with_capacity(index.symbols.len());
    let mut flat_ids = Vec::with_capacity(index.symbols.len());
    let mut signature_ids = Vec::with_capacity(index.symbols.len());
    for symbol in &index.symbols {
        name_ids.push(strings.intern(index.name(symbol.key.name)));
        detail_ids.push(strings.intern(&symbol.detail));
        flat_ids.push(strings.intern(&symbol.flat_name));
        signature_ids.push(strings.intern(index.signature(symbol.key.signature)));
    }
    write_u32(out, strings.len() as u32);
    for (_, value) in strings.iter() {
        write_string(out, value);
    }

    write_u32(out, index.symbols.len() as u32);
    for (position, symbol) in index.symbols.iter().enumerate() {
        write_opt_u32(out, symbol.key.owner);
        write_u32(out, name_ids[position]);
        out.push(symbol.key.kind.tag());
        write_u32(out, signature_ids[position]);
        out.push(symbol.kind.tag());
        write_u32(out, symbol.file);
        write_u32(out, symbol.range.start);
        write_u32(out, symbol.range.end);
        write_u32(out, detail_ids[position]);
        write_u32(out, symbol.modifiers);
        write_u32(out, flat_ids[position]);
        write_u32(out, symbol.supertypes.len() as u32);
        for supertype in &symbol.supertypes {
            write_type_ref(out, supertype);
        }
        match &symbol.return_type {
            Some(ty) => {
                out.push(1);
                write_type_ref(out, ty);
            }
            None => out.push(0),
        }
        write_opt_string(out, symbol.javadoc.as_deref());
    }

    // Occurrences grouped by symbol, matching `ProjectIndex::occurrences`.
    let total: usize = index.occurrences.values().map(Vec::len).sum();
    write_u32(out, total as u32);
    for (symbol, occurrences) in &index.occurrences {
        let _ = symbol;
        for occurrence in occurrences {
            write_u32(out, occurrence.file);
            write_u32(out, occurrence.range.start);
            write_u32(out, occurrence.range.end);
            write_u32(out, occurrence.symbol);
            out.push(occurrence.role.tag());
        }
    }

    match &index.status {
        IndexStatus::Building => out.push(0),
        IndexStatus::Ready => out.push(1),
        IndexStatus::Partial {
            failed_files,
            reason,
        } => {
            out.push(2);
            write_u32(out, failed_files.len() as u32);
            for file in failed_files {
                write_string(out, &file.to_string_lossy());
            }
            write_string(out, reason);
        }
        IndexStatus::Failed(reason) => {
            out.push(3);
            write_string(out, reason);
        }
    }
}

fn write_type_ref(out: &mut Vec<u8>, ty: &TypeRef) {
    match ty {
        TypeRef::Project(id) => {
            out.push(1);
            write_u32(out, *id);
        }
        TypeRef::External(name) => {
            out.push(2);
            write_string(out, name);
        }
    }
}

fn read_index(bytes: &[u8]) -> Option<ProjectIndex> {
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != MAGIC {
        return None;
    }
    if reader.u32()? != SCHEMA_VERSION {
        return None;
    }
    let root = PathBuf::from(reader.string()?);
    let classpath_hash = reader.u64()?;
    let toolchain_hash = reader.u64()?;
    let source_level = reader.string()?;
    let jdk_home = reader.opt_string()?.map(PathBuf::from);
    let platform_dir = reader.opt_string()?.map(PathBuf::from);
    let generation = reader.u64()?;

    let file_count = reader.u32()? as usize;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let path = PathBuf::from(reader.string()?);
        let hash = reader.u64()?;
        let source_set = SourceSetKind::from_tag(reader.u8()?)?;
        let status = match reader.u8()? {
            1 => FileStatus::Indexed,
            2 => FileStatus::Failed,
            3 => FileStatus::Skipped,
            _ => return None,
        };
        files.push(FileRecord {
            path,
            hash,
            source_set,
            status,
        });
    }

    let string_count = reader.u32()? as usize;
    let mut raw_strings = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        raw_strings.push(reader.string()?);
    }
    let strings = StringInterner::from_strings(raw_strings);

    let symbol_count = reader.u32()? as usize;
    let mut symbols = Vec::with_capacity(symbol_count);
    let mut by_key = HashMap::new();
    let mut by_name: HashMap<NameId, Vec<SymbolId>> = HashMap::new();
    let mut by_file: HashMap<FileId, FileIndex> = HashMap::new();
    let mut by_supertype: HashMap<SymbolId, Vec<SymbolId>> = HashMap::new();
    let mut occurrences: HashMap<SymbolId, Vec<Occurrence>> = HashMap::new();
    for id in 0..symbol_count as SymbolId {
        let owner = reader.opt_u32()?;
        let name = reader.u32()?;
        let key_kind = IndexSymbolKind::from_tag(reader.u8()?)?;
        let signature = reader.u32()?;
        let kind = IndexSymbolKind::from_tag(reader.u8()?)?;
        let file = reader.u32()?;
        let start = reader.u32()?;
        let end = reader.u32()?;
        let detail_id = reader.u32()?;
        let detail = strings.get(detail_id).to_string();
        let modifiers = reader.u32()?;
        let flat_id = reader.u32()?;
        let flat_name = strings.get(flat_id).to_string();
        let supertype_count = reader.u32()? as usize;
        let mut supertypes = Vec::with_capacity(supertype_count);
        for _ in 0..supertype_count {
            supertypes.push(read_type_ref(&mut reader)?);
        }
        let return_type = if reader.u8()? == 1 {
            Some(read_type_ref(&mut reader)?)
        } else {
            None
        };
        let javadoc = reader.opt_string()?;
        let key = SymbolKey {
            owner,
            name,
            kind: key_kind,
            signature,
        };
        let record = SymbolRecord {
            key,
            kind,
            file,
            range: ByteRange { start, end },
            detail,
            modifiers,
            flat_name,
            supertypes,
            return_type,
            javadoc,
        };
        by_key.insert(key, id);
        by_name.entry(name).or_default().push(id);
        by_file.entry(file).or_default().declarations.push(id);
        for supertype in &record.supertypes {
            if let TypeRef::Project(target) = supertype {
                by_supertype.entry(*target).or_default().push(id);
            }
        }
        symbols.push(record);
    }

    let occurrence_count = reader.u32()? as usize;
    for _ in 0..occurrence_count {
        let file = reader.u32()?;
        let start = reader.u32()?;
        let end = reader.u32()?;
        let symbol = reader.u32()?;
        let role = OccurrenceRole::from_tag(reader.u8()?)?;
        let occurrence = Occurrence {
            file,
            range: ByteRange { start, end },
            symbol,
            role,
        };
        by_file
            .entry(file)
            .or_default()
            .occurrences
            .push(occurrence.clone());
        occurrences.entry(symbol).or_default().push(occurrence);
    }

    let status = match reader.u8()? {
        0 => IndexStatus::Building,
        1 => IndexStatus::Ready,
        2 => {
            let failed_count = reader.u32()? as usize;
            let mut failed_files = Vec::with_capacity(failed_count);
            for _ in 0..failed_count {
                failed_files.push(PathBuf::from(reader.string()?));
            }
            let reason = reader.string()?;
            IndexStatus::Partial {
                failed_files,
                reason,
            }
        }
        3 => IndexStatus::Failed(reader.string()?),
        _ => return None,
    };

    let mut index = ProjectIndex {
        generation,
        workspace: WorkspaceFingerprint {
            root,
            classpath_hash,
            toolchain_hash,
            source_level,
            jdk_home,
            platform_dir,
        },
        strings,
        symbols,
        by_key,
        by_name,
        by_file,
        by_supertype,
        occurrences,
        dependents: HashMap::new(),
        files,
        status,
    };
    index.compute_dependents();
    Some(index)
}

fn read_type_ref(reader: &mut Reader<'_>) -> Option<TypeRef> {
    match reader.u8()? {
        1 => Some(TypeRef::Project(reader.u32()?)),
        2 => Some(TypeRef::External(reader.string()?)),
        _ => None,
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_u64(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn write_opt_string(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value);
        }
        None => out.push(0),
    }
}

fn write_opt_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            out.push(1);
            write_u32(out, value);
        }
        None => out.push(0),
    }
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    write_u64(out, value as u64);
}

fn write_u64(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.position.checked_add(len)?;
        let slice = self.bytes.get(self.position..end)?;
        self.position = end;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.position)?;
        self.position += 1;
        Some(byte)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(self.u64()? as u32)
    }

    fn u64(&mut self) -> Option<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = self.u8()?;
            value |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        let len = self.u64()? as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    fn opt_string(&mut self) -> Option<Option<String>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.string()?)),
            _ => None,
        }
    }

    fn opt_u32(&mut self) -> Option<Option<u32>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.u32()?)),
            _ => None,
        }
    }
}
