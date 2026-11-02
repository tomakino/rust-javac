// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.charset exception classes (the subset javac uses:
// IllegalCharsetNameException, UnsupportedCharsetException,
// CharacterCodingException, MalformedInputException,
// UnmappableCharacterException).  Why not 1:1: the Java exceptions are
// unchecked except CharacterCodingException; the port maps the declared
// exception role onto Result and keeps the Java class names in the error
// variants/messages.

//! Errors of the charset layer.
//!
//! java.nio.charset.IllegalCharsetNameException and
//! UnsupportedCharsetException map to [CharsetError]; CharacterCodingException
//! (MalformedInputException / UnmappableCharacterException) maps to
//! [CharacterCodingError].  The Java getInputLength() value is kept as
//! input_length.

use std::fmt;

/// Java: Charset.forName failures (IllegalCharsetNameException /
/// UnsupportedCharsetException).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CharsetError {
    /// Java: java.nio.charset.IllegalCharsetNameException.
    IllegalCharsetName { name: String },
    /// Java: java.nio.charset.UnsupportedCharsetException.
    UnsupportedCharset { name: String },
}

impl fmt::Display for CharsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CharsetError::IllegalCharsetName { name } => {
                write!(f, "java.nio.charset.IllegalCharsetNameException: {}", name)
            }
            CharsetError::UnsupportedCharset { name } => {
                write!(f, "java.nio.charset.UnsupportedCharsetException: {}", name)
            }
        }
    }
}

impl std::error::Error for CharsetError {}

/// Java: java.nio.charset.CharacterCodingException (MalformedInputException /
/// UnmappableCharacterException).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CharacterCodingError {
    /// Java: java.nio.charset.MalformedInputException (getInputLength()).
    MalformedInput { input_length: usize },
    /// Java: java.nio.charset.UnmappableCharacterException (getInputLength()).
    UnmappableCharacter { input_length: usize },
}

impl fmt::Display for CharacterCodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CharacterCodingError::MalformedInput { input_length } => write!(
                f,
                "java.nio.charset.MalformedInputException: Input length = {}",
                input_length
            ),
            CharacterCodingError::UnmappableCharacter { input_length } => write!(
                f,
                "java.nio.charset.UnmappableCharacterException: Input length = {}",
                input_length
            ),
        }
    }
}

impl std::error::Error for CharacterCodingError {}
