//! Project-wide index: model, builder, and query surface.

mod builder;
mod model;

pub use builder::{
    build_project_index, build_project_index_with_overlays, build_workspace_model, IndexBuilder,
};
pub use model::*;

use super::positions::{line_offset, offset_to_line_col};
use super::canonical;
use super::AnalysisSession;
use javac_analysis_api;
impl super::AnalysisSession {
    /// Build and cache the project index. M2 uses disk content and one build.
    pub(crate) fn ensure_project_index(&mut self) -> Option<&ProjectIndex> {
        if self.project_index.is_none() && !self.config.source_path.is_empty() {
            self.project_index = Some(build_project_index(&self.config, 128));
        }
        self.project_index.as_ref()
    }

    /// Symbol at a position according to the project index, when the current
    /// buffer is on disk and the file belongs to the index.
    pub(crate) fn project_symbol_at(
        &mut self,
        file: &std::path::Path,
        position: javac_analysis_api::Position,
    ) -> Option<SymbolId> {
        let path = std::path::PathBuf::from(canonical(file));
        if self.project_index.is_none() && !self.config.source_path.is_empty() {
            let overlays: std::collections::HashMap<std::path::PathBuf, String> = self
                .texts
                .iter()
                .filter(|(path, text)| {
                    std::fs::read_to_string(path).ok().as_deref() != Some(text.as_str())
                })
                .map(|(path, text)| (path.clone(), text.clone()))
                .collect();
            self.project_index = Some(build_project_index_with_overlays(
                &self.config,
                128,
                &overlays,
            ));
        }
        let index = self.project_index.as_ref()?;
        let file_id = index.file_id_for_path(file)?;
        let text = self
            .texts
            .get(&path)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let offset = line_offset(&text, line, column) as u32;
        index.occurrence_at(file_id, offset).map(|occurrence| occurrence.symbol)
    }

    pub(crate) fn project_symbol_record(
        &self,
        symbol: SymbolId,
    ) -> Option<&SymbolRecord> {
        self.project_index.as_ref()?.symbol(symbol)
    }


    pub(crate) fn project_definition(
        &mut self,
        file: &std::path::Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        let symbol = self.project_symbol_at(file, position)?;
        let index = self.project_index.as_ref()?;
        let record = index.symbol(symbol)?;
        let file_record = index.file(record.file)?;
        let text = self
            .texts
            .get(&std::path::PathBuf::from(canonical(&file_record.path)))
            .cloned()
            .or_else(|| std::fs::read_to_string(&file_record.path).ok())?;
        let (start_line, start_col) = offset_to_line_col(&text, record.range.start as usize);
        let (end_line, end_col) = offset_to_line_col(&text, record.range.end as usize);
        Some(javac_analysis_api::Location {
            file: file_record.path.clone(),
            range: javac_analysis_api::Range::new(
                javac_analysis_api::Position::new(start_line, start_col),
                javac_analysis_api::Position::new(end_line, end_col),
            ),
        })
    }


    pub(crate) fn project_document_symbols(
        &mut self,
        file: &std::path::Path,
    ) -> Option<Vec<javac_analysis_api::DocumentSymbol>> {
        if self.project_index.is_none() && !self.config.source_path.is_empty() {
            let overlays: std::collections::HashMap<std::path::PathBuf, String> = self
                .texts
                .iter()
                .filter(|(path, text)| {
                    std::fs::read_to_string(path).ok().as_deref() != Some(text.as_str())
                })
                .map(|(path, text)| (path.clone(), text.clone()))
                .collect();
            self.project_index = Some(build_project_index_with_overlays(
                &self.config,
                128,
                &overlays,
            ));
        }
        let index = self.project_index.as_ref()?;
        let file_id = index.file_id_for_path(file)?;
        let file_record = index.file(file_id)?;
        let text = self
            .texts
            .get(&std::path::PathBuf::from(canonical(&file_record.path)))
            .cloned()
            .or_else(|| std::fs::read_to_string(&file_record.path).ok())?;
        let declarations = index.file_index(file_id)?.declarations.clone();
        let declaration_set: std::collections::HashSet<SymbolId> =
            declarations.iter().copied().collect();
        let mut roots = Vec::new();
        for id in declarations {
            let record = index.symbol(id)?;
            if matches!(
                record.kind,
                IndexSymbolKind::Local | IndexSymbolKind::Parameter | IndexSymbolKind::Other
            ) {
                continue;
            }
            let is_child = record
                .key
                .owner
                .is_some_and(|owner| declaration_set.contains(&owner));
            if !is_child {
                if let Some(node) = build_document_symbol(index, id, &text) {
                    roots.push(node);
                }
            }
        }
        Some(roots)
    }

