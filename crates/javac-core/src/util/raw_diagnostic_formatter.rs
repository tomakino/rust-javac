// Origin: com/sun/tools/javac/util/RawDiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga)

//! A raw formatter for diagnostic messages.
//! The raw formatter will format a diagnostic according to one of two format
//! patterns, depending on whether or not the source name and position are set.
//! This formatter provides a standardized, localize-independent implementation
//! of a diagnostic formatter; as such, this formatter is best suited for
//! testing purposes.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - the Java subclass (RawDiagnosticFormatter extends
//!   AbstractDiagnosticFormatter) maps to implementing the
//!   AbstractDiagnosticFormatter trait;
//! - the upstream constructor takes Options (SimpleConfiguration(Options,
//!   parts)); Options are not ported yet, so only the option-less construction
//!   is provided (TODO(Options));
//! - NOT PORTED IN THIS STEP (dependencies are not ported yet; no substitute
//!   logic):
//!   * the source-name branches need file/PathFileObject (getShortName /
//!     getSimpleName), so the raw format of a positioned diagnostic (or a class
//!     file diagnostic) panics with a TODO message; the position-less raw
//!     format (dash + code + arguments) is fully ported;
//!   * RawDiagnosticPosHelper (positions of JCExpression arguments) needs the
//!     tree arena context: JCExpression arguments are TreeId values in the port
//!     and a formatter has no CompilerContext yet (TODO);
//!   * the Formattable argument branch uses Java Object.toString() (no Rust
//!     equivalent for an unknown Formattable object) - the Abstract
//!     formatArgument branch is used instead (registered);
//!   * the Source.DEFAULT normalization branch needs code/Source (not ported);
//! - the upstream catch (Exception) -> null in formatDiagnostic is dropped: in
//!   Rust the exceptional paths panic (registered section 6/9).

use std::cell::Cell;
use std::rc::Rc;

use javac_tools::tools::java_file_object::Kind;

use crate::api::diagnostic_formatter::{
    Configuration, DiagnosticFormatter, DiagnosticPart, MultilineLimit, PositionKind,
};
use crate::tree::jc_tree::Tag;
use crate::util::abstract_diagnostic_formatter::{
    AbstractDiagnosticFormatter, SimpleConfiguration, format_argument_default,
};
use crate::util::javac_messages::{JavacMessages, Locale};
use crate::util::options::Options;
use crate::util::jc_diagnostic::{DiagnosticArg, JCDiagnostic};
use crate::util::position::NOPOS;

/// A raw formatter for diagnostic messages.
///
/// Java: public final class RawDiagnosticFormatter extends
/// AbstractDiagnosticFormatter
pub struct RawDiagnosticFormatter {
    config: SimpleConfiguration,
    depth: Cell<i32>,
}

impl RawDiagnosticFormatter {
    /// Create a formatter based on the supplied options.
    ///
    /// Java: public RawDiagnosticFormatter(Options options)
    pub fn new_with_options(options: &Options) -> RawDiagnosticFormatter {
        RawDiagnosticFormatter {
            config: SimpleConfiguration::new_with_options(
                options,
                vec![
                    DiagnosticPart::Summary,
                    DiagnosticPart::Details,
                    DiagnosticPart::Subdiagnostics,
                ],
            ),
            depth: Cell::new(0),
        }
    }

    /// Create a formatter with the default configuration (the Java class has no
    /// no-argument constructor; kept for the formatter unit tests).
    pub fn new() -> RawDiagnosticFormatter {
        RawDiagnosticFormatter {
            config: SimpleConfiguration::new(vec![
                DiagnosticPart::Summary,
                DiagnosticPart::Details,
                DiagnosticPart::Subdiagnostics,
            ]),
            depth: Cell::new(0),
        }
    }

    /// Java: public String formatDiagnostic(JCDiagnostic d, Locale l)
    fn format_diagnostic(&self, d: &JCDiagnostic) -> String {
        let mut buf = String::new();
        if d.get_position() != NOPOS as i64 {
            // Java: buf.append(formatSource(d, false, null)).
            buf.push_str(&AbstractDiagnosticFormatter::format_source_impl(self, d, false, None));
        } else if let Some(source) = d.get_source()
            && source.get_kind() == Kind::CLASS
        {
            // Java: buf.append(formatSource(d, false, null)).
            buf.push_str(&AbstractDiagnosticFormatter::format_source_impl(self, d, false, None));
        } else {
            buf.push('-');
        }
        buf.push(' ');
        buf.push_str(&self.format_message_impl(d, None));
        if self.display_source_impl(d) {
            buf.push('\n');
            buf.push_str(&self.format_source_line(d, 0));
        }
        buf
    }

    /// Java: public String formatMessage(JCDiagnostic d, Locale l)
    fn format_message_raw(&self, d: &JCDiagnostic) -> String {
        let mut buf = String::new();
        let args = self.format_arguments(d, None);
        buf.push_str(&self.localize(None, &d.get_code_string(), &args));
        if d.is_multiline()
            && self
                .config_visible()
                .contains(&DiagnosticPart::Subdiagnostics)
        {
            let sub_diags = self.format_subdiagnostics(d, None);
            if !sub_diags.is_empty() {
                let mut result = String::from(",{");
                let mut sep = "";
                // The upstream loop calls formatSubdiagnostics(d, null) again in
                // the for statement (1:1 copy of the upstream double call).
                for sub in self.format_subdiagnostics(d, None) {
                    result.push_str(sep);
                    result.push('(');
                    result.push_str(&sub);
                    result.push(')');
                    sep = ",";
                }
                result.push('}');
                buf.push_str(&result);
            }
        }
        buf
    }

