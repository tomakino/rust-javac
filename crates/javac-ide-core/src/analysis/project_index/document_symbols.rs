//! Outline symbols derived from ProjectIndex.

use std::path::Path;

use javac_analysis_api::{DocumentSymbol, Position, Range};

use super::super::positions::offset_to_line_col;
use super::super::AnalysisSession;
use super::model::{IndexSymbolKind, ProjectIndex, SymbolId};
use super::symbols::api_symbol_kind;

impl AnalysisSession {
    pub(crate) fn project_document_symbols(&mut self, file: &Path) -> Option<Vec<DocumentSymbol>> {
        self.ensure_project_index();
        let index = self.project_index.as_ref()?;
        if !index.is_ready() {
            return None;
        }
        let file_id = index.file_id_for_path(file)?;
        let file_record = index.file(file_id)?;
        let text = self.file_text(&file_record.path)?;
        let declarations = index.file_index(file_id)?.declarations.clone();
        let declaration_set: std::collections::HashSet<SymbolId> =
            declarations.iter().copied().collect();
        let mut roots = Vec::new();
        for id in declarations {
            let record = index.symbol(id)?;
            if is_non_outline(record.kind) {
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
    Some(DocumentSymbol {
        name: index.name(record.key.name).to_string(),
        kind: api_symbol_kind(record.kind),
        range: Range::new(
            Position::new(start_line, start_col),
            Position::new(end_line, end_col),
        ),
        children,
    })
}

fn is_non_outline(kind: IndexSymbolKind) -> bool {
    matches!(
        kind,
        IndexSymbolKind::Local | IndexSymbolKind::Parameter | IndexSymbolKind::Other
    )
}
