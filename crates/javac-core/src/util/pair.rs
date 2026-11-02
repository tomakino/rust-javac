//! A generic class for pairs.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};

/// A generic class for pairs.
pub struct Pair<A, B> {
    pub fst: A,
    pub snd: B,
}

impl<A, B> Pair<A, B> {
    /// Java: public Pair(A fst, B snd) { this.fst = fst; this.snd = snd; }
    pub fn new(fst: A, snd: B) -> Self {
        Pair { fst, snd }
    }

    /// Java: public static <A,B> Pair<A,B> of(A a, B b) { return new Pair<>(a,b); }
    pub fn of(a: A, b: B) -> Self {
        Pair::new(a, b)
    }
}

/// Porting rules adapter: Rust value semantics require an explicit copy constructor;
/// the Java Pair is an immutable two-field data carrier, so the copy is
/// behaviourally identical.
impl<A: Clone, B: Clone> Clone for Pair<A, B> {
    fn clone(&self) -> Self {
        Pair {
            fst: self.fst.clone(),
            snd: self.snd.clone(),
        }
    }
}

impl<A: PartialEq, B: PartialEq> PartialEq for Pair<A, B> {
    fn eq(&self, other: &Self) -> bool {
        // Java: other instanceof Pair<?,?> pair && Objects.equals(fst, pair.fst) && Objects.equals(snd, pair.snd)
        self.fst == other.fst && self.snd == other.snd
    }
}

impl<A: Eq, B: Eq> Eq for Pair<A, B> {}

impl<A: Hash, B: Hash> Hash for Pair<A, B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Rust: Java hashCode formula is not reproduced; equality consistency
        // is what matters (Pair is never hashed as a wire value).
        self.fst.hash(state);
        self.snd.hash(state);
    }
}

// Java: public String toString() { return "Pair[" + fst + "," + snd + "]"; }
impl<A: Display, B: Display> Display for Pair<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pair[{},{}]", self.fst, self.snd)
    }
}

impl<A: fmt::Debug, B: fmt::Debug> fmt::Debug for Pair<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pair[{:?},{:?}]", self.fst, self.snd)
    }
}

#[cfg(test)]
mod tests {
    use super::Pair;

    #[test]
    fn display_matches_java_tostring() {
        let p = Pair::of(1, "x");
        assert_eq!(p.to_string(), "Pair[1,x]");
    }

    #[test]
    fn of_and_equality() {
        assert_eq!(Pair::of(1u8, 2u8), Pair::new(1u8, 2u8));
    }
}
