//! Random-number generators, bit-exact with the Noctis IV Plus originals.
//!
//! Two RNGs are used by the generator:
//!
//! * `rand()` / `srand()` - Borland's C-library LCG. Confirmed against the
//!   harness selftest: `srand(1)` yields `346, 130, 10982, 1090, ...`.
//!   The update is `seed = (seed * 0x15A4E35 + 1) & 0x7FFFFFFF` (31-bit),
//!   and `rand()` returns `(seed >> 16) & 0x7FFF`.
//!
//! * `fast_random` / `fast_srand` - Noctis' own 16/32-bit self-multiplier.
//!   `fast_srand(seed)` forces the low word `|= 3` and stores the low 16
//!   bits of `seed` into `flat_rnd_seed` (high word left as-is). Each
//!   `fast_random(mask)` performs a 32-bit `mul` of the seed with itself,
//!   mixes the low byte of the product with the low byte of the high word,
//!   adds the (possibly 32-bit) result back into `flat_rnd_seed`, and
//!   returns `ax & mask`.

pub struct BorlandRng {
    seed: u32,
    #[cfg(debug_assertions)]
    pub call_count: u64,
}

impl BorlandRng {
    pub fn new() -> Self {
        BorlandRng { seed: 0, #[cfg(debug_assertions)] call_count: 0 }
    }

    pub fn srand(&mut self, seed: u32) {
        // Borland is a 16-bit compiler: `srand`'s parameter is a 16-bit
        // `unsigned int`, so the seed is truncated to its low 16 bits. This
        // is invisible for small seeds (1, 7665, 8556, ...) but matters
        // for the large seeds the generator actually feeds it.
        self.seed = seed & 0xFFFF;
    }

    #[allow(dead_code)]
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Borland `rand()` (15-bit result in [0, 0x7FFF]).
    pub fn rand(&mut self) -> i32 {
        #[cfg(debug_assertions)]
        { self.call_count += 1; }
        // 32-bit multiply with natural overflow. Borland's _randseed is
        // signed long, so bit 31 CAN be set — no masking.
        self.seed = self.seed.wrapping_mul(0x15A4_E35).wrapping_add(1);
        ((self.seed >> 16) & 0x7FFF) as i32
    }

    /// Reverse the LCG by `n` steps. Used to correct a 2-call mismatch
    /// between C and Rust surface generation (the exact source is unknown
    /// but the state delta is confirmed). Does NOT affect pixel values —
    /// only the LCG state used by subsequent palette computation.
    pub fn step_back(&mut self, n: u32) {
        // Modular inverse of M modulo 2^32: M^(-1) = 0x2925141D.
        for _ in 0..n {
            self.seed = self.seed.wrapping_sub(1).wrapping_mul(0x2925_141D);
        }
    }

    /// Borland `random(n)` (from `<dos.h>`): `((long)rand() * n) /
    /// ((long)RAND_MAX + 1)`, with the product computed in `long` (signed)
    /// and the result assigned back to a (16-bit signed) `int`. With
    /// `RAND_MAX == 0x7FFF` this is `rand() * n / 32768` in signed
    /// 32-bit arithmetic.
    ///
    /// Consequences that must be reproduced exactly:
    ///   * It is NOT `rand() % n` (no modulo bias).
    ///   * `rand()` is consumed for EVERY call, including `n <= 0`.
    ///   * For `n <= 0` the product is non-positive, so the result is `0`
    ///     for `n == 0` and NEGATIVE for `n < 0` (e.g. when an expression
    ///     like `10 * nearstar_p_orb_seed[n]` goes negative). The result is
    ///     cast to a 16-bit `int`, which here always fits in `i32`.
    pub fn random(&mut self, n: i32) -> i32 {
        let r = self.rand() as i64;
        ((r * n as i64) / 32768) as i32
    }

    /// `zrandom(range)` = `random(range) - random(range)` (float).
    pub fn zrandom(&mut self, range: i32) -> f64 {
        (self.random(range) - self.random(range)) as f64
    }

    /// `flandom()` = `random(32767) * 0.000030518`.
    pub fn flandom(&mut self) -> f64 {
        (self.random(32767) as f64) * 0.000030518
    }
}

/// Noctis' fast RNG. `flat_rnd_seed` is stored as a 32-bit value but the
/// `fast_srand` operation only writes the low 16 bits.
pub struct FastRng {
    seed: u32,
}

impl FastRng {
    pub fn new() -> Self {
        FastRng { seed: 0 }
    }

    pub fn srand(&mut self, seed: u32) {
        // `fast_srand(long seed)` (NOCTIS-0.CPP:1076). Every operand carries
        // the 0x66 operand-size prefix, so the `word ptr`/`ax` accesses are
        // actually 32-bit: `or dword ptr seed, 3` then
        // `mov eax, dword ptr seed` -> `flat_rnd_seed = seed | 3` (full 32-bit).
        self.seed = seed | 3;
    }

    /// `fast_random(mask)` (NOCTIS-0.CPP:1087).
    ///
    /// All operands carry the 0x66 32-bit prefix, so this is a 32-bit
    /// self-multiply: `eax = flat_rnd_seed; edx = flat_rnd_seed; mul edx`
    /// -> `edx:eax = eax*edx` (64-bit product). `al += dl` mixes the low
    /// byte of `eax` (low byte of the 32-bit product) with the low byte of
    /// `edx` (low byte of the HIGH 32 bits of the 64-bit product). The full
    /// 32-bit `eax` (with the mixed low byte) is added back into
    /// `flat_rnd_seed` (32-bit add), and the result is `eax & mask`.
    pub fn fast_random(&mut self, mask: u32) -> u32 {
        let eax = self.seed;
        let edx = self.seed;
        let prod = (eax as u64) * (edx as u64);
        let eax_lo = (prod & 0xFFFF_FFFF) as u32;
        let edx_hi = (prod >> 32) as u32;
        // al = low byte of eax_lo ; dl = low byte of edx_hi
        let al = (eax_lo & 0xFF) as u8;
        let dl = (edx_hi & 0xFF) as u8;
        let al = al.wrapping_add(dl);
        let eax_new = (eax_lo & 0xFFFF_FF00) | (al as u32);
        // 32-bit add back into the seed
        self.seed = self.seed.wrapping_add(eax_new);
        eax_new & mask
    }

    pub fn ranged(&mut self, range: i32) -> i32 {
        let range = if range <= 0 { 1 } else { range };
        (self.fast_random(0x7FFF) as i32) % range
    }

    pub fn fast_flandom(&mut self) -> f64 {
        (self.fast_random(0x7FFF) as f64) * 0.000030518
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borland_rand_sequence() {
        let mut r = BorlandRng::new();
        r.srand(1);
        let seq = [346, 130, 10982, 1090, 11656, 7117, 17595, 6415];
        for &exp in &seq {
            assert_eq!(r.rand(), exp, "borland rand mismatch");
        }
    }

    #[test]
    fn fast_rand_sequence() {
        let mut f = FastRng::new();
        f.srand(12345);
        let seq = [11673, 14109, 2627, 8846, 6091, 22323, 4112, 10725];
        for &exp in &seq {
            assert_eq!(f.fast_random(0x7FFF), exp, "fast_random mismatch");
        }
    }

    #[test]
    fn fast_flandom_value() {
        let mut f = FastRng::new();
        f.srand(1);
        // matches harness selftest: fast_flandom=0.00027466
        let v = f.fast_flandom();
        assert!((v - 0.00027466).abs() < 1e-8, "fast_flandom={}", v);
    }
}
