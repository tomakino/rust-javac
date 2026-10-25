// Origin: com/sun/tools/javac/code/Lint.java @ 982d53c (jdk-17.0.19-ga)

//! A class for handling -Xlint suboptions and @SuppressWarnings.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * the nested enum LintCategory (31 constants) keeps the Java declaration
//!   order; `option`/`hidden` fields become accessors, and the static option map
//!   the enum constructor fills becomes LintCategory::get (a linear match; the
//!   ConcurrentHashMap had no iteration-order use).
//! * Java EnumSet<LintCategory> -> BTreeSet (unique elements iterated in enum
//!   declaration order, which is the EnumSet iteration order; §9.10).
//! * Java protected Lint(Lint other) copy constructor -> pub(crate) new_from;
//!   suppress(LintCategory...) -> suppress(&[LintCategory]); toString -> Display
//!   with the EnumSet format ("[A, B]").
//! * TODO(Options batch): instance(Context) and the protected Lint(Context)
//!   constructor initialize the categories from Options (-Xlint/-Xlint:all/none/
//!   custom/-preview) and Source.instance; Options/main.Option are not ported yet,
//!   so instance stays a single unimplemented entry point (no substitute Options).
//! * TODO(Attribute/Symtab batch): augment(Attribute.Compound), augment(Symbol) and
//!   the AugmentVisitor need Attribute.Compound (only AttributeKind/Visitor exist so
//!   far), Symtab.instance and Symbol.getDeclarationAttributes; the Symbol overload
//!   is declared and unimplemented, the Compound overload is not declared until the
//!   Attribute node types are ported.  No substitute logic is written.

use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use crate::code::symbol::Symbol;
use crate::util::context::Context;

/// A class for handling -Xlint suboptions and @SuppressWarnings.
pub struct Lint {
    /// Java: private final EnumSet<LintCategory> values.
    values: BTreeSet<LintCategory>,
    /// Java: private final EnumSet<LintCategory> suppressedValues.
    suppressed_values: BTreeSet<LintCategory>,
    // Java: private final AugmentVisitor augmentor;
    // TODO(Attribute/Symtab batch): the AugmentVisitor needs Attribute.Compound and
    // Symtab.instance, which are not ported yet; no field is represented until then.
}

impl Lint {
    /// Java:
    /// <pre>
    /// public static Lint instance(Context context) {
    ///     Lint instance = context.get(lintKey);
    ///     if (instance == null)
    ///         instance = new Lint(context);
    ///     return instance;
    /// }
    /// </pre>
    ///
    /// TODO(Options batch): the constructor reads Options (-Xlint/-Xlint:all/none/
    /// custom/-preview) and Source.instance; both depend on the not yet ported
    /// Options/main.Option.  No substitute Options is invented.
    pub fn instance(_context: &mut Context) -> Rc<Lint> {
        unimplemented!(
            "TODO(Options batch): Lint.instance (Options, -Xlint and Source.instance are not ported)"
        )
    }

    /// Java: protected Lint(Lint other).
    pub(crate) fn new_from(other: &Lint) -> Lint {
        Lint {
            values: other.values.clone(),
            suppressed_values: other.suppressed_values.clone(),
        }
    }

    /// Returns a new Lint that has the given LintCategorys suppressed.
    /// @param lc one or more categories to be suppressed
    ///
    /// Java: public Lint suppress(LintCategory... lc).
    pub fn suppress(&self, lc: &[LintCategory]) -> Lint {
        let mut lint = Lint::new_from(self);
        for category in lc {
            lint.values.remove(category);
            lint.suppressed_values.insert(*category);
        }
        lint
    }

    /// Returns the result of combining the values in this object with
    /// the metadata on the given symbol.
    ///
    /// TODO(Attribute/Symtab batch): the upstream walks the declaration
    /// attributes through the AugmentVisitor (Symtab/Attribute not ported) and
    /// then removes DEPRECATION for deprecated symbols; the entry point is kept
    /// unimplemented (no substitute logic).
    pub fn augment(&self, _sym: &Symbol) -> Lint {
        unimplemented!(
            "TODO(Attribute/Symtab batch): Lint.augment(Symbol) (AugmentVisitor/Symtab/Attribute.Compound are not ported)"
        )
    }

