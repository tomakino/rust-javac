// Origin: com/sun/tools/javac/code/Flags.java @ 982d53c (jdk-17.0.19-ga)
#![allow(non_upper_case_globals)]

//! Access flags and other modifiers for Java classes and members.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: Java mixed-case constants (AccessFlags, MemberClassFlags, ...)
//! keep their Java names to stay diffable against upstream; the Flag enum keeps
//! Java constant names with #[allow(non_camel_case_types)]. The Java
//! ConcurrentHashMap modifier-set cache is a pure performance detail with no
//! observable behavior and is not reproduced.
//! TODO(P3): isStatic/isEnum/isConstant need Symbol/Symbol.VarSymbol and are
//! added when code/Symbol.rs is ported.

use javac_strings::JavaString;
use javac_tools::lang::model::element::modifier::Modifier;

use crate::util::assert;

/* Standard Java flags.
 */
pub const PUBLIC: i32 = 1;
pub const PRIVATE: i32 = 1 << 1;
pub const PROTECTED: i32 = 1 << 2;
pub const STATIC: i32 = 1 << 3;
pub const FINAL: i32 = 1 << 4;
pub const SYNCHRONIZED: i32 = 1 << 5;
pub const VOLATILE: i32 = 1 << 6;
pub const TRANSIENT: i32 = 1 << 7;
pub const NATIVE: i32 = 1 << 8;
pub const INTERFACE: i32 = 1 << 9;
pub const ABSTRACT: i32 = 1 << 10;
pub const STRICTFP: i32 = 1 << 11;

/* Flag that marks a symbol synthetic, added in classfile v49.0. */
pub const SYNTHETIC: i32 = 1 << 12;

/** Flag that marks attribute interfaces, added in classfile v49.0. */
pub const ANNOTATION: i32 = 1 << 13;

/** An enumeration type or an enumeration constant, added in classfile v49.0. */
pub const ENUM: i32 = 1 << 14;

/** Added in SE8, represents constructs implicitly declared in source. */
pub const MANDATED: i32 = 1 << 15;

pub const StandardFlags: i32 = 0x0fff;

// Because the following access flags are overloaded with other bit positions,
// we translate them when reading and writing class files into unique bits
// positions: ACC_SYNTHETIC <-> SYNTHETIC, for example.
pub const ACC_SUPER: i32 = 0x0020;
pub const ACC_BRIDGE: i32 = 0x0040;
pub const ACC_VARARGS: i32 = 0x0080;
pub const ACC_MODULE: i32 = 0x8000;

/*****************************************
 * Internal compiler flags (no bits in the lower 16).
 *****************************************/

/** Flag is set if symbol is deprecated.  See also DEPRECATED_REMOVAL. */
pub const DEPRECATED: i32 = 1 << 17;

/** Flag is set for a variable symbol if the variable's definition has an
 *  initializer part.
 */
pub const HASINIT: i32 = 1 << 18;

/** Flag is set for compiler-generated anonymous method symbols
 *  that own an initializer block.
 */
pub const BLOCK: i32 = 1 << 20;

/* Flag bit 21 is available. (used earlier to tag compiler-generated abstract
 *  methods that implement an interface method (Miranda methods)). */

/** Flag is set for nested classes that do not access instance members
 *  or this of an outer class and therefore don't need to be passed
 *  a this$n reference. This value is currently set only for anonymous
 *  classes in superclass constructor calls.
 *  todo: use this value for optimizing away this$n parameters in other cases.
 */
pub const NOOUTERTHIS: i32 = 1 << 22;

/** Flag is set for package symbols if a package has a member or directory and
 *  therefore exists.
 */
pub const EXISTS: i32 = 1 << 23;

/** Flag is set for compiler-generated compound classes
 *  representing multiple variable bounds.
 */
pub const COMPOUND: i32 = 1 << 24;

/** Flag is set for class symbols if a class file was found for this class. */
pub const CLASS_SEEN: i32 = 1 << 25;

