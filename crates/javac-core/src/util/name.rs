//! An abstraction for internal compiler strings. They are stored in Utf8 format.
//! Names are stored in a Name.Table, and are unique within that table.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! the Java abstract class Name plus its two concrete
//! implementations (SharedNameTable.NameImpl, UnsharedNameTable.NameImpl) are
//! expressed as one Rust struct Name (a handle) whose bytes are snapshotted when
//! the name is interned. Name.Table is expressed as NameTableHandle, with the
//! two implementations selected by NameTableInner. The table itself is shared and
//! internally mutable (interning), which is a registered Rc<RefCell<...>>
//! exception (see ).
//!
//! Equality mirrors Java: names are equal iff they come from the same table and
//! have the same index (identity within a table).

use std::cell::RefCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};

use crate::jvm::class_file;
use crate::util::convert;
use crate::util::java_string::JavaString;
use crate::util::names::NamesStd;
use crate::util::shared_name_table::SharedNameTable;
use crate::util::unshared_name_table::UnsharedNameTable;

/// An abstraction for the hash table used to create unique Name instances.
pub struct NameTableHandle {
    inner: RefCell<NameTableInner>,
    names: RefCell<Option<Weak<NamesStd>>>,
}

enum NameTableInner {
    Shared(SharedNameTable),
    Unshared(UnsharedNameTable),
}

impl NameTableHandle {
    pub fn shared() -> Rc<NameTableHandle> {
        // Java: new SharedNameTable(names) -> this(names, 0x8000, 0x20000)
        Rc::new(NameTableHandle {
            inner: RefCell::new(NameTableInner::Shared(SharedNameTable::new(
                0x8000, 0x20000,
            ))),
            names: RefCell::new(None),
        })
    }

    pub fn unshared() -> Rc<NameTableHandle> {
        // Java: new UnsharedNameTable(names) -> this(names, 0x8000)
        Rc::new(NameTableHandle {
            inner: RefCell::new(NameTableInner::Unshared(UnsharedNameTable::new(0x8000))),
            names: RefCell::new(None),
        })
    }

    pub fn set_names(&self, names: &Rc<NamesStd>) {
        *self.names.borrow_mut() = Some(Rc::downgrade(names));
    }

    pub fn names(&self) -> Rc<NamesStd> {
        self.names
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .expect("Name table used before Names was fully constructed")
    }

    /// Java: Name.Table.fromChars(char[] cs, int start, int len)
    pub fn from_chars(self: &Rc<Self>, cs: &[u16], start: i32, len: i32) -> Name {
        let (index, length) = {
            let mut inner = self.inner.borrow_mut();
            match &mut *inner {
                NameTableInner::Shared(t) => t.from_chars(cs, start, len),
                NameTableInner::Unshared(t) => t.from_chars(cs, start, len),
            }
        };
        Name::new(Rc::clone(self), index, length)
    }

    /// Java: Name.Table.fromString(String s)
    pub fn from_string(self: &Rc<Self>, s: &JavaString) -> Name {
        self.from_chars(s.as_units(), 0, s.len() as i32)
    }

    /// Java: Name.Table.fromUtf(byte[] cs)
    pub fn from_utf_all(self: &Rc<Self>, cs: &[u8]) -> Name {
        self.from_utf(cs, 0, cs.len() as i32)
    }

    /// Java: Name.Table.fromUtf(byte[] cs, int start, int len)
    pub fn from_utf(self: &Rc<Self>, cs: &[u8], start: i32, len: i32) -> Name {
        let (index, length) = {
            let mut inner = self.inner.borrow_mut();
            match &mut *inner {
                NameTableInner::Shared(t) => t.from_utf(cs, start, len),
                NameTableInner::Unshared(t) => t.from_utf(cs, start, len),
            }
        };
        Name::new(Rc::clone(self), index, length)
    }

    /// Access the bytes of a name stored in this table (Java: NameImpl reads
    /// through the shared table; no per-name byte copy).
    pub(crate) fn with_bytes<R>(&self, index: i32, length: i32, f: impl FnOnce(&[u8]) -> R) -> R {
        let inner = self.inner.borrow();
        match &*inner {
            NameTableInner::Shared(t) => f(&t.bytes[index as usize..(index + length) as usize]),
            NameTableInner::Unshared(t) => f(t.bytes_at(index, length)),
        }
    }

    /// Java: Name.Table.dispose()
    pub fn dispose(&self) {
        let mut inner = self.inner.borrow_mut();
        match &mut *inner {
            NameTableInner::Shared(t) => t.dispose(),
            NameTableInner::Unshared(t) => t.dispose(),
        }
    }

