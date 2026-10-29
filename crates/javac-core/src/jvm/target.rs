// Origin: com/sun/tools/javac/jvm/Target.java @ 982d53c (jdk-17.0.19-ga)

//! The classfile version target.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * the Java enum constructor stores (name, majorVersion, minorVersion) per
//!   constant; Rust expresses those as match-based accessors (same pattern as
//!   code/TypeTag).  Java enum `name()` (the constant name) is reproduced by
//!   Display (the derived Debug name), while the upstream `name` field is the
//!   version string ("1.1", "5", ...) returned by `name()`.
//! * `compareTo` is Rust's derived Ord (declaration order = version order).
//! * REGISTERED(Options/Option): `instance(Context)` uses the not yet ported
//!   Options/Options.instance and Option.TARGET ("--target -target"); it is left
//!   as a single unimplemented entry point (no stand-in for Options).
//!   All other methods are ported and tested.

use crate::util::context::Context;
use crate::util::java_string::JavaString;
use std::fmt;
use std::rc::Rc;

/// The classfile version target.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    JDK1_1,
    JDK1_2,
    JDK1_3,
    /// J2SE1.4 = Merlin.
    JDK1_4,
    /// JDK 5, codename Tiger.
    JDK1_5,
    /// JDK 6.
    JDK1_6,
    /// JDK 7.
    JDK1_7,
    /// JDK 8.
    JDK1_8,
    /// JDK 9.
    JDK1_9,
    /// JDK 10.
    JDK1_10,
    /// JDK 11.
    JDK1_11,
    /// JDK 12.
    JDK1_12,
    /// JDK 13.
    JDK1_13,
    /// JDK 14.
    JDK1_14,
    /// JDK 15.
    JDK1_15,
    /// JDK 16.
    JDK1_16,
    /// JDK 17.
    JDK1_17,
}

impl Target {
    /// Java: Target.MIN.
    pub const MIN: Target = Target::JDK1_7;

    /// Java: Target.DEFAULT (= the last enum constant, MAX).
    pub const DEFAULT: Target = Target::JDK1_17;

    /// Java: Target.values().
    pub fn values() -> [Target; 17] {
        [
            Target::JDK1_1,
            Target::JDK1_2,
            Target::JDK1_3,
            Target::JDK1_4,
            Target::JDK1_5,
            Target::JDK1_6,
            Target::JDK1_7,
            Target::JDK1_8,
            Target::JDK1_9,
            Target::JDK1_10,
            Target::JDK1_11,
            Target::JDK1_12,
            Target::JDK1_13,
            Target::JDK1_14,
            Target::JDK1_15,
            Target::JDK1_16,
            Target::JDK1_17,
        ]
    }

