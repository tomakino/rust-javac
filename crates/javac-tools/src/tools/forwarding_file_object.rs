//! Forwards calls to a given file object.

use super::file_object::FileObject;
use crate::error::Error;
use crate::java_io::{Reader, Writer};
use crate::uri::URI;
use javac_strings::JavaString;
use std::io::{Read, Write};

/// Forwards calls to a given file object.  Subclasses of this class
/// might override some of these methods and might also provide
/// additional fields and methods.
///
/// @param <F> the kind of file object forwarded to by this object
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// — class mapping:
/// * Java class inheritance is not available in Rust.  This struct keeps the
///   single upstream field and forwards every method; extension is done by
///   composition (hold the forwarding object) instead of subclassing.
/// * the protected field fileObject -> pub field file_object (a public field
///   keeps the upstream "subclasses may access it" contract; there are no
///   protected members in Rust).
/// * the protected constructor -> pub constructor for the same reason.
pub struct ForwardingFileObject<F: FileObject> {
    /// The file object which all methods are delegated to.
    ///
    /// Java: protected final F fileObject;
    pub file_object: F,
}

impl<F: FileObject> ForwardingFileObject<F> {
    /// Creates a new instance of ForwardingFileObject.
    /// @param fileObject delegate to this file object
    ///
    /// Java: protected ForwardingFileObject(F fileObject) {
    ///           this.fileObject = Objects.requireNonNull(fileObject); }
    /// Rust has no null, so requireNonNull has no Rust form.
    pub fn new(file_object: F) -> ForwardingFileObject<F> {
        ForwardingFileObject { file_object }
    }
}

impl<F: FileObject> FileObject for ForwardingFileObject<F> {
    fn to_uri(&self) -> URI {
        self.file_object.to_uri()
    }

    fn get_name(&self) -> JavaString {
        self.file_object.get_name()
    }

    /// @throws IllegalStateException {@inheritDoc}
    /// @throws UnsupportedOperationException {@inheritDoc}
    /// @throws IOException {@inheritDoc}
    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
        self.file_object.open_input_stream()
    }

    /// @throws IllegalStateException {@inheritDoc}
    /// @throws UnsupportedOperationException {@inheritDoc}
    /// @throws IOException {@inheritDoc}
    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
        self.file_object.open_output_stream()
    }

    /// @throws IllegalStateException {@inheritDoc}
    /// @throws UnsupportedOperationException {@inheritDoc}
    /// @throws IOException {@inheritDoc}
    fn open_reader(&self, ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
        self.file_object.open_reader(ignore_encoding_errors)
    }

    /// @throws IllegalStateException {@inheritDoc}
    /// @throws UnsupportedOperationException {@inheritDoc}
    /// @throws IOException {@inheritDoc}
    fn get_char_content(&self, ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
        self.file_object.get_char_content(ignore_encoding_errors)
    }

    /// @throws IllegalStateException {@inheritDoc}
    /// @throws UnsupportedOperationException {@inheritDoc}
    /// @throws IOException {@inheritDoc}
    fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
        self.file_object.open_writer()
    }

    fn get_last_modified(&self) -> i64 {
        self.file_object.get_last_modified()
    }

    fn delete(&self) -> bool {
        self.file_object.delete()
    }
}

#[cfg(test)]
mod tests {
    use super::ForwardingFileObject;
    use crate::error::Error;
    use crate::java_io::{Reader, Writer};
    use crate::tools::file_object::FileObject;
    use crate::uri::URI;
    use javac_strings::JavaString;
    use std::io::{Read, Write};

    struct BaseFile {
        name: JavaString,
    }

    impl FileObject for BaseFile {
        fn to_uri(&self) -> URI {
            URI::create(&JavaString::from("mem:///base"))
        }
        fn get_name(&self) -> JavaString {
            self.name.clone()
        }
        fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
            Ok(Box::new(std::io::Cursor::new(b"base".to_vec())))
        }
        fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn open_reader(&self, _ignore: bool) -> Result<Box<dyn Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_char_content(&self, _ignore: bool) -> Result<Option<JavaString>, Error> {
            Ok(Some(self.name.clone()))
        }
        fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_last_modified(&self) -> i64 {
            42
        }
        fn delete(&self) -> bool {
            true
        }
    }

    #[test]
    fn forwards_every_method() {
        let forwarding = ForwardingFileObject::new(BaseFile {
            name: JavaString::from("Base.java"),
        });
        assert_eq!(forwarding.get_name(), JavaString::from("Base.java"));
        assert_eq!(
            forwarding.to_uri(),
            URI::create(&JavaString::from("mem:///base"))
        );
        assert_eq!(
            forwarding.get_char_content(false).unwrap(),
            Some(JavaString::from("Base.java"))
        );
        assert_eq!(forwarding.get_last_modified(), 42);
        assert!(forwarding.delete());
        assert_eq!(
            forwarding.open_output_stream().err(),
            Some(Error::UnsupportedOperationException)
        );
    }
}
