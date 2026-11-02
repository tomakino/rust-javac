//! A factory for creating parsers.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * the dependency fields are injected like ScannerFactory: Names/Source/
//!   ScannerFactory are parameters and Preview is an optional explicit
//!   injection (Preview.instance is ported; ADR 0001
//!   )); Options is the ported Options::instance(context);
//! * DocTreeMaker instance is not wired (doc comment parsing is deferred, see
//!   JavacParser: keepDocComments paths (LazyDocCommentTable wired));
//! * Locale is not ported (registered GAP, no field);
//! * newParser takes the arena CompilerContext because the JavacParser
//!   constructor allocates the error tree.

use std::rc::Rc;

use javac_strings::JavaString;

use crate::code::preview::Preview;
use crate::code::source::Source;
use crate::compiler_context::CompilerContext;
use crate::parser::javac_parser::JavacParser;
use crate::parser::scanner_factory::ScannerFactory;
use crate::util::context::Context;
use crate::util::log::Log;
use crate::util::names::Names;
use crate::util::options::Options;

/// A factory for creating parsers (Java: public class ParserFactory).
#[derive(Clone)]
pub struct ParserFactory {
    pub(crate) log: Rc<std::cell::RefCell<Log>>,
    /// Upstream ParserFactory.tokens; wired from the ScannerFactory in new().
    #[allow(dead_code)]
    pub(crate) tokens: Rc<crate::parser::tokens::Tokens>,
    pub(crate) source: Source,
    pub(crate) preview: Option<Rc<Preview>>,
    pub(crate) names: Rc<Names>,
    pub(crate) options: Rc<Options>,
    pub(crate) scanner_factory: ScannerFactory,
}

impl ParserFactory {
    /// Java: protected ParserFactory(Context context).
    ///
    /// PORTING_RULES: Names/Source/ScannerFactory/Preview are injected (see the
    /// file header); the Context supplies Log/Options.
    pub fn new(
        context: &mut Context,
        names: Rc<Names>,
        source: Source,
        scanner_factory: ScannerFactory,
        preview: Option<Rc<Preview>>,
    ) -> ParserFactory {
        let log = Log::instance(context);
        let options = Options::instance(context);
        let tokens = scanner_factory.tokens.clone();
        ParserFactory {
            log,
            tokens,
            source,
            preview,
            names,
            options,
            scanner_factory,
        }
    }

    /// Java: public JavacParser newParser(CharSequence input, boolean
    /// keepDocComments, boolean keepEndPos, boolean keepLineMap).
    pub fn new_parser(
        &self,
        ctx: &mut CompilerContext,
        input: &JavaString,
        keep_doc_comments: bool,
        keep_end_pos: bool,
        keep_line_map: bool,
    ) -> JavacParser {
        self.new_parser_full(
            ctx,
            input,
            keep_doc_comments,
            keep_end_pos,
            keep_line_map,
            false,
        )
    }

    /// Java: public JavacParser newParser(CharSequence input, boolean
    /// keepDocComments, boolean keepEndPos, boolean keepLineMap,
    /// boolean parseModuleInfo).
    pub fn new_parser_full(
        &self,
        ctx: &mut CompilerContext,
        input: &JavaString,
        keep_doc_comments: bool,
        keep_end_pos: bool,
        keep_line_map: bool,
        parse_module_info: bool,
    ) -> JavacParser {
        let scanner = self
            .scanner_factory
            .new_scanner(input, keep_doc_comments);
        JavacParser::new(
            self,
            ctx,
            scanner,
            keep_doc_comments,
            keep_line_map,
            keep_end_pos,
            parse_module_info,
        )
    }
}
