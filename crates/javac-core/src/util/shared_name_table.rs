// Origin: com/sun/tools/javac/util/SharedNameTable.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2/P3 utility-service.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getIndex (SharedNameTable.java:171)
// - NOT PORTED: getByteLength (SharedNameTable.java:176)
// - NOT PORTED: getByteAt (SharedNameTable.java:181)
// - NOT PORTED: getByteOffset (SharedNameTable.java:191)
// - NOT PORTED: hashCode (SharedNameTable.java:198)
// End of Gap registration.

//! Implementation of Name.Table that stores all names in a single shared byte
//! array, expanding it as needed. This avoids the overhead incurred by using an
//! array of bytes for each name.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the static SoftReference freelist from the Java
//! implementation is a memory optimization with no observable effect; Rust
//! creates a fresh table (registered deviation, see docs/STATUS.md).

use std::rc::Rc;

use crate::util::array_utils;
use crate::util::convert;
use crate::util::name::NameTableHandle;

struct SharedNameNode {
    /// Java: NameImpl next (next name occupying the same hash bucket) — node index.
    next: Option<u32>,
    /// The index where the bytes of this name are stored in the global name buffer.
    index: i32,
    /// The number of bytes in this name.
    length: i32,
}

/// Implementation of NameTable that stores all names in a single shared byte array.
pub struct SharedNameTable {
    /// The hash table for names.
    hashes: Vec<Option<u32>>,
    /// The name nodes (Java NameImpl objects).
    nodes: Vec<SharedNameNode>,
    /// The shared byte array holding all encountered names.
    pub bytes: Vec<u8>,
    /// The mask to be used for hashing.
    hash_mask: i32,
    /// The number of filled bytes in the shared name buffer.
    nc: i32,
}

impl SharedNameTable {
    /// Java: public SharedNameTable(Names names, int hashSize, int nameSize)
    pub fn new(hash_size: i32, name_size: i32) -> SharedNameTable {
        SharedNameTable {
            hash_mask: hash_size - 1,
            hashes: vec![None; hash_size as usize],
            nodes: Vec::new(),
            bytes: vec![0u8; name_size as usize],
            nc: 0,
        }
    }

    /// Java: public Name fromChars(char[] cs, int start, int len)
    pub fn from_chars(&mut self, cs: &[u16], start: i32, len: i32) -> (i32, Rc<[u8]>) {
        let nc = self.nc;
        self.bytes = array_utils::ensure_capacity_bytes(std::mem::take(&mut self.bytes), nc + len * 3);
        let nbytes = convert::chars2utf_into(cs, start, &mut self.bytes, nc, len) - nc;
        let h = (NameTableHandle::hash_value(&self.bytes, nc, nbytes) & self.hash_mask) as usize;
        let mut cur = self.hashes[h];
        while let Some(node_idx) = cur {
            let node = &self.nodes[node_idx as usize];
            if node.length == nbytes
                && NameTableHandle::equals(&self.bytes, node.index, &self.bytes, nc, nbytes)
            {
                return snapshot(node.index, nbytes, &self.bytes);
            }
            cur = node.next;
        }
        let index = nc;
        self.nodes.push(SharedNameNode {
            next: self.hashes[h],
            index,
            length: nbytes,
        });
        self.hashes[h] = Some((self.nodes.len() - 1) as u32);
        self.nc = nc + nbytes;
        if nbytes == 0 {
            self.nc += 1;
        }
        snapshot(index, nbytes, &self.bytes)
    }

    /// Java: public Name fromUtf(byte[] cs, int start, int len)
    pub fn from_utf(&mut self, cs: &[u8], start: i32, len: i32) -> (i32, Rc<[u8]>) {
        let h = (NameTableHandle::hash_value(cs, start, len) & self.hash_mask) as usize;
        let mut cur = self.hashes[h];
        while let Some(node_idx) = cur {
            let node = &self.nodes[node_idx as usize];
            if node.length == len
                && NameTableHandle::equals(&self.bytes, node.index, cs, start, len)
            {
                return snapshot(node.index, len, &self.bytes);
            }
            cur = node.next;
        }
        let nc = self.nc;
        self.bytes = array_utils::ensure_capacity_bytes(std::mem::take(&mut self.bytes), nc + len);
        self.bytes[nc as usize..(nc + len) as usize].copy_from_slice(&cs[start as usize..(start + len) as usize]);
        self.nodes.push(SharedNameNode {
            next: self.hashes[h],
            index: nc,
            length: len,
        });
        self.hashes[h] = Some((self.nodes.len() - 1) as u32);
        self.nc = nc + len;
        if len == 0 {
            self.nc += 1;
        }
        snapshot(nc, len, &self.bytes)
    }

    pub fn dispose(&mut self) {
        // Java: dispose(this) returns the table to the static freelist.
        self.hashes.clear();
        self.nodes.clear();
    }
}

fn snapshot(index: i32, length: i32, bytes: &[u8]) -> (i32, Rc<[u8]>) {
    let start = index as usize;
    let end = start + length as usize;
    (index, Rc::from(&bytes[start..end]))
}
