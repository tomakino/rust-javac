//! Forwards calls to a given Java file object.

use super::file_object::FileObject;
use super::forwarding_file_object::ForwardingFileObject;
use super::java_file_object::{JavaFileObject, Kind};
use crate::error::Error;
use crate::java_io::{Reader, Writer};
use crate::lang::model::element::modifier::Modifier;
use crate::lang::model::element::nesting_kind::NestingKind;
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
/// * Java: "class ForwardingJavaFileObject<F extends JavaFileObject>
///   extends ForwardingFileObject<F> implements JavaFileObject".
///   Rust has no inheritance, so the superclass instance is embedded in the
///   super_ field (the only field of this struct, like upstream has no own
///   fields) and every inherited method is forwarded explicitly.
pub struct ForwardingJavaFileObject<F: JavaFileObject> {
    /// Java: the inherited ForwardingFileObject<F> base instance.
    super_: ForwardingFileObject<F>,
}

impl<F: JavaFileObject> ForwardingJavaFileObject<F> {
    /// Creates a new instance of ForwardingJavaFileObject.
    /// @param fileObject delegate to this file object
    ///
    /// Java: protected ForwardingJavaFileObject(F fileObject) {
    ///           super(fileObject); }
    pub fn new(file_object: F) -> ForwardingJavaFileObject<F> {
        ForwardingJavaFileObject { super_: ForwardingFileObject::new(file_object) }
    }

    /// The file object which all methods are delegated to (the inherited
    /// protected field fileObject).
    pub fn file_object(&self) -> &F {
        &self.super_.file_object
    }
}

impl<F: JavaFileObject> FileObject for ForwardingJavaFileObject<F> {
    fn to_uri(&self) -> URI {
        self.super_.file_object.to_uri()
    }

    fn get_name(&self) -> JavaString {
        self.super_.file_object.get_name()
    }

    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
        self.super_.file_object.open_input_stream()
    }

    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
        self.super_.file_object.open_output_stream()
    }

    fn open_reader(&self, ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
        self.super_.file_object.open_reader(ignore_encoding_errors)
    }

    fn get_char_content(&self, ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
        self.super_.file_object.get_char_content(ignore_encoding_errors)
    }

    fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
        self.super_.file_object.open_writer()
    }

    fn get_last_modified(&self) -> i64 {
        self.super_.file_object.get_last_modified()
    }

    fn delete(&self) -> bool {
        self.super_.file_object.delete()
    }
}

impl<F: JavaFileObject> JavaFileObject for ForwardingJavaFileObject<F> {
    fn get_kind(&self) -> Kind {
        self.super_.file_object.get_kind()
    }

    fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
        self.super_.file_object.is_name_compatible(simple_name, kind)
    }

    fn get_nesting_kind(&self) -> Option<NestingKind> {
        self.super_.file_object.get_nesting_kind()
    }

    fn get_access_level(&self) -> Option<Modifier> {
        self.super_.file_object.get_access_level()
    }
}

#[cfg(test)]
mod tests {
    use super::ForwardingJavaFileObject;
    use crate::error::Error;
    use crate::java_io::{Reader, Writer};
    use crate::lang::model::element::modifier::Modifier;
    use crate::lang::model::element::nesting_kind::NestingKind;
    use crate::tools::file_object::FileObject;
    use crate::tools::java_file_object::{JavaFileObject, Kind};
    use crate::uri::URI;
    use javac_strings::JavaString;
    use std::io::{Read, Write};

    struct BaseJavaFile;

    impl FileObject for BaseJavaFile {
        fn to_uri(&self) -> URI {
            URI::create(&JavaString::from("string:///Base.java"))
        }
        fn get_name(&self) -> JavaString {
            JavaString::from("/Base.java")
        }
        fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn open_reader(&self, _ignore: bool) -> Result<Box<dyn Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_char_content(&self, _ignore: bool) -> Result<Option<JavaString>, Error> {
            Ok(Some(JavaString::from("class Base {}")))
        }
        fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_last_modified(&self) -> i64 {
            7
        }
        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for BaseJavaFile {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }
        fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
            kind == Kind::SOURCE && *simple_name == JavaString::from("Base")
        }
        fn get_nesting_kind(&self) -> Option<NestingKind> {
            Some(NestingKind::TOP_LEVEL)
        }
        fn get_access_level(&self) -> Option<Modifier> {
            Some(Modifier::PUBLIC)
        }
    }

    #[test]
    fn forwards_java_file_object_methods() {
        let forwarding = ForwardingJavaFileObject::new(BaseJavaFile);
        assert_eq!(forwarding.get_kind(), Kind::SOURCE);
        assert!(forwarding.is_name_compatible(&JavaString::from("Base"), Kind::SOURCE));
        assert!(!forwarding.is_name_compatible(&JavaString::from("Other"), Kind::SOURCE));
        assert_eq!(forwarding.get_nesting_kind(), Some(NestingKind::TOP_LEVEL));
        assert_eq!(forwarding.get_access_level(), Some(Modifier::PUBLIC));
        // inherited ForwardingFileObject behavior
        assert_eq!(forwarding.get_name(), JavaString::from("/Base.java"));
        assert_eq!(forwarding.get_last_modified(), 7);
        assert_eq!(forwarding.file_object().get_kind(), Kind::SOURCE);
    }
}
