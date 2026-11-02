//! The lexical analyzer interface (Java: public interface Lexer).
//!
//! the Java interface becomes a Rust trait; the token(int
//! lookahead) overload becomes lookahead_token (no overloading); the inner
//! Position.LineMap type becomes position::LineMapImpl.

use crate::parser::tokens::Token;
use crate::util::position::LineMapImpl;

/// The lexical analyzer interface.
pub trait Lexer {
    /// Java: void nextToken().
    fn next_token(&mut self);

    /// Java: Token token().
    fn token(&self) -> &Token;

    /// Java: Token token(int lookahead).
    fn lookahead_token(&mut self, lookahead: i32) -> &Token;

    /// Java: Token prevToken().
    fn prev_token(&self) -> &Token;

    /// Java: Token split().
    fn split(&mut self) -> Token;

    /// Java: int errPos().
    fn err_pos(&self) -> i32;

    /// Java: void errPos(int pos).
    fn set_err_pos(&mut self, pos: i32);

    /// Java: Position.LineMap getLineMap().
    fn get_line_map(&self) -> LineMapImpl;
}
