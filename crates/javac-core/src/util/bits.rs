// Origin: com/sun/tools/javac/util/Bits.java @ 982d53c (jdk-17.0.19-ga)

//! A class for extensible, mutable bit sets.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

use std::fmt;

use super::assert;

// ____________ reset _________
// / UNKNOWN \ <-------- / UNINIT \
// \____________/ | \_________/
// | | |
// |assign | | any
// | ___________ |
// ------> / NORMAL \ <----
// \___________/ |
// | |
// | |
// -----------
// any
/// PORTING_RULES: Java enum constants keep their upstream names so that diffs against
/// Bits.java stay mechanical.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BitsState {
    /// A Bits instance is in UNKNOWN state if it has been explicitly reset.
    /// It is possible to get to this state from any other by calling the
    /// reset method. An instance in the UNKNOWN state can pass to the
    /// NORMAL state after being assigned another Bits instance.
    ///
    /// Bits instances are final fields in Flow so the UNKNOWN state models
    /// the null assignment.
    UNKNOWN,
    /// A Bits instance is in UNINIT when it is created with the default
    /// constructor but it isn't explicitly reset. The main objective of this
    /// internal state is to save some memory.
    UNINIT,
    /// The normal state is reached after creating a Bits instance from an
    /// existing one or after applying any operation to an instance on UNINIT
    /// or NORMAL state. From this state a bits instance can pass to the
    /// UNKNOWN state by calling the reset method.
    NORMAL,
}

/// PORTING_RULES: Java keeps three distinguishable values in `int[] bits`: null (UNKNOWN),
/// the shared empty `unassignedBits` array (identity marker of UNINIT) and a real array.
/// Rust has no array-reference identity, so the three cases are modelled explicitly; this keeps
/// `BitsState.getState` and the protected constructor assertion identical.
#[derive(Clone)]
enum BitsArray {
    /// Java: the shared `unassignedBits` array (identity marker of BitsState.UNINIT).
    Unassigned,
    /// Java: null.
    Null,
    /// A real `int[]` (which can have length 0, e.g. after `clear` on an UNINIT instance).
    Owned(Vec<i32>),
}

impl BitsArray {
    fn is_unassigned(&self) -> bool {
        matches!(self, BitsArray::Unassigned)
    }

    /// Java: `bits.length`; reading the length of the null array throws NullPointerException.
    fn len(&self) -> usize {
        match self {
            BitsArray::Owned(v) => v.len(),
            BitsArray::Unassigned => 0,
            BitsArray::Null => panic!("java.lang.NullPointerException"),
        }
    }

    fn as_slice(&self) -> &[i32] {
        match self {
            BitsArray::Owned(v) => v.as_slice(),
            BitsArray::Unassigned => &[],
            BitsArray::Null => panic!("java.lang.NullPointerException"),
        }
    }
}

impl BitsState {
    /// Java: static BitsState getState(int[] someBits, boolean reset)
    fn get_state(some_bits: &BitsArray, reset: bool) -> BitsState {
        if reset {
            BitsState::UNKNOWN
        } else if !some_bits.is_unassigned() {
            BitsState::NORMAL
        } else {
            BitsState::UNINIT
        }
    }
}

const WORDLEN: i32 = 32;
const WORDSHIFT: u32 = 5;
const WORDMASK: i32 = WORDLEN - 1;

/// A class for extensible, mutable bit sets.
pub struct Bits {
    /// Java: public int[] bits = null;
    ///
    /// PORTING_RULES: private here because a Rust `Vec<i32>` cannot model the null/shared-empty
    /// cases; `BitsArray` is the faithful representation. javac never reads this field outside
    /// of Bits.java (verified by grep over the upstream tree).
    bits: BitsArray,
    /// Java: protected BitsState currentState;
    current_state: BitsState,
}

impl Bits {
    /// Java: public Bits() { this(false); }
    pub fn new() -> Bits {
        Bits::with_reset(false)
    }

    /// Java: public Bits(Bits someBits) {
    ///     this(someBits.dup().bits, BitsState.getState(someBits.bits, false));
    /// }
    pub fn from_bits(some_bits: &mut Bits) -> Bits {
        let dup = some_bits.dup();
        let init_state = BitsState::get_state(&some_bits.bits, false);
        Bits::with_array(dup.bits, init_state)
    }

