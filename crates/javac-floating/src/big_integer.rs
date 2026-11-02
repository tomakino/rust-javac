//! Port of `java.math.BigInteger` (JDK 17) together with the two package-private
//! helpers it uses, `MutableBigInteger`, `SignedMutableBigInteger` and
//! `BitSieve`.
//!
//! PORTING_RULES (standard-library adapter):
//! * Java `int[] mag` (little-endian) becomes `Vec<i32>`; `byte[]` becomes
//!   `Vec<u8>`; `long` stays `i64` and the two's-complement wraparound is
//!   explicit.
//! * `java.lang.Character.digit` comes from the ported `javac-unicode` crate;
//!   `Integer/Long.numberOfLeadingZeros` etc. are the `u32/u64` intrinsics.
//! * The Java object-stream support (`readObject/writeObject/`UnsafeHolder) is not
//!   ported (no Java serialization in this port; registered).
//! * The Java constants/caches (`posConst/negConst`) are rebuilt on demand; the
//!   values are identical (registered performance adapter).
//! * `setStable`-style lazy fields become `Cell<i32>` caches under `&self`.
//!
//! NOT PORTED (registered): object serialization (`readObject`, `writeObject`,
//! `readObjectNoData`, `magSerializedForm`, `serialPersistentFields`) and the
//! `java.util.stream` based `Random` methods; neither is reachable from the
//! compiler or from `FDBigInteger`.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::LazyLock;

use crate::mutable_big_integer::MutableBigInteger;
use crate::random::Random;

/// Java: static final long LONG_MASK.
pub(crate) const LONG_MASK: i64 = 0xffff_ffff;

/// Java: private static final int MAX_MAG_LENGTH.
const MAX_MAG_LENGTH: i32 = i32::MAX / 32 + 1;

/// Java: private static final int PRIME_SEARCH_BIT_LENGTH_LIMIT.
const PRIME_SEARCH_BIT_LENGTH_LIMIT: i32 = 500_000_000;

/// Java: private static final int KARATSUBA_THRESHOLD.
const KARATSUBA_THRESHOLD: i32 = 80;

/// Java: private static final int TOOM_COOK_THRESHOLD.
const TOOM_COOK_THRESHOLD: i32 = 240;

/// Java: private static final int KARATSUBA_SQUARE_THRESHOLD.
const KARATSUBA_SQUARE_THRESHOLD: i32 = 128;

/// Java: private static final int TOOM_COOK_SQUARE_THRESHOLD.
const TOOM_COOK_SQUARE_THRESHOLD: i32 = 216;

/// Java: static final int BURNIKEL_ZIEGLER_THRESHOLD.
pub(crate) const BURNIKEL_ZIEGLER_THRESHOLD: i32 = 80;

/// Java: static final int BURNIKEL_ZIEGLER_OFFSET.
pub(crate) const BURNIKEL_ZIEGLER_OFFSET: i32 = 40;

/// Java: private static final int SCHOENHAGE_BASE_CONVERSION_THRESHOLD.
const SCHOENHAGE_BASE_CONVERSION_THRESHOLD: i32 = 20;

/// Java: private static final int MULTIPLY_SQUARE_THRESHOLD.
const MULTIPLY_SQUARE_THRESHOLD: i32 = 20;

/// Java: private static final int MONTGOMERY_INTRINSIC_THRESHOLD.
const MONTGOMERY_INTRINSIC_THRESHOLD: i32 = 512;

/// Java: private static final int SMALL_PRIME_THRESHOLD.
const SMALL_PRIME_THRESHOLD: i32 = 95;

/// Java: private static final int DEFAULT_PRIME_CERTAINTY.
const DEFAULT_PRIME_CERTAINTY: i32 = 100;

/// Java: private static final int MAX_CONSTANT.
const MAX_CONSTANT: i32 = 16;

/// Java: private static long bitsPerDigit[] (scaled by 1024).
const BITS_PER_DIGIT: [i64; 37] = [
    0, 0, 1024, 1624, 2048, 2378, 2648, 2875, 3072, 3247, 3402, 3543, 3672, 3790, 3899, 4001,
    4096, 4186, 4271, 4350, 4426, 4498, 4567, 4633, 4696, 4756, 4814, 4870, 4923, 4975, 5025,
    5074, 5120, 5166, 5210, 5253, 5295,
];

/// Java: private static int digitsPerInt[].
const DIGITS_PER_INT: [i32; 37] = [
    0, 0, 30, 19, 15, 13, 11, 11, 10, 9, 9, 8, 8, 8, 8, 7, 7, 7, 7, 7, 7, 7, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 5,
];

/// Java: private static int intRadix[].
const INT_RADIX: [i32; 37] = [
    0, 0, 0x40000000, 0x4546b3db, 0x40000000, 0x48c27395, 0x159fd800, 0x75db9c97, 0x40000000,
    0x17179149, 0x3b9aca00, 0x0cc6db61, 0x19a10000, 0x309f1021, 0x57f6c100, 0x0a2f1b6f,
    0x10000000, 0x18754571, 0x247dbc80, 0x3547667b, 0x4c4b4000, 0x6b5a6e1d, 0x06c20a40,
    0x08d2d931, 0x000b6400, 0x0e8d4a51, 0x1269ae40, 0x17179149, 0x1cb91000, 0x23744899,
    0x2b73a840, 0x34e63b41, 0x40000000, 0x4cfa3cc1, 0x5c13d840, 0x6d91b519, 0x039aa400,
];

/// Java: private static int digitsPerLong[].
const DIGITS_PER_LONG: [i32; 37] = [
    0, 0, 62, 39, 31, 27, 24, 22, 20, 19, 18, 18, 17, 17, 16, 16, 15, 15, 15, 14, 14, 14, 14, 13,
    13, 13, 13, 13, 13, 12, 12, 12, 12, 12, 12, 12, 12,
];

/// Java: private static final double LOG_TWO.
const LOG_TWO: f64 = std::f64::consts::LN_2;

/// Java: public class BigInteger.
#[derive(Debug)]
pub(crate) struct BigInteger {
    /// Java: final int[] mag (little-endian two's-complement magnitude).
    mag: Vec<i32>,
    /// Java: final int signum.
    pub(crate) signum: i32,
    /// Java: private int bitCountPlusOne (stable lazy value, 0 = not computed).
    bit_count_plus_one: AtomicI32,
    /// Java: private int bitLengthPlusOne.
    bit_length_plus_one: AtomicI32,
    /// Java: private int lowestSetBitPlusTwo.
    lowest_set_bit_plus_two: AtomicI32,
    /// Java: private int firstNonzeroIntNumPlusTwo.
    first_nonzero_int_num_plus_two: AtomicI32,
}


impl Clone for BigInteger {
    fn clone(&self) -> BigInteger {
        BigInteger {
            mag: self.mag.clone(),
            signum: self.signum,
            bit_count_plus_one: AtomicI32::new(0),
            bit_length_plus_one: AtomicI32::new(0),
            lowest_set_bit_plus_two: AtomicI32::new(0),
            first_nonzero_int_num_plus_two: AtomicI32::new(0),
        }
    }
}

/// Java: ZERO/ONE/TWO/TEN/NEGATIVE_ONE.
static ZERO: LazyLock<BigInteger> = LazyLock::new(|| BigInteger::new_from_internal(Vec::new(), 0));
static ONE: LazyLock<BigInteger> = LazyLock::new(|| BigInteger::value_of_long(1));
static TWO: LazyLock<BigInteger> = LazyLock::new(|| BigInteger::value_of_long(2));
static TEN: LazyLock<BigInteger> = LazyLock::new(|| BigInteger::value_of_long(10));
static NEGATIVE_ONE: LazyLock<BigInteger> = LazyLock::new(|| BigInteger::value_of_long(-1));

/// Java: private static final BigInteger SMALL_PRIME_PRODUCT.
static SMALL_PRIME_PRODUCT: LazyLock<BigInteger> = LazyLock::new(|| {
    BigInteger::value_of_long(3i64 * 5 * 7 * 11 * 13 * 17 * 19 * 23 * 29 * 31 * 37 * 41)
});

/// Java: private static volatile BigInteger[][] powerCache (radix -> powers of
/// radix^(2^n)).
static POWER_CACHE: LazyLock<std::sync::Mutex<Vec<Vec<BigInteger>>>> = LazyLock::new(|| {
    let mut cache: Vec<Vec<BigInteger>> = Vec::with_capacity(37);
    for i in 0..37 {
        if i >= 2 {
            cache.push(vec![BigInteger::value_of_long(i as i64)]);
        } else {
            cache.push(Vec::new());
        }
    }
    std::sync::Mutex::new(cache)
});

/// Java: private static final double[] logCache.
static LOG_CACHE: LazyLock<Vec<f64>> = LazyLock::new(|| {
    let mut cache = vec![0.0f64; 37];
    for i in 2..37 {
        cache[i] = (i as f64).ln();
    }
    cache
});

impl BigInteger {
    /// Java: static final int BURNIKEL_ZIEGLER_THRESHOLD.
    pub(crate) const BURNIKEL_ZIEGLER_THRESHOLD: i32 = BURNIKEL_ZIEGLER_THRESHOLD;

    /// Java: static final int BURNIKEL_ZIEGLER_OFFSET.
    pub(crate) const BURNIKEL_ZIEGLER_OFFSET: i32 = BURNIKEL_ZIEGLER_OFFSET;

    /// Java: BigInteger(int[] magnitude, int signum) — internal, no copy.
    pub(crate) fn new_from_internal(magnitude: Vec<i32>, signum: i32) -> BigInteger {
        let signum = if magnitude.is_empty() { 0 } else { signum };
        let value = BigInteger {
            mag: magnitude,
            signum,
            bit_count_plus_one: AtomicI32::new(0),
            bit_length_plus_one: AtomicI32::new(0),
            lowest_set_bit_plus_two: AtomicI32::new(0),
            first_nonzero_int_num_plus_two: AtomicI32::new(0),
        };
        if value.mag.len() as i32 >= MAX_MAG_LENGTH {
            value.check_range();
        }
        value
    }

    /// The package-level constructor used by MutableBigInteger (Java:
    /// BigInteger(int[] magnitude, int signum)).
    pub(crate) fn new_from_mag(magnitude: Vec<i32>, signum: i32) -> BigInteger {
        BigInteger::new_from_internal(magnitude, signum)
    }

    /// The magnitude words (Java: the final `mag` field; read by
    /// MutableBigInteger and the ported helpers).
    pub(crate) fn magnitude(&self) -> &Vec<i32> {
        &self.mag
    }

    /// Java: public static BigInteger valueOf(long val).
    pub(crate) fn value_of_long(val: i64) -> BigInteger {
        if val == 0 {
            return ZERO.clone();
        }
        BigInteger::new_from_long(val)
    }

    /// Java: private BigInteger(long val).
    fn new_from_long(mut val: i64) -> BigInteger {
        let signum;
        if val < 0 {
            val = val.wrapping_neg();
            signum = -1;
        } else {
            signum = 1;
        }
        let high_word = ((val as u64) >> 32) as i32;
        let mut mag = Vec::new();
        if high_word == 0 {
            mag.push(val as i32);
        } else {
            mag.push(high_word);
            mag.push(val as i32);
        }
        BigInteger::new_from_internal(mag, signum)
    }

    /// Java: static BigInteger ZERO.
    pub(crate) fn zero() -> BigInteger {
        ZERO.clone()
    }

    /// Java: static BigInteger ONE.
    pub(crate) fn one() -> BigInteger {
        ONE.clone()
    }

    /// Java: static BigInteger TWO.
    pub(crate) fn two() -> BigInteger {
        TWO.clone()
    }

    /// Java: static BigInteger TEN.
    pub(crate) fn ten() -> BigInteger {
        TEN.clone()
    }

    /// Java: static BigInteger NEGATIVE_ONE.
    pub(crate) fn negative_one() -> BigInteger {
        NEGATIVE_ONE.clone()
    }

    /// Java: private void checkRange().
    fn check_range(&self) {
        if (self.mag.len() as i32) > MAX_MAG_LENGTH
            || (self.mag.len() as i32 == MAX_MAG_LENGTH && self.mag[0] < 0)
        {
            Self::report_overflow();
        }
    }

    /// Java: private static void reportOverflow().
    fn report_overflow() -> ! {
        panic!("ArithmeticException: BigInteger would overflow supported range");
    }

    /// Java: private static byte[] randomBits(int numBits, Random rnd).
    fn random_bits(num_bits: i32, rnd: &mut Random) -> Vec<u8> {
        if num_bits < 0 {
            panic!("IllegalArgumentException: numBits must be non-negative");
        }
        let num_bytes = ((num_bits as i64 + 7) / 8) as i32;
        let mut random_bits = vec![0u8; num_bytes as usize];
        if num_bytes > 0 {
            rnd.next_bytes(&mut random_bits);
            let excess_bits = 8 * num_bytes - num_bits;
            random_bits[0] &= ((1i32 << (8 - excess_bits)) - 1) as u8;
        }
        random_bits
    }

    /// Java: public static BigInteger bitLengthForInt(int n).
    pub(crate) fn bit_length_for_int(n: i32) -> i32 {
        32 - (n as u32).leading_zeros() as i32
    }

    /// Java: private int parseInt(char[] source, int start, int end).
    fn parse_int_digits(source: &[u16], start: usize, end: usize) -> i32 {
        let mut result = crate::digit_helper::digit(source[start], 10);
        if result == -1 {
            panic!("NumberFormatException: {}", String::from_utf16_lossy(source));
        }
        for index in start + 1..end {
            let next_val = crate::digit_helper::digit(source[index], 10);
            if next_val == -1 {
                panic!("NumberFormatException: {}", String::from_utf16_lossy(source));
            }
            result = 10 * result + next_val;
        }
        result
    }

    /// Java: private static void destructiveMulAdd(int[] x, int y, int z).
    fn destructive_mul_add(x: &mut [i32], y: i32, z: i32) {
        let ylong = y as i64 & LONG_MASK;
        let zlong = z as i64 & LONG_MASK;
        let len = x.len();
        let mut carry: i64 = 0;
        let mut i = len as isize - 1;
        while i >= 0 {
            let product = ylong.wrapping_mul(x[i as usize] as i64 & LONG_MASK).wrapping_add(carry);
            x[i as usize] = product as i32;
            carry = ((product as u64) >> 32) as i64;
            i -= 1;
        }
        let mut sum = (x[len - 1] as i64 & LONG_MASK) + zlong;
        x[len - 1] = sum as i32;
        carry = ((sum as u64) >> 32) as i64;
        let mut i = len as isize - 2;
        while i >= 0 {
            sum = (x[i as usize] as i64 & LONG_MASK) + carry;
            x[i as usize] = sum as i32;
            carry = ((sum as u64) >> 32) as i64;
            i -= 1;
        }
    }

    /// Java: public BigInteger(byte[] val, int off, int len).
    pub(crate) fn from_byte_array_subarray(val: &[u8], off: usize, len: usize) -> BigInteger {
        if val.is_empty() {
            panic!("NumberFormatException: Zero length BigInteger");
        }
        crate::big_integer::check_from_index_size(off, len, val.len());
        if len == 0 {
            return ZERO.clone();
        }
        let b = val[off] as i8;
        if b < 0 {
            let mag = Self::make_positive_byte(b as i32, val, off, len);
            BigInteger::new_from_internal(mag, -1)
        } else {
            let mag = Self::strip_leading_zero_bytes(b as i32, val, off, len);
            let signum = if mag.is_empty() { 0 } else { 1 };
            BigInteger::new_from_internal(mag, signum)
        }
    }

    /// Java: public BigInteger(byte[] val).
    pub(crate) fn from_byte_array(val: &[u8]) -> BigInteger {
        BigInteger::from_byte_array_subarray(val, 0, val.len())
    }

    /// Java: private BigInteger(int[] val).
    pub(crate) fn from_int_array_twos_complement(val: Vec<i32>) -> BigInteger {
        if val.is_empty() {
            panic!("NumberFormatException: Zero length BigInteger");
        }
        if val[0] < 0 {
            let mag = Self::make_positive_int(&val);
            BigInteger::new_from_internal(mag, -1)
        } else {
            let mag = Self::trusted_strip_leading_zero_ints(val);
            let signum = if mag.is_empty() { 0 } else { 1 };
            BigInteger::new_from_internal(mag, signum)
        }
    }

    /// Java: public BigInteger(int signum, byte[] magnitude, int off, int len).
    pub(crate) fn from_signum_magnitude_bytes(
        signum: i32,
        magnitude: &[u8],
        off: usize,
        len: usize,
    ) -> BigInteger {
        if !(-1..=1).contains(&signum) {
            panic!("NumberFormatException: Invalid signum value");
        }
        crate::big_integer::check_from_index_size(off, len, magnitude.len());
        let mag = Self::strip_leading_zero_bytes(-129, magnitude, off, len);
        let signum = if mag.is_empty() {
            0
        } else {
            if signum == 0 {
                panic!("NumberFormatException: signum-magnitude mismatch");
            }
            signum
        };
        BigInteger::new_from_internal(mag, signum)
    }

    /// Java: public BigInteger(int signum, byte[] magnitude).
    pub(crate) fn from_signum_magnitude_bytes_full(
        signum: i32,
        magnitude: &[u8],
    ) -> BigInteger {
        BigInteger::from_signum_magnitude_bytes(signum, magnitude, 0, magnitude.len())
    }

