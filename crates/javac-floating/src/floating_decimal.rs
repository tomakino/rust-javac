//! Port of \`jdk.internal.math.FloatingDecimal\` (JDK 17).
//!
//! §9.17 (standard-library adapter): upstream algorithms,
//! control flow, constants and comments are preserved; Java language
//! constructs are adapted as described in lib.rs (char[] -> Vec<u16>,
//! StringBuilder/Appendable -> String, ThreadLocal converter -> freshly
//! constructed converter, java.util.regex hex-float matcher -> an explicit
//! equivalent matcher in the parseHexString port).

use std::sync::LazyLock;

use crate::fdbig_integer::FDBigInteger;

//
// Constants of the implementation;
// most are IEEE-754 related.
// (There are more really boring constants at the end.)
//
const EXP_SHIFT: i32 = 52; // DoubleConsts.SIGNIFICAND_WIDTH - 1
const FRACT_HOB: i64 = 1i64 << EXP_SHIFT; // assumed High-Order bit
const EXP_ONE: i64 = (1023i64) << EXP_SHIFT; // exponent of 1.0
const MAX_SMALL_BIN_EXP: i32 = 62;
const MIN_SMALL_BIN_EXP: i32 = -(63 / 3);
const MAX_DECIMAL_DIGITS: i32 = 15;
const MAX_DECIMAL_EXPONENT: i32 = 308;
const MIN_DECIMAL_EXPONENT: i32 = -324;
const BIG_DECIMAL_EXPONENT: i32 = 324; // i.e. abs(MIN_DECIMAL_EXPONENT)
const MAX_NDIGITS: i32 = 1100;

const SINGLE_EXP_SHIFT: i32 = 23; // FloatConsts.SIGNIFICAND_WIDTH - 1
const SINGLE_FRACT_HOB: i32 = 1i32 << SINGLE_EXP_SHIFT;
const SINGLE_MAX_DECIMAL_DIGITS: i32 = 7;
const SINGLE_MAX_DECIMAL_EXPONENT: i32 = 38;
const SINGLE_MIN_DECIMAL_EXPONENT: i32 = -45;
const SINGLE_MAX_NDIGITS: i32 = 200;

const INT_DECIMAL_DIGITS: i32 = 9;

// DoubleConsts
const SIGNIFICAND_WIDTH: i32 = 53;
const MIN_SUB_EXPONENT: i32 = -1022 - (SIGNIFICAND_WIDTH - 1); // -1074
const EXP_BIAS: i32 = 1023;
const SIGN_BIT_MASK: i64 = 0x8000_0000_0000_0000u64 as i64;
const EXP_BIT_MASK: i64 = 0x7ff0_0000_0000_0000u64 as i64;
const SIGNIF_BIT_MASK: i64 = 0x000f_ffff_ffff_ffffu64 as i64;

// FloatConsts
const F_SIGNIFICAND_WIDTH: i32 = 24;
const F_MIN_SUB_EXPONENT: i32 = -126 - (F_SIGNIFICAND_WIDTH - 1); // -149
const F_EXP_BIAS: i32 = 127;
const F_SIGN_BIT_MASK: i32 = 0x8000_0000u32 as i32;
const F_EXP_BIT_MASK: i32 = 0x7f80_0000u32 as i32;
const F_SIGNIF_BIT_MASK: i32 = 0x007f_ffffu32 as i32;

// java.lang.Double constants used by the upstream code.
const DOUBLE_MIN_EXPONENT: i32 = -1022;
const DOUBLE_MAX_EXPONENT: i32 = 1023;
const FLOAT_MIN_EXPONENT: i32 = -126;
const FLOAT_MAX_EXPONENT: i32 = 127;
const FLOAT_MAX_VALUE: f32 = f32::MAX;
const FLOAT_MIN_VALUE: f32 = f32::from_bits(1);

const INFINITY_REP: &str = "Infinity";
const INFINITY_LENGTH: usize = INFINITY_REP.len();
const NAN_REP: &str = "NaN";
const NAN_LENGTH: usize = NAN_REP.len();

fn double_min_value() -> f64 {
    f64::from_bits(1)
}

/// A converter which can process single or double precision floating point
/// values into an ASCII String representation.
#[allow(dead_code)] // upstream public interface (used by external callers)
pub(crate) trait BinaryToASCIIConverter {
    /// Converts a floating point value into an ASCII String.
    fn to_java_format_string(&self) -> String;

    /// Appends a floating point value to a String.
    fn append_to(&self, buf: &mut String);

    /// Retrieves the decimal exponent most closely corresponding to this value.
    fn get_decimal_exponent(&self) -> i32;

    /// Retrieves the value as an array of digits.
    fn get_digits(&self, digits: &mut [u16]) -> usize;

    /// Indicates the sign of the value.
    fn is_negative(&self) -> bool;

    /// Indicates whether the value is either infinite or not a number.
    fn is_exceptional(&self) -> bool;

    /// Indicates whether the value was rounded up during the binary to ASCII
    /// conversion.
    fn digits_rounded_up(&self) -> bool;

    /// Indicates whether the binary to ASCII conversion was exact.
    fn decimal_digits_exact(&self) -> bool;
}

/// A BinaryToASCIIConverter which represents NaN and infinite values.
#[derive(Clone)]
struct ExceptionalBinaryToASCIIBuffer {
    image: String,
    is_negative: bool,
}

impl BinaryToASCIIConverter for ExceptionalBinaryToASCIIBuffer {
    fn to_java_format_string(&self) -> String {
        self.image.clone()
    }

    fn append_to(&self, buf: &mut String) {
        buf.push_str(&self.image);
    }

    fn get_decimal_exponent(&self) -> i32 {
        panic!("Exceptional value does not have an exponent");
    }

    fn get_digits(&self, _digits: &mut [u16]) -> usize {
        panic!("Exceptional value does not have digits");
    }

    fn is_negative(&self) -> bool {
        self.is_negative
    }

    fn is_exceptional(&self) -> bool {
        true
    }

    fn digits_rounded_up(&self) -> bool {
        panic!("Exceptional value is not rounded");
    }

    fn decimal_digits_exact(&self) -> bool {
        panic!("Exceptional value is not exact");
    }
}

/// A buffered implementation of BinaryToASCIIConverter.
#[derive(Clone)]
struct BinaryToASCIIBuffer {
    is_negative: bool,
    dec_exponent: i32,
    first_digit_index: usize,
    n_digits: usize,
    digits: Vec<u16>,

    // True if the dtoa() binary to decimal conversion was exact.
    exact_decimal_conversion: bool,

    // True if the result of the binary to decimal conversion was rounded-up
    // at the end of the conversion process, i.e. roundUp() method was called.
    decimal_digits_rounded_up: bool,
}

impl BinaryToASCIIBuffer {
    /// Default constructor; used for non-zero values.
    fn new() -> BinaryToASCIIBuffer {
        BinaryToASCIIBuffer {
            is_negative: false,
            dec_exponent: 0,
            first_digit_index: 0,
            n_digits: 0,
            digits: vec![0u16; 20],
            exact_decimal_conversion: false,
            decimal_digits_rounded_up: false,
        }
    }

    /// Creates a specialized value (positive and negative zeros).
    fn for_zero(is_negative: bool, digits: Vec<u16>) -> BinaryToASCIIBuffer {
        let n_digits = digits.len();
        BinaryToASCIIBuffer {
            is_negative,
            dec_exponent: 0,
            first_digit_index: 0,
            n_digits,
            digits,
            exact_decimal_conversion: false,
            decimal_digits_rounded_up: false,
        }
    }

    fn set_sign(&mut self, is_negative: bool) {
        self.is_negative = is_negative;
    }

    /// This is the easy subcase --
    /// all the significant bits, after scaling, are held in lvalue.
    fn develop_long_digits(&mut self, dec_exponent: i32, lvalue: i64, insignificant_digits: i32) {
        let mut lvalue = lvalue;
        let mut dec_exponent = dec_exponent;
        if insignificant_digits != 0 {
            // Discard non-significant low-order bits, while rounding,
            // up to insignificant value.
            let pow10 = fdbig_long5pow(insignificant_digits)
                .wrapping_shl(insignificant_digits as u32); // 10^i == 5^i * 2^i;
            let residue = lvalue % pow10;
            lvalue /= pow10;
            dec_exponent += insignificant_digits;
            if residue >= (pow10 >> 1) {
                // round up based on the low-order bits we're discarding
                lvalue += 1;
            }
        }
        let mut digitno = self.digits.len() as i32 - 1;
        let mut c;
        if lvalue <= i32::MAX as i64 {
            assert!(lvalue > 0, "{lvalue}"); // lvalue <= 0
            // even easier subcase!
            // can do int arithmetic rather than long!
            let mut ivalue = lvalue as i32;
            c = ivalue % 10;
            ivalue /= 10;
            while c == 0 {
                dec_exponent += 1;
                c = ivalue % 10;
                ivalue /= 10;
            }
            while ivalue != 0 {
                self.digits[digitno as usize] = (c + b'0' as i32) as u16;
                digitno -= 1;
                dec_exponent += 1;
                c = ivalue % 10;
                ivalue /= 10;
            }
            self.digits[digitno as usize] = (c + b'0' as i32) as u16;
        } else {
            // same algorithm as above (same bugs, too)
            // but using long arithmetic.
            c = (lvalue % 10i64) as i32;
            lvalue /= 10i64;
            while c == 0 {
                dec_exponent += 1;
                c = (lvalue % 10i64) as i32;
                lvalue /= 10i64;
            }
            while lvalue != 0 {
                self.digits[digitno as usize] = (c + b'0' as i32) as u16;
                digitno -= 1;
                dec_exponent += 1;
                c = (lvalue % 10i64) as i32;
                lvalue /= 10;
            }
            self.digits[digitno as usize] = (c + b'0' as i32) as u16;
        }
        self.dec_exponent = dec_exponent + 1;
        self.first_digit_index = digitno as usize;
        self.n_digits = self.digits.len() - digitno as usize;
    }

