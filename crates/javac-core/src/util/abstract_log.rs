// Origin: com/sun/tools/javac/util/AbstractLog.java @ 982d53c (jdk-17.0.19-ga)

//! A base class for error logs. Reports errors and warnings, and keeps track of
//! error numbers and positions.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - the Java abstract class becomes a trait: the upstream protected fields
//!   (diags / source / sourceMap) are exposed through accessor methods that the
//!   implementor provides (Rust has no inherited fields), and the two abstract
//!   methods (report / directError) are required trait methods. The concrete
//!   methods are default methods;
//! - the Java overloads map to distinct Rust names (PORTING_RULES section 3:
//!   no overloading); each method documents its upstream signature;
//! - DiagnosticPosition parameters are Option<Rc<dyn DiagnosticPosition>> (the
//!   Java null position); the private wrap(int pos) helper maps NOPOS to None;
//! - sourceMap is keyed by JavaFileObject identity in Java (the interface does
//!   not define equals); the port uses a Vec of (Rc<dyn JavaFileObject>,
//!   Rc<DiagnosticSource>) and looks the entry up with Rc::ptr_eq (registered
//!   section 9);
//! - the DiagnosticSource parameter is now forwarded to the JCDiagnostic.Factory
//!   calls exactly like the upstream methods (the source field and
//!   getDiagnosticSource() were restored together with the diagnostic
//!   formatter); the directError hook is expressed by the
//!   DiagnosticSourceLog trait (see diagnostic_source.rs) and is wired by the
//!   Log when the concrete log exists.

use std::rc::Rc;

use javac_tools::tools::java_file_object::JavaFileObject;

use crate::util::diagnostic_source::DiagnosticSource;
use crate::util::jc_diagnostic::{
    DiagnosticArg, DiagnosticFlag, DiagnosticPosition, Error as JCDiagnosticError, Factory,
    JCDiagnostic, LintCategory, Note, SimpleDiagnosticPosition, Warning,
};
use crate::util::position::NOPOS;

/// Java: public abstract class AbstractLog
pub trait AbstractLog {
    /// Java: protected JCDiagnostic.Factory diags
    fn diags(&self) -> &Rc<Factory>;

    /// Java: protected DiagnosticSource source
    fn source(&self) -> Option<&Rc<DiagnosticSource>>;

    /// Re-assign the source field (the upstream field is protected, so the
    /// accessor is part of the class contract).
    fn set_source(&mut self, source: Option<Rc<DiagnosticSource>>);

    /// Java: protected Map<JavaFileObject, DiagnosticSource> sourceMap
    fn source_map(&mut self) -> &mut Vec<(Rc<dyn JavaFileObject>, Rc<DiagnosticSource>)>;

    /// Java: protected abstract void report(JCDiagnostic diagnostic)
    fn report(&mut self, diagnostic: JCDiagnostic);

    /// Java: protected abstract void directError(String key, Object... args)
    fn direct_error(&mut self, key: &str, args: Vec<String>);

    /// Re-assign source, returning previous setting.
    ///
    /// Java: public JavaFileObject useSource(JavaFileObject file)
    fn use_source(&mut self, file: Option<Rc<dyn JavaFileObject>>) -> Option<Rc<dyn JavaFileObject>> {
        let prev = self.source().and_then(|source| source.get_file());
        let source = self.get_source(file);
        self.set_source(Some(source));
        prev
    }

    /// Java: protected DiagnosticSource getSource(JavaFileObject file)
    fn get_source(&mut self, file: Option<Rc<dyn JavaFileObject>>) -> Rc<DiagnosticSource> {
        let Some(file) = file else {
            // Java: if (file == null) return DiagnosticSource.NO_SOURCE;
            return Rc::new(DiagnosticSource::no_source());
        };
        if let Some(existing) = self
            .source_map()
            .iter()
            .find(|(candidate, _)| Rc::ptr_eq(candidate, &file))
            .map(|(_, source)| Rc::clone(source))
        {
            return existing;
        }
        // TODO(Log): pass the log itself as DiagnosticSourceLog once the
        // concrete Log exists (the Java code passes this).
        let source = Rc::new(DiagnosticSource::new(Rc::clone(&file), None));
        self.source_map().push((file, Rc::clone(&source)));
        source
    }

