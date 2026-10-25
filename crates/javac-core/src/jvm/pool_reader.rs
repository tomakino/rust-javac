// Origin: com/sun/tools/javac/jvm/PoolReader.java @ 982d53c (jdk-17.0.19-ga)

//! Pool interface towards ClassReader. Exposes methods to decode and read javac
//! entities from the constant pool.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES (1:1 where the dependency is ported; the rest is reported):
//! * ported dependencies: ClassFile constants/internalize (crate::jvm::class_file),
//!   ByteBuffer (crate::util::byte_buffer), Convert.utf2string
//!   (crate::util::convert), Name/Names (crate::util::name/names).
//! * NOT ported yet (TODO, no substitute semantics):
//!   - ClassReader: the Java constructors taking ClassReader/Names/Symtab and the
//!     helpers getClass/getModule/getPackage/getType/getNameAndType (they return
//!     ClassSymbol/ModuleSymbol/PackageSymbol/Type/NameAndType) are not ported;
//!     the ClassReader.badClassFile error channel is replaced by the local
//!     [Error] enum carrying the same message keys for later wiring;
//!   - PoolConstant (and PoolConstant.NameAndType): the lazily resolved values
//!     are represented by the raw [PoolValue] adapter below; the file header
//!     records the wiring for the P2 ClassReader (resolve() -> PoolValue, then
//!     ClassReader maps it to symbols/types);
//!   - Type: CONSTANT_NameandType stores the raw descriptor Name instead of the
//!     sigToType()-produced Type;
//!   - Symtab: CONSTANT_Class/Module/Package return the raw internalized names
//!     instead of entered symbols;
//!   - java.util.BitSet: replaced by the closed [TagSet] enum (same tag
//!     membership as the Java static BitSets);
//!   - Names/Symtab constructor injection: the name table is passed to the
//!     decoding methods (signature-level adaptation; Java stores it in a field).
//!
//! Local adapters keep the Java names/order: readPool/offset/tag/readIfNeeded/
//! resolve/getUtf8/peekName/peekClassName/peekPackageName/peekModuleName/
//! hasTag/getConstant/sizeof.
//! 
//! PORTING_RULES: some items are the wiring points for the P2 ClassReader
//! (getClass/getModule/getPackage/getType/getNameAndType are still TODO; the raw
//! adapters and the tag sets they use are consumed by the ClassReader in a later
//! batch), so they are kept with the upstream shape and marked allow(dead_code).
#![allow(dead_code)]

use crate::jvm::class_file::{
    internalize, CONSTANT_CLASS, CONSTANT_DOUBLE, CONSTANT_DYNAMIC, CONSTANT_FIELDREF,
    CONSTANT_FLOAT, CONSTANT_INTEGER, CONSTANT_INTERFACEMETHODREF, CONSTANT_INVOKEDYNAMIC,
    CONSTANT_LONG, CONSTANT_METHODHANDLE, CONSTANT_METHODREF, CONSTANT_METHODTYPE,
    CONSTANT_MODULE, CONSTANT_NAMEANDTYPE, CONSTANT_PACKAGE, CONSTANT_STRING, CONSTANT_UTF8,
};
use crate::util::byte_buffer::ByteBuffer;
use crate::util::convert;
use crate::util::name::Name;
use crate::util::names::Names;
use javac_strings::JavaString;
use std::cell::RefCell;
use std::fmt;

