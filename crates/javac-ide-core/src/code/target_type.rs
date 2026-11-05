//! Describes the type of program element an extended annotation (or extended
//! compound attribute) targets.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! the Java enum constructor validates the target type value as an
//! unsigned byte; all enum constants below are compile-time constants with valid
//! values, so the check is documented here instead of being executed at runtime.
//! The precomputed targets array of the Java static initializer is expressed as a
//! match: every value in 0x00..=0x4B that has no dedicated TargetType maps to
//! UNKNOWN, exactly like the array fill in Java.

use crate::util::assert;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetType {
    /// For annotations on a class type parameter declaration.
    CLASS_TYPE_PARAMETER,
    /// For annotations on a method type parameter declaration.
    METHOD_TYPE_PARAMETER,
    /// For annotations on the type of an "extends" or "implements" clause.
    CLASS_EXTENDS,
    /// For annotations on a bound of a type parameter of a class.
    CLASS_TYPE_PARAMETER_BOUND,
    /// For annotations on a bound of a type parameter of a method.
    METHOD_TYPE_PARAMETER_BOUND,
    /// For annotations on a field.
    FIELD,
    /// For annotations on a method return type.
    METHOD_RETURN,
    /// For annotations on the method receiver.
    METHOD_RECEIVER,
    /// For annotations on a method parameter.
    METHOD_FORMAL_PARAMETER,
    /// For annotations on a throws clause in a method declaration.
    THROWS,
    /// For annotations on a local variable.
    LOCAL_VARIABLE,
    /// For annotations on a resource variable.
    RESOURCE_VARIABLE,
    /// For annotations on an exception parameter.
    EXCEPTION_PARAMETER,
    /// For annotations on a type test.
    INSTANCEOF,
    /// For annotations on an object creation expression.
    NEW,
    /// For annotations on a constructor reference receiver.
    CONSTRUCTOR_REFERENCE,
    /// For annotations on a method reference receiver.
    METHOD_REFERENCE,
    /// For annotations on a typecast.
    CAST,
    /// For annotations on a type argument of an object creation expression.
    CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT,
    /// For annotations on a type argument of a method call.
    METHOD_INVOCATION_TYPE_ARGUMENT,
    /// For annotations on a type argument of a constructor reference.
    CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT,
    /// For annotations on a type argument of a method reference.
    METHOD_REFERENCE_TYPE_ARGUMENT,
    /// For annotations with an unknown target.
    UNKNOWN,
}

impl TargetType {
    /// Java: private static final int MAXIMUM_TARGET_TYPE_VALUE = 0x4B;
    pub const MAXIMUM_TARGET_TYPE_VALUE: i32 = 0x4B;

    /// Java: private final int targetTypeValue;
    pub const fn target_type_value(self) -> i32 {
        match self {
            TargetType::CLASS_TYPE_PARAMETER => 0x00,
            TargetType::METHOD_TYPE_PARAMETER => 0x01,
            TargetType::CLASS_EXTENDS => 0x10,
            TargetType::CLASS_TYPE_PARAMETER_BOUND => 0x11,
            TargetType::METHOD_TYPE_PARAMETER_BOUND => 0x12,
            TargetType::FIELD => 0x13,
            TargetType::METHOD_RETURN => 0x14,
            TargetType::METHOD_RECEIVER => 0x15,
            TargetType::METHOD_FORMAL_PARAMETER => 0x16,
            TargetType::THROWS => 0x17,
            TargetType::LOCAL_VARIABLE => 0x40,
            TargetType::RESOURCE_VARIABLE => 0x41,
            TargetType::EXCEPTION_PARAMETER => 0x42,
            TargetType::INSTANCEOF => 0x43,
            TargetType::NEW => 0x44,
            TargetType::CONSTRUCTOR_REFERENCE => 0x45,
            TargetType::METHOD_REFERENCE => 0x46,
            TargetType::CAST => 0x47,
            TargetType::CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT => 0x48,
            TargetType::METHOD_INVOCATION_TYPE_ARGUMENT => 0x49,
            TargetType::CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT => 0x4A,
            TargetType::METHOD_REFERENCE_TYPE_ARGUMENT => 0x4B,
            TargetType::UNKNOWN => 0xFF,
        }
    }

