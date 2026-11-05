//! Insertion-context classification for completion.
//!
//! Anchors come from the parsed trees (start offsets of blocks, switch bodies
//! and array initializers); the text scan provides the innermost open brace,
//! which is matched against those anchors.

use super::lexer::{self, BraceKind, Scan, TokenKind};

/// Where the completion is being inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InsertionContext {
    /// Statement position inside a block.
    StatementStart,
    /// Right after the return keyword.
    ReturnExpression,
    /// Inside an expression (arguments, assignment right-hand side, conditions).
    Expression,
    /// Class/interface/enum/record/anonymous class body.
    ClassBody,
    /// Type position (extends, implements, new, instanceof, ...).
    TypeContext,
    /// package/import declaration line.
    ImportPackage,
    /// Annotation argument.
    Annotation,
    /// Undetermined (syntax error, cursor inside a literal/comment).
    Unknown,
}

/// UTF-16 offsets of constructs that own a brace.
#[derive(Debug, Default)]
pub(crate) struct Anchors {
    pub block_starts: Vec<usize>,
    pub switch_starts: Vec<usize>,
    pub array_starts: Vec<usize>,
}

impl Anchors {
    pub(crate) fn is_block(&self, offset: usize) -> bool {
        self.block_starts.contains(&offset)
    }

    pub(crate) fn is_switch(&self, offset: usize) -> bool {
        self.switch_starts.contains(&offset)
    }

    pub(crate) fn is_array(&self, offset: usize) -> bool {
        self.array_starts.contains(&offset)
    }
}

/// Classify the insertion position at the cursor.
pub(crate) fn classify(text: &str, cursor_utf16: usize, anchors: &Anchors) -> InsertionContext {
    let scan = lexer::scan(text, cursor_utf16);
    classify_with_scan(text, cursor_utf16, anchors, &scan)
}

pub(crate) fn classify_with_scan(
    text: &str,
    cursor_utf16: usize,
    anchors: &Anchors,
    scan: &Scan,
) -> InsertionContext {
    if scan.inside_literal {
        return InsertionContext::Unknown;
    }
    if in_import_line(text, cursor_utf16) {
        return InsertionContext::ImportPackage;
    }
    let last = scan.last;
    if let Some(token) = last {
        if token.kind == TokenKind::At {
            return InsertionContext::Annotation;
        }
        if matches!(
            token.kind,
            TokenKind::Extends
                | TokenKind::Implements
                | TokenKind::New
                | TokenKind::InstanceOf
                | TokenKind::ClassKw
                | TokenKind::InterfaceKw
                | TokenKind::EnumKw
                | TokenKind::RecordKw
                | TokenKind::Throws
                | TokenKind::Catch
                | TokenKind::Void
        ) {
            return InsertionContext::TypeContext;
        }
    }
    let Some(open) = scan
        .braces
        .iter()
        .rev()
        .find(|brace| brace.kind == BraceKind::Brace)
    else {
        return InsertionContext::TypeContext;
    };
    if anchors.is_switch(open.offset) {
        return InsertionContext::StatementStart;
    }
    if anchors.is_array(open.offset) {
        return InsertionContext::Expression;
    }
    if !anchors.is_block(open.offset) {
        return InsertionContext::ClassBody;
    }
    match last.map(|token| token.kind) {
        None | Some(TokenKind::Semicolon) | Some(TokenKind::OpenBrace)
        | Some(TokenKind::CloseBrace) => InsertionContext::StatementStart,
        Some(TokenKind::Return) => InsertionContext::ReturnExpression,
        _ => InsertionContext::Expression,
    }
}

/// True when the cursor sits on a package/import declaration line.
fn in_import_line(text: &str, cursor_utf16: usize) -> bool {
    let mut start = 0usize;
    let mut utf16 = 0usize;
    for (byte, current) in text.char_indices() {
        if utf16 >= cursor_utf16 {
            break;
        }
        if current == '\n' {
            start = byte + 1;
        }
        utf16 += current.len_utf16();
    }
    let line = text[start..].split('\n').next().unwrap_or("");
    let trimmed = line.trim_start();
    trimmed.starts_with("import ") || trimmed.starts_with("package ")
}
