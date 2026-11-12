//! Index-backed query surface.
//!
//! Every function takes an immutable [`ProjectIndex`] snapshot plus a text
//! provider, so queries never touch the compiler and never mutate the index.

use std::path::Path;

use javac_analysis_api::{DocumentSymbol, Location, Position, Range, WorkspaceSymbol};

use crate::analysis::positions::offset_to_line_col;

use super::key::{FileId, IndexSymbolKind, SymbolId};
use super::model::{ProjectIndex, SymbolRecord};
use super::symbols::api_symbol_kind;

/// AST parameter names for a project method/constructor.
///
/// The caller passes the compiler symbol's method name (constructors are
/// `<init>`), arity and declaring owner's fully qualified name. This is used
/// only when the class-file symbol has no parameter names; it never scans
/// source text.
pub fn parameter_names(
    index: &ProjectIndex,
    method_name: &str,
    parameter_count: usize,
    owner_flat_name: Option<&str>,
) -> Option<Vec<String>> {
    index.symbols_named(method_name).into_iter().find_map(|record| {
        if !matches!(
            record.kind,
            IndexSymbolKind::Method | IndexSymbolKind::Constructor
        ) || record.parameter_names.len() != parameter_count
            || record.parameter_names.is_empty()
        {
            return None;
        }
        if let Some(owner_flat_name) = owner_flat_name {
            let owner = record.key.owner.and_then(|owner| index.symbol(owner))?;
            if owner.flat_name != owner_flat_name {
                return None;
            }
        }
        Some(record.parameter_names.clone())
    })
}

/// UTF-16 (Java char) offset of an LSP position.
///
/// [`super::model::ByteRange`] stores Java char offsets, so position lookups
/// must be converted to the same unit here.
pub fn offset_utf16(text: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut column = 0u32;
    let mut offset = 0usize;
    for ch in text.chars() {
        if line == position.line && column >= position.column {
            return offset;
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += ch.len_utf16() as u32;
        }
        offset += ch.len_utf16();
    }
    offset
}

/// The symbol referenced or declared at a position.
pub fn symbol_at(index: &ProjectIndex, file: FileId, text: &str, position: Position) -> Option<SymbolId> {
    let offset = offset_utf16(text, position) as u32;
    index
        .occurrence_at(file, offset)
        .map(|occurrence| occurrence.symbol)
}

/// Location of a symbol's declaration.
pub fn symbol_location(
    index: &ProjectIndex,
    symbol: SymbolId,
    text_of: &dyn Fn(FileId) -> Option<String>,
) -> Option<Location> {
    let record = index.symbol(symbol)?;
    let file = index.file(record.file)?;
    let text = text_of(record.file)?;
    Some(location_for_range(
        &file.path,
        &text,
        record.range.start,
        record.range.end,
    ))
}

/// An overridden project method with enough data for hover docs and links.
#[derive(Debug, Clone)]
pub struct OverridingMethod {
    pub symbol: SymbolId,
    pub owner: String,
    pub member: String,
    pub signature: String,
    pub display: String,
    pub location: Option<Location>,
    pub javadoc: Option<String>,
}

/// The methods a method overrides, using index data only (project sources are
/// not on the analysis compiler's source path, so hover needs this fallback).
///
/// Parent methods are matched by name + parameter fingerprint and must be
/// instance (non-static, non-private) declarations.
pub fn overriding_method_entries(
    index: &ProjectIndex,
    symbol: SymbolId,
) -> Vec<OverridingMethod> {
    let Some(record) = index.symbol(symbol) else {
        return Vec::new();
    };
    if record.kind != super::key::IndexSymbolKind::Method {
        return Vec::new();
    }
    let Some(owner) = record.key.owner else {
        return Vec::new();
    };
    let Some(owner_record) = index.symbol(owner) else {
        return Vec::new();
    };
    let mut result: Vec<OverridingMethod> = Vec::new();
    let mut pending: Vec<super::model::TypeRef> = owner_record.supertypes.clone();
    let mut seen = std::collections::HashSet::new();
    while let Some(supertype) = pending.pop() {
        let super::model::TypeRef::Project(parent) = supertype else {
            continue;
        };
        if !seen.insert(parent) {
            continue;
        }
        let Some(parent_record) = index.symbol(parent) else {
            continue;
        };
        pending.extend(parent_record.supertypes.iter().cloned());
        let Some(file_index) = index.file_index(parent_record.file) else {
            continue;
        };
        for candidate in &file_index.declarations {
            let Some(candidate_record) = index.symbol(*candidate) else {
                continue;
            };
            if candidate_record.kind != super::key::IndexSymbolKind::Method
                || candidate_record.key.name != record.key.name
                || candidate_record.key.signature != record.key.signature
                || candidate_record.modifiers & crate::code::flags::STATIC as u32 != 0
                || candidate_record.modifiers & crate::code::flags::PRIVATE as u32 != 0
            {
                continue;
            }
            let parent_name = parent_record.flat_name.replace('$', ".");
            let detail = &candidate_record.detail;
            let display = format!("{parent_name}#{detail}");
            if result.iter().any(|entry| entry.display == display) {
                continue;
            }
            result.push(OverridingMethod {
                symbol: *candidate,
                owner: parent_name.clone(),
                member: index.name(candidate_record.key.name).to_string(),
                signature: index.signature(candidate_record.key.signature).to_string(),
                display,
                location: symbol_location(index, *candidate, &|file_id| {
                    index
                        .file(file_id)
                        .and_then(|file| std::fs::read_to_string(&file.path).ok())
                }),
                javadoc: candidate_record.javadoc.clone(),
            });
        }
    }
    result
}