    /// Java: public Bits(boolean reset) {
    ///     this(unassignedBits, BitsState.getState(unassignedBits, reset));
    /// }
    pub fn with_reset(reset: bool) -> Bits {
        let unassigned_bits = BitsArray::Unassigned;
        let init_state = BitsState::get_state(&unassigned_bits, reset);
        Bits::with_array(unassigned_bits, init_state)
    }

    /// Java: protected Bits(int[] bits, BitsState initState)
    fn with_array(bits: BitsArray, init_state: BitsState) -> Bits {
        let mut bits = bits;
        match init_state {
            BitsState::UNKNOWN => {
                bits = BitsArray::Null;
            }
            BitsState::NORMAL => {
                assert::check(!bits.is_unassigned());
            }
            BitsState::UNINIT => {}
        }
        Bits {
            bits,
            current_state: init_state,
        }
    }

    /// Java: protected void sizeTo(int len)
    fn size_to(&mut self, len: i32) {
        if self.bits.len() < len as usize {
            self.bits = match &self.bits {
                BitsArray::Unassigned => BitsArray::Owned(vec![0; len as usize]),
                BitsArray::Null => panic!("java.lang.NullPointerException"),
                BitsArray::Owned(v) => {
                    let mut new_bits = v.clone();
                    new_bits.resize(len as usize, 0);
                    BitsArray::Owned(new_bits)
                }
            };
        }
    }

    /// This set = {}.
    ///
    /// Java: public void clear()
    pub fn clear(&mut self) {
        assert::check(self.current_state != BitsState::UNKNOWN);
        match &mut self.bits {
            BitsArray::Owned(v) => {
                for word in v.iter_mut() {
                    *word = 0;
                }
            }
            // Java: the shared unassignedBits array has length 0, the loop body never runs.
            BitsArray::Unassigned => {}
            BitsArray::Null => panic!("java.lang.NullPointerException"),
        }
        self.current_state = BitsState::NORMAL;
    }

    /// Java: public void reset() { internalReset(); }
    pub fn reset(&mut self) {
        self.internal_reset();
    }

    /// Java: protected void internalReset() { bits = null; currentState = BitsState.UNKNOWN; }
    pub fn internal_reset(&mut self) {
        self.bits = BitsArray::Null;
        self.current_state = BitsState::UNKNOWN;
    }

    /// Java: public boolean isReset() { return currentState == BitsState.UNKNOWN; }
    pub fn is_reset(&self) -> bool {
        self.current_state == BitsState::UNKNOWN
    }

    /// Java: public Bits assign(Bits someBits) {
    ///     bits = someBits.dup().bits;
    ///     currentState = BitsState.NORMAL;
    ///     return this;
    /// }
    pub fn assign(&mut self, some_bits: &mut Bits) -> &mut Bits {
        self.bits = some_bits.dup().bits;
        self.current_state = BitsState::NORMAL;
        self
    }

    /// Return a copy of this set.
    ///
    /// Java: public Bits dup()
    pub fn dup(&mut self) -> Bits {
        assert::check(self.current_state != BitsState::UNKNOWN);
        let mut tmp = Bits::new();
        tmp.bits = self.dup_bits();
        self.current_state = BitsState::NORMAL;
        tmp
    }

    /// Java: protected int[] dupBits()
    fn dup_bits(&self) -> BitsArray {
        if self.current_state != BitsState::NORMAL {
            self.bits.clone()
        } else {
            match &self.bits {
                BitsArray::Owned(v) => BitsArray::Owned(v.clone()),
                // Java: Arrays.copyOf of the shared empty array yields a fresh empty array.
                BitsArray::Unassigned => BitsArray::Owned(Vec::new()),
                BitsArray::Null => panic!("java.lang.NullPointerException"),
            }
        }
    }

    /// Include x in this set.
    ///
    /// Java: public void incl(int x)
    pub fn incl(&mut self, x: i32) {
        assert::check(self.current_state != BitsState::UNKNOWN);
        assert::check(x >= 0);
        self.size_to(((x as u32) >> WORDSHIFT) as i32 + 1);
        let windex = ((x as u32) >> WORDSHIFT) as usize;
        let value = 1i32 << (x & WORDMASK);
        match &mut self.bits {
            BitsArray::Owned(v) => v[windex] |= value,
            _ => panic!("java.lang.NullPointerException"),
        }
        self.current_state = BitsState::NORMAL;
    }

