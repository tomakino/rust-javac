//! Simple facility for unconditional assertions.
//! The methods in this class are described in terms of equivalent assert
//! statements, assuming that assertions have been enabled.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::panic::panic_any;

/// Payload of the Java java.lang.AssertionError thrown by these assertions.
///
/// The top level error printer formats it exactly like Java:
/// "java.lang.AssertionError" or "java.lang.AssertionError: <msg>".
#[derive(Debug)]
pub struct AssertionError {
    pub message: Option<String>,
}

impl AssertionError {
    pub fn new(message: Option<String>) -> Self {
        AssertionError { message }
    }
}

impl std::fmt::Display for AssertionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.message {
            None => write!(f, "java.lang.AssertionError"),
            Some(msg) => write!(f, "java.lang.AssertionError: {}", msg),
        }
    }
}

/// Equivalent to: assert cond;
pub fn check(cond: bool) {
    if !cond {
        error();
    }
}

/// Equivalent to: assert (o == null);
pub fn check_null<T>(o: Option<&T>) {
    if o.is_some() {
        error();
    }
}

/// Equivalent to: assert (t != null); return t;
pub fn check_non_null<T>(t: Option<T>) -> T {
    match t {
        Some(t) => t,
        None => error(),
    }
}

/// Equivalent to: assert cond : value;  (Java: String.valueOf(value))
pub fn check_value<T: ToString>(cond: bool, value: T) {
    if !cond {
        error_msg(value.to_string());
    }
}

/// Equivalent to: assert cond : msg;
pub fn check_msg(cond: bool, msg: &str) {
    if !cond {
        error_msg(msg);
    }
}

/// Equivalent to: assert cond : msg.get();  (message computed lazily)
pub fn check_lazy(cond: bool, msg: impl FnOnce() -> String) {
    if !cond {
        error_msg(msg());
    }
}

/// Equivalent to: assert (o == null) : value;
pub fn check_null_value<T, V: ToString>(o: Option<&T>, value: V) {
    if o.is_some() {
        error_msg(value.to_string());
    }
}

/// Equivalent to: assert (o == null) : msg;
pub fn check_null_msg<T>(o: Option<&T>, msg: &str) {
    if o.is_some() {
        error_msg(msg);
    }
}

/// Equivalent to: assert (o == null) : msg.get();
pub fn check_null_lazy<T>(o: Option<&T>, msg: impl FnOnce() -> String) {
    if o.is_some() {
        error_msg(msg());
    }
}

/// Equivalent to: assert (o != null) : msg;
pub fn check_non_null_msg<T>(t: Option<T>, msg: &str) -> T {
    match t {
        Some(t) => t,
        None => error_msg(msg),
    }
}

/// Equivalent to: assert (o != null) : msg.get();
pub fn check_non_null_lazy<T>(t: Option<T>, msg: impl FnOnce() -> String) -> T {
    match t {
        Some(t) => t,
        None => error_msg(msg()),
    }
}

/// Equivalent to: assert false;
#[track_caller]
pub fn error() -> ! {
    panic_any(AssertionError::new(None))
}

/// Equivalent to: assert false : msg;
#[track_caller]
pub fn error_msg(msg: impl Into<String>) -> ! {
    panic_any(AssertionError::new(Some(msg.into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_ok() {
        check(true);
        check_msg(true, "unused");
    }

    #[test]
    fn check_fails() {
        assert!(std::panic::catch_unwind(|| check(false)).is_err());
    }

    #[test]
    fn assertion_error_display() {
        assert_eq!(
            AssertionError::new(None).to_string(),
            "java.lang.AssertionError"
        );
        assert_eq!(
            AssertionError::new(Some("x".to_string())).to_string(),
            "java.lang.AssertionError: x"
        );
    }

    #[test]
    fn check_non_null_works() {
        assert_eq!(check_non_null(Some(7)), 7);
        assert!(std::panic::catch_unwind(|| check_non_null::<i32>(None)).is_err());
    }
}
