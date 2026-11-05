//! The lexical analyzer maps an input stream consisting of ASCII characters
//! and Unicode escapes into a token sequence.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! the Java tokenizer field type JavaTokenizer becomes
//! Box<dyn Tokenizer> (JavaTokenizer/JavadocTokenizer are the two
//! implementations); the savedTokens List<Token> becomes Vec<Token>; Token
//! values are cloned where Java keeps references.

use std::rc::Rc;

use crate::parser::java_tokenizer::Tokenizer;
use crate::parser::lexer::Lexer;
use crate::parser::tokens::{Token, Tokens, dummy_token};
use crate::util::position::LineMapImpl;

/// The lexical analyzer (Java: public class Scanner implements Lexer).
pub struct Scanner {
    tokens: Rc<Tokens>,
    /// The token, set by nextToken().
    token: Token,
    /// The previous token, set by nextToken().
    prev_token: Token,
    /// Buffer of saved tokens (used during lookahead).
    saved_tokens: Vec<Token>,
    tokenizer: Box<dyn Tokenizer>,
}

impl Scanner {
    /// Java: protected Scanner(ScannerFactory fac, JavaTokenizer tokenizer).
    pub(crate) fn new(tokens: Rc<Tokens>, tokenizer: Box<dyn Tokenizer>) -> Scanner {
        Scanner {
            tokens,
            token: dummy_token(),
            prev_token: dummy_token(),
            saved_tokens: Vec::new(),
            tokenizer,
        }
    }

    /// Java: private void ensureLookahead(int lookahead).
    fn ensure_lookahead(&mut self, lookahead: i32) {
        let mut index = self.saved_tokens.len() as i32;
        while index < lookahead {
            self.saved_tokens.push(self.tokenizer.read_token());
            index += 1;
        }
    }
}

impl Lexer for Scanner {
    fn next_token(&mut self) {
        self.prev_token = self.token.clone();
        if !self.saved_tokens.is_empty() {
            self.token = self.saved_tokens.remove(0);
        } else {
            self.token = self.tokenizer.read_token();
        }
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn lookahead_token(&mut self, lookahead: i32) -> &Token {
        if lookahead == 0 {
            return &self.token;
        }
        self.ensure_lookahead(lookahead);
        &self.saved_tokens[(lookahead - 1) as usize]
    }

    fn prev_token(&self) -> &Token {
        &self.prev_token
    }

    fn split(&mut self) -> Token {
        let split_tokens = self.token.split(&self.tokens);
        self.prev_token = split_tokens[0].clone();
        self.token = split_tokens[1].clone();
        // Java: Scanner.split() returns the new current token (the right half);
        // the left half becomes prevToken (upstream: token = token.split();
        // return token).
        self.token.clone()
    }

    fn err_pos(&self) -> i32 {
        self.tokenizer.err_pos()
    }

    fn set_err_pos(&mut self, pos: i32) {
        self.tokenizer.set_err_pos(pos);
    }

    fn get_line_map(&self) -> LineMapImpl {
        self.tokenizer.get_line_map()
    }
}