/// The raw representation of a lazily resolved constant pool value.
///
/// PORTING_RULES: PoolConstant/PoolConstant.NameAndType and the
/// ClassSymbol/ModuleSymbol/PackageSymbol/Type results of the Java resolve()
/// are not ported yet; this adapter carries the raw value (see the file header
/// for the P2 ClassReader wiring).
#[derive(Clone)]
pub(crate) enum PoolValue {
    /// Java: resolve() CONSTANT_Utf8 -> Name (names.fromUtf).
    Utf8(Name),
    /// Java: resolve() CONSTANT_Class -> ClassSymbol; the port keeps the
    /// internalized class name (the value the symbol is entered from).
    Class(Name),
    /// Java: resolve() CONSTANT_NameandType -> PoolConstant.NameAndType; the
    /// port keeps the raw name and descriptor indices resolved to Names
    /// (the Type produced by sigToType is not ported).
    NameAndType { name: Name, descriptor: Name },
    /// Java: resolve() CONSTANT_Integer -> Integer.
    Integer(i32),
    /// Java: resolve() CONSTANT_Float -> Float.
    Float(f32),
    /// Java: resolve() CONSTANT_Long -> Long.
    Long(i64),
    /// Java: resolve() CONSTANT_Double -> Double.
    Double(f64),
    /// Java: resolve() CONSTANT_String -> String (getName(index).toString()).
    String(JavaString),
    /// Java: resolve() CONSTANT_Package -> PackageSymbol; the port keeps the
    /// internalized package name.
    Package(Name),
    /// Java: resolve() CONSTANT_Module -> ModuleSymbol; the port keeps the name.
    Module(Name),
}

/// Error channel matching the ClassReader.badClassFile call sites used by this
/// class.
///
/// PORTING_RULES: TODO(P2): ClassReader.badClassFile throws a ClassFileError
/// with these message keys; the local enum keeps the keys/arguments so the
/// wiring can map them onto the ported ClassReader error type.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Error {
    /// Java: reader.badClassFile("unexpected.const.pool.tag.at", tag, offset)
    UnexpectedConstPoolTagAt { tag: i32, offset: i32 },
    /// Java: reader.badClassFile("bad.const.pool.tag.at", tag, offset)
    BadConstPoolTagAt { tag: i32, offset: i32 },
    /// Java: reader.badClassFile("bad.const.pool.index", classFile, index, length)
    BadConstPoolIndex { index: i32, length: i32 },
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedConstPoolTagAt { tag, offset } => {
                write!(f, "unexpected.const.pool.tag.at(tag={}, offset={})", tag, offset)
            }
            Error::BadConstPoolTagAt { tag, offset } => {
                write!(f, "bad.const.pool.tag.at(tag={}, offset={})", tag, offset)
            }
            Error::BadConstPoolIndex { index, length } => {
                write!(f, "bad.const.pool.index(index={}, length={})", index, length)
            }
        }
    }
}

impl std::error::Error for Error {}

/// PORTING_RULES: java.util.BitSet classCP/constantCP/moduleCP/packageCP/
/// utf8CP/nameAndTypeCP are replaced by this closed enum (same membership).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TagSet {
    Class,
    Constant,
    Module,
    Package,
    Utf8,
    NameAndType,
}

impl TagSet {
    /// Java: the corresponding static BitSet.
    fn accepts(self, tag: i32) -> bool {
        match self {
            TagSet::Class => tag == CONSTANT_CLASS,
            // Java: constantCP.set(CONSTANT_Integer, CONSTANT_String + 1)
            TagSet::Constant => (CONSTANT_INTEGER..=CONSTANT_STRING).contains(&tag),
            TagSet::Module => tag == CONSTANT_MODULE,
            TagSet::Package => tag == CONSTANT_PACKAGE,
            TagSet::Utf8 => tag == CONSTANT_UTF8,
            TagSet::NameAndType => tag == CONSTANT_NAMEANDTYPE,
        }
    }
}

/// The constant pool reader.
///
/// PORTING_RULES: single-threaded; the lazily filled values array is a
/// RefCell.  Java stores the buffer/names in fields; here they are passed to the
/// decoding methods (signature-level adaptation, see the file header).
pub(crate) struct PoolReader {
    /// Java: (inside ImmutablePoolHelper) final int[] offsets;
    offsets: Vec<i32>,
    /// Java: (inside ImmutablePoolHelper) final Object[] values;
    values: RefCell<Vec<Option<PoolValue>>>,
}

impl PoolReader {
    /// Java: PoolReader(ByteBuffer buf) / PoolReader(ClassReader, Names, Symtab)
    /// — the ClassReader form is not ported yet (see the file header).
    pub(crate) fn new() -> PoolReader {
        PoolReader { offsets: Vec::new(), values: RefCell::new(Vec::new()) }
    }