/// Human-readable names of the overridden project methods.
pub fn overriding_methods(index: &ProjectIndex, symbol: SymbolId) -> Vec<String> {
    overriding_method_entries(index, symbol)
        .into_iter()
        .map(|entry| entry.display)
        .collect()
}

/// Declarations of the methods in `symbol`'s override pair: the overridden
/// declarations up the supertype chain and the overriding declarations in the
/// subtype table. Rename edits them together with the calls.
pub fn override_declarations(index: &ProjectIndex, symbol: SymbolId) -> Vec<SymbolId> {
    let Some(record) = index.symbol(symbol) else {
        return Vec::new();
    };
    if record.kind != super::key::IndexSymbolKind::Method {
        return Vec::new();
    }
    let Some(owner) = record.key.owner else {
        return Vec::new();
    };
    let Some(owner_record) = index.symbol(owner) else {
        return Vec::new();
    };
    let is_override_pair = |candidate: &super::model::SymbolRecord| {
        candidate.kind == super::key::IndexSymbolKind::Method
            && candidate.key.name == record.key.name
            && candidate.key.signature == record.key.signature
            && candidate.modifiers & crate::code::flags::STATIC as u32 == 0
            && candidate.modifiers & crate::code::flags::PRIVATE as u32 == 0
    };
    let mut result = Vec::new();
    // Parent declarations.
    let mut pending: Vec<super::model::TypeRef> = owner_record.supertypes.clone();
    let mut seen = std::collections::HashSet::new();
    while let Some(supertype) = pending.pop() {
        let super::model::TypeRef::Project(parent) = supertype else {
            continue;
        };
        if !seen.insert(parent) {
            continue;
        }
        let Some(parent_record) = index.symbol(parent) else {
            continue;
        };
        pending.extend(parent_record.supertypes.iter().cloned());
        let Some(file_index) = index.file_index(parent_record.file) else {
            continue;
        };
        for candidate in &file_index.declarations {
            if let Some(candidate_record) = index.symbol(*candidate)
                && is_override_pair(candidate_record)
            {
                result.push(*candidate);
            }
        }
    }
    // Child declarations (the subtype table maps a supertype to its subtypes).
    let mut pending = vec![owner];
    let mut seen = std::collections::HashSet::new();
    while let Some(current) = pending.pop() {
        if !seen.insert(current) {
            continue;
        }
        let Some(children) = index.by_supertype.get(&current) else {
            continue;
        };
        for child in children {
            pending.push(*child);
            let Some(child_record) = index.symbol(*child) else {
                continue;
            };
            let Some(file_index) = index.file_index(child_record.file) else {
                continue;
            };
            for candidate in &file_index.declarations {
                if let Some(candidate_record) = index.symbol(*candidate)
                    && is_override_pair(candidate_record)
                {
                    result.push(*candidate);
                }
            }
        }
    }
    result.sort_unstable();
    result.dedup();
    result
}

