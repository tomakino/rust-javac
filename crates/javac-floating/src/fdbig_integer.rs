// Origin: jdk/internal/math/FDBigInteger.java @ 982d53c (jdk-17.0.19-ga)

//! Port of \`jdk.internal.math.FDBigInteger\` (JDK 17).
//!
//! PORTING_RULES §9.17 (standard-library adapter):
//! * Java \`int[]\` words become \`Vec<i32>\`; Java \`long\` arithmetic uses \`i64\`;
//!   logical right shifts go through \`u32\`; every place where Java relies on
//!   two's-complement wraparound uses the explicit wrapping operations.
//! * \`util.Arrays.copyOf/fill\` map to \`Vec\` operations; assertions from the
//!   upstream source are kept as Rust \`assert!\`s.
//! * The CDS archive cache (archivedCaches/CDS.initializeFromArchive) is a JVM
//!   implementation detail; the cache is rebuilt deterministically with the same
//!   contents (registered adapter, no algorithmic change).

use std::sync::LazyLock;

/// Constant for casting an int to a long via bitwise AND.
const LONG_MASK: i64 = 0xffff_ffff;

/// Maximum size of cache of powers of 5 as FDBigIntegers.
const MAX_FIVE_POW: usize = 340;

static SMALL_5_POW: [i32; 14] = [
    1,
    5,
    5 * 5,
    5 * 5 * 5,
    5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
];

static LONG_5_POW: [i64; 27] = [
    1,
    5,
    25,
    125,
    625,
    3125,
    15625,
    78125,
    390625,
    1953125,
    9765625,
    48828125,
    244140625,
    1220703125,
    6103515625,
    30517578125,
    152587890625,
    762939453125,
    3814697265625,
    19073486328125,
    95367431640625,
    476837158203125,
    2384185791015625,
    11920928955078125,
    59604644775390625,
    298023223876953125,
    1490116119384765625,
];

/// Cache of big powers of 5 as FDBigIntegers.
static POW_5_CACHE: LazyLock<Vec<FDBigInteger>> = LazyLock::new(|| {
    let mut pow_5_cache: Vec<FDBigInteger> = Vec::with_capacity(MAX_FIVE_POW);
    let mut i = 0usize;
    while i < SMALL_5_POW.len() {
        let mut pow5 = FDBigInteger::new_from_int_array(vec![SMALL_5_POW[i]], 0);
        pow5.make_immutable();
        pow_5_cache.push(pow5);
        i += 1;
    }
    let mut prev = pow_5_cache[i - 1].clone();
    while i < MAX_FIVE_POW {
        prev = prev.mult_i(5);
        prev.make_immutable();
        pow_5_cache.push(prev.clone());
        i += 1;
    }
    pow_5_cache
});

/// Zero as an FDBigInteger.
static ZERO: LazyLock<FDBigInteger> = LazyLock::new(|| {
    let mut zero = FDBigInteger::new_from_int_array(Vec::new(), 0);
    zero.make_immutable();
    zero
});

/// A simple big integer package specifically for floating point base conversion.
#[derive(Clone, Debug)]
pub struct FDBigInteger {
    // value: data[0] is least significant
    data: Vec<i32>,
    // number of least significant zero padding ints
    offset: i32,
    // data[nWords-1]!=0, all values above are zero
    // if nWords==0 -> this FDBigInteger is zero
    n_words: i32,
    is_immutable: bool,
}

impl FDBigInteger {
    /// Java: private FDBigInteger(int[] data, int offset)
    fn new_from_int_array(data: Vec<i32>, offset: i32) -> FDBigInteger {
        let mut value = FDBigInteger {
            data,
            offset,
            n_words: 0,
            is_immutable: false,
        };
        value.n_words = value.data.len() as i32;
        value.trim_leading_zeros();
        value
    }

    /// Java: private static FDBigInteger valueOfPow2(int p2)
    fn value_of_pow2(p2: i32) -> FDBigInteger {
        let wordcount = p2 >> 5;
        let bitcount = p2 & 0x1f;
        FDBigInteger::new_from_int_array(vec![1i32.wrapping_shl(bitcount as u32)], wordcount)
    }

    /// Java: public static FDBigInteger valueOfPow52(int p5, int p2)
    pub(crate) fn value_of_pow52(p5: i32, p2: i32) -> FDBigInteger {
        return if p5 != 0 {
            if p2 == 0 {
                Self::big5pow(p5)
            } else if (p5 as usize) < SMALL_5_POW.len() {
                let pow5 = SMALL_5_POW[p5 as usize];
                let wordcount = p2 >> 5;
                let bitcount = p2 & 0x1f;
                if bitcount == 0 {
                    FDBigInteger::new_from_int_array(vec![pow5], wordcount)
                } else {
                    FDBigInteger::new_from_int_array(
                        vec![
                            pow5.wrapping_shl(bitcount as u32),
                            ((pow5 as u32) >> (32 - bitcount) as u32) as i32,
                        ],
                        wordcount,
                    )
                }
            } else {
                Self::big5pow(p5).left_shift(p2)
            }
        } else {
            Self::value_of_pow2(p2)
        }
    }

