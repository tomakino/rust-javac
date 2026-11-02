//! Optional-warning state for unchecked conversions and unchecked casts.
//!
//! The lint sets and the `warned` flag are the only mutable state; they are
//! held in `RefCell`/`Cell` so a `Warner` can be driven through `&self`.
//! Callers still share it as `Rc<RefCell<Warner>>` until the warning-sink
//! interface is migrated to `Rc<Warner>`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::code::lint::LintCategory;
use crate::util::jc_diagnostic::DiagnosticPosition;

/// An interface to support optional warnings.
///
/// Java: public class Warner
#[derive(Clone, Default)]
pub struct Warner {
    /// Java: private DiagnosticPosition pos (null -> None)
    pos: Option<Rc<dyn DiagnosticPosition>>,
    /// Java: protected boolean warned
    pub warned: Cell<bool>,
    /// Java: private EnumSet<LintCategory> nonSilentLintSet
    non_silent_lint_set: RefCell<Vec<LintCategory>>,
    /// Java: private EnumSet<LintCategory> silentLintSet
    silent_lint_set: RefCell<Vec<LintCategory>>,
}

impl Warner {
    /// Java: public Warner(DiagnosticPosition pos)
    pub fn new(pos: Option<Rc<dyn DiagnosticPosition>>) -> Warner {
        Warner {
            pos,
            warned: Cell::new(false),
            non_silent_lint_set: RefCell::new(Vec::new()),
            silent_lint_set: RefCell::new(Vec::new()),
        }
    }

    /// Java: public DiagnosticPosition pos()
    pub fn pos(&self) -> Option<Rc<dyn DiagnosticPosition>> {
        self.pos.clone()
    }

    /// Java: public void warn(LintCategory lint)
    pub fn warn(&self, lint: LintCategory) {
        let mut set = self.non_silent_lint_set.borrow_mut();
        if !set.contains(&lint) {
            set.push(lint);
        }
    }

    /// Java: public void silentWarn(LintCategory lint)
    pub fn silent_warn(&self, lint: LintCategory) {
        let mut set = self.silent_lint_set.borrow_mut();
        if !set.contains(&lint) {
            set.push(lint);
        }
    }

    /// Java: public boolean hasSilentLint(LintCategory lint)
    pub fn has_silent_lint(&self, lint: LintCategory) -> bool {
        self.silent_lint_set.borrow().contains(&lint)
    }

    /// Java: public boolean hasNonSilentLint(LintCategory lint)
    pub fn has_non_silent_lint(&self, lint: LintCategory) -> bool {
        self.non_silent_lint_set.borrow().contains(&lint)
    }

    /// Java: public boolean hasLint(LintCategory lint)
    pub fn has_lint(&self, lint: LintCategory) -> bool {
        self.has_silent_lint(lint) || self.has_non_silent_lint(lint)
    }

    /// Java: public void clear()
    pub fn clear(&self) {
        self.non_silent_lint_set.borrow_mut().clear();
        self.silent_lint_set.borrow_mut().clear();
        self.warned.set(false);
    }

    /// Copies the observable warning state from `other` into `self`.
    ///
    /// The port keeps the lint sets behind interior mutability; inference code
    /// that works on a cloned sink uses this to publish the final state back to
    /// the caller's sink.
    pub fn copy_from(&self, other: &Warner) {
        *self.non_silent_lint_set.borrow_mut() = other.non_silent_lint_set.borrow().clone();
        *self.silent_lint_set.borrow_mut() = other.silent_lint_set.borrow().clone();
        self.warned.set(other.warned.get());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::jc_diagnostic::SimpleDiagnosticPosition;

    #[test]
    fn lint_sets_and_clear() {
        let pos: Option<Rc<dyn DiagnosticPosition>> =
            Some(Rc::new(SimpleDiagnosticPosition::new(3)));
        let warner = Warner::new(pos.clone());
        assert!(warner.pos().is_some());
        assert!(!warner.has_lint(LintCategory::CAST));
        warner.warn(LintCategory::CAST);
        assert!(warner.has_non_silent_lint(LintCategory::CAST));
        assert!(!warner.has_silent_lint(LintCategory::CAST));
        warner.silent_warn(LintCategory::RAW);
        assert!(warner.has_silent_lint(LintCategory::RAW));
        assert!(warner.has_lint(LintCategory::RAW));
        assert!(!warner.has_lint(LintCategory::DEPRECATION));
        warner.warned.set(true);
        warner.clear();
        assert!(!warner.has_lint(LintCategory::CAST));
        assert!(!warner.has_lint(LintCategory::RAW));
        assert!(!warner.warned.get());
    }

    #[test]
    fn default_warner_has_no_position() {
        let warner = Warner::default();
        assert!(warner.pos().is_none());
        assert!(!warner.warned.get());
        assert!(!warner.has_lint(LintCategory::CAST));
    }
}
