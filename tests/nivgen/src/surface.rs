//! Planet surface generation: `surface()` and its helpers, ported
//! bug-for-bug from `source/NOCTIS-0.CPP` (lines 332, 4380, 4485,
//! 4766, 5153, etc.).
//!
//! Every inline-assembly loop (the `rndpat` quadratic PRNG, the `ssmooth`/
//! `lssmooth`/`psmooth_grays` smoothers, the `darkside`/`darkline`
//! terminator) is reproduced with explicit **16-bit**/`32-bit` wrap semantics,
//! since Borland `int`/`unsigned` are 16-bit and Rust's `as i16` saturates.

use crate::system::SystemGen;
use crate::f80::F80;
use std::f64::consts::PI;
use std::io::Write;

pub const DEG: f64 = PI / 180.0;

/// C `wave()` memory-corruption base offset (see `wave()`). The far-write
/// aliases `cy`, turning the `add py, bx` base into `cy + WAVE_CORRUPT_BASE`.
/// Tunable via env `WAVE_BASE` for matching a specific C build's layout.
pub static mut WAVE_CORRUPT_BASE: i32 = 28092;
fn wave_corrupt_base() -> i32 {
    if let Ok(s) = std::env::var("WAVE_BASE") {
        s.parse().unwrap_or(28092)
    } else {
        28092
    }
}

/// x87 `fistp` round-to-nearest-even on a `f64` value (the FPU keeps 80-bit
// C's `(unsigned long)(f64)` truncates (mod 2^32); Rust's `f64 as u32`
// SATURATES on overflow, so large `seedval*10` values collapse to 0xFFFFFFFF
// and every body gets the same rndpat seed. Mirror the C truncation here.
fn f64_to_u32_trunc(x: f64) -> u32 {
    (x.rem_euclid(4294967296.0)) as u32
}

/// Truncate a double toward zero and wrap into a 16-bit `unsigned` (Borland
/// `unsigned px, py` are 16-bit). Used wherever the C source stores the
/// `double` result of `cx + cos(a)*gr` etc. into a 16-bit `unsigned` variable,
/// i.e. taken mod 2^16 (this matters for negative results).
fn f64_to_u16_trunc(x: f64) -> u32 {
    let t = x.trunc();
    ((t as i32) & 0xFFFF) as u32
}

#[allow(dead_code)]
fn dbg_dump(name: &str, buf: &[u8]) {
    if CURRENT_BODY.load(Ordering::Relaxed) != capture_body() {
        return;
    }
    let _ = std::fs::create_dir_all("/tmp/rchk");
    let _ = std::fs::File::create(format!("/tmp/rchk/CHK_{}.BIN", name))
        .and_then(|mut f| f.write_all(buf));
}

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::OnceLock;
// Validation-only: capture checkpoints only for this body (C's `planet -n N`
// generates a single body; Rust's generate() hits many — avoid contamination).
// Tunable via env CAPTURE_BODY (default 0).
fn capture_body() -> i32 {
    static V: OnceLock<i32> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("CAPTURE_BODY")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0)
    })
}
static CURRENT_BODY: AtomicI32 = AtomicI32::new(-1);

/// Borland signed-16-bit wrap.
pub fn i16w(x: f64) -> i32 {
    let t = (x as i32) & 0xFFFF;
    if t >= 32768 {
        t - 65536
    } else {
        t
    }
}

impl SystemGen {
    // --- rndpat: quadratic PRNG fill of p_background (NOCTIS-0.CPP:4821) ---
    fn rndpat(&mut self, seed: u16) {
        let mut ax: u16 = seed;
        let len = 64800i32;
        let mut di = 0usize;
        for cx in (1..=len).rev() {
            // add ax, cx  (16-bit wrap)
            ax = ax.wrapping_add(cx as u16);
            // xor dx,dx ; imul ax ; add ax,dx
            // F7 E8 = signed 16-bit IMUL: sign-extend AX to 32-bit, square, result in DX:AX
            let sx = if ax >= 32768 { ax as i32 - 65536 } else { ax as i32 };
            let prod = sx.wrapping_mul(sx); // 32-bit signed product
            let dx: u16 = ((prod >> 16) & 0xFFFF) as u16;
            let low: u16 = (prod & 0xFFFF) as u16;
            ax = low.wrapping_add(dx);
            let bl: u8 = (ax as u8) & 0x3E;
            self.p_background[di] = bl;
            di += 1;
        }
    }

    // --- ssmooth (NOCTIS-0.CPP:4380): in-place 4-row packed 32-bit average ---
    fn ssmooth(&mut self) {
        let q = self.quadwords;
        let cx = (q * 4 - 360 * 4) as usize;
        let mut di = 360usize;
        unsafe {
            let buf = self.p_background.as_mut_ptr();
            for _ in 0..cx {
                let w = |off: usize| -> u32 {
                    let b = buf.add(off);
                    *b as u32 | ((*b.add(1) as u32) << 8) | ((*b.add(2) as u32) << 16) | ((*b.add(3) as u32) << 24)
                };
                let mut edx: u32 = w(di - 360)
                    .wrapping_add(w(di))
                    .wrapping_add(w(di + 360))
                    .wrapping_add(w(di + 720));
                edx &= 0xFCFCFCFC;
                edx >>= 2;
                let mut al: u8 = (edx & 0xFF) as u8;
                al = al.wrapping_add(((edx >> 8) & 0xFF) as u8);
                edx >>= 16;
                al = al.wrapping_add(((edx) & 0xFF) as u8);
                al = al.wrapping_add(((edx >> 8) & 0xFF) as u8);
                al >>= 2;
                *buf.add(di) = al;
                di += 1;
            }
        }
    }

