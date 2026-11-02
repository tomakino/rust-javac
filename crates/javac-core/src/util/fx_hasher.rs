//! A fast, non-cryptographic hasher for compiler-internal maps keyed by
//! `Name`.
//!
//! performance adapter (not a 1:1 port). Java uses
//! `Name.hashCode()` (the name-table index) in its HashMaps; the port keeps
//! the same key/lookup semantics but avoids the default SipHash cost for
//! the high-frequency Name-keyed maps. Map behavior is unchanged.

use std::hash::{BuildHasherDefault, Hasher};

use crate::util::name::Name;

/// Java: Name.hashCode() — cheap, non-cryptographic hash.
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.hash
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.hash = (self.hash.rotate_left(5) ^ u64::from(byte))
                .wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.hash = (self.hash.rotate_left(5) ^ u64::from(value))
            .wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    fn write_u64(&mut self, value: u64) {
        self.hash = (self.hash.rotate_left(5) ^ value)
            .wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
}

pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

/// `HashMap<Name, V>` backed by [FxHasher].
pub type FxNameMap<V> = std::collections::HashMap<Name, V, FxBuildHasher>;

/// Build an [FxNameMap] with room for `capacity` entries (avoids the
/// repeated rehash of the small hot maps).
pub fn fx_map_with_capacity<V>(capacity: usize) -> FxNameMap<V> {
    std::collections::HashMap::with_capacity_and_hasher(capacity, FxBuildHasher::default())
}
