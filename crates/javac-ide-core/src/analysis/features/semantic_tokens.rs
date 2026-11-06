//! Semantic tokens.
//!
//! Identifier tokens come from the compiler symbol table (`FileAnalysis`
//! occurrences: declarations, references, type uses). Keywords, literals,
//! comments and operators have no symbol and are recognized lexically; those
//! scans only classify lexical categories, never identifier semantics.

use std::path::{Path, PathBuf};

use crate::code::flags;
use crate::code::symbol::SymbolKind as SymbolKindData;
use crate::ids::SymbolId;
use crate::tree::tree_scanner::TreeScannerVisitor;
use crate::workspace::index::model::OccurrenceRole;

use javac_analysis_api::{SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES};

use super::super::positions::{byte_to_utf16, offset_to_line_col};
use super::super::{canonical, FileAnalysis};

const KEYWORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const",
    "continue", "default", "do", "double", "else", "enum", "extends", "final", "finally", "float",
    "for", "goto", "if", "implements", "import", "instanceof", "int", "interface", "long", "native",
    "new", "package", "private", "protected", "public", "return", "short", "static", "strictfp",
    "super", "switch", "synchronized", "this", "throw", "throws", "transient", "try", "void",
    "volatile", "while", "var", "record", "sealed", "permits", "yield", "true", "false", "null",
];

#[derive(Debug, Clone, Copy)]
struct RawToken {
    line: u32,
    column: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

fn type_index(name: &str) -> u32 {
    SEMANTIC_TOKEN_TYPES
        .iter()
        .position(|token| *token == name)
        .unwrap_or(0) as u32
}

fn modifier_bit(name: &str) -> u32 {
    1u32 << SEMANTIC_TOKEN_MODIFIERS
        .iter()
        .position(|modifier| *modifier == name)
        .unwrap_or(0)
}

fn symbol_token(ctx: &crate::compiler_context::CompilerContext, symbol: SymbolId) -> Option<(u32, u32)> {
    let symbol = ctx.sym(symbol);
    let flags_value = symbol.flags();
    let is = |flag: i32| (flags_value & flag as i64) != 0;
    let mut modifiers = 0u32;
    if is(flags::STATIC) {
        modifiers |= modifier_bit("static");
    }
    if is(flags::FINAL) {
        modifiers |= modifier_bit("readonly");
    }
    if is(flags::ABSTRACT) {
        modifiers |= modifier_bit("abstract");
    }
    if is(flags::DEPRECATED) {
        modifiers |= modifier_bit("deprecated");
    }
    let token_type = match &symbol.kind_data {
        SymbolKindData::ClassSymbol { .. } => {
            if is(flags::ANNOTATION) {
                type_index("decorator")
            } else if is(flags::ENUM) {
                type_index("enum")
            } else if (flags_value & flags::RECORD) != 0 {
                type_index("struct")
            } else if is(flags::INTERFACE) {
                type_index("interface")
            } else {
                type_index("class")
            }
        }
        SymbolKindData::MethodSymbol { .. } => type_index("method"),
        SymbolKindData::TypeVariableSymbol => type_index("typeParameter"),
        SymbolKindData::VarSymbol { .. } => {
            let owner = symbol.owner;
            match owner.map(|owner| ctx.sym(owner)) {
                Some(owner) if matches!(owner.kind_data, SymbolKindData::ClassSymbol { .. }) => {
                    if (owner.flags() & flags::ENUM as i64) != 0 {
                        type_index("enumMember")
                    } else {
                        type_index("property")
                    }
                }
                Some(owner) if matches!(owner.kind_data, SymbolKindData::MethodSymbol { .. }) => {
                    if (flags_value & flags::PARAMETER) != 0 {
                        type_index("parameter")
                    } else {
                        type_index("variable")
                    }
                }
                _ => type_index("variable"),
            }
        }
        _ => return None,
    };
    Some((token_type, modifiers))
}

impl FileAnalysis {
    pub(crate) fn semantic_tokens_impl(&mut self, file: &Path) -> Vec<u32> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let hash = crate::workspace::model::hash_text(&text);
        if let Some((cached_hash, data)) = &self.semantic_tokens_cache {
            if *cached_hash == hash {
                return data.clone();
            }
        }
        // Declarations and references come from the index extractor (the same
        // tree/symbol extraction used for occurrence facts), including javac
        // flags. The extractor compiles the file once; the result is cached by
        // text hash above.
        let extracted = crate::workspace::index::extract::extract_file(
            &self.config,
            file,
            self.file,
            &text,
        );
        let mut raw: Vec<RawToken> = Vec::new();
        for declaration in &extracted.declarations {
            if declaration.name.starts_with('<') {
                continue;
            }
            let Some(token_type) = kind_token(declaration.identity.kind) else {
                continue;
            };
            let mut modifiers = modifier_bit("declaration");
            let flags_value = declaration.modifiers as i64;
            if flags_value & flags::STATIC as i64 != 0 {
                modifiers |= modifier_bit("static");
            }
            if flags_value & flags::FINAL as i64 != 0 {
                modifiers |= modifier_bit("readonly");
            }
            if flags_value & flags::ABSTRACT as i64 != 0 {
                modifiers |= modifier_bit("abstract");
            }
            if flags_value & flags::DEPRECATED as i64 != 0 {
                modifiers |= modifier_bit("deprecated");
            }
            let start = skip_whitespace(
                &text,
                crate::analysis::positions::declared_name_start(
                    &text,
                    declaration.range.start,
                    &declaration.name,
                ),
            );
            let length = declaration.name.encode_utf16().count() as u32;
            push_range(&text, start, length, token_type, modifiers, &mut raw);
        }
        for occurrence in &extracted.occurrences {
            if occurrence.role == OccurrenceRole::Import {
                continue;
            }
            if occurrence.identity.flat_name.contains('<') {
                continue;
            }
            let Some(token_type) = kind_token(occurrence.identity.kind) else {
                continue;
            };
            let simple = occurrence
                .identity
                .flat_name
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_string();
            let mut modifiers = 0;
            let start = if occurrence.role == OccurrenceRole::Declaration {
                modifiers |= modifier_bit("declaration");
                skip_whitespace(
                    &text,
                    crate::analysis::positions::declared_name_start(
                        &text,
                        occurrence.range.start,
                        &simple,
                    ),
                )
            } else {
                occurrence.range.start
            };
            let length = simple.encode_utf16().count() as u32;
            push_range(&text, start, length, token_type, modifiers, &mut raw);
        }
        raw.extend(lexical_tokens(&text));
        let data = encode(raw);
        self.semantic_tokens_cache = Some((hash, data.clone()));
        data
    }
}