    /// Include [start..limit) in this set.
    ///
    /// Java: public void inclRange(int start, int limit)
    pub fn incl_range(&mut self, start: i32, limit: i32) {
        assert::check(self.current_state != BitsState::UNKNOWN);
        self.size_to(((limit as u32) >> WORDSHIFT) as i32 + 1);
        let mut x = start;
        while x < limit {
            let windex = ((x as u32) >> WORDSHIFT) as usize;
            let value = 1i32 << (x & WORDMASK);
            match &mut self.bits {
                BitsArray::Owned(v) => v[windex] |= value,
                _ => panic!("java.lang.NullPointerException"),
            }
            x += 1;
        }
        self.current_state = BitsState::NORMAL;
    }

    /// Exclude [start...end] from this set.
    ///
    /// Java: public void excludeFrom(int start)
    pub fn exclude_from(&mut self, start: i32) {
        assert::check(self.current_state != BitsState::UNKNOWN);
        let mut temp = Bits::new();
        temp.size_to(self.bits.len() as i32);
        temp.incl_range(0, start);
        self.internal_and_set(&temp);
        self.current_state = BitsState::NORMAL;
    }

    /// Exclude x from this set.
    ///
    /// Java: public void excl(int x)
    pub fn excl(&mut self, x: i32) {
        assert::check(self.current_state != BitsState::UNKNOWN);
        assert::check(x >= 0);
        self.size_to(((x as u32) >> WORDSHIFT) as i32 + 1);
        let windex = ((x as u32) >> WORDSHIFT) as usize;
        let value = !(1i32 << (x & WORDMASK));
        match &mut self.bits {
            BitsArray::Owned(v) => v[windex] &= value,
            _ => panic!("java.lang.NullPointerException"),
        }
        self.current_state = BitsState::NORMAL;
    }

    /// Is x an element of this set?
    ///
    /// Java: public boolean isMember(int x)
    pub fn is_member(&self, x: i32) -> bool {
        assert::check(self.current_state != BitsState::UNKNOWN);
        let len = self.bits.len() as i32;
        // Java: 0 <= x && x < (bits.length << wordshift) &&...
        // (int overflow of the shift follows Java semantics via wrapping_shl)
        0 <= x
            && x < len.wrapping_shl(WORDSHIFT)
            && (self.bits.as_slice()[((x as u32) >> WORDSHIFT) as usize]
                & (1i32 << (x & WORDMASK)))
                != 0
    }

    /// {@literal this set = this set & xs}.
    ///
    /// Java: public Bits andSet(Bits xs)
    pub fn and_set(&mut self, xs: &Bits) -> &mut Bits {
        assert::check(self.current_state != BitsState::UNKNOWN);
        self.internal_and_set(xs);
        self.current_state = BitsState::NORMAL;
        self
    }

    /// Java: protected void internalAndSet(Bits xs)
    fn internal_and_set(&mut self, xs: &Bits) {
        assert::check(self.current_state != BitsState::UNKNOWN);
        self.size_to(xs.bits.len() as i32);
        let xs_bits = xs.bits.as_slice();
        match &mut self.bits {
            BitsArray::Owned(v) => {
                for i in 0..xs_bits.len() {
                    v[i] &= xs_bits[i];
                }
            }
            // sizeTo above guarantees a real array whenever xs is non-empty.
            BitsArray::Unassigned => {}
            BitsArray::Null => panic!("java.lang.NullPointerException"),
        }
    }

    /// this set = this set | xs.
    ///
    /// Java: public Bits orSet(Bits xs)
    pub fn or_set(&mut self, xs: &Bits) -> &mut Bits {
        assert::check(self.current_state != BitsState::UNKNOWN);
        self.size_to(xs.bits.len() as i32);
        let xs_bits = xs.bits.as_slice();
        match &mut self.bits {
            BitsArray::Owned(v) => {
                for i in 0..xs_bits.len() {
                    v[i] |= xs_bits[i];
                }
            }
            BitsArray::Unassigned => {}
            BitsArray::Null => panic!("java.lang.NullPointerException"),
        }
        self.current_state = BitsState::NORMAL;
        self
    }

