//! The kind of a type mirror.

use javac_strings::JavaString;

/// The kind of a type mirror.
///
/// <p>Note that it is possible additional type kinds will be added to
/// accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see TypeMirror
/// @since 1.6
///
/// PORTING_RULES: Java enum constants keep their Java names and order.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// The primitive type boolean.
    BOOLEAN,

    /// The primitive type byte.
    BYTE,

    /// The primitive type short.
    SHORT,

    /// The primitive type int.
    INT,

    /// The primitive type long.
    LONG,

    /// The primitive type char.
    CHAR,

    /// The primitive type float.
    FLOAT,

    /// The primitive type double.
    DOUBLE,

    /// The pseudo-type corresponding to the keyword void.
    /// @see NoType
    VOID,

    /// A pseudo-type used where no actual type is appropriate.
    /// @see NoType
    NONE,

    /// The null type.
    NULL,

    /// An array type.
    ARRAY,

    /// A class or interface type.
    DECLARED,

    /// A class or interface type that could not be resolved.
    ERROR,

    /// A type variable.
    TYPEVAR,

    /// A wildcard type argument.
    WILDCARD,

    /// A pseudo-type corresponding to a package element.
    /// @see NoType
    PACKAGE,

    /// A method, constructor, or initializer.
    EXECUTABLE,

    /// An implementation-reserved type.
    /// This is not the type you are looking for.
    OTHER,

    /// A union type.
    ///
    /// @since 1.7
    UNION,

    /// An intersection type.
    ///
    /// @since 1.8
    INTERSECTION,

    /// A pseudo-type corresponding to a module element.
    /// @see NoType
    /// @since 9
    MODULE,
}

impl TypeKind {
    /// {@return true if this kind corresponds to a primitive
    /// type and false otherwise}
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            TypeKind::BOOLEAN
                | TypeKind::BYTE
                | TypeKind::SHORT
                | TypeKind::INT
                | TypeKind::LONG
                | TypeKind::CHAR
                | TypeKind::FLOAT
                | TypeKind::DOUBLE
        )
    }

    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            TypeKind::BOOLEAN => "BOOLEAN",
            TypeKind::BYTE => "BYTE",
            TypeKind::SHORT => "SHORT",
            TypeKind::INT => "INT",
            TypeKind::LONG => "LONG",
            TypeKind::CHAR => "CHAR",
            TypeKind::FLOAT => "FLOAT",
            TypeKind::DOUBLE => "DOUBLE",
            TypeKind::VOID => "VOID",
            TypeKind::NONE => "NONE",
            TypeKind::NULL => "NULL",
            TypeKind::ARRAY => "ARRAY",
            TypeKind::DECLARED => "DECLARED",
            TypeKind::ERROR => "ERROR",
            TypeKind::TYPEVAR => "TYPEVAR",
            TypeKind::WILDCARD => "WILDCARD",
            TypeKind::PACKAGE => "PACKAGE",
            TypeKind::EXECUTABLE => "EXECUTABLE",
            TypeKind::OTHER => "OTHER",
            TypeKind::UNION => "UNION",
            TypeKind::INTERSECTION => "INTERSECTION",
            TypeKind::MODULE => "MODULE",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [TypeKind; 22] {
        [
            TypeKind::BOOLEAN,
            TypeKind::BYTE,
            TypeKind::SHORT,
            TypeKind::INT,
            TypeKind::LONG,
            TypeKind::CHAR,
            TypeKind::FLOAT,
            TypeKind::DOUBLE,
            TypeKind::VOID,
            TypeKind::NONE,
            TypeKind::NULL,
            TypeKind::ARRAY,
            TypeKind::DECLARED,
            TypeKind::ERROR,
            TypeKind::TYPEVAR,
            TypeKind::WILDCARD,
            TypeKind::PACKAGE,
            TypeKind::EXECUTABLE,
            TypeKind::OTHER,
            TypeKind::UNION,
            TypeKind::INTERSECTION,
            TypeKind::MODULE,
        ]
    }
}

impl std::fmt::Display for TypeKind {
    /// Java: Enum.toString() = name().
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::TypeKind;
    use javac_strings::JavaString;

    #[test]
    fn primitives() {
        for kind in [
            TypeKind::BOOLEAN,
            TypeKind::BYTE,
            TypeKind::SHORT,
            TypeKind::INT,
            TypeKind::LONG,
            TypeKind::CHAR,
            TypeKind::FLOAT,
            TypeKind::DOUBLE,
        ] {
            assert!(kind.is_primitive(), "{kind}");
        }
        assert!(!TypeKind::VOID.is_primitive());
        assert!(!TypeKind::DECLARED.is_primitive());
        assert!(!TypeKind::MODULE.is_primitive());
    }

    #[test]
    fn enum_shape() {
        assert_eq!(TypeKind::values().len(), 22);
        assert_eq!(TypeKind::values()[0], TypeKind::BOOLEAN);
        assert_eq!(TypeKind::values()[21], TypeKind::MODULE);
        assert_eq!(TypeKind::MODULE.name(), JavaString::from("MODULE"));
        assert_eq!(TypeKind::TYPEVAR.to_string(), "TYPEVAR");
    }
}