    /// Java: public static FDBigInteger valueOfMulPow52(long value, int p5, int p2)
    pub(crate) fn value_of_mul_pow52(value: i64, p5: i32, p2: i32) -> FDBigInteger {
        assert!(p5 >= 0, "{p5}");
        assert!(p2 >= 0, "{p2}");
        let mut v0 = value as i32;
        let mut v1 = ((value as u64) >> 32) as i32;
        let wordcount = p2 >> 5;
        let bitcount = p2 & 0x1f;
        if p5 != 0 {
            return if (p5 as usize) < SMALL_5_POW.len() {
                let pow5 = SMALL_5_POW[p5 as usize] as i64 & LONG_MASK;
                let mut carry = (v0 as i64 & LONG_MASK).wrapping_mul(pow5);
                v0 = carry as i32;
                carry = (carry as u64 >> 32) as i64;
                carry = (v1 as i64 & LONG_MASK).wrapping_mul(pow5).wrapping_add(carry);
                v1 = carry as i32;
                let v2 = (carry as u64 >> 32) as i32;
                if bitcount == 0 {
                    FDBigInteger::new_from_int_array(vec![v0, v1, v2], wordcount)
                } else {
                    FDBigInteger::new_from_int_array(
                        vec![
                            v0.wrapping_shl(bitcount as u32),
                            ((v1 as u32) << (bitcount as u32)
                                | ((v0 as u32) >> (32 - bitcount) as u32))
                                as i32,
                            ((v2 as u32) << (bitcount as u32)
                                | ((v1 as u32) >> (32 - bitcount) as u32))
                                as i32,
                            (v2 as u32 >> (32 - bitcount) as u32) as i32,
                        ],
                        wordcount,
                    )
                }
            } else {
                let pow5 = Self::big5pow(p5);
                let extra = if p2 != 0 { 1 } else { 0 };
                let mut r;
                if v1 == 0 {
                    r = vec![0; pow5.n_words as usize + 1 + extra];
                    Self::mult_arr_const_into(
                        &pow5.data,
                        pow5.n_words as usize,
                        v0,
                        &mut r,
                    );
                } else {
                    r = vec![0; pow5.n_words as usize + 2 + extra];
                    Self::mult_arr_const2_into(
                        &pow5.data,
                        pow5.n_words as usize,
                        v0,
                        v1,
                        &mut r,
                    );
                }
                FDBigInteger::new_from_int_array(r, pow5.offset).left_shift(p2)
            }
        } else if p2 != 0 {
            return if bitcount == 0 {
                FDBigInteger::new_from_int_array(vec![v0, v1], wordcount)
            } else {
                FDBigInteger::new_from_int_array(
                    vec![
                        v0.wrapping_shl(bitcount as u32),
                        ((v1 as u32) << (bitcount as u32)
                            | ((v0 as u32) >> (32 - bitcount) as u32))
                            as i32,
                        (v1 as u32 >> (32 - bitcount) as u32) as i32,
                    ],
                    wordcount,
                )
            }
        }
        FDBigInteger::new_from_int_array(vec![v0, v1], 0)
    }

    /// Java: private void trimLeadingZeros()
    fn trim_leading_zeros(&mut self) {
        let mut i = self.n_words;
        if i > 0 && self.data[(i - 1) as usize] == 0 {
            i -= 1;
            while i > 0 && self.data[(i - 1) as usize] == 0 {
                i -= 1;
            }
            self.n_words = i;
            if i == 0 {
                // all words are zero
                self.offset = 0;
            }
        }
    }

    /// Java: public int getNormalizationBias()
    pub(crate) fn get_normalization_bias(&self) -> i32 {
        if self.n_words == 0 {
            panic!("IllegalArgumentException: Zero value cannot be normalized");
        }
        let zeros = (self.data[(self.n_words - 1) as usize] as u32).leading_zeros() as i32;
        if zeros < 4 { 28 + zeros } else { zeros - 4 }
    }

    /// Java: private static void leftShift(int[] src, int idx, int result[],
    /// int bitcount, int anticount, int prev)
    fn left_shift_into(
        src: &[i32],
        mut idx: usize,
        result: &mut [i32],
        bitcount: u32,
        anticount: u32,
        mut prev: i32,
    ) {
        while idx > 0 {
            let mut v = prev.wrapping_shl(bitcount);
            prev = src[idx - 1];
            v |= ((prev as u32) >> anticount) as i32;
            result[idx] = v;
            idx -= 1;
        }
        let v = prev.wrapping_shl(bitcount);
        result[0] = v;
    }

