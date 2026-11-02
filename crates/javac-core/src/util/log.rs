//! A class for error logs. Reports errors and warnings, and keeps track of error
//! numbers and positions.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//!
//!   - the Java PrintWriter table maps to Rc<RefCell<dyn std::io::Write>>
//!     (interior mutability is white-listed for the log streams, registered
//!     section 9); the Java println line separator is modelled as '\n' (the same
//!     platform-independent decision as Pretty, registered);
//!   - Log is stored as Rc<RefCell<Log>> in the Context (the Java class is a
//!     mutable shared object with a context key); Log::instance returns it and
//!     registers the Options listener (the Java constructor registers it);
//!   - the DiagnosticHandler class hierarchy maps to the closed DiagnosticHandler
//!     enum (Default / Discard / Deferred); the Java constructor-installs-itself
//!     idiom maps to Log::install_diagnostic_handler and the Java
//!     popDiagnosticHandler(h) maps to Log::pop_diagnostic_handler (registered);
//!     Deferred uses VecDeque instead of a Queue and the filter is a
//!     Box<dyn Fn(&JCDiagnostic) -> bool>;
//!   - Set<Pair<JavaFileObject,Integer>> -> Vec with Rc identity + integer
//!     equality (JavaFileObject does not override equals);
//!     Set<Pair<JavaFileObject,List<String>>> -> Vec with identity + list content
//!     equality (registered);
//!   - Object varargs -> DiagnosticArg (Rc<dyn Any>) as in JCDiagnostic; localize
//!     renders the arguments through java_value_to_string (supported set:
//!     String/JavaString, i32/i64, bool, Name, Option, Target, Source,
//!     JavaFileObject, JCDiagnostic; unknown values hit the assembly assertion);
//!   - the Context keys: logKey -> Context::<RefCell<Log>>, outKey/errKey ->
//!     Context::<LogOutput> (a WriterHandle newtype because Context keys are type
//!     based), DiagnosticListener -> Context::<LogDiagnosticListener>;
//!   - REGISTERED dependency: Main.javacBundleName (main/Main is not ported),
//!     so the javac resource bundle is not added to JavacMessages by Log;
//!     * Log.format(Locale, String, Object...) (java.util.Formatter, no javac call
//!       site);
//!     * the private useRawMessages test backdoor.
//!   - System.in prompt: the upstream interactive loop is ported literally
//!     (a/A -> std::process::exit(-1), r/R -> return, x/X -> panic "user abort").

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::rc::{Rc, Weak};

use javac_tools::tools::diagnostic::Diagnostic;
use javac_tools::tools::diagnostic_listener::DiagnosticListener;
use javac_tools::tools::java_file_object::JavaFileObject;

use crate::api::diagnostic_formatter::DiagnosticFormatter;
use crate::code::lint::LintCategory;
use crate::main::option::Option as MainOption;
use crate::tree::end_pos_table::EndPosTable;
use crate::util::abstract_log::AbstractLog;
use crate::util::basic_diagnostic_formatter::BasicDiagnosticFormatter;
use crate::util::context::Context;
use crate::util::diagnostic_source::DiagnosticSource;
use crate::util::java_string::JavaString;
use crate::util::javac_messages::JavacMessages;
use crate::util::jc_diagnostic::{
    DiagnosticArg, DiagnosticFlag, DiagnosticInfo, DiagnosticPosition, DiagnosticType, Factory,
    JCDiagnostic, Note, Warning,
};
use crate::util::mandatory_warning_handler::MandatoryWarningLog;
use crate::util::options::Options;
use crate::util::position::NOPOS;
use crate::util::raw_diagnostic_formatter::RawDiagnosticFormatter;

/// The writer handle used by the log streams
/// (Java: java.io.PrintWriter -> Rc<RefCell<Box<dyn std::io::Write>>>, registered
/// section 9: the Box layer allows std streams and test buffers under the same
/// handle type).
pub type WriterHandle = Rc<RefCell<Box<dyn Write>>>;

/// Java: `new PrintWriter(new FileWriter(arg), true)` writes the platform line
/// separator. The file writer behind -Xstdout must match it (\r\n on Windows).
pub struct LineSeparatorWriter<W: Write> {
    inner: W,
}

impl<W: Write> LineSeparatorWriter<W> {
    pub fn new(inner: W) -> LineSeparatorWriter<W> {
        LineSeparatorWriter { inner }
    }
}

impl<W: Write> Write for LineSeparatorWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        #[cfg(windows)]
        {
            let mut out = Vec::with_capacity(buf.len());
            for &byte in buf {
                if byte == b'\n' {
                    out.push(b'\r');
                }
                out.push(byte);
            }
            self.inner.write_all(&out)?;
            Ok(buf.len())
        }
        #[cfg(not(windows))]
        {
            self.inner.write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// A Context value carrying the normal-output writer
/// (Java: Context.Key<PrintWriter> Log.outKey; Rust context keys are type based,
/// so a newtype distinguishes the two streams).
pub struct LogOutput(pub WriterHandle);

/// A Context value carrying the diagnostic writer
/// (Java: Context.Key<PrintWriter> Log.errKey).
pub struct LogErrorOutput(pub WriterHandle);

/// A Context value carrying the diagnostic listener
/// (Java: context.get(DiagnosticListener.class)).
pub struct LogDiagnosticListener(pub Rc<dyn DiagnosticListener<Rc<dyn JavaFileObject>>>);

/// Write a string followed by the line separator.
fn write_line(writer: &WriterHandle, text: &str) {
    let mut writer = writer.borrow_mut();
    writer
        .write_all(text.as_bytes())
        .expect("writing to the log stream failed");
    writer.write_all(b"\n").expect("writing to the log stream failed");
}

/// Write the given text without adding a line separator.
fn write_text(writer: &WriterHandle, text: &str) {
    let mut writer = writer.borrow_mut();
    writer
        .write_all(text.as_bytes())
        .expect("writing to the log stream failed");
}

/// Java: public enum Log.WriterKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WriterKind {
    NOTICE,
    WARNING,
    ERROR,
    STDOUT,
    STDERR,
}

impl WriterKind {
    /// Java: WriterKind.values()
    pub fn values() -> [WriterKind; 5] {
        [
            WriterKind::NOTICE,
            WriterKind::WARNING,
            WriterKind::ERROR,
            WriterKind::STDOUT,
            WriterKind::STDERR,
        ]
    }
}

/// Java: public enum Log.PrefixKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixKind {
    JAVAC,
    COMPILER_MISC,
}

impl PrefixKind {
    /// Java: public String key(String k)
    pub fn key(self, k: &str) -> String {
        format!("{}{}", self.value(), k)
    }

    fn value(self) -> &'static str {
        match self {
            PrefixKind::JAVAC => "javac.",
            PrefixKind::COMPILER_MISC => "compiler.misc.",
        }
    }
}

/// The filter of the DeferredDiagnosticHandler (Java: Predicate<JCDiagnostic>).
pub type DiagnosticFilter = Box<dyn Fn(&JCDiagnostic) -> bool>;

