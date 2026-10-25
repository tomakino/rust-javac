// Origin: com/sun/tools/javac/util/PropagatedException.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2/P3 utility-service batch.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getCause (PropagatedException.java:47)
// End of §19 gap registration.

//! Used to propagate exceptions through to the user.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::any::Any;
use std::panic::panic_any;

/// Payload of the Java com.sun.tools.javac.util.PropagatedException runtime exception.
#[derive(Debug)]
pub struct PropagatedException {
    pub cause: Box<dyn Any + Send>,
}

impl PropagatedException {
    /// Java: public PropagatedException(RuntimeException cause) { super(cause); }
    pub fn new(cause: Box<dyn Any + Send>) -> Self {
        PropagatedException { cause }
    }
}

/// Java: throw new PropagatedException(cause);
#[track_caller]
pub fn throw(cause: Box<dyn Any + Send>) -> ! {
    panic_any(PropagatedException::new(cause))
}