    /// this set = this set \ xs.
    ///
    /// Java: public Bits diffSet(Bits xs)
    pub fn diff_set(&mut self, xs: &Bits) -> &mut Bits {
        assert::check(self.current_state != BitsState::UNKNOWN);
        let xs_bits = xs.bits.as_slice();
        match &mut self.bits {
            BitsArray::Owned(v) => {
                for i in 0..v.len() {
                    if i < xs_bits.len() {
                        v[i] &= !xs_bits[i];
                    }
                }
            }
            BitsArray::Unassigned => {}
            BitsArray::Null => panic!("java.lang.NullPointerException"),
        }
        self.current_state = BitsState::NORMAL;
        self
    }

    /// this set = this set ^ xs.
    ///
    /// Java: public Bits xorSet(Bits xs)
    pub fn xor_set(&mut self, xs: &Bits) -> &mut Bits {
        assert::check(self.current_state != BitsState::UNKNOWN);
        self.size_to(xs.bits.len() as i32);
        let xs_bits = xs.bits.as_slice();
        match &mut self.bits {
            BitsArray::Owned(v) => {
                for i in 0..xs_bits.len() {
                    v[i] ^= xs_bits[i];
                }
            }
            BitsArray::Unassigned => {}
            BitsArray::Null => panic!("java.lang.NullPointerException"),
        }
        self.current_state = BitsState::NORMAL;
        self
    }

    /// Count trailing zero bits in an int. Algorithm from "Hacker's
    /// Delight" by Henry S. Warren Jr. (figure 5-13)
    ///
    /// Java: private static int trailingZeroBits(int x)
    fn trailing_zero_bits(x: i32) -> i32 {
        assert::check(WORDLEN == 32);
        if x == 0 {
            return 32;
        }
        let mut x = x as u32;
        let mut n = 1;
        if (x & 0xffff) == 0 {
            n += 16;
            x >>= 16;
        }
        if (x & 0x00ff) == 0 {
            n += 8;
            x >>= 8;
        }
        if (x & 0x000f) == 0 {
            n += 4;
            x >>= 4;
        }
        if (x & 0x0003) == 0 {
            n += 2;
            x >>= 2;
        }
        n - ((x & 1) as i32)
    }

    /// Return the index of the least bit position &ge; x that is set.
    /// If none are set, returns -1.  This provides a nice way to iterate
    /// over the members of a bit set:
    /// <pre>{@code
    /// for (int i = bits.nextBit(0); i>=0; i = bits.nextBit(i+1)) ...
    /// }</pre>
    ///
    /// Java: public int nextBit(int x)
    pub fn next_bit(&self, x: i32) -> i32 {
        assert::check(self.current_state != BitsState::UNKNOWN);
        let bits = self.bits.as_slice();
        let mut windex = ((x as u32) >> WORDSHIFT) as i32;
        if windex >= bits.len() as i32 {
            return -1;
        }
        let mut word = bits[windex as usize] & !((1i32 << (x & WORDMASK)).wrapping_sub(1));
        loop {
            if word != 0 {
                return windex
                    .wrapping_shl(WORDSHIFT)
                    .wrapping_add(Bits::trailing_zero_bits(word));
            }
            windex += 1;
            if windex >= bits.len() as i32 {
                return -1;
            }
            word = bits[windex as usize];
        }
    }
}

impl Default for Bits {
    fn default() -> Self {
        Bits::new()
    }
}

/// Java: public String toString()
impl fmt::Display for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !matches!(self.bits, BitsArray::Null) && self.bits.len() > 0 {
            let mut digits = String::with_capacity(self.bits.len() * WORDLEN as usize);
            let mut i = 0;
            while i < self.bits.len() as i32 * WORDLEN {
                digits.push(if self.is_member(i) { '1' } else { '0' });
                i += 1;
            }
            write!(f, "{}", digits)
        } else {
            write!(f, "[]")
        }
    }
}

