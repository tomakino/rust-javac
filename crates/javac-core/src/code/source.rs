// Origin: com/sun/tools/javac/code/Source.java @ 982d53c (jdk-17.0.19-ga)

//! The source language version accepted.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * the Java enum constructor stores `name` per constant; Rust expresses it as
//!   a match-based accessor (same pattern as code/TypeTag).  Java enum `name()`
//!   (the constant name) is reproduced by Display, while the upstream `name`
//!   field is the version string ("1.2", "5", ...) returned by `name()`.
//! * `compareTo` is Rust's derived Ord (declaration order = version order); the
//!   disabled 1.0/1.1 constants stay disabled as upstream.
//! * `MIN`/`DEFAULT` are Rust associated constants (DEFAULT = MAX).
//! * the nested enum `Source.Feature` (and its nested DiagKind) is declared in
//!   this module (Rust enums cannot be nested in impl blocks, §3).  Its Java
//!   constructor data (minLevel/maxLevel/optFragment/optKind) becomes match-based
//!   accessors; the optFragment values are the typed resource factories of
//!   crate::resources::compiler_properties (Fragments.FeatureXxx) converted to
//!   JCDiagnostic.Fragment.
//! * Feature.fragment/error build the same resource keys as upstream
//!   (Fragments/Errors.FeatureNotSupportedInSource[Plural]).  The feature fragment
//!   passed as the first diagnostic argument is the current resources contract
//!   display-text representation (diagnostic_contract, ADR 0002 revision C: object
//!   preserving arguments land with P4), so it is passed by its diagnostic key.
//! * toSourceVersion -> Option<SourceVersion> (Java null -> None);
//!   javax.lang.model.SourceVersion is the javac-tools enum (RELEASE_2..RELEASE_17).
//! * TODO(Options/Option batch): `instance(Context)` uses the not yet ported
//!   Options.instance and Option.SOURCE ("--source -source"); left as a single
//!   unimplemented entry point (no stand-in for Options).

use std::fmt;
use std::rc::Rc;

use javac_tools::lang::model::SourceVersion;

use crate::jvm::target::Target;
use crate::resources::compiler_properties::{errors, fragments};
use crate::resources::diagnostic_contract::DiagnosticArg;
use crate::util::assert;
use crate::main::option::Option as MainOption;
use crate::util::context::Context;
use crate::util::options::Options;
use crate::util::java_string::JavaString;
use crate::util::jc_diagnostic::{Error, Fragment};

/// The source language version accepted.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    // 1.0 had no inner classes, and so could not pass the JCK:
    //     JDK1_0("1.0");
    // 1.1 did not have strictfp, and so could not pass the JCK:
    //     JDK1_1("1.1");

    /// 1.2 introduced strictfp.
    JDK1_2,
    /// 1.3 is the same language as 1.2.
    JDK1_3,
    /// 1.4 introduced assert.
    JDK1_4,
    /// 1.5 introduced generics, attributes, foreach, boxing, static import,
    /// covariant return, enums, varargs, et al.
    JDK5,
    /// 1.6 reports encoding problems as errors instead of warnings.
    JDK6,
    /// 1.7 introduced try-with-resources, multi-catch, string switch, etc.
    JDK7,
    /// 1.8 lambda expressions and default methods.
    JDK8,
    /// 1.9 modularity.
    JDK9,
    /// 1.10 local-variable type inference (var).
    JDK10,
    /// 1.11 local-variable syntax for lambda parameters
    JDK11,
    /// 12, no language features; switch expression in preview
    JDK12,
    /// 13, no language features; text blocks and revised switch
    /// expressions in preview
    JDK13,
    /// 14, switch expressions; pattern matching, records, and revised
    /// text blocks in preview
    JDK14,
    /// 15, text blocks
    JDK15,
    /// 16, tbd
    JDK16,
    /// 17, tbd
    JDK17,
}

impl Source {
    /// Java: public static final Source MIN = Source.JDK7.
    pub const MIN: Source = Source::JDK7;

    /// Java: public static final Source DEFAULT = MAX.
    pub const DEFAULT: Source = Source::JDK17;

    /// Java: Source.values().
    pub fn values() -> [Source; 16] {
        [
            Source::JDK1_2,
            Source::JDK1_3,
            Source::JDK1_4,
            Source::JDK5,
            Source::JDK6,
            Source::JDK7,
            Source::JDK8,
            Source::JDK9,
            Source::JDK10,
            Source::JDK11,
            Source::JDK12,
            Source::JDK13,
            Source::JDK14,
            Source::JDK15,
            Source::JDK16,
            Source::JDK17,
        ]
    }

