//! Minimal x87 `long double` (80-bit float) emulator.
//!
//! Borland Turbo C++ 3.0 with `-f287` performs *all* floating-point
//! arithmetic via the 80x87 FPU, which keeps a 64-bit mantissa (plus
//! explicit integer bit) and rounds to nearest-even. This is strictly more
//! precise than IEEE `f64` (53-bit mantissa), and for the Noctis seed
//! expression `x/100000*y/100000*z/100000` the extra precision
//! flips the integer truncation of `srand` for large coordinates, which
//! changes the whole RNG sequence. To reproduce the reference bit-for-bit
//! we evaluate that expression (and `nearstar_identity`, the same
//! expression) in 80-bit precision.
//!
//! Layout mirrors the x87 extended format: sign (1) + exponent (15,
//! bias 16383) + 64-bit mantissa with an explicit integer bit.

#[derive(Clone, Copy, Debug)]
pub struct F80 {
    neg: bool,
    /// Biased exponent. 0 = zero/subnormal, 0x7FFF = special.
    exp: i32,
    /// 64-bit mantissa (integer bit explicit at bit 63).
    mant: u64,
}

const BIAS: i32 = 16383;

impl F80 {
    /// Build from an IEEE `f64`. Mirrors the x87 `fld`/`fstp`
    /// conversion (round to nearest-even).
    pub fn from_f64(x: f64) -> F80 {
        let bits = x.to_bits();
        let neg = (bits >> 63) != 0;
        let exp = ((bits >> 52) & 0x7FF) as i32;
        let frac = bits & 0xF_FFFF_FFFF_FFFF;
        if exp == 0 {
            if frac == 0 {
                return F80 { neg, exp: 0, mant: 0 };
            }
            // subnormal f64 -> normalise
            let mut f = frac;
            let mut e = -1022;
            while (f & (1 << 52)) == 0 {
                f <<= 1;
                e -= 1;
            }
            let mant = (f << 11) | (1u64 << 63);
            F80 { neg, exp: e + BIAS, mant }
        } else if exp == 0x7FF {
            F80 { neg, exp: 0x7FFF, mant: if frac == 0 { 1u64 << 63 } else { frac << 11 | (1u64 << 63) } }
        } else {
            // normal: integer bit set, mantissa = 1.frac << 11 (top bit = int bit)
            let mant = (1u64 << 63) | (frac << 11);
            F80 { neg, exp: exp - 1023 + BIAS, mant }
        }
    }

    /// Convert to `f64` (round to nearest-even), matching the x87 store
    /// of an 80-bit register into a 64-bit memory slot.
    pub fn to_f64(&self) -> f64 {
        if self.exp == 0 && self.mant == 0 {
            return f64::from_bits(if self.neg { 1u64 << 63 } else { 0 });
        }
        if self.exp == 0x7FFF {
            // infinity / nan
            let payload = self.mant & !(1u64 << 63);
            let bits = ((self.neg as u64) << 63)
                | (0x7FFu64 << 52)
                | (payload >> 11);
            return f64::from_bits(bits);
        }
        // value = mant * 2^(exp-BIAS), mantissa already in [1,2) with the
        // integer bit explicit at bit 63. The true binary exponent is just
        // (exp - BIAS); the integer bit being at bit 63 is purely storage.
        let unbiased = self.exp - BIAS;
        // Normalise mantissa so the top bit is the integer bit.
        let mant = self.mant;
        // Round to 52-bit fraction (53-bit including implicit bit),
        // nearest-even, then encode.
        // Shift mantissa so its MSB is bit 52 of the resulting f64 mantissa.
        // We need to produce (sign, e, 52-bit frac).
        // Target: keep 53 significant bits (implicit + 52).
        let total_bits = 64i32; // mant has 64 bits, top is integer bit
        let shift = total_bits - 53; // = 11: keep top 53 bits
        let keep_mask = !((1u64 << shift) - 1);
        let kept = mant & keep_mask;
        let round_bit = (mant >> (shift - 1)) & 1;
        let sticky = if shift >= 2 { (mant & ((1u64 << (shift - 1)) - 1)) != 0 } else { false };
        let mut rounded = kept >> shift; // 53-bit value (top bit = 1)
        // round half to even
        if round_bit == 1 && (sticky || (rounded & 1) == 1) {
            rounded += 1;
        }
        // rounded is up to 54 bits if it overflowed
        let mut e = unbiased;
        if (rounded >> 53) & 1 == 1 {
            // overflow from rounding -> shift down
            rounded >>= 1;
            e += 1;
        }
        // now rounded has implicit integer bit at bit 52
        if e < -1022 {
            // subnormal or underflow to zero
            if e < -1022 - 52 {
                return f64::from_bits(if self.neg { 1u64 << 63 } else { 0 });
            }
            let sub = (-1022 - e) as u32;
            let frac = (rounded >> (sub + 1)) | (((rounded >> sub) & 1) << 52);
            let bits = ((self.neg as u64) << 63) | (frac & 0xF_FFFF_FFFF_FFFF);
            return f64::from_bits(bits);
        }
        if e > 1023 {
            // overflow to infinity
            let bits = ((self.neg as u64) << 63) | (0x7FFu64 << 52);
            return f64::from_bits(bits);
        }
        let frac = rounded & !(1u64 << 52);
        let bits = ((self.neg as u64) << 63) | (((e + 1023) as u64) << 52) | frac;
        f64::from_bits(bits)
    }

