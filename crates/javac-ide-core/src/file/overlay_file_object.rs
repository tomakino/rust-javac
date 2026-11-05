
//! In-memory source file object used for editor overlays.

use std::io::{Cursor, Read, Write};

use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::java_io::{Reader, Writer};
use javac_tools::tools::file_object::FileObject;
use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
use javac_tools::uri::URI;

/// A source file whose content comes from memory (unsaved editor buffer).
pub struct OverlayFileObject {
    name: JavaString,
    content: JavaString,
}

impl OverlayFileObject {
    pub fn new(name: JavaString, content: JavaString) -> OverlayFileObject {
        OverlayFileObject { name, content }
    }
}

impl FileObject for OverlayFileObject {
    fn to_uri(&self) -> URI {
        let path = self.name.to_string_lossy().replace('\\', "/");
        let uri = format!("file:///{path}");
        URI::create(&JavaString::from(uri.as_str()))
    }

    fn get_name(&self) -> JavaString {
        self.name.clone()
    }

    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
        Ok(Box::new(Cursor::new(
            self.content.to_string_lossy().into_bytes(),
        )))
    }

    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
        Err(Error::io(JavaString::from(
            "UnsupportedOperationException: overlay files are read-only",
        )))
    }

    fn get_char_content(&self, _ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
        Ok(Some(self.content.clone()))
    }

    fn get_last_modified(&self) -> i64 {
        0
    }

    fn open_reader(&self, _ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
        Err(Error::io(JavaString::from(
            "UnsupportedOperationException: overlay files have no character stream",
        )))
    }

    fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
        Err(Error::io(JavaString::from(
            "UnsupportedOperationException: overlay files are read-only",
        )))
    }

    fn delete(&self) -> bool {
        false
    }
}

impl JavaFileObject for OverlayFileObject {
    fn get_kind(&self) -> Kind {
        Kind::SOURCE
    }

    fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
        kind == Kind::SOURCE
            && self
                .name
                .to_string_lossy()
                .ends_with(&format!("{}.java", simple_name.to_string_lossy()))
    }

    fn get_nesting_kind(&self) -> Option<javac_tools::lang::model::element::nesting_kind::NestingKind> {
        None
    }

    fn get_access_level(&self) -> Option<javac_tools::lang::model::element::modifier::Modifier> {
        None
    }
}