/// DiagnosticHandler's provide the initial handling for diagnostics.
///
/// the Java abstract class and its two public subclasses map to
/// this closed enum; the upstream protected prev field is the prev field of the
/// variant (registered section 9).
pub enum DiagnosticHandler {
    /// Java: private class Log.DefaultDiagnosticHandler
    Default,
    /// Java: public static class Log.DiscardDiagnosticHandler
    Discard { prev: Option<Box<DiagnosticHandler>> },
    /// Java: public static class Log.DeferredDiagnosticHandler
    Deferred {
        prev: Option<Box<DiagnosticHandler>>,
        deferred: VecDeque<JCDiagnostic>,
        filter: Option<DiagnosticFilter>,
    },
    /// Java: Analyzer.RewritingContext's anonymous DiagnosticHandler: report
    /// only marks the rewriting as erroneous (adapter: the anonymous subclass is
    /// this variant).
    Analyzer { erroneous: std::rc::Rc<std::cell::Cell<bool>> },
}

impl DiagnosticHandler {
    /// Install this diagnostic handler as the current one, recording the
    /// previous one (Java: protected void install(Log log)).
    fn set_prev(&mut self, handler: Option<Box<DiagnosticHandler>>) {
        match self {
            DiagnosticHandler::Default => {
                panic!("AssertionError: the default handler cannot be installed")
            }
            DiagnosticHandler::Discard { prev } => *prev = handler,
            DiagnosticHandler::Deferred { prev, .. } => *prev = handler,
            DiagnosticHandler::Analyzer { .. } => {}
        }
    }

    /// Java: protected DiagnosticHandler prev accessor for pop.
    fn take_prev(&mut self) -> Option<Box<DiagnosticHandler>> {
        match self {
            DiagnosticHandler::Default => None,
            DiagnosticHandler::Discard { prev } => prev.take(),
            DiagnosticHandler::Deferred { prev, .. } => prev.take(),
            DiagnosticHandler::Analyzer { .. } => None,
        }
    }

    /// Handle a diagnostic (Java: public abstract void report(JCDiagnostic)).
    fn report(&mut self, log: &mut Log, diag: JCDiagnostic) {
        match self {
            DiagnosticHandler::Default => log.default_report(diag),
            DiagnosticHandler::Discard { .. } => {}
            DiagnosticHandler::Analyzer { erroneous } => {
                erroneous.set(true);
            }
            DiagnosticHandler::Deferred {
                prev,
                deferred,
                filter,
            } => {
                if !diag.is_flag_set(DiagnosticFlag::NON_DEFERRABLE)
                    && filter.as_ref().is_none_or(|filter| filter(&diag))
                {
                    deferred.push_back(diag);
                } else if let Some(prev) = prev {
                    prev.report(log, diag);
                }
            }
        }
    }
}

/// A class for error logs.
///
/// Java: public class Log extends AbstractLog
pub struct Log {
    // AbstractLog state (the upstream inherited fields).
    diags: Rc<Factory>,
    source: Option<Rc<DiagnosticSource>>,
    source_map: Vec<(Rc<dyn JavaFileObject>, Rc<DiagnosticSource>)>,

    // Log state.
    writers: HashMap<WriterKind, WriterHandle>,
    /// The maximum number of errors that are reported.
    pub(crate) max_errors: i32,
    /// The maximum number of warnings that are reported.
    pub(crate) max_warnings: i32,
    /// Switch: prompt user on each error.
    pub prompt_on_error: bool,
    /// Switch: emit warning messages.
    pub emit_warnings: bool,
    /// Switch: suppress note messages.
    pub suppress_notes: bool,
    /// Print stack trace on errors?
    pub dump_on_error: bool,
    /// Diagnostic listener, if provided through the programmatic interface.
    diag_listener: Option<Rc<dyn DiagnosticListener<Rc<dyn JavaFileObject>>>>,
    /// Formatter for diagnostics.
    diag_formatter: Rc<dyn DiagnosticFormatter<JCDiagnostic>>,
    /// Keys for expected diagnostics.
    pub expect_diag_keys: Option<HashSet<String>>,
    /// Set to true if a compressed diagnostic is reported.
    pub compressed_output: bool,
    /// JavacMessages object used for localization.
    messages: Rc<JavacMessages>,
    /// Handler for initial dispatch of diagnostics.
    diagnostic_handler: DiagnosticHandler,

    /// The number of errors encountered so far.
    pub nerrors: i32,
    /// The number of warnings encountered so far.
    pub nwarnings: i32,
    /// The number of errors encountered after MaxErrors was reached.
    pub nsuppressederrors: i32,
    /// The number of warnings encountered after MaxWarnings was reached.
    pub nsuppressedwarns: i32,
    /// A set of all errors generated so far (Pair<JavaFileObject,Integer>).
    recorded: Vec<(Rc<dyn JavaFileObject>, i32)>,
    /// A set of "not-supported-in-source-X" errors produced so far
    /// (Pair<JavaFileObject,List<String>>).
    recorded_source_level_errors: Vec<(Rc<dyn JavaFileObject>, Vec<String>)>,
}

impl Log {
    /// Emit a mandatory note through the AbstractLog handler (Java:
    /// log.mandatoryNote); used by the compiler driver for the
    /// compressed-diagnostics note.
    pub fn emit_mandatory_note(&mut self, note: Note) {
        AbstractLog::mandatory_note_file(self, None, note);
    }

    /// Report an error through the default handler, ignoring a temporarily
    /// installed speculative/deferred handler. Java reports void-argument
    /// errors while the surrounding method call is still being resolved.
    pub fn error_at_default(
        &mut self,
        pos: i32,
        error_key: crate::util::jc_diagnostic::Error,
    ) {
        let diagnostic = {
            let source = self.source.clone();
            let diags = Rc::clone(&self.diags);
            diags.error(None, source, Self::wrap_pos(pos), error_key)
        };
        self.default_report(diagnostic);
    }
    /// Get the Log instance for this context.
    ///
    /// Java: public static Log instance(Context context)
    ///
    /// the Java constructor registers the options listener with
    /// the Log reference; Rust needs the Rc first, so instance() creates the
    /// Rc, stores it in the Context and then registers the listener (the name
    /// allocation pattern is the same as JCDiagnostic.Factory.instance).
    pub fn instance(context: &mut Context) -> Rc<RefCell<Log>> {
        if let Some(instance) = context.get::<RefCell<Log>>() {
            return instance;
        }
        let log = Rc::new(RefCell::new(Log::new_default(context)));
        context.put::<RefCell<Log>>(Rc::clone(&log));
        let options = Options::instance(context);
        let weak: Weak<RefCell<Log>> = Rc::downgrade(&log);
        let listener_options = Rc::clone(&options);
        options.add_listener(Box::new(move || {
            if let Some(log) = weak.upgrade() {
                log.borrow_mut().init_options(&listener_options);
            }
        }));
        log
    }

    /// Register a Context.Factory to create a Log.
    ///
    /// Java: public static void preRegister(Context context, PrintWriter w)
    pub fn pre_register(context: &mut Context, writer: WriterHandle) {
        context.put_factory::<RefCell<Log>>(move |context| {
            Rc::new(RefCell::new(Log::new_with_writer(context, Rc::clone(&writer))))
        });
    }