    fn dtoa(&mut self, bin_exp: i32, mut fract_bits: i64, n_significant_bits: i32, is_compatible_format: bool) {
        assert!(fract_bits > 0); // fractBits here can't be zero or negative
        assert_ne!(fract_bits & FRACT_HOB, 0); // Hi-order bit should be set
        // Examine number. Determine if it is an easy case,
        // which we can do pretty trivially using float/long conversion,
        // or whether we must do real work.
        let tail_zeros = (fract_bits as u64).trailing_zeros() as i32;

        // number of significant bits of fractBits;
        let n_fract_bits = EXP_SHIFT + 1 - tail_zeros;

        // reset flags to default values as dtoa() does not always set these
        // flags and a prior call to dtoa() might have set them to incorrect
        // values with respect to the current state.
        self.decimal_digits_rounded_up = false;
        self.exact_decimal_conversion = false;

        // number of significant bits to the right of the point.
        let n_tiny_bits = i32::max(0, n_fract_bits - bin_exp - 1);
        if bin_exp <= MAX_SMALL_BIN_EXP && bin_exp >= MIN_SMALL_BIN_EXP {
            // Look more closely at the number to decide if,
            // with scaling by 10^nTinyBits, the result will fit in
            // a long.
            if (n_tiny_bits as usize) < fdbig_long5pow_len()
                && (n_fract_bits + N_5_BITS[n_tiny_bits as usize]) < 64
            {
                if n_tiny_bits == 0 {
                    let insignificant;
                    if bin_exp > n_significant_bits {
                        insignificant =
                            Self::insignificant_digits_for_pow2(bin_exp - n_significant_bits - 1);
                    } else {
                        insignificant = 0;
                    }
                    if bin_exp >= EXP_SHIFT {
                        fract_bits = fract_bits.wrapping_shl((bin_exp - EXP_SHIFT) as u32);
                    } else {
                        fract_bits = ((fract_bits as u64) >> (EXP_SHIFT - bin_exp) as u32) as i64;
                    }
                    self.develop_long_digits(0, fract_bits, insignificant);
                    return;
                }
            }
        }
        //
        // This is the hard case. We are going to compute large positive
        // integers B and S and integer decExp, s.t.
        //      d = ( B / S )// 10^decExp
        //      1 <= B / S < 10
        // ...
        let dec_exp = Self::estimate_dec_exp(fract_bits, bin_exp);
        let b5;
        let mut b2;
        let s5;
        let s2;
        let m5;
        let mut m2;

        b5 = i32::max(0, -dec_exp);
        b2 = b5 + n_tiny_bits + bin_exp;

        s5 = i32::max(0, dec_exp);
        s2 = s5 + n_tiny_bits;

        m5 = b5;
        m2 = b2 - n_significant_bits;

        fract_bits = ((fract_bits as u64) >> tail_zeros as u32) as i64;
        b2 -= n_fract_bits - 1;
        let common2factor = i32::min(b2, s2);
        b2 -= common2factor;
        let s2 = s2 - common2factor;
        m2 -= common2factor;

        // HACK!! For exact powers of two, the next smallest number
        // is only half as far away as we think (because the meaning of
        // ULP changes at power-of-two bounds) for this reason, we
        // hack M2. Hope this works.
        if n_fract_bits == 1 {
            m2 -= 1;
        }

        if m2 < 0 {
            // oops.
            // since we cannot scale M down far enough,
            // we must scale the other values up.
            b2 -= m2;
            let s2_new = s2 - m2;
            m2 = 0;
            return self.dtoa_hard(fract_bits, n_fract_bits, dec_exp, b2, s2_new, s5, m5, m2, is_compatible_format);
        }
        self.dtoa_hard(fract_bits, n_fract_bits, dec_exp, b2, s2, s5, m5, m2, is_compatible_format);
    }

    #[allow(clippy::too_many_arguments)]
    fn dtoa_hard(
        &mut self,
        fract_bits: i64,
        n_fract_bits: i32,
        mut dec_exp: i32,
        b2: i32,
        s2: i32,
        s5: i32,
        m5: i32,
        m2: i32,
        is_compatible_format: bool,
    ) {
        let b5 = i32::max(0, -dec_exp);
        // binary digits needed to represent B, approx.
        let bbits = n_fract_bits
            + b2
            + (if (b5 as usize) < N_5_BITS.len() {
                N_5_BITS[b5 as usize]
            } else {
                b5 * 3
            });
        // binary digits needed to represent 10*S, approx.
        let ten_sbits = s2
            + 1
            + (if ((s5 + 1) as usize) < N_5_BITS.len() {
                N_5_BITS[(s5 + 1) as usize]
            } else {
                (s5 + 1) * 3
            });
        let mut ndigit: usize;
        let mut low: bool;
        let mut high: bool;
        let low_digit_difference: i64;
        let mut q: i32;
        if bbits < 64 && ten_sbits < 64 {
            if bbits < 32 && ten_sbits < 32 {
                // wa-hoo! They're all ints!
                let b = (fract_bits as i32)
                    .wrapping_mul(fdbig_small5pow(b5))
                    .wrapping_shl(b2 as u32);
                let s = fdbig_small5pow(s5).wrapping_shl(s2 as u32);
                let mut m = fdbig_small5pow(m5).wrapping_shl(m2 as u32);
                let tens = s.wrapping_mul(10);
                //
                // Unroll the first iteration. If our decExp estimate
                // was too high, our first quotient will be zero. In this
                // case, we discard it and decrement decExp.
                //
                ndigit = 0usize;
                q = b / s;
                let mut b = b.wrapping_rem(s).wrapping_mul(10);
                m = m.wrapping_mul(10);
                low = b < m;
                high = b.wrapping_add(m) > tens;
                assert!(q < 10, "{q}"); // excessively large digit
                if q == 0 && !high {
                    // oops. Usually ignore leading zero.
                    dec_exp -= 1;
                } else {
                    self.digits[ndigit] = (b'0' as i32 + q) as u16;
                    ndigit += 1;
                }
                //
                // HACK! Java spec sez that we always have at least
                // one digit after the . in either F- or E-form output.
                // Thus we will need more than one digit if we're using
                // E-form
                //
                if !is_compatible_format || dec_exp < -3 || dec_exp >= 8 {
                    high = false;
                    low = false;
                }
                while !low && !high {
                    let _ = b;
                    q = b / s;
                    b = b.wrapping_rem(s).wrapping_mul(10);
                    m = m.wrapping_mul(10);
                    assert!(q < 10, "{q}"); // excessively large digit
                    if m > 0 {
                        low = b < m;
                        high = b.wrapping_add(m) > tens;
                    } else {
                        // hack -- m might overflow!
                        // in this case, it is certainly > b,
                        // which won't
                        // and b+m > tens, too, since that has overflowed
                        // either!
                        low = true;
                        high = true;
                    }
                    self.digits[ndigit] = (b'0' as i32 + q) as u16;
                    ndigit += 1;
                }
                low_digit_difference = (b.wrapping_shl(1) - tens) as i64;
                self.exact_decimal_conversion = b == 0;
            } else {
                // still good! they're all longs!
                let mut b = fract_bits.wrapping_mul(fdbig_long5pow(b5))
                    .wrapping_shl(b2 as u32);
                let s = fdbig_long5pow(s5).wrapping_shl(s2 as u32);
                let mut m = fdbig_long5pow(m5).wrapping_shl(m2 as u32);
                let tens = s.wrapping_mul(10);
                //
                // Unroll the first iteration. If our decExp estimate
                // was too high, our first quotient will be zero. In this
                // case, we discard it and decrement decExp.
                //
                ndigit = 0usize;
                q = (b / s) as i32;
                b = b.wrapping_rem(s).wrapping_mul(10);
                m = m.wrapping_mul(10);
                low = b < m;
                high = b.wrapping_add(m) > tens;
                assert!(q < 10, "{q}"); // excessively large digit
                if q == 0 && !high {
                    // oops. Usually ignore leading zero.
                    dec_exp -= 1;
                } else {
                    self.digits[ndigit] = (b'0' as i32 + q) as u16;
                    ndigit += 1;
                }
                //
                // HACK! Java spec sez that we always have at least
                // one digit after the . in either F- or E-form output.
                // Thus we will need more than one digit if we're using
                // E-form
                //
                if !is_compatible_format || dec_exp < -3 || dec_exp >= 8 {
                    high = false;
                    low = false;
                }
                while !low && !high {
                    q = (b / s) as i32;
                    b = b.wrapping_rem(s).wrapping_mul(10);
                    m = m.wrapping_mul(10);
                    assert!(q < 10, "{q}"); // excessively large digit
                    if m > 0 {
                        low = b < m;
                        high = b.wrapping_add(m) > tens;
                    } else {
                        // hack -- m might overflow!
                        // in this case, it is certainly > b,
                        // which won't
                        // and b+m > tens, too, since that has overflowed
                        // either!
                        low = true;
                        high = true;
                    }
                    self.digits[ndigit] = (b'0' as i32 + q) as u16;
                    ndigit += 1;
                }
                low_digit_difference = (b.wrapping_shl(1) - tens) as i64;
                self.exact_decimal_conversion = b == 0;
            }
        } else {
            //
            // We really must do FDBigInteger arithmetic.
            // Fist, construct our FDBigInteger initial values.
            //
            let mut sval = FDBigInteger::value_of_pow52(s5, s2);
            let shift_bias = sval.get_normalization_bias();
            sval = sval.left_shift(shift_bias); // normalize so that division works better

            let mut bval =
                FDBigInteger::value_of_mul_pow52(fract_bits, b5, b2 + shift_bias);
            let mut mval = FDBigInteger::value_of_pow52(m5 + 1, m2 + shift_bias + 1);

            let ten_sval = FDBigInteger::value_of_pow52(s5 + 1, s2 + shift_bias + 1); //Sval.mult( 10 );
            //
            // Unroll the first iteration. If our decExp estimate
            // was too high, our first quotient will be zero. In this
            // case, we discard it and decrement decExp.
            //
            ndigit = 0usize;
            q = bval.quo_rem_iteration(&sval);
            low = bval.cmp(&mval) < 0;
            high = ten_sval.add_and_cmp(&bval, &mval) <= 0;

            assert!(q < 10, "{q}"); // excessively large digit
            if q == 0 && !high {
                // oops. Usually ignore leading zero.
                dec_exp -= 1;
            } else {
                self.digits[ndigit] = (b'0' as i32 + q) as u16;
                ndigit += 1;
            }
            //
            // HACK! Java spec sez that we always have at least
            // one digit after the . in either F- or E-form output.
            // Thus we will need more than one digit if we're using
            // E-form
            //
            if !is_compatible_format || dec_exp < -3 || dec_exp >= 8 {
                high = false;
                low = false;
            }
            while !low && !high {
                q = bval.quo_rem_iteration(&sval);
                assert!(q < 10, "{q}"); // excessively large digit
                mval = mval.mult_by10(); //Mval = Mval.mult( 10 );
                low = bval.cmp(&mval) < 0;
                high = ten_sval.add_and_cmp(&bval, &mval) <= 0;
                self.digits[ndigit] = (b'0' as i32 + q) as u16;
                ndigit += 1;
            }
            if high && low {
                bval = bval.left_shift(1);
                low_digit_difference = bval.cmp(&ten_sval) as i64;
            } else {
                low_digit_difference = 0; // this here only for flow analysis!
            }
            self.exact_decimal_conversion = bval.cmp(&FDBigInteger::zero()) == 0;
        }
        self.dec_exponent = dec_exp + 1;
        self.first_digit_index = 0;
        self.n_digits = ndigit;
        //
        // Last digit gets rounded based on stopping condition.
        //
        if high {
            if low {
                if low_digit_difference == 0 {
                    // it's a tie!
                    // choose based on which digits we like.
                    if (self.digits[self.first_digit_index + self.n_digits - 1] & 1) != 0 {
                        self.roundup();
                    }
                } else if low_digit_difference > 0 {
                    self.roundup();
                }
            } else {
                self.roundup();
            }
        }
    }

