//! Token stream adapter for completion context classification.
//!
//! The tokens come from the ported javac `Scanner` (`parser::lexer::Lexer`),
//! which is designed to keep producing tokens across incomplete source. This
//! module only adapts that stream; it contains no lexer of its own.

use crate::main::mini_compiler::MiniCompiler;
use crate::parser::lexer::Lexer;
use crate::parser::tokens::{Token, TokenKind};
use crate::util::java_string::JavaString;

/// Tokenize the whole buffer, including tokens after the cursor (needed to
/// detect a cursor inside a string/char literal).
pub(crate) fn tokenize(compiler: &MiniCompiler, text: &str) -> Vec<Token> {
    let input = JavaString::from(text);
    let mut scanner = compiler
        .parser_factory
        .scanner_factory
        .new_scanner(&input, false);
    let mut tokens = Vec::new();
    scanner.next_token();
    // The scanner emits EOF at the end; a malformed trailing token may leave
    // ERROR before EOF. Both are harmless for context classification.
    while scanner.token().kind != TokenKind::EOF {
        tokens.push(scanner.token().clone());
        scanner.next_token();
    }
    tokens
}

/// The token kind immediately before `cursor_utf16`, ignoring EOF.
pub(crate) fn last_before(tokens: &[Token], cursor_utf16: usize) -> Option<TokenKind> {
    tokens
        .iter()
        .take_while(|token| token.pos >= 0 && (token.pos as usize) < cursor_utf16)
        .filter(|token| token.kind != TokenKind::EOF)
        .last()
        .map(|token| token.kind)
}

/// True when the cursor is inside a string or character literal token.
pub(crate) fn inside_literal(tokens: &[Token], cursor_utf16: usize) -> bool {
    tokens.iter().any(|token| {
        matches!(
            token.kind,
            TokenKind::STRINGLITERAL | TokenKind::CHARLITERAL
        ) && token.pos >= 0
            && token.end_pos > token.pos
            && cursor_utf16 > token.pos as usize
            && cursor_utf16 <= token.end_pos as usize
    })
}