    /// Convert to `f32` (round to nearest-even), matching the x87 store
    /// of an 80-bit register directly into a 32-bit memory slot (no f64
    /// intermediate).
    pub fn to_f32(&self) -> f32 {
        if self.exp == 0 && self.mant == 0 {
            return f32::from_bits(if self.neg { 1u32 << 31 } else { 0 });
        }
        if self.exp == 0x7FFF {
            let payload = self.mant & !(1u64 << 63);
            let bits = ((self.neg as u32) << 31)
                | (0xFFu32 << 23)
                | ((payload >> (64 - 23)) as u32 & 0x7F_FFFF);
            return f32::from_bits(bits);
        }
        let unbiased = self.exp - BIAS;
        let mant = self.mant;
        // Round 64-bit mantissa to 24-bit (23 + implicit): keep top 24 bits.
        let total_bits = 64i32;
        let shift = total_bits - 24; // = 40: keep top 24 bits
        let keep_mask = !((1u64 << shift) - 1);
        let kept = mant & keep_mask;
        let round_bit = (mant >> (shift - 1)) & 1;
        let sticky = if shift >= 2 { (mant & ((1u64 << (shift - 1)) - 1)) != 0 } else { false };
        let mut rounded = (kept >> shift) as u32; // 24-bit value (top bit = 1)
        if round_bit == 1 && (sticky || (rounded & 1) == 1) {
            rounded = rounded.wrapping_add(1);
        }
        let mut e = unbiased;
        if (rounded >> 24) & 1 == 1 {
            rounded >>= 1;
            e += 1;
        }
        if e < -126 {
            if e < -126 - 23 {
                return f32::from_bits(if self.neg { 1u32 << 31 } else { 0 });
            }
            let sub = (-126 - e) as u32;
            let frac = (rounded >> (sub + 1)) | (((rounded >> sub) & 1) << 23);
            let bits = ((self.neg as u32) << 31) | (frac & 0x7F_FFFF);
            return f32::from_bits(bits);
        }
        if e > 127 {
            let bits = ((self.neg as u32) << 31) | (0xFFu32 << 23);
            return f32::from_bits(bits);
        }
        let frac = rounded & !(1u32 << 23);
        let bits = ((self.neg as u32) << 31) | (((e + 127) as u32) << 23) | frac;
        f32::from_bits(bits)
    }

    /// x87 `FTST` comparison with +0.0: returns true if the 80-bit value
    /// is strictly greater than zero. Must match the x87 condition flags
    /// exactly — rounding to f64 first flips the sign for values < 2^-1022.
    pub fn is_positive(&self) -> bool {
        if self.neg {
            return false;
        }
        // zero (exp=0, mant=0) is not positive
        if self.exp == 0 && self.mant == 0 {
            return false;
        }
        true
    }

    fn zero(neg: bool) -> F80 {
        F80 { neg, exp: 0, mant: 0 }
    }