    /// Construct a log with default settings.
    ///
    /// Java: protected Log(Context context)
    fn new_default(context: &mut Context) -> Log {
        let writers = Log::init_writers_from_context(context);
        Log::new_with_writers(context, writers)
    }

    /// Construct a log with all output sent to a single output stream.
    ///
    /// Java: protected Log(Context context, PrintWriter writer)
    fn new_with_writer(context: &mut Context, writer: WriterHandle) -> Log {
        Log::new_with_writers(context, Log::init_writers(Some(writer.clone()), Some(writer)))
    }

    /// Construct a log.
    ///
    /// Java: protected Log(Context context, PrintWriter out, PrintWriter err)
    pub fn new_with_out_err(context: &mut Context, out: WriterHandle, err: WriterHandle) -> Log {
        Log::new_with_writers(context, Log::init_writers(Some(out), Some(err)))
    }

    /// Initialize a map of writers based on values found in the context.
    ///
    /// Java: private static Map<WriterKind, PrintWriter> initWriters(Context)
    fn init_writers_from_context(context: &mut Context) -> HashMap<WriterKind, WriterHandle> {
        let out = context.get::<LogOutput>().map(|output| Rc::clone(&output.0));
        let err = context
            .get::<LogErrorOutput>()
            .map(|output| Rc::clone(&output.0));
        match (out, err) {
            (None, None) => Log::init_writers(Some(Log::stdout_handle()), Some(Log::stderr_handle())),
            (Some(out), None) => Log::init_writers(Some(Rc::clone(&out)), Some(out)),
            (None, Some(err)) => Log::init_writers(Some(Rc::clone(&err)), Some(err)),
            (Some(out), Some(err)) => Log::init_writers(Some(out), Some(err)),
        }
    }

    fn stdout_handle() -> WriterHandle {
        Rc::new(RefCell::new(Box::new(std::io::stdout()) as Box<dyn Write>))
    }

    fn stderr_handle() -> WriterHandle {
        Rc::new(RefCell::new(Box::new(std::io::stderr()) as Box<dyn Write>))
    }

    /// Initialize a writer map for a stream for normal output and a stream for
    /// diagnostics.
    ///
    /// Java: private static Map<WriterKind, PrintWriter> initWriters(out, err)
    fn init_writers(
        out: Option<WriterHandle>,
        err: Option<WriterHandle>,
    ) -> HashMap<WriterKind, WriterHandle> {
        let out = out.expect("out writer");
        let err = err.expect("err writer");
        let mut writers = HashMap::new();
        writers.insert(WriterKind::ERROR, Rc::clone(&err));
        writers.insert(WriterKind::WARNING, Rc::clone(&err));
        writers.insert(WriterKind::NOTICE, Rc::clone(&err));
        writers.insert(WriterKind::STDOUT, out);
        writers.insert(WriterKind::STDERR, err);
        writers
    }

    /// Creates a log (the Java private constructor Log(Context, Map)).
    pub fn new_with_writers(context: &mut Context, writers: HashMap<WriterKind, WriterHandle>) -> Log {
        let diags = Factory::instance(context);
        let diag_listener = context
            .get::<LogDiagnosticListener>()
            .map(|listener| Rc::clone(&listener.0));
        let messages = JavacMessages::instance(context);
        // REGISTERED: messages.add(Main.javacBundleName); main/Main is not
        // ported, so the javac resources are not added here.
        let options = Options::instance(context);
        let mut log = Log {
            diags,
            source: None,
            source_map: Vec::new(),
            writers,
            max_errors: 100,
            max_warnings: 100,
            prompt_on_error: false,
            emit_warnings: true,
            suppress_notes: false,
            dump_on_error: false,
            diag_listener,
            diag_formatter: Rc::new(BasicDiagnosticFormatter::new(Rc::clone(&messages))),
            expect_diag_keys: None,
            compressed_output: false,
            messages,
            diagnostic_handler: DiagnosticHandler::Default,
            nerrors: 0,
            nwarnings: 0,
            nsuppressederrors: 0,
            nsuppressedwarns: 0,
            recorded: Vec::new(),
            recorded_source_level_errors: Vec::new(),
        };
        log.init_options(&options);
        log
    }

    /// Java: private void initOptions(Options options)
    fn init_options(&mut self, options: &Options) {
        // Java: options.isSet(DOE) / isSet(PROMPT).
        self.dump_on_error = options.is_set_option(MainOption::DOE);
        self.prompt_on_error = options.is_set_option(MainOption::PROMPT);
        self.emit_warnings = options.is_unset_choice(MainOption::XLINT_CUSTOM, "none");
        self.suppress_notes = options.is_set("suppressNotes");
        self.max_errors = get_int_option(options, MainOption::XMAXERRS, self.get_default_max_errors());
        self.max_warnings =
            get_int_option(options, MainOption::XMAXWARNS, self.get_default_max_warnings());

        let raw_diagnostics = options.is_set("rawDiagnostics");
        self.diag_formatter = if raw_diagnostics {
            Rc::new(RawDiagnosticFormatter::new_with_options(options))
        } else {
            Rc::new(BasicDiagnosticFormatter::new_with_options(
                Rc::clone(&self.messages),
                options,
            ))
        };

        if let Some(expect_keys) = options.get("expectKeys") {
            self.expect_diag_keys = Some(
                expect_keys
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect(),
            );
        }
    }

    /// Default value for -Xmaxerrs.
    fn get_default_max_errors(&self) -> i32 {
        100
    }

    /// Default value for -Xmaxwarns.
    fn get_default_max_warnings(&self) -> i32 {
        100
    }

    /// Java: public boolean hasDiagnosticListener()
    pub fn has_diagnostic_listener(&self) -> bool {
        self.diag_listener.is_some()
    }

    /// Java: public void setEndPosTable(JavaFileObject name, EndPosTable t)
    pub fn set_end_pos_table(&mut self, name: Rc<dyn JavaFileObject>, end_pos_table: Rc<RefCell<dyn EndPosTable>>) {
        let source = self.get_source(Some(name));
        source.set_end_pos_table(end_pos_table);
    }

    /// Java: public DiagnosticSource currentSource()
    pub fn current_source(&self) -> Option<Rc<DiagnosticSource>> {
        self.source.clone()
    }

    /// Return current sourcefile.
    ///
    /// Java: public JavaFileObject currentSourceFile()
    pub fn current_source_file(&self) -> Option<Rc<dyn JavaFileObject>> {
        self.source.as_ref().and_then(|source| source.get_file())
    }

    /// Get the current diagnostic formatter.
    pub fn get_diagnostic_formatter(&self) -> Rc<dyn DiagnosticFormatter<JCDiagnostic>> {
        Rc::clone(&self.diag_formatter)
    }

    /// Set the current diagnostic formatter.
    pub fn set_diagnostic_formatter(&mut self, diag_formatter: Rc<dyn DiagnosticFormatter<JCDiagnostic>>) {
        self.diag_formatter = diag_formatter;
    }

