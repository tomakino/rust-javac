//! Indicates that an unknown kind of annotation value was encountered.
//!
//! Java class inheritance -> embedded `UnknownEntityException`;
//! the `Object p` argument/`getArgument` are not ported (no Rust Object); the
//! message is `"Unknown annotation value: \"" + av + "\""`, using the
//! `Display` (toString) contract of `AnnotationValue`.

use super::annotation_value::AnnotationValue;
use crate::lang::model::UnknownEntityException;
use javac_strings::JavaString;
use std::fmt;
use std::rc::Rc;

/// Indicates that an unknown kind of annotation value was encountered.
/// This can occur if the language evolves and new kinds of annotation
/// values can be stored in an annotation.  May be thrown by an
/// annotation value visitor to
/// indicate that the visitor was created for a prior version of the
/// language.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see AnnotationValueVisitor#visitUnknown
/// @since 1.6
#[derive(Clone)]
pub struct UnknownAnnotationValueException {
    /// Java: the superclass state (detail message).
    super_: UnknownEntityException,
    /// Java: `private transient AnnotationValue av;`
    av: Option<Rc<dyn AnnotationValue>>,
}

impl UnknownAnnotationValueException {
    /// Creates a new UnknownAnnotationValueException.
    ///
    /// @param av the unknown annotation value, may be null
    pub fn new(av: Option<Rc<dyn AnnotationValue>>) -> UnknownAnnotationValueException {
        let message = match av.as_ref() {
            Some(value) => format!("Unknown annotation value: \"{value}\""),
            None => "Unknown annotation value: \"null\"".to_string(),
        };
        UnknownAnnotationValueException {
            super_: UnknownEntityException::new(JavaString::from(message.as_str())),
            av,
        }
    }

    /// Returns the unknown annotation value.
    /// The value may be unavailable if this exception has been
    /// serialized and then read back in.
    ///
    /// @return the unknown annotation value, or null if unavailable
    pub fn get_unknown_annotation_value(&self) -> Option<&Rc<dyn AnnotationValue>> {
        self.av.as_ref()
    }
}

impl fmt::Display for UnknownAnnotationValueException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.super_, f)
    }
}

impl fmt::Debug for UnknownAnnotationValueException {
    // Rust: Rust Debug is the nearest analogue of the Java
    // Throwable.toString() used for debugging; the payload holds trait objects
    // that have no Debug bound, so the message is printed instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for UnknownAnnotationValueException {}

#[cfg(test)]
mod tests {
    use super::UnknownAnnotationValueException;

    #[test]
    fn null_value_message() {
        let e = UnknownAnnotationValueException::new(None);
        assert_eq!(e.to_string(), "Unknown annotation value: \"null\"");
        assert!(e.get_unknown_annotation_value().is_none());
    }
}
