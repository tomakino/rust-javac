// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.charset (the subset javac uses through
// com.sun.tools.javac.file.BaseFileManager / PathFileObject: Charset.forName,
// Charset.defaultCharset, CharsetDecoder with REPORT/REPLACE, CharsetEncoder).
// Why not 1:1: java.nio.charset is a service-provider charset framework with
// decoders/encoders, ByteBuffer/CharBuffer and CoderResult state machines; the
// port needs the observable conversions and the error semantics, so this crate
// exposes a small Charset value plus whole-input decode/encode operations.
// The behavior oracle is the baseline JDK 17.0.19.10 java.nio.charset
// implementation, recorded through a development-time Java program (the
// recorded expectations are embedded in the tests; no JVM at test time).
//
//! java.nio.charset platform layer for the P2 file layer.
//!
//! PORTING_RULES:
//! * supported charsets: UTF-8, US-ASCII, ISO-8859-1, UTF-16, UTF-16BE,
//!   UTF-16LE (with the Java BOM behavior);
//! * names: canonical names plus the aliases recorded from the JDK, matched
//!   case-insensitively after the Java charset-name validity check
//!   (IllegalCharsetNameException / UnsupportedCharsetException roles kept);
//!   GBK, GB18030, Big5, Shift_JIS and the other JDK charsets are NOT
//!   implemented: they are reported as UnsupportedCharset with the error
//!   carrying the requested name (no silent substitution);
//! * decode/encode keep the Java REPORT/REPLACE behavior for malformed input
//!   and unmappable characters, including the Java replacement characters
//!   (U+FFFD for decoding; '?' for UTF-8/US-ASCII/ISO-8859-1 encoding and
//!   U+FFFD for the UTF-16 family) and the Java input-length values;
//! * GBK and other legacy encodings need a table provider; the proposed
//!   dependency is encoding_rs (to be approved by the Lead) - until then an
//!   explicit UnsupportedCharset error is returned.

pub mod charset;
pub mod coder;
pub mod error;
pub mod nio;

pub use charset::{available_charsets, Charset, MalformedAction};
pub use coder::{CharsetDecoder, CharsetEncoder};
pub use error::{CharacterCodingError, CharsetError};
pub use nio::{ByteBuffer, CharBuffer, CoderResult, CodingErrorAction};

