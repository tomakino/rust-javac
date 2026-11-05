//! A delegated diagnostic formatter delegates all formatting actions to an
//! underlying formatter (aka the delegated formatter).
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//!
//! - Java generics <D extends Diagnostic<?>, F extends DiagnosticFormatter<D>>
//!   map to a single type parameter D and an Rc<RefCell<dyn
//!   DiagnosticFormatter<D>>> delegate: the upstream class keeps a reference to
//!   the delegated formatter and the configuration forwards to the formatter's
//!   own configuration; in Rust both need a shared, interior-mutable formatter
//!   (configuration setters take &mut self), so the delegate is an
//!   Rc<RefCell<...>> (registered section 9);
//! - ForwardingConfiguration.getDelegatedConfiguration cannot return a borrowed
//!   trait reference from behind RefCell; it is provided as with_configuration
//!   (a closure-based access) and the Configuration methods forward directly.

use std::cell::RefCell;
use std::rc::Rc;

use crate::api::diagnostic_formatter::{
    Configuration, DiagnosticFormatter, DiagnosticPart, MultilineLimit, PositionKind,
};
use crate::util::javac_messages::Locale;

/// A delegated diagnostic formatter.
///
/// Java: public class ForwardingDiagnosticFormatter<D extends Diagnostic<?>,
/// F extends DiagnosticFormatter<D>> implements DiagnosticFormatter<D>
pub struct ForwardingDiagnosticFormatter<D> {
    /// Java: protected F formatter
    formatter: Rc<RefCell<dyn DiagnosticFormatter<D>>>,
    /// Java: protected ForwardingConfiguration configuration
    configuration: ForwardingConfiguration<D>,
}

impl<D> ForwardingDiagnosticFormatter<D> {
    /// Java: public ForwardingDiagnosticFormatter(F formatter)
    pub fn new(
        formatter: Rc<RefCell<dyn DiagnosticFormatter<D>>>,
    ) -> ForwardingDiagnosticFormatter<D> {
        let configuration = ForwardingConfiguration {
            formatter: Rc::clone(&formatter),
        };
        ForwardingDiagnosticFormatter {
            formatter,
            configuration,
        }
    }

    /// Returns the underlying delegated formatter.
    ///
    /// Java: public F getDelegatedFormatter()
    pub fn get_delegated_formatter(&self) -> Rc<RefCell<dyn DiagnosticFormatter<D>>> {
        Rc::clone(&self.formatter)
    }

    /// Java: public Configuration getConfiguration()
    pub fn get_configuration(&self) -> &ForwardingConfiguration<D> {
        &self.configuration
    }
}

impl<D> DiagnosticFormatter<D> for ForwardingDiagnosticFormatter<D> {
    fn display_source(&self, diag: &D) -> bool {
        self.formatter.borrow().display_source(diag)
    }

    fn format(&self, diag: &D, locale: Option<&Locale>) -> String {
        self.formatter.borrow().format(diag, locale)
    }

    fn format_message(&self, diag: &D, locale: &Locale) -> String {
        self.formatter.borrow().format_message(diag, locale)
    }

    fn format_kind(&self, diag: &D, locale: &Locale) -> String {
        self.formatter.borrow().format_kind(diag, locale)
    }

    fn format_source(&self, diag: &D, fullname: bool, locale: &Locale) -> String {
        self.formatter
            .borrow()
            .format_source(diag, fullname, locale)
    }

    fn format_position(&self, diag: &D, pk: PositionKind, locale: &Locale) -> String {
        self.formatter.borrow().format_position(diag, pk, locale)
    }

    fn get_configuration(&mut self) -> &mut dyn Configuration {
        &mut self.configuration
    }
}

/// A delegated formatter configuration delegates all configurations settings to
/// an underlying configuration object (aka the delegated configuration).
///
/// Java: public static class ForwardingConfiguration implements
/// DiagnosticFormatter.Configuration
pub struct ForwardingConfiguration<D> {
    /// The configuration object to which the forwarding configuration delegates
    /// some settings.
    formatter: Rc<RefCell<dyn DiagnosticFormatter<D>>>,
}

impl<D> ForwardingConfiguration<D> {
    /// Access the underlying delegated configuration.
    ///
    /// Java: public Configuration getDelegatedConfiguration()
    ///
    /// the upstream returns a borrowed reference; the RefCell
    /// delegate cannot hand one out, so the closure form is provided.
    pub fn with_configuration<R>(&self, f: impl FnOnce(&mut dyn Configuration) -> R) -> R {
        f(self.formatter.borrow_mut().get_configuration())
    }
}