    /// Java: public final String name (the version string, not the constant name).
    pub const fn name(self) -> &'static str {
        match self {
            Target::JDK1_1 => "1.1",
            Target::JDK1_2 => "1.2",
            Target::JDK1_3 => "1.3",
            Target::JDK1_4 => "1.4",
            Target::JDK1_5 => "5",
            Target::JDK1_6 => "6",
            Target::JDK1_7 => "7",
            Target::JDK1_8 => "8",
            Target::JDK1_9 => "9",
            Target::JDK1_10 => "10",
            Target::JDK1_11 => "11",
            Target::JDK1_12 => "12",
            Target::JDK1_13 => "13",
            Target::JDK1_14 => "14",
            Target::JDK1_15 => "15",
            Target::JDK1_16 => "16",
            Target::JDK1_17 => "17",
        }
    }

    /// Java: public final int majorVersion.
    pub const fn major_version(self) -> i32 {
        match self {
            Target::JDK1_1 => 45,
            Target::JDK1_2 => 46,
            Target::JDK1_3 => 47,
            Target::JDK1_4 => 48,
            Target::JDK1_5 => 49,
            Target::JDK1_6 => 50,
            Target::JDK1_7 => 51,
            Target::JDK1_8 => 52,
            Target::JDK1_9 => 53,
            Target::JDK1_10 => 54,
            Target::JDK1_11 => 55,
            Target::JDK1_12 => 56,
            Target::JDK1_13 => 57,
            Target::JDK1_14 => 58,
            Target::JDK1_15 => 59,
            Target::JDK1_16 => 60,
            Target::JDK1_17 => 61,
        }
    }

    /// Java: public final int minorVersion.
    pub const fn minor_version(self) -> i32 {
        match self {
            // 1.1 was the only release whose class file format had a non-zero
            // minor version (3: the "new" class file format).
            Target::JDK1_1 => 3,
            _ => 0,
        }
    }

    /// Java: enum#ordinal().
    pub const fn ordinal(self) -> i32 {
        // Rust fieldless enums are numbered from 0 in declaration order, like
        // Java enum ordinals.
        self as i32
    }

    /// Java: public static Target lookup(String name).
    pub fn lookup(name: &str) -> Option<Target> {
        let target = match name {
            "1.1" => Target::JDK1_1,
            "1.2" => Target::JDK1_2,
            "1.3" => Target::JDK1_3,
            "1.4" => Target::JDK1_4,
            "5" => Target::JDK1_5,
            "6" => Target::JDK1_6,
            "7" => Target::JDK1_7,
            "8" => Target::JDK1_8,
            "9" => Target::JDK1_9,
            "10" => Target::JDK1_10,
            "11" => Target::JDK1_11,
            "12" => Target::JDK1_12,
            "13" => Target::JDK1_13,
            "14" => Target::JDK1_14,
            "15" => Target::JDK1_15,
            "16" => Target::JDK1_16,
            "17" => Target::JDK1_17,
            // Legacy aliases (Java keeps a HashMap with these extra entries).
            "1.5" => Target::JDK1_5,
            "1.6" => Target::JDK1_6,
            "1.7" => Target::JDK1_7,
            "1.8" => Target::JDK1_8,
            "1.9" => Target::JDK1_9,
            "1.10" => Target::JDK1_10,
            _ => return None,
        };
        Some(target)
    }

    /// Java: public boolean isSupported() { return this.compareTo(MIN) >= 0; }
    pub fn is_supported(self) -> bool {
        self >= Target::MIN
    }

    /// Return the character to be used in constructing synthetic
    /// identifiers, where not specified by the JLS.
    ///
    /// Java: public char syntheticNameChar()
    pub fn synthetic_name_char(self) -> u16 {
        '$' as u16
    }

    /// Does the target VM expect MethodParameters attributes?
    pub fn has_method_parameters(self) -> bool {
        self >= Target::JDK1_8
    }

    /// Does the target JDK contain StringConcatFactory class?
    pub fn has_string_concat_factory(self) -> bool {
        self >= Target::JDK1_9
    }

    /// Value of platform release used to access multi-release jar files.
    ///
    /// PORTING_RULES: Java String -> JavaString (Porting rules).
    pub fn multi_release_value(self) -> JavaString {
        let value = self.ordinal() - Target::JDK1_1.ordinal() + 1;
        JavaString::from(value.to_string().as_str())
    }

    /// All modules that export an API are roots when compiling code in the unnamed
    /// module and targeting 11 or newer.
    pub fn all_api_modules_are_roots(self) -> bool {
        self >= Target::JDK1_11
    }

    /// Does the target VM support nestmate access?
    pub fn has_nestmate_access(self) -> bool {
        self >= Target::JDK1_11
    }

    /// language runtime uses nest-based access.
    /// e.g. lambda and string concat spin dynamic proxy class as a nestmate
    /// of the target class
    pub fn runtime_use_nest_access(self) -> bool {
        self >= Target::JDK1_15
    }

    /// Does the target VM support virtual private invocations?
    pub fn has_virtual_private_invoke(self) -> bool {
        self >= Target::JDK1_11
    }

    /// Does the target VM support sealed types
    pub fn has_sealed_classes(self) -> bool {
        self >= Target::JDK1_15
    }

    /// Is the ACC_STRICT bit redundant and obsolete
    pub fn obsolete_acc_strict(self) -> bool {
        self >= Target::JDK1_17
    }

    /// Java:
    /// <pre>
    /// public static Target instance(Context context) {
    ///     Target instance = context.get(targetKey);
    ///     if (instance == null) {
    ///         Options options = Options.instance(context);
    ///         String targetString = options.get(TARGET);
    ///         if (targetString != null) instance = lookup(targetString);
    ///         if (instance == null) instance = DEFAULT;
    ///         context.put(targetKey, instance);
    ///     }
    ///     return instance;
    /// }
    /// </pre>
    ///
    /// REGISTERED(Options/Option): Options.instance/options.get and
    /// Option.TARGET ("--target -target") are not ported; the entry point stays
    /// unimplemented (no substitute Options is invented).
    pub fn instance(context: &mut Context) -> Rc<Target> {
        if let Some(instance) = context.get::<Target>() {
            return instance;
        }
        let options = crate::util::options::Options::instance(context);
        let value = options
            .get_option(crate::main::option::Option::TARGET)
            .and_then(|target_string| Target::lookup(&target_string))
            .unwrap_or(Target::DEFAULT);
        let instance = Rc::new(value);
        context.put(Rc::clone(&instance));
        instance
    }
}

impl fmt::Display for Target {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::Target;