    /// Multiply, rounding to nearest-even (x87 semantics). The full 128-bit
    /// product of the two 64-bit mantissas is rounded to 64 bits.
    pub fn mul(&self, other: &F80) -> F80 {
        if (self.exp == 0 && self.mant == 0) || (other.exp == 0 && other.mant == 0) {
            return F80::zero(self.neg ^ other.neg);
        }
        let neg = self.neg ^ other.neg;
        let a = self.mant as u128;
        let b = other.mant as u128;
        let p = a * b; // exact 128-bit product
        // Round p / 2^64 to nearest-even -> 64-bit mantissa.
        let dropped = p & 0xFFFF_FFFF_FFFF_FFFF;
        let mut m = (p >> 64) as u64;
        let halfway = dropped == 0x8000_0000_0000_0000;
        if dropped > 0x8000_0000_0000_0000 || (halfway && (m & 1) == 1) {
            m = m.wrapping_add(1);
        }
        // Normalise `m` so its integer bit sits at bit 63.
        let (mant, exp) = if (m >> 63) & 1 == 1 {
            (m, self.exp + other.exp - BIAS + 1)
        } else {
            (m << 1, self.exp + other.exp - BIAS)
        };
        F80 { neg, exp, mant }
    }

    /// Add/subtract (sub when `negate`).
    pub fn add(&self, other: &F80, negate: bool) -> F80 {
        let ob = F80 { neg: other.neg ^ negate, exp: other.exp, mant: other.mant };
        if self.exp == 0 && self.mant == 0 {
            return ob;
        }
        if ob.exp == 0 && ob.mant == 0 {
            return *self;
        }
        // Align smaller exponent to larger.
        let (a, b, ea, eb);
        if self.exp >= ob.exp {
            a = self.mant; ea = self.exp;
            let diff = self.exp - ob.exp;
            b = if diff >= 64 { 0 } else { ob.mant >> diff };
            eb = ob.exp;
        } else {
            b = ob.mant; eb = ob.exp;
            let diff = ob.exp - self.exp;
            a = if diff >= 64 { 0 } else { self.mant >> diff };
            ea = ob.exp; // after shifting a right, its effective exponent is the larger one
        }
        let (res_neg, res_mant, res_exp);
        if self.neg == ob.neg {
            let s = a as u128 + b as u128;
            // s may have a carry into bit 64 (or beyond).
            let (m, carry) = if (s >> 63) != 0 {
                // integer bit at bit 63 or higher — already (or over-) normalised.
                if (s >> 64) & 1 == 1 {
                    // two integer bits: shift right and bump exponent
                    ((s >> 1) as u64, ea + 1)
                } else {
                    (s as u64, ea)
                }
            } else {
                // shift left to restore integer bit
                let mut m = s << 1;
                let mut e = ea - 1;
                while (m >> 63) & 1 == 0 && m != 0 {
                    m <<= 1;
                    e -= 1;
                }
                (m as u64, e)
            };
            res_neg = self.neg;
            res_mant = m;
            res_exp = carry;
        } else {
            // subtract
            let (big, small, big_neg) = if (self.neg && a >= b) || (!self.neg && a >= b) {
                (a, b, self.neg)
            } else {
                (b, a, ob.neg)
            };
            let mut d = big - small;
            let mut e = ea.max(eb);
            while (d >> 63) & 1 == 0 && d != 0 {
                d <<= 1;
                e -= 1;
            }
            if d == 0 {
                return F80::zero(false);
            }
            res_neg = big_neg;
            res_mant = d;
            res_exp = e;
        }
        // clamp exponent
        let exp = if res_exp < 0 { 0 } else { res_exp };
        F80 { neg: res_neg, exp, mant: res_mant }
    }

    /// Divide, rounding to nearest-even. Uses restoring long division on
    /// the 64-bit mantissas.
    pub fn div(&self, other: &F80) -> F80 {
        if other.exp == 0 && other.mant == 0 {
            // divide by zero -> infinity
            return F80 { neg: self.neg ^ other.neg, exp: 0x7FFF, mant: 1u64 << 63 };
        }
        if self.exp == 0 && self.mant == 0 {
            return F80::zero(self.neg ^ other.neg);
        }
        let neg = self.neg ^ other.neg;
        let n = self.mant as u128;
        let d = other.mant as u128;
        // q = floor((n*2^64)/d) gives the quotient scaled by 2^64.
        // Remainder lets us round to nearest-even (x87 semantics).
        let num = n << 64;
        let q = num / d; // floor, in [2^64, 2^66)
        let rem = num % d;
        let twice = rem << 1;
        let mut q = q;
        if twice > d || (twice == d && (q & 1) == 1) {
            q += 1;
        }
        // q now rounded to nearest-even, scaled by 2^64. Put integer bit at bit 63.
        let mut mant = (q >> 1) as u64; // now in [2^63, 2^64): integer bit at bit 63.
        let mut exp = self.exp - other.exp + BIAS;
        // normalise
        if (mant >> 63) & 1 == 1 {
            // good, integer bit present
        } else {
            mant <<= 1;
            exp -= 1;
        }
        let exp = if exp < 0 { 0 } else { exp };
        F80 { neg, exp, mant }
    }

