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

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::extract::{ExtractedDeclaration, ExtractedFile, SymbolIdentity};
use super::key::{FileId, IndexSymbolKind, SymbolId, SymbolKey};
use super::model::{
    FileIndex, FileStatus, Occurrence, OccurrenceRole, ProjectIndex, SymbolRecord, TypeRef,
};

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

/// Identity used to match declarations across snapshots.
type IdentityKey = (String, u8, String);

fn identity_key(identity: &SymbolIdentity) -> IdentityKey {
    (
        identity.flat_name.clone(),
        identity.kind.tag(),
        identity.signature.clone(),
    )
}

fn record_key(index: &ProjectIndex, record: &SymbolRecord) -> IdentityKey {
    (
        record.flat_name.clone(),
        record.kind.tag(),
        index.signature(record.key.signature).to_string(),
    )
}

/// Replace one file's index facts without rebuilding the project.
///
/// Declarations are matched by `(flat name, kind, signature)`, so an unchanged
/// declaration keeps its `SymbolId` (and therefore all references from other
/// files stay valid). Removed declarations are allowed only when nothing else
/// references them. Returns `None` when the change cannot be applied safely;
/// the caller then falls back to a full rebuild.
pub fn apply_file_update(
    index: &ProjectIndex,
    file: FileId,
    text_hash: u64,
    extraction: &ExtractedFile,
) -> Option<ProjectIndex> {
    if extraction.failed || index.file(file).is_none() {
        return None;
    }
    let mut next = index.clone();

    // Live symbols and their identities in the old snapshot.
    let live: HashSet<SymbolId> = next
        .by_file
        .values()
        .flat_map(|file_index| file_index.declarations.iter().copied())
        .collect();
    let mut existing: HashMap<IdentityKey, SymbolId> = HashMap::new();
    for id in &live {
        if let Some(record) = next.symbols.get(*id as usize) {
            existing.insert(record_key(&next, record), *id);
        }
    }
    let old_ids: Vec<SymbolId> = next
        .by_file
        .get(&file)
        .map(|file_index| file_index.declarations.clone())
        .unwrap_or_default();

    // Match or create declarations, in parent-before-child order.
    let mut new_declarations: Vec<SymbolId> = Vec::new();
    let mut new_identities: HashMap<IdentityKey, SymbolId> = HashMap::new();
    for declaration in &extraction.declarations {
        let key = identity_key(&declaration.identity);
        if let Some(id) = existing.get(&key).copied() {
            let Some(record) = next.symbols.get(id as usize) else {
                return None;
            };
            if record.file != file {
                return None;
            }
            update_record(&mut next, id, declaration);
            new_declarations.push(id);
            new_identities.insert(key, id);
            continue;
        }
        let owner = match &declaration.owner {
            Some(owner_identity) => {
                let owner_key = identity_key(owner_identity);
                Some(
                    new_identities
                        .get(&owner_key)
                        .copied()
                        .or_else(|| existing.get(&owner_key).copied())?,
                )
            }
            None => None,
        };
        let key_name = if owner.is_none() && declaration.identity.kind.is_type() {
            declaration.identity.flat_name.clone()
        } else {
            declaration.name.clone()
        };
        let symbol_key = SymbolKey {
            owner,
            name: next.strings.intern(&key_name),
            kind: declaration.identity.kind,
            signature: next.strings.intern(&declaration.identity.signature),
        };
        if next.by_key.contains_key(&symbol_key) {
            return None;
        }
        let id = next.symbols.len() as SymbolId;
        next.symbols.push(SymbolRecord {
            key: symbol_key,
            kind: declaration.identity.kind,
            file,
            range: declaration.range,
            detail: declaration.detail.clone(),
            modifiers: declaration.modifiers,
            flat_name: declaration.identity.flat_name.clone(),
            supertypes: Vec::new(),
            return_type: None,
            javadoc: declaration.javadoc.clone(),
        });
        next.by_key.insert(symbol_key, id);
        next.by_name.entry(symbol_key.name).or_default().push(id);
        new_declarations.push(id);
        new_identities.insert(key, id);
    }

    // Removed declarations: only safe when nothing outside this file uses them.
    let removed: Vec<SymbolId> = old_ids
        .iter()
        .copied()
        .filter(|id| !new_declarations.contains(id))
        .collect();
    for id in &removed {
        if next.occurrences_of(*id).iter().any(|occ| occ.file != file) {
            return None;
        }
        let referenced_by_supertype = next.symbols.iter().enumerate().any(|(other, record)| {
            let other = other as SymbolId;
            live.contains(&other)
                && other != *id
                && record
                    .supertypes
                    .iter()
                    .any(|target| matches!(target, TypeRef::Project(target) if *target == *id))
        });
        if referenced_by_supertype {
            return None;
        }
    }

    // Supertypes for every declaration of this file.
    let live_now: HashSet<SymbolId> = live
        .iter()
        .copied()
        .filter(|id| !removed.contains(id))
        .chain(new_declarations.iter().copied())
        .collect();
    for declaration in &extraction.declarations {
        let Some(id) = new_identities.get(&identity_key(&declaration.identity)).copied() else {
            continue;
        };
        next.by_supertype.remove(&id);
        let mut supertypes = Vec::new();
        for flat in &declaration.supertypes {
            match lookup_type(&next, flat, &live_now) {
                Some(target) => {
                    supertypes.push(TypeRef::Project(target));
                    next.by_supertype.entry(target).or_default().push(id);
                }
                None => supertypes.push(TypeRef::External(flat.clone())),
            }
        }
        if let Some(record) = next.symbols.get_mut(id as usize) {
            record.supertypes = supertypes;
        }
    }

    // Drop this file's old occurrences from the global table.
    if let Some(file_index) = next.by_file.get(&file) {
        for occurrence in &file_index.occurrences {
            if let Some(list) = next.occurrences.get_mut(&occurrence.symbol) {
                list.retain(|candidate| candidate.file != file);
            }
        }
    }

    // Forget removed symbols. Their `SymbolId` slots stay allocated (ids are
    // stable), but no table references them any more.
    for id in &removed {
        let Some(record) = next.symbols.get(*id as usize) else {
            continue;
        };
        let key = record.key;
        let name = record.key.name;
        next.by_key.remove(&key);
        if let Some(ids) = next.by_name.get_mut(&name) {
            ids.retain(|candidate| candidate != id);
            if ids.is_empty() {
                next.by_name.remove(&name);
            }
        }
        next.occurrences.remove(id);
        next.by_supertype.remove(id);
    }

    // Map the new occurrences to stable symbol ids. References to symbols
    // outside the project (JDK/classpath types) have no index entry and are
    // skipped, matching the builder.
    let mut file_occurrences = Vec::new();
    for occurrence in &extraction.occurrences {
        let key = identity_key(&occurrence.identity);
        let Some(id) = new_identities
            .get(&key)
            .copied()
            .or_else(|| existing.get(&key).copied())
        else {
            continue;
        };
        if occurrence.role == OccurrenceRole::Declaration {
            if let Some(record) = next.symbols.get_mut(id as usize)
                && record.file == file
            {
                record.range = occurrence.range;
            }
        }
        file_occurrences.push(Occurrence {
            file,
            range: occurrence.range,
            symbol: id,
            role: occurrence.role,
        });
    }
    file_occurrences.sort_by_key(|occurrence| {
        (
            occurrence.range.start,
            occurrence.range.end,
            occurrence.symbol,
            occurrence.role.tag(),
        )
    });
    file_occurrences.dedup_by(|a, b| {
        a.range == b.range && a.symbol == b.symbol && a.role == b.role
    });

    for occurrence in &file_occurrences {
        let entry = next.occurrences.entry(occurrence.symbol).or_default();
        let duplicate = entry.iter().any(|existing| {
            existing.file == file
                && existing.range == occurrence.range
                && existing.role == occurrence.role
        });
        if !duplicate {
            entry.push(occurrence.clone());
        }
    }
    next.by_file.insert(
        file,
        FileIndex {
            declarations: new_declarations,
            occurrences: file_occurrences,
        },
    );
    if let Some(record) = next.files.get_mut(file as usize) {
        record.hash = text_hash;
        record.status = FileStatus::Indexed;
    }
    next.compute_dependents();
    Some(next)
}

fn update_record(next: &mut ProjectIndex, id: SymbolId, declaration: &ExtractedDeclaration) {
    if let Some(record) = next.symbols.get_mut(id as usize) {
        record.range = declaration.range;
        record.detail = declaration.detail.clone();
        record.modifiers = declaration.modifiers;
        record.javadoc = declaration.javadoc.clone();
    }
}

fn lookup_type(
    index: &ProjectIndex,
    flat_name: &str,
    live: &HashSet<SymbolId>,
) -> Option<SymbolId> {
    index.symbols.iter().enumerate().find_map(|(id, record)| {
        let id = id as SymbolId;
        (live.contains(&id) && record.flat_name == flat_name && record.kind.is_type())
            .then_some(id)
    })
}
