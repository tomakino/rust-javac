// Origin: com/sun/tools/javac/util/Abort.java @ 982d53c (jdk-17.0.19-ga)

//! Throwing an instance of this class causes (silent) termination of the main
//! compiler method.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::any::Any;
use std::panic::panic_any;

/// Payload of the Java com.sun.tools.javac.util.Abort error.
#[derive(Debug)]
pub struct Abort {
    pub cause: Option<Box<dyn Any + Send>>,
}

impl Abort {
    /// Java: public Abort() { super(); }
    pub fn new() -> Self {
        Abort { cause: None }
    }

    /// Java: public Abort(Throwable cause) { super(cause); }
    pub fn with_cause(cause: Box<dyn Any + Send>) -> Self {
        Abort { cause: Some(cause) }
    }
}

impl Default for Abort {
    fn default() -> Self {
        Abort::new()
    }
}

/// Java: throw new Abort();
#[track_caller]
pub fn abort() -> ! {
    panic_any(Abort::new())
}

/// Java: throw new Abort(cause);
#[track_caller]
pub fn abort_with_cause(cause: Box<dyn Any + Send>) -> ! {
    panic_any(Abort::with_cause(cause))
}