/** Flag is set for class symbols if a source file was found for this class. */
pub const SOURCE_SEEN: i32 = 1 << 26;

/* State flags (are reset during compilation).
 */

/** Flag for class symbols is set and later re-set as a lock in Enter to detect
 *  cycles in the superclass/superinterface relations. Similarly for constructor
 *  call cycle detection in Attr.
 */
pub const LOCKED: i32 = 1 << 27;

/** Flag for class symbols is set and later re-set to indicate that a class has
 *  been entered but has not yet been attributed.
 */
pub const UNATTRIBUTED: i32 = 1 << 28;

/** Flag for synthesized default constructors of anonymous classes. */
pub const ANONCONSTR: i32 = 1 << 29; //non-class members

/** Flag to indicate the super classes of this ClassSymbol has been attributed. */
pub const SUPER_OWNER_ATTRIBUTED: i32 = 1 << 29; //ClassSymbols

/** Flag for class symbols to indicate it has been checked and found acyclic. */
pub const ACYCLIC: i32 = 1 << 30;

/** Flag that marks bridge methods. */
pub const BRIDGE: i64 = 1i64 << 31;

/** Flag that marks formal parameters. */
pub const PARAMETER: i64 = 1i64 << 33;

/** Flag that marks varargs methods. */
pub const VARARGS: i64 = 1i64 << 34;

/** Flag for annotation type symbols to indicate it has been checked and found
 *  acyclic.
 */
pub const ACYCLIC_ANN: i64 = 1i64 << 35;

/** Flag that marks a generated default constructor. */
pub const GENERATEDCONSTR: i64 = 1i64 << 36;

/** Flag that marks a hypothetical method that need not really be generated in
 *  the binary, but is present in the symbol table to simplify checking for
 *  erasure clashes - also used for 292 poly sig methods.
 */
pub const HYPOTHETICAL: i64 = 1i64 << 37;

/** Flag that marks an internal proprietary class. */
pub const PROPRIETARY: i64 = 1i64 << 38;

/** Flag that marks a multi-catch parameter. */
pub const UNION: i64 = 1i64 << 39;

/** Flags an erroneous TypeSymbol as viable for recovery. TypeSymbols only. */
pub const RECOVERABLE: i64 = 1i64 << 40;

/** Flag that marks an 'effectively final' local variable. */
pub const EFFECTIVELY_FINAL: i64 = 1i64 << 41;

/** Flag that marks non-override equivalent methods with the same signature,
 *  or a conflicting match binding (BindingSymbol).
 */
pub const CLASH: i64 = 1i64 << 42;

/** Flag that marks either a default method or an interface containing default
 *  methods.
 */
pub const DEFAULT: i64 = 1i64 << 43;

/** Flag that marks class as auxiliary, ie a non-public class following the
 *  public class in a source file, that could block implicit compilation.
 */
pub const AUXILIARY: i64 = 1i64 << 44;

/** Flag that marks that a symbol is not available in the current profile. */
pub const NOT_IN_PROFILE: i64 = 1i64 << 45;

/** Flag that indicates that an override error has been detected by Check. */
pub const BAD_OVERRIDE: i64 = 1i64 << 45;

/** Flag that indicates a signature polymorphic method (292). */
pub const SIGNATURE_POLYMORPHIC: i64 = 1i64 << 46;

/** Flag that indicates that an inference variable is used in a throws clause. */
pub const THROWS: i64 = 1i64 << 47;

/** Flag that marks potentially ambiguous overloads. */
pub const POTENTIALLY_AMBIGUOUS: i64 = 1i64 << 48;

/** Flag that marks a synthetic method body for a lambda expression. */
pub const LAMBDA_METHOD: i64 = 1i64 << 49;

/** Flag to control recursion in TransTypes. */
pub const TYPE_TRANSLATED: i64 = 1i64 << 50;

/** Flag to indicate class symbol is for module-info. */
pub const MODULE: i64 = 1i64 << 51;

