
//! Compiler-agnostic analysis API for editors and language servers.
//!
//! This crate contains only plain data types and one trait. It has no
//! dependency on the compiler crates, so an LSP server can depend on this
//! crate alone and switch backends without changes.
//!
//! Positions are zero-based (line and UTF-16 column, LSP convention).

use std::path::{Path, PathBuf};

/// A zero-based line/column position (LSP convention, UTF-16 columns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// A half-open range between two positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn point(position: Position) -> Self {
        Self { start: position, end: position }
    }
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}

/// One diagnostic produced by the analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Severity,
    pub message: String,
    pub code: Option<String>,
}

/// Symbol kind, mirroring the LSP symbol kinds an editor needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Class,
    Interface,
    Enum,
    Record,
    Method,
    Constructor,
    Field,
    LocalVariable,
    Parameter,
    Package,
    TypeParameter,
    Keyword,
}

/// A symbol declared or referenced in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Declaration range in the analyzed file, when known.
    pub range: Option<Range>,
    /// Fully qualified name, when known.
    pub qualified_name: Option<String>,
}

/// A symbol returned by workspace/symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
}

/// A symbol shown in the outline view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    /// Javadoc of the declaration, when present (rendered Markdown).
    pub detail: Option<String>,
    pub children: Vec<DocumentSymbol>,
}

/// How a completion item's insert text must be interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertTextFormat {
    /// Plain text insertion.
    #[default]
    PlainText,
    /// Snippet syntax: `$0` is the final cursor position.
    Snippet,
}

/// A completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: SymbolKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    /// Extra label detail rendered next to the label (e.g. "(int i)").
    pub label_detail: Option<String>,
    /// Sort key sent to clients that reorder completions themselves.
    pub sort_text: Option<String>,
    /// Text actually inserted; `None` means the label is inserted.
    pub insert_text: Option<String>,
    /// How `insert_text` is interpreted.
    pub insert_text_format: InsertTextFormat,
    /// When set, the editor replaces this many UTF-16 columns before the
    /// cursor (the typed prefix) with `insert_text`.
    pub replace_prefix_len: Option<u32>,
    /// Text used for filtering; `None` means the label is used.
    pub filter_text: Option<String>,
    /// Edits applied together with the completion (e.g. an import statement).
    pub additional_text_edits: Vec<TextEdit>,
}

/// Kind of an inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// An inferred type, shown after a variable name.
    Type,
    /// A parameter name, shown before an argument.
    Parameter,
}

/// A hint rendered inside the source without changing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    pub position: Position,
    pub label: String,
    pub kind: InlayHintKind,
    pub padding_left: bool,
    pub padding_right: bool,
}

/// One node of the call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyItem {
    pub name: String,
    pub file: PathBuf,
    pub range: Range,
}

/// One call edge of the call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyCall {
    pub item: CallHierarchyItem,
    pub from_ranges: Vec<Range>,
}

/// One node of the type hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeHierarchyItem {
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub range: Range,
}

/// A quick fix or refactoring action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeAction {
    pub title: String,
    pub edits: Vec<TextEdit>,
    /// Diagnostics this action fixes (LSP `CodeAction.diagnostics`).
    pub diagnostics: Vec<Diagnostic>,
}

/// One callable signature shown by signature help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInformation {
    pub label: String,
    pub parameters: Vec<String>,
    /// Javadoc of the callable, rendered as Markdown.
    pub documentation: Option<String>,
}

/// Signature help result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: u32,
    pub active_parameter: u32,
}

/// A document link: a range that opens `target` (file URI or http(s) URL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLink {
    pub range: Range,
    pub target: String,
}

/// A code lens: a title and the range it annotates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLens {
    pub range: Range,
    pub title: String,
    pub command: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

/// LSP semantic token legend (tokenTypes), shared with the LSP capabilities.
pub const SEMANTIC_TOKEN_TYPES: [&str; 23] = [
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "event",
    "function",
    "method",
    "macro",
    "keyword",
    "modifier",
    "comment",
    "string",
    "number",
    "regexp",
    "operator",
    "decorator",
];

