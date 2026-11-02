//! Immutable singly linked lists.
//!
//! [`List`] is a shared chain of nodes and [`ListBuffer`] builds one
//! incrementally. Equality compares contents, while [`List::ptr_eq`] compares
//! node identity. Tails are spliced only while a list is under construction.

use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::rc::Rc;

use super::assert;
use super::list_buffer::ListBuffer;

/// A node of a singly linked list.
struct ListNode<A> {
    /// The first element.
    head: A,

    /// The rest of the list; `None` is the sentinel that ends every list.
    ///
    /// The cell lets a freshly built node be spliced before another list can
    /// observe it.
    tail: Cell<Option<Rc<ListNode<A>>>>,
}

impl<A> Drop for ListNode<A> {
    /// Releases the tail chain iteratively. The default Rc drop would recurse
    /// through every node, which is O(n) stack depth and dominates the profile
    /// when long temporary lists are released.
    fn drop(&mut self) {
        let mut next = self.tail.take();
        while let Some(rc) = next {
            match Rc::try_unwrap(rc) {
                Ok(node) => next = node.tail.take(),
                Err(_) => break,
            }
        }
    }
}

impl<A> ListNode<A> {
    /// Returns a shared handle to the tail, or `None` at the sentinel.
    ///
    /// A splice replaces the cell contents, so reading the tail safely takes it
    /// out, clones the handle, and restores it: two cell writes and one
    /// refcount bump per step. Walkers therefore own the cursor rather than
    /// borrowing through the cell.
    #[inline]
    fn tail(&self) -> Option<Rc<ListNode<A>>> {
        let tail = self.tail.replace(None);
        self.tail.set(tail.clone());
        tail
    }

    /// Replaces this node's tail, splicing a chain while a list is built.
    #[inline]
    fn set_tail(&self, tail: Option<Rc<ListNode<A>>>) {
        self.tail.set(tail);
    }
}

/// A singly linked list.
pub struct List<A> {
    /// `None` is the shared sentinel that ends every list.
    node: Option<Rc<ListNode<A>>>,
}

impl<A> List<A> {
    /// Returns the empty list.
    pub fn nil() -> List<A> {
        List { node: None }
    }

    /// Returns a list whose head is `head` and whose tail is `tail`.
    fn cons(head: A, tail: &List<A>) -> List<A> {
        List {
            node: Some(Rc::new(ListNode {
                head,
                tail: Cell::new(tail.node.clone()),
            })),
        }
    }

    /// Returns whether both lists refer to the same node.
    #[inline]
    pub fn ptr_eq(&self, other: &List<A>) -> bool {
        match (&self.node, &other.node) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// Returns a copy of `l` without the elements equal to `elem`.
    pub fn filter(l: &List<A>, elem: &A) -> List<A>
    where
        A: Clone + PartialEq,
    {
        // Comparisons use `PartialEq`; a null element is represented by
        // `Option` and compares structurally.
        // Appending builds the result in one pass instead of prepending and
        // reversing.
        let mut buf: ListBuffer<A> = ListBuffer::new();
        for a in l.iter() {
            if &a != elem {
                buf.append(a);
            }
        }
        buf.to_list()
    }

    /// Returns the elements of this list that also occur in `that`.
    pub fn intersect(&self, that: &List<A>) -> List<A>
    where
        A: Clone + PartialEq,
    {
        let mut buf: ListBuffer<A> = ListBuffer::new();
        for el in self.iter() {
            if that.contains(&el) {
                buf.append(el);
            }
        }
        buf.to_list()
    }

    /// Returns the elements of this list that do not occur in `that`.
    pub fn diff(&self, that: &List<A>) -> List<A>
    where
        A: Clone + PartialEq,
    {
        let mut buf: ListBuffer<A> = ListBuffer::new();
        for el in self.iter() {
            if !that.contains(&el) {
                buf.append(el);
            }
        }
        buf.to_list()
    }

    /// Returns at most the first `n` elements.
    pub fn take(&self, n: i32) -> List<A>
    where
        A: Clone,
    {
        let mut buf: ListBuffer<A> = ListBuffer::new();
        // A negative `n` matches no count, so every element is copied.
        for (count, el) in self.iter().enumerate() {
            if count == n as usize {
                break;
            }
            buf.append(el);
        }
        buf.to_list()
    }

    /// Returns a one-element list.
    pub fn of(x1: A) -> List<A> {
        List::cons(x1, &List::nil())
    }

    /// Returns a two-element list.
    pub fn of2(x1: A, x2: A) -> List<A> {
        List::of(x2).prepend(x1)
    }

    /// Returns a three-element list.
    pub fn of3(x1: A, x2: A, x3: A) -> List<A> {
        List::of2(x2, x3).prepend(x1)
    }

    /// Returns a four-element list followed by `rest`.
    pub fn of4(x1: A, x2: A, x3: A, rest: Vec<A>) -> List<A> {
        List::from_array(Some(rest))
            .prepend(x3)
            .prepend(x2)
            .prepend(x1)
    }

    /// Builds a list from an optional array; `None` means the empty list.
    pub fn from_array(array: Option<Vec<A>>) -> List<A> {
        let mut xs = List::nil();
        if let Some(array) = array {
            for a in array.into_iter().rev() {
                xs = List::cons(a, &xs);
            }
        }
        xs
    }

    /// Builds a list by appending every item of `coll` in order.
    pub fn from_iterable(coll: impl IntoIterator<Item = A>) -> List<A>
    where
        A: Clone,
    {
        let mut xs: ListBuffer<A> = ListBuffer::new();
        for a in coll {
            xs.append(a);
        }
        xs.to_list()
    }

    /// Returns a list of `len` copies of `init`.
    pub fn fill(len: i32, init: A) -> List<A>
    where
        A: Clone,
    {
        let mut l = List::nil();
        let mut i = 0;
        while i < len {
            l = List::cons(init.clone(), &l);
            i += 1;
        }
        l
    }

    /// Returns whether the list has no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.node.is_none()
    }