/** Flag to indicate the given ModuleSymbol is an automatic module. */
pub const AUTOMATIC_MODULE: i64 = 1i64 << 52; //ModuleSymbols only

/** Flag to indicate the given PackageSymbol contains any non-.java and non-.class
 *  resources.
 */
pub const HAS_RESOURCE: i64 = 1i64 << 52; //PackageSymbols only

/** Flag to indicate the given ParamSymbol has a user-friendly name filled. */
pub const NAME_FILLED: i64 = 1i64 << 52; //ParamSymbols only

/** Flag to indicate the given ModuleSymbol is a system module. */
pub const SYSTEM_MODULE: i64 = 1i64 << 53; //ModuleSymbols only

/** Flag to indicate the given ClassSymbol is a value based. */
pub const VALUE_BASED: i64 = 1i64 << 53; //ClassSymbols only

/** Flag to indicate the given symbol has a @Deprecated annotation. */
pub const DEPRECATED_ANNOTATION: i64 = 1i64 << 54;

/** Flag to indicate the given symbol has been deprecated and marked for removal. */
pub const DEPRECATED_REMOVAL: i64 = 1i64 << 55;

/** Flag to indicate the API element in question is for a preview API. */
pub const PREVIEW_API: i64 = 1i64 << 56; //any Symbol kind

/** Flag for synthesized default constructors of anonymous classes that have an
 *  enclosing expression.
 */
pub const ANONCONSTR_BASED: i64 = 1i64 << 57;

/** Flag that marks finalize block as body-only, should not be copied into catch
 *  clauses. Used to implement try-with-resources.
 */
pub const BODY_ONLY_FINALIZE: i64 = 1i64 << 17; //blocks only

/** Flag to indicate the API element in question is for a preview API. */
pub const PREVIEW_REFLECTIVE: i64 = 1i64 << 58; //any Symbol kind

/** Flag to indicate the given variable is a match binding variable. */
pub const MATCH_BINDING: i64 = 1i64 << 59;

/** A flag to indicate a match binding variable whose scope extends after the
 *  current statement.
 */
pub const MATCH_BINDING_TO_OUTER: i64 = 1i64 << 60;

/** Flag to indicate that a class is a record. The flag is also used to mark
 *  fields that are part of the state vector of a record and to mark the
 *  canonical constructor.
 */
pub const RECORD: i64 = 1i64 << 61; // ClassSymbols, MethodSymbols and VarSymbols

/** Flag to mark a record constructor as a compact one. */
pub const COMPACT_RECORD_CONSTRUCTOR: i64 = 1i64 << 51; // MethodSymbols only

/** Flag to mark a record field that was not initialized in the compact
 *  constructor.
 */
pub const UNINITIALIZED_FIELD: i64 = 1i64 << 51; // VarSymbols only

/** Flag is set for compiler-generated record members, it could be applied to
 *  accessors and fields.
 */
pub const GENERATED_MEMBER: i32 = 1 << 24; // MethodSymbols and VarSymbols

/** Flag to indicate sealed class/interface declaration. */
pub const SEALED: i64 = 1i64 << 62; // ClassSymbols

/** Flag to indicate type annotations have been queued for field initializers. */
pub const FIELD_INIT_TYPE_ANNOTATIONS_QUEUED: i64 = 1i64 << 53; // VarSymbols

/** Flag to indicate that the class/interface was declared with the non-sealed
 *  modifier.
 */
pub const NON_SEALED: i64 = 1i64 << 63; // ClassSymbols

/** Modifier masks.
 */
