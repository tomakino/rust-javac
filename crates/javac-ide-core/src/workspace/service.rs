//! `WorkspaceService` implementation: routes each query to the published
//! index when it can answer, and to a transient [`FileAnalysis`] otherwise.

use std::path::{Path, PathBuf};

use javac_analysis_api::{
    CallHierarchyCall, CallHierarchyItem, CodeAction, CodeLens, CompletionItem, Diagnostic,
    DocumentHighlight, DocumentLink, DocumentSymbol, FoldingRange, SelectionRange,
    InlayHint, Location, Position, Range, SignatureHelp, Symbol, SymbolKind, TextDocumentEdit,
    TextEdit, TypeHierarchyItem, WorkspaceConfiguration, WorkspaceService, WorkspaceSymbol,
};

use crate::analysis::uri::file_uri;

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
        let show_same_name = self.inlay_hints_same_name;
        self.with_analysis(file, None, |analysis, index| {
            analysis.inlay_hints_impl(file, index, show_same_name)
        })
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

    fn organize_imports(&mut self, file: &Path) -> Vec<TextEdit> {
        self.with_analysis(file, None, |analysis, _| analysis.organize_imports_impl(file))
            .unwrap_or_default()
    }

    fn semantic_tokens(&mut self, file: &Path) -> Vec<u32> {
        self.with_analysis(file, None, |analysis, _| analysis.semantic_tokens_impl(file))
            .unwrap_or_default()
    }

    fn code_lenses(&mut self, file: &Path) -> Vec<CodeLens> {
        // CodeLens bypasses `with_analysis`, so without this a fresh workspace
        // would wait for some unrelated query to start the index build.
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !matches!(
            snapshot.status,
            crate::workspace::index::model::IndexStatus::Ready
        ) {
            return Vec::new();
        }
        let Some(file_id) = snapshot.file_id_for_path(file) else {
            return Vec::new();
        };
        let Some(file_index) = snapshot.by_file.get(&file_id) else {
            return Vec::new();
        };
        let text = self.text_of(file).unwrap_or_default();
        let uri = file_uri(file);
        let import_ranges = self
            .with_analysis(file, None, |analysis, _| analysis.import_ranges_impl(file))
            .unwrap_or_default();
        let mut lenses = Vec::new();
        // One lens group per declaration range: an index can hold extra ids at
        // the same position (a synthetic constructor shares the class name
        // range), and a duplicated incremental record must not add a second
        // group. Outline-only symbols do not get lenses at all.
        let mut seen_ranges: std::collections::HashSet<(u32, u32)> =
            std::collections::HashSet::new();
        for symbol in &file_index.declarations {
            let Some(record) = snapshot.symbol(*symbol) else {
                continue;
            };
            let outline = matches!(
                record.kind,
                crate::workspace::index::key::IndexSymbolKind::Class
                    | crate::workspace::index::key::IndexSymbolKind::Interface
                    | crate::workspace::index::key::IndexSymbolKind::Enum
                    | crate::workspace::index::key::IndexSymbolKind::Annotation
                    | crate::workspace::index::key::IndexSymbolKind::Record
                    | crate::workspace::index::key::IndexSymbolKind::Field
                    | crate::workspace::index::key::IndexSymbolKind::Method
            );
            if !outline {
                continue;
            }
            let Some(occurrences) = snapshot.occurrences.get(symbol) else {
                continue;
            };
            let Some(declaration) = occurrences.iter().find(|occurrence| {
                occurrence.file == file_id
                    && occurrence.role == crate::workspace::index::model::OccurrenceRole::Declaration
            }) else {
                continue;
            };
            // Reference count comes from the shared index query (query::references),
            // not from a second traversal here; the declaration itself is one of
            // the returned locations.
            let (decl_line, decl_column) = crate::analysis::positions::offset_to_line_col(
                &text,
                declaration.range.start as usize,
            );
            if !seen_ranges.insert((declaration.range.start, declaration.range.end)) {
                continue;
            }
            // Imports are not declarations of this file (AST import spans).
            if import_ranges
                .iter()
                .any(|(start, end)| {
                    declaration.range.start >= *start && declaration.range.start <= *end
                })
            {
                continue;
            }
            let locations = crate::workspace::index::query::references(
                &snapshot,
                file_id,
                &text,
                Position::new(decl_line, decl_column),
                &|file| self.text_of_id(file),
            );
            let references = if locations.is_empty() {
                0
            } else {
                locations.len() - 1
            };
            let implementations = snapshot
                .by_supertype
                .get(symbol)
                .map(|subtypes| subtypes.len())
                .unwrap_or(0);
            // Occurrence ranges are UTF-16 offsets already.
            let start = declaration.range.start as usize;
            let end = declaration.range.end as usize;
            let (line, column) = crate::analysis::positions::offset_to_line_col(&text, start);
            let range = Range::new(
                Position::new(line, column),
                Position::new(line, column + (end.saturating_sub(start)) as u32),
            );
            lenses.push(CodeLens {
                range,
                title: format!("{references} references"),
                command: "ravac.showReferences".to_string(),
                uri: uri.clone(),
                line,
                character: column,
            });
            if implementations > 0 {
                lenses.push(CodeLens {
                    range,
                    title: format!("{implementations} implementations"),
                    command: "ravac.showReferences".to_string(),
                    uri: uri.clone(),
                    line,
                    character: column,
                });
            }
        }
        lenses
    }

    fn on_type_formatting(
        &mut self,
        file: &Path,
        position: Position,
        character: char,
    ) -> Vec<TextEdit> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.on_type_formatting_impl(file, position, character)
        })
        .unwrap_or_default()
    }

    fn document_links(&mut self, file: &Path) -> Vec<DocumentLink> {
        let text = self.text_of(file).unwrap_or_default();
        let import_links = self
            .with_analysis(file, None, |analysis, _| analysis.import_links_impl(file))
            .unwrap_or_default();
        let mut links = Vec::new();
        for (range, class) in import_links {
            let class = class.as_str();
            // Project sources: workspace model roots first (independent of the
            // index build state), then the published index (symbol lookup, not
            // path suffix matching).
            let relative = format!("{}.java", class.replace('.', "/"));
            let project = self
                .model
                .source_roots()
                .into_iter()
                .map(|root| root.join(&relative))
                .find(|candidate| candidate.is_file());
            let indexed = indexed_source(&self.index.snapshot(), class);
            let target = if let Some(path) = project {
                file_uri(&path)
            } else if let Some(path) = indexed {
                file_uri(&path)
            } else {
                match resolve_import_source(&self.config, class) {
                    Some(target) => target,
                    None => continue,
                }
            };
            links.push(DocumentLink { range, target });
        }
        links.extend(url_links(&text));
        links
    }

    fn update_analysis_config(&mut self, class_path: Vec<PathBuf>, source_path: Vec<PathBuf>) {
        Workspace::update_analysis_config(self, class_path, source_path);
    }

    fn invalidate_analysis(&mut self, file: &Path) {
        Workspace::invalidate_analysis(self, file);
    }

    fn definition(&mut self, file: &Path, position: Position) -> Option<Location> {
        // Clicking anywhere on an import node resolves the imported type: use
        // the import AST span to map the cursor onto its simple name before the
        // normal definition path runs.
        let position = self
            .with_analysis(file, None, |analysis, _| {
                analysis.import_target_position_impl(file, position)
            })
            .flatten()
            .unwrap_or(position);
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.has_unsaved_edits() && !snapshot.files.is_empty()
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
            // A (0,0) location means the record had no source position (type
            // variables etc.); fall through to the AST analysis.
            if location.range.start.line != 0 || location.range.start.column != 0 {
                return Some(location);
            }
        }
        // Unsaved/overlay file not in the index: analyze it once and read the
        // typed symbol position (never a text match).
        if let Some(location) = self
            .with_analysis(file, None, |analysis, _| {
                analysis.definition_impl(file, position)
            })
            .flatten()
        {
            return Some(location);
        }
        // The first query after a project opens can land while the background
        // index build is still running. The compiler then knows the symbol (it
        // is on the source path) but has no declaration tree for it, and the
        // index has no record yet, so the query would report "no definition".
        // Wait for the index once and answer from it instead.
        if !matches!(
            self.index.snapshot().status,
            crate::workspace::index::model::IndexStatus::Ready
        ) {
            self.wait_for_index();
            let snapshot = self.index.snapshot();
            if !self.has_unsaved_edits()
                && let Some(text) = self.text_of(file)
                && let Some(file_id) = snapshot.file_id_for_path(file)
                && let Some(location) =
                    query::definition(&snapshot, file_id, &text, position, &|id| {
                        self.text_of_id(id)
                    })
            {
                return Some(location);
            }
        }
        None
    }

    fn type_definition(&mut self, file: &Path, position: Position) -> Option<Location> {
        self.with_analysis(file, None, |analysis, _| {
            analysis.type_definition_impl(file, position)
        })
        .flatten()
    }

    fn implementation(&mut self, file: &Path, position: Position) -> Vec<Location> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        if !self.has_unsaved_edits() && !snapshot.files.is_empty()
            && let Some(text) = self.text_of(file)
            && let Some(file_id) = snapshot.file_id_for_path(file)
        {
            let locations = query::implementations(
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
        if !self.has_unsaved_edits() && !snapshot.files.is_empty()
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
        if !self.has_unsaved_edits() && !snapshot.files.is_empty()
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
        // One hover implementation: the transient analysis renders the
        // signature and resolves documentation/overrides. The index is passed
        // in for cross-file parent docs and declaration links.
        self.with_analysis(file, None, |analysis, index| {
            analysis.hover_impl(file, position, index)
        })
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
            for override_symbol in query::override_declarations(&snapshot, symbol) {
                let Some(record) = snapshot.symbol(override_symbol) else {
                    continue;
                };
                let Some(file_record) = snapshot.file(record.file) else {
                    continue;
                };
                let Some(text) = self.text_of_id(record.file) else {
                    continue;
                };
                let location = query::location_for_range(
                    &file_record.path,
                    &text,
                    record.range.start,
                    record.range.end,
                );
                if edits
                    .iter()
                    .any(|edit| edit.file == location.file && edit.edit.range == location.range)
                {
                    continue;
                }
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
        let mut symbols = Vec::new();
        if !self.is_dirty()
            && !snapshot.files.is_empty()
            && let Some(file_id) = snapshot.file_id_for_path(file)
            && let Some(text) = self.text_of_id(file_id)
        {
            symbols = query::document_symbols(&snapshot, file_id, &text);
        }
        if symbols.is_empty() {
            symbols = self
                .with_analysis(file, None, |analysis, _| analysis.document_symbols_impl(file))
                .unwrap_or_default();
        }
        // Defense in depth: never hand a nameless symbol to the client (VS Code
        // rejects `name must not be falsy`).
        retain_named_symbols(&mut symbols);
        symbols
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

    fn index_files(&self) -> usize {
        self.index.snapshot().files.len()
    }

    fn index_symbols(&self) -> usize {
        self.index.snapshot().symbols.len()
    }

    fn poll_index(&mut self) {
        Workspace::poll_index(self);
    }

    fn remove_overlay(&mut self, file: &Path) {
        Workspace::remove_overlay(self, file);
    }

    fn set_inlay_hints_same_name(&mut self, enabled: bool) {
        self.inlay_hints_same_name = enabled;
    }

    fn inlay_hints_same_name(&self) -> bool {
        self.inlay_hints_same_name
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

/// Resolve an import target to an openable source file (project root,
/// class-path sources jar, then the JDK `src.zip`).
fn resolve_import_source(
    config: &crate::workspace::analysis::AnalysisConfig,
    class: &str,
) -> Option<String> {
    let relative = format!("{}.java", class.replace('.', "/"));
    for root in &config.source_path {
        let candidate = root.join(&relative);
        if candidate.is_file() {
            return Some(file_uri(&candidate));
        }
    }
    for entry in &config.class_path {
        let Some(sources) = crate::analysis::source_files::sources_jar(entry) else {
            continue;
        };
        let Ok(archive) = javac_zip::ZipArchive::open(&sources) else {
            continue;
        };
        let Some(entry_name) = crate::analysis::source_files::zip_entry(&archive, &relative) else {
            continue;
        };
        let Some(bytes) = archive.get_entry(&entry_name) else {
            continue;
        };
        let stem = sources.file_stem()?.to_string_lossy();
        let target = crate::analysis::source_files::extract(
            &bytes,
            &format!("external-src/{stem}"),
            &relative,
            &config.output_dir,
        )?;
        return Some(file_uri(&target));
    }
    let jdk = crate::analysis::source_files::jdk_src_zip()?;
    let archive = javac_zip::ZipArchive::open(&jdk).ok()?;
    let entry_name = crate::analysis::source_files::zip_entry(&archive, &relative)?;
    let bytes = archive.get_entry(&entry_name)?;
    let target =
        crate::analysis::source_files::extract(&bytes, "jdk-src", &entry_name, &config.output_dir)?;
    Some(file_uri(&target))
}

/// Drop nameless outline entries (recursively); VS Code rejects them.
fn retain_named_symbols(symbols: &mut Vec<javac_analysis_api::DocumentSymbol>) {
    symbols.retain(|symbol| !symbol.name.trim().is_empty());
    for symbol in symbols {
        retain_named_symbols(&mut symbol.children);
    }
}
fn url_links(text: &str) -> Vec<DocumentLink> {
    let mut links = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let mut search = 0;
        while search < line.len() {
            let Some(offset) = line[search..].find("http") else {
                break;
            };
            let start = search + offset;
            let rest = &line[start..];
            if !(rest.starts_with("http://") || rest.starts_with("https://")) {
                search = start + 4;
                continue;
            }
            let end = rest
                .find(|character: char| {
                    character.is_whitespace()
                        || matches!(character, '"' | '\'' | '>' | '<' | ')' | ']' | '}')
                })
                .unwrap_or(rest.len());
            let url = &rest[..end];
            let start_column: u32 = line[..start]
                .chars()
                .map(|character| character.len_utf16() as u32)
                .sum();
            let end_column: u32 = line[..start + end]
                .chars()
                .map(|character| character.len_utf16() as u32)
                .sum();
            links.push(DocumentLink {
                range: Range::new(
                    Position::new(line_index as u32, start_column),
                    Position::new(line_index as u32, end_column),
                ),
                target: url.to_string(),
            });
            search = start + end.max(1);
        }
    }
    links
}

/// Source file of a project class, resolved through the published index
/// symbols (not by path suffix).
fn indexed_source(
    snapshot: &crate::workspace::index::model::ProjectIndex,
    fqcn: &str,
) -> Option<PathBuf> {
    let (_package, simple) = fqcn.rsplit_once('.')?;
    let name_id = snapshot.strings.find(simple)?;
    for symbol in snapshot.by_name.get(&name_id)? {
        let record = snapshot.symbols.get(*symbol as usize)?;
        if !record.kind.is_type() {
            continue;
        }
        if flat_name(snapshot, *symbol).as_deref() == Some(fqcn) {
            return snapshot
                .files
                .get(record.file as usize)
                .map(|file| file.path.clone());
        }
    }
    None
}

fn flat_name(
    snapshot: &crate::workspace::index::model::ProjectIndex,
    symbol: u32,
) -> Option<String> {
    let mut parts = Vec::new();
    let mut current = symbol;
    loop {
        let record = snapshot.symbols.get(current as usize)?;
        parts.push(snapshot.strings.get(record.key.name).to_string());
        match record.key.owner {
            Some(owner) => current = owner,
            None => break,
        }
    }
    parts.reverse();
    Some(parts.join("."))
}
