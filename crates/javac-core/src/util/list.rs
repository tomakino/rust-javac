//! Generic linked lists.
//!
//! A List<A> is an immutable singly linked list trailed by a sentinel;
//! ListBuffer is the mutable builder. Java's null list reference maps to
//! List::nil(), reference identity (==) maps to List::ptr_eq, and
//! PartialEq implements content equality.
//!
//! Nodes are shared through Rc. The tail cell exists only to splice nodes
//! while a list is being built (prependList, ListBuffer::append); the
//! mutation is never observed by another list.

use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::rc::Rc;

use super::assert;
use super::list_buffer::ListBuffer;

/// A node of a javac List.
struct ListNode<A> {
    /// The first element of the list, supposed to be immutable.
    ///
    /// Java: public A head;
    head: A,

    /// The remainder of the list except for its first element, supposed
    /// to be immutable.
    ///
    /// Java: public List<A> tail;
    ///
    /// None is Java's null tail (the sentinel); the Cell models the mutability of
    /// the Java field for setTail.
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
                Ok(mut node) => next = node.tail.take(),
                Err(_) => break,
            }
        }
    }
}

impl<A> ListNode<A> {
    /// Java: read of the tail field.
    #[inline]
    fn tail(&self) -> Option<Rc<ListNode<A>>> {
        // Read the shared tail without the take/clone/set round trip: the Cell
        // is only mutated through set_tail on freshly built nodes, so reading
        // it by pointer and cloning the Rc is observationally identical while
        // avoiding two writes on every node visit in hot list walks.
        // SAFETY: the Cell always holds a valid Option<Rc<ListNode<A>>>, and
        // List is single-threaded (not Sync), so no concurrent mutation occurs.
        unsafe { (*self.tail.as_ptr()).clone() }
    }

    /// Borrow the tail chain for read-only walks, so a walk does not bump a
    /// refcount per node.
    ///
    /// SAFETY (caller contract): the Cell always holds a valid
    /// Option<Rc<ListNode<A>>>; List is single-threaded and set_tail only runs
    /// on freshly built nodes, so the returned borrow stays valid as long as
    /// the list owning this node is alive.
    #[inline]
    fn tail_ref(&self) -> Option<&ListNode<A>> {
        unsafe { (*self.tail.as_ptr()).as_deref() }
    }

    /// Java: this.tail = tail.
    #[inline]
    fn set_tail(&self, tail: Option<Rc<ListNode<A>>>) {
        self.tail.set(tail);
    }
}

/// A class for generic linked lists.
pub struct List<A> {
    /// the Java List reference. None is the Java null reference, i.e. the
    /// sentinel that terminates every list (an empty list is represented by that null).
    node: Option<Rc<ListNode<A>>>,
}

impl<A> List<A> {
    /// Construct an empty list.
    ///
    /// Java: public static <A> List<A> nil() { return (List<A>)EMPTY_LIST; }
    pub fn nil() -> List<A> {
        List { node: None }
    }

    /// Construct a list given its head and tail.
    ///
    /// Java: List(A head, List<A> tail) { this.tail = tail; this.head = head; }
    fn cons(head: A, tail: &List<A>) -> List<A> {
        List {
            node: Some(Rc::new(ListNode {
                head,
                tail: Cell::new(tail.node.clone()),
            })),
        }
    }