/// LSP semantic token legend (tokenModifiers), shared with the LSP capabilities.
pub const SEMANTIC_TOKEN_MODIFIERS: [&str; 10] = [
    "declaration",
    "definition",
    "readonly",
    "static",
    "deprecated",
    "abstract",
    "async",
    "modification",
    "documentation",
    "defaultLibrary",
];

/// A text replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDocumentEdit {
    pub file: std::path::PathBuf,
    pub edit: TextEdit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// A location in another file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: PathBuf,
    pub range: Range,
}

/// A span the editor highlights for the symbol under the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub range: Range,
    pub kind: Option<DocumentHighlightKind>,
}

/// How a highlighted span is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentHighlightKind {
    Text,
    Read,
    Write,
}

/// A foldable region in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: Option<FoldingRangeKind>,
}

/// A selection range: the range itself plus the next outer range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRange {
    pub range: Range,
    pub parent: Option<Box<SelectionRange>>,
}

/// The kind of a foldable region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldingRangeKind {
    Comment,
    Imports,
    Region,
}

/// The configuration a workspace was created with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceConfiguration {
    pub class_path: Vec<PathBuf>,
    pub source_path: Vec<PathBuf>,
    pub source_level: String,
    pub output_dir: PathBuf,
}

/// The analysis backend an editor talks to.
///
/// Implementations keep the workspace state across calls (index snapshots,
/// overlays, transient per-file analyses); callers must treat them as
/// single-threaded.
///
/// Replaces the old `AnalysisHost`; there is no compatibility layer.
pub trait WorkspaceService {
    /// Analyze a file and return its diagnostics.
    ///
    /// When text is provided it overrides the file content on disk (unsaved
    /// editor buffers).
    fn diagnostics(&mut self, file: &Path, text: Option<&str>) -> Vec<Diagnostic>;

    /// Symbols declared in the file.
    fn symbols(&mut self, file: &Path) -> Vec<Symbol>;

    /// The symbol at a position (go-to-definition / hover input).
    fn symbol_at(&mut self, file: &Path, position: Position) -> Option<Symbol>;

    /// Completion candidates at a position.
    fn completion(&mut self, file: &Path, position: Position) -> Vec<CompletionItem>;

    /// Inlay hints for a file (inferred types, parameter names).
    fn inlay_hints(&mut self, file: &Path) -> Vec<InlayHint>;

    /// Spans of the symbol under the cursor (editor highlight).
    fn document_highlight(&mut self, file: &Path, position: Position) -> Vec<DocumentHighlight>;

    /// Foldable regions of a file.
    fn folding_ranges(&mut self, file: &Path) -> Vec<FoldingRange>;

    /// Selection ranges for each position (inner to outer).
    fn selection_ranges(&mut self, file: &Path, positions: &[Position]) -> Vec<SelectionRange>;

