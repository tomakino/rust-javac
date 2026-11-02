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
//! a Java Reader/Writer transports UTF-16 `char` values;
//! Rust has no std equivalent (std::io is byte oriented), so the traits below
//! are the minimal representation.  Encoding/decoding belongs to the ports of
//! `java.io.InputStreamReader` / `OutputStreamWriter` (done by the
//! `com.sun.tools.javac.file` / output layer port, docs/porting adapter.md §7),
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
        StringReader {
            content: content.as_units().to_vec(),
            next: 0,
            closed: false,
        }
    }

    /// Constructs the reader from an already encoded UTF-16 sequence.
    pub fn from_units(content: Vec<u16>) -> StringReader {
        StringReader {
            content,
            next: 0,
            closed: false,
        }
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

/// Java: java.io.InputStreamReader (a byte stream decoded by a
/// java.nio.charset.CharsetDecoder).
///
/// charset decoding is the javac-charset CharsetDecoder (the
/// java.nio.charset state machine); the java.io character-stream semantics are
/// represented by the Reader trait.
pub struct InputStreamReader {
    input: Box<dyn std::io::Read>,
    decoder: javac_charset::CharsetDecoder,
    pending: Vec<u16>,
    next: usize,
    eof: bool,
    closed: bool,
}

impl InputStreamReader {
    /// Java: InputStreamReader(InputStream in, CharsetDecoder dec).
    ///
    /// # Arguments
    ///   - input: the byte stream;
    ///   - decoder: the configured decoder.
    ///
    /// # Returns
    ///   - the character stream.
    pub fn new(
        input: Box<dyn std::io::Read>,
        decoder: javac_charset::CharsetDecoder,
    ) -> InputStreamReader {
        InputStreamReader {
            input,
            decoder,
            pending: Vec::new(),
            next: 0,
            eof: false,
            closed: false,
        }
    }

    fn io_error(error: std::io::Error) -> Error {
        Error::io(JavaString::from(error.to_string().as_str()))
    }

    fn decode_more(&mut self) -> Result<(), Error> {
        use javac_charset::{ByteBuffer, CharBuffer};
        let mut bytes = [0u8; 8192];
        let count = self
            .input
            .read(&mut bytes)
            .map_err(InputStreamReader::io_error)?;
        let end_of_input = count == 0;
        if end_of_input {
            self.eof = true;
        }
        let mut input = ByteBuffer::wrap(&bytes[..count]);
        let mut output = CharBuffer::allocate(count + 16);
        loop {
            let result = self.decoder.decode(&mut input, &mut output, end_of_input);
            if result.is_underflow() {
                break;
            } else if result.is_overflow() {
                let mut bigger = CharBuffer::allocate(output.capacity() * 2 + 16);
                bigger.put_units(output.written_units());
                output = bigger;
            } else if result.is_malformed() {
                return Err(Error::io(JavaString::from("MalformedInputException")));
            } else if result.is_unmappable() {
                return Err(Error::io(JavaString::from("UnmappableCharacterException")));
            } else {
                panic!("AssertionError: unexpected CoderResult");
            }
        }
        self.pending.extend_from_slice(output.written_units());
        Ok(())
    }
}

impl Reader for InputStreamReader {
    // Java: InputStreamReader.read(char[] cbuf, int off, int len)
    fn read(&mut self, cbuf: &mut [u16]) -> Result<i32, Error> {
        if self.closed {
            return Err(Error::io(JavaString::from("Stream closed")));
        }
        if cbuf.is_empty() {
            return Ok(0);
        }
        let mut written = 0usize;
        loop {
            while self.next < self.pending.len() && written < cbuf.len() {
                cbuf[written] = self.pending[self.next];
                written += 1;
                self.next += 1;
            }
            if written == cbuf.len() {
                return Ok(written as i32);
            }
            if self.next >= self.pending.len() {
                self.pending.clear();
                self.next = 0;
            }
            if self.eof {
                if written == 0 {
                    return Ok(-1);
                }
                return Ok(written as i32);
            }
            self.decode_more()?;
        }
    }

    // Java: InputStreamReader.close()
    fn close(&mut self) -> Result<(), Error> {
        self.closed = true;
        Ok(())
    }
}

/// Java: java.io.OutputStreamWriter (a byte stream encoded by a
/// java.nio.charset.CharsetEncoder).
///
/// charset encoding is the javac-charset CharsetEncoder (the
/// java.nio.charset state machine); the java.io character-stream semantics are
/// represented by the Writer trait.
pub struct OutputStreamWriter {
    output: Box<dyn std::io::Write>,
    encoder: javac_charset::CharsetEncoder,
    pending: Vec<u16>,
    closed: bool,
}

impl OutputStreamWriter {
    /// Java: OutputStreamWriter(OutputStream out, CharsetEncoder enc).
    ///
    /// # Arguments
    ///   - output: the byte stream;
    ///   - encoder: the configured encoder.
    ///
    /// # Returns
    ///   - the character stream.
    pub fn new(
        output: Box<dyn std::io::Write>,
        encoder: javac_charset::CharsetEncoder,
    ) -> OutputStreamWriter {
        OutputStreamWriter {
            output,
            encoder,
            pending: Vec::new(),
            closed: false,
        }
    }

    fn io_error(error: std::io::Error) -> Error {
        Error::io(JavaString::from(error.to_string().as_str()))
    }

    fn drain(&mut self, end_of_input: bool) -> Result<(), Error> {
        use javac_charset::ByteBuffer;
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut input = javac_charset::CharBuffer::wrap_units(&self.pending);
        let mut buffer = ByteBuffer::allocate(self.pending.len() * 4 + 16);
        loop {
            let result = self.encoder.encode(&mut input, &mut buffer, end_of_input);
            if result.is_underflow() {
                break;
            } else if result.is_overflow() {
                let mut bigger = ByteBuffer::allocate(buffer.capacity() * 2 + 16);
                bigger.put_flipped(&buffer);
                buffer = bigger;
            } else if result.is_malformed() {
                return Err(Error::io(JavaString::from("MalformedInputException")));
            } else if result.is_unmappable() {
                return Err(Error::io(JavaString::from("UnmappableCharacterException")));
            } else {
                panic!("AssertionError: unexpected CoderResult");
            }
        }
        buffer.flip();
        self.output
            .write_all(buffer.slice_upto_limit())
            .map_err(OutputStreamWriter::io_error)?;
        let consumed = input.position();
        self.pending.drain(..consumed);
        Ok(())
    }
}

impl Writer for OutputStreamWriter {
    // Java: OutputStreamWriter.write(char[] cbuf, int off, int len)
    fn write(&mut self, cbuf: &[u16]) -> Result<(), Error> {
        if self.closed {
            return Err(Error::io(JavaString::from("Stream closed")));
        }
        self.pending.extend_from_slice(cbuf);
        self.drain(false)
    }

    // Java: OutputStreamWriter.flush()
    fn flush(&mut self) -> Result<(), Error> {
        self.drain(false)?;
        self.output.flush().map_err(OutputStreamWriter::io_error)
    }

    // Java: OutputStreamWriter.close()
    fn close(&mut self) -> Result<(), Error> {
        self.drain(true)?;
        self.output.flush().map_err(OutputStreamWriter::io_error)?;
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