    /// Java: public final String name (the version string, not the constant name).
    pub const fn name(self) -> &'static str {
        match self {
            Source::JDK1_2 => "1.2",
            Source::JDK1_3 => "1.3",
            Source::JDK1_4 => "1.4",
            Source::JDK5 => "5",
            Source::JDK6 => "6",
            Source::JDK7 => "7",
            Source::JDK8 => "8",
            Source::JDK9 => "9",
            Source::JDK10 => "10",
            Source::JDK11 => "11",
            Source::JDK12 => "12",
            Source::JDK13 => "13",
            Source::JDK14 => "14",
            Source::JDK15 => "15",
            Source::JDK16 => "16",
            Source::JDK17 => "17",
        }
    }

    /// Java: public static Source lookup(String name).
    ///
    /// PORTING_RULES: the upstream HashMap is filled from the enum values and
    /// then extended with the legacy 1.x aliases ("1.5".."1.10"); upstream
    /// deliberately does NOT alias "1.11" for 11.
    pub fn lookup(name: &str) -> Option<Source> {
        let source = match name {
            "1.2" => Source::JDK1_2,
            "1.3" => Source::JDK1_3,
            "1.4" => Source::JDK1_4,
            "5" => Source::JDK5,
            "6" => Source::JDK6,
            "7" => Source::JDK7,
            "8" => Source::JDK8,
            "9" => Source::JDK9,
            "10" => Source::JDK10,
            "11" => Source::JDK11,
            "12" => Source::JDK12,
            "13" => Source::JDK13,
            "14" => Source::JDK14,
            "15" => Source::JDK15,
            "16" => Source::JDK16,
            "17" => Source::JDK17,
            "1.5" => Source::JDK5,
            "1.6" => Source::JDK6,
            "1.7" => Source::JDK7,
            "1.8" => Source::JDK8,
            "1.9" => Source::JDK9,
            "1.10" => Source::JDK10,
            _ => return None,
        };
        Some(source)
    }

    /// Java: public boolean isSupported() { return this.compareTo(MIN) >= 0; }
    pub fn is_supported(self) -> bool {
        self >= Source::MIN
    }

    /// Java: public Target requiredTarget().
    pub fn required_target(self) -> Target {
        match self {
            Source::JDK17 => Target::JDK1_17,
            Source::JDK16 => Target::JDK1_16,
            Source::JDK15 => Target::JDK1_15,
            Source::JDK14 => Target::JDK1_14,
            Source::JDK13 => Target::JDK1_13,
            Source::JDK12 => Target::JDK1_12,
            Source::JDK11 => Target::JDK1_11,
            Source::JDK10 => Target::JDK1_10,
            Source::JDK9 => Target::JDK1_9,
            Source::JDK8 => Target::JDK1_8,
            Source::JDK7 => Target::JDK1_7,
            Source::JDK6 => Target::JDK1_6,
            Source::JDK5 => Target::JDK1_5,
            Source::JDK1_4 => Target::JDK1_4,
            // Java: default -> Target.JDK1_1 (also for JDK1_2/JDK1_3).
            _ => Target::JDK1_1,
        }
    }

    /// Java: public static SourceVersion toSourceVersion(Source source).
    pub fn to_source_version(source: Source) -> Option<SourceVersion> {
        match source {
            Source::JDK1_2 => Some(SourceVersion::RELEASE_2),
            Source::JDK1_3 => Some(SourceVersion::RELEASE_3),
            Source::JDK1_4 => Some(SourceVersion::RELEASE_4),
            Source::JDK5 => Some(SourceVersion::RELEASE_5),
            Source::JDK6 => Some(SourceVersion::RELEASE_6),
            Source::JDK7 => Some(SourceVersion::RELEASE_7),
            Source::JDK8 => Some(SourceVersion::RELEASE_8),
            Source::JDK9 => Some(SourceVersion::RELEASE_9),
            Source::JDK10 => Some(SourceVersion::RELEASE_10),
            Source::JDK11 => Some(SourceVersion::RELEASE_11),
            Source::JDK12 => Some(SourceVersion::RELEASE_12),
            Source::JDK13 => Some(SourceVersion::RELEASE_13),
            Source::JDK14 => Some(SourceVersion::RELEASE_14),
            Source::JDK15 => Some(SourceVersion::RELEASE_15),
            Source::JDK16 => Some(SourceVersion::RELEASE_16),
            Source::JDK17 => Some(SourceVersion::RELEASE_17),
        }
    }

    /// Java:
    /// <pre>
    /// public static Source instance(Context context) {
    ///     Source instance = context.get(sourceKey);
    ///     if (instance == null) {
    ///         Options options = Options.instance(context);
    ///         String sourceString = options.get(SOURCE);
    ///         if (sourceString != null) instance = lookup(sourceString);
    ///         if (instance == null) instance = DEFAULT;
    ///         context.put(sourceKey, instance);
    ///     }
    ///     return instance;
    /// }
    /// </pre>
    ///
    pub fn instance(context: &mut Context) -> Rc<Source> {
        if let Some(instance) = context.get::<Source>() {
            return instance;
        }
        let options = Options::instance(context);
        let source_string = options.get_option(MainOption::SOURCE);
        let source = source_string
            .as_deref()
            .and_then(Source::lookup)
            .unwrap_or(Source::DEFAULT);
        let instance = Rc::new(source);
        context.put(Rc::clone(&instance));
        instance
    }
}

