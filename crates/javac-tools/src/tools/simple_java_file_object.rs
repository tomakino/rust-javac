//! Provides simple implementations for most methods in JavaFileObject.

use super::file_object::FileObject;
use super::java_file_object::{JavaFileObject, Kind};
use crate::error::Error;
use crate::java_io::{Reader, StringReader, Writer};
use crate::lang::model::element::modifier::Modifier;
use crate::lang::model::element::nesting_kind::NestingKind;
use crate::uri::URI;
use javac_strings::JavaString;
use std::io::{Read, Write};

/// Provides simple implementations for most methods in JavaFileObject.
/// This class is designed to be subclassed and used as a basis for
/// JavaFileObject implementations.  Subclasses can override the
/// implementation and specification of any method of this class as
/// long as the general contract of JavaFileObject is obeyed.
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// PORTING_RULES — class mapping:
/// * Java "this class is designed to be subclassed" cannot be expressed in
///   Rust (no inheritance).  The struct keeps the upstream state and default
///   implementations; a Rust implementation that wants different behavior
///   re-implements the trait (or wraps this struct and re-implements the few
///   methods that call other virtual methods), which is the equivalent of
///   overriding.  Because to_uri() can therefore not be overridden, the path
///   invariant established by the constructor holds for every method.
/// * protected final URI uri / Kind kind -> pub fields uri / kind (public
///   because the upstream fields are protected for subclasses and Rust has no
///   protected visibility).
pub struct SimpleJavaFileObject {
    /// A URI for this file object.
    ///
    /// Java: protected final URI uri;
    pub uri: URI,

    /// The kind of this file object.
    ///
    /// Java: protected final Kind kind;
    pub kind: Kind,
}

impl SimpleJavaFileObject {
    /// Construct a SimpleJavaFileObject of the given kind and with the
    /// given URI.
    ///
    /// @param uri  the URI for this file object
    /// @param kind the kind of this file object
    ///
    /// PORTING_RULES: the upstream constructor throws
    /// IllegalArgumentException("URI must have a path: " + uri) for a URI
    /// without a path; that is API misuse and maps to panic! (crate::error).
    /// The Objects.requireNonNull checks have no Rust form.
    pub fn new(uri: URI, kind: Kind) -> SimpleJavaFileObject {
        if uri.get_path().is_none() {
            panic!("URI must have a path: {uri}");
        }
        SimpleJavaFileObject { uri, kind }
    }
}

impl FileObject for SimpleJavaFileObject {
    fn to_uri(&self) -> URI {
        self.uri.clone()
    }

    fn get_name(&self) -> JavaString {
        // Java: return toUri().getPath();
        // PORTING_RULES: toUri() cannot be overridden in Rust and the
        // constructor guarantees a path, so the Java null case is unreachable.
        self.to_uri().get_path().expect("SimpleJavaFileObject URI has a path")
    }

    /// This implementation always throws {@linkplain
    /// UnsupportedOperationException}.  Subclasses can change this
    /// behavior as long as the contract of {@link FileObject} is
    /// obeyed.
    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
        Err(Error::UnsupportedOperationException)
    }

    /// This implementation always throws {@linkplain
    /// UnsupportedOperationException}.  Subclasses can change this
    /// behavior as long as the contract of {@link FileObject} is
    /// obeyed.
    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
        Err(Error::UnsupportedOperationException)
    }

    /// Wraps the result of {@linkplain #getCharContent} in a Reader.
    /// Subclasses can change this behavior as long as the contract of
    /// {@link FileObject} is obeyed.
    ///
    /// @param  ignoreEncodingErrors {@inheritDoc}
    /// @return a Reader wrapping the result of getCharContent
    /// @throws IllegalStateException {@inheritDoc}
    /// @throws UnsupportedOperationException {@inheritDoc}
    /// @throws IOException {@inheritDoc}
    ///
    /// PORTING_RULES: the upstream CharBuffer special case (CharArrayReader
    /// over the backing array) is representation-only; the interface returns
    /// the character content of getCharContent and the same characters are
    /// read back through StringReader.  The Java null result maps to
    /// UnsupportedOperationException exactly as upstream.
    fn open_reader(&self, ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
        let char_content = self.get_char_content(ignore_encoding_errors)?;
        match char_content {
            // Java: if (charContent == null) throw new UnsupportedOperationException();
            None => Err(Error::UnsupportedOperationException),
            // Java: return new StringReader(charContent.toString());
            Some(content) => Ok(Box::new(StringReader::new(content))),
        }
    }

    /// This implementation always throws {@linkplain
    /// UnsupportedOperationException}.  Subclasses can change this
    /// behavior as long as the contract of {@link FileObject} is
    /// obeyed.
    fn get_char_content(&self, _ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
        Err(Error::UnsupportedOperationException)
    }

    /// Wraps the result of openOutputStream in a Writer.  Subclasses
    /// can change this behavior as long as the contract of {@link
    /// FileObject} is obeyed.
    ///
    /// @return a Writer wrapping the result of openOutputStream
    /// @throws IllegalStateException {@inheritDoc}
    /// @throws UnsupportedOperationException {@inheritDoc}
    /// @throws IOException {@inheritDoc}
    fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
        // Java: return new OutputStreamWriter(openOutputStream());
        // The java.io.OutputStreamWriter default (StreamEncoder) uses the
        // platform default charset and CodingErrorAction.REPLACE (oracle: a
        // US-ASCII writer emits 3F for U+00E9 instead of throwing).
        let charset = javac_charset::Charset::default_charset();
        let mut encoder = charset.new_encoder();
        encoder
            .on_malformed_input(javac_charset::CodingErrorAction::Replace)
            .on_unmappable_character(javac_charset::CodingErrorAction::Replace);
        let output = self.open_output_stream()?;
        Ok(Box::new(crate::java_io::OutputStreamWriter::new(
            output, encoder,
        )))
    }

    /// This implementation returns 0L.  Subclasses can change this
    /// behavior as long as the contract of {@link FileObject} is
    /// obeyed.
    ///
    /// @return 0L
    fn get_last_modified(&self) -> i64 {
        0
    }

    /// This implementation does nothing.  Subclasses can change this
    /// behavior as long as the contract of {@link FileObject} is
    /// obeyed.
    ///
    /// @return false
    fn delete(&self) -> bool {
        false
    }
}