    /// Returns whether the list has at least one element.
    #[inline]
    pub fn non_empty(&self) -> bool {
        self.node.is_some()
    }

    /// Returns the number of elements.
    pub fn length(&self) -> i32 {
        let mut len = 0;
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            len += 1;
            cur = node.tail();
        }
        len
    }

    /// Returns the number of elements; an alias for [`length`](Self::length).
    pub fn size(&self) -> i32 {
        self.length()
    }

    /// Replaces the tail of this list with `tail` and returns `tail`.
    ///
    /// # Panics
    ///
    /// Panics when this list is the empty sentinel, which has no node to splice.
    pub fn set_tail(&mut self, tail: List<A>) -> List<A> {
        match &self.node {
            Some(node) => node.set_tail(tail.node.clone()),
            // The sentinel has no node whose tail can be replaced.
            None => panic!("java.util.UnsupportedOperationException"),
        }
        tail
    }

    /// Returns the first element.
    ///
    /// # Panics
    ///
    /// Panics on the empty list.
    #[inline]
    pub fn head(&self) -> &A {
        match &self.node {
            Some(node) => &node.head,
            None => panic!("java.lang.NullPointerException"),
        }
    }

    /// Returns the list after its first element.
    #[inline]
    pub fn tail(&self) -> List<A> {
        match &self.node {
            Some(node) => List { node: node.tail() },
            None => List { node: None },
        }
    }

    /// Returns a new list with `x` in front.
    #[inline]
    pub fn prepend(&self, x: A) -> List<A> {
        List::cons(x, self)
    }

    /// Returns this list preceded by the elements of `xs`.
    pub fn prepend_list(&self, xs: &List<A>) -> List<A>
    where
        A: Clone,
    {
        if self.is_empty() {
            return xs.clone();
        }
        if xs.is_empty() {
            return self.clone();
        }
        if xs.tail().is_empty() {
            return self.prepend(xs.head().clone());
        }
        // return this.prependList(xs.tail).prepend(xs.head);
        let mut result = self.clone();
        let mut rev = xs.reverse();
        // `reverse` always allocates fresh nodes, so the identity check holds;
        // the loop below reuses those nodes for the splice.
        assert::check(!rev.ptr_eq(xs));
        // `reverse` allocated fresh nodes, so they can be reused instead of
        // allocated again.
        while rev.non_empty() {
            let mut h = rev;
            rev = h.tail();
            h.set_tail(result);
            result = h;
        }
        result
    }

    /// Returns a new list with the elements in reverse order.
    ///
    /// An empty or singleton list is returned unchanged.
    pub fn reverse(&self) -> List<A>
    where
        A: Clone,
    {
        // if it is empty or a singleton, return itself
        if self.is_empty() || self.tail().is_empty() {
            return self.clone();
        }

        // Each step owns the next node, so the source chain stays alive while
        // the reversed chain is built.
        let mut rev = List::nil();
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            rev = List::cons(node.head.clone(), &rev);
            cur = node.tail();
        }
        rev
    }

    /// Returns a new list with `x` at the end.
    pub fn append(&self, x: A) -> List<A>
    where
        A: Clone,
    {
        List::of(x).prepend_list(self)
    }

    /// Returns a new list with the elements of `x` at the end.
    pub fn append_list(&self, x: &List<A>) -> List<A>
    where
        A: Clone,
    {
        x.prepend_list(self)
    }

    /// Returns a new list with the elements of the buffer at the end.
    pub fn append_list_buffer(&self, x: &ListBuffer<A>) -> List<A>
    where
        A: Clone,
    {
        self.append_list(&x.to_list())
    }

    /// Copies the elements into a new vector.
    pub fn to_vec(&self) -> Vec<A>
    where
        A: Clone,
    {
        let mut result = Vec::new();
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            result.push(node.head.clone());
            cur = node.tail();
        }
        result
    }

    /// Formats the elements separated by `sep`.
    pub fn to_string_sep(&self, sep: &str) -> String
    where
        A: fmt::Display,
    {
        let Some(first) = self.node.clone() else {
            return String::new();
        };
        use std::fmt::Write;
        let mut buf = String::new();
        let _ = write!(buf, "{}", first.head);
        let mut cur = first.tail();
        while let Some(node) = cur {
            buf.push_str(sep);
            let _ = write!(buf, "{}", node.head);
            cur = node.tail();
        }
        buf
    }

    /// Returns whether two lists have equal elements.
    pub fn equals(xs: &List<A>, ys: &List<A>) -> bool
    where
        A: PartialEq,
    {
        let mut x = xs.node.clone();
        let mut y = ys.node.clone();
        loop {
            match (x.take(), y.take()) {
                (None, None) => return true,
                (Some(xn), Some(yn)) => {
                    if xn.head != yn.head {
                        return false;
                    }
                    x = xn.tail();
                    y = yn.tail();
                }
                _ => return false,
            }
        }
    }

    /// Returns whether the list contains `x`.
    pub fn contains(&self, x: &A) -> bool
    where
        A: PartialEq,
    {
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            if &node.head == x {
                return true;
            }
            cur = node.tail();
        }
        false
    }

    /// Returns the last element, or `None` when the list is empty.
    pub fn last(&self) -> Option<A>
    where
        A: Clone,
    {
        let mut last = None;
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            last = Some(node.head.clone());
            cur = node.tail();
        }
        last
    }

    /// Returns a list with `mapper` applied to every element.
    pub fn map<Z, F>(&self, mut mapper: F) -> List<Z>
    where
        A: Clone,
        Z: Clone,
        F: FnMut(&A) -> Z,
    {
        // A fresh list is always built. When the mapper is the identity the
        // contents are unchanged, but comparing the input and output types is
        // not possible in Rust; only list reference identity could differ.
        let mut buf: ListBuffer<Z> = ListBuffer::new();
        for a in self.iter() {
            let z = mapper(&a);
            buf.append(z);
        }
        buf.to_list()
    }

    // Reflective element-type conversion has no Rust counterpart: the element
    // type is checked statically, so no runtime cast is needed.

    /// Returns the element at `index`.
    ///
    /// # Panics
    ///
    /// Panics when `index` is negative or beyond the end of the list.
    pub fn get(&self, index: i32) -> A
    where
        A: Clone,
    {
        if index < 0 {
            panic!("java.lang.IndexOutOfBoundsException: {}", index);
        }

        let mut cur = self.node.clone();
        let mut i = index;
        while i > 0 {
            match cur.take() {
                Some(node) => {
                    cur = node.tail();
                    i -= 1;
                }
                None => break,
            }
        }

        if cur.is_none() {
            panic!(
                "java.lang.IndexOutOfBoundsException: Index: {}, Size: {}",
                index,
                self.size()
            );
        }
        cur.expect("get checked non-empty").head.clone()
    }

    /// Returns the index of the first occurrence of `o`, or `-1`.
    pub fn index_of(&self, o: &A) -> i32
    where
        A: PartialEq,
    {
        let mut i = 0;
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            if &node.head == o {
                return i;
            }
            cur = node.tail();
            i += 1;
        }
        -1
    }

    /// Returns the index of the last occurrence of `o`, or `-1`.
    pub fn last_index_of(&self, o: &A) -> i32
    where
        A: PartialEq,
    {
        let mut last = -1;
        let mut i = 0;
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            if &node.head == o {
                last = i;
            }
            cur = node.tail();
            i += 1;
        }
        last
    }

    /// Returns an iterator over the elements.
    pub fn iter(&self) -> ListIter<'_, A> {
        ListIter {
            cur: self.node.clone(),
            _borrow: PhantomData,
        }
    }
}

