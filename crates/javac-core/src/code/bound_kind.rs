//! Kind of a wildcard bound.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::fmt;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoundKind {
    EXTENDS,
    SUPER,
    UNBOUND,
}

impl BoundKind {
    /// Java: private final String name;
    pub const fn name(self) -> &'static str {
        match self {
            BoundKind::EXTENDS => "? extends ",
            BoundKind::SUPER => "? super ",
            BoundKind::UNBOUND => "?",
        }
    }
}

// Java: public String toString() { return name; }
impl fmt::Display for BoundKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_java_tostring() {
        assert_eq!(BoundKind::EXTENDS.to_string(), "? extends ");
        assert_eq!(BoundKind::SUPER.to_string(), "? super ");
        assert_eq!(BoundKind::UNBOUND.to_string(), "?");
    }
}
