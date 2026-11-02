//! Reads syntactic units from source code.
//! Parsers are normally created from a ParserFactory.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the JCTree results are arena TreeIds (ADR 0001); the parse
//! methods take the CompilerContext because tree creation needs the arena.

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;

/// Java: public interface Parser
pub trait Parser {
    /// Java: JCCompilationUnit parseCompilationUnit()
    fn parse_compilation_unit(&mut self, ctx: &mut CompilerContext) -> TreeId;

    /// Java: JCExpression parseExpression()
    fn parse_expression(&mut self, ctx: &mut CompilerContext) -> TreeId;

    /// Java: JCStatement parseStatement()
    fn parse_statement(&mut self, ctx: &mut CompilerContext) -> TreeId;

    /// Java: JCExpression parseType()
    fn parse_type(&mut self, ctx: &mut CompilerContext) -> TreeId;
}
