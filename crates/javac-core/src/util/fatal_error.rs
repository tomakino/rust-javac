// Origin: com/sun/tools/javac/util/FatalError.java @ 982d53c (jdk-17.0.19-ga)

//! Throwing an instance of this class causes immediate termination of the main
//! compiler method. It is used when some non-recoverable error has been detected
//! in the compiler environment at runtime.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::any::Any;
use std::panic::panic_any;

/// Payload of the Java com.sun.tools.javac.util.FatalError error.
#[derive(Debug)]
pub struct FatalError {
    pub message: String,
    pub cause: Option<Box<dyn Any + Send>>,
}

impl FatalError {
    /// Java: public FatalError(String s) { super(s); }
    pub fn new(message: impl Into<String>) -> Self {
        FatalError { message: message.into(), cause: None }
    }

    /// Java: public FatalError(String s, Throwable t) — internal helper.
    pub fn with_cause(message: impl Into<String>, cause: Box<dyn Any + Send>) -> Self {
        FatalError { message: message.into(), cause: Some(cause) }
    }

    // TODO(P0+): Java constructors FatalError(JCDiagnostic) and FatalError(JCDiagnostic, Throwable)
    // are added as soon as util::JCDiagnostic is ported (they format the diagnostic via d.toString()).
}

impl std::fmt::Display for FatalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Java: throw new FatalError(s);
#[track_caller]
pub fn fatal_error(message: impl Into<String>) -> ! {
    panic_any(FatalError::new(message))
}
