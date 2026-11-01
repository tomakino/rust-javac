// Origin: com/sun/tools/javac/util/ListBuffer.java @ 982d53c (jdk-17.0.19-ga)

//! A class for constructing lists by appending elements. Modelled after
//! java.lang.StringBuffer.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the four Java fields and the incremental algorithm are kept one-for-one:
//! elems/last/count/shared, append splices a node after the last data node with set_tail,
//! toList hands out the internal list in O(1) and sets shared, append after a toList first
//! runs the private copy() (copy-on-write). The only representational change is that the
//! foreign java.util.Collection/Queue interface obligations
//! (remove/removeAll/retainAll) are not ported: remove(Object) throws
//! UnsupportedOperationException in javac and none of the three is called (grep over the
//! upstream tree); they are registered in docs/STATUS.md.
//! PORTING_RULES: toList is a &self method here because it only updates the mutable
//! Java "shared" field, which is modelled with Cell<bool>; keeping it callable on a shared
//! reference mirrors the upstream call sites (e.g. List.appendList(ListBuffer)).
//! PORTING_RULES: Java's anonymous Iterator in iterator() is expressed with the ListIter used
//! by List.iterator(); hasNext/next semantics are identical.

use std::cell::Cell;

use super::list::{List, ListIter};

/// A class for constructing lists by appending elements.
pub struct ListBuffer<A> {
    /// The list of elements of this buffer.
    ///
    /// Java: private List<A> elems;
    elems: List<A>,

    /// A pointer pointing to the last element of 'elems' containing data,
    /// or null if the list is empty.
    ///
    /// Java: private List<A> last;
    last: Option<List<A>>,

    /// The number of element in this buffer.
    ///
    /// Java: private int count;
    count: i32,

    /// Has a list been created from this buffer yet?
    ///
    /// Java: private boolean shared;
    shared: Cell<bool>,
}

impl<A> ListBuffer<A> {
    /// Java: public static <T> ListBuffer<T> of(T x)
    pub fn of(x: A) -> ListBuffer<A>
    where
        A: Clone,
    {
        let mut lb = ListBuffer::new();
        lb.add(x);
        lb
    }

    /// Create a new initially empty list buffer.
    ///
    /// Java: public ListBuffer() { clear(); }
    pub fn new() -> ListBuffer<A> {
        let mut lb = ListBuffer {
            elems: List::nil(),
            last: None,
            count: 0,
            shared: Cell::new(false),
        };
        lb.clear();
        lb
    }

    /// Java: public final void clear()
    pub fn clear(&mut self) {
        self.elems = List::nil();
        self.last = None;
        self.count = 0;
        self.shared.set(false);
    }

    /// Return the number of elements in this buffer.
    ///
    /// Java: public int length() { return count; }
    pub fn length(&self) -> i32 {
        self.count
    }

    /// Java: public int size() { return count; }
    pub fn size(&self) -> i32 {
        self.count
    }

    /// Is buffer empty?
    ///
    /// Java: public boolean isEmpty() { return count == 0; }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Is buffer not empty?
    ///
    /// Java: public boolean nonEmpty() { return count != 0; }
    pub fn non_empty(&self) -> bool {
        self.count != 0
    }

    /// Copy list and sets last.
    ///
    /// Java: private void copy()
    fn copy(&mut self)
    where
        A: Clone,
    {
        if self.elems.non_empty() {
            let mut orig = self.elems.clone();

            self.elems = List::of(orig.head().clone());
            self.last = Some(self.elems.clone());

            loop {
                // Java: while ((orig = orig.tail).nonEmpty()) {
                // last.tail = List.of(orig.head);
                // last = last.tail;
                // }
                orig = orig.tail();
                if orig.is_empty() {
                    break;
                }
                let new_last = List::of(orig.head().clone());
                match &mut self.last {
                    Some(last) => {
                        last.set_tail(new_last.clone());
                        *last = new_last;
                    }
                    // unreachable: last is assigned just before the loop
                    None => crate::util::assert::error(),
                }
            }
        }
    }

    /// Prepend an element to buffer.
    ///
    /// Java: public ListBuffer<A> prepend(A x) {
    ///     elems = elems.prepend(x);
    ///     if (last == null) last = elems;
    ///     count++;
    ///     return this;
    /// }
    pub fn prepend(&mut self, x: A) -> &mut Self {
        self.elems = self.elems.prepend(x);
        if self.last.is_none() {
            self.last = Some(self.elems.clone());
        }
        self.count += 1;
        self
    }