    /// Java: peekClassName(index, mapper) — Get class name without resolving.
    pub(crate) fn peek_class_name(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        internalize_name: bool,
        names: &Names,
    ) -> Result<JavaString, Error> {
        let name_index = poolbuf.get_char(self.offset(index)?) as i32;
        self.get_utf8(poolbuf, name_index, internalize_name, names)
    }

    /// Java: peekPackageName(index, mapper) — Get package name without resolving.
    pub(crate) fn peek_package_name(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        internalize_name: bool,
        names: &Names,
    ) -> Result<JavaString, Error> {
        let name_index = poolbuf.get_char(self.offset(index)?) as i32;
        self.get_utf8(poolbuf, name_index, internalize_name, names)
    }

    /// Java: peekModuleName(index, mapper) — Get module name without resolving.
    pub(crate) fn peek_module_name(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        internalize_name: bool,
        names: &Names,
    ) -> Result<JavaString, Error> {
        let name_index = poolbuf.get_char(self.offset(index)?) as i32;
        self.get_utf8(poolbuf, name_index, internalize_name, names)
    }

    /// Java: peekName(int index, NameMapper<Z> mapper) — Peek a name from the
    /// pool at given index without resolving.
    ///
    /// PORTING_RULES: the generic NameMapper<Z> is represented by the
    /// internalize_name flag (the two mappers ModuleNameReader constructs).
    pub(crate) fn peek_name(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        internalize_name: bool,
        names: &Names,
    ) -> Result<JavaString, Error> {
        self.get_utf8(poolbuf, index, internalize_name, names)
    }

    /// Java: getName(int index) -> Name
    pub(crate) fn get_name(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        names: &Names,
    ) -> Result<Name, Error> {
        match self.read_if_needed(poolbuf, index, TagSet::Utf8, names)? {
            PoolValue::Utf8(name) => Ok(name),
            other => Err(Error::UnexpectedConstPoolTagAt {
                tag: pool_tag_of(&other),
                offset: self.offset(index)?,
            }),
        }
    }

    /// Java: getConstant(int index) -> Object — Get a constant from the pool at
    /// given index (raw [PoolValue] adapter).
    pub(crate) fn get_constant(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        names: &Names,
    ) -> Result<PoolValue, Error> {
        self.read_if_needed(poolbuf, index, TagSet::Constant, names)
    }

    /// Java: hasTag(int index, int tag)
    pub(crate) fn has_tag(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        tag: i32,
    ) -> Result<bool, Error> {
        Ok(self.tag(poolbuf, index)? == tag)
    }

    /// Java: private <Z> Z getUtf8(int index, NameMapper<Z> mapper)
    fn get_utf8(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        internalize_name: bool,
        _names: &Names,
    ) -> Result<JavaString, Error> {
        let tag = self.tag(poolbuf, index)?;
        let offset = self.offset(index)?;
        if tag == CONSTANT_UTF8 {
            let len = poolbuf.get_char(offset) as i32;
            if internalize_name {
                let internalized = internalize(&poolbuf.elems, offset + 2, len);
                Ok(convert::utf2string(&internalized))
            } else {
                Ok(convert::utf2string_range(&poolbuf.elems, offset + 2, len))
            }
        } else {
            Err(Error::UnexpectedConstPoolTagAt { tag, offset: offset - 1 })
        }
    }