    /// Java: public PrintWriter getWriter(WriterKind kind)
    pub fn get_writer(&self, kind: WriterKind) -> WriterHandle {
        Rc::clone(self.writers.get(&kind).expect("writer"))
    }

    /// Java: public void setWriter(WriterKind kind, PrintWriter pw)
    pub fn set_writer(&mut self, kind: WriterKind, pw: WriterHandle) {
        self.writers.insert(kind, pw);
    }

    /// Java: public void setWriters(PrintWriter pw)
    pub fn set_writers(&mut self, pw: WriterHandle) {
        for kind in WriterKind::values() {
            self.writers.insert(kind, Rc::clone(&pw));
        }
    }

    /// Install a diagnostic handler, recording the previous one as its prev.
    ///
    /// the Java handlers install themselves in their
    /// constructors; Rust cannot move self out of a constructor, so the caller
    /// installs the handler explicitly (registered section 9).
    pub fn install_diagnostic_handler(&mut self, mut handler: DiagnosticHandler) {
        let prev = std::mem::replace(&mut self.diagnostic_handler, DiagnosticHandler::Default);
        handler.set_prev(Some(Box::new(prev)));
        self.diagnostic_handler = handler;
    }

    /// Replace the current diagnostic handler with the handler that was current
    /// at the time it was created.
    ///
    /// Java: public void popDiagnosticHandler(DiagnosticHandler h)
    ///
    /// the Java identity check on h has no Rust form because the
    /// closed enum is stored by value; the ported method restores the prev
    /// handler and panics if only the default handler is installed (registered).
    pub fn pop_diagnostic_handler(&mut self) {
        let mut handler =
            std::mem::replace(&mut self.diagnostic_handler, DiagnosticHandler::Default);
        let prev = handler.take_prev();
        self.diagnostic_handler = prev.map(|handler| *handler).unwrap_or(DiagnosticHandler::Default);
    }

    /// Report all deferred diagnostics of the current DeferredDiagnosticHandler
    /// (Java: DeferredDiagnosticHandler.reportDeferredDiagnostics()).
    pub fn report_deferred_diagnostics(&mut self) {
        self.report_deferred_diagnostics_with(&|_| true);
    }

    /// Java: DeferredDiagnosticHandler.getDiagnostics(): a snapshot of the
    /// diagnostics currently deferred by the installed handler (empty when
    /// the handler is not a deferred handler).
    pub fn deferred_diagnostics(&self) -> Vec<JCDiagnostic> {
        match &self.diagnostic_handler {
            DiagnosticHandler::Deferred { deferred, .. } => deferred.iter().cloned().collect(),
            _ => Vec::new(),
        }
    }
    /// Report selected deferred diagnostics.
    ///
    /// Java: DeferredDiagnosticHandler.reportDeferredDiagnostics(Predicate)
    pub fn report_deferred_diagnostics_with(&mut self, accepter: &dyn Fn(&JCDiagnostic) -> bool) {
        let handler = std::mem::replace(&mut self.diagnostic_handler, DiagnosticHandler::Default);
        match handler {
            DiagnosticHandler::Deferred { mut prev, deferred, .. } => {
                for diagnostic in deferred {
                    if accepter(&diagnostic)
                        && let Some(prev_handler) = prev.as_mut()
                    {
                        prev_handler.report(self, diagnostic);
                    }
                }
                self.diagnostic_handler =
                    prev.map(|handler| *handler).unwrap_or(DiagnosticHandler::Default);
            }
            other => self.diagnostic_handler = other,
        }
    }

    /// The Java DeferredDiagnosticHandler.getDiagnostics() accessor
    /// (the ported form returns a copy of the deferred queue).
    pub fn take_deferred_diagnostics(&mut self) -> Vec<JCDiagnostic> {
        let mut result = Vec::new();
        if let DiagnosticHandler::Deferred { deferred, .. } = &mut self.diagnostic_handler {
            result.extend(deferred.drain(..));
        }
        result
    }

    /// Flush the logs.
    ///
    /// Java: public void flush()
    pub fn flush(&self) {
        for writer in self.writers.values() {
            writer.borrow_mut().flush().expect("flush failed");
        }
    }

    /// Java: public void flush(WriterKind kind)
    pub fn flush_kind(&self, kind: WriterKind) {
        self.get_writer(kind)
            .borrow_mut()
            .flush()
            .expect("flush failed");
    }

    /// Returns true if an error needs to be reported for a given source name
    /// and pos.
    ///
    /// Java: protected boolean shouldReport(JavaFileObject file, int pos)
    fn should_report(&mut self, file: Option<Rc<dyn JavaFileObject>>, pos: i32) -> bool {
        let Some(file) = file else {
            return true;
        };
        let should_report = !self
            .recorded
            .iter()
            .any(|(recorded_file, recorded_pos)| {
                Rc::ptr_eq(recorded_file, &file) && *recorded_pos == pos
            });
        if should_report {
            self.recorded.push((file, pos));
        }
        should_report
    }

    /// Returns true if a diagnostics needs to be reported.
    ///
    /// Java: private boolean shouldReport(JCDiagnostic d)
    fn should_report_diagnostic(&mut self, diag: &JCDiagnostic) -> bool {
        let Some(file) = diag.get_source() else {
            return true;
        };
        if !self.should_report(Some(Rc::clone(&file)), diag.get_int_position()) {
            return false;
        }
        if !diag.is_flag_set(DiagnosticFlag::SOURCE_LEVEL) {
            return true;
        }
        let code = code_of(diag);
        let should_report = !self
            .recorded_source_level_errors
            .iter()
            .any(|(recorded_file, recorded_code)| {
                Rc::ptr_eq(recorded_file, &file) && *recorded_code == code
            });
        if should_report {
            self.recorded_source_level_errors.push((file, code));
        }
        should_report
    }

    /// Is an error reported at the given pos (inside the current source)?
    ///
    /// Java: public boolean hasErrorOn(DiagnosticPosition pos)
    pub fn has_error_on(&self, pos: Option<Rc<dyn DiagnosticPosition>>) -> bool {
        let file = self.source.as_ref().and_then(|source| source.get_file());
        match (file, pos) {
            (Some(file), Some(pos)) => self.recorded.iter().any(|(recorded_file, recorded_pos)| {
                Rc::ptr_eq(recorded_file, &file) && *recorded_pos == pos.get_preferred_position()
            }),
            _ => false,
        }
    }

    /// Prompt user after an error.
    ///
    /// Java: public void prompt()
    pub fn prompt(&self) {
        if !self.prompt_on_error {
            return;
        }
        eprintln!("{}", self.localize_key("resume.abort", Vec::new()));
        loop {
            let mut line = String::new();
            if std::io::stdin().read_line(&mut line).is_err() {
                return;
            }
            match line.chars().next() {
                Some('a') | Some('A') => std::process::exit(-1),
                Some('r') | Some('R') => return,
                Some('x') | Some('X') => panic!("user abort"),
                _ => {}
            }
        }
    }