    /// Java: public FDBigInteger leftShift(int shift)
    pub(crate) fn left_shift(&mut self, shift: i32) -> FDBigInteger {
        if shift == 0 || self.n_words == 0 {
            return self.clone();
        }
        let wordcount = shift >> 5;
        let bitcount = (shift & 0x1f) as u32;
        if self.is_immutable {
            if bitcount == 0 {
                return FDBigInteger::new_from_int_array(
                    self.data[..self.n_words as usize].to_vec(),
                    self.offset + wordcount,
                );
            }
            let anticount = 32 - bitcount;
            let idx = (self.n_words - 1) as usize;
            let prev = self.data[idx];
            let hi = ((prev as u32) >> anticount) as i32;
            let mut result;
            if hi != 0 {
                result = vec![0; self.n_words as usize + 1];
                result[self.n_words as usize] = hi;
            } else {
                result = vec![0; self.n_words as usize];
            }
            let src = self.data.clone();
            FDBigInteger::left_shift_into(
                &src,
                idx,
                &mut result,
                bitcount,
                anticount,
                prev,
            );
            FDBigInteger::new_from_int_array(result, self.offset + wordcount)
        } else {
            if bitcount != 0 {
                let anticount = 32 - bitcount;
                if self.data[0].wrapping_shl(bitcount) == 0 {
                    let mut idx = 0usize;
                    let mut prev = self.data[idx];
                    while idx < (self.n_words - 1) as usize {
                        let mut v = ((prev as u32) >> anticount) as i32;
                        prev = self.data[idx + 1];
                        v |= prev.wrapping_shl(bitcount);
                        self.data[idx] = v;
                        idx += 1;
                    }
                    let v = ((prev as u32) >> anticount) as i32;
                    self.data[idx] = v;
                    if v == 0 {
                        self.n_words -= 1;
                    }
                    self.offset += 1;
                } else {
                    let idx = (self.n_words - 1) as usize;
                    let prev = self.data[idx];
                    let hi = ((prev as u32) >> anticount) as i32;
                    if hi != 0 {
                        if self.n_words as usize == self.data.len() {
                            let mut result = vec![0; self.n_words as usize + 1];
                            let src = self.data.clone();
                            FDBigInteger::left_shift_into(
                                &src,
                                idx,
                                &mut result,
                                bitcount,
                                anticount,
                                prev,
                            );
                            result[self.n_words as usize] = hi;
                            self.data = result;
                            self.n_words += 1;
                        } else {
                            self.data[self.n_words as usize] = hi;
                            self.n_words += 1;
                            let mut i = idx;
                            let mut prev_local = self.data[idx];
                            while i > 0 {
                                let mut v = prev_local.wrapping_shl(bitcount);
                                prev_local = self.data[i - 1];
                                v |= ((prev_local as u32) >> anticount) as i32;
                                self.data[i] = v;
                                i -= 1;
                            }
                            self.data[0] = prev_local.wrapping_shl(bitcount);
                        }
                    } else {
                        let mut i = idx;
                        let mut prev_local = self.data[idx];
                        while i > 0 {
                            let mut v = prev_local.wrapping_shl(bitcount);
                            prev_local = self.data[i - 1];
                            v |= ((prev_local as u32) >> anticount) as i32;
                            self.data[i] = v;
                            i -= 1;
                        }
                        self.data[0] = prev_local.wrapping_shl(bitcount);
                    }
                }
            }
            self.offset += wordcount;
            self.clone()
        }
    }

    /// Java: private int size()
    fn size(&self) -> i32 {
        self.n_words + self.offset
    }

    /// Java: public int quoRemIteration(FDBigInteger S)
    pub(crate) fn quo_rem_iteration(&mut self, s: &FDBigInteger) -> i32 {
        assert!(!self.is_immutable, "cannot modify immutable value");
        // ensure that this and S have the same number of
        // digits. If S is properly normalized and q < 10 then
        // this must be so.
        let th_size = self.size();
        let s_size = s.size();
        if th_size < s_size {
            // this value is significantly less than S, result of division is zero.
            // just mult this by 10.
            let p = Self::mult_and_carry_by_10_in_place(
                &mut self.data,
                self.n_words as usize,
            );
            if p != 0 {
                self.data[self.n_words as usize] = p;
                self.n_words += 1;
            } else {
                self.trim_leading_zeros();
            }
            return 0;
        } else if th_size > s_size {
            panic!("IllegalArgumentException: disparate values");
        }
        // estimate q the obvious way. We will usually be
        // right. If not, then we're only off by a little and
        // will re-add.
        let q = (self.data[(self.n_words - 1) as usize] as u32 as u64)
            / (s.data[(s.n_words - 1) as usize] as u32 as u64);
        let mut q = q as i64;
        let mut diff = self.mult_diff_me(q, s);
        if diff != 0 {
            // q is too big.
            // add S back in until this turns +. This should
            // not be very many times!
            let mut sum: i64 = 0;
            let t_start = s.offset - self.offset;
            let sd = &s.data;
            while sum == 0 {
                let mut s_index = 0usize;
                let mut t_index = t_start as usize;
                while t_index < self.n_words as usize {
                    sum += (self.data[t_index] as u32 as i64) + (sd[s_index] as u32 as i64);
                    self.data[t_index] = sum as i32;
                    sum = (sum as u64 >> 32) as i64; // Signed or unsigned, answer is 0 or 1
                    s_index += 1;
                    t_index += 1;
                }
                //
                // Originally the following line read
                // "if ( sum !=0 && sum != -1 )"
                // but that would be wrong, because of the
                // treatment of the two values as entirely unsigned,
                // it would be impossible for a carry-out to be interpreted
                // as -1 -- it would have to be a single-bit carry-out, or +1.
                //
                assert!(sum == 0 || sum == 1, "{sum}");
                q -= 1;
            }
            let _ = &mut diff;
        }
        // finally, we can multiply this by 10.
        // it cannot overflow, right, as the high-order word has
        // at least 4 high-order zeros!
        let p = Self::mult_and_carry_by_10_in_place(&mut self.data, self.n_words as usize);
        assert_eq!(p, 0, "{p}");
        self.trim_leading_zeros();
        q as i32
    }