    /// Checks if a warning category is enabled. A warning category may be enabled
    /// on the command line, or by default, and can be temporarily disabled with
    /// the SuppressWarnings annotation.
    pub fn is_enabled(&self, lc: LintCategory) -> bool {
        self.values.contains(&lc)
    }

    /// Checks is a warning category has been specifically suppressed, by means
    /// of the SuppressWarnings annotation, or, in the case of the deprecated
    /// category, whether it has been implicitly suppressed by virtue of the
    /// current entity being itself deprecated.
    pub fn is_suppressed(&self, lc: LintCategory) -> bool {
        self.suppressed_values.contains(&lc)
    }
}

impl fmt::Display for Lint {
    /// Java: public String toString() {
    ///           return "Lint:[values" + values + " suppressedValues" + suppressedValues + "]";
    ///       }
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Lint:[values{} suppressedValues{}]",
            format_categories(&self.values),
            format_categories(&self.suppressed_values)
        )
    }
}

/// Java: EnumSet.toString() ("[A, B]" in enum declaration order).
fn format_categories(categories: &BTreeSet<LintCategory>) -> String {
    let names: Vec<&str> = categories.iter().map(|category| category.name()).collect();
    format!("[{}]", names.join(", "))
}

/// Categories of warnings that can be generated by the compiler.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LintCategory {
    /// Warn when code refers to a auxiliary class that is hidden in a source file
    /// (ie source file name is different from the class name, and the type is not
    /// properly nested) and the referring code is not located in the same source file.
    AUXILIARYCLASS,
    /// Warn about use of unnecessary casts.
    CAST,
    /// Warn about issues related to classfile contents
    CLASSFILE,
    /// Warn about use of deprecated items.
    DEPRECATION,
    /// Warn about items which are documented with an {@code @deprecated} JavaDoc
    /// comment, but which do not have {@code @Deprecated} annotation.
    DEP_ANN,
    /// Warn about division by constant integer 0.
    DIVZERO,
    /// Warn about empty statement after if.
    EMPTY,
    /// Warn about issues regarding module exports.
    EXPORTS,
    /// Warn about falling through from one case of a switch statement to the next.
    FALLTHROUGH,
    /// Warn about finally clauses that do not terminate normally.
    FINALLY,
    /// Warn about compiler generation of a default constructor.
    MISSING_EXPLICIT_CTOR,
    /// Warn about module system related issues.
    MODULE,
    /// Warn about issues regarding module opens.
    OPENS,
    /// Warn about issues relating to use of command line options
    OPTIONS,
    /// Warn about issues regarding method overloads.
    OVERLOADS,
    /// Warn about issues regarding method overrides.
    OVERRIDES,
    /// Warn about invalid path elements on the command line.
    /// Such warnings cannot be suppressed with the SuppressWarnings
    /// annotation.
    PATH,
    /// Warn about issues regarding annotation processing.
    PROCESSING,
    /// Warn about unchecked operations on raw types.
    RAW,
    /// Warn about use of deprecated-for-removal items.
    REMOVAL,
    /// Warn about use of automatic modules in the requires clauses.
    REQUIRES_AUTOMATIC,
    /// Warn about automatic modules in requires transitive.
    REQUIRES_TRANSITIVE_AUTOMATIC,
    /// Warn about Serializable classes that do not provide a serial version ID.
    SERIAL,
    /// Warn about issues relating to use of statics
    STATIC,
    /// Warn about unnecessary uses of the strictfp modifier
    STRICTFP,
    /// Warn about synchronization attempts on instances of @ValueBased classes.
    SYNCHRONIZATION,
    /// Warn about issues relating to use of text blocks
    TEXT_BLOCKS,
    /// Warn about issues relating to use of try blocks (i.e. try-with-resources)
    TRY,
    /// Warn about unchecked operations on raw types.
    UNCHECKED,
    /// Warn about potentially unsafe vararg methods
    VARARGS,
    /// Warn about use of preview features.
    PREVIEW,
}

