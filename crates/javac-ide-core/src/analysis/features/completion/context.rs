//! Insertion-context classification for completion.
//!
//! Anchors come from parsed AST nodes (block/switch/array-initializer start
//! positions); the token stream comes from the ported javac Scanner. The
//! scanner keeps producing tokens for incomplete source, so this remains
//! usable while the buffer does not parse cleanly.

use crate::parser::tokens::TokenKind;

use super::token_stream;

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

/// UTF-16 offsets of AST constructs that own a brace.
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
pub(crate) fn classify(
    tokens: &[crate::parser::tokens::Token],
    cursor_utf16: usize,
    anchors: &Anchors,
    import_ranges: &[(u32, u32)],
    annotation_ranges: &[(u32, u32)],
    type_ranges: &[(u32, u32)],
) -> InsertionContext {
    if token_stream::inside_literal(tokens, cursor_utf16) {
        return InsertionContext::Unknown;
    }
    if import_ranges
        .iter()
        .any(|(start, end)| cursor_utf16 >= *start as usize && cursor_utf16 <= *end as usize)
    {
        return InsertionContext::ImportPackage;
    }
    if annotation_ranges
        .iter()
        .any(|(start, end)| cursor_utf16 >= *start as usize && cursor_utf16 <= *end as usize)
    {
        return InsertionContext::Annotation;
    }
    if type_ranges
        .iter()
        .any(|(start, end)| cursor_utf16 >= *start as usize && cursor_utf16 <= *end as usize)
    {
        return InsertionContext::TypeContext;
    }
    let last = token_stream::last_before(tokens, cursor_utf16);
    let open = tokens
        .iter()
        .take_while(|token| token.pos >= 0 && (token.pos as usize) < cursor_utf16)
        .fold(Vec::new(), |mut stack, token| {
            match token.kind {
                TokenKind::LBRACE => stack.push(token.pos as usize),
                TokenKind::RBRACE => {
                    stack.pop();
                }
                _ => {}
            }
            stack
        });
    let Some(open) = open.last().copied() else {
        return InsertionContext::TypeContext;
    };
    if anchors.is_switch(open) {
        return InsertionContext::StatementStart;
    }
    if anchors.is_array(open) {
        return InsertionContext::Expression;
    }
    if !anchors.is_block(open) {
        return InsertionContext::ClassBody;
    }
    match last {
        None | Some(TokenKind::SEMI) | Some(TokenKind::LBRACE)
        | Some(TokenKind::RBRACE) => InsertionContext::StatementStart,
        Some(TokenKind::RETURN) => InsertionContext::ReturnExpression,
        _ => InsertionContext::Expression,
    }
}
