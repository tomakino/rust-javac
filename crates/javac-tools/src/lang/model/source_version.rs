//! Source versions of the Java programming language.
//!
//! See the appropriate edition of
//! <cite>The Java Language Specification</cite>
//! for information about a particular source version.
//!
//! <p>Note that additional source version constants will be added to
//! model future releases of the language.
//!
//! — string/collection mapping:
//! * `CharSequence` parameters -> `&javac_strings::JavaString` (Java String
//!   semantics; docs/porting adapter.md §7).
//! * `SourceVersion` overloads are expressed with distinct function names
//!   (Java overload mapping, docs/porting adapter.md §3):
//!   `isName(CharSequence, SourceVersion)`  -> `is_name_version`
//!   `isKeyword(CharSequence, SourceVersion)` -> `is_keyword_version`
//!
//! `is_identifier` uses the registered `javac-unicode` adapter (JDK 17
//! `java.lang.Character` data): `is_java_identifier_start` /
//! `is_java_identifier_part`, exactly like the upstream
//! `Character.isJavaIdentifierStart/Part` calls.

use javac_strings::JavaString;

/// Source versions of the Java programming language.
///
/// See the appropriate edition of
/// <cite>The Java Language Specification</cite>
/// for information about a particular source version.
///
/// <p>Note that additional source version constants will be added to
/// model future releases of the language.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// Java enum constants keep their Java names
/// (UPPER_SNAKE_CASE); declaration order is preserved because it defines
/// `compareTo`, which upstream uses (`version.compareTo(RELEASE_2) >= 0`).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceVersion {
    /*
     * Summary of language evolution
     * 1.1: nested classes
     * 1.2: strictfp
     * 1.3: no changes
     * 1.4: assert
     * 1.5: annotations, generics, autoboxing, var-args...
     * 1.6: no changes
     * 1.7: diamond syntax, try-with-resources, etc.
     * 1.8: lambda expressions and default methods
     *   9: modules, small cleanups to 1.7 and 1.8 changes
     *  10: local-variable type inference (var)
     *  11: local-variable syntax for lambda parameters
     *  12: no changes (switch expressions in preview)
     *  13: no changes (switch expressions and text blocks in preview)
     *  14: switch expressions (pattern matching and records in
     *      preview, text blocks in preview again)
     *  15: text blocks (records and pattern matching in preview again)
     */

    /// The original version.
    ///
    /// The language described in
    /// <cite>The Java Language Specification, First Edition</cite>.
    RELEASE_0,

    /// The version recognized by the Java Platform 1.1.
    ///
    /// The language is {@code RELEASE_0} augmented with nested classes as described in the 1.1 update to
    /// <cite>The Java Language Specification, First Edition</cite>.
    RELEASE_1,

    /// The version recognized by the Java 2 Platform, Standard Edition,
    /// v 1.2.
    ///
    /// The language described in
    /// <cite>The Java Language Specification,
    /// Second Edition</cite>, which includes the {@code
    /// strictfp} modifier.
    RELEASE_2,

    /// The version recognized by the Java 2 Platform, Standard Edition,
    /// v 1.3.
    ///
    /// No major changes from {@code RELEASE_2}.
    RELEASE_3,

    /// The version recognized by the Java 2 Platform, Standard Edition,
    /// v 1.4.
    ///
    /// Added a simple assertion facility.
    RELEASE_4,

    /// The version recognized by the Java 2 Platform, Standard
    /// Edition 5.0.
    ///
    /// The language described in
    /// <cite>The Java Language Specification,
    /// Third Edition</cite>.  First release to support
    /// generics, annotations, autoboxing, var-args, enhanced {@code
    /// for} loop, and hexadecimal floating-point literals.
    RELEASE_5,

    /// The version recognized by the Java Platform, Standard Edition
    /// 6.
    ///
    /// No major changes from {@code RELEASE_5}.
    RELEASE_6,

    /// The version recognized by the Java Platform, Standard Edition
    /// 7.
    ///
    /// Additions in this release include, diamond syntax for
    /// constructors, {@code try}-with-resources, strings in switch,
    /// binary literals, and multi-catch.
    /// @since 1.7
    RELEASE_7,

    /// The version recognized by the Java Platform, Standard Edition
    /// 8.
    ///
    /// Additions in this release include lambda expressions and default methods.
    /// @since 1.8
    RELEASE_8,

    /// The version recognized by the Java Platform, Standard Edition
    /// 9.
    ///
    /// Additions in this release include modules and removal of a
    /// single underscore from the set of legal identifier names.
    ///
    /// @since 9
    RELEASE_9,

    /// The version recognized by the Java Platform, Standard Edition
    /// 10.
    ///
    /// Additions in this release include local-variable type inference
    /// ({@code var}).
    ///
    /// @since 10
    RELEASE_10,

    /// The version recognized by the Java Platform, Standard Edition
    /// 11.
    ///
    /// Additions in this release include local-variable syntax for
    /// lambda parameters.
    ///
    /// @since 11
    RELEASE_11,

    /// The version recognized by the Java Platform, Standard Edition
    /// 12.
    ///
    /// @since 12
    RELEASE_12,

    /// The version recognized by the Java Platform, Standard Edition
    /// 13.
    ///
    /// @since 13
    RELEASE_13,

    /// The version recognized by the Java Platform, Standard Edition
    /// 14.
    ///
    /// Additions in this release include switch expressions.
    ///
    /// @since 14
    RELEASE_14,

    /// The version recognized by the Java Platform, Standard Edition
    /// 15.
    ///
    /// Additions in this release include text blocks.
    ///
    /// @since 15
    RELEASE_15,

    /// The version recognized by the Java Platform, Standard Edition
    /// 16.
    ///
    /// Additions in this release include pattern matching for {@code
    /// instanceof} and records.
    ///
    /// @since 16
    RELEASE_16,

    /// The version recognized by the Java Platform, Standard Edition
    /// 17.
    ///
    /// Additions in this release include sealed classes and
    /// restoration of always-strict floating-point semantics.
    ///
    /// @since 17
    RELEASE_17,
}

