//! Context-aware completion ranking.
//!
//! The completion context selects the kind-weight table, the same way a
//! search engine switches its ranking model per intent. The sort key is a
//! single tuple so every dimension is considered exactly once:
//!
//!   (match quality, exact, context kind, name, arity, signature length,
//!    signature, label)

use javac_analysis_api::{CompletionItem, SymbolKind};

use super::search::{match_label, Match};

/// Completion intent, selected from the parsed insertion context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RankContext {
    /// new expressions - classes and constructors.
    New,
    /// After a dot / receiver chain - members.
    Receiver,
    /// Statement position - locals first, then fields, methods, types.
    Statement,
    /// Import declaration - packages before classes.
    Import,
    /// Package members - subpackages before classes.
    Package,
    /// Class body - types before methods and fields.
    ClassBody,
}

fn context_kind_rank(context: RankContext, kind: SymbolKind) -> u8 {
    match context {
        RankContext::New => match kind {
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Record => 0,
            _ => 9,
        },
        RankContext::Receiver => match kind {
            // IDEA keeps fields before methods for instance receivers.
            SymbolKind::Field => 0,
            SymbolKind::Method => 1,
            // Package completion lists subpackages before classes.
            SymbolKind::Package => 0,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Record => 2,
            _ => 3,
        },
        RankContext::Statement => match kind {
            SymbolKind::LocalVariable | SymbolKind::Parameter => 0,
            SymbolKind::Field => 1,
            SymbolKind::Method => 2,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Record => 3,
            SymbolKind::Package => 4,
            _ => 5,
        },
        RankContext::Import | RankContext::Package => match kind {
            SymbolKind::Package => 0,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Record => 1,
            _ => 2,
        },
        RankContext::ClassBody => match kind {
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Record => 0,
            SymbolKind::Method => 1,
            SymbolKind::Field => 2,
            _ => 3,
        },
    }
}

/// The part before the parenthesis (constructors keep their signature in the
/// label, so this groups overloads).
fn name_of(item: &CompletionItem) -> String {
    item.label
        .split('(')
        .next()
        .unwrap_or(&item.label)
        .to_ascii_lowercase()
}

/// The parameter list: labelDetails when present, otherwise the label tail.
fn signature_of(item: &CompletionItem) -> String {
    item.label_detail.clone().unwrap_or_else(|| {
        item.label
            .split_once('(')
            .map(|(_, rest)| rest.to_string())
            .unwrap_or_default()
    })
}

fn arity(signature: &str) -> usize {
    let inner = signature.trim_start_matches('(').trim_end_matches(')');
    if inner.trim().is_empty() {
        0
    } else {
        inner.matches(',').count() + 1
    }
}

fn exact_rank(item: &CompletionItem, prefix: &str) -> u8 {
    if prefix.is_empty() {
        return 1;
    }
    if item.label.eq_ignore_ascii_case(prefix) { 0 } else { 1 }
}

/// Match and rank candidates for one context. Best first, stable for ties.
pub(crate) fn rank_items(
    items: Vec<CompletionItem>,
    prefix: &str,
    context: RankContext,
) -> Vec<CompletionItem> {
    let mut scored: Vec<(CompletionItem, Match)> = Vec::with_capacity(items.len());
    for item in items {
        if let Some(matched) = match_label(prefix, &item.label) {
            scored.push((item, matched));
        }
    }
    scored.sort_by(|(left, left_match), (right, right_match)| {
        let left_signature = signature_of(left);
        let right_signature = signature_of(right);
        let left_key = (
            left_match.rank(),
            exact_rank(left, prefix),
            context_kind_rank(context, left.kind),
            name_of(left),
            arity(&left_signature),
            left_signature.len(),
            left_signature,
            left.label.clone(),
        );
        let right_key = (
            right_match.rank(),
            exact_rank(right, prefix),
            context_kind_rank(context, right.kind),
            name_of(right),
            arity(&right_signature),
            right_signature.len(),
            right_signature,
            right.label.clone(),
        );
        left_key.cmp(&right_key)
    });
    scored.into_iter().map(|(item, _)| item).collect()
}

