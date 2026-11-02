//! A hash table that maps Object to int.
//!
//! This is a custom hash table optimised for the Object -> int
//! maps. This is done to avoid unnecessary object allocation in the image set.
//!
//! @author Charles Turner
//! @author Per Bothner
//!
//! Java hashes keys with System.identityHashCode and compares them with the
//! reference equality operator; Rust uses K: Hash + Eq. Callers instantiate K with an
//! identity-preserving key type (an arena id or a pointer wrapper), which is exactly where javac
//! relies on object identity (JavacParser's end-position map is keyed by trees). Only the bucket
//! layout depends on the hash value and the table has no iteration order, so get/put/remove
//! semantics are unaffected.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const DEFAULT_INITIAL_SIZE: i32 = 64;

/// Java stores null in objs for an empty slot and the shared DELETED object for a
/// tombstone; both are modelled with an enum so that the lookup loop stays identical.
#[derive(Clone)]
enum Slot<K> {
    /// Java: objs[i] == null
    Empty,
    /// Java: objs[i] == DELETED
    Deleted,
    /// Java: objs[i] == key
    Key(K),
}

/// A hash table that maps Object to int.
pub struct IntHashTable<K> {
    /// Java: protected Object[] objs; // the domain set
    objs: Vec<Slot<K>>,
    /// Java: protected int[] ints; // the image set
    ints: Vec<i32>,
    /// Java: protected int mask; // used to clip int's into the domain
    mask: i32,
    /// Java: protected int num_bindings; // the number of mappings (including DELETED)
    num_bindings: i32,
}

impl<K: Hash + Eq> IntHashTable<K> {
    /// Construct an Object -> int hash table.
    ///
    /// The default size of the hash table is 64 mappings.
    ///
    /// Java: public IntHashTable()
    pub fn new() -> IntHashTable<K> {
        IntHashTable::with_capacity(DEFAULT_INITIAL_SIZE)
    }

    /// Construct an Object -> int hash table with a specified amount of mappings.
    /// @param capacity The number of default mappings in this hash table.
    ///
    /// Java: public IntHashTable(int capacity)
    pub fn with_capacity(capacity: i32) -> IntHashTable<K> {
        let mut log2_size: u32 = 4;
        // Java: while (capacity > (1 << log2Size)) log2Size++;
        // (int shift overflow follows Java semantics via wrapping_shl)
        while capacity > 1i32.wrapping_shl(log2_size) {
            log2_size += 1;
        }
        let capacity = 1i32.wrapping_shl(log2_size);
        IntHashTable {
            objs: (0..capacity as usize).map(|_| Slot::Empty).collect(),
            ints: vec![0; capacity as usize],
            mask: capacity - 1,
            num_bindings: 0,
        }
    }