    /// Truncate toward zero to `i32` (matches `(long) double` cast).
    pub fn to_i32_trunc(&self) -> i32 {
        if self.exp == 0 && self.mant == 0 {
            return 0;
        }
        // value = mant * 2^(exp-BIAS), mantissa in [1,2) with integer bit
        // at bit 63. Equivalent integer: (mant) * 2^((exp-BIAS) - 63).
        let unbiased = (self.exp - BIAS) - 63;
        let mant = self.mant;
        let val: i64 = if unbiased >= 0 {
            // integer (no fractional part relevant); shift left
            if unbiased > 63 {
                return if self.neg { i32::MIN } else { i32::MAX };
            }
            (mant << unbiased) as i64
        } else {
            // fractional: shift right, truncating
            let s = (-unbiased) as u32;
            if s >= 64 {
                return 0;
            }
            (mant >> s) as i64
        };
        let v = val as i32;
        if self.neg {
            -v
        } else {
            v
        }
    }

    /// Round to nearest, ties to even, to `i32`. Mirrors the x87 `fistp` /
    /// `fist` store-with-rounding behaviour under the default rounding mode
    /// (RC = round-to-nearest-even). This is what `fild`/`fmul`/`fistp`
    /// sequences in the generator actually do — NOT a truncation.
    pub fn to_i32_rne(&self) -> i32 {
        if self.exp == 0 && self.mant == 0 {
            return 0;
        }
        let unbiased = (self.exp - BIAS) - 63;
        // value = mant * 2^unbiased, mant in [2^63, 2^64).
        //   unbiased <= -65  -> value < 0.25  -> rounds to 0
        //   unbiased == -64  -> value in [0.5, 1.0)
        //   unbiased == -63  -> value in [1.0, 2.0)
        if unbiased < -64 {
            return 0; // |value| < 0.5 -> rounds to 0
        }
        if unbiased > 30 {
            // |value| >= 2^31, out of i32 range
            return if self.neg { i32::MIN } else { i32::MAX };
        }
        // s = number of fractional bits (bits below the units place).
        let s: u32 = if unbiased >= 0 {
            0
        } else if unbiased < -64 {
            64 // all fractional (handled by the early return above, but keep safe)
        } else {
            (-unbiased) as u32 // 1..64
        };
        // Integer part (mantissa shifted so the units bit is at bit 0).
        let int_part: i64 = if s == 0 {
            self.mant as i64
        } else if s >= 64 {
            0
        } else {
            (self.mant >> s) as i64
        };
        // Top fractional bit (the "half" bit) and any bits strictly below it.
        // s ranges 1..64; bit index (s-1) is always a valid 0..63 shift.
        let half_bit = if s == 0 {
            false
        } else {
            (self.mant >> (s - 1)) & 1 == 1
        };
        let lower_bits = if s <= 1 {
            0u64
        } else {
            self.mant & ((1u64 << (s - 1)) - 1)
        };
        let exactly_half = half_bit && lower_bits == 0;
        let above_half = half_bit && lower_bits != 0;

        let mut result = int_part;
        if above_half || (exactly_half && (int_part & 1) == 1) {
            result += 1;
        }
        let v = result as i32;
        if self.neg {
            -v
        } else {
            v
        }
    }

    /// Build from an i64 (exact for |value| < 2^63).
    /// Normalised so the integer bit sits at bit 63.
    pub fn from_i64(x: i64) -> F80 {
        let neg = x < 0;
        let abs = x.unsigned_abs();
        if abs == 0 {
            return F80 { neg: false, exp: 0, mant: 0 };
        }
        let leading = 63 - abs.leading_zeros();
        let mant = abs << (63 - leading);
        let exp = BIAS + leading as i32;
        F80 { neg, exp, mant }
    }