    // --- lssmooth (NOCTIS-0.CPP:4416): 2x2 average, in-place ---
    fn lssmooth(&mut self) {
        let q = self.quadwords;
        let cx = (q * 4 - 80 * 4) as usize;
        let mut di = 0usize;
        unsafe {
            let buf = self.p_background.as_mut_ptr();
            for _ in 0..cx {
                let d0 = *buf.add(di) as u32;
                let d1 = *buf.add(di + 1) as u32;
                let b0 = *buf.add(di + 360) as u32;
                let b1 = *buf.add(di + 361) as u32;
                let dx = (d0 | (d1 << 8)) & 0x3F3F;
                let bx = (b0 | (b1 << 8)) & 0x3F3F;
                let mut al: u8 = (d0 as u8) & 0xC0;
                let mut dl = ((d0 & 0x3F) as u8).wrapping_add((dx >> 8) as u8);
                dl = dl.wrapping_add((bx & 0xFF) as u8);
                dl = dl.wrapping_add(((bx >> 8) & 0xFF) as u8);
                dl >>= 2;
                al |= dl;
                *buf.add(di) = al;
                di += 1;
            }
        }
    }

    // --- psmooth_grays (NOCTIS-0.CPP:480): 2x2 average on target ---
    fn psmooth_grays(&mut self) {
        let w = 360usize;
        let h = 180usize;
        let src = self.p_background.clone();
        for y in 0..h {
            for x in 0..w {
                let mut sum: u32 = 0;
                let mut cnt: u32 = 0;
                for dy in 0..2usize {
                    for dx in 0..2usize {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < w && ny < h {
                            sum += src[ny * w + nx] as u32;
                            cnt += 1;
                        }
                    }
                }
                self.p_background[y * w + x] = (sum / cnt) as u8;
            }
        }
    }

    // --- pclear (NOCTIS-0.CPP:332): fill QUADWORDS*4 bytes with pattern ---
    fn pclear(&mut self, pattern: u8) {
        let n = (self.quadwords * 4) as usize;
        for i in 0..n {
            self.p_background[i] = pattern;
        }
    }

    // --- spot (NOCTIS-0.CPP:4485) ---
    fn spot(&mut self) {
        let di = (self.s_py.wrapping_add(self.s_px)) as usize; // py + px (px,py are byte offsets)
        if di >= self.p_background.len() {
            return;
        }
        let al = self.p_background[di].wrapping_add(self.s_gr as u8);
        let al = if al < 0x3E { al } else { 0x3E };
        self.p_background[di] = al;
    }

    // --- permanent_storm (NOCTIS-0.CPP:4498) ---
    fn permanent_storm(&mut self) {
        let mut g = 1;
        while g < self.s_cr {
            let mut a: f32 = 0.0;
            while (a as f64) < 2.0 * PI {
                let af = a as f64;
                let px = f64_to_u16_trunc(self.s_cx as f64 + g as f64 * af.cos());
                let py = f64_to_u16_trunc(self.s_cy as f64 + g as f64 * af.sin());
                let py = py.wrapping_mul(360);
                self.s_px = (px & 0xFFFF) as u16;
                self.s_py = (py & 0xFFFF) as u16;
                self.spot();
                a += (4.0 * DEG) as f32;
            }
            g += 1;
        }
    }

    // --- crater (NOCTIS-0.CPP:4510) ---
    fn crater(&mut self) {
        let mut a: f32 = 0.0;
        while (a as f64) < 2.0 * PI {
            let af = a as f64;
            let mut gr = 0;
            let mut last_di = 0usize;
            while gr < self.s_cr {
                let px_i = f64_to_u16_trunc(self.s_cx as f64 + af.cos() * gr as f64) as i64;
                let py_i = f64_to_u16_trunc(self.s_cy as f64 + af.sin() * gr as f64) as i64;
                let vptr = px_i + 360 * py_i;
                let di = (vptr as u32 & 0xFFFF) as usize;
                last_di = di;
                if di < self.p_background.len() {
                    let ah = (gr & 0xFF) as u8;
                    let cl = (self.s_lawe as u32 & 0xFF) as u8;
                    let ah = ah >> cl;
                    // C: `sub al, ah ; jnc entro ; xor al, al`.
                    // `jnc` tests the CARRY (borrow) flag (unsigned al < ah),
                    // NOT the sign flag. Compare the original byte, not the result.
                    let al0 = self.p_background[di];
                    let al = if al0 >= ah { al0 - ah } else { 0 };
                    self.p_background[di] = al;
                }
                gr += 1;
            }
            // second asm block: runs ONCE per angle (after the gr loop), using
            // the LAST vptr / di (from gr = cr-1). Writes 0x3E at di, 0x01 at di+1.
            if last_di < self.p_background.len() {
                self.p_background[last_di] = 0x3E;
                if last_di + 1 < self.p_background.len() {
                    self.p_background[last_di + 1] = 0x01;
                }
            }
            // crays && !random(crays) -- runs ONCE per angle.  Borland
            // short-circuits: when crays==0 the `random(crays)` call is NEVER
            // made, so the RNG state must not advance.
            let mut triggered = false;
            if self.s_cray != 0 {
                let rc = self.rng.random(self.s_cray);
                triggered = rc == 0;
            }
            if triggered {
                let r2 = self.rng.random(2);
                let b = (2 + r2) * self.s_cr;
                if (self.s_cy - b) > 0 && (self.s_cy + b) < 179 {
                    let mut gr2 = self.s_cr + 1;
                    while gr2 < b {
                        let px_i = f64_to_u16_trunc(self.s_cx as f64 + af.cos() * gr2 as f64) as i64;
                        let py_i = f64_to_u16_trunc(self.s_cy as f64 + af.sin() * gr2 as f64) as i64;
                        let vptr = px_i + 360 * py_i;
                        let di = (vptr as u32 & 0xFFFF) as usize;
                        if di < self.p_background.len() {
                            let al = self.p_background[di];
                            let mut al = al.wrapping_add((self.s_cr & 0xFF) as u8);
                            if al >= 0x3E {
                                al = 0x3E;
                            }
                            self.p_background[di] = al;
                        }
                        gr2 += 1;
                    }
                }
            }
            a += (4.0 * DEG) as f32;
        }
    }

