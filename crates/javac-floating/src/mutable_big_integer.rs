//! Port of `java.math.MutableBigInteger` (JDK 17), the mutable magnitude helper
//! used by `java.math.BigInteger`.
//!
//! PORTING_RULES (standard-library adapter):
//! * Java `int[]` magnitude words become `Vec<i32>`; `long` arithmetic uses
//!   `i64` and two's-complement wraparound is explicit (`wrapping_*`, `as u32`
//!   for logical shifts).
//! * `java.util.Arrays.copyOf/fill/copyOfRange` and `System.arraycopy` map to
//!   `Vec` operations; Java `assert` statements are kept as `assert!`s.
//! * Methods returning Java `null` for "no remainder" return `Option`.
//! * `toBigDecimal`/`toCompactValue` are not ported: they depend on
//!   `java.math.BigDecimal`, which is outside this port (registered; not used by
//!   `BigInteger`).

use crate::big_integer::{BigInteger, SignedMutableBigInteger};

/// Java: static final MutableBigInteger ONE.
pub(crate) fn one() -> MutableBigInteger {
    MutableBigInteger::new_int(1)
}

/// Java: static final int KNUTH_POW2_THRESH_LEN.
const KNUTH_POW2_THRESH_LEN: i32 = 6;

/// Java: static final int KNUTH_POW2_THRESH_ZEROS.
const KNUTH_POW2_THRESH_ZEROS: i32 = 3;

/// Java: class MutableBigInteger.
#[derive(Clone, Debug)]
pub(crate) struct MutableBigInteger {
    /// The magnitude in big-endian order, possibly with an offset and unused
    /// trailing words (Java: int[] value).
    pub(crate) value: Vec<i32>,
    /// The number of ints of the value array used for the magnitude.
    pub(crate) int_len: i32,
    /// The offset into the value array where the magnitude begins.
    pub(crate) offset: i32,
}

impl MutableBigInteger {
    /// Java: MutableBigInteger() — an empty value with one word of capacity.
    pub(crate) fn new_empty() -> MutableBigInteger {
        MutableBigInteger {
            value: vec![0],
            int_len: 0,
            offset: 0,
        }
    }

    /// Java: MutableBigInteger(int val).
    pub(crate) fn new_int(val: i32) -> MutableBigInteger {
        MutableBigInteger {
            value: vec![val],
            int_len: 1,
            offset: 0,
        }
    }

    /// Java: MutableBigInteger(int[] val).
    pub(crate) fn new_from_vec(val: Vec<i32>) -> MutableBigInteger {
        let int_len = val.len() as i32;
        MutableBigInteger {
            value: val,
            int_len,
            offset: 0,
        }
    }

    /// Java: MutableBigInteger(BigInteger b).
    pub(crate) fn new_from_big_integer(b: &BigInteger) -> MutableBigInteger {
        let mag = b.magnitude();
        let int_len = mag.len() as i32;
        MutableBigInteger {
            value: mag.clone(),
            int_len,
            offset: 0,
        }
    }

    /// Java: MutableBigInteger(MutableBigInteger val).
    pub(crate) fn new_from_mutable(val: &MutableBigInteger) -> MutableBigInteger {
        let int_len = val.int_len;
        let start = val.offset as usize;
        let end = start + int_len as usize;
        MutableBigInteger {
            value: val.value[start..end].to_vec(),
            int_len,
            offset: 0,
        }
    }

    /// Java: private void ones(int n).
    fn ones(&mut self, n: i32) {
        if n as usize > self.value.len() {
            self.value = vec![0; n as usize];
        }
        for slot in self.value.iter_mut() {
            *slot = -1;
        }
        self.offset = 0;
        self.int_len = n;
    }

    /// Java: private int[] getMagnitudeArray().
    ///
    /// The port returns a copy (Java returns the internal array); the callers
    /// convert to BigInteger and do not rely on the aliasing (registered).
    fn get_magnitude_array(&mut self) -> Vec<i32> {
        if self.offset > 0 || self.value.len() as i32 != self.int_len {
            let start = self.offset as usize;
            let end = start + self.int_len as usize;
            let tmp = self.value[start..end].to_vec();
            for slot in self.value.iter_mut() {
                *slot = 0;
            }
            self.offset = 0;
            self.int_len = tmp.len() as i32;
            self.value = tmp;
        }
        self.value.clone()
    }

    /// Java: private long toLong().
    fn to_long(&self) -> i64 {
        assert!(
            self.int_len <= 2,
            "this MutableBigInteger exceeds the range of long"
        );
        if self.int_len == 0 {
            return 0;
        }
        let d = self.value[self.offset as usize] as i64 & crate::big_integer::LONG_MASK;
        if self.int_len == 2 {
            (d << 32) | (self.value[self.offset as usize + 1] as i64 & crate::big_integer::LONG_MASK)
        } else {
            d
        }
    }

    /// Java: BigInteger toBigInteger(int sign).
    pub(crate) fn to_big_integer_sign(&mut self, sign: i32) -> BigInteger {
        if self.int_len == 0 || sign == 0 {
            return BigInteger::zero();
        }
        BigInteger::new_from_mag(self.get_magnitude_array(), sign)
    }

    /// Java: BigInteger toBigInteger().
    pub(crate) fn to_big_integer(&mut self) -> BigInteger {
        self.normalize();
        if self.is_zero() {
            self.to_big_integer_sign(0)
        } else {
            self.to_big_integer_sign(1)
        }
    }

    /// Java: void clear().
    fn clear(&mut self) {
        self.offset = 0;
        self.int_len = 0;
        for slot in self.value.iter_mut() {
            *slot = 0;
        }
    }

    /// Java: void reset().
    fn reset(&mut self) {
        self.offset = 0;
        self.int_len = 0;
    }

    /// Java: final int compare(MutableBigInteger b).
    pub(crate) fn compare(&self, b: &MutableBigInteger) -> i32 {
        let blen = b.int_len;
        if self.int_len < blen {
            return -1;
        }
        if self.int_len > blen {
            return 1;
        }
        let bval = &b.value;
        let mut i = self.offset as usize;
        let mut j = b.offset as usize;
        let end = self.offset + self.int_len;
        while (i as i32) < end {
            let b1 = self.value[i].wrapping_add(0x80000000u32 as i32);
            let b2 = bval[j].wrapping_add(0x80000000u32 as i32);
            if b1 < b2 {
                return -1;
            }
            if b1 > b2 {
                return 1;
            }
            i += 1;
            j += 1;
        }
        0
    }

    /// Java: private int compareShifted(MutableBigInteger b, int ints).
    fn compare_shifted(&self, b: &MutableBigInteger, ints: i32) -> i32 {
        let blen = b.int_len;
        let alen = self.int_len - ints;
        if alen < blen {
            return -1;
        }
        if alen > blen {
            return 1;
        }
        let bval = &b.value;
        let mut i = self.offset as usize;
        let mut j = b.offset as usize;
        let end = alen + self.offset;
        while (i as i32) < end {
            let b1 = self.value[i].wrapping_add(0x80000000u32 as i32);
            let b2 = bval[j].wrapping_add(0x80000000u32 as i32);
            if b1 < b2 {
                return -1;
            }
            if b1 > b2 {
                return 1;
            }
            i += 1;
            j += 1;
        }
        0
    }

    /// Java: final int compareHalf(MutableBigInteger b).
    pub(crate) fn compare_half(&self, b: &MutableBigInteger) -> i32 {
        let blen = b.int_len;
        let len = self.int_len;
        if len <= 0 {
            return if blen <= 0 { 0 } else { -1 };
        }
        if len > blen {
            return 1;
        }
        if len < blen - 1 {
            return -1;
        }
        let bval = &b.value;
        let mut bstart: usize = 0;
        let mut carry: i32 = 0;
        if len != blen {
            if bval[bstart] == 1 {
                bstart += 1;
                carry = 0x80000000u32 as i32;
            } else {
                return -1;
            }
        }
        let val = &self.value;
        let mut i = self.offset as usize;
        let mut j = bstart;
        while (i as i32) < len + self.offset {
            let bv = bval[j];
            j += 1;
            let hb = (((bv as u32) >> 1) as i64 + carry as i64) & crate::big_integer::LONG_MASK;
            let v = val[i] as i64 & crate::big_integer::LONG_MASK;
            i += 1;
            if v != hb {
                return if v < hb { -1 } else { 1 };
            }
            carry = (bv & 1) << 31;
        }
        if carry == 0 { 0 } else { -1 }
    }

    /// Java: private final int getLowestSetBit().
    fn get_lowest_set_bit(&self) -> i32 {
        if self.int_len == 0 {
            return -1;
        }
        let mut j = self.int_len - 1;
        while j > 0 && self.value[(j + self.offset) as usize] == 0 {
            j -= 1;
        }
        let b = self.value[(j + self.offset) as usize];
        if b == 0 {
            return -1;
        }
        ((self.int_len - 1 - j) << 5) + (b as u32).trailing_zeros() as i32
    }

    /// Java: private final int getInt(int index).
    fn get_int(&self, index: i32) -> i32 {
        self.value[(self.offset + index) as usize]
    }

    /// Java: private final long getLong(int index).
    fn get_long(&self, index: i32) -> i64 {
        self.value[(self.offset + index) as usize] as i64 & crate::big_integer::LONG_MASK
    }