impl LintCategory {
    /// Java: LintCategory.values().
    pub fn values() -> [LintCategory; 31] {
        [
            LintCategory::AUXILIARYCLASS,
            LintCategory::CAST,
            LintCategory::CLASSFILE,
            LintCategory::DEPRECATION,
            LintCategory::DEP_ANN,
            LintCategory::DIVZERO,
            LintCategory::EMPTY,
            LintCategory::EXPORTS,
            LintCategory::FALLTHROUGH,
            LintCategory::FINALLY,
            LintCategory::MISSING_EXPLICIT_CTOR,
            LintCategory::MODULE,
            LintCategory::OPENS,
            LintCategory::OPTIONS,
            LintCategory::OVERLOADS,
            LintCategory::OVERRIDES,
            LintCategory::PATH,
            LintCategory::PROCESSING,
            LintCategory::RAW,
            LintCategory::REMOVAL,
            LintCategory::REQUIRES_AUTOMATIC,
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC,
            LintCategory::SERIAL,
            LintCategory::STATIC,
            LintCategory::STRICTFP,
            LintCategory::SYNCHRONIZATION,
            LintCategory::TEXT_BLOCKS,
            LintCategory::TRY,
            LintCategory::UNCHECKED,
            LintCategory::VARARGS,
            LintCategory::PREVIEW,
        ]
    }

