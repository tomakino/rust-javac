//! Index lifecycle and project-wide location queries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use javac_analysis_api::{Location, Position, Range};

use super::super::positions::{line_offset, offset_to_line_col};
use super::super::{canonical, AnalysisSession};
use super::builder::{build_project_index_with_overlays, collect_java_files, hash_text};
use super::model::{ProjectIndex, SymbolId};

impl AnalysisSession {
    /// Ensure the current index snapshot matches the source files and overlays.
    pub(crate) fn ensure_project_index(&mut self) {
        if self.config.source_path.is_empty() {
            return;
        }
        let overlays = self.overlay_texts();
        let current = self
            .project_index
            .as_ref()
            .is_some_and(|index| index_matches(index, &self.config.source_path, &overlays));
        if !current {
            self.project_index = Some(build_project_index_with_overlays(
                &self.config,
                128,
                &overlays,
            ));
        }
    }

    fn overlay_texts(&self) -> HashMap<PathBuf, String> {
        self.texts
            .iter()
            .filter(|(path, text)| {
                std::fs::read_to_string(path).ok().as_deref() != Some(text.as_str())
            })
            .map(|(path, text)| (path.clone(), text.clone()))
            .collect()
    }

    /// Symbol at a position according to the project index.
    pub(crate) fn project_symbol_at(
        &mut self,
        file: &Path,
        position: Position,
    ) -> Option<SymbolId> {
        self.ensure_project_index();
        let index = self.project_index.as_ref()?;
        if !index.is_ready() {
            return None;
        }
        let path = PathBuf::from(canonical(file));
        let file_id = index.file_id_for_path(file)?;
        let text = self
            .texts
            .get(&path)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let offset = line_offset(&text, line, column) as u32;
        index
            .occurrence_at(file_id, offset)
            .map(|occurrence| occurrence.symbol)
    }

    pub(crate) fn project_definition(
        &mut self,
        file: &Path,
        position: Position,
    ) -> Option<Location> {
        let symbol = self.project_symbol_at(file, position)?;
        let index = self.project_index.as_ref()?;
        let record = index.symbol(symbol)?;
        let file_record = index.file(record.file)?;
        let text = self.file_text(&file_record.path)?;
        Some(location_for_range(
            &file_record.path,
            &text,
            record.range.start,
            record.range.end,
        ))
    }

    pub(crate) fn project_references(&self, symbol: SymbolId) -> Vec<Location> {
        let Some(index) = self.project_index.as_ref() else {
            return Vec::new();
        };
        if !index.is_ready() {
            return Vec::new();
        }
        let mut locations = Vec::new();
        for occurrence in index.occurrences_of(symbol) {
            let Some(file) = index.file(occurrence.file) else {
                continue;
            };
            let Some(text) = self.file_text(&file.path) else {
                continue;
            };
            locations.push(location_for_range(
                &file.path,
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

    pub(crate) fn file_text(&self, path: &Path) -> Option<String> {
        self.texts
            .get(&PathBuf::from(canonical(path)))
            .cloned()
            .or_else(|| std::fs::read_to_string(path).ok())
    }
}

fn index_matches(
    index: &ProjectIndex,
    source_roots: &[PathBuf],
    overlays: &HashMap<PathBuf, String>,
) -> bool {
    let files = collect_java_files(source_roots);
    if files.len() != index.files.len() {
        return false;
    }
    for (path, record) in files.iter().zip(index.files.iter()) {
        if path != &record.path {
            return false;
        }
        let Some(text) = overlays
            .get(path)
            .cloned()
            .or_else(|| std::fs::read_to_string(path).ok())
        else {
            return false;
        };
        if hash_text(&text) != record.hash {
            return false;
        }
    }
    true
}

fn location_for_range(path: &Path, text: &str, start: u32, end: u32) -> Location {
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