impl SourceVersion {
    // Note that when adding constants for newer releases, the
    // behavior of latest() and latestSupported() must be updated too.

    /// {@return the latest source version that can be modeled}
    pub fn latest() -> SourceVersion {
        SourceVersion::RELEASE_17
    }

    /// Java: `enum SourceVersion#name()` (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            SourceVersion::RELEASE_0 => "RELEASE_0",
            SourceVersion::RELEASE_1 => "RELEASE_1",
            SourceVersion::RELEASE_2 => "RELEASE_2",
            SourceVersion::RELEASE_3 => "RELEASE_3",
            SourceVersion::RELEASE_4 => "RELEASE_4",
            SourceVersion::RELEASE_5 => "RELEASE_5",
            SourceVersion::RELEASE_6 => "RELEASE_6",
            SourceVersion::RELEASE_7 => "RELEASE_7",
            SourceVersion::RELEASE_8 => "RELEASE_8",
            SourceVersion::RELEASE_9 => "RELEASE_9",
            SourceVersion::RELEASE_10 => "RELEASE_10",
            SourceVersion::RELEASE_11 => "RELEASE_11",
            SourceVersion::RELEASE_12 => "RELEASE_12",
            SourceVersion::RELEASE_13 => "RELEASE_13",
            SourceVersion::RELEASE_14 => "RELEASE_14",
            SourceVersion::RELEASE_15 => "RELEASE_15",
            SourceVersion::RELEASE_16 => "RELEASE_16",
            SourceVersion::RELEASE_17 => "RELEASE_17",
        })
    }

    /// Java: `enum SourceVersion#valueOf(String)`.
    ///
    /// an unknown name raises `IllegalArgumentException`
    /// upstream, which is API misuse and maps to `panic!`.
    pub fn value_of(name: &JavaString) -> SourceVersion {
        SourceVersion::values()
            .into_iter()
            .find(|v| v.name() == *name)
            .unwrap_or_else(|| panic!("IllegalArgumentException: No enum constant SourceVersion.{name}"))
    }

    /// Java: `enum SourceVersion#values()`.
    pub fn values() -> [SourceVersion; 18] {
        [
            SourceVersion::RELEASE_0,
            SourceVersion::RELEASE_1,
            SourceVersion::RELEASE_2,
            SourceVersion::RELEASE_3,
            SourceVersion::RELEASE_4,
            SourceVersion::RELEASE_5,
            SourceVersion::RELEASE_6,
            SourceVersion::RELEASE_7,
            SourceVersion::RELEASE_8,
            SourceVersion::RELEASE_9,
            SourceVersion::RELEASE_10,
            SourceVersion::RELEASE_11,
            SourceVersion::RELEASE_12,
            SourceVersion::RELEASE_13,
            SourceVersion::RELEASE_14,
            SourceVersion::RELEASE_15,
            SourceVersion::RELEASE_16,
            SourceVersion::RELEASE_17,
        ]
    }

    /// {@return the latest source version fully supported by the
    /// current execution environment}  {@code RELEASE_9} or later must
    /// be returned.
    ///
    /// @apiNote This method is included alongside {@link latest} to
    /// allow identification of situations where the language model API
    /// is running on a platform version different than the latest
    /// version modeled by the API.
    ///
    /// upstream computes this from
    /// `Runtime.version().feature()`; rust-javac targets the JDK 17 platform
    /// (docs/PLAN.md baseline), so the execution environment always supports
    /// `RELEASE_17`, exactly as upstream does when running on JDK 17.
    pub fn latest_supported() -> SourceVersion {
        // Java: private static final SourceVersion latestSupported =
        //           getLatestSupported();
        get_latest_supported()
    }

    /// Returns whether or not {@code name} is a syntactically valid
    /// identifier (simple name) or keyword in the latest source
    /// version.  The method returns {@code true} if the name consists
    /// of an initial character for which {@link
    /// Character#isJavaIdentifierStart(int)} returns {@code true},
    /// followed only by characters for which {@link
    /// Character#isJavaIdentifierPart(int)} returns {@code true}.
    /// This pattern matches regular identifiers, keywords, restricted
    /// keywords, restricted identifiers and the literals {@code "true"},
    /// {@code "false"}, {@code "null"}.
    ///
    /// The method returns {@code false} for all other strings.
    ///
    /// @param name the string to check
    /// @return {@code true} if this string is a
    /// syntactically valid identifier or keyword, {@code false}
    /// otherwise.
    ///
    /// @jls 3.8 Identifiers
    pub fn is_identifier(name: &JavaString) -> bool {
        // Java:
        //     String id = name.toString();
        //     if (id.length() == 0) return false;
        //     int cp = id.codePointAt(0);
        //     if (!Character.isJavaIdentifierStart(cp)) return false;
        //     for (int i = Character.charCount(cp); i < id.length();
        //              i += Character.charCount(cp)) {
        //         cp = id.codePointAt(i);
        //         if (!Character.isJavaIdentifierPart(cp)) return false;
        //     }
        //     return true;
        //
        // Rust: Character.isJavaIdentifierStart/Part are provided by
        // the registered javac-unicode adapter (JDK 17 Character data).
        if name.is_empty() {
            return false;
        }
        let mut cp = name.code_point_at(0);
        if !javac_unicode::is_java_identifier_start(cp) {
            return false;
        }
        let mut i = char_count(cp);
        while i < name.len() {
            cp = name.code_point_at(i);
            if !javac_unicode::is_java_identifier_part(cp) {
                return false;
            }
            i += char_count(cp);
        }
        true
    }

    /// Returns whether or not {@code name} is a syntactically valid
    /// qualified name in the latest source version.
    ///
    /// Syntactically, a qualified name is a sequence of identifiers
    /// separated by period characters ("{@code .}"). This method
    /// splits the input string into period-separated segments and
    /// applies checks to each segment in turn.
    ///
    /// Unlike {@link #isIdentifier isIdentifier}, this method returns
    /// {@code false} for keywords, boolean literals, and the null
    /// literal in any segment.
    ///
    /// This method returns {@code true} for <i>restricted
    /// keywords</i> and <i>restricted identifiers</i>.
    ///
    /// @param name the string to check
    /// @return {@code true} if this string is a
    /// syntactically valid name, {@code false} otherwise.
    /// @jls 3.9 Keywords
    /// @jls 6.2 Names and Identifiers
    pub fn is_name(name: &JavaString) -> bool {
        SourceVersion::is_name_version(name, SourceVersion::latest())
    }

    /// Returns whether or not {@code name} is a syntactically valid
    /// qualified name in the given source version.
    ///
    /// @param name the string to check
    /// @param version the version to use
    /// @return {@code true} if this string is a
    /// syntactically valid name, {@code false} otherwise.
    /// @jls 3.9 Keywords
    /// @jls 6.2 Names and Identifiers
    /// @since 9
    ///
    /// overload `isName(CharSequence, SourceVersion)`.
    /// Java: for (String s : id.split("\\.", -1)).  `split_char` keeps the
    /// trailing empty strings like `String.split(regex, -1)`.
    pub fn is_name_version(name: &JavaString, version: SourceVersion) -> bool {
        for s in name.split_char(b'.' as u16) {
            if !SourceVersion::is_identifier(&s) || SourceVersion::is_keyword_version(&s, version)
            {
                return false;
            }
        }
        true
    }

    /// Returns whether or not {@code s} is a keyword, boolean literal,
    /// or null literal in the latest source version.
    /// This method returns {@code false} for <i>restricted
    /// keywords</i> and <i>restricted identifiers</i>.
    ///
    /// @param s the string to check
    /// @return {@code true} if {@code s} is a keyword, or boolean
    /// literal, or null literal, {@code false} otherwise.
    /// @jls 3.9 Keywords
    /// @jls 3.10.3 Boolean Literals
    /// @jls 3.10.8 The Null Literal
    pub fn is_keyword(s: &JavaString) -> bool {
        SourceVersion::is_keyword_version(s, SourceVersion::latest())
    }

    /// Returns whether or not {@code s} is a keyword, boolean literal,
    /// or null literal in the given source version.
    /// This method returns {@code false} for <i>restricted
    /// keywords</i> and <i>restricted identifiers</i>.
    ///
    /// @param s the string to check
    /// @param version the version to use
    /// @return {@code true} if {@code s} is a keyword, or boolean
    /// literal, or null literal, {@code false} otherwise.
    /// @jls 3.9 Keywords
    /// @jls 3.10.3 Boolean Literals
    /// @jls 3.10.8 The Null Literal
    /// @since 9
    ///
    /// overload `isKeyword(CharSequence, SourceVersion)`.
    /// The Java switch on a String is a chain of equals() comparisons; it is
    /// transcribed as an if/else chain over JavaString equality.
    /// `version.compareTo(X) >= 0` becomes `version >= X` because the enum
    /// derives Ord from declaration order (= Java enum natural order).
    pub fn is_keyword_version(s: &JavaString, version: SourceVersion) -> bool {
        // A trip through history
        if *s == JavaString::from("strictfp") {
            return version >= SourceVersion::RELEASE_2;
        }
        if *s == JavaString::from("assert") {
            return version >= SourceVersion::RELEASE_4;
        }
        if *s == JavaString::from("enum") {
            return version >= SourceVersion::RELEASE_5;
        }
        if *s == JavaString::from("_") {
            return version >= SourceVersion::RELEASE_9;
        }

        // case "non-sealed": can be added once it is a keyword only
        // dependent on release and not also preview features being
        // enabled.

        // Keywords common across versions

        // Modifiers
        if *s == JavaString::from("public")
            || *s == JavaString::from("protected")
            || *s == JavaString::from("private")
            || *s == JavaString::from("abstract")
            || *s == JavaString::from("static")
            || *s == JavaString::from("final")
            || *s == JavaString::from("transient")
            || *s == JavaString::from("volatile")
            || *s == JavaString::from("synchronized")
            || *s == JavaString::from("native")
        {
            return true;
        }

        // Declarations
        if *s == JavaString::from("class")
            || *s == JavaString::from("interface")
            || *s == JavaString::from("extends")
            || *s == JavaString::from("package")
            || *s == JavaString::from("throws")
            || *s == JavaString::from("implements")
        {
            return true;
        }

        // Primitive types and void
        if *s == JavaString::from("boolean")
            || *s == JavaString::from("byte")
            || *s == JavaString::from("char")
            || *s == JavaString::from("short")
            || *s == JavaString::from("int")
            || *s == JavaString::from("long")
            || *s == JavaString::from("float")
            || *s == JavaString::from("double")
            || *s == JavaString::from("void")
        {
            return true;
        }

        // Control flow
        if *s == JavaString::from("if")
            || *s == JavaString::from("else")
            || *s == JavaString::from("try")
            || *s == JavaString::from("catch")
            || *s == JavaString::from("finally")
            || *s == JavaString::from("do")
            || *s == JavaString::from("while")
            || *s == JavaString::from("for")
            || *s == JavaString::from("continue")
            || *s == JavaString::from("switch")
            || *s == JavaString::from("case")
            || *s == JavaString::from("default")
            || *s == JavaString::from("break")
            || *s == JavaString::from("throw")
            || *s == JavaString::from("return")
        {
            return true;
        }

        // Other keywords
        if *s == JavaString::from("this")
            || *s == JavaString::from("new")
            || *s == JavaString::from("super")
            || *s == JavaString::from("import")
            || *s == JavaString::from("instanceof")
        {
            return true;
        }

        // Forbidden!
        if *s == JavaString::from("goto") || *s == JavaString::from("const") {
            return true;
        }

        // literals
        if *s == JavaString::from("null")
            || *s == JavaString::from("true")
            || *s == JavaString::from("false")
        {
            return true;
        }

        false
    }
}

