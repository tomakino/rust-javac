//! An interface for type tag values, which distinguish between different sorts
//! of types.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the Java enum constructor stores (numericClass, superClasses,
//! isPrimitive) per constant; Rust expresses those as match-based accessors.
//! Java enum name() is reproduced by Display (the derived Debug name).

use javac_source::tree::tree::Kind;
use javac_tools::lang::model::type_::TypeKind;

use crate::util::assert;

/// Java: public static class NumericClasses
pub mod numeric_classes {
    pub const BYTE_CLASS: i32 = 1;
    pub const CHAR_CLASS: i32 = 2;
    pub const SHORT_CLASS: i32 = 4;
    pub const INT_CLASS: i32 = 8;
    pub const LONG_CLASS: i32 = 16;
    pub const FLOAT_CLASS: i32 = 32;
    pub const DOUBLE_CLASS: i32 = 64;

    pub const BYTE_SUPERCLASSES: i32 =
        BYTE_CLASS | SHORT_CLASS | INT_CLASS | LONG_CLASS | FLOAT_CLASS | DOUBLE_CLASS;
    pub const CHAR_SUPERCLASSES: i32 =
        CHAR_CLASS | INT_CLASS | LONG_CLASS | FLOAT_CLASS | DOUBLE_CLASS;
    pub const SHORT_SUPERCLASSES: i32 =
        SHORT_CLASS | INT_CLASS | LONG_CLASS | FLOAT_CLASS | DOUBLE_CLASS;
    pub const INT_SUPERCLASSES: i32 = INT_CLASS | LONG_CLASS | FLOAT_CLASS | DOUBLE_CLASS;
    pub const LONG_SUPERCLASSES: i32 = LONG_CLASS | FLOAT_CLASS | DOUBLE_CLASS;
    pub const FLOAT_SUPERCLASSES: i32 = FLOAT_CLASS | DOUBLE_CLASS;
}

use numeric_classes::{
    BYTE_CLASS, BYTE_SUPERCLASSES, CHAR_CLASS, CHAR_SUPERCLASSES, DOUBLE_CLASS, FLOAT_CLASS,
    FLOAT_SUPERCLASSES, INT_CLASS, INT_SUPERCLASSES, LONG_CLASS, LONG_SUPERCLASSES, SHORT_CLASS,
    SHORT_SUPERCLASSES,
};

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeTag {
    /// The tag of the basic type byte.
    BYTE,
    /// The tag of the basic type char.
    CHAR,
    /// The tag of the basic type short.
    SHORT,
    /// The tag of the basic type long.
    LONG,
    /// The tag of the basic type float.
    FLOAT,
    /// The tag of the basic type int.
    INT,
    /// The tag of the basic type double.
    DOUBLE,
    /// The tag of the basic type boolean.
    BOOLEAN,
    /// The tag of the type void.
    VOID,
    /// The tag of all class and interface types.
    CLASS,
    /// The tag of all array types.
    ARRAY,
    /// The tag of all (monomorphic) method types.
    METHOD,
    /// The tag of all package "types".
    PACKAGE,
    /// The tag of all module "types".
    MODULE,
    /// The tag of all (source-level) type variables.
    TYPEVAR,
    /// The tag of all type arguments.
    WILDCARD,
    /// The tag of all polymorphic (method-) types.
    FORALL,
    /// The tag of deferred expression types in method context.
    DEFERRED,
    /// The tag of the bottom type null.
    BOT,
    /// The tag of a missing type.
    NONE,
    /// The tag of the error type.
    ERROR,
    /// The tag of an unknown type.
    UNKNOWN,
    /// The tag of all instantiatable type variables.
    UNDETVAR,
    /// Pseudo-types, these are special tags.
    UNINITIALIZED_THIS,
    UNINITIALIZED_OBJECT,
}

impl TypeTag {
    /// Java: final int numericClass;
    pub const fn numeric_class(self) -> i32 {
        match self {
            TypeTag::BYTE => BYTE_CLASS,
            TypeTag::CHAR => CHAR_CLASS,
            TypeTag::SHORT => SHORT_CLASS,
            TypeTag::INT => INT_CLASS,
            TypeTag::LONG => LONG_CLASS,
            TypeTag::FLOAT => FLOAT_CLASS,
            TypeTag::DOUBLE => DOUBLE_CLASS,
            _ => 0,
        }
    }

    /// Java: final int superClasses;
    pub const fn super_classes(self) -> i32 {
        match self {
            TypeTag::BYTE | TypeTag::BOOLEAN => BYTE_SUPERCLASSES,
            TypeTag::CHAR => CHAR_SUPERCLASSES,
            TypeTag::SHORT => SHORT_SUPERCLASSES,
            TypeTag::INT => INT_SUPERCLASSES,
            TypeTag::LONG => LONG_SUPERCLASSES,
            TypeTag::FLOAT => FLOAT_SUPERCLASSES,
            TypeTag::DOUBLE => DOUBLE_CLASS,
            _ => 0,
        }
    }

    /// Java: final boolean isPrimitive;
    pub const fn is_primitive(self) -> bool {
        matches!(
            self,
            TypeTag::BYTE
                | TypeTag::CHAR
                | TypeTag::SHORT
                | TypeTag::LONG
                | TypeTag::FLOAT
                | TypeTag::INT
                | TypeTag::DOUBLE
                | TypeTag::BOOLEAN
        )
    }

    /// Java: public boolean isStrictSubRangeOf(TypeTag tag)
    pub const fn is_strict_sub_range_of(self, tag: TypeTag) -> bool {
        // Please don't change the implementation of this method to call method
        // isSubRangeOf. Both methods are called from hotspot code, the current
        // implementation is better performance-wise than the commented modification.
        (self.super_classes() & tag.numeric_class()) != 0 && (self as u8) != (tag as u8)
    }

