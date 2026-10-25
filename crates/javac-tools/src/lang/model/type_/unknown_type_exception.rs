// Origin: javax/lang/model/type/UnknownTypeException.java @ 982d53c (jdk-17.0.19-ga)

//! Indicates that an unknown kind of type was encountered.
//!
//! PORTING_RULES: Java class inheritance -> embedded `UnknownEntityException`;
//! the `Object p` argument/`getArgument` are not ported (no Rust Object); the
//! message is `"Unknown type: \"" + t + "\""`, using the `Display` (toString)
//! contract of `TypeMirror`.

use super::type_mirror::TypeMirror;
use crate::lang::model::UnknownEntityException;
use javac_strings::JavaString;
use std::fmt;
use std::rc::Rc;

/// Indicates that an unknown kind of type was encountered.  This can
/// occur if the language evolves and new kinds of types are added to
/// the TypeMirror hierarchy.  May be thrown by a
/// type visitor to indicate that the visitor was created
/// for a prior version of the language.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see TypeVisitor#visitUnknown
/// @since 1.6
#[derive(Clone)]
pub struct UnknownTypeException {
    /// Java: the superclass state (detail message).
    super_: UnknownEntityException,
    /// Java: `private transient TypeMirror type;`
    type_: Option<Rc<dyn TypeMirror>>,
}

impl UnknownTypeException {
    /// Creates a new UnknownTypeException.
    ///
    /// @param t the unknown type, may be null
    pub fn new(t: Option<Rc<dyn TypeMirror>>) -> UnknownTypeException {
        let message = match t.as_ref() {
            Some(mirror) => format!("Unknown type: \"{mirror}\""),
            None => "Unknown type: \"null\"".to_string(),
        };
        UnknownTypeException {
            super_: UnknownEntityException::new(JavaString::from(message.as_str())),
            type_: t,
        }
    }

    /// Returns the unknown type.
    /// The value may be unavailable if this exception has been
    /// serialized and then read back in.
    ///
    /// @return the unknown type, or null if unavailable
    pub fn get_unknown_type(&self) -> Option<&Rc<dyn TypeMirror>> {
        self.type_.as_ref()
    }
}

impl std::fmt::Display for UnknownTypeException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.super_, f)
    }
}

impl fmt::Debug for UnknownTypeException {
    // PORTING_RULES: Rust Debug is the nearest analogue of the Java
    // Throwable.toString() used for debugging; the payload holds trait objects
    // that have no Debug bound, so the message is printed instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for UnknownTypeException {}

#[cfg(test)]
mod tests {
    use super::UnknownTypeException;

    #[test]
    fn null_type_message() {
        let e = UnknownTypeException::new(None);
        assert_eq!(e.to_string(), "Unknown type: \"null\"");
        assert!(e.get_unknown_type().is_none());
    }
}