    /// Java: public FDBigInteger multBy10()
    pub(crate) fn mult_by10(&mut self) -> FDBigInteger {
        if self.n_words == 0 {
            return self.clone();
        }
        if self.is_immutable {
            let mut res = vec![0; self.n_words as usize + 1];
            let carry =
                Self::mult_and_carry_by_10_into(&self.data, self.n_words as usize, &mut res);
            res[self.n_words as usize] = carry;
            FDBigInteger::new_from_int_array(res, self.offset)
        } else {
            let p = Self::mult_and_carry_by_10_in_place(&mut self.data, self.n_words as usize);
            if p != 0 {
                if self.n_words as usize == self.data.len() {
                    if self.data[0] == 0 {
                        self.data.copy_within(1.., 0);
                        self.n_words -= 1;
                        self.offset += 1;
                    } else {
                        self.data.push(0);
                    }
                }
                self.data[self.n_words as usize] = p;
                self.n_words += 1;
            } else {
                self.trim_leading_zeros();
            }
            self.clone()
        }
    }

    /// Java: public FDBigInteger multByPow52(int p5, int p2)
    pub(crate) fn mult_by_pow52(&mut self, p5: i32, p2: i32) -> FDBigInteger {
        if self.n_words == 0 {
            return self.clone();
        }
        let mut res = self.clone();
        if p5 != 0 {
            let extra = if p2 != 0 { 1 } else { 0 };
            if (p5 as usize) < SMALL_5_POW.len() {
                let mut r = vec![0; self.n_words as usize + 1 + extra];
                Self::mult_arr_const_into(
                    &self.data,
                    self.n_words as usize,
                    SMALL_5_POW[p5 as usize],
                    &mut r,
                );
                res = FDBigInteger::new_from_int_array(r, self.offset);
            } else {
                let pow5 = Self::big5pow(p5);
                let mut r = vec![0; (self.n_words + pow5.size() + extra as i32) as usize];
                Self::mult_arrays_into(
                    &self.data,
                    self.n_words as usize,
                    &pow5.data,
                    pow5.n_words as usize,
                    &mut r,
                );
                res = FDBigInteger::new_from_int_array(r, self.offset + pow5.offset);
            }
        }
        res.left_shift(p2)
    }

    /// Java: private static void mult(int[] s1, int s1Len, int[] s2, int s2Len,
    /// int[] dst)
    fn mult_arrays_into(
        s1: &[i32],
        s1_len: usize,
        s2: &[i32],
        s2_len: usize,
        dst: &mut [i32],
    ) {
        for i in 0..s1_len {
            let v = s1[i] as u32 as u64;
            let mut p: u64 = 0;
            for j in 0..s2_len {
                p += (dst[i + j] as u32 as u64) + v * (s2[j] as u32 as u64);
                dst[i + j] = p as i32;
                p >>= 32;
            }
            dst[i + s2_len] = p as i32;
        }
    }

    /// Java: public FDBigInteger leftInplaceSub(FDBigInteger subtrahend)
    ///
    /// PORTING_RULES §9.17 adaptation: upstream mutates and returns \`this\`
    /// when it is not immutable; the Rust port returns an equal value instead
    /// (value semantics), keeping every arithmetic step 1:1.
    pub(crate) fn left_inplace_sub(&self, subtrahend: &FDBigInteger) -> FDBigInteger {
        assert!(self.size() >= subtrahend.size(), "result should be positive");
        let mut minuend = self.clone();
        let mut offset_diff = subtrahend.offset - minuend.offset;
        let sub_len = subtrahend.n_words as usize;
        if offset_diff < 0 {
            // need to expand minuend
            let min_len = minuend.n_words as usize;
            let r_len = (min_len as i32 - offset_diff) as usize;
            if r_len < minuend.data.len() {
                let shift = (-offset_diff) as usize;
                minuend.data.copy_within(0..min_len, shift);
                for x in &mut minuend.data[0..shift] {
                    *x = 0;
                }
            } else {
                let mut r = vec![0i32; r_len];
                let shift = (-offset_diff) as usize;
                r[shift..shift + min_len].copy_from_slice(&minuend.data[..min_len]);
                minuend.data = r;
            }
            minuend.offset = subtrahend.offset;
            minuend.n_words = r_len as i32;
            offset_diff = 0;
        }
        let mut borrow: i64 = 0;
        let mut m_index = offset_diff as usize;
        let s_data = &subtrahend.data;
        let min_len = minuend.n_words as usize;
        let mut s_index = 0usize;
        while s_index < sub_len && m_index < min_len {
            let diff = (minuend.data[m_index] as u32 as i64)
                - (s_data[s_index] as u32 as i64)
                + borrow;
            minuend.data[m_index] = diff as i32;
            borrow = diff >> 32; // signed shift
            s_index += 1;
            m_index += 1;
        }
        while borrow != 0 && m_index < min_len {
            let diff = (minuend.data[m_index] as u32 as i64) + borrow;
            minuend.data[m_index] = diff as i32;
            borrow = diff >> 32; // signed shift
            m_index += 1;
        }
        assert_eq!(borrow, 0, "{borrow}");
        // result should be positive
        minuend.trim_leading_zeros();
        minuend
    }