    #[test]
    fn values_are_complete_and_in_declaration_order() {
        let values = Target::values();
        assert_eq!(values.len(), 17);
        assert_eq!(values[0], Target::JDK1_1);
        assert_eq!(values[1], Target::JDK1_2);
        assert_eq!(values[values.len() - 1], Target::JDK1_17);
        assert_eq!(Target::MIN, Target::JDK1_7);
        assert_eq!(Target::DEFAULT, Target::JDK1_17);
    }

    #[test]
    fn names_majors_and_minors_match_the_classfile_versions() {
        assert_eq!(Target::JDK1_1.name(), "1.1");
        assert_eq!(Target::JDK1_1.major_version(), 45);
        assert_eq!(Target::JDK1_1.minor_version(), 3);
        assert_eq!(Target::JDK1_8.name(), "8");
        assert_eq!(Target::JDK1_8.major_version(), 52);
        assert_eq!(Target::JDK1_8.minor_version(), 0);
        assert_eq!(Target::JDK1_17.name(), "17");
        assert_eq!(Target::JDK1_17.major_version(), 61);
        assert_eq!(Target::JDK1_17.minor_version(), 0);
        for (index, target) in Target::values().iter().enumerate() {
            assert_eq!(target.ordinal(), index as i32);
            assert_eq!(target.major_version(), 45 + index as i32);
        }
    }

    #[test]
    fn lookup_accepts_names_and_legacy_aliases() {
        assert_eq!(Target::lookup("1.1"), Some(Target::JDK1_1));
        assert_eq!(Target::lookup("5"), Some(Target::JDK1_5));
        assert_eq!(Target::lookup("17"), Some(Target::JDK1_17));
        assert_eq!(Target::lookup("1.5"), Some(Target::JDK1_5));
        assert_eq!(Target::lookup("1.10"), Some(Target::JDK1_10));
        assert_eq!(Target::lookup("1.11"), None);
        assert_eq!(Target::lookup("18"), None);
        assert_eq!(Target::lookup(""), None);
    }

    #[test]
    fn support_status_follows_min() {
        assert!(!Target::JDK1_6.is_supported());
        assert!(Target::JDK1_7.is_supported());
        assert!(Target::JDK1_17.is_supported());
    }

    #[test]
    fn target_properties_switch_at_the_documented_versions() {
        assert_eq!(Target::JDK1_8.synthetic_name_char(), '$' as u16);
        assert!(!Target::JDK1_7.has_method_parameters());
        assert!(Target::JDK1_8.has_method_parameters());
        assert!(!Target::JDK1_8.has_string_concat_factory());
        assert!(Target::JDK1_9.has_string_concat_factory());
        assert!(!Target::JDK1_10.all_api_modules_are_roots());
        assert!(Target::JDK1_11.all_api_modules_are_roots());
        assert!(!Target::JDK1_10.has_nestmate_access());
        assert!(Target::JDK1_11.has_nestmate_access());
        assert!(!Target::JDK1_14.runtime_use_nest_access());
        assert!(Target::JDK1_15.runtime_use_nest_access());
        assert!(!Target::JDK1_10.has_virtual_private_invoke());
        assert!(Target::JDK1_11.has_virtual_private_invoke());
        assert!(!Target::JDK1_14.has_sealed_classes());
        assert!(Target::JDK1_15.has_sealed_classes());
        assert!(!Target::JDK1_16.obsolete_acc_strict());
        assert!(Target::JDK1_17.obsolete_acc_strict());
    }

    #[test]
    fn multi_release_value_is_the_release_number() {
        assert_eq!(Target::JDK1_1.multi_release_value().to_string_lossy(), "1");
        assert_eq!(Target::JDK1_8.multi_release_value().to_string_lossy(), "8");
        assert_eq!(Target::JDK1_17.multi_release_value().to_string_lossy(), "17");
    }

    #[test]
    fn display_reproduces_the_enum_constant_name() {
        assert_eq!(Target::JDK1_8.to_string(), "JDK1_8");
        assert_eq!(Target::JDK1_17.to_string(), "JDK1_17");
    }

    #[test]
    fn instance_reads_the_target_option_and_caches_the_value() {
        // Java: Target.instance reads Option.TARGET and caches the instance.
        let mut context = crate::util::context::Context::new();
        {
            let options = crate::util::options::Options::instance(&mut context);
            options.put_option(crate::main::option::Option::TARGET, "8");
        }
        let target = Target::instance(&mut context);
        assert_eq!(*target, Target::JDK1_8);
        let cached = Target::instance(&mut context);
        assert!(std::rc::Rc::ptr_eq(&target, &cached));
    }
}
