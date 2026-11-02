//! A handler to process mandatory warnings, setting up a deferred diagnostic to
//! be printed at the end of the compilation if some warnings get suppressed
//! because too many warnings have already been generated.
//!
//! Note that the SuppressWarnings annotation can be used to suppress warnings
//! about conditions that would otherwise merit a warning. Such processing is
//! done when the condition is detected, and in those cases, no call is made on
//! any API to generate a warning at all. In consequence, this handler only
//! returns to handle those warnings that JLS says must be generated.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - the Java Log dependency (Log is not ported yet) becomes the
//!   MandatoryWarningLog trait, exposing exactly the members used by the
//!   upstream class; the Log implements the trait (registered section 9);
//!   a test mock implements it to exercise the de-duplication/aggregation
//!   logic;
//! - the Java HashSet<JavaFileObject> maps to Vec<Rc<dyn JavaFileObject>> with
//!   Rc::ptr_eq identity lookups (JavaFileObject does not override equals);
//!   the nullable source files are Option, and Objects.equals on them maps to
//!   the same identity check including None == None (registered section 9);
//! - the Java Object deferredDiagnosticArg maps to Option<DiagnosticArg>
//!   (Rc<dyn Any>) so the original object identity is preserved (ADR 0002
//!   revision C); the Source field is code::source::Source (Copy).
//!
//! NOT PORTED IN THIS STEP:
//! - the concrete Log implementation (the Log); the trait documents the
//!   upstream members: currentSourceFile(), nwarnings, MaxWarnings,
//!   mandatoryWarning/warning/mandatoryNote/note.

use std::rc::Rc;

use javac_tools::tools::java_file_object::JavaFileObject;

use crate::code::lint::LintCategory;
use crate::code::source::Source;
use crate::util::jc_diagnostic::{DiagnosticArg, DiagnosticPosition, Note, Warning};

/// The subset of com.sun.tools.javac.util.Log needed by
/// MandatoryWarningHandler.
///
/// PORTING_RULES: the upstream class holds a concrete Log reference; the port
/// uses a trait so the handler is testable and the Log can implement it
/// (registered section 9).
pub trait MandatoryWarningLog {
    /// Java: JavaFileObject currentSourceFile()
    fn current_source_file(&mut self) -> Option<Rc<dyn JavaFileObject>>;

    /// Java: log.nwarnings (public field of Log)
    fn nwarnings(&self) -> i32;

    /// Java: log.MaxWarnings (public field of Log)
    fn max_warnings(&self) -> i32;

    /// Java: Log.mandatoryWarning(LintCategory lc, DiagnosticPosition pos,
    /// Warning warningKey)
    fn mandatory_warning(
        &mut self,
        lc: Option<LintCategory>,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    );

    /// Java: Log.warning(LintCategory lc, DiagnosticPosition pos,
    /// Warning warningKey)
    fn warning(
        &mut self,
        lc: Option<LintCategory>,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    );

    /// Java: Log.mandatoryNote(JavaFileObject file, Note noteKey)
    fn mandatory_note(&mut self, file: Option<Rc<dyn JavaFileObject>>, note: Note);

    /// Java: Log.note(JavaFileObject file, Note noteKey)
    fn note(&mut self, file: Option<Rc<dyn JavaFileObject>>, note: Note);
}

/// The kinds of different deferred diagnostics that might be generated if a
/// mandatory warning is suppressed because too many warnings have already been
/// output.
///
/// The parameter is a fragment used to build an I18N message key for Log.
///
/// Java: private enum MandatoryWarningHandler.DeferredDiagnosticKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredDiagnosticKind {
    /// FILE has ISSUES
    IN_FILE,
    /// FILE has additional ISSUES
    ADDITIONAL_IN_FILE,
    /// Some files have ISSUES
    IN_FILES,
    /// Some files have additional ISSUES
    ADDITIONAL_IN_FILES,
}

