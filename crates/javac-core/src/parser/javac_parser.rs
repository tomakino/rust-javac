// Origin: com/sun/tools/javac/parser/JavacParser.java @ 982d53c (jdk-17.0.19-ga)

//! The parser maps a token sequence into an abstract syntax tree.
//! The parser is a hand-written recursive-descent parser that
//! implements the grammar described in the Java Language Specification.
//! For efficiency reasons, an operator precedence scheme is used
//! for parsing binary operation expressions.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES / batch status (S7a):
//! * JCTree values are arena TreeIds (ADR 0001); every parse method takes the
//!   CompilerContext because tree creation needs the arena;
//! * the Java nested Abstract/Simple/Empty EndPosTable classes are ported in
//!   this file (Simple uses util::int_hash_table::IntHashTable);
//! * keepDocComments=false path only: DocCommentParser/LazyDocCommentTable are
//!   not ported yet (TODO(DocCommentParser batch)); the doc comments of the
//!   compilation unit are None and JavadocTokenizer comments are ignored;
//! * deferred to S7b (reported): records, sealed types, patterns, switch
//!   expressions/yield, module declarations, annotation type declarations.

use std::cell::RefCell;
use std::rc::Rc;

use javac_strings::JavaString;

use crate::code::bound_kind::BoundKind;
use crate::code::source::{Feature, Source};
use crate::code::type_tag::TypeTag;
use crate::tree::tree_maker::TreeFactory;
use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::parser::lexer::Lexer;
use crate::parser::parser::Parser;
use crate::parser::parser_factory::ParserFactory;
use crate::parser::scanner::Scanner;
use crate::parser::tokens::{dummy_token, Comment, CommentStyle, Token, TokenKind, Tokens};
use crate::resources::compiler_properties::{errors, warnings};
use crate::tree::doc_comment_table::DocCommentTable;
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::jc_tree::{LiteralValue, Tag, TreeKind};
use javac_source::tree::case_tree::CaseKind;
use crate::tree::tree_info;
use crate::tree::tree_maker::TreeMaker;
use crate::util::abstract_log::AbstractLog;
use crate::util::convert;
use crate::util::int_hash_table::IntHashTable;
use crate::util::jc_diagnostic::{
    DiagnosticFlag, Error as JCDiagnosticError, SimpleDiagnosticPosition, Warning as JCDiagnosticWarning,
};
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::position::{LineMap, NOPOS};


/// Adapter: the upstream Flags class is not ported (flags.rs keeps constants
/// of the same names); the parser uses the Java long modifier values.
mod Flags {
    pub const PUBLIC: i64 = crate::code::flags::PUBLIC as i64;
    pub const PRIVATE: i64 = crate::code::flags::PRIVATE as i64;
    pub const PROTECTED: i64 = crate::code::flags::PROTECTED as i64;
    pub const STATIC: i64 = crate::code::flags::STATIC as i64;
    pub const FINAL: i64 = crate::code::flags::FINAL as i64;
    pub const SYNCHRONIZED: i64 = crate::code::flags::SYNCHRONIZED as i64;
    pub const VOLATILE: i64 = crate::code::flags::VOLATILE as i64;
    pub const TRANSIENT: i64 = crate::code::flags::TRANSIENT as i64;
    pub const NATIVE: i64 = crate::code::flags::NATIVE as i64;
    pub const INTERFACE: i64 = crate::code::flags::INTERFACE as i64;
    pub const ABSTRACT: i64 = crate::code::flags::ABSTRACT as i64;
    pub const STRICTFP: i64 = crate::code::flags::STRICTFP as i64;
    pub const ANNOTATION: i64 = crate::code::flags::ANNOTATION as i64;
    pub const ENUM: i64 = crate::code::flags::ENUM as i64;
    pub const DEPRECATED: i64 = crate::code::flags::DEPRECATED as i64;
    pub const PARAMETER: i64 = crate::code::flags::PARAMETER;
    pub const VARARGS: i64 = crate::code::flags::VARARGS;
    pub const DEFAULT: i64 = crate::code::flags::DEFAULT;
    pub const MODIFIER_FLAGS: i64 = crate::code::flags::ModifierFlags;
}
/// The number of precedence levels of infix operators.
const INFIX_PRECEDENCE_LEVELS: i32 = 10;

/// The recovery threshold, used to detect a possible infinite loop in parsing.
const RECOVERY_THRESHOLD: i32 = 50;

// When terms are parsed, the mode determines which is expected:
//     mode = EXPR        : an expression
//     mode = TYPE        : a type
//     mode = NOPARAMS    : no parameters allowed for type
//     mode = TYPEARG     : type argument
//     mode |= NOLAMBDA   : lambdas are not allowed
pub(crate) const EXPR: i32 = 0x1;
pub(crate) const TYPE: i32 = 0x2;
pub(crate) const NOPARAMS: i32 = 0x4;
pub(crate) const TYPEARG: i32 = 0x8;
pub(crate) const DIAMOND: i32 = 0x10;
pub(crate) const NOLAMBDA: i32 = 0x20;

/// Java: interface ErrorRecoveryAction (adapter: a Rust enum of the two
/// upstream actions decoded by doRecover).
#[derive(Clone, Copy)]
enum ErrorRecoveryAction {
    BlockStmt,
    CatchClause,
}

/// Java: public class JavacParser implements Parser
pub struct JavacParser {
    /// Is the parser instantiated to parse a module-info file?
    parse_module_info: bool,
    /// The scanner used for lexical analysis.
    S: Scanner,
    /// The factory to be used for abstract syntax tree construction.
    F: TreeMaker,
    /// The log to be used for error diagnostics.
    log: Rc<RefCell<Log>>,
    /// The Source language setting.
    source: Source,
    /// The Preview language setting (None: Preview.instance is not wired yet).
    preview: Option<Rc<crate::code::preview::Preview>>,
    /// The name table.
    names: Rc<Names>,
    /// End position mappings container.
    end_pos_table: Box<dyn AbstractEndPosTable>,
    /// Type annotations that have already been read but have not yet been used.
    type_annotations_pushed_back: List<TreeId>,
    /// If the parser notices extra annotations, then it either immediately
    /// issues an error (if this is false) or places the extra annotations in
    /// type_annotations_pushed_back (if this is true).
    permit_type_annotations_push_back: bool,
    /// Switch: should we fold strings?
    allow_string_folding: bool,
    /// Switch: should we keep docComments?
    keep_doc_comments: bool,
    /// Switch: should we keep line table?
    keep_line_map: bool,
    /// Switch: is "this" allowed as an identifier? Needed for receiver types.
    allow_this_ident: bool,
    /// Switch: is yield statement allowed in this source level?
    allow_yield_statement: bool,
    /// Switch: are records allowed in this source level?
    allow_records: bool,
    /// Switch: are sealed types allowed in this source level?
    allow_sealed_types: bool,
    /// The type of the method receiver, as specified by a first "this" parameter.
    receiver_param: Option<TreeId>,
    /// The current mode.
    mode: i32,
    /// The mode of the term that was parsed last.
    last_mode: i32,
    /* ---------- token management -------------- */
    /// The current token.
    token: Token,
    /* ---------- error recovery -------------- */
    /// The erroneous tree (Java: private JCErroneous errorTree).
    error_tree: TreeId,
    /// The position of the last error.
    error_pos: i32,
    /// The number of consecutive errors at the same token position.
    count: i32,
    /// The doc comments of the compilation unit (None: not wired yet).
    doc_comments: Option<Rc<dyn DocCommentTable>>,
}

/// Java: abstract EndPosTable that also tracks the last error position.
pub(crate) trait AbstractEndPosTable: EndPosTable {
    /// Java: public int errorEndPos
    fn error_end_pos(&self) -> i32;

    /// Java: public void setErrorEndPos(int errPos)
    fn set_error_end_pos(&mut self, err_pos: i32);

    /// Java: protected abstract <T extends JCTree> T to(T t)
    fn to(&mut self, t: TreeId, end_pos: i32) -> TreeId;

    /// Java: protected abstract <T extends JCTree> T toP(T t)
    fn to_p(&mut self, t: TreeId, end_pos: i32) -> TreeId;

    /// Coerce the table into the plain EndPosTable used by the compilation unit.
    fn boxed(self: Box<Self>) -> Box<dyn EndPosTable>;
}

/// Java: protected static class SimpleEndPosTable extends AbstractEndPosTable.
pub(crate) struct SimpleEndPosTable {
    /// Java: public int errorEndPos
    error_end_pos: i32,
    /// Java: private IntHashTable<JCTree> endPosMap
    end_pos_map: IntHashTable<TreeId>,
}

impl SimpleEndPosTable {
    fn new() -> SimpleEndPosTable {
        SimpleEndPosTable {
            error_end_pos: NOPOS,
            end_pos_map: IntHashTable::new(),
        }
    }
}

impl EndPosTable for SimpleEndPosTable {
    /// Java: public int getEndPos(JCTree tree)
    fn get_end_pos(&self, tree: TreeId) -> i32 {
        let value = self.end_pos_map.get(&tree);
        // As long as Position.NOPOS==-1, this just returns value.
        if value == -1 {
            NOPOS
        } else {
            value
        }
    }

    /// Java: public void storeEnd(JCTree tree, int endpos)
    fn store_end(&mut self, tree: TreeId, endpos: i32) {
        let end = if self.error_end_pos > endpos {
            self.error_end_pos
        } else {
            endpos
        };
        self.end_pos_map.put(tree, end);
    }

    /// Java: public int replaceTree(JCTree oldTree, JCTree newTree)
    fn replace_tree(&mut self, old_tree: TreeId, new_tree: TreeId) -> i32 {
        let pos = self.end_pos_map.remove(&old_tree);
        if pos != -1 {
            self.store_end(new_tree, pos);
            pos
        } else {
            NOPOS
        }
    }
}

impl AbstractEndPosTable for SimpleEndPosTable {
    fn error_end_pos(&self) -> i32 {
        self.error_end_pos
    }

    fn set_error_end_pos(&mut self, err_pos: i32) {
        if err_pos > self.error_end_pos {
            self.error_end_pos = err_pos;
        }
    }

    fn to(&mut self, t: TreeId, end_pos: i32) -> TreeId {
        self.store_end(t, end_pos);
        t
    }

    fn to_p(&mut self, t: TreeId, end_pos: i32) -> TreeId {
        self.store_end(t, end_pos);
        t
    }

    fn boxed(self: Box<Self>) -> Box<dyn EndPosTable> {
        self
    }
}

/// Java: protected static class EmptyEndPosTable extends AbstractEndPosTable.
pub(crate) struct EmptyEndPosTable {
    /// Java: public int errorEndPos
    error_end_pos: i32,
}

impl EmptyEndPosTable {
    fn new() -> EmptyEndPosTable {
        EmptyEndPosTable { error_end_pos: NOPOS }
    }
}

impl EndPosTable for EmptyEndPosTable {
    fn get_end_pos(&self, _tree: TreeId) -> i32 {
        NOPOS
    }

    fn store_end(&mut self, _tree: TreeId, _endpos: i32) {}

    fn replace_tree(&mut self, _old_tree: TreeId, _new_tree: TreeId) -> i32 {
        NOPOS
    }
}

impl AbstractEndPosTable for EmptyEndPosTable {
    fn error_end_pos(&self) -> i32 {
        self.error_end_pos
    }

    fn set_error_end_pos(&mut self, err_pos: i32) {
        if err_pos > self.error_end_pos {
            self.error_end_pos = err_pos;
        }
    }

    fn to(&mut self, t: TreeId, _end_pos: i32) -> TreeId {
        t
    }

    fn to_p(&mut self, t: TreeId, _end_pos: i32) -> TreeId {
        t
    }

    fn boxed(self: Box<Self>) -> Box<dyn EndPosTable> {
        self
    }
}