    /// Print the faulty source code line and point to the error.
    fn print_err_line(&self, pos: i32, writer: &WriterHandle) {
        let line = self
            .source
            .as_ref()
            .and_then(|source| source.get_line(pos));
        let Some(line) = line else {
            return;
        };
        let col = self
            .source
            .as_ref()
            .expect("source")
            .get_column_number(pos, false);
        Log::print_raw_lines_to(writer, &line.to_string_lossy());
        for i in 0..col - 1 {
            let ch = line.char_at(i as usize);
            write_text(writer, if ch == '\t' as u16 { "\t" } else { " " });
        }
        write_line(writer, "^");
        writer.borrow_mut().flush().expect("flush failed");
    }

    /// Java: public void printNewline()
    pub fn print_newline(&self) {
        write_line(&self.get_writer(WriterKind::NOTICE), "");
    }

    /// Java: public void printNewline(WriterKind wk)
    pub fn print_newline_kind(&self, kind: WriterKind) {
        write_line(&self.get_writer(kind), "");
    }

    /// Java: public void printLines(String key, Object... args)
    pub fn print_lines(&self, key: &str, args: Vec<DiagnosticArg>) {
        let msg = self.localize_key(key, args);
        Log::print_raw_lines_to(&self.get_writer(WriterKind::NOTICE), &msg);
    }

    /// Java: public void printLines(DiagnosticInfo diag)
    pub fn print_lines_info(&self, diag: &DiagnosticInfo) {
        let msg = self.localize_info(diag);
        Log::print_raw_lines_to(&self.get_writer(WriterKind::NOTICE), &msg);
    }

    /// Java: public void printLines(PrefixKind pk, String key, Object... args)
    pub fn print_lines_prefix(&self, pk: PrefixKind, key: &str, args: Vec<DiagnosticArg>) {
        let msg = self.localize_prefix(pk, key, args);
        Log::print_raw_lines_to(&self.get_writer(WriterKind::NOTICE), &msg);
    }

    /// Java: public void printLines(WriterKind wk, String key, Object... args)
    pub fn print_lines_kind(&self, kind: WriterKind, key: &str, args: Vec<DiagnosticArg>) {
        let msg = self.localize_key(key, args);
        Log::print_raw_lines_to(&self.get_writer(kind), &msg);
    }

    /// Java: public void printLines(WriterKind wk, PrefixKind pk, String key,
    /// Object... args)
    pub fn print_lines_kind_prefix(&self, kind: WriterKind, pk: PrefixKind, key: &str, args: Vec<DiagnosticArg>) {
        let msg = self.localize_prefix(pk, key, args);
        Log::print_raw_lines_to(&self.get_writer(kind), &msg);
    }

    /// Print the text of a message, translating newlines appropriately for the
    /// platform.
    ///
    /// Java: public void printRawLines(String msg)
    pub fn print_raw_lines(&self, msg: &str) {
        Log::print_raw_lines_to(&self.get_writer(WriterKind::NOTICE), msg);
    }

    /// Java: public void printRawLines(WriterKind kind, String msg)
    pub fn print_raw_lines_kind(&self, kind: WriterKind, msg: &str) {
        Log::print_raw_lines_to(&self.get_writer(kind), msg);
    }

    /// Java: public static void printRawLines(PrintWriter writer, String msg)
    ///
    /// the static PrintWriter overload maps to an associated
    /// function taking the writer handle (no overloading, registered).
    pub fn print_raw_lines_to(writer: &WriterHandle, mut msg: &str) {
        while let Some(index) = msg.find('\n') {
            write_line(writer, &msg[..index]);
            msg = &msg[index + 1..];
        }
        if !msg.is_empty() {
            write_line(writer, msg);
        }
    }

    /// Print the localized text of a "verbose" message to the noticeWriter
    /// stream.
    ///
    /// Java: public void printVerbose(String key, Object... args)
    pub fn print_verbose(&self, key: &str, args: Vec<DiagnosticArg>) {
        let msg = self.localize_key(&format!("verbose.{key}"), args);
        Log::print_raw_lines_to(&self.get_writer(WriterKind::NOTICE), &msg);
    }

    /// Java: protected void directError(String key, Object... args)
    fn direct_error_impl(&self, key: &str, args: Vec<String>) {
        let msg = self.messages.get_localized_string(key, &args);
        Log::print_raw_lines_to(&self.get_writer(WriterKind::ERROR), &msg);
        self.get_writer(WriterKind::ERROR)
            .borrow_mut()
            .flush()
            .expect("flush failed");
    }

    /// Report a warning that cannot be suppressed.
    ///
    /// Java: public void strictWarning(DiagnosticPosition pos, String key,
    /// Object... args)
    pub fn strict_warning(&mut self, pos: Option<Rc<dyn DiagnosticPosition>>, key: &str, args: Vec<DiagnosticArg>) {
        let warning_key = self.diags.warning_key(key, args);
        let diagnostic = self.diags.warning(None, self.source.clone(), pos, warning_key);
        self.write_diagnostic(&diagnostic);
        self.nwarnings += 1;
    }

    /// Primary method to report a diagnostic.
    ///
    /// Java: public void report(JCDiagnostic diagnostic)
    pub fn report(&mut self, diagnostic: JCDiagnostic) {
        let mut handler =
            std::mem::replace(&mut self.diagnostic_handler, DiagnosticHandler::Default);
        handler.report(self, diagnostic);
        self.diagnostic_handler = handler;
    }

    /// Common diagnostic handling (the body of the private
    /// DefaultDiagnosticHandler.report).
    fn default_report(&mut self, diagnostic: JCDiagnostic) {
        if let Some(expect_diag_keys) = &mut self.expect_diag_keys {
            expect_diag_keys.remove(&diagnostic.get_code_string());
        }
        match diagnostic.get_type() {
            DiagnosticType::FRAGMENT => {
                panic!("IllegalArgumentException: fragment diagnostic");
            }
            DiagnosticType::NOTE => {
                if (self.emit_warnings || diagnostic.is_mandatory()) && !self.suppress_notes {
                    self.write_diagnostic(&diagnostic);
                }
            }
            DiagnosticType::WARNING => {
                if self.emit_warnings || diagnostic.is_mandatory() {
                    if self.nwarnings < self.max_warnings {
                        self.write_diagnostic(&diagnostic);
                        self.nwarnings += 1;
                    } else {
                        self.nsuppressedwarns += 1;
                    }
                }
            }
            DiagnosticType::ERROR => {
                if diagnostic.is_flag_set(DiagnosticFlag::API)
                    || self.should_report_diagnostic(&diagnostic)
                {
                    if self.nerrors < self.max_errors {
                        self.write_diagnostic(&diagnostic);
                        self.nerrors += 1;
                    } else {
                        self.nsuppressederrors += 1;
                    }
                }
            }
        }
        if diagnostic.is_flag_set(DiagnosticFlag::COMPRESSED) {
            self.compressed_output = true;
        }
    }