fn kind_token(kind: crate::workspace::index::key::IndexSymbolKind) -> Option<u32> {
    use crate::workspace::index::key::IndexSymbolKind;
    Some(match kind {
        IndexSymbolKind::Class => type_index("class"),
        IndexSymbolKind::Interface => type_index("interface"),
        IndexSymbolKind::Enum => type_index("enum"),
        IndexSymbolKind::Annotation => type_index("decorator"),
        IndexSymbolKind::Record => type_index("struct"),
        IndexSymbolKind::Field => type_index("property"),
        IndexSymbolKind::Method | IndexSymbolKind::Constructor => type_index("method"),
        IndexSymbolKind::Local => type_index("variable"),
        IndexSymbolKind::Parameter => type_index("parameter"),
        IndexSymbolKind::TypeParameter => type_index("typeParameter"),
        IndexSymbolKind::Other => return None,
    })
}

fn skip_whitespace(text: &str, start: u32) -> u32 {
    let bytes = text.as_bytes();
    let mut index = start as usize;
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    index as u32
}

fn push_range(
    text: &str,
    start: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
    raw: &mut Vec<RawToken>,
) {
    if length == 0 {
        return;
    }
    let start = byte_to_utf16(text, start as usize);
    let (line, column) = offset_to_line_col(text, start);
    raw.push(RawToken {
        line,
        column,
        length,
        token_type,
        modifiers,
    });
}