    pub(crate) fn project_references(
        &self,
        symbol: SymbolId,
    ) -> Vec<javac_analysis_api::Location> {
        let Some(index) = self.project_index.as_ref() else {
            return Vec::new();
        };
        let mut locations = Vec::new();
        for occurrence in index.occurrences_of(symbol) {
            let Some(file) = index.file(occurrence.file) else {
                continue;
            };
            let Some(text) = self
                .texts
                .get(&std::path::PathBuf::from(canonical(&file.path)))
                .cloned()
                .or_else(|| std::fs::read_to_string(&file.path).ok())
            else {
                continue;
            };
            let (start_line, start_col) =
                offset_to_line_col(&text, occurrence.range.start as usize);
            let (end_line, end_col) = offset_to_line_col(&text, occurrence.range.end as usize);
            locations.push(javac_analysis_api::Location {
                file: file.path.clone(),
                range: javac_analysis_api::Range::new(
                    javac_analysis_api::Position::new(start_line, start_col),
                    javac_analysis_api::Position::new(end_line, end_col),
                ),
            });
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

    pub(crate) fn workspace_symbols_impl(
        &mut self,
        query: &str,
    ) -> Vec<javac_analysis_api::WorkspaceSymbol> {
        if self.project_index.is_none() && !self.config.source_path.is_empty() {
            self.project_index = Some(build_project_index(&self.config, 128));
        }
        let Some(index) = self.project_index.as_ref() else {
            return Vec::new();
        };
        let needle = query.trim().to_lowercase();
        let mut hits: Vec<(bool, &SymbolRecord)> = index
            .symbols
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    IndexSymbolKind::Class
                        | IndexSymbolKind::Interface
                        | IndexSymbolKind::Enum
                        | IndexSymbolKind::Annotation
                        | IndexSymbolKind::Record
                        | IndexSymbolKind::Field
                        | IndexSymbolKind::Method
                        | IndexSymbolKind::Constructor
                )
            })
            .filter_map(|record| {
                let name = index.name(record.key.name);
                let starts = name.to_lowercase().starts_with(&needle);
                (needle.is_empty() || name.to_lowercase().contains(&needle)).then_some((starts, record))
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
                let text = self
                    .texts
                    .get(&std::path::PathBuf::from(canonical(&file.path)))
                    .cloned()
                    .or_else(|| std::fs::read_to_string(&file.path).ok())
                    .unwrap_or_default();
                let (start_line, start_col) =
                    offset_to_line_col(&text, record.range.start as usize);
                let (end_line, end_col) = offset_to_line_col(&text, record.range.end as usize);
                let container = record
                    .key
                    .owner
                    .and_then(|owner| index.symbol(owner))
                    .map(|owner| owner.flat_name.clone());
                Some(javac_analysis_api::WorkspaceSymbol {
                    name: index.name(record.key.name).to_string(),
                    kind: api_symbol_kind(record.kind),
                    location: javac_analysis_api::Location {
                        file: file.path.clone(),
                        range: javac_analysis_api::Range::new(
                            javac_analysis_api::Position::new(start_line, start_col),
                            javac_analysis_api::Position::new(end_line, end_col),
                        ),
                    },
                    container_name: container,
                })
            })
            .collect()
    }
}

fn api_symbol_kind(kind: IndexSymbolKind) -> javac_analysis_api::SymbolKind {
    use javac_analysis_api::SymbolKind;
    match kind {
        IndexSymbolKind::Class | IndexSymbolKind::Annotation => SymbolKind::Class,
        IndexSymbolKind::Interface => SymbolKind::Interface,
        IndexSymbolKind::Enum => SymbolKind::Enum,
        IndexSymbolKind::Record => SymbolKind::Record,
        IndexSymbolKind::Field => SymbolKind::Field,
        IndexSymbolKind::Method | IndexSymbolKind::Constructor => SymbolKind::Method,
        IndexSymbolKind::Local => SymbolKind::LocalVariable,
        IndexSymbolKind::Parameter => SymbolKind::Parameter,
        IndexSymbolKind::TypeParameter => SymbolKind::TypeParameter,
        IndexSymbolKind::Other => SymbolKind::Class,
    }
}
fn build_document_symbol(
    index: &ProjectIndex,
    id: SymbolId,
    text: &str,
) -> Option<javac_analysis_api::DocumentSymbol> {
    let record = index.symbol(id)?;
    if matches!(
        record.kind,
        IndexSymbolKind::Local | IndexSymbolKind::Parameter | IndexSymbolKind::Other
    ) {
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
    Some(javac_analysis_api::DocumentSymbol {
        name: index.name(record.key.name).to_string(),
        kind: api_symbol_kind(record.kind),
        range: javac_analysis_api::Range::new(
            javac_analysis_api::Position::new(start_line, start_col),
            javac_analysis_api::Position::new(end_line, end_col),
        ),
        children,
    })
}
