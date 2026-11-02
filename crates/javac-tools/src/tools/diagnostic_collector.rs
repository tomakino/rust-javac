//! Provides an easy way to collect diagnostics in a list.

use super::diagnostic::Diagnostic;
use super::diagnostic_listener::DiagnosticListener;
use std::cell::RefCell;
use std::rc::Rc;

/// Provides an easy way to collect diagnostics in a list.
///
/// @param <S> the type of source objects used by diagnostics received
/// by this object
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// — class mapping:
/// * field diagnostics (Java:
///   Collections.synchronizedList(new ArrayList&lt;Diagnostic&lt;? extends S&gt;&gt;()))
///   -> RefCell of Vec of Rc of dyn Diagnostic of S.  The Java file manager
///   API is single-threaded in the port; RefCell gives report(&self) the
///   mutable list Java has through its mutable list reference.
/// * getDiagnostics() returns a Vec: Java returns an unmodifiable *view*, a
///   Rust Vec clone is a snapshot.  The snapshot contains the same Rc
///   elements; later report calls do not appear in an already returned
///   snapshot (registered difference).
pub struct DiagnosticCollector<S> {
    diagnostics: RefCell<Vec<Rc<dyn Diagnostic<S>>>>,
}

impl<S> DiagnosticCollector<S> {
    /// Creates a new instance of DiagnosticCollector.
    pub fn new() -> DiagnosticCollector<S> {
        DiagnosticCollector {
            diagnostics: RefCell::new(Vec::new()),
        }
    }

    /// Returns a list view of diagnostics collected by this object.
    ///
    /// @return a list view of diagnostics
    pub fn get_diagnostics(&self) -> Vec<Rc<dyn Diagnostic<S>>> {
        self.diagnostics.borrow().clone()
    }
}

impl<S> DiagnosticListener<S> for DiagnosticCollector<S> {
    // Java: public void report(Diagnostic<? extends S> diagnostic)
    //           Objects.requireNonNull(diagnostic);
    //           diagnostics.add(diagnostic);
    // Rust: Rc is non-nullable, so the null check has no Rust form.
    fn report(&self, diagnostic: Rc<dyn Diagnostic<S>>) {
        self.diagnostics.borrow_mut().push(diagnostic);
    }
}

impl<S> Default for DiagnosticCollector<S> {
    fn default() -> DiagnosticCollector<S> {
        DiagnosticCollector::new()
    }
}

#[cfg(test)]
mod tests {
    use super::DiagnosticCollector;
    use crate::tools::diagnostic::{Diagnostic, Kind};
    use crate::tools::diagnostic_listener::DiagnosticListener;
    use javac_strings::JavaString;
    use std::rc::Rc;

    struct Diag(i32);

    impl Diagnostic<i32> for Diag {
        fn get_kind(&self) -> Kind {
            Kind::WARNING
        }
        fn get_source(&self) -> Option<i32> {
            Some(self.0)
        }
        fn get_position(&self) -> i64 {
            self.0 as i64
        }
        fn get_start_position(&self) -> i64 {
            self.0 as i64
        }
        fn get_end_position(&self) -> i64 {
            self.0 as i64
        }
        fn get_line_number(&self) -> i64 {
            1
        }
        fn get_column_number(&self) -> i64 {
            1
        }
        fn get_code(&self) -> Option<JavaString> {
            None
        }
        fn get_message(&self, _locale: Option<&JavaString>) -> JavaString {
            JavaString::new()
        }
    }

    #[test]
    fn collects_in_order() {
        let collector: DiagnosticCollector<i32> = DiagnosticCollector::new();
        collector.report(Rc::new(Diag(1)));
        collector.report(Rc::new(Diag(2)));
        let diagnostics = collector.get_diagnostics();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].get_position(), 1);
        assert_eq!(diagnostics[1].get_position(), 2);
    }

    #[test]
    fn snapshot_does_not_see_later_reports() {
        let collector: DiagnosticCollector<i32> = DiagnosticCollector::new();
        collector.report(Rc::new(Diag(1)));
        let snapshot = collector.get_diagnostics();
        collector.report(Rc::new(Diag(2)));
        assert_eq!(snapshot.len(), 1);
        assert_eq!(collector.get_diagnostics().len(), 2);
    }
}