    /// Append an element to buffer.
    ///
    /// Java: public ListBuffer<A> append(A x) {
    ///     Assert.checkNonNull(x);
    ///     if (shared) copy();
    ///     List<A> newLast = List.of(x);
    ///     if (last != null) {
    ///         last.tail = newLast;
    ///         last = newLast;
    ///     } else {
    ///         elems = last = newLast;
    ///     }
    ///     count++;
    ///     return this;
    /// }
    // PORTING_RULES: Java needs no Clone here; the Rust copy() (run when shared) must rebuild
    // the existing chain, so appending requires A: Clone. A is a handle or an id in javac, so
    // the clone is O(1).
    pub fn append(&mut self, x: A) -> &mut Self
    where
        A: Clone,
    {
        // Java: Assert.checkNonNull(x); — a Rust value is never a null reference; when the
        // element type is a Java reference, it is represented as Option.
        if self.shared.get() {
            self.copy();
        }
        let new_last = List::of(x);
        if let Some(last) = &mut self.last {
            last.set_tail(new_last.clone());
            *last = new_last;
        } else {
            self.elems = new_last.clone();
            self.last = Some(new_last);
        }
        self.count += 1;
        self
    }

    /// Append all elements in a list to buffer.
    ///
    /// Java: public ListBuffer<A> appendList(List<A> xs) {
    ///     while (xs.nonEmpty()) { append(xs.head); xs = xs.tail; }
    ///     return this;
    /// }
    pub fn append_list(&mut self, xs: &List<A>) -> &mut Self
    where
        A: Clone,
    {
        let mut xs = xs.clone();
        while xs.non_empty() {
            // Java: append(xs.head);
            self.append(xs.head().clone());
            xs = xs.tail();
        }
        self
    }

    /// Append all elements in a list to buffer.
    ///
    /// Java: public ListBuffer<A> appendList(ListBuffer<A> xs) { return appendList(xs.toList()); }
    pub fn append_list_buffer(&mut self, xs: &ListBuffer<A>) -> &mut Self
    where
        A: Clone,
    {
        let xs = xs.to_list();
        self.append_list(&xs)
    }

    /// Append all elements in an array to buffer.
    ///
    /// Java: public ListBuffer<A> appendArray(A[] xs)
    pub fn append_array(&mut self, xs: &[A]) -> &mut Self
    where
        A: Clone,
    {
        for x in xs {
            self.append(x.clone());
        }
        self
    }

    /// Convert buffer to a list of all its elements.
    ///
    /// Java: public List<A> toList() { shared = true; return elems; }
    pub fn to_list(&self) -> List<A> {
        self.shared.set(true);
        self.elems.clone()
    }

    /// Does the list contain the specified element?
    ///
    /// Java: public boolean contains(Object x) { return elems.contains(x); }
    pub fn contains(&self, x: &A) -> bool
    where
        A: PartialEq,
    {
        self.elems.contains(x)
    }

    /// Convert buffer to an array.
    ///
    /// Java: public <T> T[] toArray(T[] vec) / public Object[] toArray()
    pub fn to_vec(&self) -> Vec<A>
    where
        A: Clone,
    {
        self.elems.to_vec()
    }

    /// The first element in this buffer.
    ///
    /// Java: public A first() { return elems.head; } — null when empty (a NullPointerException
    /// if the field is read on the sentinel); the Rust panic models that.
    pub fn first(&self) -> &A {
        self.elems.head()
    }

    /// Return first element in this buffer and remove.
    ///
    /// Java: public A next() {
    ///     A x = elems.head;
    ///     if (!elems.isEmpty()) {
    ///         elems = elems.tail;
    ///         if (elems.isEmpty()) last = null;
    ///         count--;
    ///     }
    ///     return x;
    /// }
    ///
    /// PORTING_RULES: Java returns the null head for an empty buffer; Option<A> models that
    /// null (None when the buffer is empty). The Java method name next() is kept, so the method
    /// is not a std::iter::Iterator::next.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<A>
    where
        A: Clone,
    {
        let x = if self.elems.is_empty() {
            None
        } else {
            Some(self.elems.head().clone())
        };
        if !self.elems.is_empty() {
            self.elems = self.elems.tail();
            if self.elems.is_empty() {
                self.last = None;
            }
            self.count -= 1;
        }
        x
    }

    /// An enumeration of all elements in this buffer.
    ///
    /// Java: public Iterator<A> iterator()
    pub fn iterator(&self) -> ListIter<'_, A> {
        self.elems.iter()
    }

    /// Java: public boolean add(A a) { append(a); return true; }
    pub fn add(&mut self, a: A) -> bool
    where
        A: Clone,
    {
        self.append(a);
        true
    }

    /// Java: public boolean containsAll(Collection<?> c)
    pub fn contains_all(&self, c: &[A]) -> bool
    where
        A: PartialEq,
    {
        for x in c {
            if !self.contains(x) {
                return false;
            }
        }
        true
    }

    /// Java: public boolean addAll(Collection<? extends A> c)
    pub fn add_all(&mut self, c: impl IntoIterator<Item = A>) -> bool
    where
        A: Clone,
    {
        for a in c {
            self.append(a);
        }
        true
    }

    /// Java: public boolean offer(A a) { append(a); return true; }
    pub fn offer(&mut self, a: A) -> bool
    where
        A: Clone,
    {
        self.append(a);
        true
    }

    /// Java: public A poll() { return next(); }
    pub fn poll(&mut self) -> Option<A>
    where
        A: Clone,
    {
        self.next()
    }

    /// Java: public A peek() { return first(); }
    pub fn peek(&self) -> &A {
        self.first()
    }

    /// Java: public A last() { return last != null ? last.head : null; }
    pub fn last(&self) -> Option<&A> {
        self.last.as_ref().map(|last| last.head())
    }

    // PORTING_RULES: the remaining java.util.Queue/Collection obligations of ListBuffer
    // (remove(Object), removeAll, retainAll) throw UnsupportedOperationException in javac and
    // have no callers (grep over the upstream tree); see docs/STATUS.md.
}

