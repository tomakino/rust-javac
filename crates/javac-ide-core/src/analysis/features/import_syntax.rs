//! Structured package/import state decoded from AST nodes.

/// One `import static` declaration decoded from `Import.qualid`.
#[derive(Debug, Clone)]
pub(crate) struct StaticImportSyntax {
    pub type_path: String,
    pub member: Option<String>,
    pub wildcard: bool,
}

/// One import declaration decoded from its `Import` AST node.
#[derive(Debug, Clone)]
pub(crate) struct ImportDecl {
    pub path: String,
    pub segments: Vec<String>,
    pub static_import: bool,
    pub wildcard: bool,
    pub start: usize,
    pub end: usize,
}

/// Package/import state of one compilation unit, decoded from AST nodes.
#[derive(Debug, Clone, Default)]
pub(crate) struct ImportSyntax {
    pub package: String,
    pub imports: Vec<String>,
    pub wildcard: Vec<String>,
    pub static_imports: Vec<StaticImportSyntax>,
    pub declarations: Vec<ImportDecl>,
    /// UTF-16 end of the last package/import declaration.
    pub end: usize,
}
