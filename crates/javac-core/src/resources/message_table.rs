//! A message table parsed lazily from embedded .properties text.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::util::properties::parse_properties;

/// A read-only message table: resource key -> localized template.
///
/// The raw .properties text is embedded at compile time (include_str!);
/// parsing happens once on first lookup and is cached in the OnceLock.
pub struct MessageTable {
    text: &'static str,
    entries: OnceLock<HashMap<String, String>>,
}

impl MessageTable {
    /// Builds a table from embedded properties text.
    pub const fn new(text: &'static str) -> MessageTable {
        MessageTable {
            text,
            entries: OnceLock::new(),
        }
    }

    fn ensure(&self) -> &HashMap<String, String> {
        self.entries
            .get_or_init(|| parse_properties(self.text).into_iter().collect())
    }

    /// Looks up a message template.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.ensure().get(key).map(String::as_str)
    }

    /// Number of keys in the table.
    pub fn len(&self) -> usize {
        self.ensure().len()
    }

    /// Whether the table has no keys.
    pub fn is_empty(&self) -> bool {
        self.ensure().is_empty()
    }

    /// Iterates the table entries (order is unspecified).
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.ensure().iter().map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

impl std::fmt::Debug for MessageTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageTable")
            .field("bytes", &self.text.len())
            .finish()
    }
}