/// Implementation locations: subtypes of a type, or the declarations in
/// project subtypes that override a method.
pub fn implementations(
    index: &ProjectIndex,
    file: FileId,
    text: &str,
    position: Position,
    text_of: &dyn Fn(FileId) -> Option<String>,
) -> Vec<Location> {
    let Some(symbol) = symbol_at(index, file, text, position) else {
        return Vec::new();
    };
    let Some(record) = index.symbol(symbol) else {
        return Vec::new();
    };
    let mut declarations: Vec<SymbolId> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    match record.kind {
        super::key::IndexSymbolKind::Class
        | super::key::IndexSymbolKind::Interface
        | super::key::IndexSymbolKind::Enum
        | super::key::IndexSymbolKind::Record
        | super::key::IndexSymbolKind::Annotation => {
            let mut pending = vec![symbol];
            while let Some(current) = pending.pop() {
                let Some(children) = index.by_supertype.get(&current) else {
                    continue;
                };
                for child in children {
                    if !seen.insert(*child) {
                        continue;
                    }
                    pending.push(*child);
                    declarations.push(*child);
                }
            }
        }
        super::key::IndexSymbolKind::Method => {
            let Some(owner) = record.key.owner else {
                return Vec::new();
            };
            let mut pending = vec![owner];
            let mut subtypes = Vec::new();
            while let Some(current) = pending.pop() {
                let Some(children) = index.by_supertype.get(&current) else {
                    continue;
                };
                for child in children {
                    if !seen.insert(*child) {
                        continue;
                    }
                    pending.push(*child);
                    subtypes.push(*child);
                }
            }
            for subtype in subtypes {
                let Some(subtype_record) = index.symbol(subtype) else {
                    continue;
                };
                let Some(file_index) = index.file_index(subtype_record.file) else {
                    continue;
                };
                for candidate in &file_index.declarations {
                    if let Some(candidate_record) = index.symbol(*candidate)
                        && candidate_record.kind == super::key::IndexSymbolKind::Method
                        && candidate_record.key.name == record.key.name
                        && candidate_record.key.signature == record.key.signature
                        && candidate_record.modifiers & crate::code::flags::STATIC as u32 == 0
                        && candidate_record.modifiers & crate::code::flags::PRIVATE as u32 == 0
                    {
                        declarations.push(*candidate);
                    }
                }
            }
        }
        _ => return Vec::new(),
    }
    let mut locations: Vec<Location> = declarations
        .into_iter()
        .filter_map(|declaration| {
            let record = index.symbol(declaration)?;
            let file_record = index.file(record.file)?;
            let text = text_of(record.file)?;
            Some(location_for_range(
                &file_record.path,
                &text,
                record.range.start,
                record.range.end,
            ))
        })
        .collect();
    locations.sort_by(|left, right| {
        (
            left.file.to_string_lossy().to_string(),
            left.range.start.line,
            left.range.start.column,
        )
            .cmp(&(
                right.file.to_string_lossy().to_string(),
                right.range.start.line,
                right.range.start.column,
            ))
    });
    locations.dedup_by(|left, right| left.file == right.file && left.range == right.range);
    locations
}

/// Go-to-definition.
pub fn definition(
    index: &ProjectIndex,
    file: FileId,
    text: &str,
    position: Position,
    text_of: &dyn Fn(FileId) -> Option<String>,
) -> Option<Location> {
    let symbol = symbol_at(index, file, text, position)?;
    symbol_location(index, symbol, text_of)
}

/// All indexed occurrences of the symbol under the position.
pub fn references(
    index: &ProjectIndex,
    file: FileId,
    text: &str,
    position: Position,
    text_of: &dyn Fn(FileId) -> Option<String>,
) -> Vec<Location> {
    let Some(symbol) = symbol_at(index, file, text, position) else {
        return Vec::new();
    };
    let mut locations = Vec::new();
    for occurrence in index.occurrences_of(symbol) {
        let Some(file_record) = index.file(occurrence.file) else {
            continue;
        };
        let Some(text) = text_of(occurrence.file) else {
            continue;
        };
        locations.push(location_for_range(
            &file_record.path,
            &text,
            occurrence.range.start,
            occurrence.range.end,
        ));
    }
    locations.sort_by_key(|location| {
        (
            location.file.to_string_lossy().to_string(),
            location.range.start.line,
            location.range.start.column,
        )
    });
    locations.dedup_by(|a, b| a.file == b.file && a.range == b.range);
    locations
}

/// The range a rename would edit (the occurrence under the cursor).
pub fn prepare_rename_range(
    index: &ProjectIndex,
    file: FileId,
    text: &str,
    position: Position,
) -> Option<Range> {
    let offset = offset_utf16(text, position) as u32;
    let occurrence = index.occurrence_at(file, offset)?;
    let (start_line, start_col) = offset_to_line_col(text, occurrence.range.start as usize);
    let (end_line, end_col) = offset_to_line_col(text, occurrence.range.end as usize);
    Some(Range::new(
        Position::new(start_line, start_col),
        Position::new(end_line, end_col),
    ))
}

/// Outline symbols for one file.
pub fn document_symbols(index: &ProjectIndex, file: FileId, text: &str) -> Vec<DocumentSymbol> {
    let Some(file_index) = index.file_index(file) else {
        return Vec::new();
    };
    let declaration_set: std::collections::HashSet<SymbolId> =
        file_index.declarations.iter().copied().collect();
    let mut roots = Vec::new();
    for id in &file_index.declarations {
        let Some(record) = index.symbol(*id) else {
            continue;
        };
        if is_non_outline(record.kind) {
            continue;
        }
        let is_child = record
            .key
            .owner
            .is_some_and(|owner| declaration_set.contains(&owner));
        if !is_child {
            if let Some(node) = build_document_symbol(index, *id, text) {
                roots.push(node);
            }
        }
    }
    roots
}

