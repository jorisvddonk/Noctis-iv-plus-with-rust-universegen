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
        let mut mant = self.mant;
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
        let (mut a, mut b, mut ea, eb);
        if self.exp >= ob.exp {
            a = self.mant; ea = self.exp;
            let diff = self.exp - ob.exp;
            b = if diff >= 64 { 0 } else { ob.mant >> diff };
            eb = ob.exp;
        } else {
            b = ob.mant; eb = ob.exp;
            let diff = ob.exp - self.exp;
            a = if diff >= 64 { 0 } else { self.mant >> diff };
            ea = self.exp;
        }
        let (res_neg, res_mant, mut res_exp);
        if self.neg == ob.neg {
            let s = a as u128 + b as u128;
            // s may have a carry into bit 64
            let (m, carry) = if s >> 63 == 1 {
                // already normalised (top bit integer bit), but could have 2 int bits
                if (s >> 64) & 1 == 1 {
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
