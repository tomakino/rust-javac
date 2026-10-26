// Origin: com/sun/tools/javac/util/Iterators.java @ 982d53c (jdk-17.0.19-ga)

//! Utilities for Iterators.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: Java java.util.Iterator<O> maps to std::iter::Iterator<Item = O>; Java
//! Iterable<I> parameters map to a Rust Iterator (the call site performs iterable.iterator()).
//! The explicit Java iterator contract is kept where it is observable: hasNext is has_next and
//! the Java NoSuchElementException paths panic. FilterIterator.next() returns the null
//! "current" element in Java when exhausted; Option models that null.
//! PORTING_RULES: Iterator.remove() throws UnsupportedOperationException in both classes and has
//! no callers; std::iter::Iterator has no remove, so it is not ported (registered in STATUS.md).

/// Java: public static <I, O> Iterator<O> createCompoundIterator(Iterable<I> inputs,
///                                                              Function<I, Iterator<O>> convertor)
pub fn create_compound_iterator<I, F, CI>(inputs: I, convertor: F) -> CompoundIterator<I, F, CI>
where
    I: Iterator,
    CI: Iterator,
{
    CompoundIterator {
        inputs,
        convertor,
        // Java: private Iterator<O> currentIterator = EMPTY;
        current_iterator: CurrentIterator::Empty,
    }
}

/// PORTING_RULES: Java distinguishes three states for currentIterator: the EMPTY sentinel
/// (initial), null (exhausted) and a real iterator; the enum keeps that distinction because the
/// next()/hasNext() interaction depends on it.
enum CurrentIterator<CI: Iterator> {
    Empty,
    Done,
    /// Java: currentIterator.hasNext() does not consume the element; Peekable preserves that.
    Iter(std::iter::Peekable<CI>),
}

/// Java: private static class CompoundIterator<I, O> implements Iterator<O>
pub struct CompoundIterator<I: Iterator, F, CI: Iterator> {
    /// Java: private final Iterator<I> inputs; (Java: Iterable<I> inputs, then inputs.iterator())
    inputs: I,
    /// Java: private final Function<I, Iterator<O>> convertor;
    convertor: F,
    /// Java: private Iterator<O> currentIterator = EMPTY;
    current_iterator: CurrentIterator<CI>,
}

impl<I, F, CI, O> CompoundIterator<I, F, CI>
where
    I: Iterator,
    F: FnMut(I::Item) -> CI,
    CI: Iterator<Item = O>,
{
    /// Java: public boolean hasNext()
    pub fn has_next(&mut self) -> bool {
        // Java: if (currentIterator != null && !currentIterator.hasNext()) { update(); }
        // return currentIterator != null;
        let need_update = match &mut self.current_iterator {
            CurrentIterator::Done => false,
            CurrentIterator::Empty => true,
            CurrentIterator::Iter(it) => it.peek().is_none(),
        };
        if need_update {
            self.update();
        }
        !matches!(self.current_iterator, CurrentIterator::Done)
    }

    /// Java: private void update()
    fn update(&mut self) {
        for input in self.inputs.by_ref() {
            let mut current = (self.convertor)(input).peekable();
            // Java: if (currentIterator.hasNext()) return;
            if current.peek().is_some() {
                self.current_iterator = CurrentIterator::Iter(current);
                return;
            }
        }
        self.current_iterator = CurrentIterator::Done;
    }
}

impl<I, F, CI, O> Iterator for CompoundIterator<I, F, CI>
where
    I: Iterator,
    F: FnMut(I::Item) -> CI,
    CI: Iterator<Item = O>,
{
    type Item = O;

    /// Java: public O next() {
    ///     if (currentIterator == EMPTY && !hasNext()) {
    ///         throw new NoSuchElementException();
    ///     }
    ///     return currentIterator.next();
    /// }
    fn next(&mut self) -> Option<O> {
        if matches!(self.current_iterator, CurrentIterator::Empty) && !self.has_next() {
            // Java: throw new NoSuchElementException();
            panic!("java.util.NoSuchElementException");
        }
        match &mut self.current_iterator {
            CurrentIterator::Iter(it) => match it.next() {
                Some(value) => Some(value),
                // Java: the underlying currentIterator.next() throws NoSuchElementException if
                // it was exhausted without a hasNext() in between.
                None => panic!("java.util.NoSuchElementException"),
            },
            // Java: currentIterator is null here; calling next() on it is a NullPointerException.
            CurrentIterator::Done => panic!("java.lang.NullPointerException"),
            CurrentIterator::Empty => unreachable!(),
        }
    }
}

