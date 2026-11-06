//! Batch incremental updates: file adds, edits, and deletes in one snapshot swap.
//!
//! Declarations are matched by `(flat name, kind, signature)`, so an unchanged
//! declaration keeps its `SymbolId` and references from other files stay valid.
//! The workspace decides which files form the rebuild set; this module only
//! guarantees that a removal is rejected when something outside the set still
//! references it.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::extract::{ExtractedDeclaration, ExtractedFile, SymbolIdentity};
use super::key::{FileId, IndexSymbolKind, SymbolId, SymbolKey};
use super::model::{
    FileIndex, FileRecord, FileStatus, Occurrence, OccurrenceRole, ProjectIndex, SourceSetKind,
    SymbolRecord, TypeRef,
};

/// One incremental change to apply to a snapshot.
#[derive(Debug)]
pub enum UpdateTask {
    /// An existing file's content changed.
    Update {
        file: FileId,
        text_hash: u64,
        extraction: ExtractedFile,
    },
    /// A file that is not in the snapshot yet. `file` must be the next id.
    Add {
        file: FileId,
        path: PathBuf,
        source_set: SourceSetKind,
        text_hash: u64,
        extraction: ExtractedFile,
    },
    /// A file that disappeared from disk.
    Delete { file: FileId },
}

/// Apply a batch of incremental changes.
///
/// The update/add tasks define the rebuild set. A removed declaration is
/// rejected (`None`) when a file outside that set still references it, so the
/// caller can fall back to a full rebuild instead of silently dropping a
/// reference.
pub fn apply_batch(index: &ProjectIndex, tasks: &[UpdateTask]) -> Option<ProjectIndex> {
    let mut next = index.clone();
    let mut rebuilding: HashSet<FileId> = HashSet::new();
    let mut deleting: HashSet<FileId> = HashSet::new();
    for task in tasks {
        match task {
            UpdateTask::Update { file, .. } | UpdateTask::Add { file, .. } => {
                rebuilding.insert(*file);
            }
            UpdateTask::Delete { file } => {
                deleting.insert(*file);
            }
        }
    }

    // Reserve records for added files; ids must be sequential.
    for task in tasks {
        if let UpdateTask::Add {
            file,
            path,
            source_set,
            ..
        } = task
        {
            if *file as usize != next.files.len() {
                return None;
            }
            next.files.push(FileRecord {
                path: path.clone(),
                hash: 0,
                source_set: *source_set,
                status: FileStatus::Indexed,
            });
            next.by_file.insert(*file, FileIndex::default());
        }
    }

    for task in tasks {
        match task {
            UpdateTask::Update {
                file,
                text_hash,
                extraction,
            }
            | UpdateTask::Add {
                file,
                text_hash,
                extraction,
                ..
            } => {
                apply_update(
                    &mut next,
                    *file,
                    *text_hash,
                    extraction,
                    &rebuilding,
                    &deleting,
                )?;
            }
            UpdateTask::Delete { file } => apply_delete(&mut next, *file),
        }
    }
    next.compute_dependents();
    Some(next)
}

/// Whether a file's non-local declaration shape (identities + supertypes)
/// differs from the snapshot.
///
/// Local variables are ignored: their identity contains a declaration offset,
/// so a body edit would otherwise force a dependent rebuild.
pub fn declarations_changed(
    index: &ProjectIndex,
    file: FileId,
    extraction: &ExtractedFile,
) -> bool {
    let mut old: Vec<(String, u8, String, Vec<String>)> = index
        .symbols_in_file(file)
        .into_iter()
        .filter(|record| !is_local_kind(record.kind))
        .map(|record| {
            (
                record.flat_name.clone(),
                record.kind.tag(),
                index.signature(record.key.signature).to_string(),
                supertype_flats(index, record),
            )
        })
        .collect();
    let mut new: Vec<(String, u8, String, Vec<String>)> = extraction
        .declarations
        .iter()
        .filter(|declaration| !declaration.identity.is_local())
        .map(|declaration| {
            let mut supertypes = declaration.supertypes.clone();
            supertypes.sort();
            (
                declaration.identity.flat_name.clone(),
                declaration.identity.kind.tag(),
                declaration.identity.signature.clone(),
                supertypes,
            )
        })
        .collect();
    old.sort();
    new.sort();
    old != new
}

fn is_local_kind(kind: IndexSymbolKind) -> bool {
    matches!(
        kind,
        IndexSymbolKind::Local | IndexSymbolKind::Parameter | IndexSymbolKind::TypeParameter
    )
}