    /// Java: private BigInteger(int signum, int[] magnitude).
    pub(crate) fn from_signum_magnitude_ints(
        signum: i32,
        magnitude: Vec<i32>,
    ) -> BigInteger {
        let mag = Self::strip_leading_zero_ints(magnitude);
        if !(-1..=1).contains(&signum) {
            panic!("NumberFormatException: Invalid signum value");
        }
        let signum = if mag.is_empty() {
            0
        } else {
            if signum == 0 {
                panic!("NumberFormatException: signum-magnitude mismatch");
            }
            signum
        };
        BigInteger::new_from_internal(mag, signum)
    }

    /// Java: private BigInteger(byte[] magnitude, int signum).
    pub(crate) fn from_bytes_signum(magnitude: &[u8], signum: i32) -> BigInteger {
        let signum = if magnitude.is_empty() { 0 } else { signum };
        let mag = Self::strip_leading_zero_bytes(-129, magnitude, 0, magnitude.len());
        BigInteger::new_from_internal(mag, signum)
    }

    /// Java: public BigInteger(String val, int radix).
    pub(crate) fn from_string_radix(val: &str, radix: i32) -> BigInteger {
        let chars: Vec<u16> = val.encode_utf16().collect();
        let len = chars.len() as i32;
        let mut cursor: i32 = 0;
        if radix < 2 || radix > 36 {
            panic!("NumberFormatException: Radix out of range");
        }
        if len == 0 {
            panic!("NumberFormatException: Zero length BigInteger");
        }
        let mut sign = 1;
        let index1 = val.rfind('-').map(|index| index as i32);
        let index2 = val.rfind('+').map(|index| index as i32);
        if let Some(index1) = index1 {
            if index1 != 0 || index2.is_some() {
                panic!("NumberFormatException: Illegal embedded sign character");
            }
            sign = -1;
            cursor = 1;
        } else if let Some(index2) = index2 {
            if index2 != 0 {
                panic!("NumberFormatException: Illegal embedded sign character");
            }
            cursor = 1;
        }
        if cursor == len {
            panic!("NumberFormatException: Zero length BigInteger");
        }
        while cursor < len
            && crate::digit_helper::digit(chars[cursor as usize], radix) == 0
        {
            cursor += 1;
        }
        if cursor == len {
            return ZERO.clone();
        }
        let num_digits = len - cursor;
        let signum = sign;
        let num_bits = ((num_digits as i64 * BITS_PER_DIGIT[radix as usize]) >> 10) + 1;
        if num_bits + 31 >= (1i64 << 32) {
            Self::report_overflow();
        }
        let num_words = ((num_bits + 31) as u32 >> 5) as i32;
        let mut magnitude = vec![0i32; num_words as usize];
        let mut first_group_len = num_digits % DIGITS_PER_INT[radix as usize];
        if first_group_len == 0 {
            first_group_len = DIGITS_PER_INT[radix as usize];
        }
        let group_start = cursor;
        cursor += first_group_len;
        let group = &chars[group_start as usize..cursor as usize];
        let group_str: String = String::from_utf16_lossy(group);
        magnitude[(num_words - 1) as usize] = parse_int_str(&group_str, radix);
        if magnitude[(num_words - 1) as usize] < 0 {
            panic!("NumberFormatException: Illegal digit");
        }
        let super_radix = INT_RADIX[radix as usize];
        while cursor < len {
            let group_start = cursor;
            cursor += DIGITS_PER_INT[radix as usize];
            let group = &chars[group_start as usize..cursor as usize];
            let group_str: String = String::from_utf16_lossy(group);
            let group_val = parse_int_str(&group_str, radix);
            if group_val < 0 {
                panic!("NumberFormatException: Illegal digit");
            }
            Self::destructive_mul_add(&mut magnitude, super_radix, group_val);
        }
        let mag = Self::trusted_strip_leading_zero_ints(magnitude);
        BigInteger::new_from_internal(mag, signum)
    }

    /// Java: BigInteger(char[] val, int sign, int len) — the decimal-only
    /// constructor used by BigDecimal.
    pub(crate) fn from_char_array_decimal(val: &[u16], sign: i32, len: i32) -> BigInteger {
        let mut cursor: i32 = 0;
        while cursor < len && crate::digit_helper::digit(val[cursor as usize], 10) == 0 {
            cursor += 1;
        }
        if cursor == len {
            return ZERO.clone();
        }
        let num_digits = len - cursor;
        let signum = sign;
        let num_words = if len < 10 {
            1
        } else {
            let num_bits = ((num_digits as i64 * BITS_PER_DIGIT[10]) >> 10) + 1;
            if num_bits + 31 >= (1i64 << 32) {
                Self::report_overflow();
            }
            ((num_bits + 31) as u32 >> 5) as i32
        };
        let mut magnitude = vec![0i32; num_words as usize];
        let mut first_group_len = num_digits % DIGITS_PER_INT[10];
        if first_group_len == 0 {
            first_group_len = DIGITS_PER_INT[10];
        }
        let start = cursor as usize;
        cursor += first_group_len;
        magnitude[(num_words - 1) as usize] =
            Self::parse_int_digits(val, start, cursor as usize);
        while cursor < len {
            let start = cursor as usize;
            cursor += DIGITS_PER_INT[10];
            let group_val = Self::parse_int_digits(val, start, cursor as usize);
            Self::destructive_mul_add(&mut magnitude, INT_RADIX[10], group_val);
        }
        let mag = Self::trusted_strip_leading_zero_ints(magnitude);
        BigInteger::new_from_internal(mag, signum)
    }

    /// Java: public BigInteger(String val).
    pub(crate) fn from_string(val: &str) -> BigInteger {
        BigInteger::from_string_radix(val, 10)
    }

    /// Java: public BigInteger(int numBits, Random rnd).
    pub(crate) fn from_random_bits(num_bits: i32, rnd: &mut Random) -> BigInteger {
        let magnitude = Self::random_bits(num_bits, rnd);
        let mag = Self::strip_leading_zero_bytes(-129, &magnitude, 0, magnitude.len());
        let signum = if mag.is_empty() { 0 } else { 1 };
        BigInteger::new_from_internal(mag, signum)
    }

    /// Java: public BigInteger(int bitLength, int certainty, Random rnd).
    pub(crate) fn from_prime(bit_length: i32, certainty: i32, rnd: &mut Random) -> BigInteger {
        if bit_length < 2 {
            panic!("ArithmeticException: bitLength < 2");
        }
        let prime = if bit_length < SMALL_PRIME_THRESHOLD {
            Self::small_prime(bit_length, certainty, rnd)
        } else {
            Self::large_prime(bit_length, certainty, rnd)
        };
        BigInteger::new_from_internal(prime.mag.clone(), 1)
    }

    /// Java: public static BigInteger probablePrime(int bitLength, Random rnd).
    pub(crate) fn probable_prime(bit_length: i32, rnd: &mut Random) -> BigInteger {
        if bit_length < 2 {
            panic!("ArithmeticException: bitLength < 2");
        }
        if bit_length < SMALL_PRIME_THRESHOLD {
            Self::small_prime(bit_length, DEFAULT_PRIME_CERTAINTY, rnd)
        } else {
            Self::large_prime(bit_length, DEFAULT_PRIME_CERTAINTY, rnd)
        }
    }

    /// Java: private static BigInteger smallPrime(int bitLength, int certainty,
    /// Random rnd).
    fn small_prime(bit_length: i32, certainty: i32, rnd: &mut Random) -> BigInteger {
        let mag_len = (bit_length + 31) >> 5;
        let mut temp = vec![0i32; mag_len as usize];
        let high_bit = 1i32 << ((bit_length + 31) & 0x1f);
        let high_mask = (high_bit << 1) - 1;
        loop {
            for item in temp.iter_mut() {
                *item = rnd.next_int();
            }
            temp[0] = (temp[0] & high_mask) | high_bit;
            if bit_length > 2 {
                temp[(mag_len - 1) as usize] |= 1;
            }
            let p = BigInteger::new_from_internal(temp.clone(), 1);
            if bit_length > 6 {
                let r = p
                    .remainder(&SMALL_PRIME_PRODUCT)
                    .long_value();
                if r % 3 == 0
                    || r % 5 == 0
                    || r % 7 == 0
                    || r % 11 == 0
                    || r % 13 == 0
                    || r % 17 == 0
                    || r % 19 == 0
                    || r % 23 == 0
                    || r % 29 == 0
                    || r % 31 == 0
                    || r % 37 == 0
                    || r % 41 == 0
                {
                    continue;
                }
            }
            if bit_length < 4 {
                return p;
            }
            if p.prime_to_certainty(certainty, Some(rnd)) {
                return p;
            }
        }
    }

    /// Java: private static BigInteger largePrime(int bitLength, int certainty,
    /// Random rnd).
    fn large_prime(bit_length: i32, certainty: i32, rnd: &mut Random) -> BigInteger {
        let mut p = BigInteger::from_random_bits(bit_length, rnd).set_bit(bit_length - 1);
        let last = p.mag.len() - 1;
        p.mag[last] &= 0xffff_fffeu32 as i32;
        let search_len = Self::get_prime_search_len(bit_length);
        let mut search_sieve = BitSieve::new(&p, search_len);
        let mut candidate = search_sieve.retrieve(&p, certainty, Some(rnd));
        while candidate.is_none()
            || candidate.as_ref().map(|c| c.bit_length()) != Some(bit_length)
        {
            p = p.add(&BigInteger::value_of_long(2 * search_len as i64));
            if p.bit_length() != bit_length {
                p = BigInteger::from_random_bits(bit_length, rnd).set_bit(bit_length - 1);
            }
            let last = p.mag.len() - 1;
            p.mag[last] &= 0xffff_fffeu32 as i32;
            search_sieve = BitSieve::new(&p, search_len);
            candidate = search_sieve.retrieve(&p, certainty, Some(rnd));
        }
        candidate.expect("candidate")
    }

    /// Java: public BigInteger nextProbablePrime().
    pub(crate) fn next_probable_prime(&self) -> BigInteger {
        if self.signum < 0 {
            panic!("ArithmeticException: start < 0: {}", self.to_debug_string());
        }
        if self.signum == 0 || self.equals(&ONE) {
            return TWO.clone();
        }
        let mut result = self.add(&ONE);
        if result.bit_length() < SMALL_PRIME_THRESHOLD {
            if !result.test_bit(0) {
                result = result.add(&ONE);
            }
            loop {
                if result.bit_length() > 6 {
                    let r = result.remainder(&SMALL_PRIME_PRODUCT).long_value();
                    if r % 3 == 0
                        || r % 5 == 0
                        || r % 7 == 0
                        || r % 11 == 0
                        || r % 13 == 0
                        || r % 17 == 0
                        || r % 19 == 0
                        || r % 23 == 0
                        || r % 29 == 0
                        || r % 31 == 0
                        || r % 37 == 0
                        || r % 41 == 0
                    {
                        result = result.add(&TWO);
                        continue;
                    }
                }
                if result.bit_length() < 4 {
                    return result;
                }
                if result.prime_to_certainty(DEFAULT_PRIME_CERTAINTY, None) {
                    return result;
                }
                result = result.add(&TWO);
            }
        }
        if result.test_bit(0) {
            result = result.subtract(&ONE);
        }
        let search_len = Self::get_prime_search_len(result.bit_length());
        loop {
            let mut search_sieve = BitSieve::new(&result, search_len);
            let candidate = search_sieve.retrieve(&result, DEFAULT_PRIME_CERTAINTY, None);
            if let Some(candidate) = candidate {
                return candidate;
            }
            result = result.add(&BigInteger::value_of_long(2 * search_len as i64));
        }
    }

    /// Java: private static int getPrimeSearchLen(int bitLength).
    fn get_prime_search_len(bit_length: i32) -> i32 {
        if bit_length > PRIME_SEARCH_BIT_LENGTH_LIMIT + 1 {
            panic!("ArithmeticException: Prime search implementation restriction on bitLength");
        }
        bit_length / 20 * 64
    }

    /// Java: boolean primeToCertainty(int certainty, Random random).
    fn prime_to_certainty(&self, certainty: i32, mut random: Option<&mut Random>) -> bool {
        let n = (std::cmp::min(certainty, i32::MAX - 1) + 1) / 2;
        let size_in_bits = self.bit_length();
        let mut rounds;
        if size_in_bits < 100 {
            rounds = 50;
            rounds = if n < rounds { n } else { rounds };
            return self.passes_miller_rabin(rounds, random.as_deref_mut());
        }
        if size_in_bits < 256 {
            rounds = 27;
        } else if size_in_bits < 512 {
            rounds = 15;
        } else if size_in_bits < 768 {
            rounds = 8;
        } else if size_in_bits < 1024 {
            rounds = 4;
        } else {
            rounds = 2;
        }
        rounds = if n < rounds { n } else { rounds };
        self.passes_miller_rabin(rounds, random) && self.passes_lucas_lehmer()
    }

    /// Java: private boolean passesLucasLehmer().
    fn passes_lucas_lehmer(&self) -> bool {
        let this_plus_one = self.add(&ONE);
        let mut d = 5;
        while Self::jacobi_symbol(d, self) != -1 {
            d = if d < 0 { d.abs() + 2 } else { -(d + 2) };
        }
        let u = Self::lucas_lehmer_sequence(d, &this_plus_one, self);
        u.modulo(self).equals(&ZERO)
    }

    /// Java: private static int jacobiSymbol(int p, BigInteger n).
    fn jacobi_symbol(p: i32, n: &BigInteger) -> i32 {
        if p == 0 {
            return 0;
        }
        let mut j = 1;
        let mut u = n.mag[n.mag.len() - 1];
        let mut p = p;
        if p < 0 {
            p = -p;
            let n8 = u & 7;
            if n8 == 3 || n8 == 7 {
                j = -j;
            }
        }
        while (p & 3) == 0 {
            p >>= 2;
        }
        if (p & 1) == 0 {
            p >>= 1;
            if ((u ^ (u >> 1)) & 2) != 0 {
                j = -j;
            }
        }
        if p == 1 {
            return j;
        }
        if (p & u & 2) != 0 {
            j = -j;
        }
        u = n.modulo(&BigInteger::value_of_long(p as i64)).int_value();
        while u != 0 {
            while (u & 3) == 0 {
                u >>= 2;
            }
            if (u & 1) == 0 {
                u >>= 1;
                if ((p ^ (p >> 1)) & 2) != 0 {
                    j = -j;
                }
            }
            if u == 1 {
                return j;
            }
            assert!(u < p);
            let t = u;
            u = p;
            p = t;
            if (u & p & 2) != 0 {
                j = -j;
            }
            u %= p;
        }
        0
    }

    /// Java: private static BigInteger lucasLehmerSequence(int z, BigInteger k,
    /// BigInteger n).
    fn lucas_lehmer_sequence(z: i32, k: &BigInteger, n: &BigInteger) -> BigInteger {
        let d = BigInteger::value_of_long(z as i64);
        let mut u = ONE.clone();
        let mut v = ONE.clone();
        let mut i = k.bit_length() - 2;
        while i >= 0 {
            let mut u2 = u.multiply(&v).modulo(n);
            let mut v2 = v
                .square()
                .add(&d.multiply(&u.square()))
                .modulo(n);
            if v2.test_bit(0) {
                v2 = v2.subtract(n);
            }
            v2 = v2.shift_right(1);
            u = u2;
            v = v2;
            if k.test_bit(i) {
                u2 = u.add(&v).modulo(n);
                if u2.test_bit(0) {
                    u2 = u2.subtract(n);
                }
                u2 = u2.shift_right(1);
                v2 = v.add(&d.multiply(&u)).modulo(n);
                if v2.test_bit(0) {
                    v2 = v2.subtract(n);
                }
                v2 = v2.shift_right(1);
                u = u2;
                v = v2;
            }
            i -= 1;
        }
        u
    }

    /// Java: private boolean passesMillerRabin(int iterations, Random rnd).
    fn passes_miller_rabin(&self, iterations: i32, mut rnd: Option<&mut Random>) -> bool {
        let this_minus_one = self.subtract(&ONE);
        let mut m = this_minus_one.clone();
        let a = m.get_lowest_set_bit();
        m = m.shift_right(a);
        let mut owned_random;
        let rnd: &mut Random = match rnd {
            Some(r) => r,
            None => {
                owned_random = crate::random::thread_local_random();
                &mut owned_random
            }
        };
        for _ in 0..iterations {
            let b;
            loop {
                let candidate = BigInteger::from_random_bits(self.bit_length(), rnd);
                if candidate.compare_to(&ONE) > 0 && candidate.compare_to(self) < 0 {
                    b = candidate;
                    break;
                }
            }
            let mut j = 0;
            let mut z = b.mod_pow(&m, self);
            while !((j == 0 && z.equals(&ONE)) || z.equals(&this_minus_one)) {
                j += 1;
                if (j - 1 > 0 && z.equals(&ONE)) || j == a {
                    return false;
                }
                z = z.mod_pow(&TWO, self);
            }
        }
        true
    }

    /// Java: public BigInteger add(BigInteger val).
    pub(crate) fn add(&self, val: &BigInteger) -> BigInteger {
        if val.signum == 0 {
            return self.clone();
        }
        if self.signum == 0 {
            return val.clone();
        }
        if val.signum == self.signum {
            return BigInteger::new_from_internal(
                Self::add_mags(&self.mag, &val.mag),
                self.signum,
            );
        }
        let cmp = self.compare_magnitude(val);
        if cmp == 0 {
            return ZERO.clone();
        }
        let mut result_mag = if cmp > 0 {
            Self::subtract_mags(&self.mag, &val.mag)
        } else {
            Self::subtract_mags(&val.mag, &self.mag)
        };
        result_mag = Self::trusted_strip_leading_zero_ints(result_mag);
        BigInteger::new_from_internal(result_mag, if cmp == self.signum { 1 } else { -1 })
    }

