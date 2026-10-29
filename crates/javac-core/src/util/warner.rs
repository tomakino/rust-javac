// Origin: com/sun/tools/javac/util/Warner.java @ 982d53c (jdk-17.0.19-ga)

//! An interface to support optional warnings, needed for support of unchecked
//! conversions and unchecked casts.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - the Java DiagnosticPosition object reference (nullable) maps to
//!   Option<Rc<dyn DiagnosticPosition>>;
//! - the Java EnumSet<LintCategory> fields map to Vec<LintCategory>: the only
//!   operations in the class are add / clear / contains, and LintCategory is
//!   Copy + Eq (registered in PORTING_RULES section 9);
//! - the protected warned field is a public Rust field (the Java subclasses -
//!   e.g. Warner's users in Attr/Check - read it; Rust has no protected).

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
    pub warned: bool,
    /// Java: private EnumSet<LintCategory> nonSilentLintSet
    non_silent_lint_set: Vec<LintCategory>,
    /// Java: private EnumSet<LintCategory> silentLintSet
    silent_lint_set: Vec<LintCategory>,
}

impl Warner {
    /// Java: public Warner(DiagnosticPosition pos)
    pub fn new(pos: Option<Rc<dyn DiagnosticPosition>>) -> Warner {
        Warner {
            pos,
            warned: false,
            non_silent_lint_set: Vec::new(),
            silent_lint_set: Vec::new(),
        }
    }

    /// Java: public DiagnosticPosition pos()
    pub fn pos(&self) -> Option<Rc<dyn DiagnosticPosition>> {
        self.pos.clone()
    }

    /// Java: public void warn(LintCategory lint)
    pub fn warn(&mut self, lint: LintCategory) {
        if !self.non_silent_lint_set.contains(&lint) {
            self.non_silent_lint_set.push(lint);
        }
    }

    /// Java: public void silentWarn(LintCategory lint)
    pub fn silent_warn(&mut self, lint: LintCategory) {
        if !self.silent_lint_set.contains(&lint) {
            self.silent_lint_set.push(lint);
        }
    }

    /// Java: public boolean hasSilentLint(LintCategory lint)
    pub fn has_silent_lint(&self, lint: LintCategory) -> bool {
        self.silent_lint_set.contains(&lint)
    }

    /// Java: public boolean hasNonSilentLint(LintCategory lint)
    pub fn has_non_silent_lint(&self, lint: LintCategory) -> bool {
        self.non_silent_lint_set.contains(&lint)
    }

    /// Java: public boolean hasLint(LintCategory lint)
    pub fn has_lint(&self, lint: LintCategory) -> bool {
        self.has_silent_lint(lint) || self.has_non_silent_lint(lint)
    }

    /// Java: public void clear()
    pub fn clear(&mut self) {
        self.non_silent_lint_set.clear();
        self.silent_lint_set.clear();
        self.warned = false;
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
        let mut warner = Warner::new(pos.clone());
        assert!(warner.pos().is_some());
        assert!(!warner.has_lint(LintCategory::CAST));
        warner.warn(LintCategory::CAST);
        assert!(warner.has_non_silent_lint(LintCategory::CAST));
        assert!(!warner.has_silent_lint(LintCategory::CAST));
        warner.silent_warn(LintCategory::RAW);
        assert!(warner.has_silent_lint(LintCategory::RAW));
        assert!(warner.has_lint(LintCategory::RAW));
        assert!(!warner.has_lint(LintCategory::DEPRECATION));
        warner.warned = true;
        warner.clear();
        assert!(!warner.has_lint(LintCategory::CAST));
        assert!(!warner.has_lint(LintCategory::RAW));
        assert!(!warner.warned);
    }

    #[test]
    fn default_warner_has_no_position() {
        let warner = Warner::default();
        assert!(warner.pos().is_none());
        assert!(!warner.warned);
        assert!(!warner.has_lint(LintCategory::CAST));
    }
}
