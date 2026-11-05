
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

/// A symbol shown in the outline view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
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
}

/// One callable signature shown by signature help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInformation {
    pub label: String,
    pub parameters: Vec<String>,
}

/// Signature help result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: u32,
    pub active_parameter: u32,
}

/// A text replacement.
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

/// The analysis backend an editor talks to.
///
/// Implementations may keep state across calls (a resident session); callers
/// must treat them as single-threaded.
pub trait AnalysisHost {
    /// Analyze a file and return its diagnostics.
    ///
    /// When text is provided it overrides the file content on disk (unsaved
    /// editor buffers).
    fn analyze(&mut self, file: &Path, text: Option<&str>) -> Vec<Diagnostic>;

    /// Symbols declared in the file.
    fn symbols(&mut self, file: &Path) -> Vec<Symbol>;

    /// The symbol at a position (go-to-definition / hover input).
    fn symbol_at(&mut self, file: &Path, position: Position) -> Option<Symbol>;

    /// Completion candidates at a position.
    fn completion(&mut self, file: &Path, position: Position) -> Vec<CompletionItem>;

    /// The declaration of the symbol at a position.
    fn definition(&mut self, file: &Path, position: Position) -> Option<Location>;

    /// The declaration of the type of the symbol at a position.
    fn type_definition(&mut self, file: &Path, position: Position) -> Option<Location> {
        self.definition(file, position)
    }

    /// All implementations/subclasses of the type at a position.
    fn implementation(&mut self, file: &Path, position: Position) -> Vec<Location> {
        let _ = (file, position);
        Vec::new()
    }

    /// Signature help for the call at a position.
    fn signature_help(&mut self, file: &Path, position: Position) -> Option<SignatureHelp> {
        let _ = (file, position);
        None
    }

    /// Format the whole document (external formatter).
    fn format(&mut self, file: &Path, text: &str) -> Option<String> {
        let _ = (file, text);
        None
    }

    /// Quick fixes available at a position.
    fn code_actions(&mut self, file: &Path, position: Position) -> Vec<CodeAction> {
        let _ = (file, position);
        Vec::new()
    }

    /// Prepare the type hierarchy for the type at a position.
    fn prepare_type_hierarchy(&mut self, file: &Path, position: Position) -> Option<TypeHierarchyItem> {
        let _ = (file, position);
        None
    }

    /// Supertypes of a type hierarchy item.
    fn supertypes(&mut self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
        let _ = item;
        Vec::new()
    }

    /// Subtypes of a type hierarchy item.
    fn subtypes(&mut self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
        let _ = item;
        Vec::new()
    }

    /// Prepare the call hierarchy for the method at a position.
    fn prepare_call_hierarchy(&mut self, file: &Path, position: Position) -> Option<CallHierarchyItem> {
        let _ = (file, position);
        None
    }

    /// Incoming calls of a call hierarchy item.
    fn incoming_calls(&mut self, item: &CallHierarchyItem) -> Vec<CallHierarchyCall> {
        let _ = item;
        Vec::new()
    }

    /// Outgoing calls of a call hierarchy item.
    fn outgoing_calls(&mut self, item: &CallHierarchyItem) -> Vec<CallHierarchyCall> {
        let _ = item;
        Vec::new()
    }

    /// References to the symbol at a position.
    fn references(&mut self, file: &Path, position: Position) -> Vec<Location>;

    /// Hover text for the symbol at a position.
    fn hover(&mut self, file: &Path, position: Position) -> Option<String>;

    /// Rename the symbol at a position; returns the edits to apply.
    fn rename(&mut self, file: &Path, position: Position, new_name: &str) -> Vec<TextEdit> {
        let _ = (file, position, new_name);
        Vec::new()
    }

    /// Hierarchical symbols of a file (outline view).
    fn document_symbols(&mut self, file: &Path) -> Vec<DocumentSymbol> {
        let _ = file;
        Vec::new()
    }
}