fn build_document_symbol(
    index: &ProjectIndex,
    id: SymbolId,
    text: &str,
) -> Option<DocumentSymbol> {
    let record = index.symbol(id)?;
    if is_non_outline(record.kind) {
        return None;
    }
    let (start_line, start_col) = offset_to_line_col(text, record.range.start as usize);
    let (end_line, end_col) = offset_to_line_col(text, record.range.end as usize);
    let mut children = Vec::new();
    if let Some(file_index) = index.by_file.get(&record.file) {
        for child in &file_index.declarations {
            if *child == id {
                continue;
            }
            if let Some(child_record) = index.symbol(*child) {
                if child_record.key.owner == Some(id) {
                    if let Some(node) = build_document_symbol(index, *child, text) {
                        children.push(node);
                    }
                }
            }
        }
    }
    let name = index.display_name(record);
    if name.trim().is_empty() {
        // A nameless synthetic symbol must never reach the client: VS Code
        // rejects DocumentSymbol with `name must not be falsy`.
        return None;
    }
    Some(DocumentSymbol {
        name,
        kind: api_symbol_kind(record.kind),
        range: Range::new(
            Position::new(start_line, start_col),
            Position::new(end_line, end_col),
        ),
        detail: record
            .javadoc
            .clone()
            .or_else(|| Some(record.detail.clone())),
        children,
    })
}

fn is_non_outline(kind: IndexSymbolKind) -> bool {
    matches!(
        kind,
        IndexSymbolKind::Local | IndexSymbolKind::Parameter | IndexSymbolKind::Other
    )
}

/// Project-wide symbol search, IDEA-style: exact prefix beats substring.
pub fn workspace_symbols(
    index: &ProjectIndex,
    query: &str,
    text_of: &dyn Fn(FileId) -> Option<String>,
) -> Vec<WorkspaceSymbol> {
    let needle = query.trim().to_lowercase();
    let mut hits: Vec<(bool, &SymbolRecord)> = index
        .symbols
        .iter()
        .filter(|record| is_workspace_symbol(record.kind))
        .filter_map(|record| {
            let name = index.name(record.key.name);
            let lowered = name.to_lowercase();
            let starts = lowered.starts_with(&needle);
            (needle.is_empty() || ida_match(&lowered, &needle)).then_some((starts, record))
        })
        .collect();
    hits.sort_by(|(a_starts, a), (b_starts, b)| {
        b_starts
            .cmp(a_starts)
            .then_with(|| index.name(a.key.name).cmp(index.name(b.key.name)))
            .then_with(|| a.flat_name.cmp(&b.flat_name))
    });
    hits.into_iter()
        .take(200)
        .filter_map(|(_, record)| {
            let file = index.file(record.file)?;
            let text = text_of(record.file)?;
            let (start_line, start_col) = offset_to_line_col(&text, record.range.start as usize);
            let (end_line, end_col) = offset_to_line_col(&text, record.range.end as usize);
            let container = record
                .key
                .owner
                .and_then(|owner| index.symbol(owner))
                .map(|owner| owner.flat_name.clone());
            Some(WorkspaceSymbol {
                name: index.display_name(record),
                kind: api_symbol_kind(record.kind),
                location: Location {
                    file: file.path.clone(),
                    range: Range::new(
                        Position::new(start_line, start_col),
                        Position::new(end_line, end_col),
                    ),
                },
                container_name: container,
            })
        })
        .collect()
}

/// IDEA's `CamelHumps`-friendly matcher: prefix, then substring, then
/// subsequence of word starts.
fn ida_match(name: &str, needle: &str) -> bool {
    if name.starts_with(needle) || name.contains(needle) {
        return true;
    }
    // Subsequence over characters keeps `fbn` matching `FooBarName`.
    let mut chars = needle.chars().peekable();
    for ch in name.chars() {
        if chars.peek().is_some_and(|next| *next == ch) {
            chars.next();
        }
    }
    chars.peek().is_none()
}

fn is_workspace_symbol(kind: IndexSymbolKind) -> bool {
    matches!(
        kind,
        IndexSymbolKind::Class
            | IndexSymbolKind::Interface
            | IndexSymbolKind::Enum
            | IndexSymbolKind::Annotation
            | IndexSymbolKind::Record
            | IndexSymbolKind::Field
            | IndexSymbolKind::Method
            | IndexSymbolKind::Constructor
    )
}

pub fn location_for_range(path: &Path, text: &str, start: u32, end: u32) -> Location {
    let (start_line, start_column) = offset_to_line_col(text, start as usize);
    let (end_line, end_column) = offset_to_line_col(text, end as usize);
    Location {
        file: path.to_path_buf(),
        range: Range::new(
            Position::new(start_line, start_column),
            Position::new(end_line, end_column),
        ),
    }
}
