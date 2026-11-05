//! Grow-only arena storage addressed by compact indices.
//!
//! `Arena<T>` owns its values and returns a `u32` index from [`push`](Arena::push);
//! callers wrap that index in a domain ID. Elements are never removed, so an
//! index remains valid for the arena's lifetime.

/// A grow-only arena whose elements are reached through indices.
///
/// The arena owns every value; callers never hold references into it across
/// mutations, and indices are the only stable handles.
#[derive(Debug, Clone)]
pub struct Arena<T> {
    values: Vec<T>,
}

impl<T> Arena<T> {
    /// Creates an empty arena.
    pub const fn new() -> Arena<T> {
        Arena { values: Vec::new() }
    }

    /// Appends `value` and returns the index that identifies it.
    pub fn push(&mut self, value: T) -> u32 {
        let index = self.values.len();
        assert!(
            index <= u32::MAX as usize,
            "arena size exceeds the u32 index space"
        );
        self.values.push(value);
        index as u32
    }

    /// Returns the element at `index`.
    ///
    /// # Panics
    ///
    /// Panics when `index` is outside the arena. An out-of-range index means
    /// the caller broke the arena/ID contract; it is an internal error, so the
    /// bounds check is kept in every profile.
    #[inline]
    pub fn get(&self, index: u32) -> &T {
        &self.values[index as usize]
    }

    /// Returns the element at `index` mutably.
    ///
    /// # Panics
    ///
    /// Panics when `index` is outside the arena, for the same reason as
    /// [`get`](Arena::get).
    #[inline]
    pub fn get_mut(&mut self, index: u32) -> &mut T {
        &mut self.values[index as usize]
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether the arena is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterates over the elements in index order.
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
}