    /// Subtract by adding the negated operand.
    pub fn sub(&self, other: &F80) -> F80 {
        self.add(other, true)
    }

    /// Truncate toward zero to i64 (Borland `(long)(double)` behavior).
    pub fn trunc_i64(&self) -> i64 {
        if self.exp < BIAS {
            return 0i64;
        }
        let shift = 64 - (self.exp - BIAS) as u32 - 1;
        if shift >= 64 { return 0i64; }
        let int_bits = self.mant >> shift;
        if self.neg { -(int_bits as i64) } else { int_bits as i64 }
    }

    /// Round to nearest, ties away from zero. Add 0.5 and truncate.
    fn round_half_away(&self) -> F80 {
        // Represent 0.5 as an F80 (exponent bias - 1, mantissa = 1<<63)
        let half = F80 { neg: false, exp: BIAS - 1, mant: 1u64 << 63 };
        let added = if self.neg {
            self.add(&half, true)  // subtract 0.5 for negative
        } else {
            self.add(&half, false)  // add 0.5 for positive
        };
        // Truncate toward zero (F80 truncation: clear low mantissa bits after the integer)
        if added.exp < BIAS {
            return F80::from_f64(0.0);
        }
        let shift = 64 - (added.exp - BIAS) as u32 - 1;
        if shift >= 64 { return *self; }
        let masked = added.mant & !((1u64 << shift) - 1);
        F80 { mant: masked, ..added }
    }

    /// Negation (flip sign bit).
    pub fn neg(&self) -> F80 {
        F80 { neg: !self.neg, exp: self.exp, mant: self.mant }
    }

    /// Absolute value.
    pub fn abs(&self) -> F80 {
        F80 { neg: false, exp: self.exp, mant: self.mant }
    }

    /// Full ordering (matches x87 `fcomi` semantics).
    pub fn cmp(&self, other: &F80) -> std::cmp::Ordering {
        use std::cmp::Ordering::{Equal, Greater, Less};
        // Handle zero
        let self_zero = self.exp == 0 && self.mant == 0;
        let other_zero = other.exp == 0 && other.mant == 0;
        if self_zero && other_zero { return Equal; }
        if self_zero { return if other.neg { Greater } else { Less }; }
        if other_zero { return if self.neg { Less } else { Greater }; }
        // Negative < positive
        if self.neg && !other.neg { return Less; }
        if !self.neg && other.neg { return Greater; }
        // Same sign: compare (exponent, mantissa)
        if self.neg {
            // Both negative: larger exponent/mantissa = smaller value
            if self.exp != other.exp { return other.exp.cmp(&self.exp); }
            other.mant.cmp(&self.mant)
        } else {
            if self.exp != other.exp { return self.exp.cmp(&other.exp); }
            self.mant.cmp(&other.mant)
        }
    }

    /// x87 π constant (80-bit extended precision).
    pub fn pi() -> F80 {
        F80 { neg: false, exp: BIAS + 1, mant: 0xC90FDA_A22168C235 }
    }

    /// π/2
    pub fn pi_2() -> F80 {
        let half = F80 { neg: false, exp: BIAS - 1, mant: 1u64 << 63 };
        F80::pi().mul(&half)
    }

    /// 2π
    pub fn two_pi() -> F80 {
        let two = F80 { neg: false, exp: BIAS + 1, mant: 1u64 << 63 };
        F80::pi().mul(&two)
    }

    /// Square root via Newton's method: y_{n+1} = (y_n + x/y_n)/2.
    pub fn sqrt(&self) -> F80 {
        if self.exp == 0 && self.mant == 0 { return F80::zero(self.neg); }
        if self.neg { return F80 { neg: true, exp: 0x7FFF, mant: 1u64 << 63 }; } // -0 -> -inf
        // Initial guess from f64
        let mut y = F80::from_f64(self.to_f64().sqrt());
        // Newton iteration (6 iterations for 64-bit convergence)
        let half = F80 { neg: false, exp: BIAS - 1, mant: 1u64 << 63 };
        for _ in 0..6 {
            let inv = self.div(&y);
            let sum = y.add(&inv, false);
            y = sum.mul(&half);
        }
        y
    }