impl<A> Default for ListBuffer<A> {
    fn default() -> Self {
        ListBuffer::new()
    }
}

impl<A> FromIterator<A> for ListBuffer<A>
where
    A: Clone,
{
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> ListBuffer<A> {
        let mut lb = ListBuffer::new();
        lb.add_all(iter);
        lb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_keeps_order_and_length() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.length(), 0);
        buf.append(1).append(2).append(3);
        assert_eq!(buf.length(), 3);
        assert!(buf.non_empty());
        assert_eq!(buf.to_list().to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn prepend_adds_to_front() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.append(2).append(3);
        buf.prepend(1);
        assert_eq!(buf.to_list().to_vec(), vec![1, 2, 3]);
        let mut empty: ListBuffer<i32> = ListBuffer::new();
        empty.prepend(7);
        assert_eq!(empty.to_list().to_vec(), vec![7]);
        assert_eq!(*empty.first(), 7);
        assert_eq!(empty.last(), Some(&7));
    }

    #[test]
    fn to_list_hands_out_the_internal_list() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.append(1);
        let first = buf.to_list();
        let second = buf.to_list();
        // Java: toList returns elems itself, so two calls without mutation return the same list.
        assert!(first.ptr_eq(&second));
        // Java: appending after toList copies first; the handed-out list does not change.
        buf.append(2);
        assert_eq!(first.to_vec(), vec![1]);
        assert_eq!(buf.to_list().to_vec(), vec![1, 2]);
        assert!(!buf.to_list().ptr_eq(&first));
    }

    #[test]
    fn clear_resets_everything() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.append(1).append(2);
        buf.to_list();
        buf.clear();
        assert_eq!(buf.length(), 0);
        assert!(buf.is_empty());
        assert_eq!(buf.last(), None);
        // a fresh list can be created again after clear
        buf.append(9);
        assert_eq!(buf.to_list().to_vec(), vec![9]);
    }

    #[test]
    fn append_list_and_buffer_and_array() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.append_list(&List::of2(1, 2));
        assert_eq!(buf.to_list().to_vec(), vec![1, 2]);
        let mut other: ListBuffer<i32> = ListBuffer::new();
        other.append(3).append(4);
        buf.append_list_buffer(&other);
        assert_eq!(buf.to_list().to_vec(), vec![1, 2, 3, 4]);
        buf.append_array(&[5, 6]);
        assert_eq!(buf.to_list().to_vec(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn next_poll_peek_first_last() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.append(10).append(20);
        assert_eq!(*buf.peek(), 10);
        assert_eq!(buf.poll(), Some(10));
        assert_eq!(buf.length(), 1);
        assert_eq!(buf.poll(), Some(20));
        assert_eq!(buf.length(), 0);
        assert_eq!(buf.poll(), None);
        assert_eq!(buf.last(), None);
    }

    #[test]
    fn contains_and_add_all() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.add_all(vec![1, 2, 3]);
        assert!(buf.contains(&2));
        assert!(!buf.contains(&4));
        assert!(buf.contains_all(&[1, 3]));
        assert!(!buf.contains_all(&[1, 4]));
        assert!(buf.offer(4));
        assert_eq!(buf.length(), 4);
    }

    #[test]
    fn iterator_walks_snapshot() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.append(1).append(2);
        let mut it = buf.iterator();
        assert_eq!(it.next(), Some(1));
        assert_eq!(it.next(), Some(2));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn of_creates_single_element_buffer() {
        let buf = ListBuffer::of(42);
        assert_eq!(buf.length(), 1);
        assert_eq!(buf.to_list().to_vec(), vec![42]);
    }

    #[test]
    fn append_after_repeated_to_list_keeps_handed_out_lists_stable() {
        let mut buf: ListBuffer<i32> = ListBuffer::new();
        buf.append(1);
        let first = buf.to_list();
        buf.append(2);
        let second = buf.to_list();
        buf.append(3);
        assert_eq!(first.to_vec(), vec![1]);
        assert_eq!(second.to_vec(), vec![1, 2]);
        assert_eq!(buf.to_list().to_vec(), vec![1, 2, 3]);
    }
}
