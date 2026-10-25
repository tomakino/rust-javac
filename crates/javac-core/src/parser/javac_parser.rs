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
//! PORTING_RULES / batch status (S7b):
//! * JCTree values are arena TreeIds (ADR 0001); every parse method takes the
//!   CompilerContext because tree creation needs the arena;
//! * the Java nested Abstract/Simple/Empty EndPosTable classes are ported in
//!   this file (Simple uses util::int_hash_table::IntHashTable);
//! * the full Java 17 grammar is parsed: records, sealed types, patterns
//!   (instanceof/switch), switch expressions/rules, lambdas, method
//!   references, module declarations and annotation type declarations;
//! * keepDocComments=true and keepDocComments=false both work: the
//!   ported LazyDocCommentTable attaches comments lazily (JavacParser.attach)
//!   and tree/Pretty prints them from the compilation unit doc comment table;
//! * remaining dependency: DocCommentParser's ReferenceParser is still a
//!   TODO(parser S7) marker (doc-comment reference parsing only; registered).

use std::cell::RefCell;
use std::rc::Rc;

use javac_strings::JavaString;

use crate::code::bound_kind::BoundKind;
use crate::code::source::{Feature, Source};
use crate::code::type_tag::TypeTag;
use crate::tree::tree_maker::TreeFactory;
use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::parser::lazy_doc_comment_table::LazyDocCommentTable;
use crate::parser::lexer::Lexer;
use crate::parser::parser::Parser;
use crate::parser::parser_factory::ParserFactory;
use crate::parser::scanner::Scanner;
use crate::parser::tokens::{dummy_token, Comment, CommentStyle, Token, TokenKind};
use crate::resources::compiler_properties::{errors, fragments, warnings};
use crate::resources::diagnostic_contract::{DiagnosticArg, FragmentInfo, SourceArg};
use crate::tree::doc_comment_table::DocCommentTable;
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::jc_tree::{LiteralValue, Tag, TreeKind};
use javac_source::tree::case_tree::CaseKind;
use javac_source::tree::member_reference_tree::ReferenceMode;
use javac_source::tree::module_tree::ModuleKind;
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
use crate::util::position::NOPOS;


/// Type adapter: values are the upstream long (i64) modifiers from
/// crate::code::flags (the ported home of the upstream Flags constants); this
/// module only re-exports them as i64 for the Java long-typed parser API. Do
/// not add or copy constant values here: future flags.rs changes must flow
/// through automatically.
#[allow(non_upper_case_globals)]
mod Flags {
    pub const StandardFlags: i64 = crate::code::flags::StandardFlags as i64;
    pub const COMPACT_RECORD_CONSTRUCTOR: i64 = crate::code::flags::COMPACT_RECORD_CONSTRUCTOR;
    pub const RECORD: i64 = crate::code::flags::RECORD;
    pub const GENERATED_MEMBER: i64 = crate::code::flags::GENERATED_MEMBER as i64;
    pub const SEALED: i64 = crate::code::flags::SEALED;
    pub const NON_SEALED: i64 = crate::code::flags::NON_SEALED;
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
/// P4 placeholder: a fragment used as a diagnostic argument is currently
/// carried as its key text (diagnostic_contract::DiagnosticArg is a text
/// placeholder until the P4 JCDiagnostic reformatting batch; registered).
fn fragment_diagnostic_arg(fragment: FragmentInfo) -> DiagnosticArg {
    DiagnosticArg::from_display(fragment.info().key())
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

/// Java: private enum EnumeratorEstimate
#[derive(Clone, Copy, PartialEq, Eq)]
enum EnumeratorEstimate {
    Enumerator,
    Member,
    Unknown,
}

/// Java: interface ErrorRecoveryAction (adapter: a Rust enum of the two
/// upstream actions decoded by doRecover).
#[derive(Clone, Copy)]
enum ErrorRecoveryAction {
    BlockStmt,
    CatchClause,
}

/// Java: private enum PatternResult
#[derive(Clone, Copy, PartialEq, Eq)]
enum PatternResult {
    Expression,
    Pattern,
}

/// Java: JavacParser.LambdaParameterKind (the LambdaClassifier kind enum).
#[derive(Clone, Copy, PartialEq, Eq)]
enum LambdaParameterKind {
    Var,
    Explicit,
    Implicit,
    Error,
}

/// Java: JavacParser.decisionTable (fragment for a mixed lambda parameter list).
fn lambda_decision_fragment(
    current: LambdaParameterKind,
    new_kind: LambdaParameterKind,
) -> Option<FragmentInfo> {
    use LambdaParameterKind::{Explicit, Implicit, Var};
    match (current, new_kind) {
        (Var, Explicit) | (Explicit, Var) => Some(fragments::var_and_explicit_not_allowed()),
        (Var, Implicit) | (Implicit, Var) => Some(fragments::var_and_implicit_not_allowed()),
        (Explicit, Implicit) | (Implicit, Explicit) => {
            Some(fragments::implicit_and_explicit_not_allowed())
        }
        _ => None,
    }
}

/// Java: JavacParser.LambdaClassifier.reduce(LambdaParameterKind newKind)
fn reduce_lambda_kind(
    kind: &mut Option<LambdaParameterKind>,
    diag_fragment: &mut Option<FragmentInfo>,
    new_kind: LambdaParameterKind,
    source: Source,
) {
    match *kind {
        None => *kind = Some(new_kind),
        Some(current) if current != new_kind && current != LambdaParameterKind::Error => {
            *kind = Some(LambdaParameterKind::Error);
            let var_index =
                current == LambdaParameterKind::Var || new_kind == LambdaParameterKind::Var;
            *diag_fragment = if Feature::VAR_SYNTAX_IMPLICIT_LAMBDAS.allowed_in_source(source)
                || !var_index
            {
                lambda_decision_fragment(current, new_kind)
            } else {
                None
            };
        }
        _ => {}
    }
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
    /// The doc comments of the compilation unit (None when keepDocComments=false).
    doc_comments: Option<Rc<std::cell::RefCell<dyn DocCommentTable>>>,
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
        let doc_comments = JavacParser::new_doc_comment_table(keep_doc_comments, fac);
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
            doc_comments,
        };
        // prime the pump
        parser.next_token();
        parser
    }