    /// Java: private Object resolve(ByteBuffer poolbuf, int tag, int offset)
    fn resolve(
        &self,
        poolbuf: &ByteBuffer,
        tag: i32,
        offset: i32,
        names: &Names,
    ) -> Result<PoolValue, Error> {
        match tag {
            CONSTANT_UTF8 => {
                let len = poolbuf.get_char(offset) as i32;
                let name = names.table.from_utf(&poolbuf.elems, offset + 2, len);
                Ok(PoolValue::Utf8(name))
            }
            CONSTANT_CLASS => {
                // Java: Name name = names.fromUtf(getName(index).map(ClassFile::internalize));
                //       return syms.enterClass(reader.currentModule, name);
                // TODO(P2): Symtab/currentModule are not ported; the port keeps the
                // internalized raw name.
                let index = poolbuf.get_char(offset) as i32;
                let name = self.get_name(poolbuf, index, names)?;
                let internalized = name.map(|bytes, start, len| {
                    let bytes = internalize(bytes, start, len);
                    names.table.from_utf(&bytes, 0, len)
                });
                Ok(PoolValue::Class(internalized))
            }
            CONSTANT_NAMEANDTYPE => {
                // Java: Name name = getName(poolbuf.getChar(offset));
                //       Type type = getType(poolbuf.getChar(offset + 2));
                //       return new NameAndType(name, type);
                // TODO(P2): Type/sigToType is not ported; the raw descriptor Name is
                // kept instead.
                let name_index = poolbuf.get_char(offset) as i32;
                let descriptor_index = poolbuf.get_char(offset + 2) as i32;
                let name = self.get_name(poolbuf, name_index, names)?;
                let descriptor = self.get_name(poolbuf, descriptor_index, names)?;
                Ok(PoolValue::NameAndType { name, descriptor })
            }
            CONSTANT_INTEGER => Ok(PoolValue::Integer(poolbuf.get_int(offset))),
            CONSTANT_FLOAT => Ok(PoolValue::Float(poolbuf.get_float(offset))),
            CONSTANT_LONG => Ok(PoolValue::Long(poolbuf.get_long(offset))),
            CONSTANT_DOUBLE => Ok(PoolValue::Double(poolbuf.get_double(offset))),
            CONSTANT_STRING => {
                // Java: return getName(poolbuf.getChar(offset)).toString();
                let index = poolbuf.get_char(offset) as i32;
                let name = self.get_name(poolbuf, index, names)?;
                Ok(PoolValue::String(name.to_java_string()))
            }
            CONSTANT_PACKAGE => {
                // Java: Name name = getName(poolbuf.getChar(offset));
                //       return syms.enterPackage(reader.currentModule, names.fromUtf(internalize(name)));
                // TODO(P2): Symtab/currentModule are not ported; the port keeps the
                // internalized raw name.
                let index = poolbuf.get_char(offset) as i32;
                let name = self.get_name(poolbuf, index, names)?;
                let internalized = name.map(|bytes, start, len| {
                    let bytes = internalize(bytes, start, len);
                    names.table.from_utf(&bytes, 0, len)
                });
                Ok(PoolValue::Package(internalized))
            }
            CONSTANT_MODULE => {
                // Java: Name name = getName(poolbuf.getChar(offset));
                //       return syms.enterModule(name);
                // TODO(P2): Symtab is not ported; the port keeps the name.
                let index = poolbuf.get_char(offset) as i32;
                let name = self.get_name(poolbuf, index, names)?;
                Ok(PoolValue::Module(name))
            }
            _ => Err(Error::UnexpectedConstPoolTagAt { tag, offset: offset - 1 }),
        }
    }

    /// Java: int readPool(ByteBuffer poolbuf, int offset)
    pub(crate) fn read_pool(&mut self, poolbuf: &ByteBuffer, offset: i32) -> Result<i32, Error> {
        let pool_size = poolbuf.get_char(offset) as i32;
        let mut index = 1;
        let mut offset = offset + 2;
        let mut offsets = vec![0i32; pool_size as usize];
        while index < pool_size {
            let tag = poolbuf.get_byte(offset) as i32;
            offset += 1;
            offsets[index as usize] = offset;
            match tag {
                CONSTANT_UTF8 => {
                    let len = poolbuf.get_char(offset) as i32;
                    offset += 2 + len;
                }
                CONSTANT_CLASS | CONSTANT_STRING | CONSTANT_MODULE | CONSTANT_PACKAGE
                | CONSTANT_METHODTYPE => {
                    offset += 2;
                }
                CONSTANT_METHODHANDLE => {
                    offset += 3;
                }
                CONSTANT_FIELDREF | CONSTANT_METHODREF | CONSTANT_INTERFACEMETHODREF
                | CONSTANT_NAMEANDTYPE | CONSTANT_INTEGER | CONSTANT_FLOAT | CONSTANT_DYNAMIC
                | CONSTANT_INVOKEDYNAMIC => {
                    offset += 4;
                }
                CONSTANT_LONG | CONSTANT_DOUBLE => {
                    offset += 8;
                }
                _ => {
                    return Err(Error::BadConstPoolTagAt { tag, offset: offset - 1 });
                }
            }
            index += self.sizeof(tag);
        }
        self.offsets = offsets;
        self.values = RefCell::new(vec![None; pool_size as usize]);
        Ok(offset)
    }