    /// Java: @Override protected String localize(Locale l, String key,
    /// Object... args)
    fn raw_localize(&self, key: &str, args: &[String]) -> String {
        let mut buf = String::new();
        buf.push_str(key);
        let mut sep = ": ";
        for arg in args {
            buf.push_str(sep);
            buf.push_str(arg);
            sep = ", ";
        }
        buf
    }
}

impl Default for RawDiagnosticFormatter {
    fn default() -> RawDiagnosticFormatter {
        RawDiagnosticFormatter::new()
    }
}

impl AbstractDiagnosticFormatter for RawDiagnosticFormatter {
    fn messages(&self) -> Option<&Rc<JavacMessages>> {
        // Java: RawDiagnosticFormatter passes null to the super constructor.
        None
    }

    fn depth(&self) -> i32 {
        self.depth.get()
    }

    fn set_depth(&self, depth: i32) {
        self.depth.set(depth);
    }

    fn config_visible(&self) -> Vec<DiagnosticPart> {
        self.config.get_visible()
    }

    fn config_multiline_limit(&self, limit: MultilineLimit) -> i32 {
        self.config.get_multiline_limit(limit)
    }

    fn config_caret_enabled(&self) -> bool {
        self.config.is_caret_enabled()
    }

    fn format_diagnostic_impl(&self, d: &JCDiagnostic, _locale: Option<&Locale>) -> String {
        self.format_diagnostic(d)
    }

    fn format_message_impl(&self, d: &JCDiagnostic, _locale: Option<&Locale>) -> String {
        self.format_message_raw(d)
    }

    // The diag parameter is required by the upstream override signature: the
    // upstream body also reads diag.getCode() in the Source.DEFAULT
    // normalization branch, which waits for code/Source (see the module doc).
    #[allow(clippy::only_used_in_recursion)]
    fn format_argument(
        &self,
        diag: &JCDiagnostic,
        arg: &DiagnosticArg,
        _locale: Option<&Locale>,
    ) -> String {
        if let Some(tag) = arg.downcast_ref::<Tag>() {
            let name = format!("{tag:?}").to_lowercase();
            return format!("compiler.misc.tree.tag.{name}");
        }
        let s = format_argument_default(self, diag, arg, None);
        if arg.downcast_ref::<JCDiagnostic>().is_some() {
            format!("({s})")
        } else {
            s
        }
    }

    fn localize(&self, _locale: Option<&Locale>, key: &str, args: &[String]) -> String {
        self.raw_localize(key, args)
    }

    fn is_raw_impl(&self) -> bool {
        true
    }
}

impl DiagnosticFormatter<JCDiagnostic> for RawDiagnosticFormatter {
    fn display_source(&self, diag: &JCDiagnostic) -> bool {
        self.display_source_impl(diag)
    }

    fn format(&self, diag: &JCDiagnostic, locale: Option<&Locale>) -> String {
        self.format_impl(diag, locale)
    }

    fn format_message(&self, diag: &JCDiagnostic, locale: &Locale) -> String {
        self.format_message_impl(diag, Some(locale))
    }

    fn format_kind(&self, diag: &JCDiagnostic, locale: &Locale) -> String {
        self.format_kind_impl(diag, Some(locale))
    }

    fn format_source(&self, diag: &JCDiagnostic, fullname: bool, locale: &Locale) -> String {
        self.format_source_impl(diag, fullname, Some(locale))
    }

    fn format_position(
        &self,
        diag: &JCDiagnostic,
        pk: PositionKind,
        locale: &Locale,
    ) -> String {
        self.format_position_impl(diag, pk, Some(locale))
    }

    fn get_configuration(&mut self) -> &mut dyn Configuration {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::jc_diagnostic::Factory;
    use crate::util::javac_messages::JavacMessages;

    fn factory() -> Factory {
        Factory::new(
            Rc::new(JavacMessages::new(
                crate::util::javac_messages::DEFAULT_BUNDLE_NAME,
            )),
            "compiler",
        )
    }

    #[test]
    fn raw_format_without_position() {
        let factory = factory();
        let diagnostic = factory.create(
            crate::util::jc_diagnostic::DiagnosticType::WARNING,
            None,
            None,
            "test",
            vec![Rc::new(crate::util::java_string::JavaString::from("X"))],
        );
        assert_eq!(
            formatter().format(&diagnostic, None),
            "- compiler.warn.test: X"
        );
        assert!(formatter().is_raw_impl());
    }

    #[test]
    fn raw_message_and_localize() {
        let factory = factory();
        let diagnostic = factory.create(
            crate::util::jc_diagnostic::DiagnosticType::ERROR,
            None,
            None,
            "test",
            Vec::new(),
        );
        assert_eq!(
            formatter().format_message(&diagnostic, &Locale::default_locale()),
            "compiler.err.test"
        );
    }

    #[test]
    fn positioned_raw_format_is_not_ported_yet() {
        let factory = factory();
        let diagnostic = factory.create(
            crate::util::jc_diagnostic::DiagnosticType::ERROR,
            None,
            None,
            "test",
            Vec::new(),
        );
        // The position-less path works (see raw_format_without_position); the
        // positioned path needs file/PathFileObject.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let fmt = formatter();
            fmt.format(&diagnostic, None)
        }));
        let _ = result;
    }

    fn formatter() -> RawDiagnosticFormatter {
        RawDiagnosticFormatter::new()
    }
}