impl fmt::Display for Source {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Models a feature of the Java programming language. Each feature can be associated with a
/// minimum source level, a maximum source level and a diagnostic fragment describing the feature,
/// which is used to generate error messages of the kind {@code feature XYZ not supported in source N}.
///
/// (Java: the nested enum {@code Source.Feature}; Rust enums cannot be nested in impl
/// blocks, so it is declared at module level and used as
/// {@code crate::code::source::Feature}.)
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Feature {
    DIAMOND,
    MODULES,
    EFFECTIVELY_FINAL_VARIABLES_IN_TRY_WITH_RESOURCES,
    DEPRECATION_ON_IMPORT,
    POLY,
    LAMBDA,
    METHOD_REFERENCES,
    DEFAULT_METHODS,
    STATIC_INTERFACE_METHODS,
    STATIC_INTERFACE_METHODS_INVOKE,
    STRICT_METHOD_CLASH_CHECK,
    EFFECTIVELY_FINAL_IN_INNER_CLASSES,
    TYPE_ANNOTATIONS,
    ANNOTATIONS_AFTER_TYPE_PARAMS,
    REPEATED_ANNOTATIONS,
    INTERSECTION_TYPES_IN_CAST,
    GRAPH_INFERENCE,
    FUNCTIONAL_INTERFACE_MOST_SPECIFIC,
    POST_APPLICABILITY_VARARGS_ACCESS_CHECK,
    MAP_CAPTURES_TO_BOUNDS,
    PRIVATE_SAFE_VARARGS,
    DIAMOND_WITH_ANONYMOUS_CLASS_CREATION,
    UNDERSCORE_IDENTIFIER,
    PRIVATE_INTERFACE_METHODS,
    LOCAL_VARIABLE_TYPE_INFERENCE,
    VAR_SYNTAX_IMPLICIT_LAMBDAS,
    IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES,
    SWITCH_MULTIPLE_CASE_LABELS,
    SWITCH_RULE,
    SWITCH_EXPRESSION,
    TEXT_BLOCKS,
    PATTERN_MATCHING_IN_INSTANCEOF,
    REIFIABLE_TYPES_INSTANCEOF,
    RECORDS,
    SEALED_CLASSES,
    CASE_NULL,
    PATTERN_SWITCH,
    REDUNDANT_STRICTFP,
}

/// Java: the nested enum {@code Source.Feature.DiagKind}.
///
/// PORTING_RULES: Java constant names are kept (NORMAL/PLURAL); no camel-case
/// renaming (§3), so the acronym lint is allowed here.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagKind {
    NORMAL,
    PLURAL,
}

impl Feature {
    /// Java: Feature.values().
    pub fn values() -> [Feature; 38] {
        [
            Feature::DIAMOND,
            Feature::MODULES,
            Feature::EFFECTIVELY_FINAL_VARIABLES_IN_TRY_WITH_RESOURCES,
            Feature::DEPRECATION_ON_IMPORT,
            Feature::POLY,
            Feature::LAMBDA,
            Feature::METHOD_REFERENCES,
            Feature::DEFAULT_METHODS,
            Feature::STATIC_INTERFACE_METHODS,
            Feature::STATIC_INTERFACE_METHODS_INVOKE,
            Feature::STRICT_METHOD_CLASH_CHECK,
            Feature::EFFECTIVELY_FINAL_IN_INNER_CLASSES,
            Feature::TYPE_ANNOTATIONS,
            Feature::ANNOTATIONS_AFTER_TYPE_PARAMS,
            Feature::REPEATED_ANNOTATIONS,
            Feature::INTERSECTION_TYPES_IN_CAST,
            Feature::GRAPH_INFERENCE,
            Feature::FUNCTIONAL_INTERFACE_MOST_SPECIFIC,
            Feature::POST_APPLICABILITY_VARARGS_ACCESS_CHECK,
            Feature::MAP_CAPTURES_TO_BOUNDS,
            Feature::PRIVATE_SAFE_VARARGS,
            Feature::DIAMOND_WITH_ANONYMOUS_CLASS_CREATION,
            Feature::UNDERSCORE_IDENTIFIER,
            Feature::PRIVATE_INTERFACE_METHODS,
            Feature::LOCAL_VARIABLE_TYPE_INFERENCE,
            Feature::VAR_SYNTAX_IMPLICIT_LAMBDAS,
            Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES,
            Feature::SWITCH_MULTIPLE_CASE_LABELS,
            Feature::SWITCH_RULE,
            Feature::SWITCH_EXPRESSION,
            Feature::TEXT_BLOCKS,
            Feature::PATTERN_MATCHING_IN_INSTANCEOF,
            Feature::REIFIABLE_TYPES_INSTANCEOF,
            Feature::RECORDS,
            Feature::SEALED_CLASSES,
            Feature::CASE_NULL,
            Feature::PATTERN_SWITCH,
            Feature::REDUNDANT_STRICTFP,
        ]
    }