/// Java: `private static SourceVersion getLatestSupported()`.
///
/// upstream maps the runtime feature version to a constant:
/// `(intVersion >= 11) ? valueOf("RELEASE_" + Math.min(17, intVersion))
///  : RELEASE_10`. rust-javac runs as the JDK 17 platform
/// (docs/PLAN.md baseline), so the execution environment is JDK 17 and the
/// result is `RELEASE_17` for every run; the mapping is kept here so the
/// upstream diff remains visible.
fn get_latest_supported() -> SourceVersion {
    // Java: int intVersion = Runtime.version().feature();
    //       return (intVersion >= 11) ?
    //           valueOf("RELEASE_" + Math.min(17, intVersion)) :
    //           RELEASE_10;
    SourceVersion::RELEASE_17
}

/// Java: `Character.charCount(int codePoint)` — 2 for a supplementary code
/// point (encoded as a UTF-16 surrogate pair), 1 otherwise.
fn char_count(code_point: i32) -> usize {
    if code_point >= javac_unicode::MIN_SUPPLEMENTARY_CODE_POINT { 2 } else { 1 }
}

impl std::fmt::Display for SourceVersion {
    /// Java: `Enum.toString()` = `name()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::SourceVersion;
    use javac_strings::JavaString;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    #[test]
    fn latest_is_release_17() {
        assert_eq!(SourceVersion::latest(), SourceVersion::RELEASE_17);
        assert_eq!(SourceVersion::latest_supported(), SourceVersion::RELEASE_17);
    }

