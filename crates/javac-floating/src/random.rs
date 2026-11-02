//! Port of the parts of `java.util.Random` used by the ported
//! `java.math.BigInteger` (candidate generation and Miller-Rabin bases).
//!
//! PORTING_RULES (standard-library adapter):
//! * the 48-bit LCG algorithms (`next`, `nextBytes`, `nextInt`,
//!   `nextInt(bound)`) are kept 1:1;
//! * Java's `synchronized` methods take `&mut self`;
//! * `ThreadLocalRandom.current()` (used by `passesMillerRabin` when no random
//!   source is supplied) is replaced by a fresh `Random` (the JDK's
//!   thread-local generator is not ported; registered: only the randomness
//!   source changes, not any control flow of BigInteger).
//!
//! NOT PORTED (registered): the `java.util.stream` based methods (`ints`,
//! `longs`, `doubles`), `nextGaussian`, and the object-stream support; none is
//! reachable from `BigInteger` or `FDBigInteger`.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Java: private static final long mask.
const MASK: i64 = (1i64 << 48) - 1;

/// Java: private static final double DOUBLE_UNIT.
const DOUBLE_UNIT: f64 = 1.0 / (1i64 << 53) as f64;

/// Java: private static final AtomicLong seedUniquifier.
static SEED_UNIQUIFIER: AtomicI64 = AtomicI64::new(8682522807148012);

/// Java: public class Random.
pub(crate) struct Random {
    /// Java: private final AtomicLong seed (the port uses a plain long under
    /// `&mut self`).
    seed: i64,
}

impl Random {
    /// Java: public Random() — seeds from the uniquifier and the clock.
    pub(crate) fn new() -> Random {
        let mut random = Random { seed: 0 };
        random.set_seed(Self::seed_uniquifier() ^ Self::nano_time());
        random
    }

    /// Java: public Random(long seed).
    pub(crate) fn with_seed(seed: i64) -> Random {
        let mut random = Random { seed: 0 };
        random.set_seed(seed);
        random
    }

    /// The clock source used by the no-argument constructor
    /// (java.lang.System.nanoTime; registered adapter).
    fn nano_time() -> i64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos() as i64,
            Err(_) => 0,
        }
    }

    /// Java: private static long seedUniquifier().
    fn seed_uniquifier() -> i64 {
        loop {
            let current = SEED_UNIQUIFIER.load(Ordering::SeqCst);
            let next = current.wrapping_mul(1181783497276652981);
            if SEED_UNIQUIFIER
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return next;
            }
        }
    }

    /// Java: private static long initialScramble(long seed).
    fn initial_scramble(seed: i64) -> i64 {
        (seed ^ 0x5DEECE66D) & MASK
    }

    /// Java: public synchronized void setSeed(long seed).
    pub(crate) fn set_seed(&mut self, seed: i64) {
        self.seed = Self::initial_scramble(seed);
    }

    /// Java: protected int next(int bits).
    fn next(&mut self, bits: i32) -> i32 {
        self.seed = self.seed.wrapping_mul(0x5DEECE66D).wrapping_add(0xB) & MASK;
        ((self.seed as u64) >> (48 - bits) as u64) as i32
    }

    /// Java: public void nextBytes(byte[] bytes).
    pub(crate) fn next_bytes(&mut self, bytes: &mut [u8]) {
        let mut i = 0;
        while i < bytes.len() {
            let mut rnd = self.next_int();
            let mut n = std::cmp::min(bytes.len() - i, 4);
            while n > 0 {
                n -= 1;
                bytes[i] = rnd as u8;
                rnd = ((rnd as u32) >> 8) as i32;
                i += 1;
            }
        }
    }

    /// Java: public int nextInt().
    pub(crate) fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// Java: public int nextInt(int bound).
    pub(crate) fn next_int_bound(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            panic!("IllegalArgumentException: bound must be positive");
        }
        let mut r = self.next(31);
        let m = bound - 1;
        if (bound & m) == 0 {
            r = ((bound as i64 * r as i64) >> 31) as i32;
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    break;
                }
                u = self.next(31);
            }
        }
        r
    }

    /// Java: public long nextLong().
    pub(crate) fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }

    /// Java: public boolean nextBoolean().
    pub(crate) fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    /// Java: public float nextFloat().
    pub(crate) fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1i64 << 24) as f32
    }

    /// Java: public double nextDouble().
    pub(crate) fn next_double(&mut self) -> f64 {
        (((self.next(26) as i64) << 27) + self.next(27) as i64) as f64 * DOUBLE_UNIT
    }
}

/// Java: java.util.concurrent.ThreadLocalRandom.current() — replaced by a fresh
/// `Random` (registered adapter; see the module documentation).
pub(crate) fn thread_local_random() -> Random {
    Random::new()
}