    // --- band (NOCTIS-0.CPP:4554) ---
    fn band(&mut self) {
        // C: `les di, p_background; add di, py` then `inc di` per iteration.
        // `di` is a 16-bit register, so it WRAPS at 0x10000.
        let base = (self.s_py & 0xFFFF) as usize;
        let cx = self.s_cr as usize;
        let ah = (self.s_g & 0xFF) as u8;
        for i in 0..cx {
            let di = (base + i) & 0xFFFF;
            if di < self.p_background.len() {
                // C: sub al,ah ; jnc min (jump if no borrow = al >= ah)
                let al = if self.p_background[di] >= ah {
                    self.p_background[di] - ah
                } else {
                    0
                };
                self.p_background[di] = al;
            }
        }
    }

    // --- wave (NOCTIS-0.CPP:4571) ---
    fn wave(&mut self) {
        let mut px = 360i32;
        // C `wave()` bug: the `add py, bx` base (`bx` = `cy`) is corrupted by the
        // `mov byte ptr es:[di], 0` far-write aliasing `cy`'s memory. After the
        // first column the base becomes a garbage large value (G); we replicate
        // that by adding a constant corruption offset to `bx` for px < 360.
        let bx0 = (self.s_cy as i32) & 0xFFFF;
        while px > 0 {
            let f = (px as f64) * self.s_a;
            let prod = F80::from_f64(f.sin()).mul(&F80::from_f64(self.s_cr as f64));
            let bx = if px == 360 { bx0 } else { bx0 + wave_corrupt_base() };
            let py = (prod.to_i32_rne() + bx) & 0xFFFF;
            let ax = (py & 0xFFFF) as i32;
            let prod = (ax as i64).wrapping_mul(360i64) & 0xFFFF;
            let di = ((prod + (px as i64 & 0xFFFF)) & 0xFFFF) as usize;
            if di < self.p_background.len() {
                self.p_background[di] = 0;
            }
            px -= 1;
        }
    }

    // --- fracture (NOCTIS-0.CPP:4594) ---
    fn fracture(&mut self, max_latitude: f64) {
        self.s_a = (self.rng.random(360) as f32 * DEG as f32) as f64;
        self.s_gr += 1;
        let mut px = self.s_cx as f64;
        let mut py = self.s_cy as f64;
        loop {
            self.s_a += (self.rng.random(self.s_g) - self.rng.random(self.s_g)) as f64 * DEG;
            px += self.s_kfract * self.s_a.cos();
            if px > 359.0 {
                px -= 360.0;
            }
            if px < 0.0 {
                px += 360.0;
            }
            py += self.s_kfract * self.s_a.sin();
            if py > max_latitude - 1.0 {
                py -= max_latitude;
            }
            if py < 0.0 {
                py += max_latitude;
            }
            // C: `vptr = px + 360 * (unsigned)py`. `py` is truncated to
            // `(unsigned)` (16-bit integer) BEFORE the *360. `px` is a float
            // added to the int result.
            let px_i = px as i32;
            let py_i = py as i32;
            let vptr = px_i as i64 + 360 * py_i as i64;
            let di = vptr as usize;
            if di < self.p_background.len() {
                self.p_background[di] >>= (self.s_b & 0xFF) as u8;
            }
            self.s_gr -= 1;
            if self.s_gr == 0 {
                break;
            }
        }
    }

    // --- volcano (NOCTIS-0.CPP:4617) ---
    fn volcano(&mut self) {
        let mut a: f32 = 0.0;
        while (a as f64) < 2.0 * PI {
            let af = a as f64;
            let b = self.s_gr;
            let mut g = self.s_cr / 2;
            while g < self.s_cr {
                let px = f64_to_u16_trunc(self.s_cx as f64 + af.cos() * g as f64);
                let py = f64_to_u16_trunc(self.s_cy as f64 + af.sin() * g as f64);
                let py = py.wrapping_mul(360);
                self.s_px = (px & 0xFFFF) as u16;
                self.s_py = (py & 0xFFFF) as u16;
                self.spot();
                if self.s_gr > 0 {
                    self.s_gr -= 1;
                }
                g += 1;
            }
            self.s_gr = b;
            a += (4.0 * DEG) as f32;
        }
    }

    // --- contrast (NOCTIS-0.CPP:4633) ---
    fn contrast(&mut self, kt: f64, kq: f64, thrshld: f64) {
        // Borland x87 computes this chain in 80-bit precision. Mirror it with
        // F80 so rounding matches (plain f64 drifts from x87 extended precision).
        let ktf = F80::from_f64(kt);
        let kqf = F80::from_f64(kq);
        let thr = F80::from_f64(thrshld);
        for c in 0..64800 {
            let a0 = F80::from_f64(self.p_background[c] as f64);
            let mut a = a0.add(&thr, true);
            if a.to_f64() > 0.0 {
                a = a.mul(&ktf);
            } else {
                a = a.mul(&kqf);
            }
            a = a.add(&thr, false);
            let av = a.to_f64();
            let av = if av < 0.0 { 0.0 } else if av > 63.0 { 63.0 } else { av };
            self.p_background[c] = av as u8;
        }
    }

    // --- randoface (NOCTIS-0.CPP:4651) ---
    fn randoface(&mut self, range: i32, upon: i32) {
        for c in 0..64800 {
            let mut gr = self.p_background[c] as i32;
            let cond = if upon > 0 {
                gr >= upon
            } else {
                gr <= -upon
            };
            if cond {
                gr += self.rng.random(range);
                gr -= self.rng.random(range);
                if gr > 63 {
                    gr = 63;
                }
                if gr < 0 {
                    gr = 0;
                }
                self.p_background[c] = gr as u8;
            }
        }
    }

    // --- negate (NOCTIS-0.CPP:4667) ---
    fn negate(&mut self) {
        for i in self.p_background.iter_mut() {
            *i = 0x3Eu8.wrapping_sub(*i);
        }
    }

    fn negate_bg(&mut self, buf: &mut [u8]) {
        for i in buf.iter_mut() {
            *i = 0x3Eu8.wrapping_sub(*i);
        }
    }

