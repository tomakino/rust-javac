//! Single-file incremental classification.
//!
//! The updater never mutates an index in place. It compares the new file's
//! declaration fingerprint with the published snapshot and tells the workspace
//! whether the change is:
//!
//! - `Unchanged`: same content hash;
//! - `OccurrencesOnly`: declarations/signatures/supertypes are identical, so
//!   only this file's occurrence table needs replacing;
//! - `Rebuild`: declarations changed; the listed dependent files (plus this
//!   file) must be re-indexed in the background.

use std::path::PathBuf;

use super::key::{FileId, IndexSymbolKind};
use super::model::ProjectIndex;

/// One declaration as compared by the updater.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub flat_name: String,
    pub name: String,
    pub kind: IndexSymbolKind,
    pub signature: String,
    pub supertypes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDeclarations {
    pub declarations: Vec<Declaration>,
}

impl FileDeclarations {
    /// Declaration-set + signature + supertype equality.
    pub fn same_shape(&self, other: &Self) -> bool {
        let mut left = self.declarations.clone();
        let mut right = other.declarations.clone();
        left.sort_by(|a, b| a.flat_name.cmp(&b.flat_name).then(a.kind.tag().cmp(&b.kind.tag())));
        right.sort_by(|a, b| a.flat_name.cmp(&b.flat_name).then(a.kind.tag().cmp(&b.kind.tag())));
        left == right
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    Unchanged,
    OccurrencesOnly,
    Rebuild { dirty_files: Vec<FileId> },
}

/// The declarations currently recorded for one file.
pub fn declarations_of(index: &ProjectIndex, file: FileId) -> FileDeclarations {
    let mut declarations = Vec::new();
    for symbol in index.symbols_in_file(file) {
        declarations.push(Declaration {
            flat_name: symbol.flat_name.clone(),
            name: index.name(symbol.key.name).to_string(),
            kind: symbol.kind,
            signature: index.signature(symbol.key.signature).to_string(),
            supertypes: symbol
                .supertypes
                .iter()
                .map(|supertype| match supertype {
                    super::model::TypeRef::Project(id) => index
                        .symbol(*id)
                        .map(|record| record.flat_name.clone())
                        .unwrap_or_default(),
                    super::model::TypeRef::External(name) => name.clone(),
                })
                .collect(),
        });
    }
    FileDeclarations { declarations }
}

/// Files that reference a symbol declared by `file`.
///
/// The index has no reverse dependency table, so the updater conservatively
/// returns every file that holds an occurrence of one of `file`'s symbols.
pub fn dependent_files(index: &ProjectIndex, file: FileId) -> Vec<FileId> {
    let mut files = Vec::new();
    for symbol in index.symbols_in_file(file) {
        let Some(record) = index.symbol_by_flat(&symbol.flat_name) else {
            continue;
        };
        for occurrence in index.occurrences_of(record) {
            if occurrence.file != file && !files.contains(&occurrence.file) {
                files.push(occurrence.file);
            }
        }
    }
    files
}

/// Classify a change against the published snapshot.
pub fn classify(
    index: &ProjectIndex,
    file: FileId,
    old_hash: u64,
    new_hash: u64,
    new_declarations: Option<&FileDeclarations>,
) -> UpdateOutcome {
    if old_hash == new_hash {
        return UpdateOutcome::Unchanged;
    }
    let Some(new_declarations) = new_declarations else {
        return UpdateOutcome::Rebuild {
            dirty_files: vec![file],
        };
    };
    let old_declarations = declarations_of(index, file);
    if old_declarations.same_shape(new_declarations) {
        return UpdateOutcome::OccurrencesOnly;
    }
    let mut dirty_files = dependent_files(index, file);
    dirty_files.push(file);
    UpdateOutcome::Rebuild { dirty_files }
}

/// Paths of the dirty files; handy for `IndexStatus` and logging.
pub fn dirty_paths(index: &ProjectIndex, dirty: &[FileId]) -> Vec<PathBuf> {
    dirty
        .iter()
        .filter_map(|file| index.file(*file).map(|record| record.path.clone()))
        .collect()
}

/// Whether a file record is eligible for an incremental update.
pub fn is_incremental_candidate(index: &ProjectIndex, file: FileId) -> bool {
    index
        .file(file)
        .is_some_and(|record| matches!(record.status, super::model::FileStatus::Indexed))
}
