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

pub(super) struct OuterNewCall {
    pub(super) typeargs: Option<List<crate::ids::TypeId>>,
    pub(super) argument_types: Vec<Option<crate::ids::TypeId>>,
    pub(super) arguments: usize,
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
/// Byte offset of the last non-whitespace byte before `before`.
pub(super) fn previous_non_ws_byte(text: &str, before: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = before.min(bytes.len());
    while index > 0 {
        index -= 1;
        if !bytes[index].is_ascii_whitespace() {
            return Some(index);
        }
    }
    None
}

/// Last standalone occurrence of `keyword` before `before`.
pub(super) fn last_keyword_before(text: &str, before: usize, keyword: &str) -> Option<usize> {
    let mut search = before;
    while let Some(position) = text[..search].rfind(keyword) {
        let before_ok = text[..position]
            .chars()
            .next_back()
            .map(|ch| !is_ident_char(ch))
            .unwrap_or(true);
        let after_ok = text[position + keyword.len()..]
            .chars()
            .next()
            .map(|ch| !is_ident_char(ch))
            .unwrap_or(true);
        if before_ok && after_ok {
            return Some(position);
        }
        search = position;
    }
    None
}

pub(super) fn method_symbol(
    ctx: &crate::compiler_context::CompilerContext,
    meth: crate::ids::TreeId,
) -> Option<crate::ids::SymbolId> {
    match &ctx.tree(meth).kind {
        TreeKind::Ident { sym, .. } | TreeKind::FieldAccess { sym, .. } => *sym,
        _ => None,
    }
}

pub(super) fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

/// The innermost unmatched `(` before `before_byte`.
pub(super) fn last_open_paren_before(text: &str, before_byte: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut stack = Vec::new();
    let mut index = 0usize;
    while index < before_byte {
        match bytes[index] {
            b'"' | b'\'' => {
                let quote = bytes[index];
                index += 1;
                while index < before_byte {
                    if bytes[index] == b'\\' {
                        index += 2;
                    } else if bytes[index] == quote {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
            }
            b'/' if index + 1 < before_byte && bytes[index + 1] == b'/' => {
                index += 2;
                while index < before_byte && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if index + 1 < before_byte && bytes[index + 1] == b'*' => {
                index += 2;
                while index + 1 < before_byte
                    && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(before_byte);
            }
            b'(' => {
                stack.push(index);
                index += 1;
            }
            b')' => {
                stack.pop();
                index += 1;
            }
            _ => index += 1,
        }
    }
    stack.last().copied()
}

/// Top-level argument index of `position` inside the call opened at `open`.
pub(super) fn argument_index(text: &str, open: usize, position: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = open + 1;
    let mut argument = 0usize;
    while index < position {
        match bytes[index] {
            b'"' | b'\'' => {
                let quote = bytes[index];
                index += 1;
                while index < position {
                    if bytes[index] == b'\\' {
                        index += 2;
                    } else if bytes[index] == quote {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
            }
            b'/' if index + 1 < position && bytes[index + 1] == b'/' => {
                index += 2;
                while index < position && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if index + 1 < position && bytes[index + 1] == b'*' => {
                index += 2;
                while index + 1 < position
                    && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(position);
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = (depth - 1).max(0);
                index += 1;
            }
            b',' if depth == 0 => {
                argument += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    argument
}

/// Class simple name of `new SomeType` immediately before `open`, if the
/// enclosing call is a constructor call.
pub(super) fn outer_constructor_name(text: &str, open: usize) -> Option<String> {
    let before = text.get(..open)?.trim_end();
    let new_pos = before.rfind("new")?;
    if new_pos > 0
        && before[..new_pos]
            .chars()
            .next_back()
            .is_some_and(is_ident_char)
    {
        return None;
    }
    let after = &before[new_pos + 3..];
    if !after.chars().next().is_some_and(|ch| ch.is_whitespace()) {
        return None;
    }
    let rest = after.trim();
    if rest.is_empty()
        || rest.contains('=')
        || rest.contains(';')
        || rest.contains(')')
        || rest.contains('(')
    {
        return None;
    }
    let simple = strip_type_arguments(rest);
    let simple = simple.rsplit('.').next()?.trim();
    if simple.is_empty() || simple.ends_with(']') {
        return None;
    }
    Some(simple.to_string())
}

pub(super) fn strip_type_arguments(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for ch in text.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
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
        sort_text: None,
        insert_text: None,
        insert_text_format: javac_analysis_api::InsertTextFormat::PlainText,
        replace_prefix_len: None,
        filter_text: None,
        additional_text_edits: Vec::new(),
    }
}

/// Byte offset where the receiver expression starts (before the dot).
pub(super) fn expression_start_byte(text: &str, before: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = before;
    let mut depth = 0i32;
    while index > 0 {
        let byte = bytes[index - 1];
        match byte {
            b')' | b']' => {
                depth += 1;
                index -= 1;
            }
            b'(' | b'[' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                index -= 1;
            }
            b'.' => {
                index -= 1;
            }
            b' ' | b'\t' => {
                if depth == 0 {
                    break;
                }
                index -= 1;
            }
            byte if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' => {
                index -= 1;
            }
            _ => break,
        }
    }
    index
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