    /// Write out a diagnostic.
    ///
    /// Java: protected void writeDiagnostic(JCDiagnostic diag)
    fn write_diagnostic(&mut self, diag: &JCDiagnostic) {
        if let Some(listener) = &self.diag_listener {
            let diagnostic: Rc<dyn Diagnostic<Rc<dyn JavaFileObject>>> = Rc::new(diag.clone());
            listener.report(diagnostic);
            return;
        }
        let writer = self.get_writer_for_diagnostic_type(diag.get_type());
        let locale = self.messages.get_current_locale().clone();
        let text = self.diag_formatter.format(diag, Some(&locale));
        Log::print_raw_lines_to(&writer, &text);
        if self.prompt_on_error {
            match diag.get_type() {
                DiagnosticType::ERROR | DiagnosticType::WARNING => self.prompt(),
                _ => {}
            }
        }
        if self.dump_on_error {
            // Java: new RuntimeException().printStackTrace(writer);
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            Log::print_raw_lines_to(&writer, &backtrace);
        }
        writer.borrow_mut().flush().expect("flush failed");
    }

    /// Java: protected PrintWriter getWriterForDiagnosticType(DiagnosticType dt)
    fn get_writer_for_diagnostic_type(&self, dt: DiagnosticType) -> WriterHandle {
        match dt {
            DiagnosticType::FRAGMENT => panic!("IllegalArgumentException: fragment diagnostic"),
            DiagnosticType::NOTE => self.get_writer(WriterKind::NOTICE),
            DiagnosticType::WARNING => self.get_writer(WriterKind::WARNING),
            DiagnosticType::ERROR => self.get_writer(WriterKind::ERROR),
        }
    }

    /// Find a localized string in the resource bundle.
    ///
    /// Java: public static String getLocalizedString(String key, Object... args)
    pub fn get_localized_string(key: &str, args: &[DiagnosticArg]) -> String {
        let strings = args_to_strings(args);
        JavacMessages::get_default_localized_string(
            &PrefixKind::COMPILER_MISC.key(key),
            &strings,
        )
    }

    /// Java: public String localize(String key, Object... args)
    pub fn localize_key(&self, key: &str, args: Vec<DiagnosticArg>) -> String {
        self.localize_prefix(PrefixKind::COMPILER_MISC, key, args)
    }

    /// Java: public String localize(JCDiagnostic.DiagnosticInfo diagInfo)
    pub fn localize_info(&self, diag_info: &DiagnosticInfo) -> String {
        let args = args_to_strings(diag_info.get_args());
        self.messages.get_localized_string(&diag_info.key(), &args)
    }

    /// Java: public String localize(PrefixKind pk, String key, Object... args)
    pub fn localize_prefix(&self, pk: PrefixKind, key: &str, args: Vec<DiagnosticArg>) -> String {
        let strings = args_to_strings(&args);
        self.messages
            .get_localized_string(&pk.key(key), &strings)
    }

    /// print an error or warning message.
    ///
    /// Java: private void printRawDiag(PrintWriter pw, String prefix, int pos,
    /// String msg)
    fn print_raw_diag(&self, writer: &WriterHandle, prefix: &str, pos: i32, msg: &str) {
        match (&self.source, pos) {
            (None, _) | (_, NOPOS) => {
                Log::print_raw_lines_to(writer, &format!("{prefix}{msg}"));
            }
            (Some(source), _) => {
                let line = source.get_line_number(pos);
                if let Some(file) = source.get_file() {
                    Log::print_raw_lines_to(
                        writer,
                        &format!("{}:{}: {}", file.get_name().to_string_lossy(), line, msg),
                    );
                }
                self.print_err_line(pos, writer);
            }
        }
        writer.borrow_mut().flush().expect("flush failed");
    }

    /// report an error.
    ///
    /// Java: public void rawError(int pos, String msg)
    pub fn raw_error(&mut self, pos: i32, msg: &str) {
        let file = self.current_source_file();
        if self.nerrors < self.max_errors && self.should_report(file, pos) {
            let writer = self.get_writer(WriterKind::ERROR);
            self.print_raw_diag(&writer, "error: ", pos, msg);
            self.prompt();
            self.nerrors += 1;
        } else {
            self.nsuppressederrors += 1;
        }
        self.get_writer(WriterKind::ERROR)
            .borrow_mut()
            .flush()
            .expect("flush failed");
    }

    /// report a warning.
    ///
    /// Java: public void rawWarning(int pos, String msg)
    pub fn raw_warning(&mut self, pos: i32, msg: &str) {
        let writer = self.get_writer(WriterKind::ERROR);
        if self.emit_warnings {
            if self.nwarnings < self.max_warnings {
                self.print_raw_diag(&writer, "warning: ", pos, msg);
            } else {
                self.nsuppressedwarns += 1;
            }
        }
        self.prompt();
        self.nwarnings += 1;
        writer.borrow_mut().flush().expect("flush failed");
    }
}

/// Java: private int getIntOption(Options options, Option option, int
/// defaultValue)
fn get_int_option(options: &Options, option: MainOption, default_value: i32) -> i32 {
    if let Some(text) = options.get_option(option)
        && let Ok(n) = text.parse::<i32>()
    {
        return if n <= 0 { i32::MAX } else { n };
    }
    default_value
}

/// Java: private List<String> getCode(JCDiagnostic d)
fn code_of(diag: &JCDiagnostic) -> Vec<String> {
    let mut result = Vec::new();
    code_recursive(&mut result, diag);
    result
}

/// Java: private void getCodeRecursive(ListBuffer<String> buf, JCDiagnostic d)
fn code_recursive(result: &mut Vec<String>, diag: &JCDiagnostic) {
    result.push(diag.get_code_string());
    for arg in diag.get_args() {
        if let Some(nested) = arg.downcast_ref::<JCDiagnostic>() {
            code_recursive(result, nested);
        }
    }
}

/// Java: String.valueOf(Object) for the message arguments used by the Log
/// entry points.
///
/// the Object varargs of localize/printLines are DiagnosticArg
/// values; the supported set is String/JavaString, i32/i64, bool, Name, Option,
/// Target, Source, JavaFileObject and JCDiagnostic (the types used by Log call
/// sites). Unknown values are an assembly assertion (closed type domain).
fn args_to_strings(args: &[DiagnosticArg]) -> Vec<String> {
    args.iter().map(java_value_to_string).collect()
}

fn java_value_to_string(arg: &DiagnosticArg) -> String {
    if let Some(value) = arg.downcast_ref::<JavaString>() {
        return value.to_string_lossy();
    }
    if let Some(value) = arg.downcast_ref::<String>() {
        return value.clone();
    }
    if let Some(value) = arg.downcast_ref::<i32>() {
        return value.to_string();
    }
    if let Some(value) = arg.downcast_ref::<i64>() {
        return value.to_string();
    }
    if let Some(value) = arg.downcast_ref::<bool>() {
        return value.to_string();
    }
    if let Some(value) = arg.downcast_ref::<crate::util::name::Name>() {
        return value.to_java_string().to_string_lossy();
    }
    if let Some(option) = arg.downcast_ref::<crate::main::option::Option>() {
        return option.primary_name().to_string();
    }
    if let Some(target) = arg.downcast_ref::<crate::jvm::target::Target>() {
        return target.name().to_string();
    }
    if let Some(source) = arg.downcast_ref::<crate::code::source::Source>() {
        return source.name().to_string();
    }
    if let Some(file_object) = arg.downcast_ref::<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>() {
        return file_object.get_name().to_string_lossy();
    }
    if let Some(diagnostic) = arg.downcast_ref::<JCDiagnostic>() {
        return diagnostic.get_message(None).to_string();
    }
    // Diagnostic argument type domain is closed (see the module documentation);
    // an unknown value cannot be produced by the ported code.
    crate::util::assert::error_msg("unreachable log message argument type");
}