    /// Java: BigInteger add(long val).
    pub(crate) fn add_long(&self, val: i64) -> BigInteger {
        if val == 0 {
            return self.clone();
        }
        if self.signum == 0 {
            return BigInteger::value_of_long(val);
        }
        if (if val > 0 { 1 } else if val < 0 { -1 } else { 0 }) == self.signum {
            return BigInteger::new_from_internal(
                Self::add_mag_long(&self.mag, val.abs()),
                self.signum,
            );
        }
        let cmp = self.compare_magnitude_long(val);
        if cmp == 0 {
            return ZERO.clone();
        }
        let mut result_mag = if cmp > 0 {
            Self::subtract_mag_long(&self.mag, val.abs())
        } else {
            Self::subtract_long_little(val.abs(), &self.mag)
        };
        result_mag = Self::trusted_strip_leading_zero_ints(result_mag);
        BigInteger::new_from_internal(result_mag, if cmp == self.signum { 1 } else { -1 })
    }

    /// Java: private static int[] add(int[] x, long val).
    fn add_mag_long(x: &[i32], val: i64) -> Vec<i32> {
        let mut sum: i64;
        let mut x_index = x.len();
        let result: Vec<i32>;
        let high_word = ((val as u64) >> 32) as i32;
        if high_word == 0 {
            let mut res = vec![0i32; x_index];
            sum = (x[x_index - 1] as i64 & LONG_MASK) + val;
            res[x_index - 1] = sum as i32;
            x_index -= 1;
            result = Self::propagate_add_carry(x, res, x_index, sum);
        } else {
            if x_index == 1 {
                let mut res = vec![0i32; 2];
                sum = val + (x[0] as i64 & LONG_MASK);
                res[1] = sum as i32;
                res[0] = ((sum as u64) >> 32) as i32;
                return res;
            }
            let mut res = vec![0i32; x_index];
            x_index -= 1;
            sum = (x[x_index] as i64 & LONG_MASK) + (val & LONG_MASK);
            res[x_index] = sum as i32;
            x_index -= 1;
            sum = (x[x_index] as i64 & LONG_MASK) + (high_word as i64 & LONG_MASK) + ((sum as u64) >> 32) as i64;
            res[x_index] = sum as i32;
            result = Self::propagate_add_carry(x, res, x_index, sum);
        }
        result
    }

    fn propagate_add_carry(x: &[i32], mut result: Vec<i32>, mut x_index: usize, sum: i64) -> Vec<i32> {
        let mut carry = (sum as u64) >> 32 != 0;
        while x_index > 0 && carry {
            x_index -= 1;
            result[x_index] = x[x_index].wrapping_add(1);
            carry = result[x_index] == 0;
        }
        while x_index > 0 {
            x_index -= 1;
            result[x_index] = x[x_index];
        }
        if carry {
            let mut bigger = vec![0i32; result.len() + 1];
            bigger[1..].copy_from_slice(&result);
            bigger[0] = 1;
            return bigger;
        }
        result
    }

    /// Java: private static int[] add(int[] x, int[] y).
    fn add_mags(x: &[i32], y: &[i32]) -> Vec<i32> {
        let (x, y) = if x.len() < y.len() { (y, x) } else { (x, y) };
        let mut x_index = x.len();
        let mut y_index = y.len();
        let mut result = vec![0i32; x_index];
        let mut sum: i64 = 0;
        if y_index == 1 {
            x_index -= 1;
            sum = (x[x_index] as i64 & LONG_MASK) + (y[0] as i64 & LONG_MASK);
            result[x_index] = sum as i32;
        } else {
            while y_index > 0 {
                x_index -= 1;
                y_index -= 1;
                sum = (x[x_index] as i64 & LONG_MASK)
                    + (y[y_index] as i64 & LONG_MASK)
                    + ((sum as u64) >> 32) as i64;
                result[x_index] = sum as i32;
            }
        }
        Self::propagate_add_carry(x, result, x_index, sum)
    }

    /// Java: private static int[] subtract(long val, int[] little).
    fn subtract_long_little(val: i64, little: &[i32]) -> Vec<i32> {
        let high_word = ((val as u64) >> 32) as i32;
        if high_word == 0 {
            let mut result = vec![0i32; 1];
            result[0] = (val - (little[0] as i64 & LONG_MASK)) as i32;
            result
        } else {
            let mut result = vec![0i32; 2];
            if little.len() == 1 {
                let difference = ((val as i32) as i64 & LONG_MASK) - (little[0] as i64 & LONG_MASK);
                result[1] = difference as i32;
                let borrow = difference >> 32 != 0;
                result[0] = if borrow { high_word - 1 } else { high_word };
                result
            } else {
                let mut difference = ((val as i32) as i64 & LONG_MASK) - (little[1] as i64 & LONG_MASK);
                result[1] = difference as i32;
                difference = (high_word as i64 & LONG_MASK) - (little[0] as i64 & LONG_MASK) + (difference >> 32);
                result[0] = difference as i32;
                result
            }
        }
    }

    /// Java: private static int[] subtract(int[] big, long val).
    fn subtract_mag_long(big: &[i32], val: i64) -> Vec<i32> {
        let high_word = ((val as u64) >> 32) as i32;
        let mut big_index = big.len();
        let mut result = vec![0i32; big_index];
        let mut difference: i64;
        if high_word == 0 {
            big_index -= 1;
            difference = (big[big_index] as i64 & LONG_MASK) - val;
            result[big_index] = difference as i32;
        } else {
            big_index -= 1;
            difference = (big[big_index] as i64 & LONG_MASK) - (val & LONG_MASK);
            result[big_index] = difference as i32;
            big_index -= 1;
            difference = (big[big_index] as i64 & LONG_MASK) - (high_word as i64 & LONG_MASK) + (difference >> 32);
            result[big_index] = difference as i32;
        }
        Self::propagate_sub_borrow(big, result, big_index, difference)
    }

    fn propagate_sub_borrow(big: &[i32], mut result: Vec<i32>, mut big_index: usize, difference: i64) -> Vec<i32> {
        let mut borrow = difference >> 32 != 0;
        while big_index > 0 && borrow {
            big_index -= 1;
            result[big_index] = big[big_index].wrapping_sub(1);
            borrow = result[big_index] == -1;
        }
        while big_index > 0 {
            big_index -= 1;
            result[big_index] = big[big_index];
        }
        result
    }

    /// Java: public BigInteger subtract(BigInteger val).
    pub(crate) fn subtract(&self, val: &BigInteger) -> BigInteger {
        if val.signum == 0 {
            return self.clone();
        }
        if self.signum == 0 {
            return val.negate();
        }
        if val.signum != self.signum {
            return BigInteger::new_from_internal(Self::add_mags(&self.mag, &val.mag), self.signum);
        }
        let cmp = self.compare_magnitude(val);
        if cmp == 0 {
            return ZERO.clone();
        }
        let mut result_mag = if cmp > 0 {
            Self::subtract_mags(&self.mag, &val.mag)
        } else {
            Self::subtract_mags(&val.mag, &self.mag)
        };
        result_mag = Self::trusted_strip_leading_zero_ints(result_mag);
        BigInteger::new_from_internal(result_mag, if cmp == self.signum { 1 } else { -1 })
    }

    /// Java: private static int[] subtract(int[] big, int[] little).
    fn subtract_mags(big: &[i32], little: &[i32]) -> Vec<i32> {
        let mut big_index = big.len();
        let mut result = vec![0i32; big_index];
        let mut little_index = little.len();
        let mut difference: i64 = 0;
        while little_index > 0 {
            big_index -= 1;
            little_index -= 1;
            difference = (big[big_index] as i64 & LONG_MASK)
                - (little[little_index] as i64 & LONG_MASK)
                + (difference >> 32);
            result[big_index] = difference as i32;
        }
        Self::propagate_sub_borrow(big, result, big_index, difference)
    }

    /// Java: public BigInteger multiply(BigInteger val).
    pub(crate) fn multiply(&self, val: &BigInteger) -> BigInteger {
        self.multiply_recursive(val, false)
    }

    /// Java: private BigInteger multiply(BigInteger val, boolean isRecursion).
    fn multiply_recursive(&self, val: &BigInteger, is_recursion: bool) -> BigInteger {
        if val.signum == 0 || self.signum == 0 {
            return ZERO.clone();
        }
        let xlen = self.mag.len() as i32;
        if std::ptr::eq(self, val) && xlen > MULTIPLY_SQUARE_THRESHOLD {
            return self.square();
        }
        let ylen = val.mag.len() as i32;
        if xlen < KARATSUBA_THRESHOLD || ylen < KARATSUBA_THRESHOLD {
            let result_sign = if self.signum == val.signum { 1 } else { -1 };
            if val.mag.len() == 1 {
                return Self::multiply_by_int(&self.mag, val.mag[0], result_sign);
            }
            if self.mag.len() == 1 {
                return Self::multiply_by_int(&val.mag, self.mag[0], result_sign);
            }
            let mut result = Self::multiply_to_len(&self.mag, xlen, &val.mag, ylen, None);
            result = Self::trusted_strip_leading_zero_ints(result);
            return BigInteger::new_from_internal(result, result_sign);
        }
        if xlen < TOOM_COOK_THRESHOLD && ylen < TOOM_COOK_THRESHOLD {
            return Self::multiply_karatsuba(self, val);
        }
        if !is_recursion {
            if (Self::bit_length_of(&self.mag, self.mag.len() as i32) as i64
                + Self::bit_length_of(&val.mag, val.mag.len() as i32) as i64)
                > 32i64 * MAX_MAG_LENGTH as i64
            {
                Self::report_overflow();
            }
        }
        Self::multiply_toom_cook3(self, val)
    }

    /// Java: private static BigInteger multiplyByInt(int[] x, int y, int sign).
    fn multiply_by_int(x: &[i32], y: i32, sign: i32) -> BigInteger {
        if y.count_ones() == 1 {
            return BigInteger::new_from_internal(
                Self::left_shift_mag(x, y.trailing_zeros() as i32),
                sign,
            );
        }
        let xlen = x.len();
        let mut rmag = vec![0i32; xlen + 1];
        let mut carry: i64 = 0;
        let yl = y as i64 & LONG_MASK;
        let mut rstart = rmag.len();
        for i in (0..xlen).rev() {
            let product = (x[i] as i64 & LONG_MASK).wrapping_mul(yl).wrapping_add(carry);
            rstart -= 1;
            rmag[rstart] = product as i32;
            carry = ((product as u64) >> 32) as i64;
        }
        if carry == 0 {
            rmag = rmag[1..].to_vec();
        } else {
            rmag[rstart - 1] = carry as i32;
        }
        BigInteger::new_from_internal(rmag, sign)
    }

    /// Java: BigInteger multiply(long v).
    pub(crate) fn multiply_long(&self, v: i64) -> BigInteger {
        if v == 0 || self.signum == 0 {
            return ZERO.clone();
        }
        if v == INFLATED {
            return self.multiply(&BigInteger::value_of_long(v));
        }
        let rsign = if v > 0 { self.signum } else { -self.signum };
        let mut v = v;
        if v < 0 {
            v = -v;
        }
        let dh = ((v as u64) >> 32) as i64;
        let dl = v & LONG_MASK;
        let xlen = self.mag.len();
        let mut rmag = if dh == 0 {
            vec![0i32; xlen + 1]
        } else {
            vec![0i32; xlen + 2]
        };
        let mut carry: i64 = 0;
        let mut rstart = rmag.len();
        for i in (0..xlen).rev() {
            let product = (self.mag[i] as i64 & LONG_MASK).wrapping_mul(dl).wrapping_add(carry);
            rstart -= 1;
            rmag[rstart] = product as i32;
            carry = ((product as u64) >> 32) as i64;
        }
        rstart -= 1;
        rmag[rstart] = carry as i32;
        if dh != 0 {
            carry = 0;
            rstart = rmag.len() - 1;
            for i in (0..xlen).rev() {
                let product = (self.mag[i] as i64 & LONG_MASK)
                    .wrapping_mul(dh)
                    .wrapping_add(rmag[rstart] as i64 & LONG_MASK)
                    .wrapping_add(carry);
                rmag[rstart] = product as i32;
                rstart -= 1;
                carry = ((product as u64) >> 32) as i64;
            }
            rmag[0] = carry as i32;
        }
        if carry == 0 {
            rmag = rmag[1..].to_vec();
        }
        BigInteger::new_from_internal(rmag, rsign)
    }

    /// Java: private static int[] multiplyToLen(int[] x, int xlen, int[] y,
    /// int ylen, int[] z).
    fn multiply_to_len(
        x: &[i32],
        xlen: i32,
        y: &[i32],
        ylen: i32,
        z: Option<Vec<i32>>,
    ) -> Vec<i32> {
        Self::multiply_to_len_check(x, xlen);
        Self::multiply_to_len_check(y, ylen);
        Self::impl_multiply_to_len(x, xlen, y, ylen, z)
    }

    fn multiply_to_len_check(array: &[i32], length: i32) {
        if length <= 0 {
            return;
        }
        if length > array.len() as i32 {
            panic!("ArrayIndexOutOfBoundsException: {}", length - 1);
        }
    }

    /// Java: private static int[] implMultiplyToLen(int[] x, int xlen, int[] y,
    /// int ylen, int[] z).
    fn impl_multiply_to_len(
        x: &[i32],
        xlen: i32,
        y: &[i32],
        ylen: i32,
        z: Option<Vec<i32>>,
    ) -> Vec<i32> {
        let xstart = xlen - 1;
        let ystart = ylen - 1;
        let mut z = match z {
            Some(z) if z.len() >= (xlen + ylen) as usize => z,
            _ => vec![0i32; (xlen + ylen) as usize],
        };
        let mut carry: i64 = 0;
        let mut j = ystart;
        let mut k = ystart + 1 + xstart;
        while j >= 0 {
            let product = (y[j as usize] as i64 & LONG_MASK)
                .wrapping_mul(x[xstart as usize] as i64 & LONG_MASK)
                .wrapping_add(carry);
            z[k as usize] = product as i32;
            carry = ((product as u64) >> 32) as i64;
            j -= 1;
            k -= 1;
        }
        z[xstart as usize] = carry as i32;
        let mut i = xstart - 1;
        while i >= 0 {
            carry = 0;
            let mut j = ystart;
            let mut k = ystart + 1 + i;
            while j >= 0 {
                let product = (y[j as usize] as i64 & LONG_MASK)
                    .wrapping_mul(x[i as usize] as i64 & LONG_MASK)
                    .wrapping_add(z[k as usize] as i64 & LONG_MASK)
                    .wrapping_add(carry);
                z[k as usize] = product as i32;
                carry = ((product as u64) >> 32) as i64;
                j -= 1;
                k -= 1;
            }
            z[i as usize] = carry as i32;
            i -= 1;
        }
        z
    }

    /// Java: private static BigInteger multiplyKaratsuba(BigInteger x,
    /// BigInteger y).
    fn multiply_karatsuba(x: &BigInteger, y: &BigInteger) -> BigInteger {
        let xlen = x.mag.len() as i32;
        let ylen = y.mag.len() as i32;
        let half = (std::cmp::max(xlen, ylen) + 1) / 2;
        let xl = x.get_lower(half);
        let xh = x.get_upper(half);
        let yl = y.get_lower(half);
        let yh = y.get_upper(half);
        let p1 = xh.multiply(&yh);
        let p2 = xl.multiply(&yl);
        let p3 = xh.add(&xl).multiply(&yh.add(&yl));
        let result = p1
            .shift_left(32 * half)
            .add(&p3.subtract(&p1).subtract(&p2))
            .shift_left(32 * half)
            .add(&p2);
        if x.signum != y.signum {
            result.negate()
        } else {
            result
        }
    }

    /// Java: private static BigInteger multiplyToomCook3(BigInteger a,
    /// BigInteger b).
    fn multiply_toom_cook3(a: &BigInteger, b: &BigInteger) -> BigInteger {
        let alen = a.mag.len() as i32;
        let blen = b.mag.len() as i32;
        let largest = std::cmp::max(alen, blen);
        let k = (largest + 2) / 3;
        let r = largest - 2 * k;
        let a2 = a.get_toom_slice(k, r, 0, largest);
        let a1 = a.get_toom_slice(k, r, 1, largest);
        let a0 = a.get_toom_slice(k, r, 2, largest);
        let b2 = b.get_toom_slice(k, r, 0, largest);
        let b1 = b.get_toom_slice(k, r, 1, largest);
        let b0 = b.get_toom_slice(k, r, 2, largest);
        let v0 = a0.multiply(&b0);
        let mut da1 = a2.add(&a0);
        let mut db1 = b2.add(&b0);
        let vm1 = da1.subtract(&a1).multiply(&db1.subtract(&b1));
        da1 = da1.add(&a1);
        db1 = db1.add(&b1);
        let v1 = da1.multiply(&db1);
        let v2 = da1
            .add(&a2)
            .shift_left(1)
            .subtract(&a0)
            .multiply(&db1.add(&b2).shift_left(1).subtract(&b0));
        let vinf = a2.multiply(&b2);
        let mut t2 = v2.subtract(&vm1).exact_divide_by3();
        let mut tm1 = v1.subtract(&vm1).shift_right(1);
        let mut t1 = v1.subtract(&v0);
        t2 = t2.subtract(&t1).shift_right(1);
        t1 = t1.subtract(&tm1).subtract(&vinf);
        t2 = t2.subtract(&vinf.shift_left(1));
        tm1 = tm1.subtract(&t2);
        let ss = k * 32;
        let result = vinf
            .shift_left(ss)
            .add(&t2)
            .shift_left(ss)
            .add(&t1)
            .shift_left(ss)
            .add(&tm1)
            .shift_left(ss)
            .add(&v0);
        if a.signum != b.signum {
            result.negate()
        } else {
            result
        }
    }

