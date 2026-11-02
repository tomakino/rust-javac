//! Thrown when an application attempts to access the Class object
//! corresponding to a TypeMirror.
//!
//! Java `extends MirroredTypesException` -> embedded base-class
//! instance (`super_`); the inherited `types` list contains exactly the single
//! type, exactly as upstream.  Serialization (`readObject`) is not ported.

use super::mirrored_types_exception::MirroredTypesException;
use super::type_mirror::TypeMirror;
use javac_strings::JavaString;
use std::fmt;
use std::rc::Rc;

/// Thrown when an application attempts to access the {@link Class} object
/// corresponding to a {@link TypeMirror}.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see MirroredTypesException
/// @see Element#getAnnotation(Class)
/// @since 1.6
#[derive(Clone)]
pub struct MirroredTypeException {
    /// Java: the inherited MirroredTypesException state.
    super_: MirroredTypesException,
    /// Java: `private transient TypeMirror type;`
    type_: Rc<dyn TypeMirror>,
}

impl MirroredTypeException {
    /// Constructs a new MirroredTypeException for the specified type.
    ///
    /// @param type  the type being accessed
    ///
    /// Java: public MirroredTypeException(TypeMirror type) {
    ///           super("Attempt to access Class object for TypeMirror " +
    ///                 type.toString(), type);
    ///           this.type = type; }
    pub fn new(type_: Rc<dyn TypeMirror>) -> MirroredTypeException {
        let message =
            JavaString::from(format!("Attempt to access Class object for TypeMirror {type_}").as_str());
        MirroredTypeException {
            super_: MirroredTypesException::new_single(message, type_.clone()),
            type_,
        }
    }

    /// Returns the type mirror corresponding to the type being accessed.
    /// The type mirror may be unavailable if this exception has been
    /// serialized and then read back in.
    ///
    /// @return the type mirror, or null if unavailable
    pub fn get_type_mirror(&self) -> &Rc<dyn TypeMirror> {
        &self.type_
    }
}

impl fmt::Display for MirroredTypeException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.super_, f)
    }
}

impl fmt::Debug for MirroredTypeException {
    // Rust: Rust Debug is the nearest analogue of the Java
    // Throwable.toString() used for debugging; the payload holds trait objects
    // that have no Debug bound, so the message is printed instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for MirroredTypeException {}
