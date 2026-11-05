// PORTING ADAPTER (re-export): Java String/char semantics.
//!
//! The implementation lives in the bottom-most crate javac-strings so that both
//! javac-tools and javac-core can share the same Java String representation.
//! This file keeps the util/java_string.rs path stable for the port.

pub use javac_strings::JavaString;