    /// Java: public FDBigInteger rightInplaceSub(FDBigInteger subtrahend)
    ///
    /// PORTING_RULES §9.17 adaptation: upstream mutates the \`subtrahend\`
    /// argument and returns it; the Rust port takes it by value and returns the
    /// mutated value (value semantics), keeping every arithmetic step 1:1.
    // PORTING_RULES §9.17: upstream assigns `subLen -= offsetDiff` but does
    // not read subLen afterwards; the assignment is kept 1:1.
    #[allow(unused_assignments)]
    pub(crate) fn right_inplace_sub(&self, mut subtrahend: FDBigInteger) -> FDBigInteger {
        assert!(self.size() >= subtrahend.size(), "result should be positive");
        let minuend = self;
        let mut offset_diff = minuend.offset - subtrahend.offset;
        let mut sub_len = subtrahend.n_words as usize;
        let min_len = minuend.n_words as usize;
        if offset_diff < 0 {
            let r_len = min_len;
            if r_len < subtrahend.data.len() {
                let shift = (-offset_diff) as usize;
                subtrahend.data.copy_within(0..sub_len, shift);
                for x in &mut subtrahend.data[0..shift] {
                    *x = 0;
                }
            } else {
                let mut r = vec![0i32; r_len];
                let shift = (-offset_diff) as usize;
                r[shift..shift + sub_len].copy_from_slice(&subtrahend.data[..sub_len]);
                subtrahend.data = r;
            }
            subtrahend.offset = minuend.offset;
            sub_len = (sub_len as i32 - offset_diff) as usize;
            offset_diff = 0;
        } else {
            let r_len = min_len + offset_diff as usize;
            if r_len >= subtrahend.data.len() {
                subtrahend.data.resize(r_len, 0);
            }
        }
        let s_data = &mut subtrahend.data;
        let m_data = &minuend.data;
        let mut s_index = 0usize;
        let mut borrow: i64 = 0;
        while s_index < offset_diff as usize {
            let diff = 0i64 - (s_data[s_index] as u32 as i64) + borrow;
            s_data[s_index] = diff as i32;
            borrow = diff >> 32; // signed shift
            s_index += 1;
        }
        let mut m_index = 0usize;
        while m_index < min_len {
            let diff = (m_data[m_index] as u32 as i64)
                - (s_data[s_index] as u32 as i64)
                + borrow;
            s_data[s_index] = diff as i32;
            borrow = diff >> 32; // signed shift
            s_index += 1;
            m_index += 1;
        }
        assert_eq!(borrow, 0, "{borrow}");
        subtrahend.n_words = s_index as i32;
        subtrahend.trim_leading_zeros();
        subtrahend
    }

    /// Java: private static int checkZeroTail(int[] a, int from)
    fn check_zero_tail(a: &[i32], from: usize) -> i32 {
        let mut from = from;
        while from > 0 {
            from -= 1;
            if a[from] != 0 {
                return 1;
            }
        }
        0
    }

    /// Java: public int cmp(FDBigInteger other)
    pub(crate) fn cmp(&self, other: &FDBigInteger) -> i32 {
        let a_size = self.n_words + self.offset;
        let b_size = other.n_words + other.offset;
        if a_size > b_size {
            return 1;
        } else if a_size < b_size {
            return -1;
        }
        let mut a_len = self.n_words;
        let mut b_len = other.n_words;
        while a_len > 0 && b_len > 0 {
            a_len -= 1;
            b_len -= 1;
            let a = self.data[a_len as usize];
            let b = other.data[b_len as usize];
            if a != b {
                return if (a as u32 as u64) < (b as u32 as u64) { -1 } else { 1 };
            }
        }
        if a_len > 0 {
            return Self::check_zero_tail(&self.data, a_len as usize);
        }
        if b_len > 0 {
            return -Self::check_zero_tail(&other.data, b_len as usize);
        }
        0
    }

    /// Java: public int cmpPow52(int p5, int p2)
    pub(crate) fn cmp_pow52(&self, p5: i32, p2: i32) -> i32 {
        if p5 == 0 {
            let wordcount = p2 >> 5;
            let bitcount = p2 & 0x1f;
            let size = self.n_words + self.offset;
            if size > wordcount + 1 {
                return 1;
            } else if size < wordcount + 1 {
                return -1;
            }
            let a = self.data[(self.n_words - 1) as usize];
            let b = 1i32.wrapping_shl(bitcount as u32);
            if a != b {
                return if (a as u32 as u64) < (b as u32 as u64) { -1 } else { 1 };
            }
            return Self::check_zero_tail(&self.data, (self.n_words - 1) as usize);
        }
        let mut shifted = Self::big5pow(p5);
        let shifted = shifted.left_shift(p2);
        self.cmp(&shifted)
    }