    // add one to the least significant digit.
    // in the unlikely event there is a carry out, deal with it.
    // assert that this will only happen where there
    // is only one digit, e.g. (float)1e-44 seems to do it.
    //
    fn roundup(&mut self) {
        let mut i = self.first_digit_index + self.n_digits - 1;
        let mut q = self.digits[i];
        if q == b'9' as u16 {
            while q == b'9' as u16 && i > self.first_digit_index {
                self.digits[i] = b'0' as u16;
                i -= 1;
                q = self.digits[i];
            }
            if q == b'9' as u16 {
                // carryout! High-order 1, rest 0s, larger exp.
                self.dec_exponent += 1;
                self.digits[self.first_digit_index] = b'1' as u16;
                return;
            }
            // else fall through.
        }
        self.digits[i] = q + 1;
        self.decimal_digits_rounded_up = true;
    }

    /// Estimate decimal exponent. (If it is small-ish,
    /// we could double-check.)
    fn estimate_dec_exp(fract_bits: i64, bin_exp: i32) -> i32 {
        let d2 = f64::from_bits(
            (EXP_ONE | (fract_bits & SIGNIF_BIT_MASK)) as u64,
        );
        let d = (d2 - 1.5) * 0.289529654 + 0.176091259 + (bin_exp as f64) * 0.301029995663981;
        let d_bits = d.to_bits(); // can't be NaN here so use raw
        let exponent = ((d_bits & EXP_BIT_MASK as u64) >> EXP_SHIFT) as i32 - EXP_BIAS;
        let is_negative = (d_bits & SIGN_BIT_MASK as u64) != 0; // discover sign
        if (0..52).contains(&exponent) {
            // hot path
            let mask = SIGNIF_BIT_MASK >> exponent;
            let r = (((d_bits as i64 & SIGNIF_BIT_MASK) | FRACT_HOB)
                >> (EXP_SHIFT - exponent)) as i32;
            return if is_negative {
                if (mask as i64 & d_bits as i64) == 0 { -r } else { -r - 1 }
            } else {
                r
            };
        } else if exponent < 0 {
            return if (d_bits as i64 & !SIGN_BIT_MASK) == 0 {
                0
            } else if is_negative {
                -1
            } else {
                0
            };
        }
        // if (exponent >= 52)
        d as i32
    }

    #[allow(dead_code)] // upstream defines it for documentation of the table
    fn insignificant_digits(insignificant: i32) -> i32 {
        let mut i = 0;
        let mut insignificant = insignificant;
        while insignificant >= 10 {
            insignificant /= 10;
            i += 1;
        }
        i
    }

    /// Calculates
    /// insignificantDigitsForPow2(v) == insignificantDigits(1L<<v)
    fn insignificant_digits_for_pow2(p2: i32) -> i32 {
        if p2 > 1 && (p2 as usize) < INSIGNIFICANT_DIGITS_NUMBER.len() {
            return INSIGNIFICANT_DIGITS_NUMBER[p2 as usize];
        }
        0
    }

    fn get_chars(&self, result: &mut [u16]) -> usize {
        assert!(self.n_digits <= 19, "{}", self.n_digits); // generous bound on size of nDigits
        let mut i = 0usize;
        if self.is_negative {
            result[0] = b'-' as u16;
            i = 1;
        }
        if self.dec_exponent > 0 && self.dec_exponent < 8 {
            // print digits.digits.
            let mut char_length = i32::min(self.n_digits as i32, self.dec_exponent) as usize;
            result[i..i + char_length]
                .copy_from_slice(&self.digits[self.first_digit_index..self.first_digit_index + char_length]);
            i += char_length;
            if char_length < self.dec_exponent as usize {
                char_length = self.dec_exponent as usize - char_length;
                for x in &mut result[i..i + char_length] {
                    *x = b'0' as u16;
                }
                i += char_length;
                result[i] = b'.' as u16;
                i += 1;
                result[i] = b'0' as u16;
                i += 1;
            } else {
                result[i] = b'.' as u16;
                i += 1;
                if char_length < self.n_digits {
                    let t = self.n_digits - char_length;
                    result[i..i + t].copy_from_slice(
                        &self.digits[self.first_digit_index + char_length
                            ..self.first_digit_index + char_length + t],
                    );
                    i += t;
                } else {
                    result[i] = b'0' as u16;
                    i += 1;
                }
            }
        } else if self.dec_exponent <= 0 && self.dec_exponent > -3 {
            result[i] = b'0' as u16;
            i += 1;
            result[i] = b'.' as u16;
            i += 1;
            if self.dec_exponent != 0 {
                for x in &mut result[i..(i as i32 - self.dec_exponent) as usize] {
                    *x = b'0' as u16;
                }
                i = (i as i32 - self.dec_exponent) as usize;
            }
            result[i..i + self.n_digits].copy_from_slice(
                &self.digits[self.first_digit_index..self.first_digit_index + self.n_digits],
            );
            i += self.n_digits;
        } else {
            result[i] = self.digits[self.first_digit_index];
            i += 1;
            result[i] = b'.' as u16;
            i += 1;
            if self.n_digits > 1 {
                result[i..i + self.n_digits - 1].copy_from_slice(
                    &self.digits[self.first_digit_index + 1
                        ..self.first_digit_index + self.n_digits],
                );
                i += self.n_digits - 1;
            } else {
                result[i] = b'0' as u16;
                i += 1;
            }
            result[i] = b'E' as u16;
            i += 1;
            let e;
            if self.dec_exponent <= 0 {
                result[i] = b'-' as u16;
                i += 1;
                e = -self.dec_exponent + 1;
            } else {
                e = self.dec_exponent - 1;
            }
            // decExponent has 1, 2, or 3, digits
            if e <= 9 {
                result[i] = (e + b'0' as i32) as u16;
                i += 1;
            } else if e <= 99 {
                result[i] = (e / 10 + b'0' as i32) as u16;
                i += 1;
                result[i] = (e % 10 + b'0' as i32) as u16;
                i += 1;
            } else {
                result[i] = (e / 100 + b'0' as i32) as u16;
                i += 1;
                let mut e = e % 100;
                result[i] = (e / 10 + b'0' as i32) as u16;
                i += 1;
                result[i] = (e % 10 + b'0' as i32) as u16;
                i += 1;
                e = 0;
                let _ = e;
            }
        }
        i
    }
}

impl BinaryToASCIIConverter for BinaryToASCIIBuffer {
    fn to_java_format_string(&self) -> String {
        let mut buffer = [0u16; 26];
        let len = self.get_chars(&mut buffer);
        String::from_utf16_lossy(&buffer[..len])
    }

    fn append_to(&self, buf: &mut String) {
        let mut buffer = [0u16; 26];
        let len = self.get_chars(&mut buffer);
        buf.push_str(&String::from_utf16_lossy(&buffer[..len]));
    }

    fn get_decimal_exponent(&self) -> i32 {
        self.dec_exponent
    }

    fn get_digits(&self, digits: &mut [u16]) -> usize {
        digits[..self.n_digits].copy_from_slice(
            &self.digits[self.first_digit_index..self.first_digit_index + self.n_digits],
        );
        self.n_digits
    }

    fn is_negative(&self) -> bool {
        self.is_negative
    }

    fn is_exceptional(&self) -> bool {
        false
    }

    fn digits_rounded_up(&self) -> bool {
        self.decimal_digits_rounded_up
    }

    fn decimal_digits_exact(&self) -> bool {
        self.exact_decimal_conversion
    }
}

static B2AC_POSITIVE_INFINITY: LazyLock<ExceptionalBinaryToASCIIBuffer> = LazyLock::new(|| {
    ExceptionalBinaryToASCIIBuffer {
        image: INFINITY_REP.to_string(),
        is_negative: false,
    }
});
static B2AC_NEGATIVE_INFINITY: LazyLock<ExceptionalBinaryToASCIIBuffer> = LazyLock::new(|| {
    ExceptionalBinaryToASCIIBuffer {
        image: format!("-{INFINITY_REP}"),
        is_negative: true,
    }
});
static B2AC_NOT_A_NUMBER: LazyLock<ExceptionalBinaryToASCIIBuffer> = LazyLock::new(|| {
    ExceptionalBinaryToASCIIBuffer {
        image: NAN_REP.to_string(),
        is_negative: false,
    }
});
static B2AC_POSITIVE_ZERO: LazyLock<BinaryToASCIIBuffer> =
    LazyLock::new(|| BinaryToASCIIBuffer::for_zero(false, vec![b'0' as u16]));
static B2AC_NEGATIVE_ZERO: LazyLock<BinaryToASCIIBuffer> =
    LazyLock::new(|| BinaryToASCIIBuffer::for_zero(true, vec![b'0' as u16]));

/// Returns a BinaryToASCIIConverter for a double.
pub(crate) fn get_binary_to_ascii_converter_double(d: f64) -> Box<dyn BinaryToASCIIConverter> {
    get_binary_to_ascii_converter_double_compatible(d, true)
}

