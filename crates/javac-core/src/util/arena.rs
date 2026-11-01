//! Generic arena container: the ADR 0001 ID/arena infrastructure.
//!
//! Arena<T> owns the elements, which are only reachable through arena indices; arenas only grow (ADR 0001 rule 8),
//! and are released with CompilerContext when compilation ends. This module only provides the container, not compiler entities;
//! CompilerContext (later task) holds the Symbol/Type/Tree/Scope arenas per ADR 0001.
//!
//! PORTING_RULES: non-1:1 adapter file (Java mutable object graph -> arena + ID), see ADR 0001.

/// A grow-only arena: push returns the new element index; callers build the matching ID from it.
///
/// No Rc/RefCell: ownership belongs to the arena and access goes through IDs (ADR 0001 rule 4).
#[derive(Debug, Clone)]
pub struct Arena<T> {
    values: Vec<T>,
}

impl<T> Arena<T> {
    /// Create an empty arena.
    pub const fn new() -> Arena<T> {
        Arena { values: Vec::new() }
    }

    /// Append an element and return its arena index (the caller wraps it with the matching ID type new(index)).
    pub fn push(&mut self, value: T) -> u32 {
        let index = self.values.len();
        assert!(
            index <= u32::MAX as usize,
            "arena size exceeds the u32 index space"
        );
        self.values.push(value);
        index as u32
    }

    /// Read an element by index; out-of-bounds is an internal error, asserted in debug builds (ADR 0001 rule 8).
    pub fn get(&self, index: u32) -> &T {
        debug_assert!(
            (index as usize) < self.values.len(),
            "arena index out of range: {index} (len {})",
            self.values.len()
        );
        // SAFETY: debug builds assert the range above; in release the ID/arena
        // contract (ADR 0001 rule 8) guarantees the index came from this arena,
        // and an out-of-range index is an internal error, not user input.
        unsafe { self.values.get_unchecked(index as usize) }
    }

    /// Read an element mutably by index; out-of-bounds is an internal error, asserted in debug builds (ADR 0001 rule 8).
    pub fn get_mut(&mut self, index: u32) -> &mut T {
        debug_assert!(
            (index as usize) < self.values.len(),
            "arena index out of range: {index} (len {})",
            self.values.len()
        );
        // SAFETY: see Arena::get.
        unsafe { self.values.get_unchecked_mut(index as usize) }
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the arena is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterate over all elements in index order.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.values.iter()
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Arena<T> {
        Arena::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Item {
        value: u32,
    }

    #[test]
    fn push_get_and_metadata() {
        let mut arena = Arena::new();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
        let first = arena.push(Item { value: 10 });
        let second = arena.push(Item { value: 20 });
        assert_eq!((first, second), (0, 1));
        assert_eq!(arena.len(), 2);
        assert!(!arena.is_empty());
        assert_eq!(arena.get(first), &Item { value: 10 });
        assert_eq!(arena.get(second), &Item { value: 20 });
    }

    #[test]
    fn get_mut_updates_in_place() {
        let mut arena = Arena::new();
        let index = arena.push(Item { value: 1 });
        arena.get_mut(index).value = 99;
        assert_eq!(arena.get(index).value, 99);
    }

    #[test]
    fn iter_returns_elements_in_index_order() {
        let mut arena = Arena::new();
        for value in [3u32, 1, 2] {
            arena.push(Item { value });
        }
        let values: Vec<u32> = arena.iter().map(|item| item.value).collect();
        assert_eq!(values, vec![3, 1, 2]);
    }

    #[test]
    fn default_matches_new() {
        let arena: Arena<Item> = Arena::default();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
    }

    #[test]
    #[should_panic(expected = "arena index out of range")]
    fn get_out_of_range_panics() {
        let arena: Arena<Item> = Arena::new();
        let _ = arena.get(0);
    }

    #[test]
    #[should_panic(expected = "arena index out of range")]
    fn get_mut_out_of_range_panics() {
        let mut arena: Arena<Item> = Arena::new();
        let _ = arena.get_mut(0);
    }
}