/// Keep only candidates matching the prefix (no fuzzy fallback).
pub(crate) fn filter_items(mut items: Vec<CompletionItem>, prefix: &str) -> Vec<CompletionItem> {
    items.retain(|item| match_label(prefix, &item.label).is_some());
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use javac_analysis_api::InsertTextFormat;

    fn item(label: &str, kind: SymbolKind) -> CompletionItem {
        CompletionItem {
            label: label.to_string(),
            kind,
            detail: None,
            documentation: None,
            label_detail: None,
            sort_text: None,
            insert_text: None,
            insert_text_format: InsertTextFormat::PlainText,
            replace_prefix_len: None,
            filter_text: None,
        }
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|item| item.label.as_str()).collect()
    }

    #[test]
    fn start_matches_come_first() {
        let items = vec![
            item("arraycopy", SymbolKind::Method),
            item("identityHashCode", SymbolKind::Method),
            item("console", SymbolKind::Method),
        ];
        let ranked = rank_items(items, "co", RankContext::Receiver);
        assert_eq!(labels(&ranked), vec!["console", "identityHashCode", "arraycopy"]);
    }

    #[test]
    fn receiver_context_prefers_fields_over_methods() {
        let items = vec![
            item("value", SymbolKind::Class),
            item("value", SymbolKind::Field),
            item("value", SymbolKind::Method),
        ];
        let ranked = rank_items(items, "value", RankContext::Receiver);
        assert_eq!(ranked[0].kind, SymbolKind::Field);
        assert_eq!(ranked[1].kind, SymbolKind::Method);
        assert_eq!(ranked[2].kind, SymbolKind::Class);
    }

    #[test]
    fn new_context_keeps_classes_first() {
        let items = vec![
            item("ZipFile", SymbolKind::Method),
            item("ZipFile", SymbolKind::Class),
        ];
        let ranked = rank_items(items, "Zip", RankContext::New);
        assert_eq!(ranked[0].kind, SymbolKind::Class);
    }

    #[test]
    fn package_context_puts_packages_first() {
        let items = vec![
            item("ArrayList", SymbolKind::Class),
            item("concurrent", SymbolKind::Package),
        ];
        let ranked = rank_items(items, "", RankContext::Package);
        assert_eq!(ranked[0].kind, SymbolKind::Package);
    }

    #[test]
    fn overloads_by_arity_then_signature_length() {
        let mut first = item("FileInputStream(File file)", SymbolKind::Class);
        first.label_detail = Some("(File file)".to_string());
        let mut second = item("FileInputStream(String name)", SymbolKind::Class);
        second.label_detail = Some("(String name)".to_string());
        let mut third = item("FileInputStream(FileDescriptor fdObj)", SymbolKind::Class);
        third.label_detail = Some("(FileDescriptor fdObj)".to_string());
        let ranked = rank_items(vec![third, first, second], "Filein", RankContext::New);
        assert_eq!(
            labels(&ranked),
            vec![
                "FileInputStream(File file)",
                "FileInputStream(String name)",
                "FileInputStream(FileDescriptor fdObj)"
            ]
        );
    }

    #[test]
    fn exact_label_wins() {
        let items = vec![item("System", SymbolKind::Class), item("SystemX", SymbolKind::Class)];
        let ranked = rank_items(items, "System", RankContext::Statement);
        assert_eq!(labels(&ranked), vec!["System", "SystemX"]);
    }

    #[test]
    fn filter_keeps_only_matches() {
        let items = vec![
            item("File", SymbolKind::Class),
            item("FilterOutputStream", SymbolKind::Class),
        ];
        let filtered = filter_items(items, "File");
        assert_eq!(labels(&filtered), vec!["File"]);
    }
}
