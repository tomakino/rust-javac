//! workspace/symbol query over ProjectIndex.

use std::path::PathBuf;

use javac_analysis_api::{Location, Position, Range, WorkspaceSymbol};

use super::super::positions::offset_to_line_col;
use super::super::{canonical, AnalysisSession};
use super::model::{IndexSymbolKind, SymbolRecord};
use super::symbols::api_symbol_kind;

impl AnalysisSession {
    pub(crate) fn workspace_symbols_impl(&mut self, query: &str) -> Vec<WorkspaceSymbol> {
        self.ensure_project_index();
        let Some(index) = self.project_index.as_ref() else {
            return Vec::new();
        };
        if !index.is_ready() {
            return Vec::new();
        }
        let needle = query.trim().to_lowercase();
        let mut hits: Vec<(bool, &SymbolRecord)> = index
            .symbols
            .iter()
            .filter(|record| is_workspace_symbol(record.kind))
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
                let text = self.file_text(&file.path)?;
                let (start_line, start_col) =
                    offset_to_line_col(&text, record.range.start as usize);
                let (end_line, end_col) = offset_to_line_col(&text, record.range.end as usize);
                let container = record
                    .key
                    .owner
                    .and_then(|owner| index.symbol(owner))
                    .map(|owner| owner.flat_name.clone());
                Some(WorkspaceSymbol {
                    name: index.name(record.key.name).to_string(),
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