pub const AccessFlags: i32 = PUBLIC | PROTECTED | PRIVATE;
pub const LocalClassFlags: i32 = FINAL | ABSTRACT | STRICTFP | ENUM | SYNTHETIC;
pub const StaticLocalFlags: i32 = LocalClassFlags | STATIC | INTERFACE;
pub const MemberClassFlags: i32 = LocalClassFlags | INTERFACE | AccessFlags;
pub const MemberStaticClassFlags: i32 = MemberClassFlags | STATIC;
pub const ClassFlags: i32 = LocalClassFlags | INTERFACE | PUBLIC | ANNOTATION;
pub const InterfaceVarFlags: i32 = FINAL | STATIC | PUBLIC;
pub const VarFlags: i32 = AccessFlags | FINAL | STATIC | VOLATILE | TRANSIENT | ENUM;
pub const ConstructorFlags: i32 = AccessFlags;
pub const InterfaceMethodFlags: i32 = ABSTRACT | PUBLIC;
pub const MethodFlags: i32 =
    AccessFlags | ABSTRACT | STATIC | NATIVE | SYNCHRONIZED | FINAL | STRICTFP;
pub const RecordMethodFlags: i32 =
    AccessFlags | ABSTRACT | STATIC | SYNCHRONIZED | FINAL | STRICTFP;

pub const ExtendedStandardFlags: i64 = StandardFlags as i64 | DEFAULT | SEALED | NON_SEALED;
pub const ExtendedMemberClassFlags: i64 = MemberClassFlags as i64 | SEALED | NON_SEALED;
pub const ExtendedMemberStaticClassFlags: i64 = MemberStaticClassFlags as i64 | SEALED | NON_SEALED;
pub const ExtendedClassFlags: i64 = ClassFlags as i64 | SEALED | NON_SEALED;
pub const ModifierFlags: i64 =
    ((StandardFlags & !INTERFACE) as i64) | DEFAULT | SEALED | NON_SEALED;
pub const InterfaceMethodMask: i64 =
    ABSTRACT as i64 | PRIVATE as i64 | STATIC as i64 | PUBLIC as i64 | STRICTFP as i64 | DEFAULT;
pub const AnnotationTypeElementMask: i64 = ABSTRACT as i64 | PUBLIC as i64;
pub const LocalVarFlags: i64 = FINAL as i64 | PARAMETER;
pub const ReceiverParamFlags: i64 = PARAMETER;

/// Java: public static String toString(long flags)
///
/// PORTING_RULES: Java String results are JavaString (PORTING_RULES Porting rules); this is
/// the overloaded static toString, so it gets a descriptive Rust name.
pub fn to_java_string(flags: i64) -> JavaString {
    let mut buf = String::new();
    let mut sep = "";
    for flag in as_flag_set(flags) {
        buf.push_str(sep);
        buf.push_str(&flag.to_string());
        sep = " ";
    }
    JavaString::from(buf.as_str())
}

/// Java: public static EnumSet<Flag> asFlagSet(long flags)
///
/// EnumSet iteration order is the enum declaration order, so a Vec built from
/// Flag::ALL in declaration order is equivalent.
pub fn as_flag_set(flags: i64) -> Vec<Flag> {
    let mut remaining = flags;
    let mut flag_set = Vec::new();
    for flag in Flag::ALL {
        if (remaining & flag.value()) != 0 {
            flag_set.push(flag);
            remaining &= !flag.value();
        }
    }
    // Java: Assert.check(flags == 0);
    assert::check(remaining == 0);
    flag_set
}

/// Java: public static Set<Modifier> asModifierSet(long flags)
///
/// The Java EnumSet iteration order is the Modifier declaration order; the
/// ConcurrentHashMap cache is a performance detail and is not reproduced.
pub fn as_modifier_set(flags: i64) -> Vec<Modifier> {
    let mut modifiers = Vec::new();
    for modifier in Modifier::values() {
        let bit: i64 = match modifier {
            Modifier::PUBLIC => PUBLIC as i64,
            Modifier::PROTECTED => PROTECTED as i64,
            Modifier::PRIVATE => PRIVATE as i64,
            Modifier::ABSTRACT => ABSTRACT as i64,
            Modifier::DEFAULT => DEFAULT,
            Modifier::STATIC => STATIC as i64,
            Modifier::SEALED => SEALED,
            Modifier::NON_SEALED => NON_SEALED,
            Modifier::FINAL => FINAL as i64,
            Modifier::TRANSIENT => TRANSIENT as i64,
            Modifier::VOLATILE => VOLATILE as i64,
            Modifier::SYNCHRONIZED => SYNCHRONIZED as i64,
            Modifier::NATIVE => NATIVE as i64,
            Modifier::STRICTFP => STRICTFP as i64,
        };
        if (flags & bit) != 0 {
            modifiers.push(modifier);
        }
    }
    modifiers
}