    /// Cosine via Taylor series with range reduction to [0, π/2].
    pub fn cos(&self) -> F80 {
        let _zero = F80::zero(false);
        let one = F80 { neg: false, exp: BIAS, mant: 1u64 << 63 };
        let _half = F80 { neg: false, exp: BIAS - 1, mant: 1u64 << 63 };
        let mut x = self.abs();
        // Reduce modulo 2π
        let two_pi = F80::two_pi();
        while x.cmp(&two_pi) != std::cmp::Ordering::Less {
            x = x.add(&two_pi, true); // x - 2π
        }
        let pi = F80::pi();
        // If x > π: cos(x) = cos(2π - x)
        if x.cmp(&pi) == std::cmp::Ordering::Greater {
            x = F80::two_pi().add(&x, true); // 2π - x
        }
        let pi_2 = F80::pi_2();
        let mut negate = false;
        // If x > π/2: cos(x) = -cos(π - x)
        if x.cmp(&pi_2) == std::cmp::Ordering::Greater {
            x = pi.add(&x, true); // π - x
            negate = !negate;
        }
        // Now x ∈ [0, π/2]; compute cos via Taylor series
        // cos(x) = Σ_{n=0}^{N} (-1)^n * x^{2n} / (2n)!
        // 20 terms gives 64-bit precision for x ∈ [0, π/2]
        let x2 = x.mul(&x);
        let neg_x2 = x2.mul(&one.neg()); // -x^2
        let mut term = one.clone();  // n=0: x^0/0! = 1
        let mut result = term.clone();
        // Pre-compute inverse factorials for speed: 1/(2n*(2n-1)) for n=1..20
        // but since F80 division is fast, just divide each iteration.
        for n in 1..=20 {
            // term = term * (-x^2) / (2n*(2n-1))
            let d1 = F80::from_i64(2 * n - 1);
            let d2 = F80::from_i64(2 * n);
            term = term.mul(&neg_x2).div(&d1).div(&d2);
            result = result.add(&term, false);
        }
        if negate { result.neg() } else { result }
    }

