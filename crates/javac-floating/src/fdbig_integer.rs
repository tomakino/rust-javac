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

static LONG_5_POW: [i64; 28] = [
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
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
    5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5,
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
        if p5 != 0 {
            if p2 == 0 {
                return Self::big5pow(p5);
            } else if (p5 as usize) < SMALL_5_POW.len() {
                let pow5 = SMALL_5_POW[p5 as usize];
                let wordcount = p2 >> 5;
                let bitcount = p2 & 0x1f;
                if bitcount == 0 {
                    return FDBigInteger::new_from_int_array(vec![pow5], wordcount);
                } else {
                    return FDBigInteger::new_from_int_array(
                        vec![
                            pow5.wrapping_shl(bitcount as u32),
                            ((pow5 as u32) >> (32 - bitcount) as u32) as i32,
                        ],
                        wordcount,
                    );
                }
            } else {
                return Self::big5pow(p5).left_shift(p2);
            }
        } else {
            return Self::value_of_pow2(p2);
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
            if (p5 as usize) < SMALL_5_POW.len() {
                let pow5 = SMALL_5_POW[p5 as usize] as i64 & LONG_MASK;
                let mut carry = (v0 as i64 & LONG_MASK) * pow5;
                v0 = carry as i32;
                carry = (carry as u64 >> 32) as i64;
                carry = ((v1 as i64 & LONG_MASK) * pow5 + carry) as i64;
                v1 = carry as i32;
                let v2 = (carry as u64 >> 32) as i32;
                if bitcount == 0 {
                    return FDBigInteger::new_from_int_array(vec![v0, v1, v2], wordcount);
                } else {
                    return FDBigInteger::new_from_int_array(
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
                    );
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
                return FDBigInteger::new_from_int_array(r, pow5.offset).left_shift(p2);
            }
        } else if p2 != 0 {
            if bitcount == 0 {
                return FDBigInteger::new_from_int_array(vec![v0, v1], wordcount);
            } else {
                return FDBigInteger::new_from_int_array(
                    vec![
                        v0.wrapping_shl(bitcount as u32),
                        ((v1 as u32) << (bitcount as u32)
                            | ((v0 as u32) >> (32 - bitcount) as u32))
                            as i32,
                        (v1 as u32 >> (32 - bitcount) as u32) as i32,
                    ],
                    wordcount,
                );
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
            panic!("Zero value cannot be normalized");
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
            panic!("disparate values");
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
        assert!(p == 0, "{p}");
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