// TODO(P3): isStatic(Symbol), isEnum(Symbol), isConstant(Symbol.VarSymbol) are
// ported together with code/Symbol.rs.

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flag {
    PUBLIC,
    PRIVATE,
    PROTECTED,
    STATIC,
    FINAL,
    SYNCHRONIZED,
    VOLATILE,
    TRANSIENT,
    NATIVE,
    INTERFACE,
    ABSTRACT,
    DEFAULT,
    STRICTFP,
    BRIDGE,
    SYNTHETIC,
    ANNOTATION,
    DEPRECATED,
    HASINIT,
    BLOCK,
    ENUM,
    MANDATED,
    NOOUTERTHIS,
    EXISTS,
    COMPOUND,
    CLASS_SEEN,
    SOURCE_SEEN,
    LOCKED,
    UNATTRIBUTED,
    ANONCONSTR,
    ACYCLIC,
    PARAMETER,
    VARARGS,
    ACYCLIC_ANN,
    GENERATEDCONSTR,
    HYPOTHETICAL,
    PROPRIETARY,
    UNION,
    EFFECTIVELY_FINAL,
    CLASH,
    AUXILIARY,
    NOT_IN_PROFILE,
    BAD_OVERRIDE,
    SIGNATURE_POLYMORPHIC,
    THROWS,
    LAMBDA_METHOD,
    TYPE_TRANSLATED,
    MODULE,
    AUTOMATIC_MODULE,
    SYSTEM_MODULE,
    DEPRECATED_ANNOTATION,
    DEPRECATED_REMOVAL,
    HAS_RESOURCE,
    POTENTIALLY_AMBIGUOUS,
    ANONCONSTR_BASED,
    NAME_FILLED,
    PREVIEW_API,
    PREVIEW_REFLECTIVE,
    MATCH_BINDING,
    MATCH_BINDING_TO_OUTER,
    RECORD,
    RECOVERABLE,
    SEALED,
    NON_SEALED,
}

impl Flag {
    /// Java: enum Flag ... (declaration order matters for asFlagSet).
    pub const ALL: [Flag; 63] = [
        Flag::PUBLIC,
        Flag::PRIVATE,
        Flag::PROTECTED,
        Flag::STATIC,
        Flag::FINAL,
        Flag::SYNCHRONIZED,
        Flag::VOLATILE,
        Flag::TRANSIENT,
        Flag::NATIVE,
        Flag::INTERFACE,
        Flag::ABSTRACT,
        Flag::DEFAULT,
        Flag::STRICTFP,
        Flag::BRIDGE,
        Flag::SYNTHETIC,
        Flag::ANNOTATION,
        Flag::DEPRECATED,
        Flag::HASINIT,
        Flag::BLOCK,
        Flag::ENUM,
        Flag::MANDATED,
        Flag::NOOUTERTHIS,
        Flag::EXISTS,
        Flag::COMPOUND,
        Flag::CLASS_SEEN,
        Flag::SOURCE_SEEN,
        Flag::LOCKED,
        Flag::UNATTRIBUTED,
        Flag::ANONCONSTR,
        Flag::ACYCLIC,
        Flag::PARAMETER,
        Flag::VARARGS,
        Flag::ACYCLIC_ANN,
        Flag::GENERATEDCONSTR,
        Flag::HYPOTHETICAL,
        Flag::PROPRIETARY,
        Flag::UNION,
        Flag::EFFECTIVELY_FINAL,
        Flag::CLASH,
        Flag::AUXILIARY,
        Flag::NOT_IN_PROFILE,
        Flag::BAD_OVERRIDE,
        Flag::SIGNATURE_POLYMORPHIC,
        Flag::THROWS,
        Flag::LAMBDA_METHOD,
        Flag::TYPE_TRANSLATED,
        Flag::MODULE,
        Flag::AUTOMATIC_MODULE,
        Flag::SYSTEM_MODULE,
        Flag::DEPRECATED_ANNOTATION,
        Flag::DEPRECATED_REMOVAL,
        Flag::HAS_RESOURCE,
        Flag::POTENTIALLY_AMBIGUOUS,
        Flag::ANONCONSTR_BASED,
        Flag::NAME_FILLED,
        Flag::PREVIEW_API,
        Flag::PREVIEW_REFLECTIVE,
        Flag::MATCH_BINDING,
        Flag::MATCH_BINDING_TO_OUTER,
        Flag::RECORD,
        Flag::RECOVERABLE,
        Flag::SEALED,
        Flag::NON_SEALED,
    ];