    /// Java: reference comparison between two List values, i.e. the Java == operator.
    #[inline]
    pub fn ptr_eq(&self, other: &List<A>) -> bool {
        match (&self.node, &other.node) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// Returns the list obtained from 'l' after removing all elements 'elem'
    ///
    /// Java: public static <A> List<A> filter(List<A> l, A elem)
    pub fn filter(l: &List<A>, elem: &A) -> List<A>
    where
        A: Clone + PartialEq,
    {
        // Java: Assert.checkNonNull(elem); — a Rust reference cannot be null. Java's element
        // null check (a != null) is provided by the element type: null elements are represented
        // as Option values, whose PartialEq compares them correctly.
        // Java builds this by prepending then reversing; ListBuffer already
        // appends in O(1) with a single chained construction, so skip the
        // double allocation.
        let mut buf: ListBuffer<A> = ListBuffer::new();
        for a in l.iter() {
            if &a != elem {
                buf.append(a);
            }
        }
        buf.to_list()
    }

    /// Java: public List<A> intersect(List<A> that)
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

    /// Java: public List<A> diff(List<A> that)
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

    /// Create a new list from the first n elements of this list
    ///
    /// Java: public List<A> take(int n)
    pub fn take(&self, n: i32) -> List<A>
    where
        A: Clone,
    {
        let mut buf: ListBuffer<A> = ListBuffer::new();
        // Java: if (count++ == n) break;
        // (a negative n never matches, so every element is appended, as in Java)
        for (count, el) in self.iter().enumerate() {
            if count == n as usize {
                break;
            }
            buf.append(el);
        }
        buf.to_list()
    }

    /// Construct a list consisting of given element.
    ///
    /// Java: public static <A> List<A> of(A x1)
    pub fn of(x1: A) -> List<A> {
        List::cons(x1, &List::nil())
    }

    /// Construct a list consisting of given elements.
    ///
    /// Java: public static <A> List<A> of(A x1, A x2)
    pub fn of2(x1: A, x2: A) -> List<A> {
        List::of(x2).prepend(x1)
    }

    /// Construct a list consisting of given elements.
    ///
    /// Java: public static <A> List<A> of(A x1, A x2, A x3)
    pub fn of3(x1: A, x2: A, x3: A) -> List<A> {
        List::of2(x2, x3).prepend(x1)
    }

    /// Construct a list consisting of given elements.
    ///
    /// Java: public static <A> List<A> of(A x1, A x2, A x3, A... rest)
    pub fn of4(x1: A, x2: A, x3: A, rest: Vec<A>) -> List<A> {
        List::from_array(Some(rest))
            .prepend(x3)
            .prepend(x2)
            .prepend(x1)
    }

    /// Construct a list consisting all elements of given array.
    /// @param array an array; if null return an empty list
    ///
    /// Java: public static <A> List<A> from(A[] array)
    pub fn from_array(array: Option<Vec<A>>) -> List<A> {
        let mut xs = List::nil();
        if let Some(array) = array {
            for a in array.into_iter().rev() {
                // Java: xs = new List<>(array[i], xs);
                xs = List::cons(a, &xs);
            }
        }
        xs
    }

    /// Java: public static <A> List<A> from(Iterable<? extends A> coll)
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

    /// Construct a list consisting of a given number of identical elements.
    /// @param len    The number of elements in the list.
    /// @param init   The value of each element.
    ///
    /// Java: @Deprecated public static <A> List<A> fill(int len, A init)
    pub fn fill(len: i32, init: A) -> List<A>
    where
        A: Clone,
    {
        let mut l = List::nil();
        let mut i = 0;
        while i < len {
            // Java: l = new List<>(init, l);
            l = List::cons(init.clone(), &l);
            i += 1;
        }
        l
    }

    /// Does list have no elements?
    ///
    /// Java: public boolean isEmpty() { return tail == null; }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.node.is_none()
    }

    /// Does list have elements?
    ///
    /// Java: public boolean nonEmpty() { return tail != null; }
    #[inline]
    pub fn non_empty(&self) -> bool {
        self.node.is_some()
    }

    /// Return the number of elements in this list.
    ///
    /// Java: public int length()
    pub fn length(&self) -> i32 {
        let mut len = 0;
        let mut cur = self.node.as_deref();
        while let Some(node) = cur {
            len += 1;
            cur = node.tail_ref();
        }
        len
    }

    /// Java: public int size() { return length(); }
    pub fn size(&self) -> i32 {
        self.length()
    }