impl JavaFileObject for SimpleJavaFileObject {
    /// @return this.kind
    fn get_kind(&self) -> Kind {
        self.kind
    }

    /// This implementation compares the path of its URI to the given
    /// simple name.  This method returns true if the given kind is
    /// equal to the kind of this object, and if the path is equal to
    /// simpleName + kind.extension or if it ends with
    /// "/" + simpleName + kind.extension.
    ///
    /// <p>This method calls {@link #getKind} and {@link #toUri} and
    /// does not access the fields {@link #uri} and {@link #kind}
    /// directly.
    ///
    /// <p>Subclasses can change this behavior as long as the contract
    /// of {@link JavaFileObject} is obeyed.
    fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
        // Java: String baseName = simpleName + kind.extension;
        let base_name = simple_name.concat(&kind.extension());
        // Java: return kind.equals(getKind())
        //           && (baseName.equals(toUri().getPath())
        //               || toUri().getPath().endsWith("/" + baseName));
        let path = self.to_uri().get_path();
        match path {
            Some(path) => {
                kind == self.get_kind()
                    && (path == base_name || path.ends_with(&JavaString::from("/").concat(&base_name)))
            }
            // Java would throw NullPointerException here; the constructor
            // guarantees a path, so this branch is unreachable for this type.
            None => false,
        }
    }

    /// This implementation returns null.  Subclasses can
    /// change this behavior as long as the contract of
    /// {@link JavaFileObject} is obeyed.
    fn get_nesting_kind(&self) -> Option<NestingKind> {
        None
    }

    /// This implementation returns null.  Subclasses can
    /// change this behavior as long as the contract of
    /// {@link JavaFileObject} is obeyed.
    fn get_access_level(&self) -> Option<Modifier> {
        None
    }
}