    /// Java: final long value;
    pub const fn value(self) -> i64 {
        match self {
            Flag::PUBLIC => PUBLIC as i64,
            Flag::PRIVATE => PRIVATE as i64,
            Flag::PROTECTED => PROTECTED as i64,
            Flag::STATIC => STATIC as i64,
            Flag::FINAL => FINAL as i64,
            Flag::SYNCHRONIZED => SYNCHRONIZED as i64,
            Flag::VOLATILE => VOLATILE as i64,
            Flag::TRANSIENT => TRANSIENT as i64,
            Flag::NATIVE => NATIVE as i64,
            Flag::INTERFACE => INTERFACE as i64,
            Flag::ABSTRACT => ABSTRACT as i64,
            Flag::DEFAULT => DEFAULT,
            Flag::STRICTFP => STRICTFP as i64,
            Flag::BRIDGE => BRIDGE,
            Flag::SYNTHETIC => SYNTHETIC as i64,
            Flag::ANNOTATION => ANNOTATION as i64,
            Flag::DEPRECATED => DEPRECATED as i64,
            Flag::HASINIT => HASINIT as i64,
            Flag::BLOCK => BLOCK as i64,
            Flag::ENUM => ENUM as i64,
            Flag::MANDATED => MANDATED as i64,
            Flag::NOOUTERTHIS => NOOUTERTHIS as i64,
            Flag::EXISTS => EXISTS as i64,
            Flag::COMPOUND => COMPOUND as i64,
            Flag::CLASS_SEEN => CLASS_SEEN as i64,
            Flag::SOURCE_SEEN => SOURCE_SEEN as i64,
            Flag::LOCKED => LOCKED as i64,
            Flag::UNATTRIBUTED => UNATTRIBUTED as i64,
            Flag::ANONCONSTR => ANONCONSTR as i64,
            Flag::ACYCLIC => ACYCLIC as i64,
            Flag::PARAMETER => PARAMETER,
            Flag::VARARGS => VARARGS,
            Flag::ACYCLIC_ANN => ACYCLIC_ANN,
            Flag::GENERATEDCONSTR => GENERATEDCONSTR,
            Flag::HYPOTHETICAL => HYPOTHETICAL,
            Flag::PROPRIETARY => PROPRIETARY,
            Flag::UNION => UNION,
            Flag::EFFECTIVELY_FINAL => EFFECTIVELY_FINAL,
            Flag::CLASH => CLASH,
            Flag::AUXILIARY => AUXILIARY,
            Flag::NOT_IN_PROFILE => NOT_IN_PROFILE,
            Flag::BAD_OVERRIDE => BAD_OVERRIDE,
            Flag::SIGNATURE_POLYMORPHIC => SIGNATURE_POLYMORPHIC,
            Flag::THROWS => THROWS,
            Flag::LAMBDA_METHOD => LAMBDA_METHOD,
            Flag::TYPE_TRANSLATED => TYPE_TRANSLATED,
            Flag::MODULE => MODULE,
            Flag::AUTOMATIC_MODULE => AUTOMATIC_MODULE,
            Flag::SYSTEM_MODULE => SYSTEM_MODULE,
            Flag::DEPRECATED_ANNOTATION => DEPRECATED_ANNOTATION,
            Flag::DEPRECATED_REMOVAL => DEPRECATED_REMOVAL,
            Flag::HAS_RESOURCE => HAS_RESOURCE,
            Flag::POTENTIALLY_AMBIGUOUS => POTENTIALLY_AMBIGUOUS,
            Flag::ANONCONSTR_BASED => ANONCONSTR_BASED,
            Flag::NAME_FILLED => NAME_FILLED,
            Flag::PREVIEW_API => PREVIEW_API,
            Flag::PREVIEW_REFLECTIVE => PREVIEW_REFLECTIVE,
            Flag::MATCH_BINDING => MATCH_BINDING,
            Flag::MATCH_BINDING_TO_OUTER => MATCH_BINDING_TO_OUTER,
            Flag::RECORD => RECORD,
            Flag::RECOVERABLE => RECOVERABLE,
            Flag::SEALED => SEALED,
            Flag::NON_SEALED => NON_SEALED,
        }
    }

