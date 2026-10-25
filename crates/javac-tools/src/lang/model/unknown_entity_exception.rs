// Origin: javax/lang/model/UnknownEntityException.java @ 982d53c (jdk-17.0.19-ga)

//! Superclass of exceptions which indicate that an unknown kind of entity was
//! encountered.
//!
//! PORTING_RULES: Java class hierarchy is not available in Rust.  This struct
//! keeps the upstream state (detail message, Java String) and the exception
//! behavior; the subclasses (UnknownElementException, ...) embed it as their
//! first member instead of extending it.  Java `RuntimeException` maps to a
//! Rust error value here because the value is part of the visitor contract;
//! the abstract visitors turn it into `panic!` at the point where Java throws
//! it (the entry points catch_unwind, docs/PORTING_RULES.md §6).

use javac_strings::JavaString;
use std::fmt;

/// Superclass of exceptions which indicate that an unknown kind of
/// entity was encountered.  This situation can occur if the language
/// evolves and new kinds of constructs are introduced.  Subclasses of
/// this exception may be thrown by visitors to indicate that the
/// visitor was created for a prior version of the language.
///
/// @author Joseph D. Darcy
/// @see javax.lang.model.element.UnknownElementException
/// @see javax.lang.model.element.UnknownAnnotationValueException
/// @see javax.lang.model.type.UnknownTypeException
/// @since 1.7
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEntityException {
    /// Java: the detail message of the exception (String).
    message: JavaString,
}

impl UnknownEntityException {
    /// Creates a new UnknownEntityException with the specified
    /// detail message.
    ///
    /// @param message the detail message
    ///
    /// Java: protected UnknownEntityException(String message) {
    ///           super(message); }
    pub fn new(message: JavaString) -> UnknownEntityException {
        UnknownEntityException { message }
    }

    /// Java: Throwable.getMessage().
    pub fn get_message(&self) -> &JavaString {
        &self.message
    }
}

impl fmt::Display for UnknownEntityException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message.to_string_lossy())
    }
}

impl std::error::Error for UnknownEntityException {}

#[cfg(test)]
mod tests {
    use super::UnknownEntityException;
    use javac_strings::JavaString;

    #[test]
    fn keeps_message() {
        let e = UnknownEntityException::new(JavaString::from("unknown"));
        assert_eq!(e.get_message(), &JavaString::from("unknown"));
        assert_eq!(e.to_string(), "unknown");
    }
}