impl std::fmt::Display for SimpleJavaFileObject {
    /// Java: public String toString() {
    ///           return getClass().getName() + "[" + toUri() + "]"; }
    ///
    /// PORTING_RULES: Rust has no getClass(); the upstream binary class name
    /// of this class is used.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "javax.tools.SimpleJavaFileObject[{}]", self.to_uri())
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleJavaFileObject;
    use crate::error::Error;
    use crate::java_io::Reader;
    use crate::tools::file_object::FileObject;
    use crate::tools::java_file_object::{JavaFileObject, Kind};
    use crate::uri::URI;
    use javac_strings::JavaString;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    fn read_all(reader: &mut Box<dyn Reader>) -> String {
        let mut units: Vec<u16> = Vec::new();
        let mut buf = [0u16; 8];
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
    fn uri_and_name() {
        let file = SimpleJavaFileObject::new(URI::create(&js("string:///a/b/Foo.java")), Kind::SOURCE);
        assert_eq!(file.to_uri().to_uri_string(), js("string:///a/b/Foo.java"));
        assert_eq!(file.get_name(), js("/a/b/Foo.java"));
        assert_eq!(file.get_kind(), Kind::SOURCE);
        assert!(file.get_nesting_kind().is_none());
        assert!(file.get_access_level().is_none());
        assert_eq!(file.get_last_modified(), 0);
        assert!(!file.delete());
    }

    #[test]
    fn name_compatibility() {
        let file = SimpleJavaFileObject::new(URI::create(&js("string:///a/b/Foo.java")), Kind::SOURCE);
        // path equals simpleName + extension
        assert!(file.is_name_compatible(&js("Foo"), Kind::SOURCE));
        // path ends with "/" + simpleName + extension
        let file2 = SimpleJavaFileObject::new(URI::create(&js("string:///x/Foo.java")), Kind::SOURCE);
        assert!(file2.is_name_compatible(&js("Foo"), Kind::SOURCE));
        // different kind
        assert!(!file.is_name_compatible(&js("Foo"), Kind::CLASS));
        // different name
        assert!(!file.is_name_compatible(&js("Bar"), Kind::SOURCE));
        // a Java String with an unpaired surrogate is not a class name
        assert!(!file.is_name_compatible(&JavaString::from_units(vec![0xD800]), Kind::SOURCE));
    }

    #[test]
    fn default_byte_access_is_unsupported() {
        let file = SimpleJavaFileObject::new(URI::create(&js("string:///Foo.java")), Kind::SOURCE);
        assert_eq!(file.open_input_stream().err(), Some(Error::UnsupportedOperationException));
        assert_eq!(file.open_output_stream().err(), Some(Error::UnsupportedOperationException));
        assert_eq!(
            file.get_char_content(false).err(),
            Some(Error::UnsupportedOperationException)
        );
    }

    #[test]
    fn open_reader_wraps_char_content() {
        // Emulates the upstream "JavaSourceFromString" pattern: a subtype that
        // overrides getCharContent.  In Rust the equivalent is a wrapper type
        // that re-implements openReader (see the class PORTING_RULES note).
        struct SourceFromString(SimpleJavaFileObject);

        impl FileObject for SourceFromString {
            fn to_uri(&self) -> URI {
                self.0.to_uri()
            }
            fn get_name(&self) -> JavaString {
                self.0.get_name()
            }
            fn open_input_stream(&self) -> Result<Box<dyn std::io::Read>, Error> {
                self.0.open_input_stream()
            }
            fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, Error> {
                self.0.open_output_stream()
            }
            // Java: SimpleJavaFileObject.openReader calls getCharContent through
            // virtual dispatch, so a Java subclass only overrides getCharContent.
            // Rust composition cannot dispatch back to the wrapper, so the Rust
            // wrapper re-implements the four-line base implementation.
            fn open_reader(&self, ignore: bool) -> Result<Box<dyn Reader>, Error> {
                let char_content = self.get_char_content(ignore)?;
                match char_content {
                    None => Err(Error::UnsupportedOperationException),
                    Some(content) => {
                        Ok(Box::new(crate::java_io::StringReader::new(content)))
                    }
                }
            }
            fn get_char_content(&self, _ignore: bool) -> Result<Option<JavaString>, Error> {
                Ok(Some(JavaString::from("class Foo {}")))
            }
            fn open_writer(&self) -> Result<Box<dyn crate::java_io::Writer>, Error> {
                self.0.open_writer()
            }
            fn get_last_modified(&self) -> i64 {
                self.0.get_last_modified()
            }
            fn delete(&self) -> bool {
                self.0.delete()
            }
        }

        impl JavaFileObject for SourceFromString {
            fn get_kind(&self) -> Kind {
                self.0.get_kind()
            }
            fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
                self.0.is_name_compatible(simple_name, kind)
            }
            fn get_nesting_kind(
                &self,
            ) -> Option<crate::lang::model::element::nesting_kind::NestingKind> {
                self.0.get_nesting_kind()
            }
            fn get_access_level(&self) -> Option<crate::lang::model::element::modifier::Modifier> {
                self.0.get_access_level()
            }
        }

        let file = SourceFromString(SimpleJavaFileObject::new(
            URI::create(&js("string:///Foo.java")),
            Kind::SOURCE,
        ));
        let mut reader = file.open_reader(false).expect("reader");
        assert_eq!(read_all(&mut reader), "class Foo {}");
        assert_eq!(
            file.0.to_string(),
            "javax.tools.SimpleJavaFileObject[string:///Foo.java]"
        );
    }

    #[test]
    fn open_reader_on_null_char_content_is_unsupported() {
        let file = SimpleJavaFileObject::new(URI::create(&js("string:///Foo.java")), Kind::SOURCE);
        // getCharContent default throws UnsupportedOperationException upstream,
        // so openReader propagates it.
        assert_eq!(
            file.open_reader(false).err(),
            Some(Error::UnsupportedOperationException)
        );
    }

    #[test]
    #[should_panic(expected = "URI must have a path")]
    fn uri_without_path_panics() {
        // java.net.URI.create("mailto:a@b.c") has no path.
        SimpleJavaFileObject::new(URI::create(&js("mailto:a@b.c")), Kind::SOURCE);
    }
}
