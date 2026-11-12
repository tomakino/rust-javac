use super::context::InsertionContext;
use crate::tree::jc_tree::TreeKind;
use crate::util::list::List;
use crate::workspace::index::IndexSymbolKind;

/// Compilation-point context for visibility decisions.
pub(super) struct CallContext {
    pub(super) current_class: Option<crate::ids::SymbolId>,
    pub(super) current_package: String,
    pub(super) insertion: InsertionContext,
}

#[derive(Clone)]
pub(super) struct ExpectedNew {
    pub(super) label: String,
    pub(super) array: bool,
    pub(super) class: bool,
    pub(super) array_labels: Vec<String>,
}

pub(super) const KEYWORDS: &[&str] = &[
    "public", "private", "protected", "static", "final", "abstract", "native",
    "synchronized", "transient", "volatile", "strictfp", "default", "class",
    "interface", "enum", "record", "extends", "implements", "import", "package",
    "void", "int", "long", "short", "byte", "char", "boolean", "float", "double",
    "if", "else", "for", "while", "do", "switch", "case", "break", "continue",
    "return", "new", "try", "catch", "finally", "throw", "throws", "this", "super",
    "null", "true", "false", "instanceof", "var", "yield", "assert",
];
/// Build a completion item with the minimal fields set.
pub(super) fn method_symbol(
    ctx: &crate::compiler_context::CompilerContext,
    meth: crate::ids::TreeId,
) -> Option<crate::ids::SymbolId> {
    match &ctx.tree(meth).kind {
        TreeKind::Ident { sym, .. } | TreeKind::FieldAccess { sym, .. } => *sym,
        _ => None,
    }
}

pub(super) fn completion_item(
    label: String,
    kind: javac_analysis_api::SymbolKind,
    detail: Option<String>,
    label_detail: Option<String>,
) -> javac_analysis_api::CompletionItem {
    javac_analysis_api::CompletionItem {
        label,
        kind,
        detail,
        documentation: None,
        label_detail,
        signature: None,
        sort_text: None,
        insert_text: None,
        insert_text_format: javac_analysis_api::InsertTextFormat::PlainText,
        replace_prefix_len: None,
        filter_text: None,
        additional_text_edits: Vec::new(),
    }
}

/// Anonymous classes (Outer$1) are compiler artifacts, not source entities.
pub(super) fn is_anonymous_class(flat: &str) -> bool {
    let simple = flat.rsplit(['.', '$']).next().unwrap_or(flat);
    !simple.is_empty() && simple.chars().all(|c| c.is_ascii_digit())
}

/// JDK-internal packages are hidden from completion (IDEA hides them too).
pub(super) fn is_internal_package(flat: &str) -> bool {
    let package = flat.rsplit_once('.').map(|(package, _)| package).unwrap_or(flat);
    package.starts_with("sun.")
        || package.starts_with("com.sun.")
        || package.contains(".internal")
        || package.starts_with("jdk.internal.")
        || package.starts_with("jdk.nio.zipfs")
}

pub(super) fn index_kind_name(kind: IndexSymbolKind) -> &'static str {
    match kind {
        IndexSymbolKind::Interface => "interface",
        IndexSymbolKind::Enum => "enum",
        IndexSymbolKind::Record => "record",
        _ => "class",
    }
}