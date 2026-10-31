//! Errors produced while opening or reading a zip archive.

use std::fmt;
use std::io;

/// Errors produced while opening or reading a zip archive.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io(io::Error),
    /// The file is not a zip archive (no usable end of central directory record).
    NotAZip(String),
    /// The archive structure is damaged or truncated.
    Corrupted(String),
    /// An entry uses a compression method other than stored (0) or deflate (8).
    UnsupportedCompression(u16),
    /// The archive uses the zip64 extension (accepted by rawzip; retained for API
    /// compatibility).
    UnsupportedZip64(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::NotAZip(message) => write!(f, "not a zip archive: {}", message),
            Error::Corrupted(message) => write!(f, "corrupted zip archive: {}", message),
            Error::UnsupportedCompression(method) => {
                write!(f, "unsupported zip compression method: {}", method)
            }
            Error::UnsupportedZip64(message) => write!(f, "unsupported zip64 archive: {}", message),
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