    /// Java: protected static int hashValue(byte bytes[], int offset, int length)
    pub fn hash_value(bytes: &[u8], offset: i32, length: i32) -> i32 {
        let mut h: i32 = 0;
        let start = offset as usize;
        // Java: for (int i = 0; i < length; i++) { h = (h << 5) - h + bytes[offset + i]; }
        // (a negative length iterates zero times, as in Java)
        let count = if length <= 0 { 0 } else { length as usize };
        for &byte in &bytes[start..start + count] {
            h = (h << 5).wrapping_sub(h).wrapping_add(byte as i8 as i32);
        }
        h
    }

    /// Java: protected static boolean equals(byte[] bytes1, int offset1,
    ///                                          byte[] bytes2, int offset2, int length)
    pub fn equals(bytes1: &[u8], offset1: i32, bytes2: &[u8], offset2: i32, length: i32) -> bool {
        (0..length).all(|i| bytes1[(offset1 + i) as usize] == bytes2[(offset2 + i) as usize])
    }
}

/// An abstraction for internal compiler strings.
#[derive(Clone)]
pub struct Name {
    pub table: Rc<NameTableHandle>,
    index: i32,
    length: i32,
}

impl Name {
    pub(crate) fn new(table: Rc<NameTableHandle>, index: i32, length: i32) -> Name {
        Name {
            table,
            index,
            length,
        }
    }

    /// Access this name's bytes in the owning table (Java: Name.getByteArray()
    /// + getByteOffset(); the shared table stores the bytes, as in
    /// SharedNameTable.NameImpl). Callers must not intern new names inside `f`
    /// (that would re-enter the table's mutable borrow).
    pub(crate) fn with_bytes<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        self.table.with_bytes(self.index, self.length, f)
    }

    /// Java: public boolean contentEquals(CharSequence cs)
    pub fn content_equals(&self, cs: &JavaString) -> bool {
        self.to_java_string() == *cs
    }

    /// Java: public int poolTag() { return ClassFile.CONSTANT_Utf8; }
    pub fn pool_tag(&self) -> i32 {
        class_file::CONSTANT_UTF8
    }

    /// Java: public int length()
    pub fn length(&self) -> usize {
        self.to_java_string().len()
    }

    /// Java: public char charAt(int index)
    pub fn char_at(&self, index: usize) -> u16 {
        self.to_java_string().char_at(index)
    }

    /// Java: public CharSequence subSequence(int start, int end)
    pub fn sub_sequence(&self, start: usize, end: usize) -> JavaString {
        let units = self.to_java_string().as_units()[start..end].to_vec();
        JavaString::from_units(units)
    }

    /// Java: public Name append(Name n)
    pub fn append(&self, n: &Name) -> Name {
        let mut bs =
            Vec::with_capacity(self.get_byte_length() as usize + n.get_byte_length() as usize);
        self.with_bytes(|b| bs.extend_from_slice(b));
        n.with_bytes(|b| bs.extend_from_slice(b));
        self.table.from_utf(&bs, 0, bs.len() as i32)
    }

    /// Java: public Name append(char c, Name n)
    pub fn append_char(&self, c: u8, n: &Name) -> Name {
        let mut bs =
            Vec::with_capacity(self.get_byte_length() as usize + 1 + n.get_byte_length() as usize);
        self.with_bytes(|b| bs.extend_from_slice(b));
        bs.push(c);
        n.with_bytes(|b| bs.extend_from_slice(b));
        self.table.from_utf(&bs, 0, bs.len() as i32)
    }

    /// Java: public int compareTo(Name other) { return other.getIndex() - this.getIndex(); }
    pub fn compare_to(&self, other: &Name) -> i32 {
        other.get_index() - self.get_index()
    }

    /// Java: public boolean isEmpty()
    pub fn is_empty(&self) -> bool {
        self.get_byte_length() == 0
    }

    /// Java: public int lastIndexOf(byte b)
    pub fn last_index_of(&self, b: u8) -> i32 {
        self.with_bytes(|bytes| {
            bytes
                .iter()
                .rposition(|candidate| *candidate == b)
                .map(|index| index as i32)
                .unwrap_or(-1)
        })
    }

    /// Java: public boolean startsWith(Name prefix)
    pub fn starts_with(&self, prefix: &Name) -> bool {
        if self.get_byte_length() < prefix.get_byte_length() {
            return false;
        }
        prefix.with_bytes(|prefix_bytes| {
            self.with_bytes(|bytes| bytes[..prefix_bytes.len()] == *prefix_bytes)
        })
    }

    /// Java: public Name subName(int start, int end)
    pub fn sub_name(&self, start: i32, end: i32) -> Name {
        let end = if end < start { start } else { end };
        let bytes = self.with_bytes(|bytes| bytes[start as usize..end as usize].to_vec());
        self.table.from_utf(&bytes, 0, end - start)
    }

    /// Java: public String toString() { return Convert.utf2string(getByteArray(), getByteOffset(), getByteLength()); }
    pub fn to_java_string(&self) -> JavaString {
        self.with_bytes(|bytes| convert::utf2string_range(bytes, 0, self.length))
    }

    /// Java: public byte[] toUtf()
    pub fn to_utf(&self) -> Vec<u8> {
        self.with_bytes(|bytes| bytes.to_vec())
    }

    /// Java: public abstract int getIndex()
    pub fn get_index(&self) -> i32 {
        self.index
    }

    /// Java: public abstract int getByteLength()
    pub fn get_byte_length(&self) -> i32 {
        self.length
    }

    /// Java: public abstract byte getByteAt(int i)
    pub fn get_byte_at(&self, i: i32) -> u8 {
        self.with_bytes(|bytes| bytes[i as usize])
    }

    /// Java: public void getBytes(byte cs[], int start)
    pub fn get_bytes(&self, cs: &mut [u8], start: i32) {
        self.with_bytes(|bytes| {
            cs[start as usize..start as usize + bytes.len()].copy_from_slice(bytes)
        });
    }

    /// Java: public abstract byte[] getByteArray()
    pub fn get_byte_array(&self) -> Vec<u8> {
        self.to_utf()
    }

    /// Java: public abstract int getByteOffset() — 0 for a snapshot handle.
    pub fn get_byte_offset(&self) -> i32 {
        0
    }

    /// Java: Name.getByteArray() as a borrowed slice, for the few callers that
    /// only need to inspect the bytes without copying (must not intern names
    /// inside the callback).
    pub fn with_byte_array<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        self.with_bytes(f)
    }

    /// Java: public <X> X map(NameMapper<X> mapper)
    pub fn map<X>(&self, mapper: impl FnOnce(&[u8], i32, i32) -> X) -> X {
        let bytes = self.to_utf();
        mapper(&bytes, 0, self.get_byte_length())
    }

    /// Adapter: access to the standard names table (Java:
    /// name.table.names). Call sites use name.names().
    pub fn names(&self) -> Rc<NamesStd> {
        self.table.names()
    }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        // Java SharedNameTable.NameImpl.equals: table == table && index == index.
        // For UnsharedNameTable.NameImpl Java uses identity, which is equivalent
        // because indices are unique within a table.
        Rc::ptr_eq(&self.table, &other.table) && self.index == other.index
    }
}

