//! A factory for creating scanners.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own
//! risk.  This code and its internal interfaces are subject to change
//! or deletion without notice.</b>
//!
//! PORTING_RULES:
//! * the dependency fields are injected: Names has no Context registration in
//!   the port; Source.instance/Preview.instance/Lint.instance are ported, so
//!   new()/instance() take Names/Source/Preview/Lint explicitly (ADR 0001),
//!   Source as parameters while Preview and Lint are optional Context
//!   injections (None = the Java default state: no preview flags, no -Xlint);
//!   the factory still keeps the upstream dependency fields
//!   (log/names/source/preview/tokens/lint);
//! * newScanner(CharSequence) takes the ported JavaString (the UTF-16
//!   CharBuffer case) and newScanner(char[], int, boolean) takes &[u16]; the
//!   keepDocComments selection constructs JavadocTokenizer or JavaTokenizer.

use std::cell::RefCell;
use std::rc::Rc;

use javac_strings::JavaString;

use crate::code::lint::Lint;
use crate::code::preview::Preview;
use crate::code::source::Source;
use crate::parser::java_tokenizer::{JavaTokenizer, Tokenizer};
use crate::parser::javadoc_tokenizer::JavadocTokenizer;
use crate::parser::scanner::Scanner;
use crate::parser::tokens::Tokens;
use crate::util::context::Context;
use crate::util::log::Log;
use crate::util::names::Names;

/// A factory for creating scanners (Java: public class ScannerFactory).
#[derive(Clone)]
pub struct ScannerFactory {
    pub(crate) log: Rc<RefCell<Log>>,
    pub(crate) names: Rc<Names>,
    pub(crate) source: Source,
    /// Java: final Preview preview. PORTING_RULES: Preview.instance is ported;
    /// the dependency is injected explicitly (ADR 0001), so it is an
    /// optional injection; None is the default (no preview features enabled).
    pub(crate) preview: Option<Rc<Preview>>,
    pub(crate) tokens: Rc<Tokens>,
    /// Java: final Lint lint. PORTING_RULES: Lint.instance is ported; None is
    /// the explicit no-Lint case (no -Xlint categories),
    /// matching the Java default Lint state.
    pub(crate) lint: Option<Rc<Lint>>,
}

impl ScannerFactory {
    /// Java: protected ScannerFactory(Context context).
    pub fn new(context: &mut Context, names: Rc<Names>, source: Source) -> ScannerFactory {
        let log = Log::instance(context);
        let preview = context.get::<Preview>();
        let lint = context.get::<Lint>();
        let tokens = Rc::new(Tokens::new(names.clone()));
        ScannerFactory {
            log,
            names,
            source,
            preview,
            tokens,
            lint,
        }
    }

    /// Java: public static ScannerFactory instance(Context context).
    ///
    /// PORTING_RULES: names and source are parameters (see the file header);
    /// the factory is registered in the Context under its own type.
    pub fn instance(context: &mut Context, names: Rc<Names>, source: Source) -> Rc<ScannerFactory> {
        if let Some(instance) = context.get::<ScannerFactory>() {
            return instance;
        }
        let factory = Rc::new(ScannerFactory::new(context, names, source));
        context.put(Rc::clone(&factory));
        factory
    }

    /// Java: public Scanner newScanner(CharSequence input, boolean keepDocComments).
    pub fn new_scanner(&self, input: &JavaString, keep_doc_comments: bool) -> Scanner {
        let buffer: Rc<[u16]> = Rc::from(input.as_units().to_vec());
        self.new_scanner_from_buffer(buffer, keep_doc_comments)
    }

    /// Java: public Scanner newScanner(char[] input, int inputLength,
    /// boolean keepDocComments).
    pub fn new_scanner_chars(&self, input: &[u16], keep_doc_comments: bool) -> Scanner {
        let buffer: Rc<[u16]> = Rc::from(input);
        self.new_scanner_from_buffer(buffer, keep_doc_comments)
    }

    fn new_scanner_from_buffer(&self, buffer: Rc<[u16]>, keep_doc_comments: bool) -> Scanner {
        let length = buffer.len() as i32;
        let tokenizer: Box<dyn Tokenizer> = if keep_doc_comments {
            Box::new(JavadocTokenizer::new(self, buffer, length))
        } else {
            Box::new(JavaTokenizer::new(self, buffer, length))
        };
        Scanner::new(self.tokens.clone(), tokenizer)
    }
}