    /// Java: public int addAndCmp(FDBigInteger x, FDBigInteger y)
    pub(crate) fn add_and_cmp(&self, x: &FDBigInteger, y: &FDBigInteger) -> i32 {
        let x_size = x.size();
        let y_size = y.size();
        let (big, small, b_size, s_size);
        if x_size >= y_size {
            big = x;
            small = y;
            b_size = x_size;
            s_size = y_size;
        } else {
            big = y;
            small = x;
            b_size = y_size;
            s_size = x_size;
        }
        let th_size = self.size();
        if b_size == 0 {
            return if th_size == 0 { 0 } else { 1 };
        }
        if s_size == 0 {
            return self.cmp(big);
        }
        if b_size > th_size {
            return -1;
        }
        if b_size + 1 < th_size {
            return 1;
        }
        let mut top = big.data[(big.n_words - 1) as usize] as u32 as i64;
        if s_size == b_size {
            top += small.data[(small.n_words - 1) as usize] as u32 as i64;
        }
        if ((top as u64) >> 32) == 0 {
            if (((top + 1) as u64) >> 32) == 0 {
                // good case - no carry extension
                if b_size < th_size {
                    return 1;
                }
                // here sum.nWords == this.nWords
                let v = self.data[(self.n_words - 1) as usize] as u32 as i64;
                if v < top {
                    return -1;
                }
                if v > top + 1 {
                    return 1;
                }
            }
        } else {
            // (top>>>32)!=0 guaranteed carry extension
            if b_size + 1 > th_size {
                return -1;
            }
            // here sum.nWords == this.nWords
            top = (top as u64 >> 32) as i64;
            let v = self.data[(self.n_words - 1) as usize] as u32 as i64;
            if v < top {
                return -1;
            }
            if v > top + 1 {
                return 1;
            }
        }
        self.cmp(&big.add(small))
    }

    /// Java: public void makeImmutable()
    pub(crate) fn make_immutable(&mut self) {
        self.is_immutable = true;
    }

    /// Java: private FDBigInteger mult(int i)
    fn mult_i(&self, i: i32) -> FDBigInteger {
        if self.n_words == 0 {
            return self.clone();
        }
        let mut r = vec![0; self.n_words as usize + 1];
        Self::mult_arr_const_into(&self.data, self.n_words as usize, i, &mut r);
        FDBigInteger::new_from_int_array(r, self.offset)
    }

    /// Java: private FDBigInteger mult(FDBigInteger other)
    fn mult_fdbig(&self, other: &FDBigInteger) -> FDBigInteger {
        if self.n_words == 0 {
            return self.clone();
        }
        if self.size() == 1 {
            return other.mult_i(self.data[0]);
        }
        if other.n_words == 0 {
            return other.clone();
        }
        if other.size() == 1 {
            return self.mult_i(other.data[0]);
        }
        let mut r = vec![0; (self.n_words + other.n_words) as usize];
        Self::mult_arrays_into(
            &self.data,
            self.n_words as usize,
            &other.data,
            other.n_words as usize,
            &mut r,
        );
        FDBigInteger::new_from_int_array(r, self.offset + other.offset)
    }

    /// Java: private FDBigInteger add(FDBigInteger other)
    fn add(&self, other: &FDBigInteger) -> FDBigInteger {
        let t_size = self.size();
        let o_size = other.size();
        let (big, big_len, small, small_len);
        if t_size >= o_size {
            big = self;
            big_len = t_size;
            small = other;
            small_len = o_size;
        } else {
            big = other;
            big_len = o_size;
            small = self;
            small_len = t_size;
        }
        let mut r = vec![0i32; big_len as usize + 1];
        let mut i = 0usize;
        let mut carry: i64 = 0;
        while i < small_len as usize {
            carry += (if i < big.offset as usize {
                0i64
            } else {
                big.data[i - big.offset as usize] as u32 as i64
            }) + (if i < small.offset as usize {
                0i64
            } else {
                small.data[i - small.offset as usize] as u32 as i64
            });
            r[i] = carry as i32;
            carry >>= 32; // signed shift.
            i += 1;
        }
        while i < big_len as usize {
            carry += if i < big.offset as usize {
                0i64
            } else {
                big.data[i - big.offset as usize] as u32 as i64
            };
            r[i] = carry as i32;
            carry >>= 32; // signed shift.
            i += 1;
        }
        r[big_len as usize] = carry as i32;
        FDBigInteger::new_from_int_array(r, 0)
    }

    /// Java: private void multAddMe(int iv, int addend)
    fn mult_add_me(&mut self, iv: i32, addend: i32) {
        let v = iv as u32 as u64;
        // unroll 0th iteration, doing addition.
        let mut p = v * (self.data[0] as u32 as u64) + (addend as u32 as u64);
        self.data[0] = p as i32;
        p >>= 32;
        for i in 1..self.n_words as usize {
            p += v * (self.data[i] as u32 as u64);
            self.data[i] = p as i32;
            p >>= 32;
        }
        if p != 0 {
            self.data[self.n_words as usize] = p as i32; // will fail noisily if illegal!
            self.n_words += 1;
        }
    }

