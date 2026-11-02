//! Indicates that an unknown kind of module directive was encountered.
//!
//! Java class inheritance -> embedded `UnknownEntityException`;
//! the `Object p` argument/`getArgument` are not ported (no Rust Object); the
//! message is `"Unknown directive: " + d`, using the `Display` (toString)
//! contract of `Directive`.

use super::module_element::Directive;
use crate::lang::model::UnknownEntityException;
use javac_strings::JavaString;
use std::fmt;
use std::rc::Rc;

/// Indicates that an unknown kind of module directive was encountered.
/// This can occur if the language evolves and new kinds of directives are
/// added to the Directive hierarchy.  May be thrown by a
/// directive visitor to
/// indicate that the visitor was created for a prior version of the language.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see ModuleElement.DirectiveVisitor#visitUnknown
/// @since 9
#[derive(Clone)]
pub struct UnknownDirectiveException {
    /// Java: the superclass state (detail message).
    super_: UnknownEntityException,
    /// Java: `private final transient ModuleElement.Directive directive;`
    directive: Option<Rc<dyn Directive>>,
}

impl UnknownDirectiveException {
    /// Creates a new UnknownDirectiveException.
    ///
    /// @param d the unknown directive, may be null
    pub fn new(directive: Option<Rc<dyn Directive>>) -> UnknownDirectiveException {
        let message = match directive.as_ref() {
            Some(d) => format!("Unknown directive: {d}"),
            None => "Unknown directive: null".to_string(),
        };
        UnknownDirectiveException {
            super_: UnknownEntityException::new(JavaString::from(message.as_str())),
            directive,
        }
    }

    /// Returns the unknown directive.
    /// The value may be unavailable if this exception has been
    /// serialized and then read back in.
    ///
    /// @return the unknown directive, or null if unavailable
    pub fn get_unknown_directive(&self) -> Option<&Rc<dyn Directive>> {
        self.directive.as_ref()
    }
}

impl std::fmt::Display for UnknownDirectiveException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.super_, f)
    }
}

impl fmt::Debug for UnknownDirectiveException {
    // Rust: Rust Debug is the nearest analogue of the Java
    // Throwable.toString() used for debugging; the payload holds trait objects
    // that have no Debug bound, so the message is printed instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for UnknownDirectiveException {}

#[cfg(test)]
mod tests {
    use super::UnknownDirectiveException;

    #[test]
    fn null_directive_message() {
        let e = UnknownDirectiveException::new(None);
        assert_eq!(e.to_string(), "Unknown directive: null");
        assert!(e.get_unknown_directive().is_none());
    }
}
