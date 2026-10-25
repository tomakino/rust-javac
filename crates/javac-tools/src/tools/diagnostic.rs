// Origin: javax/tools/Diagnostic.java @ 982d53c (jdk-17.0.19-ga)

//! Interface for diagnostics from tools.

use javac_strings::JavaString;

/// Interface for diagnostics from tools.  A diagnostic usually reports
/// a problem at a specific position in a source file.  However, not
/// all diagnostics are associated with a position or a file.
///
/// <p>A position is a zero-based character offset from the beginning of
/// a file.  Negative values (except {@link #NOPOS}) are not valid
/// positions.
///
/// <p>Line and column numbers begin at 1.  Negative values (except
/// {@link #NOPOS}) and 0 are not valid line or column numbers.
///
/// @param <S> the type of source object used by this diagnostic
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
///
/// PORTING_RULES — signature mapping:
/// * Locale -> Option of &javac_strings::JavaString (locale tag; null = no
///   locale).
/// * Java String code/message -> Option of JavaString / JavaString.
/// * The S type parameter is the Java "source object" type.  The ported
///   compiler instantiates it with Rc of dyn JavaFileObject; getSource()
///   returns Option of S because Java returns null when no source object is
///   associated with the diagnostic.
/// * NOPOS is an interface constant upstream; it is a module constant here
///   because Rust trait constants cannot be referenced without naming the
///   trait's type parameters.
pub trait Diagnostic<S> {
    /// Returns the kind of this diagnostic, for example, error or
    /// warning.
    /// @return the kind of this diagnostic
    fn get_kind(&self) -> Kind;

    /// Returns the source object associated with this diagnostic.
    ///
    /// @return the source object associated with this diagnostic.
    /// null if no source object is associated with the
    /// diagnostic.
    fn get_source(&self) -> Option<S>;

    /// Returns a character offset from the beginning of the source object
    /// associated with this diagnostic that indicates the location of
    /// the problem.  In addition, the following must be true:
    ///
    /// <p>getStartPosition() <= getPosition()
    /// <p>getPosition() <= getEndPosition()
    ///
    /// @return character offset from beginning of source; {@link
    /// #NOPOS} if {@link #getSource()} would return null or if
    /// no location is suitable
    fn get_position(&self) -> i64;

    /// Returns the character offset from the beginning of the file
    /// associated with this diagnostic that indicates the start of the
    /// problem.
    ///
    /// @return offset from beginning of file; {@link #NOPOS} if and
    /// only if {@link #getPosition()} returns {@link #NOPOS}
    fn get_start_position(&self) -> i64;

    /// Returns the character offset from the beginning of the file
    /// associated with this diagnostic that indicates the end of the
    /// problem.
    ///
    /// @return offset from beginning of file; {@link #NOPOS} if and
    /// only if {@link #getPosition()} returns {@link #NOPOS}
    fn get_end_position(&self) -> i64;

    /// Returns the line number of the character offset returned by
    /// {@linkplain #getPosition()}.
    ///
    /// @return a line number or {@link #NOPOS} if and only if {@link
    /// #getPosition()} returns {@link #NOPOS}
    fn get_line_number(&self) -> i64;

    /// Returns the column number of the character offset returned by
    /// {@linkplain #getPosition()}.
    ///
    /// @return a column number or {@link #NOPOS} if and only if {@link
    /// #getPosition()} returns {@link #NOPOS}
    fn get_column_number(&self) -> i64;

    /// Returns a diagnostic code indicating the type of diagnostic.  The
    /// code is implementation-dependent and might be null.
    ///
    /// @return a diagnostic code
    fn get_code(&self) -> Option<JavaString>;

    /// Returns a localized message for the given locale.  The actual
    /// message is implementation-dependent.  If the locale is
    /// null use the default locale.
    ///
    /// @param locale a locale; might be null
    /// @return a localized message
    fn get_message(&self, locale: Option<&JavaString>) -> JavaString;
}

/// Used to signal that no position is available.
///
/// Java: Diagnostic.NOPOS.
pub const NOPOS: i64 = -1;

/// Kinds of diagnostics, for example, error or warning.
///
/// The kind of a diagnostic can be used to determine how the
/// diagnostic should be presented to the user. For example,
/// errors might be colored red or prefixed with the word "Error",
/// while warnings might be colored yellow or prefixed with the
/// word "Warning". There is no requirement that the Kind
/// should imply any inherent semantic meaning to the message
/// of the diagnostic: for example, a tool might provide an
/// option to report all warnings as errors.
///
/// PORTING_RULES: Java enum Diagnostic.Kind -> Rust enum Kind in this module
/// (the Java name is nested in the interface).  Constant names and declaration
/// order are preserved.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Problem which prevents the tool's normal completion.
    ERROR,
    /// Problem which does not usually prevent the tool from
    /// completing normally.
    WARNING,
    /// Problem similar to a warning, but is mandated by the tool's
    /// specification.  For example, the Java Language
    /// Specification mandates warnings on certain
    /// unchecked operations and the use of deprecated methods.
    MANDATORY_WARNING,
    /// Informative message from the tool.
    NOTE,
    /// Diagnostic which does not fit within the other kinds.
    OTHER,
}

impl Kind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Kind::ERROR => "ERROR",
            Kind::WARNING => "WARNING",
            Kind::MANDATORY_WARNING => "MANDATORY_WARNING",
            Kind::NOTE => "NOTE",
            Kind::OTHER => "OTHER",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [Kind; 5] {
        [
            Kind::ERROR,
            Kind::WARNING,
            Kind::MANDATORY_WARNING,
            Kind::NOTE,
            Kind::OTHER,
        ]
    }
}

impl std::fmt::Display for Kind {
    /// Java: Enum.toString() = name().
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, Kind, NOPOS};
    use javac_strings::JavaString;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    struct Diag;

    impl Diagnostic<u32> for Diag {
        fn get_kind(&self) -> Kind {
            Kind::ERROR
        }

        fn get_source(&self) -> Option<u32> {
            None
        }

        fn get_position(&self) -> i64 {
            NOPOS
        }

        fn get_start_position(&self) -> i64 {
            NOPOS
        }

        fn get_end_position(&self) -> i64 {
            NOPOS
        }

        fn get_line_number(&self) -> i64 {
            NOPOS
        }

        fn get_column_number(&self) -> i64 {
            NOPOS
        }

        fn get_code(&self) -> Option<JavaString> {
            Some(js("compiler.err.test"))
        }

        fn get_message(&self, locale: Option<&JavaString>) -> JavaString {
            match locale {
                Some(tag) => js("[")
                    .concat(tag)
                    .concat(&js("] error")),
                None => js("error"),
            }
        }
    }

    #[test]
    fn nopos_is_minus_one() {
        // Java: Diagnostic.NOPOS == -1
        assert_eq!(NOPOS, -1);
    }

    #[test]
    fn external_implementation() {
        let diag = Diag;
        assert_eq!(diag.get_kind(), Kind::ERROR);
        assert_eq!(diag.get_position(), NOPOS);
        assert_eq!(diag.get_code(), Some(js("compiler.err.test")));
        assert_eq!(diag.get_message(None), js("error"));
        assert_eq!(diag.get_message(Some(&js("en"))), js("[en] error"));
    }

    #[test]
    fn kind_order_and_names() {
        assert_eq!(Kind::values()[0], Kind::ERROR);
        assert_eq!(Kind::MANDATORY_WARNING.name(), js("MANDATORY_WARNING"));
        assert_eq!(Kind::MANDATORY_WARNING.to_string(), "MANDATORY_WARNING");
    }
}