impl DeferredDiagnosticKind {
    /// Java: String getKey(String prefix) { return prefix + value; }
    fn get_key(self, prefix: &str) -> String {
        let value = match self {
            DeferredDiagnosticKind::IN_FILE => ".filename",
            DeferredDiagnosticKind::ADDITIONAL_IN_FILE => ".filename.additional",
            DeferredDiagnosticKind::IN_FILES => ".plural",
            DeferredDiagnosticKind::ADDITIONAL_IN_FILES => ".plural.additional",
        };
        format!("{prefix}{value}")
    }
}

/// A handler to process mandatory warnings.
///
/// Java: public class MandatoryWarningHandler
///
/// PORTING_RULES: the Java Log field becomes the generic L: MandatoryWarningLog
/// parameter (registered section 9).
pub struct MandatoryWarningHandler<L: MandatoryWarningLog> {
    /// The log to which to report warnings.
    log: L,
    /// The source language version (the upstream Source field; nullable).
    source: Option<Source>,
    /// Whether or not to report individual warnings, or simply to report a
    /// single aggregate warning at the end of the compilation.
    verbose: bool,
    /// The common prefix for all I18N message keys generated by this handler.
    prefix: String,
    /// A set containing the source files for which specific warnings have been
    /// generated -- i.e. in verbose mode.
    sources_with_reported_warnings: Option<Vec<Option<Rc<dyn JavaFileObject>>>>,
    /// The latest best guess at what the final deferred diagnostic will be.
    deferred_diagnostic_kind: Option<DeferredDiagnosticKind>,
    /// If deferredDiagnosticKind is IN_FILE or ADDITIONAL_IN_FILE, the value of
    /// log.currentSourceFile() for the file in question.
    deferred_diagnostic_source: Option<Rc<dyn JavaFileObject>>,
    /// An optional argument used when constructing the deferred diagnostic
    /// message, based on deferredDiagnosticKind.
    deferred_diagnostic_arg: Option<DiagnosticArg>,
    /// True if mandatory warnings and notes are being enforced.
    enforce_mandatory: bool,
    /// A LintCategory included in point-of-use diagnostics.
    lint_category: Option<LintCategory>,
}

impl<L: MandatoryWarningLog> MandatoryWarningHandler<L> {
    /// Create a handler for mandatory warnings.
    ///
    /// Java: public MandatoryWarningHandler(Log log, Source source,
    /// boolean verbose, boolean enforceMandatory, String prefix,
    /// LintCategory lc)
    pub fn new(
        log: L,
        source: Option<Source>,
        verbose: bool,
        enforce_mandatory: bool,
        prefix: &str,
        lc: Option<LintCategory>,
    ) -> MandatoryWarningHandler<L> {
        MandatoryWarningHandler {
            log,
            source,
            verbose,
            prefix: prefix.to_string(),
            sources_with_reported_warnings: None,
            deferred_diagnostic_kind: None,
            deferred_diagnostic_source: None,
            deferred_diagnostic_arg: None,
            enforce_mandatory,
            lint_category: lc,
        }
    }

