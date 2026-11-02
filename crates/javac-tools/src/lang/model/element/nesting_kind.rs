//! The <i>nesting kind</i> of a type element.

use javac_strings::JavaString;

/// The <i>nesting kind</i> of a type element.
/// Type elements come in four varieties:
/// top-level, member, local, and anonymous.
/// <i>Nesting kind</i> is a non-standard term used here to denote this
/// classification.
///
/// <p>Note that it is possible additional nesting kinds will be added
/// in future versions of the platform.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// Java enum constants keep their Java names
/// (UPPER_SNAKE_CASE).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NestingKind {
    /// A top-level class or interface, not contained within another
    /// class or interface.
    TOP_LEVEL,

    /// A class or interface that is a named member of another class or
    /// interface.
    /// @jls 8.5 Member Class and Interface Declarations
    MEMBER,

    /// A named class or interface declared within a construct other
    /// than a class or interface.
    /// @jls 14.3 Local Class Declarations
    LOCAL,

    /// A class without a name.
    /// @jls 15.9.5 Anonymous Class Declarations
    ANONYMOUS,
}

impl NestingKind {
    /// Does this constant correspond to a nested type element?
    /// A <i>nested</i> type element is any that is not top-level.
    /// More specifically, an <i>inner</i> type element is any nested type element that
    /// is not {@linkplain Modifier#STATIC static}.
    /// @return whether or not the constant is nested
    /// @jls 14.3 Local Class Declarations
    pub fn is_nested(&self) -> bool {
        *self != NestingKind::TOP_LEVEL
    }

    /// Java: `enum#name()` (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            NestingKind::TOP_LEVEL => "TOP_LEVEL",
            NestingKind::MEMBER => "MEMBER",
            NestingKind::LOCAL => "LOCAL",
            NestingKind::ANONYMOUS => "ANONYMOUS",
        })
    }

    /// Java: `enum#values()`.
    pub fn values() -> [NestingKind; 4] {
        [
            NestingKind::TOP_LEVEL,
            NestingKind::MEMBER,
            NestingKind::LOCAL,
            NestingKind::ANONYMOUS,
        ]
    }
}

impl std::fmt::Display for NestingKind {
    /// Java: `Enum.toString()` = `name()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::NestingKind;
    use javac_strings::JavaString;

    #[test]
    fn only_top_level_is_not_nested() {
        assert!(!NestingKind::TOP_LEVEL.is_nested());
        assert!(NestingKind::MEMBER.is_nested());
        assert!(NestingKind::LOCAL.is_nested());
        assert!(NestingKind::ANONYMOUS.is_nested());
    }

    #[test]
    fn declaration_order_and_names() {
        assert_eq!(NestingKind::values()[0], NestingKind::TOP_LEVEL);
        assert_eq!(NestingKind::ANONYMOUS.name(), JavaString::from("ANONYMOUS"));
        assert_eq!(NestingKind::ANONYMOUS.to_string(), "ANONYMOUS");
    }
}