    // --- crater_juice (NOCTIS-0.CPP:4679) ---
    fn crater_juice(&mut self) {
        self.s_lawe = self.rng.random(3);
        self.s_cray = self.rng.random(3) * 2;
        for _c in 0..self.s_r {
            self.s_cx = self.rng.random(360);
            self.s_cr = 2 + self.rng.random(1 + self.s_r - _c);
            while self.s_cr > 20 {
                self.s_cr -= 10;
            }
            self.s_cy = self.rng.random(178 - 2 * self.s_cr) + self.s_cr;
            self.crater();
            if self.s_cr > 15 {
                self.lssmooth();
            }
        }
    }

    // --- cirrus (NOCTIS-0.CPP:4715) overlay helper ---
    fn cirrus(&mut self) {
        let bx = (self.s_py.wrapping_add(self.s_px)) as usize;
        let idx = (bx >> 1) + 0; // objectschart base is 0; px,py already halved via shr
        if idx < self.objectschart.len() {
            let mut al = self.objectschart[idx].wrapping_add((self.s_gr & 0xFF) as u8);
            al = if al < 0x1F { al } else { 0x1F };
            self.objectschart[idx] = al;
        }
    }

    // --- atm_cyclon (NOCTIS-0.CPP:4729) ---
    fn atm_cyclon(&mut self) {
        let mut b = 0i32;
        let mut cr = self.s_cr;
        let mut a: f32 = self.s_a as f32;
        while cr > 0 {
            // C: px,py are `unsigned`; the double results are TRUNCATED to
            // unsigned at each assignment, and `py *= 360` is an integer
            // multiply (truncated py first).
            let mut px = f64_to_u16_trunc(self.s_cx as f64 + cr as f64 * (a as f64).cos());
            let mut py = f64_to_u16_trunc(self.s_cy as f64 + cr as f64 * (a as f64).sin());
            py = py.wrapping_mul(360);
            self.s_px = (px & 0xFFFF) as u16;
            self.s_py = (py & 0xFFFF) as u16;
            self.cirrus();
            px = px.wrapping_add(self.rng.random(4) as u32);
            self.s_px = (px & 0xFFFF) as u16;
            self.cirrus();
            py = py.wrapping_add(359);
            self.s_py = (py & 0xFFFF) as u16;
            self.cirrus();
            px = px.wrapping_sub(self.rng.random(4) as u32);
            self.s_px = (px & 0xFFFF) as u16;
            self.cirrus();
            py = py.wrapping_add(361);
            self.s_py = (py & 0xFFFF) as u16;
            self.cirrus();
            px = px.wrapping_add(self.rng.random(4) as u32);
            self.s_px = (px & 0xFFFF) as u16;
            self.cirrus();
            b += 1;
            b %= self.s_g;
            if b == 0 {
                cr -= 1;
            }
            a += (6.0 * DEG) as f32;
        }
        self.s_a = a as f64;
    }

    // --- storm (NOCTIS-0.CPP:4747) ---
    fn storm(&mut self) {
        let mut g = 1;
        while g < self.s_cr {
            let mut a: f32 = 0.0;
            while (a as f64) < 2.0 * PI {
                let af = a as f64;
                let px = f64_to_u16_trunc(self.s_cx as f64 + g as f64 * af.cos());
                let py = f64_to_u16_trunc(self.s_cy as f64 + g as f64 * af.sin());
                let py = py.wrapping_mul(360);
                self.s_px = (px & 0xFFFF) as u16;
                self.s_py = (py & 0xFFFF) as u16;
                self.cirrus();
                a += (4.0 * DEG) as f32;
            }
            g += 1;
        }
    }

    // --- planet_viewpoint / cplx_planet_viewpoint / planet_xyz ---
    fn planet_xyz(&mut self, n: i32) {
        let n = n as usize;
        let ors = self.p_orb_ray[n] * self.p_orb_ray[n];
        let p_riv = if self.p_owner[n] > -1 {
            let xx = self.p_ray[self.p_owner[n] as usize];
            let p_m = std::f64::consts::PI * xx * xx * xx * 0.44e-4;
            (p_m / ors).sqrt()
        } else {
            (self.s_m / ors).sqrt()
        };
        let beta = (self.secs * p_riv * PI) / 180.0;
        let alfa = self.p_orb_tilt[n] * DEG;
        let xx = -self.p_orb_ray[n] * beta.sin() * alfa.cos();
        let mut plz = self.p_orb_ray[n] * beta.cos() * alfa.cos();
        let ply = self.p_orb_ray[n] * alfa.sin();
        plz *= self.p_orb_ecc[n];
        let obeta = self.p_orb_orient[n];
        self.plx = xx * obeta.cos() + plz * obeta.sin();
        let newplz = plz * obeta.cos() - xx * obeta.sin();
        self.ply = ply + self.nearstar_y;
        self.plz = newplz + self.nearstar_z;
        self.plx += self.nearstar_x;
    }

    fn planet_viewpoint(&self, obs_x: f64, obs_z: f64) -> i32 {
        let mut min = 1e99;
        let mut plwp = 0i32;
        for a in 0..360u32 {
            let xa = self.plx + (DEG * a as f64).cos() - obs_x;
            let zz = self.plz + (DEG * a as f64).sin() - obs_z;
            let xx = xa * xa + zz * zz;
            if xx < min {
                plwp = a as i32;
                min = xx;
            }
        }
        plwp
    }

    fn cplx_planet_viewpoint(&mut self, logical_id: i32) -> i32 {
        let owner = self.p_owner[logical_id as usize];
        if owner == -1 {
            return self.planet_viewpoint(self.nearstar_x, self.nearstar_z);
        }
        if self.p_type[owner as usize] == 10 {
            self.planet_xyz(owner);
            let ownerplx = self.plx;
            let ownerplz = self.plz;
            self.planet_xyz(logical_id);
            return self.planet_viewpoint(ownerplx, ownerplz);
        } else {
            return self.planet_viewpoint(self.nearstar_x, self.nearstar_z);
        }
    }

