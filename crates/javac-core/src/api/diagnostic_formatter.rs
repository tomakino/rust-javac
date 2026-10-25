// Origin: com/sun/tools/javac/api/DiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga)

//! Provides simple functionalities for javac diagnostic formatting.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: java.util.Locale -> util::javac_messages::Locale (None models
//! the Java null locale in the format() entry point, like JavacMessages
//! getLocalizedString(Locale, ...)); the generic bound D extends Diagnostic<?>
//! is carried by the implementations (JCDiagnostic) instead of the trait so
//! that this API layer has no dependency on diagnostic internals; the nested
//! Java enums/interfaces map to Rust enums/traits.

use crate::util::javac_messages::Locale;

/// Provides simple functionalities for javac diagnostic formatting.
///
/// @param D type of diagnostic handled by this formatter
pub trait DiagnosticFormatter<D> {
    /// Whether the source code output for this diagnostic is to be displayed.
    fn display_source(&self, diag: &D) -> bool;

    /// Format the contents of a diagnostics.
    fn format(&self, diag: &D, locale: Option<&Locale>) -> String;

    /// Controls the way in which a diagnostic message is displayed.
    fn format_message(&self, diag: &D, locale: &Locale) -> String;

    /// Controls the way in which a diagnostic kind is displayed.
    fn format_kind(&self, diag: &D, locale: &Locale) -> String;

    /// Controls the way in which a diagnostic source is displayed.
    fn format_source(&self, diag: &D, fullname: bool, locale: &Locale) -> String;

    /// Controls the way in which a diagnostic position is displayed.
    fn format_position(&self, diag: &D, pk: PositionKind, locale: &Locale) -> String;

    /// Get the configuration of this formatter (verbosity options).
    fn get_configuration(&mut self) -> &mut dyn Configuration;
}

/// This enum defines a set of constants for all the kinds of position that a
/// diagnostic can be asked for. All positions are intended to be relative to a
/// given diagnostic source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionKind {
    /// Start position
    Start,
    /// End position
    End,
    /// Line number
    Line,
    /// Column number
    Column,
    /// Offset position
    Offset,
}

/// This interface provides functionalities for tuning the output of a
/// diagnostic formatter in multiple ways.
pub trait Configuration {
    /// Configure the set of diagnostic parts that should be displayed by the formatter.
    fn set_visible(&mut self, visible_parts: &[DiagnosticPart]);

    /// Retrieve the set of diagnostic parts that should be displayed by the formatter.
    fn get_visible(&self) -> Vec<DiagnosticPart>;

    /// Set a limit for multiline diagnostics.
    fn set_multiline_limit(&mut self, limit: MultilineLimit, value: i32);

    /// Get a multiline diagnostic limit (-1 if no limit is set).
    fn get_multiline_limit(&self, limit: MultilineLimit) -> i32;
}

/// A given diagnostic message can be divided into sub-parts each of which
/// might/might not be displayed by the formatter, according to the current
/// configuration settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPart {
    /// Short description of the diagnostic - usually one line long.
    Summary,
    /// Longer description that provides additional details w.r.t. the ones in the
    /// diagnostic's description.
    Details,
    /// Source line the diagnostic refers to (if applicable).
    Source,
    /// Subdiagnostics attached to a given multiline diagnostic.
    Subdiagnostics,
    /// JLS paragraph this diagnostic might refer to (if applicable).
    Jls,
}

/// A multiline limit controls the verbosity of multiline diagnostics either by
/// setting a maximum depth of nested multidiagnostics, or by limiting the
/// amount of subdiagnostics attached to a given diagnostic (or both).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultilineLimit {
    /// Controls the maximum depth of nested multiline diagnostics.
    Depth,
    /// Controls the maximum amount of subdiagnostics that are part of a given
    /// multiline diagnostic.
    Length,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyDiagnostic;

    #[derive(Default)]
    struct DummyConfiguration {
        visible: Vec<DiagnosticPart>,
        depth: i32,
        length: i32,
    }

    impl Configuration for DummyConfiguration {
        fn set_visible(&mut self, visible_parts: &[DiagnosticPart]) {
            self.visible = visible_parts.to_vec();
        }

        fn get_visible(&self) -> Vec<DiagnosticPart> {
            self.visible.clone()
        }

        fn set_multiline_limit(&mut self, limit: MultilineLimit, value: i32) {
            match limit {
                MultilineLimit::Depth => self.depth = value,
                MultilineLimit::Length => self.length = value,
            }
        }

        fn get_multiline_limit(&self, limit: MultilineLimit) -> i32 {
            match limit {
                MultilineLimit::Depth => self.depth,
                MultilineLimit::Length => self.length,
            }
        }
    }

    struct DummyFormatter {
        configuration: DummyConfiguration,
    }

    impl DiagnosticFormatter<DummyDiagnostic> for DummyFormatter {
        fn display_source(&self, _diag: &DummyDiagnostic) -> bool {
            true
        }

        fn format(&self, _diag: &DummyDiagnostic, locale: Option<&Locale>) -> String {
            let locale = locale.map_or("en", Locale::get_tag);
            format!("format:{locale}")
        }

        fn format_message(&self, _diag: &DummyDiagnostic, _locale: &Locale) -> String {
            "message".to_string()
        }

        fn format_kind(&self, _diag: &DummyDiagnostic, _locale: &Locale) -> String {
            "error".to_string()
        }

        fn format_source(&self, _diag: &DummyDiagnostic, fullname: bool, _locale: &Locale) -> String {
            if fullname { "full" } else { "short" }.to_string()
        }

        fn format_position(&self, _diag: &DummyDiagnostic, pk: PositionKind, _locale: &Locale) -> String {
            format!("{pk:?}")
        }

        fn get_configuration(&mut self) -> &mut dyn Configuration {
            &mut self.configuration
        }
    }

    #[test]
    fn external_implementation_is_usable() {
        let mut formatter = DummyFormatter {
            configuration: DummyConfiguration::default(),
        };
        let diag = DummyDiagnostic;
        assert!(formatter.display_source(&diag));
        assert_eq!(formatter.format(&diag, None), "format:en");
        assert_eq!(
            formatter.format(&diag, Some(&Locale::new("zh_CN"))),
            "format:zh_CN"
        );
        assert_eq!(formatter.format_position(&diag, PositionKind::Line, &Locale::new("en")), "Line");
        let configuration = formatter.get_configuration();
        configuration.set_visible(&[DiagnosticPart::Summary, DiagnosticPart::Source]);
        configuration.set_multiline_limit(MultilineLimit::Depth, 3);
        assert_eq!(
            configuration.get_visible(),
            vec![DiagnosticPart::Summary, DiagnosticPart::Source]
        );
        assert_eq!(configuration.get_multiline_limit(MultilineLimit::Depth), 3);
        assert_eq!(configuration.get_multiline_limit(MultilineLimit::Length), 0);
    }

    #[test]
    fn dyn_formatter_is_object_safe() {
        let mut formatter = DummyFormatter {
            configuration: DummyConfiguration::default(),
        };
        let diag = DummyDiagnostic;
        {
            let formatter: &mut dyn DiagnosticFormatter<DummyDiagnostic> = &mut formatter;
            assert_eq!(formatter.format_kind(&diag, &Locale::new("en")), "error");
            assert_eq!(formatter.format_source(&diag, true, &Locale::new("en")), "full");
            let _ = formatter.get_configuration();
        }
        assert_eq!(
            formatter.format_message(&diag, &Locale::new("en")),
            "message"
        );
    }
}
