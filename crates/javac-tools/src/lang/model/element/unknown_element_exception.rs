// Origin: javax/lang/model/element/UnknownElementException.java @ 982d53c (jdk-17.0.19-ga)

//! Indicates that an unknown kind of element was encountered.
//!
//! PORTING_RULES:
//! * Java class inheritance -> embedded `UnknownEntityException`.
//! * the Java `Object p` "additional argument" is stored upstream only for
//!   `getArgument()`; `Object` has no Rust counterpart in this API, so the
//!   field and `getArgument` are not ported (registered difference).
//! * the message is `"Unknown element: \"" + e + "\""`; the element text is its
//!   `toString()`, which is the `Display` contract of `Element`.

use super::element::Element;
use crate::lang::model::UnknownEntityException;
use javac_strings::JavaString;
use std::fmt;
use std::rc::Rc;

/// Indicates that an unknown kind of element was encountered.  This
/// can occur if the language evolves and new kinds of elements are
/// added to the Element hierarchy.  May be thrown by an
/// element visitor to indicate that the
/// visitor was created for a prior version of the language.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see ElementVisitor#visitUnknown
/// @since 1.6
#[derive(Clone)]
pub struct UnknownElementException {
    /// Java: the superclass state (detail message).
    super_: UnknownEntityException,
    /// Java: `private transient Element element;`
    element: Option<Rc<dyn Element>>,
}

impl UnknownElementException {
    /// Creates a new UnknownElementException.
    ///
    /// @param e the unknown element, may be null
    ///
    /// Java: public UnknownElementException(Element e, Object p) {
    ///           super("Unknown element: \"" + e + "\"");
    ///           element = e; ... }
    pub fn new(element: Option<Rc<dyn Element>>) -> UnknownElementException {
        let message = match element.as_ref() {
            Some(e) => format!("Unknown element: \"{e}\""),
            None => "Unknown element: \"null\"".to_string(),
        };
        UnknownElementException {
            super_: UnknownEntityException::new(JavaString::from(message.as_str())),
            element,
        }
    }

    /// Returns the unknown element.
    /// The value may be unavailable if this exception has been
    /// serialized and then read back in.
    ///
    /// @return the unknown element, or null if unavailable
    pub fn get_unknown_element(&self) -> Option<&Rc<dyn Element>> {
        self.element.as_ref()
    }
}

impl std::fmt::Display for UnknownElementException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.super_, f)
    }
}

impl fmt::Debug for UnknownElementException {
    // PORTING_RULES: Rust Debug is the nearest analogue of the Java
    // Throwable.toString() used for debugging; the payload holds trait objects
    // that have no Debug bound, so the message is printed instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for UnknownElementException {}

#[cfg(test)]
mod tests {
    use super::UnknownElementException;

    #[test]
    fn null_element_message() {
        let e = UnknownElementException::new(None);
        assert_eq!(e.to_string(), "Unknown element: \"null\"");
        assert!(e.get_unknown_element().is_none());
    }
}
