//! Thrown when an application attempts to access a sequence of Class objects
//! each corresponding to a TypeMirror.
//!
//!
//! * Java class inheritance -> embedded base-class state (message + types).
//! * `List<? extends TypeMirror>` -> `Vec<Rc<dyn TypeMirror>>`; the upstream
//!   list is unmodifiable and the exception keeps a defensive copy, which the
//!   Vec does as well.
//! * serialization (`readObject`) is a JDK mechanism with no Rust counterpart;
//!   the transient-field reset is not ported (registered difference).
//! * the Java `package-private` trusted constructor used by
//!   MirroredTypeException -> `pub(crate)` constructor here.

use super::type_mirror::TypeMirror;
use javac_strings::JavaString;
use std::fmt;
use std::rc::Rc;

/// Thrown when an application attempts to access a sequence of Class objects
/// each corresponding to a TypeMirror.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see MirroredTypeException
/// @see Element#getAnnotation(Class)
/// @since 1.6
#[derive(Clone)]
pub struct MirroredTypesException {
    /// Java: the Throwable detail message.
    message: JavaString,
    /// Java: `transient List<? extends TypeMirror> types;`
    types: Vec<Rc<dyn TypeMirror>>,
}

impl MirroredTypesException {
    /// Trusted constructor to be called by MirroredTypeException.
    ///
    /// Java: MirroredTypesException(String message, TypeMirror type)
    pub(crate) fn new_single(
        message: JavaString,
        type_: Rc<dyn TypeMirror>,
    ) -> MirroredTypesException {
        MirroredTypesException {
            message,
            types: vec![type_],
        }
    }

    /// Constructs a new MirroredTypesException for the specified types.
    ///
    /// @param types  the types being accessed
    pub fn new(types: Vec<Rc<dyn TypeMirror>>) -> MirroredTypesException {
        // Java: super("Attempt to access Class objects for TypeMirrors " +
        //           (types = new ArrayList<>(types)).toString() );
        // A Java List toString is "[a, b, ...]".
        let mut rendered = String::from("[");
        for (index, mirror) in types.iter().enumerate() {
            if index > 0 {
                rendered.push_str(", ");
            }
            rendered.push_str(&mirror.to_string());
        }
        rendered.push(']');
        let message = format!("Attempt to access Class objects for TypeMirrors {rendered}");
        MirroredTypesException {
            message: JavaString::from(message.as_str()),
            types,
        }
    }

    /// Returns the type mirrors corresponding to the types being accessed.
    /// The type mirrors may be unavailable if this exception has been
    /// serialized and then read back in.
    ///
    /// @return the type mirrors in construction order, or null if unavailable
    pub fn get_type_mirrors(&self) -> &[Rc<dyn TypeMirror>] {
        &self.types
    }
}

impl fmt::Display for MirroredTypesException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message.to_string_lossy())
    }
}

impl fmt::Debug for MirroredTypesException {
    // Rust: Rust Debug is the nearest analogue of the Java
    // Throwable.toString() used for debugging; the payload holds trait objects
    // that have no Debug bound, so the message is printed instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for MirroredTypesException {}