impl Eq for Name {}

impl Hash for Name {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Java SharedNameTable.NameImpl.hashCode returns index; include the table
        // identity so that names from different tables do not collide silently.
        (Rc::as_ptr(&self.table) as usize).hash(state);
        self.index.hash(state);
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_java_string().to_string_lossy())
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::java_string::JavaString;

    fn table() -> Rc<NameTableHandle> {
        // Names sets the Weak<NamesStd> back-reference; the table works on its
        // own for these tests.
        crate::util::names::Names::new(false).table
    }

    #[test]
    fn interned_names_are_unique() {
        let t = table();
        let a = t.from_string(&JavaString::from("abc"));
        let b = t.from_string(&JavaString::from("abc"));
        assert_eq!(a, b);
        assert_eq!(a.get_byte_length(), 3);
        assert_eq!(a.to_java_string().to_string_lossy(), "abc");
    }

    #[test]
    fn append_and_sub_name() {
        let t = table();
        let a = t.from_string(&JavaString::from("java"));
        let b = t.from_string(&JavaString::from("lang"));
        let q = a.append(&b);
        assert_eq!(q.to_java_string().to_string_lossy(), "javalang");
        assert_eq!(q.sub_name(4, 8).to_java_string().to_string_lossy(), "lang");
        assert_eq!(q.last_index_of(b'a'), 5);
        assert!(q.starts_with(&a));
    }

    #[test]
    fn empty_name() {
        let t = table();
        let e = t.from_string(&JavaString::new());
        assert!(e.is_empty());
        assert_eq!(e.get_byte_length(), 0);
    }
}
