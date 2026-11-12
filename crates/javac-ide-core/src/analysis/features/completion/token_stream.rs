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
        .new_comment_scanner(&input);
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


/// Qualifier typed so far on the current `import` declaration line.
pub(crate) fn import_qualifier(
    tokens: &[Token],
    cursor_utf16: usize,
) -> Option<(Vec<String>, bool)> {
    use crate::parser::tokens::TokenKind;
    let mut parts = Vec::new();
    let mut static_import = false;
    let mut active = false;
    for token in tokens
        .iter()
        .take_while(|token| token.pos >= 0 && (token.pos as usize) < cursor_utf16)
    {
        match token.kind {
            TokenKind::IMPORT => {
                active = true;
                parts.clear();
                static_import = false;
            }
            TokenKind::STATIC if active => static_import = true,
            TokenKind::IDENTIFIER if active => {
                parts.push(token.name().to_string());
            }
            TokenKind::DOT if active => {}
            TokenKind::SEMI => active = false,
            _ if active => active = false,
            _ => {}
        }
    }
    active.then_some((parts, static_import))
}

/// Start of a receiver chain ending before `cursor_utf16` (the dot is skipped).
/// Only token kinds participate; no text is scanned.
pub(crate) fn receiver_start_utf16(tokens: &[Token], cursor_utf16: usize) -> Option<usize> {
    use crate::parser::tokens::TokenKind;
    let before: Vec<&Token> = tokens
        .iter()
        .take_while(|token| token.pos >= 0 && (token.pos as usize) < cursor_utf16)
        .collect();
    let mut index = before.len();
    if index > 0 && before[index - 1].kind == TokenKind::DOT {
        index -= 1;
    }
    let mut start = None;
    while index > 0 {
        let token = before[index - 1];
        match token.kind {
            TokenKind::IDENTIFIER
            | TokenKind::THIS
            | TokenKind::SUPER
            | TokenKind::CLASS => {
                start = Some(token.pos as usize);
                index -= 1;
            }
            TokenKind::RPAREN | TokenKind::RBRACKET => {
                let close = token.kind;
                let open = if close == TokenKind::RPAREN {
                    TokenKind::LPAREN
                } else {
                    TokenKind::LBRACKET
                };
                let mut depth = 1i32;
                index -= 1;
                while index > 0 && depth > 0 {
                    let candidate = before[index - 1];
                    if candidate.kind == close {
                        depth += 1;
                    } else if candidate.kind == open {
                        depth -= 1;
                    }
                    index -= 1;
                }
                if depth != 0 {
                    break;
                }
                continue;
            }
            _ => break,
        }
        while index > 0 && before[index - 1].kind == TokenKind::DOT {
            index -= 1;
        }
    }
    start
}


/// True when the token immediately before the completion prefix is part of a
/// `new Type` header (`new`, or a dotted type name after it).
pub(crate) fn after_new(tokens: &[Token], cursor_utf16: usize) -> bool {
    use crate::parser::tokens::TokenKind;
    let mut tokens = tokens
        .iter()
        .filter(|token| token.pos >= 0 && (token.pos as usize) < cursor_utf16)
        .rev();
    let mut advanced = false;
    while let Some(token) = tokens.next() {
        match token.kind {
            TokenKind::IDENTIFIER | TokenKind::DOT => advanced = true,
            TokenKind::NEW => return true,
            _ => return false,
        }
    }
    false
}




/// Comment starts and styles from the scanner's token comment list.
pub(crate) fn comment_styles(
    tokens: &[Token],
) -> Vec<(usize, crate::parser::tokens::CommentStyle)> {
    let mut comments = Vec::new();
    for token in tokens {
        let Some(list) = &token.comments else {
            continue;
        };
        for comment in list.iter() {
            let position = comment.source_pos();
            eprintln!("STYLE {:?} pos={}", comment.get_style(), position);
            if position >= 0 {
                comments.push((position as usize, comment.get_style()));
            }
        }
    }
    comments.sort_by_key(|(position, _)| *position);
    comments
}