    /// Java: private BigInteger getToomSlice(int lowerSize, int upperSize,
    /// int slice, int fullsize).
    fn get_toom_slice(&self, lower_size: i32, upper_size: i32, slice: i32, fullsize: i32) -> BigInteger {
        let len = self.mag.len() as i32;
        let offset = fullsize - len;
        let (mut start, end);
        if slice == 0 {
            start = -offset;
            end = upper_size - 1 - offset;
        } else {
            start = upper_size + (slice - 1) * lower_size - offset;
            end = start + lower_size - 1;
        }
        if start < 0 {
            start = 0;
        }
        if end < 0 {
            return ZERO.clone();
        }
        let slice_size = end - start + 1;
        if slice_size <= 0 {
            return ZERO.clone();
        }
        if start == 0 && slice_size >= len {
            return self.abs();
        }
        let int_slice = self.mag[start as usize..(start + slice_size) as usize].to_vec();
        BigInteger::new_from_internal(Self::trusted_strip_leading_zero_ints(int_slice), 1)
    }

    /// Java: private BigInteger exactDivideBy3().
    fn exact_divide_by3(&self) -> BigInteger {
        let len = self.mag.len();
        let mut result = vec![0i32; len];
        let mut borrow: i64 = 0;
        let mut i = len as isize - 1;
        while i >= 0 {
            let x = self.mag[i as usize] as i64 & LONG_MASK;
            let w = x - borrow;
            if borrow > x {
                borrow = 1;
            } else {
                borrow = 0;
            }
            let q = (w.wrapping_mul(0xAAAAAAABu64 as i64)) & LONG_MASK;
            result[i as usize] = q as i32;
            if q >= 0x55555556 {
                borrow += 1;
                if q >= 0xAAAAAAAB {
                    borrow += 1;
                }
            }
            i -= 1;
        }
        let result = Self::trusted_strip_leading_zero_ints(result);
        BigInteger::new_from_internal(result, self.signum)
    }

    /// Java: private BigInteger getLower(int n).
    fn get_lower(&self, n: i32) -> BigInteger {
        let len = self.mag.len();
        if len as i32 <= n {
            return self.abs();
        }
        let lower_ints = self.mag[(len - n as usize)..].to_vec();
        BigInteger::new_from_internal(Self::trusted_strip_leading_zero_ints(lower_ints), 1)
    }

    /// Java: private BigInteger getUpper(int n).
    fn get_upper(&self, n: i32) -> BigInteger {
        let len = self.mag.len();
        if len as i32 <= n {
            return ZERO.clone();
        }
        let upper_len = len - n as usize;
        let upper_ints = self.mag[..upper_len].to_vec();
        BigInteger::new_from_internal(Self::trusted_strip_leading_zero_ints(upper_ints), 1)
    }

    /// Java: private BigInteger square().
    fn square(&self) -> BigInteger {
        self.square_with_recursion(false)
    }

    /// Java: private BigInteger square(boolean isRecursion).
    fn square_with_recursion(&self, is_recursion: bool) -> BigInteger {
        if self.signum == 0 {
            return ZERO.clone();
        }
        let len = self.mag.len() as i32;
        if len < KARATSUBA_SQUARE_THRESHOLD {
            let z = Self::square_to_len(&self.mag, len, None);
            return BigInteger::new_from_internal(Self::trusted_strip_leading_zero_ints(z), 1);
        }
        if len < TOOM_COOK_SQUARE_THRESHOLD {
            return self.square_karatsuba();
        }
        if !is_recursion {
            if Self::bit_length_of(&self.mag, self.mag.len() as i32) as i64 > 16i64 * MAX_MAG_LENGTH as i64
            {
                Self::report_overflow();
            }
        }
        self.square_toom_cook3()
    }

    /// Java: private static final int[] squareToLen(int[] x, int len, int[] z).
    fn square_to_len(x: &[i32], len: i32, z: Option<Vec<i32>>) -> Vec<i32> {
        let zlen = len << 1;
        let z = match z {
            Some(z) if z.len() >= zlen as usize => z,
            _ => vec![0i32; zlen as usize],
        };
        Self::impl_square_to_len_checks(x, len, &z, zlen);
        Self::impl_square_to_len(x, len, z, zlen)
    }

    /// Java: private static void implSquareToLenChecks(int[] x, int len,
    /// int[] z, int zlen).
    fn impl_square_to_len_checks(x: &[i32], len: i32, z: &[i32], zlen: i32) {
        if len < 1 {
            panic!("IllegalArgumentException: invalid input length: {len}");
        }
        if len > x.len() as i32 {
            panic!(
                "IllegalArgumentException: input length out of bound: {len} > {}",
                x.len()
            );
        }
        if len * 2 > z.len() as i32 {
            panic!(
                "IllegalArgumentException: input length out of bound: {} > {}",
                len * 2,
                z.len()
            );
        }
        if zlen < 1 {
            panic!("IllegalArgumentException: invalid input length: {zlen}");
        }
        if zlen > z.len() as i32 {
            panic!(
                "IllegalArgumentException: input length out of bound: {len} > {}",
                z.len()
            );
        }
    }

    /// Java: private static final int[] implSquareToLen(int[] x, int len,
    /// int[] z, int zlen).
    fn impl_square_to_len(x: &[i32], len: i32, mut z: Vec<i32>, zlen: i32) -> Vec<i32> {
        let mut last_product_low_word: i32 = 0;
        let mut j = 0;
        let mut i = 0;
        while j < len {
            let piece = x[j as usize] as i64 & LONG_MASK;
            let product = piece.wrapping_mul(piece);
            z[i as usize] = last_product_low_word.wrapping_shl(31) | ((product as u64 >> 33) as i32);
            i += 1;
            z[i as usize] = ((product as u64) >> 1) as i32;
            i += 1;
            last_product_low_word = product as i32;
            j += 1;
        }
        let mut i = len;
        let mut offset: i32 = 1;
        while i > 0 {
            let mut t = x[(i - 1) as usize];
            t = Self::mul_add(&mut z, x, offset, i - 1, t);
            Self::add_one(&mut z, offset - 1, i, t);
            i -= 1;
            offset += 2;
        }
        Self::primitive_left_shift_arr(&mut z, zlen, 1);
        let last = (zlen - 1) as usize;
        z[last] |= x[(len - 1) as usize] & 1;
        z
    }

    /// Java: private BigInteger squareKaratsuba().
    fn square_karatsuba(&self) -> BigInteger {
        let half = (self.mag.len() as i32 + 1) / 2;
        let xl = self.get_lower(half);
        let xh = self.get_upper(half);
        let xhs = xh.square_with_recursion(true);
        let xls = xl.square_with_recursion(true);
        xhs.shift_left(half * 32)
            .add(&xl.add(&xh).square_with_recursion(true).subtract(&xhs.add(&xls)))
            .shift_left(half * 32)
            .add(&xls)
    }

    /// Java: private BigInteger squareToomCook3().
    fn square_toom_cook3(&self) -> BigInteger {
        let len = self.mag.len() as i32;
        let k = (len + 2) / 3;
        let r = len - 2 * k;
        let a2 = self.get_toom_slice(k, r, 0, len);
        let a1 = self.get_toom_slice(k, r, 1, len);
        let a0 = self.get_toom_slice(k, r, 2, len);
        let v0 = a0.square_with_recursion(true);
        let mut da1 = a2.add(&a0);
        let vm1 = da1.subtract(&a1).square_with_recursion(true);
        da1 = da1.add(&a1);
        let v1 = da1.square_with_recursion(true);
        let vinf = a2.square_with_recursion(true);
        let v2 = da1.add(&a2).shift_left(1).subtract(&a0).square_with_recursion(true);
        let mut t2 = v2.subtract(&vm1).exact_divide_by3();
        let mut tm1 = v1.subtract(&vm1).shift_right(1);
        let mut t1 = v1.subtract(&v0);
        t2 = t2.subtract(&t1).shift_right(1);
        t1 = t1.subtract(&tm1).subtract(&vinf);
        t2 = t2.subtract(&vinf.shift_left(1));
        tm1 = tm1.subtract(&t2);
        let ss = k * 32;
        vinf
            .shift_left(ss)
            .add(&t2)
            .shift_left(ss)
            .add(&t1)
            .shift_left(ss)
            .add(&tm1)
            .shift_left(ss)
            .add(&v0)
    }

    /// Java: public BigInteger divide(BigInteger val).
    pub(crate) fn divide(&self, val: &BigInteger) -> BigInteger {
        if (val.mag.len() as i32) < BURNIKEL_ZIEGLER_THRESHOLD
            || self.mag.len() as i32 - (val.mag.len() as i32) < BURNIKEL_ZIEGLER_OFFSET
        {
            self.divide_knuth(val)
        } else {
            self.divide_burnikel_ziegler(val)
        }
    }

    /// Java: private BigInteger divideKnuth(BigInteger val).
    fn divide_knuth(&self, val: &BigInteger) -> BigInteger {
        let mut q = MutableBigInteger::new_empty();
        let mut a = MutableBigInteger::new_from_vec(self.mag.clone());
        let b = MutableBigInteger::new_from_vec(val.mag.clone());
        a.divide_knuth_top(&b, &mut q);
        q.to_big_integer_sign(self.signum * val.signum)
    }

    /// Java: public BigInteger[] divideAndRemainder(BigInteger val).
    pub(crate) fn divide_and_remainder(&self, val: &BigInteger) -> (BigInteger, BigInteger) {
        if (val.mag.len() as i32) < BURNIKEL_ZIEGLER_THRESHOLD
            || self.mag.len() as i32 - (val.mag.len() as i32) < BURNIKEL_ZIEGLER_OFFSET
        {
            self.divide_and_remainder_knuth(val)
        } else {
            self.divide_and_remainder_burnikel_ziegler(val)
        }
    }

    /// Java: private BigInteger[] divideAndRemainderKnuth(BigInteger val).
    fn divide_and_remainder_knuth(&self, val: &BigInteger) -> (BigInteger, BigInteger) {
        let mut q = MutableBigInteger::new_empty();
        let mut a = MutableBigInteger::new_from_vec(self.mag.clone());
        let b = MutableBigInteger::new_from_vec(val.mag.clone());
        let mut r = a.divide_knuth_top(&b, &mut q).expect("remainder");
        (
            q.to_big_integer_sign(if self.signum == val.signum { 1 } else { -1 }),
            r.to_big_integer_sign(self.signum),
        )
    }

    /// Java: public BigInteger remainder(BigInteger val).
    pub(crate) fn remainder(&self, val: &BigInteger) -> BigInteger {
        if (val.mag.len() as i32) < BURNIKEL_ZIEGLER_THRESHOLD
            || self.mag.len() as i32 - (val.mag.len() as i32) < BURNIKEL_ZIEGLER_OFFSET
        {
            self.remainder_knuth(val)
        } else {
            self.remainder_burnikel_ziegler(val)
        }
    }

    /// Java: private BigInteger remainderKnuth(BigInteger val).
    fn remainder_knuth(&self, val: &BigInteger) -> BigInteger {
        let mut q = MutableBigInteger::new_empty();
        let mut a = MutableBigInteger::new_from_vec(self.mag.clone());
        let b = MutableBigInteger::new_from_vec(val.mag.clone());
        a.divide_knuth_top(&b, &mut q)
            .expect("remainder")
            .to_big_integer_sign(self.signum)
    }

    /// Java: private BigInteger divideBurnikelZiegler(BigInteger val).
    fn divide_burnikel_ziegler(&self, val: &BigInteger) -> BigInteger {
        self.divide_and_remainder_burnikel_ziegler(val).0
    }

    /// Java: private BigInteger remainderBurnikelZiegler(BigInteger val).
    fn remainder_burnikel_ziegler(&self, val: &BigInteger) -> BigInteger {
        self.divide_and_remainder_burnikel_ziegler(val).1
    }

    /// Java: private BigInteger[] divideAndRemainderBurnikelZiegler(BigInteger val).
    fn divide_and_remainder_burnikel_ziegler(&self, val: &BigInteger) -> (BigInteger, BigInteger) {
        let mut q = MutableBigInteger::new_empty();
        let mut a = MutableBigInteger::new_from_big_integer(self);
        let b = MutableBigInteger::new_from_big_integer(val);
        let mut r = a.divide_and_remainder_burnikel_ziegler(&b, &mut q);
        let q_bi = if q.is_zero() {
            ZERO.clone()
        } else {
            q.to_big_integer_sign(self.signum * val.signum)
        };
        let r_bi = if r.is_zero() {
            ZERO.clone()
        } else {
            r.to_big_integer_sign(self.signum)
        };
        (q_bi, r_bi)
    }

    /// Java: public BigInteger pow(int exponent).
    pub(crate) fn pow(&self, exponent: i32) -> BigInteger {
        if exponent < 0 {
            panic!("ArithmeticException: Negative exponent");
        }
        if self.signum == 0 {
            return if exponent == 0 { ONE.clone() } else { self.clone() };
        }
        let mut part_to_square = self.abs();
        let powers_of_two = part_to_square.get_lowest_set_bit();
        let bits_to_shift_long = powers_of_two as i64 * exponent as i64;
        if bits_to_shift_long > i32::MAX as i64 {
            Self::report_overflow();
        }
        let bits_to_shift = bits_to_shift_long as i32;
        let remaining_bits;
        if powers_of_two > 0 {
            part_to_square = part_to_square.shift_right(powers_of_two);
            remaining_bits = part_to_square.bit_length();
            if remaining_bits == 1 {
                if self.signum < 0 && (exponent & 1) == 1 {
                    return NEGATIVE_ONE.shift_left(bits_to_shift);
                }
                return ONE.shift_left(bits_to_shift);
            }
        } else {
            remaining_bits = part_to_square.bit_length();
            if remaining_bits == 1 {
                if self.signum < 0 && (exponent & 1) == 1 {
                    return NEGATIVE_ONE.clone();
                }
                return ONE.clone();
            }
        }
        let scale_factor = remaining_bits as i64 * exponent as i64;
        if part_to_square.mag.len() == 1 && scale_factor <= 62 {
            let new_sign = if self.signum < 0 && (exponent & 1) == 1 {
                -1
            } else {
                1
            };
            let mut result: i64 = 1;
            let mut base_to_pow2 = part_to_square.mag[0] as i64 & LONG_MASK;
            let mut working_exponent = exponent;
            while working_exponent != 0 {
                if (working_exponent & 1) == 1 {
                    result = result.wrapping_mul(base_to_pow2);
                }
                working_exponent = ((working_exponent as u32) >> 1) as i32;
                if working_exponent != 0 {
                    base_to_pow2 = base_to_pow2.wrapping_mul(base_to_pow2);
                }
            }
            if powers_of_two > 0 {
                if bits_to_shift as i64 + scale_factor <= 62 {
                    return BigInteger::value_of_long(result.wrapping_shl(bits_to_shift as u32) * new_sign);
                }
                return BigInteger::value_of_long(result * new_sign).shift_left(bits_to_shift);
            }
            return BigInteger::value_of_long(result * new_sign);
        }
        if self.bit_length() as i64 * exponent as i64 / 32 > MAX_MAG_LENGTH as i64 {
            Self::report_overflow();
        }
        let mut answer = ONE.clone();
        let mut working_exponent = exponent;
        while working_exponent != 0 {
            if (working_exponent & 1) == 1 {
                answer = answer.multiply(&part_to_square);
            }
            working_exponent = ((working_exponent as u32) >> 1) as i32;
            if working_exponent != 0 {
                part_to_square = part_to_square.square_with_recursion(true);
            }
        }
        if powers_of_two > 0 {
            answer = answer.shift_left(bits_to_shift);
        }
        if self.signum < 0 && (exponent & 1) == 1 {
            answer.negate()
        } else {
            answer
        }
    }

    /// Java: public BigInteger sqrt().
    pub(crate) fn sqrt(&self) -> BigInteger {
        if self.signum < 0 {
            panic!("ArithmeticException: Negative BigInteger");
        }
        MutableBigInteger::new_from_big_integer(self).sqrt_public().to_big_integer()
    }

    /// Java: public BigInteger[] sqrtAndRemainder().
    pub(crate) fn sqrt_and_remainder(&self) -> (BigInteger, BigInteger) {
        let s = self.sqrt();
        let r = self.subtract(&s.square_with_recursion(false));
        assert!(r.compare_to(&ZERO) >= 0);
        (s, r)
    }

    /// Java: public BigInteger gcd(BigInteger val).
    pub(crate) fn gcd(&self, val: &BigInteger) -> BigInteger {
        if val.signum == 0 {
            return self.abs();
        }
        if self.signum == 0 {
            return val.abs();
        }
        let mut a = MutableBigInteger::new_from_big_integer(self);
        let b = MutableBigInteger::new_from_big_integer(val);
        let mut result = a.hybrid_gcd_public(&b);
        result.to_big_integer_sign(1)
    }

