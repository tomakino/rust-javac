//! Represents a field, enum constant, method or constructor parameter, local
//! variable, resource variable, or exception parameter.
//!
//!
//! * `Object getConstantValue()` returns null when the variable is not a
//!   compile-time constant, so the Rust method returns
//!   `Option<ConstantValue>`.  The upstream documentation says the value is "of
//!   a primitive type or a String", wrapped in the corresponding wrapper class;
//!   `ConstantValue` is that closed set (registered representation contract).
//! * the Java `@Override` re-declarations of `asType`, `getSimpleName` and
//!   `getEnclosingElement` only narrow their documentation; Rust inherits them
//!   from `Element` and does not repeat them.

use super::element::Element;
use javac_strings::JavaString;

/// The value of a compile-time constant variable.
///
/// representation of the Java `Object` returned by
/// `VariableElement.getConstantValue`; the upstream documentation restricts it
/// to a primitive wrapper or a String.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
    /// Java: a `Boolean` wrapper value.
    Boolean(bool),
    /// Java: a `Byte` wrapper value.
    Byte(i8),
    /// Java: a `Character` wrapper value (UTF-16 code unit).
    Character(u16),
    /// Java: a `Double` wrapper value.
    Double(f64),
    /// Java: a `Float` wrapper value.
    Float(f32),
    /// Java: an `Integer` wrapper value.
    Integer(i32),
    /// Java: a `Long` wrapper value.
    Long(i64),
    /// Java: a `Short` wrapper value.
    Short(i16),
    /// Java: a String value.
    String(JavaString),
}

/// Represents a field, enum constant, method or constructor
/// parameter, local variable, resource variable, or exception
/// parameter.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait VariableElement: Element {
    /// Returns the value of this variable if this is a final
    /// field initialized to a compile-time constant.  Returns
    /// null otherwise.  The value will be of a primitive type or a
    /// String.  If the value is of a primitive type, it is
    /// wrapped in the appropriate wrapper class (such as Integer).
    ///
    /// <p>Note that not all final fields will have
    /// constant values.  In particular, enum constants are
    /// <em>not</em> considered to be compile-time constants.  To have a
    /// constant value, a field's type must be either a primitive type
    /// or String.
    ///
    /// @return the value of this variable if this is a final
    /// field initialized to a compile-time constant, or null
    /// otherwise
    ///
    /// @see Elements#getConstantExpression(Object)
    /// @jls 15.29 Constant Expressions
    /// @jls 4.12.4 final Variables
    fn get_constant_value(&self) -> Option<ConstantValue>;
}