    /// Java: enum#name() (returns a Java String).
    pub fn name(self) -> &'static str {
        match self {
            Flag::PUBLIC => "PUBLIC",
            Flag::PRIVATE => "PRIVATE",
            Flag::PROTECTED => "PROTECTED",
            Flag::STATIC => "STATIC",
            Flag::FINAL => "FINAL",
            Flag::SYNCHRONIZED => "SYNCHRONIZED",
            Flag::VOLATILE => "VOLATILE",
            Flag::TRANSIENT => "TRANSIENT",
            Flag::NATIVE => "NATIVE",
            Flag::INTERFACE => "INTERFACE",
            Flag::ABSTRACT => "ABSTRACT",
            Flag::DEFAULT => "DEFAULT",
            Flag::STRICTFP => "STRICTFP",
            Flag::BRIDGE => "BRIDGE",
            Flag::SYNTHETIC => "SYNTHETIC",
            Flag::ANNOTATION => "ANNOTATION",
            Flag::DEPRECATED => "DEPRECATED",
            Flag::HASINIT => "HASINIT",
            Flag::BLOCK => "BLOCK",
            Flag::ENUM => "ENUM",
            Flag::MANDATED => "MANDATED",
            Flag::NOOUTERTHIS => "NOOUTERTHIS",
            Flag::EXISTS => "EXISTS",
            Flag::COMPOUND => "COMPOUND",
            Flag::CLASS_SEEN => "CLASS_SEEN",
            Flag::SOURCE_SEEN => "SOURCE_SEEN",
            Flag::LOCKED => "LOCKED",
            Flag::UNATTRIBUTED => "UNATTRIBUTED",
            Flag::ANONCONSTR => "ANONCONSTR",
            Flag::ACYCLIC => "ACYCLIC",
            Flag::PARAMETER => "PARAMETER",
            Flag::VARARGS => "VARARGS",
            Flag::ACYCLIC_ANN => "ACYCLIC_ANN",
            Flag::GENERATEDCONSTR => "GENERATEDCONSTR",
            Flag::HYPOTHETICAL => "HYPOTHETICAL",
            Flag::PROPRIETARY => "PROPRIETARY",
            Flag::UNION => "UNION",
            Flag::EFFECTIVELY_FINAL => "EFFECTIVELY_FINAL",
            Flag::CLASH => "CLASH",
            Flag::AUXILIARY => "AUXILIARY",
            Flag::NOT_IN_PROFILE => "NOT_IN_PROFILE",
            Flag::BAD_OVERRIDE => "BAD_OVERRIDE",
            Flag::SIGNATURE_POLYMORPHIC => "SIGNATURE_POLYMORPHIC",
            Flag::THROWS => "THROWS",
            Flag::LAMBDA_METHOD => "LAMBDA_METHOD",
            Flag::TYPE_TRANSLATED => "TYPE_TRANSLATED",
            Flag::MODULE => "MODULE",
            Flag::AUTOMATIC_MODULE => "AUTOMATIC_MODULE",
            Flag::SYSTEM_MODULE => "SYSTEM_MODULE",
            Flag::DEPRECATED_ANNOTATION => "DEPRECATED_ANNOTATION",
            Flag::DEPRECATED_REMOVAL => "DEPRECATED_REMOVAL",
            Flag::HAS_RESOURCE => "HAS_RESOURCE",
            Flag::POTENTIALLY_AMBIGUOUS => "POTENTIALLY_AMBIGUOUS",
            Flag::ANONCONSTR_BASED => "ANONCONSTR_BASED",
            Flag::NAME_FILLED => "NAME_FILLED",
            Flag::PREVIEW_API => "PREVIEW_API",
            Flag::PREVIEW_REFLECTIVE => "PREVIEW_REFLECTIVE",
            Flag::MATCH_BINDING => "MATCH_BINDING",
            Flag::MATCH_BINDING_TO_OUTER => "MATCH_BINDING_TO_OUTER",
            Flag::RECORD => "RECORD",
            Flag::RECOVERABLE => "RECOVERABLE",
            Flag::SEALED => "SEALED",
            Flag::NON_SEALED => "NON_SEALED",
        }
    }
}

