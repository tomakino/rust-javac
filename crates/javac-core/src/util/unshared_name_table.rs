// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2/P3 utility-service.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: create (UnsharedNameTable.java:41)
// - NOT PORTED: getIndex (UnsharedNameTable.java:153)
// - NOT PORTED: getByteLength (UnsharedNameTable.java:158)
// - NOT PORTED: getByteAt (UnsharedNameTable.java:163)
// - NOT PORTED: getByteOffset (UnsharedNameTable.java:173)
// End of Gap registration.

//! Implementation of Name.Table that stores names in individual arrays.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! the Java implementation keeps weak references to entries so
//! that unreachable names can be collected; Rust ownership already bounds the
//! lifetime of name table entries, so entries are kept until dispose(). This is
//! a memory-management deviation with no observable compiler behavior.

use std::rc::Rc;

use crate::util::convert;
use crate::util::name::NameTableHandle;

struct UnsharedNameNode {
    /// Java: HashEntry.next — node index of the next entry in this bucket.
    next: Option<u32>,
    bytes: Vec<u8>,
}

/// Implementation of NameTable that stores names in individual arrays.
pub struct UnsharedNameTable {
    /// The hash table for names.
    hashes: Vec<Option<u32>>,
    nodes: Vec<UnsharedNameNode>,
    /// Index counter for names in this table.
    pub index: i32,
}

impl UnsharedNameTable {
    /// Java: public UnsharedNameTable(Names names, int hashSize)
    pub fn new(hash_size: i32) -> UnsharedNameTable {
        UnsharedNameTable {
            hashes: vec![None; (hash_size - 1 + 1) as usize],
            nodes: Vec::new(),
            index: 0,
        }
    }

    /// Java: public Name fromChars(char[] cs, int start, int len)
    pub fn from_chars(&mut self, cs: &[u16], start: i32, len: i32) -> (i32, i32) {
        let mut name = vec![0u8; (len * 3) as usize];
        let nbytes = convert::chars2utf_into(cs, start, &mut name, 0, len);
        self.from_utf(&name, 0, nbytes)
    }

    /// Java: public Name fromUtf(byte[] cs, int start, int len)
    pub fn from_utf(&mut self, cs: &[u8], start: i32, len: i32) -> (i32, i32) {
        let h = (NameTableHandle::hash_value(cs, start, len) & (self.hashes.len() as i32 - 1)) as usize;
        let mut cur = self.hashes[h];
        while let Some(node_idx) = cur {
            let node = &self.nodes[node_idx as usize];
            if node.bytes.len() == len as usize
                && NameTableHandle::equals(&node.bytes, 0, cs, start, len)
            {
                return (node_idx as i32, len);
            }
            cur = node.next;
        }
        let bytes = cs[start as usize..(start + len) as usize].to_vec();
        let index = self.index;
        self.nodes.push(UnsharedNameNode {
            next: self.hashes[h],
            bytes,
        });
        self.hashes[h] = Some((self.nodes.len() - 1) as u32);
        self.index += 1;
        (index, len)
    }

    /// Java: the name bytes are read through the shared table entry.
    pub(crate) fn bytes_at(&self, index: i32, len: i32) -> &[u8] {
        &self.nodes[index as usize].bytes[..len as usize]
    }

    pub fn dispose(&mut self) {
        self.hashes.clear();
        self.nodes.clear();
    }
}