    /// Java: private static int[] leftShift(int[] a, int len, int n).
    fn left_shift_mag(a: &[i32], n: i32) -> Vec<i32> {
        let n_ints = ((n as u32) >> 5) as usize;
        let n_bits = n & 0x1F;
        let bits_in_high_word = Self::bit_length_for_int(a[0]);
        if n <= 32 - bits_in_high_word {
            let mut result = a.to_vec();
            Self::primitive_left_shift_arr(&mut result, 32, n_bits);
            return result;
        }
        if n_bits <= 32 - bits_in_high_word {
            let mut result = vec![0i32; n_ints + a.len()];
            result[..a.len()].copy_from_slice(a);
            let result_len = result.len() as i32;
            Self::primitive_left_shift_arr(&mut result, result_len, n_bits);
            result
        } else {
            let mut result = vec![0i32; n_ints + a.len() + 1];
            result[..a.len()].copy_from_slice(a);
            let result_len = result.len() as i32;
            Self::primitive_right_shift_arr(&mut result, result_len, 32 - n_bits);
            result
        }
    }

    /// Java: static void primitiveRightShift(int[] a, int len, int n).
    fn primitive_right_shift_arr(a: &mut [i32], len: i32, n: i32) {
        let num_iter = (len - 1) as usize;
        let new_idx = 1usize;
        let mut idx = num_iter;
        let mut nidx = if new_idx == 0 { num_iter - 1 } else { num_iter };
        while nidx >= new_idx {
            let low = a[idx];
            let high = a[idx - 1];
            a[nidx] = ((low as u32) >> n) as i32 | high.wrapping_shl((32 - n) as u32);
            if nidx == 0 {
                break;
            }
            nidx -= 1;
            idx -= 1;
        }
        a[0] = ((a[0] as u32) >> n) as i32;
    }

    /// Java: static void primitiveLeftShift(int[] a, int len, int n).
    fn primitive_left_shift_arr(a: &mut [i32], len: i32, n: i32) {
        if len == 0 || n == 0 {
            return;
        }
        let num_iter = (len - 1) as usize;
        let mut old_idx = 0usize;
        let mut new_idx = 0usize;
        while old_idx < num_iter {
            let low = a[old_idx];
            let high = a[old_idx + 1];
            a[new_idx] = low.wrapping_shl(n as u32) | ((high as u32) >> (32 - n)) as i32;
            old_idx += 1;
            new_idx += 1;
        }
        let last = (len - 1) as usize;
        a[last] = a[last].wrapping_shl(n as u32);
    }

    /// Java: private static int bitLength(int[] val, int len).
    fn bit_length_of(val: &[i32], len: i32) -> i32 {
        if len == 0 {
            return 0;
        }
        ((len - 1) << 5) + Self::bit_length_for_int(val[0])
    }

    /// Java: public BigInteger abs().
    pub(crate) fn abs(&self) -> BigInteger {
        if self.signum >= 0 {
            self.clone()
        } else {
            self.negate()
        }
    }

    /// Java: public BigInteger negate().
    pub(crate) fn negate(&self) -> BigInteger {
        BigInteger::new_from_internal(self.mag.clone(), -self.signum)
    }

    /// Java: public int signum().
    pub(crate) fn signum(&self) -> i32 {
        self.signum
    }

    /// Java: public BigInteger mod(BigInteger m).
    pub(crate) fn modulo(&self, m: &BigInteger) -> BigInteger {
        if m.signum <= 0 {
            panic!("ArithmeticException: BigInteger: modulus not positive");
        }
        let result = self.remainder(m);
        if result.signum >= 0 {
            result
        } else {
            result.add(m)
        }
    }

    /// Java: public BigInteger modPow(BigInteger exponent, BigInteger m).
    pub(crate) fn mod_pow(&self, exponent: &BigInteger, m: &BigInteger) -> BigInteger {
        if m.signum <= 0 {
            panic!("ArithmeticException: BigInteger: modulus not positive");
        }
        if exponent.signum == 0 {
            return if m.equals(&ONE) { ZERO.clone() } else { ONE.clone() };
        }
        if self.equals(&ONE) {
            return if m.equals(&ONE) { ZERO.clone() } else { ONE.clone() };
        }
        if self.equals(&ZERO) && exponent.signum >= 0 {
            return ZERO.clone();
        }
        if self.equals(&NEGATIVE_ONE) && !exponent.test_bit(0) {
            return if m.equals(&ONE) { ZERO.clone() } else { ONE.clone() };
        }
        let invert_result = exponent.signum < 0;
        let exponent = if invert_result { exponent.negate() } else { exponent.clone() };
        let base = if self.signum < 0 || self.compare_to(m) >= 0 {
            self.modulo(m)
        } else {
            self.clone()
        };
        let result;
        if m.test_bit(0) {
            result = base.odd_mod_pow(&exponent, m);
        } else {
            let p = m.get_lowest_set_bit();
            let m1 = m.shift_right(p);
            let m2 = ONE.shift_left(p);
            let base2 = if self.signum < 0 || self.compare_to(&m1) >= 0 {
                self.modulo(&m1)
            } else {
                self.clone()
            };
            let a1 = if m1.equals(&ONE) {
                ZERO.clone()
            } else {
                base2.odd_mod_pow(&exponent, &m1)
            };
            let a2 = base.mod_pow2(&exponent, p);
            let y1 = m2.mod_inverse(&m1);
            let y2 = m1.mod_inverse(&m2);
            if (m.mag.len() as i32) < MAX_MAG_LENGTH / 2 {
                result = a1
                    .multiply(&m2)
                    .multiply(&y1)
                    .add(&a2.multiply(&m1).multiply(&y2))
                    .modulo(m);
            } else {
                let mut t1 = MutableBigInteger::new_empty();
                let mut left = MutableBigInteger::new_from_big_integer(&a1.multiply(&m2));
                left.multiply_public(&MutableBigInteger::new_from_big_integer(&y1), &mut t1);
                let mut t2 = MutableBigInteger::new_empty();
                let mut left2 = MutableBigInteger::new_from_big_integer(&a2.multiply(&m1));
                left2.multiply_public(&MutableBigInteger::new_from_big_integer(&y2), &mut t2);
                t1.add_public(&t2);
                let mut q = MutableBigInteger::new_empty();
                result = t1
                    .divide_public(&MutableBigInteger::new_from_big_integer(m), &mut q)
                    .expect("remainder")
                    .to_big_integer_sign(1);
            }
        }
        if invert_result {
            result.mod_inverse(m)
        } else {
            result
        }
    }

    /// Java: private static int[] montgomeryMultiply(int[] a, int[] b, int[] n,
    /// int len, long inv, int[] product).
    fn montgomery_multiply(
        a: &[i32],
        b: &[i32],
        n: &[i32],
        len: i32,
        inv: i64,
        product: Option<Vec<i32>>,
    ) -> Vec<i32> {
        Self::impl_montgomery_multiply_checks(a, b, n, len, product.as_ref());
        if len > MONTGOMERY_INTRINSIC_THRESHOLD {
            let product = Self::multiply_to_len(a, len, b, len, product);
            return Self::mont_reduce(product, n, len, inv as i32);
        }
        Self::impl_montgomery_multiply(a, b, n, len, inv, Self::materialize(product, len))
    }

    /// Java: private static int[] montgomerySquare(int[] a, int[] n, int len,
    /// long inv, int[] product).
    fn montgomery_square(
        a: &[i32],
        n: &[i32],
        len: i32,
        inv: i64,
        product: Option<Vec<i32>>,
    ) -> Vec<i32> {
        Self::impl_montgomery_multiply_checks(a, a, n, len, product.as_ref());
        if len > MONTGOMERY_INTRINSIC_THRESHOLD {
            let product = Self::square_to_len(a, len, product);
            return Self::mont_reduce(product, n, len, inv as i32);
        }
        Self::impl_montgomery_square(a, n, len, inv, Self::materialize(product, len))
    }

    /// Java: private static void implMontgomeryMultiplyChecks(...).
    fn impl_montgomery_multiply_checks(
        a: &[i32],
        b: &[i32],
        n: &[i32],
        len: i32,
        product: Option<&Vec<i32>>,
    ) {
        if len % 2 != 0 {
            panic!("IllegalArgumentException: input array length must be even: {len}");
        }
        if len < 1 {
            panic!("IllegalArgumentException: invalid input length: {len}");
        }
        if len > a.len() as i32
            || len > b.len() as i32
            || len > n.len() as i32
            || product.map(|p| len > p.len() as i32).unwrap_or(false)
        {
            panic!("IllegalArgumentException: input array length out of bound: {len}");
        }
    }

    /// Java: private static int[] materialize(int[] z, int len).
    fn materialize(z: Option<Vec<i32>>, len: i32) -> Vec<i32> {
        match z {
            Some(z) if z.len() >= len as usize => z,
            _ => vec![0i32; len as usize],
        }
    }

    /// Java: private static int[] implMontgomeryMultiply(...).
    fn impl_montgomery_multiply(
        a: &[i32],
        b: &[i32],
        n: &[i32],
        len: i32,
        inv: i64,
        product: Vec<i32>,
    ) -> Vec<i32> {
        let product = Self::multiply_to_len(a, len, b, len, Some(product));
        Self::mont_reduce(product, n, len, inv as i32)
    }

    /// Java: private static int[] implMontgomerySquare(...).
    fn impl_montgomery_square(
        a: &[i32],
        n: &[i32],
        len: i32,
        inv: i64,
        product: Vec<i32>,
    ) -> Vec<i32> {
        let product = Self::square_to_len(a, len, Some(product));
        Self::mont_reduce(product, n, len, inv as i32)
    }

    /// Java: static int[] bnExpModThreshTable.
    const BN_EXP_MOD_THRESH_TABLE: [i32; 7] = [
        7, 25, 81, 241, 673, 1793, i32::MAX,
    ];

    /// Java: private BigInteger oddModPow(BigInteger y, BigInteger z).
    fn odd_mod_pow(&self, y: &BigInteger, z: &BigInteger) -> BigInteger {
        if y.equals(&ONE) {
            return self.clone();
        }
        if self.signum == 0 {
            return ZERO.clone();
        }
        let base = self.mag.clone();
        let exp = y.mag.clone();
        let mut mod_mag = z.mag.clone();
        let mut mod_len = mod_mag.len() as i32;
        if mod_len % 2 != 0 {
            let mut x = vec![0i32; mod_len as usize + 1];
            x[1..].copy_from_slice(&mod_mag);
            mod_mag = x;
            mod_len += 1;
        }
        let mut wbits: i32 = 0;
        let mut ebits = Self::bit_length_of(&exp, exp.len() as i32);
        if ebits != 17 || exp[0] != 65537 {
            while ebits > Self::BN_EXP_MOD_THRESH_TABLE[wbits as usize] {
                wbits += 1;
            }
        }
        let tblmask = 1i32 << wbits;
        let mut table: Vec<Vec<i32>> = (0..tblmask).map(|_| vec![0i32; mod_len as usize]).collect();
        let n0 = (mod_mag[mod_mag.len() - 1] as i64 & LONG_MASK)
            + ((mod_mag[mod_mag.len() - 2] as i64 & LONG_MASK) << 32);
        let inv = MutableBigInteger::inverse_mod64(n0).wrapping_neg();
        let mut a = Self::left_shift_mag(&base, mod_len << 5);
        let mut q = MutableBigInteger::new_empty();
        let mut a2 = MutableBigInteger::new_from_vec(a.clone());
        let mut b2 = MutableBigInteger::new_from_vec(mod_mag.clone());
        b2.normalize_public();
        let r = a2.divide_public(&b2, &mut q).expect("remainder");
        table[0] = r.to_int_array_public();
        if table[0].len() < mod_len as usize {
            let offset = mod_len as usize - table[0].len();
            let mut t2 = vec![0i32; mod_len as usize];
            t2[offset..].copy_from_slice(&table[0]);
            table[0] = t2;
        }
        let mut b = Self::montgomery_square(&table[0], &mod_mag, mod_len, inv, None);
        let mut t = b[..mod_len as usize].to_vec();
        let mut i = 1;
        while i < tblmask {
            table[i as usize] = Self::montgomery_multiply(&t, &table[(i - 1) as usize], &mod_mag, mod_len, inv, None);
            i += 1;
        }
        let mut bitpos = 1i32 << ((ebits - 1) & 31);
        let mut buf: i32 = 0;
        let mut elen = exp.len() as i32;
        let mut e_index = 0usize;
        for _ in 0..=wbits {
            buf = (buf << 1)
                | if (exp[e_index] & bitpos) != 0 { 1 } else { 0 };
            bitpos = ((bitpos as u32) >> 1) as i32;
            if bitpos == 0 {
                e_index += 1;
                bitpos = 1i32 << 31;
                elen -= 1;
            }
        }
        let mut multpos = ebits;
        ebits -= 1;
        let mut isone = true;
        multpos = ebits - wbits;
        while (buf & 1) == 0 {
            buf = ((buf as u32) >> 1) as i32;
            multpos += 1;
        }
        let mut mult = table[(buf >> 1) as usize].clone();
        buf = 0;
        if multpos == ebits {
            isone = false;
        }
        loop {
            ebits -= 1;
            buf <<= 1;
            if elen != 0 {
                if (exp[e_index] & bitpos) != 0 {
                    buf |= 1;
                }
                bitpos = ((bitpos as u32) >> 1) as i32;
                if bitpos == 0 {
                    e_index += 1;
                    bitpos = 1i32 << 31;
                    elen -= 1;
                }
            }
            if (buf & tblmask) != 0 {
                multpos = ebits - wbits;
                while (buf & 1) == 0 {
                    buf = ((buf as u32) >> 1) as i32;
                    multpos += 1;
                }
                mult = table[(buf >> 1) as usize].clone();
                buf = 0;
            }
            if ebits == multpos {
                if isone {
                    b = mult.clone();
                    isone = false;
                } else {
                    t = b.clone();
                    a = Self::montgomery_multiply(&t, &mult, &mod_mag, mod_len, inv, Some(a));
                    t = a.clone();
                    a = b.clone();
                    b = t;
                }
            }
            if ebits == 0 {
                break;
            }
            if !isone {
                t = b.clone();
                a = Self::montgomery_square(&t, &mod_mag, mod_len, inv, Some(a));
                t = a.clone();
                a = b.clone();
                b = t;
            }
        }
        let mut t2 = vec![0i32; 2 * mod_len as usize];
        t2[mod_len as usize..].copy_from_slice(&b[..mod_len as usize]);
        let b = Self::mont_reduce(t2, &mod_mag, mod_len, inv as i32);
        let t2 = b[..mod_len as usize].to_vec();
        BigInteger::from_signum_magnitude_ints(1, t2)
    }

    /// Java: private static int[] montReduce(int[] n, int[] mod, int mlen,
    /// int inv).
    fn mont_reduce(mut n: Vec<i32>, mod_mag: &[i32], mlen: i32, inv: i32) -> Vec<i32> {
        let mut c = 0;
        let mut len = mlen;
        let mut offset: i32 = 0;
        loop {
            let n_end = n[n.len() - 1 - offset as usize];
            let carry = Self::mul_add(&mut n, mod_mag, offset, mlen, inv.wrapping_mul(n_end));
            c += Self::add_one(&mut n, offset, mlen, carry);
            offset += 1;
            len -= 1;
            if len <= 0 {
                break;
            }
        }
        while c > 0 {
            c += Self::sub_n(&mut n, mod_mag, mlen);
        }
        while Self::int_array_cmp_to_len(&n, mod_mag, mlen) >= 0 {
            Self::sub_n(&mut n, mod_mag, mlen);
        }
        n
    }

    /// Java: private static int intArrayCmpToLen(int[] arg1, int[] arg2, int len).
    fn int_array_cmp_to_len(arg1: &[i32], arg2: &[i32], len: i32) -> i32 {
        for i in 0..len as usize {
            let b1 = arg1[i] as i64 & LONG_MASK;
            let b2 = arg2[i] as i64 & LONG_MASK;
            if b1 < b2 {
                return -1;
            }
            if b1 > b2 {
                return 1;
            }
        }
        0
    }

    /// Java: private static int subN(int[] a, int[] b, int len).
    fn sub_n(a: &mut [i32], b: &[i32], len: i32) -> i32 {
        let mut sum: i64 = 0;
        let mut len = len;
        while len > 0 {
            len -= 1;
            sum = (a[len as usize] as i64 & LONG_MASK)
                - (b[len as usize] as i64 & LONG_MASK)
                + (sum >> 32);
            a[len as usize] = sum as i32;
        }
        (sum >> 32) as i32
    }

    /// Java: static int mulAdd(int[] out, int[] in, int offset, int len, int k).
    fn mul_add(out: &mut [i32], input: &[i32], offset: i32, len: i32, k: i32) -> i32 {
        let k_long = k as i64 & LONG_MASK;
        let mut carry: i64 = 0;
        let mut offset = out.len() as i32 - offset - 1;
        let mut j = len - 1;
        while j >= 0 {
            let product = (input[j as usize] as i64 & LONG_MASK)
                .wrapping_mul(k_long)
                .wrapping_add(out[offset as usize] as i64 & LONG_MASK)
                .wrapping_add(carry);
            out[offset as usize] = product as i32;
            offset -= 1;
            carry = ((product as u64) >> 32) as i64;
            j -= 1;
        }
        carry as i32
    }