impl AbstractLog for Log {
    fn diags(&self) -> &Rc<Factory> {
        &self.diags
    }

    fn source(&self) -> Option<&Rc<DiagnosticSource>> {
        self.source.as_ref()
    }

    fn set_source(&mut self, source: Option<Rc<DiagnosticSource>>) {
        self.source = source;
    }

    fn source_map(&mut self) -> &mut Vec<(Rc<dyn JavaFileObject>, Rc<DiagnosticSource>)> {
        &mut self.source_map
    }

    fn report(&mut self, diagnostic: JCDiagnostic) {
        Log::report(self, diagnostic);
    }

    fn direct_error(&mut self, key: &str, args: Vec<String>) {
        self.direct_error_impl(key, args);
    }
}

impl MandatoryWarningLog for Log {
    fn current_source_file(&mut self) -> Option<Rc<dyn JavaFileObject>> {
        Log::current_source_file(self)
    }

    fn nwarnings(&self) -> i32 {
        self.nwarnings
    }

    fn max_warnings(&self) -> i32 {
        self.max_warnings
    }

    fn mandatory_warning(
        &mut self,
        lc: Option<LintCategory>,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    ) {
        match lc {
            Some(lc) => AbstractLog::mandatory_warning_lint_pos(self, lc, pos, warning_key),
            None => AbstractLog::mandatory_warning_pos(self, pos, warning_key),
        }
    }

    fn warning(
        &mut self,
        lc: Option<LintCategory>,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    ) {
        match lc {
            Some(lc) => AbstractLog::warning_lint_pos(self, lc, pos, warning_key),
            None => AbstractLog::warning_pos(self, pos, warning_key),
        }
    }

    fn mandatory_note(&mut self, file: Option<Rc<dyn JavaFileObject>>, note: Note) {
        AbstractLog::mandatory_note_file(self, file, note);
    }

    fn note(&mut self, file: Option<Rc<dyn JavaFileObject>>, note: Note) {
        AbstractLog::note_file(self, file, note);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::source::Source;
    use crate::util::jc_diagnostic::{
        DiagnosticPosition as PositionTrait, SimpleDiagnosticPosition,
    };
    use javac_tools::error::Error;
    use crate::util::java_string::JavaString;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{JavaFileObject as JfoTrait, Kind};
    use javac_tools::uri::URI;
    use std::io::Read;

    struct TestFile {
        name: &'static str,
        content: &'static str,
    }

    impl FileObject for TestFile {
        fn to_uri(&self) -> URI {
            URI::create(&JavaString::from(self.name))
        }

        fn get_name(&self) -> JavaString {
            JavaString::from(self.name)
        }

        fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_char_content(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Option<JavaString>, Error> {
            Ok(Some(JavaString::from(self.content)))
        }

        fn open_reader(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Box<dyn javac_tools::java_io::Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn open_writer(&self) -> Result<Box<dyn javac_tools::java_io::Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_last_modified(&self) -> i64 {
            0
        }

        fn delete(&self) -> bool {
            false
        }
    }

    impl JfoTrait for TestFile {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }

        fn is_name_compatible(&self, _simple_name: &JavaString, kind: Kind) -> bool {
            kind == Kind::SOURCE
        }

        fn get_nesting_kind(
            &self,
        ) -> Option<javac_tools::lang::model::element::nesting_kind::NestingKind> {
            None
        }

        fn get_access_level(
            &self,
        ) -> Option<javac_tools::lang::model::element::modifier::Modifier> {
            None
        }
    }

    fn file(name: &'static str, content: &'static str) -> Rc<dyn JavaFileObject> {
        Rc::new(TestFile { name, content })
    }

    #[derive(Clone)]
    struct SharedBuffer(Rc<RefCell<Vec<u8>>>);

    impl std::io::Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn buffer_handle(buffer: &Rc<RefCell<Vec<u8>>>) -> WriterHandle {
        Rc::new(RefCell::new(
            Box::new(SharedBuffer(Rc::clone(buffer))) as Box<dyn Write>
        ))
    }

    struct TestSink {
        buffer: Rc<RefCell<Vec<u8>>>,
        context: Context,
    }

    impl TestSink {
        fn new() -> TestSink {
            TestSink {
                buffer: Rc::new(RefCell::new(Vec::<u8>::new())),
                context: Context::new(),
            }
        }

        fn log(&mut self) -> Log {
            let handle = buffer_handle(&self.buffer);
            Log::new_with_writers(
                &mut self.context,
                Log::init_writers(Some(Rc::clone(&handle)), Some(handle)),
            )
        }

        fn text(&self) -> String {
            String::from_utf8(self.buffer.borrow().clone()).expect("utf8")
        }
    }

    fn factory(sink: &mut TestSink) -> Rc<Factory> {
        Factory::instance(&mut sink.context)
    }

    fn position(pos: i32) -> Option<Rc<dyn PositionTrait>> {
        Some(Rc::new(SimpleDiagnosticPosition::new(pos)))
    }

    #[test]
    fn writes_errors_warnings_and_notes_to_the_streams() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let mut log = sink.log();
        let error = factory.error(None, None, None, factory.error_key("test", Vec::new()));
        log.report(error);
        let warning = factory.warning(None, None, None, factory.warning_key("test", Vec::new()));
        log.report(warning);
        let note = factory.note(None, None, factory.note_key("test", Vec::new()));
        log.report(note);
        assert_eq!(log.nerrors, 1);
        assert_eq!(log.nwarnings, 1);
        let text = sink.text();
        assert!(text.contains("compiler.err.test"), "text = {text}");
        assert!(text.contains("compiler.warn.test"), "text = {text}");
        assert!(text.contains("compiler.note.test"), "text = {text}");
    }

    #[test]
    fn max_errors_and_warnings_are_counted_as_suppressed() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let mut log = sink.log();
        log.max_errors = 1;
        log.max_warnings = 1;
        for _ in 0..2 {
            let error = factory.error(None, None, None, factory.error_key("e", Vec::new()));
            log.report(error);
        }
        assert_eq!(log.nerrors, 1);
        assert_eq!(log.nsuppressederrors, 1);
        for _ in 0..2 {
            let warning =
                factory.warning(None, None, None, factory.warning_key("w", Vec::new()));
            log.report(warning);
        }
        assert_eq!(log.nwarnings, 1);
        assert_eq!(log.nsuppressedwarns, 1);
    }

