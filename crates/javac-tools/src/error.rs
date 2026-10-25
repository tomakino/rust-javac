// PORTING ADAPTER (not a 1:1 port)
// Adapter for: java.io.IOException, java.lang.UnsupportedOperationException,
//              java.lang.IllegalArgumentException, java.lang.IllegalStateException.
// Why not 1:1: Rust has no checked exceptions and no exception hierarchy; the
//              observable Java exception paths are expressed as Result (checked
//              IOException / declared UnsupportedOperationException) or panic
//              (API misuse), see the mapping below.
//! Mapping of the Java exceptions that participate in the `javax.tools`
//! contract.
//!
//! PORTING_RULES: the `javax.tools` methods declare `java.io.IOException`
//! and two further classes of failures are part of the observable contract:
//! `UnsupportedOperationException` (the documented result of several default
//! implementations) and programming-error exceptions
//! (`IllegalArgumentException`, `IllegalStateException`,
//! `NullPointerException`).
//!
//! * checked `IOException`                      -> `Result<T, Error>` as
//!   `Error::IOException`
//! * declared `UnsupportedOperationException`   -> `Err(Error::UnsupportedOperationException)`
//! * other runtime exceptions                   -> `panic!` at the call site
//!   (they express API misuse; entry points catch_unwind, see
//!   docs/PORTING_RULES.md §6)
//!
//! TODO(P6): replace this placeholder with the error type introduced by the
//! `file`/`util` port; the variant names stay aligned with the Java exception
//! classes so that the exception mapping remains explicit.

use javac_strings::JavaString;
use std::fmt;

/// Placeholder for the Java exceptions raised by `javax.tools` methods.
///
/// Java: `java.io.IOException` (checked) and
/// `java.lang.UnsupportedOperationException` (declared result of the default
/// implementations).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Java: `java.io.IOException`.  The message is a Java String
    /// (`Exception.getMessage()`).
    IOException(JavaString),
    /// Java: `java.lang.UnsupportedOperationException`.
    UnsupportedOperationException,
}

impl Error {
    /// Creates an `IOException` with the given message.
    pub fn io(message: JavaString) -> Error {
        Error::IOException(message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IOException(message) => f.write_str(&message.to_string_lossy()),
            Error::UnsupportedOperationException => f.write_str("UnsupportedOperationException"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::Error;
    use javac_strings::JavaString;

    #[test]
    fn io_error_keeps_message() {
        let e = Error::io(JavaString::from("cannot read"));
        assert_eq!(e, Error::IOException(JavaString::from("cannot read")));
        assert_eq!(e.to_string(), "cannot read");
    }

    #[test]
    fn unsupported_operation_display() {
        assert_eq!(
            Error::UnsupportedOperationException.to_string(),
            "UnsupportedOperationException"
        );
    }
}