    /// Format to 6 decimal places using F80 arithmetic (matches C printf "%.6f").
    /// No f64 conversion — the 80-bit value is rounded and decomposed directly.
    pub fn format_6f(&self) -> String {
        let one_million = F80::from_f64(1_000_000.0);
        // Integer part (truncated toward zero)
        let int_part = self.trunc_i64();
        // Fractional part: 6 decimal digits
        let int_f80 = F80::from_i64(int_part);
        let frac_scaled = self.sub(&int_f80).mul(&one_million);
        let frac_rounded = frac_scaled.round_half_away();
        let frac_int = frac_rounded.trunc_i64().unsigned_abs();
        let sign = if self.neg && (int_part != 0 || frac_int != 0) { "-" } else { "" };
        if self.neg && int_part == 0 && frac_int == 0 {
            format!("-0.{:06}", 0u64)
        } else {
            format!("{}{}.{:06}", sign, int_part.unsigned_abs(), frac_int.min(999999))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::F80;

    #[test]
    fn f64_roundtrip() {
        for &v in &[3.812345_f64, -42.87654, 4.81093, 3812345.0, 1e-7, 1e30] {
            let f = F80::from_f64(v);
            let back = f.to_f64();
            assert!((back - v).abs() < 1e-9 * v.abs().max(1.0), "{} -> {}", v, back);
        }
    }

    #[test]
    fn seed_expr_t1c06() {
        // T1C06: x=3812345, y=-4287654, z=481093.
        // Borland `-f287` evaluates this in 80-bit extended precision;
        // the (long) cast then truncates to -7863, and `srand` keeps only
        // the low 16 bits (0xE149 = 5737). We only assert the f80
        // evaluation matches a plain f64 evaluation closely.
        let x = F80::from_f64(3812345.0);
        let y = F80::from_f64(-4287654.0);
        let z = F80::from_f64(481093.0);
        let c = F80::from_f64(100000.0);
        let sd = x.div(&c).mul(&y).div(&c).mul(&z).div(&c).to_f64();
        assert!((sd - (-7863.9540143459)).abs() < 1e-6, "sd = {}", sd);
    }

    #[test]
    fn mul_div_basic() {
        let a = F80::from_f64(2.0);
        let b = F80::from_f64(3.0);
        assert!((a.mul(&b).to_f64() - 6.0).abs() < 1e-12);
        let c = F80::from_f64(7.0);
        let d = F80::from_f64(2.0);
        assert!((c.div(&d).to_f64() - 3.5).abs() < 1e-12);
    }
}

#[cfg(test)]
mod rne_tests {
    use super::F80;
    #[test]
    fn rne_basic() {
        assert_eq!(F80::from_f64(2.4).to_i32_rne(), 2);
        assert_eq!(F80::from_f64(2.5).to_i32_rne(), 2); // tie to even
        assert_eq!(F80::from_f64(3.5).to_i32_rne(), 4); // tie to even
        assert_eq!(F80::from_f64(2.6).to_i32_rne(), 3);
        assert_eq!(F80::from_f64(-2.5).to_i32_rne(), -2); // tie to even (neg even)
        assert_eq!(F80::from_f64(-2.6).to_i32_rne(), -3);
        assert_eq!(F80::from_f64(0.0).to_i32_rne(), 0);
    }
    #[test]
    fn rne_wave1() {
        let cr = 3.0f64; let a = 8.0/30.0;
        let mut diff=0;
        for px in 1..=360 {
            let prod = ((px as f64)*a).sin()*cr;
            let trunc = prod as i32;
            let rne = F80::from_f64(prod).to_i32_rne();
            if rne != trunc { diff+=1; }
        }
        println!("wave1 rne!=trunc count: {}", diff);
        assert!(diff > 0); // should differ (rounding mode matters)
    }
}

#[cfg(test)]
mod wave_test {
    use super::F80;
    #[test]
    fn wave1_check() {
        let cr = 3.0f64; let a = 8.0/30.0;
        // expected (recovered from C buffer): px -> py
        let exp = [(1,147),(2,148),(3,148),(4,149),(8,149),(13,145),(16,143),(20,144),(50,148),(100,149),(200,146),(300,143),(359,149)];
        for (px,ey) in exp.iter() {
            let f = (*px as f64)*a;
            let prod = F80::from_f64(f.sin()).mul(&F80::from_f64(cr));
            let py = (prod.to_i32_rne() + 146) & 0xFFFF;
            println!("px={} rust_py={} exp={}", px, py, ey);
            assert_eq!(py, *ey, "px={}", px);
        }
    }
}

#[cfg(test)]
mod dbg_rne {
    use super::F80;
    #[test]
    fn dbg_0791() {
        let x = F80::from_f64(0.791);
        println!("exp={} BIAS={} mant={:x} unbiased={}", x.exp, super::BIAS, x.mant, (x.exp as i32 - super::BIAS) - 63);
        println!("to_i32_rne={}", x.to_i32_rne());
        let y = F80::from_f64(0.2637*3.0);
        println!("y=0.7911 to_i32_rne={}", y.to_i32_rne());
    }
}

#[cfg(test)]
mod wave_all_test {
    use super::F80;
    use std::fs;
    #[test]
    fn wave_all_dump() {
        let cr = 3.0f64; let a = 8.0/30.0;
        let mut out = String::new();
        for px in (1..=360).rev() {  // wave iterates 360..1
            let f = (px as f64)*a;
            let prod = F80::from_f64(f.sin()).mul(&F80::from_f64(cr));
            let py = (prod.to_i32_rne() + 146) & 0xFFFF;
            out.push_str(&format!("{} {}\n", px, py));
        }
        let _ = fs::write("/tmp/rust_wave1.txt", out);
    }
}

#[cfg(test)]
mod cos_tests {
    use super::F80;
    #[test]
    fn f80_cos_test() {
        let cases = [
            (0.0_f64, 1.0),
            (0.5, 0.8775825618903728),
            (1.0, 0.5403023058681398),
            (1.5707963267948966, 0.0), // π/2
            (3.141592653589793, -1.0), // π
            (4.71238898038469, 0.0),   // 3π/2
            (6.283185307179586, 1.0),   // 2π
            (2.0, -0.4161468365471424),
            (3.0, -0.9899924966004454),
        ];
        for &(x, expected) in &cases {
            let cos_f80 = F80::from_f64(x).cos().to_f64();
            let err = (cos_f80 - expected).abs();
            let cutoff = if expected.abs() < 1e-10 { 1e-10 } else { expected.abs() * 1e-10 };
            assert!(err < cutoff, "cos({:.10}): expected {:.10e} got {:.10e} err={:.2e}",
                x, expected, cos_f80, err);
        }
        println!("F80 cos: all {} cases passed", cases.len());
    }
}
