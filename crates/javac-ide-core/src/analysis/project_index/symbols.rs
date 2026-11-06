//! Shared index-to-API symbol kind mapping.

use javac_analysis_api::SymbolKind;

use super::model::IndexSymbolKind;

pub(crate) fn api_symbol_kind(kind: IndexSymbolKind) -> SymbolKind {
    match kind {
        IndexSymbolKind::Class | IndexSymbolKind::Annotation => SymbolKind::Class,
        IndexSymbolKind::Interface => SymbolKind::Interface,
        IndexSymbolKind::Enum => SymbolKind::Enum,
        IndexSymbolKind::Record => SymbolKind::Record,
        IndexSymbolKind::Field => SymbolKind::Field,
        IndexSymbolKind::Method | IndexSymbolKind::Constructor => SymbolKind::Method,
        IndexSymbolKind::Local => SymbolKind::LocalVariable,
        IndexSymbolKind::Parameter => SymbolKind::Parameter,
        IndexSymbolKind::TypeParameter => SymbolKind::TypeParameter,
        IndexSymbolKind::Other => SymbolKind::Class,
    }
}