fn get_binary_to_ascii_converter_double_compatible(
    d: f64,
    is_compatible_format: bool,
) -> Box<dyn BinaryToASCIIConverter> {
    let d_bits = d.to_bits() as i64;
    let is_negative = (d_bits & SIGN_BIT_MASK) != 0; // discover sign
    let mut fract_bits = d_bits & SIGNIF_BIT_MASK;
    let mut bin_exp = ((d_bits & EXP_BIT_MASK) >> EXP_SHIFT) as i32;
    // Discover obvious special cases of NaN and Infinity.
    if bin_exp == ((EXP_BIT_MASK >> EXP_SHIFT) as i32) {
        return if fract_bits == 0 {
            if is_negative {
                Box::new(B2AC_NEGATIVE_INFINITY.clone())
            } else {
                Box::new(B2AC_POSITIVE_INFINITY.clone())
            }
        } else {
            Box::new(B2AC_NOT_A_NUMBER.clone())
        }
    }
    // Finish unpacking
    // Normalize denormalized numbers.
    // Insert assumed high-order bit for normalized numbers.
    // Subtract exponent bias.
    let n_significant_bits;
    if bin_exp == 0 {
        if fract_bits == 0 {
            // not a denorm, just a 0!
            return if is_negative {
                Box::new(B2AC_NEGATIVE_ZERO.clone())
            } else {
                Box::new(B2AC_POSITIVE_ZERO.clone())
            };
        }
        let leading_zeros = (fract_bits as u64).leading_zeros() as i32;
        let shift = leading_zeros - (63 - EXP_SHIFT);
        fract_bits = fract_bits.wrapping_shl(shift as u32);
        bin_exp = 1 - shift;
        n_significant_bits = 64 - leading_zeros; // recall binExp is  - shift count.
    } else {
        fract_bits |= FRACT_HOB;
        n_significant_bits = EXP_SHIFT + 1;
    }
    bin_exp -= EXP_BIAS;
    let mut buf = BinaryToASCIIBuffer::new();
    buf.set_sign(is_negative);
    // call the routine that actually does all the hard work.
    buf.dtoa(bin_exp, fract_bits, n_significant_bits, is_compatible_format);
    Box::new(buf)
}

pub(crate) fn get_binary_to_ascii_converter_float(f: f32) -> Box<dyn BinaryToASCIIConverter> {
    let f_bits = f.to_bits() as i32;
    let is_negative = (f_bits & F_SIGN_BIT_MASK) != 0;
    let mut fract_bits = f_bits & F_SIGNIF_BIT_MASK;
    let mut bin_exp = (f_bits & F_EXP_BIT_MASK) >> SINGLE_EXP_SHIFT;
    // Discover obvious special cases of NaN and Infinity.
    if bin_exp == (F_EXP_BIT_MASK >> SINGLE_EXP_SHIFT) {
        return if fract_bits == 0 {
            if is_negative {
                Box::new(B2AC_NEGATIVE_INFINITY.clone())
            } else {
                Box::new(B2AC_POSITIVE_INFINITY.clone())
            }
        } else {
            Box::new(B2AC_NOT_A_NUMBER.clone())
        }
    }
    // Finish unpacking
    // Normalize denormalized numbers.
    // Insert assumed high-order bit for normalized numbers.
    // Subtract exponent bias.
    let n_significant_bits;
    if bin_exp == 0 {
        if fract_bits == 0 {
            // not a denorm, just a 0!
            return if is_negative {
                Box::new(B2AC_NEGATIVE_ZERO.clone())
            } else {
                Box::new(B2AC_POSITIVE_ZERO.clone())
            };
        }
        let leading_zeros = (fract_bits as u32).leading_zeros() as i32;
        let shift = leading_zeros - (31 - SINGLE_EXP_SHIFT);
        fract_bits = fract_bits.wrapping_shl(shift as u32);
        bin_exp = 1 - shift;
        n_significant_bits = 32 - leading_zeros; // recall binExp is  - shift count.
    } else {
        fract_bits |= SINGLE_FRACT_HOB;
        n_significant_bits = SINGLE_EXP_SHIFT + 1;
    }
    bin_exp -= F_EXP_BIAS;
    let mut buf = BinaryToASCIIBuffer::new();
    buf.set_sign(is_negative);
    // call the routine that actually does all the hard work.
    buf.dtoa(
        bin_exp,
        (fract_bits as i64) << (EXP_SHIFT - SINGLE_EXP_SHIFT),
        n_significant_bits,
        true,
    );
    Box::new(buf)
}

// approximately ceil( log2( long5pow[i] ) )
static N_5_BITS: [i32; 27] = [
    0, 3, 5, 7, 10, 12, 14, 17, 19, 21, 24, 26, 28, 31, 33, 35, 38, 40, 42, 45, 47, 49, 52, 54,
    56, 59, 61,
];

/// If insignificant==(1L << ixd)
/// i = insignificantDigitsNumber[idx] is the same as:
/// int i;
/// for ( i = 0; insignificant >= 10L; i++ )
///        insignificant /= 10L;
static INSIGNIFICANT_DIGITS_NUMBER: [i32; 64] = [
    0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 8, 8, 8,
    9, 9, 9, 9, 10, 10, 10, 11, 11, 11, 12, 12, 12, 12, 13, 13, 13, 14, 14, 14, 15, 15, 15, 15, 16,
    16, 16, 17, 17, 17, 18, 18, 18, 19,
];





// placeholder until the fdbig tables are public; replaced below.
fn fdbig_small5pow(i: i32) -> i32 {
    crate::fdbig_integer::small5pow(i)
}

fn fdbig_long5pow(i: i32) -> i64 {
    crate::fdbig_integer::long5pow(i)
}

fn fdbig_long5pow_len() -> usize {
    crate::fdbig_integer::long5pow_len()
}

/// Java: public static String toJavaFormatString(double d)
pub fn to_java_string_double(d: f64) -> String {
    get_binary_to_ascii_converter_double(d).to_java_format_string()
}

/// Java: public static String toJavaFormatString(float f)
pub fn to_java_string_float(f: f32) -> String {
    get_binary_to_ascii_converter_float(f).to_java_format_string()
}

/// Java: public static double parseDouble(String s) throws NumberFormatException
pub fn parse_double(s: &str) -> f64 {
    try_parse_double(s).unwrap_or_else(|e| panic!("{e}"))
}

/// Java: public static float parseFloat(String s) throws NumberFormatException
pub fn parse_float(s: &str) -> f32 {
    try_parse_float(s).unwrap_or_else(|e| panic!("{e}"))
}

/// Rust form of Java's checked NumberFormatException for the parse entry points
/// (Java \`throws NumberFormatException\` -> \`Result\`; the panicking
/// parse_double/parse_float preserve the Java exception semantics for callers
/// that do not catch it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberFormatException {
    message: String,
}

impl NumberFormatException {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for NumberFormatException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "java.lang.NumberFormatException: {}", self.message)
    }
}

fn to_parse_error(input: &str, error: ParseError) -> NumberFormatException {
    let message = match error {
        ParseError::Message(message) => message,
        ParseError::IndexOutOfBounds => format!("For input string: \"{input}\""),
    };
    NumberFormatException { message }
}

/// Java: public static double parseDouble(String s) throws NumberFormatException
pub fn try_parse_double(s: &str) -> Result<f64, NumberFormatException> {
    match read_java_format_string_impl(s) {
        Ok(converter) => Ok(converter.double_value()),
        Err(error) => Err(to_parse_error(s, error)),
    }
}

/// Java: public static float parseFloat(String s) throws NumberFormatException
pub fn try_parse_float(s: &str) -> Result<f32, NumberFormatException> {
    match read_java_format_string_impl(s) {
        Ok(converter) => Ok(converter.float_value()),
        Err(error) => Err(to_parse_error(s, error)),
    }
}

/// A converter which can process an ASCII String representation
/// of a single or double precision floating point value into a
/// float or a double.
pub(crate) trait ASCIIToBinaryConverter {
    fn double_value(&self) -> f64;
    fn float_value(&self) -> f32;
}

/// A ASCIIToBinaryConverter container for a double.
#[derive(Clone, Copy)]
struct PreparedASCIIToBinaryBuffer {
    double_val: f64,
    float_val: f32,
}

impl ASCIIToBinaryConverter for PreparedASCIIToBinaryBuffer {
    fn double_value(&self) -> f64 {
        self.double_val
    }

    fn float_value(&self) -> f32 {
        self.float_val
    }
}

static A2BC_POSITIVE_INFINITY: PreparedASCIIToBinaryBuffer = PreparedASCIIToBinaryBuffer {
    double_val: f64::INFINITY,
    float_val: f32::INFINITY,
};
static A2BC_NEGATIVE_INFINITY: PreparedASCIIToBinaryBuffer = PreparedASCIIToBinaryBuffer {
    double_val: f64::NEG_INFINITY,
    float_val: f32::NEG_INFINITY,
};
static A2BC_NOT_A_NUMBER: PreparedASCIIToBinaryBuffer = PreparedASCIIToBinaryBuffer {
    double_val: f64::NAN,
    float_val: f32::NAN,
};
static A2BC_POSITIVE_ZERO: PreparedASCIIToBinaryBuffer = PreparedASCIIToBinaryBuffer {
    double_val: 0.0,
    float_val: 0.0,
};
static A2BC_NEGATIVE_ZERO: PreparedASCIIToBinaryBuffer = PreparedASCIIToBinaryBuffer {
    double_val: -0.0,
    float_val: -0.0,
};

/// A buffered implementation of ASCIIToBinaryConverter.
struct ASCIIToBinaryBuffer {
    is_negative: bool,
    dec_exponent: i32,
    digits: Vec<u16>,
    n_digits: i32,
}

impl ASCIIToBinaryBuffer {
    fn new(is_negative: bool, dec_exponent: i32, digits: Vec<u16>, n_digits: i32) -> Self {
        ASCIIToBinaryBuffer {
            is_negative,
            dec_exponent,
            digits,
            n_digits,
        }
    }
}

/// All the positive powers of 10 that can be represented exactly in
/// double/float.
static SMALL_10_POW: [f64; 23] = [
    1.0e0, 1.0e1, 1.0e2, 1.0e3, 1.0e4, 1.0e5, 1.0e6, 1.0e7, 1.0e8, 1.0e9, 1.0e10, 1.0e11,
    1.0e12, 1.0e13, 1.0e14, 1.0e15, 1.0e16, 1.0e17, 1.0e18, 1.0e19, 1.0e20, 1.0e21, 1.0e22,
];
static SINGLE_SMALL_10_POW: [f32; 11] = [
    1.0e0, 1.0e1, 1.0e2, 1.0e3, 1.0e4, 1.0e5, 1.0e6, 1.0e7, 1.0e8, 1.0e9, 1.0e10,
];
static BIG_10_POW: [f64; 5] = [1e16, 1e32, 1e64, 1e128, 1e256];
static TINY_10_POW: [f64; 5] = [1e-16, 1e-32, 1e-64, 1e-128, 1e-256];

const MAX_SMALL_TEN: i32 = SMALL_10_POW.len() as i32 - 1;
const SINGLE_MAX_SMALL_TEN: i32 = SINGLE_SMALL_10_POW.len() as i32 - 1;