    /// Compute the hash code of a given object.
    ///
    /// Java: protected int hash(Object key) { return System.identityHashCode(key); }
    fn hash(key: &K) -> i32 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as i32
    }

    /// Find either the index of a key's value, or the index of an available space.
    ///
    /// Java: protected int lookup(Object key)
    fn lookup(&self, key: &K) -> usize {
        let hash = Self::hash(key);
        // Java: int hash1 = hash ^ (hash >>> 15);
        let hash1 = hash ^ ((hash as u32 >> 15) as i32);
        // Java: int hash2 = (hash ^ (hash << 6)) | 1; //ensure coprimeness
        let hash2 = (hash ^ hash.wrapping_shl(6)) | 1;
        let mut deleted: i32 = -1;
        // Java: for (int i = hash1 & mask;; i = (i + hash2) & mask)
        let mut i = hash1 & self.mask;
        loop {
            match &self.objs[i as usize] {
                Slot::Key(k) if k == key => return i as usize,
                Slot::Empty => {
                    // Java: return deleted >= 0 ? deleted : i;
                    return if deleted >= 0 {
                        deleted as usize
                    } else {
                        i as usize
                    };
                }
                Slot::Deleted => {
                    if deleted < 0 {
                        deleted = i;
                    }
                }
                Slot::Key(_) => {}
            }
            i = i.wrapping_add(hash2) & self.mask;
        }
    }

    /// Return the value to which the specified key is mapped.
    ///
    /// @param key The key to whose value you want to find.
    /// @return A non-negative integer if the value is found.
    ///         Otherwise, it is -1.
    ///
    /// Java: public int get(Object key)
    pub fn get(&self, key: &K) -> i32 {
        let index = self.lookup(key);
        match &self.objs[index] {
            Slot::Empty | Slot::Deleted => -1,
            Slot::Key(_) => self.ints[index],
        }
    }

    /// Associates the specified key with the specified value in this map.
    ///
    /// @param key key with which the specified value is to be associated.
    /// @param value value to be associated with the specified key.
    /// @return previous value associated with specified key, or -1 if there was
    /// no mapping for key.
    ///
    /// Java: public int put(Object key, int value)
    pub fn put(&mut self, key: K, value: i32) -> i32 {
        let index = self.lookup(&key);
        let old_is_empty = matches!(&self.objs[index], Slot::Empty);
        let old_is_deleted = matches!(&self.objs[index], Slot::Deleted);
        if old_is_empty || old_is_deleted {
            self.objs[index] = Slot::Key(key);
            self.ints[index] = value;
            // Java: if (old != DELETED) num_bindings++;
            if !old_is_deleted {
                self.num_bindings += 1;
            }
            // Java: if (3 * num_bindings >= 2 * objs.length) rehash();
            if 3 * self.num_bindings >= 2 * self.objs.len() as i32 {
                self.rehash();
            }
            -1
        } else {
            // update existing mapping
            let old_value = self.ints[index];
            self.ints[index] = value;
            old_value
        }
    }

    /// Remove the mapping(key and value) of the specified key.
    ///
    /// @param key the key to whose value you want to remove.
    /// @return the removed value associated with the specified key,
    ///         or -1 if there was no mapping for the specified key.
    ///
    /// Java: public int remove(Object key)
    pub fn remove(&mut self, key: &K) -> i32 {
        let index = self.lookup(key);
        let old_is_empty = matches!(&self.objs[index], Slot::Empty);
        let old_is_deleted = matches!(&self.objs[index], Slot::Deleted);
        if old_is_empty || old_is_deleted {
            return -1;
        }
        self.objs[index] = Slot::Deleted;
        self.ints[index]
    }

    /// Expand the hash table when it exceeds the load factor.
    ///
    /// Rehash the existing objects.
    ///
    /// Java: protected void rehash()
    fn rehash(&mut self) {
        // Java: Object[] oldObjsTable = objs; int[] oldIntsTable = ints;
        let old_objs_table = std::mem::take(&mut self.objs);
        let old_ints_table = std::mem::take(&mut self.ints);
        let new_capacity = old_objs_table.len() << 1;
        self.objs = (0..new_capacity).map(|_| Slot::Empty).collect();
        self.ints = vec![0; new_capacity];
        self.mask = new_capacity as i32 - 1;
        self.num_bindings = 0; // this is recomputed below
        // Java: for (int i = oldIntsTable.length; --i >= 0;) {
        // key = oldObjsTable[i];
        // if (key != null && key != DELETED) put(key, oldIntsTable[i]);
        // }
        for (i, slot) in old_objs_table.into_iter().enumerate().rev() {
            if let Slot::Key(key) = slot {
                self.put(key, old_ints_table[i]);
            }
        }
    }

    /// Removes all mappings from this map.
    ///
    /// Java: public void clear()
    pub fn clear(&mut self) {
        // Java: for (int i = objs.length; --i >= 0;) objs[i] = null;
        for slot in self.objs.iter_mut().rev() {
            *slot = Slot::Empty;
        }
        self.num_bindings = 0;
    }
}

impl<K: Hash + Eq> Default for IntHashTable<K> {
    fn default() -> Self {
        IntHashTable::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_update() {
        let mut table: IntHashTable<i32> = IntHashTable::new();
        assert_eq!(table.get(&1), -1);
        assert_eq!(table.put(1, 10), -1);
        assert_eq!(table.get(&1), 10);
        assert_eq!(table.put(1, 20), 10);
        assert_eq!(table.get(&1), 20);
        assert_eq!(table.get(&2), -1);
    }

    #[test]
    fn remove_then_get() {
        let mut table: IntHashTable<i32> = IntHashTable::new();
        table.put(1, 10);
        table.put(2, 20);
        assert_eq!(table.remove(&1), 10);
        assert_eq!(table.get(&1), -1);
        assert_eq!(table.get(&2), 20);
        assert_eq!(table.remove(&1), -1);
        // a removed slot is reused by a later put
        assert_eq!(table.put(3, 30), -1);
        assert_eq!(table.get(&3), 30);
    }

    #[test]
    fn grows_past_load_factor() {
        let mut table: IntHashTable<i32> = IntHashTable::new();
        for i in 0..100 {
            assert_eq!(table.put(i, i * 3), -1);
        }
        for i in 0..100 {
            assert_eq!(table.get(&i), i * 3, "key {}", i);
        }
        assert!(table.objs.len() >= 128);
    }

    #[test]
    fn with_capacity_rounds_up_to_power_of_two() {
        let table: IntHashTable<i32> = IntHashTable::with_capacity(10);
        assert_eq!(table.objs.len(), 16);
        assert_eq!(table.mask, 15);
        let small: IntHashTable<i32> = IntHashTable::with_capacity(1);
        assert_eq!(small.objs.len(), 16);
        assert_eq!(small.mask, 15);
    }

    #[test]
    fn clear_removes_all_mappings() {
        let mut table: IntHashTable<i32> = IntHashTable::new();
        table.put(1, 10);
        table.put(2, 20);
        table.clear();
        assert_eq!(table.get(&1), -1);
        assert_eq!(table.get(&2), -1);
        table.put(3, 30);
        assert_eq!(table.get(&3), 30);
    }

    #[test]
    fn string_keys() {
        let mut table: IntHashTable<String> = IntHashTable::new();
        table.put("a".to_string(), 1);
        table.put("b".to_string(), 2);
        assert_eq!(table.get(&"a".to_string()), 1);
        assert_eq!(table.get(&"b".to_string()), 2);
        assert_eq!(table.get(&"c".to_string()), -1);
    }
}