impl JavacParser {
    /// Java: protected JavacParser(ParserFactory fac, Lexer S,
    /// boolean keepDocComments, boolean keepLineMap, boolean keepEndPositions,
    /// boolean parseModuleInfo).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        fac: &ParserFactory,
        ctx: &mut CompilerContext,
        scanner: Scanner,
        keep_doc_comments: bool,
        keep_line_map: bool,
        keep_end_positions: bool,
        parse_module_info: bool,
    ) -> JavacParser {
        let mut F = TreeMaker::new(Some(fac.names.clone()));
        let error_tree = F.erroneous_empty(ctx);
        let end_pos_table: Box<dyn AbstractEndPosTable> = if keep_end_positions {
            Box::new(SimpleEndPosTable::new())
        } else {
            Box::new(EmptyEndPosTable::new())
        };
        let mut parser = JavacParser {
            parse_module_info,
            S: scanner,
            F,
            log: fac.log.clone(),
            source: fac.source,
            preview: fac.preview.clone(),
            names: fac.names.clone(),
            end_pos_table,
            type_annotations_pushed_back: List::nil(),
            permit_type_annotations_push_back: false,
            allow_string_folding: fac.options.get_boolean_or("allowStringFolding", true),
            keep_doc_comments,
            keep_line_map,
            allow_this_ident: false,
            allow_yield_statement: Feature::SWITCH_EXPRESSION.allowed_in_source(fac.source),
            allow_records: Feature::RECORDS.allowed_in_source(fac.source),
            allow_sealed_types: Feature::SEALED_CLASSES.allowed_in_source(fac.source),
            receiver_param: None,
            mode: 0,
            last_mode: 0,
            token: dummy_token(),
            error_tree,
            error_pos: NOPOS,
            count: 0,
            doc_comments: None,
        };
        // prime the pump
        parser.next_token();
        parser
    }

    /// Java: protected DocCommentTable newDocCommentTable(boolean keepDocComments,
    /// ParserFactory fac).
    ///
    /// TODO(DocCommentParser batch): LazyDocCommentTable is not ported; the
    /// keepDocComments=false path returns None (registered).
    fn new_doc_comment_table(
        &self,
        keep_doc_comments: bool,
        _fac: &ParserFactory,
    ) -> Option<Rc<dyn DocCommentTable>> {
        if keep_doc_comments {
            unimplemented!(
                "TODO(DocCommentParser batch): LazyDocCommentTable (keepDocComments=true) is not ported"
            );
        }
        None
    }

    /* ---------- token management -------------- */

    /// Java: public Token token()
    pub fn token(&self) -> &Token {
        &self.token
    }

    /// Java: public void nextToken()
    pub fn next_token(&mut self) {
        self.S.next_token();
        self.token = self.S.token().clone();
    }

    fn prev_token(&self) -> &Token {
        self.S.prev_token()
    }

    /// Java: protected boolean peekToken(Predicate<TokenKind> tk)
    fn peek_token(&mut self, kind: TokenKind) -> bool {
        self.peek_token_at(0, kind)
    }

    /// Java: protected boolean peekToken(int lookahead, Predicate<TokenKind> tk)
    fn peek_token_at(&mut self, lookahead: i32, kind: TokenKind) -> bool {
        self.S.lookahead_token(lookahead + 1).kind == kind
    }

    fn peek_token2(&mut self, kind1: TokenKind, kind2: TokenKind) -> bool {
        self.peek_token_at2(0, kind1, kind2)
    }

    fn peek_token_at2(&mut self, lookahead: i32, kind1: TokenKind, kind2: TokenKind) -> bool {
        self.S.lookahead_token(lookahead + 1).kind == kind1
            && self.S.lookahead_token(lookahead + 2).kind == kind2
    }

    fn peek_token3(&mut self, kind1: TokenKind, kind2: TokenKind, kind3: TokenKind) -> bool {
        self.peek_token_at3(0, kind1, kind2, kind3)
    }

    fn peek_token_at3(
        &mut self,
        lookahead: i32,
        kind1: TokenKind,
        kind2: TokenKind,
        kind3: TokenKind,
    ) -> bool {
        self.S.lookahead_token(lookahead + 1).kind == kind1
            && self.S.lookahead_token(lookahead + 2).kind == kind2
            && self.S.lookahead_token(lookahead + 3).kind == kind3
    }

    /* ---------- error recovery -------------- */

    /// Java: private static final int RECOVERY_THRESHOLD = 50; (const above)

    /// Java: protected void skip(boolean stopAtImport, boolean stopAtMemberDecl,
    /// boolean stopAtIdentifier, boolean stopAtStatement)
    fn skip(
        &mut self,
        stop_at_import: bool,
        stop_at_member_decl: bool,
        stop_at_identifier: bool,
        stop_at_statement: bool,
    ) {
        loop {
            match self.token.kind {
                TokenKind::SEMI => {
                    self.next_token();
                    return;
                }
                TokenKind::PUBLIC
                | TokenKind::FINAL
                | TokenKind::ABSTRACT
                | TokenKind::MONKEYS_AT
                | TokenKind::EOF
                | TokenKind::CLASS
                | TokenKind::INTERFACE
                | TokenKind::ENUM => {
                    return;
                }
                TokenKind::IMPORT => {
                    if stop_at_import {
                        return;
                    }
                }
                TokenKind::LBRACE
                | TokenKind::RBRACE
                | TokenKind::PRIVATE
                | TokenKind::PROTECTED
                | TokenKind::STATIC
                | TokenKind::TRANSIENT
                | TokenKind::NATIVE
                | TokenKind::VOLATILE
                | TokenKind::SYNCHRONIZED
                | TokenKind::STRICTFP
                | TokenKind::LT
                | TokenKind::BYTE
                | TokenKind::SHORT
                | TokenKind::CHAR
                | TokenKind::INT
                | TokenKind::LONG
                | TokenKind::FLOAT
                | TokenKind::DOUBLE
                | TokenKind::BOOLEAN
                | TokenKind::VOID => {
                    if stop_at_member_decl {
                        return;
                    }
                }
                TokenKind::UNDERSCORE | TokenKind::IDENTIFIER => {
                    if stop_at_identifier {
                        return;
                    }
                }
                TokenKind::CASE
                | TokenKind::DEFAULT
                | TokenKind::IF
                | TokenKind::FOR
                | TokenKind::WHILE
                | TokenKind::DO
                | TokenKind::TRY
                | TokenKind::SWITCH
                | TokenKind::RETURN
                | TokenKind::THROW
                | TokenKind::BREAK
                | TokenKind::CONTINUE
                | TokenKind::ELSE
                | TokenKind::FINALLY
                | TokenKind::CATCH
                | TokenKind::THIS
                | TokenKind::SUPER
                | TokenKind::NEW
                | TokenKind::ASSERT => {
                    if stop_at_statement {
                        return;
                    }
                }
                _ => {}
            }
            self.next_token();
        }
    }

    /// The presentable text of a token kind (used for the expected diagnostics).
    fn token_text(kind: TokenKind) -> String {
        match kind.name() {
            Some(text) => text.to_string(),
            None => match kind {
                TokenKind::EOF => "end of file".to_string(),
                _ => format!("{:?}", kind),
            },
        }
    }

    fn token_arg(kind: TokenKind) -> crate::resources::diagnostic_contract::TokenArg {
        crate::resources::diagnostic_contract::TokenArg::from(Self::token_text(kind))
    }

    /// Java: protected JCErroneous syntaxError(int pos, Error errorKey)
    fn syntax_error(&mut self, ctx: &mut CompilerContext, pos: i32, error_key: JCDiagnosticError) -> TreeId {
        self.syntax_error_list(ctx, pos, List::nil(), error_key)
    }

    /// Java: protected JCErroneous syntaxError(int pos, List<JCTree> errs, Error errorKey)
    fn syntax_error_list(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        errs: List<TreeId>,
        error_key: JCDiagnosticError,
    ) -> TreeId {
        self.set_error_end_pos(pos);
        let err = self.F.at(pos).erroneous(ctx, errs.clone());
        self.report_syntax_error(ctx, pos, error_key);
        if !errs.is_empty() {
            let last = errs.last().expect("non-empty");
            self.store_end(last, pos);
        }
        self.to_p(err)
    }

    /// Java: protected void reportSyntaxError(int pos, Error errorKey)
    fn report_syntax_error(&mut self, _ctx: &mut CompilerContext, pos: i32, error_key: JCDiagnosticError) {
        let preferred = pos;
        if preferred > self.S.err_pos() || preferred == NOPOS {
            let key = if self.token.kind == TokenKind::EOF {
                JCDiagnosticError::from(errors::premature_eof())
            } else {
                error_key
            };
            let position = Rc::new(SimpleDiagnosticPosition::new(preferred));
            self.log
                .borrow_mut()
                .error_flag_pos(DiagnosticFlag::SYNTAX, Some(position), key);
        }
        self.S.set_err_pos(preferred);
        if self.token.pos == self.error_pos && self.token.kind != TokenKind::EOF {
            // check for a possible infinite loop in parsing
            assert!(self.count < RECOVERY_THRESHOLD, "recovery threshold");
            self.count += 1;
        } else {
            self.count = 0;
            self.error_pos = self.token.pos;
        }
    }

    /// Java: public void accept(TokenKind tk)
    fn accept(&mut self, ctx: &mut CompilerContext, tk: TokenKind) {
        if self.token.kind == tk {
            self.next_token();
        } else {
            self.set_error_end_pos(self.token.pos);
            let end_pos = self.prev_token().end_pos;
            let key = JCDiagnosticError::from(errors::expected(Self::token_arg(tk)));
            self.report_syntax_error(ctx, end_pos, key);
        }
    }

    /// Java: JCExpression illegal(int pos)
    fn illegal(&mut self, ctx: &mut CompilerContext, pos: i32) -> TreeId {
        self.set_error_end_pos(pos);
        if (self.mode & EXPR) != 0 {
            self.syntax_error(ctx, pos, JCDiagnosticError::from(errors::illegal_start_of_expr()))
        } else {
            self.syntax_error(ctx, pos, JCDiagnosticError::from(errors::illegal_start_of_type()))
        }
    }

    /// Java: JCExpression illegal()
    fn illegal_at_token(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        self.illegal(ctx, pos)
    }

    /// Java: protected void checkNoMods(long mods)
    fn check_no_mods(&mut self, ctx: &mut CompilerContext, mods: i64) {
        let pos = self.token.pos;
        self.check_no_mods_at(ctx, pos, mods);
    }

    /// Java: protected void checkNoMods(int pos, long mods)
    fn check_no_mods_at(&mut self, ctx: &mut CompilerContext, pos: i32, mods: i64) {
        if mods != 0 {
            let lowest_mod = mods & -mods;
            let flag_text = crate::code::flags::to_java_string(lowest_mod);
            let flag_name = self.names.table.from_string(&flag_text);
            let key = JCDiagnosticError::from(errors::modifier_not_allowed_here(flag_name));
            let _ = self.syntax_error(ctx, pos, key);
        }
    }

    /// Java: protected void attach(JCTree tree, Comment dc)
    ///
    /// TODO(DocCommentParser batch): with keepDocComments=false the upstream
    /// docComments table is null and attach() is a no-op; that is the behavior
    /// kept here.
    fn attach(&mut self, _tree: TreeId, _dc: Option<Rc<dyn Comment>>) {}

    /// Java: protected void setErrorEndPos(int errPos)
    fn set_error_end_pos(&mut self, err_pos: i32) {
        self.end_pos_table.set_error_end_pos(err_pos);
    }

    /// Java: protected void storeEnd(JCTree tree, int endpos)
    fn store_end(&mut self, tree: TreeId, endpos: i32) {
        self.end_pos_table.store_end(tree, endpos);
    }

    /// Java: protected <T extends JCTree> T to(T t)
    fn to(&mut self, t: TreeId) -> TreeId {
        let end = self.token.end_pos;
        self.end_pos_table.to(t, end)
    }

    /// Java: protected <T extends JCTree> T toP(T t)
    fn to_p(&mut self, t: TreeId) -> TreeId {
        let end = self.prev_token().end_pos;
        self.end_pos_table.to_p(t, end)
    }

    /// Java: public int getStartPos(JCTree tree)
    pub fn get_start_pos(&self, ctx: &CompilerContext, tree: TreeId) -> i32 {
        tree_info::get_start_pos(ctx, tree)
    }

    /// Java: public int getEndPos(JCTree tree)
    pub fn get_end_pos(&self, tree: TreeId) -> i32 {
        self.end_pos_table.get_end_pos(tree)
    }

    /* ---------- parsing -------------- */

    /// Java: public Name ident()
    pub fn ident(&mut self, ctx: &mut CompilerContext) -> Name {
        self.ident_allow_class(ctx, false)
    }

    /// Java: protected Name ident(boolean allowClass)
    pub(crate) fn ident_allow_class(&mut self, ctx: &mut CompilerContext, allow_class: bool) -> Name {
        match self.token.kind {
            TokenKind::IDENTIFIER => {
                let name = self.token.name().clone();
                self.next_token();
                name
            }
            TokenKind::ASSERT => {
                let pos = self.token.pos;
                self.log.borrow_mut().error_flag_at(
                    DiagnosticFlag::SYNTAX,
                    pos,
                    JCDiagnosticError::from(errors::assert_as_identifier()),
                );
                self.next_token();
                self.names.error.clone()
            }
            TokenKind::ENUM => {
                let pos = self.token.pos;
                self.log.borrow_mut().error_flag_at(
                    DiagnosticFlag::SYNTAX,
                    pos,
                    JCDiagnosticError::from(errors::enum_as_identifier()),
                );
                self.next_token();
                self.names.error.clone()
            }
            TokenKind::THIS => {
                if self.allow_this_ident {
                    self.check_source_level(self.token.pos, Feature::TYPE_ANNOTATIONS);
                    let name = self.token.name().clone();
                    self.next_token();
                    name
                } else {
                    let pos = self.token.pos;
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        pos,
                        JCDiagnosticError::from(errors::this_as_identifier()),
                    );
                    self.next_token();
                    self.names.error.clone()
                }
            }
            TokenKind::UNDERSCORE => {
                let pos = self.token.pos;
                if Feature::UNDERSCORE_IDENTIFIER.allowed_in_source(self.source) {
                    let key = JCDiagnosticWarning::from(warnings::underscore_as_identifier());
                    self.log.borrow_mut().warning_at(pos, key);
                } else {
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        pos,
                        JCDiagnosticError::from(errors::underscore_as_identifier()),
                    );
                }
                let name = self.token.name().clone();
                self.next_token();
                name
            }
            _ => {
                self.accept(ctx, TokenKind::IDENTIFIER);
                if allow_class && self.token.kind == TokenKind::CLASS {
                    self.next_token();
                    return self.names._class.clone();
                }
                self.names.error.clone()
            }
        }
    }

    /// Java: public JCExpression qualident(boolean allowAnnos)
    pub(crate) fn qualident(&mut self, ctx: &mut CompilerContext, allow_annos: bool) -> TreeId {
        let pos = self.token.pos;
        let name = self.ident(ctx);
        let t = self.F.at(pos).ident(ctx, name);
        let mut t = self.to_p(t);
        while self.token.kind == TokenKind::DOT {
            let pos = self.token.pos;
            self.next_token();
            let mut ty_annos: List<TreeId> = List::nil();
            if allow_annos {
                ty_annos = self.type_annotations_opt(ctx);
            }
            let name = self.ident(ctx);
            t = self.F.at(pos).select(ctx, t, name);
            t = self.to_p(t);
            if !ty_annos.is_empty() {
                let head = ty_annos.head();
                let head_pos = tree_info::get_start_pos(ctx, *head);
                t = self.F.at(head_pos).annotated_type(ctx, ty_annos, t);
                t = self.to_p(t);
            }
        }
        t
    }

    /// Java: JCExpression literal(Name prefix)
    fn literal(&mut self, ctx: &mut CompilerContext, prefix: Name) -> TreeId {
        let pos = self.token.pos;
        self.literal_at(ctx, prefix, pos)
    }

    /// Java: JCExpression literal(Name prefix, int pos)
    fn literal_at(&mut self, ctx: &mut CompilerContext, prefix: Name, pos: i32) -> TreeId {
        let mut t = self.error_tree;
        match self.token.kind {
            TokenKind::INTLITERAL => {
                let string = self.strval(prefix);
                match convert::string2int(&string, self.token.radix()) {
                    Ok(value) => {
                        t = self.F.at(pos).literal(
                            ctx,
                            crate::code::type_tag::TypeTag::INT,
                            Some(LiteralValue::Integer(value)),
                        );
                    }
                    Err(_) => {
                        let key = JCDiagnosticError::from(errors::int_number_too_large(string));
                        self.log.borrow_mut().error_flag_at(
                            DiagnosticFlag::SYNTAX,
                            self.token.pos,
                            key,
                        );
                    }
                }
            }
            TokenKind::LONGLITERAL => {
                let string = self.strval(prefix);
                match convert::string2long(&string, self.token.radix()) {
                    Ok(value) => {
                        t = self.F.at(pos).literal(
                            ctx,
                            crate::code::type_tag::TypeTag::LONG,
                            Some(LiteralValue::Long(value)),
                        );
                    }
                    Err(_) => {
                        let key = JCDiagnosticError::from(errors::int_number_too_large(string));
                        self.log.borrow_mut().error_flag_at(
                            DiagnosticFlag::SYNTAX,
                            self.token.pos,
                            key,
                        );
                    }
                }
            }
            TokenKind::FLOATLITERAL => {
                let string_val = self.token.string_val().clone();
                let proper = if self.token.radix() == 16 {
                    JavaString::from(format!("0x{}", string_val.to_string_lossy()).as_str())
                } else {
                    string_val.clone()
                };
                let text = proper.to_string_lossy();
                let number: f32 = text.parse().unwrap_or(f32::NAN);
                let is_zero = Self::is_zero(&proper);
                if number == 0.0f32 && !is_zero {
                    let key = JCDiagnosticError::from(errors::fp_number_too_small());
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        self.token.pos,
                        key,
                    );
                } else if number == f32::INFINITY {
                    let key = JCDiagnosticError::from(errors::fp_number_too_large());
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        self.token.pos,
                        key,
                    );
                } else {
                    t = self.F.at(pos).literal(
                        ctx,
                        crate::code::type_tag::TypeTag::FLOAT,
                        Some(LiteralValue::Float(number)),
                    );
                }
            }
            TokenKind::DOUBLELITERAL => {
                let string_val = self.token.string_val().clone();
                let proper = if self.token.radix() == 16 {
                    JavaString::from(format!("0x{}", string_val.to_string_lossy()).as_str())
                } else {
                    string_val.clone()
                };
                let text = proper.to_string_lossy();
                let number: f64 = text.parse().unwrap_or(f64::NAN);
                let is_zero = Self::is_zero(&proper);
                if number == 0.0f64 && !is_zero {
                    let key = JCDiagnosticError::from(errors::fp_number_too_small());
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        self.token.pos,
                        key,
                    );
                } else if number == f64::INFINITY {
                    let key = JCDiagnosticError::from(errors::fp_number_too_large());
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        self.token.pos,
                        key,
                    );
                } else {
                    t = self.F.at(pos).literal(
                        ctx,
                        crate::code::type_tag::TypeTag::DOUBLE,
                        Some(LiteralValue::Double(number)),
                    );
                }
            }
            TokenKind::CHARLITERAL => {
                let value = self.token.string_val().as_units()[0];
                t = self.F.at(pos).literal(
                    ctx,
                    crate::code::type_tag::TypeTag::CHAR,
                    Some(LiteralValue::Character(value)),
                );
            }
            TokenKind::STRINGLITERAL => {
                let value = self.token.string_val().clone();
                t = self.F.at(pos).literal(
                    ctx,
                    crate::code::type_tag::TypeTag::CLASS,
                    Some(LiteralValue::String(value)),
                );
            }
            TokenKind::TRUE | TokenKind::FALSE => {
                let value = self.token.kind == TokenKind::TRUE;
                t = self.F.at(pos).literal(
                    ctx,
                    crate::code::type_tag::TypeTag::BOOLEAN,
                    Some(LiteralValue::Boolean(value)),
                );
            }
            TokenKind::NULL => {
                t = self
                    .F
                    .at(pos)
                    .literal(ctx, crate::code::type_tag::TypeTag::BOT, None);
            }
            _ => {
                panic!("java.lang.AssertionError in JavacParser.literal");
            }
        }
        if t == self.error_tree {
            t = self.F.at(pos).erroneous_empty(ctx);
        }
        let end = self.token.end_pos;
        self.store_end(t, end);
        self.next_token();
        t
    }

    /// Java: boolean isZero(String s) (where clause of literal)
    fn is_zero(s: &JavaString) -> bool {
        let cs = s.as_units();
        let base = if cs.len() > 1
            && char::from_u32(u32::from(cs[1])).map(|c| c.to_ascii_lowercase()) == Some('x')
        {
            16
        } else {
            10
        };
        let mut i = if base == 16 { 2 } else { 0 };
        while i < cs.len() && (cs[i] == '0' as u16 || cs[i] == '.' as u16) {
            i += 1;
        }
        !(i < cs.len()
            && char::from_u32(u32::from(cs[i]))
                .and_then(|c| c.to_digit(base))
                .map(|digit| digit > 0)
                .unwrap_or(false))
    }

    /// Java: String strval(Name prefix)
    fn strval(&self, prefix: Name) -> JavaString {
        let s = self.token.string_val().clone();
        if prefix.is_empty() {
            s
        } else {
            prefix.to_java_string().concat(&s)
        }
    }

    /// Java: public JCExpression parseExpression()
    pub fn parse_expression(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.term_with_mode(ctx, EXPR)
    }

    /// Java: public JCExpression parseType()
    pub fn parse_type(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.parse_type_impl(ctx, false)
    }

    /// Java: protected void checkSourceLevel(int pos, Feature feature)
    fn check_source_level(&mut self, pos: i32, feature: Feature) {
        match self.preview.clone() {
            Some(preview) => {
                if preview.is_preview(feature) && !preview.is_enabled() {
                    let error = preview.disabled_error(feature);
                    self.log.borrow_mut().error_at(pos, error);
                } else if !feature.allowed_in_source(self.source) {
                    let error = feature.error(self.source.name());
                    self.log
                        .borrow_mut()
                        .error_flag_at(DiagnosticFlag::SOURCE_LEVEL, pos, error);
                } else if preview.is_preview(feature) {
                    preview.warn_preview(pos, feature);
                }
            }
            None => {
                if !feature.allowed_in_source(self.source) {
                    let error = feature.error(self.source.name());
                    self.log
                        .borrow_mut()
                        .error_flag_at(DiagnosticFlag::SOURCE_LEVEL, pos, error);
                }
            }
        }
    }

    /// Java: protected JCExpression term(int newmode)
    pub(crate) fn term_with_mode(&mut self, ctx: &mut CompilerContext, newmode: i32) -> TreeId {
        let prevmode = self.mode;
        self.mode = newmode;
        let t = self.term(ctx);
        self.last_mode = self.mode;
        self.mode = prevmode;
        t
    }

    /* ---------- operator helpers -------------- */

    /// Java: protected void selectExprMode()
    fn select_expr_mode(&mut self) {
        self.mode = (self.mode & NOLAMBDA) | EXPR;
    }

    /// Java: protected void selectTypeMode()
    fn select_type_mode(&mut self) {
        self.mode = (self.mode & NOLAMBDA) | TYPE;
    }

    /// Java: lookahead token kind (Java: S.token(lookahead).kind).
    fn lookahead_kind(&mut self, lookahead: i32) -> TokenKind {
        self.S.lookahead_token(lookahead).kind
    }

    /// Java: static int earlier(int pos1, int pos2)
    fn earlier(pos1: i32, pos2: i32) -> i32 {
        if pos1 == NOPOS {
            return pos2;
        }
        if pos2 == NOPOS {
            return pos1;
        }
        if pos1 < pos2 { pos1 } else { pos2 }
    }

    /// Java: static JCTree.Tag optag(TokenKind token)
    fn optag(token: TokenKind) -> Option<Tag> {
        use TokenKind::*;
        let tag = match token {
            BARBAR => Tag::OR,
            AMPAMP => Tag::AND,
            BAR => Tag::BITOR,
            BAREQ => Tag::BITOR_ASG,
            CARET => Tag::BITXOR,
            CARETEQ => Tag::BITXOR_ASG,
            AMP => Tag::BITAND,
            AMPEQ => Tag::BITAND_ASG,
            EQEQ => Tag::EQ,
            BANGEQ => Tag::NE,
            LT => Tag::LT,
            GT => Tag::GT,
            LTEQ => Tag::LE,
            GTEQ => Tag::GE,
            LTLT => Tag::SL,
            LTLTEQ => Tag::SL_ASG,
            GTGT => Tag::SR,
            GTGTEQ => Tag::SR_ASG,
            GTGTGT => Tag::USR,
            GTGTGTEQ => Tag::USR_ASG,
            PLUS => Tag::PLUS,
            PLUSEQ => Tag::PLUS_ASG,
            SUB => Tag::MINUS,
            SUBEQ => Tag::MINUS_ASG,
            STAR => Tag::MUL,
            STAREQ => Tag::MUL_ASG,
            SLASH => Tag::DIV,
            SLASHEQ => Tag::DIV_ASG,
            PERCENT => Tag::MOD,
            PERCENTEQ => Tag::MOD_ASG,
            INSTANCEOF => Tag::TYPETEST,
            _ => return None,
        };
        Some(tag)
    }

    /// Java: static JCTree.Tag unoptag(TokenKind token)
    fn unoptag(token: TokenKind) -> Option<Tag> {
        use TokenKind::*;
        let tag = match token {
            PLUS => Tag::POS,
            SUB => Tag::NEG,
            BANG => Tag::NOT,
            TILDE => Tag::COMPL,
            PLUSPLUS => Tag::PREINC,
            SUBSUB => Tag::PREDEC,
            _ => return None,
        };
        Some(tag)
    }

    /// Java: static int prec(TokenKind token)
    fn prec(token: TokenKind) -> i32 {
        match Self::optag(token) {
            Some(tag) => tree_info::op_prec(tag),
            None => -1,
        }
    }

    /// Java: static TypeTag typetag(TokenKind token)
    fn typetag(token: TokenKind) -> TypeTag {
        use TokenKind::*;
        match token {
            BYTE => TypeTag::BYTE,
            CHAR => TypeTag::CHAR,
            SHORT => TypeTag::SHORT,
            INT => TypeTag::INT,
            LONG => TypeTag::LONG,
            FLOAT => TypeTag::FLOAT,
            DOUBLE => TypeTag::DOUBLE,
            BOOLEAN => TypeTag::BOOLEAN,
            _ => TypeTag::NONE,
        }
    }

    /// Java: static boolean isLAX_IDENTIFIER(TokenKind)
    fn is_lax_identifier(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::IDENTIFIER | TokenKind::UNDERSCORE | TokenKind::ASSERT | TokenKind::ENUM
        )
    }

    /* ---------- annotations and modifiers -------------- */

    /// Java: protected List<JCAnnotation> annotationsOpt(Tag kind)
    fn annotations_opt(&mut self, ctx: &mut CompilerContext, is_type_annotation: bool) -> List<TreeId> {
        if self.token.kind != TokenKind::MONKEYS_AT {
            return List::nil();
        }
        let mut buf: Vec<TreeId> = Vec::new();
        let prevmode = self.mode;
        while self.token.kind == TokenKind::MONKEYS_AT {
            let pos = self.token.pos;
            self.next_token();
            buf.push(self.annotation(ctx, pos, is_type_annotation));
        }
        self.last_mode = self.mode;
        self.mode = prevmode;
        List::from_iterable(buf)
    }

    /// Java: List<JCAnnotation> typeAnnotationsOpt()
    fn type_annotations_opt(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        self.annotations_opt(ctx, true)
    }

    /// Java: JCAnnotation annotation(int pos, Tag kind)
    fn annotation(&mut self, ctx: &mut CompilerContext, pos: i32, is_type_annotation: bool) -> TreeId {
        if is_type_annotation {
            self.check_source_level(pos, Feature::TYPE_ANNOTATIONS);
        }
        let ident = self.qualident(ctx, false);
        let field_values = self.annotation_field_values_opt(ctx);
        let ann = if is_type_annotation {
            self.F.at(pos).type_annotation(ctx, ident, field_values)
        } else {
            self.F.at(pos).annotation(ctx, ident, field_values)
        };
        let end = self.prev_token().end_pos;
        self.store_end(ann, end);
        ann
    }

    /// Java: List<JCExpression> annotationFieldValuesOpt()
    fn annotation_field_values_opt(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        if self.token.kind == TokenKind::LPAREN {
            self.annotation_field_values(ctx)
        } else {
            List::nil()
        }
    }

    /// Java: List<JCExpression> annotationFieldValues()
    fn annotation_field_values(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        self.accept(ctx, TokenKind::LPAREN);
        let mut buf = Vec::new();
        if self.token.kind != TokenKind::RPAREN {
            buf.push(self.annotation_field_value(ctx));
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                buf.push(self.annotation_field_value(ctx));
            }
        }
        self.accept(ctx, TokenKind::RPAREN);
        List::from_iterable(buf)
    }

    /// Java: JCExpression annotationFieldValue()
    fn annotation_field_value(&mut self, ctx: &mut CompilerContext) -> TreeId {
        if Self::is_lax_identifier(self.token.kind) {
            self.select_expr_mode();
            let t1 = self.term1(ctx);
            if ctx.tree(t1).tag == Tag::IDENT && self.token.kind == TokenKind::EQ {
                let pos = self.token.pos;
                self.accept(ctx, TokenKind::EQ);
                let value = self.annotation_value(ctx);
                return { let __to_p_tree = self.F.at(pos).assign(ctx, t1, value); self.to_p(__to_p_tree) };
            }
            return t1;
        }
        self.annotation_value(ctx)
    }

    /// Java: JCExpression annotationValue()
    fn annotation_value(&mut self, ctx: &mut CompilerContext) -> TreeId {
        match self.token.kind {
            TokenKind::MONKEYS_AT => {
                let pos = self.token.pos;
                self.next_token();
                self.annotation(ctx, pos, false)
            }
            TokenKind::LBRACE => {
                let pos = self.token.pos;
                self.accept(ctx, TokenKind::LBRACE);
                let mut buf = Vec::new();
                if self.token.kind == TokenKind::COMMA {
                    self.next_token();
                } else if self.token.kind != TokenKind::RBRACE {
                    buf.push(self.annotation_value(ctx));
                    while self.token.kind == TokenKind::COMMA {
                        self.next_token();
                        if self.token.kind == TokenKind::RBRACE {
                            break;
                        }
                        buf.push(self.annotation_value(ctx));
                    }
                }
                self.accept(ctx, TokenKind::RBRACE);
                { let __to_p_tree = self.F.at(pos).new_array(ctx, None, List::nil(), Some(List::from_iterable(buf))); self.to_p(__to_p_tree) }
            }
            _ => {
                self.select_expr_mode();
                self.term1(ctx)
            }
        }
    }

    /// Java: protected JCModifiers modifiersOpt()
    fn modifiers_opt(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.modifiers_opt_partial(ctx, None)
    }

    /// Java: protected JCModifiers modifiersOpt(JCModifiers partial)
    fn modifiers_opt_partial(&mut self, ctx: &mut CompilerContext, partial: Option<TreeId>) -> TreeId {
        let mut flags: i64 = 0;
        let mut annotations: Vec<TreeId> = Vec::new();
        let mut pos = self.token.pos;
        if let Some(partial) = partial {
            if let TreeKind::Modifiers { flags: pflags, annotations: pannos } = &ctx.tree(partial).kind {
                flags = *pflags;
                annotations.extend(pannos.to_vec());
            }
            pos = ctx.tree(partial).pos;
        }
        if self.token.deprecated_flag() {
            flags |= Flags::DEPRECATED;
        }
        'outer: loop {
            let flag: i64 = match self.token.kind {
                TokenKind::PRIVATE => Flags::PRIVATE,
                TokenKind::PROTECTED => Flags::PROTECTED,
                TokenKind::PUBLIC => Flags::PUBLIC,
                TokenKind::STATIC => Flags::STATIC,
                TokenKind::TRANSIENT => Flags::TRANSIENT,
                TokenKind::FINAL => Flags::FINAL,
                TokenKind::ABSTRACT => Flags::ABSTRACT,
                TokenKind::NATIVE => Flags::NATIVE,
                TokenKind::VOLATILE => Flags::VOLATILE,
                TokenKind::SYNCHRONIZED => Flags::SYNCHRONIZED,
                TokenKind::STRICTFP => Flags::STRICTFP,
                TokenKind::MONKEYS_AT => Flags::ANNOTATION,
                TokenKind::DEFAULT => {
                    self.check_source_level_at_token(Feature::DEFAULT_METHODS);
                    Flags::DEFAULT
                }
                TokenKind::ERROR => {
                    self.next_token();
                    continue 'outer;
                }
                _ => break 'outer,
            };
            if (flags & flag) != 0 {
                let tpos = self.token.pos;
                let key = JCDiagnosticError::from(errors::repeated_modifier());
                self.log
                    .borrow_mut()
                    .error_flag_at(DiagnosticFlag::SYNTAX, tpos, key);
            }
            let last_pos = self.token.pos;
            self.next_token();
            let mut flag = flag;
            if flag == Flags::ANNOTATION && self.token.kind != TokenKind::INTERFACE {
                let ann = self.annotation(ctx, last_pos, false);
                if flags == 0 && annotations.is_empty() {
                    pos = ctx.tree(ann).pos;
                }
                annotations.push(ann);
                flag = 0;
            }
            flags |= flag;
        }
        match self.token.kind {
            TokenKind::ENUM => flags |= Flags::ENUM,
            TokenKind::INTERFACE => flags |= Flags::INTERFACE,
            _ => {}
        }
        if (flags & (Flags::MODIFIER_FLAGS | Flags::ANNOTATION)) == 0 && annotations.is_empty() {
            pos = NOPOS;
        }
        let mods = self
            .F
            .at(pos)
            .modifiers_with_annotations(ctx, flags, List::from_iterable(annotations));
        if pos != NOPOS {
            let end = self.prev_token().end_pos;
            self.store_end(mods, end);
        }
        mods
    }

    /// Java: JCModifiers optFinal(long flags)
    fn opt_final(&mut self, ctx: &mut CompilerContext, flags: i64) -> TreeId {
        let mods = self.modifiers_opt(ctx);
        let mods_flags = match &ctx.tree(mods).kind {
            TreeKind::Modifiers { flags, .. } => *flags,
            _ => panic!("expected modifiers"),
        };
        let mods_pos = ctx.tree(mods).pos;
        self.check_no_mods_at(ctx, mods_pos, mods_flags & !(Flags::FINAL | Flags::DEPRECATED));
        if let TreeKind::Modifiers { flags: f, .. } = &mut ctx.tree_mut(mods).kind {
            *f |= flags;
        }
        mods
    }

    /// Java: Feature check without position (checkSourceLevel(Feature)).
    fn check_source_level_at_token(&mut self, feature: Feature) {
        let pos = self.token.pos;
        self.check_source_level(pos, feature);
    }

    /* ---------- type parameters / type lists -------------- */

    /// Java: protected List<JCTypeParameter> typeParametersOpt()
    fn type_parameters_opt(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        if self.token.kind == TokenKind::LT {
            let mut typarams = Vec::new();
            self.next_token();
            typarams.push(self.type_parameter(ctx));
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                typarams.push(self.type_parameter(ctx));
            }
            self.accept(ctx, TokenKind::GT);
            return List::from_iterable(typarams);
        }
        List::nil()
    }

    /// Java: JCTypeParameter typeParameter()
    fn type_parameter(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        let annos = self.type_annotations_opt(ctx);
        let name = self.type_name(ctx);
        let mut bounds = Vec::new();
        if self.token.kind == TokenKind::EXTENDS {
            self.next_token();
            bounds.push(self.parse_type(ctx));
            while self.token.kind == TokenKind::AMP {
                self.next_token();
                bounds.push(self.parse_type(ctx));
            }
        }
        let tp = self
            .F
            .at(pos)
            .type_parameter(ctx, name, List::from_iterable(bounds));
        if !annos.is_empty() {
            if let TreeKind::TypeParameter { annotations, .. } = &mut ctx.tree_mut(tp).kind {
                *annotations = annos;
            }
        }
        self.to_p(tp)
    }

    /// Java: List<JCExpression> qualidentList(boolean allowAnnos)
    fn qualident_list(&mut self, ctx: &mut CompilerContext, allow_annos: bool) -> List<TreeId> {
        let mut ts = Vec::new();
        let mut type_annos = if allow_annos { self.type_annotations_opt(ctx) } else { List::nil() };
        let mut qi = self.qualident(ctx, allow_annos);
        if !type_annos.is_empty() {
            qi = self.insert_annotations_to_most_inner(ctx, qi, type_annos, false);
        }
        ts.push(qi);
        while self.token.kind == TokenKind::COMMA {
            self.next_token();
            type_annos = if allow_annos { self.type_annotations_opt(ctx) } else { List::nil() };
            qi = self.qualident(ctx, allow_annos);
            if !type_annos.is_empty() {
                qi = self.insert_annotations_to_most_inner(ctx, qi, type_annos, false);
            }
            ts.push(qi);
        }
        List::from_iterable(ts)
    }

    /// Java: List<JCExpression> typeList()
    fn type_list(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut ts = Vec::new();
        ts.push(self.parse_type(ctx));
        while self.token.kind == TokenKind::COMMA {
            self.next_token();
            ts.push(self.parse_type(ctx));
        }
        List::from_iterable(ts)
    }

    /* ---------- type arguments -------------- */

    /// Java: JCExpression typeArgumentsOpt(JCExpression t)
    fn type_arguments_opt_expr(&mut self, ctx: &mut CompilerContext, t: TreeId) -> TreeId {
        if self.token.kind == TokenKind::LT && (self.mode & TYPE) != 0 && (self.mode & NOPARAMS) == 0 {
            self.select_type_mode();
            self.type_arguments_tree(ctx, t, false)
        } else {
            t
        }
    }

    /// Java: List<JCExpression> typeArgumentsOpt()
    fn type_arguments_opt_list(&mut self, ctx: &mut CompilerContext) -> Option<List<TreeId>> {
        self.type_arguments_opt_list_mode(ctx, TYPE)
    }

    /// Java: List<JCExpression> typeArgumentsOpt(int useMode) — None for null.
    fn type_arguments_opt_list_mode(&mut self, ctx: &mut CompilerContext, use_mode: i32) -> Option<List<TreeId>> {
        if self.token.kind == TokenKind::LT {
            if (self.mode & use_mode) == 0 || (self.mode & NOPARAMS) != 0 {
                let _ = self.illegal_at_token(ctx);
            }
            self.mode = use_mode;
            return Some(self.type_arguments_list(ctx, false));
        }
        None
    }

    /// Java: List<JCExpression> typeArguments(boolean diamondAllowed)
    fn type_arguments_list(&mut self, ctx: &mut CompilerContext, diamond_allowed: bool) -> List<TreeId> {
        if self.token.kind == TokenKind::LT {
            self.next_token();
            if self.token.kind == TokenKind::GT && diamond_allowed {
                self.check_source_level_at_token(Feature::DIAMOND);
                self.mode |= DIAMOND;
                self.next_token();
                return List::nil();
            }
            let mut args = Vec::new();
            args.push(self.type_argument(ctx));
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                args.push(self.type_argument(ctx));
            }
            match self.token.kind {
                TokenKind::GTGTGTEQ
                | TokenKind::GTGTEQ
                | TokenKind::GTEQ
                | TokenKind::GTGTGT
                | TokenKind::GTGT => {
                    self.token = self.S.split();
                }
                TokenKind::GT => self.next_token(),
                _ => {
                    let pos = self.token.pos;
                    let err = self.syntax_error(
                        ctx,
                        pos,
                        JCDiagnosticError::from(errors::expected(Self::token_arg(TokenKind::GT))),
                    );
                    args.push(err);
                }
            }
            List::from_iterable(args)
        } else {
            let pos = self.token.pos;
            let err = self.syntax_error(
                ctx,
                pos,
                JCDiagnosticError::from(errors::expected(Self::token_arg(TokenKind::LT))),
            );
            List::of(err)
        }
    }

    /// Java: JCExpression typeArgument()
    fn type_argument(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let annotations = self.type_annotations_opt(ctx);
        if self.token.kind != TokenKind::QUES {
            return self.parse_type_with_annotations(ctx, false, annotations);
        }
        let pos = self.token.pos;
        self.next_token();
        let mut result;
        if self.token.kind == TokenKind::EXTENDS {
            let kind = self.F.at(pos).type_bound_kind(ctx, BoundKind::EXTENDS);
            let t = self.to(kind);
            self.next_token();
            let bound = self.parse_type(ctx);
            result = self.F.at(pos).wildcard(ctx, t, Some(bound));
        } else if self.token.kind == TokenKind::SUPER {
            let kind = self.F.at(pos).type_bound_kind(ctx, BoundKind::SUPER);
            let t = self.to(kind);
            self.next_token();
            let bound = self.parse_type(ctx);
            result = self.F.at(pos).wildcard(ctx, t, Some(bound));
        } else if Self::is_lax_identifier(self.token.kind) {
            let t = self.F.at(NOPOS).type_bound_kind(ctx, BoundKind::UNBOUND);
            let wc = { let __to_p_tree = self.F.at(pos).wildcard(ctx, t, None); self.to_p(__to_p_tree) };
            let name = self.ident(ctx);
            let id = self.F.at(self.token.pos).ident(ctx, name);
            let id = self.to_p(id);
            result = self.F.at(pos).erroneous(ctx, List::of2(wc, id));
        } else {
            let t = self.F.at(pos).type_bound_kind(ctx, BoundKind::UNBOUND);
            let t = self.to_p(t);
            result = { let __to_p_tree = self.F.at(pos).wildcard(ctx, t, None); self.to_p(__to_p_tree) };
        }
        if !annotations.is_empty() {
            let head = annotations.head();
            let head_pos = tree_info::get_start_pos(ctx, *head);
            result = { let __to_p_tree = self.F.at(head_pos).annotated_type(ctx, annotations, result); self.to_p(__to_p_tree) };
        }
        result
    }

    /// Java: JCTypeApply typeArguments(JCExpression t, boolean diamondAllowed)
    fn type_arguments_tree(&mut self, ctx: &mut CompilerContext, t: TreeId, diamond_allowed: bool) -> TreeId {
        let pos = self.token.pos;
        let args = self.type_arguments_list(ctx, diamond_allowed);
        { let __to_p_tree = self.F.at(pos).type_apply(ctx, t, args); self.to_p(__to_p_tree) }
    }

    /// Java: public JCExpression parseType(boolean allowVar)
    fn parse_type_impl(&mut self, ctx: &mut CompilerContext, allow_var: bool) -> TreeId {
        let annotations = self.type_annotations_opt(ctx);
        self.parse_type_with_annotations(ctx, allow_var, annotations)
    }

    /// Java: public JCExpression parseType(boolean allowVar,
    /// List<JCAnnotation> annotations)
    fn parse_type_with_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        allow_var: bool,
        annotations: List<TreeId>,
    ) -> TreeId {
        let mut result = self.unannotated_type(ctx, allow_var);
        if !annotations.is_empty() {
            result = self.insert_annotations_to_most_inner(ctx, result, annotations, false);
        }
        result
    }

    /// Java: public JCExpression unannotatedType(boolean allowVar)
    fn unannotated_type(&mut self, ctx: &mut CompilerContext, allow_var: bool) -> TreeId {
        let _ = allow_var;
        self.term_with_mode(ctx, TYPE)
    }

    /* ---------- array / argument plumbing -------------- */

    /// Java: private JCExpression bracketsOpt(JCExpression t, List<JCAnnotation> annotations)
    fn brackets_opt(&mut self, ctx: &mut CompilerContext, t: TreeId, annotations: List<TreeId>) -> TreeId {
        let next_level_annotations = self.type_annotations_opt(ctx);
        let mut t = t;
        if self.token.kind == TokenKind::LBRACKET {
            let pos = self.token.pos;
            self.next_token();
            t = self.brackets_opt_cont(ctx, t, pos, next_level_annotations);
        } else if !next_level_annotations.is_empty() {
            if self.permit_type_annotations_push_back {
                self.type_annotations_pushed_back = next_level_annotations;
            } else {
                let head = next_level_annotations.head();
                let head_pos = tree_info::get_start_pos(ctx, *head);
                return self.illegal(ctx, head_pos);
            }
        }
        if !annotations.is_empty() {
            let pos = self.token.pos;
            t = { let __to_p_tree = self.F.at(pos).annotated_type(ctx, annotations, t); self.to_p(__to_p_tree) };
        }
        t
    }

    /// Java: private JCExpression bracketsOpt(JCExpression t)
    fn brackets_opt_simple(&mut self, ctx: &mut CompilerContext, t: TreeId) -> TreeId {
        self.brackets_opt(ctx, t, List::nil())
    }

    /// Java: private JCExpression bracketsOptCont(JCExpression t, int pos,
    /// List<JCAnnotation> annotations)
    fn brackets_opt_cont(&mut self, ctx: &mut CompilerContext, t: TreeId, pos: i32, annotations: List<TreeId>) -> TreeId {
        self.accept(ctx, TokenKind::RBRACKET);
        let mut t = self.brackets_opt_simple(ctx, t);
        t = { let __to_p_tree = self.F.at(pos).type_array(ctx, t); self.to_p(__to_p_tree) };
        if !annotations.is_empty() {
            t = { let __to_p_tree = self.F.at(pos).annotated_type(ctx, annotations, t); self.to_p(__to_p_tree) };
        }
        t
    }

    /// Java: JCExpression bracketsSuffix(JCExpression t)
    fn brackets_suffix(&mut self, ctx: &mut CompilerContext, t: TreeId) -> TreeId {
        if (self.mode & EXPR) != 0 && self.token.kind == TokenKind::DOT {
            self.select_expr_mode();
            let pos = self.token.pos;
            self.next_token();
            self.accept(ctx, TokenKind::CLASS);
            let class_name = self.names._class.clone();
            { let __to_p_tree = self.F.at(pos).select(ctx, t, class_name); self.to_p(__to_p_tree) }
        } else if (self.mode & TYPE) != 0 {
            if self.token.kind != TokenKind::COLCOL {
                self.select_type_mode();
            }
            t
        } else {
            t
        }
    }

    /// Java: JCPrimitiveTypeTree basicType()
    fn basic_type(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let token_kind = self.token.kind;
        let t = self.F.at(self.token.pos).type_ident(ctx, Self::typetag(token_kind));
        let t = self.to(t);
        self.next_token();
        t
    }

    /// Java: JCExpression argumentsOpt(List<JCExpression> typeArgs, JCExpression t)
    fn arguments_opt(&mut self, ctx: &mut CompilerContext, type_args: Option<List<TreeId>>, t: TreeId) -> TreeId {
        if (self.mode & EXPR) != 0 && self.token.kind == TokenKind::LPAREN || type_args.is_some() {
            self.select_expr_mode();
            self.arguments_apply(ctx, type_args.unwrap_or_else(List::nil), t)
        } else {
            t
        }
    }

    /// Java: List<JCExpression> arguments()
    fn arguments(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut args = Vec::new();
        if self.token.kind == TokenKind::LPAREN {
            self.next_token();
            if self.token.kind != TokenKind::RPAREN {
                args.push(self.parse_expression(ctx));
                while self.token.kind == TokenKind::COMMA {
                    self.next_token();
                    args.push(self.parse_expression(ctx));
                }
            }
            self.accept(ctx, TokenKind::RPAREN);
        } else {
            let pos = self.token.pos;
            let _ = self.syntax_error(
                ctx,
                pos,
                JCDiagnosticError::from(errors::expected(Self::token_arg(TokenKind::LPAREN))),
            );
        }
        List::from_iterable(args)
    }

    /// Java: JCExpression arguments(List<JCExpression> typeArgs, JCExpression t)
    fn arguments_apply(&mut self, ctx: &mut CompilerContext, type_args: List<TreeId>, t: TreeId) -> TreeId {
        let pos = self.token.pos;
        let args = self.arguments(ctx);
        let mi = self.F.at(pos).apply(ctx, type_args, t, args);
        self.to_p(mi)
    }

    /// Java: JCExpression superSuffix(List<JCExpression> typeArgs, JCExpression t)
    fn super_suffix(&mut self, ctx: &mut CompilerContext, type_args: Option<List<TreeId>>, t: TreeId) -> TreeId {
        self.next_token();
        if self.token.kind == TokenKind::LPAREN || type_args.is_some() {
            self.arguments_apply(ctx, type_args.unwrap_or_else(List::nil), t)
        } else {
            let pos = self.token.pos;
            self.accept(ctx, TokenKind::DOT);
            let ta = if self.token.kind == TokenKind::LT {
                Some(self.type_arguments_list(ctx, false))
            } else {
                None
            };
            let name = self.ident(ctx);
            let t = { let __to_p_tree = self.F.at(pos).select(ctx, t, name); self.to_p(__to_p_tree) };
            self.arguments_opt(ctx, ta, t)
        }
    }

    /// Java: JCExpression creator(int newpos, List<JCExpression> typeArgs)
    fn creator(&mut self, ctx: &mut CompilerContext, newpos: i32, type_args: Option<List<TreeId>>) -> TreeId {
        let new_annotations = self.type_annotations_opt(ctx);
        match self.token.kind {
            TokenKind::BYTE
            | TokenKind::SHORT
            | TokenKind::CHAR
            | TokenKind::INT
            | TokenKind::LONG
            | TokenKind::FLOAT
            | TokenKind::DOUBLE
            | TokenKind::BOOLEAN
                if type_args.is_none() =>
            {
                let t = self.basic_type(ctx);
                return self.array_creator_rest(ctx, newpos, t);
            }
            _ => {}
        }
        let mut t = self.qualident(ctx, true);
        let oldmode = self.mode;
        self.select_type_mode();
        let mut first = true;
        if self.token.kind == TokenKind::LT {
            t = self.type_arguments_tree(ctx, t, true);
            first = false;
        }
        while self.token.kind == TokenKind::DOT {
            let pos = self.token.pos;
            self.next_token();
            let _tyannos = self.type_annotations_opt(ctx);
            let name = self.ident(ctx);
            t = { let __to_p_tree = self.F.at(pos).select(ctx, t, name); self.to_p(__to_p_tree) };
            if self.token.kind == TokenKind::LT {
                t = self.type_arguments_tree(ctx, t, true);
            }
        }
        let _ = first;
        self.mode = oldmode;
        if self.token.kind == TokenKind::LBRACKET || self.token.kind == TokenKind::MONKEYS_AT {
            return self.array_creator_rest(ctx, newpos, t);
        }
        let args = self.arguments(ctx);
        let mut body = None;
        if self.token.kind == TokenKind::LBRACE {
            let pos = self.token.pos;
            let defs = self.class_interface_body(ctx, self.names.empty.clone(), false);
            let mods = self.F.at(NOPOS).modifiers(ctx, 0);
            let body_node = { let __to_p_tree = self.F.at(pos).anonymous_class_def(ctx, mods, defs); self.to_p(__to_p_tree) };
            body = Some(body_node);
        }
        { let __to_p_tree = self.F.at(newpos).new_class(ctx, None, type_args.unwrap_or_else(List::nil), t, args, body); self.to_p(__to_p_tree) }
    }

    /// Java: JCExpression innerCreator(int newpos, List<JCExpression> typeArgs,
    /// JCExpression encl)
    fn inner_creator(&mut self, ctx: &mut CompilerContext, newpos: i32, type_args: Option<List<TreeId>>, encl: TreeId) -> TreeId {
        let new_annotations = self.type_annotations_opt(ctx);
        let name = self.ident(ctx);
        let mut t = { let __to_p_tree = self.F.at(self.token.pos).ident(ctx, name); self.to_p(__to_p_tree) };
        if !new_annotations.is_empty() {
            let head = new_annotations.head();
            let head_pos = tree_info::get_start_pos(ctx, *head);
            t = { let __to_p_tree = self.F.at(head_pos).annotated_type(ctx, new_annotations, t); self.to_p(__to_p_tree) };
        }
        if self.token.kind == TokenKind::LT {
            let oldmode = self.mode;
            t = self.type_arguments_tree(ctx, t, true);
            self.mode = oldmode;
        }
        let args = self.arguments(ctx);
        let body = if self.token.kind == TokenKind::LBRACE {
            let pos = self.token.pos;
            let defs = self.class_interface_body(ctx, self.names.empty.clone(), false);
            let mods = self.F.at(NOPOS).modifiers(ctx, 0);
            Some({ let __to_p_tree = self.F.at(pos).anonymous_class_def(ctx, mods, defs); self.to_p(__to_p_tree) })
        } else {
            None
        };
        { let __to_p_tree = self.F.at(newpos).new_class(ctx, Some(encl), type_args.unwrap_or_else(List::nil), t, args, body); self.to_p(__to_p_tree) }
    }

    /// Java: JCExpression arrayCreatorRest(int newpos, JCExpression elemtype)
    fn array_creator_rest(&mut self, ctx: &mut CompilerContext, newpos: i32, elemtype: TreeId) -> TreeId {
        let annos = self.type_annotations_opt(ctx);
        self.accept(ctx, TokenKind::LBRACKET);
        if self.token.kind == TokenKind::RBRACKET {
            self.accept(ctx, TokenKind::RBRACKET);
            let elemtype = self.brackets_opt(ctx, elemtype, annos);
            if self.token.kind == TokenKind::LBRACE {
                return self.array_initializer(ctx, newpos, elemtype);
            }
            let t = { let __to_p_tree = self.F.at(newpos).new_array(ctx, Some(elemtype), List::nil(), None); self.to_p(__to_p_tree) };
            let pos = self.token.pos;
            return self.syntax_error_list(
                ctx,
                pos,
                List::of(t),
                JCDiagnosticError::from(errors::array_dimension_missing()),
            );
        }
        let mut dims = Vec::new();
        let mut dim_annotations: Vec<List<TreeId>> = Vec::new();
        dim_annotations.push(annos);
        dims.push(self.parse_expression(ctx));
        self.accept(ctx, TokenKind::RBRACKET);
        while self.token.kind == TokenKind::LBRACKET || self.token.kind == TokenKind::MONKEYS_AT {
            let maybe_dim_annos = self.type_annotations_opt(ctx);
            let pos = self.token.pos;
            self.next_token();
            if self.token.kind == TokenKind::RBRACKET {
                let mut elemtype = elemtype;
                elemtype = self.brackets_opt_cont(ctx, elemtype, pos, maybe_dim_annos);
                let _ = elemtype;
            } else {
                dim_annotations.push(maybe_dim_annos);
                dims.push(self.parse_expression(ctx));
                self.accept(ctx, TokenKind::RBRACKET);
            }
        }
        let mut elems: Option<List<TreeId>> = None;
        let errpos = self.token.pos;
        if self.token.kind == TokenKind::LBRACE {
            elems = Some(self.array_initializer_elements(ctx, newpos));
        }
        let na = self
            .F
            .at(newpos)
            .new_array(ctx, Some(elemtype), List::from_iterable(dims), elems);
        let na = self.to_p(na);
        if let TreeKind::NewArray { dim_annotations: field, .. } = &mut ctx.tree_mut(na).kind {
            *field = List::from_iterable(dim_annotations);
        }
        let _ = errpos;
        na
    }

    /// Java: JCExpression arrayInitializer(int newpos, JCExpression t)
    fn array_initializer(&mut self, ctx: &mut CompilerContext, newpos: i32, t: TreeId) -> TreeId {
        let elems = self.array_initializer_elements(ctx, newpos);
        { let __to_p_tree = self.F.at(newpos).new_array(ctx, Some(t), List::nil(), Some(elems)); self.to_p(__to_p_tree) }
    }

    /// Java: List<JCExpression> arrayInitializerElements(int newpos, JCExpression t)
    fn array_initializer_elements(&mut self, ctx: &mut CompilerContext, _newpos: i32) -> List<TreeId> {
        self.accept(ctx, TokenKind::LBRACE);
        let mut elems = Vec::new();
        if self.token.kind == TokenKind::COMMA {
            self.next_token();
        } else if self.token.kind != TokenKind::RBRACE {
            elems.push(self.variable_initializer(ctx));
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                if self.token.kind == TokenKind::RBRACE {
                    break;
                }
                elems.push(self.variable_initializer(ctx));
            }
        }
        self.accept(ctx, TokenKind::RBRACE);
        List::from_iterable(elems)
    }

    /// Java: public JCExpression variableInitializer()
    fn variable_initializer(&mut self, ctx: &mut CompilerContext) -> TreeId {
        if self.token.kind == TokenKind::LBRACE {
            let pos = self.token.pos;
            self.array_initializer(ctx, pos, self.error_tree)
        } else {
            self.parse_expression(ctx)
        }
    }

    /// Java: JCExpression parExpression()
    fn par_expression(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        self.accept(ctx, TokenKind::LPAREN);
        let t = self.parse_expression(ctx);
        self.accept(ctx, TokenKind::RPAREN);
        { let __to_p_tree = self.F.at(pos).parens(ctx, t); self.to_p(__to_p_tree) }
    }

    /* ---------- primary expressions -------------- */

    /// Java: protected JCExpression term()
    fn term(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let t = self.term1(ctx);
        let is_assign = self.token.kind == TokenKind::EQ || Self::is_assignop(self.token.kind);
        if (self.mode & EXPR) != 0 && is_assign {
            self.term_rest(ctx, t)
        } else {
            t
        }
    }

    fn is_assignop(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::PLUSEQ
                | TokenKind::SUBEQ
                | TokenKind::STAREQ
                | TokenKind::SLASHEQ
                | TokenKind::PERCENTEQ
                | TokenKind::AMPEQ
                | TokenKind::BAREQ
                | TokenKind::CARETEQ
                | TokenKind::LTLTEQ
                | TokenKind::GTGTEQ
                | TokenKind::GTGTGTEQ
        )
    }

    /// Java: JCExpression termRest(JCExpression t)
    fn term_rest(&mut self, ctx: &mut CompilerContext, t: TreeId) -> TreeId {
        match self.token.kind {
            TokenKind::EQ => {
                let pos = self.token.pos;
                self.next_token();
                self.select_expr_mode();
                let t1 = self.term(ctx);
                { let __to_p_tree = self.F.at(pos).assign(ctx, t, t1); self.to_p(__to_p_tree) }
            }
            kind if Self::is_assignop(kind) => {
                let pos = self.token.pos;
                let tk = self.token.kind;
                self.next_token();
                self.select_expr_mode();
                let t1 = self.term(ctx);
                let tag = Self::optag(tk).expect("assign operator");
                self.F.at(pos).assignop(ctx, tag, t, t1)
            }
            _ => t,
        }
    }

    /// Java: JCExpression term1()
    fn term1(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let t = self.term2(ctx);
        if (self.mode & EXPR) != 0 && self.token.kind == TokenKind::QUES {
            self.select_expr_mode();
            self.term1_rest(ctx, t)
        } else {
            t
        }
    }

    /// Java: JCExpression term1Rest(JCExpression t)
    fn term1_rest(&mut self, ctx: &mut CompilerContext, t: TreeId) -> TreeId {
        if self.token.kind == TokenKind::QUES {
            let pos = self.token.pos;
            self.next_token();
            let t1 = self.term(ctx);
            self.accept(ctx, TokenKind::COLON);
            let t2 = self.term1(ctx);
            self.F.at(pos).conditional(ctx, t, t1, t2)
        } else {
            t
        }
    }

    /// Java: JCExpression term2()
    fn term2(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let t = self.term3(ctx);
        if (self.mode & EXPR) != 0 && Self::prec(self.token.kind) >= tree_info::OR_PREC {
            self.select_expr_mode();
            self.term2_rest(ctx, t, tree_info::OR_PREC)
        } else {
            t
        }
    }

    /// Java: JCExpression term2Rest(JCExpression t, int minprec)
    fn term2_rest(&mut self, ctx: &mut CompilerContext, mut t: TreeId, minprec: i32) -> TreeId {
        loop {
            let p = Self::prec(self.token.kind);
            if p < minprec {
                break;
            }
            if self.token.kind == TokenKind::INSTANCEOF {
                let pos = self.token.pos;
                self.next_token();
                if self.token.kind == TokenKind::LPAREN {
                    unimplemented!("TODO(S7b): pattern matching instanceof");
                }
                let type_ = self.unannotated_type(ctx, false);
                if self.token.kind == TokenKind::IDENTIFIER {
                    unimplemented!("TODO(S7b): pattern matching instanceof");
                }
                t = self.F.at(pos).type_test(ctx, t, type_);
                continue;
            }
            let op = self.token.clone();
            let pos = op.pos;
            self.next_token();
            self.select_expr_mode();
            let mut rhs = self.term3(ctx);
            loop {
                let p2 = Self::prec(self.token.kind);
                if p2 > p {
                    rhs = self.term2_rest(ctx, rhs, p2);
                } else {
                    break;
                }
            }
            let tag = Self::optag(op.kind).expect("binary operator");
            t = self.F.at(pos).binary(ctx, tag, t, rhs);
        }
        if ctx.tree(t).tag == Tag::PLUS {
            t = self.fold_strings(ctx, t);
        }
        t
    }

    /// Java: protected JCExpression foldStrings(JCExpression tree)
    fn fold_strings(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        if !self.allow_string_folding {
            return tree;
        }
        // Flatten the left-leaning + tree into operands.
        let mut operands: Vec<TreeId> = Vec::new();
        let flatten = |ctx: &CompilerContext, id: TreeId, out: &mut Vec<TreeId>| {
            let mut stack = vec![id];
            let mut collected = Vec::new();
            while let Some(current) = stack.pop() {
                let node = ctx.tree(current);
                if node.tag == Tag::PLUS {
                    if let TreeKind::Binary { lhs, rhs, .. } = &node.kind {
                        stack.push(rhs.expect("rhs"));
                        stack.push(lhs.expect("lhs"));
                    }
                } else {
                    collected.push(current);
                }
            }
            out.extend(collected);
        };
        flatten(ctx, tree, &mut operands);
        let mut needs = false;
        let mut index = 0;
        while index + 1 < operands.len() {
            if Self::is_string_literal(ctx, operands[index]) && Self::is_string_literal(ctx, operands[index + 1]) {
                needs = true;
                break;
            }
            index += 1;
        }
        if !needs {
            return tree;
        }
        // Merge adjacent string literals.
        let mut merged: Vec<TreeId> = Vec::new();
        let mut i = 0;
        while i < operands.len() {
            if Self::is_string_literal(ctx, operands[i]) {
                let mut text = String::new();
                let mut last = operands[i];
                while i < operands.len() && Self::is_string_literal(ctx, operands[i]) {
                    last = operands[i];
                    if let TreeKind::Literal { value: Some(LiteralValue::String(s)), .. } = &ctx.tree(operands[i]).kind {
                        text.push_str(&s.to_string_lossy());
                    }
                    i += 1;
                }
                if merged.is_empty() {
                    // keep the first literal as-is when it is alone
                }
                let pos = ctx.tree(operands[i - 1]).pos;
                let lit = self
                    .F
                    .at(pos)
                    .literal(ctx, TypeTag::CLASS, Some(LiteralValue::String(JavaString::from(text.as_str()))));
                self.store_end(lit, self.get_end_pos(last));
                merged.push(lit);
            } else {
                merged.push(operands[i]);
                i += 1;
            }
        }
        let mut result = merged[0];
        for op in &merged[1..] {
            let pos = ctx.tree(*op).pos;
            result = self.F.at(pos).binary(ctx, Tag::PLUS, result, *op);
            let end = self.get_end_pos(*op);
            self.store_end(result, end);
        }
        result
    }

    fn is_string_literal(ctx: &CompilerContext, id: TreeId) -> bool {
        matches!(
            &ctx.tree(id).kind,
            TreeKind::Literal { typetag: TypeTag::CLASS, value: Some(LiteralValue::String(_)) }
        )
    }

    /// Java: protected JCExpression term3()
    fn term3(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        let mut type_args = self.type_arguments_opt_list(ctx);
        let t;
        match self.token.kind {
            TokenKind::QUES => {
                if (self.mode & TYPE) != 0 && (self.mode & (TYPEARG | NOPARAMS)) == TYPEARG {
                    self.select_type_mode();
                    return self.type_argument(ctx);
                }
                return self.illegal(ctx, pos);
            }
            TokenKind::PLUSPLUS
            | TokenKind::SUBSUB
            | TokenKind::BANG
            | TokenKind::TILDE
            | TokenKind::PLUS
            | TokenKind::SUB => {
                if type_args.is_none() && (self.mode & EXPR) != 0 {
                    let tk = self.token.kind;
                    self.next_token();
                    self.select_expr_mode();
                    if self.token.kind == TokenKind::INTLITERAL || self.token.kind == TokenKind::LONGLITERAL {
                        if self.token.radix() == 10 {
                            self.select_expr_mode();
                            let hyphen = self.names.hyphen.clone();
                            return self.literal_at(ctx, hyphen, pos);
                        }
                    }
                    let t = self.term3(ctx);
                    let tag = Self::unoptag(tk).expect("unary operator");
                    return self.F.at(pos).unary(ctx, tag, t);
                }
                return self.illegal(ctx, pos);
            }
            TokenKind::LPAREN => {
                if type_args.is_none() && (self.mode & EXPR) != 0 {
                    match self.analyze_parens(ctx) {
                        ParensResult::Cast => {
                            self.accept(ctx, TokenKind::LPAREN);
                            self.select_type_mode();
                            let pos1 = pos;
                            let mut targets = vec![self.parse_type(ctx)];
                            while self.token.kind == TokenKind::AMP {
                                self.check_source_level_at_token(Feature::INTERSECTION_TYPES_IN_CAST);
                                self.accept(ctx, TokenKind::AMP);
                                targets.push(self.parse_type(ctx));
                            }
                            let mut t = targets[0];
                            if targets.len() > 1 {
                                targets.reverse();
                                t = { let __to_p_tree = self.F.at(pos1).type_intersection(ctx, List::from_iterable(targets)); self.to_p(__to_p_tree) };
                            }
                            self.accept(ctx, TokenKind::RPAREN);
                            self.select_expr_mode();
                            let t1 = self.term3(ctx);
                            return self.F.at(pos).type_cast(ctx, t1, t);
                        }
                        ParensResult::ImplicitLambda | ParensResult::ExplicitLambda => {
                            unimplemented!("TODO(S7b): lambda expressions");
                        }
                        ParensResult::Parens => {
                            self.accept(ctx, TokenKind::LPAREN);
                            self.select_expr_mode();
                            let term3 = self.term3(ctx);
                let inner_t = self.term2_rest(ctx, term3, tree_info::OR_PREC);
                            let term1_rest = self.term1_rest(ctx, inner_t);
                let inner_t = self.term_rest(ctx, term1_rest);
                            self.accept(ctx, TokenKind::RPAREN);
                            return { let __to_p_tree = self.F.at(pos).parens(ctx, inner_t); self.to_p(__to_p_tree) };
                        }
                    }
                }
                return self.illegal(ctx, pos);
            }
            TokenKind::THIS => {
                if (self.mode & EXPR) == 0 {
                    return self.illegal(ctx, pos);
                }
                self.select_expr_mode();
                let this_name = self.names._this.clone();
                let t = { let __to_tree = self.F.at(pos).ident(ctx, this_name); self.to(__to_tree) };
                self.next_token();
                let t = if type_args.is_none() {
                    self.arguments_opt(ctx, None, t)
                } else {
                    let args = type_args.take().unwrap();
                    self.arguments_apply(ctx, args, t)
                };
                return t;
            }
            TokenKind::SUPER => {
                if (self.mode & EXPR) == 0 {
                    return self.illegal(ctx, pos);
                }
                self.select_expr_mode();
                let super_name = self.names._super.clone();
                let t = { let __to_tree = self.F.at(pos).ident(ctx, super_name); self.to(__to_tree) };
                return self.super_suffix(ctx, type_args, t);
            }
            TokenKind::INTLITERAL
            | TokenKind::LONGLITERAL
            | TokenKind::FLOATLITERAL
            | TokenKind::DOUBLELITERAL
            | TokenKind::CHARLITERAL
            | TokenKind::STRINGLITERAL
            | TokenKind::TRUE
            | TokenKind::FALSE
            | TokenKind::NULL => {
                if type_args.is_none() && (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    t = self.literal(ctx, self.names.empty.clone());
                } else {
                    return self.illegal(ctx, pos);
                }
            }
            TokenKind::NEW => {
                if type_args.is_some() {
                    return self.illegal(ctx, pos);
                }
                if (self.mode & EXPR) == 0 {
                    return self.illegal(ctx, pos);
                }
                self.select_expr_mode();
                self.next_token();
                let ta = if self.token.kind == TokenKind::LT {
                    Some(self.type_arguments_list(ctx, false))
                } else {
                    None
                };
                t = self.creator(ctx, pos, ta);
                type_args = None;
            }
            TokenKind::MONKEYS_AT => {
                let type_annos = self.type_annotations_opt(ctx);
                if type_annos.is_empty() {
                    panic!("java.lang.AssertionError: Expected type annotations");
                }
                let expr = self.term3(ctx);
                if (self.mode & TYPE) == 0 {
                    if ctx.tree(expr).tag == Tag::SELECT {
                        return self.illegal(ctx, pos);
                    }
                    return self.illegal(ctx, pos);
                }
                t = self.insert_annotations_to_most_inner(ctx, expr, type_annos, false);
            }
            TokenKind::UNDERSCORE | TokenKind::IDENTIFIER | TokenKind::ASSERT | TokenKind::ENUM => {
                if type_args.is_some() {
                    return self.illegal(ctx, pos);
                }
                if (self.mode & EXPR) != 0
                    && (self.mode & NOLAMBDA) == 0
                    && self.lookahead_kind(0) == TokenKind::ARROW
                {
                    unimplemented!("TODO(S7b): lambda expressions");
                }
                let name = self.ident(ctx);
                let mut t = { let __to_p_tree = self.F.at(self.token.pos).ident(ctx, name); self.to_p(__to_p_tree) };
                t = self.type_arguments_opt_expr(ctx, t);
                return self.term3_rest(ctx, t, None);
            }
            TokenKind::BYTE
            | TokenKind::SHORT
            | TokenKind::CHAR
            | TokenKind::INT
            | TokenKind::LONG
            | TokenKind::FLOAT
            | TokenKind::DOUBLE
            | TokenKind::BOOLEAN => {
                if type_args.is_some() {
                    let _ = self.illegal(ctx, pos);
                }
                let basic_type = self.basic_type(ctx);
                let brackets = self.brackets_opt_simple(ctx, basic_type);
                t = self.brackets_suffix(ctx, brackets);
            }
            TokenKind::VOID => {
                if type_args.is_some() {
                    let _ = self.illegal(ctx, pos);
                }
                if (self.mode & EXPR) != 0 {
                    self.next_token();
                    if self.token.kind == TokenKind::DOT {
                        let ti = { let __to_p_tree = self.F.at(pos).type_ident(ctx, TypeTag::VOID); self.to_p(__to_p_tree) };
                        t = self.brackets_suffix(ctx, ti);
                    } else {
                        return self.illegal(ctx, pos);
                    }
                } else {
                    let ti = { let __to_tree = self.F.at(pos).type_ident(ctx, TypeTag::VOID); self.to(__to_tree) };
                    self.next_token();
                    return ti;
                }
            }
            _ => return self.illegal(ctx, pos),
        }
        self.term3_rest(ctx, t, type_args.take())
    }

    /// Java: JCExpression term3Rest(JCExpression t, List<JCExpression> typeArgs)
    fn term3_rest(&mut self, ctx: &mut CompilerContext, mut t: TreeId, type_args: Option<List<TreeId>>) -> TreeId {
        if type_args.is_some() {
            let _ = self.illegal(ctx, ctx.tree(t).pos);
        }
        loop {
            let pos1 = self.token.pos;
            let annos = self.type_annotations_opt(ctx);
            if self.token.kind == TokenKind::LBRACKET {
                self.next_token();
                if (self.mode & TYPE) != 0 {
                    let oldmode = self.mode;
                    self.select_type_mode();
                    if self.token.kind == TokenKind::RBRACKET {
                        self.next_token();
                        t = self.brackets_opt_simple(ctx, t);
                        let t2 = { let __to_p_tree = self.F.at(pos1).type_array(ctx, t); self.to_p(__to_p_tree) };
                        if self.token.kind == TokenKind::COLCOL {
                            self.select_expr_mode();
                            t = t2;
                            continue;
                        }
                        if !annos.is_empty() {
                            return { let __to_p_tree = self.F.at(pos1).annotated_type(ctx, annos, t2); self.to_p(__to_p_tree) };
                        }
                        return t2;
                    }
                    self.mode = oldmode;
                }
                if (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    let t1 = self.term(ctx);
                    t = { let __to_tree = self.F.at(pos1).indexed(ctx, t, t1); self.to(__to_tree) };
                }
                self.accept(ctx, TokenKind::RBRACKET);
            } else if self.token.kind == TokenKind::DOT {
                self.next_token();
                let mut ta = self.type_arguments_opt_list(ctx);
                if self.token.kind == TokenKind::SUPER && (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    let super_name = self.names._super.clone();
                    t = { let __to_tree = self.F.at(pos1).select(ctx, t, super_name); self.to(__to_tree) };
                    self.next_token();
                    t = self.arguments_apply(ctx, ta.take().unwrap_or_else(List::nil), t);
                    ta = None;
                } else if self.token.kind == TokenKind::NEW && (self.mode & EXPR) != 0 {
                    if ta.is_some() {
                        return self.illegal(ctx, pos1);
                    }
                    self.select_expr_mode();
                    let pos2 = self.token.pos;
                    self.next_token();
                    let ta2 = if self.token.kind == TokenKind::LT {
                        Some(self.type_arguments_list(ctx, false))
                    } else {
                        None
                    };
                    t = self.inner_creator(ctx, pos2, ta2, t);
                    ta = None;
                } else {
                    let tyannos = self.type_annotations_opt(ctx);
                    let name = self.ident_allow_class(ctx, true);
                    t = { let __to_p_tree = self.F.at(pos1).select(ctx, t, name); self.to_p(__to_p_tree) };
                    if !tyannos.is_empty() {
                        let head = tyannos.head();
                        let head_pos = tree_info::get_start_pos(ctx, *head);
                        t = { let __to_p_tree = self.F.at(head_pos).annotated_type(ctx, tyannos, t); self.to_p(__to_p_tree) };
                    }
                    let t2 = self.type_arguments_opt_expr(ctx, t);
                    t = self.arguments_opt(ctx, ta.take(), t2);
                    ta = None;
                }
            } else if (self.mode & EXPR) != 0 && self.token.kind == TokenKind::COLCOL {
                unimplemented!("TODO(S7b): method references");
            } else {
                if !annos.is_empty() {
                    if self.permit_type_annotations_push_back {
                        self.type_annotations_pushed_back = annos;
                    } else {
                        let head = annos.head();
                        let head_pos = tree_info::get_start_pos(ctx, *head);
                        t = self.illegal(ctx, head_pos);
                    }
                }
                break;
            }
        }
        while (self.token.kind == TokenKind::PLUSPLUS || self.token.kind == TokenKind::SUBSUB) && (self.mode & EXPR) != 0 {
            self.select_expr_mode();
            let pos = self.token.pos;
            let tag = if self.token.kind == TokenKind::PLUSPLUS { Tag::POSTINC } else { Tag::POSTDEC };
            t = { let __to_tree = self.F.at(pos).unary(ctx, tag, t); self.to(__to_tree) };
            self.next_token();
        }
        self.to_p(t)
    }

    /* ---------- parens analysis -------------- */

    /// Java: ParensResult analyzeParens()
    fn analyze_parens(&mut self, _ctx: &mut CompilerContext) -> ParensResult {
        let mut depth = 0i32;
        let mut type_seen = false;
        let mut default_result = ParensResult::Parens;
        let mut lookahead = 0i32;
        loop {
            match self.lookahead_kind(lookahead) {
                TokenKind::COMMA => type_seen = true,
                TokenKind::EXTENDS | TokenKind::SUPER | TokenKind::DOT | TokenKind::AMP => {}
                TokenKind::QUES => {
                    if self.lookahead_kind(lookahead + 1) == TokenKind::EXTENDS
                        || self.lookahead_kind(lookahead + 1) == TokenKind::SUPER
                    {
                        type_seen = true;
                    }
                }
                TokenKind::BYTE
                | TokenKind::SHORT
                | TokenKind::INT
                | TokenKind::LONG
                | TokenKind::FLOAT
                | TokenKind::DOUBLE
                | TokenKind::BOOLEAN
                | TokenKind::CHAR
                | TokenKind::VOID => {
                    if self.lookahead_kind(lookahead + 1) == TokenKind::RPAREN {
                        return ParensResult::Cast;
                    }
                    if Self::is_lax_identifier(self.lookahead_kind(lookahead + 1)) {
                        return ParensResult::ExplicitLambda;
                    }
                }
                TokenKind::LPAREN => {
                    if lookahead != 0 {
                        return ParensResult::Parens;
                    }
                    if self.lookahead_kind(lookahead + 1) == TokenKind::RPAREN {
                        return ParensResult::ExplicitLambda;
                    }
                }
                TokenKind::RPAREN => {
                    if type_seen {
                        return ParensResult::Cast;
                    }
                    match self.lookahead_kind(lookahead + 1) {
                        TokenKind::BANG
                        | TokenKind::TILDE
                        | TokenKind::LPAREN
                        | TokenKind::THIS
                        | TokenKind::SUPER
                        | TokenKind::INTLITERAL
                        | TokenKind::LONGLITERAL
                        | TokenKind::FLOATLITERAL
                        | TokenKind::DOUBLELITERAL
                        | TokenKind::CHARLITERAL
                        | TokenKind::STRINGLITERAL
                        | TokenKind::TRUE
                        | TokenKind::FALSE
                        | TokenKind::NULL
                        | TokenKind::NEW
                        | TokenKind::IDENTIFIER
                        | TokenKind::ASSERT
                        | TokenKind::ENUM
                        | TokenKind::UNDERSCORE
                        | TokenKind::SWITCH
                        | TokenKind::BYTE
                        | TokenKind::SHORT
                        | TokenKind::CHAR
                        | TokenKind::INT
                        | TokenKind::LONG
                        | TokenKind::FLOAT
                        | TokenKind::DOUBLE
                        | TokenKind::BOOLEAN
                        | TokenKind::VOID => return ParensResult::Cast,
                        _ => return default_result,
                    }
                }
                TokenKind::UNDERSCORE | TokenKind::ASSERT | TokenKind::ENUM | TokenKind::IDENTIFIER => {
                    if self.lookahead_kind(lookahead + 1) == TokenKind::IDENTIFIER {
                        return ParensResult::ExplicitLambda;
                    } else if self.lookahead_kind(lookahead + 1) == TokenKind::RPAREN
                        && self.lookahead_kind(lookahead + 2) == TokenKind::ARROW
                    {
                        return if (self.mode & NOLAMBDA) == 0 {
                            ParensResult::ImplicitLambda
                        } else {
                            ParensResult::Parens
                        };
                    } else if depth == 0 && self.lookahead_kind(lookahead + 1) == TokenKind::COMMA {
                        default_result = ParensResult::ImplicitLambda;
                    }
                    type_seen = false;
                }
                TokenKind::FINAL | TokenKind::ELLIPSIS => return ParensResult::ExplicitLambda,
                TokenKind::MONKEYS_AT => {
                    type_seen = true;
                    lookahead = self.skip_annotation(lookahead);
                }
                TokenKind::LBRACKET => {
                    if self.lookahead_kind(lookahead + 1) == TokenKind::RBRACKET
                        && Self::is_lax_identifier(self.lookahead_kind(lookahead + 2))
                    {
                        return ParensResult::ExplicitLambda;
                    } else if self.lookahead_kind(lookahead + 1) == TokenKind::RBRACKET
                        && (self.lookahead_kind(lookahead + 2) == TokenKind::RPAREN
                            || self.lookahead_kind(lookahead + 2) == TokenKind::AMP)
                    {
                        return ParensResult::Cast;
                    } else if self.lookahead_kind(lookahead + 1) == TokenKind::RBRACKET {
                        type_seen = true;
                        lookahead += 1;
                    } else {
                        return ParensResult::Parens;
                    }
                }
                TokenKind::LT => depth += 1,
                TokenKind::GTGTGT => depth -= 1,
                TokenKind::GTGT => depth -= 1,
                TokenKind::GT => {
                    depth -= 1;
                    if depth == 0 {
                        if self.lookahead_kind(lookahead + 1) == TokenKind::RPAREN
                            || self.lookahead_kind(lookahead + 1) == TokenKind::AMP
                        {
                            return ParensResult::Cast;
                        }
                        if self.lookahead_kind(lookahead + 1) == TokenKind::ELLIPSIS {
                            return ParensResult::ExplicitLambda;
                        }
                        type_seen = true;
                    } else if depth < 0 {
                        return ParensResult::Parens;
                    }
                }
                _ => return default_result,
            }
            lookahead += 1;
        }
    }

    fn skip_annotation(&mut self, lookahead: i32) -> i32 {
        let mut lookahead = lookahead + 1;
        while self.lookahead_kind(lookahead) == TokenKind::DOT {
            lookahead += 2;
        }
        if self.lookahead_kind(lookahead) == TokenKind::LPAREN {
            let mut nesting = 0i32;
            loop {
                match self.lookahead_kind(lookahead) {
                    TokenKind::EOF => return lookahead,
                    TokenKind::LPAREN => nesting += 1,
                    TokenKind::RPAREN => {
                        nesting -= 1;
                        if nesting == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                lookahead += 1;
            }
        }
        lookahead
    }

    /* ---------- remainder appended by the next batch ---------- */
    /* ---------- declarations & statements -------------- */

    /// Java: private JCExpression insertAnnotationsToMostInner(...)
    fn insert_annotations_to_most_inner(
        &mut self,
        ctx: &mut CompilerContext,
        type_: TreeId,
        annos: List<TreeId>,
        create_new_level: bool,
    ) -> TreeId {
        let orig_end = self.get_end_pos(type_);
        let mut most_inner = type_;
        let mut last_array: Option<TreeId> = None;
        loop {
            let elem = match &ctx.tree(most_inner).kind {
                TreeKind::ArrayTypeTree { elemtype } => *elemtype,
                _ => break,
            };
            last_array = Some(most_inner);
            most_inner = elem.expect("array element type");
        }
        let mut orig_end = orig_end;
        if create_new_level {
            let pos = self.token.pos;
            let new_array = { let __to_tree = self.F.at(pos).type_array(ctx, most_inner); self.to(__to_tree) };
            most_inner = new_array;
            orig_end = self.get_end_pos(most_inner);
        }
        let mut most_inner_to_return = most_inner;
        if !annos.is_empty() {
            let mut last_to_modify = most_inner;
            loop {
                let next = match &ctx.tree(most_inner).kind {
                    TreeKind::FieldAccess { selected, .. } => *selected,
                    TreeKind::TypeApply { clazz, .. } => *clazz,
                    _ => break,
                };
                last_to_modify = most_inner;
                most_inner = next.expect("type child");
            }
            let head = annos.head();
            let head_pos = tree_info::get_start_pos(ctx, *head);
            let annotated = self.F.at(head_pos).annotated_type(ctx, annos.clone(), most_inner);
            let annotated = self.to_p(annotated);
            match &mut ctx.tree_mut(last_to_modify).kind {
                TreeKind::TypeApply { clazz, .. } => {
                    *clazz = Some(annotated);
                }
                TreeKind::FieldAccess { selected, .. } => {
                    *selected = Some(annotated);
                }
                _ => {
                    most_inner_to_return = annotated;
                }
            }
        }
        match last_array {
            None => most_inner_to_return,
            Some(array) => {
                if let TreeKind::ArrayTypeTree { elemtype } = &mut ctx.tree_mut(array).kind {
                    *elemtype = Some(most_inner_to_return);
                }
                self.store_end(type_, orig_end);
                type_
            }
        }
    }

    /// Java: Name typeName()
    fn type_name(&mut self, ctx: &mut CompilerContext) -> Name {
        self.ident(ctx)
    }

    /* ---------- variables -------------- */

    /// Java: public <T extends ListBuffer<? super JCVariableDecl>> T variableDeclarators(
    /// JCModifiers mods, JCExpression type, T vdefs, boolean localDecl)
    fn variable_declarators(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        type_: TreeId,
        mut vdefs: Vec<TreeId>,
        local_decl: bool,
    ) -> Vec<TreeId> {
        let pos = self.token.pos;
        let name = self.ident(ctx);
        vdefs = self.variable_declarators_rest(ctx, pos, mods, type_, name, false, vdefs, local_decl);
        vdefs
    }

    /// Java: protected <T extends ListBuffer<? super JCVariableDecl>> T variableDeclaratorsRest(...)
    #[allow(clippy::too_many_arguments)]
    fn variable_declarators_rest(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        mods: TreeId,
        type_: TreeId,
        name: Name,
        req_init: bool,
        mut vdefs: Vec<TreeId>,
        local_decl: bool,
    ) -> Vec<TreeId> {
        let head = self.variable_declarator_rest(ctx, pos, mods, type_, name, req_init, local_decl, false);
        vdefs.push(head);
        while self.token.kind == TokenKind::COMMA {
            let last = *vdefs.last().expect("declarator");
            let end = self.token.end_pos;
            self.store_end(last, end);
            self.next_token();
            let pos = self.token.pos;
            let name = self.ident(ctx);
            let def = self.variable_declarator_rest(ctx, pos, mods, type_, name, req_init, local_decl, true);
            vdefs.push(def);
        }
        vdefs
    }

    /// Java: JCVariableDecl variableDeclaratorRest(int pos, JCModifiers mods,
    /// JCExpression type, Name name, boolean reqInit, Comment dc,
    /// boolean localDecl, boolean compound)
    #[allow(clippy::too_many_arguments)]
    fn variable_declarator_rest(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        mods: TreeId,
        type_: TreeId,
        name: Name,
        req_init: bool,
        _local_decl: bool,
        _compound: bool,
    ) -> TreeId {
        let type_ = self.brackets_opt_simple(ctx, type_);
        let mut init = None;
        if self.token.kind == TokenKind::EQ {
            self.next_token();
            init = Some(self.variable_initializer(ctx));
        } else if req_init {
            let p = self.token.pos;
            let _ = self.syntax_error(
                ctx,
                p,
                JCDiagnosticError::from(errors::expected(Self::token_arg(TokenKind::EQ))),
            );
        }
        let def = { let __to_p_tree = self.F.at(pos).var_def(ctx, mods, name, Some(type_), init); self.to_p(__to_p_tree) };
        def
    }

    /// Java: JCVariableDecl variableDeclaratorId(JCModifiers mods, JCExpression type)
    fn variable_declarator_id(&mut self, ctx: &mut CompilerContext, mods: TreeId, type_: TreeId) -> TreeId {
        let pos = self.token.pos;
        let name = self.ident(ctx);
        let type_ = self.brackets_opt_simple(ctx, type_);
        { let __to_p_tree = self.F.at(pos).var_def(ctx, mods, name, Some(type_), None); self.to_p(__to_p_tree) }
    }

    /* ---------- blocks and statements -------------- */

    /// Java: JCBlock block(int pos, long flags)
    fn block_at(&mut self, ctx: &mut CompilerContext, pos: i32, flags: i64) -> TreeId {
        self.accept(ctx, TokenKind::LBRACE);
        let stats = self.block_statements(ctx);
        let t = self.F.at(pos).block(ctx, flags, stats);
        let _ = self.syntax_error_discard(ctx);
        let endpos = self.token.pos;
        if let TreeKind::Block { endpos: field, .. } = &mut ctx.tree_mut(t).kind {
            *field = endpos;
        }
        self.accept(ctx, TokenKind::RBRACE);
        self.to_p(t)
    }

    /// Java: public JCBlock block()
    fn block(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        self.block_at(ctx, pos, 0)
    }

    /// Java: private JCErroneous syntaxError discard used by block (orphaned cases).
    fn syntax_error_discard(&mut self, ctx: &mut CompilerContext) -> Option<TreeId> {
        if self.token.kind == TokenKind::CASE || self.token.kind == TokenKind::DEFAULT {
            let pos = self.token.pos;
            let _ = self.syntax_error(
                ctx,
                pos,
                JCDiagnosticError::from(errors::orphaned(Self::token_arg(self.token.kind))),
            );
            while self.token.kind == TokenKind::CASE || self.token.kind == TokenKind::DEFAULT {
                let _ = self.switch_block_statement_group(ctx);
            }
        }
        None
    }

    /// Java: List<JCStatement> blockStatements()
    fn block_statements(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut stats: Vec<TreeId> = Vec::new();
        let mut last_err_pos = -1i32;
        loop {
            let stat = self.block_statement(ctx);
            if stat.is_empty() {
                return List::from_iterable(stats);
            }
            if self.token.pos == last_err_pos {
                return List::from_iterable(stats);
            }
            if self.token.pos <= self.end_pos_table.error_end_pos() {
                self.skip(false, true, true, true);
                last_err_pos = self.token.pos;
            }
            stats.extend(stat.iter());
        }
    }

    /// Java: JCStatement parseStatementAsBlock()
    fn parse_statement_as_block(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        let stats = self.block_statement(ctx);
        if stats.is_empty() {
            let e = self.syntax_error(
                ctx,
                pos,
                JCDiagnosticError::from(errors::illegal_start_of_stmt()),
            );
            return { let __to_p_tree = self.F.at(pos).exec(ctx, e); self.to_p(__to_p_tree) };
        }
        let first = *stats.head();
        let error = match ctx.tree(first).tag {
            Tag::CLASSDEF => Some(JCDiagnosticError::from(errors::class_not_allowed())),
            Tag::VARDEF => Some(JCDiagnosticError::from(errors::variable_not_allowed())),
            _ => None,
        };
        if let Some(error) = error {
            self.log
                .borrow_mut()
                .error_flag_at(DiagnosticFlag::SYNTAX, ctx.tree(first).pos, error);
            let block = self.F.at(ctx.tree(first).pos).block(ctx, 0, stats);
            let block = self.to_p(block);
            let err = self.F.at(ctx.tree(first).pos).erroneous(ctx, List::of(block));
            return { let __to_p_tree = self.F.at(pos).exec(ctx, err); self.to_p(__to_p_tree) };
        }
        first
    }

    /// Java: List<JCStatement> blockStatement()
    fn block_statement(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let pos = self.token.pos;
        match self.token.kind {
            TokenKind::RBRACE | TokenKind::CASE | TokenKind::DEFAULT | TokenKind::EOF => List::nil(),
            TokenKind::LBRACE
            | TokenKind::IF
            | TokenKind::FOR
            | TokenKind::WHILE
            | TokenKind::DO
            | TokenKind::TRY
            | TokenKind::SWITCH
            | TokenKind::SYNCHRONIZED
            | TokenKind::RETURN
            | TokenKind::THROW
            | TokenKind::BREAK
            | TokenKind::CONTINUE
            | TokenKind::SEMI
            | TokenKind::ELSE
            | TokenKind::FINALLY
            | TokenKind::CATCH
            | TokenKind::ASSERT => List::of(self.parse_simple_statement(ctx)),
            TokenKind::MONKEYS_AT | TokenKind::FINAL => {
                let mods = self.modifiers_opt(ctx);
                if self.token.kind == TokenKind::CLASS
                    || self.token.kind == TokenKind::INTERFACE
                    || self.token.kind == TokenKind::ENUM
                {
                    return List::of(self.class_or_interface_declaration(ctx, mods));
                }
                let type_ = self.parse_type_impl(ctx, true);
                List::from_iterable(self.local_variable_declarations(ctx, mods, type_))
            }
            TokenKind::ABSTRACT | TokenKind::STRICTFP => {
                let mods = self.modifiers_opt(ctx);
                List::of(self.class_or_interface_declaration(ctx, mods))
            }
            TokenKind::INTERFACE | TokenKind::CLASS => {
                let mods = self.modifiers_opt(ctx);
                List::of(self.class_or_interface_declaration(ctx, mods))
            }
            TokenKind::ENUM => {
                let mods = self.modifiers_opt(ctx);
                List::of(self.class_or_interface_declaration(ctx, mods))
            }
            _ => {
                // Label, local variable declaration or expression statement.
                if Self::is_lax_identifier(self.token.kind)
                    && self.lookahead_kind(1) == TokenKind::COLON
                {
                    let name = self.ident(ctx);
                    self.accept(ctx, TokenKind::COLON);
                    let stat = self.parse_statement_as_block(ctx);
                    return List::of(self.F.at(pos).labelled(ctx, name, stat));
                }
                if self.looks_like_local_var_decl() {
                    let type_ = self.parse_type_impl(ctx, true);
                    let mods = self.F.at(NOPOS).modifiers(ctx, 0);
                    return List::from_iterable(self.local_variable_declarations(ctx, mods, type_));
                }
                let t = self.parse_expression(ctx);
                let t = self.check_expr_stat(ctx, t);
                self.accept(ctx, TokenKind::SEMI);
                let exec = { let __to_p_tree = self.F.at(pos).exec(ctx, t); self.to_p(__to_p_tree) };
                List::of(exec)
            }
        }
    }

    /// Heuristic used for local variable declarations in block context
    /// (Java: term(EXPR|TYPE) + lastmode disambiguation).
    fn looks_like_local_var_decl(&mut self) -> bool {
        let mut lookahead = 0i32;
        if Self::is_basic_type(self.lookahead_kind(lookahead)) {
            return true;
        }
        if !Self::is_lax_identifier(self.lookahead_kind(lookahead)) {
            return false;
        }
        lookahead += 1;
        // skip a qualified name and optional type arguments
        loop {
            match self.lookahead_kind(lookahead) {
                TokenKind::DOT => {
                    lookahead += 1;
                    if !Self::is_lax_identifier(self.lookahead_kind(lookahead)) {
                        return false;
                    }
                    lookahead += 1;
                }
                TokenKind::LT => {
                    lookahead = self.skip_type_arguments(lookahead);
                }
                _ => break,
            }
        }
        // skip array levels
        while self.lookahead_kind(lookahead) == TokenKind::LBRACKET
            && self.lookahead_kind(lookahead + 1) == TokenKind::RBRACKET
        {
            lookahead += 2;
        }
        Self::is_lax_identifier(self.lookahead_kind(lookahead))
    }

    fn is_basic_type(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::BYTE
                | TokenKind::SHORT
                | TokenKind::CHAR
                | TokenKind::INT
                | TokenKind::LONG
                | TokenKind::FLOAT
                | TokenKind::DOUBLE
                | TokenKind::BOOLEAN
        )
    }

    /// Skip a balanced type argument list starting at '<' and return the index
    /// after the matching '>'.
    fn skip_type_arguments(&mut self, mut lookahead: i32) -> i32 {
        let mut depth = 0i32;
        loop {
            match self.lookahead_kind(lookahead) {
                TokenKind::LT => depth += 1,
                TokenKind::GT => depth -= 1,
                TokenKind::GTGT => depth -= 2,
                TokenKind::GTGTGT => depth -= 3,
                TokenKind::EOF => return lookahead,
                _ => {}
            }
            lookahead += 1;
            if depth <= 0 {
                return lookahead;
            }
        }
    }

    /// Java: private List<JCStatement> localVariableDeclarations(JCModifiers mods,
    /// JCExpression type)
    fn local_variable_declarations(&mut self, ctx: &mut CompilerContext, mods: TreeId, type_: TreeId) -> Vec<TreeId> {
        let stats = self.variable_declarators(ctx, mods, type_, Vec::new(), true);
        self.accept(ctx, TokenKind::SEMI);
        if let Some(last) = stats.last() {
            let end = self.prev_token().end_pos;
            self.store_end(*last, end);
        }
        stats
    }

    /// Java: public JCStatement parseSimpleStatement()
    fn parse_simple_statement(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        match self.token.kind {
            TokenKind::LBRACE => self.block(ctx),
            TokenKind::IF => {
                self.next_token();
                let cond = self.par_expression(ctx);
                let thenpart = self.parse_statement_as_block(ctx);
                let elsepart = if self.token.kind == TokenKind::ELSE {
                    self.next_token();
                    Some(self.parse_statement_as_block(ctx))
                } else {
                    None
                };
                self.F.at(pos).if_(ctx, cond, thenpart, elsepart)
            }
            TokenKind::FOR => {
                self.next_token();
                self.accept(ctx, TokenKind::LPAREN);
                let mut inits: Vec<TreeId> = Vec::new();
                let mut enhanced: Option<(TreeId, TreeId)> = None;
                if self.token.kind != TokenKind::SEMI {
                    if self.looks_like_local_var_decl() {
                        let mods = self.opt_final(ctx, 0);
                        let type_ = self.parse_type_impl(ctx, true);
                        let name = self.ident(ctx);
                        let def = self.variable_declarator_id(ctx, mods, type_);
                        if self.token.kind == TokenKind::COLON {
                            let var = self.to_p(def);
                            self.accept(ctx, TokenKind::COLON);
                            let expr = self.parse_expression(ctx);
                            enhanced = Some((var, expr));
                        } else {
                            // reparse as a normal declarator list from the name
                            let mut vdefs = Vec::new();
                            let first = self.variable_declarator_rest(
                                ctx, pos, mods, type_, name, false, true, true,
                            );
                            vdefs.push(first);
                            while self.token.kind == TokenKind::COMMA {
                                self.next_token();
                                let p = self.token.pos;
                                let n = self.ident(ctx);
                                let d = self.variable_declarator_rest(ctx, p, mods, type_, n, false, true, true);
                                vdefs.push(d);
                            }
                            let _ = def;
                            inits = vdefs;
                        }
                    } else {
                        inits = self.for_init_expressions(ctx);
                    }
                }
                if let Some((var, expr)) = enhanced {
                    self.accept(ctx, TokenKind::RPAREN);
                    let body = self.parse_statement_as_block(ctx);
                    return self.F.at(pos).foreach_loop(ctx, var, expr, body);
                }
                self.accept(ctx, TokenKind::SEMI);
                let cond = if self.token.kind == TokenKind::SEMI {
                    None
                } else {
                    Some(self.parse_expression(ctx))
                };
                self.accept(ctx, TokenKind::SEMI);
                let steps = if self.token.kind == TokenKind::RPAREN {
                    List::nil()
                } else {
                    self.for_update(ctx)
                };
                self.accept(ctx, TokenKind::RPAREN);
                let body = self.parse_statement_as_block(ctx);
                self.F.at(pos).for_loop(ctx, List::from_iterable(inits), cond, steps, body)
            }
            TokenKind::WHILE => {
                self.next_token();
                let cond = self.par_expression(ctx);
                let body = self.parse_statement_as_block(ctx);
                self.F.at(pos).while_loop(ctx, cond, body)
            }
            TokenKind::DO => {
                self.next_token();
                let body = self.parse_statement_as_block(ctx);
                self.accept(ctx, TokenKind::WHILE);
                let cond = self.par_expression(ctx);
                self.accept(ctx, TokenKind::SEMI);
                { let __to_p_tree = self.F.at(pos).do_loop(ctx, body, cond); self.to_p(__to_p_tree) }
            }
            TokenKind::TRY => {
                self.next_token();
                let mut resources = List::nil();
                if self.token.kind == TokenKind::LPAREN {
                    self.next_token();
                    resources = self.resources(ctx);
                    self.accept(ctx, TokenKind::RPAREN);
                }
                let body = self.block(ctx);
                let mut catchers: Vec<TreeId> = Vec::new();
                let mut finalizer = None;
                if self.token.kind == TokenKind::CATCH || self.token.kind == TokenKind::FINALLY {
                    while self.token.kind == TokenKind::CATCH {
                        catchers.push(self.catch_clause(ctx));
                    }
                    if self.token.kind == TokenKind::FINALLY {
                        self.next_token();
                        finalizer = Some(self.block(ctx));
                    }
                }
                if resources.is_empty() {
                    if finalizer.is_none() && catchers.is_empty() {
                        let p = self.token.pos;
                        let _ = self.log.borrow_mut().error_flag_at(
                            DiagnosticFlag::SYNTAX,
                            p,
                            JCDiagnosticError::from(errors::try_without_catch_finally_or_resource_decls()),
                        );
                    }
                    self.F.at(pos).try_block(ctx, body, List::from_iterable(catchers), finalizer)
                } else {
                    self.F.at(pos).try_with_resources(
                        ctx,
                        resources,
                        body,
                        List::from_iterable(catchers),
                        finalizer,
                    )
                }
            }
            TokenKind::SWITCH => {
                self.next_token();
                let selector = self.par_expression(ctx);
                self.accept(ctx, TokenKind::LBRACE);
                let cases = self.switch_block_statement_groups(ctx);
                let t = { let __to_tree = self.F.at(pos).switch(ctx, selector, cases); self.to(__to_tree) };
                let endpos = self.token.end_pos;
                if let TreeKind::Switch { endpos: field, .. } = &mut ctx.tree_mut(t).kind {
                    *field = endpos;
                }
                self.accept(ctx, TokenKind::RBRACE);
                t
            }
            TokenKind::SYNCHRONIZED => {
                self.next_token();
                let lock = self.par_expression(ctx);
                let body = self.block(ctx);
                self.F.at(pos).synchronized(ctx, lock, body)
            }
            TokenKind::RETURN => {
                self.next_token();
                let result = if self.token.kind == TokenKind::SEMI {
                    None
                } else {
                    Some(self.parse_expression(ctx))
                };
                self.accept(ctx, TokenKind::SEMI);
                { let __to_p_tree = self.F.at(pos).return_(ctx, result); self.to_p(__to_p_tree) }
            }
            TokenKind::THROW => {
                self.next_token();
                let exc = self.parse_expression(ctx);
                self.accept(ctx, TokenKind::SEMI);
                { let __to_p_tree = self.F.at(pos).throw_(ctx, exc); self.to_p(__to_p_tree) }
            }
            TokenKind::BREAK => {
                self.next_token();
                let label = if Self::is_lax_identifier(self.token.kind) {
                    Some(self.ident(ctx))
                } else {
                    None
                };
                self.accept(ctx, TokenKind::SEMI);
                { let __to_p_tree = self.F.at(pos).break_(ctx, label); self.to_p(__to_p_tree) }
            }
            TokenKind::CONTINUE => {
                self.next_token();
                let label = if Self::is_lax_identifier(self.token.kind) {
                    Some(self.ident(ctx))
                } else {
                    None
                };
                self.accept(ctx, TokenKind::SEMI);
                { let __to_p_tree = self.F.at(pos).continue_(ctx, label); self.to_p(__to_p_tree) }
            }
            TokenKind::SEMI => {
                self.next_token();
                { let __to_p_tree = self.F.at(pos).skip(ctx); self.to_p(__to_p_tree) }
            }
            TokenKind::ELSE => {
                self.next_token();
                self.do_recover(ctx, pos, ErrorRecoveryAction::BlockStmt, JCDiagnosticError::from(errors::else_without_if()))
            }
            TokenKind::FINALLY => {
                self.next_token();
                self.do_recover(ctx, pos, ErrorRecoveryAction::BlockStmt, JCDiagnosticError::from(errors::finally_without_try()))
            }
            TokenKind::CATCH => self.do_recover(
                ctx,
                pos,
                ErrorRecoveryAction::CatchClause,
                JCDiagnosticError::from(errors::catch_without_try()),
            ),
            TokenKind::ASSERT => {
                self.next_token();
                let assertion = self.parse_expression(ctx);
                let mut message = None;
                if self.token.kind == TokenKind::COLON {
                    self.next_token();
                    message = Some(self.parse_expression(ctx));
                }
                self.accept(ctx, TokenKind::SEMI);
                { let __to_p_tree = self.F.at(pos).assert_(ctx, assertion, message); self.to_p(__to_p_tree) }
            }
            _ => {
                panic!("java.lang.AssertionError in parseSimpleStatement");
            }
        }
    }

    /// Java: private JCStatement doRecover(int startPos, ErrorRecoveryAction action,
    /// Error errorKey)
    fn do_recover(
        &mut self,
        ctx: &mut CompilerContext,
        start_pos: i32,
        action: ErrorRecoveryAction,
        error_key: JCDiagnosticError,
    ) -> TreeId {
        let err_pos = self.S.err_pos();
        let stm = match action {
            ErrorRecoveryAction::BlockStmt => self.parse_statement_as_block(ctx),
            ErrorRecoveryAction::CatchClause => self.catch_clause(ctx),
        };
        self.S.set_err_pos(err_pos);
        let start = ctx.tree(stm).pos;
        let err = self.syntax_error_list(ctx, start, List::of(stm), error_key);
        { let __to_p_tree = self.F.at(start).exec(ctx, err); self.to_p(__to_p_tree) }
    }

    /// Java: protected JCCatch catchClause()
    fn catch_clause(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        self.accept(ctx, TokenKind::CATCH);
        self.accept(ctx, TokenKind::LPAREN);
        let mods = self.opt_final(ctx, Flags::PARAMETER);
        let catch_types = self.catch_types(ctx);
        let first = *catch_types.head();
        let param_type = if catch_types.length() > 1 {
            let p = self.get_start_pos(ctx, first);
            { let __to_p_tree = self.F.at(p).type_union(ctx, catch_types.clone()); self.to_p(__to_p_tree) }
        } else {
            first
        };
        let formal = self.variable_declarator_id(ctx, mods, param_type);
        self.accept(ctx, TokenKind::RPAREN);
        let body = self.block(ctx);
        self.F.at(pos).catch(ctx, formal, body)
    }

    /// Java: List<JCExpression> catchTypes()
    fn catch_types(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut types = vec![self.parse_type(ctx)];
        while self.token.kind == TokenKind::BAR {
            self.next_token();
            types.push(self.parse_type(ctx));
        }
        List::from_iterable(types)
    }

    /// Java: List<JCCase> switchBlockStatementGroups()
    fn switch_block_statement_groups(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut cases = Vec::new();
        loop {
            let pos = self.token.pos;
            match self.token.kind {
                TokenKind::CASE | TokenKind::DEFAULT => {
                    cases.extend(self.switch_block_statement_group(ctx));
                }
                TokenKind::RBRACE | TokenKind::EOF => return List::from_iterable(cases),
                _ => {
                    self.next_token();
                    let _ = self.syntax_error(
                        ctx,
                        pos,
                        JCDiagnosticError::from(errors::expected3(
                            Self::token_arg(TokenKind::CASE),
                            Self::token_arg(TokenKind::DEFAULT),
                            Self::token_arg(TokenKind::RBRACE),
                        )),
                    );
                }
            }
        }
    }

    /// Java: protected List<JCCase> switchBlockStatementGroup()
    fn switch_block_statement_group(&mut self, ctx: &mut CompilerContext) -> Vec<TreeId> {
        let pos = self.token.pos;
        let labels = if self.token.kind == TokenKind::CASE {
            self.next_token();
            let mut labels = Vec::new();
            labels.push(self.parse_expression(ctx));
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                labels.push(self.parse_expression(ctx));
            }
            List::from_iterable(labels)
        } else {
            self.accept(ctx, TokenKind::DEFAULT);
            List::nil()
        };
        self.accept(ctx, TokenKind::COLON);
        let stats = self.block_statements(ctx);
        let c = self.F.at(pos).case(ctx, CaseKind::STATEMENT, labels, stats.clone(), None);
        let c = self.to_p(c);
        if stats.is_empty() {
            let end = self.prev_token().end_pos;
            self.store_end(c, end);
        }
        vec![c]
    }

    /// Java: List<JCStatement> forInit()
    fn for_init_expressions(&mut self, ctx: &mut CompilerContext) -> Vec<TreeId> {
        let pos = self.token.pos;
        let first = self.parse_expression(ctx);
        self.more_statement_expressions(ctx, pos, first)
    }

    /// Java: <T> T moreStatementExpressions(int pos, JCExpression first, T stats)
    fn more_statement_expressions(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        first: TreeId,
    ) -> Vec<TreeId> {
        let mut stats = Vec::new();
        let first = self.check_expr_stat(ctx, first);
        stats.push({ let __to_p_tree = self.F.at(pos).exec(ctx, first); self.to_p(__to_p_tree) });
        while self.token.kind == TokenKind::COMMA {
            self.next_token();
            let pos = self.token.pos;
            let t = self.parse_expression(ctx);
            let t = self.check_expr_stat(ctx, t);
            stats.push({ let __to_p_tree = self.F.at(pos).exec(ctx, t); self.to_p(__to_p_tree) });
        }
        stats
    }

    /// Java: List<JCExpressionStatement> forUpdate()
    fn for_update(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let pos = self.token.pos;
        let first = self.parse_expression(ctx);
        let stats = self.more_statement_expressions(ctx, pos, first);
        List::from_iterable(stats)
    }

    /// Java: List<JCTree> resources()
    fn resources(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut defs = vec![self.resource(ctx)];
        while self.token.kind == TokenKind::SEMI {
            if let Some(last) = defs.last() {
                let end = self.token.end_pos;
                self.store_end(*last, end);
            }
            self.next_token();
            if self.token.kind == TokenKind::RPAREN {
                break;
            }
            defs.push(self.resource(ctx));
        }
        List::from_iterable(defs)
    }

    /// Java: protected JCTree resource()
    fn resource(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let start_pos = self.token.pos;
        if self.token.kind == TokenKind::FINAL || self.token.kind == TokenKind::MONKEYS_AT {
            let mods = self.opt_final(ctx, Flags::FINAL);
            let t = self.parse_type_impl(ctx, true);
            let pos = self.token.pos;
            let name = self.ident(ctx);
            return self.variable_declarator_rest(ctx, pos, mods, t, name, true, true, false);
        }
        if self.looks_like_local_var_decl() {
            let mods = self.F.at(start_pos).modifiers(ctx, Flags::FINAL);
            let t = self.parse_type_impl(ctx, true);
            let pos = self.token.pos;
            let name = self.ident(ctx);
            return self.variable_declarator_rest(ctx, pos, mods, t, name, true, true, false);
        }
        self.parse_expression(ctx)
    }

    /* ---------- class/interface declarations -------------- */

    /// Java: protected JCStatement classOrRecordOrInterfaceOrEnumDeclaration(
    /// JCModifiers mods, Comment dc)
    fn class_or_interface_declaration(&mut self, ctx: &mut CompilerContext, mods: TreeId) -> TreeId {
        if self.token.kind == TokenKind::CLASS {
            self.class_declaration(ctx, mods)
        } else if self.token.kind == TokenKind::INTERFACE {
            self.interface_declaration(ctx, mods)
        } else {
            let pos = self.token.pos;
            let errs = if Self::is_lax_identifier(self.token.kind) {
                let name = self.ident(ctx);
                let id = { let __to_p_tree = self.F.at(pos).ident(ctx, name); self.to_p(__to_p_tree) };
                let _ = self.set_error_end_pos(self.token.pos);
                List::of2(mods, id)
            } else {
                List::of(mods)
            };
            let err = self.syntax_error_list(
                ctx,
                pos,
                errs,
                JCDiagnosticError::from(errors::expected3(
                    Self::token_arg(TokenKind::CLASS),
                    Self::token_arg(TokenKind::INTERFACE),
                    Self::token_arg(TokenKind::ENUM),
                )),
            );
            { let __to_p_tree = self.F.at(pos).exec(ctx, err); self.to_p(__to_p_tree) }
        }
    }

    /// Java: protected JCClassDecl classDeclaration(JCModifiers mods, Comment dc)
    fn class_declaration(&mut self, ctx: &mut CompilerContext, mods: TreeId) -> TreeId {
        let pos = self.token.pos;
        self.accept(ctx, TokenKind::CLASS);
        let name = self.type_name(ctx);
        let typarams = self.type_parameters_opt(ctx);
        let mut extending = None;
        if self.token.kind == TokenKind::EXTENDS {
            self.next_token();
            extending = Some(self.parse_type(ctx));
        }
        let mut implementing = List::nil();
        if self.token.kind == TokenKind::IMPLEMENTS {
            self.next_token();
            implementing = self.type_list(ctx);
        }
        let defs = self.class_interface_body(ctx, name.clone(), false);
        { let __to_p_tree = self.F.at(pos).class_def(ctx, mods, name, typarams, extending, implementing, defs); self.to_p(__to_p_tree) }
    }

    /// Java: protected JCClassDecl interfaceDeclaration(JCModifiers mods, Comment dc)
    fn interface_declaration(&mut self, ctx: &mut CompilerContext, mods: TreeId) -> TreeId {
        let pos = self.token.pos;
        self.accept(ctx, TokenKind::INTERFACE);
        let name = self.type_name(ctx);
        let typarams = self.type_parameters_opt(ctx);
        let mut extending = List::nil();
        if self.token.kind == TokenKind::EXTENDS {
            self.next_token();
            extending = self.type_list(ctx);
        }
        let defs = self.class_interface_body(ctx, name.clone(), true);
        { let __to_p_tree = self.F.at(pos).class_def(ctx, mods, name, typarams, None, extending, defs); self.to_p(__to_p_tree) }
    }

    /// Java: List<JCTree> classInterfaceOrRecordBody(Name className,
    /// boolean isInterface, boolean isRecord)
    fn class_interface_body(&mut self, ctx: &mut CompilerContext, class_name: Name, is_interface: bool) -> List<TreeId> {
        self.accept(ctx, TokenKind::LBRACE);
        let mut defs: Vec<TreeId> = Vec::new();
        while self.token.kind != TokenKind::RBRACE && self.token.kind != TokenKind::EOF {
            defs.extend(self.class_body_declaration(ctx, class_name.clone(), is_interface));
            if self.token.pos <= self.end_pos_table.error_end_pos() {
                self.skip(false, true, true, false);
            }
        }
        self.accept(ctx, TokenKind::RBRACE);
        List::from_iterable(defs)
    }

    /// Java: protected List<JCTree> classOrInterfaceOrRecordBodyDeclaration(
    /// Name className, boolean isInterface, boolean isRecord)
    fn class_body_declaration(&mut self, ctx: &mut CompilerContext, class_name: Name, is_interface: bool) -> Vec<TreeId> {
        if self.token.kind == TokenKind::SEMI {
            self.next_token();
            return Vec::new();
        }
        let pos = self.token.pos;
        let mods = self.modifiers_opt(ctx);
        if self.token.kind == TokenKind::CLASS || self.token.kind == TokenKind::INTERFACE {
            return vec![self.class_or_interface_declaration(ctx, mods)];
        }
        if self.token.kind == TokenKind::ENUM {
            return vec![self.class_or_interface_declaration(ctx, mods)];
        }
        if self.token.kind == TokenKind::LBRACE {
            return vec![self.block_at(ctx, pos, 0)];
        }
        let mut pos = self.token.pos;
        let typarams = self.type_parameters_opt(ctx);
        let annos_after = self.annotations_opt(ctx, false);
        let mut mods = mods;
        if !annos_after.is_empty() {
            self.check_source_level(pos, Feature::ANNOTATIONS_AFTER_TYPE_PARAMS);
            if let TreeKind::Modifiers { annotations, .. } = &mut ctx.tree_mut(mods).kind {
                let mut list = annotations.clone();
                list = list.append_list(&annos_after);
                *annotations = list;
            }
            if ctx.tree(mods).pos == NOPOS {
                let first = *annos_after.head();
                let p = ctx.tree(first).pos;
                ctx.tree_mut(mods).pos = p;
            }
        }
        let is_void = self.token.kind == TokenKind::VOID;
        let type_;
        if is_void {
            type_ = { let __to_tree = self.F.at(pos).type_ident(ctx, TypeTag::VOID); self.to(__to_tree) };
            self.next_token();
        } else {
            // method return types are un-annotated types
            type_ = self.term_with_mode(ctx, TYPE);
        }
        let is_constructor_style = (self.token.kind == TokenKind::LPAREN && !is_interface)
            && ctx.tree(type_).tag == Tag::IDENT;
        if is_constructor_style {
            let construct_name = self.names.init.clone();
            return vec![self.method_declarator_rest(
                ctx,
                pos,
                mods,
                None,
                construct_name,
                typarams,
                is_interface,
                true,
            )];
        }
        pos = self.token.pos;
        let name = self.ident(ctx);
        if self.token.kind == TokenKind::LPAREN {
            vec![self.method_declarator_rest(
                ctx, pos, mods, Some(type_), name, typarams, is_interface, is_void,
            )]
        } else if !is_void && typarams.is_empty() {
            let defs = self.variable_declarators_rest(
                ctx, pos, mods, type_, name, is_interface, Vec::new(), false,
            );
            self.accept(ctx, TokenKind::SEMI);
            if let Some(last) = defs.last() {
                let end = self.prev_token().end_pos;
                self.store_end(*last, end);
            }
            defs
        } else {
            let pos = self.token.pos;
            let mut err: Vec<TreeId> = Vec::new();
            if is_void || !typarams.is_empty() {
                let m = self.F.at(pos).method_def(
                    ctx,
                    mods,
                    name,
                    Some(type_),
                    typarams,
                    None,
                    List::nil(),
                    List::nil(),
                    None,
                    None,
                );
                err.push(self.to_p(m));
            }
            let p = self.token.pos;
            let e = self.syntax_error_list(
                ctx,
                p,
                List::from_iterable(err),
                JCDiagnosticError::from(errors::expected(Self::token_arg(TokenKind::LPAREN))),
            );
            vec![e]
        }
    }

    /// Java: protected JCTree methodDeclaratorRest(int pos, JCModifiers mods,
    /// JCExpression type, Name name, List<JCTypeParameter> typarams,
    /// boolean isInterface, boolean isVoid, boolean isRecord, Comment dc)
    #[allow(clippy::too_many_arguments)]
    fn method_declarator_rest(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        mods: TreeId,
        type_: Option<TreeId>,
        name: Name,
        typarams: List<TreeId>,
        is_interface: bool,
        is_void: bool,
    ) -> TreeId {
        if is_interface {
            let mods_flags = match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => *flags,
                _ => 0,
            };
            if (mods_flags & Flags::STATIC) != 0 {
                self.check_source_level(pos, Feature::STATIC_INTERFACE_METHODS);
            }
            if (mods_flags & Flags::PRIVATE) != 0 {
                self.check_source_level(pos, Feature::PRIVATE_INTERFACE_METHODS);
            }
        }
        let prev_receiver = self.receiver_param;
        self.receiver_param = None;
        let mut params = List::nil();
        let mut thrown = List::nil();
        params = self.formal_parameters(ctx);
        let mut type_ = type_;
        if !is_void {
            if let Some(t) = type_ {
                type_ = Some(self.brackets_opt_simple(ctx, t));
            }
        }
        if self.token.kind == TokenKind::THROWS {
            self.next_token();
            thrown = self.qualident_list(ctx, true);
        }
        let mut body = None;
        let mut default_value = None;
        if self.token.kind == TokenKind::LBRACE {
            body = Some(self.block(ctx));
        } else {
            if self.token.kind == TokenKind::DEFAULT {
                self.accept(ctx, TokenKind::DEFAULT);
                default_value = Some(self.annotation_value(ctx));
            }
            self.accept(ctx, TokenKind::SEMI);
            if self.token.pos <= self.end_pos_table.error_end_pos() {
                self.skip(false, true, false, false);
                if self.token.kind == TokenKind::LBRACE {
                    body = Some(self.block(ctx));
                }
            }
        }
        let result = self.F.at(pos).method_def(
            ctx, mods, name, type_, typarams, None, params, thrown, body, default_value,
        );
        let result = self.to_p(result);
        self.receiver_param = prev_receiver;
        result
    }

    /// Java: List<JCVariableDecl> formalParameters()
    fn formal_parameters(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut params = Vec::new();
        self.accept(ctx, TokenKind::LPAREN);
        if self.token.kind != TokenKind::RPAREN {
            let mut last_param = self.formal_parameter(ctx);
            params.push(last_param);
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                last_param = self.formal_parameter(ctx);
                params.push(last_param);
            }
        }
        if self.token.kind == TokenKind::RPAREN {
            self.next_token();
        } else {
            let p = self.token.pos;
            self.set_error_end_pos(p);
            let end = self.prev_token().end_pos;
            let _ = self.report_syntax_error_pos(
                ctx,
                end,
                JCDiagnosticError::from(errors::expected3(
                    Self::token_arg(TokenKind::COMMA),
                    Self::token_arg(TokenKind::RPAREN),
                    Self::token_arg(TokenKind::LBRACKET),
                )),
            );
        }
        List::from_iterable(params)
    }

    /// Java: protected JCVariableDecl formalParameter(boolean lambdaParameter,
    /// boolean recordComponent)
    fn formal_parameter(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let mods = self.opt_final(ctx, Flags::PARAMETER);
        self.permit_type_annotations_push_back = true;
        let type_ = self.parse_type_impl(ctx, false);
        self.permit_type_annotations_push_back = false;
        let mut mods = mods;
        if self.token.kind == TokenKind::ELLIPSIS {
            let varargs_annos = self.type_annotations_pushed_back.clone();
            self.type_annotations_pushed_back = List::nil();
            if let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind {
                *flags |= Flags::VARARGS;
            }
            let _ = varargs_annos;
            self.next_token();
        } else if !self.type_annotations_pushed_back.is_empty() {
            let head = *self.type_annotations_pushed_back.head();
            let p = ctx.tree(head).pos;
            let _ = self.syntax_error(
                ctx,
                p,
                JCDiagnosticError::from(errors::illegal_start_of_type()),
            );
            self.type_annotations_pushed_back = List::nil();
        }
        let pos = self.token.pos;
        let name = self.ident(ctx);
        let type_ = self.brackets_opt_simple(ctx, type_);
        { let __to_p_tree = self.F.at(pos).var_def(ctx, mods, name, Some(type_), None); self.to_p(__to_p_tree) }
    }

    /* ---------- compilation unit -------------- */

    /// Java: CompilationUnit = [ { "@" Annotation } PACKAGE Qualident ";"]
    /// {ImportDeclaration} {TypeDeclaration}
    pub fn parse_compilation_unit_impl(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let first_token = self.token.clone();
        let mut mods: Option<TreeId> = None;
        let mut seen_import = false;
        let mut seen_package = false;
        let mut defs: Vec<TreeId> = Vec::new();
        if self.token.kind == TokenKind::MONKEYS_AT {
            mods = Some(self.modifiers_opt(ctx));
        }
        if self.token.kind == TokenKind::PACKAGE {
            let package_pos = self.token.pos;
            let mut annotations = List::nil();
            seen_package = true;
            if let Some(mods_id) = mods.take() {
                let flags = match &ctx.tree(mods_id).kind {
                    TreeKind::Modifiers { flags, .. } => *flags,
                    _ => 0,
                };
                self.check_no_mods_at(ctx, package_pos, flags & !Flags::DEPRECATED);
                if let TreeKind::Modifiers { annotations: annos, .. } = &ctx.tree(mods_id).kind {
                    annotations = annos.clone();
                }
            }
            self.next_token();
            let pid = self.qualident(ctx, false);
            self.accept(ctx, TokenKind::SEMI);
            let pd = { let __to_p_tree = self.F.at(package_pos).package_decl(ctx, annotations, pid); self.to_p(__to_p_tree) };
            defs.push(pd);
            let _ = seen_package;
        }
        let mut check_for_imports = true;
        let mut first_type_decl = true;
        while self.token.kind != TokenKind::EOF {
            if self.token.pos <= self.end_pos_table.error_end_pos() {
                self.skip(check_for_imports, false, false, false);
                if self.token.kind == TokenKind::EOF {
                    break;
                }
            }
            if check_for_imports && mods.is_none() && self.token.kind == TokenKind::IMPORT {
                seen_import = true;
                defs.push(self.import_declaration(ctx));
            } else {
                let first = first_token.clone();
                let mods_id = match mods.take() {
                    Some(m) => self.modifiers_opt_partial(ctx, Some(m)),
                    None => {
                        if self.token.kind != TokenKind::SEMI {
                            self.modifiers_opt(ctx)
                        } else {
                            self.F.at(self.token.pos).modifiers(ctx, 0)
                        }
                    }
                };
                let def = self.type_declaration(ctx, mods_id);
                defs.push(def);
                if ctx.tree(def).tag == Tag::CLASSDEF {
                    check_for_imports = false;
                }
                let _ = first;
                first_type_decl = false;
            }
            let _ = seen_import;
            let _ = seen_package;
        }
        let defs_list = List::from_iterable(defs);
        let toplevel = self.F.at(first_token.pos).top_level(ctx, defs_list.clone());
        if defs_list.is_empty() {
            let end = self.prev_token().end_pos;
            self.store_end(toplevel, end);
        }
        if self.keep_line_map {
            let line_map = self.S.get_line_map();
            if let TreeKind::CompilationUnit { line_map: field, .. } = &mut ctx.tree_mut(toplevel).kind {
                *field = Some(Rc::new(line_map));
            }
        }
        let table = std::mem::replace(&mut self.end_pos_table, Box::new(EmptyEndPosTable::new()));
        if let TreeKind::CompilationUnit { end_positions, .. } = &mut ctx.tree_mut(toplevel).kind {
            *end_positions = Some(table.boxed());
        }
        toplevel
    }

    /// Java: protected JCTree importDeclaration()
    fn import_declaration(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        self.next_token();
        let mut import_static = false;
        if self.token.kind == TokenKind::STATIC {
            import_static = true;
            self.next_token();
        }
        let first_pos = self.token.pos;
        let first_name = self.ident(ctx);
        let mut qual_id = { let __to_p_tree = self.F.at(first_pos).ident(ctx, first_name); self.to_p(__to_p_tree) };
        let mut forward = true;
        while self.token.kind == TokenKind::DOT {
            self.next_token();
            if self.token.kind == TokenKind::STAR {
                self.next_token();
                forward = false;
                break;
            }
            let p = self.token.pos;
            let first_name = self.ident(ctx);
            qual_id = { let __to_p_tree = self.F.at(p).select(ctx, qual_id, first_name); self.to_p(__to_p_tree) };
        }
        let _ = forward;
        self.accept(ctx, TokenKind::SEMI);
        { let __to_p_tree = self.F.at(pos).import(ctx, qual_id, import_static); self.to_p(__to_p_tree) }
    }

    /// Java: JCTree typeDeclaration(JCModifiers mods, Comment docComment)
    fn type_declaration(&mut self, ctx: &mut CompilerContext, mods: TreeId) -> TreeId {
        let pos = self.token.pos;
        if matches!(
            self.token.kind,
            TokenKind::CLASS | TokenKind::INTERFACE | TokenKind::ENUM
        ) {
            return self.class_or_interface_declaration(ctx, mods);
        }
        // Upstream: a LAX identifier is consumed and reported as an additional
        // error tree; otherwise the modifiers alone are reported.
        let is_lax_identifier = matches!(
            self.token.kind,
            TokenKind::IDENTIFIER
                | TokenKind::ASSERT
                | TokenKind::ENUM
                | TokenKind::UNDERSCORE
                | TokenKind::THIS
        );
        let errs = if is_lax_identifier {
            let name = self.ident(ctx);
            let ident_tree = { let __to_p_tree = self.F.at(self.token.pos).ident(ctx, name); self.to_p(__to_p_tree) };
            self.set_error_end_pos(self.token.pos);
            List::of(mods).prepend(ident_tree)
        } else {
            List::of(mods)
        };
        let error = if self.parse_module_info {
            JCDiagnosticError::from(errors::expected_module_or_open())
        } else if self.allow_records {
            JCDiagnosticError::from(errors::expected4(
                Self::token_arg(TokenKind::CLASS),
                Self::token_arg(TokenKind::INTERFACE),
                Self::token_arg(TokenKind::ENUM),
                JavaString::from("record"),
            ))
        } else {
            JCDiagnosticError::from(errors::expected3(
                Self::token_arg(TokenKind::CLASS),
                Self::token_arg(TokenKind::INTERFACE),
                Self::token_arg(TokenKind::ENUM),
            ))
        };
        self.syntax_error_list(ctx, pos, errs, error)
    }

    /// Java: protected JCExpression checkExprStat(JCExpression t)
    fn check_expr_stat(&mut self, ctx: &mut CompilerContext, t: TreeId) -> TreeId {
        if !tree_info::is_expression_statement(ctx, t) {
            let pos = ctx.tree(t).pos;
            let ret = self.F.at(pos).erroneous(ctx, List::of(t));
            self.log.borrow_mut().error_flag_at(
                DiagnosticFlag::SYNTAX,
                pos,
                JCDiagnosticError::from(errors::not_stmt()),
            );
            return ret;
        }
        t
    }

    /// Java: protected void reportSyntaxError(int pos, Error errorKey)
    fn report_syntax_error_pos(&mut self, ctx: &mut CompilerContext, pos: i32, error_key: JCDiagnosticError) -> Option<TreeId> {
        self.report_syntax_error(ctx, pos, error_key);
        None
    }


}

impl Parser for JavacParser {
    fn parse_compilation_unit(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.parse_compilation_unit_impl(ctx)
    }

    fn parse_expression(&mut self, ctx: &mut CompilerContext) -> TreeId {
        JavacParser::parse_expression(self, ctx)
    }

    fn parse_statement(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.parse_statement_as_block(ctx)
    }

    fn parse_type(&mut self, ctx: &mut CompilerContext) -> TreeId {
        JavacParser::parse_type(self, ctx)
    }
}

/// Java: JavacParser.ParensResult
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParensResult {
    Cast,
    ImplicitLambda,
    ExplicitLambda,
    Parens,
}
