//! Immutable project index snapshot and its plain-data records.

use std::collections::HashMap;
use std::path::PathBuf;

use super::key::{FileId, NameId, SignatureId, StringInterner, SymbolId, SymbolKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

impl ByteRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn contains(self, offset: u32) -> bool {
        offset >= self.start && offset < self.end.max(self.start + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    Project(SymbolId),
    External(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OccurrenceRole {
    Declaration,
    Reference,
    Import,
    TypeUse,
    Override,
}

impl OccurrenceRole {
    /// Stable on-disk tag.
    pub fn tag(self) -> u8 {
        match self {
            Self::Declaration => 1,
            Self::Reference => 2,
            Self::Import => 3,
            Self::TypeUse => 4,
            Self::Override => 5,
        }
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Declaration,
            2 => Self::Reference,
            3 => Self::Import,
            4 => Self::TypeUse,
            5 => Self::Override,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub file: FileId,
    pub range: ByteRange,
    pub symbol: SymbolId,
    pub role: OccurrenceRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRecord {
    pub key: SymbolKey,
    pub kind: super::key::IndexSymbolKind,
    pub file: FileId,
    pub range: ByteRange,
    pub detail: String,
    pub modifiers: u32,
    /// Fully qualified name, display-only. It is never used as an index key.
    pub flat_name: String,
    pub supertypes: Vec<TypeRef>,
    pub return_type: Option<TypeRef>,
    /// Rendered Javadoc of the declaration, when present.
    pub javadoc: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSetKind {
    Main,
    Test,
    Generated,
}

impl SourceSetKind {
    pub fn tag(self) -> u8 {
        match self {
            Self::Main => 1,
            Self::Test => 2,
            Self::Generated => 3,
        }
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Main,
            2 => Self::Test,
            3 => Self::Generated,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Indexed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub path: PathBuf,
    pub hash: u64,
    pub source_set: SourceSetKind,
    pub status: FileStatus,
}

impl FileRecord {
    pub fn failed(&self) -> bool {
        matches!(self.status, FileStatus::Failed)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileIndex {
    pub declarations: Vec<SymbolId>,
    pub occurrences: Vec<Occurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexStatus {
    Building,
    Ready,
    Partial {
        failed_files: Vec<PathBuf>,
        reason: String,
    },
    Failed(String),
}

impl Default for IndexStatus {
    fn default() -> Self {
        Self::Building
    }
}

impl IndexStatus {
    /// Only a fully-built index may answer queries; `Partial` is explicit.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// Everything that must stay equal for a disk index to be reused.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceFingerprint {
    pub root: PathBuf,
    pub classpath_hash: u64,
    pub toolchain_hash: u64,
    pub source_level: String,
    pub jdk_home: Option<PathBuf>,
    pub platform_dir: Option<PathBuf>,
}

/// Read-only snapshot of the whole project. `Arc<ProjectIndex>` is the unit of
/// publication; queries clone the `Arc` and never observe a partial mutation.
#[derive(Debug, Clone)]
pub struct ProjectIndex {
    pub generation: u64,
    pub workspace: WorkspaceFingerprint,
    pub strings: StringInterner,
    pub symbols: Vec<SymbolRecord>,
    pub by_key: HashMap<SymbolKey, SymbolId>,
    pub by_name: HashMap<NameId, Vec<SymbolId>>,
    pub by_file: HashMap<FileId, FileIndex>,
    pub by_supertype: HashMap<SymbolId, Vec<SymbolId>>,
    pub occurrences: HashMap<SymbolId, Vec<Occurrence>>,
    /// Declaring file -> files that reference its symbols. Used to bound
    /// invalidation when a declaration changes.
    pub dependents: HashMap<FileId, Vec<FileId>>,
    pub files: Vec<FileRecord>,
    pub status: IndexStatus,
}

impl ProjectIndex {
    /// Recompute the reverse dependency table from the occurrence facts.
    pub fn compute_dependents(&mut self) {
        let mut dependents: HashMap<FileId, Vec<FileId>> = HashMap::new();
        for (symbol, occurrences) in &self.occurrences {
            let Some(record) = self.symbols.get(*symbol as usize) else {
                continue;
            };
            let declarer = record.file;
            for occurrence in occurrences {
                if occurrence.file == declarer {
                    continue;
                }
                let entry = dependents.entry(declarer).or_default();
                if !entry.contains(&occurrence.file) {
                    entry.push(occurrence.file);
                }
            }
        }
        for files in dependents.values_mut() {
            files.sort_unstable();
        }
        self.dependents = dependents;
    }

    /// Files that must be re-indexed when `file`'s declarations change.
    pub fn dependents_of(&self, file: FileId) -> &[FileId] {
        self.dependents
            .get(&file)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Transitive closure of `file` plus everything that depends on it.
    pub fn dependent_closure(&self, file: FileId) -> Vec<FileId> {
        let mut visited = vec![file];
        let mut index = 0;
        while index < visited.len() {
            let current = visited[index];
            index += 1;
            for dependent in self.dependents_of(current) {
                if !visited.contains(dependent) {
                    visited.push(*dependent);
                }
            }
        }
        visited
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&SymbolRecord> {
        self.symbols.get(id as usize)
    }

    pub fn file(&self, id: FileId) -> Option<&FileRecord> {
        self.files.get(id as usize)
    }

    pub fn file_id_for_path(&self, path: &std::path::Path) -> Option<FileId> {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.files
            .iter()
            .position(|file| file.path == canonical)
            .map(|index| index as FileId)
    }

    pub fn file_index(&self, id: FileId) -> Option<&FileIndex> {
        self.by_file.get(&id)
    }

    pub fn name(&self, id: NameId) -> &str {
        self.strings.get(id)
    }

    pub fn signature(&self, id: SignatureId) -> &str {
        self.strings.get(id)
    }

    pub fn symbols_named(&self, name: &str) -> Vec<&SymbolRecord> {
        let Some(id) = self.strings.find(name) else {
            return Vec::new();
        };
        self.by_name
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|symbol| self.symbol(*symbol))
            .collect()
    }

    pub fn occurrences_of(&self, symbol: SymbolId) -> &[Occurrence] {
        self.occurrences
            .get(&symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn symbols_in_file(&self, file: FileId) -> Vec<&SymbolRecord> {
        self.by_file
            .get(&file)
            .map(|index| {
                index
                    .declarations
                    .iter()
                    .filter_map(|symbol| self.symbol(*symbol))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn occurrence_at(&self, file: FileId, offset: u32) -> Option<&Occurrence> {
        let index = self.by_file.get(&file)?;
        index
            .occurrences
            .iter()
            .find(|occurrence| occurrence.range.contains(offset))
    }

    pub fn is_ready(&self) -> bool {
        self.status.is_ready()
    }

    pub fn symbols(&self) -> impl Iterator<Item = &SymbolRecord> {
        self.symbols.iter()
    }

    /// Look up a live declaration by its fully qualified name.
    ///
    /// Removed declarations keep their id slot (ids are stable) but are no
    /// longer listed by any file index, so the scan is over live symbols only.
    pub fn symbol_by_flat(&self, flat_name: &str) -> Option<SymbolId> {
        self.by_file
            .values()
            .flat_map(|file_index| file_index.declarations.iter())
            .copied()
            .find(|id| {
                self.symbols
                    .get(*id as usize)
                    .is_some_and(|record| record.flat_name == flat_name)
            })
    }

    /// The short name an editor shows for a declaration.
    pub fn display_name(&self, record: &SymbolRecord) -> String {
        simple_name(&record.flat_name)
    }

    /// Rough resident-memory estimate of this snapshot, in bytes.
    ///
    /// Used by the memory-bound test; deliberately conservative.
    pub fn approximate_memory_bytes(&self) -> usize {
        let mut total = std::mem::size_of::<Self>();
        for (_, text) in self.strings.iter() {
            total += text.len() + 24;
        }
        total += self.symbols.len() * std::mem::size_of::<SymbolRecord>();
        for record in &self.symbols {
            total += record.flat_name.len() + record.detail.len();
            if let Some(javadoc) = &record.javadoc {
                total += javadoc.len();
            }
            total += record.supertypes.len() * std::mem::size_of::<TypeRef>();
        }
        let occurrence_count: usize = self.occurrences.values().map(Vec::len).sum();
        total += occurrence_count * std::mem::size_of::<Occurrence>();
        total += self.by_file.len() * std::mem::size_of::<FileIndex>();
        total += self.files.len() * std::mem::size_of::<FileRecord>();
        for record in &self.files {
            total += record.path.to_string_lossy().len();
        }
        total += self.by_key.len() * (std::mem::size_of::<SymbolKey>() + 24);
        total += self.by_name.len() * 24;
        total
    }
}

/// `p1.A` -> `A`, `Outer.Inner` -> `Inner`.
pub fn simple_name(flat_name: &str) -> String {
    flat_name
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(flat_name)
        .to_string()
}
