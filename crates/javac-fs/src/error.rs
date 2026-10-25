// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.file (the subset used by com.sun.tools.javac.file).
// Why not 1:1: java.nio.file is a FileSystem virtual-machine API; javac only
// needs path syntax plus a handful of file queries, so this crate provides that
// minimal layer over std::path/std::fs.  The behavior oracle is the default
// java.nio.file FileSystem (WindowsPath) of the baseline JDK 17.0.19, recorded
// from a development-time Java program and embedded in the tests; the crate is
// platform-aware (see the Linux notes in lib.rs).

//! Errors of the path/file layer.
//!
//! PORTING_RULES: the declared java.io.IOException is mapped to [std::io::Error];
//! java.nio.file.InvalidPathException and IllegalArgumentException (thrown by
//! Path.relativize for mismatched roots) keep their Java roles/classes here so
//! the P2 file layer can map them onto javac diagnostics.

use std::fmt;
use std::io;

/// Errors of the path/file layer.
#[derive(Debug)]
pub enum Error {
    /// Java: IOException
    Io(io::Error),
    /// Java: java.nio.file.InvalidPathException
    InvalidPath { path: String },
    /// Java: IllegalArgumentException (e.g. Path.relativize with different roots)
    IllegalArgument { message: String },
    /// A URI is not a file URI (java.nio.file.Path.of(URI) with another scheme).
    UnsupportedUri { uri: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::InvalidPath { path } => write!(f, "InvalidPathException: {}", path),
            Error::IllegalArgument { message } => write!(f, "IllegalArgumentException: {}", message),
            Error::UnsupportedUri { uri } => write!(f, "unsupported URI: {}", uri),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}