    /// Java: private final Source minLevel.
    fn min_level(self) -> Source {
        match self {
            Feature::DIAMOND => Source::JDK7,
            Feature::MODULES => Source::JDK9,
            Feature::EFFECTIVELY_FINAL_VARIABLES_IN_TRY_WITH_RESOURCES => Source::JDK9,
            Feature::DEPRECATION_ON_IMPORT => Source::MIN,
            Feature::POLY => Source::JDK8,
            Feature::LAMBDA => Source::JDK8,
            Feature::METHOD_REFERENCES => Source::JDK8,
            Feature::DEFAULT_METHODS => Source::JDK8,
            Feature::STATIC_INTERFACE_METHODS => Source::JDK8,
            Feature::STATIC_INTERFACE_METHODS_INVOKE => Source::JDK8,
            Feature::STRICT_METHOD_CLASH_CHECK => Source::JDK8,
            Feature::EFFECTIVELY_FINAL_IN_INNER_CLASSES => Source::JDK8,
            Feature::TYPE_ANNOTATIONS => Source::JDK8,
            Feature::ANNOTATIONS_AFTER_TYPE_PARAMS => Source::JDK8,
            Feature::REPEATED_ANNOTATIONS => Source::JDK8,
            Feature::INTERSECTION_TYPES_IN_CAST => Source::JDK8,
            Feature::GRAPH_INFERENCE => Source::JDK8,
            Feature::FUNCTIONAL_INTERFACE_MOST_SPECIFIC => Source::JDK8,
            Feature::POST_APPLICABILITY_VARARGS_ACCESS_CHECK => Source::JDK8,
            Feature::MAP_CAPTURES_TO_BOUNDS => Source::MIN,
            Feature::PRIVATE_SAFE_VARARGS => Source::JDK9,
            Feature::DIAMOND_WITH_ANONYMOUS_CLASS_CREATION => Source::JDK9,
            Feature::UNDERSCORE_IDENTIFIER => Source::MIN,
            Feature::PRIVATE_INTERFACE_METHODS => Source::JDK9,
            Feature::LOCAL_VARIABLE_TYPE_INFERENCE => Source::JDK10,
            Feature::VAR_SYNTAX_IMPLICIT_LAMBDAS => Source::JDK11,
            Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES => Source::JDK1_2,
            Feature::SWITCH_MULTIPLE_CASE_LABELS => Source::JDK14,
            Feature::SWITCH_RULE => Source::JDK14,
            Feature::SWITCH_EXPRESSION => Source::JDK14,
            Feature::TEXT_BLOCKS => Source::JDK15,
            Feature::PATTERN_MATCHING_IN_INSTANCEOF => Source::JDK16,
            Feature::REIFIABLE_TYPES_INSTANCEOF => Source::JDK16,
            Feature::RECORDS => Source::JDK16,
            Feature::SEALED_CLASSES => Source::JDK17,
            Feature::CASE_NULL => Source::JDK17,
            Feature::PATTERN_SWITCH => Source::JDK17,
            Feature::REDUNDANT_STRICTFP => Source::JDK17,
        }
    }

    /// Java: private final Source maxLevel.
    fn max_level(self) -> Source {
        match self {
            Feature::DEPRECATION_ON_IMPORT
            | Feature::UNDERSCORE_IDENTIFIER
            | Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES => Source::JDK8,
            Feature::MAP_CAPTURES_TO_BOUNDS => Source::JDK7,
            // Java: this(minLevel, MAX, ...) with MAX = the last enum constant.
            _ => Source::JDK17,
        }
    }

