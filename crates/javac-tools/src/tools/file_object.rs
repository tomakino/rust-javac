//! File abstraction for tools: an abstraction of regular files and other
//! sources of data.

use crate::error::Error;
use crate::java_io::{Reader, Writer};
use crate::uri::URI;
use javac_strings::JavaString;
use std::io::{Read, Write};

/// File abstraction for tools.  In this context, <em>file</em> means
/// an abstraction of regular files and other sources of data.  For
/// example, a file object can be used to represent regular files,
/// memory cache, or data in databases.
///
/// <p>All methods in this interface might throw a SecurityException if
/// a security exception occurs.
///
/// <p>Unless explicitly allowed, all methods in this interface might
/// throw a NullPointerException if given a {@code null} argument.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
///
/// — signature mapping:
/// * `java.net.URI` -> `crate::uri::URI` (minimal mapping, see uri.rs).
/// * Java `String`/`CharSequence` -> `javac_strings::JavaString` (UTF-16
///   semantics, docs/porting adapter.md §7).  `getName` -> `JavaString`,
///   `getCharContent` -> `Option<JavaString>` (`null` means "not available").
/// * `InputStream`/`OutputStream` -> `Box<dyn std::io::Read>` /
///   `Box<dyn std::io::Write>`; `Reader`/`Writer` -> the minimal character
///   stream traits of `crate::java_io`.
/// * declared `IOException` -> `Result<T, Error>`.
/// * The returned streams own their state (`'static`): the ported file object
///   implementations (regular files, jar entries, in-memory class files) hold
///   the underlying handle, so no borrowed stream is needed.
pub trait FileObject {
    /// Returns a URI identifying this file object.
    /// @return a URI
    fn to_uri(&self) -> URI;

    /// Returns a user-friendly name for this file object.  The exact
    /// value returned is not specified but implementations should take
    /// care to preserve names as given by the user.  For example, if
    /// the user writes the filename {@code "BobsApp\Test.java"} on
    /// the command line, this method should return {@code
    /// "BobsApp\Test.java"} whereas the {@linkplain #toUri toUri}
    /// method might return {@code
    /// file:///C:/Documents%20and%20Settings/UncleBob/BobsApp/Test.java}.
    ///
    /// @return a user-friendly name
    fn get_name(&self) -> JavaString;

    /// Returns an InputStream for this file object.
    ///
    /// @return an InputStream
    /// @throws IllegalStateException if this file object was
    /// opened for writing and does not support reading
    /// @throws UnsupportedOperationException if this kind of file
    /// object does not support byte access
    /// @throws IOException if an I/O error occurred
    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error>;

    /// Returns an OutputStream for this file object.
    ///
    /// @return an OutputStream
    /// @throws IllegalStateException if this file object was
    /// opened for reading and does not support writing
    /// @throws UnsupportedOperationException if this kind of
    /// file object does not support byte access
    /// @throws IOException if an I/O error occurred
    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error>;

    /// Returns a reader for this object.  The returned reader will
    /// replace bytes that cannot be decoded with the default
    /// translation character.  In addition, the reader may report a
    /// diagnostic unless {@code ignoreEncodingErrors} is true.
    ///
    /// @param ignoreEncodingErrors ignore encoding errors if true
    /// @return a Reader
    /// @throws IllegalStateException if this file object was
    /// opened for writing and does not support reading
    /// @throws UnsupportedOperationException if this kind of
    /// file object does not support character access
    /// @throws IOException if an I/O error occurred
    fn open_reader(&self, ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error>;

    /// Returns the character content of this file object, if available.
    /// Any byte that cannot be decoded will be replaced by the default
    /// translation character.  In addition, a diagnostic may be
    /// reported unless {@code ignoreEncodingErrors} is true.
    ///
    /// @param ignoreEncodingErrors ignore encoding errors if true
    /// @return a CharSequence if available; {@code null} otherwise
    /// @throws IllegalStateException if this file object was
    /// opened for writing and does not support reading
    /// @throws UnsupportedOperationException if this kind of
    /// file object does not support character access
    /// @throws IOException if an I/O error occurred
    fn get_char_content(&self, ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error>;

    /// Returns a Writer for this file object.
    ///
    /// @return a Writer
    /// @throws IllegalStateException if this file object was
    /// opened for reading and does not support writing
    /// @throws UnsupportedOperationException if this kind of
    /// file object does not support character access
    /// @throws IOException if an I/O error occurred
    fn open_writer(&self) -> Result<Box<dyn Writer>, Error>;

    /// Returns the time this file object was last modified.  The time is
    /// measured in milliseconds since the epoch (00:00:00 GMT, January
    /// 1, 1970).
    ///
    /// @return the time this file object was last modified; or 0 if
    /// the file object does not exist, if an I/O error occurred, or if
    /// the operation is not supported
    fn get_last_modified(&self) -> i64;

    /// Deletes this file object.  In case of errors, returns false.
    /// @return true if and only if this file object is successfully
    /// deleted; false otherwise
    fn delete(&self) -> bool;

    /// Adapter: the upstream jrt FileSystem identity test
    /// (JRTIndex.isInJRT(FileObject)) is expressed as a trait method because
    /// Rust cannot downcast a JavaFileObject (registered adapter).
    fn is_in_jrt(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::FileObject;
    use crate::error::Error;
    use crate::java_io::{Reader, Writer};
    use crate::uri::URI;
    use javac_strings::JavaString;
    use std::io::{Read, Write};

    struct MemoryFile {
        uri: URI,
        bytes: Vec<u8>,
    }

    impl FileObject for MemoryFile {
        fn to_uri(&self) -> URI {
            self.uri.clone()
        }

        fn get_name(&self) -> JavaString {
            self.uri.get_path().unwrap_or_default()
        }

        fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
            Ok(Box::new(std::io::Cursor::new(self.bytes.clone())))
        }

        fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn open_reader(&self, _ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_char_content(&self, _ignore: bool) -> Result<Option<JavaString>, Error> {
            Ok(Some(JavaString::from_utf8_lossy(&self.bytes)))
        }

        fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_last_modified(&self) -> i64 {
            0
        }

        fn delete(&self) -> bool {
            false
        }
    }

    #[test]
    fn external_implementation_is_usable() {
        let file = MemoryFile {
            uri: URI::create(&JavaString::from("mem:///Foo.java")),
            bytes: b"class Foo {}".to_vec(),
        };
        assert_eq!(file.get_name(), JavaString::from("/Foo.java"));
        assert_eq!(
            file.get_char_content(false).unwrap(),
            Some(JavaString::from("class Foo {}"))
        );
        let mut stream = file.open_input_stream().unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"class Foo {}");
        assert!(!file.delete());
    }
}
