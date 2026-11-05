//! Minimal Java lexer used by the completion insertion logic.
//!
//! The compiler front end exposes tree *start* positions only (UTF-16 offsets),
//! so insertion context cannot be derived from tree ranges. This scanner walks
//! the text up to the cursor, skipping comments and literals, and records the
//! open bracket stack plus the last significant token.

/// The kind of the last significant token before the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenKind {
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    Semicolon,
    Comma,
    Equals,
    Operator,
    Question,
    Colon,
    Arrow,
    Dot,
    At,
    Return,
    New,
    InstanceOf,
    Extends,
    Implements,
    Throws,
    Catch,
    ClassKw,
    InterfaceKw,
    EnumKw,
    RecordKw,
    Import,
    Package,
    Void,
    Identifier,
    Literal,
    Keyword,
    /// Anything else (brackets, generic angle brackets, stray punctuation).
    Other,
}

/// A significant token with its UTF-16 offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub offset: usize,
}

/// Opening bracket kinds kept on the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BraceKind {
    Brace,
    Paren,
    Bracket,
}

/// One open bracket with its UTF-16 offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Brace {
    pub kind: BraceKind,
    pub offset: usize,
}

/// Result of scanning the text before the cursor.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    /// Still-open brackets, in opening order.
    pub braces: Vec<Brace>,
    /// Last significant token before the cursor.
    pub last: Option<Token>,
    /// The cursor sits inside a string/char/text-block literal or a comment.
    pub inside_literal: bool,
}

/// UTF-16 length of a text slice.
fn utf16_len(text: &str) -> usize {
    text.chars().map(|current| current.len_utf16()).sum()
}

/// Advance past a line comment starting at the given byte offset.
fn skip_line_comment(text: &str, byte: usize) -> usize {
    match text[byte..].find('\n') {
        Some(offset) => byte + offset,
        None => text.len(),
    }
}

/// Advance past a block comment starting at the given byte offset.
fn skip_block_comment(text: &str, byte: usize) -> usize {
    match text[byte + 2..].find("*/") {
        Some(offset) => byte + 2 + offset + 2,
        None => text.len(),
    }
}

/// Advance past a quoted literal (string, char or text block).
fn skip_quoted(text: &str, byte: usize, quote: char, triple: bool) -> usize {
    let width = if triple { 3 } else { 1 };
    let mut index = byte + width;
    let mut escaped = false;
    while index < text.len() {
        let current = text[index..].chars().next().unwrap_or('"');
        if escaped {
            escaped = false;
            index += current.len_utf8();
            continue;
        }
        if current == '\\' {
            escaped = true;
            index += 1;
            continue;
        }
        if triple && text[index..].starts_with("\"\"\"") {
            return index + 3;
        }
        if !triple && current == quote {
            return index + current.len_utf8();
        }
        index += current.len_utf8();
    }
    text.len()
}

fn keyword_kind(word: &str) -> TokenKind {
    match word {
        "return" => TokenKind::Return,
        "new" => TokenKind::New,
        "instanceof" => TokenKind::InstanceOf,
        "extends" => TokenKind::Extends,
        "implements" => TokenKind::Implements,
        "throws" => TokenKind::Throws,
        "catch" => TokenKind::Catch,
        "class" => TokenKind::ClassKw,
        "interface" => TokenKind::InterfaceKw,
        "enum" => TokenKind::EnumKw,
        "record" => TokenKind::RecordKw,
        "import" => TokenKind::Import,
        "package" => TokenKind::Package,
        "void" => TokenKind::Void,
        "if" | "else" | "for" | "while" | "do" | "switch" | "case" | "default"
        | "try" | "finally" | "throw" | "synchronized" | "static" | "final"
        | "public" | "private" | "protected" | "abstract" | "native" | "transient"
        | "volatile" | "strictfp" | "assert" | "yield" | "var" | "this" | "super"
        | "break" | "continue" | "boolean" | "byte" | "char" | "short" | "int"
        | "long" | "float" | "double" | "true" | "false" | "null" => TokenKind::Keyword,
        _ => TokenKind::Identifier,
    }
}