    /// Java: private static boolean sameFile(JavaFileObject a, JavaFileObject b)
    ///
    /// PORTING_RULES: Objects.equals on the JavaFileObject references (the
    /// interface does not override equals) -> identity comparison, with two
    /// nulls equal (registered section 9).
    fn same_file(a: &Option<Rc<dyn JavaFileObject>>, b: &Option<Rc<dyn JavaFileObject>>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// Report a mandatory warning.
    ///
    /// Java: public void report(DiagnosticPosition pos, Warning warnKey)
    pub fn report(&mut self, pos: Option<Rc<dyn DiagnosticPosition>>, warn_key: Warning) {
        let current_source = self.log.current_source_file();

        if self.verbose {
            if self.sources_with_reported_warnings.is_none() {
                self.sources_with_reported_warnings = Some(Vec::new());
            }

            if self.log.nwarnings() < self.log.max_warnings() {
                // generate message and remember the source file
                self.log_mandatory_warning(pos, warn_key);
                self.sources_with_reported_warnings
                    .as_mut()
                    .expect("initialized above")
                    .push(current_source);
            } else if self.deferred_diagnostic_kind.is_none() {
                // set up deferred message
                let already_reported = self
                    .sources_with_reported_warnings
                    .as_ref()
                    .expect("initialized above")
                    .iter()
                    .any(|candidate| Self::same_file(candidate, &current_source));
                if already_reported {
                    // more errors in a file that already has reported warnings
                    self.deferred_diagnostic_kind = Some(DeferredDiagnosticKind::ADDITIONAL_IN_FILE);
                } else {
                    // warnings in a new source file
                    self.deferred_diagnostic_kind = Some(DeferredDiagnosticKind::IN_FILE);
                }
                self.deferred_diagnostic_source = current_source.clone();
                self.deferred_diagnostic_arg =
                    current_source.map(|file| Rc::new(file) as DiagnosticArg);
            } else if (self.deferred_diagnostic_kind == Some(DeferredDiagnosticKind::IN_FILE)
                || self.deferred_diagnostic_kind
                    == Some(DeferredDiagnosticKind::ADDITIONAL_IN_FILE))
                && !Self::same_file(&self.deferred_diagnostic_source, &current_source)
            {
                // additional errors in more than one source file
                self.deferred_diagnostic_kind =
                    Some(DeferredDiagnosticKind::ADDITIONAL_IN_FILES);
                self.deferred_diagnostic_arg = None;
            }
        } else {
            if self.deferred_diagnostic_kind.is_none() {
                // warnings in a single source
                self.deferred_diagnostic_kind = Some(DeferredDiagnosticKind::IN_FILE);
                self.deferred_diagnostic_source = current_source.clone();
                self.deferred_diagnostic_arg =
                    current_source.map(|file| Rc::new(file) as DiagnosticArg);
            } else if self.deferred_diagnostic_kind == Some(DeferredDiagnosticKind::IN_FILE)
                && !Self::same_file(&self.deferred_diagnostic_source, &current_source)
            {
                // warnings in multiple source files
                self.deferred_diagnostic_kind = Some(DeferredDiagnosticKind::IN_FILES);
                self.deferred_diagnostic_arg = None;
            }
        }
    }

    /// Report any diagnostic that might have been deferred by previous calls of
    /// report().
    ///
    /// Java: public void reportDeferredDiagnostic()
    pub fn report_deferred_diagnostic(&mut self) {
        let Some(kind) = self.deferred_diagnostic_kind else {
            return;
        };
        let key = kind.get_key(&self.prefix);
        match &self.deferred_diagnostic_arg {
            None => {
                let args: Vec<DiagnosticArg> = match self.source {
                    Some(source) => vec![Rc::new(source) as DiagnosticArg],
                    None => Vec::new(),
                };
                let file = self.deferred_diagnostic_source.clone();
                self.log_mandatory_note(file, &key, args);
            }
            Some(deferred_arg) => {
                let mut args: Vec<DiagnosticArg> = vec![Rc::clone(deferred_arg)];
                if let Some(source) = self.source {
                    args.push(Rc::new(source) as DiagnosticArg);
                }
                let file = self.deferred_diagnostic_source.clone();
                self.log_mandatory_note(file, &key, args);
            }
        }

        if !self.verbose {
            let file = self.deferred_diagnostic_source.clone();
            let recompile = format!("{}.recompile", self.prefix);
            self.log_mandatory_note(file, &recompile, Vec::new());
        }
    }

    /// Reports a mandatory warning to the log. If mandatory warnings are not
    /// being enforced, treat this as an ordinary warning.
    ///
    /// Java: private void logMandatoryWarning(DiagnosticPosition pos,
    /// Warning warnKey)
    fn log_mandatory_warning(&mut self, pos: Option<Rc<dyn DiagnosticPosition>>, warn_key: Warning) {
        // Note: the following log methods are safe if lintCategory is null.
        if self.enforce_mandatory {
            self.log
                .mandatory_warning(self.lint_category, pos, warn_key);
        } else {
            self.log.warning(self.lint_category, pos, warn_key);
        }
    }

    /// Reports a mandatory note to the log. If mandatory notes are not being
    /// enforced, treat this as an ordinary note.
    ///
    /// Java: private void logMandatoryNote(JavaFileObject file, String msg,
    /// Object... args)
    fn log_mandatory_note(
        &mut self,
        file: Option<Rc<dyn JavaFileObject>>,
        msg: &str,
        args: Vec<DiagnosticArg>,
    ) {
        let note = Note::new("compiler", msg, args);
        if self.enforce_mandatory {
            self.log.mandatory_note(file, note);
        } else {
            self.log.note(file, note);
        }
    }

    /// The underlying log (Rust accessor for the generic log parameter).
    pub fn log(&self) -> &L {
        &self.log
    }

    /// Mutable access to the underlying log (Rust accessor for the generic log
    /// parameter, used by the integration tests).
    pub fn log_mut(&mut self) -> &mut L {
        &mut self.log
    }

    /// Java: public void clear()
    pub fn clear(&mut self) {
        self.sources_with_reported_warnings = None;
        self.deferred_diagnostic_kind = None;
        self.deferred_diagnostic_source = None;
        self.deferred_diagnostic_arg = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::jc_diagnostic::{Factory, SimpleDiagnosticPosition};
    use crate::util::javac_messages::JavacMessages;
    use javac_tools::error::Error;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{JavaFileObject as JfoTrait, Kind};
    use javac_tools::uri::URI;
    use std::io::{Read, Write};
    use crate::util::java_string::JavaString;

    struct TestFile {
        name: &'static str,
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
            Ok(None)
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

    fn file(name: &'static str) -> Rc<dyn JavaFileObject> {
        Rc::new(TestFile { name })
    }

    #[derive(Default)]
    struct MockLog {
        nwarnings: i32,
        max_warnings: i32,
        current: Option<Rc<dyn JavaFileObject>>,
        mandatory_warnings: Vec<Warning>,
        warnings: Vec<Warning>,
        mandatory_notes: Vec<(Option<Rc<dyn JavaFileObject>>, Note)>,
        notes: Vec<(Option<Rc<dyn JavaFileObject>>, Note)>,
    }

    impl MandatoryWarningLog for MockLog {
        fn current_source_file(&mut self) -> Option<Rc<dyn JavaFileObject>> {
            self.current.clone()
        }

        fn nwarnings(&self) -> i32 {
            self.nwarnings
        }

        fn max_warnings(&self) -> i32 {
            self.max_warnings
        }

        fn mandatory_warning(
            &mut self,
            _lc: Option<LintCategory>,
            _pos: Option<Rc<dyn DiagnosticPosition>>,
            warning_key: Warning,
        ) {
            self.mandatory_warnings.push(warning_key);
        }

        fn warning(
            &mut self,
            _lc: Option<LintCategory>,
            _pos: Option<Rc<dyn DiagnosticPosition>>,
            warning_key: Warning,
        ) {
            self.warnings.push(warning_key);
        }

        fn mandatory_note(&mut self, file: Option<Rc<dyn JavaFileObject>>, note: Note) {
            self.mandatory_notes.push((file, note));
        }

        fn note(&mut self, file: Option<Rc<dyn JavaFileObject>>, note: Note) {
            self.notes.push((file, note));
        }
    }

    fn factory() -> Factory {
        Factory::new(
            Rc::new(JavacMessages::new(
                crate::util::javac_messages::DEFAULT_BUNDLE_NAME,
            )),
            "compiler",
        )
    }

    fn warning_key(factory: &Factory) -> Warning {
        factory.warning_key("mask.filename", Vec::new())
    }

    fn position() -> Option<Rc<dyn DiagnosticPosition>> {
        Some(Rc::new(SimpleDiagnosticPosition::new(1)))
    }

    fn mock() -> MockLog {
        MockLog {
            nwarnings: 0,
            max_warnings: 100,
            current: None,
            mandatory_warnings: Vec::new(),
            warnings: Vec::new(),
            mandatory_notes: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn verbose_reports_individual_warnings_below_the_limit() {
        let factory = factory();
        let mut log = mock();
        log.current = Some(file("A.java"));
        let mut handler =
            MandatoryWarningHandler::new(log, None, true, true, "mask", None);
        handler.report(position(), warning_key(&factory));
        assert_eq!(handler.log.mandatory_warnings.len(), 1);
        assert!(handler.deferred_diagnostic_kind.is_none());
        handler.report_deferred_diagnostic();
        assert!(handler.log.mandatory_notes.is_empty());
    }

    #[test]
    fn non_verbose_aggregates_the_files() {
        let factory = factory();
        let mut log = mock();
        log.current = Some(file("A.java"));
        let mut handler = MandatoryWarningHandler::new(
            log,
            Some(Source::JDK17),
            false,
            true,
            "mask",
            None,
        );
        handler.report(position(), warning_key(&factory));
        assert_eq!(
            handler.deferred_diagnostic_kind,
            Some(DeferredDiagnosticKind::IN_FILE)
        );
        handler.log.current = Some(file("B.java"));
        handler.report(position(), warning_key(&factory));
        assert_eq!(
            handler.deferred_diagnostic_kind,
            Some(DeferredDiagnosticKind::IN_FILES)
        );
        handler.report_deferred_diagnostic();
        assert_eq!(handler.log.mandatory_notes.len(), 2);
        let (_, first_note) = &handler.log.mandatory_notes[0];
        // The real resources use the note kind: compiler.note.<prefix>.<suffix>.
        assert_eq!(first_note.info().key(), "compiler.note.mask.plural");
        let (_, second_note) = &handler.log.mandatory_notes[1];
        assert_eq!(
            second_note.info().key(),
            "compiler.note.mask.recompile"
        );
    }

    #[test]
    fn verbose_aggregates_in_file_and_additional_cases() {
        let factory = factory();
        let mut log = mock();
        log.nwarnings = 100;
        log.max_warnings = 100;
        let file_a = file("A.java");
        log.current = Some(Rc::clone(&file_a));
        let mut handler = MandatoryWarningHandler::new(
            log,
            None,
            true,
            true,
            "mask",
            Some(LintCategory::DEPRECATION),
        );
        handler.report(position(), warning_key(&factory));
        assert_eq!(
            handler.deferred_diagnostic_kind,
            Some(DeferredDiagnosticKind::IN_FILE)
        );
        // The same file again: additional in file.
        handler.report(position(), warning_key(&factory));
        assert_eq!(
            handler.deferred_diagnostic_kind,
            Some(DeferredDiagnosticKind::IN_FILE)
        );
        // A different file: additional in files.
        handler.log.current = Some(file("B.java"));
        handler.report(position(), warning_key(&factory));
        assert_eq!(
            handler.deferred_diagnostic_kind,
            Some(DeferredDiagnosticKind::ADDITIONAL_IN_FILES)
        );
        let _ = file_a;
    }

    #[test]
    fn enforce_mandatory_false_uses_ordinary_warnings_and_notes() {
        let factory = factory();
        let mut log = mock();
        log.current = Some(file("A.java"));
        let mut handler = MandatoryWarningHandler::new(
            log,
            None,
            false,
            false,
            "mask",
            None,
        );
        handler.report(position(), warning_key(&factory));
        handler.report_deferred_diagnostic();
        assert!(handler.log.mandatory_warnings.is_empty());
        assert!(handler.log.warnings.is_empty());
        assert_eq!(handler.log.notes.len(), 2);
        assert!(handler.log.mandatory_notes.is_empty());
    }

    #[test]
    fn clear_resets_the_deferred_state() {
        let factory = factory();
        let mut log = mock();
        log.current = Some(file("A.java"));
        let mut handler = MandatoryWarningHandler::new(
            log,
            None,
            false,
            true,
            "mask",
            None,
        );
        handler.report(position(), warning_key(&factory));
        handler.clear();
        assert!(handler.deferred_diagnostic_kind.is_none());
        assert!(handler.sources_with_reported_warnings.is_none());
        handler.report_deferred_diagnostic();
        assert!(handler.log.mandatory_notes.is_empty());
    }
}
