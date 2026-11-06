//! `WorkspaceService` implementation: routes each query to the published
//! index when it can answer, and to a transient [`FileAnalysis`] otherwise.

use std::path::{Path, PathBuf};

use javac_analysis_api::{
    CallHierarchyCall, CallHierarchyItem, CodeAction, CompletionItem, Diagnostic, DocumentHighlight,
    DocumentSymbol, FoldingRange, SelectionRange,
    InlayHint, Location, Position, Range, SignatureHelp, Symbol, SymbolKind, TextDocumentEdit,
    TextEdit, TypeHierarchyItem, WorkspaceConfiguration, WorkspaceService, WorkspaceSymbol,
};

use super::index::{self, query};
use super::state::{empty_index, Workspace};

impl WorkspaceService for Workspace {
    fn diagnostics(&mut self, file: &Path, text: Option<&str>) -> Vec<Diagnostic> {
        // Register the overlay first: other queries rely on it even when the
        // file itself is not diagnosed.
        if let Some(text) = text {
            self.set_overlay(file, text);
        }
        // Extracted JDK/classpath sources are reference material, not project
        // sources; do not diagnose them.
        if self.is_extracted_source(file) {
            return Vec::new();
        }
        self.with_analysis(file, text, |analysis, _| {
            analysis.diagnostics_impl(file)
        })
        .unwrap_or_default()
    }

