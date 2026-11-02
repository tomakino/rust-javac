//! Array growth utilities.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! Java overloads map to distinct Rust functions:
//! - ensureCapacity(byte[]) -> ensure_capacity_bytes
//! - ensureCapacity(char[]) -> ensure_capacity_u16
//! - ensureCapacity(int[])  -> ensure_capacity_i32
//! - ensureCapacity(T[])    -> ensure_capacity_refs (elements are Vec<Option<T>>, i.e. null)

/// Java: private static int calculateNewLength(int currentLength, int maxIndex)
fn calculate_new_length(current_length: i32, max_index: i32) -> i32 {
    let mut current_length = current_length;
    // Java int arithmetic wraps; Rust must wrap as well to stay identical.
    while current_length < max_index.wrapping_add(1) {
        current_length = current_length.wrapping_mul(2);
    }
    current_length
}

/// Java: public static byte[] ensureCapacity(byte[] array, int maxIndex)
pub fn ensure_capacity_bytes(array: Vec<u8>, max_index: i32) -> Vec<u8> {
    if max_index < array.len() as i32 {
        return array;
    }
    let new_length = calculate_new_length(array.len() as i32, max_index);
    let mut result = vec![0u8; new_length as usize];
    result[..array.len()].copy_from_slice(&array);
    result
}

/// Java: public static char[] ensureCapacity(char[] array, int maxIndex)
pub fn ensure_capacity_u16(array: Vec<u16>, max_index: i32) -> Vec<u16> {
    if max_index < array.len() as i32 {
        return array;
    }
    let new_length = calculate_new_length(array.len() as i32, max_index);
    let mut result = vec![0u16; new_length as usize];
    result[..array.len()].copy_from_slice(&array);
    result
}

/// Java: public static int[] ensureCapacity(int[] array, int maxIndex)
pub fn ensure_capacity_i32(array: Vec<i32>, max_index: i32) -> Vec<i32> {
    if max_index < array.len() as i32 {
        return array;
    }
    let new_length = calculate_new_length(array.len() as i32, max_index);
    let mut result = vec![0i32; new_length as usize];
    result[..array.len()].copy_from_slice(&array);
    result
}

/// Java: public static <T> T[] ensureCapacity(T[] array, int maxIndex)
///
/// Rust has no null, so a Java reference array is represented as Vec<Option<T>>.
pub fn ensure_capacity_refs<T: Clone>(array: Vec<Option<T>>, max_index: i32) -> Vec<Option<T>> {
    if max_index < array.len() as i32 {
        return array;
    }
    let new_length = calculate_new_length(array.len() as i32, max_index);
    let mut result: Vec<Option<T>> = Vec::with_capacity(new_length as usize);
    result.resize_with(new_length as usize, || None);
    result[..array.len()].clone_from_slice(&array);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_and_copies() {
        let a = ensure_capacity_bytes(vec![1, 2], 2);
        assert_eq!(a.len(), 4);
        assert_eq!(a[..2], [1, 2]);
        assert_eq!(a[2..], [0, 0]);
    }

    #[test]
    fn keeps_existing_when_big_enough() {
        let a = ensure_capacity_i32(vec![1, 2, 3], 1);
        assert_eq!(a, vec![1, 2, 3]);
    }

    #[test]
    fn refs_grow_with_none() {
        let a: Vec<Option<u8>> = vec![Some(1)];
        let a = ensure_capacity_refs(a, 1);
        assert_eq!(a, vec![Some(1), None]);
    }
}