    /// Java: public boolean isLocal()
    pub const fn is_local(self) -> bool {
        matches!(
            self,
            TargetType::LOCAL_VARIABLE
                | TargetType::RESOURCE_VARIABLE
                | TargetType::EXCEPTION_PARAMETER
                | TargetType::INSTANCEOF
                | TargetType::NEW
                | TargetType::CONSTRUCTOR_REFERENCE
                | TargetType::METHOD_REFERENCE
                | TargetType::CAST
                | TargetType::CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT
                | TargetType::METHOD_INVOCATION_TYPE_ARGUMENT
                | TargetType::CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT
                | TargetType::METHOD_REFERENCE_TYPE_ARGUMENT
        )
    }

    /// Java: public static boolean isValidTargetTypeValue(int tag)
    pub const fn is_valid_target_type_value(tag: i32) -> bool {
        if tag == TargetType::UNKNOWN.target_type_value() {
            return true;
        }
        tag >= 0 && tag < TargetType::MAXIMUM_TARGET_TYPE_VALUE + 1
    }

    /// Java: public static TargetType fromTargetTypeValue(int tag)
    pub fn from_target_type_value(tag: i32) -> TargetType {
        if tag == TargetType::UNKNOWN.target_type_value() {
            return TargetType::UNKNOWN;
        }
        if !(0..=TargetType::MAXIMUM_TARGET_TYPE_VALUE).contains(&tag) {
            assert::error_msg(format!("Unknown TargetType: {}", tag));
        }
        match tag {
            0x00 => TargetType::CLASS_TYPE_PARAMETER,
            0x01 => TargetType::METHOD_TYPE_PARAMETER,
            0x10 => TargetType::CLASS_EXTENDS,
            0x11 => TargetType::CLASS_TYPE_PARAMETER_BOUND,
            0x12 => TargetType::METHOD_TYPE_PARAMETER_BOUND,
            0x13 => TargetType::FIELD,
            0x14 => TargetType::METHOD_RETURN,
            0x15 => TargetType::METHOD_RECEIVER,
            0x16 => TargetType::METHOD_FORMAL_PARAMETER,
            0x17 => TargetType::THROWS,
            0x40 => TargetType::LOCAL_VARIABLE,
            0x41 => TargetType::RESOURCE_VARIABLE,
            0x42 => TargetType::EXCEPTION_PARAMETER,
            0x43 => TargetType::INSTANCEOF,
            0x44 => TargetType::NEW,
            0x45 => TargetType::CONSTRUCTOR_REFERENCE,
            0x46 => TargetType::METHOD_REFERENCE,
            0x47 => TargetType::CAST,
            0x48 => TargetType::CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT,
            0x49 => TargetType::METHOD_INVOCATION_TYPE_ARGUMENT,
            0x4A => TargetType::CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT,
            0x4B => TargetType::METHOD_REFERENCE_TYPE_ARGUMENT,
            // Java: the targets array is prefilled with UNKNOWN for unused values.
            _ => TargetType::UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_locality() {
        assert_eq!(TargetType::CLASS_TYPE_PARAMETER.target_type_value(), 0x00);
        assert_eq!(TargetType::FIELD.target_type_value(), 0x13);
        assert_eq!(TargetType::LOCAL_VARIABLE.target_type_value(), 0x40);
        assert_eq!(TargetType::UNKNOWN.target_type_value(), 0xFF);
        assert!(!TargetType::FIELD.is_local());
        assert!(TargetType::LOCAL_VARIABLE.is_local());
        assert!(TargetType::CAST.is_local());
        assert!(!TargetType::UNKNOWN.is_local());
    }

    #[test]
    fn validity_matches_java() {
        assert!(TargetType::is_valid_target_type_value(0x00));
        assert!(TargetType::is_valid_target_type_value(0x4B));
        assert!(!TargetType::is_valid_target_type_value(0x4C));
        assert!(!TargetType::is_valid_target_type_value(-1));
        assert!(TargetType::is_valid_target_type_value(0xFF));
    }

    #[test]
    fn lookup_and_unused_values() {
        assert_eq!(TargetType::from_target_type_value(0x13), TargetType::FIELD);
        assert_eq!(
            TargetType::from_target_type_value(0x4B),
            TargetType::METHOD_REFERENCE_TYPE_ARGUMENT
        );
        assert_eq!(
            TargetType::from_target_type_value(0x02),
            TargetType::UNKNOWN
        );
        assert_eq!(
            TargetType::from_target_type_value(0xFF),
            TargetType::UNKNOWN
        );
        assert!(std::panic::catch_unwind(|| TargetType::from_target_type_value(0x4C)).is_err());
    }
}