impl<D> Configuration for ForwardingConfiguration<D> {
    fn set_visible(&mut self, visible_parts: &[DiagnosticPart]) {
        self.formatter
            .borrow_mut()
            .get_configuration()
            .set_visible(visible_parts);
    }

    fn get_visible(&self) -> Vec<DiagnosticPart> {
        self.formatter
            .borrow_mut()
            .get_configuration()
            .get_visible()
    }

    fn set_multiline_limit(&mut self, limit: MultilineLimit, value: i32) {
        self.formatter
            .borrow_mut()
            .get_configuration()
            .set_multiline_limit(limit, value);
    }

    fn get_multiline_limit(&self, limit: MultilineLimit) -> i32 {
        self.formatter
            .borrow_mut()
            .get_configuration()
            .get_multiline_limit(limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::abstract_diagnostic_formatter::SimpleConfiguration;

    #[derive(Default)]
    struct DummyFormatter {
        config: Option<SimpleConfiguration>,
    }

    impl DummyFormatter {
        fn new() -> DummyFormatter {
            DummyFormatter {
                config: Some(SimpleConfiguration::new(vec![DiagnosticPart::Summary])),
            }
        }

        fn config(&mut self) -> &mut SimpleConfiguration {
            self.config.as_mut().expect("configuration")
        }
    }

    impl DiagnosticFormatter<u32> for DummyFormatter {
        fn display_source(&self, _diag: &u32) -> bool {
            true
        }

        fn format(&self, diag: &u32, _locale: Option<&Locale>) -> String {
            format!("format:{diag}")
        }

        fn format_message(&self, diag: &u32, _locale: &Locale) -> String {
            format!("message:{diag}")
        }

        fn format_kind(&self, _diag: &u32, _locale: &Locale) -> String {
            "kind".to_string()
        }

        fn format_source(&self, _diag: &u32, fullname: bool, _locale: &Locale) -> String {
            if fullname { "full" } else { "short" }.to_string()
        }

        fn format_position(&self, _diag: &u32, pk: PositionKind, _locale: &Locale) -> String {
            format!("{pk:?}")
        }

        fn get_configuration(&mut self) -> &mut dyn Configuration {
            self.config()
        }
    }

    fn delegate() -> Rc<RefCell<dyn DiagnosticFormatter<u32>>> {
        Rc::new(RefCell::new(DummyFormatter::new()))
    }

    #[test]
    fn forwards_formatting_calls() {
        let delegated = delegate();
        let forwarding = ForwardingDiagnosticFormatter::new(Rc::clone(&delegated));
        assert!(forwarding.display_source(&7));
        assert_eq!(forwarding.format(&7, None), "format:7");
        assert_eq!(
            forwarding.format_message(&7, &Locale::default_locale()),
            "message:7"
        );
        assert_eq!(
            forwarding.format_kind(&7, &Locale::default_locale()),
            "kind"
        );
        assert_eq!(
            forwarding.format_source(&7, true, &Locale::default_locale()),
            "full"
        );
        assert_eq!(
            forwarding.format_position(&7, PositionKind::Line, &Locale::default_locale()),
            "Line"
        );
        assert!(Rc::ptr_eq(
            &forwarding.get_delegated_formatter(),
            &delegated
        ));
        let _ = forwarding.get_configuration();
    }

    #[test]
    fn forwards_configuration() {
        let delegated = delegate();
        let forwarding = ForwardingDiagnosticFormatter::new(Rc::clone(&delegated));
        {
            let configuration = forwarding.get_configuration();
            assert_eq!(configuration.get_visible(), vec![DiagnosticPart::Summary]);
            let delegated_configuration = delegated.clone();
            configuration.with_configuration(|cfg| {
                cfg.set_visible(&[DiagnosticPart::Details]);
            });
            let _ = delegated_configuration;
        }
        assert_eq!(
            delegated.borrow_mut().get_configuration().get_visible(),
            vec![DiagnosticPart::Details]
        );
        {
            let configuration = forwarding.get_configuration();
            assert_eq!(configuration.get_visible(), vec![DiagnosticPart::Details]);
        }
        {
            let mut configuration =
                ForwardingConfiguration::<u32>::new_for_test(Rc::clone(&delegated));
            configuration.set_multiline_limit(MultilineLimit::Depth, 3);
            assert_eq!(configuration.get_multiline_limit(MultilineLimit::Depth), 3);
        }
        let _ = forwarding.get_configuration();
    }

    impl<D> ForwardingConfiguration<D> {
        fn new_for_test(
            formatter: Rc<RefCell<dyn DiagnosticFormatter<D>>>,
        ) -> ForwardingConfiguration<D> {
            ForwardingConfiguration { formatter }
        }
    }
}
