//! The kind of an element.

use javac_strings::JavaString;

/// The kind of an element.
///
/// <p>Note that it is possible additional element kinds will be added
/// to accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see Element
/// @since 1.6
///
/// Java enum constants keep their Java names
/// (UPPER_SNAKE_CASE) and declaration order.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementKind {
    /// A package.
    PACKAGE,

    // Declared types
    /// An enum class.
    ENUM,
    /// A class not described by a more specific kind (like ENUM or RECORD).
    CLASS,

    /// An annotation interface. (Formerly known as an annotation type.)
    ANNOTATION_TYPE,
    /// An interface not described by a more specific kind (like
    /// ANNOTATION_TYPE).
    INTERFACE,

    // Variables
    /// An enum constant.
    ENUM_CONSTANT,
    /// A field not described by a more specific kind (like ENUM_CONSTANT).
    FIELD,
    /// A parameter of a method or constructor.
    PARAMETER,
    /// A local variable.
    LOCAL_VARIABLE,
    /// A parameter of an exception handler.
    EXCEPTION_PARAMETER,

    // Executables
    /// A method.
    METHOD,
    /// A constructor.
    CONSTRUCTOR,
    /// A static initializer.
    STATIC_INIT,
    /// An instance initializer.
    INSTANCE_INIT,

    /// A type parameter.
    TYPE_PARAMETER,

    /// An implementation-reserved element.  This is not the element
    /// you are looking for.
    OTHER,

    // Constants added since initial release
    /// A resource variable.
    /// @since 1.7
    RESOURCE_VARIABLE,

    /// A module.
    /// @since 9
    MODULE,

    /// A record class.
    /// @since 16
    RECORD,

    /// A record component of a record.
    /// @since 16
    RECORD_COMPONENT,

    /// A binding variable in a pattern.
    /// @since 16
    BINDING_VARIABLE,
}

impl ElementKind {
    /// Returns true if this is a kind of class:
    /// either CLASS or ENUM or RECORD.
    ///
    /// @return true if this is a kind of class
    pub fn is_class(&self) -> bool {
        *self == ElementKind::CLASS || *self == ElementKind::ENUM || *self == ElementKind::RECORD
    }

    /// Returns true if this is a kind of interface:
    /// either INTERFACE or ANNOTATION_TYPE.
    ///
    /// @return true if this is a kind of interface
    pub fn is_interface(&self) -> bool {
        *self == ElementKind::INTERFACE || *self == ElementKind::ANNOTATION_TYPE
    }

    /// Returns true if this is a kind of field:
    /// either FIELD or ENUM_CONSTANT.
    ///
    /// @return true if this is a kind of field
    pub fn is_field(&self) -> bool {
        *self == ElementKind::FIELD || *self == ElementKind::ENUM_CONSTANT
    }

    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            ElementKind::PACKAGE => "PACKAGE",
            ElementKind::ENUM => "ENUM",
            ElementKind::CLASS => "CLASS",
            ElementKind::ANNOTATION_TYPE => "ANNOTATION_TYPE",
            ElementKind::INTERFACE => "INTERFACE",
            ElementKind::ENUM_CONSTANT => "ENUM_CONSTANT",
            ElementKind::FIELD => "FIELD",
            ElementKind::PARAMETER => "PARAMETER",
            ElementKind::LOCAL_VARIABLE => "LOCAL_VARIABLE",
            ElementKind::EXCEPTION_PARAMETER => "EXCEPTION_PARAMETER",
            ElementKind::METHOD => "METHOD",
            ElementKind::CONSTRUCTOR => "CONSTRUCTOR",
            ElementKind::STATIC_INIT => "STATIC_INIT",
            ElementKind::INSTANCE_INIT => "INSTANCE_INIT",
            ElementKind::TYPE_PARAMETER => "TYPE_PARAMETER",
            ElementKind::OTHER => "OTHER",
            ElementKind::RESOURCE_VARIABLE => "RESOURCE_VARIABLE",
            ElementKind::MODULE => "MODULE",
            ElementKind::RECORD => "RECORD",
            ElementKind::RECORD_COMPONENT => "RECORD_COMPONENT",
            ElementKind::BINDING_VARIABLE => "BINDING_VARIABLE",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [ElementKind; 21] {
        [
            ElementKind::PACKAGE,
            ElementKind::ENUM,
            ElementKind::CLASS,
            ElementKind::ANNOTATION_TYPE,
            ElementKind::INTERFACE,
            ElementKind::ENUM_CONSTANT,
            ElementKind::FIELD,
            ElementKind::PARAMETER,
            ElementKind::LOCAL_VARIABLE,
            ElementKind::EXCEPTION_PARAMETER,
            ElementKind::METHOD,
            ElementKind::CONSTRUCTOR,
            ElementKind::STATIC_INIT,
            ElementKind::INSTANCE_INIT,
            ElementKind::TYPE_PARAMETER,
            ElementKind::OTHER,
            ElementKind::RESOURCE_VARIABLE,
            ElementKind::MODULE,
            ElementKind::RECORD,
            ElementKind::RECORD_COMPONENT,
            ElementKind::BINDING_VARIABLE,
        ]
    }
}

impl std::fmt::Display for ElementKind {
    /// Java: Enum.toString() = name().
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::ElementKind;
    use javac_strings::JavaString;

    #[test]
    fn classes() {
        assert!(ElementKind::CLASS.is_class());
        assert!(ElementKind::ENUM.is_class());
        assert!(ElementKind::RECORD.is_class());
        assert!(!ElementKind::INTERFACE.is_class());
        assert!(!ElementKind::ANNOTATION_TYPE.is_class());
    }

    #[test]
    fn interfaces() {
        assert!(ElementKind::INTERFACE.is_interface());
        assert!(ElementKind::ANNOTATION_TYPE.is_interface());
        assert!(!ElementKind::CLASS.is_interface());
    }

    #[test]
    fn fields() {
        assert!(ElementKind::FIELD.is_field());
        assert!(ElementKind::ENUM_CONSTANT.is_field());
        assert!(!ElementKind::PARAMETER.is_field());
    }

    #[test]
    fn enum_shape() {
        assert_eq!(ElementKind::values().len(), 21);
        assert_eq!(ElementKind::values()[0], ElementKind::PACKAGE);
        assert_eq!(
            ElementKind::BINDING_VARIABLE.name(),
            JavaString::from("BINDING_VARIABLE")
        );
        assert_eq!(ElementKind::MODULE.to_string(), "MODULE");
    }
}