    /// Files that transitively depend on `file` (including `file`).
    fn dependent_closure(&mut self, _file: &Path) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Replace the analysis class/source path (for example when build
    /// artifacts become available).
    fn update_analysis_config(&mut self, _class_path: Vec<PathBuf>, _source_path: Vec<PathBuf>) {}

    /// Drop the transient analysis for one file (its class files changed).
    fn invalidate_analysis(&mut self, _file: &Path) {}

    /// Reindent the current line after a trigger character (`}`, `;`, newline).
    fn on_type_formatting(
        &mut self,
        _file: &Path,
        _position: Position,
        _character: char,
    ) -> Vec<TextEdit> {
        Vec::new()
    }

    /// Links from import statements and http(s) URLs.
    fn document_links(&mut self, _file: &Path) -> Vec<DocumentLink> {
        Vec::new()
    }

    /// LSP `semanticTokens/full` data (5 integers per token, see legend).
    fn semantic_tokens(&mut self, _file: &Path) -> Vec<u32> {
        Vec::new()
    }

    /// Code lenses (reference and implementation counts) for one file.
    fn code_lenses(&mut self, _file: &Path) -> Vec<CodeLens> {
        Vec::new()
    }

    /// Organize the import block; empty when nothing changes.
    fn organize_imports(&mut self, _file: &Path) -> Vec<TextEdit> {
        Vec::new()
    }

    /// The declaration of the symbol at a position.
    fn definition(&mut self, file: &Path, position: Position) -> Option<Location>;

    /// The overridden methods of the method at a position.
    fn super_methods(&mut self, file: &Path, position: Position) -> Vec<Location>;

    /// The declaration of the type of the symbol at a position.
    fn type_definition(&mut self, file: &Path, position: Position) -> Option<Location>;

    /// All implementations/subclasses of the type at a position.
    fn implementation(&mut self, file: &Path, position: Position) -> Vec<Location>;

    /// Signature help for the call at a position.
    fn signature_help(&mut self, file: &Path, position: Position) -> Option<SignatureHelp>;

    /// Format the whole document (external formatter).
    fn format(&mut self, file: &Path, text: &str) -> Option<String>;

    /// Quick fixes available at a position.
    fn code_actions(&mut self, file: &Path, position: Position) -> Vec<CodeAction>;

    /// Prepare the type hierarchy for the type at a position.
    fn prepare_type_hierarchy(&mut self, file: &Path, position: Position) -> Option<TypeHierarchyItem>;

    /// Supertypes of a type hierarchy item.
    fn supertypes(&mut self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem>;

    /// Subtypes of a type hierarchy item.
    fn subtypes(&mut self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem>;

    /// Prepare the call hierarchy for the method at a position.
    fn prepare_call_hierarchy(&mut self, file: &Path, position: Position) -> Option<CallHierarchyItem>;

    /// Incoming calls of a call hierarchy item.
    fn incoming_calls(&mut self, item: &CallHierarchyItem) -> Vec<CallHierarchyCall>;

    /// Outgoing calls of a call hierarchy item.
    fn outgoing_calls(&mut self, item: &CallHierarchyItem) -> Vec<CallHierarchyCall>;

    /// Range that a rename at this position would edit, if any.
    fn prepare_rename(&mut self, file: &Path, position: Position) -> Option<Range>;

    /// References to the symbol at a position.
    fn references(&mut self, file: &Path, position: Position) -> Vec<Location>;

    /// Hover text for the symbol at a position.
    fn hover(&mut self, file: &Path, position: Position) -> Option<String>;

    /// Rename the symbol at a position across all project files.
    fn rename_workspace(
        &mut self,
        file: &Path,
        position: Position,
        new_name: &str,
    ) -> Vec<TextDocumentEdit>;

    /// Project-wide symbol search.
    fn workspace_symbols(&mut self, query: &str) -> Vec<WorkspaceSymbol>;

    /// Hierarchical symbols of a file (outline view).
    fn document_symbols(&mut self, file: &Path) -> Vec<DocumentSymbol>;

    /// Invalidate cached state for the given paths (didChangeWatchedFiles).
    fn invalidate(&mut self, paths: &[PathBuf]);

    /// Generation of the published index snapshot (0 before the first build).
    ///
    /// Used by smoke tests to prove that a repeated query served the same
    /// snapshot instead of rebuilding the project.
    fn index_generation(&self) -> u64;

    /// Whether a background index build is in flight.
    ///
    /// Queries keep serving the previous snapshot while this is true.
    fn is_index_building(&self) -> bool;

    /// Number of files in the published index snapshot.
    fn index_files(&self) -> usize {
        0
    }

    /// Number of symbols in the published index snapshot.
    fn index_symbols(&self) -> usize {
        0
    }

    /// Advance a finished background index build: publish the new snapshot and
    /// clear the building flag.
    ///
    /// Readers of `index_generation`/`is_index_building` must call this before
    /// reading, otherwise a finished build keeps waiting for the next query and
    /// the stats look stuck at `building: true, generation: 0`.
    fn poll_index(&mut self) {}

    /// Drop the unsaved buffer for `file` (the client closed the document).
    fn remove_overlay(&mut self, _file: &Path) {}

    /// Toggle parameter-name hints where the argument source text is exactly
    /// the parameter name. Other parameter hints are unaffected.
    fn set_inlay_hints_same_name(&mut self, _enabled: bool) {}

    /// Current same-name parameter-hint setting.
    fn inlay_hints_same_name(&self) -> bool {
        true
    }

    /// Current workspace configuration.
    fn configuration(&self) -> WorkspaceConfiguration;

    /// Replace the workspace configuration (class path / source roots).
    ///
    /// A change that affects indexing invalidates the index.
    fn reconfigure(&mut self, configuration: WorkspaceConfiguration);
}