    /// Java: private int sizeof(int tag)
    fn sizeof(&self, tag: i32) -> i32 {
        match tag {
            CONSTANT_DOUBLE | CONSTANT_LONG => 2,
            _ => 1,
        }
    }

    /// Java: ImmutablePoolHelper.offset(int index)
    pub(crate) fn offset(&self, index: i32) -> Result<i32, Error> {
        let index = self.check_index(index)?;
        Ok(self.offsets[index as usize])
    }

    /// Java: ImmutablePoolHelper.tag(int index)
    pub(crate) fn tag(&self, poolbuf: &ByteBuffer, index: i32) -> Result<i32, Error> {
        let offset = self.offset(index)?;
        Ok(poolbuf.elems[(offset - 1) as usize] as i32)
    }

    /// Java: ImmutablePoolHelper.readIfNeeded(int index, BitSet expectedTags)
    ///
    /// PORTING_RULES: Java is generic over the expected value type (P); the raw
    /// PoolValue adapter is returned instead (see the file header).
    pub(crate) fn read_if_needed(
        &self,
        poolbuf: &ByteBuffer,
        index: i32,
        expected: TagSet,
        names: &Names,
    ) -> Result<PoolValue, Error> {
        let index = self.check_index(index)?;
        if let Some(value) = self.values.borrow()[index as usize].clone() {
            return Ok(value);
        }
        let current_tag = self.tag(poolbuf, index)?;
        if !expected.accepts(current_tag) {
            return Err(Error::UnexpectedConstPoolTagAt {
                tag: current_tag,
                offset: self.offset(index)?,
            });
        }
        let value = self.resolve(poolbuf, current_tag, self.offset(index)?, names)?;
        self.values.borrow_mut()[index as usize] = Some(value.clone());
        Ok(value)
    }

    /// Java: ImmutablePoolHelper.checkIndex(int index)
    fn check_index(&self, index: i32) -> Result<i32, Error> {
        if index <= 0 || index as usize >= self.offsets.len() {
            Err(Error::BadConstPoolIndex { index, length: self.offsets.len() as i32 })
        } else {
            Ok(index)
        }
    }
}

