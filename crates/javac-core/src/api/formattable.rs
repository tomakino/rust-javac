// Origin: com/sun/tools/javac/api/Formattable.java @ 982d53c (jdk-17.0.19-ga)

//! This interface must be implemented by any javac class that has non-trivial
//! formatting needs (e.g. where toString() does not apply because of localization).
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: java.util.Locale -> util::javac_messages::Locale; Messages ->
//! &dyn Messages; the Java overloads toString(Locale, Messages) / toString()
//! map to to_string_with_locale() / the Rust Display implementation
//! (PORTING_RULES section 3, Java overloads get distinct Rust names).

use std::fmt;

use crate::api::messages::Messages;
use crate::util::javac_messages::Locale;

/// This interface must be implemented by any javac class that has non-trivial
/// formatting needs (e.g. where toString() does not apply because of localization).
pub trait Formattable {
    /// Used to obtain a localized String representing the object accordingly
    /// to a given locale.
    ///
    /// @param locale locale in which the object's representation is to be rendered
    /// @param messages messages object used for localization
    /// @return a locale-dependent string representing the object
    fn to_string_with_locale(&self, locale: &Locale, messages: &dyn Messages) -> String;

    /// Retrieve a pretty name of this object's kind.
    ///
    /// @return a string representing the object's kind
    fn get_kind(&self) -> String;
}

/// Java: Formattable.LocalizedString
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedString {
    /// Java: String key (package-private upstream; public Rust field).
    pub key: String,
}

impl LocalizedString {
    /// Java: LocalizedString(String key)
    pub fn new(key: impl Into<String>) -> LocalizedString {
        LocalizedString { key: key.into() }
    }
}

impl Formattable for LocalizedString {
    fn to_string_with_locale(&self, locale: &Locale, messages: &dyn Messages) -> String {
        messages.get_localized_string(Some(locale), &self.key, &[])
    }

    fn get_kind(&self) -> String {
        "LocalizedString".to_string()
    }
}

impl fmt::Display for LocalizedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticMessages;

    impl Messages for StaticMessages {
        fn add(&mut self, _bundle_name: &str) {}

        fn get_localized_string(
            &self,
            locale: Option<&Locale>,
            key: &str,
            args: &[String],
        ) -> String {
            let tag = locale.map_or("en", Locale::get_tag);
            format!("{tag}:{key}:{}", args.len())
        }
    }

    #[test]
    fn localized_string_uses_messages() {
        let value = LocalizedString::new("compiler.misc.example");
        assert_eq!(value.get_kind(), "LocalizedString");
        assert_eq!(
            value.to_string_with_locale(&Locale::new("zh_CN"), &StaticMessages),
            "zh_CN:compiler.misc.example:0"
        );
    }

    #[test]
    fn display_returns_the_key() {
        let value = LocalizedString::new("compiler.misc.example");
        assert_eq!(value.to_string(), "compiler.misc.example");
    }

    #[test]
    fn dyn_formattable_is_object_safe() {
        let value = LocalizedString::new("k");
        let formattable: &dyn Formattable = &value;
        assert_eq!(formattable.get_kind(), "LocalizedString");
        assert_eq!(
            formattable.to_string_with_locale(&Locale::new("en"), &StaticMessages),
            "en:k:0"
        );
    }
}