    //
    // original doc:
    //
    // do this -=q*S
    // returns borrow
    //
    /// Java: private long multDiffMe(long q, FDBigInteger S)
    fn mult_diff_me(&mut self, q: i64, s: &FDBigInteger) -> i64 {
        let mut diff: i64 = 0;
        if q != 0 {
            let mut delta_size = s.offset - self.offset;
            if delta_size >= 0 {
                let sd = &s.data;
                let td = &mut self.data;
                let mut s_index = 0usize;
                let mut t_index = delta_size as usize;
                while s_index < s.n_words as usize {
                    diff = diff.wrapping_add(
                        (td[t_index] as u32 as i64)
                            .wrapping_sub(q.wrapping_mul(sd[s_index] as u32 as i64)),
                    );
                    td[t_index] = diff as i32;
                    diff >>= 32; // N.B. SIGNED shift.
                    s_index += 1;
                    t_index += 1;
                }
            } else {
                delta_size = -delta_size;
                let mut rd = vec![0i32; (self.n_words + delta_size) as usize];
                let mut s_index = 0usize;
                let mut r_index = 0usize;
                let sd = &s.data;
                while r_index < delta_size as usize && s_index < s.n_words as usize {
                    diff = diff.wrapping_sub(q.wrapping_mul(sd[s_index] as u32 as i64));
                    rd[r_index] = diff as i32;
                    diff >>= 32; // N.B. SIGNED shift.
                    s_index += 1;
                    r_index += 1;
                }
                let mut t_index = 0usize;
                let td = &self.data;
                while s_index < s.n_words as usize {
                    diff = diff.wrapping_add(
                        (td[t_index] as u32 as i64)
                            .wrapping_sub(q.wrapping_mul(sd[s_index] as u32 as i64)),
                    );
                    rd[r_index] = diff as i32;
                    diff >>= 32; // N.B. SIGNED shift.
                    s_index += 1;
                    t_index += 1;
                    r_index += 1;
                }
                self.n_words += delta_size;
                self.offset -= delta_size;
                self.data = rd;
            }
        }
        diff
    }

    /// Java: private static int multAndCarryBy10(int[] src, int srcLen,
    /// int[] dst)
    fn mult_and_carry_by_10_into(src: &[i32], src_len: usize, dst: &mut [i32]) -> i32 {
        let mut carry: u64 = 0;
        for i in 0..src_len {
            let product = (src[i] as u32 as u64) * 10u64 + carry;
            dst[i] = product as i32;
            carry = product >> 32;
        }
        carry as i32
    }

    /// Java: private static int multAndCarryBy10(int[] src, int srcLen,
    /// int[] dst), in-place form (src and dst are the same array upstream).
    fn mult_and_carry_by_10_in_place(data: &mut [i32], src_len: usize) -> i32 {
        let mut carry: u64 = 0;
        for i in 0..src_len {
            let product = (data[i] as u32 as u64) * 10u64 + carry;
            data[i] = product as i32;
            carry = product >> 32;
        }
        carry as i32
    }

    /// Java: private static void mult(int[] src, int srcLen, int value,
    /// int[] dst)
    fn mult_arr_const_into(src: &[i32], src_len: usize, value: i32, dst: &mut [i32]) {
        let val = value as u32 as u64;
        let mut carry: u64 = 0;
        for i in 0..src_len {
            let product = (src[i] as u32 as u64) * val + carry;
            dst[i] = product as i32;
            carry = product >> 32;
        }
        dst[src_len] = carry as i32;
    }

    /// Java: private static void mult(int[] src, int srcLen, int v0, int v1,
    /// int[] dst)
    fn mult_arr_const2_into(
        src: &[i32],
        src_len: usize,
        v0: i32,
        v1: i32,
        dst: &mut [i32],
    ) {
        let v = v0 as u32 as u64;
        let mut carry: u64 = 0;
        for j in 0..src_len {
            let product = v * (src[j] as u32 as u64) + carry;
            dst[j] = product as i32;
            carry = product >> 32;
        }
        dst[src_len] = carry as i32;
        let v = v1 as u32 as u64;
        let mut carry: u64 = 0;
        for j in 0..src_len {
            let product = (dst[j + 1] as u32 as u64) + v * (src[j] as u32 as u64) + carry;
            dst[j + 1] = product as i32;
            carry = product >> 32;
        }
        dst[src_len + 1] = carry as i32;
    }

    // Fails assertion for negative exponent.
    /// Java: private static FDBigInteger big5pow(int p)
    fn big5pow(p: i32) -> FDBigInteger {
        assert!(p >= 0, "{p}"); // negative power of 5
        if (p as usize) < MAX_FIVE_POW {
            return POW_5_CACHE[p as usize].clone();
        }
        Self::big5pow_rec(p)
    }