    /// Java: static int addOne(int[] a, int offset, int mlen, int carry).
    fn add_one(a: &mut [i32], offset: i32, mlen: i32, carry: i32) -> i32 {
        let mut offset = a.len() as i32 - mlen - offset - 1;
        let t = (a[offset as usize] as i64 & LONG_MASK) + (carry as i64 & LONG_MASK);
        a[offset as usize] = t as i32;
        if ((t as u64) >> 32) == 0 {
            return 0;
        }
        let mut mlen = mlen;
        while mlen > 0 {
            mlen -= 1;
            offset -= 1;
            if offset < 0 {
                return 1;
            }
            a[offset as usize] = a[offset as usize].wrapping_add(1);
            if a[offset as usize] != 0 {
                return 0;
            }
        }
        1
    }

    /// Java: private BigInteger modPow2(BigInteger exponent, int p).
    fn mod_pow2(&self, exponent: &BigInteger, p: i32) -> BigInteger {
        let mut result = ONE.clone();
        let mut base_to_pow2 = self.mod2(p);
        let mut exp_offset = 0;
        let mut limit = exponent.bit_length();
        if self.test_bit(0) {
            limit = if p - 1 < limit { p - 1 } else { limit };
        }
        while exp_offset < limit {
            if exponent.test_bit(exp_offset) {
                result = result.multiply(&base_to_pow2).mod2(p);
            }
            exp_offset += 1;
            if exp_offset < limit {
                base_to_pow2 = base_to_pow2.square_with_recursion(true).mod2(p);
            }
        }
        result
    }

    /// Java: private BigInteger mod2(int p).
    fn mod2(&self, p: i32) -> BigInteger {
        if self.bit_length() <= p {
            return self.clone();
        }
        let num_ints = ((p + 31) as u32 >> 5) as i32;
        let start = self.mag.len() - num_ints as usize;
        let mut mag = self.mag[start..].to_vec();
        let excess_bits = (num_ints << 5) - p;
        mag[0] &= ((1i64 << (32 - excess_bits)) - 1) as i32;
        if mag[0] == 0 {
            BigInteger::from_signum_magnitude_ints(1, mag)
        } else {
            BigInteger::from_int_array_twos_complement(mag)
        }
    }

    /// Java: public BigInteger modInverse(BigInteger m).
    pub(crate) fn mod_inverse(&self, m: &BigInteger) -> BigInteger {
        if m.signum != 1 {
            panic!("ArithmeticException: BigInteger: modulus not positive");
        }
        if m.equals(&ONE) {
            return ZERO.clone();
        }
        let mut mod_val = self.clone();
        if self.signum < 0 || self.compare_magnitude(m) >= 0 {
            mod_val = self.modulo(m);
        }
        if mod_val.equals(&ONE) {
            return ONE.clone();
        }
        let mut a = MutableBigInteger::new_from_big_integer(&mod_val);
        let b = MutableBigInteger::new_from_big_integer(m);
        let mut result = a.mutable_mod_inverse_public(&b);
        result.to_big_integer_sign(1)
    }

    /// Java: public BigInteger shiftLeft(int n).
    pub(crate) fn shift_left(&self, n: i32) -> BigInteger {
        if self.signum == 0 {
            return ZERO.clone();
        }
        if n > 0 {
            return BigInteger::new_from_internal(Self::left_shift_mag(&self.mag, n), self.signum);
        }
        if n == 0 {
            return self.clone();
        }
        self.shift_right_impl(n.wrapping_neg())
    }

    /// Java: public BigInteger shiftRight(int n).
    pub(crate) fn shift_right(&self, n: i32) -> BigInteger {
        if self.signum == 0 {
            return ZERO.clone();
        }
        if n > 0 {
            return self.shift_right_impl(n);
        }
        if n == 0 {
            return self.clone();
        }
        BigInteger::new_from_internal(Self::left_shift_mag(&self.mag, n.wrapping_neg()), self.signum)
    }

    /// Java: private BigInteger shiftRightImpl(int n).
    fn shift_right_impl(&self, n: i32) -> BigInteger {
        let n_ints = ((n as u32) >> 5) as i32;
        let n_bits = n & 0x1F;
        let mag_len = self.mag.len() as i32;
        let mut new_mag;
        if n_ints >= mag_len {
            return if self.signum >= 0 {
                ZERO.clone()
            } else {
                NEGATIVE_ONE.clone()
            };
        }
        if n_bits == 0 {
            new_mag = self.mag[..(mag_len - n_ints) as usize].to_vec();
        } else {
            let mut i: usize = 0;
            let high_bits = ((self.mag[0] as u32) >> n_bits) as i32;
            if high_bits != 0 {
                new_mag = vec![0i32; (mag_len - n_ints) as usize];
                new_mag[i] = high_bits;
                i += 1;
            } else {
                new_mag = vec![0i32; (mag_len - n_ints - 1) as usize];
            }
            let num_iter = mag_len - n_ints - 1;
            // shiftRightImplWorker(newMag, mag, i, nBits, numIter)
            let shift_count_left = 32 - n_bits;
            let mut idx = num_iter as usize;
            let mut nidx = if i == 0 { num_iter - 1 } else { num_iter } as usize;
            while nidx >= i {
                new_mag[nidx] = ((self.mag[idx] as u32) >> n_bits) as i32
                    | self.mag[idx - 1].wrapping_shl(shift_count_left as u32);
                if nidx == 0 {
                    break;
                }
                nidx -= 1;
                idx -= 1;
            }
        }
        if self.signum < 0 {
            let mut ones_lost = false;
            let mut i = mag_len - 1;
            let j = mag_len - n_ints;
            while i >= j && !ones_lost {
                ones_lost = self.mag[i as usize] != 0;
                i -= 1;
            }
            if !ones_lost && n_bits != 0 {
                ones_lost = self.mag[(mag_len - n_ints - 1) as usize]
                    .wrapping_shl((32 - n_bits) as u32)
                    != 0;
            }
            if ones_lost {
                new_mag = Self::java_increment(new_mag);
            }
        }
        BigInteger::new_from_internal(new_mag, self.signum)
    }

    /// Java: int[] javaIncrement(int[] val).
    fn java_increment(mut val: Vec<i32>) -> Vec<i32> {
        let mut last_sum: i32 = 0;
        let mut i = val.len() as isize - 1;
        while i >= 0 && last_sum == 0 {
            val[i as usize] = val[i as usize].wrapping_add(1);
            last_sum = val[i as usize];
            i -= 1;
        }
        if last_sum == 0 {
            let mut new_val = vec![0i32; val.len() + 1];
            new_val[0] = 1;
            return new_val;
        }
        val
    }

    /// Java: public BigInteger and(BigInteger val).
    pub(crate) fn and(&self, val: &BigInteger) -> BigInteger {
        let len = std::cmp::max(self.int_length(), val.int_length());
        let mut result = vec![0i32; len as usize];
        for i in 0..len as usize {
            result[i] = self.get_int((len as usize - i - 1) as i32)
                & val.get_int((len as usize - i - 1) as i32);
        }
        Self::value_of_ints(result)
    }

    /// Java: public BigInteger or(BigInteger val).
    pub(crate) fn or(&self, val: &BigInteger) -> BigInteger {
        let len = std::cmp::max(self.int_length(), val.int_length());
        let mut result = vec![0i32; len as usize];
        for i in 0..len as usize {
            result[i] = self.get_int((len as usize - i - 1) as i32)
                | val.get_int((len as usize - i - 1) as i32);
        }
        Self::value_of_ints(result)
    }

    /// Java: public BigInteger xor(BigInteger val).
    pub(crate) fn xor(&self, val: &BigInteger) -> BigInteger {
        let len = std::cmp::max(self.int_length(), val.int_length());
        let mut result = vec![0i32; len as usize];
        for i in 0..len as usize {
            result[i] = self.get_int((len as usize - i - 1) as i32)
                ^ val.get_int((len as usize - i - 1) as i32);
        }
        Self::value_of_ints(result)
    }

    /// Java: public BigInteger not().
    pub(crate) fn not(&self) -> BigInteger {
        let len = self.int_length();
        let mut result = vec![0i32; len as usize];
        for i in 0..len as usize {
            result[i] = !self.get_int((len as usize - i - 1) as i32);
        }
        Self::value_of_ints(result)
    }

    /// Java: public BigInteger andNot(BigInteger val).
    pub(crate) fn and_not(&self, val: &BigInteger) -> BigInteger {
        let len = std::cmp::max(self.int_length(), val.int_length());
        let mut result = vec![0i32; len as usize];
        for i in 0..len as usize {
            result[i] = self.get_int((len as usize - i - 1) as i32)
                & !val.get_int((len as usize - i - 1) as i32);
        }
        Self::value_of_ints(result)
    }

    /// Java: public boolean testBit(int n).
    pub(crate) fn test_bit(&self, n: i32) -> bool {
        if n < 0 {
            panic!("ArithmeticException: Negative bit address");
        }
        (self.get_int(((n as u32) >> 5) as i32) & (1i32 << (n & 31))) != 0
    }

    /// Java: public BigInteger setBit(int n).
    pub(crate) fn set_bit(&self, n: i32) -> BigInteger {
        if n < 0 {
            panic!("ArithmeticException: Negative bit address");
        }
        let int_num = ((n as u32) >> 5) as i32;
        let res_len = std::cmp::max(self.int_length(), int_num + 2);
        let mut result = vec![0i32; res_len as usize];
        for i in 0..result.len() {
            let index = result.len() - i - 1;
            result[index] = self.get_int(i as i32);
        }
        let index = result.len() - int_num as usize - 1;
        result[index] |= 1i32 << (n & 31);
        Self::value_of_ints(result)
    }

    /// Java: public BigInteger clearBit(int n).
    pub(crate) fn clear_bit(&self, n: i32) -> BigInteger {
        if n < 0 {
            panic!("ArithmeticException: Negative bit address");
        }
        let int_num = ((n as u32) >> 5) as i32;
        let res_len = std::cmp::max(self.int_length(), (((n + 1) as u32) >> 5) as i32 + 1);
        let mut result = vec![0i32; res_len as usize];
        for i in 0..result.len() {
            let index = result.len() - i - 1;
            result[index] = self.get_int(i as i32);
        }
        let index = result.len() - int_num as usize - 1;
        result[index] &= !(1i32 << (n & 31));
        Self::value_of_ints(result)
    }

    /// Java: public BigInteger flipBit(int n).
    pub(crate) fn flip_bit(&self, n: i32) -> BigInteger {
        if n < 0 {
            panic!("ArithmeticException: Negative bit address");
        }
        let int_num = ((n as u32) >> 5) as i32;
        let res_len = std::cmp::max(self.int_length(), int_num + 2);
        let mut result = vec![0i32; res_len as usize];
        for i in 0..result.len() {
            let index = result.len() - i - 1;
            result[index] = self.get_int(i as i32);
        }
        let index = result.len() - int_num as usize - 1;
        result[index] ^= 1i32 << (n & 31);
        Self::value_of_ints(result)
    }

    /// Java: public int getLowestSetBit().
    pub(crate) fn get_lowest_set_bit(&self) -> i32 {
        let cached = self.lowest_set_bit_plus_two.load(Ordering::Relaxed);
        let mut lsb = cached - 2;
        if lsb == -2 {
            lsb = 0;
            if self.signum == 0 {
                lsb -= 1;
            } else {
                let mut i = 0;
                let mut b;
                loop {
                    b = self.get_int(i);
                    if b != 0 {
                        break;
                    }
                    i += 1;
                }
                lsb += (i << 5) + (b as u32).trailing_zeros() as i32;
            }
            self.lowest_set_bit_plus_two.store(lsb + 2, Ordering::Relaxed);
        }
        lsb
    }

    /// Java: public int bitLength().
    pub(crate) fn bit_length(&self) -> i32 {
        let cached = self.bit_length_plus_one.load(Ordering::Relaxed);
        let mut n = cached - 1;
        if n == -1 {
            let len = self.mag.len();
            if len == 0 {
                n = 0;
            } else {
                let mag_bit_length = ((len as i32 - 1) << 5) + Self::bit_length_for_int(self.mag[0]);
                if self.signum < 0 {
                    let mut pow2 = self.mag[0].count_ones() == 1;
                    let mut i = 1;
                    while i < len && pow2 {
                        pow2 = self.mag[i] == 0;
                        i += 1;
                    }
                    n = if pow2 { mag_bit_length - 1 } else { mag_bit_length };
                } else {
                    n = mag_bit_length;
                }
            }
            self.bit_length_plus_one.store(n + 1, Ordering::Relaxed);
        }
        n
    }

    /// Java: public int bitCount().
    pub(crate) fn bit_count(&self) -> i32 {
        let cached = self.bit_count_plus_one.load(Ordering::Relaxed);
        let mut bc = cached - 1;
        if bc == -1 {
            bc = 0;
            for word in &self.mag {
                bc += word.count_ones() as i32;
            }
            if self.signum < 0 {
                let mut mag_trailing_zero_count = 0;
                let mut j = self.mag.len() - 1;
                while self.mag[j] == 0 {
                    mag_trailing_zero_count += 32;
                    j -= 1;
                }
                mag_trailing_zero_count += (self.mag[j] as u32).trailing_zeros() as i32;
                bc += mag_trailing_zero_count - 1;
            }
            self.bit_count_plus_one.store(bc + 1, Ordering::Relaxed);
        }
        bc
    }

    /// Java: public boolean isProbablePrime(int certainty).
    pub(crate) fn is_probable_prime(&self, certainty: i32) -> bool {
        if certainty <= 0 {
            return true;
        }
        let w = self.abs();
        if w.equals(&TWO) {
            return true;
        }
        if !w.test_bit(0) || w.equals(&ONE) {
            return false;
        }
        w.prime_to_certainty(certainty, None)
    }

    /// Java: public int compareTo(BigInteger val).
    pub(crate) fn compare_to(&self, val: &BigInteger) -> i32 {
        if self.signum == val.signum {
            return match self.signum {
                1 => self.compare_magnitude(val),
                -1 => val.compare_magnitude(self),
                _ => 0,
            };
        }
        if self.signum > val.signum {
            1
        } else {
            -1
        }
    }

    /// Java: final int compareMagnitude(BigInteger val).
    pub(crate) fn compare_magnitude(&self, val: &BigInteger) -> i32 {
        let m1 = &self.mag;
        let m2 = &val.mag;
        if m1.len() < m2.len() {
            return -1;
        }
        if m1.len() > m2.len() {
            return 1;
        }
        for i in 0..m1.len() {
            let a = m1[i];
            let b = m2[i];
            if a != b {
                return if (a as i64 & LONG_MASK) < (b as i64 & LONG_MASK) {
                    -1
                } else {
                    1
                };
            }
        }
        0
    }

    /// Java: final int compareMagnitude(long val).
    fn compare_magnitude_long(&self, val: i64) -> i32 {
        assert_ne!(val, i64::MIN);
        let m1 = &self.mag;
        let len = m1.len() as i32;
        if len > 2 {
            return 1;
        }
        let mut val = val;
        if val < 0 {
            val = val.wrapping_neg();
        }
        let high_word = ((val as u64) >> 32) as i32;
        if high_word == 0 {
            if len < 1 {
                return -1;
            }
            if len > 1 {
                return 1;
            }
            let a = m1[0];
            let b = val as i32;
            if a != b {
                return if (a as i64 & LONG_MASK) < (b as i64 & LONG_MASK) {
                    -1
                } else {
                    1
                };
            }
            0
        } else {
            if len < 2 {
                return -1;
            }
            let a = m1[0];
            let b = high_word;
            if a != b {
                return if (a as i64 & LONG_MASK) < (b as i64 & LONG_MASK) {
                    -1
                } else {
                    1
                };
            }
            let a = m1[1];
            let b = val as i32;
            if a != b {
                return if (a as i64 & LONG_MASK) < (b as i64 & LONG_MASK) {
                    -1
                } else {
                    1
                };
            }
            0
        }
    }

    /// Java: public boolean equals(Object x).
    pub(crate) fn equals(&self, x: &BigInteger) -> bool {
        if self.signum != x.signum {
            return false;
        }
        if self.mag.len() != x.mag.len() {
            return false;
        }
        self.mag == x.mag
    }

    /// Java: public BigInteger min(BigInteger val).
    pub(crate) fn min(&self, val: &BigInteger) -> BigInteger {
        if self.compare_to(val) < 0 {
            self.clone()
        } else {
            val.clone()
        }
    }

    /// Java: public BigInteger max(BigInteger val).
    pub(crate) fn max(&self, val: &BigInteger) -> BigInteger {
        if self.compare_to(val) > 0 {
            self.clone()
        } else {
            val.clone()
        }
    }

    /// Java: public int hashCode().
    pub(crate) fn hash_code(&self) -> i32 {
        let mut hash_code = 0i64;
        for word in &self.mag {
            hash_code = 31i64.wrapping_mul(hash_code) + (*word as i64 & LONG_MASK);
        }
        (hash_code as i32).wrapping_mul(self.signum)
    }

    /// Java: public String toString(int radix).
    pub(crate) fn to_string_radix(&self, radix: i32) -> String {
        if self.signum == 0 {
            return String::from("0");
        }
        let radix = if radix < 2 || radix > 36 { 10 } else { radix };
        let abs = self.abs();
        let b = abs.bit_length();
        let num_chars = (b as f64 * LOG_TWO / LOG_CACHE[radix as usize]).floor() as i32 + 1
            + if self.signum < 0 { 1 } else { 0 };
        let mut sb = String::with_capacity(num_chars as usize);
        if self.signum < 0 {
            sb.push('-');
        }
        Self::to_string_impl(&abs, &mut sb, radix, 0);
        sb
    }