    /// Java: protected DocCommentTable newDocCommentTable(boolean keepDocComments,
    /// ParserFactory fac).
    ///
    /// The keepDocComments=true path uses the ported LazyDocCommentTable
    /// (doc comments are attached lazily; see JavacParser.attach).
    fn new_doc_comment_table(
        keep_doc_comments: bool,
        fac: &ParserFactory,
    ) -> Option<Rc<std::cell::RefCell<dyn DocCommentTable>>> {
        if keep_doc_comments {
            let names = fac.names.clone();
            let diags = fac.log.borrow().diags().clone();
            let diag_source = fac.log.borrow().current_source();
            let doc_tree_maker = Rc::new(std::cell::RefCell::new(
                crate::tree::doc_tree_maker::DocTreeMaker::new(diags),
            ));
            let table: Rc<std::cell::RefCell<dyn DocCommentTable>> = Rc::new(
                std::cell::RefCell::new(LazyDocCommentTable::new(
                    names,
                    Rc::clone(&doc_tree_maker),
                    diag_source,
                )),
            );
            Some(table)
        } else {
            None
        }
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

    // Java: private static final int RECOVERY_THRESHOLD = 50; (const above)

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

    /// Java: accept(TokenKind tk, Function<TokenKind, Error> errorKey)
    fn accept_error(&mut self, ctx: &mut CompilerContext, tk: TokenKind, error_key: JCDiagnosticError) {
        if self.token.kind == tk {
            self.next_token();
        } else {
            self.set_error_end_pos(self.token.pos);
            let end_pos = self.prev_token().end_pos;
            self.report_syntax_error(ctx, end_pos, error_key);
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
    /// Java: protected void attach(JCTree tree, Comment dc)
    /// (Java: with keepDocComments=false the upstream docComments table is null
    /// and attach() is a no-op; that is the behavior
    /// kept here.
    fn attach(&mut self, tree: TreeId, dc: Option<Rc<dyn Comment>>) {
        // Java: if (keepDocComments && dc != null) docComments.putComment(tree, dc);
        if self.keep_doc_comments && let Some(dc) = dc {
            if let Some(doc_comments) = &self.doc_comments {
                doc_comments.borrow_mut().put_comment(tree, dc);
            }
        }
    }

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
                    JavaString::from("0x").concat(&string_val)
                } else {
                    string_val
                };
                // Java: try { n = Float.valueOf(proper); } catch
                // (NumberFormatException ex) { n = Float.NaN; } (the error was
                // already reported in the scanner).
                let n = match javac_floating::try_parse_float(&proper.to_string_lossy()) {
                    Ok(value) => value,
                    Err(_) => f32::NAN,
                };
                if n == 0.0f32 && !Self::is_zero(&proper) {
                    let key = JCDiagnosticError::from(errors::fp_number_too_small());
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        self.token.pos,
                        key,
                    );
                } else if n == f32::INFINITY {
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
                        Some(LiteralValue::Float(n)),
                    );
                }
            }
            TokenKind::DOUBLELITERAL => {
                let string_val = self.token.string_val().clone();
                let proper = if self.token.radix() == 16 {
                    JavaString::from("0x").concat(&string_val)
                } else {
                    string_val
                };
                // Java: try { n = Double.valueOf(proper); } catch
                // (NumberFormatException ex) { n = Double.NaN; } (the error was
                // already reported in the scanner).
                let n = match javac_floating::try_parse_double(&proper.to_string_lossy()) {
                    Ok(value) => value,
                    Err(_) => f64::NAN,
                };
                if n == 0.0f64 && !Self::is_zero(&proper) {
                    let key = JCDiagnosticError::from(errors::fp_number_too_small());
                    self.log.borrow_mut().error_flag_at(
                        DiagnosticFlag::SYNTAX,
                        self.token.pos,
                        key,
                    );
                } else if n == f64::INFINITY {
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
                        Some(LiteralValue::Double(n)),
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
                TokenKind::IDENTIFIER => {
                    if self.is_non_sealed_class_start(false) {
                        // Java: flag = NON_SEALED; nextToken(); nextToken();
                        self.next_token();
                        self.next_token();
                        Flags::NON_SEALED
                    } else if self.is_sealed_class_start(false) {
                        self.check_source_level(self.token.pos, Feature::SEALED_CLASSES);
                        Flags::SEALED
                    } else {
                        break 'outer;
                    }
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
            args.push(if (self.mode & EXPR) == 0 {
                self.type_argument(ctx)
            } else {
                self.parse_type(ctx)
            });
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                args.push(if (self.mode & EXPR) == 0 {
                    self.type_argument(ctx)
                } else {
                    self.parse_type(ctx)
                });
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
            let err = self.F.at(pos).erroneous(ctx, List::of2(wc, id));
            let err_pos = ctx.tree(err).pos;
            self.report_syntax_error(
                ctx,
                err_pos,
                JCDiagnosticError::from(errors::expected3(
                    Self::token_arg(TokenKind::GT),
                    Self::token_arg(TokenKind::EXTENDS),
                    Self::token_arg(TokenKind::SUPER),
                )),
            );
            result = err;
        } else {
            let t = self.F.at(pos).type_bound_kind(ctx, BoundKind::UNBOUND);
            let t = self.to_p(t);
            result = { let __to_p_tree = self.F.at(pos).wildcard(ctx, t, None); self.to_p(__to_p_tree) };
        }
        if !annotations.is_empty() {
            let head = annotations.head();
            let head_pos = ctx.tree(*head).pos;
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
    /// Java: public JCExpression unannotatedType(boolean allowVar)
    fn unannotated_type(&mut self, ctx: &mut CompilerContext, allow_var: bool) -> TreeId {
        let result = self.term_with_mode(ctx, TYPE);
        if let Some(restricted_type_name) = self.restricted_type_name(ctx, result, !allow_var)
            && (!allow_var
                || restricted_type_name.to_java_string() != self.names.var.to_java_string())
        {
            let pos = ctx.tree(result).pos;
            let _ = self.syntax_error(
                ctx,
                pos,
                JCDiagnosticError::from(errors::restricted_type_not_allowed_here(
                    restricted_type_name,
                )),
            );
        }
        result
    }

    /// Java: Name restrictedTypeName(JCExpression e, boolean shouldWarn)
    fn restricted_type_name(
        &mut self,
        ctx: &mut CompilerContext,
        e: TreeId,
        should_warn: bool,
    ) -> Option<Name> {
        match &ctx.tree(e).kind {
            TreeKind::Ident { name, .. } => {
                let pos = ctx.tree(e).pos;
                let name = name.clone();
                if self
                    .restricted_type_name_starting_at_source(&name, pos, should_warn)
                    .is_some()
                {
                    Some(name)
                } else {
                    None
                }
            }
            TreeKind::ArrayTypeTree { elemtype } => {
                let elemtype = elemtype.expect("array element type");
                self.restricted_type_name(ctx, elemtype, should_warn)
            }
            _ => None,
        }
    }

    /// Java: Source restrictedTypeNameStartingAtSource(Name name, int pos,
    /// boolean shouldWarn)
    ///
    /// PORTING_RULES: the diagnostic argument Source is the generated
    /// SourceArg placeholder (P4 contract), registered.
    fn restricted_type_name_starting_at_source(
        &mut self,
        name: &Name,
        pos: i32,
        should_warn: bool,
    ) -> Option<Source> {
        let is = |other: &Name| name.to_java_string() == other.to_java_string();
        if is(&self.names.var) {
            if Feature::LOCAL_VARIABLE_TYPE_INFERENCE.allowed_in_source(self.source) {
                return Some(Source::JDK10);
            }
            if should_warn {
                let key = JCDiagnosticWarning::from(warnings::restricted_type_not_allowed(
                    name.clone(),
                    crate::resources::diagnostic_contract::SourceArg::placeholder("JDK10"),
                ));
                self.log.borrow_mut().warning_at(pos, key);
            }
        }
        if is(&self.names.r#yield) {
            if self.allow_yield_statement {
                return Some(Source::JDK14);
            }
            if should_warn {
                let key = JCDiagnosticWarning::from(warnings::restricted_type_not_allowed(
                    name.clone(),
                    crate::resources::diagnostic_contract::SourceArg::placeholder("JDK14"),
                ));
                self.log.borrow_mut().warning_at(pos, key);
            }
        }
        if is(&self.names.record) {
            if self.allow_records {
                return Some(Source::JDK14);
            }
            if should_warn {
                let key = JCDiagnosticWarning::from(
                    warnings::restricted_type_not_allowed_preview(
                        name.clone(),
                        crate::resources::diagnostic_contract::SourceArg::placeholder("JDK14"),
                    ),
                );
                self.log.borrow_mut().warning_at(pos, key);
            }
        }
        if is(&self.names.sealed) {
            if self.allow_sealed_types {
                return Some(Source::JDK15);
            }
            if should_warn {
                let key = JCDiagnosticWarning::from(
                    warnings::restricted_type_not_allowed_preview(
                        name.clone(),
                        crate::resources::diagnostic_contract::SourceArg::placeholder("JDK15"),
                    ),
                );
                self.log.borrow_mut().warning_at(pos, key);
            }
        }
        if is(&self.names.permits) {
            if self.allow_sealed_types {
                return Some(Source::JDK15);
            }
            if should_warn {
                let key = JCDiagnosticWarning::from(
                    warnings::restricted_type_not_allowed_preview(
                        name.clone(),
                        crate::resources::diagnostic_contract::SourceArg::placeholder("JDK15"),
                    ),
                );
                self.log.borrow_mut().warning_at(pos, key);
            }
        }
        None
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
        let mut mi = self.F.at(pos).apply(ctx, type_args, t, args);
        if ctx.tree(t).tag == Tag::IDENT {
            let (t_pos, t_name) = match &ctx.tree(t).kind {
                TreeKind::Ident { name, .. } => (ctx.tree(t).pos, name.clone()),
                _ => unreachable!(),
            };
            if self.is_invalid_unqualified_method_identifier(t_pos, &t_name) {
                let key = JCDiagnosticError::from(errors::invalid_yield());
                self.log
                    .borrow_mut()
                    .error_flag_at(DiagnosticFlag::SYNTAX, t_pos, key);
                // PORTING_RULES: upstream F.Erroneous(...) uses the TreeMaker's
                // current position; the Rust factory requires an explicit at(pos)
                // (registered adapter).
                mi = self.F.at(pos).erroneous(ctx, List::of(mi));
            }
        }
        self.to_p(mi)
    }

    /// Java: boolean isInvalidUnqualifiedMethodIdentifier(int pos, Name name)
    fn is_invalid_unqualified_method_identifier(&mut self, pos: i32, name: &Name) -> bool {
        if name.to_java_string() == self.names.r#yield.to_java_string() {
            if self.allow_yield_statement {
                return true;
            }
            let key = JCDiagnosticWarning::from(warnings::invalid_yield());
            self.log.borrow_mut().warning_at(pos, key);
        }
        false
    }

    /// Java: JCExpression memberReferenceSuffix(JCExpression t)
    fn member_reference_suffix(&mut self, ctx: &mut CompilerContext, t: TreeId) -> TreeId {
        let pos1 = self.token.pos;
        self.accept(ctx, TokenKind::COLCOL);
        self.member_reference_suffix_at(ctx, pos1, t)
    }

    /// Java: JCExpression memberReferenceSuffix(int pos1, JCExpression t)
    fn member_reference_suffix_at(
        &mut self,
        ctx: &mut CompilerContext,
        _pos1: i32,
        t: TreeId,
    ) -> TreeId {
        self.check_source_level_at_token(Feature::METHOD_REFERENCES);
        self.select_expr_mode();
        let mut type_args: Option<List<TreeId>> = None;
        if self.token.kind == TokenKind::LT {
            type_args = Some(self.type_arguments_list(ctx, false));
        }
        let (ref_mode, ref_name) = if self.token.kind == TokenKind::NEW {
            let name = self.names.init.clone();
            self.next_token();
            (ReferenceMode::NEW, name)
        } else {
            (ReferenceMode::INVOKE, self.ident(ctx))
        };
        // Java: F.at(t.getStartPosition()).Reference(refMode, refName, t, typeArgs)
        let pos = tree_info::get_start_pos(ctx, t);
        let reference = self.F.at(pos).reference(ctx, ref_mode, ref_name, t, type_args);
        self.to_p(reference)
    }

    /// Java: JCExpression superSuffix(List<JCExpression> typeArgs, JCExpression t)
    fn super_suffix(&mut self, ctx: &mut CompilerContext, type_args: Option<List<TreeId>>, t: TreeId) -> TreeId {
        self.next_token();
        let mut t = t;
        if self.token.kind == TokenKind::LPAREN || type_args.is_some() {
            t = self.arguments_apply(ctx, type_args.unwrap_or_else(List::nil), t);
        } else if self.token.kind == TokenKind::COLCOL {
            if type_args.is_some() {
                return self.illegal(ctx, self.token.pos);
            }
            t = self.member_reference_suffix(ctx, t);
        } else {
            let pos = self.token.pos;
            self.accept(ctx, TokenKind::DOT);
            let ta = if self.token.kind == TokenKind::LT {
                Some(self.type_arguments_list(ctx, false))
            } else {
                None
            };
            let name = self.ident(ctx);
            t = { let __to_p_tree = self.F.at(pos).select(ctx, t, name); self.to_p(__to_p_tree) };
            t = self.arguments_opt(ctx, ta, t);
        }
        t
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
            | TokenKind::BOOLEAN => {
                if type_args.is_none() {
                    if new_annotations.is_empty() {
                        let basic = self.basic_type(ctx);
                        return self.array_creator_rest(ctx, newpos, basic);
                    } else {
                        let head = *new_annotations.head();
                        let head_pos = ctx.tree(head).pos;
                        let basic = self.basic_type(ctx);
                        let annotated = {
                            let x = self.F.at(head_pos).annotated_type(ctx, new_annotations, basic);
                            self.to_p(x)
                        };
                        return self.array_creator_rest(ctx, newpos, annotated);
                    }
                }
            }
            _ => {}
        }
        let mut t = self.qualident(ctx, true);
        let oldmode = self.mode;
        self.select_type_mode();
        let mut diamond_found = false;
        let mut last_typeargs_pos = -1;
        if self.token.kind == TokenKind::LT {
            last_typeargs_pos = self.token.pos;
            t = self.type_arguments_tree(ctx, t, true);
            diamond_found = (self.mode & DIAMOND) != 0;
        }
        while self.token.kind == TokenKind::DOT {
            if diamond_found {
                // cannot select after a diamond
                let _ = self.illegal_at_token(ctx);
            }
            let pos = self.token.pos;
            self.next_token();
            let tyannos = self.type_annotations_opt(ctx);
            let name = self.ident(ctx);
            t = {
                let x = self.F.at(pos).select(ctx, t, name);
                self.to_p(x)
            };
            if !tyannos.is_empty() {
                let head = *tyannos.head();
                let head_pos = ctx.tree(head).pos;
                t = {
                    let x = self.F.at(head_pos).annotated_type(ctx, tyannos, t);
                    self.to_p(x)
                };
            }
            if self.token.kind == TokenKind::LT {
                last_typeargs_pos = self.token.pos;
                t = self.type_arguments_tree(ctx, t, true);
                diamond_found = (self.mode & DIAMOND) != 0;
            }
        }
        self.mode = oldmode;
        if self.token.kind == TokenKind::LBRACKET || self.token.kind == TokenKind::MONKEYS_AT {
            // handle type annotations for non primitive arrays
            if !new_annotations.is_empty() {
                t = self.insert_annotations_to_most_inner(ctx, t, new_annotations, false);
            }
            let e = self.array_creator_rest(ctx, newpos, t);
            if diamond_found {
                self.report_syntax_error(
                    ctx,
                    last_typeargs_pos,
                    JCDiagnosticError::from(errors::cannot_create_array_with_diamond()),
                );
                let err = self.F.at(newpos).erroneous(ctx, List::of(e));
                return self.to_p(err);
            } else if let Some(type_args) = type_args {
                let mut pos = newpos;
                if !type_args.is_empty() && ctx.tree(*type_args.head()).pos != NOPOS {
                    // note: this should always happen but we should
                    // not rely on this as the parser is continuously
                    // modified to improve error recovery.
                    pos = ctx.tree(*type_args.head()).pos;
                }
                let prev_end = self.prev_token().end_pos;
                self.set_error_end_pos(prev_end);
                let mut errs: Vec<TreeId> = vec![e];
                for arg in type_args.iter() {
                    errs.push(arg);
                }
                let err = self.F.at(pos).erroneous(ctx, List::from_iterable(errs));
                let err_pos = ctx.tree(err).pos;
                self.report_syntax_error(
                    ctx,
                    err_pos,
                    JCDiagnosticError::from(errors::cannot_create_array_with_type_arguments()),
                );
                return self.to_p(err);
            }
            return e;
        } else if self.token.kind == TokenKind::LPAREN {
            // handle type annotations for instantiations and anonymous classes
            if !new_annotations.is_empty() {
                t = self.insert_annotations_to_most_inner(ctx, t, new_annotations, false);
            }
            return self.class_creator_rest(ctx, newpos, None, type_args, t);
        } else {
            let token_pos = self.token.pos;
            self.set_error_end_pos(token_pos);
            self.report_syntax_error(
                ctx,
                token_pos,
                JCDiagnosticError::from(errors::expected2(
                    Self::token_arg(TokenKind::LPAREN),
                    Self::token_arg(TokenKind::LBRACKET),
                )),
            );
            let new_class = self.F.at(newpos).new_class(
                ctx,
                None,
                type_args.unwrap_or_else(List::nil),
                t,
                List::nil(),
                None,
            );
            let new_class = self.to_p(new_class);
            let err = self.F.at(newpos).erroneous(ctx, List::of(new_class));
            self.to_p(err)
        }
    }

    /// Java: JCExpression classCreatorRest(int newpos, JCExpression encl,
    /// List<JCExpression> typeArgs, JCExpression t)
    fn class_creator_rest(
        &mut self,
        ctx: &mut CompilerContext,
        newpos: i32,
        encl: Option<TreeId>,
        type_args: Option<List<TreeId>>,
        t: TreeId,
    ) -> TreeId {
        let args = self.arguments(ctx);
        let mut body = None;
        if self.token.kind == TokenKind::LBRACE {
            let pos = self.token.pos;
            let defs = self.class_interface_body(ctx, self.names.empty.clone(), false, false);
            let mods = self.F.at(NOPOS).modifiers(ctx, 0);
            let anonymous = {
                let x = self.F.at(pos).anonymous_class_def(ctx, mods, defs);
                self.to_p(x)
            };
            body = Some(anonymous);
        }
        {
            let x = self.F.at(newpos).new_class(
                ctx,
                encl,
                type_args.unwrap_or_else(List::nil),
                t,
                args,
                body,
            );
            self.to_p(x)
        }
    }

    /// Java: JCExpression innerCreator(int newpos, List<JCExpression> typeArgs,
    /// JCExpression encl)
    fn inner_creator(&mut self, ctx: &mut CompilerContext, newpos: i32, type_args: Option<List<TreeId>>, encl: TreeId) -> TreeId {
        let new_annotations = self.type_annotations_opt(ctx);
        let pos = self.token.pos;
        let name = self.ident(ctx);
        let mut t = {
            let x = self.F.at(pos).ident(ctx, name);
            self.to_p(x)
        };
        if !new_annotations.is_empty() {
            let head = *new_annotations.head();
            let head_pos = ctx.tree(head).pos;
            t = {
                let x = self.F.at(head_pos).annotated_type(ctx, new_annotations, t);
                self.to_p(x)
            };
        }
        if self.token.kind == TokenKind::LT {
            let oldmode = self.mode;
            t = self.type_arguments_tree(ctx, t, true);
            self.mode = oldmode;
        }
        self.class_creator_rest(ctx, newpos, Some(encl), type_args, t)
    }

    /// Java: JCExpression arrayCreatorRest(int newpos, JCExpression elemtype)
    fn array_creator_rest(&mut self, ctx: &mut CompilerContext, newpos: i32, mut elemtype: TreeId) -> TreeId {
        let annos = self.type_annotations_opt(ctx);
        self.accept(ctx, TokenKind::LBRACKET);
        if self.token.kind == TokenKind::RBRACKET {
            self.accept(ctx, TokenKind::RBRACKET);
            let elemtype = self.brackets_opt(ctx, elemtype, annos);
            if self.token.kind == TokenKind::LBRACE {
                return self.array_initializer(ctx, newpos, Some(elemtype));
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
                elemtype = self.brackets_opt_cont(ctx, elemtype, pos, maybe_dim_annos);
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
        let has_elems = elems.is_some();
        let na = self
            .F
            .at(newpos)
            .new_array(ctx, Some(elemtype), List::from_iterable(dims), elems);
        let na = self.to_p(na);
        if let TreeKind::NewArray { dim_annotations: field, .. } = &mut ctx.tree_mut(na).kind {
            *field = List::from_iterable(dim_annotations);
        }
        if has_elems {
            return self.syntax_error_list(
                ctx,
                errpos,
                List::of(na),
                JCDiagnosticError::from(errors::illegal_array_creation_both_dimension_and_initialization()),
            );
        }
        na
    }

    /// Java: JCExpression arrayInitializer(int newpos, JCExpression t)
    fn array_initializer(&mut self, ctx: &mut CompilerContext, newpos: i32, t: Option<TreeId>) -> TreeId {
        let elems = self.array_initializer_elements(ctx, newpos);
        { let __to_p_tree = self.F.at(newpos).new_array(ctx, t, List::nil(), Some(elems)); self.to_p(__to_p_tree) }
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
            self.array_initializer(ctx, pos, None)
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
    fn term2_rest(&mut self, ctx: &mut CompilerContext, t: TreeId, minprec: i32) -> TreeId {
        let mut od_stack: Vec<TreeId> = Vec::with_capacity((INFIX_PRECEDENCE_LEVELS + 1) as usize);
        od_stack.push(t);
        let mut op_stack: Vec<Token> = Vec::with_capacity((INFIX_PRECEDENCE_LEVELS + 1) as usize);
        let mut top: usize = 0;
        let mut top_op: Token = dummy_token();
        while Self::prec(self.token.kind) >= minprec {
            while op_stack.len() <= top {
                op_stack.push(dummy_token());
            }
            op_stack[top] = top_op.clone();
            if self.token.kind == TokenKind::INSTANCEOF {
                let pos = self.token.pos;
                self.next_token();
                let pattern: TreeId;
                if self.token.kind == TokenKind::LPAREN {
                    self.check_source_level(self.token.pos, Feature::PATTERN_SWITCH);
                    // Java: pattern = parsePattern(token.pos, null, null, true);
                    pattern = self.parse_pattern(ctx, self.token.pos, None, None, true);
                } else {
                let pattern_pos = self.token.pos;
                let mods = self.opt_final(ctx, 0);
                let type_pos = self.token.pos;
                let mut type_ = self.unannotated_type(ctx, false);
                pattern = if self.token.kind == TokenKind::IDENTIFIER {
                    self.check_source_level(self.token.pos, Feature::PATTERN_MATCHING_IN_INSTANCEOF);
                    // Java: pattern = parsePattern(patternPos, mods, type, true);
                    self.parse_pattern(ctx, pattern_pos, Some(mods), Some(type_), true)
                } else {
                    let mods_flags = match &ctx.tree(mods).kind {
                        TreeKind::Modifiers { flags, .. } => *flags,
                        _ => 0,
                    };
                    self.check_no_mods_at(ctx, type_pos, mods_flags & !Flags::DEPRECATED);
                    let annotations = match &ctx.tree(mods).kind {
                        TreeKind::Modifiers { annotations, .. } => annotations.clone(),
                        _ => List::nil(),
                    };
                    if !annotations.is_empty() {
                        let head = *annotations.head();
                        let head_pos = ctx.tree(head).pos;
                        self.check_source_level(head_pos, Feature::TYPE_ANNOTATIONS);
                        let mut type_annos: Vec<TreeId> = Vec::new();
                        for decl in annotations.iter() {
                            let (ann_type, args) = match &ctx.tree(decl).kind {
                                TreeKind::Annotation {
                                    annotation_type,
                                    args,
                                    ..
                                } => (annotation_type.expect("annotation type"), args.clone()),
                                _ => unreachable!(),
                            };
                            let pos = ctx.tree(decl).pos;
                            let type_anno =
                                self.F.at(pos).type_annotation(ctx, ann_type, args);
                            self.end_pos_table.replace_tree(decl, type_anno);
                            type_annos.push(type_anno);
                        }
                        type_ = self.insert_annotations_to_most_inner(
                            ctx,
                            type_,
                            List::from_iterable(type_annos),
                            false,
                        );
                    }
                    type_
                };
                }
                od_stack[top] = self.F.at(pos).type_test(ctx, od_stack[top], pattern);
            } else {
                top_op = self.token.clone();
                self.next_token();
                top += 1;
                while od_stack.len() <= top {
                    od_stack.push(t);
                }
                od_stack[top] = self.term3(ctx);
            }
            while top > 0 && Self::prec(top_op.kind) >= Self::prec(self.token.kind) {
                let tag = Self::optag(top_op.kind).expect("binary operator");
                let binary = self.F.at(top_op.pos).binary(
                    ctx,
                    tag,
                    od_stack[top - 1],
                    od_stack[top],
                );
                od_stack[top - 1] = binary;
                top -= 1;
                top_op = op_stack[top].clone();
            }
        }
        assert_eq!(top, 0);
        let mut t = od_stack[0];
        if ctx.tree(t).tag == Tag::PLUS {
            t = self.fold_strings(ctx, t);
        }
        t
    }

    /// Java: public JCPattern parsePattern(int pos, JCModifiers mods,
    /// JCExpression parsedType, boolean inInstanceOf)
    fn parse_pattern(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        mods: Option<TreeId>,
        parsed_type: Option<TreeId>,
        in_instance_of: bool,
    ) -> TreeId {
        let pattern: TreeId;
        if self.token.kind == TokenKind::LPAREN && parsed_type.is_none() {
            let start_pos = self.token.pos;
            self.accept(ctx, TokenKind::LPAREN);
            let inner = self.parse_pattern(ctx, self.token.pos, None, None, false);
            self.accept(ctx, TokenKind::RPAREN);
            pattern = {
                let x = self.F.at(start_pos).parenthesized_pattern(ctx, inner);
                self.to_p(x)
            };
        } else {
            let mods = match mods {
                Some(mods) => mods,
                None => self.opt_final(ctx, 0),
            };
            let e = match parsed_type {
                Some(t) => t,
                None => self.term_with_mode(ctx, TYPE | NOLAMBDA),
            };
            let var_pos = self.token.pos;
            let var_name = self.ident(ctx);
            let var = {
                let x = self.F.at(var_pos).var_def(ctx, mods, var_name, Some(e), None);
                self.to_p(x)
            };
            pattern = {
                let x = self.F.at(pos).binding_pattern(ctx, var);
                self.to_p(x)
            };
        }
        if !in_instance_of && self.token.kind == TokenKind::AMPAMP {
            self.check_source_level_at_token(Feature::PATTERN_SWITCH);
            self.next_token();
            let guard = self.term_with_mode(ctx, EXPR | NOLAMBDA);
            return self.F.at(pos).guard_pattern(ctx, pattern, guard);
        }
        pattern
    }

    /// Java: protected JCExpression foldStrings(JCExpression tree)
    fn fold_strings(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        if !self.allow_string_folding {
            return tree;
        }
        // ListBuffer<JCExpression> opStack and ListBuffer<JCLiteral> litBuf;
        // prepend is represented by inserting at index 0.
        let mut op_stack: Vec<TreeId> = Vec::new();
        let mut lit_buf: Vec<TreeId> = Vec::new();
        let mut needs_folding = false;
        let mut curr = tree;
        loop {
            if ctx.tree(curr).tag == Tag::PLUS {
                let (lhs, rhs) = match &ctx.tree(curr).kind {
                    TreeKind::Binary { lhs, rhs, .. } => {
                        (lhs.expect("lhs"), rhs.expect("rhs"))
                    }
                    _ => unreachable!(),
                };
                needs_folding |=
                    self.fold_if_needed(ctx, rhs, &mut lit_buf, &mut op_stack, false);
                curr = lhs;
            } else {
                needs_folding |=
                    self.fold_if_needed(ctx, curr, &mut lit_buf, &mut op_stack, true);
                break;
            }
        }
        if needs_folding {
            let ops = op_stack;
            let mut res = ops[0];
            for op in &ops[1..] {
                let pos = tree_info::get_start_pos(ctx, res);
                let binary = self.F.at(pos).binary(ctx, Tag::PLUS, res, *op);
                let end = self.get_end_pos(*op);
                self.store_end(binary, end);
                res = binary;
            }
            res
        } else {
            tree
        }
    }

    /// Java: private boolean foldIfNeeded(JCExpression tree,
    /// ListBuffer<JCLiteral> litBuf, ListBuffer<JCExpression> opStack, boolean last)
    fn fold_if_needed(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        lit_buf: &mut Vec<TreeId>,
        op_stack: &mut Vec<TreeId>,
        last: bool,
    ) -> bool {
        if let Some(str_lit) = Self::string_literal(ctx, tree) {
            lit_buf.insert(0, str_lit);
            last && self.merge_literals(ctx, lit_buf, op_stack)
        } else {
            let res = self.merge_literals(ctx, lit_buf, op_stack);
            lit_buf.clear();
            op_stack.insert(0, tree);
            res
        }
    }

    /// Java: boolean merge(ListBuffer<JCLiteral> litBuf,
    /// ListBuffer<JCExpression> opStack)
    fn merge_literals(
        &mut self,
        ctx: &mut CompilerContext,
        lit_buf: &mut Vec<TreeId>,
        op_stack: &mut Vec<TreeId>,
    ) -> bool {
        if lit_buf.is_empty() {
            return false;
        }
        if lit_buf.len() == 1 {
            let first = lit_buf[0];
            op_stack.insert(0, first);
            return false;
        }
        let first = lit_buf[0];
        let pos = tree_info::get_start_pos(ctx, first);
        let mut text = String::new();
        for lit in lit_buf.iter() {
            if let TreeKind::Literal {
                value: Some(LiteralValue::String(s)),
                ..
            } = &ctx.tree(*lit).kind
            {
                text.push_str(&s.to_string_lossy());
            }
        }
        let merged = self.F.at(pos).literal(
            ctx,
            TypeTag::CLASS,
            Some(LiteralValue::String(JavaString::from(text.as_str()))),
        );
        let last = *lit_buf.last().expect("non-empty");
        let end = self.get_end_pos(last);
        self.store_end(merged, end);
        op_stack.insert(0, merged);
        true
    }

    /// Java: private JCLiteral stringLiteral(JCTree tree)
    fn string_literal(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
        if ctx.tree(tree).tag == Tag::LITERAL {
            if let TreeKind::Literal { typetag, .. } = &ctx.tree(tree).kind {
                if *typetag == TypeTag::CLASS {
                    return Some(tree);
                }
            }
        }
        None
    }

    /// Java: protected JCExpression term3()
    fn term3(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pos = self.token.pos;
        let mut type_args = self.type_arguments_opt_list_mode(ctx, EXPR);
        let t: TreeId;
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
                    if tk == TokenKind::SUB
                        && (self.token.kind == TokenKind::INTLITERAL
                            || self.token.kind == TokenKind::LONGLITERAL)
                        && self.token.radix() == 10
                    {
                        self.select_expr_mode();
                        t = self.literal_at(ctx, self.names.hyphen.clone(), pos);
                    } else {
                        let inner = self.term3(ctx);
                        return self.F.at(pos).unary(ctx, Self::unoptag(tk).expect("unary operator"), inner);
                    }
                } else {
                    return self.illegal(ctx, pos);
                }
            }
            TokenKind::LPAREN => {
                if type_args.is_none() && (self.mode & EXPR) != 0 {
                    // Java: ParensResult pres = analyzeParens(); switch (pres) { ... }
                    let pres = self.analyze_parens();
                    match pres {
                        ParensResult::Cast => {
                            self.accept(ctx, TokenKind::LPAREN);
                            self.select_type_mode();
                            let pos1 = pos;
                            let first = self.parse_type(ctx);
                            let mut cast_type = first;
                            let mut targets: List<TreeId> = List::of(first);
                            while self.token.kind == TokenKind::AMP {
                                self.check_source_level_at_token(
                                    Feature::INTERSECTION_TYPES_IN_CAST,
                                );
                                self.accept(ctx, TokenKind::AMP);
                                let next = self.parse_type(ctx);
                                targets = targets.prepend(next);
                            }
                            if targets.length() > 1 {
                                let reversed = targets.reverse();
                                cast_type = {
                                    let x = self.F.at(pos1).type_intersection(ctx, reversed);
                                    self.to_p(x)
                                };
                            }
                            self.accept(ctx, TokenKind::RPAREN);
                            self.select_expr_mode();
                            let t1 = self.term3(ctx);
                            return self.F.at(pos).type_cast(ctx, t1, cast_type);
                        }
                        ParensResult::ImplicitLambda | ParensResult::ExplicitLambda => {
                            // Java: t = lambdaExpressionOrStatement(true,
                            // pres == ParensResult.EXPLICIT_LAMBDA, pos);
                            t = self.lambda_expression_or_statement(
                                ctx,
                                true,
                                pres == ParensResult::ExplicitLambda,
                                pos,
                            );
                        }
                        ParensResult::Parens => {
                            self.accept(ctx, TokenKind::LPAREN);
                            self.select_expr_mode();
                            let t3 = self.term3(ctx);
                            let inner = self.term2_rest(ctx, t3, tree_info::OR_PREC);
                            let inner = self.term1_rest(ctx, inner);
                            let inner = self.term_rest(ctx, inner);
                            self.accept(ctx, TokenKind::RPAREN);
                            t = {
                                let x = self.F.at(pos).parens(ctx, inner);
                                self.to_p(x)
                            };
                        }
                    }
                } else {
                    return self.illegal(ctx, pos);
                }
            }
            TokenKind::THIS => {
                if (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    let this_name = self.names._this.clone();
                    let this_tree = {
                        let x = self.F.at(pos).ident(ctx, this_name);
                        self.to(x)
                    };
                    self.next_token();
                    t = if type_args.is_none() {
                        self.arguments_opt(ctx, None, this_tree)
                    } else {
                        let ta = type_args.take().expect("type args");
                        self.arguments_apply(ctx, ta, this_tree)
                    };
                    type_args = None;
                } else {
                    return self.illegal(ctx, pos);
                }
            }
            TokenKind::SUPER => {
                if (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    let super_name = self.names._super.clone();
                    let t0 = {
                        let x = self.F.at(pos).ident(ctx, super_name);
                        self.to(x)
                    };
                    t = self.super_suffix(ctx, type_args.take(), t0);
                    type_args = None;
                } else {
                    return self.illegal(ctx, pos);
                }
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
                if (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    self.next_token();
                    let ta = if self.token.kind == TokenKind::LT {
                        Some(self.type_arguments_list(ctx, false))
                    } else {
                        None
                    };
                    t = self.creator(ctx, pos, ta);
                    type_args = None;
                } else {
                    return self.illegal(ctx, pos);
                }
            }
            TokenKind::MONKEYS_AT => {
                let type_annos = self.type_annotations_opt(ctx);
                if type_annos.is_empty() {
                    panic!("java.lang.AssertionError: Expected type annotations, but found none!");
                }
                let expr = self.term3(ctx);
                if (self.mode & TYPE) == 0 {
                    match ctx.tree(expr).tag {
                        Tag::REFERENCE => {
                            // Java: JCMemberReference mref = (JCMemberReference) expr;
                            // mref.expr = toP(F.at(pos).AnnotatedType(typeAnnos, mref.expr));
                            // t = mref;
                            let mref_expr = match &ctx.tree(expr).kind {
                                TreeKind::MemberReference { expr, .. } => *expr,
                                _ => unreachable!(),
                            };
                            let inner = mref_expr.expect("member reference expr");
                            let annotated = {
                                let x = self
                                    .F
                                    .at(pos)
                                    .annotated_type(ctx, type_annos.clone(), inner);
                                self.to_p(x)
                            };
                            if let TreeKind::MemberReference { expr, .. } =
                                &mut ctx.tree_mut(expr).kind
                            {
                                *expr = Some(annotated);
                            }
                            t = expr;
                        }
                        Tag::SELECT => {
                            let sel_name = match &ctx.tree(expr).kind {
                                TreeKind::FieldAccess { name, .. } => name.clone(),
                                _ => unreachable!(),
                            };
                            if sel_name.to_java_string()
                                != self.names._class.to_java_string()
                            {
                                return self.illegal(ctx, pos);
                            }
                            let err_pos = self.token.pos;
                            let key = JCDiagnosticError::from(
                                errors::no_annotations_on_dot_class(),
                            );
                            self.log.borrow_mut().error_at(err_pos, key);
                            return expr;
                        }
                        _ => {
                            let head = *type_annos.head();
                            let head_pos = ctx.tree(head).pos;
                            return self.illegal(ctx, head_pos);
                        }
                    }
                } else {
                    t = self.insert_annotations_to_most_inner(ctx, expr, type_annos, false);
                }
            }
            TokenKind::UNDERSCORE
            | TokenKind::IDENTIFIER
            | TokenKind::ASSERT
            | TokenKind::ENUM => {
                if type_args.is_some() {
                    return self.illegal(ctx, pos);
                }
                if (self.mode & EXPR) != 0
                    && (self.mode & NOLAMBDA) == 0
                    && self.peek_token(TokenKind::ARROW)
                {
                    // Java: t = lambdaExpressionOrStatement(false, false, pos);
                    t = self.lambda_expression_or_statement(ctx, false, false, pos);
                } else {
                let ident_pos = self.token.pos;
                let name = self.ident(ctx);
                let mut ident_tree = {
                    let x = self.F.at(ident_pos).ident(ctx, name);
                    self.to_p(x)
                };
                loop {
                    let pos = self.token.pos;
                    let annos = self.type_annotations_opt(ctx);
                    if !annos.is_empty()
                        && self.token.kind != TokenKind::LBRACKET
                        && self.token.kind != TokenKind::ELLIPSIS
                    {
                        let head = *annos.head();
                        let head_pos = ctx.tree(head).pos;
                        return self.illegal(ctx, head_pos);
                    }
                    match self.token.kind {
                        TokenKind::LBRACKET => {
                            self.next_token();
                            if self.token.kind == TokenKind::RBRACKET {
                                self.next_token();
                                ident_tree = self.brackets_opt_simple(ctx, ident_tree);
                                ident_tree = {
                                    let x = self.F.at(pos).type_array(ctx, ident_tree);
                                    self.to_p(x)
                                };
                                if !annos.is_empty() {
                                    ident_tree = {
                                        let x = self.F.at(pos)
                                            .annotated_type(ctx, annos.clone(), ident_tree);
                                        self.to_p(x)
                                    };
                                }
                                ident_tree = self.brackets_suffix(ctx, ident_tree);
                            } else {
                                if (self.mode & EXPR) != 0 {
                                    self.select_expr_mode();
                                    let t1 = self.term(ctx);
                                    if !annos.is_empty() {
                                        let head = *annos.head();
                                        let head_pos = ctx.tree(head).pos;
                                        ident_tree = self.illegal(ctx, head_pos);
                                    }
                                    ident_tree = {
                                        let x = self.F.at(pos).indexed(ctx, ident_tree, t1);
                                        self.to(x)
                                    };
                                }
                                self.accept(ctx, TokenKind::RBRACKET);
                            }
                            break;
                        }
                        TokenKind::LPAREN => {
                            if (self.mode & EXPR) != 0 {
                                self.select_expr_mode();
                                ident_tree = if type_args.is_none() {
                                    self.arguments_opt(ctx, None, ident_tree)
                                } else {
                                    let ta = type_args.take().expect("type args");
                                    self.arguments_apply(ctx, ta, ident_tree)
                                };
                                if !annos.is_empty() {
                                    let head = *annos.head();
                                    let head_pos = ctx.tree(head).pos;
                                    ident_tree = self.illegal(ctx, head_pos);
                                }
                                type_args = None;
                            }
                            break;
                        }
                        TokenKind::DOT => {
                            self.next_token();
                            if self.token.kind == TokenKind::IDENTIFIER && type_args.is_some() {
                                return self.illegal(ctx, pos);
                            }
                            let oldmode = self.mode;
                            self.mode &= !NOPARAMS;
                            type_args = self.type_arguments_opt_list_mode(ctx, EXPR);
                            self.mode = oldmode;
                            if (self.mode & EXPR) != 0 {
                                match self.token.kind {
                                    TokenKind::CLASS => {
                                        if type_args.is_some() {
                                            return self.illegal(ctx, pos);
                                        }
                                        self.select_expr_mode();
                                        let class_name = self.names._class.clone();
                                        ident_tree = {
                                            let x = self.F.at(pos).select(ctx, ident_tree, class_name);
                                            self.to(x)
                                        };
                                        self.next_token();
                                        break;
                                    }
                                    TokenKind::THIS => {
                                        if type_args.is_some() {
                                            return self.illegal(ctx, pos);
                                        }
                                        self.select_expr_mode();
                                        let this_name = self.names._this.clone();
                                        ident_tree = {
                                            let x = self.F.at(pos).select(ctx, ident_tree, this_name);
                                            self.to(x)
                                        };
                                        self.next_token();
                                        break;
                                    }
                                    TokenKind::SUPER => {
                                        self.select_expr_mode();
                                        let super_name = self.names._super.clone();
                                        ident_tree = {
                                            let x = self.F.at(pos).select(ctx, ident_tree, super_name);
                                            self.to(x)
                                        };
                                        ident_tree = self.super_suffix(ctx, type_args.take(), ident_tree);
                                        type_args = None;
                                        break;
                                    }
                                    TokenKind::NEW => {
                                        if type_args.is_some() {
                                            return self.illegal(ctx, pos);
                                        }
                                        self.select_expr_mode();
                                        let pos1 = self.token.pos;
                                        self.next_token();
                                        if self.token.kind == TokenKind::LT {
                                            type_args = Some(self.type_arguments_list(ctx, false));
                                        }
                                        ident_tree =
                                            self.inner_creator(ctx, pos1, type_args.take(), ident_tree);
                                        type_args = None;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            let mut tyannos: Option<List<TreeId>> = None;
                            if (self.mode & TYPE) != 0 && self.token.kind == TokenKind::MONKEYS_AT {
                                tyannos = Some(self.type_annotations_opt(ctx));
                            }
                            let name = self.ident(ctx);
                            ident_tree = {
                                let x = self.F.at(pos).select(ctx, ident_tree, name);
                                self.to_p(x)
                            };
                            if self.token.pos <= self.end_pos_table.error_end_pos()
                                && self.token.kind == TokenKind::MONKEYS_AT
                            {
                                if type_args.is_some() {
                                    let _ = self.illegal(ctx, pos);
                                }
                                return self.to_p(ident_tree);
                            }
                            if let Some(tyannos) = tyannos {
                                if !tyannos.is_empty() {
                                    let head = *tyannos.head();
                                    let head_pos = ctx.tree(head).pos;
                                    ident_tree = {
                                        let x = self.F.at(head_pos)
                                            .annotated_type(ctx, tyannos, ident_tree);
                                        self.to_p(x)
                                    };
                                }
                            }
                        }
                        TokenKind::ELLIPSIS => {
                            if self.permit_type_annotations_push_back {
                                self.type_annotations_pushed_back = annos.clone();
                            } else if !annos.is_empty() {
                                let head = *annos.head();
                                let head_pos = ctx.tree(head).pos;
                                let _ = self.illegal(ctx, head_pos);
                            }
                            break;
                        }
                        TokenKind::LT => {
                            if (self.mode & TYPE) == 0 && self.is_unbound_member_ref() {
                                let pos1 = self.token.pos;
                                self.accept(ctx, TokenKind::LT);
                                let mut args: Vec<TreeId> = vec![self.type_argument(ctx)];
                                while self.token.kind == TokenKind::COMMA {
                                    self.next_token();
                                    args.push(self.type_argument(ctx));
                                }
                                self.accept(ctx, TokenKind::GT);
                                ident_tree = {
                                    let x = self.F.at(pos1)
                                        .type_apply(ctx, ident_tree, List::from_iterable(args));
                                    self.to_p(x)
                                };
                                while self.token.kind == TokenKind::DOT {
                                    self.next_token();
                                    self.select_type_mode();
                                    let p = self.token.pos;
                                    let name = self.ident(ctx);
                                    ident_tree = {
                                        let x = self.F.at(p).select(ctx, ident_tree, name);
                                        self.to_p(x)
                                    };
                                    ident_tree = self.type_arguments_opt_expr(ctx, ident_tree);
                                }
                                ident_tree = self.brackets_opt_simple(ctx, ident_tree);
                                if self.token.kind != TokenKind::COLCOL {
                                    ident_tree = self.illegal(ctx, self.token.pos);
                                }
                                self.select_expr_mode();
                                return self.term3_rest(ctx, ident_tree, type_args.take());
                            }
                            break;
                        }
                        _ => break,
                    }
                }
                if type_args.is_some() {
                    let _ = self.illegal(ctx, pos);
                }
                ident_tree = self.type_arguments_opt_expr(ctx, ident_tree);
                t = ident_tree;
                }
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
                let basic = self.basic_type(ctx);
                let brackets = self.brackets_opt_simple(ctx, basic);
                t = self.brackets_suffix(ctx, brackets);
            }
            TokenKind::VOID => {
                if type_args.is_some() {
                    let _ = self.illegal(ctx, pos);
                }
                if (self.mode & EXPR) != 0 {
                    self.next_token();
                    if self.token.kind == TokenKind::DOT {
                        let ti = {
                            let x = self.F.at(pos).type_ident(ctx, TypeTag::VOID);
                            self.to_p(x)
                        };
                        t = self.brackets_suffix(ctx, ti);
                    } else {
                        return self.illegal(ctx, pos);
                    }
                } else {
                    let ti = {
                        let x = self.F.at(pos).type_ident(ctx, TypeTag::VOID);
                        self.to(x)
                    };
                    self.next_token();
                    return ti;
                }
            }
            TokenKind::SWITCH => {
                // Java: checkSourceLevel(Feature.SWITCH_EXPRESSION);
                // allowYieldStatement = true; int switchPos = token.pos;
                // nextToken(); JCExpression selector = parExpression();
                // accept(LBRACE); ListBuffer<JCCase> cases = new ListBuffer<>();
                self.check_source_level_at_token(Feature::SWITCH_EXPRESSION);
                self.allow_yield_statement = true;
                let switch_pos = self.token.pos;
                self.next_token();
                let selector = self.par_expression(ctx);
                self.accept(ctx, TokenKind::LBRACE);
                let mut cases: Vec<TreeId> = Vec::new();
                loop {
                    let rule_pos = self.token.pos;
                    match self.token.kind {
                        TokenKind::CASE | TokenKind::DEFAULT => {
                            cases.extend(self.switch_expression_statement_group(ctx));
                        }
                        TokenKind::RBRACE | TokenKind::EOF => {
                            let e = self
                                .F
                                .at(switch_pos)
                                .switch_expression(ctx, selector, List::from_iterable(cases));
                            let endpos = self.token.pos;
                            if let TreeKind::SwitchExpression { endpos: field, .. } =
                                &mut ctx.tree_mut(e).kind
                            {
                                *field = endpos;
                            }
                            self.accept(ctx, TokenKind::RBRACE);
                            return e;
                        }
                        _ => {
                            self.next_token(); // to ensure progress
                            let key = JCDiagnosticError::from(errors::expected3(
                                Self::token_arg(TokenKind::CASE),
                                Self::token_arg(TokenKind::DEFAULT),
                                Self::token_arg(TokenKind::RBRACE),
                            ));
                            let _ = self.syntax_error(ctx, rule_pos, key);
                        }
                    }
                }
            }
            _ => return self.illegal(ctx, pos),
        }
        self.term3_rest(ctx, t, type_args.take())
    }

    /// Java: JCExpression term3Rest(JCExpression t, List<JCExpression> typeArgs)
    fn term3_rest(
        &mut self,
        ctx: &mut CompilerContext,
        mut t: TreeId,
        type_args: Option<List<TreeId>>,
    ) -> TreeId {
        let mut type_args = type_args;
        if type_args.is_some() {
            let t_pos = ctx.tree(t).pos;
            let _ = self.illegal(ctx, t_pos);
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
                        t = {
                            let x = self.F.at(pos1).type_array(ctx, t);
                            self.to_p(x)
                        };
                        if self.token.kind == TokenKind::COLCOL {
                            self.select_expr_mode();
                            continue;
                        }
                        if !annos.is_empty() {
                            t = {
                                let x = self.F.at(pos1).annotated_type(ctx, annos, t);
                                self.to_p(x)
                            };
                        }
                        return t;
                    }
                    self.mode = oldmode;
                }
                if (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    let t1 = self.term(ctx);
                    t = {
                        let x = self.F.at(pos1).indexed(ctx, t, t1);
                        self.to(x)
                    };
                }
                self.accept(ctx, TokenKind::RBRACKET);
            } else if self.token.kind == TokenKind::DOT {
                self.next_token();
                type_args = self.type_arguments_opt_list_mode(ctx, EXPR);
                if self.token.kind == TokenKind::SUPER && (self.mode & EXPR) != 0 {
                    self.select_expr_mode();
                    let super_name = self.names._super.clone();
                    t = {
                        let x = self.F.at(pos1).select(ctx, t, super_name);
                        self.to(x)
                    };
                    self.next_token();
                    t = self.arguments_apply(
                        ctx,
                        type_args.take().unwrap_or_else(List::nil),
                        t,
                    );
                } else if self.token.kind == TokenKind::NEW && (self.mode & EXPR) != 0 {
                    if type_args.is_some() {
                        return self.illegal(ctx, pos1);
                    }
                    self.select_expr_mode();
                    let pos2 = self.token.pos;
                    self.next_token();
                    if self.token.kind == TokenKind::LT {
                        type_args = Some(self.type_arguments_list(ctx, false));
                    }
                    t = self.inner_creator(ctx, pos2, type_args.take(), t);
                } else {
                    let mut tyannos: Option<List<TreeId>> = None;
                    if (self.mode & TYPE) != 0 && self.token.kind == TokenKind::MONKEYS_AT {
                        tyannos = Some(self.type_annotations_opt(ctx));
                    }
                    let name = self.ident_allow_class(ctx, true);
                    t = {
                        let x = self.F.at(pos1).select(ctx, t, name);
                        self.to_p(x)
                    };
                    if self.token.pos <= self.end_pos_table.error_end_pos()
                        && self.token.kind == TokenKind::MONKEYS_AT
                    {
                        break;
                    }
                    if let Some(tyannos) = tyannos {
                        if !tyannos.is_empty() {
                            let head = *tyannos.head();
                            let head_pos = ctx.tree(head).pos;
                            t = {
                                let x = self.F.at(head_pos)
                                    .annotated_type(ctx, tyannos, t);
                                self.to_p(x)
                            };
                        }
                    }
                    let t2 = self.type_arguments_opt_expr(ctx, t);
                    t = self.arguments_opt(ctx, type_args.take(), t2);
                }
            } else if (self.mode & EXPR) != 0 && self.token.kind == TokenKind::COLCOL {
                self.select_expr_mode();
                if type_args.is_some() {
                    return self.illegal(ctx, pos1);
                }
                // Java: t = memberReferenceSuffix(t); (accepts the COLCOL itself)
                t = self.member_reference_suffix(ctx, t);
            } else {
                if !annos.is_empty() {
                    if self.permit_type_annotations_push_back {
                        self.type_annotations_pushed_back = annos;
                    } else {
                        let head = *annos.head();
                        let head_pos = ctx.tree(head).pos;
                        return self.illegal(ctx, head_pos);
                    }
                }
                break;
            }
        }
        while (self.token.kind == TokenKind::PLUSPLUS || self.token.kind == TokenKind::SUBSUB)
            && (self.mode & EXPR) != 0
        {
            self.select_expr_mode();
            let tag = if self.token.kind == TokenKind::PLUSPLUS {
                Tag::POSTINC
            } else {
                Tag::POSTDEC
            };
            let pos = self.token.pos;
            t = {
                let x = self.F.at(pos).unary(ctx, tag, t);
                self.to(x)
            };
            self.next_token();
        }
        self.to_p(t)
    }

    /// Java: JCExpression lambdaExpressionOrStatement(boolean hasParens,
    /// boolean explicitParams, int pos)
    fn lambda_expression_or_statement(
        &mut self,
        ctx: &mut CompilerContext,
        has_parens: bool,
        explicit_params: bool,
        pos: i32,
    ) -> TreeId {
        let params = if explicit_params {
            self.formal_parameters_impl(ctx, true, false)
        } else {
            self.implicit_parameters(ctx, has_parens)
        };
        if explicit_params {
            // Java: LambdaClassifier validation (lambda parameter kinds).
            let mut kind: Option<LambdaParameterKind> = None;
            let mut diag_fragment: Option<FragmentInfo> = None;
            for param in params.iter() {
                let param_pos = ctx.tree(param).pos;
                let (vartype, pname) = match &ctx.tree(param).kind {
                    TreeKind::VariableDecl {
                        vartype,
                        name,
                        ..
                    } => (*vartype, name.clone()),
                    _ => (None, self.names.empty.clone()),
                };
                if let Some(vartype) = vartype {
                    if let Some(restricted) = self.restricted_type_name(ctx, vartype, false)
                        && ctx.tree(vartype).has_tag(Tag::TYPEARRAY)
                    {
                        let key = if Feature::VAR_SYNTAX_IMPLICIT_LAMBDAS
                            .allowed_in_source(self.source)
                        {
                            JCDiagnosticError::from(errors::restricted_type_not_allowed_array(
                                restricted,
                            ))
                        } else {
                            JCDiagnosticError::from(errors::restricted_type_not_allowed_here(
                                restricted,
                            ))
                        };
                        self.log
                            .borrow_mut()
                            .error_flag_at(DiagnosticFlag::SYNTAX, param_pos, key);
                    }
                }
                // Java: LambdaClassifier.addParameter(param)
                if let Some(vartype) = vartype {
                    if pname.to_java_string() != self.names.empty.to_java_string() {
                        let new_kind = if self.restricted_type_name(ctx, vartype, false).is_some()
                        {
                            LambdaParameterKind::Var
                        } else {
                            LambdaParameterKind::Explicit
                        };
                        reduce_lambda_kind(&mut kind, &mut diag_fragment, new_kind, self.source);
                    }
                }
                let implicit = (vartype.is_none()
                    && pname.to_java_string() != self.names.empty.to_java_string())
                    || (vartype.is_some()
                        && pname.to_java_string() == self.names.empty.to_java_string());
                if implicit {
                    reduce_lambda_kind(
                        &mut kind,
                        &mut diag_fragment,
                        LambdaParameterKind::Implicit,
                        self.source,
                    );
                }
                if kind == Some(LambdaParameterKind::Error) {
                    break;
                }
            }
            if let Some(fragment) = diag_fragment {
                let key = JCDiagnosticError::from(errors::invalid_lambda_parameter_declaration(
                    fragment_diagnostic_arg(fragment),
                ));
                self.log
                    .borrow_mut()
                    .error_flag_at(DiagnosticFlag::SYNTAX, pos, key);
            }
            // Java: for (JCVariableDecl param : params) { if (param.vartype != null
            // && restrictedTypeName(param.vartype, true) != null) { checkSourceLevel(
            // param.pos, Feature.VAR_SYNTAX_IMPLICIT_LAMBDAS);
            // param.startPos = TreeInfo.getStartPos(param.vartype); param.vartype = null; } }
            for param in params.iter() {
                let vartype = match &ctx.tree(param).kind {
                    TreeKind::VariableDecl { vartype, .. } => *vartype,
                    _ => None,
                };
                if let Some(vartype) = vartype
                    && self.restricted_type_name(ctx, vartype, true).is_some()
                {
                    let param_pos = ctx.tree(param).pos;
                    self.check_source_level(param_pos, Feature::VAR_SYNTAX_IMPLICIT_LAMBDAS);
                    let start = tree_info::get_start_pos(ctx, vartype);
                    if let TreeKind::VariableDecl {
                        vartype,
                        start_pos,
                        ..
                    } = &mut ctx.tree_mut(param).kind
                    {
                        *vartype = None;
                        *start_pos = start;
                    }
                }
            }
        }
        self.lambda_expression_or_statement_rest(ctx, params, pos)
    }

    /// Java: JCExpression lambdaExpressionOrStatementRest(List<JCVariableDecl> args, int pos)
    fn lambda_expression_or_statement_rest(
        &mut self,
        ctx: &mut CompilerContext,
        args: List<TreeId>,
        pos: i32,
    ) -> TreeId {
        self.check_source_level_at_token(Feature::LAMBDA);
        self.accept(ctx, TokenKind::ARROW);
        if self.token.kind == TokenKind::LBRACE {
            self.lambda_statement(ctx, args, pos, self.token.pos)
        } else {
            self.lambda_expression(ctx, args, pos)
        }
    }

    /// Java: JCExpression lambdaStatement(List<JCVariableDecl> args, int pos, int pos2)
    fn lambda_statement(
        &mut self,
        ctx: &mut CompilerContext,
        args: List<TreeId>,
        pos: i32,
        pos2: i32,
    ) -> TreeId {
        let block = self.block_at(ctx, pos2, 0);
        let t = self.F.at(pos).lambda(ctx, args, block);
        self.to_p(t)
    }

    /// Java: JCExpression lambdaExpression(List<JCVariableDecl> args, int pos)
    fn lambda_expression(
        &mut self,
        ctx: &mut CompilerContext,
        args: List<TreeId>,
        pos: i32,
    ) -> TreeId {
        let expr = self.parse_expression(ctx);
        let t = self.F.at(pos).lambda(ctx, args, expr);
        self.to_p(t)
    }

    /// Java: List<JCVariableDecl> implicitParameters(boolean hasParens)
    fn implicit_parameters(&mut self, ctx: &mut CompilerContext, has_parens: bool) -> List<TreeId> {
        if has_parens {
            self.accept(ctx, TokenKind::LPAREN);
        }
        let mut params: Vec<TreeId> = Vec::new();
        if self.token.kind != TokenKind::RPAREN && self.token.kind != TokenKind::ARROW {
            params.push(self.implicit_parameter(ctx));
            while self.token.kind == TokenKind::COMMA {
                self.next_token();
                params.push(self.implicit_parameter(ctx));
            }
        }
        if has_parens {
            self.accept(ctx, TokenKind::RPAREN);
        }
        List::from_iterable(params)
    }

    /// Java: JCVariableDecl implicitParameter()
    fn implicit_parameter(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let mods = self.F.at(self.token.pos).modifiers(ctx, Flags::PARAMETER);
        self.variable_declarator_id_full(ctx, mods, None, true, false)
    }

    /// Java: boolean isUnboundMemberRef()
    fn is_unbound_member_ref(&mut self) -> bool {
        let mut pos = 0i32;
        let mut depth = 0i32;
        'outer: loop {
            let tk = self.S.lookahead_token(pos).kind;
            match tk {
                TokenKind::IDENTIFIER
                | TokenKind::UNDERSCORE
                | TokenKind::QUES
                | TokenKind::EXTENDS
                | TokenKind::SUPER
                | TokenKind::DOT
                | TokenKind::RBRACKET
                | TokenKind::LBRACKET
                | TokenKind::COMMA
                | TokenKind::BYTE
                | TokenKind::SHORT
                | TokenKind::INT
                | TokenKind::LONG
                | TokenKind::FLOAT
                | TokenKind::DOUBLE
                | TokenKind::BOOLEAN
                | TokenKind::CHAR
                | TokenKind::MONKEYS_AT => {}
                TokenKind::LPAREN => {
                    let mut nesting = 0i32;
                    loop {
                        let tk2 = self.S.lookahead_token(pos).kind;
                        match tk2 {
                            TokenKind::EOF => return false,
                            TokenKind::LPAREN => nesting += 1,
                            TokenKind::RPAREN => {
                                nesting -= 1;
                                if nesting == 0 {
                                    pos += 1;
                                    continue 'outer;
                                }
                            }
                            _ => {}
                        }
                        pos += 1;
                    }
                }
                TokenKind::LT => depth += 1,
                TokenKind::GTGTGT => depth -= 1,
                TokenKind::GTGT => depth -= 1,
                TokenKind::GT => {
                    depth -= 1;
                    if depth == 0 {
                        let next_kind = self.S.lookahead_token(pos + 1).kind;
                        return next_kind == TokenKind::DOT
                            || next_kind == TokenKind::LBRACKET
                            || next_kind == TokenKind::COLCOL;
                    }
                }
                _ => return false,
            }
            pos += 1;
        }
    }

    /// Java: ParensResult analyzeParens()
    fn analyze_parens(&mut self) -> ParensResult {
        let mut depth = 0i32;
        let mut type_seen = false;
        let mut default_result = ParensResult::Parens;
        let mut lookahead = 0i32;
        loop {
            let tk = self.S.lookahead_token(lookahead).kind;
            match tk {
                TokenKind::COMMA => {
                    type_seen = true;
                }
                TokenKind::EXTENDS
                | TokenKind::SUPER
                | TokenKind::DOT
                | TokenKind::AMP => {}
                TokenKind::QUES => {
                    if self.peek_token_at(lookahead, TokenKind::EXTENDS)
                        || self.peek_token_at(lookahead, TokenKind::SUPER)
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
                    if self.peek_token_at(lookahead, TokenKind::RPAREN) {
                        return ParensResult::Cast;
                    } else if self.is_lax_identifier_kind_at(lookahead + 1) {
                        return ParensResult::ExplicitLambda;
                    }
                }
                TokenKind::LPAREN => {
                    if lookahead != 0 {
                        return ParensResult::Parens;
                    } else if self.peek_token_at(lookahead, TokenKind::RPAREN) {
                        return ParensResult::ExplicitLambda;
                    }
                }
                TokenKind::RPAREN => {
                    if type_seen {
                        return ParensResult::Cast;
                    }
                    let next = self.S.lookahead_token(lookahead + 1).kind;
                    match next {
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
                TokenKind::UNDERSCORE
                | TokenKind::ASSERT
                | TokenKind::ENUM
                | TokenKind::IDENTIFIER => {
                    if self.is_lax_identifier_kind_at(lookahead + 1) {
                        return ParensResult::ExplicitLambda;
                    } else if self.peek_token_at2(
                        lookahead,
                        TokenKind::RPAREN,
                        TokenKind::ARROW,
                    ) {
                        return if (self.mode & NOLAMBDA) == 0 {
                            ParensResult::ImplicitLambda
                        } else {
                            ParensResult::Parens
                        };
                    } else if depth == 0 && self.peek_token_at(lookahead, TokenKind::COMMA) {
                        default_result = ParensResult::ImplicitLambda;
                    }
                    type_seen = false;
                }
                TokenKind::FINAL | TokenKind::ELLIPSIS => {
                    return ParensResult::ExplicitLambda;
                }
                TokenKind::MONKEYS_AT => {
                    type_seen = true;
                    lookahead = self.skip_annotation(lookahead);
                }
                TokenKind::LBRACKET => {
                    if self.peek_token_at(lookahead, TokenKind::RBRACKET)
                        && self.is_lax_identifier_kind_at(lookahead + 2)
                    {
                        return ParensResult::ExplicitLambda;
                    } else if self.peek_token_at2(
                        lookahead,
                        TokenKind::RBRACKET,
                        TokenKind::RPAREN,
                    ) || self.peek_token_at2(
                        lookahead,
                        TokenKind::RBRACKET,
                        TokenKind::AMP,
                    ) {
                        return ParensResult::Cast;
                    } else if self.peek_token_at(lookahead, TokenKind::RBRACKET) {
                        type_seen = true;
                        lookahead += 1;
                    } else {
                        return ParensResult::Parens;
                    }
                }
                TokenKind::LT => depth += 1,
                TokenKind::GTGTGT | TokenKind::GTGT | TokenKind::GT => {
                    depth -= if tk == TokenKind::GTGTGT {
                        3
                    } else if tk == TokenKind::GTGT {
                        2
                    } else {
                        1
                    };
                    if depth == 0 {
                        if self.peek_token_at(lookahead, TokenKind::RPAREN)
                            || self.peek_token_at(lookahead, TokenKind::AMP)
                        {
                            return ParensResult::Cast;
                        } else if (self.is_lax_identifier_kind_at(lookahead + 1)
                            && self.peek_token_at(lookahead, TokenKind::COMMA))
                            || (self.is_lax_identifier_kind_at(lookahead + 1)
                                && self.peek_token_at2(
                                    lookahead,
                                    TokenKind::RPAREN,
                                    TokenKind::ARROW,
                                ))
                            || self.peek_token_at(lookahead, TokenKind::ELLIPSIS)
                        {
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

    /// Java: LAX_IDENTIFIER predicate applied to S.token(lookahead).
    fn is_lax_identifier_kind_at(&mut self, lookahead: i32) -> bool {
        Self::is_lax_identifier(self.S.lookahead_token(lookahead).kind)
    }

    /// Java: private int skipAnnotation(int lookahead)
    fn skip_annotation(&mut self, mut lookahead: i32) -> i32 {
        lookahead += 1; // skip '@'
        while self.peek_token_at(lookahead, TokenKind::DOT) {
            lookahead += 2;
        }
        if self.peek_token_at(lookahead, TokenKind::LPAREN) {
            lookahead += 1;
            let mut nesting = 0i32;
            loop {
                let tk2 = self.S.lookahead_token(lookahead).kind;
                match tk2 {
                    TokenKind::EOF => return lookahead,
                    TokenKind::LPAREN => nesting += 1,
                    TokenKind::RPAREN => {
                        nesting -= 1;
                        if nesting == 0 {
                            return lookahead;
                        }
                    }
                    _ => {}
                }
                lookahead += 1;
            }
        }
        lookahead
    }

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
        let pos = self.token.pos;
        let name = self.ident(ctx);
        if let Some(source) = self.restricted_type_name_starting_at_source(&name, pos, true) {
            let key = JCDiagnosticError::from(errors::restricted_type_not_allowed(
                name.clone(),
                SourceArg::placeholder(source.to_string()),
            ));
            self.report_syntax_error(ctx, pos, key);
        }
        name
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
        vdefs =
            self.variable_declarators_rest(ctx, pos, mods, type_, name, false, None, vdefs, local_decl);
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
        dc: Option<Rc<dyn Comment>>,
        mut vdefs: Vec<TreeId>,
        local_decl: bool,
    ) -> Vec<TreeId> {
        let head = self.variable_declarator_rest(
            ctx, pos, mods, type_, name, req_init, dc.clone(), local_decl, false,
        );
        vdefs.push(head);
        while self.token.kind == TokenKind::COMMA {
            let last = *vdefs.last().expect("declarator");
            let end = self.token.end_pos;
            self.store_end(last, end);
            self.next_token();
            let pos = self.token.pos;
            let name = self.ident(ctx);
            let def = self.variable_declarator_rest(
                ctx, pos, mods, type_, name, req_init, dc.clone(), local_decl, true,
            );
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
        dc: Option<Rc<dyn Comment>>,
        local_decl: bool,
        compound: bool,
    ) -> TreeId {
        let mut declared_using_var = false;
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
        let elem_type = tree_info::innermost_type(ctx, type_, true).expect("type has no innermost");
        let mut type_opt = Some(type_);
        let mut start_pos = NOPOS;
        if ctx.tree(elem_type).has_tag(Tag::IDENT) {
            let type_name = match &ctx.tree(elem_type).kind {
                TreeKind::Ident { name, .. } => name.clone(),
                _ => unreachable!(),
            };
            if self
                .restricted_type_name_starting_at_source(
                    &type_name,
                    pos,
                    !compound && local_decl,
                )
                .is_some()
            {
                if type_name.to_java_string() != self.names.var.to_java_string() {
                    let elem_pos = ctx.tree(elem_type).pos;
                    let key = JCDiagnosticError::from(errors::restricted_type_not_allowed_here(
                        type_name,
                    ));
                    let _ = self.syntax_error(ctx, elem_pos, key);
                } else if ctx.tree(type_).has_tag(Tag::TYPEARRAY) && !compound {
                    let elem_pos = ctx.tree(elem_type).pos;
                    let key = JCDiagnosticError::from(errors::restricted_type_not_allowed_array(
                        type_name,
                    ));
                    let _ = self.syntax_error(ctx, elem_pos, key);
                } else {
                    declared_using_var = true;
                    if compound {
                        let elem_pos = ctx.tree(elem_type).pos;
                        let key = JCDiagnosticError::from(
                            errors::restricted_type_not_allowed_compound(type_name),
                        );
                        let _ = self.syntax_error(ctx, elem_pos, key);
                    }
                    start_pos = tree_info::get_start_pos(ctx, mods);
                    if start_pos == NOPOS {
                        start_pos = tree_info::get_start_pos(ctx, type_);
                    }
                    // implicit type
                    type_opt = None;
                }
            }
        }
        let def = {
            let __to_p_tree = self.F.at(pos).var_def_declared_using_var(
                ctx,
                mods,
                name,
                type_opt,
                init,
                declared_using_var,
            );
            self.to_p(__to_p_tree)
        };
        if let TreeKind::VariableDecl { start_pos: field, .. } = &mut ctx.tree_mut(def).kind {
            *field = start_pos;
        }
        self.attach(def, dc);
        def
    }

    /// Java: JCVariableDecl variableDeclaratorId(JCModifiers mods, JCExpression type)
    /// Java: JCVariableDecl variableDeclaratorId(JCModifiers mods, JCExpression type)
    fn variable_declarator_id(&mut self, ctx: &mut CompilerContext, mods: TreeId, type_: TreeId) -> TreeId {
        self.variable_declarator_id_full(ctx, mods, Some(type_), false, false)
    }

    /* ---------- blocks and statements -------------- */

    /// Java: JCBlock block(int pos, long flags)
    fn block_at(&mut self, ctx: &mut CompilerContext, pos: i32, flags: i64) -> TreeId {
        self.accept(ctx, TokenKind::LBRACE);
        let stats = self.block_statements(ctx);
        let t = self.F.at(pos).block(ctx, flags, stats);
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
                let dc = self.token.comment(CommentStyle::JAVADOC);
                let mods = self.modifiers_opt(ctx);
                if matches!(
                    self.token.kind,
                    TokenKind::INTERFACE | TokenKind::CLASS | TokenKind::ENUM
                ) || self.is_record_start()
                {
                    return List::of(
                        self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc),
                    );
                }
                let t = self.parse_type_impl(ctx, true);
                List::from_iterable(self.local_variable_declarations(ctx, mods, t))
            }
            TokenKind::ABSTRACT | TokenKind::STRICTFP => {
                let dc = self.token.comment(CommentStyle::JAVADOC);
                let mods = self.modifiers_opt(ctx);
                List::of(self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc))
            }
            TokenKind::INTERFACE | TokenKind::CLASS => {
                let dc = self.token.comment(CommentStyle::JAVADOC);
                let mods = self.modifiers_opt(ctx);
                List::of(self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc))
            }
            TokenKind::ENUM => {
                if !self.allow_records {
                    let pos_err = self.token.pos;
                    let key = JCDiagnosticError::from(errors::local_enum());
                    self.log.borrow_mut().error_at(pos_err, key);
                }
                let dc = self.token.comment(CommentStyle::JAVADOC);
                let mods = self.modifiers_opt(ctx);
                List::of(self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc))
            }
            TokenKind::IDENTIFIER => {
                if self.token.name() == &self.names.r#yield && self.allow_yield_statement {
                    let is_yield_statement = self.is_yield_statement_lookahead();
                    if is_yield_statement {
                        self.next_token();
                        let t = self.term_with_mode(ctx, EXPR);
                        self.accept(ctx, TokenKind::SEMI);
                        let y = self.F.at(pos).yield_(ctx, t);
                        let y = self.to_p(y);
                        return List::of(y);
                    }
                } else {
                    if self.is_non_sealed_class_start(true) {
                        let err_pos = self.token.pos;
                        let key = JCDiagnosticError::from(
                            errors::sealed_or_non_sealed_local_classes_not_allowed(),
                        );
                        self.log.borrow_mut().error_at(err_pos, key);
                        self.next_token();
                        self.next_token();
                        self.next_token();
                        let mods = self.modifiers_opt(ctx);
                        let dc = self.token.comment(CommentStyle::JAVADOC);
                        return List::of(
                            self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc),
                        );
                    } else if self.is_sealed_class_start(true) {
                        let err_pos = self.token.pos;
                        self.check_source_level(err_pos, Feature::SEALED_CLASSES);
                        let key = JCDiagnosticError::from(
                            errors::sealed_or_non_sealed_local_classes_not_allowed(),
                        );
                        self.log.borrow_mut().error_at(err_pos, key);
                        self.next_token();
                        let mods = self.modifiers_opt(ctx);
                        let dc = self.token.comment(CommentStyle::JAVADOC);
                        return List::of(
                            self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc),
                        );
                    }
                }
                self.block_statement_default(ctx, pos)
            }
            _ => self.block_statement_default(ctx, pos),
        }
    }

    /// Java: the code after the blockStatement switch (yield fall-through,
    /// sealed/non-sealed local class errors, record declarations and the
    /// label / local-variable / expression-statement disambiguation).
    fn block_statement_default(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
    ) -> List<TreeId> {
        if self.is_record_start() && self.allow_records {
            // Java: dc = token.comment(JAVADOC);
            // return List.of(recordDeclaration(F.at(pos).Modifiers(0), dc));
            let dc = self.token.comment(CommentStyle::JAVADOC);
            let mods = self.F.at(pos).modifiers(ctx, 0);
            return List::of(self.record_declaration(ctx, mods, dc));
        }
        let prev_token = self.token.clone();
        let t = self.term_with_mode(ctx, EXPR | TYPE);
        if self.token.kind == TokenKind::COLON && ctx.tree(t).tag == Tag::IDENT {
            self.next_token();
            let stat = self.parse_statement_as_block(ctx);
            let label = prev_token.name().clone();
            return List::of(self.F.at(pos).labelled(ctx, label, stat));
        }
        if (self.last_mode & TYPE) != 0 && Self::is_lax_identifier(self.token.kind) {
            let mods = self.F.at(NOPOS).modifiers(ctx, 0);
            return List::from_iterable(self.local_variable_declarations(ctx, mods, t));
        }
        let t = self.check_expr_stat(ctx, t);
        self.accept(ctx, TokenKind::SEMI);
        let exec = self.F.at(pos).exec(ctx, t);
        let exec = self.to_p(exec);
        List::of(exec)
    }

    /// Java: the isYieldStatement lookahead computation in blockStatement's
    /// IDENTIFIER case.
    fn is_yield_statement_lookahead(&mut self) -> bool {
        let next = self.S.lookahead_token(1).clone();
        match next.kind {
            TokenKind::PLUS
            | TokenKind::SUB
            | TokenKind::STRINGLITERAL
            | TokenKind::CHARLITERAL
            | TokenKind::INTLITERAL
            | TokenKind::LONGLITERAL
            | TokenKind::FLOATLITERAL
            | TokenKind::DOUBLELITERAL
            | TokenKind::NULL
            | TokenKind::IDENTIFIER
            | TokenKind::TRUE
            | TokenKind::FALSE
            | TokenKind::NEW
            | TokenKind::SWITCH
            | TokenKind::THIS
            | TokenKind::SUPER => true,
            TokenKind::PLUSPLUS | TokenKind::SUBSUB => {
                self.S.lookahead_token(2).kind != TokenKind::SEMI
            }
            TokenKind::BANG | TokenKind::TILDE => {
                self.S.lookahead_token(1).kind != TokenKind::SEMI
            }
            TokenKind::LPAREN => {
                let mut lookahead = 2;
                let mut balance = 1;
                let mut has_comma = false;
                loop {
                    let l = self.S.lookahead_token(lookahead).clone();
                    if l.kind == TokenKind::EOF || balance == 0 {
                        return (!has_comma && lookahead != 3) || l.kind == TokenKind::ARROW;
                    }
                    match l.kind {
                        TokenKind::LPAREN => balance += 1,
                        TokenKind::RPAREN => balance -= 1,
                        TokenKind::COMMA => {
                            if balance == 1 {
                                has_comma = true;
                            }
                        }
                        _ => {}
                    }
                    lookahead += 1;
                }
            }
            TokenKind::SEMI => true,
            _ => false,
        }
    }

    /// Java: protected boolean isNonSealedClassStart(boolean local)
    fn is_non_sealed_class_start(&mut self, local: bool) -> bool {
        if self.is_non_sealed_identifier_at(0) {
            let next = self.S.lookahead_token(3).clone();
            return self.allowed_after_sealed_or_non_sealed(&next, local, true);
        }
        false
    }

    /// Java: protected boolean isNonSealedIdentifier(Token someToken, int
    /// lookAheadOffset)
    fn is_non_sealed_identifier_at(&mut self, look_ahead_offset: i32) -> bool {
        let some_token = if look_ahead_offset == 0 {
            self.token.clone()
        } else {
            self.S.lookahead_token(look_ahead_offset).clone()
        };
        self.is_non_sealed_identifier(&some_token, look_ahead_offset)
    }

    /// Java: protected boolean isNonSealedIdentifier(Token someToken, int
    /// lookAheadOffset) using the given token.
    fn is_non_sealed_identifier(&mut self, some_token: &Token, look_ahead_offset: i32) -> bool {
        if some_token.name() == &self.names.non
            && self.peek_token_at2(look_ahead_offset, TokenKind::SUB, TokenKind::IDENTIFIER)
        {
            let token_sub = self.S.lookahead_token(look_ahead_offset + 1).clone();
            let token_sealed = self.S.lookahead_token(look_ahead_offset + 2).clone();
            if some_token.end_pos == token_sub.pos
                && token_sub.end_pos == token_sealed.pos
                && token_sealed.name() == &self.names.sealed
            {
                self.check_source_level(some_token.pos, Feature::SEALED_CLASSES);
                return true;
            }
        }
        false
    }

    /// Java: protected boolean isSealedClassStart(boolean local)
    fn is_sealed_class_start(&mut self, local: bool) -> bool {
        if self.token.name() == &self.names.sealed {
            let next = self.S.lookahead_token(1).clone();
            if self.allowed_after_sealed_or_non_sealed(&next, local, false) {
                self.check_source_level(self.token.pos, Feature::SEALED_CLASSES);
                return true;
            }
        }
        false
    }

    /// Java: private boolean allowedAfterSealedOrNonSealed(Token next, boolean
    /// local, boolean currentIsNonSealed)
    fn allowed_after_sealed_or_non_sealed(
        &mut self,
        next: &Token,
        local: bool,
        current_is_non_sealed: bool,
    ) -> bool {
        if local {
            match next.kind {
                TokenKind::MONKEYS_AT => {
                    let after_next = self.S.lookahead_token(2).clone();
                    after_next.kind != TokenKind::INTERFACE || current_is_non_sealed
                }
                TokenKind::ABSTRACT
                | TokenKind::FINAL
                | TokenKind::STRICTFP
                | TokenKind::CLASS
                | TokenKind::INTERFACE
                | TokenKind::ENUM => true,
                _ => false,
            }
        } else {
            match next.kind {
                TokenKind::MONKEYS_AT => {
                    let after_next = self.S.lookahead_token(2).clone();
                    after_next.kind != TokenKind::INTERFACE || current_is_non_sealed
                }
                TokenKind::PUBLIC
                | TokenKind::PROTECTED
                | TokenKind::PRIVATE
                | TokenKind::ABSTRACT
                | TokenKind::STATIC
                | TokenKind::FINAL
                | TokenKind::STRICTFP
                | TokenKind::CLASS
                | TokenKind::INTERFACE
                | TokenKind::ENUM => true,
                TokenKind::IDENTIFIER => {
                    let offset = if current_is_non_sealed { 3 } else { 1 };
                    let next_owned = next.clone();
                    self.is_non_sealed_identifier(&next_owned, offset)
                        || next.name() == &self.names.sealed
                }
                _ => false,
            }
        }
    }

    /// Java: List<JCStatement> forInit()
    fn for_init(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let stats: Vec<TreeId> = Vec::new();
        let pos = self.token.pos;
        if self.token.kind == TokenKind::FINAL || self.token.kind == TokenKind::MONKEYS_AT {
            let mods = self.opt_final(ctx, 0);
            let t = self.parse_type_impl(ctx, true);
            return List::from_iterable(self.variable_declarators(ctx, mods, t, stats, true));
        }
        let t = self.term_with_mode(ctx, EXPR | TYPE);
        if (self.last_mode & TYPE) != 0 && Self::is_lax_identifier(self.token.kind) {
            let mods = self.modifiers_opt(ctx);
            return List::from_iterable(self.variable_declarators(ctx, mods, t, stats, true));
        }
        if (self.last_mode & TYPE) != 0 && self.token.kind == TokenKind::COLON {
            let key = JCDiagnosticError::from(errors::bad_initializer(JavaString::from("for-loop")));
            self.log
                .borrow_mut()
                .error_flag_at(DiagnosticFlag::SYNTAX, pos, key);
            let mods = self.modifiers_opt(ctx);
            let err_name = self.names.error.clone();
            let vdef = self.F.at(pos).var_def(ctx, mods, err_name, Some(t), None);
            return List::of(vdef);
        }
        List::from_iterable(self.more_statement_expressions(ctx, pos, t))
    }

    /// Java: ((JCVariableDecl) inits.head).init == null used by the for-statement
    /// enhanced-for detection.
    fn var_def_has_no_init(ctx: &CompilerContext, tree: TreeId) -> bool {
        match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { init, .. } => init.is_none(),
            _ => false,
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
                let inits: List<TreeId> = if self.token.kind == TokenKind::SEMI {
                    List::nil()
                } else {
                    self.for_init(ctx)
                };
                if inits.length() == 1
                    && ctx.tree(*inits.head()).tag == Tag::VARDEF
                    && Self::var_def_has_no_init(ctx, *inits.head())
                    && self.token.kind == TokenKind::COLON
                {
                    let var = *inits.head();
                    self.accept(ctx, TokenKind::COLON);
                    let expr = self.parse_expression(ctx);
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
                self.F.at(pos).for_loop(ctx, inits, cond, steps, body)
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
        let err = self.syntax_error_list(ctx, start_pos, List::of(stm), error_key);
        { let __to_p_tree = self.F.at(start_pos).exec(ctx, err); self.to_p(__to_p_tree) }
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

    /// Java: private List<JCCase> switchExpressionStatementGroup()
    fn switch_expression_statement_group(&mut self, ctx: &mut CompilerContext) -> Vec<TreeId> {
        let case_pos = self.token.pos;
        let mut pats: Vec<TreeId> = Vec::new();
        if self.token.kind == TokenKind::DEFAULT {
            self.next_token();
            let label = {
                let x = self.F.at(case_pos).default_case_label(ctx);
                self.to_p(x)
            };
            pats.push(label);
        } else {
            self.accept(ctx, TokenKind::CASE);
            loop {
                let label = self.parse_case_label(ctx);
                pats.push(label);
                if self.token.kind != TokenKind::COMMA {
                    break;
                }
                self.check_source_level_at_token(Feature::SWITCH_MULTIPLE_CASE_LABELS);
                self.next_token();
            }
        }
        let stats: List<TreeId>;
        let body: Option<TreeId>;
        let kind: CaseKind;
        match self.token.kind {
            TokenKind::ARROW => {
                self.check_source_level_at_token(Feature::SWITCH_RULE);
                self.next_token();
                if self.token.kind == TokenKind::THROW || self.token.kind == TokenKind::LBRACE {
                    let statement = self.parse_statement(ctx);
                    stats = List::of(statement);
                    body = Some(statement);
                    kind = CaseKind::RULE;
                } else {
                    let value = self.parse_expression(ctx);
                    let value_pos = ctx.tree(value).pos;
                    let yielded = {
                        let x = self.F.at(value_pos).yield_(ctx, value);
                        self.to(x)
                    };
                    stats = List::of(yielded);
                    body = Some(value);
                    kind = CaseKind::RULE;
                    self.accept(ctx, TokenKind::SEMI);
                }
            }
            _ => {
                self.accept_error(
                    ctx,
                    TokenKind::COLON,
                    JCDiagnosticError::from(errors::expected2(
                        Self::token_arg(TokenKind::COLON),
                        Self::token_arg(TokenKind::ARROW),
                    )),
                );
                stats = self.block_statements(ctx);
                kind = CaseKind::STATEMENT;
                body = None;
            }
        }
        let c = self
            .F
            .at(case_pos)
            .case(ctx, kind, List::from_iterable(pats), stats, body);
        vec![self.to_p(c)]
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
        let stats;
        let c;
        let mut cases: Vec<TreeId> = Vec::new();
        match self.token.kind {
            TokenKind::CASE => {
                self.next_token();
                let mut pats: Vec<TreeId> = Vec::new();
                loop {
                    pats.push(self.parse_case_label(ctx));
                    if self.token.kind != TokenKind::COMMA {
                        break;
                    }
                    self.next_token();
                    self.check_source_level_at_token(Feature::SWITCH_MULTIPLE_CASE_LABELS);
                }
                let case_kind;
                let mut body: Option<TreeId> = None;
                if self.token.kind == TokenKind::ARROW {
                    self.check_source_level_at_token(Feature::SWITCH_RULE);
                    self.accept(ctx, TokenKind::ARROW);
                    case_kind = CaseKind::RULE;
                    let statement = self.parse_statement_as_block(ctx);
                    let tag = ctx.tree(statement).tag;
                    if tag != Tag::EXEC && tag != Tag::BLOCK && tag != Tag::THROW {
                        let stmt_pos = ctx.tree(statement).pos;
                        let key = JCDiagnosticError::from(
                            errors::switch_case_unexpected_statement(),
                        );
                        self.log.borrow_mut().error_at(stmt_pos, key);
                    }
                    stats = List::of(statement);
                    body = Some(statement);
                } else {
                    self.accept_error(
                        ctx,
                        TokenKind::COLON,
                        JCDiagnosticError::from(errors::expected2(
                            Self::token_arg(TokenKind::COLON),
                            Self::token_arg(TokenKind::ARROW),
                        )),
                    );
                    case_kind = CaseKind::STATEMENT;
                    stats = self.block_statements(ctx);
                }
                c = self.F.at(pos).case(
                    ctx,
                    case_kind,
                    List::from_iterable(pats),
                    stats.clone(),
                    body,
                );
                let c = self.to_p(c);
                if stats.is_empty() {
                    let end = self.prev_token().end_pos;
                    self.store_end(c, end);
                }
                cases.push(c);
                cases
            }
            TokenKind::DEFAULT => {
                self.next_token();
                let case_kind;
                let pattern_pos = self.token.pos;
                let mut body: Option<TreeId> = None;
                if self.token.kind == TokenKind::ARROW {
                    self.check_source_level_at_token(Feature::SWITCH_RULE);
                    self.accept(ctx, TokenKind::ARROW);
                    case_kind = CaseKind::RULE;
                    let statement = self.parse_statement_as_block(ctx);
                    let tag = ctx.tree(statement).tag;
                    if tag != Tag::EXEC && tag != Tag::BLOCK && tag != Tag::THROW {
                        let stmt_pos = ctx.tree(statement).pos;
                        let key = JCDiagnosticError::from(
                            errors::switch_case_unexpected_statement(),
                        );
                        self.log.borrow_mut().error_at(stmt_pos, key);
                    }
                    stats = List::of(statement);
                    body = Some(statement);
                } else {
                    self.accept_error(
                        ctx,
                        TokenKind::COLON,
                        JCDiagnosticError::from(errors::expected2(
                            Self::token_arg(TokenKind::COLON),
                            Self::token_arg(TokenKind::ARROW),
                        )),
                    );
                    case_kind = CaseKind::STATEMENT;
                    stats = self.block_statements(ctx);
                }
                let default_pattern = {
                    let x = self.F.at(pattern_pos).default_case_label(ctx);
                    self.to_p(x)
                };
                c = self.F.at(pos).case(
                    ctx,
                    case_kind,
                    List::of(default_pattern),
                    stats.clone(),
                    body,
                );
                let c = self.to_p(c);
                if stats.is_empty() {
                    let end = self.prev_token().end_pos;
                    self.store_end(c, end);
                }
                cases.push(c);
                cases
            }
            _ => {
                panic!("java.lang.AssertionError: should not reach here");
            }
        }
    }

    /// Java: private JCCaseLabel parseCaseLabel()
    fn parse_case_label(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let pattern_pos = self.token.pos;
        if self.token.kind == TokenKind::DEFAULT {
            self.check_source_level_at_token(Feature::PATTERN_SWITCH);
            self.next_token();
            let label = {
                let x = self.F.at(pattern_pos).default_case_label(ctx);
                self.to_p(x)
            };
            return label;
        }
        let mut lookahead = 0i32;
        while self.S.lookahead_token(lookahead).kind == TokenKind::LPAREN {
            lookahead += 1;
        }
        let mods = self.opt_final(ctx, 0);
        let (mods_flags, mods_annotations_empty) = match &ctx.tree(mods).kind {
            TreeKind::Modifiers { flags, annotations, .. } => (*flags, annotations.is_empty()),
            _ => (0, true),
        };
        let pattern = mods_flags != 0
            || !mods_annotations_empty
            || self.analyze_pattern(lookahead) == PatternResult::Pattern;
        if pattern {
            self.check_source_level_at_token(Feature::PATTERN_SWITCH);
            // Java: return parsePattern(patternPos, mods, null, false);
            return self.parse_pattern(ctx, pattern_pos, Some(mods), None, false);
        }
        self.term_with_mode(ctx, EXPR | NOLAMBDA)
    }

    /// Java: PatternResult analyzePattern(int lookahead)
    fn analyze_pattern(&mut self, mut lookahead: i32) -> PatternResult {
        let mut depth = 0i32;
        loop {
            let token = self.S.lookahead_token(lookahead).kind;
            match token {
                TokenKind::BYTE
                | TokenKind::SHORT
                | TokenKind::INT
                | TokenKind::LONG
                | TokenKind::FLOAT
                | TokenKind::DOUBLE
                | TokenKind::BOOLEAN
                | TokenKind::CHAR
                | TokenKind::VOID
                | TokenKind::ASSERT
                | TokenKind::ENUM
                | TokenKind::IDENTIFIER
                | TokenKind::UNDERSCORE => {
                    if depth == 0
                        && Self::is_lax_identifier(self.S.lookahead_token(lookahead + 1).kind)
                    {
                        return PatternResult::Pattern;
                    }
                }
                TokenKind::DOT
                | TokenKind::QUES
                | TokenKind::EXTENDS
                | TokenKind::SUPER
                | TokenKind::COMMA => {}
                TokenKind::LT => depth += 1,
                TokenKind::GTGTGT => depth -= 1,
                TokenKind::GTGT => depth -= 1,
                TokenKind::GT => {
                    depth -= 1;
                    if depth == 0 {
                        return if Self::is_lax_identifier(
                            self.S.lookahead_token(lookahead + 1).kind,
                        ) {
                            PatternResult::Pattern
                        } else {
                            PatternResult::Expression
                        };
                    } else if depth < 0 {
                        return PatternResult::Expression;
                    }
                }
                TokenKind::MONKEYS_AT => {
                    lookahead = self.skip_annotation(lookahead);
                }
                TokenKind::LBRACKET => {
                    if self.S.lookahead_token(lookahead + 1).kind == TokenKind::RBRACKET
                        && Self::is_lax_identifier(
                            self.S.lookahead_token(lookahead + 2).kind,
                        )
                    {
                        return PatternResult::Pattern;
                    } else if self.S.lookahead_token(lookahead + 1).kind == TokenKind::RBRACKET {
                        lookahead += 1;
                    } else {
                        return PatternResult::Expression;
                    }
                }
                _ => return PatternResult::Expression,
            }
            lookahead += 1;
        }
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
            return self.variable_declarator_rest(ctx, pos, mods, t, name, true, None, true, false);
        }
        let t = self.term_with_mode(ctx, EXPR | TYPE);
        if (self.last_mode & TYPE) != 0 && Self::is_lax_identifier(self.token.kind) {
            let mods = self.F.at(start_pos).modifiers(ctx, Flags::FINAL);
            let pos = self.token.pos;
            let name = self.ident(ctx);
            return self.variable_declarator_rest(ctx, pos, mods, t, name, true, None, true, false);
        }
        let check_pos = self.token.pos;
        self.check_source_level(
            check_pos,
            Feature::EFFECTIVELY_FINAL_VARIABLES_IN_TRY_WITH_RESOURCES,
        );
        if ctx.tree(t).tag != Tag::IDENT && ctx.tree(t).tag != Tag::SELECT {
            let pos = ctx.tree(t).pos;
            let key = JCDiagnosticError::from(errors::try_with_resources_expr_needs_var());
            self.log.borrow_mut().error_at(pos, key);
        }
        t
    }


    /* ---------- class/interface declarations -------------- */

    /// Java: List<JCExpression> permitsClause(JCModifiers mods, String classOrInterface)
    fn permits_clause(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        class_or_interface: &str,
    ) -> List<TreeId> {
        if self.allow_sealed_types
            && self.token.kind == TokenKind::IDENTIFIER
            && self.token.name().to_java_string() == self.names.permits.to_java_string()
        {
            self.check_source_level(self.token.pos, Feature::SEALED_CLASSES);
            let mods_flags = match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => *flags,
                _ => 0,
            };
            if (mods_flags & Flags::SEALED) == 0 {
                // Java: Errors.InvalidPermitsClause(Fragments.ClassIsNotSealed(classOrInterface)).
                let class_fragment = fragments::class_is_not_sealed(JavaString::from(class_or_interface));
                let key = JCDiagnosticError::from(errors::invalid_permits_clause(
                    fragment_diagnostic_arg(class_fragment),
                ));
                self.log.borrow_mut().error_at(self.token.pos, key);
            }
            self.next_token();
            return self.qualident_list(ctx, false);
        }
        List::nil()
    }

    /// Java: protected JCClassDecl recordDeclaration(JCModifiers mods, Comment dc)
    fn record_declaration(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        dc: Option<Rc<dyn Comment>>,
    ) -> TreeId {
        let pos = self.token.pos;
        self.next_token();
        if let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind {
            *flags |= Flags::RECORD;
        }
        let name = self.type_name(ctx);
        let typarams = self.type_parameters_opt(ctx);
        let header_fields = self.formal_parameters_impl(ctx, false, true);
        let mut implementing = List::nil();
        if self.token.kind == TokenKind::IMPLEMENTS {
            self.next_token();
            implementing = self.type_list(ctx);
        }
        let defs = self.class_interface_body(ctx, name.clone(), false, true);
        // Java: for (JCTree def : defs) { if (def.hasTag(METHODDEF)) { ... } }
        let mut defs_vec: Vec<TreeId> = defs.iter().collect();
        for def in defs_vec.iter_mut() {
            if !ctx.tree(*def).has_tag(Tag::METHODDEF) {
                continue;
            }
            let (is_init, params_empty, is_compact) = match &ctx.tree(*def).kind {
                TreeKind::MethodDecl {
                    name: mname,
                    params,
                    mods: mmods,
                    ..
                } => {
                    let mflags = match mmods {
                        Some(m) => match &ctx.tree(*m).kind {
                            TreeKind::Modifiers { flags, .. } => *flags,
                            _ => 0,
                        },
                        None => 0,
                    };
                    (
                        mname.to_java_string() == self.names.init.to_java_string(),
                        params.is_empty(),
                        (mflags & Flags::COMPACT_RECORD_CONSTRUCTOR) != 0,
                    )
                }
                _ => (false, false, false),
            };
            if !(is_init && params_empty && is_compact) {
                continue;
            }
            let mut tmp_params: Vec<TreeId> = Vec::new();
            for param in header_fields.iter() {
                let param_pos = ctx.tree(param).pos;
                let (param_flags, param_annos, param_name, param_vartype) =
                    match &ctx.tree(param).kind {
                        TreeKind::VariableDecl {
                            mods: Some(pm),
                            name,
                            vartype,
                            ..
                        } => {
                            let (pf, pa) = match &ctx.tree(*pm).kind {
                                TreeKind::Modifiers { flags, annotations } => {
                                    (*flags, annotations.clone())
                                }
                                _ => (0, List::nil()),
                            };
                            (pf, pa, name.clone(), *vartype)
                        }
                        _ => panic!("expected record header variable declaration"),
                    };
                // Java: F.at(param).VarDef(F.Modifiers(PARAMETER | GENERATED_MEMBER
                //        | param.mods.flags & VARARGS, param.mods.annotations),
                //        param.name, param.vartype, null)
                let generated_flags =
                    Flags::PARAMETER | Flags::GENERATED_MEMBER | (param_flags & Flags::VARARGS);
                let generated_mods =
                    self.F.at(param_pos).modifiers_with_annotations(ctx, generated_flags, param_annos);
                let generated =
                    self.F.at(param_pos).var_def(ctx, generated_mods, param_name, param_vartype, None);
                tmp_params.push(generated);
            }
            if let TreeKind::MethodDecl { params, .. } = &mut ctx.tree_mut(*def).kind {
                *params = List::from_iterable(tmp_params);
            }
        }
        let mut defs_list = List::from_iterable(defs_vec);
        // Java: for (int i = fields.size() - 1; i >= 0; i--) defs = defs.prepend(fields.get(i));
        let header_vec: Vec<TreeId> = header_fields.iter().collect();
        for field in header_vec.iter().rev() {
            defs_list = defs_list.prepend(*field);
        }
        let result =
            self.F.at(pos).class_def(ctx, mods, name, typarams, None, implementing, defs_list);
        let result = self.to_p(result);
        self.attach(result, dc);
        result
    }

    /// Java: protected JCClassDecl classDeclaration(JCModifiers mods, Comment dc)
    fn class_declaration(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        dc: Option<Rc<dyn Comment>>,
    ) -> TreeId {
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
        let permitting = self.permits_clause(ctx, mods, "class");
        let defs = self.class_interface_body(ctx, name.clone(), false, false);
        let result = self.F.at(pos).class_def_permitting(
            ctx, mods, name, typarams, extending, implementing, permitting, defs,
        );
        let result = self.to_p(result);
        self.attach(result, dc);
        result
    }

    /// Java: protected JCClassDecl interfaceDeclaration(JCModifiers mods, Comment dc)
    fn interface_declaration(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        dc: Option<Rc<dyn Comment>>,
    ) -> TreeId {
        let pos = self.token.pos;
        self.accept(ctx, TokenKind::INTERFACE);
        let name = self.type_name(ctx);
        let typarams = self.type_parameters_opt(ctx);
        let mut extending = List::nil();
        if self.token.kind == TokenKind::EXTENDS {
            self.next_token();
            extending = self.type_list(ctx);
        }
        let permitting = self.permits_clause(ctx, mods, "interface");
        let defs = self.class_interface_body(ctx, name.clone(), true, false);
        let result = self.F.at(pos).class_def_permitting(
            ctx, mods, name, typarams, None, extending, permitting, defs,
        );
        let result = self.to_p(result);
        self.attach(result, dc);
        result
    }

    /// Java: List<JCTree> classInterfaceOrRecordBody(Name className,
    /// boolean isInterface, boolean isRecord)
    fn class_interface_body(
        &mut self,
        ctx: &mut CompilerContext,
        class_name: Name,
        is_interface: bool,
        is_record: bool,
    ) -> List<TreeId> {
        self.accept(ctx, TokenKind::LBRACE);
        let mut defs: Vec<TreeId> = Vec::new();
        while self.token.kind != TokenKind::RBRACE && self.token.kind != TokenKind::EOF {
            defs.extend(self.class_body_declaration(ctx, class_name.clone(), is_interface, is_record));
            if self.token.pos <= self.end_pos_table.error_end_pos() {
                self.skip(false, true, true, false);
            }
        }
        self.accept(ctx, TokenKind::RBRACE);
        List::from_iterable(defs)
    }

    /// Java: protected List<JCTree> classOrInterfaceOrRecordBodyDeclaration(
    /// Name className, boolean isInterface, boolean isRecord)
    fn class_body_declaration(
        &mut self,
        ctx: &mut CompilerContext,
        class_name: Name,
        is_interface: bool,
        is_record: bool,
    ) -> Vec<TreeId> {
        if self.token.kind == TokenKind::SEMI {
            self.next_token();
            return Vec::new();
        }
        // Java: Comment dc = token.comment(CommentStyle.JAVADOC);
        let dc = self.token.comment(CommentStyle::JAVADOC);
        let pos = self.token.pos;
        let mods = self.modifiers_opt(ctx);
        if self.token.kind == TokenKind::CLASS
            || (self.allow_records && self.is_record_start())
            || self.token.kind == TokenKind::INTERFACE
            || self.token.kind == TokenKind::ENUM
        {
            return vec![self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc)];
        }
        let mods_flags = match &ctx.tree(mods).kind {
            TreeKind::Modifiers { flags, .. } => *flags,
            _ => 0,
        };
        let mods_annotations_empty = match &ctx.tree(mods).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.is_empty(),
            _ => true,
        };
        if self.token.kind == TokenKind::LBRACE
            && (mods_flags & Flags::StandardFlags & !Flags::STATIC) == 0
            && mods_annotations_empty
        {
            if is_interface {
                let err_pos = self.token.pos;
                let key = JCDiagnosticError::from(errors::initializer_not_allowed());
                self.log
                    .borrow_mut()
                    .error_flag_at(DiagnosticFlag::SYNTAX, err_pos, key);
            } else if is_record && (mods_flags & Flags::STATIC) == 0 {
                let err_pos = self.token.pos;
                let key = JCDiagnosticError::from(
                    errors::instance_initializer_not_allowed_in_records(),
                );
                self.log
                    .borrow_mut()
                    .error_flag_at(DiagnosticFlag::SYNTAX, err_pos, key);
            }
            return vec![self.block_at(ctx, pos, mods_flags)];
        }
        let mut pos = self.token.pos;
        let typarams = self.type_parameters_opt(ctx);
        // if there are type parameters but no modifiers, save the start
        // position of the method in the modifiers.
        if !typarams.is_empty() && ctx.tree(mods).pos == NOPOS {
            ctx.tree_mut(mods).pos = pos;
            self.store_end(mods, pos);
        }
        let annos_after_params = self.annotations_opt(ctx, false);
        if !annos_after_params.is_empty() {
            let head = *annos_after_params.head();
            let head_pos = ctx.tree(head).pos;
            self.check_source_level(head_pos, Feature::ANNOTATIONS_AFTER_TYPE_PARAMS);
            if let TreeKind::Modifiers { annotations, .. } = &mut ctx.tree_mut(mods).kind {
                let mut list = annotations.clone();
                list = list.append_list(&annos_after_params);
                *annotations = list;
            }
            if ctx.tree(mods).pos == NOPOS {
                let head = *annos_after_params.head();
                let head_pos = ctx.tree(head).pos;
                ctx.tree_mut(mods).pos = head_pos;
            }
        }
        let tk = self.token.clone();
        pos = self.token.pos;
        let type_;
        let is_void = self.token.kind == TokenKind::VOID;
        if is_void {
            type_ = {
                let x = self.F.at(pos).type_ident(ctx, TypeTag::VOID);
                self.to(x)
            };
            self.next_token();
        } else {
            // method returns types are un-annotated types
            type_ = self.unannotated_type(ctx, false);
        }
        let has_ident_type = ctx.tree(type_).tag == Tag::IDENT;
        if ((self.token.kind == TokenKind::LPAREN && !is_interface)
            || (is_record && self.token.kind == TokenKind::LBRACE))
            && has_ident_type
        {
            if is_interface || tk.name().to_java_string() != class_name.to_java_string() {
                let key = JCDiagnosticError::from(errors::invalid_meth_decl_ret_type_req());
                self.log
                    .borrow_mut()
                    .error_flag_at(DiagnosticFlag::SYNTAX, pos, key);
            } else if !annos_after_params.is_empty() {
                let head = *annos_after_params.head();
                let head_pos = ctx.tree(head).pos;
                let _ = self.illegal(ctx, head_pos);
            }
            if is_record && self.token.kind == TokenKind::LBRACE {
                if let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind {
                    *flags |= Flags::COMPACT_RECORD_CONSTRUCTOR;
                }
            }
            let init_name = self.names.init.clone();
            return vec![self.method_declarator_rest(
                ctx,
                pos,
                mods,
                None,
                init_name,
                typarams,
                is_interface,
                true,
                is_record,
                dc,
            )];
        } else if is_record && has_ident_type && self.token.kind == TokenKind::THROWS {
            // trying to define a compact constructor with a throws clause
            // Java: log.error(SYNTAX, token.pos, Errors.InvalidCanonicalConstructorInRecord(
            //         Fragments.Compact, className,
            //         Fragments.ThrowsClauseNotAllowedForCanonicalConstructor(Fragments.Compact)));
            let key = JCDiagnosticError::from(errors::invalid_canonical_constructor_in_record(
                fragment_diagnostic_arg(fragments::compact()),
                class_name.clone(),
                fragment_diagnostic_arg(
                    fragments::throws_clause_not_allowed_for_canonical_constructor(
                        fragment_diagnostic_arg(fragments::compact()),
                    ),
                ),
            ));
            self.log
                .borrow_mut()
                .error_flag_at(DiagnosticFlag::SYNTAX, self.token.pos, key);
            self.skip(false, true, false, false);
            let init_name = self.names.init.clone();
            return vec![self.method_declarator_rest(
                ctx, pos, mods, None, init_name, typarams, is_interface, true, is_record, dc,
            )];
        } else {
            pos = self.token.pos;
            let name = self.ident(ctx);
            if self.token.kind == TokenKind::LPAREN {
                return vec![self.method_declarator_rest(
                    ctx,
                    pos,
                    mods,
                    Some(type_),
                    name,
                    typarams,
                    is_interface,
                    is_void,
                    false,
                    dc,
                )];
            } else if !is_void && typarams.is_empty() {
                if !is_record || (mods_flags & Flags::STATIC) != 0 {
                    let defs = self.variable_declarators_rest(
                        ctx,
                        pos,
                        mods,
                        type_,
                        name,
                        is_interface,
                        dc,
                        Vec::new(),
                        false,
                    );
                    self.accept(ctx, TokenKind::SEMI);
                    if let Some(last) = defs.last() {
                        let end = self.prev_token().end_pos;
                        self.store_end(*last, end);
                    }
                    defs
                } else {
                    let err_pos = pos;
                    let _ = self.variable_declarators_rest(
                        ctx,
                        pos,
                        mods,
                        type_,
                        name,
                        is_interface,
                        dc,
                        Vec::new(),
                        false,
                    );
                    self.accept(ctx, TokenKind::SEMI);
                    let err = self.syntax_error_list(
                        ctx,
                        err_pos,
                        List::nil(),
                        JCDiagnosticError::from(
                            errors::record_cannot_declare_instance_fields(),
                        ),
                    );
                    vec![err]
                }
            } else {
                pos = self.token.pos;
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
    }

    /// Java: protected JCTree methodDeclaratorRest(int pos, JCModifiers mods,
    /// JCExpression type, Name name, List<JCTypeParameter> typarams,
    /// boolean isInterface, boolean isVoid, boolean isRecord, Comment dc)
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
        is_record: bool,
        dc: Option<Rc<dyn Comment>>,
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
        let mut type_ = type_;
        if !is_record
            || name.to_java_string() != self.names.init.to_java_string()
            || self.token.kind == TokenKind::LPAREN
        {
            params = self.formal_parameters(ctx);
            if !is_void {
                if let Some(t) = type_ {
                    type_ = Some(self.brackets_opt_simple(ctx, t));
                }
            }
            if self.token.kind == TokenKind::THROWS {
                self.next_token();
                thrown = self.qualident_list(ctx, true);
            }
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
        self.attach(result, dc);
        self.receiver_param = prev_receiver;
        result
    }

    /// Java: List<JCVariableDecl> formalParameters()
    fn formal_parameters(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        self.formal_parameters_impl(ctx, false, false)
    }

    /// Java: List<JCVariableDecl> formalParameters(boolean lambdaParameters,
    /// boolean recordComponents)
    fn formal_parameters_impl(
        &mut self,
        ctx: &mut CompilerContext,
        lambda_parameters: bool,
        record_components: bool,
    ) -> List<TreeId> {
        let mut params: Vec<TreeId> = Vec::new();
        self.accept(ctx, TokenKind::LPAREN);
        if self.token.kind != TokenKind::RPAREN {
            self.allow_this_ident = !lambda_parameters && !record_components;
            let mut last_param = self.formal_parameter(ctx, lambda_parameters, record_components);
            let is_receiver = match &ctx.tree(last_param).kind {
                TreeKind::VariableDecl {
                    nameexpr: Some(_), ..
                } => true,
                _ => false,
            };
            if is_receiver {
                self.receiver_param = Some(last_param);
            } else {
                params.push(last_param);
            }
            self.allow_this_ident = false;
            while self.token.kind == TokenKind::COMMA {
                let last_flags = match &ctx.tree(last_param).kind {
                    TreeKind::VariableDecl { mods: Some(m), .. } => match &ctx.tree(*m).kind {
                        TreeKind::Modifiers { flags, .. } => *flags,
                        _ => 0,
                    },
                    _ => 0,
                };
                if (last_flags & Flags::VARARGS) != 0 {
                    // Java: log.error(SYNTAX, lastParam, Errors.VarargsMustBeLast)
                    let err_pos = ctx.tree(last_param).pos;
                    let key = JCDiagnosticError::from(errors::varargs_must_be_last());
                    self.log
                        .borrow_mut()
                        .error_flag_at(DiagnosticFlag::SYNTAX, err_pos, key);
                }
                self.next_token();
                last_param = self.formal_parameter(ctx, lambda_parameters, record_components);
                params.push(last_param);
            }
        }
        if self.token.kind == TokenKind::RPAREN {
            self.next_token();
        } else {
            let p = self.token.pos;
            self.set_error_end_pos(p);
            let end = self.prev_token().end_pos;
            let key = JCDiagnosticError::from(errors::expected3(
                Self::token_arg(TokenKind::COMMA),
                Self::token_arg(TokenKind::RPAREN),
                Self::token_arg(TokenKind::LBRACKET),
            ));
            let _ = self.report_syntax_error_pos(ctx, end, key);
        }
        List::from_iterable(params)
    }

    /// Java: protected JCVariableDecl formalParameter(boolean lambdaParameter,
    /// boolean recordComponent)
    fn formal_parameter(
        &mut self,
        ctx: &mut CompilerContext,
        lambda_parameter: bool,
        record_component: bool,
    ) -> TreeId {
        // Java: JCModifiers mods = !recordComponent ? optFinal(Flags.PARAMETER) : modifiersOpt();
        let mods = if !record_component {
            self.opt_final(ctx, Flags::PARAMETER)
        } else {
            self.modifiers_opt(ctx)
        };
        if record_component {
            let mods_flags = match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => *flags,
                _ => 0,
            };
            if mods_flags != 0 {
                let err_pos = ctx.tree(mods).pos;
                let key = JCDiagnosticError::from(errors::record_cant_declare_field_modifiers());
                self.log.borrow_mut().error_at(err_pos, key);
            }
            if let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind {
                *flags |= Flags::RECORD | Flags::FINAL | Flags::PRIVATE | Flags::GENERATED_MEMBER;
            }
        }
        // need to distinguish between vararg annos and array annos
        // look at typeAnnotationsPushedBack comment
        self.permit_type_annotations_push_back = true;
        let mut type_ = self.parse_type_impl(ctx, lambda_parameter);
        self.permit_type_annotations_push_back = false;
        if self.token.kind == TokenKind::ELLIPSIS {
            let varargs_annos = self.type_annotations_pushed_back.clone();
            self.type_annotations_pushed_back = List::nil();
            if let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind {
                *flags |= Flags::VARARGS;
            }
            // Java: type = insertAnnotationsToMostInner(type, varargsAnnos, true);
            type_ = self.insert_annotations_to_most_inner(ctx, type_, varargs_annos, true);
            self.next_token();
        } else {
            // if not a var arg, then typeAnnotationsPushedBack should be null
            if !self.type_annotations_pushed_back.is_empty() {
                let head = *self.type_annotations_pushed_back.head();
                let p = ctx.tree(head).pos;
                let key = JCDiagnosticError::from(errors::illegal_start_of_type());
                self.report_syntax_error(ctx, p, key);
            }
            self.type_annotations_pushed_back = List::nil();
        }
        self.variable_declarator_id_full(ctx, mods, Some(type_), lambda_parameter, record_component)
    }

    /// Java: JCVariableDecl variableDeclaratorId(JCModifiers mods, JCExpression type,
    /// boolean lambdaParameter, boolean recordComponent) — the overload is named with
    /// the _full suffix (PORTING_RULES section 3).
    fn variable_declarator_id_full(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        type_: Option<TreeId>,
        lambda_parameter: bool,
        record_component: bool,
    ) -> TreeId {
        let pos = self.token.pos;
        let name: Name;
        if lambda_parameter && self.token.kind == TokenKind::UNDERSCORE {
            let key = JCDiagnosticError::from(errors::underscore_as_identifier_in_lambda());
            self.log.borrow_mut().error_at(pos, key);
            name = self.token.name().clone();
            self.next_token();
        } else {
            let mods_flags = match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => *flags,
                _ => 0,
            };
            let mods_annotations_empty = match &ctx.tree(mods).kind {
                TreeKind::Modifiers { annotations, .. } => annotations.is_empty(),
                _ => true,
            };
            if self.allow_this_ident
                || !lambda_parameter
                || Self::is_lax_identifier(self.token.kind)
                || mods_flags != Flags::PARAMETER
                || !mods_annotations_empty
            {
                let pn = self.qualident(ctx, false);
                let mut found_name: Option<Name> = None;
                if let TreeKind::Ident { name: id_name, .. } = &ctx.tree(pn).kind {
                    if id_name.to_java_string() != self.names._this.to_java_string() {
                        found_name = Some(id_name.clone());
                    }
                }
                match found_name {
                    Some(n) => name = n,
                    None => {
                        if self.allow_this_ident {
                            if (mods_flags & Flags::VARARGS) != 0 {
                                let key = JCDiagnosticError::from(errors::varargs_and_receiver());
                                self.log.borrow_mut().error_at(self.token.pos, key);
                            }
                            if self.token.kind == TokenKind::LBRACKET {
                                let key = JCDiagnosticError::from(errors::array_and_receiver());
                                self.log.borrow_mut().error_at(self.token.pos, key);
                            }
                            let wrong_receiver = ctx.tree(pn).has_tag(Tag::SELECT)
                                && match &ctx.tree(pn).kind {
                                    TreeKind::FieldAccess { name, .. } => {
                                        name.to_java_string() != self.names._this.to_java_string()
                                    }
                                    _ => false,
                                };
                            if wrong_receiver {
                                let key = JCDiagnosticError::from(errors::wrong_receiver());
                                self.log.borrow_mut().error_at(self.token.pos, key);
                            }
                        }
                        let vd = self.F.at(pos).receiver_var_def(ctx, mods, pn, type_);
                        return self.to_p(vd);
                    }
                }
            } else {
                // Java: name = names.empty (mixed implicit/var/explicit lambda params).
                name = self.names.empty.clone();
            }
        }
        let mods_flags = match &ctx.tree(mods).kind {
            TreeKind::Modifiers { flags, .. } => *flags,
            _ => 0,
        };
        if (mods_flags & Flags::VARARGS) != 0 && self.token.kind == TokenKind::LBRACKET {
            let key = JCDiagnosticError::from(errors::varargs_and_old_array_syntax());
            self.log.borrow_mut().error_at(self.token.pos, key);
        }
        if record_component && self.token.kind == TokenKind::LBRACKET {
            let key = JCDiagnosticError::from(errors::record_component_and_old_array_syntax());
            self.log.borrow_mut().error_at(self.token.pos, key);
        }
        // PORTING_RULES adapter: type is Some in every current call site; the
        // null type of implicit lambda parameters is parsed by the lambda batch.
        let type_ = match type_ {
            Some(t) => Some(self.brackets_opt_simple(ctx, t)),
            None => None,
        };
        let declared_using_var = match type_ {
            Some(t) => {
                ctx.tree(t).has_tag(Tag::IDENT)
                    && match &ctx.tree(t).kind {
                        TreeKind::Ident { name, .. } => {
                            name.to_java_string() == self.names.var.to_java_string()
                        }
                        _ => false,
                    }
            }
            None => false,
        };
        let vd = self.F.at(pos).var_def_declared_using_var(
            ctx,
            mods,
            name,
            type_,
            None,
            declared_using_var,
        );
        self.to_p(vd)
    }

    /* ---------- compilation unit -------------- */

    /// Java: CompilationUnit = [ { "@" Annotation } PACKAGE Qualident ";"]
    /// {ImportDeclaration} {TypeDeclaration}
    pub fn parse_compilation_unit_impl(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let first_token = self.token.clone();
        let mut mods: Option<TreeId> = None;
        let mut consumed_toplevel_doc = false;
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
            // Java: attach(pd, firstToken.comment(JAVADOC)); consumedToplevelDoc = true;
            self.attach(pd, first_token.comment(CommentStyle::JAVADOC));
            consumed_toplevel_doc = true;
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
                let mut doc_comment = self.token.comment(CommentStyle::JAVADOC);
                if first_type_decl && !seen_import && !seen_package {
                    // Java: docComment = firstToken.comment(JAVADOC);
                    doc_comment = first_token.comment(CommentStyle::JAVADOC);
                    consumed_toplevel_doc = true;
                }
                if mods.is_some() || self.token.kind != TokenKind::SEMI {
                    let taken = mods.take();
                    mods = Some(self.modifiers_opt_partial(ctx, taken));
                }
                if first_type_decl && self.token.kind == TokenKind::IDENTIFIER {
                    // Java: ModuleKind kind = ModuleKind.STRONG;
                    // if (token.name() == names.open) { kind = ModuleKind.OPEN; nextToken(); }
                    // if (token.name() == names.module) { ... moduleDecl(mods, kind, docComment); break; }
                    // else if (kind != ModuleKind.STRONG) reportSyntaxError(ExpectedModule);
                    let mut kind = ModuleKind::STRONG;
                    if self.token.name().to_java_string() == self.names.open.to_java_string() {
                        kind = ModuleKind::OPEN;
                        self.next_token();
                    }
                    if self.token.kind == TokenKind::IDENTIFIER
                        && self.token.name().to_java_string() == self.names.module.to_java_string()
                    {
                        if let Some(mods_id) = mods {
                            let mods_flags = match &ctx.tree(mods_id).kind {
                                TreeKind::Modifiers { flags, .. } => *flags,
                                _ => 0,
                            };
                            let mods_pos = ctx.tree(mods_id).pos;
                            self.check_no_mods_at(ctx, mods_pos, mods_flags & !Flags::DEPRECATED);
                        }
                        let mods_for_module = mods.take().expect("module modifiers");
                        defs.push(self.module_decl(ctx, mods_for_module, kind, doc_comment));
                        consumed_toplevel_doc = true;
                        break;
                    } else if kind != ModuleKind::STRONG {
                        let p = self.token.pos;
                        let _ = self.syntax_error(
                            ctx,
                            p,
                            JCDiagnosticError::from(errors::expected_module()),
                        );
                    }
                }
                let mut def = self.type_declaration(ctx, mods.take(), doc_comment);
                // Java: if (def instanceof JCExpressionStatement statement)
                //     def = statement.expr;
                if let TreeKind::ExpressionStatement { expr: Some(expr) } = &ctx.tree(def).kind {
                    def = *expr;
                }
                defs.push(def);
                if ctx.tree(def).tag == Tag::CLASSDEF {
                    check_for_imports = false;
                }
                first_type_decl = false;
                let _ = first;
            }
            let _ = seen_import;
            let _ = seen_package;
        }
        let defs_list = List::from_iterable(defs);
        let toplevel = self.F.at(first_token.pos).top_level(ctx, defs_list.clone());
        if let TreeKind::CompilationUnit { doc_comments, .. } = &mut ctx.tree_mut(toplevel).kind {
            *doc_comments = self.doc_comments.clone();
        }
        if !consumed_toplevel_doc {
            // Java: attach(toplevel, firstToken.comment(JAVADOC));
            self.attach(toplevel, first_token.comment(CommentStyle::JAVADOC));
        }
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

    /// Java: JCModuleDecl moduleDecl(JCModifiers mods, ModuleKind kind, Comment dc)
    fn module_decl(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        kind: ModuleKind,
        dc: Option<Rc<dyn Comment>>,
    ) -> TreeId {
        let pos = self.token.pos;
        self.check_source_level_at_token(Feature::MODULES);
        self.next_token();
        let name = self.qualident(ctx, false);
        self.accept(ctx, TokenKind::LBRACE);
        let directives = self.module_directive_list(ctx);
        self.accept(ctx, TokenKind::RBRACE);
        self.accept(ctx, TokenKind::EOF);
        let result = self
            .F
            .at(pos)
            .module_def(ctx, mods, kind, Some(name), directives);
        let result = self.to_p(result);
        self.attach(result, dc);
        result
    }

    /// Java: List<JCDirective> moduleDirectiveList()
    fn module_directive_list(&mut self, ctx: &mut CompilerContext) -> List<TreeId> {
        let mut defs: Vec<TreeId> = Vec::new();
        while self.token.kind == TokenKind::IDENTIFIER {
            let pos = self.token.pos;
            let directive_name = self.token.name().to_java_string();
            if directive_name == self.names.requires.to_java_string() {
                self.next_token();
                let mut is_transitive = false;
                let mut is_static_phase = false;
                loop {
                    match self.token.kind {
                        TokenKind::IDENTIFIER => {
                            if self.token.name().to_java_string()
                                == self.names.transitive.to_java_string()
                                && !is_transitive
                            {
                                // Java: Token t1 = S.token(1);
                                let t1 = self.S.lookahead_token(1).kind;
                                if t1 == TokenKind::SEMI || t1 == TokenKind::DOT {
                                    break;
                                }
                                is_transitive = true;
                            } else {
                                break;
                            }
                        }
                        TokenKind::STATIC => {
                            if is_static_phase {
                                let p = self.token.pos;
                                let key = JCDiagnosticError::from(errors::repeated_modifier());
                                self.log
                                    .borrow_mut()
                                    .error_flag_at(DiagnosticFlag::SYNTAX, p, key);
                            }
                            is_static_phase = true;
                        }
                        _ => break,
                    }
                    self.next_token();
                }
                let module_name = self.qualident(ctx, false);
                self.accept(ctx, TokenKind::SEMI);
                let d = self
                    .F
                    .at(pos)
                    .requires(ctx, is_transitive, is_static_phase, module_name);
                defs.push(self.to_p(d));
            } else if directive_name == self.names.exports.to_java_string()
                || directive_name == self.names.opens.to_java_string()
            {
                let exports = directive_name == self.names.exports.to_java_string();
                self.next_token();
                let pkg_name = self.qualident(ctx, false);
                let mut module_names: Option<List<TreeId>> = None;
                if self.token.kind == TokenKind::IDENTIFIER
                    && self.token.name().to_java_string() == self.names.to.to_java_string()
                {
                    self.next_token();
                    module_names = Some(self.qualident_list(ctx, false));
                }
                self.accept(ctx, TokenKind::SEMI);
                let d = if exports {
                    self.F.at(pos).exports(ctx, pkg_name, module_names)
                } else {
                    self.F.at(pos).opens(ctx, pkg_name, module_names)
                };
                defs.push(self.to_p(d));
            } else if directive_name == self.names.provides.to_java_string() {
                self.next_token();
                let service_name = self.qualident(ctx, false);
                if self.token.kind == TokenKind::IDENTIFIER
                    && self.token.name().to_java_string() == self.names.with.to_java_string()
                {
                    self.next_token();
                    let impl_names = self.qualident_list(ctx, false);
                    self.accept(ctx, TokenKind::SEMI);
                    let d = self.F.at(pos).provides(ctx, service_name, impl_names);
                    defs.push(self.to_p(d));
                } else {
                    // Java: log.error(SYNTAX, token.pos, Errors.ExpectedStr("'" + names.with + "'"));
                    let p = self.token.pos;
                    let with_text = format!("'{}'", self.names.with);
                    let key = JCDiagnosticError::from(errors::expected_str(JavaString::from(
                        with_text.as_str(),
                    )));
                    self.log
                        .borrow_mut()
                        .error_flag_at(DiagnosticFlag::SYNTAX, p, key);
                    self.skip(false, false, false, false);
                }
            } else if directive_name == self.names.uses.to_java_string() {
                self.next_token();
                let service = self.qualident(ctx, false);
                self.accept(ctx, TokenKind::SEMI);
                let d = self.F.at(pos).uses(ctx, service);
                defs.push(self.to_p(d));
            } else {
                self.set_error_end_pos(pos);
                let key = JCDiagnosticError::from(errors::invalid_module_directive());
                let _ = self.syntax_error(ctx, pos, key);
                break;
            }
        }
        List::from_iterable(defs)
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
        while self.token.kind == TokenKind::DOT {
            let pos1 = self.token.pos;
            self.accept(ctx, TokenKind::DOT);
            if self.token.kind == TokenKind::STAR {
                let asterisk = self.names.asterisk.clone();
                qual_id = { let __to_tree = self.F.at(pos1).select(ctx, qual_id, asterisk); self.to(__to_tree) };
                self.next_token();
                break;
            }
            let first_name = self.ident(ctx);
            qual_id = { let __to_p_tree = self.F.at(pos1).select(ctx, qual_id, first_name); self.to_p(__to_p_tree) };
        }
        self.accept(ctx, TokenKind::SEMI);
        { let __to_p_tree = self.F.at(pos).import(ctx, qual_id, import_static); self.to_p(__to_p_tree) }
    }

    /// Java: JCTree typeDeclaration(JCModifiers mods, Comment docComment)
    ///
    /// PORTING_RULES: the doc comment received here is attached to the created
    /// declaration through JavacParser.attach when keepDocComments is set.
    fn type_declaration(
        &mut self,
        ctx: &mut CompilerContext,
        mods: Option<TreeId>,
        dc: Option<Rc<dyn Comment>>,
    ) -> TreeId {
        let pos = self.token.pos;
        if mods.is_none() && self.token.kind == TokenKind::SEMI {
            self.next_token();
            let skip = self.F.at(pos).skip(ctx);
            return self.to_p(skip);
        }
        let mods = self.modifiers_opt_partial(ctx, mods);
        self.class_or_record_or_interface_or_enum_declaration(ctx, mods, dc)
    }

    /// Java: protected JCStatement
    /// classOrRecordOrInterfaceOrEnumDeclaration(JCModifiers mods, Comment dc)
    fn class_or_record_or_interface_or_enum_declaration(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        dc: Option<Rc<dyn Comment>>,
    ) -> TreeId {
        if self.token.kind == TokenKind::CLASS {
            return self.class_declaration(ctx, mods, dc);
        }
        if self.is_record_start() {
            return self.record_declaration(ctx, mods, dc);
        } else if self.token.kind == TokenKind::INTERFACE {
            return self.interface_declaration(ctx, mods, dc);
        } else if self.token.kind == TokenKind::ENUM {
            return self.enum_declaration(ctx, mods, dc);
        }
        let pos = self.token.pos;
        if self.token.kind == TokenKind::IDENTIFIER && self.token.name() == &self.names.record {
            self.check_source_level(self.token.pos, Feature::RECORDS);
            let err = self.syntax_error_list(
                ctx,
                self.token.pos,
                List::of(mods),
                JCDiagnosticError::from(errors::record_header_expected()),
            );
            let exec = self.F.at(pos).exec(ctx, err);
            return self.to_p(exec);
        }
        let is_lax_identifier = Self::is_lax_identifier(self.token.kind);
        let errs = if is_lax_identifier {
            let name = self.ident(ctx);
            let id = { let __to_p_tree = self.F.at(pos).ident(ctx, name); self.to_p(__to_p_tree) };
            let _ = self.set_error_end_pos(self.token.pos);
            List::of2(mods, id)
        } else {
            List::of(mods)
        };
        let erroneous_tree = if self.parse_module_info {
            self.syntax_error_list(
                ctx,
                pos,
                errs,
                JCDiagnosticError::from(errors::expected_module_or_open()),
            )
        } else if self.allow_records {
            self.syntax_error_list(
                ctx,
                pos,
                errs,
                JCDiagnosticError::from(errors::expected4(
                    Self::token_arg(TokenKind::CLASS),
                    Self::token_arg(TokenKind::INTERFACE),
                    Self::token_arg(TokenKind::ENUM),
                    JavaString::from("record"),
                )),
            )
        } else {
            self.syntax_error_list(
                ctx,
                pos,
                errs,
                JCDiagnosticError::from(errors::expected3(
                    Self::token_arg(TokenKind::CLASS),
                    Self::token_arg(TokenKind::INTERFACE),
                    Self::token_arg(TokenKind::ENUM),
                )),
            )
        };
        let exec = self.F.at(pos).exec(ctx, erroneous_tree);
        self.to_p(exec)
    }

    /// Java: protected JCClassDecl enumDeclaration(JCModifiers mods, Comment dc)
    fn enum_declaration(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        dc: Option<Rc<dyn Comment>>,
    ) -> TreeId {
        let pos = self.token.pos;
        self.accept(ctx, TokenKind::ENUM);
        let name = self.type_name(ctx);
        let mut implementing = List::nil();
        if self.token.kind == TokenKind::IMPLEMENTS {
            self.next_token();
            implementing = self.type_list(ctx);
        }
        let defs = self.enum_body(ctx, name.clone());
        if let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind {
            *flags |= Flags::ENUM;
        }
        let result = self.F.at(pos).class_def(
            ctx,
            mods,
            name,
            List::nil(),
            None,
            implementing,
            defs,
        );
        let result = self.to_p(result);
        self.attach(result, dc);
        result
    }

    /// Java: List<JCTree> enumBody(Name enumName)
    fn enum_body(&mut self, ctx: &mut CompilerContext, enum_name: Name) -> List<TreeId> {
        self.accept(ctx, TokenKind::LBRACE);
        let mut defs: Vec<TreeId> = Vec::new();
        let mut was_semi = false;
        let mut has_structural_errors = false;
        let mut was_error = false;
        if self.token.kind == TokenKind::COMMA {
            self.next_token();
            if self.token.kind == TokenKind::SEMI {
                was_semi = true;
                self.next_token();
            } else if self.token.kind != TokenKind::RBRACE {
                let end = self.prev_token().end_pos;
                self.report_syntax_error(
                    ctx,
                    end,
                    JCDiagnosticError::from(errors::expected2(
                        Self::token_arg(TokenKind::RBRACE),
                        Self::token_arg(TokenKind::SEMI),
                    )),
                );
                was_error = true;
            }
        }
        while self.token.kind != TokenKind::RBRACE && self.token.kind != TokenKind::EOF {
            if self.token.kind == TokenKind::SEMI {
                self.accept(ctx, TokenKind::SEMI);
                was_semi = true;
                if self.token.kind == TokenKind::RBRACE || self.token.kind == TokenKind::EOF {
                    break;
                }
            }
            let mut member_type = self.estimate_enumerator_or_member(enum_name.clone());
            if member_type == EnumeratorEstimate::Unknown {
                member_type = if was_semi {
                    EnumeratorEstimate::Member
                } else {
                    EnumeratorEstimate::Enumerator
                };
            }
            if member_type == EnumeratorEstimate::Enumerator {
                was_error = false;
                if was_semi && !has_structural_errors {
                    self.report_syntax_error(
                        ctx,
                        self.token.pos,
                        JCDiagnosticError::from(errors::enum_constant_not_expected()),
                    );
                    has_structural_errors = true;
                }
                defs.push(self.enumerator_declaration(ctx, enum_name.clone()));
                if self.token.pos <= self.end_pos_table.error_end_pos() {
                    self.skip(false, true, true, false);
                } else if self.token.kind != TokenKind::RBRACE
                    && self.token.kind != TokenKind::SEMI
                    && self.token.kind != TokenKind::EOF
                {
                    if self.token.kind == TokenKind::COMMA {
                        self.next_token();
                    } else {
                        self.set_error_end_pos(self.token.pos);
                        let end = self.prev_token().end_pos;
                        self.report_syntax_error(
                            ctx,
                            end,
                            JCDiagnosticError::from(errors::expected3(
                                Self::token_arg(TokenKind::COMMA),
                                Self::token_arg(TokenKind::RBRACE),
                                Self::token_arg(TokenKind::SEMI),
                            )),
                        );
                        was_error = true;
                    }
                }
            } else {
                if !was_semi && !has_structural_errors && !was_error {
                    self.report_syntax_error(
                        ctx,
                        self.token.pos,
                        JCDiagnosticError::from(errors::enum_constant_expected()),
                    );
                    has_structural_errors = true;
                }
                was_error = false;
                defs.extend(self.class_body_declaration(ctx, enum_name.clone(), false, false));
                if self.token.pos <= self.end_pos_table.error_end_pos() {
                    self.skip(false, true, true, false);
                }
            }
        }
        self.accept(ctx, TokenKind::RBRACE);
        List::from_iterable(defs)
    }

    /// Java: private EnumeratorEstimate estimateEnumeratorOrMember(Name enumName)
    fn estimate_enumerator_or_member(&mut self, enum_name: Name) -> EnumeratorEstimate {
        let ident = matches!(
            self.token.kind,
            TokenKind::IDENTIFIER | TokenKind::UNDERSCORE
        );
        if ident
            && self.token.name() != &enum_name
            && (!self.allow_records || !self.is_record_start())
        {
            let next = self.S.lookahead_token(1).clone();
            match next.kind {
                TokenKind::LPAREN | TokenKind::LBRACE | TokenKind::COMMA | TokenKind::SEMI => {
                    return EnumeratorEstimate::Enumerator;
                }
                _ => {}
            }
        }
        match self.token.kind {
            TokenKind::IDENTIFIER => {
                if self.allow_records && self.is_record_start() {
                    return EnumeratorEstimate::Member;
                }
                EnumeratorEstimate::Member
            }
            TokenKind::MONKEYS_AT | TokenKind::LT | TokenKind::UNDERSCORE => {
                EnumeratorEstimate::Unknown
            }
            _ => EnumeratorEstimate::Member,
        }
    }

    /// Java: JCTree enumeratorDeclaration(Name enumName)
    fn enumerator_declaration(&mut self, ctx: &mut CompilerContext, enum_name: Name) -> TreeId {
        let dc = self.token.comment(CommentStyle::JAVADOC);
        let mut flags = Flags::PUBLIC | Flags::STATIC | Flags::FINAL | Flags::ENUM;
        if self.token.deprecated_flag() {
            flags |= Flags::DEPRECATED;
        }
        let pos = self.token.pos;
        let annotations = self.annotations_opt(ctx, false);
        let mods_pos = if annotations.is_empty() { NOPOS } else { pos };
        let mods = self
            .F
            .at(mods_pos)
            .modifiers_with_annotations(ctx, flags, annotations);
        let type_args = self.type_arguments_opt_list(ctx).unwrap_or_else(List::nil);
        let ident_pos = self.token.pos;
        let name = self.ident(ctx);
        let create_pos = self.token.pos;
        let args = if self.token.kind == TokenKind::LPAREN {
            self.arguments(ctx)
        } else {
            List::nil()
        };
        let mut body: Option<TreeId> = None;
        if self.token.kind == TokenKind::LBRACE {
            let mods1 = self
                .F
                .at(NOPOS)
                .modifiers(ctx, Flags::ENUM);
            let defs = self.class_interface_body(ctx, self.names.empty.clone(), false, false);
            let anonymous = self.F.at(ident_pos).anonymous_class_def(ctx, mods1, defs);
            body = Some(self.to_p(anonymous));
        }
        let create_pos = if args.is_empty() && body.is_none() {
            ident_pos
        } else {
            create_pos
        };
        let ident = self.F.at(ident_pos).ident(ctx, enum_name.clone());
        let create = self.F.at(create_pos).new_class(
            ctx,
            None,
            type_args,
            ident,
            args,
            body,
        );
        if create_pos != ident_pos {
            let end = self.prev_token().end_pos;
            self.store_end(create, end);
        }
        let ident2 = self.F.at(ident_pos).ident(ctx, enum_name);
        let result = self.F.at(pos).var_def(ctx, mods, name, Some(ident2), Some(create));
        let result = self.to_p(result);
        self.attach(result, dc);
        result
    }

    /// Java: protected boolean isRecordStart()
    fn is_record_start(&mut self) -> bool {
        if self.token.kind == TokenKind::IDENTIFIER && self.token.name() == &self.names.record
            && (self.peek_token_at2(0, TokenKind::IDENTIFIER, TokenKind::LPAREN)
                || self.peek_token_at2(0, TokenKind::IDENTIFIER, TokenKind::EOF)
                || self.peek_token_at2(0, TokenKind::IDENTIFIER, TokenKind::LT))
        {
            self.check_source_level(self.token.pos, Feature::RECORDS);
            true
        } else {
            false
        }
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