    /// Java: public List<A> setTail(List<A> tail) { this.tail = tail; return tail; }
    pub fn set_tail(&mut self, tail: List<A>) -> List<A> {
        match &self.node {
            Some(node) => node.set_tail(tail.node.clone()),
            // Java: the EMPTY_LIST singleton overrides setTail to throw
            // UnsupportedOperationException; a Rust None is that null reference.
            None => panic!("java.util.UnsupportedOperationException"),
        }
        tail
    }

    /// Returns the first element of the list.
    ///
    /// Java: public A head; — reading the field on the null sentinel is a
    /// NullPointerException, which is what the empty-list panic models.
    #[inline]
    pub fn head(&self) -> &A {
        match &self.node {
            Some(node) => &node.head,
            None => panic!("java.lang.NullPointerException"),
        }
    }

    /// Returns the remainder of the list except for its first element.
    ///
    /// Java: public List<A> tail; — null on the sentinel.
    #[inline]
    pub fn tail(&self) -> List<A> {
        match &self.node {
            Some(node) => List { node: node.tail() },
            None => List { node: None },
        }
    }

    /// Prepend given element to front of list, forming and returning
    /// a new list.
    ///
    /// Java: public List<A> prepend(A x) { return new List<>(x, this); }
    #[inline]
    pub fn prepend(&self, x: A) -> List<A> {
        List::cons(x, self)
    }

