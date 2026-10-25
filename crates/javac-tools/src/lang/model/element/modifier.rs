// Origin: javax/lang/model/element/Modifier.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a modifier on a program element such
//! as a class, method, or field.

use javac_strings::JavaString;

/// Represents a modifier on a program element such
/// as a class, method, or field.
///
/// <p>Not all modifiers are applicable to all kinds of elements.
/// When two or more modifiers appear in the source code of an element
/// then it is customary, though not required, that they appear in the same
/// order as the constants listed in the detail section below.
///
/// <p>Note that it is possible additional modifiers will be added in
/// future versions of the platform.
///
/// @jls 8.1.1 Class Modifiers
/// @jls 8.3.1 Field Modifiers
/// @jls 8.4.3 Method Modifiers
/// @jls 8.8.3 Constructor Modifiers
/// @jls 9.1.1 Interface Modifiers
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// PORTING_RULES: Java enum constants keep their Java names
/// (UPPER_SNAKE_CASE).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    // Note java.lang.reflect.Modifier includes INTERFACE, but that's a VMism.

    /// The modifier {@code public}
    PUBLIC,
    /// The modifier {@code protected}
    PROTECTED,
    /// The modifier {@code private}
    PRIVATE,
    /// The modifier {@code abstract}
    ABSTRACT,
    /// The modifier {@code default}
    /// @since 1.8
    DEFAULT,
    /// The modifier {@code static}
    STATIC,

    /// The modifier {@code sealed}
    /// @since 17
    SEALED,

    /// The modifier {@code non-sealed}
    /// @since 17
    NON_SEALED,

    /// The modifier {@code final}
    FINAL,
    /// The modifier {@code transient}
    TRANSIENT,
    /// The modifier {@code volatile}
    VOLATILE,
    /// The modifier {@code synchronized}
    SYNCHRONIZED,
    /// The modifier {@code native}
    NATIVE,
    /// The modifier {@code strictfp}
    STRICTFP,
}

impl Modifier {
    /// Java: `enum#name()` (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Modifier::PUBLIC => "PUBLIC",
            Modifier::PROTECTED => "PROTECTED",
            Modifier::PRIVATE => "PRIVATE",
            Modifier::ABSTRACT => "ABSTRACT",
            Modifier::DEFAULT => "DEFAULT",
            Modifier::STATIC => "STATIC",
            Modifier::SEALED => "SEALED",
            Modifier::NON_SEALED => "NON_SEALED",
            Modifier::FINAL => "FINAL",
            Modifier::TRANSIENT => "TRANSIENT",
            Modifier::VOLATILE => "VOLATILE",
            Modifier::SYNCHRONIZED => "SYNCHRONIZED",
            Modifier::NATIVE => "NATIVE",
            Modifier::STRICTFP => "STRICTFP",
        })
    }

    /// Java: `enum#values()`.
    pub fn values() -> [Modifier; 14] {
        [
            Modifier::PUBLIC,
            Modifier::PROTECTED,
            Modifier::PRIVATE,
            Modifier::ABSTRACT,
            Modifier::DEFAULT,
            Modifier::STATIC,
            Modifier::SEALED,
            Modifier::NON_SEALED,
            Modifier::FINAL,
            Modifier::TRANSIENT,
            Modifier::VOLATILE,
            Modifier::SYNCHRONIZED,
            Modifier::NATIVE,
            Modifier::STRICTFP,
        ]
    }
}

impl std::fmt::Display for Modifier {
    /// Returns this modifier's name as defined in <cite>The
    /// Java Language Specification</cite>.
    /// The modifier name is the {@linkplain #name() name of the enum
    /// constant} in lowercase and with any underscores ("{@code _}")
    /// replaced with hyphens ("{@code -}").
    /// @return the modifier's name
    ///
    /// PORTING_RULES: upstream implements this as
    /// `name().toLowerCase(java.util.Locale.US)`, i.e. lowercase only, and
    /// overrides `NON_SEALED.toString()` to return "non-sealed" (the hyphen
    /// replacement is only stated in the Javadoc, not performed by the shared
    /// implementation).  Both branches are copied here verbatim.  The enum
    /// constant names are ASCII, so the ASCII lowercase below is exactly
    /// `String.toLowerCase(Locale.US)` for this input.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Java: NON_SEALED { public String toString() { return "non-sealed"; } }
        if *self == Modifier::NON_SEALED {
            return f.write_str("non-sealed");
        }
        // Java: return name().toLowerCase(java.util.Locale.US);
        f.write_str(&self.name().to_string_lossy().to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::Modifier;
    use javac_strings::JavaString;

    #[test]
    fn lowercases_enum_names() {
        assert_eq!(Modifier::PUBLIC.to_string(), "public");
        assert_eq!(Modifier::STRICTFP.to_string(), "strictfp");
        assert_eq!(Modifier::PUBLIC.name(), JavaString::from("PUBLIC"));
    }

    #[test]
    fn non_sealed_has_hyphen() {
        // Upstream overrides toString() for NON_SEALED with "non-sealed".
        assert_eq!(Modifier::NON_SEALED.to_string(), "non-sealed");
    }

    #[test]
    fn declaration_order() {
        assert_eq!(Modifier::values()[0], Modifier::PUBLIC);
        assert_eq!(Modifier::values()[7], Modifier::NON_SEALED);
        assert_eq!(Modifier::values().len(), 14);
    }
}