impl ASCIIToBinaryConverter for ASCIIToBinaryBuffer {
    /// Takes a FloatingDecimal, which we presumably just scanned in,
    /// and finds out what its value is, as a double.
    fn double_value(&self) -> f64 {
        let mut digits = self.digits.clone();
        let mut n_digits = self.n_digits;
        let k_digits = i32::min(n_digits, MAX_DECIMAL_DIGITS + 1);
        //
        // convert the lead kDigits to a long integer.
        //
        // (special performance hack: start to do it using int)
        let mut i_value = digits[0] as i32 - b'0' as i32;
        let i_digits = i32::min(k_digits, INT_DECIMAL_DIGITS);
        for i in 1..i_digits as usize {
            i_value = i_value * 10 + digits[i] as i32 - b'0' as i32;
        }
        let mut l_value = i_value as i64;
        for i in i_digits as usize..k_digits as usize {
            l_value = l_value * 10 + (digits[i] as i32 - b'0' as i32) as i64;
        }
        let mut d_value = l_value as f64;
        let mut exp = self.dec_exponent - k_digits;
        //
        // lValue now contains a long integer with the value of
        // the first kDigits digits of the number.
        // dValue contains the (double) of the same.
        //
        if n_digits <= MAX_DECIMAL_DIGITS {
            //
            // possibly an easy case.
            // We know that the digits can be represented
            // exactly. And if the exponent isn't too outrageous,
            // the whole thing can be done with one operation,
            // thus one rounding error.
            // Note that all our constructors trim all leading and
            // trailing zeros, so simple values (including zero)
            // will always end up here
            //
            if exp == 0 || d_value == 0.0 {
                return if self.is_negative { -d_value } else { d_value }; // small floating integer
            } else if exp >= 0 {
                if exp <= MAX_SMALL_TEN {
                    //
                    // Can get the answer with one operation,
                    // thus one roundoff.
                    //
                    let r_value = d_value * SMALL_10_POW[exp as usize];
                    return if self.is_negative { -r_value } else { r_value };
                }
                let slop = MAX_DECIMAL_DIGITS - k_digits;
                if exp <= MAX_SMALL_TEN + slop {
                    //
                    // We can multiply dValue by 10^(slop)
                    // and it is still "small" and exact.
                    // Then we can multiply by 10^(exp-slop)
                    // with one rounding.
                    //
                    d_value *= SMALL_10_POW[slop as usize];
                    let r_value = d_value * SMALL_10_POW[(exp - slop) as usize];
                    return if self.is_negative { -r_value } else { r_value };
                }
                //
                // Else we have a hard case with a positive exp.
                //
            } else {
                if exp >= -MAX_SMALL_TEN {
                    //
                    // Can get the answer in one division.
                    //
                    let r_value = d_value / SMALL_10_POW[-exp as usize];
                    return if self.is_negative { -r_value } else { r_value };
                }
                //
                // Else we have a hard case with a negative exp.
                //
            }
        }

        //
        // Harder cases:
        // The sum of digits plus exponent is greater than
        // what we think we can do with one error.
        //
        // Start by approximating the right answer by,
        // naively, scaling by powers of 10.
        //
        if exp > 0 {
            if self.dec_exponent > MAX_DECIMAL_EXPONENT + 1 {
                //
                // Lets face it. This is going to be
                // Infinity. Cut to the chase.
                //
                return if self.is_negative {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                };
            }
            if (exp & 15) != 0 {
                d_value *= SMALL_10_POW[(exp & 15) as usize];
            }
            exp >>= 4;
            if exp != 0 {
                let mut j = 0usize;
                while exp > 1 {
                    if (exp & 1) != 0 {
                        d_value *= BIG_10_POW[j];
                    }
                    j += 1;
                    exp >>= 1;
                }
                //
                // The reason for the weird exp > 1 condition
                // in the above loop was so that the last multiply
                // would get unrolled. We handle it here.
                // It could overflow.
                //
                let mut t = d_value * BIG_10_POW[j];
                if t.is_infinite() {
                    //
                    // It did overflow.
                    // Look more closely at the result.
                    // If the exponent is just one too large,
                    // then use the maximum finite as our estimate
                    // value. Else call the result infinity
                    // and punt it.
                    // ( I presume this could happen because
                    // rounding forces the result here to be
                    // an ULP or two larger than
                    // Double.MAX_VALUE ).
                    //
                    t = d_value / 2.0;
                    t *= BIG_10_POW[j];
                    if t.is_infinite() {
                        return if self.is_negative {
                            f64::NEG_INFINITY
                        } else {
                            f64::INFINITY
                        };
                    }
                    t = f64::MAX;
                }
                d_value = t;
            }
        } else if exp < 0 {
            exp = -exp;
            if self.dec_exponent < MIN_DECIMAL_EXPONENT - 1 {
                //
                // Lets face it. This is going to be
                // zero. Cut to the chase.
                //
                return if self.is_negative { -0.0 } else { 0.0 };
            }
            if (exp & 15) != 0 {
                d_value /= SMALL_10_POW[(exp & 15) as usize];
            }
            exp >>= 4;
            if exp != 0 {
                let mut j = 0usize;
                while exp > 1 {
                    if (exp & 1) != 0 {
                        d_value *= TINY_10_POW[j];
                    }
                    j += 1;
                    exp >>= 1;
                }
                //
                // The reason for the weird exp > 1 condition
                // in the above loop was so that the last multiply
                // would get unrolled. We handle it here.
                // It could underflow.
                //
                let mut t = d_value * TINY_10_POW[j];
                if t == 0.0 {
                    //
                    // It did underflow.
                    // Look more closely at the result.
                    // If the exponent is just one too small,
                    // then use the minimum finite as our estimate
                    // value. Else call the result 0.0
                    // and punt it.
                    // ( I presume this could happen because
                    // rounding forces the result here to be
                    // an ULP or two less than
                    // Double.MIN_VALUE ).
                    //
                    t = d_value * 2.0;
                    t *= TINY_10_POW[j];
                    if t == 0.0 {
                        return if self.is_negative { -0.0 } else { 0.0 };
                    }
                    t = double_min_value();
                }
                d_value = t;
            }
        }

        //
        // dValue is now approximately the result.
        // The hard part is adjusting it, by comparison
        // with FDBigInteger arithmetic.
        // Formulate the EXACT big-number result as
        // bigD0 * 10^exp
        //
        if n_digits > MAX_NDIGITS {
            n_digits = MAX_NDIGITS + 1;
            digits[MAX_NDIGITS as usize] = b'1' as u16;
        }
        let mut big_d0 = FDBigInteger::new_with_digits(
            l_value,
            &digits,
            k_digits as usize,
            n_digits as usize,
        );
        exp = self.dec_exponent - n_digits;

        let mut ieee_bits = d_value.to_bits() as i64; // IEEE-754 bits of double candidate
        let b5 = i32::max(0, -exp); // powers of 5 in bigB, value is not modified inside correctionLoop
        let d5 = i32::max(0, exp); // powers of 5 in bigD, value is not modified inside correctionLoop
        big_d0 = big_d0.mult_by_pow52(d5, 0);
        big_d0.make_immutable(); // prevent bigD0 modification inside correctionLoop
        let mut big_d = FDBigInteger::zero();
        let mut big_d_valid = false;
        let mut prev_d2 = 0;
        'correction_loop: loop {
            // here ieeeBits can't be NaN, Infinity or zero
            let mut binexp = ((ieee_bits as u64) >> EXP_SHIFT) as i32;
            let mut big_bbits = ieee_bits & SIGNIF_BIT_MASK;
            if binexp > 0 {
                big_bbits |= FRACT_HOB;
            } else {
                // Normalize denormalized numbers.
                assert_ne!(big_bbits, 0, "{big_bbits}"); // doubleToBigInt(0.0)
                let leading_zeros = (big_bbits as u64).leading_zeros() as i32;
                let shift = leading_zeros - (63 - EXP_SHIFT);
                big_bbits = big_bbits.wrapping_shl(shift as u32);
                binexp = 1 - shift;
            }
            binexp -= EXP_BIAS;
            let low_order_zeros = (big_bbits as u64).trailing_zeros() as i32;
            big_bbits = ((big_bbits as u64) >> low_order_zeros) as i64;
            let big_int_exp = binexp - EXP_SHIFT + low_order_zeros;
            let big_int_nbits = EXP_SHIFT + 1 - low_order_zeros;

            //
            // Scale bigD, bigB appropriately for
            // big-integer operations.
            // Naively, we multiply by powers of ten
            // and powers of two. What we actually do
            // is keep track of the powers of 5 and
            // powers of 2 we would use, then factor out
            // common divisors before doing the work.
            //
            let mut b2 = b5; // powers of 2 in bigB
            let mut d2 = d5; // powers of 2 in bigD
            let mut ulp2; // powers of 2 in halfUlp.
            if big_int_exp >= 0 {
                b2 += big_int_exp;
            } else {
                d2 -= big_int_exp;
            }
            ulp2 = b2;
            // shift bigB and bigD left by a number s. t.
            // halfUlp is still an integer.
            let hulpbias;
            if binexp <= -EXP_BIAS {
                // This is going to be a denormalized number
                // (if not actually zero).
                // half an ULP is at 2^-(EXP_BIAS+EXP_SHIFT+1)
                hulpbias = binexp + low_order_zeros + EXP_BIAS;
            } else {
                hulpbias = 1 + low_order_zeros;
            }
            b2 += hulpbias;
            d2 += hulpbias;
            // if there are common factors of 2, we might just as well
            // factor them out, as they add nothing useful.
            let common2 = i32::min(b2, i32::min(d2, ulp2));
            b2 -= common2;
            d2 -= common2;
            ulp2 -= common2;
            // do multiplications by powers of 5 and 2
            let big_b = FDBigInteger::value_of_mul_pow52(big_bbits, b5, b2);
            if !big_d_valid || prev_d2 != d2 {
                big_d = big_d0.left_shift(d2);
                prev_d2 = d2;
                big_d_valid = true;
            }
            //
            // to recap:
            // bigB is the scaled-big-int version of our floating-point
            // candidate.
            // bigD is the scaled-big-int version of the exact value
            // as we understand it.
            // halfUlp is 1/2 an ulp of bigB, except for special cases
            // of exact powers of 2
            //
            // the plan is to compare bigB with bigD, and if the difference
            // is less than halfUlp, then we're satisfied. Otherwise,
            // use the ratio of difference to halfUlp to calculate a fudge
            // factor to add to the floating value, then go 'round again.
            //
            let mut diff;
            let mut cmp_result;
            let overvalue;
            cmp_result = big_b.cmp(&big_d);
            if cmp_result > 0 {
                overvalue = true; // our candidate is too big.
                diff = big_b.left_inplace_sub(&big_d); // bigB is not user further - reuse
                if big_int_nbits == 1 && big_int_exp > -EXP_BIAS + 1 {
                    // candidate is a normalized exact power of 2 and
                    // is too big (larger than Double.MIN_NORMAL). We will be subtracting.
                    // For our purposes, ulp is the ulp of the
                    // next smaller range.
                    ulp2 -= 1;
                    if ulp2 < 0 {
                        // rats. Cannot de-scale ulp this far.
                        // must scale diff in other direction.
                        ulp2 = 0;
                        diff = diff.left_shift(1);
                    }
                }
            } else if cmp_result < 0 {
                overvalue = false; // our candidate is too small.
                diff = big_d.right_inplace_sub(big_b); // bigB is not user further - reuse
            } else {
                // the candidate is exactly right!
                // this happens with surprising frequency
                break 'correction_loop;
            }
            cmp_result = diff.cmp_pow52(b5, ulp2);
            if cmp_result < 0 {
                // difference is small.
                // this is close enough
                break 'correction_loop;
            } else if cmp_result == 0 {
                // difference is exactly half an ULP
                // round to some other value maybe, then finish
                if (ieee_bits & 1) != 0 {
                    // half ties to even
                    ieee_bits += if overvalue { -1 } else { 1 }; // nextDown or nextUp
                }
                break 'correction_loop;
            } else {
                // difference is non-trivial.
                // could scale addend by ratio of difference to
                // halfUlp here, if we bothered to compute that difference.
                // Most of the time ( I hope ) it is about 1 anyway.
                ieee_bits += if overvalue { -1 } else { 1 }; // nextDown or nextUp
                if ieee_bits == 0 || ieee_bits == EXP_BIT_MASK {
                    // 0.0 or Double.POSITIVE_INFINITY
                    break 'correction_loop; // oops. Fell off end of range.
                }
                continue; // try again.
            }
        }
        if self.is_negative {
            ieee_bits |= SIGN_BIT_MASK;
        }
        f64::from_bits(ieee_bits as u64)
    }

    /// Takes a FloatingDecimal, which we presumably just scanned in,
    /// and finds out what its value is, as a float.
    /// This is distinct from doubleValue() to avoid the extremely
    /// unlikely case of a double rounding error, wherein the conversion
    /// to double has one rounding error, and the conversion of that double
    /// to a float has another rounding error, IN THE WRONG DIRECTION,
    /// ( because of the preference to a zero low-order bit ).
    fn float_value(&self) -> f32 {
        let mut digits = self.digits.clone();
        let mut n_digits = self.n_digits;
        let k_digits = i32::min(n_digits, SINGLE_MAX_DECIMAL_DIGITS + 1);
        //
        // convert the lead kDigits to an integer.
        //
        let mut i_value = digits[0] as i32 - b'0' as i32;
        for i in 1..k_digits as usize {
            i_value = i_value * 10 + digits[i] as i32 - b'0' as i32;
        }
        let mut f_value = i_value as f32;
        let mut exp = self.dec_exponent - k_digits;
        //
        // iValue now contains an integer with the value of
        // the first kDigits digits of the number.
        // fValue contains the (float) of the same.
        //
        if n_digits <= SINGLE_MAX_DECIMAL_DIGITS {
            //
            // possibly an easy case.
            // We know that the digits can be represented
            // exactly. And if the exponent isn't too outrageous,
            // the whole thing can be done with one operation,
            // thus one rounding error.
            // Note that all our constructors trim all leading and
            // trailing zeros, so simple values (including zero)
            // will always end up here.
            //
            if exp == 0 || f_value == 0.0 {
                return if self.is_negative { -f_value } else { f_value }; // small floating integer
            } else if exp >= 0 {
                if exp <= SINGLE_MAX_SMALL_TEN {
                    //
                    // Can get the answer with one operation,
                    // thus one roundoff.
                    //
                    f_value *= SINGLE_SMALL_10_POW[exp as usize];
                    return if self.is_negative { -f_value } else { f_value };
                }
                let slop = SINGLE_MAX_DECIMAL_DIGITS - k_digits;
                if exp <= SINGLE_MAX_SMALL_TEN + slop {
                    //
                    // We can multiply fValue by 10^(slop)
                    // and it is still "small" and exact.
                    // Then we can multiply by 10^(exp-slop)
                    // with one rounding.
                    //
                    f_value *= SINGLE_SMALL_10_POW[slop as usize];
                    f_value *= SINGLE_SMALL_10_POW[(exp - slop) as usize];
                    return if self.is_negative { -f_value } else { f_value };
                }
                //
                // Else we have a hard case with a positive exp.
                //
            } else {
                if exp >= -SINGLE_MAX_SMALL_TEN {
                    //
                    // Can get the answer in one division.
                    //
                    f_value /= SINGLE_SMALL_10_POW[-exp as usize];
                    return if self.is_negative { -f_value } else { f_value };
                }
                //
                // Else we have a hard case with a negative exp.
                //
            }
        } else if self.dec_exponent >= n_digits && (n_digits + self.dec_exponent) <= MAX_DECIMAL_DIGITS {
            //
            // In double-precision, this is an exact floating integer.
            // So we can compute to double, then shorten to float
            // with one round, and get the right answer.
            //
            // First, finish accumulating digits.
            // Then convert that integer to a double, multiply
            // by the appropriate power of ten, and convert to float.
            //
            let mut l_value = i_value as i64;
            for i in k_digits as usize..n_digits as usize {
                l_value = l_value * 10 + (digits[i] as i32 - b'0' as i32) as i64;
            }
            let mut d_value = l_value as f64;
            exp = self.dec_exponent - n_digits;
            d_value *= SMALL_10_POW[exp as usize];
            f_value = d_value as f32;
            return if self.is_negative { -f_value } else { f_value };
        }
        //
        // Harder cases:
        // The sum of digits plus exponent is greater than
        // what we think we can do with one error.
        //
        // Start by approximating the right answer by,
        // naively, scaling by powers of 10.
        // Scaling uses doubles to avoid overflow/underflow.
        //
        let mut d_value = f_value as f64;
        if exp > 0 {
            if self.dec_exponent > SINGLE_MAX_DECIMAL_EXPONENT + 1 {
                //
                // Lets face it. This is going to be
                // Infinity. Cut to the chase.
                //
                return if self.is_negative {
                    f32::NEG_INFINITY
                } else {
                    f32::INFINITY
                };
            }
            if (exp & 15) != 0 {
                d_value *= SMALL_10_POW[(exp & 15) as usize];
            }
            exp >>= 4;
            if exp != 0 {
                let mut j = 0usize;
                while exp > 0 {
                    if (exp & 1) != 0 {
                        d_value *= BIG_10_POW[j];
                    }
                    j += 1;
                    exp >>= 1;
                }
            }
        } else if exp < 0 {
            exp = -exp;
            if self.dec_exponent < SINGLE_MIN_DECIMAL_EXPONENT - 1 {
                //
                // Lets face it. This is going to be
                // zero. Cut to the chase.
                //
                return if self.is_negative { -0.0 } else { 0.0 };
            }
            if (exp & 15) != 0 {
                d_value /= SMALL_10_POW[(exp & 15) as usize];
            }
            exp >>= 4;
            if exp != 0 {
                let mut j = 0usize;
                while exp > 0 {
                    if (exp & 1) != 0 {
                        d_value *= TINY_10_POW[j];
                    }
                    j += 1;
                    exp >>= 1;
                }
            }
        }
        f_value = f32::max(FLOAT_MIN_VALUE, f32::min(FLOAT_MAX_VALUE, d_value as f32));

        //
        // fValue is now approximately the result.
        // The hard part is adjusting it, by comparison
        // with FDBigInteger arithmetic.
        // Formulate the EXACT big-number result as
        // bigD0 * 10^exp
        //
        if n_digits > SINGLE_MAX_NDIGITS {
            n_digits = SINGLE_MAX_NDIGITS + 1;
            digits[SINGLE_MAX_NDIGITS as usize] = b'1' as u16;
        }
        let mut big_d0 =
            FDBigInteger::new_with_digits(i_value as i64, &digits, k_digits as usize, n_digits as usize);
        exp = self.dec_exponent - n_digits;

        let mut ieee_bits = f_value.to_bits() as i32; // IEEE-754 bits of float candidate
        let b5 = i32::max(0, -exp); // powers of 5 in bigB, value is not modified inside correctionLoop
        let d5 = i32::max(0, exp); // powers of 5 in bigD, value is not modified inside correctionLoop
        big_d0 = big_d0.mult_by_pow52(d5, 0);
        big_d0.make_immutable(); // prevent bigD0 modification inside correctionLoop
        let mut big_d = FDBigInteger::zero();
        let mut big_d_valid = false;
        let mut prev_d2 = 0;
        'correction_loop: loop {
            // here ieeeBits can't be NaN, Infinity or zero
            let mut binexp = (ieee_bits as u32 >> SINGLE_EXP_SHIFT) as i32;
            let mut big_bbits = ieee_bits & F_SIGNIF_BIT_MASK;
            if binexp > 0 {
                big_bbits |= SINGLE_FRACT_HOB;
            } else {
                // Normalize denormalized numbers.
                assert_ne!(big_bbits, 0, "{big_bbits}"); // floatToBigInt(0.0)
                let leading_zeros = (big_bbits as u32).leading_zeros() as i32;
                let shift = leading_zeros - (31 - SINGLE_EXP_SHIFT);
                big_bbits = big_bbits.wrapping_shl(shift as u32);
                binexp = 1 - shift;
            }
            binexp -= F_EXP_BIAS;
            let low_order_zeros = (big_bbits as u32).trailing_zeros() as i32;
            big_bbits = (big_bbits as u32 >> low_order_zeros) as i32;
            let big_int_exp = binexp - SINGLE_EXP_SHIFT + low_order_zeros;
            let big_int_nbits = SINGLE_EXP_SHIFT + 1 - low_order_zeros;

            //
            // Scale bigD, bigB appropriately for
            // big-integer operations.
            // ...
            //
            let mut b2 = b5; // powers of 2 in bigB
            let mut d2 = d5; // powers of 2 in bigD
            let mut ulp2; // powers of 2 in halfUlp.
            if big_int_exp >= 0 {
                b2 += big_int_exp;
            } else {
                d2 -= big_int_exp;
            }
            ulp2 = b2;
            // shift bigB and bigD left by a number s. t.
            // halfUlp is still an integer.
            let hulpbias;
            if binexp <= -F_EXP_BIAS {
                // This is going to be a denormalized number
                // (if not actually zero).
                // half an ULP is at 2^-(EXP_BIAS+SINGLE_EXP_SHIFT+1)
                hulpbias = binexp + low_order_zeros + F_EXP_BIAS;
            } else {
                hulpbias = 1 + low_order_zeros;
            }
            b2 += hulpbias;
            d2 += hulpbias;
            // if there are common factors of 2, we might just as well
            // factor them out, as they add nothing useful.
            let common2 = i32::min(b2, i32::min(d2, ulp2));
            b2 -= common2;
            d2 -= common2;
            ulp2 -= common2;
            // do multiplications by powers of 5 and 2
            let big_b = FDBigInteger::value_of_mul_pow52(big_bbits as i64, b5, b2);
            if !big_d_valid || prev_d2 != d2 {
                big_d = big_d0.left_shift(d2);
                prev_d2 = d2;
                big_d_valid = true;
            }
            //
            // to recap:
            // ...
            //
            let mut diff;
            let mut cmp_result;
            let overvalue;
            cmp_result = big_b.cmp(&big_d);
            if cmp_result > 0 {
                overvalue = true; // our candidate is too big.
                diff = big_b.left_inplace_sub(&big_d); // bigB is not user further - reuse
                if big_int_nbits == 1 && big_int_exp > -F_EXP_BIAS + 1 {
                    // candidate is a normalized exact power of 2 and
                    // is too big (larger than Float.MIN_NORMAL). We will be subtracting.
                    // For our purposes, ulp is the ulp of the
                    // next smaller range.
                    ulp2 -= 1;
                    if ulp2 < 0 {
                        // rats. Cannot de-scale ulp this far.
                        // must scale diff in other direction.
                        ulp2 = 0;
                        diff = diff.left_shift(1);
                    }
                }
            } else if cmp_result < 0 {
                overvalue = false; // our candidate is too small.
                diff = big_d.right_inplace_sub(big_b); // bigB is not user further - reuse
            } else {
                // the candidate is exactly right!
                // this happens with surprising frequency
                break 'correction_loop;
            }
            cmp_result = diff.cmp_pow52(b5, ulp2);
            if cmp_result < 0 {
                // difference is small.
                // this is close enough
                break 'correction_loop;
            } else if cmp_result == 0 {
                // difference is exactly half an ULP
                // round to some other value maybe, then finish
                if (ieee_bits & 1) != 0 {
                    // half ties to even
                    ieee_bits += if overvalue { -1 } else { 1 }; // nextDown or nextUp
                }
                break 'correction_loop;
            } else {
                // difference is non-trivial.
                // could scale addend by ratio of difference to
                // halfUlp here, if we bothered to compute that difference.
                // Most of the time ( I hope ) it is about 1 anyway.
                ieee_bits += if overvalue { -1 } else { 1 }; // nextDown or nextUp
                if ieee_bits == 0 || ieee_bits == F_EXP_BIT_MASK {
                    // 0.0 or Float.POSITIVE_INFINITY
                    break 'correction_loop; // oops. Fell off end of range.
                }
                continue; // try again.
            }
        }
        if self.is_negative {
            ieee_bits |= F_SIGN_BIT_MASK;
        }
        f32::from_bits(ieee_bits as u32)
    }
}