    /// Java: public String toString().
    pub(crate) fn to_debug_string(&self) -> String {
        self.to_string_radix(10)
    }

    /// Java: private static void padWithZeros(StringBuilder buf, int numZeros).
    fn pad_with_zeros(buf: &mut String, num_zeros: i32) {
        let mut num_zeros = num_zeros;
        while num_zeros >= NUM_ZEROS {
            buf.push_str(ZEROS);
            num_zeros -= NUM_ZEROS;
        }
        if num_zeros > 0 {
            for _ in 0..num_zeros {
                buf.push('0');
            }
        }
    }

    /// Java: private void smallToString(int radix, StringBuilder buf, int digits).
    fn small_to_string(&self, radix: i32, buf: &mut String, digits: i32) {
        assert!(self.signum >= 0);
        if self.signum == 0 {
            Self::pad_with_zeros(buf, digits);
            return;
        }
        let max_num_digit_groups = (4 * self.mag.len() as i32 + 6) / 7;
        let mut digit_groups = vec![0i64; max_num_digit_groups as usize];
        let mut tmp = self.clone();
        let mut num_groups = 0usize;
        while tmp.signum != 0 {
            let d = Self::long_radix(radix);
            let mut q = MutableBigInteger::new_empty();
            let mut a = MutableBigInteger::new_from_vec(tmp.mag.clone());
            let b = MutableBigInteger::new_from_vec(d.mag.clone());
            let mut r = a.divide_public(&b, &mut q).expect("remainder");
            let q2 = q.to_big_integer_sign(tmp.signum * d.signum);
            let r2 = r.to_big_integer_sign(tmp.signum * d.signum);
            digit_groups[num_groups] = r2.long_value();
            num_groups += 1;
            tmp = q2;
        }
        let s = Self::long_to_string(digit_groups[num_groups - 1], radix);
        Self::pad_with_zeros(
            buf,
            digits - (s.len() as i32 + (num_groups as i32 - 1) * DIGITS_PER_LONG[radix as usize]),
        );
        buf.push_str(&s);
        let mut i = num_groups as isize - 2;
        while i >= 0 {
            let s = Self::long_to_string(digit_groups[i as usize], radix);
            let num_leading_zeros = DIGITS_PER_LONG[radix as usize] - s.len() as i32;
            if num_leading_zeros != 0 {
                for _ in 0..num_leading_zeros {
                    buf.push('0');
                }
            }
            buf.push_str(&s);
            i -= 1;
        }
    }

    /// Java: private static void toString(BigInteger u, StringBuilder sb,
    /// int radix, int digits).
    fn to_string_impl(u: &BigInteger, sb: &mut String, radix: i32, digits: i32) {
        assert!(u.signum >= 0);
        if u.mag.len() as i32 <= SCHOENHAGE_BASE_CONVERSION_THRESHOLD {
            u.small_to_string(radix, sb, digits);
            return;
        }
        let b = u.bit_length();
        let n = ((b as f64 * LOG_TWO / LOG_CACHE[radix as usize]).ln() / LOG_TWO - 1.0).round()
            as i32;
        let v = Self::get_radix_conversion_cache(radix, n);
        let (q, r) = u.divide_and_remainder(&v);
        let expected_digits = 1i32 << n;
        Self::to_string_impl(&q, sb, radix, digits - expected_digits);
        Self::to_string_impl(&r, sb, radix, expected_digits);
    }

    /// Java: private static BigInteger getRadixConversionCache(int radix,
    /// int exponent).
    fn get_radix_conversion_cache(radix: i32, exponent: i32) -> BigInteger {
        let mut cache = POWER_CACHE.lock().expect("power cache");
        let cache_line = &mut cache[radix as usize];
        if exponent < cache_line.len() as i32 {
            return cache_line[exponent as usize].clone();
        }
        let old_length = cache_line.len() as i32;
        // Build the missing powers radix^(2^n) by squaring the last cached power.
        let mut i = old_length;
        while i <= exponent {
            let prev = cache_line[(i - 1) as usize].clone();
            cache_line.push(prev.pow(2));
            i += 1;
        }
        cache_line[exponent as usize].clone()
    }

    /// Java: public byte[] toByteArray().
    pub(crate) fn to_byte_array(&self) -> Vec<u8> {
        let byte_len = self.bit_length() / 8 + 1;
        let mut byte_array = vec![0u8; byte_len as usize];
        let mut i = byte_len - 1;
        let mut bytes_copied = 4;
        let mut next_int = 0i32;
        let mut int_index = 0i32;
        while i >= 0 {
            if bytes_copied == 4 {
                next_int = self.get_int(int_index);
                int_index += 1;
                bytes_copied = 1;
            } else {
                next_int = ((next_int as u32) >> 8) as i32;
                bytes_copied += 1;
            }
            byte_array[i as usize] = next_int as u8;
            i -= 1;
        }
        byte_array
    }

    /// Java: public int intValue().
    pub(crate) fn int_value(&self) -> i32 {
        self.get_int(0)
    }

    /// Java: public long longValue().
    pub(crate) fn long_value(&self) -> i64 {
        let mut result: i64 = 0;
        let mut i = 1;
        while i >= 0 {
            result = (result << 32) + (self.get_int(i) as i64 & LONG_MASK);
            i -= 1;
        }
        result
    }

    /// Java: public float floatValue().
    pub(crate) fn float_value(&self) -> f32 {
        if self.signum == 0 {
            return 0.0f32;
        }
        let exponent = ((self.mag.len() as i32 - 1) << 5)
            + Self::bit_length_for_int(self.mag[0])
            - 1;
        if exponent < 63 {
            return self.long_value() as f32;
        }
        if exponent > FLOAT_MAX_EXPONENT {
            return if self.signum > 0 {
                f32::INFINITY
            } else {
                f32::NEG_INFINITY
            };
        }
        let shift = exponent - FLOAT_SIGNIFICAND_WIDTH;
        let n_bits = shift & 0x1F;
        let n_bits2 = 32 - n_bits;
        let mut twice_signif_floor;
        if n_bits == 0 {
            twice_signif_floor = self.mag[0];
        } else {
            twice_signif_floor = ((self.mag[0] as u32) >> n_bits) as i32;
            if twice_signif_floor == 0 {
                twice_signif_floor =
                    self.mag[0].wrapping_shl(n_bits2 as u32) | ((self.mag[1] as u32) >> n_bits) as i32;
            }
        }
        let mut signif_floor = twice_signif_floor >> 1;
        signif_floor &= FLOAT_SIGNIF_BIT_MASK;
        let increment = (twice_signif_floor & 1) != 0
            && ((signif_floor & 1) != 0 || self.abs().get_lowest_set_bit() < shift);
        let signif_rounded = if increment { signif_floor + 1 } else { signif_floor };
        let mut bits = ((exponent + FLOAT_EXP_BIAS) << (FLOAT_SIGNIFICAND_WIDTH - 1))
            + signif_rounded;
        bits |= self.signum & FLOAT_SIGN_BIT_MASK;
        f32::from_bits(bits as u32)
    }

    /// Java: public double doubleValue().
    pub(crate) fn double_value(&self) -> f64 {
        if self.signum == 0 {
            return 0.0f64;
        }
        let exponent = ((self.mag.len() as i32 - 1) << 5)
            + Self::bit_length_for_int(self.mag[0])
            - 1;
        if exponent < 63 {
            return self.long_value() as f64;
        }
        if exponent > DOUBLE_MAX_EXPONENT {
            return if self.signum > 0 {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
        }
        let shift = exponent - DOUBLE_SIGNIFICAND_WIDTH;
        let n_bits = shift & 0x1F;
        let n_bits2 = 32 - n_bits;
        let high_bits: i32;
        let low_bits: i32;
        if n_bits == 0 {
            high_bits = self.mag[0];
            low_bits = self.mag[1];
        } else {
            high_bits = ((self.mag[0] as u32) >> n_bits) as i32;
            low_bits = self.mag[0].wrapping_shl(n_bits2 as u32) | ((self.mag[1] as u32) >> n_bits) as i32;
            if high_bits == 0 {
                let high_bits2 = low_bits;
                let low_bits2 = self.mag[1].wrapping_shl(n_bits2 as u32)
                    | ((self.mag[2] as u32) >> n_bits) as i32;
                return Self::double_value_from_parts(
                    exponent,
                    high_bits2,
                    low_bits2,
                    shift,
                    self,
                );
            }
        }
        Self::double_value_from_parts(exponent, high_bits, low_bits, shift, self)
    }

    fn double_value_from_parts(
        exponent: i32,
        high_bits: i32,
        low_bits: i32,
        shift: i32,
        value: &BigInteger,
    ) -> f64 {
        let twice_signif_floor = ((high_bits as i64 & LONG_MASK) << 32) | (low_bits as i64 & LONG_MASK);
        let mut signif_floor = twice_signif_floor >> 1;
        signif_floor &= DOUBLE_SIGNIF_BIT_MASK;
        let increment = (twice_signif_floor & 1) != 0
            && ((signif_floor & 1) != 0 || value.abs().get_lowest_set_bit() < shift);
        let signif_rounded = if increment { signif_floor + 1 } else { signif_floor };
        let mut bits = ((exponent + DOUBLE_EXP_BIAS) as i64) << (DOUBLE_SIGNIFICAND_WIDTH - 1);
        bits += signif_rounded;
        bits |= (value.signum as i64) & DOUBLE_SIGN_BIT_MASK;
        f64::from_bits(bits as u64)
    }

    /// Java: public long longValueExact().
    pub(crate) fn long_value_exact(&self) -> i64 {
        if self.mag.len() <= 2 && self.bit_length() <= 63 {
            self.long_value()
        } else {
            panic!("ArithmeticException: BigInteger out of long range");
        }
    }

    /// Java: public int intValueExact().
    pub(crate) fn int_value_exact(&self) -> i32 {
        if self.mag.len() <= 1 && self.bit_length() <= 31 {
            self.int_value()
        } else {
            panic!("ArithmeticException: BigInteger out of int range");
        }
    }

    /// Java: public short shortValueExact().
    pub(crate) fn short_value_exact(&self) -> i16 {
        if self.mag.len() <= 1 && self.bit_length() <= 31 {
            let value = self.int_value();
            if value >= i16::MIN as i32 && value <= i16::MAX as i32 {
                return value as i16;
            }
        }
        panic!("ArithmeticException: BigInteger out of short range");
    }

    /// Java: public byte byteValueExact().
    pub(crate) fn byte_value_exact(&self) -> i8 {
        if self.mag.len() <= 1 && self.bit_length() <= 31 {
            let value = self.int_value();
            if value >= i8::MIN as i32 && value <= i8::MAX as i32 {
                return value as i8;
            }
        }
        panic!("ArithmeticException: BigInteger out of byte range");
    }

    /// Java: private int intLength().
    fn int_length(&self) -> i32 {
        ((self.bit_length() as u32) >> 5) as i32 + 1
    }

    /// Java: private int signBit().
    fn sign_bit(&self) -> i32 {
        if self.signum < 0 { 1 } else { 0 }
    }

    /// Java: private int signInt().
    fn sign_int(&self) -> i32 {
        if self.signum < 0 { -1 } else { 0 }
    }

    /// Java: private int getInt(int n).
    fn get_int(&self, n: i32) -> i32 {
        if n < 0 {
            return 0;
        }
        if n as usize >= self.mag.len() {
            return self.sign_int();
        }
        let mag_int = self.mag[self.mag.len() - n as usize - 1];
        if self.signum >= 0 {
            mag_int
        } else if n <= self.first_nonzero_int_num() {
            mag_int.wrapping_neg()
        } else {
            !mag_int
        }
    }

    /// Java: private int firstNonzeroIntNum().
    fn first_nonzero_int_num(&self) -> i32 {
        let mut fn_value = self.first_nonzero_int_num_plus_two.load(Ordering::Relaxed) - 2;
        if fn_value == -2 {
            let mlen = self.mag.len() as i32;
            let mut i = mlen - 1;
            while i >= 0 && self.mag[i as usize] == 0 {
                i -= 1;
            }
            fn_value = mlen - i - 1;
            self.first_nonzero_int_num_plus_two.store(fn_value + 2, Ordering::Relaxed);
        }
        fn_value
    }

    /// Java: private static BigInteger valueOf(int val[]).
    fn value_of_ints(val: Vec<i32>) -> BigInteger {
        if val[0] > 0 {
            BigInteger::new_from_internal(val, 1)
        } else {
            BigInteger::from_int_array_twos_complement(val)
        }
    }

    /// Java: private static int[] stripLeadingZeroInts(int[] val).
    fn strip_leading_zero_ints(val: Vec<i32>) -> Vec<i32> {
        let mut keep = 0;
        while keep < val.len() && val[keep] == 0 {
            keep += 1;
        }
        val[keep..].to_vec()
    }

    /// Java: private static int[] trustedStripLeadingZeroInts(int[] val).
    fn trusted_strip_leading_zero_ints(val: Vec<i32>) -> Vec<i32> {
        let mut keep = 0;
        while keep < val.len() && val[keep] == 0 {
            keep += 1;
        }
        if keep == 0 {
            val
        } else {
            val[keep..].to_vec()
        }
    }

    /// Java: private static int[] stripLeadingZeroBytes(byte[] a, int from,
    /// int len).
    fn strip_leading_zero_bytes(b: i32, a: &[u8], from: usize, len: usize) -> Vec<i32> {
        if len == 0 {
            return Vec::new();
        }
        let to = from + len;
        let mut from = from;
        let mut b = b;
        if b < -128 {
            b = a[from] as i8 as i32;
        }
        from += 1;
        while b == 0 && from < to {
            b = a[from] as i8 as i32;
            from += 1;
        }
        if b == 0 {
            return Vec::new();
        }
        let mut res = vec![0i32; ((to - from) >> 2) + 1];
        let mut d0 = b & 0xFF;
        while ((to - from) & 0x3) != 0 {
            d0 = (d0 << 8) | (a[from] as i32 & 0xFF);
            from += 1;
        }
        res[0] = d0;
        let mut i = 1;
        while from < to {
            res[i] = ((a[from] as i32) << 24)
                | ((a[from + 1] as i32 & 0xFF) << 16)
                | ((a[from + 2] as i32 & 0xFF) << 8)
                | (a[from + 3] as i32 & 0xFF);
            from += 4;
            i += 1;
        }
        res
    }

    /// Java: private static int[] makePositive(int b, byte[] a, int from,
    /// int len).
    fn make_positive_byte(b: i32, a: &[u8], from: usize, len: usize) -> Vec<i32> {
        let to = from + len;
        let mut from = from + 1;
        let mut b = b;
        while b == -1 && from < to {
            b = a[from] as i8 as i32;
            from += 1;
        }
        let mut d0 = (-1i32) << 8 | (b & 0xFF);
        while ((to - from) & 0x3) != 0 {
            b = a[from] as i8 as i32;
            from += 1;
            d0 = (d0 << 8) | (b & 0xFF);
        }
        let f = from;
        while b == 0 && from < to {
            b = a[from] as i8 as i32;
            from += 1;
        }
        let mut d = b & 0xFF;
        while ((to - from) & 0x3) != 0 {
            d = (d << 8) | (a[from] as i32 & 0xFF);
            from += 1;
        }
        let c = if ((to - from) as i32 | d0 | d) == 0 { 1 } else { 0 };
        let mut res = vec![0i32; c + 1 + ((to - f) >> 2)];
        res[0] = if c == 0 { d0 } else { -1 };
        let mut i = res.len() - ((to - from) >> 2);
        if i > 1 {
            res[i - 1] = d;
        }
        while from < to {
            res[i] = ((a[from] as i32) << 24)
                | ((a[from + 1] as i32 & 0xFF) << 16)
                | ((a[from + 2] as i32 & 0xFF) << 8)
                | (a[from + 3] as i32 & 0xFF);
            from += 4;
            i += 1;
        }
        let mut i = res.len() as isize - 1;
        while i >= 0 && res[i as usize] == 0 {
            i -= 1;
        }
        res[i as usize] = res[i as usize].wrapping_neg();
        i -= 1;
        while i >= 0 {
            res[i as usize] = !res[i as usize];
            i -= 1;
        }
        res
    }

    /// Java: private static int[] makePositive(int a[]).
    fn make_positive_int(a: &[i32]) -> Vec<i32> {
        let mut keep = 0;
        while keep < a.len() && a[keep] == -1 {
            keep += 1;
        }
        let mut j = keep;
        while j < a.len() && a[j] == 0 {
            j += 1;
        }
        let extra_int = if j == a.len() { 1 } else { 0 };
        let mut result = vec![0i32; a.len() - keep + extra_int];
        for i in keep..a.len() {
            result[i - keep + extra_int] = !a[i];
        }
        let mut i = result.len() as isize - 1;
        loop {
            result[i as usize] = result[i as usize].wrapping_add(1);
            if result[i as usize] != 0 || i == 0 {
                break;
            }
            i -= 1;
        }
        result
    }

    /// Java: private static int[] longRadix[] (built on demand).
    fn long_radix(radix: i32) -> BigInteger {
        let values = &*LONG_RADIX;
        values[radix as usize].clone()
    }

    /// Java: private static String Long.toString(long, int) for the small
    /// digit groups.
    fn long_to_string(value: i64, radix: i32) -> String {
        if value == 0 {
            return String::from("0");
        }
        let negative = value < 0;
        let mut magnitude = value.unsigned_abs();
        let mut digits = Vec::new();
        while magnitude > 0 {
            let digit = (magnitude % radix as u64) as i32;
            digits.push(crate::digit_helper::for_digit(digit, radix));
            magnitude /= radix as u64;
        }
        digits.reverse();
        let mut result: String = digits.iter().collect();
        if negative {
            result.insert(0, '-');
        }
        result
    }
}

/// Java: java.lang.Integer.parseInt(String, int) for one digit group.
fn parse_int_str(s: &str, radix: i32) -> i32 {
    let mut result: i32 = 0;
    for c in s.chars() {
        let code = c as u32;
        let d = crate::digit_helper::digit(code as u16, radix);
        if d < 0 {
            panic!("NumberFormatException: For input string: {s}");
        }
        result = result.wrapping_mul(radix).wrapping_add(d);
    }
    result
}

/// Java: java.util.Objects.checkFromIndexSize(int fromIndex, int size,
/// int length).
fn check_from_index_size(from_index: usize, size: usize, length: usize) {
    if from_index > length || size > length - from_index {
        panic!("IndexOutOfBoundsException: Range [{from_index}, {from_index} + {size}) out of bounds for length {length}");
    }
}

/// Java: private static BigInteger longRadix[].
static LONG_RADIX: LazyLock<Vec<BigInteger>> = LazyLock::new(|| {
    vec![
        BigInteger::zero(),
        BigInteger::zero(),
        BigInteger::value_of_long(0x4000000000000000),
        BigInteger::value_of_long(0x383d9170b85ff80b),
        BigInteger::value_of_long(0x4000000000000000),
        BigInteger::value_of_long(0x6765c793fa10079d),
        BigInteger::value_of_long(0x41c21cb8e1000000),
        BigInteger::value_of_long(0x3642798750226111),
        BigInteger::value_of_long(0x1000000000000000),
        BigInteger::value_of_long(0x12bf307ae81ffd59),
        BigInteger::value_of_long(0xde0b6b3a7640000),
        BigInteger::value_of_long(0x4d28cb56c33fa539),
        BigInteger::value_of_long(0x1eca170c00000000),
        BigInteger::value_of_long(0x780c7372621bd74d),
        BigInteger::value_of_long(0x1e39a5057d810000),
        BigInteger::value_of_long(0x5b27ac993df97701),
        BigInteger::value_of_long(0x1000000000000000),
        BigInteger::value_of_long(0x27b95e997e21d9f1),
        BigInteger::value_of_long(0x5da0e1e53c5c8000),
        BigInteger::value_of_long(0xb16a458ef403f19),
        BigInteger::value_of_long(0x16bcc41e90000000),
        BigInteger::value_of_long(0x2d04b7fdd9c0ef49),
        BigInteger::value_of_long(0x5658597bcaa24000),
        BigInteger::value_of_long(0x6feb266931a75b7),
        BigInteger::value_of_long(0xc29e98000000000),
        BigInteger::value_of_long(0x14adf4b7320334b9),
        BigInteger::value_of_long(0x226ed36478bfa000),
        BigInteger::value_of_long(0x383d9170b85ff80b),
        BigInteger::value_of_long(0x5a3c23e39c000000),
        BigInteger::value_of_long(0x4e900abb53e6b71),
        BigInteger::value_of_long(0x7600ec618141000),
        BigInteger::value_of_long(0xaee5720ee830681),
        BigInteger::value_of_long(0x1000000000000000),
        BigInteger::value_of_long(0x172588ad4f5f0981),
        BigInteger::value_of_long(0x211e44f7d02c1000),
        BigInteger::value_of_long(0x2ee56725f06e5c71),
        BigInteger::value_of_long(0x41c21cb8e1000000),
    ]
});

/// Java: private static int NUM_ZEROS.
const NUM_ZEROS: i32 = 63;

/// Java: private static final String ZEROS.
const ZEROS: &str = "000000000000000000000000000000000000000000000000000000000000000";

/// Java: java.math.BigDecimal.INFLATED (the package-private sentinel used by
/// `multiply(long)`; BigDecimal itself is not ported).
const INFLATED: i64 = i64::MIN;

/// Java: java.lang.Float constants used by floatValue.
const FLOAT_SIGNIFICAND_WIDTH: i32 = 24;
const FLOAT_EXP_BIAS: i32 = 127;
const FLOAT_SIGNIF_BIT_MASK: i32 = 0x007f_ffff;
const FLOAT_SIGN_BIT_MASK: i32 = 0x8000_0000u32 as i32;
const FLOAT_MAX_EXPONENT: i32 = 127;

/// Java: java.lang.Double constants used by doubleValue.
const DOUBLE_SIGNIFICAND_WIDTH: i32 = 53;
const DOUBLE_EXP_BIAS: i32 = 1023;
const DOUBLE_SIGNIF_BIT_MASK: i64 = 0x000f_ffff_ffff_ffff;
const DOUBLE_SIGN_BIT_MASK: i64 = 0x8000_0000_0000_0000u64 as i64;
const DOUBLE_MAX_EXPONENT: i32 = 1023;

/// Java: BitSieve (java/math/BitSieve.java).
struct BitSieve {
    /// Java: private long bits[].
    bits: Vec<i64>,
    /// Java: private int length.
    length: i32,
}

/// Java: private static BitSieve smallSieve = new BitSieve().
static SMALL_SIEVE: LazyLock<BitSieve> = LazyLock::new(BitSieve::new_small);

impl BitSieve {
    /// Java: private BitSieve().
    fn new_small() -> BitSieve {
        let length = 150 * 64;
        let mut sieve = BitSieve {
            bits: vec![0i64; (Self::unit_index(length - 1) + 1) as usize],
            length,
        };
        sieve.set(0);
        let mut next_index = 1;
        let mut next_prime = 3;
        loop {
            sieve.sieve_single(length, next_index + next_prime, next_prime);
            next_index = sieve.sieve_search(length, next_index + 1);
            next_prime = 2 * next_index + 1;
            if !(next_index > 0 && next_prime < length) {
                break;
            }
        }
        sieve
    }