/// Scan the text up to the cursor.
pub(crate) fn scan(text: &str, cursor_utf16: usize) -> Scan {
    let mut scan = Scan::default();
    let mut byte = 0usize;
    let mut utf16 = 0usize;

    while byte < text.len() && utf16 < cursor_utf16 {
        let current = text[byte..].chars().next().unwrap_or(' ');
        let next = text.as_bytes().get(byte + 1).copied();

        // Comments.
        if current == '/' && next == Some(b'/') {
            let end = skip_line_comment(text, byte);
            utf16 += utf16_len(&text[byte..end]);
            byte = end;
            continue;
        }
        if current == '/' && next == Some(b'*') {
            let end = skip_block_comment(text, byte);
            if cursor_utf16 > utf16 && cursor_utf16 < utf16 + utf16_len(&text[byte..end]) {
                scan.inside_literal = true;
            }
            utf16 += utf16_len(&text[byte..end]);
            byte = end;
            continue;
        }
        // Text blocks and strings.
        if text[byte..].starts_with("\"\"\"") {
            let end = skip_quoted(text, byte, '"', true);
            let length = utf16_len(&text[byte..end]);
            if cursor_utf16 > utf16 && cursor_utf16 < utf16 + length {
                scan.inside_literal = true;
            }
            utf16 += length;
            byte = end;
            continue;
        }
        if current == '"' || current == '\'' {
            let end = skip_quoted(text, byte, current, false);
            let length = utf16_len(&text[byte..end]);
            if cursor_utf16 > utf16 && cursor_utf16 < utf16 + length {
                scan.inside_literal = true;
            }
            utf16 += length;
            byte = end;
            continue;
        }
        // Whitespace.
        if current.is_whitespace() {
            utf16 += current.len_utf16();
            byte += current.len_utf8();
            continue;
        }

        let offset = utf16;
        let token = match current {
            '{' => {
                scan.braces.push(Brace { kind: BraceKind::Brace, offset });
                TokenKind::OpenBrace
            }
            '}' => {
                while scan.braces.last().is_some_and(|brace| brace.kind != BraceKind::Brace) {
                    scan.braces.pop();
                }
                scan.braces.pop();
                TokenKind::CloseBrace
            }
            '(' => {
                scan.braces.push(Brace { kind: BraceKind::Paren, offset });
                TokenKind::OpenParen
            }
            ')' => {
                if scan.braces.last().is_some_and(|brace| brace.kind == BraceKind::Paren) {
                    scan.braces.pop();
                }
                TokenKind::CloseParen
            }
            '[' => {
                scan.braces.push(Brace { kind: BraceKind::Bracket, offset });
                TokenKind::Other
            }
            ']' => {
                if scan.braces.last().is_some_and(|brace| brace.kind == BraceKind::Bracket) {
                    scan.braces.pop();
                }
                TokenKind::Other
            }
            ';' => TokenKind::Semicolon,
            ',' => TokenKind::Comma,
            '?' => TokenKind::Question,
            ':' => TokenKind::Colon,
            '.' => TokenKind::Dot,
            '@' => TokenKind::At,
            '=' => {
                if next == Some(b'=') {
                    byte += 1;
                    TokenKind::Operator
                } else {
                    TokenKind::Equals
                }
            }
            '-' => {
                if next == Some(b'>') {
                    byte += 1;
                    TokenKind::Arrow
                } else {
                    TokenKind::Operator
                }
            }
            '<' | '>' | '+' | '*' | '/' | '%' | '!' | '&' | '|' | '^' | '~' => {
                TokenKind::Operator
            }
            _ if current.is_alphanumeric() || current == '_' || current == '$' => {
                let start = byte;
                let mut end = byte;
                while end < text.len() {
                    let candidate = text[end..].chars().next().unwrap_or(' ');
                    if candidate.is_alphanumeric() || candidate == '_' || candidate == '$' {
                        end += candidate.len_utf8();
                    } else {
                        break;
                    }
                }
                let word = &text[start..end];
                utf16 += utf16_len(&text[byte..end]);
                byte = end;
                let kind = if word.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    TokenKind::Literal
                } else {
                    keyword_kind(word)
                };
                scan.last = Some(Token { kind, offset });
                continue;
            }
            _ => TokenKind::Other,
        };

        scan.last = Some(Token { kind: token, offset });
        utf16 += current.len_utf16();
        byte += current.len_utf8();
    }

    scan
}