    /// Return the underlying diagnostic source.
    ///
    /// Java: public DiagnosticSource currentSource()
    fn current_source(&self) -> Option<Rc<DiagnosticSource>> {
        self.source().cloned()
    }

    /// Report an error, unless another error was already reported at same
    /// source position.
    ///
    /// Java: public void error(String key, Object... args)
    fn error_key_args(&mut self, key: &str, args: Vec<DiagnosticArg>) {
        let error_key = {
            let diags = self.diags();
            diags.error_key(key, args)
        };
        self.error_key(error_key);
    }

    /// Java: public void error(JCDiagnostic.Error errorKey)
    fn error_key(&mut self, error_key: JCDiagnosticError) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.error(None, source, None, error_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void error(DiagnosticPosition pos, JCDiagnostic.Error errorKey)
    fn error_pos(
        &mut self,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        error_key: JCDiagnosticError,
    ) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.error(None, source, pos, error_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void error(DiagnosticFlag flag, DiagnosticPosition pos,
    /// JCDiagnostic.Error errorKey)
    fn error_flag_pos(
        &mut self,
        flag: DiagnosticFlag,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        error_key: JCDiagnosticError,
    ) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.error(Some(flag), source, pos, error_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void error(int pos, String key, Object... args)
    fn error_at_key(&mut self, pos: i32, key: &str, args: Vec<DiagnosticArg>) {
        let error_key = {
            let diags = self.diags();
            diags.error_key(key, args)
        };
        self.error_at(pos, error_key);
    }

    /// Java: public void error(int pos, JCDiagnostic.Error errorKey)
    fn error_at(&mut self, pos: i32, error_key: JCDiagnosticError) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.error(None, source, Self::wrap_pos(pos), error_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void error(DiagnosticFlag flag, int pos,
    /// JCDiagnostic.Error errorKey)
    fn error_flag_at(&mut self, flag: DiagnosticFlag, pos: i32, error_key: JCDiagnosticError) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.error(Some(flag), source, Self::wrap_pos(pos), error_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void warning(JCDiagnostic.Warning warningKey)
    fn warning_key(&mut self, warning_key: Warning) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.warning(None, source, None, warning_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void warning(LintCategory lc, JCDiagnostic.Warning warningKey)
    fn warning_lint_key(&mut self, lc: LintCategory, warning_key: Warning) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.warning(Some(lc), source, None, warning_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void warning(DiagnosticPosition pos,
    /// JCDiagnostic.Warning warningKey)
    fn warning_pos(
        &mut self,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    ) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.warning(None, source, pos, warning_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void warning(LintCategory lc, DiagnosticPosition pos,
    /// JCDiagnostic.Warning warningKey)
    fn warning_lint_pos(
        &mut self,
        lc: LintCategory,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    ) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.warning(Some(lc), source, pos, warning_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void warning(int pos, JCDiagnostic.Warning warningKey)
    fn warning_at(&mut self, pos: i32, warning_key: Warning) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.warning(None, source, Self::wrap_pos(pos), warning_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void mandatoryWarning(DiagnosticPosition pos,
    /// JCDiagnostic.Warning warningKey)
    fn mandatory_warning_pos(
        &mut self,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    ) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.mandatory_warning(None, source, pos, warning_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void mandatoryWarning(LintCategory lc,
    /// DiagnosticPosition pos, JCDiagnostic.Warning warningKey)
    fn mandatory_warning_lint_pos(
        &mut self,
        lc: LintCategory,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        warning_key: Warning,
    ) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.mandatory_warning(Some(lc), source, pos, warning_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void note(JCDiagnostic.Note noteKey)
    fn note_key(&mut self, note_key: Note) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.note(source, None, note_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void note(DiagnosticPosition pos, JCDiagnostic.Note noteKey)
    fn note_pos(
        &mut self,
        pos: Option<Rc<dyn DiagnosticPosition>>,
        note_key: Note,
    ) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.note(source, pos, note_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void note(int pos, JCDiagnostic.Note noteKey)
    fn note_at(&mut self, pos: i32, note_key: Note) {
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.source().cloned();
            diags.note(source, Self::wrap_pos(pos), note_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void note(JavaFileObject file, JCDiagnostic.Note noteKey)
    fn note_file(
        &mut self,
        file: Option<Rc<dyn JavaFileObject>>,
        note_key: Note,
    ) {
        // Java: report(diags.note(getSource(file), null, noteKey));
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.get_source(file);
            diags.note(Some(source), None, note_key)
        };
        self.report(diagnostic);
    }

    /// Java: public void mandatoryNote(final JavaFileObject file,
    /// JCDiagnostic.Note noteKey)
    fn mandatory_note_file(
        &mut self,
        file: Option<Rc<dyn JavaFileObject>>,
        note_key: Note,
    ) {
        // Java: report(diags.mandatoryNote(getSource(file), noteKey));
        let diagnostic = {
            let diags = Rc::clone(self.diags());
            let source = self.get_source(file);
            diags.mandatory_note(Some(source), note_key)
        };
        self.report(diagnostic);
    }

    /// Java: private DiagnosticPosition wrap(int pos)
    ///
    /// Rust trait associated function: maps the NOPOS sentinel to a null
    /// position (None).
    fn wrap_pos(pos: i32) -> Option<Rc<dyn DiagnosticPosition>> {
        if pos == NOPOS {
            None
        } else {
            Some(Rc::new(SimpleDiagnosticPosition::new(pos)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::jc_diagnostic::{DiagnosticType, Factory};
    use crate::util::javac_messages::JavacMessages;
    use javac_tools::error::Error;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use javac_tools::uri::URI;
    use std::io::{Read, Write};

    struct TestFileObject;

    impl FileObject for TestFileObject {
        fn to_uri(&self) -> URI {
            URI::create(&crate::util::java_string::JavaString::from("mem:///A.java"))
        }

        fn get_name(&self) -> crate::util::java_string::JavaString {
            crate::util::java_string::JavaString::from("A.java")
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
        ) -> Result<Option<crate::util::java_string::JavaString>, Error> {
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

    impl JavaFileObject for TestFileObject {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }

        fn is_name_compatible(&self, _simple_name: &crate::util::java_string::JavaString, kind: Kind) -> bool {
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

    /// A concrete AbstractLog used to exercise the default methods.
    struct TestLog {
        diags: Rc<Factory>,
        source: Option<Rc<DiagnosticSource>>,
        source_map: Vec<(Rc<dyn JavaFileObject>, Rc<DiagnosticSource>)>,
        reported: Vec<JCDiagnostic>,
        direct_errors: Vec<String>,
    }

    impl TestLog {
        fn new() -> TestLog {
            let messages = Rc::new(JavacMessages::new(
                crate::util::javac_messages::DEFAULT_BUNDLE_NAME,
            ));
            TestLog {
                diags: Rc::new(Factory::new(messages, "compiler")),
                source: None,
                source_map: Vec::new(),
                reported: Vec::new(),
                direct_errors: Vec::new(),
            }
        }
    }

    impl AbstractLog for TestLog {
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
            self.reported.push(diagnostic);
        }

        fn direct_error(&mut self, key: &str, _args: Vec<String>) {
            self.direct_errors.push(key.to_string());
        }
    }

    #[test]
    fn use_source_returns_previous_and_caches() {
        let mut log = TestLog::new();
        assert!(log.current_source().is_none());
        let file = Rc::new(TestFileObject) as Rc<dyn JavaFileObject>;
        let prev = log.use_source(Some(Rc::clone(&file)));
        assert!(prev.is_none());
        let first = log.current_source().expect("source set");
        assert!(first.get_file().is_some());
        // The same file object returns the cached DiagnosticSource.
        let prev = log.use_source(Some(Rc::clone(&file)));
        assert!(prev.is_some());
        let second = log.current_source().expect("source set");
        assert!(Rc::ptr_eq(&first, &second));
        // A null file returns the no-source instance.
        let prev = log.use_source(None);
        assert!(prev.is_some());
        let no_source = log.current_source().expect("source set");
        assert!(no_source.get_file().is_none());
        assert!(!Rc::ptr_eq(&no_source, &first));
        assert_eq!(log.source_map.len(), 1);
    }

    #[test]
    fn error_methods_forward_diagnostics() {
        let mut log = TestLog::new();
        let file = Rc::new(TestFileObject) as Rc<dyn JavaFileObject>;
        log.use_source(Some(file));
        log.error_key_args("test.error", vec![Rc::new(crate::util::java_string::JavaString::from("a"))]);
        assert_eq!(log.reported.len(), 1);
        assert_eq!(log.reported[0].get_type(), DiagnosticType::ERROR);
        assert_eq!(log.reported[0].get_code_string(), "compiler.err.test.error");
        assert!(log.reported[0].is_mandatory());

        log.error_at(5, log.diags.error_key("test.error2", Vec::new()));
        assert_eq!(log.reported.len(), 2);
        assert_eq!(
            log.reported[1]
                .get_diagnostic_position()
                .expect("position")
                .get_preferred_position(),
            5
        );
        assert_eq!(log.reported[1].get_int_position(), 5);

        log.error_at(NOPOS, log.diags.error_key("test.error3", Vec::new()));
        assert!(log.reported[2].get_diagnostic_position().is_none());
        assert_eq!(log.reported[2].get_int_position(), NOPOS);
    }

    #[test]
    fn flagged_error_and_warning_methods() {
        let mut log = TestLog::new();
        let file = Rc::new(TestFileObject) as Rc<dyn JavaFileObject>;
        log.use_source(Some(file));
        log.error_flag_pos(
            DiagnosticFlag::SYNTAX,
            Some(Rc::new(SimpleDiagnosticPosition::new(1))),
            log.diags.error_key("test.syntax", Vec::new()),
        );
        assert!(log.reported[0].is_flag_set(DiagnosticFlag::SYNTAX));

        log.error_flag_at(
            DiagnosticFlag::RESOLVE_ERROR,
            2,
            log.diags.error_key("test.resolve", Vec::new()),
        );
        assert!(log.reported[1].is_flag_set(DiagnosticFlag::RESOLVE_ERROR));
        assert!(log.reported[1].is_flag_set(DiagnosticFlag::RECOVERABLE));

        log.warning_at(3, log.diags.warning_key("test.warning", Vec::new()));
        assert_eq!(log.reported[2].get_type(), DiagnosticType::WARNING);
        assert_eq!(log.reported[2].get_int_position(), 3);

        let lint = LintCategory::DEPRECATION;
        log.warning_lint_key(lint, log.diags.warning_key("test.lint", Vec::new()));
        assert_eq!(log.reported[3].get_lint_category(), Some(&lint));

        log.mandatory_warning_lint_pos(
            lint,
            Some(Rc::new(SimpleDiagnosticPosition::new(4))),
            log.diags.warning_key("test.mandatory", Vec::new()),
        );
        assert!(log.reported[4].is_mandatory());
        assert_eq!(log.reported[4].get_int_position(), 4);
    }

    #[test]
    fn note_methods_forward_diagnostics() {
        let mut log = TestLog::new();
        let file = Rc::new(TestFileObject) as Rc<dyn JavaFileObject>;
        log.use_source(Some(Rc::clone(&file)));
        log.note_key(log.diags.note_key("test.note", Vec::new()));
        assert_eq!(log.reported[0].get_type(), DiagnosticType::NOTE);
        assert!(!log.reported[0].is_mandatory());

        log.note_pos(
            Some(Rc::new(SimpleDiagnosticPosition::new(6))),
            log.diags.note_key("test.note2", Vec::new()),
        );
        assert_eq!(log.reported[1].get_int_position(), 6);

        log.note_at(7, log.diags.note_key("test.note3", Vec::new()));
        assert_eq!(log.reported[2].get_int_position(), 7);

        log.mandatory_note_file(Some(Rc::clone(&file)), log.diags.note_key("test.note4", Vec::new()));
        assert!(log.reported[3].is_mandatory());
        // The source was cached for the file.
        assert_eq!(log.source_map.len(), 1);
    }
}