    fn symbols(&mut self, file: &Path) -> Vec<Symbol> {
        self.with_analysis(file, None, |analysis, _| {
            analysis
                .symbols()
                .into_iter()
                .map(api_symbol)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    }

    fn symbol_at(&mut self, file: &Path, position: Position) -> Option<Symbol> {
        self.with_analysis(file, None, |analysis, _| {
            analysis
                .symbol_at(file, position.line as usize + 1, position.column as usize + 1)
                .map(api_symbol)
        })
        .flatten()
    }

    fn completion(&mut self, file: &Path, position: Position) -> Vec<CompletionItem> {
        self.with_analysis(file, None, |analysis, index| {
            analysis.completion_impl(file, position, index)
        })
        .unwrap_or_default()
    }

    fn inlay_hints(&mut self, file: &Path) -> Vec<InlayHint> {
        self.with_analysis(file, None, |analysis, _| analysis.inlay_hints_impl(file))
            .unwrap_or_default()
    }

    fn document_highlight(&mut self, file: &Path, position: Position) -> Vec<DocumentHighlight> {
        let key = PathBuf::from(crate::analysis::canonical(file));
        self.references(file, position)
            .into_iter()
            .filter(|location| PathBuf::from(crate::analysis::canonical(&location.file)) == key)
            .map(|location| DocumentHighlight {
                range: location.range,
                kind: None,
            })
            .collect()
    }

    fn folding_ranges(&mut self, file: &Path) -> Vec<FoldingRange> {
        self.with_analysis(file, None, |analysis, _| analysis.folding_ranges_impl(file))
            .unwrap_or_default()
    }

    fn selection_ranges(&mut self, file: &Path, positions: &[Position]) -> Vec<SelectionRange> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.selection_ranges_impl(file, positions)
        })
        .unwrap_or_default()
    }

    fn dependent_closure(&mut self, file: &Path) -> Vec<PathBuf> {
        // Use the published snapshot without triggering an index update: the
        // closure only decides which files to recompile, and a stale edge set
        // is corrected by the next index round. Compare normalized path text
        // (`canonicalize` is very slow on Windows). Only build the index when
        // nothing has been published yet.
        if self.index.snapshot().generation == 0 {
            self.ensure_index();
        }
        let snapshot = self.index.snapshot();
        let key = crate::analysis::positions::normalized_path_key(file);
        let Some(id) = snapshot
            .files
            .iter()
            .position(|record| {
                crate::analysis::positions::normalized_path_key(&record.path) == key
            })
            .map(|index| index as u32)
        else {
            return Vec::new();
        };
        snapshot
            .dependent_closure(id)
            .into_iter()
            .filter_map(|file| snapshot.file(file).map(|record| record.path.clone()))
            .collect()
    }

    fn update_analysis_config(&mut self, class_path: Vec<PathBuf>, source_path: Vec<PathBuf>) {
        Workspace::update_analysis_config(self, class_path, source_path);
    }

    fn invalidate_analysis(&mut self, file: &Path) {
        Workspace::invalidate_analysis(self, file);
    }

    fn definition(&mut self, file: &Path, position: Position) -> Option<Location> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.is_dirty() && !snapshot.files.is_empty()
            && let Some(text) = self.text_of(file)
            && let Some(file_id) = snapshot.file_id_for_path(file)
            && let Some(location) = query::definition(
                &snapshot,
                file_id,
                &text,
                position,
                &|id| self.text_of_id(id),
            )
        {
            return Some(location);
        }
        // Unsaved/overlay file not in the index: analyze it once and read the
        // typed symbol position (never a text match).
        self.with_analysis(file, None, |analysis, _| {
            analysis.definition_impl(file, position)
        })
        .flatten()
    }

    fn type_definition(&mut self, file: &Path, position: Position) -> Option<Location> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.type_definition_impl(file, position)
        })
        .flatten()
    }

    fn implementation(&mut self, file: &Path, position: Position) -> Vec<Location> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.implementation_impl(file, position)
        })
        .unwrap_or_default()
    }

    fn super_methods(&mut self, file: &Path, position: Position) -> Vec<Location> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.super_methods_impl(file, position)
        })
        .unwrap_or_default()
    }

    fn signature_help(&mut self, file: &Path, position: Position) -> Option<SignatureHelp> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.signature_help_impl(file, position)
        })
        .flatten()
    }

    fn format(&mut self, file: &Path, text: &str) -> Option<String> {
        self.with_analysis(file, Some(text), |analysis, _| {
            analysis.format_impl(file, text)
        })
        .flatten()
    }

    fn code_actions(&mut self, file: &Path, position: Position) -> Vec<CodeAction> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.code_actions_impl(file, position)
        })
        .unwrap_or_default()
    }

    fn prepare_type_hierarchy(
        &mut self,
        file: &Path,
        position: Position,
    ) -> Option<TypeHierarchyItem> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.prepare_type_hierarchy_impl(file, position)
        })
        .flatten()
    }

    fn supertypes(&mut self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
        let file = item.file.clone();
        self.with_analysis(&file, None, |analysis, _| analysis.supertypes_impl(item))
            .unwrap_or_default()
    }

    fn subtypes(&mut self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
        let file = item.file.clone();
        self.with_analysis(&file, None, |analysis, _| analysis.subtypes_impl(item))
            .unwrap_or_default()
    }

    fn prepare_call_hierarchy(
        &mut self,
        file: &Path,
        position: Position,
    ) -> Option<CallHierarchyItem> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.prepare_call_hierarchy_impl(file, position)
        })
        .flatten()
    }

    fn incoming_calls(&mut self, item: &CallHierarchyItem) -> Vec<CallHierarchyCall> {
        let file = item.file.clone();
        self.with_analysis(&file, None, |analysis, _| analysis.incoming_calls_impl(item))
            .unwrap_or_default()
    }

    fn outgoing_calls(&mut self, item: &CallHierarchyItem) -> Vec<CallHierarchyCall> {
        let file = item.file.clone();
        self.with_analysis(&file, None, |analysis, _| analysis.outgoing_calls_impl(item))
            .unwrap_or_default()
    }

    fn prepare_rename(&mut self, file: &Path, position: Position) -> Option<Range> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.is_dirty() && !snapshot.files.is_empty()
            && let Some(text) = self.text_of(file)
            && let Some(file_id) = snapshot.file_id_for_path(file)
            && let Some(range) = query::prepare_rename_range(&snapshot, file_id, &text, position)
        {
            return Some(range);
        }
        self.with_analysis(file, None, |analysis, _| {
            analysis.prepare_rename_impl(file, position)
        })
        .flatten()
    }

    fn references(&mut self, file: &Path, position: Position) -> Vec<Location> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.is_dirty() && !snapshot.files.is_empty()
            && let Some(text) = self.text_of(file)
            && let Some(file_id) = snapshot.file_id_for_path(file)
        {
            let locations = query::references(
                &snapshot,
                file_id,
                &text,
                position,
                &|id| self.text_of_id(id),
            );
            if !locations.is_empty() {
                return locations;
            }
        }
        self.with_analysis(file, None, |analysis, _| {
            analysis.references_impl(file, position)
        })
        .unwrap_or_default()
    }

    fn hover(&mut self, file: &Path, position: Position) -> Option<String> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.is_dirty() && !snapshot.files.is_empty()
            && let Some(text) = self.text_of(file)
            && let Some(file_id) = snapshot.file_id_for_path(file)
            && let Some(symbol) = query::symbol_at(&snapshot, file_id, &text, position)
            && let Some(record) = snapshot.symbol(symbol)
        {
            let mut detail = record.detail.clone();
            if let Some(return_type) = &record.return_type {
                detail.push_str(": ");
                detail.push_str(&match return_type {
                    index::TypeRef::Project(id) => snapshot
                        .symbol(*id)
                        .map(|record| record.flat_name.clone())
                        .unwrap_or_default(),
                    index::TypeRef::External(name) => name.clone(),
                });
            }
            if let Some(javadoc) = &record.javadoc {
                detail.push_str("\n\n");
                detail.push_str(javadoc);
            }
            if matches!(record.kind, index::IndexSymbolKind::Method)
                && let Some(supers) = self.with_analysis(file, None, |analysis, _| {
                    analysis.super_methods_with_names_impl(file, position)
                })
                && !supers.is_empty()
            {
                detail.push_str("\n\n---\n\n**Overrides:**");
                for (_, name) in supers.iter().take(5) {
                    detail.push_str(&format!("\n- `{name}`"));
                }
                if supers.len() > 5 {
                    detail.push_str(&format!("\n- … (+{} more)", supers.len() - 5));
                }
            }
            return Some(detail);
        }
        self.with_analysis(file, None, |analysis, _| analysis.hover_impl(file, position))
            .flatten()
    }

    fn rename_workspace(
        &mut self,
        file: &Path,
        position: Position,
        new_name: &str,
    ) -> Vec<TextDocumentEdit> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.has_unsaved_edits() && !snapshot.files.is_empty()
            && let Some(text) = self.text_of(file)
            && let Some(file_id) = snapshot.file_id_for_path(file)
            && let Some(symbol) = query::symbol_at(&snapshot, file_id, &text, position)
        {
            let mut edits = Vec::new();
            for occurrence in snapshot.occurrences_of(symbol) {
                let Some(record) = snapshot.file(occurrence.file) else {
                    continue;
                };
                let Some(text) = self.text_of_id(occurrence.file) else {
                    continue;
                };
                let location = query::location_for_range(
                    &record.path,
                    &text,
                    occurrence.range.start,
                    occurrence.range.end,
                );
                edits.push(TextDocumentEdit {
                    file: location.file,
                    edit: TextEdit {
                        range: location.range,
                        new_text: new_name.to_string(),
                    },
                });
            }
            if !edits.is_empty() {
                return edits;
            }
        }
        self.with_analysis(file, None, |analysis, _| {
            analysis.rename_workspace_impl(file, position, new_name)
        })
        .unwrap_or_default()
    }

    fn workspace_symbols(&mut self, query_text: &str) -> Vec<WorkspaceSymbol> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !snapshot.is_ready() {
            return Vec::new();
        }
        query::workspace_symbols(&snapshot, query_text, &|id| self.text_of_id(id))
    }

    fn document_symbols(&mut self, file: &Path) -> Vec<DocumentSymbol> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.is_dirty() && !snapshot.files.is_empty()
            && let Some(file_id) = snapshot.file_id_for_path(file)
            && let Some(text) = self.text_of_id(file_id)
        {
            let symbols = query::document_symbols(&snapshot, file_id, &text);
            if !symbols.is_empty() {
                return symbols;
            }
        }
        self.with_analysis(file, None, |analysis, _| {
            analysis.document_symbols_impl(file)
        })
        .unwrap_or_default()
    }

    fn invalidate(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.overlays.remove(path);
        }
        self.rebuild();
    }

    fn index_generation(&self) -> u64 {
        self.index.snapshot().generation
    }

    fn is_index_building(&self) -> bool {
        self.is_building()
    }

    fn configuration(&self) -> WorkspaceConfiguration {
        WorkspaceConfiguration {
            class_path: self.config.class_path.clone(),
            source_path: self.config.source_path.clone(),
            source_level: self.config.source_level.clone(),
            output_dir: self.config.output_dir.clone(),
        }
    }

    fn reconfigure(&mut self, configuration: WorkspaceConfiguration) {
        let class_path_changed = configuration.class_path != self.config.class_path;
        let source_path_changed = configuration.source_path != self.config.source_path;
        let level_changed = configuration.source_level != self.config.source_level;
        self.config.class_path = configuration.class_path;
        self.config.source_path = configuration.source_path;
        self.config.source_level = configuration.source_level;
        self.config.output_dir = configuration.output_dir;
        self.model = super::model::WorkspaceModel::from_config(&self.config);
        self.analyses.entries.clear();
        if class_path_changed || source_path_changed || level_changed {
            // A stale snapshot is dangerous across a reconfiguration, so drop
            // it and rebuild before answering the next index query.
            self.discard_build();
            self.index.publish(empty_index(index::IndexStatus::Building));
            self.rebuild();
        }
    }
}

fn api_symbol(symbol: crate::analysis::symbols::SymbolInfo) -> Symbol {
    Symbol {
        name: symbol.name,
        kind: symbol_kind(symbol.kind),
        range: None,
        qualified_name: Some(symbol.flat_name),
    }
}

fn symbol_kind(kind: &str) -> SymbolKind {
    match kind {
        "class" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "enum" => SymbolKind::Enum,
        "record" => SymbolKind::Record,
        "method" => SymbolKind::Method,
        "field" => SymbolKind::Field,
        "local" => SymbolKind::LocalVariable,
        "parameter" => SymbolKind::Parameter,
        _ => SymbolKind::Class,
    }
}