    /// Java: {@code public final String option} (the -Xlint suboption name).
    pub const fn option(self) -> &'static str {
        match self {
            LintCategory::AUXILIARYCLASS => "auxiliaryclass",
            LintCategory::CAST => "cast",
            LintCategory::CLASSFILE => "classfile",
            LintCategory::DEPRECATION => "deprecation",
            LintCategory::DEP_ANN => "dep-ann",
            LintCategory::DIVZERO => "divzero",
            LintCategory::EMPTY => "empty",
            LintCategory::EXPORTS => "exports",
            LintCategory::FALLTHROUGH => "fallthrough",
            LintCategory::FINALLY => "finally",
            LintCategory::MISSING_EXPLICIT_CTOR => "missing-explicit-ctor",
            LintCategory::MODULE => "module",
            LintCategory::OPENS => "opens",
            LintCategory::OPTIONS => "options",
            LintCategory::OVERLOADS => "overloads",
            LintCategory::OVERRIDES => "overrides",
            LintCategory::PATH => "path",
            LintCategory::PROCESSING => "processing",
            LintCategory::RAW => "rawtypes",
            LintCategory::REMOVAL => "removal",
            LintCategory::REQUIRES_AUTOMATIC => "requires-automatic",
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC => "requires-transitive-automatic",
            LintCategory::SERIAL => "serial",
            LintCategory::STATIC => "static",
            LintCategory::STRICTFP => "strictfp",
            LintCategory::SYNCHRONIZATION => "synchronization",
            LintCategory::TEXT_BLOCKS => "text-blocks",
            LintCategory::TRY => "try",
            LintCategory::UNCHECKED => "unchecked",
            LintCategory::VARARGS => "varargs",
            LintCategory::PREVIEW => "preview",
        }
    }

    /// Java: {@code public final boolean hidden}.
    ///
    /// PORTING_RULES: every upstream constant is created with the one-argument
    /// constructor (hidden = false); the field is kept as an accessor.
    pub const fn hidden(self) -> bool {
        false
    }

    /// Java: static LintCategory get(String option) — the static option map.
    pub fn get(option: &str) -> Option<LintCategory> {
        let category = match option {
            "auxiliaryclass" => LintCategory::AUXILIARYCLASS,
            "cast" => LintCategory::CAST,
            "classfile" => LintCategory::CLASSFILE,
            "deprecation" => LintCategory::DEPRECATION,
            "dep-ann" => LintCategory::DEP_ANN,
            "divzero" => LintCategory::DIVZERO,
            "empty" => LintCategory::EMPTY,
            "exports" => LintCategory::EXPORTS,
            "fallthrough" => LintCategory::FALLTHROUGH,
            "finally" => LintCategory::FINALLY,
            "missing-explicit-ctor" => LintCategory::MISSING_EXPLICIT_CTOR,
            "module" => LintCategory::MODULE,
            "opens" => LintCategory::OPENS,
            "options" => LintCategory::OPTIONS,
            "overloads" => LintCategory::OVERLOADS,
            "overrides" => LintCategory::OVERRIDES,
            "path" => LintCategory::PATH,
            "processing" => LintCategory::PROCESSING,
            "rawtypes" => LintCategory::RAW,
            "removal" => LintCategory::REMOVAL,
            "requires-automatic" => LintCategory::REQUIRES_AUTOMATIC,
            "requires-transitive-automatic" => LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC,
            "serial" => LintCategory::SERIAL,
            "static" => LintCategory::STATIC,
            "strictfp" => LintCategory::STRICTFP,
            "synchronization" => LintCategory::SYNCHRONIZATION,
            "text-blocks" => LintCategory::TEXT_BLOCKS,
            "try" => LintCategory::TRY,
            "unchecked" => LintCategory::UNCHECKED,
            "varargs" => LintCategory::VARARGS,
            "preview" => LintCategory::PREVIEW,
            _ => return None,
        };
        Some(category)
    }

    /// Java: enum#name() — the constant name (Enum.toString()).
    pub const fn name(self) -> &'static str {
        match self {
            LintCategory::AUXILIARYCLASS => "AUXILIARYCLASS",
            LintCategory::CAST => "CAST",
            LintCategory::CLASSFILE => "CLASSFILE",
            LintCategory::DEPRECATION => "DEPRECATION",
            LintCategory::DEP_ANN => "DEP_ANN",
            LintCategory::DIVZERO => "DIVZERO",
            LintCategory::EMPTY => "EMPTY",
            LintCategory::EXPORTS => "EXPORTS",
            LintCategory::FALLTHROUGH => "FALLTHROUGH",
            LintCategory::FINALLY => "FINALLY",
            LintCategory::MISSING_EXPLICIT_CTOR => "MISSING_EXPLICIT_CTOR",
            LintCategory::MODULE => "MODULE",
            LintCategory::OPENS => "OPENS",
            LintCategory::OPTIONS => "OPTIONS",
            LintCategory::OVERLOADS => "OVERLOADS",
            LintCategory::OVERRIDES => "OVERRIDES",
            LintCategory::PATH => "PATH",
            LintCategory::PROCESSING => "PROCESSING",
            LintCategory::RAW => "RAW",
            LintCategory::REMOVAL => "REMOVAL",
            LintCategory::REQUIRES_AUTOMATIC => "REQUIRES_AUTOMATIC",
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC => "REQUIRES_TRANSITIVE_AUTOMATIC",
            LintCategory::SERIAL => "SERIAL",
            LintCategory::STATIC => "STATIC",
            LintCategory::STRICTFP => "STRICTFP",
            LintCategory::SYNCHRONIZATION => "SYNCHRONIZATION",
            LintCategory::TEXT_BLOCKS => "TEXT_BLOCKS",
            LintCategory::TRY => "TRY",
            LintCategory::UNCHECKED => "UNCHECKED",
            LintCategory::VARARGS => "VARARGS",
            LintCategory::PREVIEW => "PREVIEW",
        }
    }
}

impl fmt::Display for LintCategory {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::{Lint, LintCategory};

    fn make_lint(values: &[LintCategory], suppressed: &[LintCategory]) -> Lint {
        Lint {
            values: values.iter().copied().collect(),
            suppressed_values: suppressed.iter().copied().collect(),
        }
    }