    // --- shade (NOCTIS-0.CPP:1152) ---
    fn shade(&mut self, first_color: i32, number_of_colors: i32,
             start_r: f64, start_g: f64, start_b: f64,
             finish_r: f64, finish_g: f64, finish_b: f64) {
        let count = number_of_colors;
        if count <= 0 {
            return;
        }
        let k = 1.00 / count as f64;
        let delta_r = (finish_r - start_r) * k;
        let delta_g = (finish_g - start_g) * k;
        let delta_b = (finish_b - start_b) * k;
        let mut fc = (first_color * 3) as usize;
        let mut sr = start_r;
        let mut sg = start_g;
        let mut sb = start_b;
        let mut c = count;
        while c > 0 {
            let put = |v: f64| -> u8 {
                if v >= 0.0 && v < 64.0 {
                    v as u8
                } else if v > 0.0 {
                    63u8
                } else {
                    0u8
                }
            };
            if fc + 2 < self.tmppal.len() {
                self.tmppal[fc + 0] = put(sr);
                self.tmppal[fc + 1] = put(sg);
                self.tmppal[fc + 2] = put(sb);
            }
            sr += delta_r;
            sg += delta_g;
            sb += delta_b;
            fc += 3;
            c -= 1;
        }
    }

    // --- tavola_colori (NOCTIS-0.CPP:179) ---
    // Copies `nuova_tavolozza[0 .. nr_colori*3]` into
    // `tmppal[3*colore_di_partenza ..]` then filters each component through
    // `*= filtro / 63` clamped to 63. The VGA-port asm tail is skipped (only
    // the palette bytes matter for the hash). In `surface()` the source and
    // destination overlap (both offset by `colorbase*3`), so this is an
    // identity copy followed by the filter.
    fn tavola_colori(&mut self, colorbase: i32, filtro_r: i32, filtro_g: i32, filtro_b: i32) {
        let base = (3 * colorbase) as usize;
        let nr = (64 * 3) as usize; // nr_colori *= 3
        if base + nr > self.tmppal.len() {
            return;
        }
        // identity copy (src == dst here)
        // filter pass
        let mut c = 0usize;
        while c < nr {
            let mut temp = self.tmppal[base + c] as i32;
            temp *= filtro_r;
            temp /= 63;
            if temp > 63 {
                temp = 63;
            }
            self.tmppal[base + c] = temp as u8;
            c += 1;
            let mut temp = self.tmppal[base + c] as i32;
            temp *= filtro_g;
            temp /= 63;
            if temp > 63 {
                temp = 63;
            }
            self.tmppal[base + c] = temp as u8;
            c += 1;
            let mut temp = self.tmppal[base + c] as i32;
            temp *= filtro_b;
            temp /= 63;
            if temp > 63 {
                temp = 63;
            }
            self.tmppal[base + c] = temp as u8;
            c += 1;
        }
    }

