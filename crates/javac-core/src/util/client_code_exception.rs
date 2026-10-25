// Origin: com/sun/tools/javac/util/ClientCodeException.java @ 982d53c (jdk-17.0.19-ga)

//! An exception used for propagating exceptions found in client code invoked
//! from javac.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::any::Any;
use std::panic::panic_any;

/// Payload of the Java com.sun.tools.javac.util.ClientCodeException runtime exception.
#[derive(Debug)]
pub struct ClientCodeException {
    pub cause: Box<dyn Any + Send>,
}

impl ClientCodeException {
    /// Java: public ClientCodeException(Throwable cause) { super(cause); }
    pub fn new(cause: Box<dyn Any + Send>) -> Self {
        ClientCodeException { cause }
    }
}

/// Java: throw new ClientCodeException(cause);
#[track_caller]
pub fn throw(cause: Box<dyn Any + Send>) -> ! {
    panic_any(ClientCodeException::new(cause))
}