    /// Java: final void normalize().
    pub(crate) fn normalize(&mut self) {
        if self.int_len == 0 {
            self.offset = 0;
            return;
        }
        let mut index = self.offset;
        if self.value[index as usize] != 0 {
            return;
        }
        let index_bound = index + self.int_len;
        loop {
            index += 1;
            if !(index < index_bound && self.value[index as usize] == 0) {
                break;
            }
        }
        let num_zeros = index - self.offset;
        self.int_len -= num_zeros;
        self.offset = if self.int_len == 0 {
            0
        } else {
            self.offset + num_zeros
        };
    }

    /// Java: private final void ensureCapacity(int len).
    fn ensure_capacity(&mut self, len: i32) {
        if (self.value.len() as i32) < len {
            self.value = vec![0; len as usize];
            self.offset = 0;
            self.int_len = len;
        }
    }

    /// Java: int[] toIntArray().
    pub(crate) fn to_int_array_public(&self) -> Vec<i32> {
        let mut result = Vec::with_capacity(self.int_len as usize);
        for i in 0..self.int_len {
            result.push(self.value[(self.offset + i) as usize]);
        }
        result
    }

    /// Java: void setInt(int index, int val).
    fn set_int(&mut self, index: i32, val: i32) {
        self.value[(self.offset + index) as usize] = val;
    }

    /// Java: void setValue(int[] val, int length).
    fn set_value(&mut self, val: Vec<i32>, length: i32) {
        self.value = val;
        self.int_len = length;
        self.offset = 0;
    }

    /// Java: void copyValue(MutableBigInteger src).
    fn copy_value(&mut self, src: &MutableBigInteger) {
        let len = src.int_len;
        if (self.value.len() as i32) < len {
            self.value = vec![0; len as usize];
        }
        let start = src.offset as usize;
        let end = start + len as usize;
        for (i, word) in src.value[start..end].iter().enumerate() {
            self.value[i] = *word;
        }
        self.int_len = len;
        self.offset = 0;
    }

    /// Java: void copyValue(int[] val).
    fn copy_value_vec(&mut self, val: &[i32]) {
        let len = val.len() as i32;
        if (self.value.len() as i32) < len {
            self.value = vec![0; len as usize];
        }
        for (i, word) in val.iter().enumerate() {
            self.value[i] = *word;
        }
        self.int_len = len;
        self.offset = 0;
    }

    /// Java: boolean isOne().
    fn is_one(&self) -> bool {
        self.int_len == 1 && self.value[self.offset as usize] == 1
    }

    /// Java: boolean isZero().
    pub(crate) fn is_zero(&self) -> bool {
        self.int_len == 0
    }

    /// Java: boolean isEven().
    fn is_even(&self) -> bool {
        self.int_len == 0 || (self.value[(self.offset + self.int_len - 1) as usize] & 1) == 0
    }

    /// Java: boolean isOdd().
    fn is_odd(&self) -> bool {
        if self.is_zero() {
            false
        } else {
            (self.value[(self.offset + self.int_len - 1) as usize] & 1) == 1
        }
    }

    /// Java: boolean isNormal().
    fn is_normal(&self) -> bool {
        if self.int_len + self.offset > self.value.len() as i32 {
            return false;
        }
        if self.int_len == 0 {
            return true;
        }
        self.value[self.offset as usize] != 0
    }


    /// Java: public String toString().
    pub(crate) fn to_debug_string(&mut self) -> String {
        let b = self.to_big_integer_sign(1);
        b.to_debug_string()
    }

    /// Java: void safeRightShift(int n).
    fn safe_right_shift(&mut self, n: i32) {
        if n / 32 >= self.int_len {
            self.reset();
        } else {
            self.right_shift(n);
        }
    }

    /// Java: void rightShift(int n).
    pub(crate) fn right_shift(&mut self, n: i32) {
        if self.int_len == 0 {
            return;
        }
        let n_ints = ((n as u32) >> 5) as i32;
        let n_bits = n & 0x1F;
        self.int_len -= n_ints;
        if n_bits == 0 {
            return;
        }
        let bits_in_high_word = BigInteger::bit_length_for_int(self.value[self.offset as usize]);
        if n_bits >= bits_in_high_word {
            self.primitive_left_shift(32 - n_bits);
            self.int_len -= 1;
        } else {
            self.primitive_right_shift(n_bits);
        }
    }

    /// Java: void safeLeftShift(int n).
    fn safe_left_shift(&mut self, n: i32) {
        if n > 0 {
            self.left_shift(n);
        }
    }

    /// Java: void leftShift(int n).
    pub(crate) fn left_shift(&mut self, n: i32) {
        if self.int_len == 0 {
            return;
        }
        let n_ints = ((n as u32) >> 5) as i32;
        let n_bits = n & 0x1F;
        let bits_in_high_word = BigInteger::bit_length_for_int(self.value[self.offset as usize]);
        if n <= 32 - bits_in_high_word {
            self.primitive_left_shift(n_bits);
            return;
        }
        let mut new_len = self.int_len + n_ints + 1;
        if n_bits <= 32 - bits_in_high_word {
            new_len -= 1;
        }
        if (self.value.len() as i32) < new_len {
            let mut result = vec![0; new_len as usize];
            for i in 0..self.int_len {
                result[i as usize] = self.value[(self.offset + i) as usize];
            }
            self.set_value(result, new_len);
        } else if self.value.len() as i32 - self.offset >= new_len {
            for i in 0..(new_len - self.int_len) {
                self.value[(self.offset + self.int_len + i) as usize] = 0;
            }
        } else {
            for i in 0..self.int_len {
                self.value[i as usize] = self.value[(self.offset + i) as usize];
            }
            for i in self.int_len..new_len {
                self.value[i as usize] = 0;
            }
            self.offset = 0;
        }
        self.int_len = new_len;
        if n_bits == 0 {
            return;
        }
        if n_bits <= 32 - bits_in_high_word {
            self.primitive_left_shift(n_bits);
        } else {
            self.primitive_right_shift(32 - n_bits);
        }
    }

    /// Java: private int divadd(int[] a, int[] result, int offset).
    fn divadd(a: &[i32], result: &mut [i32], offset: usize) -> i32 {
        let mut carry: i64 = 0;
        let mut j = a.len() as isize - 1;
        while j >= 0 {
            let sum = (a[j as usize] as i64 & crate::big_integer::LONG_MASK)
                + (result[j as usize + offset] as i64 & crate::big_integer::LONG_MASK)
                + carry;
            result[j as usize + offset] = sum as i32;
            carry = ((sum as u64) >> 32) as i64;
            j -= 1;
        }
        carry as i32
    }

    /// Java: private int mulsub(int[] q, int[] a, int x, int len, int offset).
    fn mulsub(q: &mut [i32], a: &[i32], x: i32, len: i32, offset: i32) -> i32 {
        let x_long = x as i64 & crate::big_integer::LONG_MASK;
        let mut carry: i64 = 0;
        let mut offset = offset as usize + len as usize;
        let mut j = len - 1;
        while j >= 0 {
            let product = (a[j as usize] as i64 & crate::big_integer::LONG_MASK)
                .wrapping_mul(x_long)
                .wrapping_add(carry);
            let difference = (q[offset] as i64).wrapping_sub(product);
            q[offset] = difference as i32;
            carry = ((product as u64) >> 32) as i64
                + if (difference as i64 & crate::big_integer::LONG_MASK)
                    > ((!(product as i32)) as i64 & crate::big_integer::LONG_MASK)
                {
                    1
                } else {
                    0
                };
            offset -= 1;
            j -= 1;
        }
        carry as i32
    }

    /// Java: private int mulsubBorrow(int[] q, int[] a, int x, int len, int offset).
    fn mulsub_borrow(q: &[i32], a: &[i32], x: i32, len: i32, offset: i32) -> i32 {
        let x_long = x as i64 & crate::big_integer::LONG_MASK;
        let mut carry: i64 = 0;
        let mut offset = offset as usize + len as usize;
        let mut j = len - 1;
        while j >= 0 {
            let product = (a[j as usize] as i64 & crate::big_integer::LONG_MASK)
                .wrapping_mul(x_long)
                .wrapping_add(carry);
            let difference = (q[offset] as i64).wrapping_sub(product);
            carry = ((product as u64) >> 32) as i64
                + if (difference as i64 & crate::big_integer::LONG_MASK)
                    > ((!(product as i32)) as i64 & crate::big_integer::LONG_MASK)
                {
                    1
                } else {
                    0
                };
            offset -= 1;
            j -= 1;
        }
        carry as i32
    }

    /// Java: private final void primitiveRightShift(int n).
    fn primitive_right_shift(&mut self, n: i32) {
        let n2 = 32 - n;
        let mut i = self.offset + self.int_len - 1;
        let mut c = self.value[i as usize];
        while i > self.offset {
            let b = c;
            c = self.value[(i - 1) as usize];
            self.value[i as usize] = (c.wrapping_shl(n2 as u32)) | ((b as u32 >> n) as i32);
            i -= 1;
        }
        self.value[self.offset as usize] = ((self.value[self.offset as usize] as u32) >> n) as i32;
    }

    /// Java: private final void primitiveLeftShift(int n).
    fn primitive_left_shift(&mut self, n: i32) {
        let n2 = 32 - n;
        let mut i = self.offset;
        let mut c = self.value[i as usize];
        let m = i + self.int_len - 1;
        while i < m {
            let b = c;
            c = self.value[(i + 1) as usize];
            self.value[i as usize] =
                (b.wrapping_shl(n as u32)) | ((c as u32 >> n2) as i32);
            i += 1;
        }
        let last = (self.offset + self.int_len - 1) as usize;
        self.value[last] = self.value[last].wrapping_shl(n as u32);
    }