    /// Java: BitSieve(BigInteger base, int searchLen).
    fn new(base: &BigInteger, search_len: i32) -> BitSieve {
        let mut sieve = BitSieve {
            bits: vec![0i64; (Self::unit_index(search_len - 1) + 1) as usize],
            length: search_len,
        };
        let small = &*SMALL_SIEVE;
        let mut step = small.sieve_search(small.length, 0);
        let mut converted_step = step * 2 + 1;
        let mut b = MutableBigInteger::new_from_vec(base.mag.clone());
        let mut q = MutableBigInteger::new_empty();
        loop {
            let start = b.divide_one_word(converted_step, &mut q);
            let start = converted_step - start;
            let start = if start % 2 == 0 {
                start + converted_step
            } else {
                start
            };
            sieve.sieve_single(search_len, (start - 1) / 2, converted_step);
            step = small.sieve_search(small.length, step + 1);
            converted_step = step * 2 + 1;
            if !(step > 0) {
                break;
            }
        }
        sieve
    }

    /// Java: private static int unitIndex(int bitIndex).
    fn unit_index(bit_index: i32) -> i32 {
        ((bit_index as u32) >> 6) as i32
    }

    /// Java: private static long bit(int bitIndex).
    fn bit(bit_index: i32) -> i64 {
        1i64 << (bit_index & 63)
    }

    /// Java: private boolean get(int bitIndex).
    fn get(&self, bit_index: i32) -> bool {
        let unit_index = Self::unit_index(bit_index);
        (self.bits[unit_index as usize] & Self::bit(bit_index)) != 0
    }

    /// Java: private void set(int bitIndex).
    fn set(&mut self, bit_index: i32) {
        let unit_index = Self::unit_index(bit_index);
        self.bits[unit_index as usize] |= Self::bit(bit_index);
    }

    /// Java: private int sieveSearch(int limit, int start).
    fn sieve_search(&self, limit: i32, start: i32) -> i32 {
        if start >= limit {
            return -1;
        }
        let mut index = start;
        loop {
            if !self.get(index) {
                return index;
            }
            index += 1;
            if !(index < limit - 1) {
                break;
            }
        }
        -1
    }

    /// Java: private void sieveSingle(int limit, int start, int step).
    fn sieve_single(&mut self, limit: i32, start: i32, step: i32) {
        let mut start = start;
        while start < limit {
            self.set(start);
            start += step;
        }
    }

    /// Java: BigInteger retrieve(BigInteger initValue, int certainty,
    /// java.util.Random random).
    fn retrieve(
        &self,
        init_value: &BigInteger,
        certainty: i32,
        mut random: Option<&mut Random>,
    ) -> Option<BigInteger> {
        let mut offset = 1i64;
        for i in 0..self.bits.len() {
            let mut next_long = !self.bits[i];
            for _ in 0..64 {
                if (next_long & 1) == 1 {
                    let candidate = init_value.add(&BigInteger::value_of_long(offset));
                    if candidate.prime_to_certainty(
                        certainty,
                        random.as_deref_mut(),
                    ) {
                        return Some(candidate);
                    }
                }
                next_long = ((next_long as u64) >> 1) as i64;
                offset += 2;
            }
        }
        None
    }
}

/// Java: class SignedMutableBigInteger extends MutableBigInteger
/// (java/math/SignedMutableBigInteger.java).
pub(crate) struct SignedMutableBigInteger {
    /// The magnitude (the inherited MutableBigInteger state).
    pub(crate) inner: MutableBigInteger,
    /// Java: int sign = 1.
    pub(crate) sign: i32,
}

impl SignedMutableBigInteger {
    /// Java: MutableBigInteger compare (inherited by SignedMutableBigInteger).
    pub(crate) fn compare(&self, b: &MutableBigInteger) -> i32 {
        self.inner.compare(b)
    }

    /// Java: MutableBigInteger leftShift (inherited).
    pub(crate) fn left_shift(&mut self, n: i32) {
        self.inner.left_shift(n)
    }

    /// Java: MutableBigInteger rightShift (inherited).
    pub(crate) fn right_shift(&mut self, n: i32) {
        self.inner.right_shift(n)
    }

    /// Java: SignedMutableBigInteger().
    pub(crate) fn new_empty() -> SignedMutableBigInteger {
        SignedMutableBigInteger {
            inner: MutableBigInteger::new_empty(),
            sign: 1,
        }
    }

    /// Java: SignedMutableBigInteger(int val).
    pub(crate) fn new_int(val: i32) -> SignedMutableBigInteger {
        SignedMutableBigInteger {
            inner: MutableBigInteger::new_int(val),
            sign: 1,
        }
    }

    /// Java: SignedMutableBigInteger(MutableBigInteger val).
    pub(crate) fn new_from_mutable(val: &MutableBigInteger) -> SignedMutableBigInteger {
        SignedMutableBigInteger {
            inner: MutableBigInteger::new_from_mutable(val),
            sign: 1,
        }
    }

    /// Java: void signedAdd(SignedMutableBigInteger addend).
    pub(crate) fn signed_add(&mut self, addend: &SignedMutableBigInteger) {
        if self.sign == addend.sign {
            self.inner.add(&addend.inner);
        } else {
            self.sign = self.sign * self.inner.subtract(&addend.inner);
        }
    }

    /// Java: void signedAdd(MutableBigInteger addend).
    pub(crate) fn signed_add_mutable(&mut self, addend: &MutableBigInteger) {
        if self.sign == 1 {
            self.inner.add(addend);
        } else {
            self.sign = self.sign * self.inner.subtract(addend);
        }
    }

    /// Java: void signedSubtract(SignedMutableBigInteger addend).
    pub(crate) fn signed_subtract(&mut self, addend: &SignedMutableBigInteger) {
        if self.sign == addend.sign {
            self.sign = self.sign * self.inner.subtract(&addend.inner);
        } else {
            self.inner.add(&addend.inner);
        }
    }

    /// Java: void signedSubtract(MutableBigInteger addend).
    pub(crate) fn signed_subtract_mutable(&mut self, addend: &MutableBigInteger) {
        if self.sign == 1 {
            self.sign = self.sign * self.inner.subtract(addend);
        } else {
            self.inner.add(addend);
        }
        if self.inner.int_len == 0 {
            self.sign = 1;
        }
    }

    /// Java: public String toString().
    pub(crate) fn to_debug_string(&mut self) -> String {
        let sign = self.sign;
        self.inner.to_big_integer_sign(sign).to_debug_string()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn bi(value: &str) -> BigInteger {
        BigInteger::from_string(value)
    }

    /// The expected values are the development-time javac oracle output
    /// (JDK 17 java.math.BigInteger; the test itself runs without a JVM).
    #[test]
    fn big_integer_arithmetic_matches_the_javac_oracle() {
        let a = bi("123456789012345678901234567890");
        let b = bi("-98765432109876543210987654321");
        let c = bi("18446744073709551617");
        assert_eq!(a.to_debug_string(), "123456789012345678901234567890");
        assert_eq!(b.to_debug_string(), "-98765432109876543210987654321");
        assert_eq!(a.add(&b).to_debug_string(), "24691356902469135690246913569");
        assert_eq!(
            a.subtract(&b).to_debug_string(),
            "222222221122222222112222222211"
        );
        assert_eq!(
            a.multiply(&b).to_debug_string(),
            "-12193263113702179522618503273362292333223746380111126352690"
        );
        assert_eq!(
            a.divide(&bi("1234567890123456789")).to_debug_string(),
            "100000000000"
        );
        assert_eq!(
            a.remainder(&bi("1234567890123456789")).to_debug_string(),
            "1234567890"
        );
        assert_eq!(
            a.modulo(&bi("4294967296")).to_debug_string(),
            "1312754386"
        );
        assert_eq!(
            bi("12345").pow(17).to_debug_string(),
            "3592101522916075416804149916723226187790444507512040527030181884765625"
        );
        assert_eq!(
            a.shift_left(67).to_debug_string(),
            "18219006328581585121987126704180493664091289681920"
        );
        assert_eq!(a.shift_right(67).to_debug_string(), "836575742");
        assert_eq!(a.and(&b).to_debug_string(), "39857104640305249140344357442");
        assert_eq!(
            a.or(&b).to_debug_string(),
            "-15165747737836113450097443873"
        );
        assert_eq!(
            a.xor(&b).to_debug_string(),
            "-55022852378141362590441801315"
        );
        assert_eq!(a.not().to_debug_string(), "-123456789012345678901234567891");
        assert!(!a.test_bit(77));
        assert_eq!(a.bit_length(), 97);
        assert_eq!(a.bit_count(), 54);
        assert_eq!(c.get_lowest_set_bit(), 0);
        assert_eq!(b.signum(), -1);
        assert_eq!(
            a.to_string_radix(2),
            "1100011101110100100001111111101101100001101110011111000001110111001001110001111110000101011010010"
        );
        assert_eq!(a.to_string_radix(16), "18ee90ff6c373e0ee4e3f0ad2");
        assert_eq!(a.to_string_radix(36), "byw97um9s91dlz68tsi");
        let mut hex = String::new();
        for byte in a.to_byte_array() {
            hex.push_str(&format!("{byte:02X}"));
        }
        assert_eq!(hex, "018EE90FF6C373E0EE4E3F0AD2");
        assert_eq!(
            BigInteger::from_string_radix("zzzzzzzzzzzz", 36).to_debug_string(),
            "4738381338321616895"
        );
        assert_eq!(
            bi("123456789012345678901234567890")
                .gcd(&bi("98765432109876543210"))
                .to_debug_string(),
            "90"
        );
        assert_eq!(
            bi("152415787532388367504953515625666819450083828735361")
                .sqrt()
                .to_debug_string(),
            "12345678901234567890123456"
        );
        assert_eq!(
            bi("1234567")
                .mod_pow(&bi("123456789"), &bi("987654323"))
                .to_debug_string(),
            "569109174"
        );
        assert_eq!(
            bi("1234567").mod_inverse(&bi("987654323")).to_debug_string(),
            "177586530"
        );
        assert_eq!(b.negate().to_debug_string(), "98765432109876543210987654321");
        assert_eq!(b.abs().to_debug_string(), "98765432109876543210987654321");
        assert_eq!(a.compare_to(&b), 1);
        assert_eq!(
            BigInteger::value_of_long(-9223372036854775808).to_debug_string(),
            "-9223372036854775808"
        );
        assert_eq!(
            bi("-9223372036854775808").long_value_exact(),
            -9223372036854775808
        );
        assert_eq!(bi("-2147483648").int_value_exact(), -2147483648);
        assert_eq!(
            bi("123456789012345678901234567890").double_value(),
            1.2345678901234568E29
        );
        assert_eq!(
            bi("123456789012345678901234567890").float_value(),
            1.2345679E29
        );
    }

    /// Round trips through the byte two's-complement representation and the
    /// decimal/radix parsers (development-time oracle: JDK 17).
    #[test]
    fn big_integer_byte_and_radix_roundtrip_matches_the_oracle() {
        for text in ["0", "1", "-1", "255", "-256", "65537", "-65537"] {
            let value = bi(text);
            let bytes = value.to_byte_array();
            assert_eq!(BigInteger::from_byte_array(&bytes).to_debug_string(), text);
        }
        let value = bi("123456789012345678901234567890");
        assert_eq!(
            BigInteger::from_string_radix(&value.to_string_radix(36), 36).to_debug_string(),
            value.to_debug_string()
        );
        let value = bi("-98765432109876543210987654321");
        assert_eq!(
            BigInteger::from_string_radix(&value.to_string_radix(16), 16).to_debug_string(),
            value.to_debug_string()
        );
    }
}