    // --- surface() main (NOCTIS-0.CPP:4766) ---
    pub fn surface(&mut self, logical_id: i32, ptype: i32, seedval: f64, colorbase: i32) {
        CURRENT_BODY.store(logical_id, Ordering::Relaxed);
        if std::env::var("NIVDBG").is_ok() {
            eprintln!("surface entry logical_id={} ptype={} capture={}", logical_id, ptype, capture_body());
        }
        if ptype == 10 {
            return;
        }
        let qw = self.quadwords;

        self.frng.srand(f64_to_u32_trunc(seedval + 4112.0));

        let rtperiod = 10 * (self.frng.ranged(50) + 1)
            + 10 * self.frng.ranged(25)
            + self.frng.ranged(250)
            + 41;
        let mut rotation = (self.secs / rtperiod as f64) as i32;
        rotation %= 360;

        self.p_rtperiod[logical_id as usize] = rtperiod;
        self.p_rotation[logical_id as usize] = rotation;

        let plwp0 = 89 - self.cplx_planet_viewpoint(logical_id);
        let mut plwp = plwp0 + rotation;
        plwp %= 360;
        if plwp < 0 {
            plwp += 360;
        }

        self.frng.srand(f64_to_u32_trunc(seedval * 10.0));
        let seed = self.frng.fast_random(0xFFFF) as u16;

        self.rng.srand(seed as u32);
        self.rndpat(seed);
        dbg_dump("0_RN", &self.p_background);
        dbg_dump("0A_O", &self.objectschart);

        // overlay (objectschart) cleared to 0 (32400 bytes)
        for i in self.objectschart.iter_mut() {
            *i = 0;
        }

        self.rng.srand(seed as u32);
        self.quadwords = 16200;

        // scratch globals used by helpers
        self.s_c = 0;
        self.s_gr = 0;
        self.s_r = 0;
        self.s_g = 0;
        self.s_b = 0;
        self.s_cr = 0;
        self.s_cx = 0;
        self.s_cy = 0;
        self.s_a = 0.0;
        self.s_kfract = 2.0;
        self.s_lawe = 0;
        self.s_cray = 0;
        self.s_px = 0;
        self.s_py = 0;

        match ptype {
            0 => self.surf_case0(),
            1 => self.surf_case1(),
            2 => self.surf_case2(),
            3 => self.surf_case3(seed),
            4 => self.surf_case4(),
            5 => self.surf_case5(),
            6 => self.surf_case6(),
            7 => self.surf_case7(),
            8 => self.surf_case8(),
            9 => self.surf_case9(),
            _ => {}
        }

        // renorm for felisian / mars-like
        if ptype == 3 || ptype == 5 {
            for px in 0..64800 {
                self.p_background[px] >>= 1;
            }
        }

        if ptype == 3 {
            if self.frng.ranged(2) != 0 {
                self.lssmooth();
            } else {
                self.ssmooth();
            }
        }

        // merge terrain + atmosphere overlay
        let mut px = 0usize;
        let mut py = 0usize;
        while px < 32400 {
            if py < self.p_background.len() {
                self.p_background[py] = self.p_background[py].wrapping_add(self.objectschart[px]);
                if self.p_background[py] > 0x3E {
                    self.p_background[py] = 0x3E;
                }
            }
            if py + 1 < self.p_background.len() {
                self.p_background[py + 1] = self.p_background[py + 1].wrapping_add(self.objectschart[px]);
                if self.p_background[py + 1] > 0x3E {
                    self.p_background[py + 1] = 0x3E;
                }
            }
            py += 2;
            px += 1;
        }
        { dbg_dump("5_ME", &self.p_background); dbg_dump("5A_M", &self.objectschart); }

        if ptype == 2 {
            if self.rng.random(3) == 0 {
                self.psmooth_grays();
                self.s_c = 1; // knot1
            }
        }

        // terminator
        let mut ts = plwp + 35;
        if ts >= 360 {
            ts -= 360;
        }
        let mut te = ts + 130;
        if te >= 360 {
            te -= 360;
        }
        self.p_term_start[logical_id as usize] = ts;
        self.p_term_end[logical_id as usize] = te;

        // darkside/darkline asm
        let mut di = (plwp + 35) as usize;
        for _row in 0..179 {
            for _col in 0..130 {
                if di < self.p_background.len() {
                    self.p_background[di] >>= 2;
                }
                di += 1;
            }
            di += 230;
        }

        if ptype == 2 {
            if self.s_c != 0 {
                self.ssmooth();
            } else {
                let r = 3 + self.frng.ranged(5);
                for _c in 0..(r as usize) {
                    self.ssmooth();
                }
            }
        }

        if ptype == 6 {
            for _c in 0..3 {
                if self.frng.ranged(2) != 0 {
                    self.ssmooth();
                }
            }
        }

        if ptype == 9 {
            for _c in 0..6 {
                self.ssmooth();
            }
        }
        { dbg_dump("6_FI", &self.p_background); dbg_dump("6A_F", &self.objectschart); }

        if colorbase == 255 {
            self.quadwords = qw;
            return;
        }

        let t = (ptype << 2) as usize;
        let pr = self.planet_rgb_and_var_at(t + 0);
        let pg = self.planet_rgb_and_var_at(t + 1);
        let pb = self.planet_rgb_and_var_at(t + 2);
        let pc = self.planet_rgb_and_var_at(t + 3);

        let mut r = pr; r <<= 1; r += self.nearstar_r; r >>= 1;
        let mut g = pg; g <<= 1; g += self.nearstar_g; g >>= 1;
        let mut b = pb; b <<= 1; b += self.nearstar_b; b >>= 1;

        let c1 = self.rng.random(pc) - self.rng.random(pc);
        let c2 = self.rng.random(pc) - self.rng.random(pc);
        let c3 = self.rng.random(pc) - self.rng.random(pc);
        let r1 = (r + c1) as f64 * 0.25;
        let g1 = (g + c2) as f64 * 0.25;
        let b1 = (b + c3) as f64 * 0.25;
        let r2 = (r + self.rng.random(pc) - self.rng.random(pc)) as f64 * 0.75;
        let g2 = (g + self.rng.random(pc) - self.rng.random(pc)) as f64 * 0.75;
        let b2 = (b + self.rng.random(pc) - self.rng.random(pc)) as f64 * 0.75;
        let r3 = (r + self.rng.random(pc) - self.rng.random(pc)) as f64 * 1.25;
        let g3 = (g + self.rng.random(pc) - self.rng.random(pc)) as f64 * 1.25;
        let b3 = (b + self.rng.random(pc) - self.rng.random(pc)) as f64 * 1.25;

        self.shade(colorbase + 0, 16, 0.0, 0.0, 0.0, r1, g1, b1);
        self.shade(colorbase + 16, 16, r1, g1, b1, r2, g2, b2);
        self.shade(colorbase + 32, 16, r2, g2, b2, r3, g3, b3);
        self.shade(colorbase + 48, 16, r3, g3, b3, 64.0, 64.0, 64.0);

        let mut brt = self.p_owner[logical_id as usize];
        if brt == -1 {
            brt = logical_id;
        }
        if brt <= 4 {
            brt = 64;
        } else {
            brt = 64 - (4 * (brt - 4));
        }

        self.tavola_colori(colorbase, brt, brt, brt);

        self.quadwords = qw;
    }

    // --- planet_rgb_and_var accessor (44 bytes, 11 entries *4) ---
    fn planet_rgb_and_var_at(&self, i: usize) -> i32 {
        crate::system::PLANET_RGB_AND_VAR[i] as i32
    }

    // --- surface_seedval_for (NOCTIS harness) ---
    pub fn surface_seedval_for(&self, n: i32) -> f64 {
        let n = n as usize;
        let is_moon = self.p_owner[n] > -1;
        if is_moon {
            if self.p_type[n] != 0 {
                1000000.0 * self.nearstar_ray as f64 * self.p_type[n] as f64
                    * self.p_orb_orient[n]
            } else {
                2000000.0 * n as f64 * self.nearstar_ray as f64
                    * self.p_orb_orient[n]
            }
        } else {
            if self.p_type[n] != 0 {
                1000000.0 * self.p_type[n] as f64
                    * self.p_orb_seed[n] * self.p_orb_tilt[n]
                    * self.p_orb_ecc[n] * self.p_orb_orient[n]
            } else {
                2000000.0 * n as f64
                    * self.p_orb_seed[n] * self.p_orb_tilt[n]
                    * self.p_orb_ecc[n] * self.p_orb_orient[n]
            }
        }
    }

    pub fn gen_planet_surface(&mut self, n: i32) {
        let n = n as usize;
        let is_moon = self.p_owner[n] > -1;
        let colorbase = if is_moon { 128 } else { 192 };
        let seedval = self.surface_seedval_for(n as i32);
        if is_moon {
            // swap p_background <-> s_background
            std::mem::swap(&mut self.p_background, &mut self.s_background);
            self.surface(n as i32, self.p_type[n] as i32, seedval, colorbase);
            std::mem::swap(&mut self.p_background, &mut self.s_background);
        } else {
            self.surface(n as i32, self.p_type[n] as i32, seedval, colorbase);
        }
    }

    // ===================== switch(type) cases =====================