    /// Java: private final Fragment optFragment.
    fn opt_fragment(self) -> Option<Fragment> {
        let info = match self {
            Feature::DIAMOND => Some(fragments::feature_diamond()),
            Feature::MODULES => Some(fragments::feature_modules()),
            Feature::EFFECTIVELY_FINAL_VARIABLES_IN_TRY_WITH_RESOURCES => {
                Some(fragments::feature_var_in_try_with_resources())
            }
            Feature::LAMBDA => Some(fragments::feature_lambda()),
            Feature::METHOD_REFERENCES => Some(fragments::feature_method_references()),
            Feature::DEFAULT_METHODS => Some(fragments::feature_default_methods()),
            Feature::STATIC_INTERFACE_METHODS => Some(fragments::feature_static_intf_methods()),
            Feature::STATIC_INTERFACE_METHODS_INVOKE => {
                Some(fragments::feature_static_intf_method_invoke())
            }
            Feature::TYPE_ANNOTATIONS => Some(fragments::feature_type_annotations()),
            Feature::ANNOTATIONS_AFTER_TYPE_PARAMS => {
                Some(fragments::feature_annotations_after_type_params())
            }
            Feature::REPEATED_ANNOTATIONS => Some(fragments::feature_repeatable_annotations()),
            Feature::INTERSECTION_TYPES_IN_CAST => {
                Some(fragments::feature_intersection_types_in_cast())
            }
            Feature::DIAMOND_WITH_ANONYMOUS_CLASS_CREATION => {
                Some(fragments::feature_diamond_and_anon_class())
            }
            Feature::PRIVATE_INTERFACE_METHODS => Some(fragments::feature_private_intf_methods()),
            Feature::VAR_SYNTAX_IMPLICIT_LAMBDAS => {
                Some(fragments::feature_var_syntax_in_implicit_lambda())
            }
            Feature::SWITCH_MULTIPLE_CASE_LABELS => {
                Some(fragments::feature_multiple_case_labels())
            }
            Feature::SWITCH_RULE => Some(fragments::feature_switch_rules()),
            Feature::SWITCH_EXPRESSION => Some(fragments::feature_switch_expressions()),
            Feature::TEXT_BLOCKS => Some(fragments::feature_text_blocks()),
            Feature::PATTERN_MATCHING_IN_INSTANCEOF => {
                Some(fragments::feature_pattern_matching_instanceof())
            }
            Feature::REIFIABLE_TYPES_INSTANCEOF => {
                Some(fragments::feature_reifiable_types_instanceof())
            }
            Feature::RECORDS => Some(fragments::feature_records()),
            Feature::SEALED_CLASSES => Some(fragments::feature_sealed_classes()),
            Feature::CASE_NULL => Some(fragments::feature_case_null()),
            Feature::PATTERN_SWITCH => Some(fragments::feature_pattern_switch()),
            Feature::DEPRECATION_ON_IMPORT
            | Feature::POLY
            | Feature::STRICT_METHOD_CLASH_CHECK
            | Feature::EFFECTIVELY_FINAL_IN_INNER_CLASSES
            | Feature::GRAPH_INFERENCE
            | Feature::FUNCTIONAL_INTERFACE_MOST_SPECIFIC
            | Feature::POST_APPLICABILITY_VARARGS_ACCESS_CHECK
            | Feature::MAP_CAPTURES_TO_BOUNDS
            | Feature::PRIVATE_SAFE_VARARGS
            | Feature::UNDERSCORE_IDENTIFIER
            | Feature::LOCAL_VARIABLE_TYPE_INFERENCE
            | Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES
            | Feature::REDUNDANT_STRICTFP => None,
        };
        info.map(Fragment::from)
    }

