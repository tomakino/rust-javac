
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

/// A completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: SymbolKind,
    pub detail: Option<String>,
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

    /// References to the symbol at a position.
    fn references(&mut self, file: &Path, position: Position) -> Vec<Location>;

    /// Hover text for the symbol at a position.
    fn hover(&mut self, file: &Path, position: Position) -> Option<String>;
}