    fn surf_case0(&mut self) {
        let r = self.frng.ranged(3) + 5;
        for _c in 0..r {
            self.ssmooth();
        }
        // cmp byte[di],28 ; jb low ; mov byte[di],62
        for i in 0..64800 {
            if self.p_background[i] >= 28 {
                self.p_background[i] = 62;
            }
        }
        let r = self.frng.ranged(5) + 5;
        for _c in 0..r {
            self.ssmooth();
        }
        let r = 5 + self.frng.ranged(26);
        for _c in 0..r {
            self.s_cr = 5 + self.frng.ranged(20);
            self.s_cx = self.frng.ranged(360);
            self.s_cy = self.frng.ranged(130) + 25;
            self.s_gr = self.frng.ranged(self.s_cr / 2) + self.s_cr / 2 + 2;
            self.volcano();
        }
        let r = 100 + self.frng.ranged(100);
        self.s_b = self.frng.ranged(3) + 1;
        self.s_g = 360;
        for _c in 0..r {
            self.s_cx = self.frng.ranged(360);
            self.s_cy = self.frng.ranged(180);
            self.s_gr = self.frng.ranged(100);
            self.fracture(180.0);
        }
        self.lssmooth();
    }

    fn surf_case1(&mut self) {
        if self.frng.ranged(2) != 0 {
            self.ssmooth();
        }
        let r = 10 + self.frng.ranged(41);
        self.s_r = r;
        self.crater_juice();
        self.lssmooth();
        if self.frng.ranged(5) == 0 {
            self.negate();
        }
    }

    fn surf_case2(&mut self) {
        let r = 5 + self.frng.ranged(25);
        for _c in 0..r {
            self.s_cr = self.frng.ranged(20) + 1;
            self.s_cy = self.frng.ranged(178 - 2 * self.s_cr) + self.s_cr;
            match self.rng.random(2) {
                0 => {
                    self.s_cx = (((10 * self.secs as i64) / (self.frng.ranged(3600) + 180) as i64) % 360) as i32;
                    self.s_gr = self.frng.ranged(12) + 2;
                    self.storm();
                }
                _ => {
                    self.s_gr = self.frng.ranged(15) + 3;
                    self.s_py = ((self.s_cy * 360)) as u16;
                    self.s_cr *= 360;
                    self.s_g = 1 + self.frng.ranged(self.s_gr);
                    self.band();
                }
            }
        }
        if self.frng.ranged(3) == 0 {
            self.negate();
        }
    }

    fn surf_case3(&mut self, seed: u16) {
        let r = self.frng.ranged(3) + 4;
        self.s_g = 26 + self.frng.ranged(3) - self.frng.ranged(5);
        for _c in 0..r {
            self.ssmooth();
        }
        // sda loop: compare threshold g
        let g = self.s_g;
        let mut ax: u16 = seed;
        for i in 0..64000usize {
            if self.p_background[i] >= (g as u8) {
                // terra branch: quadratic texture
                ax = ax.wrapping_add(64000u16);
                let prod: i32 = (ax as i16 as i32) * (ax as i16 as i32);
                let dx: u16 = ((prod >> 16) & 0xFFFF) as u16;
                ax = ax.wrapping_add(dx);
                let bl: u8 = (ax as u8) & 0x3E;
                let nv = self.p_background[i].wrapping_add(bl);
                self.p_background[i] = if nv < 0x3E { nv } else { 0x3E };
            } else {
                self.p_background[i] = 16;
            }
        }
        let r = 20 + self.frng.ranged(40);
        for _c in 0..r {
            self.s_gr = self.frng.ranged(5) + 1;
            self.s_cr = self.frng.ranged(10) + 10;
            if self.frng.ranged(3) != 0 {
                self.s_cy = self.frng.ranged(172 - 2 * self.s_cr) + self.s_cr + 2;
            } else {
                self.s_cy = 60 + self.frng.ranged(10) - self.frng.ranged(10);
            }
            self.s_cx = (((self.secs as i64) / (self.frng.ranged(360) + 180) as i64) % 360) as i32;
            self.s_g = self.frng.ranged(5) + 7;
            self.s_a = (self.frng.ranged(360) as f32 * DEG as f32) as f64;
            self.atm_cyclon();
        }
    }

    fn surf_case4(&mut self) {
        self.ssmooth();
        if self.frng.ranged(2) != 0 {
            self.ssmooth();
        }
        // lmrip loop: cmp byte[di],32 ; jne proxy ; mov word[di],0x3E01 ; mov byte[di+360],0x01
        for i in 0..64000usize {
            if self.p_background[i] == 32 {
                self.p_background[i] = 0x01;
                if i + 360 < self.p_background.len() {
                    self.p_background[i + 360] = 0x01;
                }
                // word 0x3E01 -> low byte 0x01 at i, high 0x3E at i+1
                if i + 1 < self.p_background.len() {
                    self.p_background[i + 1] = 0x3E;
                }
            }
        }
        let r = self.frng.ranged(30);
        let mut r = r;
        if r > 20 {
            r *= 10;
        }
        self.s_b = self.frng.ranged(3) + 1;
        self.s_g = 200 + self.frng.ranged(300);
        for _c in 0..r {
            self.s_cx = self.frng.ranged(360);
            self.s_cy = self.frng.ranged(180);
            self.s_gr = 50 + self.frng.ranged(100);
            self.fracture(180.0);
        }
        let r = self.frng.ranged(25) + 1;
        self.s_r = r;
        self.crater_juice();
        self.lssmooth();
        if self.frng.ranged(2) != 0 {
            self.lssmooth();
        }
        dbg_dump("C5_FIN", &self.p_background);
    }