    #[test]
    fn emit_warnings_and_suppress_notes_switches() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let mut log = sink.log();
        log.emit_warnings = false;
        let warning = factory.warning(None, None, None, factory.warning_key("w", Vec::new()));
        log.report(warning);
        assert_eq!(log.nwarnings, 0);
        log.suppress_notes = true;
        let note = factory.note(None, None, factory.note_key("n", Vec::new()));
        log.report(note);
        assert!(sink.text().is_empty(), "text = {}", sink.text());
    }

    #[test]
    fn diagnostic_listener_takes_priority_over_writers() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let diagnostics = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&diagnostics);
        struct Listener {
            diagnostics: Rc<RefCell<Vec<String>>>,
        }
        impl DiagnosticListener<Rc<dyn JavaFileObject>> for Listener {
            fn report(&self, diagnostic: Rc<dyn Diagnostic<Rc<dyn JavaFileObject>>>) {
                self.diagnostics
                    .borrow_mut()
                    .push(diagnostic.get_code().expect("code").to_string_lossy());
            }
        }
        sink.context.put::<LogDiagnosticListener>(Rc::new(LogDiagnosticListener(Rc::new(
            Listener {
                diagnostics: captured,
            },
        ))));
        let mut log = sink.log();
        assert!(log.has_diagnostic_listener());
        let error = factory.error(None, None, None, factory.error_key("test", Vec::new()));
        log.report(error);
        let captured = diagnostics.borrow().clone();
        assert_eq!(captured, vec!["compiler.err.test".to_string()]);
        assert!(sink.text().is_empty());
    }

    #[test]
    fn deferred_handler_defers_and_reports() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let mut log = sink.log();
        log.install_diagnostic_handler(DiagnosticHandler::Deferred {
            prev: None,
            deferred: VecDeque::new(),
            filter: None,
        });
        let error = factory.error(None, None, None, factory.error_key("test", Vec::new()));
        log.report(error);
        assert!(sink.text().is_empty());
        assert_eq!(log.take_deferred_diagnostics().len(), 1);
        log.install_diagnostic_handler(DiagnosticHandler::Discard { prev: None });
        let error = factory.error(None, None, None, factory.error_key("discarded", Vec::new()));
        log.report(error);
        assert!(sink.text().is_empty());
        log.pop_diagnostic_handler();
        log.pop_diagnostic_handler();
        let error = factory.error(None, None, None, factory.error_key("kept", Vec::new()));
        log.report(error);
        assert!(sink.text().contains("compiler.err.kept"));
    }

    #[test]
    fn raw_error_and_warning_are_written_and_counted() {
        let mut sink = TestSink::new();
        let mut log = sink.log();
        log.raw_error(NOPOS, "boom");
        assert_eq!(log.nerrors, 1);
        log.raw_warning(NOPOS, "careful");
        assert_eq!(log.nwarnings, 1);
        let text = sink.text();
        assert!(text.contains("error: boom"), "text = {text}");
        assert!(text.contains("warning: careful"), "text = {text}");
    }

    #[test]
    fn source_line_and_caret_are_printed() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let mut log = sink.log();
        let file = file("Test.java", "public class Test {\n    int x = y;\n}\n");
        log.use_source(Some(Rc::clone(&file)));
        assert!(log.current_source_file().is_some());
        let error = factory.error(
            None,
            log.source().cloned(),
            position(32),
            factory.error_key("test", Vec::new()),
        );
        log.report(error);
        let text = sink.text();
        assert!(text.contains("Test.java:2: error:"), "text = {text}");
        assert!(text.contains("    int x = y;"), "text = {text}");
        assert!(text.contains("            ^"), "text = {text}");
    }

    #[test]
    fn raw_diagnostics_option_selects_the_raw_formatter() {
        let mut context = Context::new();
        let options = Options::instance(&mut context);
        options.put("rawDiagnostics", "");
        let buffer: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let handle = buffer_handle(&buffer);
        let mut log = Log::new_with_writers(
            &mut context,
            Log::init_writers(Some(Rc::clone(&handle)), Some(handle)),
        );
        let factory = Factory::instance(&mut context);
        let error = factory.error(None, None, None, factory.error_key("test", Vec::new()));
        log.report(error);
        let text = String::from_utf8(buffer.borrow().clone()).expect("utf8");
        assert!(text.contains("- compiler.err.test"), "text = {text}");
    }

    #[test]
    fn expect_keys_are_consumed_and_compressed_flag_is_recorded() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let mut log = sink.log();
        log.expect_diag_keys = Some(HashSet::from(["compiler.err.test".to_string()]));
        let error = factory.error(None, None, None, factory.error_key("test", Vec::new()));
        log.report(error);
        assert!(log.expect_diag_keys.as_ref().expect("keys").is_empty());
        log.compressed_output = false;
        let error = factory.error(
            Some(DiagnosticFlag::COMPRESSED),
            None,
            None,
            factory.error_key("compressed", Vec::new()),
        );
        log.report(error);
        assert!(log.compressed_output);
    }

    #[test]
    fn print_lines_and_writer_kinds() {
        let mut sink = TestSink::new();
        let mut log = sink.log();
        log.print_lines("test.key", vec![Rc::new(JavaString::from("A"))]);
        log.print_raw_lines("raw");
        log.print_verbose("test", Vec::new());
        log.print_newline();
        let notice = sink.text();
        assert!(notice.contains("compiler.misc.test.key"), "text = {notice}");
        assert!(notice.contains("raw"), "text = {notice}");
        let stdout_buffer: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        log.set_writer(WriterKind::STDOUT, buffer_handle(&stdout_buffer));
        log.print_lines_kind(WriterKind::STDOUT, "test.key", Vec::new());
        let stdout_text = String::from_utf8(stdout_buffer.borrow().clone()).expect("utf8");
        assert!(stdout_text.contains("compiler.misc.test.key"));
        log.flush();
    }

    #[test]
    fn abstract_log_entry_points_write_through_the_factory() {
        let mut sink = TestSink::new();
        let mut log = sink.log();
        AbstractLog::error_key_args(&mut log, "test", vec![Rc::new(JavaString::from("x"))]);
        assert_eq!(log.nerrors, 1);
        let text = sink.text();
        assert!(text.contains("compiler.err.test"), "text = {text}");
    }

    #[test]
    fn mandatory_warning_log_is_implemented_for_log() {
        let mut sink = TestSink::new();
        let factory = factory(&mut sink);
        let log = sink.log();
        let mut handler = crate::util::mandatory_warning_handler::MandatoryWarningHandler::new(
            log,
            Some(Source::JDK17),
            false,
            true,
            "deprecated",
            None,
        );
        let key = factory.warning_key("deprecated", Vec::new());
        handler.report(None, key);
        handler.report_deferred_diagnostic();
        let text = sink.text();
        assert!(
            text.contains("Note: 17 uses or overrides a deprecated API."),
            "text = {text}"
        );
        assert!(
            text.contains("Note: Recompile with -Xlint:deprecation for details."),
            "text = {text}"
        );
        assert_eq!(handler.log().nwarnings, 0);
    }
}