enum ParseError {
    Message(String),
    IndexOutOfBounds,
}

fn char_at(units: &[u16], index: usize) -> Result<u16, ParseError> {
    units.get(index).copied().ok_or(ParseError::IndexOutOfBounds)
}

fn java_trim(units: &[u16]) -> &[u16] {
    let mut start = 0usize;
    let mut end = units.len();
    while start < end && units[start] <= ' ' as u16 {
        start += 1;
    }
    while end > start && units[end - 1] <= ' ' as u16 {
        end -= 1;
    }
    &units[start..end]
}

fn starts_with_at(units: &[u16], offset: usize, needle: &str) -> bool {
    let needle: Vec<u16> = needle.encode_utf16().collect();
    units.len() >= offset + needle.len() && units[offset..offset + needle.len()] == needle[..]
}

fn read_java_format_string_impl(
    input: &str,
) -> Result<Box<dyn ASCIIToBinaryConverter>, ParseError> {
    let mut is_negative = false;
    let mut sign_seen = false;
    let all_units: Vec<u16> = input.encode_utf16().collect();
    let in_units = java_trim(&all_units);
    let len = in_units.len();
    if len == 0 {
        return Err(ParseError::Message("empty String".to_string()));
    }
    let mut i = 0usize;
    match char_at(in_units, i)? {
        x if x == b'-' as u16 => {
            is_negative = true;
            i += 1;
            sign_seen = true;
        }
        x if x == b'+' as u16 => {
            i += 1;
            sign_seen = true;
        }
        _ => {}
    }
    let mut c = char_at(in_units, i)?;
    if c == b'N' as u16 {
        if (len - i) == NAN_LENGTH && starts_with_at(in_units, i, NAN_REP) {
            return Ok(Box::new(A2BC_NOT_A_NUMBER));
        }
        return Err(ParseError::IndexOutOfBounds);
    } else if c == b'I' as u16 {
        if (len - i) == INFINITY_LENGTH && starts_with_at(in_units, i, INFINITY_REP) {
            return Ok(if is_negative {
                Box::new(A2BC_NEGATIVE_INFINITY)
            } else {
                Box::new(A2BC_POSITIVE_INFINITY)
            });
        }
        return Err(ParseError::IndexOutOfBounds);
    } else if c == b'0' as u16 {
        if len > i + 1 {
            let ch = char_at(in_units, i + 1)?;
            if ch == b'x' as u16 || ch == b'X' as u16 {
                return parse_hex_string(input);
            }
        }
    }
    let mut digits: Vec<u16> = vec![0u16; len];
    let mut dec_seen = false;
    let mut n_digits = 0usize;
    let mut dec_pt = 0usize;
    let mut n_lead_zero = 0usize;
    let mut n_trail_zero = 0usize;
    'skip_leading_zeros_loop: while i < len {
        c = char_at(in_units, i)?;
        if c == b'0' as u16 {
            n_lead_zero += 1;
        } else if c == b'.' as u16 {
            if dec_seen {
                return Err(ParseError::Message("multiple points".to_string()));
            }
            dec_pt = i;
            if sign_seen {
                dec_pt -= 1;
            }
            dec_seen = true;
        } else {
            break 'skip_leading_zeros_loop;
        }
        i += 1;
    }
    'digit_loop: while i < len {
        c = char_at(in_units, i)?;
        if c >= b'1' as u16 && c <= b'9' as u16 {
            digits[n_digits] = c;
            n_digits += 1;
            n_trail_zero = 0;
        } else if c == b'0' as u16 {
            digits[n_digits] = c;
            n_digits += 1;
            n_trail_zero += 1;
        } else if c == b'.' as u16 {
            if dec_seen {
                return Err(ParseError::Message("multiple points".to_string()));
            }
            dec_pt = i;
            if sign_seen {
                dec_pt -= 1;
            }
            dec_seen = true;
        } else {
            break 'digit_loop;
        }
        i += 1;
    }
    n_digits -= n_trail_zero;
    let is_zero = n_digits == 0;
    if is_zero && n_lead_zero == 0 {
        return Err(ParseError::IndexOutOfBounds);
    }
    let mut dec_exp = if dec_seen {
        dec_pt as i32 - n_lead_zero as i32
    } else {
        n_digits as i32 + n_trail_zero as i32
    };
    if i < len {
        c = char_at(in_units, i)?;
    }
    if i < len && (c == b'e' as u16 || c == b'E' as u16) {
        let mut exp_sign = 1i32;
        let mut exp_val = 0i32;
        let really_big = i32::MAX / 10;
        let mut exp_overflow = false;
        i += 1;
        c = char_at(in_units, i)?;
        if c == b'-' as u16 {
            exp_sign = -1;
            i += 1;
        } else if c == b'+' as u16 {
            i += 1;
        }
        let exp_at = i;
        'exp_loop: while i < len {
            if exp_val >= really_big {
                exp_overflow = true;
            }
            c = char_at(in_units, i)?;
            i += 1;
            if c >= b'0' as u16 && c <= b'9' as u16 {
                exp_val = exp_val * 10 + (c as i32 - b'0' as i32);
            } else {
                i -= 1;
                break 'exp_loop;
            }
        }
        let exp_limit = BIG_DECIMAL_EXPONENT + n_digits as i32 + n_trail_zero as i32;
        if exp_overflow || exp_val > exp_limit {
            if !exp_overflow && exp_sign == 1 && dec_exp < 0 && (exp_val + dec_exp) < exp_limit {
                dec_exp += exp_val;
            } else {
                dec_exp = exp_sign * exp_limit;
            }
        } else {
            dec_exp = dec_exp + exp_sign * exp_val;
        }
        if i == exp_at {
            return Err(ParseError::IndexOutOfBounds);
        }
    }
    if i < len {
        let last = char_at(in_units, i)?;
        if (i != len - 1)
            || (last != b'f' as u16
                && last != b'F' as u16
                && last != b'd' as u16
                && last != b'D' as u16)
        {
            return Err(ParseError::IndexOutOfBounds);
        }
    }
    if is_zero {
        return Ok(if is_negative {
            Box::new(A2BC_NEGATIVE_ZERO)
        } else {
            Box::new(A2BC_POSITIVE_ZERO)
        });
    }
    Ok(Box::new(ASCIIToBinaryBuffer::new(
        is_negative,
        dec_exp,
        digits,
        n_digits as i32,
    )))
}
/// Extracts the groups of the upstream HexFloatPattern.VALUE regex
/// ([+-])?0[xX](((\\p{XDigit}+)\\.?)|((\\p{XDigit}*)\\.(\\p{XDigit}+)))[pP]([-+])?(\\p{Digit}+)[fFdD]?
/// as (isNegative, group4, group6, group7, group8Positive, group9).
fn match_hex_float(s: &str) -> Option<(bool, Option<String>, String, String, bool, String)> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    let mut is_negative = false;
    if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
        is_negative = chars[i] == '-';
        i += 1;
    }
    if i + 1 >= chars.len() || chars[i] != '0' || (chars[i + 1] != 'x' && chars[i + 1] != 'X') {
        return None;
    }
    i += 2;
    let int_start = i;
    while i < chars.len() && chars[i].is_ascii_hexdigit() {
        i += 1;
    }
    let integer: String = chars[int_start..i].iter().collect();
    let mut saw_dot = false;
    let mut fraction = String::new();
    if i < chars.len() && chars[i] == '.' {
        saw_dot = true;
        i += 1;
        let frac_start = i;
        while i < chars.len() && chars[i].is_ascii_hexdigit() {
            i += 1;
        }
        fraction = chars[frac_start..i].iter().collect();
    }
    let group4;
    let group6;
    let group7;
    if saw_dot && !fraction.is_empty() {
        group4 = None;
        group6 = integer;
        group7 = fraction;
    } else {
        if integer.is_empty() {
            return None;
        }
        group4 = Some(integer);
        group6 = String::new();
        group7 = String::new();
    }
    if i >= chars.len() || (chars[i] != 'p' && chars[i] != 'P') {
        return None;
    }
    i += 1;
    let mut positive_exponent = true;
    if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
        positive_exponent = chars[i] == '+';
        i += 1;
    }
    let exp_start = i;
    while i < chars.len() && chars[i].is_numeric() {
        i += 1;
    }
    if i == exp_start {
        return None;
    }
    let exponent: String = chars[exp_start..i].iter().collect();
    if i < chars.len() && matches!(chars[i], 'f' | 'F' | 'd' | 'D') {
        i += 1;
    }
    if i != chars.len() {
        return None;
    }
    Some((
        is_negative,
        group4,
        group6,
        group7,
        positive_exponent,
        exponent,
    ))
}
#[allow(unused_assignments)] // upstream assigns round=false before recomputing it 1:1
fn parse_hex_string(input: &str) -> Result<Box<dyn ASCIIToBinaryConverter>, ParseError> {
    let (is_negative, group4, group6, group7, positive_exponent, exponent_digits) =
        match match_hex_float(input) {
            Some(fields) => fields,
            None => return Err(ParseError::IndexOutOfBounds),
        };
    let significand_string: String;
    let signif_length: usize;
    let exponent_adjust: i32;
    {
        let left_digits: usize;
        let right_digits: usize;
        if let Some(group4) = group4 {
            significand_string = strip_leading_zeros(&group4).to_string();
            left_digits = significand_string.chars().count();
            right_digits = 0;
        } else {
            let group6 = strip_leading_zeros(&group6).to_string();
            left_digits = group6.chars().count();
            right_digits = group7.chars().count();
            significand_string = format!("{group6}{group7}");
        }
        let significand_string = strip_leading_zeros(&significand_string).to_string();
        signif_length = significand_string.chars().count();
        if left_digits >= 1 {
            exponent_adjust = 4 * (left_digits as i32 - 1);
        } else {
            exponent_adjust = -4 * (right_digits as i32 - signif_length as i32 + 1);
        }
        if signif_length == 0 {
            return Ok(if is_negative {
                Box::new(A2BC_NEGATIVE_ZERO)
            } else {
                Box::new(A2BC_POSITIVE_ZERO)
            });
        }
        let _ = &significand_string;
    }
    let significand_string = strip_leading_zeros(&significand_string).to_string();
    let unsigned_raw_exponent: i64 = match exponent_digits.parse::<i32>() {
        Ok(value) => value as i64,
        Err(_) => {
            return Ok(if is_negative {
                if positive_exponent {
                    Box::new(A2BC_NEGATIVE_INFINITY)
                } else {
                    Box::new(A2BC_NEGATIVE_ZERO)
                }
            } else if positive_exponent {
                Box::new(A2BC_POSITIVE_INFINITY)
            } else {
                Box::new(A2BC_POSITIVE_ZERO)
            });
        }
    };
    let raw_exponent = if positive_exponent {
        1i64
    } else {
        -1i64
    } * unsigned_raw_exponent;
    let mut exponent: i64 = raw_exponent + exponent_adjust as i64;
    let mut round = false;
    let mut sticky = false;
    let mut significand: i64 = 0;
    let leading_digit = get_hex_digit(&significand_string, 0) as i64;
    let mut next_shift: i32;
    if leading_digit == 1 {
        significand |= leading_digit << 52;
        next_shift = 52 - 4;
    } else if leading_digit <= 3 {
        significand |= leading_digit << 51;
        next_shift = 52 - 5;
        exponent += 1;
    } else if leading_digit <= 7 {
        significand |= leading_digit << 50;
        next_shift = 52 - 6;
        exponent += 2;
    } else if leading_digit <= 15 {
        significand |= leading_digit << 49;
        next_shift = 52 - 7;
        exponent += 3;
    } else {
        panic!("java.lang.AssertionError: Result from digit conversion too large!");
    }
    let mut i = 1usize;
    while i < signif_length && next_shift >= 0 {
        let current_digit = get_hex_digit(&significand_string, i) as i64;
        significand |= current_digit << next_shift;
        next_shift -= 4;
        i += 1;
    }
    if i < signif_length {
        let mut current_digit = get_hex_digit(&significand_string, i) as i64;
        match next_shift {
            -1 => {
                significand |= (current_digit & 0xE) >> 1;
                round = (current_digit & 0x1) != 0;
            }
            -2 => {
                significand |= (current_digit & 0xC) >> 2;
                round = (current_digit & 0x2) != 0;
                sticky = (current_digit & 0x1) != 0;
            }
            -3 => {
                significand |= (current_digit & 0x8) >> 3;
                round = (current_digit & 0x4) != 0;
                sticky = (current_digit & 0x3) != 0;
            }
            -4 => {
                round = (current_digit & 0x8) != 0;
                sticky = (current_digit & 0x7) != 0;
            }
            _ => {
                panic!("java.lang.AssertionError: Unexpected shift distance remainder.");
            }
        }
        i += 1;
        while i < signif_length && !sticky {
            current_digit = get_hex_digit(&significand_string, i) as i64;
            sticky = sticky || current_digit != 0;
            i += 1;
        }
    }
    let mut float_bits: i32 = if is_negative { F_SIGN_BIT_MASK } else { 0 };
    if exponent >= FLOAT_MIN_EXPONENT as i64 {
        if exponent > FLOAT_MAX_EXPONENT as i64 {
            float_bits |= F_EXP_BIT_MASK;
        } else {
            let thresh_shift = SIGNIFICAND_WIDTH - F_SIGNIFICAND_WIDTH - 1;
            let float_sticky =
                (significand & ((1i64 << thresh_shift) - 1)) != 0 || round || sticky;
            let mut i_value = (significand as u64 >> thresh_shift as u32) as i32;
            if (i_value & 3) != 1 || float_sticky {
                i_value += 1;
            }
            float_bits |= (((exponent as i32) + (F_EXP_BIAS - 1)) << SINGLE_EXP_SHIFT)
                + (i_value >> 1);
        }
    } else if exponent >= (F_MIN_SUB_EXPONENT - 1) as i64 {
        let thresh_shift =
            ((SIGNIFICAND_WIDTH - 2 + F_MIN_SUB_EXPONENT) as i64 - exponent) as i32;
        assert!(thresh_shift >= SIGNIFICAND_WIDTH - F_SIGNIFICAND_WIDTH);
        assert!(thresh_shift < SIGNIFICAND_WIDTH);
        let float_sticky = (significand & ((1i64 << thresh_shift) - 1)) != 0 || round || sticky;
        let mut i_value = (significand as u64 >> thresh_shift as u32) as i32;
        if (i_value & 3) != 1 || float_sticky {
            i_value += 1;
        }
        float_bits |= i_value >> 1;
    }
    let f_value = f32::from_bits(float_bits as u32);
    if exponent > DOUBLE_MAX_EXPONENT as i64 {
        return Ok(if is_negative {
            Box::new(A2BC_NEGATIVE_INFINITY)
        } else {
            Box::new(A2BC_POSITIVE_INFINITY)
        });
    }
    if exponent <= DOUBLE_MAX_EXPONENT as i64 && exponent >= DOUBLE_MIN_EXPONENT as i64 {
        significand = (((exponent + EXP_BIAS as i64) << (SIGNIFICAND_WIDTH - 1)) & EXP_BIT_MASK)
            | (SIGNIF_BIT_MASK & significand);
    } else {
        if exponent < (MIN_SUB_EXPONENT - 1) as i64 {
            return Ok(if is_negative {
                Box::new(A2BC_NEGATIVE_ZERO)
            } else {
                Box::new(A2BC_POSITIVE_ZERO)
            });
        }
        let mut round = round;
        sticky = sticky || round;
        round = false;
        let bits_discarded = 53 - ((exponent as i32) - MIN_SUB_EXPONENT + 1);
        assert!((1..=53).contains(&bits_discarded));
        round = (significand & (1i64 << (bits_discarded - 1))) != 0;
        if bits_discarded > 1 {
            let mask = !((!0i64) << (bits_discarded - 1));
            sticky = sticky || ((significand & mask) != 0);
        }
        significand = significand >> bits_discarded;
        significand = (((((DOUBLE_MIN_EXPONENT - 1) as i64) + EXP_BIAS as i64)
            << (SIGNIFICAND_WIDTH - 1))
            & EXP_BIT_MASK)
            | (SIGNIF_BIT_MASK & significand);
        return finish_hex_parse(is_negative, significand, round, sticky, f_value);
    }
    let least_zero = (significand & 1) == 0;
    if (least_zero && round && sticky) || ((!least_zero) && round) {
        significand += 1;
    }
    let value = if is_negative {
        f64::from_bits((significand | SIGN_BIT_MASK) as u64)
    } else {
        f64::from_bits(significand as u64)
    };
    Ok(Box::new(PreparedASCIIToBinaryBuffer {
        double_val: value,
        float_val: f_value,
    }))
}