impl fmt::Debug for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::Bits;

    #[test]
    fn new_is_uninit_not_reset() {
        let b = Bits::new();
        assert!(!b.is_reset());
    }

    #[test]
    fn with_reset_is_reset() {
        let b = Bits::with_reset(true);
        assert!(b.is_reset());
    }

    #[test]
    fn incl_excl_member() {
        let mut b = Bits::new();
        b.incl(0);
        b.incl(31);
        b.incl(32);
        assert!(b.is_member(0));
        assert!(b.is_member(31));
        assert!(b.is_member(32));
        assert!(!b.is_member(1));
        assert!(!b.is_member(33));
        b.excl(31);
        assert!(!b.is_member(31));
        assert!(b.is_member(32));
    }

    #[test]
    fn incl_range_sets_half_open_interval() {
        let mut b = Bits::new();
        b.incl_range(2, 6);
        for i in 0..10 {
            assert_eq!(b.is_member(i), (2..6).contains(&i), "bit {}", i);
        }
        assert_eq!(b.next_bit(0), 2);
        assert_eq!(b.next_bit(3), 3);
        assert_eq!(b.next_bit(6), -1);
    }

    #[test]
    fn clear_zeroes_all_bits() {
        let mut b = Bits::new();
        b.incl(1);
        b.clear();
        assert!(!b.is_member(1));
        assert_eq!(b.next_bit(0), -1);
    }

    #[test]
    fn next_bit_walks_members() {
        let mut b = Bits::new();
        for i in [3, 40, 70] {
            b.incl(i);
        }
        let mut seen = Vec::new();
        let mut i = b.next_bit(0);
        while i >= 0 {
            seen.push(i);
            i = b.next_bit(i + 1);
        }
        assert_eq!(seen, vec![3, 40, 70]);
    }

    #[test]
    fn set_operations() {
        let mut a = Bits::new();
        a.incl(1);
        a.incl(2);
        let mut b = Bits::new();
        b.incl(2);
        b.incl(3);

        let mut and = Bits::new();
        and.incl(1);
        and.incl(2);
        and.and_set(&b);
        assert!(and.is_member(2));
        assert!(!and.is_member(1));

        let mut or = Bits::new();
        or.incl(1);
        or.incl(2);
        or.or_set(&b);
        assert!(or.is_member(1) && or.is_member(2) && or.is_member(3));

        let mut xor = Bits::new();
        xor.incl(1);
        xor.incl(2);
        xor.xor_set(&b);
        assert!(xor.is_member(1) && !xor.is_member(2) && xor.is_member(3));

        let mut diff = Bits::new();
        diff.incl(1);
        diff.incl(2);
        diff.diff_set(&b);
        assert!(diff.is_member(1) && !diff.is_member(2));
    }

    #[test]
    fn exclude_from_keeps_prefix() {
        let mut b = Bits::new();
        for i in 0..64 {
            b.incl(i);
        }
        b.exclude_from(40);
        assert!(b.is_member(0) && b.is_member(39));
        assert!(!b.is_member(40) && !b.is_member(63));
    }

    #[test]
    fn dup_is_independent_after_normal_state() {
        let mut a = Bits::new();
        a.incl(5);
        let mut b = a.dup();
        b.incl(6);
        assert!(b.is_member(6));
        assert!(!a.is_member(6));
    }

    #[test]
    fn from_bits_copies() {
        let mut a = Bits::new();
        a.incl(7);
        let mut b = Bits::from_bits(&mut a);
        b.incl(8);
        assert!(!a.is_member(8));
        assert!(b.is_member(7) && b.is_member(8));
    }

    #[test]
    fn assign_replaces_contents() {
        let mut a = Bits::new();
        a.incl(1);
        let mut b = Bits::new();
        b.incl(9);
        a.assign(&mut b);
        // Java: assign copies the contents of someBits over this set.
        assert!(a.is_member(9));
        assert!(!a.is_member(1));
        b.incl(10);
        assert!(!a.is_member(10));
    }

    #[test]
    fn to_string_matches_java_layout() {
        let mut b = Bits::new();
        b.incl(0);
        assert_eq!(b.to_string(), "1".to_string() + &"0".repeat(31));
        let empty = Bits::new();
        assert_eq!(empty.to_string(), "[]");
        let reset = Bits::with_reset(true);
        assert_eq!(reset.to_string(), "[]");
    }

    #[test]
    fn reset_then_use_panics() {
        let mut b = Bits::with_reset(true);
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| b.incl(1)));
        assert!(err.is_err());
    }
}
