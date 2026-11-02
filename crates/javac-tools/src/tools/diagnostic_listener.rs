//! Interface for receiving diagnostics from tools.

use super::diagnostic::Diagnostic;
use std::rc::Rc;

/// Interface for receiving diagnostics from tools.
///
/// @param <S> the type of source objects used by diagnostics received
/// by this listener
///
/// @author Jonathan Gibbons
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
///
/// * Diagnostic of "? extends S" -> Rc of dyn Diagnostic of S: the diagnostic
///   is a shared Java object reference, so Rc expresses the shared identity
///   that a collecting listener stores.  The wildcard "? extends S" cannot be
///   expressed in Rust; the compiler only uses S = Rc of dyn JavaFileObject.
/// * report takes &self: Java listeners collect into a mutable list, and
///   DiagnosticCollector does exactly that behind a RefCell (Java:
///   Collections.synchronizedList).
pub trait DiagnosticListener<S> {
    /// Invoked when a problem is found.
    ///
    /// @param diagnostic a diagnostic representing the problem that
    /// was found
    /// @throws NullPointerException if the diagnostic argument is
    /// null and the implementation cannot handle null
    /// arguments
    fn report(&self, diagnostic: Rc<dyn Diagnostic<S>>);
}

#[cfg(test)]
mod tests {
    use super::DiagnosticListener;
    use crate::tools::diagnostic::{Diagnostic, Kind};
    use javac_strings::JavaString;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Diag;

    impl Diagnostic<u32> for Diag {
        fn get_kind(&self) -> Kind {
            Kind::NOTE
        }
        fn get_source(&self) -> Option<u32> {
            None
        }
        fn get_position(&self) -> i64 {
            -1
        }
        fn get_start_position(&self) -> i64 {
            -1
        }
        fn get_end_position(&self) -> i64 {
            -1
        }
        fn get_line_number(&self) -> i64 {
            -1
        }
        fn get_column_number(&self) -> i64 {
            -1
        }
        fn get_code(&self) -> Option<JavaString> {
            None
        }
        fn get_message(&self, _locale: Option<&JavaString>) -> JavaString {
            JavaString::new()
        }
    }

    struct CountingListener {
        count: RefCell<u32>,
    }

    impl DiagnosticListener<u32> for CountingListener {
        fn report(&self, _diagnostic: Rc<dyn Diagnostic<u32>>) {
            *self.count.borrow_mut() += 1;
        }
    }

    #[test]
    fn listener_receives_diagnostics() {
        let listener = CountingListener { count: RefCell::new(0) };
        listener.report(Rc::new(Diag));
        listener.report(Rc::new(Diag));
        assert_eq!(*listener.count.borrow(), 2);
    }
}