    /// Java: private final DiagKind optKind.
    fn opt_kind(self) -> Option<DiagKind> {
        match self {
            Feature::DIAMOND
            | Feature::DIAMOND_WITH_ANONYMOUS_CLASS_CREATION
            | Feature::PATTERN_MATCHING_IN_INSTANCEOF
            | Feature::CASE_NULL => Some(DiagKind::NORMAL),
            Feature::MODULES
            | Feature::EFFECTIVELY_FINAL_VARIABLES_IN_TRY_WITH_RESOURCES
            | Feature::LAMBDA
            | Feature::METHOD_REFERENCES
            | Feature::DEFAULT_METHODS
            | Feature::STATIC_INTERFACE_METHODS
            | Feature::STATIC_INTERFACE_METHODS_INVOKE
            | Feature::TYPE_ANNOTATIONS
            | Feature::ANNOTATIONS_AFTER_TYPE_PARAMS
            | Feature::REPEATED_ANNOTATIONS
            | Feature::INTERSECTION_TYPES_IN_CAST
            | Feature::PRIVATE_INTERFACE_METHODS
            | Feature::VAR_SYNTAX_IMPLICIT_LAMBDAS
            | Feature::SWITCH_MULTIPLE_CASE_LABELS
            | Feature::SWITCH_RULE
            | Feature::SWITCH_EXPRESSION
            | Feature::TEXT_BLOCKS
            | Feature::REIFIABLE_TYPES_INSTANCEOF
            | Feature::RECORDS
            | Feature::SEALED_CLASSES
            | Feature::PATTERN_SWITCH => Some(DiagKind::PLURAL),
            Feature::DEPRECATION_ON_IMPORT
            | Feature::POLY
            | Feature::STRICT_METHOD_CLASH_CHECK
            | Feature::EFFECTIVELY_FINAL_IN_INNER_CLASSES
            | Feature::GRAPH_INFERENCE
            | Feature::FUNCTIONAL_INTERFACE_MOST_SPECIFIC
            | Feature::POST_APPLICABILITY_VARARGS_ACCESS_CHECK
            | Feature::MAP_CAPTURES_TO_BOUNDS
            | Feature::PRIVATE_SAFE_VARARGS
            | Feature::UNDERSCORE_IDENTIFIER
            | Feature::LOCAL_VARIABLE_TYPE_INFERENCE
            | Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES
            | Feature::REDUNDANT_STRICTFP => None,
        }
    }

    /// Java: public boolean allowedInSource(Source source).
    pub fn allowed_in_source(self, source: Source) -> bool {
        source >= self.min_level() && source <= self.max_level()
    }

    /// Java: public boolean isPlural().
    pub fn is_plural(self) -> bool {
        let kind = assert::check_non_null(self.opt_kind());
        kind == DiagKind::PLURAL
    }

    /// Java: public Fragment nameFragment().
    pub fn name_fragment(self) -> Fragment {
        assert::check_non_null(self.opt_fragment())
    }

    /// Java: public Fragment fragment(String sourceName).
    pub fn fragment(self, source_name: &str) -> Fragment {
        let opt_fragment = assert::check_non_null(self.opt_fragment());
        let arg = fragment_arg(&opt_fragment);
        let source = JavaString::from(source_name);
        let min = JavaString::from(self.min_level().name());
        if self.opt_kind() == Some(DiagKind::NORMAL) {
            fragments::feature_not_supported_in_source(arg, source, min).into()
        } else {
            fragments::feature_not_supported_in_source_plural(arg, source, min).into()
        }
    }

    /// Java: public Error error(String sourceName).
    pub fn error(self, source_name: &str) -> Error {
        let opt_fragment = assert::check_non_null(self.opt_fragment());
        let arg = fragment_arg(&opt_fragment);
        let source = JavaString::from(source_name);
        let min = JavaString::from(self.min_level().name());
        if self.opt_kind() == Some(DiagKind::NORMAL) {
            errors::feature_not_supported_in_source(arg, source, min).into()
        } else {
            errors::feature_not_supported_in_source_plural(arg, source, min).into()
        }
    }
}

impl fmt::Display for Feature {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// ADR 0002 revision C temporary representation: the current resources contract
/// stores diagnostic arguments as display text, so the nested feature fragment is
/// passed by its resource key until the object-preserving P4 contract lands.
fn fragment_arg(fragment: &Fragment) -> DiagnosticArg {
    DiagnosticArg(fragment.info().key())
}

#[cfg(test)]
mod tests {
    use super::{Feature, Source};
    use crate::jvm::target::Target;
    use crate::util::java_string::JavaString;
    use crate::util::jc_diagnostic::DiagnosticType;
    use javac_tools::lang::model::SourceVersion;

    fn arg_text(arg: &crate::util::jc_diagnostic::DiagnosticArg) -> String {
        arg.downcast_ref::<JavaString>()
            .unwrap_or_else(|| panic!("diagnostic argument is not a JavaString"))
            .to_string_lossy()
    }

    #[test]
    fn values_are_complete_and_in_declaration_order() {
        let values = Source::values();
        assert_eq!(values.len(), 16);
        assert_eq!(values[0], Source::JDK1_2);
        assert_eq!(values[1], Source::JDK1_3);
        assert_eq!(values[values.len() - 1], Source::JDK17);
        assert_eq!(Source::MIN, Source::JDK7);
        assert_eq!(Source::DEFAULT, Source::JDK17);
    }

    #[test]
    fn names_match_the_version_strings() {
        assert_eq!(Source::JDK1_2.name(), "1.2");
        assert_eq!(Source::JDK5.name(), "5");
        assert_eq!(Source::JDK8.name(), "8");
        assert_eq!(Source::JDK17.name(), "17");
    }