    /// Java: public boolean isSubRangeOf(TypeTag tag)
    pub const fn is_sub_range_of(self, tag: TypeTag) -> bool {
        (self.super_classes() & tag.numeric_class()) != 0
    }

    /// Returns the number of type tags.
    ///
    /// Java: return (UNDETVAR.ordinal() + 1);
    pub const fn get_type_tag_count() -> i32 {
        // BYTE..UNDETVAR inclusive (the last two tags are pseudo-types and are
        // not included in the total).
        23
    }

    /// Java: public Kind getKindLiteral()
    pub fn get_kind_literal(self) -> Kind {
        match self {
            TypeTag::INT => Kind::INT_LITERAL,
            TypeTag::LONG => Kind::LONG_LITERAL,
            TypeTag::FLOAT => Kind::FLOAT_LITERAL,
            TypeTag::DOUBLE => Kind::DOUBLE_LITERAL,
            TypeTag::BOOLEAN => Kind::BOOLEAN_LITERAL,
            TypeTag::CHAR => Kind::CHAR_LITERAL,
            TypeTag::CLASS => Kind::STRING_LITERAL,
            TypeTag::BOT => Kind::NULL_LITERAL,
            other => assert::error_msg(format!("unknown literal kind {}", other)),
        }
    }

    /// Java: public TypeKind getPrimitiveTypeKind()
    pub fn get_primitive_type_kind(self) -> TypeKind {
        match self {
            TypeTag::BOOLEAN => TypeKind::BOOLEAN,
            TypeTag::BYTE => TypeKind::BYTE,
            TypeTag::SHORT => TypeKind::SHORT,
            TypeTag::INT => TypeKind::INT,
            TypeTag::LONG => TypeKind::LONG,
            TypeTag::CHAR => TypeKind::CHAR,
            TypeTag::FLOAT => TypeKind::FLOAT,
            TypeTag::DOUBLE => TypeKind::DOUBLE,
            TypeTag::VOID => TypeKind::VOID,
            other => assert::error_msg(format!("unknown primitive type {}", other)),
        }
    }

    /// Returns true if the given value is within the allowed range for this type.
    pub fn check_range(self, value: i32) -> bool {
        match self {
            TypeTag::BOOLEAN => (0..=1).contains(&value),
            TypeTag::BYTE => (i8::MIN as i32..=i8::MAX as i32).contains(&value),
            TypeTag::CHAR => (0..=u16::MAX as i32).contains(&value),
            TypeTag::SHORT => (i16::MIN as i32..=i16::MAX as i32).contains(&value),
            TypeTag::INT => true,
            _ => assert::error_msg("AssertionError"),
        }
    }
}

// Java enum toString() returns name(); derived Debug prints the variant name.
impl std::fmt::Display for TypeTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_range_relations() {
        assert!(TypeTag::BYTE.is_sub_range_of(TypeTag::INT));
        assert!(TypeTag::BYTE.is_strict_sub_range_of(TypeTag::INT));
        assert!(!TypeTag::BYTE.is_strict_sub_range_of(TypeTag::BYTE));
        assert!(TypeTag::CHAR.is_sub_range_of(TypeTag::INT));
        assert!(!TypeTag::CHAR.is_sub_range_of(TypeTag::BYTE));
        assert!(TypeTag::DOUBLE.is_sub_range_of(TypeTag::DOUBLE));
        assert!(!TypeTag::CLASS.is_sub_range_of(TypeTag::INT));
    }

    #[test]
    fn counts_and_primitive_flags() {
        assert_eq!(TypeTag::get_type_tag_count(), 23);
        assert!(TypeTag::INT.is_primitive());
        assert!(!TypeTag::CLASS.is_primitive());
        assert!(!TypeTag::VOID.is_primitive());
    }

    #[test]
    fn literal_and_primitive_kinds() {
        assert_eq!(TypeTag::INT.get_kind_literal(), Kind::INT_LITERAL);
        assert_eq!(TypeTag::BOT.get_kind_literal(), Kind::NULL_LITERAL);
        assert_eq!(TypeTag::INT.get_primitive_type_kind(), TypeKind::INT);
        assert_eq!(TypeTag::VOID.get_primitive_type_kind(), TypeKind::VOID);
        // Java maps CLASS to STRING_LITERAL; METHOD has no literal kind.
        assert_eq!(TypeTag::CLASS.get_kind_literal(), Kind::STRING_LITERAL);
        assert!(std::panic::catch_unwind(|| TypeTag::METHOD.get_kind_literal()).is_err());
        assert!(std::panic::catch_unwind(|| TypeTag::CLASS.get_primitive_type_kind()).is_err());
    }

    #[test]
    fn check_ranges() {
        assert!(TypeTag::BOOLEAN.check_range(1));
        assert!(!TypeTag::BOOLEAN.check_range(2));
        assert!(TypeTag::BYTE.check_range(127));
        assert!(!TypeTag::BYTE.check_range(128));
        assert!(TypeTag::CHAR.check_range(65535));
        assert!(!TypeTag::CHAR.check_range(-1));
        assert!(TypeTag::INT.check_range(i32::MAX));
        assert!(std::panic::catch_unwind(|| TypeTag::CLASS.check_range(0)).is_err());
    }

    #[test]
    fn display_is_java_name() {
        assert_eq!(TypeTag::INT.to_string(), "INT");
        assert_eq!(TypeTag::UNINITIALIZED_OBJECT.to_string(), "UNINITIALIZED_OBJECT");
    }
}