/// The tag of a resolved [PoolValue] (used for the unexpected-tag error).
fn pool_tag_of(value: &PoolValue) -> i32 {
    match value {
        PoolValue::Utf8(_) => CONSTANT_UTF8,
        PoolValue::Class(_) => CONSTANT_CLASS,
        PoolValue::NameAndType { .. } => CONSTANT_NAMEANDTYPE,
        PoolValue::Integer(_) => CONSTANT_INTEGER,
        PoolValue::Float(_) => CONSTANT_FLOAT,
        PoolValue::Long(_) => CONSTANT_LONG,
        PoolValue::Double(_) => CONSTANT_DOUBLE,
        PoolValue::String(_) => CONSTANT_STRING,
        PoolValue::Package(_) => CONSTANT_PACKAGE,
        PoolValue::Module(_) => CONSTANT_MODULE,
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, PoolReader, PoolValue, TagSet};
    use crate::jvm::class_file::{
        CONSTANT_CLASS, CONSTANT_DOUBLE, CONSTANT_DYNAMIC, CONSTANT_FIELDREF, CONSTANT_FLOAT,
        CONSTANT_INTEGER, CONSTANT_INTERFACEMETHODREF, CONSTANT_INVOKEDYNAMIC, CONSTANT_LONG,
        CONSTANT_METHODHANDLE, CONSTANT_METHODREF, CONSTANT_METHODTYPE, CONSTANT_MODULE,
        CONSTANT_NAMEANDTYPE, CONSTANT_PACKAGE, CONSTANT_STRING, CONSTANT_UTF8,
    };
    use crate::util::byte_buffer::ByteBuffer;
    use crate::util::names::Names;
    use std::path::{Path, PathBuf};

    /// The baseline JDK 17 used by the container-reader tests.
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

    fn baseline_class_bytes(binary_name: &str) -> Option<Vec<u8>> {
        let home = baseline_java_home()?;
        let class_path = javac_classpath::ClassPath::system(&home).expect("system class path");
        class_path.read_class(binary_name)
    }

    fn buffer_with(bytes: &[u8]) -> ByteBuffer {
        let mut buf = ByteBuffer::with_initial_size(bytes.len() as i32);
        buf.elems = bytes.to_vec();
        buf.length = bytes.len() as i32;
        buf
    }

    #[test]
    fn object_constant_pool_matches_javap() {
        let Some(bytes) = baseline_class_bytes("java.lang.Object") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        // javap -v java.lang.Object (17.0.19): constant_pool_count = 92 and the
        // recorded tag counts/samples below.
        let buf = buffer_with(&bytes);
        let mut reader = PoolReader::new();
        let end = reader.read_pool(&buf, 8).expect("readPool");
        // The constant pool is followed by access_flags/this_class/... so the
        // pool end is inside the file; pool_count 92 matches javap.
        assert!(end > 8 && (end as usize) < bytes.len(), "pool end inside the file");
        assert_eq!(buf.get_char(8), 92, "constant_pool_count");

        let names = Names::new(false);
        // Sample entries recorded from javap: #2 Utf8 java/lang/StringBuilder,
        // #5 Utf8 <init>, #6 Utf8 ()V, #91 Utf8 Object.java.
        assert_eq!(
            reader.peek_name(&buf, 2, false, &names).unwrap().to_string_lossy(),
            "java/lang/StringBuilder"
        );
        assert_eq!(reader.peek_name(&buf, 5, false, &names).unwrap().to_string_lossy(), "<init>");
        assert_eq!(reader.peek_name(&buf, 6, false, &names).unwrap().to_string_lossy(), "()V");
        assert_eq!(
            reader.peek_name(&buf, 91, false, &names).unwrap().to_string_lossy(),
            "Object.java"
        );

        // #1 Class #2 -> internalized name java.lang.StringBuilder.
        match reader.read_if_needed(&buf, 1, TagSet::Class, &names).unwrap() {
            PoolValue::Class(name) => assert_eq!(name.to_java_string().to_string_lossy(), "java.lang.StringBuilder"),
            other => panic!("expected Class, got {}", describe(&other)),
        }
        // #4 NameAndType #5:#6 -> <init> : ()V.
        match reader.read_if_needed(&buf, 4, TagSet::NameAndType, &names).unwrap() {
            PoolValue::NameAndType { name, descriptor } => {
                assert_eq!(name.to_java_string().to_string_lossy(), "<init>");
                assert_eq!(descriptor.to_java_string().to_string_lossy(), "()V");
            }
            other => panic!("expected NameAndType, got {}", describe(&other)),
        }
        // #23 String #24 -> "@"; #49 Integer 999999; #54 Long 9223372036854775807.
        match reader.get_constant(&buf, 23, &names).unwrap() {
            PoolValue::String(value) => assert_eq!(value.to_string_lossy(), "@"),
            other => panic!("expected String, got {}", describe(&other)),
        }
        match reader.get_constant(&buf, 49, &names).unwrap() {
            PoolValue::Integer(value) => assert_eq!(value, 999999),
            other => panic!("expected Integer, got {}", describe(&other)),
        }
        match reader.get_constant(&buf, 54, &names).unwrap() {
            PoolValue::Long(value) => assert_eq!(value, 9223372036854775807),
            other => panic!("expected Long, got {}", describe(&other)),
        }
        assert!(reader.has_tag(&buf, 1, CONSTANT_CLASS).unwrap());
        assert!(!reader.has_tag(&buf, 1, CONSTANT_UTF8).unwrap());

        // Bad indices keep the ClassReader.badClassFile("bad.const.pool.index")
        // channel.
        assert!(matches!(
            reader.read_if_needed(&buf, 0, TagSet::Constant, &names),
            Err(Error::BadConstPoolIndex { .. })
        ));
        assert!(matches!(
            reader.read_if_needed(&buf, 92, TagSet::Constant, &names),
            Err(Error::BadConstPoolIndex { .. })
        ));
    }

    fn describe(value: &PoolValue) -> String {
        match value {
            PoolValue::Utf8(_) => "Utf8".to_string(),
            PoolValue::Class(_) => "Class".to_string(),
            PoolValue::NameAndType { .. } => "NameAndType".to_string(),
            PoolValue::Integer(_) => "Integer".to_string(),
            PoolValue::Float(_) => "Float".to_string(),
            PoolValue::Long(_) => "Long".to_string(),
            PoolValue::Double(_) => "Double".to_string(),
            PoolValue::String(_) => "String".to_string(),
            PoolValue::Package(_) => "Package".to_string(),
            PoolValue::Module(_) => "Module".to_string(),
        }
    }

    /// readPool must skip every tag with the upstream sizes (2/3/4/8 bytes and
    /// the two-slot long/double rule).
    #[test]
    fn read_pool_skips_all_tag_sizes() {
        let entries: &[(i32, usize)] = &[
            (CONSTANT_UTF8, 2),
            (CONSTANT_CLASS, 2),
            (CONSTANT_STRING, 2),
            (CONSTANT_MODULE, 2),
            (CONSTANT_PACKAGE, 2),
            (CONSTANT_METHODTYPE, 2),
            (CONSTANT_METHODHANDLE, 3),
            (CONSTANT_FIELDREF, 4),
            (CONSTANT_METHODREF, 4),
            (CONSTANT_INTERFACEMETHODREF, 4),
            (CONSTANT_NAMEANDTYPE, 4),
            (CONSTANT_INTEGER, 4),
            (CONSTANT_FLOAT, 4),
            (CONSTANT_DYNAMIC, 4),
            (CONSTANT_INVOKEDYNAMIC, 4),
            (CONSTANT_LONG, 8),
            (CONSTANT_DOUBLE, 8),
        ];
        // Indices: 1..=15 for the first 15 entries, then Long uses 16/17, Double
        // uses 18/19 -> constant_pool_count = 20.
        let pool_size: i32 = 20;
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&(pool_size as u16).to_be_bytes());
        for (tag, size) in entries {
            bytes.push(*tag as u8);
            // CONSTANT_Utf8 payload is a u2 length followed by the bytes; here it
            // is the two length bytes with an empty string.
            let payload = *size;
            bytes.extend(vec![0u8; payload]);
        }
        let buf = buffer_with(&bytes);
        let mut reader = PoolReader::new();
        let end = reader.read_pool(&buf, 0).expect("readPool skips all tags");
        assert_eq!(end as usize, bytes.len());
        assert_eq!(reader.tag(&buf, 1).unwrap(), CONSTANT_UTF8);
        assert_eq!(reader.tag(&buf, 16).unwrap(), CONSTANT_LONG);
        assert_eq!(reader.tag(&buf, 18).unwrap(), CONSTANT_DOUBLE);
    }

    /// A corrupt tag reports the ClassReader.badClassFile("bad.const.pool.tag.at")
    /// channel.
    #[test]
    fn read_pool_reports_bad_tag() {
        let bytes: Vec<u8> = vec![0, 3, 99, 0, 0];
        let buf = buffer_with(&bytes);
        let mut reader = PoolReader::new();
        assert!(matches!(
            reader.read_pool(&buf, 0),
            Err(Error::BadConstPoolTagAt { tag: 99, .. })
        ));
    }

    /// Keeps the Path import needed by baseline_java_home's documentation.
    #[allow(dead_code)]
    fn _unused_path(_: &Path) {}
}