impl<A> Clone for List<A> {
    fn clone(&self) -> Self {
        List {
            node: self.node.clone(),
        }
    }
}

impl<A> Default for List<A> {
    fn default() -> Self {
        List::nil()
    }
}

/// An iterator over a shared list.
///
/// The cursor owns a handle to the current node, so it never borrows through
/// the mutable tail cell; the lifetime keeps the list immutably borrowed for
/// the iterator's duration.
pub struct ListIter<'a, A> {
    cur: Option<Rc<ListNode<A>>>,
    /// Ties the iterator to the source list borrow even though the cursor is owned.
    _borrow: PhantomData<&'a A>,
}

impl<A> ListIter<'_, A> {
    /// Returns whether more elements remain.
    #[inline]
    pub fn has_next(&self) -> bool {
        self.cur.is_some()
    }
}

impl<A: Clone> Iterator for ListIter<'_, A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        let node = self.cur.take()?;
        let result = node.head.clone();
        self.cur = node.tail();
        Some(result)
    }
}

/// Content equality over the element chains.
impl<A: PartialEq> PartialEq for List<A> {
    fn eq(&self, other: &Self) -> bool {
        List::equals(self, other)
    }
}

impl<A: Eq> Eq for List<A> {}

/// Hashes the elements with the list polynomial.
impl<A: Hash> Hash for List<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut h: i32 = 1;
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            let mut elem_hasher = std::collections::hash_map::DefaultHasher::new();
            node.head.hash(&mut elem_hasher);
            // Elements hash through the caller's hasher; the polynomial matches
            // the list contract, but the integer is not a cross-language value.
            h = h.wrapping_mul(31).wrapping_add(elem_hasher.finish() as i32);
            cur = node.tail();
        }
        h.hash(state);
    }
}

