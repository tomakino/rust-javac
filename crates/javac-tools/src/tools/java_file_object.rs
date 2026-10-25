// Origin: javax/tools/JavaFileObject.java @ 982d53c (jdk-17.0.19-ga)

//! File abstraction for tools operating on Java programming language
//! source and class files.

use super::file_object::FileObject;
use crate::lang::model::element::modifier::Modifier;
use crate::lang::model::element::nesting_kind::NestingKind;
use javac_strings::JavaString;

/// File abstraction for tools operating on Java programming language
/// source and class files.
///
/// <p>All methods in this interface might throw a SecurityException if
/// a security exception occurs.
///
/// <p>Unless explicitly allowed, all methods in this interface might
/// throw a NullPointerException if given a {@code null} argument.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @see JavaFileManager
/// @since 1.6
///
/// PORTING_RULES:
/// * `javax.lang.model.element.NestingKind` and `Modifier` are ported in
///   `crate::lang::model::element` because this interface signature needs them.
/// * Java returns `null` from `getNestingKind`/`getAccessLevel` when the
///   information is not known -> `Option<...>`.
/// * `extends FileObject` -> Rust supertrait.
/// * `String simpleName` -> `&javac_strings::JavaString`.
pub trait JavaFileObject: FileObject {
    /// Returns the kind of this file object.
    ///
    /// @return the kind
    fn get_kind(&self) -> Kind;

    /// Checks if this file object is compatible with the specified
    /// simple name and kind.  A simple name is a single identifier
    /// (not qualified) as defined in
    /// <cite>The Java Language Specification</cite>, section {@jls 6.2}.
    ///
    /// @param simpleName a simple name of a class
    /// @param kind a kind
    /// @return {@code true} if this file object is compatible; {@code false}
    /// otherwise
    fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool;

    /// Provides a hint about the nesting level of the class
    /// represented by this file object.  This method may return
    /// {@link NestingKind#MEMBER} to mean
    /// {@link NestingKind#LOCAL} or {@link NestingKind#ANONYMOUS}.
    /// If the nesting level is not known or this file object does not
    /// represent a class file this method returns {@code null}.
    ///
    /// @return the nesting kind, or {@code null} if the nesting kind
    /// is not known
    fn get_nesting_kind(&self) -> Option<NestingKind>;

    /// Provides a hint about the access level of the class represented
    /// by this file object.  If the access level is not known or
    /// this file object does not represent a class file this method
    /// returns {@code null}.
    ///
    /// @return the access level
    fn get_access_level(&self) -> Option<Modifier>;
}

/// Kinds of JavaFileObjects.
///
/// PORTING_RULES: Java `enum JavaFileObject.Kind` (nested enum) -> Rust enum
/// `Kind` in this module.  The Java `public final String extension` field is
/// expressed as the `extension()` accessor because Rust enum variants cannot
/// carry a per-variant field; the `Kind(String extension)` constructor is
/// private upstream and has no Rust form.  The extension is a Java String.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Source files written in the Java programming language.  For
    /// example, regular files ending with {@code .java}.
    SOURCE,

    /// Class files for the Java Virtual Machine.  For example,
    /// regular files ending with {@code .class}.
    CLASS,

    /// HTML files.  For example, regular files ending with {@code .html}.
    HTML,

    /// Any other kind.
    OTHER,
}

impl Kind {
    /// The extension which (by convention) is normally used for
    /// this kind of file object.  If no convention exists, the
    /// empty string ({@code ""}) is used.
    ///
    /// Java: the `extension` field of `JavaFileObject.Kind`.
    pub fn extension(&self) -> JavaString {
        JavaString::from(self.extension_str())
    }

    fn extension_str(&self) -> &'static str {
        match self {
            Kind::SOURCE => ".java",
            Kind::CLASS => ".class",
            Kind::HTML => ".html",
            Kind::OTHER => "",
        }
    }

    /// Java: `enum#name()` (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Kind::SOURCE => "SOURCE",
            Kind::CLASS => "CLASS",
            Kind::HTML => "HTML",
            Kind::OTHER => "OTHER",
        })
    }

    /// Java: `enum#values()`.
    pub fn values() -> [Kind; 4] {
        [Kind::SOURCE, Kind::CLASS, Kind::HTML, Kind::OTHER]
    }
}

impl std::fmt::Display for Kind {
    /// Java: `Enum.toString()` = `name()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::Kind;
    use javac_strings::JavaString;

    #[test]
    fn extensions() {
        assert_eq!(Kind::SOURCE.extension(), JavaString::from(".java"));
        assert_eq!(Kind::CLASS.extension(), JavaString::from(".class"));
        assert_eq!(Kind::HTML.extension(), JavaString::from(".html"));
        assert_eq!(Kind::OTHER.extension(), JavaString::from(""));
    }

    #[test]
    fn declaration_order_and_names() {
        assert_eq!(Kind::values()[0], Kind::SOURCE);
        assert_eq!(Kind::CLASS.name(), JavaString::from("CLASS"));
        assert_eq!(Kind::CLASS.to_string(), "CLASS");
    }
}