    #[test]
    fn categories_are_complete_and_in_declaration_order() {
        let values = LintCategory::values();
        assert_eq!(values.len(), 31);
        assert_eq!(values[0], LintCategory::AUXILIARYCLASS);
        assert_eq!(values[1], LintCategory::CAST);
        assert_eq!(values[values.len() - 1], LintCategory::PREVIEW);
    }

    #[test]
    fn option_strings_match_the_xlint_names() {
        assert_eq!(LintCategory::AUXILIARYCLASS.option(), "auxiliaryclass");
        assert_eq!(LintCategory::DEP_ANN.option(), "dep-ann");
        assert_eq!(LintCategory::RAW.option(), "rawtypes");
        assert_eq!(
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC.option(),
            "requires-transitive-automatic"
        );
        assert_eq!(LintCategory::PREVIEW.option(), "preview");
    }

    #[test]
    fn get_maps_every_option_back_to_its_category() {
        for category in LintCategory::values() {
            assert_eq!(LintCategory::get(category.option()), Some(category));
            // The 31 option strings are unique (the Java map would otherwise
            // silently drop an entry).
            let count = LintCategory::values()
                .iter()
                .filter(|other| other.option() == category.option())
                .count();
            assert_eq!(count, 1);
        }
        assert_eq!(LintCategory::get("unknown"), None);
        assert_eq!(LintCategory::get(""), None);
        assert_eq!(LintCategory::get("RAW"), None);
    }

    #[test]
    fn hidden_is_false_for_all_categories() {
        for category in LintCategory::values() {
            assert!(!category.hidden());
        }
    }

    #[test]
    fn names_are_the_enum_constant_names() {
        assert_eq!(LintCategory::AUXILIARYCLASS.name(), "AUXILIARYCLASS");
        assert_eq!(
            LintCategory::MISSING_EXPLICIT_CTOR.name(),
            "MISSING_EXPLICIT_CTOR"
        );
        assert_eq!(LintCategory::PREVIEW.to_string(), "PREVIEW");
    }

    #[test]
    fn is_enabled_and_is_suppressed_follow_the_sets() {
        let lint = make_lint(&[LintCategory::CAST, LintCategory::RAW], &[LintCategory::DEPRECATION]);
        assert!(lint.is_enabled(LintCategory::CAST));
        assert!(lint.is_enabled(LintCategory::RAW));
        assert!(!lint.is_enabled(LintCategory::DEPRECATION));
        assert!(lint.is_suppressed(LintCategory::DEPRECATION));
        assert!(!lint.is_suppressed(LintCategory::CAST));
    }

    #[test]
    fn suppress_returns_a_new_lint_and_keeps_the_original() {
        let original = make_lint(&[LintCategory::CAST, LintCategory::RAW], &[]);
        let suppressed = original.suppress(&[LintCategory::CAST]);
        // the original is untouched (Java copy constructor semantics)
        assert!(original.is_enabled(LintCategory::CAST));
        assert!(!original.is_suppressed(LintCategory::CAST));
        // the copy has the category suppressed and no longer enabled
        assert!(!suppressed.is_enabled(LintCategory::CAST));
        assert!(suppressed.is_suppressed(LintCategory::CAST));
        assert!(suppressed.is_enabled(LintCategory::RAW));
    }

    #[test]
    fn display_uses_the_enumset_format() {
        let lint = make_lint(
            &[LintCategory::CAST, LintCategory::RAW],
            &[LintCategory::DEPRECATION],
        );
        // EnumSet iteration order = enum declaration order (DEPRECATION < CAST < RAW).
        assert_eq!(
            lint.to_string(),
            "Lint:[values[CAST, RAW] suppressedValues[DEPRECATION]]"
        );
        assert_eq!(
            make_lint(&[], &[]).to_string(),
            "Lint:[values[] suppressedValues[]]"
        );
    }

    #[test]
    #[should_panic(expected = "TODO(Options batch)")]
    fn instance_needs_the_options_port() {
        let mut context = crate::util::context::Context::new();
        let _ = Lint::instance(&mut context);
    }
}