/// Formats the elements separated by commas.
impl<A: fmt::Display> fmt::Display for List<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_sep(","))
    }
}

impl<A: fmt::Debug> fmt::Debug for List<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "List[")?;
        let mut cur = self.node.clone();
        let mut first = true;
        while let Some(node) = cur {
            if !first {
                write!(f, ",")?;
            }
            first = false;
            write!(f, "{:?}", node.head)?;
            cur = node.tail();
        }
        write!(f, "]")
    }
}

/// Collects items by appending them in order.
impl<A: Clone> FromIterator<A> for List<A> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> List<A> {
        List::from_iterable(iter)
    }
}

/// Consuming iterator over a [`List`], yielding owned copies of the elements.
///
/// Lists are immutable and nodes are shared, so iteration walks the shared
/// chain; elements are cloned like every other accessor (`head`, `get`).
pub struct ListIntoIter<A> {
    cur: Option<Rc<ListNode<A>>>,
}

impl<A: Clone> Iterator for ListIntoIter<A> {
    type Item = A;

    fn next(&mut self) -> Option<A> {
        let node = self.cur.take()?;
        let result = node.head.clone();
        self.cur = node.tail();
        Some(result)
    }
}

impl<A: Clone> IntoIterator for List<A> {
    type Item = A;
    type IntoIter = ListIntoIter<A>;

    fn into_iter(self) -> Self::IntoIter {
        ListIntoIter { cur: self.node }
    }
}

