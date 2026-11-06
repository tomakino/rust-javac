//! `WorkspaceService` implementation: routes each query to the published
//! index when it can answer, and to a transient [`FileAnalysis`] otherwise.

use std::path::{Path, PathBuf};

use javac_analysis_api::{
    CallHierarchyCall, CallHierarchyItem, CodeAction, CodeLens, CompletionItem, Diagnostic,
    DocumentHighlight, DocumentLink, DocumentSymbol, FoldingRange, SelectionRange,
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

    fn organize_imports(&mut self, file: &Path) -> Vec<TextEdit> {
        self.with_analysis(file, None, |analysis, _| analysis.organize_imports_impl(file))
            .unwrap_or_default()
    }

    fn semantic_tokens(&mut self, file: &Path) -> Vec<u32> {
        self.with_analysis(file, None, |analysis, _| analysis.semantic_tokens_impl(file))
            .unwrap_or_default()
    }

    fn code_lenses(&mut self, file: &Path) -> Vec<CodeLens> {
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
        let text = std::fs::read_to_string(file).unwrap_or_default();
        let uri = file_uri(file);
        let mut lenses = Vec::new();
        for symbol in &file_index.declarations {
            let Some(occurrences) = snapshot.occurrences.get(symbol) else {
                continue;
            };
            let Some(declaration) = occurrences.iter().find(|occurrence| {
                occurrence.file == file_id
                    && occurrence.role == crate::workspace::index::model::OccurrenceRole::Declaration
            }) else {
                continue;
            };
            let references = occurrences
                .iter()
                .filter(|occurrence| {
                    occurrence.role != crate::workspace::index::model::OccurrenceRole::Declaration
                })
                .count();
            let implementations = snapshot
                .by_supertype
                .get(symbol)
                .map(|subtypes| subtypes.len())
                .unwrap_or(0);
            let start =
                crate::analysis::positions::byte_to_utf16(&text, declaration.range.start as usize);
            let end =
                crate::analysis::positions::byte_to_utf16(&text, declaration.range.end as usize);
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
        let text = std::fs::read_to_string(file).unwrap_or_default();
        let mut links = Vec::new();
        for (line_index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("import ") else {
                continue;
            };
            let path = rest.strip_prefix("static ").unwrap_or(rest);
            let path = path.trim_end_matches(';').trim();
            if path.is_empty() {
                continue;
            }
            let class = if rest.starts_with("static ") {
                let base = path.trim_end_matches(".*");
                base.rsplit_once('.').map(|(class, _)| class).unwrap_or(base)
            } else {
                path.trim_end_matches(".*")
            };
            // Project sources: workspace model roots first (independent of the
            // index build state), then the published index.
            let relative = format!("{}.java", class.replace('.', "/"));
            let project = self
                .model
                .source_roots()
                .into_iter()
                .map(|root| root.join(&relative))
                .find(|candidate| candidate.is_file());
            let indexed = self.index.snapshot().files.iter().find_map(|record| {
                let path = record.path.to_string_lossy().replace('\\', "/");
                if path.ends_with(&relative) {
                    Some(record.path.clone())
                } else {
                    None
                }
            });
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
            let end: u32 = line.chars().map(|character| character.len_utf16() as u32).sum();
            links.push(DocumentLink {
                range: Range::new(
                    Position::new(line_index as u32, 0),
                    Position::new(line_index as u32, end),
                ),
                target,
            });
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
        let Some(sources) = sources_jar(entry) else {
            continue;
        };
        let Ok(archive) = javac_zip::ZipArchive::open(&sources) else {
            continue;
        };
        let Some(bytes) = find_entry(&archive, &relative) else {
            continue;
        };
        let target = extract_source(&bytes, &sources, &relative, &config.output_dir)?;
        return Some(file_uri(&target));
    }
    let jdk = jdk_src_zip()?;
    let archive = javac_zip::ZipArchive::open(&jdk).ok()?;
    let bytes = find_entry(&archive, &relative)?;
    let target = extract_source(&bytes, &jdk, &relative, &config.output_dir)?;
    Some(file_uri(&target))
}

/// The `-sources.jar` next to a class-path jar.
fn sources_jar(entry: &Path) -> Option<PathBuf> {
    let name = entry.to_string_lossy();
    let stem = name.strip_suffix(".jar")?;
    let sources = PathBuf::from(format!("{stem}-sources.jar"));
    sources.is_file().then_some(sources)
}

/// Find a zip entry by relative path; JDK `src.zip` entries carry a module
/// prefix (`java.base/java/util/List.java`).
fn find_entry(archive: &javac_zip::ZipArchive, relative: &str) -> Option<Vec<u8>> {
    if let Some(bytes) = archive.get_entry(relative) {
        return Some(bytes);
    }
    let suffix = format!("/{relative}");
    let name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name.ends_with(&suffix))?;
    archive.get_entry(&name)
}

fn extract_source(
    bytes: &[u8],
    archive_path: &Path,
    relative: &str,
    output_dir: &Path,
) -> Option<PathBuf> {
    let key = format!("{}:{}", archive_path.display(), relative);
    let hash = crate::workspace::model::hash_text(&key);
    let target = output_dir
        .join("doclink")
        .join(format!("{hash:016x}"))
        .join(relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    if !target.exists() {
        std::fs::write(&target, bytes).ok()?;
    }
    Some(target)
}

fn file_uri(path: &Path) -> String {
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
}

/// The JDK sources archive: the platform directory first, then `JAVA_HOME`.
fn jdk_src_zip() -> Option<PathBuf> {
    if let Some(platform) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        let candidate = PathBuf::from(platform).join("src.zip");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let home = std::env::var_os("JAVA_HOME")?;
    let candidate = PathBuf::from(home).join("lib").join("src.zip");
    candidate.is_file().then_some(candidate)
}

/// `http(s)://` links. Scanning text is intentional and unambiguous here
/// (a URL has no compiler representation, unlike the semantic decisions).
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
