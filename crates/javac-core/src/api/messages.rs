// Origin: com/sun/tools/javac/api/Messages.java @ 982d53c (jdk-17.0.19-ga)

//! This interface defines the minimum requirements in order to provide support
//! for localized formatted strings.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: java.util.Locale -> crate::util::javac_messages::Locale
//! (minimal locale-tag adapter); Object... args -> &[String] because the
//! formatting layer renders diagnostic arguments before substitution (ADR 0004).
//! The Java add() declares MissingResourceException; in the Rust mapping bundle
//! resolution is lazy (as upstream's ResourceBundleHelper), so add() itself
//! cannot fail.

use crate::util::javac_messages::Locale;

/// This interface defines the minimum requirements in order to provide support
/// for localized formatted strings.
pub trait Messages {
    /// Add a new resource bundle to the list that is searched for localized messages.
    ///
    /// @param bundleName the name to identify the resource bundle of localized messages.
    fn add(&mut self, bundle_name: &str);

    /// Get a localized formatted string.
    ///
    /// @param locale locale in which the text is to be localized; None means
    ///        the current locale of the Messages implementation.
    /// @param key locale-independent message key
    /// @param args misc message arguments (already rendered to strings)
    /// @return a localized formatted string
    fn get_localized_string(&self, locale: Option<&Locale>, key: &str, args: &[String]) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct RecordingMessages {
        added: Vec<String>,
        formatted: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Messages for RecordingMessages {
        fn add(&mut self, bundle_name: &str) {
            self.added.push(bundle_name.to_string());
        }

        fn get_localized_string(
            &self,
            _locale: Option<&Locale>,
            key: &str,
            args: &[String],
        ) -> String {
            self.formatted
                .borrow_mut()
                .push((key.to_string(), args.to_vec()));
            format!("{key}:{}", args.join(","))
        }
    }

    #[test]
    fn external_implementation_is_usable() {
        let mut messages = RecordingMessages {
            added: Vec::new(),
            formatted: RefCell::new(Vec::new()),
        };
        messages.add("com.sun.tools.javac.resources.compiler");
        assert_eq!(messages.added, vec!["com.sun.tools.javac.resources.compiler"]);
        let rendered = messages.get_localized_string(
            Some(&Locale::new("en")),
            "compiler.err.example",
            &["a".to_string(), "b".to_string()],
        );
        assert_eq!(rendered, "compiler.err.example:a,b");
        assert_eq!(messages.formatted.borrow().len(), 1);
    }

    #[test]
    fn dyn_messages_is_object_safe() {
        let messages = RecordingMessages {
            added: Vec::new(),
            formatted: RefCell::new(Vec::new()),
        };
        let messages: &dyn Messages = &messages;
        assert_eq!(
            messages.get_localized_string(None, "k", &[]),
            "k:"
        );
    }
}