impl<'a, A: Clone> IntoIterator for &'a List<A> {
    type Item = A;
    type IntoIter = ListIter<'a, A>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[i32]) -> List<i32> {
        List::from_array(Some(items.to_vec()))
    }

    #[test]
    fn construction() {
        assert!(List::<i32>::nil().is_empty());
        assert!(!List::of(1).is_empty());
        assert_eq!(List::of2(1, 2).to_vec(), vec![1, 2]);
        assert_eq!(List::of3(1, 2, 3).to_vec(), vec![1, 2, 3]);
        assert_eq!(List::of4(1, 2, 3, vec![4, 5]).to_vec(), vec![1, 2, 3, 4, 5]);
        assert_eq!(List::<i32>::from_array(None).length(), 0);
        assert_eq!(List::from_iterable(vec![1, 2]).to_vec(), vec![1, 2]);
        assert_eq!(List::fill(3, 7).to_vec(), vec![7, 7, 7]);
    }

    #[test]
    fn into_iter_yields_elements_in_order() {
        let l = list(&[1, 2, 3]);
        let collected: Vec<i32> = (&l).into_iter().collect();
        assert_eq!(collected, vec![1, 2, 3]);

        let owned: Vec<i32> = list(&[4, 5]).into_iter().collect();
        assert_eq!(owned, vec![4, 5]);

        assert!(List::<i32>::nil().into_iter().next().is_none());
        let empty: Vec<i32> = (&List::<i32>::nil()).into_iter().collect();
        assert!(empty.is_empty());
    }

    #[test]
    fn for_loops_use_into_iterator() {
        let l = list(&[10, 20]);
        let mut sum = 0;
        for x in &l {
            sum += x;
        }
        assert_eq!(sum, 30);

        let mut owned = Vec::new();
        for x in list(&[30, 40]) {
            owned.push(x);
        }
        assert_eq!(owned, vec![30, 40]);
    }
    #[test]
    fn length_and_tail() {
        let l = list(&[1, 2, 3]);
        assert_eq!(l.length(), 3);
        assert_eq!(l.size(), 3);
        assert_eq!(*l.head(), 1);
        assert_eq!(l.tail().to_vec(), vec![2, 3]);
        assert!(l.tail().tail().tail().is_empty());
        assert!(l.non_empty());
    }

    #[test]
    fn prepend_and_reverse_share_singletons() {
        let nil = list(&[]);
        assert!(nil.reverse().is_empty());
        let single = list(&[1]);
        // An empty or singleton list is returned unchanged.
        assert!(single.reverse().ptr_eq(&single));
        let many = list(&[1, 2, 3]);
        let rev = many.reverse();
        assert!(!rev.ptr_eq(&many));
        assert_eq!(rev.to_vec(), vec![3, 2, 1]);
        assert_eq!(many.to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn append_and_append_list() {
        let l = list(&[1, 2]);
        assert_eq!(l.append(3).to_vec(), vec![1, 2, 3]);
        let other = list(&[4, 5]);
        assert_eq!(l.append_list(&other).to_vec(), vec![1, 2, 4, 5]);
        assert_eq!(other.append_list(&l).to_vec(), vec![4, 5, 1, 2]);
        let buf = ListBuffer::from_iter(vec![7, 8]);
        assert_eq!(l.append_list_buffer(&buf).to_vec(), vec![1, 2, 7, 8]);
    }

    #[test]
    fn prepend_list_reuses_new_reversed_nodes() {
        let l = list(&[1, 2, 3]);
        let xs = list(&[4, 5, 6]);
        let combined = l.prepend_list(&xs);
        assert_eq!(combined.to_vec(), vec![4, 5, 6, 1, 2, 3]);
        // the original lists are untouched
        assert_eq!(l.to_vec(), vec![1, 2, 3]);
        assert_eq!(xs.to_vec(), vec![4, 5, 6]);
        // empty special cases
        assert!(List::<i32>::nil().prepend_list(&l).ptr_eq(&l));
        assert!(l.prepend_list(&List::nil()).ptr_eq(&l));
    }

    #[test]
    fn set_tail_splices_nodes() {
        let mut l = list(&[1]);
        let tail = list(&[2, 3]);
        let ret = l.set_tail(tail.clone());
        assert_eq!(l.to_vec(), vec![1, 2, 3]);
        assert!(ret.ptr_eq(&tail));
        // Splicing the empty sentinel panics.
        let mut empty = List::<i32>::nil();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                empty.set_tail(List::of(1))
            }))
            .is_err()
        );
    }

    #[test]
    fn filter_intersect_diff_take() {
        let l = list(&[1, 2, 3, 2]);
        assert_eq!(List::filter(&l, &2).to_vec(), vec![1, 3]);
        let other = list(&[2, 3, 9]);
        assert_eq!(l.intersect(&other).to_vec(), vec![2, 3, 2]);
        assert_eq!(l.diff(&other).to_vec(), vec![1]);
        assert_eq!(l.take(2).to_vec(), vec![1, 2]);
        assert_eq!(l.take(0).to_vec(), Vec::<i32>::new());
        assert_eq!(l.take(10).to_vec(), vec![1, 2, 3, 2]);
    }

    #[test]
    fn map_builds_new_list() {
        let l = list(&[1, 2, 3]);
        let m = l.map(|x| x * 2);
        assert_eq!(m.to_vec(), vec![2, 4, 6]);
        assert!(!m.ptr_eq(&l));
    }

    #[test]
    fn get_contains_index_of() {
        let l = list(&[10, 20, 30, 20]);
        assert_eq!(l.get(0), 10);
        assert_eq!(l.get(3), 20);
        assert_eq!(l.index_of(&20), 1);
        assert_eq!(l.last_index_of(&20), 3);
        assert_eq!(l.index_of(&99), -1);
        assert!(l.contains(&30));
        assert!(!l.contains(&31));
        assert_eq!(l.last(), Some(20));
        assert_eq!(List::<i32>::nil().last(), None);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| l.get(-1))).is_err()
        );
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| l.get(4))).is_err());
    }

    #[test]
    fn equality_is_content_equality() {
        assert_eq!(list(&[1, 2]), list(&[1, 2]));
        assert_ne!(list(&[1, 2]), list(&[2, 1]));
        assert_eq!(List::<i32>::nil(), List::<i32>::nil());
        assert_ne!(list(&[1]), List::<i32>::nil());
        // `Option` models a null element.
        let with_null: List<Option<i32>> = List::of(None);
        assert_eq!(with_null.to_vec(), vec![None]);
    }

    #[test]
    fn to_string_matches_java() {
        assert_eq!(list(&[1, 2, 3]).to_string(), "1,2,3");
        assert_eq!(List::<i32>::nil().to_string(), "");
        assert_eq!(list(&[1, 2, 3]).to_string_sep(", "), "1, 2, 3");
    }

    #[test]
    fn iterator_yields_elements_in_order() {
        let l = list(&[1, 2, 3]);
        let mut it = l.iter();
        assert_eq!(it.next(), Some(1));
        assert_eq!(it.next(), Some(2));
        assert_eq!(it.next(), Some(3));
        assert_eq!(it.next(), None);
        assert_eq!(List::<i32>::nil().iter().next(), None);
    }

    #[test]
    fn iterator_has_next_does_not_consume() {
        let l = list(&[1, 2]);
        let mut it = l.iter();
        assert!(it.has_next());
        assert!(it.has_next());
        assert_eq!(it.next(), Some(1));
        assert!(it.has_next());
        assert_eq!(it.next(), Some(2));
        assert!(!it.has_next());
    }

    #[test]
    fn hash_of_equal_lists_matches() {
        use std::collections::hash_map::DefaultHasher;
        fn hash_of(l: &List<i32>) -> u64 {
            let mut h = DefaultHasher::new();
            l.hash(&mut h);
            h.finish()
        }
        assert_eq!(hash_of(&list(&[1, 2])), hash_of(&list(&[1, 2])));
    }

    #[test]
    fn ptr_eq_distinguishes_equal_contents() {
        let a = list(&[1, 2]);
        let b = list(&[1, 2]);
        assert_eq!(a, b); // equals
        assert!(!a.ptr_eq(&b)); // reference identity
        assert!(a.ptr_eq(&a.clone()));
        assert!(List::<i32>::nil().ptr_eq(&List::<i32>::nil()));
    }

    #[test]
    fn long_list_reverse_and_length() {
        let items: Vec<i32> = (0..1000).collect();
        let l = List::from_array(Some(items));
        assert_eq!(l.length(), 1000);
        let rev = l.reverse();
        assert_eq!(rev.length(), 1000);
        assert_eq!(rev.head(), &999);
        assert_eq!(rev.tail().tail().head(), &997);
    }
}