fn finish_hex_parse(
    is_negative: bool,
    significand: i64,
    round: bool,
    sticky: bool,
    f_value: f32,
) -> Result<Box<dyn ASCIIToBinaryConverter>, ParseError> {
    let least_zero = (significand & 1) == 0;
    let mut significand = significand;
    if (least_zero && round && sticky) || ((!least_zero) && round) {
        significand += 1;
    }
    let value = if is_negative {
        f64::from_bits((significand | SIGN_BIT_MASK) as u64)
    } else {
        f64::from_bits(significand as u64)
    };
    Ok(Box::new(PreparedASCIIToBinaryBuffer {
        double_val: value,
        float_val: f_value,
    }))
}

fn strip_leading_zeros(s: &str) -> &str {
    if !s.is_empty() && s.starts_with('0') {
        for (i, ch) in s.char_indices() {
            if i > 0 && ch != '0' {
                return &s[i..];
            }
        }
        return "";
    }
    s
}

fn get_hex_digit(s: &str, position: usize) -> i32 {
    let ch = s.chars().nth(position).unwrap_or_else(|| {
        panic!("java.lang.AssertionError: Unexpected failure of digit conversion")
    });
    let value = ch.to_digit(16).map(|v| v as i32).unwrap_or(-1);
    if value <= -1 || value >= 16 {
        panic!("java.lang.AssertionError: Unexpected failure of digit conversion of {ch}");
    }
    value
}