    fn surf_case5(&mut self) {
        let r = self.frng.ranged(3) + 4;
        for _c in 0..r {
            self.ssmooth();
        }
        dbg_dump("C5_SM", &self.p_background);
        let c1 = self.frng.ranged(200) as f64 / 900.0 + 0.6;
        let c2 = self.frng.ranged(350) as f64 / 100.0 + 4.0;
        let c3 = (25 + self.frng.ranged(3)) as f64;
        if CURRENT_BODY.load(Ordering::Relaxed) == capture_body() {
            use std::io::Write;
            let _ = std::fs::OpenOptions::new().create(true).write(true).open("/tmp/rchk/C5ARGS.BIN")
                .and_then(|mut f| {
                    let ik1=(c1*1000.0) as i32; let ik2=(c2*1000.0) as i32; let t3=c3 as i32;
                    f.write_all(&[ik1 as u8,(ik1>>8) as u8,(ik1>>16) as u8,(ik1>>24) as u8, ik2 as u8,(ik2>>8) as u8,(ik2>>16) as u8,(ik2>>24) as u8, t3 as u8,(t3>>8) as u8,(t3>>16) as u8,(t3>>24) as u8])
                });
        }
        self.contrast(c1, c2, c3);
        dbg_dump("C5_CT", &self.p_background);
        let rf1 = 5 + self.frng.ranged(3);
        let rf2 = -20 * (self.frng.ranged(3) + 1);
        self.randoface(rf1, rf2);
        dbg_dump("C5_RF", &self.p_background);
        let r = 5 + self.frng.ranged(5);
        for _c in 0..r {
            self.s_cr = 5 + self.frng.ranged(10);
            self.s_cx = self.frng.ranged(360);
            self.s_cy = self.frng.ranged(145) + 15;
            self.s_gr = self.frng.ranged(self.s_cr / 2) + 2;
            self.volcano();
        }
        dbg_dump("C5_VB", &self.p_background);
        let r = 5 + self.frng.ranged(5);
        for _c in 0..r {
            self.s_cr = self.frng.ranged(30) + 1;
            self.s_cy = self.frng.ranged(178 - 2 * self.s_cr) + self.s_cr;
            self.s_cx = (((60 * self.secs as i64) / (self.frng.ranged(3600) + 360) as i64) % 360) as i32;
            self.s_gr = self.frng.ranged(2) + 1;
            self.permanent_storm();
        }
        dbg_dump("C5_PS", &self.p_background);
        for _c in 0..10000 {
            self.s_gr = self.frng.ranged(10) + 10;
            self.s_px = (self.frng.ranged(360)) as u16;
            self.s_py = ((self.frng.ranged(10) * 360)) as u16;
            self.spot();
            self.s_px = (self.frng.ranged(360)) as u16;
            self.s_py = (125 - self.frng.ranged(10)) as u16 * 360;
            self.spot();
        }
        dbg_dump("C5_SP", &self.p_background);
        if self.frng.ranged(2) != 0 {
            self.ssmooth();
        } else {
            self.lssmooth();
        }
        dbg_dump("C5_FIN", &self.p_background);
    }

    fn surf_case6(&mut self) {
        let r = 3 + self.frng.ranged(5);
        for _c in 0..r {
            self.ssmooth();
        }
        dbg_dump("1_SMO", &self.p_background);
        let r = 50 + self.frng.ranged(100);
        for _c in 0..r {
            self.s_cr = self.frng.ranged(10) + 1;
            self.s_cy = self.frng.ranged(178 - 2 * self.s_cr) + self.s_cr;
            let _rnd8 = self.frng.ranged(8);
            if _rnd8 != 0 {
                self.s_gr = self.frng.ranged(5) + 2;
                self.s_g = 1 + self.frng.ranged(self.s_gr);
                self.s_py = ((self.s_cy * 360)) as u16;
                self.s_cr *= 360;
                eprintln!("B cr={} cy={} gr={} g={} py={}", self.s_cr, self.s_cy, self.s_gr, self.s_g, self.s_py);
                self.band();
            } else {
                let raw = self.frng.ranged(10);
                self.s_a = ((5 + raw) as f32 / 30.0_f32) as f64;
                self.s_cr = self.s_cr / 4 + 1;
                eprintln!("W cr={} cy={} raw={} a={}", self.s_cr, self.s_cy, raw, self.s_a);
                self.wave();
            }
        }
        { dbg_dump("2_BW", &self.p_background); dbg_dump("2A_B", &self.objectschart); }
        let r = 50 + self.frng.ranged(100);
        for _c in 0..r {
            self.s_cr = self.frng.ranged(15) + 1;
            self.s_cy = self.frng.ranged(178 - 2 * self.s_cr) + self.s_cr;
            self.s_cx = (((60 * self.secs as i64) / (self.frng.ranged(8000) + 360) as i64) % 360) as i32;
            self.s_gr = self.frng.ranged(2) + 1;
            if self.frng.ranged(10) != 0 {
                self.s_cr = self.s_cr / 2 + 1;
            } else {
                self.s_gr *= 3;
            }
            self.storm();
        }
        { dbg_dump("3_ST", &self.p_background); dbg_dump("3A_S", &self.objectschart); }
        self.lssmooth();
        if self.frng.ranged(3) == 0 {
            self.negate();
        }
        { dbg_dump("4_LS", &self.p_background); dbg_dump("4A_L", &self.objectschart); }
    }

    fn surf_case7(&mut self) {
        let r = 5 + self.frng.ranged(5);
        for _c in 0..r {
            self.ssmooth();
        }
        let r = 10 + self.frng.ranged(50);
        self.s_g = 5 + self.frng.ranged(20);
        self.s_b = self.frng.ranged(2) + 1;
        for _c in 0..r {
            self.s_cx = self.frng.ranged(360);
            self.s_cy = self.frng.ranged(180);
            self.s_gr = self.frng.ranged(300);
            self.fracture(180.0);
        }
        if self.frng.ranged(2) != 0 {
            self.lssmooth();
        }
        let rface = 1 + self.frng.ranged(10);
        self.randoface(rface, 1);
        if self.frng.ranged(2) != 0 {
            self.negate();
        }
    }

    fn surf_case8(&mut self) {
        let r = self.frng.ranged(10) + 1;
        for _c in 0..r {
            self.lssmooth();
        }
        let r = 100 + self.frng.ranged(50);
        for _c in 0..r {
            self.s_cr = self.frng.ranged(5) + 1;
            self.s_gr = self.frng.ranged(5) + 1;
            self.s_cx = self.frng.ranged(360);
            self.s_cy = self.frng.ranged(178 - 2 * self.s_cr) + self.s_cr;
            self.permanent_storm();
        }
        if self.frng.ranged(2) != 0 {
            self.negate();
        }
    }

    fn surf_case9(&mut self) {
        self.pclear(0x1F);
        for px in 0..32400 {
            self.objectschart[px] = 0x1F;
        }
    }
}