/// Java: public static <E> Iterator<E> createFilterIterator(Iterator<E> input, Predicate<E> test)
pub fn create_filter_iterator<I, P, E>(input: I, test: P) -> FilterIterator<I, P, E>
where
    I: Iterator<Item = E>,
    P: FnMut(&E) -> bool,
{
    let mut result = FilterIterator {
        input,
        test,
        // Java: private E current = update();
        current: None,
    };
    result.current = result.update();
    result
}

/// Java: the anonymous Iterator<E> returned by createFilterIterator.
pub struct FilterIterator<I, P, E> {
    input: I,
    test: P,
    /// Java: private E current;
    current: Option<E>,
}

impl<I, P, E> FilterIterator<I, P, E>
where
    I: Iterator<Item = E>,
    P: FnMut(&E) -> bool,
{
    /// Java: private E update()
    fn update(&mut self) -> Option<E> {
        // Java: while (input.hasNext()) { E sym = input.next(); if (test.test(sym)) return sym; }
        self.input.find(|sym| (self.test)(sym))
    }

    /// Java: public boolean hasNext() { return current != null; }
    pub fn has_next(&self) -> bool {
        self.current.is_some()
    }
}

impl<I, P, E> Iterator for FilterIterator<I, P, E>
where
    I: Iterator<Item = E>,
    P: FnMut(&E) -> bool,
{
    type Item = E;

    /// Java: public E next() { E res = current; current = update(); return res; }
    fn next(&mut self) -> Option<E> {
        let res = self.current.take();
        self.current = self.update();
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_iterator_flattens_inputs_in_order() {
        let inputs = vec![vec![1, 2], vec![], vec![3]].into_iter();
        let mut it = create_compound_iterator(inputs, |v: Vec<i32>| v.into_iter());
        // Java's Iterator contract requires hasNext() between sources (the underlying iterator
        // throws NoSuchElementException once exhausted).
        let mut out = Vec::new();
        while it.has_next() {
            out.push(it.next().unwrap());
        }
        assert_eq!(out, vec![1, 2, 3]);
    }

    #[test]
    fn compound_iterator_next_on_exhausted_source_panics() {
        let inputs = vec![vec![1]].into_iter();
        let mut it = create_compound_iterator(inputs, |v: Vec<i32>| v.into_iter());
        assert_eq!(it.next(), Some(1));
        // Java: next() after the current iterator is exhausted throws NoSuchElementException.
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| it.next())).is_err()
        );
    }

    #[test]
    fn compound_iterator_without_inputs_is_exhausted() {
        let inputs: std::vec::IntoIter<Vec<i32>> = Vec::new().into_iter();
        let mut it = create_compound_iterator(inputs, |v: Vec<i32>| v.into_iter());
        assert!(!it.has_next());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| it.next())).is_err()
        );
    }

    #[test]
    fn compound_iterator_next_without_has_next() {
        let inputs = vec![vec![7]].into_iter();
        let mut it = create_compound_iterator(inputs, |v: Vec<i32>| v.into_iter());
        assert_eq!(it.next(), Some(7));
        assert!(!it.has_next());
    }

    #[test]
    fn filter_iterator_prefetches_and_filters() {
        let mut it = create_filter_iterator(vec![1, 2, 3, 4].into_iter(), |x: &i32| x % 2 == 0);
        assert!(it.has_next());
        assert_eq!(it.next(), Some(2));
        assert_eq!(it.next(), Some(4));
        assert!(!it.has_next());
        assert_eq!(it.next(), None);
    }

    #[test]
    fn filter_iterator_empty() {
        let mut it = create_filter_iterator(Vec::<i32>::new().into_iter(), |_| true);
        assert!(!it.has_next());
        assert_eq!(it.next(), None);
    }
}
