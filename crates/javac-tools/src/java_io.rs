// PORTING ADAPTER (not a 1:1 port)
// Adapter for: java.io.Reader, java.io.Writer, java.io.StringReader.
// Why not 1:1: java.io is not part of the reference checkout and Rust
//              std::io is byte oriented only; a Java Reader/Writer transports
//              UTF-16 `char` values, so the minimal traits below represent the
//              character streams (encoding/decoding belongs to the ported
//              InputStreamReader/OutputStreamWriter in javac-core).
//! Minimal mapping of `java.io.Reader` / `java.io.Writer` (character
//! streams) for the `javax.tools` layer.
//!
//! PORTING_RULES: a Java Reader/Writer transports UTF-16 `char` values;
//! Rust has no std equivalent (std::io is byte oriented), so the traits below
//! are the minimal representation.  Encoding/decoding belongs to the ports of
//! `java.io.InputStreamReader` / `OutputStreamWriter` (done by the
//! `com.sun.tools.javac.file` / output layer port, docs/PORTING_RULES.md §7),
//! never to this adapter.
//!
//! `read(char[] cbuf, int off, int len)` / `write(char[] cbuf, int off, int len)`
//! are represented without `off`/`len`: the caller passes the exact slice it
//! wants to read/write (`off` is carried by the slice offset).  This is a
//! signature-level mapping only; the return value and the `-1` end-of-file
//! marker keep the Java semantics.
//!
//! A minimal `java.io.StringReader` is provided because the default
//! implementation of `SimpleJavaFileObject.openReader` constructs one from the
//! `getCharContent` result.

use crate::error::Error;
use javac_strings::JavaString;

/// Mapping of `java.io.Reader`: a source of UTF-16 `char` values.
pub trait Reader {
    /// Java: `read(char[] cbuf, int off, int len)`.
    ///
    /// Returns the number of characters read into `cbuf`, or `-1` at end of
    /// stream (Java returns `int`; the negative value is observable behavior).
    fn read(&mut self, cbuf: &mut [u16]) -> Result<i32, Error>;

    /// Java: `close()`.
    fn close(&mut self) -> Result<(), Error>;
}

/// Mapping of `java.io.Writer`: a sink of UTF-16 `char` values.
pub trait Writer {
    /// Java: `write(char[] cbuf, int off, int len)`.
    fn write(&mut self, cbuf: &[u16]) -> Result<(), Error>;

    /// Java: `flush()`.
    fn flush(&mut self) -> Result<(), Error>;

    /// Java: `close()`.
    fn close(&mut self) -> Result<(), Error>;
}

/// Minimal mapping of `java.io.StringReader`.
pub struct StringReader {
    content: Vec<u16>,
    next: usize,
    closed: bool,
}

impl StringReader {
    /// Java: `StringReader(String s)`.
    pub fn new(content: JavaString) -> StringReader {
        StringReader { content: content.as_units().to_vec(), next: 0, closed: false }
    }

    /// Constructs the reader from an already encoded UTF-16 sequence.
    pub fn from_units(content: Vec<u16>) -> StringReader {
        StringReader { content, next: 0, closed: false }
    }

    /// Java: `StringReader.ready()`.
    pub fn ready(&self) -> Result<bool, Error> {
        if self.closed {
            return Err(Error::io(JavaString::from("Stream closed")));
        }
        Ok(true)
    }
}

impl Reader for StringReader {
    // Java: java.io.StringReader.read(char[] cbuf, int off, int len)
    fn read(&mut self, cbuf: &mut [u16]) -> Result<i32, Error> {
        if self.closed {
            return Err(Error::io(JavaString::from("Stream closed")));
        }
        if cbuf.is_empty() {
            return Ok(0);
        }
        if self.next >= self.content.len() {
            return Ok(-1);
        }
        let n = std::cmp::min(self.content.len() - self.next, cbuf.len());
        cbuf[..n].copy_from_slice(&self.content[self.next..self.next + n]);
        self.next += n;
        Ok(n as i32)
    }

    // Java: java.io.StringReader.close()
    fn close(&mut self) -> Result<(), Error> {
        self.closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Reader, StringReader};
    use javac_strings::JavaString;

    fn read_all(reader: &mut StringReader) -> String {
        let mut units: Vec<u16> = Vec::new();
        let mut buf = [0u16; 4];
        loop {
            let n = reader.read(&mut buf).expect("read");
            if n < 0 {
                break;
            }
            units.extend_from_slice(&buf[..n as usize]);
        }
        String::from_utf16(&units).expect("valid utf-16")
    }

    #[test]
    fn reads_content_then_eof() {
        let mut r = StringReader::new(JavaString::from("a😀b"));
        // A surrogate pair occupies two chars, exactly as String.charAt does.
        assert_eq!(read_all(&mut r), "a😀b");
        assert_eq!(r.read(&mut [0u16; 4]).expect("eof"), -1);
    }

    #[test]
    fn close_makes_read_fail() {
        let mut r = StringReader::new(JavaString::from("x"));
        r.close().expect("close");
        assert!(r.read(&mut [0u16; 1]).is_err());
    }
}