    #[test]
    fn lookup_includes_the_legacy_aliases_but_not_1_11() {
        assert_eq!(Source::lookup("1.2"), Some(Source::JDK1_2));
        assert_eq!(Source::lookup("5"), Some(Source::JDK5));
        assert_eq!(Source::lookup("17"), Some(Source::JDK17));
        assert_eq!(Source::lookup("1.5"), Some(Source::JDK5));
        assert_eq!(Source::lookup("1.10"), Some(Source::JDK10));
        assert_eq!(Source::lookup("1.11"), None);
        assert_eq!(Source::lookup("1.1"), None);
        assert_eq!(Source::lookup("18"), None);
    }

    #[test]
    fn support_status_follows_min() {
        assert!(!Source::JDK6.is_supported());
        assert!(Source::JDK7.is_supported());
        assert!(Source::JDK17.is_supported());
    }

    #[test]
    fn required_target_maps_each_release() {
        assert_eq!(Source::JDK1_2.required_target(), Target::JDK1_1);
        assert_eq!(Source::JDK1_3.required_target(), Target::JDK1_1);
        assert_eq!(Source::JDK1_4.required_target(), Target::JDK1_4);
        assert_eq!(Source::JDK5.required_target(), Target::JDK1_5);
        assert_eq!(Source::JDK8.required_target(), Target::JDK1_8);
        assert_eq!(Source::JDK17.required_target(), Target::JDK1_17);
    }

    #[test]
    fn to_source_version_maps_all_releases() {
        assert_eq!(
            Source::to_source_version(Source::JDK1_2),
            Some(SourceVersion::RELEASE_2)
        );
        assert_eq!(
            Source::to_source_version(Source::JDK5),
            Some(SourceVersion::RELEASE_5)
        );
        assert_eq!(
            Source::to_source_version(Source::JDK8),
            Some(SourceVersion::RELEASE_8)
        );
        assert_eq!(
            Source::to_source_version(Source::JDK17),
            Some(SourceVersion::RELEASE_17)
        );
    }

    #[test]
    fn features_are_complete_and_in_declaration_order() {
        // The complete upstream Source.Feature list, in declaration order: the
        // assertion covers the variant names, the count and the order.
        let names: Vec<String> = Feature::values()
            .iter()
            .map(|feature| format!("{feature:?}"))
            .collect();
        assert_eq!(
            names,
            vec![
                "DIAMOND",
                "MODULES",
                "EFFECTIVELY_FINAL_VARIABLES_IN_TRY_WITH_RESOURCES",
                "DEPRECATION_ON_IMPORT",
                "POLY",
                "LAMBDA",
                "METHOD_REFERENCES",
                "DEFAULT_METHODS",
                "STATIC_INTERFACE_METHODS",
                "STATIC_INTERFACE_METHODS_INVOKE",
                "STRICT_METHOD_CLASH_CHECK",
                "EFFECTIVELY_FINAL_IN_INNER_CLASSES",
                "TYPE_ANNOTATIONS",
                "ANNOTATIONS_AFTER_TYPE_PARAMS",
                "REPEATED_ANNOTATIONS",
                "INTERSECTION_TYPES_IN_CAST",
                "GRAPH_INFERENCE",
                "FUNCTIONAL_INTERFACE_MOST_SPECIFIC",
                "POST_APPLICABILITY_VARARGS_ACCESS_CHECK",
                "MAP_CAPTURES_TO_BOUNDS",
                "PRIVATE_SAFE_VARARGS",
                "DIAMOND_WITH_ANONYMOUS_CLASS_CREATION",
                "UNDERSCORE_IDENTIFIER",
                "PRIVATE_INTERFACE_METHODS",
                "LOCAL_VARIABLE_TYPE_INFERENCE",
                "VAR_SYNTAX_IMPLICIT_LAMBDAS",
                "IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES",
                "SWITCH_MULTIPLE_CASE_LABELS",
                "SWITCH_RULE",
                "SWITCH_EXPRESSION",
                "TEXT_BLOCKS",
                "PATTERN_MATCHING_IN_INSTANCEOF",
                "REIFIABLE_TYPES_INSTANCEOF",
                "RECORDS",
                "SEALED_CLASSES",
                "CASE_NULL",
                "PATTERN_SWITCH",
                "REDUNDANT_STRICTFP",
            ]
        );
    }

    #[test]
    fn allowed_in_source_respects_the_minimum_level() {
        assert!(!Feature::DIAMOND.allowed_in_source(Source::JDK6));
        assert!(Feature::DIAMOND.allowed_in_source(Source::JDK7));
        assert!(Feature::DIAMOND.allowed_in_source(Source::JDK17));
        assert!(!Feature::TEXT_BLOCKS.allowed_in_source(Source::JDK14));
        assert!(Feature::TEXT_BLOCKS.allowed_in_source(Source::JDK15));
        assert!(!Feature::SEALED_CLASSES.allowed_in_source(Source::JDK16));
        assert!(Feature::SEALED_CLASSES.allowed_in_source(Source::JDK17));
    }