/// Lexical tokens: comments, strings/char/text blocks, keywords, numbers and
/// operators. Multi-line comments/literals are split per line (LSP tokens do
/// not span lines in practice).
fn lexical_tokens(text: &str) -> Vec<RawToken> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut line = 0u32;
    let mut column = 0u32;
    let mut index = 0usize;
    let mut push = |tokens: &mut Vec<RawToken>, start_line: u32, start_column: u32, length: u32, token_type: u32, modifiers: u32| {
        if length > 0 {
            tokens.push(RawToken {
                line: start_line,
                column: start_column,
                length,
                token_type,
                modifiers,
            });
        }
    };
    while index < chars.len() {
        let current = chars[index];
        // Line comment.
        if current == '/' && index + 1 < chars.len() && chars[index + 1] == '/' {
            let start_line = line;
            let start_column = column;
            let mut length = 0u32;
            while index < chars.len() && chars[index] != '\n' {
                length += chars[index].len_utf16() as u32;
                index += 1;
            }
            column += length;
            push(&mut tokens, start_line, start_column, length, type_index("comment"), 0);
            continue;
        }
        // Block comment.
        if current == '/' && index + 1 < chars.len() && chars[index + 1] == '*' {
            let javadoc = index + 2 < chars.len() && chars[index + 2] == '*';
            let modifiers = if javadoc { modifier_bit("documentation") } else { 0 };
            index += 2;
            column += 2;
            let mut start_line = line;
            let mut start_column = column - 2;
            let mut length = 2u32;
            while index < chars.len() {
                if chars[index] == '*' && index + 1 < chars.len() && chars[index + 1] == '/' {
                    length += 2;
                    index += 2;
                    column += 2;
                    push(&mut tokens, start_line, start_column, length, type_index("comment"), modifiers);
                    break;
                }
                if chars[index] == '\n' {
                    push(&mut tokens, start_line, start_column, length, type_index("comment"), modifiers);
                    line += 1;
                    column = 0;
                    index += 1;
                    start_line = line;
                    start_column = 0;
                    length = 0;
                    continue;
                }
                length += chars[index].len_utf16() as u32;
                column += chars[index].len_utf16() as u32;
                index += 1;
            }
            continue;
        }
        // Text block.
        if current == '"' && index + 2 < chars.len() && chars[index + 1] == '"' && chars[index + 2] == '"' {
            let mut start_line = line;
            let mut start_column = column;
            let mut length = 3u32;
            column += 3;
            index += 3;
            while index < chars.len() {
                if chars[index] == '"' && index + 2 < chars.len() && chars[index + 1] == '"' && chars[index + 2] == '"' {
                    length += 3;
                    column += 3;
                    index += 3;
                    push(&mut tokens, start_line, start_column, length, type_index("string"), 0);
                    break;
                }
                if chars[index] == '\n' {
                    push(&mut tokens, start_line, start_column, length, type_index("string"), 0);
                    line += 1;
                    column = 0;
                    index += 1;
                    start_line = line;
                    start_column = 0;
                    length = 0;
                    continue;
                }
                length += chars[index].len_utf16() as u32;
                column += chars[index].len_utf16() as u32;
                index += 1;
            }
            continue;
        }
        // String / char literal.
        if current == '"' || current == '\'' {
            let quote = current;
            let start_line = line;
            let start_column = column;
            let mut length = 0u32;
            let mut escaped = false;
            while index < chars.len() {
                let value = chars[index];
                if value == '\n' {
                    break;
                }
                length += value.len_utf16() as u32;
                column += value.len_utf16() as u32;
                index += 1;
                if escaped {
                    escaped = false;
                    continue;
                }
                if value == '\\' {
                    escaped = true;
                    continue;
                }
                if value == quote {
                    break;
                }
            }
            push(&mut tokens, start_line, start_column, length, type_index("string"), 0);
            continue;
        }
        // Identifier / keyword.
        if current.is_alphabetic() || current == '_' || current == '$' {
            let start_line = line;
            let start_column = column;
            let mut length = 0u32;
            let mut word = String::new();
            while index < chars.len() {
                let value = chars[index];
                if value.is_alphanumeric() || value == '_' || value == '$' {
                    word.push(value);
                    length += value.len_utf16() as u32;
                    column += value.len_utf16() as u32;
                    index += 1;
                } else {
                    break;
                }
            }
            if KEYWORDS.contains(&word.as_str()) {
                push(&mut tokens, start_line, start_column, length, type_index("keyword"), 0);
            }
            continue;
        }
        // Number.
        if current.is_ascii_digit() {
            let start_line = line;
            let start_column = column;
            let mut length = 0u32;
            while index < chars.len() {
                let value = chars[index];
                if value.is_alphanumeric() || value == '_' || value == '.' {
                    length += value.len_utf16() as u32;
                    column += value.len_utf16() as u32;
                    index += 1;
                } else {
                    break;
                }
            }
            push(&mut tokens, start_line, start_column, length, type_index("number"), 0);
            continue;
        }
        // Operators (two-character forms first).
        if "+-*/%=<>!&|^~?:".contains(current) {
            let start_line = line;
            let start_column = column;
            let mut length = 1u32;
            if index + 1 < chars.len() {
                let next = chars[index + 1];
                let two = matches!(
                    (current, next),
                    ('=', '=')
                        | ('!', '=')
                        | ('<', '=')
                        | ('>', '=')
                        | ('&', '&')
                        | ('|', '|')
                        | ('+', '+')
                        | ('-', '-')
                        | ('+', '=')
                        | ('-', '=')
                        | ('*', '=')
                        | ('/', '=')
                        | ('%', '=')
                        | ('&', '=')
                        | ('|', '=')
                        | ('^', '=')
                        | ('-', '>')
                        | (':', ':')
                        | ('<', '<')
                        | ('>', '>')
                );
                if two {
                    length = 2;
                }
            }
            column += length;
            index += length as usize;
            push(&mut tokens, start_line, start_column, length, type_index("operator"), 0);
            continue;
        }
        if current == '\n' {
            line += 1;
            column = 0;
        } else {
            column += current.len_utf16() as u32;
        }
        index += 1;
    }
    tokens
}

fn encode(mut tokens: Vec<RawToken>) -> Vec<u32> {
    // Semantic tokens are collected first, so the stable sort keeps them on
    // duplicate positions (declaration before a lexical token).
    tokens.sort_by_key(|token| (token.line, token.column));
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut previous_line = 0u32;
    let mut previous_column = 0u32;
    let mut last_position: Option<(u32, u32)> = None;
    for token in tokens {
        if token.length == 0 {
            continue;
        }
        if last_position == Some((token.line, token.column)) {
            continue;
        }
        last_position = Some((token.line, token.column));
        let delta_line = token.line.saturating_sub(previous_line);
        let delta_start = if delta_line == 0 {
            token.column.saturating_sub(previous_column)
        } else {
            token.column
        };
        data.extend([
            delta_line,
            delta_start,
            token.length,
            token.token_type,
            token.modifiers,
        ]);
        previous_line = token.line;
        previous_column = token.column;
    }
    data
}