    /// Java: private BigInteger getLower(int n).
    pub(crate) fn get_lower(&self, n: i32) -> BigInteger {
        if self.is_zero() {
            return BigInteger::zero();
        } else if self.int_len < n {
            return self.clone().to_big_integer_sign(1);
        }
        let mut len = n;
        while len > 0
            && self.value[(self.offset + self.int_len - len) as usize] == 0
        {
            len -= 1;
        }
        let sign = if len > 0 { 1 } else { 0 };
        let start = (self.offset + self.int_len - len) as usize;
        let end = (self.offset + self.int_len) as usize;
        BigInteger::new_from_mag(self.value[start..end].to_vec(), sign)
    }

    /// Java: private void keepLower(int n).
    fn keep_lower(&mut self, n: i32) {
        if self.int_len >= n {
            self.offset += self.int_len - n;
            self.int_len = n;
        }
    }

    /// Java: void add(MutableBigInteger addend).
    pub(crate) fn add(&mut self, addend: &MutableBigInteger) {
        let mut x = self.int_len;
        let mut y = addend.int_len;
        let mut result_len = if self.int_len > addend.int_len {
            self.int_len
        } else {
            addend.int_len
        };
        let mut result = if (self.value.len() as i32) < result_len {
            vec![0; result_len as usize]
        } else {
            self.value.clone()
        };
        let mut rstart = result.len() as isize - 1;
        let mut carry: i64 = 0;
        while x > 0 && y > 0 {
            x -= 1;
            y -= 1;
            let sum = (self.value[(x + self.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                + (addend.value[(y + addend.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK)
                + carry;
            result[rstart as usize] = sum as i32;
            rstart -= 1;
            carry = ((sum as u64) >> 32) as i64;
        }
        while x > 0 {
            x -= 1;
            let sum = (self.value[(x + self.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                + carry;
            result[rstart as usize] = sum as i32;
            rstart -= 1;
            carry = ((sum as u64) >> 32) as i64;
        }
        while y > 0 {
            y -= 1;
            let sum = (addend.value[(y + addend.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                + carry;
            result[rstart as usize] = sum as i32;
            rstart -= 1;
            carry = ((sum as u64) >> 32) as i64;
        }
        if carry > 0 {
            result_len += 1;
            if (result.len() as i32) < result_len {
                let mut temp = vec![0; result_len as usize];
                temp[1..1 + result.len()].copy_from_slice(&result);
                temp[0] = 1;
                result = temp;
            } else {
                result[rstart as usize] = 1;
                rstart -= 1;
            }
        }
        self.value = result;
        self.int_len = result_len;
        self.offset = self.value.len() as i32 - result_len;
    }

    /// Java: void addShifted(MutableBigInteger addend, int n).
    fn add_shifted(&mut self, addend: &MutableBigInteger, n: i32) {
        if addend.is_zero() {
            return;
        }
        let mut x = self.int_len;
        let mut y = addend.int_len + n;
        let mut result_len = if self.int_len > y { self.int_len } else { y };
        let mut result = if (self.value.len() as i32) < result_len {
            vec![0; result_len as usize]
        } else {
            self.value.clone()
        };
        let mut rstart = result.len() as isize - 1;
        let mut carry: i64 = 0;
        while x > 0 && y > 0 {
            x -= 1;
            y -= 1;
            let bval = if y + addend.offset < addend.value.len() as i32 {
                addend.value[(y + addend.offset) as usize]
            } else {
                0
            };
            let sum = (self.value[(x + self.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                + (bval as i64 & crate::big_integer::LONG_MASK)
                + carry;
            result[rstart as usize] = sum as i32;
            rstart -= 1;
            carry = ((sum as u64) >> 32) as i64;
        }
        while x > 0 {
            x -= 1;
            let sum = (self.value[(x + self.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                + carry;
            result[rstart as usize] = sum as i32;
            rstart -= 1;
            carry = ((sum as u64) >> 32) as i64;
        }
        while y > 0 {
            y -= 1;
            let bval = if y + addend.offset < addend.value.len() as i32 {
                addend.value[(y + addend.offset) as usize]
            } else {
                0
            };
            let sum = (bval as i64 & crate::big_integer::LONG_MASK) + carry;
            result[rstart as usize] = sum as i32;
            rstart -= 1;
            carry = ((sum as u64) >> 32) as i64;
        }
        if carry > 0 {
            result_len += 1;
            if (result.len() as i32) < result_len {
                let mut temp = vec![0; result_len as usize];
                temp[1..1 + result.len()].copy_from_slice(&result);
                temp[0] = 1;
                result = temp;
            } else {
                result[rstart as usize] = 1;
                rstart -= 1;
            }
        }
        self.value = result;
        self.int_len = result_len;
        self.offset = self.value.len() as i32 - result_len;
    }

    /// Java: void addDisjoint(MutableBigInteger addend, int n).
    fn add_disjoint(&mut self, addend: &MutableBigInteger, n: i32) {
        if addend.is_zero() {
            return;
        }
        let x = self.int_len;
        let mut y = addend.int_len + n;
        let result_len = if self.int_len > y { self.int_len } else { y };
        let mut result = if (self.value.len() as i32) < result_len {
            vec![0; result_len as usize]
        } else {
            for i in (self.offset + self.int_len) as usize..self.value.len() {
                self.value[i] = 0;
            }
            self.value.clone()
        };
        let mut rstart = result.len() as isize - 1;
        let start = self.offset as usize;
        let copy_start = (rstart + 1 - x as isize) as usize;
        result[copy_start..copy_start + x as usize]
            .copy_from_slice(&self.value[start..start + x as usize]);
        y -= x;
        rstart -= x as isize;
        let len = std::cmp::min(
            y,
            addend.value.len() as i32 - addend.offset,
        );
        let copy_start = (rstart + 1 - y as isize) as usize;
        let astart = addend.offset as usize;
        result[copy_start..copy_start + len as usize]
            .copy_from_slice(&addend.value[astart..astart + len as usize]);
        for i in (rstart + 1 - y as isize + len as isize) as usize..(rstart + 1) as usize {
            result[i] = 0;
        }
        self.value = result;
        self.int_len = result_len;
        self.offset = self.value.len() as i32 - result_len;
    }

    /// Java: void addLower(MutableBigInteger addend, int n).
    fn add_lower(&mut self, addend: &MutableBigInteger, n: i32) {
        let mut a = MutableBigInteger::new_from_mutable(addend);
        if a.offset + a.int_len >= n {
            a.offset = a.offset + a.int_len - n;
            a.int_len = n;
        }
        a.normalize();
        self.add(&a);
    }

    /// Java: int subtract(MutableBigInteger b).
    pub(crate) fn subtract(&mut self, b: &MutableBigInteger) -> i32 {
        let sign = self.compare(b);
        if sign == 0 {
            self.reset();
            return 0;
        }
        let (big, small) = if sign < 0 {
            (b.clone(), self.clone())
        } else {
            (self.clone(), b.clone())
        };
        let result_len = big.int_len;
        let mut result = vec![0; result_len as usize];
        let mut diff: i64 = 0;
        let mut x = big.int_len;
        let mut y = small.int_len;
        let mut rstart = result.len() as isize - 1;
        while y > 0 {
            x -= 1;
            y -= 1;
            diff = (big.value[(x + big.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                - (small.value[(y + small.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK)
                - ((-(diff >> 32)) as i32 as i64);
            result[rstart as usize] = diff as i32;
            rstart -= 1;
        }
        while x > 0 {
            x -= 1;
            diff = (big.value[(x + big.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                - ((-(diff >> 32)) as i32 as i64);
            result[rstart as usize] = diff as i32;
            rstart -= 1;
        }
        self.value = result;
        self.int_len = result_len;
        self.offset = 0;
        self.normalize();
        self.offset = self.value.len() as i32 - self.int_len;
        sign
    }

    /// Java: private int difference(MutableBigInteger b).
    fn difference(&mut self, b: &mut MutableBigInteger) -> i32 {
        let sign = self.compare(b);
        if sign == 0 {
            return 0;
        }
        if sign < 0 {
            // The larger number is the argument; run the same in-place subtraction
            // on b (Java aliases a = this/b = b and writes the larger one).
            let result = b.difference_positive(self);
            let _ = result;
            return sign;
        }
        let (big, small) = (self.clone(), b.clone());
        let mut diff: i64 = 0;
        let mut x = big.int_len;
        let mut y = small.int_len;
        while y > 0 {
            x -= 1;
            y -= 1;
            diff = (big.value[(x + big.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                - (small.value[(y + small.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK)
                - ((-(diff >> 32)) as i32 as i64);
            self.value[(x + self.offset) as usize] = diff as i32;
        }
        while x > 0 {
            x -= 1;
            diff = (big.value[(x + big.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                - ((-(diff >> 32)) as i32 as i64);
            self.value[(x + self.offset) as usize] = diff as i32;
        }
        self.normalize();
        sign
    }

    /// In-place magnitude subtraction for the sign < 0 branch of difference.
    fn difference_positive(&mut self, small: &MutableBigInteger) -> i32 {
        let big = self.clone();
        let mut diff: i64 = 0;
        let mut x = big.int_len;
        let mut y = small.int_len;
        while y > 0 {
            x -= 1;
            y -= 1;
            diff = (big.value[(x + big.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                - (small.value[(y + small.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK)
                - ((-(diff >> 32)) as i32 as i64);
            self.value[(x + self.offset) as usize] = diff as i32;
        }
        while x > 0 {
            x -= 1;
            diff = (big.value[(x + big.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                - ((-(diff >> 32)) as i32 as i64);
            self.value[(x + self.offset) as usize] = diff as i32;
        }
        self.normalize();
        1
    }

    /// Java: void multiply(MutableBigInteger y, MutableBigInteger z).
    fn multiply(&self, y: &MutableBigInteger, z: &mut MutableBigInteger) {
        let x_len = self.int_len;
        let y_len = y.int_len;
        let new_len = x_len + y_len;
        if (z.value.len() as i32) < new_len {
            z.value = vec![0; new_len as usize];
        }
        z.offset = 0;
        z.int_len = new_len;
        let mut carry: i64 = 0;
        let mut j = y_len - 1;
        let mut k = y_len + x_len - 1;
        while j >= 0 {
            let product = (y.value[(j + y.offset) as usize] as i64
                & crate::big_integer::LONG_MASK)
                * (self.value[(x_len - 1 + self.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK)
                + carry;
            z.value[k as usize] = product as i32;
            carry = ((product as u64) >> 32) as i64;
            j -= 1;
            k -= 1;
        }
        z.value[(x_len - 1) as usize] = carry as i32;
        let mut i = x_len - 2;
        while i >= 0 {
            carry = 0;
            let mut j = y_len - 1;
            let mut k = y_len + i;
            while j >= 0 {
                let product = (y.value[(j + y.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK)
                    * (self.value[(i + self.offset) as usize] as i64
                        & crate::big_integer::LONG_MASK)
                    + (z.value[k as usize] as i64 & crate::big_integer::LONG_MASK)
                    + carry;
                z.value[k as usize] = product as i32;
                carry = ((product as u64) >> 32) as i64;
                j -= 1;
                k -= 1;
            }
            z.value[i as usize] = carry as i32;
            i -= 1;
        }
        z.normalize();
    }

    /// Java: void mul(int y, MutableBigInteger z).
    fn mul(&self, y: i32, z: &mut MutableBigInteger) {
        if y == 1 {
            z.copy_value(self);
            return;
        }
        if y == 0 {
            z.clear();
            return;
        }
        let y_long = y as i64 & crate::big_integer::LONG_MASK;
        let mut zval = if (z.value.len() as i32) < self.int_len + 1 {
            vec![0; (self.int_len + 1) as usize]
        } else {
            z.value.clone()
        };
        let mut carry: i64 = 0;
        let mut i = self.int_len - 1;
        while i >= 0 {
            let product = y_long
                * (self.value[(i + self.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK)
                + carry;
            zval[(i + 1) as usize] = product as i32;
            carry = ((product as u64) >> 32) as i64;
            i -= 1;
        }
        if carry == 0 {
            z.offset = 1;
            z.int_len = self.int_len;
        } else {
            z.offset = 0;
            z.int_len = self.int_len + 1;
            zval[0] = carry as i32;
        }
        z.value = zval;
    }

    /// Java: int divideOneWord(int divisor, MutableBigInteger quotient).
    pub(crate) fn divide_one_word(&self, divisor: i32, quotient: &mut MutableBigInteger) -> i32 {
        let divisor_long = divisor as i64 & crate::big_integer::LONG_MASK;
        if self.int_len == 1 {
            let dividend_value = self.value[self.offset as usize] as i64
                & crate::big_integer::LONG_MASK;
            let q = (dividend_value / divisor_long) as i32;
            let r = (dividend_value - q as i64 * divisor_long) as i32;
            quotient.value[0] = q;
            quotient.int_len = if q == 0 { 0 } else { 1 };
            quotient.offset = 0;
            return r;
        }
        if (quotient.value.len() as i32) < self.int_len {
            quotient.value = vec![0; self.int_len as usize];
        }
        quotient.offset = 0;
        quotient.int_len = self.int_len;
        let shift = (divisor as u32).leading_zeros() as i32;
        let mut rem = self.value[self.offset as usize];
        let mut rem_long = rem as i64 & crate::big_integer::LONG_MASK;
        if rem_long < divisor_long {
            quotient.value[0] = 0;
        } else {
            quotient.value[0] = (rem_long / divisor_long) as i32;
            rem = (rem_long - quotient.value[0] as i64 * divisor_long) as i32;
            rem_long = rem as i64 & crate::big_integer::LONG_MASK;
        }
        let mut xlen = self.int_len;
        loop {
            xlen -= 1;
            if xlen <= 0 {
                break;
            }
            let dividend_estimate = (rem_long << 32)
                | (self.value[(self.offset + self.int_len - xlen) as usize] as i64
                    & crate::big_integer::LONG_MASK);
            let q;
            if dividend_estimate >= 0 {
                q = (dividend_estimate / divisor_long) as i32;
                rem = dividend_estimate
                .wrapping_sub((q as i64).wrapping_mul(divisor_long)) as i32;
            } else {
                let tmp = MutableBigInteger::div_word(dividend_estimate, divisor);
                q = (tmp & crate::big_integer::LONG_MASK) as i32;
                rem = ((tmp as u64) >> 32) as i32;
            }
            quotient.value[(self.int_len - xlen) as usize] = q;
            rem_long = rem as i64 & crate::big_integer::LONG_MASK;
        }
        quotient.normalize();
        if shift > 0 {
            rem % divisor
        } else {
            rem
        }
    }

    /// Java: MutableBigInteger divide(MutableBigInteger b, MutableBigInteger quotient).
    pub(crate) fn divide(
        &self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
    ) -> Option<MutableBigInteger> {
        self.divide_with_remainder(b, quotient, true)
    }

    /// Java: MutableBigInteger divide(MutableBigInteger b, MutableBigInteger
    /// quotient, boolean needRemainder).
    pub(crate) fn divide_with_remainder(
        &self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
        need_remainder: bool,
    ) -> Option<MutableBigInteger> {
        if b.int_len < BigInteger::BURNIKEL_ZIEGLER_THRESHOLD
            || self.int_len - b.int_len < BigInteger::BURNIKEL_ZIEGLER_OFFSET
        {
            self.divide_knuth(b, quotient, need_remainder)
        } else {
            Some(self.divide_and_remainder_burnikel_ziegler(b, quotient))
        }
    }

    /// Java: MutableBigInteger divideKnuth(MutableBigInteger b, MutableBigInteger
    /// quotient).
    pub(crate) fn divide_knuth_top(
        &self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
    ) -> Option<MutableBigInteger> {
        self.divide_knuth(b, quotient, true)
    }

    /// Java: MutableBigInteger divideKnuth(MutableBigInteger b, MutableBigInteger
    /// quotient, boolean needRemainder).
    fn divide_knuth(
        &self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
        need_remainder: bool,
    ) -> Option<MutableBigInteger> {
        if b.int_len == 0 {
            panic!("ArithmeticException: BigInteger divide by zero");
        }
        if self.int_len == 0 {
            quotient.int_len = 0;
            quotient.offset = 0;
            return if need_remainder {
                Some(MutableBigInteger::new_empty())
            } else {
                None
            };
        }
        let cmp = self.compare(b);
        if cmp < 0 {
            quotient.int_len = 0;
            quotient.offset = 0;
            return if need_remainder {
                Some(MutableBigInteger::new_from_mutable(self))
            } else {
                None
            };
        }
        if cmp == 0 {
            quotient.value[0] = 1;
            quotient.int_len = 1;
            quotient.offset = 0;
            return if need_remainder {
                Some(MutableBigInteger::new_empty())
            } else {
                None
            };
        }
        quotient.clear();
        if b.int_len == 1 {
            let r = self.divide_one_word(b.value[b.offset as usize], quotient);
            if need_remainder {
                if r == 0 {
                    return Some(MutableBigInteger::new_empty());
                }
                return Some(MutableBigInteger::new_int(r));
            }
            return None;
        }
        if self.int_len >= KNUTH_POW2_THRESH_LEN {
            let trailing_zero_bits =
                std::cmp::min(self.get_lowest_set_bit(), b.get_lowest_set_bit());
            if trailing_zero_bits >= KNUTH_POW2_THRESH_ZEROS * 32 {
                let mut a = MutableBigInteger::new_from_mutable(self);
                let mut b_shifted = MutableBigInteger::new_from_mutable(b);
                a.right_shift(trailing_zero_bits);
                b_shifted.right_shift(trailing_zero_bits);
                let mut r = a.divide_knuth(&b_shifted, quotient, true).expect("remainder");
                r.left_shift(trailing_zero_bits);
                return Some(r);
            }
        }
        self.divide_magnitude(b, quotient, need_remainder)
    }

    /// Java: MutableBigInteger divideAndRemainderBurnikelZiegler(MutableBigInteger
    /// b, MutableBigInteger quotient).
    pub(crate) fn divide_and_remainder_burnikel_ziegler(
        &self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
    ) -> MutableBigInteger {
        let r = self.int_len;
        let s = b.int_len;
        quotient.offset = 0;
        quotient.int_len = 0;
        if r < s {
            return MutableBigInteger::new_from_mutable(self);
        }
        let m = 1i32
            << (32 - (s / BigInteger::BURNIKEL_ZIEGLER_THRESHOLD).leading_zeros()) as i32;
        let j = (s + m - 1) / m;
        let n = j * m;
        let n32 = 32i64 * n as i64;
        let sigma = std::cmp::max(0, n32 - b.bit_length()) as i32;
        let mut b_shifted = MutableBigInteger::new_from_mutable(b);
        b_shifted.safe_left_shift(sigma);
        let mut a_shifted = MutableBigInteger::new_from_mutable(self);
        a_shifted.safe_left_shift(sigma);
        let mut t = ((a_shifted.bit_length() + n32) / n32) as i32;
        if t < 2 {
            t = 2;
        }
        let a1 = a_shifted.get_block(t - 1, t, n);
        let mut z = a_shifted.get_block(t - 2, t, n);
        z.add_disjoint(&a1, n);
        let mut qi = MutableBigInteger::new_empty();
        let mut ri;
        let mut i = t - 2;
        while i > 0 {
            ri = z.divide_2n1n(&b_shifted, &mut qi);
            z = a_shifted.get_block(i - 1, t, n);
            z.add_disjoint(&ri, n);
            quotient.add_shifted(&qi, i * n);
            i -= 1;
        }
        ri = z.divide_2n1n(&b_shifted, &mut qi);
        quotient.add(&qi);
        ri.right_shift(sigma);
        ri
    }

    /// Java: private MutableBigInteger divide2n1n(MutableBigInteger b,
    /// MutableBigInteger quotient).
    fn divide_2n1n(
        &mut self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
    ) -> MutableBigInteger {
        let n = b.int_len;
        if n % 2 != 0 || n < BigInteger::BURNIKEL_ZIEGLER_THRESHOLD {
            return self.divide_knuth(b, quotient, true).expect("remainder");
        }
        let mut a_upper = MutableBigInteger::new_from_mutable(self);
        a_upper.safe_right_shift(32 * (n / 2));
        self.keep_lower(n / 2);
        let mut q1 = MutableBigInteger::new_empty();
        let r1 = a_upper.divide_3n2n(b, &mut q1);
        self.add_disjoint(&r1, n / 2);
        let r2 = self.divide_3n2n(b, quotient);
        quotient.add_disjoint(&q1, n / 2);
        r2
    }

    /// Java: private MutableBigInteger divide3n2n(MutableBigInteger b,
    /// MutableBigInteger quotient).
    fn divide_3n2n(
        &mut self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
    ) -> MutableBigInteger {
        let n = b.int_len / 2;
        let mut a12 = MutableBigInteger::new_from_mutable(self);
        a12.safe_right_shift(32 * n);
        let mut b1 = MutableBigInteger::new_from_mutable(b);
        b1.safe_right_shift(n * 32);
        let b2 = b.get_lower(n);
        let mut r;
        let mut d;
        if self.compare_shifted(b, n) < 0 {
            r = a12.divide_2n1n(&b1, quotient);
            d = MutableBigInteger::new_from_big_integer(&quotient.to_big_integer().multiply(&b2));
        } else {
            quotient.ones(n);
            a12.add(&b1);
            b1.left_shift(32 * n);
            a12.subtract(&b1);
            r = a12;
            d = MutableBigInteger::new_from_big_integer(&b2);
            d.left_shift(32 * n);
            d.subtract(&MutableBigInteger::new_from_big_integer(&b2));
        }
        r.left_shift(32 * n);
        r.add_lower(self, n);
        while r.compare(&d) < 0 {
            r.add(b);
            quotient.subtract(&MutableBigInteger::new_int(1));
        }
        r.subtract(&d);
        r
    }

    /// Java: private MutableBigInteger getBlock(int index, int numBlocks, int
    /// blockLength).
    fn get_block(&self, index: i32, num_blocks: i32, block_length: i32) -> MutableBigInteger {
        let block_start = index * block_length;
        if block_start >= self.int_len {
            return MutableBigInteger::new_empty();
        }
        let block_end = if index == num_blocks - 1 {
            self.int_len
        } else {
            (index + 1) * block_length
        };
        if block_end > self.int_len {
            return MutableBigInteger::new_empty();
        }
        let start = (self.offset + self.int_len - block_end) as usize;
        let end = (self.offset + self.int_len - block_start) as usize;
        MutableBigInteger::new_from_vec(self.value[start..end].to_vec())
    }

    /// Java: long bitLength().
    pub(crate) fn bit_length(&self) -> i64 {
        if self.int_len == 0 {
            return 0;
        }
        self.int_len as i64 * 32
            - (self.value[self.offset as usize] as u32).leading_zeros() as i64
    }

    /// Java: long divide(long v, MutableBigInteger quotient).
    pub(crate) fn divide_long_value(
        &self,
        v: i64,
        quotient: &mut MutableBigInteger,
    ) -> i64 {
        if v == 0 {
            panic!("ArithmeticException: BigInteger divide by zero");
        }
        if self.int_len == 0 {
            quotient.int_len = 0;
            quotient.offset = 0;
            return 0;
        }
        let mut v = v;
        if v < 0 {
            v = -v;
        }
        let d = ((v as u64) >> 32) as i32;
        quotient.clear();
        if d == 0 {
            self.divide_one_word(v as i32, quotient) as i64 & crate::big_integer::LONG_MASK
        } else {
            self.divide_long_magnitude(v, quotient).to_long()
        }
    }

    /// Java: private static void copyAndShift(int[] src, int srcFrom, int srcLen,
    /// int[] dst, int dstFrom, int shift).
    fn copy_and_shift(
        src: &[i32],
        src_from: usize,
        src_len: usize,
        dst: &mut [i32],
        dst_from: usize,
        shift: i32,
    ) {
        let n2 = 32 - shift;
        let mut c = src[src_from];
        let mut src_from = src_from;
        for i in 0..src_len - 1 {
            let b = c;
            src_from += 1;
            c = src[src_from];
            dst[dst_from + i] = b.wrapping_shl(shift as u32) | ((c as u32 >> n2) as i32);
        }
        dst[dst_from + src_len - 1] = c.wrapping_shl(shift as u32);
    }

    /// Java: private MutableBigInteger divideMagnitude(MutableBigInteger div,
    /// MutableBigInteger quotient, boolean needRemainder).
    fn divide_magnitude(
        &self,
        div: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
        need_remainder: bool,
    ) -> Option<MutableBigInteger> {
        let shift = (div.value[div.offset as usize] as u32).leading_zeros() as i32;
        let dlen = div.int_len;
        let mut divisor: Vec<i32>;
        let mut rem: MutableBigInteger;
        if shift > 0 {
            divisor = vec![0; dlen as usize];
            Self::copy_and_shift(
                &div.value,
                div.offset as usize,
                dlen as usize,
                &mut divisor,
                0,
                shift,
            );
            if (self.value[self.offset as usize] as u32).leading_zeros() as i32 >= shift {
                let mut remarr = vec![0; self.int_len as usize + 1];
                rem = MutableBigInteger::new_from_vec(remarr.clone());
                rem.int_len = self.int_len;
                rem.offset = 1;
                Self::copy_and_shift(
                    &self.value,
                    self.offset as usize,
                    self.int_len as usize,
                    &mut remarr,
                    1,
                    shift,
                );
                rem.value = remarr;
            } else {
                let mut remarr = vec![0; self.int_len as usize + 2];
                rem = MutableBigInteger::new_from_vec(remarr.clone());
                rem.int_len = self.int_len + 1;
                rem.offset = 1;
                let mut r_from = self.offset as usize;
                let mut c = 0i32;
                let n2 = 32 - shift;
                for i in 1..self.int_len as usize + 1 {
                    let b = c;
                    c = self.value[r_from];
                    r_from += 1;
                    remarr[i] = b.wrapping_shl(shift as u32) | ((c as u32 >> n2) as i32);
                }
                remarr[self.int_len as usize + 1] = c.wrapping_shl(shift as u32);
                rem.value = remarr;
            }
        } else {
            divisor = div.value[div.offset as usize..(div.offset + div.int_len) as usize].to_vec();
            rem = MutableBigInteger::new_from_vec(vec![0; self.int_len as usize + 1]);
            rem.value[1..1 + self.int_len as usize]
                .copy_from_slice(&self.value[self.offset as usize..(self.offset + self.int_len) as usize]);
            rem.int_len = self.int_len;
            rem.offset = 1;
        }
        let nlen = rem.int_len;
        let limit = nlen - dlen + 1;
        if (quotient.value.len() as i32) < limit {
            quotient.value = vec![0; limit as usize];
            quotient.offset = 0;
        }
        quotient.int_len = limit;
        if rem.int_len == nlen {
            rem.offset = 0;
            rem.value[0] = 0;
            rem.int_len += 1;
        }
        let dh = divisor[0];
        let dh_long = dh as i64 & crate::big_integer::LONG_MASK;
        let dl = divisor[1];
        let mut j = 0;
        while j < limit - 1 {
            let mut qhat: i32 = 0;
            let mut qrem: i32 = 0;
            let mut skip_correction = false;
            let nh = rem.value[(j + rem.offset) as usize];
            let nh2 = nh.wrapping_add(0x80000000u32 as i32);
            let nm = rem.value[(j + 1 + rem.offset) as usize];
            if nh == dh {
                qhat = !0;
                qrem = nh.wrapping_add(nm);
                skip_correction =
                    qrem.wrapping_add(0x80000000u32 as i32) < nh2;
            } else {
                let n_chunk = ((nh as i64) << 32) | (nm as i64 & crate::big_integer::LONG_MASK);
                if n_chunk >= 0 {
                    qhat = (n_chunk / dh_long) as i32;
                    qrem = n_chunk
                        .wrapping_sub((qhat as i64).wrapping_mul(dh_long)) as i32;
                } else {
                    let tmp = MutableBigInteger::div_word(n_chunk, dh);
                    qhat = (tmp & crate::big_integer::LONG_MASK) as i32;
                    qrem = ((tmp as u64) >> 32) as i32;
                }
            }
            if qhat == 0 {
                j += 1;
                continue;
            }
            if !skip_correction {
                let nl = rem.value[(j + 2 + rem.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK;
                let mut rs = ((qrem as i64 & crate::big_integer::LONG_MASK) << 32) | nl;
                let mut est_product =
                    (dl as i64 & crate::big_integer::LONG_MASK)
                    .wrapping_mul(qhat as i64 & crate::big_integer::LONG_MASK);
                if Self::unsigned_long_compare(est_product, rs) {
                    qhat -= 1;
                    qrem = ((qrem as i64 & crate::big_integer::LONG_MASK) + dh_long) as i32;
                    if qrem as i64 & crate::big_integer::LONG_MASK >= dh_long {
                        est_product = est_product.wrapping_sub(dl as i64 & crate::big_integer::LONG_MASK);
                        rs = ((qrem as i64 & crate::big_integer::LONG_MASK) << 32) | nl;
                        if Self::unsigned_long_compare(est_product, rs) {
                            qhat -= 1;
                        }
                    }
                }
            }
            rem.value[(j + rem.offset) as usize] = 0;
            let borrow = MutableBigInteger::mulsub(
                &mut rem.value,
                &divisor,
                qhat,
                dlen,
                j + rem.offset,
            );
            if borrow.wrapping_add(0x80000000u32 as i32) > nh2 {
                MutableBigInteger::divadd(
                    &divisor,
                    &mut rem.value,
                    (j + 1 + rem.offset) as usize,
                );
                qhat -= 1;
            }
            quotient.value[j as usize] = qhat;
            j += 1;
        }
        let mut qhat: i32 = 0;
        let mut qrem: i32 = 0;
        let mut skip_correction = false;
        let nh = rem.value[(limit - 1 + rem.offset) as usize];
        let nh2 = nh.wrapping_add(0x80000000u32 as i32);
        let nm = rem.value[(limit + rem.offset) as usize];
        if nh == dh {
            qhat = !0;
            qrem = nh.wrapping_add(nm);
            skip_correction = qrem.wrapping_add(0x80000000u32 as i32) < nh2;
        } else {
            let n_chunk = ((nh as i64) << 32) | (nm as i64 & crate::big_integer::LONG_MASK);
            if n_chunk >= 0 {
                qhat = (n_chunk / dh_long) as i32;
                qrem = n_chunk
                        .wrapping_sub((qhat as i64).wrapping_mul(dh_long)) as i32;
            } else {
                let tmp = MutableBigInteger::div_word(n_chunk, dh);
                qhat = (tmp & crate::big_integer::LONG_MASK) as i32;
                qrem = ((tmp as u64) >> 32) as i32;
            }
        }
        if qhat != 0 {
            if !skip_correction {
                let nl = rem.value[(limit + 1 + rem.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK;
                let mut rs = ((qrem as i64 & crate::big_integer::LONG_MASK) << 32) | nl;
                let mut est_product =
                    (dl as i64 & crate::big_integer::LONG_MASK)
                    .wrapping_mul(qhat as i64 & crate::big_integer::LONG_MASK);
                if Self::unsigned_long_compare(est_product, rs) {
                    qhat -= 1;
                    qrem = ((qrem as i64 & crate::big_integer::LONG_MASK) + dh_long) as i32;
                    if qrem as i64 & crate::big_integer::LONG_MASK >= dh_long {
                        est_product = est_product.wrapping_sub(dl as i64 & crate::big_integer::LONG_MASK);
                        rs = ((qrem as i64 & crate::big_integer::LONG_MASK) << 32) | nl;
                        if Self::unsigned_long_compare(est_product, rs) {
                            qhat -= 1;
                        }
                    }
                }
            }
            rem.value[(limit - 1 + rem.offset) as usize] = 0;
            let borrow = if need_remainder {
                MutableBigInteger::mulsub(
                    &mut rem.value,
                    &divisor,
                    qhat,
                    dlen,
                    limit - 1 + rem.offset,
                )
            } else {
                MutableBigInteger::mulsub_borrow(
                    &rem.value,
                    &divisor,
                    qhat,
                    dlen,
                    limit - 1 + rem.offset,
                )
            };
            if borrow.wrapping_add(0x80000000u32 as i32) > nh2 {
                if need_remainder {
                    MutableBigInteger::divadd(
                        &divisor,
                        &mut rem.value,
                        (limit - 1 + 1 + rem.offset) as usize,
                    );
                }
                qhat -= 1;
            }
            quotient.value[(limit - 1) as usize] = qhat;
        }
        if need_remainder {
            if shift > 0 {
                rem.right_shift(shift);
            }
            rem.normalize();
        }
        quotient.normalize();
        if need_remainder { Some(rem) } else { None }
    }

    /// Java: private MutableBigInteger divideLongMagnitude(long ldivisor,
    /// MutableBigInteger quotient).
    fn divide_long_magnitude(
        &self,
        ldivisor: i64,
        quotient: &mut MutableBigInteger,
    ) -> MutableBigInteger {
        let mut rem = MutableBigInteger::new_from_vec(vec![0; self.int_len as usize + 1]);
        rem.value[1..1 + self.int_len as usize]
            .copy_from_slice(&self.value[self.offset as usize..(self.offset + self.int_len) as usize]);
        rem.int_len = self.int_len;
        rem.offset = 1;
        let nlen = rem.int_len;
        let limit = nlen - 2 + 1;
        if (quotient.value.len() as i32) < limit {
            quotient.value = vec![0; limit as usize];
            quotient.offset = 0;
        }
        quotient.int_len = limit;
        let shift = (ldivisor as u64).leading_zeros() as i32;
        let mut ldivisor = ldivisor;
        if shift > 0 {
            ldivisor = ldivisor.wrapping_shl(shift as u32);
            rem.left_shift(shift);
        }
        if rem.int_len == nlen {
            rem.offset = 0;
            rem.value[0] = 0;
            rem.int_len += 1;
        }
        let dh = ((ldivisor as u64) >> 32) as i32;
        let dh_long = dh as i64 & crate::big_integer::LONG_MASK;
        let dl = (ldivisor & crate::big_integer::LONG_MASK) as i32;
        let mut j = 0;
        while j < limit {
            let mut qhat: i32 = 0;
            let mut qrem: i32 = 0;
            let mut skip_correction = false;
            let nh = rem.value[(j + rem.offset) as usize];
            let nh2 = nh.wrapping_add(0x80000000u32 as i32);
            let nm = rem.value[(j + 1 + rem.offset) as usize];
            if nh == dh {
                qhat = !0;
                qrem = nh.wrapping_add(nm);
                skip_correction = qrem.wrapping_add(0x80000000u32 as i32) < nh2;
            } else {
                let n_chunk = ((nh as i64) << 32) | (nm as i64 & crate::big_integer::LONG_MASK);
                if n_chunk >= 0 {
                    qhat = (n_chunk / dh_long) as i32;
                    qrem = n_chunk
                        .wrapping_sub((qhat as i64).wrapping_mul(dh_long)) as i32;
                } else {
                    let tmp = MutableBigInteger::div_word(n_chunk, dh);
                    qhat = (tmp & crate::big_integer::LONG_MASK) as i32;
                    qrem = ((tmp as u64) >> 32) as i32;
                }
            }
            if qhat == 0 {
                j += 1;
                continue;
            }
            if !skip_correction {
                let nl = rem.value[(j + 2 + rem.offset) as usize] as i64
                    & crate::big_integer::LONG_MASK;
                let mut rs = ((qrem as i64 & crate::big_integer::LONG_MASK) << 32) | nl;
                let mut est_product =
                    (dl as i64 & crate::big_integer::LONG_MASK)
                    .wrapping_mul(qhat as i64 & crate::big_integer::LONG_MASK);
                if Self::unsigned_long_compare(est_product, rs) {
                    qhat -= 1;
                    qrem = ((qrem as i64 & crate::big_integer::LONG_MASK) + dh_long) as i32;
                    if qrem as i64 & crate::big_integer::LONG_MASK >= dh_long {
                        est_product = est_product.wrapping_sub(dl as i64 & crate::big_integer::LONG_MASK);
                        rs = ((qrem as i64 & crate::big_integer::LONG_MASK) << 32) | nl;
                        if Self::unsigned_long_compare(est_product, rs) {
                            qhat -= 1;
                        }
                    }
                }
            }
            rem.value[(j + rem.offset) as usize] = 0;
            let borrow = MutableBigInteger::mulsub_long(
                &mut rem.value,
                dh,
                dl,
                qhat,
                j + rem.offset,
            );
            if borrow.wrapping_add(0x80000000u32 as i32) > nh2 {
                MutableBigInteger::divadd_long(dh, dl, &mut rem.value, (j + 1 + rem.offset) as usize);
                qhat -= 1;
            }
            quotient.value[j as usize] = qhat;
            j += 1;
        }
        if shift > 0 {
            rem.right_shift(shift);
        }
        quotient.normalize();
        rem.normalize();
        rem
    }

    /// Java: private int divaddLong(int dh, int dl, int[] result, int offset).
    fn divadd_long(dh: i32, dl: i32, result: &mut [i32], offset: usize) -> i32 {
        let mut carry: i64 = 0;
        let mut sum = (dl as i64 & crate::big_integer::LONG_MASK)
            + (result[1 + offset] as i64 & crate::big_integer::LONG_MASK);
        result[1 + offset] = sum as i32;
        sum = (dh as i64 & crate::big_integer::LONG_MASK)
            + (result[offset] as i64 & crate::big_integer::LONG_MASK)
            + carry;
        result[offset] = sum as i32;
        carry = ((sum as u64) >> 32) as i64;
        carry as i32
    }

    /// Java: private int mulsubLong(int[] q, int dh, int dl, int x, int offset).
    fn mulsub_long(q: &mut [i32], dh: i32, dl: i32, x: i32, offset: i32) -> i32 {
        let x_long = x as i64 & crate::big_integer::LONG_MASK;
        let mut offset = offset as usize + 2;
        let mut product = (dl as i64 & crate::big_integer::LONG_MASK)
            .wrapping_mul(x_long);
        let mut difference = (q[offset] as i64) - product;
        q[offset] = difference as i32;
        offset -= 1;
        let mut carry = ((product as u64) >> 32) as i64
            + if (difference as i64 & crate::big_integer::LONG_MASK)
                > ((!(product as i32)) as i64 & crate::big_integer::LONG_MASK)
            {
                1
            } else {
                0
            };
        product = (dh as i64 & crate::big_integer::LONG_MASK)
            .wrapping_mul(x_long)
            .wrapping_add(carry);
        difference = (q[offset] as i64) - product;
        q[offset] = difference as i32;
        carry = ((product as u64) >> 32) as i64
            + if (difference as i64 & crate::big_integer::LONG_MASK)
                > ((!(product as i32)) as i64 & crate::big_integer::LONG_MASK)
            {
                1
            } else {
                0
            };
        carry as i32
    }

    /// Java: private boolean unsignedLongCompare(long one, long two).
    fn unsigned_long_compare(one: i64, two: i64) -> bool {
        one.wrapping_add(i64::MIN) > two.wrapping_add(i64::MIN)
    }

    /// Java: static long divWord(long n, int d).
    pub(crate) fn div_word(n: i64, d: i32) -> i64 {
        let d_long = d as i64 & crate::big_integer::LONG_MASK;
        let mut r: i64;
        let mut q: i64;
        if d_long == 1 {
            q = n as i32 as i64;
            r = 0;
            return (r << 32) | (q & crate::big_integer::LONG_MASK);
        }
        q = (((n as u64) >> 1) / ((d_long as u64) >> 1)) as i64;
        r = n.wrapping_sub(q.wrapping_mul(d_long));
        while r < 0 {
            r += d_long;
            q -= 1;
        }
        while r >= d_long {
            r -= d_long;
            q += 1;
        }
        (r << 32) | (q & crate::big_integer::LONG_MASK)
    }

    /// Java: MutableBigInteger sqrt().
    pub(crate) fn sqrt_public(&mut self) -> MutableBigInteger {
        if self.is_zero() {
            return MutableBigInteger::new_int(0);
        } else if self.value.len() == 1 && (self.value[0] as i64 & crate::big_integer::LONG_MASK) < 4 {
            return one();
        }
        if self.bit_length() <= 63 {
            let v = BigInteger::new_from_mag(self.value.clone(), 1).long_value_exact();
            let mut xk = (v as f64).sqrt().floor() as i64;
            loop {
                let xk1 = (xk + v / xk) / 2;
                if xk1 >= xk {
                    return MutableBigInteger::new_from_vec(vec![
                        ((xk as u64) >> 32) as i32,
                        (xk & crate::big_integer::LONG_MASK) as i32,
                    ]);
                }
                xk = xk1;
            }
        }
        let bit_length = self.bit_length() as i32;
        if bit_length as i64 != self.bit_length() {
            panic!("ArithmeticException: bitLength() integer overflow");
        }
        let mut shift = bit_length - 63;
        if shift % 2 == 1 {
            shift += 1;
        }
        let mut xk = MutableBigInteger::new_from_mutable(self);
        xk.right_shift(shift);
        xk.normalize();
        let d = BigInteger::new_from_mag(xk.value.clone(), 1).double_value();
        let bi = BigInteger::value_of_long(d.sqrt().ceil() as i64);
        xk = MutableBigInteger::new_from_big_integer(&bi);
        xk.left_shift(shift / 2);
        let mut xk1 = MutableBigInteger::new_empty();
        loop {
            self.divide_with_remainder(&xk, &mut xk1, false);
            xk1.add(&xk);
            xk1.right_shift(1);
            if xk1.compare(&xk) >= 0 {
                return xk;
            }
            xk.copy_value(&xk1);
            xk1.reset();
        }
    }

    /// Java: MutableBigInteger hybridGCD(MutableBigInteger b).
    pub(crate) fn hybrid_gcd_public(&self, b: &MutableBigInteger) -> MutableBigInteger {
        let mut a = MutableBigInteger::new_from_mutable(self);
        let mut b = MutableBigInteger::new_from_mutable(b);
        let mut q = MutableBigInteger::new_empty();
        while b.int_len != 0 {
            if (a.int_len - b.int_len).abs() < 2 {
                return a.binary_gcd(&b);
            }
            let r = a
                .divide_with_remainder(&b, &mut q, true)
                .expect("remainder");
            a = b;
            b = r;
        }
        a
    }

    /// Java: private MutableBigInteger binaryGCD(MutableBigInteger v).
    fn binary_gcd(&self, v: &MutableBigInteger) -> MutableBigInteger {
        let mut u = MutableBigInteger::new_from_mutable(self);
        let mut v = MutableBigInteger::new_from_mutable(v);
        let mut r = MutableBigInteger::new_empty();
        let s1 = u.get_lowest_set_bit();
        let s2 = v.get_lowest_set_bit();
        let k = if s1 < s2 { s1 } else { s2 };
        if k != 0 {
            u.right_shift(k);
            v.right_shift(k);
        }
        let u_odd = k == s1;
        let mut t = if u_odd { v.clone() } else { u.clone() };
        let mut tsign = if u_odd { -1 } else { 1 };
        loop {
            let lb = t.get_lowest_set_bit();
            if lb < 0 {
                break;
            }
            t.right_shift(lb);
            if tsign > 0 {
                u = t.clone();
            } else {
                v = t.clone();
            }
            if u.int_len < 2 && v.int_len < 2 {
                let x = u.value[u.offset as usize];
                let y = v.value[v.offset as usize];
                let x = Self::binary_gcd_int(x, y);
                r.value[0] = x;
                r.int_len = 1;
                r.offset = 0;
                if k > 0 {
                    r.left_shift(k);
                }
                return r;
            }
            tsign = u.difference(&mut v);
            if tsign == 0 {
                break;
            }
            t = if tsign >= 0 { u.clone() } else { v.clone() };
        }
        if k > 0 {
            u.left_shift(k);
        }
        u
    }

    /// Java: static int binaryGcd(int a, int b).
    fn binary_gcd_int(a: i32, b: i32) -> i32 {
        if b == 0 {
            return a;
        }
        if a == 0 {
            return b;
        }
        let a_zeros = (a as u32).trailing_zeros() as i32;
        let b_zeros = (b as u32).trailing_zeros() as i32;
        let mut a = ((a as u32) >> a_zeros) as i32;
        let mut b = ((b as u32) >> b_zeros) as i32;
        let t = if a_zeros < b_zeros { a_zeros } else { b_zeros };
        while a != b {
            if a.wrapping_add(0x80000000u32 as i32) > b.wrapping_add(0x80000000u32 as i32) {
                a = a.wrapping_sub(b);
                a = ((a as u32) >> (a as u32).trailing_zeros()) as i32;
            } else {
                b = b.wrapping_sub(a);
                b = ((b as u32) >> (b as u32).trailing_zeros()) as i32;
            }
        }
        a.wrapping_shl(t as u32)
    }

    /// Java: MutableBigInteger mutableModInverse(MutableBigInteger p).
    pub(crate) fn mutable_mod_inverse_public(&self, p: &MutableBigInteger) -> MutableBigInteger {
        if p.is_odd() {
            return self.mod_inverse(p);
        }
        if self.is_even() {
            panic!("ArithmeticException: BigInteger not invertible.");
        }
        let powers_of_2 = p.get_lowest_set_bit();
        let mut odd_mod = MutableBigInteger::new_from_mutable(p);
        odd_mod.right_shift(powers_of_2);
        if odd_mod.is_one() {
            return self.mod_inverse_mp2(powers_of_2);
        }
        let odd_part = self.mod_inverse(&odd_mod);
        let even_part = self.mod_inverse_mp2(powers_of_2);
        let y1 = Self::mod_inverse_bp2(&odd_mod, powers_of_2);
        let y2 = odd_mod.mod_inverse_mp2(powers_of_2);
        let mut temp1 = MutableBigInteger::new_empty();
        let mut temp2 = MutableBigInteger::new_empty();
        let mut result = MutableBigInteger::new_empty();
        let mut odd_part = odd_part;
        odd_part.left_shift(powers_of_2);
        odd_part.multiply(&y1, &mut result);
        odd_part.clear();
        even_part.multiply(&odd_mod, &mut temp1);
        temp1.multiply(&y2, &mut temp2);
        result.add(&temp2);
        result
            .divide_with_remainder(p, &mut temp1, true)
            .expect("remainder")
    }

    /// Java: MutableBigInteger modInverseMP2(int k).
    fn mod_inverse_mp2(&self, k: i32) -> MutableBigInteger {
        if self.is_even() {
            panic!("ArithmeticException: Non-invertible. (GCD != 1)");
        }
        if k > 64 {
            return self.euclid_mod_inverse(k);
        }
        let mut t = Self::inverse_mod32(self.value[(self.offset + self.int_len - 1) as usize]);
        if k < 33 {
            t = if k == 32 { t } else { t & ((1i32 << k) - 1) };
            return MutableBigInteger::new_int(t);
        }
        let mut p_long = self.value[(self.offset + self.int_len - 1) as usize] as i64
            & crate::big_integer::LONG_MASK;
        if self.int_len > 1 {
            p_long |= (self.value[(self.offset + self.int_len - 2) as usize] as i64) << 32;
        }
        let mut t_long = t as i64 & crate::big_integer::LONG_MASK;
        t_long = t_long * (2 - p_long * t_long);
        t_long = if k == 64 {
            t_long
        } else {
            t_long & ((1i64 << k) - 1)
        };
        let mut result = MutableBigInteger::new_from_vec(vec![0; 2]);
        result.value[0] = ((t_long as u64) >> 32) as i32;
        result.value[1] = t_long as i32;
        result.int_len = 2;
        result.normalize();
        result
    }

    /// Java: static int inverseMod32(int val).
    fn inverse_mod32(val: i32) -> i32 {
        let mut t = val;
        t = t.wrapping_mul(2i32.wrapping_sub(val.wrapping_mul(t)));
        t = t.wrapping_mul(2i32.wrapping_sub(val.wrapping_mul(t)));
        t = t.wrapping_mul(2i32.wrapping_sub(val.wrapping_mul(t)));
        t = t.wrapping_mul(2i32.wrapping_sub(val.wrapping_mul(t)));
        t
    }

    /// Java: static long inverseMod64(long val).
    pub(crate) fn inverse_mod64(val: i64) -> i64 {
        let mut t = val;
        t = t.wrapping_mul(2i64.wrapping_sub(val.wrapping_mul(t)));
        t = t.wrapping_mul(2i64.wrapping_sub(val.wrapping_mul(t)));
        t = t.wrapping_mul(2i64.wrapping_sub(val.wrapping_mul(t)));
        t = t.wrapping_mul(2i64.wrapping_sub(val.wrapping_mul(t)));
        t = t.wrapping_mul(2i64.wrapping_sub(val.wrapping_mul(t)));
        assert_eq!(t.wrapping_mul(val), 1);
        t
    }

    /// Java: static MutableBigInteger modInverseBP2(MutableBigInteger mod, int k).
    fn mod_inverse_bp2(modulus: &MutableBigInteger, k: i32) -> MutableBigInteger {
        Self::fixup(
            MutableBigInteger::new_int(1),
            MutableBigInteger::new_from_mutable(modulus),
            k,
        )
    }

    /// Java: private MutableBigInteger modInverse(MutableBigInteger mod).
    fn mod_inverse(&self, modulus: &MutableBigInteger) -> MutableBigInteger {
        let mut p = MutableBigInteger::new_from_mutable(modulus);
        let mut f = MutableBigInteger::new_from_mutable(self);
        let mut g = MutableBigInteger::new_from_mutable(&p);
        let mut c = SignedMutableBigInteger::new_int(1);
        let mut d = SignedMutableBigInteger::new_empty();
        let mut k = 0;
        if f.is_even() {
            let trailing_zeros = f.get_lowest_set_bit();
            f.right_shift(trailing_zeros);
            d.left_shift(trailing_zeros);
            k = trailing_zeros;
        }
        while !f.is_one() {
            if f.is_zero() {
                panic!("ArithmeticException: BigInteger not invertible.");
            }
            if f.compare(&g) < 0 {
                std::mem::swap(&mut f, &mut g);
                std::mem::swap(&mut d, &mut c);
            }
            if ((f.value[(f.offset + f.int_len - 1) as usize]
                ^ g.value[(g.offset + g.int_len - 1) as usize])
                & 3)
                == 0
            {
                f.subtract(&g);
                c.signed_subtract(&d);
            } else {
                f.add(&g);
                c.signed_add(&d);
            }
            let trailing_zeros = f.get_lowest_set_bit();
            f.right_shift(trailing_zeros);
            d.left_shift(trailing_zeros);
            k += trailing_zeros;
        }
        if c.compare(&p) >= 0 {
            let mut quotient = MutableBigInteger::new_empty();
            let remainder = c
                .inner
                .divide_with_remainder(&p, &mut quotient, true)
                .expect("remainder");
            c.inner.copy_value(&remainder);
        }
        if c.sign < 0 {
            c.signed_add_mutable(&p);
        }
        Self::fixup(c.inner, p, k)
    }

    /// Java: static MutableBigInteger fixup(MutableBigInteger c, MutableBigInteger
    /// p, int k).
    fn fixup(mut c: MutableBigInteger, mut p: MutableBigInteger, k: i32) -> MutableBigInteger {
        let mut temp = MutableBigInteger::new_empty();
        let r = -Self::inverse_mod32(p.value[(p.offset + p.int_len - 1) as usize]);
        let mut i = 0;
        let num_words = k >> 5;
        while i < num_words {
            let v = r.wrapping_mul(c.value[(c.offset + c.int_len - 1) as usize]);
            p.mul(v, &mut temp);
            c.add(&temp);
            c.int_len -= 1;
            i += 1;
        }
        let num_bits = k & 0x1f;
        if num_bits != 0 {
            let mut v = r.wrapping_mul(c.value[(c.offset + c.int_len - 1) as usize]);
            v &= (1i32 << num_bits) - 1;
            p.mul(v, &mut temp);
            c.add(&temp);
            c.right_shift(num_bits);
        }
        if c.compare(&p) >= 0 {
            let mut quotient = MutableBigInteger::new_empty();
            c = c
                .divide_with_remainder(&p, &mut quotient, true)
                .expect("remainder");
        }
        c
    }

    /// Java: MutableBigInteger euclidModInverse(int k).
    fn euclid_mod_inverse(&self, k: i32) -> MutableBigInteger {
        let mut b = MutableBigInteger::new_int(1);
        b.left_shift(k);
        let mut modulus = MutableBigInteger::new_from_mutable(&b);
        let mut a = MutableBigInteger::new_from_mutable(self);
        let mut q = MutableBigInteger::new_empty();
        let mut r = b
            .divide_with_remainder(&a, &mut q, true)
            .expect("remainder");
        let mut swapper = b;
        b = r;
        r = swapper;
        let mut t1 = MutableBigInteger::new_from_mutable(&q);
        let mut t0 = MutableBigInteger::new_int(1);
        let mut temp = MutableBigInteger::new_empty();
        while !b.is_one() {
            r = a
                .divide_with_remainder(&b, &mut q, true)
                .expect("remainder");
            if r.int_len == 0 {
                panic!("ArithmeticException: BigInteger not invertible.");
            }
            swapper = r;
            a = swapper;
            if q.int_len == 1 {
                t1.mul(q.value[q.offset as usize], &mut temp);
            } else {
                q.multiply(&t1, &mut temp);
            }
            swapper = q;
            q = temp;
            temp = swapper;
            t0.add(&q);
            if a.is_one() {
                return t0;
            }
            r = b
                .divide_with_remainder(&a, &mut q, true)
                .expect("remainder");
            if r.int_len == 0 {
                panic!("ArithmeticException: BigInteger not invertible.");
            }
            swapper = b;
            b = r;
            if q.int_len == 1 {
                t0.mul(q.value[q.offset as usize], &mut temp);
            } else {
                q.multiply(&t0, &mut temp);
            }
            swapper = q;
            q = temp;
            temp = swapper;
            t1.add(&q);
        }
        modulus.subtract(&t1);
        modulus
    }

    /// Java: long toCompactValue(int sign) — used only by BigDecimal (not ported).
    #[allow(dead_code)]
    fn to_compact_value_unused(&self) -> i64 {
        0
    }
    /// Java: MutableBigInteger normalize (adapter).
    pub(crate) fn normalize_public(&mut self) {
        self.normalize()
    }

    /// Java: MutableBigInteger compare (public within the port for
    /// SignedMutableBigInteger).
    pub(crate) fn compare_public(&self, b: &MutableBigInteger) -> i32 {
        self.compare(b)
    }

    /// Java: MutableBigInteger divide (adapter for the BigInteger division
    /// call sites).
    pub(crate) fn divide_public(
        &self,
        b: &MutableBigInteger,
        quotient: &mut MutableBigInteger,
    ) -> Option<MutableBigInteger> {
        self.divide_with_remainder(b, quotient, true)
    }

    /// Java: MutableBigInteger add (adapter for the BigInteger modPow call
    /// sites).
    pub(crate) fn add_public(&mut self, addend: &MutableBigInteger) {
        self.add(addend)
    }

    /// Java: MutableBigInteger multiply (adapter for the BigInteger modPow
    /// call sites).
    pub(crate) fn multiply_public(&self, y: &MutableBigInteger, z: &mut MutableBigInteger) {
        self.multiply(y, z)
    }

}