    /// Prepend given list of elements to front of list, forming and returning
    /// a new list.
    ///
    /// Java: public List<A> prependList(List<A> xs)
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
        // Java: Assert.check(rev != xs); — reference comparison, hence ptr_eq.
        assert::check(!rev.ptr_eq(xs));
        // since xs.reverse() returned a new list, we can reuse the
        // individual List objects, instead of allocating new ones.
        while rev.non_empty() {
            let mut h = rev;
            rev = h.tail();
            h.set_tail(result);
            result = h;
        }
        result
    }

    /// Reverse list.
    /// If the list is empty or a singleton, then the same list is returned.
    /// Otherwise a new list is formed.
    ///
    /// Java: public List<A> reverse()
    pub fn reverse(&self) -> List<A>
    where
        A: Clone,
    {
        // if it is empty or a singleton, return itself
        if self.is_empty() || self.tail().is_empty() {
            return self.clone();
        }

        // Walk the shared nodes by reference: only the element is cloned, so a
        // reversal no longer bumps the refcount of every node on the way.
        let mut rev = List::nil();
        let mut cur = self.node.as_deref();
        while let Some(node) = cur {
            // Java: rev = new List<>(l.head, rev);
            rev = List::cons(node.head.clone(), &rev);
            // SAFETY: same invariant as ListNode::tail; the Cell is only mutated
            // on freshly built nodes and `self` keeps the chain alive here.
            cur = node.tail_ref();
        }
        rev
    }

    /// Append given element at length, forming and returning
    /// a new list.
    ///
    /// Java: public List<A> append(A x) { return of(x).prependList(this); }
    pub fn append(&self, x: A) -> List<A>
    where
        A: Clone,
    {
        List::of(x).prepend_list(self)
    }

    /// Append given list at length, forming and returning
    /// a new list.
    ///
    /// Java: public List<A> appendList(List<A> x) { return x.prependList(this); }
    pub fn append_list(&self, x: &List<A>) -> List<A>
    where
        A: Clone,
    {
        x.prepend_list(self)
    }

    /// Append given list buffer at length, forming and returning a new list.
    ///
    /// Java: public List<A> appendList(ListBuffer<A> x) { return appendList(x.toList()); }
    pub fn append_list_buffer(&self, x: &ListBuffer<A>) -> List<A>
    where
        A: Clone,
    {
        self.append_list(&x.to_list())
    }

    /// Copy successive elements of this list into a new vector.
    ///
    /// Java: public <T> T[] toArray(T[] vec) — a Java reference array is a Vec in Rust.
    pub fn to_vec(&self) -> Vec<A>
    where
        A: Clone,
    {
        let mut result = Vec::new();
        let mut cur = self.node.as_deref();
        while let Some(node) = cur {
            result.push(node.head.clone());
            cur = node.tail_ref();
        }
        result
    }

    /// Form a string listing all elements with given separator character.
    ///
    /// Java: public String toString(String sep)
    pub fn to_string_sep(&self, sep: &str) -> String
    where
        A: fmt::Display,
    {
        let Some(first) = self.node.as_ref() else {
            return String::new();
        };
        use std::fmt::Write;
        let mut buf = String::new();
        let _ = write!(buf, "{}", first.head);
        let mut cur = first.tail_ref();
        while let Some(node) = cur {
            buf.push_str(sep);
            let _ = write!(buf, "{}", node.head);
            cur = node.tail_ref();
        }
        buf
    }

    /// Are the two lists the same?
    ///
    /// Java: public static boolean equals(List<?> xs, List<?> ys)
    pub fn equals(xs: &List<A>, ys: &List<A>) -> bool
    where
        A: PartialEq,
    {
        let mut x = xs.node.as_deref();
        let mut y = ys.node.as_deref();
        loop {
            match (x, y) {
                (None, None) => return true,
                (Some(xn), Some(yn)) => {
                    // Java: xs.head == null ? ys.head == null : xs.head.equals(ys.head) — the
                    // Option element type handles the null case through PartialEq.
                    if xn.head != yn.head {
                        return false;
                    }
                    x = xn.tail_ref();
                    y = yn.tail_ref();
                }
                _ => return false,
            }
        }
    }

    /// Does the list contain the specified element?
    ///
    /// Java: public boolean contains(Object x)
    pub fn contains(&self, x: &A) -> bool
    where
        A: PartialEq,
    {
        let mut cur = self.node.as_deref();
        while let Some(node) = cur {
            if &node.head == x {
                return true;
            }
            cur = node.tail_ref();
        }
        false
    }

    /// The last element in the list, if any, or null.
    ///
    /// Java: public A last() {
    ///     A last = null;
    ///     List<A> t = this;
    ///     while (t.tail != null) { last = t.head; t = t.tail; }
    ///     return last;
    /// }
    pub fn last(&self) -> Option<A>
    where
        A: Clone,
    {
        let mut last = None;
        let mut cur = self.node.as_deref();
        while let Some(node) = cur {
            last = Some(node.head.clone());
            cur = node.tail_ref();
        }
        last
    }

    /// Java: public <Z> List<Z> map(Function<A, Z> mapper)
    pub fn map<Z, F>(&self, mut mapper: F) -> List<Z>
    where
        A: Clone,
        Z: Clone,
        F: FnMut(&A) -> Z,
    {
        // Java: boolean changed = false;... changed |= (z != a);
        // Rust: Java compares the mapped value with the input reference and returns
        // this when the mapper is the identity; Rust cannot compare values of unrelated types
        // and the list is immutable, so a fresh list with the same elements is returned instead.
        // The only observable difference would be List reference identity, not contents.
        let mut buf: ListBuffer<Z> = ListBuffer::new();
        for a in self.iter() {
            let z = mapper(&a);
            buf.append(z);
        }
        buf.to_list()
    }

    // REGISTERED(P3): public static <T> List<T> convert(Class<T> klass, List<?> list)
    //
    // Rust: convert runs a reflective element-type check (klass.cast) before the
    // unchecked cast; Rust's type system performs that check statically at the ported call site
    // (Type.getTypeArguments returns List<Type> whose elements are TypeVar), so no 1:1 runtime
    // equivalent exists. Registered in docs/STATUS.md.

    /// Java: public A get(int index)
    pub fn get(&self, index: i32) -> A
    where
        A: Clone,
    {
        if index < 0 {
            // Java: throw new IndexOutOfBoundsException(String.valueOf(index));
            panic!("java.lang.IndexOutOfBoundsException: {}", index);
        }

        // Java: for (int i = index; i-- > 0 && !l.isEmpty(); l = l.tail);
        let mut cur = self.node.as_deref();
        let mut i = index;
        while i > 0 {
            match cur {
                Some(node) => {
                    cur = node.tail_ref();
                    i -= 1;
                }
                None => break,
            }
        }

        if cur.is_none() {
            // Java: throw new IndexOutOfBoundsException("Index: " + index + ", Size: " + size());
            panic!(
                "java.lang.IndexOutOfBoundsException: Index: {}, Size: {}",
                index,
                self.size()
            );
        }
        cur.expect("get checked non-empty").head.clone()
    }

    /// Java: public int indexOf(Object o)
    pub fn index_of(&self, o: &A) -> i32
    where
        A: PartialEq,
    {
        let mut i = 0;
        let mut cur = self.node.as_deref();
        while let Some(node) = cur {
            if &node.head == o {
                return i;
            }
            cur = node.tail_ref();
            i += 1;
        }
        -1
    }

    /// Java: public int lastIndexOf(Object o)
    pub fn last_index_of(&self, o: &A) -> i32
    where
        A: PartialEq,
    {
        let mut last = -1;
        let mut i = 0;
        let mut cur = self.node.as_deref();
        while let Some(node) = cur {
            if &node.head == o {
                last = i;
            }
            cur = node.tail_ref();
            i += 1;
        }
        last
    }

    /// An enumeration of all elements in this list.
    ///
    /// Java: public Iterator<A> iterator()
    pub fn iter(&self) -> ListIter<'_, A> {
        ListIter {
            cur: self.node.as_deref(),
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

/// Java: the Iterator<A> returned by List.iterator().
/// The iterator borrows the shared nodes instead of owning a `List` copy, so
/// walking a list no longer bumps a refcount per element.
pub struct ListIter<'a, A> {
    /// Java: List<A> elems = List.this;
    cur: Option<&'a ListNode<A>>,
}

