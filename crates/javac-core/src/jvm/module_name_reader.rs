// Origin: com/sun/tools/javac/jvm/ModuleNameReader.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: dependent service (see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: utf8Mapper (ModuleNameReader.java:149)
// End of Gap registration.

//! Stripped down ClassReader, just sufficient to read module names from
//! module-info.class files while analyzing the module path.
//!
//! <p>
//! <b>This is NOT part of any supported API. If you write code that depends on this, you do so at
//! your own risk. This code and its internal interfaces are subject to change or deletion without
//! notice.</b>
//!
//! PORTING_RULES (1:1 where the dependency is ported; the rest is reported):
//! * ported dependencies: ClassFile.JAVA_MAGIC (crate::jvm::class_file),
//!   ByteBuffer (crate::util::byte_buffer), Convert.utf2string
//!   (crate::util::convert), Names (crate::util::names), PoolReader
//!   (crate::jvm::pool_reader).
//! * NOT ported yet (TODO, no substitute semantics):
//!   - java.io.InputStream / Files.newInputStream: the Rust form takes
//!     &mut R: Read (the JavaFileObject entry point still uses the JavaFileObject
//!     abstraction of javac-tools); declared IOException -> io::Error;
//!   - NameMapper<Z>: the two mappers are represented by the internalize flag
//!     of PoolReader::peek_name (same two conversions);
//!   - Convert / ClassFile.internalize are the ported helpers.
//! * Java overloads of readModuleName(Path/JavaFileObject/InputStream) become
//!   distinct Rust method names (read_module_name /
//!   read_module_name_from_java_file_object / read_module_name_from_stream).

use crate::jvm::class_file::JAVA_MAGIC;
use crate::jvm::pool_reader::PoolReader;
use crate::util::byte_buffer::ByteBuffer;
use crate::util::names::Names;
use javac_strings::JavaString;
use javac_tools::tools::java_file_object::JavaFileObject;
use std::fmt;
use std::io::{self, Read};
use std::path::Path;

/// Java: public static class ModuleNameReader.BadClassFile extends Exception.
pub struct BadClassFile {
    message: JavaString,
}

impl BadClassFile {
    fn new(message: JavaString) -> BadClassFile {
        BadClassFile { message }
    }
}

impl fmt::Debug for BadClassFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BadClassFile: {}", self.message)
    }
}

impl fmt::Display for BadClassFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message.to_string_lossy())
    }
}

impl std::error::Error for BadClassFile {}