    #[test]
    fn value_of_and_values() {
        assert_eq!(SourceVersion::values().len(), 18);
        assert_eq!(SourceVersion::value_of(&js("RELEASE_17")), SourceVersion::RELEASE_17);
        assert_eq!(SourceVersion::value_of(&js("RELEASE_8")), SourceVersion::RELEASE_8);
        assert_eq!(SourceVersion::RELEASE_17.name(), js("RELEASE_17"));
    }

    #[test]
    #[should_panic]
    fn value_of_unknown_name_panics() {
        SourceVersion::value_of(&js("RELEASE_99"));
    }

    #[test]
    fn version_dependent_keywords() {
        // strictfp since 1.2, assert since 1.4, enum since 1.5, _ since 9.
        assert!(!SourceVersion::is_keyword_version(&js("strictfp"), SourceVersion::RELEASE_1));
        assert!(SourceVersion::is_keyword_version(&js("strictfp"), SourceVersion::RELEASE_2));
        assert!(!SourceVersion::is_keyword_version(&js("assert"), SourceVersion::RELEASE_3));
        assert!(SourceVersion::is_keyword_version(&js("assert"), SourceVersion::RELEASE_4));
        assert!(!SourceVersion::is_keyword_version(&js("enum"), SourceVersion::RELEASE_4));
        assert!(SourceVersion::is_keyword_version(&js("enum"), SourceVersion::RELEASE_5));
        assert!(!SourceVersion::is_keyword_version(&js("_"), SourceVersion::RELEASE_8));
        assert!(SourceVersion::is_keyword_version(&js("_"), SourceVersion::RELEASE_9));
    }