fn supertype_flats(index: &ProjectIndex, record: &SymbolRecord) -> Vec<String> {
    let mut flats: Vec<String> = record
        .supertypes
        .iter()
        .map(|supertype| match supertype {
            TypeRef::Project(id) => index
                .symbol(*id)
                .map(|record| record.flat_name.clone())
                .unwrap_or_default(),
            TypeRef::External(name) => name.clone(),
        })
        .collect();
    flats.sort();
    flats
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

fn apply_update(
    next: &mut ProjectIndex,
    file: FileId,
    text_hash: u64,
    extraction: &ExtractedFile,
    rebuilding: &HashSet<FileId>,
    deleting: &HashSet<FileId>,
) -> Option<()> {
    if extraction.failed || next.file(file).is_none() {
        return None;
    }

    // Live symbols and their identities in the current snapshot.
    let live: HashSet<SymbolId> = next
        .by_file
        .values()
        .flat_map(|file_index| file_index.declarations.iter().copied())
        .collect();
    let mut existing: HashMap<IdentityKey, SymbolId> = HashMap::new();
    for id in &live {
        if let Some(record) = next.symbols.get(*id as usize) {
            existing.insert(record_key(next, record), *id);
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
            update_record(next, id, declaration);
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

    // Removed declarations: safe only when every reference comes from a file
    // that is part of this batch (it will be re-extracted) or deleted.
    let removed: Vec<SymbolId> = old_ids
        .iter()
        .copied()
        .filter(|id| !new_declarations.contains(id))
        .collect();
    for id in &removed {
        let outside_reference = next.occurrences_of(*id).iter().any(|occurrence| {
            occurrence.file != file
                && !rebuilding.contains(&occurrence.file)
                && !deleting.contains(&occurrence.file)
        });
        if outside_reference {
            return None;
        }
        let outside_supertype = next.symbols.iter().enumerate().any(|(other, record)| {
            let other = other as SymbolId;
            live.contains(&other)
                && other != *id
                && record.file != file
                && !rebuilding.contains(&record.file)
                && !deleting.contains(&record.file)
                && record
                    .supertypes
                    .iter()
                    .any(|target| matches!(target, TypeRef::Project(target) if *target == *id))
        });
        if outside_supertype {
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
            match lookup_type(next, flat, &live_now) {
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
    file_occurrences
        .dedup_by(|a, b| a.range == b.range && a.symbol == b.symbol && a.role == b.role);

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
    Some(())
}

/// Remove a file and every index fact that points at it.
///
/// References from other files to the deleted declarations are cleared rather
/// than treated as unsafe: the source that used them no longer compiles, and a
/// full rebuild would only reproduce a `Partial` index.
fn apply_delete(next: &mut ProjectIndex, file: FileId) {
    let ids: Vec<SymbolId> = next
        .by_file
        .get(&file)
        .map(|file_index| file_index.declarations.clone())
        .unwrap_or_default();
    let id_set: HashSet<SymbolId> = ids.iter().copied().collect();

    // Clear occurrences in other files that pointed at the deleted symbols.
    for (other, file_index) in next.by_file.iter_mut() {
        if *other == file {
            continue;
        }
        file_index
            .occurrences
            .retain(|occurrence| !id_set.contains(&occurrence.symbol));
    }
    // Clear the deleted file's own occurrences from the global table.
    if let Some(file_index) = next.by_file.get(&file) {
        let own: Vec<Occurrence> = file_index.occurrences.clone();
        for occurrence in own {
            if let Some(list) = next.occurrences.get_mut(&occurrence.symbol) {
                list.retain(|candidate| candidate.file != file);
            }
        }
    }

    let mut flats: HashMap<SymbolId, String> = HashMap::new();
    for id in &ids {
        let Some(record) = next.symbols.get(*id as usize) else {
            continue;
        };
        flats.insert(*id, record.flat_name.clone());
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

    // Supertype edges to deleted types become external references.
    for record in next.symbols.iter_mut() {
        for supertype in record.supertypes.iter_mut() {
            if let TypeRef::Project(target) = supertype
                && let Some(flat) = flats.get(target)
            {
                *supertype = TypeRef::External(flat.clone());
            }
        }
    }

    next.by_file.remove(&file);
    if let Some(record) = next.files.get_mut(file as usize) {
        record.status = FileStatus::Deleted;
        record.hash = 0;
    }
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
