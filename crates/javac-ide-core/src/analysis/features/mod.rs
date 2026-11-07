//! Editor features. Each capability lives in its own module; the
//! AnalysisHost trait implementation in the parent module only forwards.

pub(crate) mod actions;
pub(crate) mod completion;
pub(crate) mod folding;
pub(crate) mod format;
pub(crate) mod hierarchy;
pub(crate) mod hover;
pub(crate) mod inlay_hints;
pub(crate) mod navigation;
pub(crate) mod on_type_formatting;
pub(crate) mod selection_range;
pub(crate) mod semantic_tokens;
pub(crate) mod organize_imports;
pub(crate) mod generate;
pub(crate) mod signature;

/// Map an internal symbol kind string to the public kind.
pub(crate) fn map_kind(kind: &'static str) -> javac_analysis_api::SymbolKind {
    match kind {
        "class" => javac_analysis_api::SymbolKind::Class,
        "interface" => javac_analysis_api::SymbolKind::Interface,
        "enum" => javac_analysis_api::SymbolKind::Enum,
        "record" => javac_analysis_api::SymbolKind::Record,
        "method" => javac_analysis_api::SymbolKind::Method,
        "constructor" => javac_analysis_api::SymbolKind::Constructor,
        "field" => javac_analysis_api::SymbolKind::Field,
        "local" => javac_analysis_api::SymbolKind::LocalVariable,
        "keyword" => javac_analysis_api::SymbolKind::Keyword,
        "nested" => javac_analysis_api::SymbolKind::Class,
        _ => javac_analysis_api::SymbolKind::Field,
    }
}