impl<A> ListIter<'_, A> {
    /// Java: public boolean hasNext() { return elems.tail != null; }
    #[inline]
    pub fn has_next(&self) -> bool {
        self.cur.is_some()
    }
}

impl<A: Clone> Iterator for ListIter<'_, A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        // Java: throw new NoSuchElementException(); the None result models it.
        let node = self.cur.take()?;
        let result = node.head.clone();
        self.cur = node.tail_ref();
        Some(result)
    }
}

/// Java: public boolean equals(Object other) and public static boolean equals(List, List)
impl<A: PartialEq> PartialEq for List<A> {
    fn eq(&self, other: &Self) -> bool {
        List::equals(self, other)
    }
}

impl<A: Eq> Eq for List<A> {}

/// Java: public int hashCode() — h = h * 31 + (head == null ? 0 : head.hashCode())
impl<A: Hash> Hash for List<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut h: i32 = 1;
        let mut cur = self.node.clone();
        while let Some(node) = cur {
            let mut elem_hasher = std::collections::hash_map::DefaultHasher::new();
            node.head.hash(&mut elem_hasher);
            // Rust: elements hash through Rust's Hasher; the exact int is not the Java
            // Object.hashCode value, but the formula and its wrapping follow List.hashCode.
            h = h.wrapping_mul(31).wrapping_add(elem_hasher.finish() as i32);
            cur = node.tail();
        }
        h.hash(state);
    }
}

/// Java: public String toString()
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

/// Java: public static <Z> Collector<Z, ListBuffer<Z>, List<Z>> collector()
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
        // Java: reverse() returns this for an empty or singleton list.
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
        // Java: the EMPTY_LIST singleton throws UnsupportedOperationException
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
        // Java: List.of(null) is legal; Option models the null element.
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
        assert!(!a.ptr_eq(&b)); // Java ==
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