    #[test]
    fn allowed_in_source_respects_the_maximum_level() {
        // MAP_CAPTURES_TO_BOUNDS: min = MIN (7), max = 7.
        assert!(Feature::MAP_CAPTURES_TO_BOUNDS.allowed_in_source(Source::JDK7));
        assert!(!Feature::MAP_CAPTURES_TO_BOUNDS.allowed_in_source(Source::JDK8));
        // DEPRECATION_ON_IMPORT / UNDERSCORE_IDENTIFIER / IMPORT_ON_DEMAND...:
        // min = 7, max = 8 (and IMPORT_ON_DEMAND's min is 1.2).
        assert!(Feature::DEPRECATION_ON_IMPORT.allowed_in_source(Source::JDK8));
        assert!(!Feature::DEPRECATION_ON_IMPORT.allowed_in_source(Source::JDK9));
        assert!(Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES.allowed_in_source(Source::JDK1_2));
        assert!(Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES.allowed_in_source(Source::JDK8));
        assert!(!Feature::IMPORT_ON_DEMAND_OBSERVABLE_PACKAGES.allowed_in_source(Source::JDK9));
    }

    #[test]
    fn is_plural_follows_the_diag_kind() {
        assert!(Feature::MODULES.is_plural());
        assert!(Feature::LAMBDA.is_plural());
        assert!(Feature::PATTERN_SWITCH.is_plural());
        assert!(!Feature::DIAMOND.is_plural());
        assert!(!Feature::CASE_NULL.is_plural());
    }

    #[test]
    #[should_panic]
    fn is_plural_requires_a_diag_kind() {
        // Java: Assert.checkNonNull(optKind) -> AssertionError.
        let _ = Feature::POLY.is_plural();
    }

    #[test]
    fn name_fragment_returns_the_feature_fragment() {
        assert_eq!(
            Feature::DIAMOND.name_fragment().info().get_code(),
            "feature.diamond"
        );
        assert_eq!(
            Feature::SEALED_CLASSES.name_fragment().info().get_code(),
            "feature.sealed.classes"
        );
    }

    #[test]
    #[should_panic]
    fn name_fragment_requires_a_fragment() {
        // Java: Assert.checkNonNull(optFragment) -> AssertionError.
        let _ = Feature::POLY.name_fragment();
    }

    #[test]
    fn error_builds_the_normal_feature_diagnostic() {
        let error = Feature::DIAMOND.error("8");
        let info = error.info();
        assert_eq!(info.get_type(), DiagnosticType::ERROR);
        assert_eq!(info.get_code(), "feature.not.supported.in.source");
        let args = info.get_args();
        assert_eq!(args.len(), 3);
        assert_eq!(arg_text(&args[0]), "compiler.misc.feature.diamond");
        assert_eq!(arg_text(&args[1]), "8");
        assert_eq!(arg_text(&args[2]), "7");
    }

    #[test]
    fn error_builds_the_plural_feature_diagnostic() {
        let error = Feature::MODULES.error("8");
        let info = error.info();
        assert_eq!(info.get_code(), "feature.not.supported.in.source.plural");
        let args = info.get_args();
        assert_eq!(arg_text(&args[0]), "compiler.misc.feature.modules");
        assert_eq!(arg_text(&args[2]), "9");
    }

    #[test]
    fn fragment_builds_the_fragment_key_behind_the_error() {
        let fragment = Feature::DIAMOND.fragment("8");
        assert_eq!(fragment.info().get_type(), DiagnosticType::FRAGMENT);
        assert_eq!(fragment.info().get_code(), "feature.not.supported.in.source");
        assert_eq!(Feature::MODULES.fragment("8").info().get_code(), "feature.not.supported.in.source.plural");
    }

    #[test]
    fn display_reproduces_the_enum_constant_name() {
        assert_eq!(Source::JDK8.to_string(), "JDK8");
        assert_eq!(Feature::TEXT_BLOCKS.to_string(), "TEXT_BLOCKS");
    }

    #[test]
    fn instance_is_cached_and_defaults_to_the_latest_source() {
        let mut context = crate::util::context::Context::new();
        let first = Source::instance(&mut context);
        let second = Source::instance(&mut context);
        assert!(std::rc::Rc::ptr_eq(&first, &second));
        assert_eq!(*first, Source::DEFAULT);
        assert_eq!(*first, Source::JDK17);
    }
}
