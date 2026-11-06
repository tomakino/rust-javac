//! Compact, compiler-independent project index model.

use std::collections::HashMap;
use std::path::PathBuf;

pub type FileId = u32;
pub type SymbolId = u32;
pub type NameId = u32;
pub type SignatureId = u32;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexSymbolKind {
    Class,
    Interface,
    Enum,
    Annotation,
    Record,
    Field,
    Method,
    Constructor,
    Local,
    Parameter,
    TypeParameter,
    Other,
}

impl IndexSymbolKind {
    pub fn is_type(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Interface | Self::Enum | Self::Annotation | Self::Record
        )
    }

    pub fn is_member(self) -> bool {
        matches!(self, Self::Field | Self::Method | Self::Constructor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolKey {
    pub owner: Option<SymbolId>,
    pub name: NameId,
    pub kind: IndexSymbolKind,
    pub signature: SignatureId,
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
    pub kind: IndexSymbolKind,
    pub file: FileId,
    pub range: ByteRange,
    pub detail: String,
    pub modifiers: u32,
    pub flat_name: String,
    pub supertypes: Vec<TypeRef>,
    pub return_type: Option<TypeRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSetKind {
    Main,
    Test,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub path: PathBuf,
    pub hash: u64,
    pub source_set: SourceSetKind,
    pub failed: bool,
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

#[derive(Debug, Default)]
pub struct StringInterner {
    strings: Vec<String>,
    pub(crate) ids: HashMap<String, u32>,
}

impl StringInterner {
    pub fn intern(&mut self, value: &str) -> u32 {
        if let Some(id) = self.ids.get(value) {
            return *id;
        }
        let id = self.strings.len() as u32;
        let owned = value.to_string();
        self.strings.push(owned.clone());
        self.ids.insert(owned, id);
        id
    }

    pub fn get(&self, id: u32) -> &str {
        self.strings
            .get(id as usize)
            .map(String::as_str)
            .unwrap_or("")
    }
}

#[derive(Debug)]
pub struct ProjectIndex {
    pub generation: u64,
    pub strings: StringInterner,
    pub symbols: Vec<SymbolRecord>,
    pub by_key: HashMap<SymbolKey, SymbolId>,
    pub by_name: HashMap<NameId, Vec<SymbolId>>,
    pub by_file: HashMap<FileId, FileIndex>,
    pub by_supertype: HashMap<SymbolId, Vec<SymbolId>>,
    pub occurrences: HashMap<SymbolId, Vec<Occurrence>>,
    pub files: Vec<FileRecord>,
    pub status: IndexStatus,
}

impl ProjectIndex {
    pub fn symbol(&self, id: SymbolId) -> Option<&SymbolRecord> {
        self.symbols.get(id as usize)
    }

    pub fn file(&self, id: FileId) -> Option<&FileRecord> {
        self.files.get(id as usize)
    }

    pub fn file_index(&self, id: FileId) -> Option<&FileIndex> {
        self.by_file.get(&id)
    }

    pub fn name(&self, id: NameId) -> &str {
        self.strings.get(id)
    }

    pub fn symbols_named(&self, name: &str) -> Vec<&SymbolRecord> {
        let Some(id) = self.strings.ids.get(name).copied() else {
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
        matches!(self.status, IndexStatus::Ready)
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceModel {
    pub root: PathBuf,
    pub source_path: Vec<PathBuf>,
    pub class_path: Vec<PathBuf>,
    pub source_level: String,
    pub classpath_hash: u64,
    pub generation: u64,
}
impl WorkspaceModel {
    pub fn from_config(config: &crate::analysis::AnalysisConfig) -> Self {
        use std::hash::{Hash, Hasher};
        let mut classpath_hasher = std::collections::hash_map::DefaultHasher::new();
        for entry in &config.class_path {
            entry.to_string_lossy().hash(&mut classpath_hasher);
        }
        let classpath_hash = classpath_hasher.finish();
        Self {
            root: config
                .source_path
                .first()
                .cloned()
                .unwrap_or_else(|| config.output_dir.clone()),
            source_path: config.source_path.clone(),
            class_path: config.class_path.clone(),
            source_level: config.source_level.clone(),
            classpath_hash,
            generation: 0,
        }
    }
}