impl std::fmt::Display for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Java: NON_SEALED overrides toString() to return "non-sealed".
        if *self == Flag::NON_SEALED {
            return f.write_str("non-sealed");
        }
        // Java: return lowercaseName (StringUtils.toLowerCase(name())).
        f.write_str(&self.name().to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_values_match_upstream() {
        assert_eq!(Flag::PUBLIC.value(), 1);
        assert_eq!(Flag::BRIDGE.value(), 1i64 << 31);
        assert_eq!(Flag::NON_SEALED.value(), 1i64 << 63);
        assert_eq!(Flag::AUTOMATIC_MODULE.value(), 1i64 << 52);
        assert_eq!(Flag::RECORD.value(), 1i64 << 61);
    }

    #[test]
    fn as_flag_set_order_and_shared_bits() {
        let flags = (PUBLIC | STATIC | FINAL) as i64;
        let set = as_flag_set(flags);
        assert_eq!(set, vec![Flag::PUBLIC, Flag::STATIC, Flag::FINAL]);
        // AUTOMATIC_MODULE, HAS_RESOURCE and NAME_FILLED share bit 52; the first
        // match in declaration order wins because the bit is cleared afterwards.
        assert_eq!(as_flag_set(1i64 << 52), vec![Flag::AUTOMATIC_MODULE]);
        assert!(std::panic::catch_unwind(|| as_flag_set(1i64 << 32)).is_err());
    }

    #[test]
    fn to_java_string_matches_java_tostring() {
        let flags = (PUBLIC | STATIC | FINAL) as i64;
        assert_eq!(to_java_string(flags).to_string_lossy(), "public static final");
        assert_eq!(to_java_string(NON_SEALED).to_string_lossy(), "non-sealed");
        assert_eq!(to_java_string(0).to_string_lossy(), "");
    }

    #[test]
    fn modifier_set_order() {
        let flags = (PUBLIC | STATIC | FINAL) as i64;
        assert_eq!(
            as_modifier_set(flags),
            vec![Modifier::PUBLIC, Modifier::STATIC, Modifier::FINAL]
        );
        assert_eq!(as_modifier_set(NON_SEALED), vec![Modifier::NON_SEALED]);
    }

    #[test]
    fn mask_constants_are_consistent() {
        assert_eq!(AccessFlags, PUBLIC | PROTECTED | PRIVATE);
        assert_eq!(MethodFlags, AccessFlags | ABSTRACT | STATIC | NATIVE | SYNCHRONIZED | FINAL | STRICTFP);
        assert_eq!(ModifierFlags, ((StandardFlags & !INTERFACE) as i64) | DEFAULT | SEALED | NON_SEALED);
    }
}