    #[test]
    fn common_keywords_and_literals() {
        for kw in ["public", "class", "int", "while", "new", "goto", "null", "true", "false"] {
            assert!(SourceVersion::is_keyword(&js(kw)), "{kw}");
        }
        assert!(!SourceVersion::is_keyword(&js("var")));
        assert!(!SourceVersion::is_keyword(&js("record")));
        assert!(!SourceVersion::is_keyword(&js("sealed")));
        assert!(!SourceVersion::is_keyword(&js("non-sealed")));
        assert!(!SourceVersion::is_keyword(&js("")));
        // A Java String with an unpaired surrogate can never equal a keyword.
        assert!(!SourceVersion::is_keyword(&JavaString::from_units(vec![0xD800])));
    }

    #[test]
    fn identifiers() {
        // Character.isJavaIdentifierStart/Part through javac-unicode.
        assert!(SourceVersion::is_identifier(&js("a")));
        assert!(SourceVersion::is_identifier(&js("_")));
        assert!(SourceVersion::is_identifier(&js("$")));
        assert!(SourceVersion::is_identifier(&js("abc123")));
        assert!(SourceVersion::is_identifier(&js("é")));
        // A supplementary code point (surrogate pair): a letter is a valid start.
        assert!(SourceVersion::is_identifier(&js("\u{1D49C}")));
        assert!(!SourceVersion::is_identifier(&js("")));
        assert!(!SourceVersion::is_identifier(&js("1abc")));
        assert!(!SourceVersion::is_identifier(&js("a-b")));
        assert!(!SourceVersion::is_identifier(&js("a.b")));
        assert!(!SourceVersion::is_identifier(&js("a\u{1F600}")));
    }

    #[test]
    fn names() {
        assert!(SourceVersion::is_name(&js("java.lang.String")));
        assert!(SourceVersion::is_name(&js("com.example.Test")));
        assert!(!SourceVersion::is_name(&js("")));
        assert!(!SourceVersion::is_name(&js("int")));
        assert!(!SourceVersion::is_name(&js("java.long")));
        assert!(!SourceVersion::is_name(&js("a..b")));
        assert!(!SourceVersion::is_name(&js("a.-b")));
        // "_" is an identifier but a keyword since release 9.
        assert!(SourceVersion::is_identifier(&js("_")));
        assert!(!SourceVersion::is_name(&js("_")));
    }

    #[test]
    fn split_preserves_trailing_empty_segment() {
        // Java: "a.".split("\\.", -1) -> ["a", ""]
        let parts = js("a.").split_char(b'.' as u16);
        assert_eq!(parts.len(), 2);
        assert!(parts[1].is_empty());
    }
}
