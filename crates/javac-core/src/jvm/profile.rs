// Origin: com/sun/tools/javac/jvm/Profile.java @ 982d53c (jdk-17.0.19-ga)

//! The target profile.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * the Java enum constructor stores (name, value, targets); the DEFAULT
//!   constant overrides isValid.  Rust expresses the constructor data as
//!   match-based accessors and the isValid override as the DEFAULT arm.
//! * `String name` is null for DEFAULT -> Option<&'static str>; `Set<Target>`
//!   -> a one-element slice (#14 Set literal mapping; the only upstream use is
//!   EnumSet.of(t), so the set is represented as a fixed array).
//! * the Java overloads lookup(String)/lookup(int) become lookup and
//!   lookup_value (§3 overloads).
//! * TODO(Options/Option batch): instance(Context) uses the not yet ported
//!   Options.instance and Option.PROFILE ("-profile"); left unimplemented
//!   (no stand-in for Options).

use crate::jvm::target::Target;
use crate::util::context::Context;
use std::fmt;
use std::rc::Rc;

/// The target profile.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Profile {
    COMPACT1,
    COMPACT2,
    COMPACT3,
    DEFAULT,
}

impl Profile {
    /// Java: Profile.values().
    pub fn values() -> [Profile; 4] {
        [
            Profile::COMPACT1,
            Profile::COMPACT2,
            Profile::COMPACT3,
            Profile::DEFAULT,
        ]
    }

    /// Java: public final String name (null for DEFAULT -> None).
    pub fn name(self) -> Option<&'static str> {
        match self {
            Profile::COMPACT1 => Some("compact1"),
            Profile::COMPACT2 => Some("compact2"),
            Profile::COMPACT3 => Some("compact3"),
            Profile::DEFAULT => None,
        }
    }

    /// Java: public final int value (Integer.MAX_VALUE for DEFAULT).
    pub const fn value(self) -> i32 {
        match self {
            Profile::COMPACT1 => 1,
            Profile::COMPACT2 => 2,
            Profile::COMPACT3 => 3,
            Profile::DEFAULT => i32::MAX,
        }
    }

    /// Java: final Set<Target> targets (null for DEFAULT -> None).
    ///
    /// PORTING_RULES: the upstream EnumSet.of(t) always has exactly one element.
    pub fn targets(self) -> Option<[Target; 1]> {
        match self {
            Profile::COMPACT1 | Profile::COMPACT2 | Profile::COMPACT3 => Some([Target::JDK1_8]),
            Profile::DEFAULT => None,
        }
    }

    /// Java: public static Profile lookup(String name).
    ///
    /// PORTING_RULES: Java calls name.equals(p.name); a null p.name (DEFAULT)
    /// never matches.
    pub fn lookup(name: &str) -> Option<Profile> {
        Profile::values()
            .iter()
            .copied()
            .find(|profile| profile.name() == Some(name))
    }

    /// Java: public static Profile lookup(int value).
    ///
    /// PORTING_RULES: upstream searches by the integer value, so
    /// lookup_value(Integer.MAX_VALUE) returns DEFAULT (its value field is
    /// Integer.MAX_VALUE) — kept as upstream.
    pub fn lookup_value(value: i32) -> Option<Profile> {
        Profile::values()
            .iter()
            .copied()
            .find(|profile| profile.value() == value)
    }

    /// Java: public boolean isValid(Target t).
    ///
    /// PORTING_RULES: DEFAULT overrides isValid to return true; the compact
    /// profiles check membership in their target set.
    pub fn is_valid(self, target: Target) -> bool {
        match self.targets() {
            None => true,
            Some(targets) => targets.contains(&target),
        }
    }

    /// Java:
    /// <pre>
    /// public static Profile instance(Context context) {
    ///     Profile instance = context.get(profileKey);
    ///     if (instance == null) {
    ///         Options options = Options.instance(context);
    ///         String profileString = options.get(PROFILE);
    ///         if (profileString != null) instance = lookup(profileString);
    ///         if (instance == null) instance = DEFAULT;
    ///         context.put(profileKey, instance);
    ///     }
    ///     return instance;
    /// }
    /// </pre>
    ///
    /// TODO(Options/Option batch): Options.instance/options.get and
    /// Option.PROFILE ("-profile") are not ported; the entry point stays
    /// unimplemented (no substitute Options is invented).
    pub fn instance(_context: &mut Context) -> Rc<Profile> {
        unimplemented!(
            "TODO(Options/Option batch): Profile.instance (Options and Option.PROFILE are not ported)"
        )
    }
}

impl fmt::Display for Profile {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::Profile;
    use crate::jvm::target::Target;

    #[test]
    fn values_names_and_indices() {
        let values = Profile::values();
        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Profile::COMPACT1);
        assert_eq!(values[3], Profile::DEFAULT);
        assert_eq!(Profile::COMPACT1.name(), Some("compact1"));
        assert_eq!(Profile::COMPACT2.name(), Some("compact2"));
        assert_eq!(Profile::COMPACT3.name(), Some("compact3"));
        assert_eq!(Profile::DEFAULT.name(), None);
        assert_eq!(Profile::COMPACT1.value(), 1);
        assert_eq!(Profile::COMPACT2.value(), 2);
        assert_eq!(Profile::COMPACT3.value(), 3);
        assert_eq!(Profile::DEFAULT.value(), i32::MAX);
    }

    #[test]
    fn lookup_by_name_matches_the_compact_profiles() {
        assert_eq!(Profile::lookup("compact1"), Some(Profile::COMPACT1));
        assert_eq!(Profile::lookup("compact2"), Some(Profile::COMPACT2));
        assert_eq!(Profile::lookup("compact3"), Some(Profile::COMPACT3));
        // DEFAULT has a null name, so no input string matches it.
        assert_eq!(Profile::lookup("default"), None);
        assert_eq!(Profile::lookup(""), None);
    }

    #[test]
    fn lookup_by_value_includes_the_default_max_value() {
        assert_eq!(Profile::lookup_value(1), Some(Profile::COMPACT1));
        assert_eq!(Profile::lookup_value(3), Some(Profile::COMPACT3));
        assert_eq!(Profile::lookup_value(4), None);
        // Upstream quirk: DEFAULT.value is Integer.MAX_VALUE.
        assert_eq!(Profile::lookup_value(i32::MAX), Some(Profile::DEFAULT));
    }

    #[test]
    fn validity_follows_the_target_set() {
        assert!(Profile::COMPACT1.is_valid(Target::JDK1_8));
        assert!(Profile::COMPACT2.is_valid(Target::JDK1_8));
        assert!(Profile::COMPACT3.is_valid(Target::JDK1_8));
        assert!(!Profile::COMPACT1.is_valid(Target::JDK1_9));
        for target in Target::values() {
            assert!(Profile::DEFAULT.is_valid(target));
        }
    }

    #[test]
    fn display_reproduces_the_enum_constant_name() {
        assert_eq!(Profile::COMPACT1.to_string(), "COMPACT1");
        assert_eq!(Profile::DEFAULT.to_string(), "DEFAULT");
    }

    #[test]
    #[should_panic(expected = "TODO(Options/Option batch)")]
    fn instance_needs_the_options_port() {
        let mut context = crate::util::context::Context::new();
        let _ = Profile::instance(&mut context);
    }
}