/// The errors of the module name reader.
///
/// PORTING_RULES: the declared IOException -> io::Error and BadClassFile keeps
/// its Java role; the JavaFileObject open error is kept as the javac-tools
/// error value.
pub enum Error {
    /// Java: throws IOException
    Io(io::Error),
    /// Java: JavaFileObject.openInputStream() failure
    FileObject(javac_tools::error::Error),
    /// Java: throws BadClassFile
    BadClassFile(BadClassFile),
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::FileObject(err) => write!(f, "file object error: {:?}", err),
            Error::BadClassFile(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::BadClassFile(err) => Some(err),
            Error::FileObject(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<javac_tools::error::Error> for Error {
    fn from(err: javac_tools::error::Error) -> Error {
        Error::FileObject(err)
    }
}

fn bad_class_file(message: impl Into<String>) -> Error {
    // Java strings are JavaString; the BadClassFile messages are constructed
    // from the same texts as upstream (some include numbers).
    let message: String = message.into();
    Error::BadClassFile(BadClassFile::new(JavaString::from(message.as_str())))
}

fn pool_error(err: crate::jvm::pool_reader::Error) -> Error {
    bad_class_file(err.to_string())
}

/// Stripped down ClassReader, just sufficient to read module names from
/// module-info.class files while analyzing the module path.
pub struct ModuleNameReader {
    /// Java: private ByteBuffer buf = new ByteBuffer(INITIAL_BUFFER_SIZE);
    buf: ByteBuffer,
    /// Java: private int bp;
    bp: i32,
    /// Java: private PoolReader reader;
    reader: PoolReader,
    /// PORTING_RULES: the Java PoolReader holds the Names; the port passes it
    /// explicitly to the decoding methods.
    names: Names,
}

impl Default for ModuleNameReader {
    fn default() -> ModuleNameReader {
        ModuleNameReader::new()
    }
}

impl ModuleNameReader {
    /// Java: private static final int INITIAL_BUFFER_SIZE = 0x0fff0;
    const INITIAL_BUFFER_SIZE: i32 = 0x0fff0;

    /// Java: public ModuleNameReader()
    pub fn new() -> ModuleNameReader {
        ModuleNameReader {
            buf: ByteBuffer::with_initial_size(Self::INITIAL_BUFFER_SIZE),
            bp: 0,
            reader: PoolReader::new(),
            names: Names::new(false),
        }
    }

    /// Java: public String readModuleName(Path p) throws IOException, BadClassFile
    pub fn read_module_name(&mut self, p: &Path) -> Result<JavaString, Error> {
        let mut input = std::fs::File::open(p)?;
        self.read_module_name_from_reader(&mut input)
    }

    /// Java: public String readModuleName(JavaFileObject jfo) throws IOException, BadClassFile
    pub fn read_module_name_from_java_file_object(
        &mut self,
        jfo: &dyn JavaFileObject,
    ) -> Result<JavaString, Error> {
        let mut input = jfo.open_input_stream()?;
        self.read_module_name_from_reader(input.as_mut())
    }

    /// Java: public String readModuleName(InputStream in) throws IOException, BadClassFile
    ///
    /// Rust form: the InputStream is a &mut dyn Read.
    pub fn read_module_name_from_reader(
        &mut self,
        input: &mut dyn Read,
    ) -> Result<JavaString, Error> {
        self.bp = 0;
        self.buf.reset();
        // Java: buf.appendStream(in); the dyn-reader form needs one Sized hop
        // because ByteBuffer::append_stream takes &mut R: Read.
        let mut input = input;
        self.buf.append_stream(&mut input)?;

        let magic = self.next_int();
        if magic != JAVA_MAGIC {
            return Err(bad_class_file("illegal.start.of.class.file"));
        }

        let _minor_version = self.next_char();
        let major_version = self.next_char();
        if (major_version as i32) < 53 {
            return Err(bad_class_file(format!(
                "bad major version number for module: {}",
                major_version
            )));
        }

        self.reader = PoolReader::new();
        self.bp = self
            .reader
            .read_pool(&self.buf, self.bp)
            .map_err(pool_error)?;

        let access_flags = self.next_char();
        if access_flags != 0x8000 {
            return Err(bad_class_file(format!(
                "invalid access flags for module: 0x{:x}",
                access_flags
            )));
        }

        let _this_class = self.next_char();
        // could, should, check this_class == CONSTANT_Class("module-info")
        let super_class = self.next_char();
        self.check_zero(super_class, "super_class")?;
        let interface_count = self.next_char();
        self.check_zero(interface_count, "interface_count")?;
        let fields_count = self.next_char();
        self.check_zero(fields_count, "fields_count")?;
        let methods_count = self.next_char();
        self.check_zero(methods_count, "methods_count")?;
        let attributes_count = self.next_char();
        for _ in 0..attributes_count {
            let attr_name = self.next_char();
            let attr_length = self.next_int();
            let name = self
                .reader
                .peek_name(&self.buf, attr_name as i32, false, &self.names)
                .map_err(pool_error)?;
            if name.to_string_lossy() == "Module" && attr_length > 2 {
                let module_name_index = self.next_char();
                return self
                    .reader
                    .peek_module_name(&self.buf, module_name_index as i32, true, &self.names)
                    .map_err(pool_error);
            } else {
                // skip over unknown attributes
                self.bp += attr_length;
            }
        }
        Err(bad_class_file("no Module attribute"))
    }

    /// Java: void checkZero(int count, String name) throws BadClassFile
    fn check_zero(&self, count: u16, name: &str) -> Result<(), Error> {
        if count != 0 {
            Err(bad_class_file(format!("invalid {} for module: {}", name, count)))
        } else {
            Ok(())
        }
    }

    /// Java: char nextChar()
    fn next_char(&mut self) -> u16 {
        let res = self.buf.get_char(self.bp);
        self.bp += 2;
        res
    }

    /// Java: int nextInt()
    fn next_int(&mut self) -> i32 {
        let res = self.buf.get_int(self.bp);
        self.bp += 4;
        res
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ModuleNameReader};
    use crate::jvm::class_file::JAVA_MAGIC;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn baseline_java_home() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let home = PathBuf::from(home);
            if home.join("lib").join("modules").is_file() || home.join("jmods").is_dir() {
                return Some(home);
            }
        }
        let fallback = PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot");
        if fallback.join("lib").join("modules").is_file() || fallback.join("jmods").is_dir() {
            return Some(fallback);
        }
        None
    }

    fn module_info_bytes(module: &str) -> Option<Vec<u8>> {
        let home = baseline_java_home()?;
        let class_path = javac_classpath::ClassPath::system(&home).expect("system class path");
        class_path.read_class_in_module(module, "module-info")
    }

    #[test]
    fn reads_the_module_name_of_java_base() {
        let Some(bytes) = module_info_bytes("java.base") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        let mut reader = ModuleNameReader::new();
        let name = reader
            .read_module_name_from_reader(&mut Cursor::new(bytes))
            .expect("module name");
        assert_eq!(name.to_string_lossy(), "java.base");
        // Cross-check with the container's module list (jimage/jmods).
        let home = baseline_java_home().unwrap();
        let class_path = javac_classpath::ClassPath::system(&home).expect("system class path");
        assert!(class_path.list_modules().iter().any(|module| module == "java.base"));
    }

    #[test]
    fn reads_the_module_name_from_a_path() {
        let Some(bytes) = module_info_bytes("java.base") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        let path = std::env::temp_dir().join(format!(
            "javac-module-info-{}-{}.class",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, &bytes).expect("write module-info");
        let mut reader = ModuleNameReader::new();
        let result = reader.read_module_name(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result.expect("module name").to_string_lossy(), "java.base");
    }

    fn assert_bad_class_file(result: Result<javac_strings::JavaString, Error>, expected: &str) {
        match result {
            Err(Error::BadClassFile(err)) => {
                assert_eq!(err.message.to_string_lossy(), expected)
            }
            Err(other) => panic!("expected BadClassFile, got {}", other),
            Ok(_) => panic!("expected BadClassFile({}), got a module name", expected),
        }
    }

    #[test]
    fn rejects_a_non_class_file() {
        let mut reader = ModuleNameReader::new();
        let wrong_magic = vec![0u8; 16];
        assert_bad_class_file(
            reader.read_module_name_from_reader(&mut Cursor::new(wrong_magic)),
            "illegal.start.of.class.file",
        );
    }

    #[test]
    fn rejects_a_pre_module_class_file() {
        let bytes = vec![
            0xCA, 0xFE, 0xBA, 0xBE, // magic
            0x00, 0x00, // minor version
            0x00, 0x34, // major version 52 (< 53)
        ];
        let mut reader = ModuleNameReader::new();
        assert_bad_class_file(
            reader.read_module_name_from_reader(&mut Cursor::new(bytes)),
            "bad major version number for module: 52",
        );
    }

    #[test]
    fn rejects_a_module_info_without_module_attribute() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(JAVA_MAGIC as u32).to_be_bytes()); // magic
        bytes.extend_from_slice(&[0x00, 0x00]); // minor
        bytes.extend_from_slice(&[0x00, 0x35]); // major 53
        bytes.extend_from_slice(&[0x00, 0x01]); // constant_pool_count = 1 (empty pool)
        bytes.extend_from_slice(&[0x80, 0x00]); // access_flags = ACC_MODULE
        bytes.extend_from_slice(&[0x00, 0x00]); // this_class
        bytes.extend_from_slice(&[0x00, 0x00]); // super_class
        bytes.extend_from_slice(&[0x00, 0x00]); // interface_count
        bytes.extend_from_slice(&[0x00, 0x00]); // fields_count
        bytes.extend_from_slice(&[0x00, 0x00]); // methods_count
        bytes.extend_from_slice(&[0x00, 0x00]); // attributes_count = 0
        let mut reader = ModuleNameReader::new();
        assert_bad_class_file(
            reader.read_module_name_from_reader(&mut Cursor::new(bytes)),
            "no Module attribute",
        );
    }

    #[test]
    fn rejects_wrong_access_flags() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(JAVA_MAGIC as u32).to_be_bytes());
        bytes.extend_from_slice(&[0x00, 0x00]);
        bytes.extend_from_slice(&[0x00, 0x35]);
        bytes.extend_from_slice(&[0x00, 0x01]); // empty pool
        bytes.extend_from_slice(&[0x00, 0x00]); // access_flags = 0 (not ACC_MODULE)
        let mut reader = ModuleNameReader::new();
        assert_bad_class_file(
            reader.read_module_name_from_reader(&mut Cursor::new(bytes)),
            "invalid access flags for module: 0x0",
        );
    }
}