    // slow path
    /// Java: private static FDBigInteger big5powRec(int p)
    fn big5pow_rec(p: i32) -> FDBigInteger {
        if (p as usize) < MAX_FIVE_POW {
            return POW_5_CACHE[p as usize].clone();
        }
        // construct the value.
        // recursively.
        // in order to compute 5^p,
        // compute its square root, 5^(p/2) and square.
        // or, let q = p / 2, r = p -q, then
        // 5^p = 5^(q+r) = 5^q * 5^r
        let q = p >> 1;
        let r = p - q;
        let bigq = Self::big5pow_rec(q);
        if (r as usize) < SMALL_5_POW.len() {
            bigq.mult_i(SMALL_5_POW[r as usize])
        } else {
            bigq.mult_fdbig(&Self::big5pow_rec(r))
        }
    }

    // for debugging ...
    /// Java: public String toHexString()
    #[allow(dead_code)]
    pub(crate) fn to_hex_string(&self) -> String {
        if self.n_words == 0 {
            return "0".to_string();
        }
        let mut sb = String::with_capacity(((self.n_words + self.offset) * 8) as usize);
        let mut i = self.n_words - 1;
        while i >= 0 {
            let sub_str = format!("{:x}", self.data[i as usize] as u32);
            for _ in sub_str.len()..8 {
                sb.push('0');
            }
            sb.push_str(&sub_str);
            i -= 1;
        }
        for _ in 0..self.offset {
            sb.push_str("00000000");
        }
        sb
    }

    // for debugging ...
    /// Java: public BigInteger toBigInteger().
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - the value as a BigInteger.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: FDBigInteger.java:toBigInteger.
    pub(crate) fn to_big_integer(&self) -> crate::big_integer::BigInteger {
        let n_words = self.n_words as usize;
        let mut magnitude = vec![0u8; n_words * 4 + 1];
        let len = magnitude.len();
        for i in 0..n_words {
            let w = self.data[i];
            magnitude[len - 4 * i - 1] = w as u8;
            magnitude[len - 4 * i - 2] = (w >> 8) as u8;
            magnitude[len - 4 * i - 3] = (w >> 16) as u8;
            magnitude[len - 4 * i - 4] = (w >> 24) as u8;
        }
        crate::big_integer::BigInteger::from_byte_array(&magnitude).shift_left(self.offset * 32)
    }

    /// Java: public String toString().
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - the decimal string.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: FDBigInteger.java:toString.
    pub(crate) fn to_string_debug(&self) -> String {
        self.to_big_integer().to_debug_string()
    }

    /// Java: zero as an FDBigInteger (upstream static ZERO field).
    pub(crate) fn zero() -> FDBigInteger {
        ZERO.clone()
    }

    /// Java: public FDBigInteger(long lValue, char[] digits, int kDigits,
    /// int nDigits)
    pub(crate) fn new_with_digits(
        l_value: i64,
        digits: &[u16],
        k_digits: usize,
        n_digits: usize,
    ) -> FDBigInteger {
        let n = (n_digits + 8) / 9;
        let n = if n < 2 { 2 } else { n }; // estimate size needed.
        let mut data = vec![0i32; n]; // allocate enough space
        data[0] = l_value as i32; // starting value
        data[1] = ((l_value as u64) >> 32) as i32;
        let mut value = FDBigInteger {
            data,
            offset: 0,
            n_words: 2,
            is_immutable: false,
        };
        let mut i = k_digits as i64;
        let limit = n_digits as i64 - 5; // slurp digits 5 at a time.
        while i < limit {
            let ilim = i + 5;
            let mut v = (digits[i as usize] - b'0' as u16) as i32;
            i += 1;
            while i < ilim {
                v = 10 * v + (digits[i as usize] - b'0' as u16) as i32;
                i += 1;
            }
            value.mult_add_me(100000, v); // ... where 100000 is 10^5.
        }
        let mut factor = 1i32;
        let mut v = 0i32;
        while i < n_digits as i64 {
            v = 10 * v + (digits[i as usize] - b'0' as u16) as i32;
            i += 1;
            factor *= 10;
        }
        if factor != 1 {
            value.mult_add_me(factor, v);
        }
        value.trim_leading_zeros();
        value
    }
}


/// Read-only accessors for the upstream static power tables (FloatingDecimal
/// accesses FDBigInteger.SMALL_5_POW / LONG_5_POW directly).
pub(crate) fn small5pow(i: i32) -> i32 {
    SMALL_5_POW[i as usize]
}

pub(crate) fn long5pow(i: i32) -> i64 {
    LONG_5_POW[i as usize]
}

pub(crate) fn long5pow_len() -> usize {
    LONG_5_POW.len()
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream FDBigInteger.toBigInteger/toString (development-time oracle:
    /// 5^3 * 2^2 = 500).
    #[test]
    fn to_big_integer_and_to_string_use_the_integer_value() {
        let value = FDBigInteger::value_of_pow52(3, 2);
        assert_eq!(value.to_string_debug(), "500");
        assert_eq!(
            value.to_big_integer().to_debug_string(),
            "500"
        );
        assert_eq!(FDBigInteger::zero().to_string_debug(), "0");
    }
}
