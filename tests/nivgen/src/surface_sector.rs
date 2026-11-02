//! Surface SECTOR generation: `build_surface()` and its helpers, ported
//! bug-for-bug from `source/NOCTIS-1.CPP` (lines 1494-3139).
//!
//! This reproduces the 200x200 heightmap (`p_surfacemap`) and the 40000-byte
//! object placement map (`objectschart`) for a single landing site. The C
//! source is ground truth; output must match bit-for-bit.
//!
//! Key porting notes:
//! * All locals the C declares as `float` round to f32 per assignment (the
//!   value is computed in f64 then stored back to f32). Accumulating in f64
//!   across statements drifts. See `f()` helper below and gotchas §22/§23.
//! * Borland `random(n)` / `srand` / `fast_srand` come from `self.rng` /
//!   `self.frng` (see `rng.rs`). `srand` truncates its seed to 16 bits.
//! * Function-argument RNG evaluation order is RIGHT-TO-LEFT (Borland pushes
//!   args right-to-left and evaluates side effects that way). When a call
//!   passes more than one `random()`/`flandom()` in its argument list, the
//!   Rust must consume them in REVERSE argument order. See the `round_hill`
//!   / `std_crater` call sites in `build_surface`.
//! * `objectschart` is an array of `quadrant` bitfields (NOCTIS-D.H:171):
//!   `nr_of_objects` (bits 0-1), `object0_class` (2-3), `object1_class`
//!   (4-5), `object2_class` (6-7). Borland lays bitfields out low-to-high,
//!   so one `quadrant` = exactly one byte. `ruinschart` is an alias of
//!   `objectschart` as `unsigned char` (raw byte writes, e.g. `= AF1`).
//! * `txtr` is aliased onto `p_background` (the C harness does
//!   `txtr = p_background`); the heightmap lives in the separate
//!   `p_surfacemap`. So texture writes here go into `self.p_background`.

use crate::f80::F80;
use crate::rng::BorlandRng;
use crate::system::SystemGen;
use std::f64::consts::PI;

const ST_BYTES: usize = 64800;

// Scenario types (NOCTIS-1.CPP:48 + harness NIVTEST.CPP:102).
const SC_OCEAN: i32 = 1;
const SC_PLAINS: i32 = 2;
const SC_DESERT: i32 = 3;
const SC_ICY: i32 = 4;
// make_ruins switches on the local OCEAN/PLAINS/DESERT/ICY (NOCTIS-1.CPP:47)
// which have the same numeric values as SC_*.
const OCEAN: i32 = 1;
const PLAINS: i32 = 2;
const DESERT: i32 = 3;
const ICY: i32 = 4;

// Object class codes (NOCTIS-D.H:148).
const ROCKS: i32 = 0;
const VEGET: i32 = 1;
const TREES: i32 = 2;

// Ruin markers (NOCTIS-D.H:155).
const AF1: u8 = 0x40;

/// M_PI_2 as f64 constant.
const M_PI_2: f64 = PI / 2.0;

/// Round an f64 to f32, mirroring a Borland `float` destination assignment
/// (the intermediate computation happens in double, then stores to float).
fn f(x: f64) -> f32 {
    x as f32
}

impl SystemGen {
    // ===================== objectschart bitfield access =====================
    // PORT-NOTE (NOCTIS-D.H:171): `struct quadrant` is four 2-bit fields packed
    // into a single byte, low-to-high: nr_of_objects(0-1), object0_class(2-3),
    // object1_class(4-5), object2_class(6-7).
    fn obj_nr(&self, ptr: usize) -> u8 {
        self.objectschart[ptr] & 3
    }
    fn obj_c0(&self, ptr: usize) -> u8 {
        (self.objectschart[ptr] >> 2) & 3
    }
    fn obj_c1(&self, ptr: usize) -> u8 {
        (self.objectschart[ptr] >> 4) & 3
    }
    fn obj_c2(&self, ptr: usize) -> u8 {
        (self.objectschart[ptr] >> 6) & 3
    }
    fn set_obj_nr(&mut self, ptr: usize, v: u8) {
        let b = self.objectschart[ptr] & !3;
        self.objectschart[ptr] = b | (v & 3);
    }
    fn set_obj_c0(&mut self, ptr: usize, v: u8) {
        let b = self.objectschart[ptr] & !(3 << 2);
        self.objectschart[ptr] = b | ((v & 3) << 2);
    }
    fn set_obj_c1(&mut self, ptr: usize, v: u8) {
        let b = self.objectschart[ptr] & !(3 << 4);
        self.objectschart[ptr] = b | ((v & 3) << 4);
    }
    fn set_obj_c2(&mut self, ptr: usize, v: u8) {
        let b = self.objectschart[ptr] & !(3 << 6);
        self.objectschart[ptr] = b | ((v & 3) << 6);
    }

    // `ruinschart[pt] = v` : raw byte write (alias onto objectschart).
    fn ruins_write(&mut self, pt: usize, v: u8) {
        self.objectschart[pt] = v;
    }

    // In the C far-heap, `p_surfacemap` (ps_bytes=40000) is farmalloc'd and
    // immediately followed by a 16-byte farmalloc heap header, then
    // `objectschart` (oc_bytes=40000). `build_surface`'s inclination loop reads
    // `p_surfacemap[ptr+200]` for ptr up to 39999 (indices up to 40199), which
    // fall in this header+objectschart region. The 16 header bytes are the
    // fixed Borland farmalloc header for this build (measured from the C
    // reference under DOSBox-X) and are deterministic. PORT-NOTE: mirrors the
    // SAMPLED_BYTES approach in surface.rs — the OOB tail is heap scratch, but
    // farmalloc gap bytes between p_surfacemap and objectschart.
    // Set per-body from C fixture dump; hardcoded default for legacy.
    // See bulk_sectors.py gen/load/rust pipeline.
    //
    fn surf_h(&self, i: usize) -> u8 {
        if i < 40000 {
            self.p_surfacemap[i]
        } else {
            let k = i - 40000;
            if k < self.oc_gap.len() {
                self.oc_gap[k]
            } else {
                let j = k - self.oc_gap.len();
                if j < self.objectschart.len() { self.objectschart[j] } else { 0 }
            }
        }
    }

    // ===================== helper functions =====================

    /// round_hill (NOCTIS-1.CPP:1494). `h`, `hmax` are float; `r` is unsigned.
    /// NB: in Borland C++, `r` is `unsigned`, so `cx - r` (and `cz - r`) is
    /// evaluated in **unsigned** arithmetic. When `cx < r` (or `cz < r`) the
    /// start bound underflows to a huge unsigned, so `x < cx + r` is false
    /// immediately and the loop body never executes. We must reproduce that
    /// unsigned comparison exactly, otherwise hills over-accumulate.
    fn round_hill(&mut self, cx: i32, cz: i32, r: u32, h: f32, hmax: f32, allowcanyons: i32) {
        let v: f32 = f((r as f64) / M_PI_2);
        let x0 = (cx as u32).wrapping_sub(r);
        let x1 = (cx as u32) + r;
        let z0 = (cz as u32).wrapping_sub(r);
        let z1 = (cz as u32) + r;
        let mut x = x0;
        while x < x1 {
            let mut z = z0;
            while z < z1 {
                let xi = x as i32;
                let zi = z as i32;
                if xi > -1 && zi > -1 && xi < 200 && zi < 200 {
                    let dx: f32 = f((xi - cx) as f64);
                    let dz: f32 = f((zi - cz) as f64);
                    let d: f32 = f((((dx as f64) * (dx as f64) + (dz as f64) * (dz as f64)).sqrt()));
                    // Compute cos(d/v)*h via F80 (80-bit extended precision) to match x87
                    let prod = F80::from_f64(d as f64)
                        .div(&F80::from_f64(v as f64))
                        .cos()
                        .mul(&F80::from_f64(h as f64));
                    let mut y: f32 = prod.to_f32();
                    if y >= 0.0 {
                        y = f(y as f64 + self.p_surfacemap[(200 * (zi as i64) + xi as i64) as usize] as f64);
                        if allowcanyons != 0 {
                            if y > 127.0 {
                                y = f(254.0 - y as f64);
                            }
                        } else {
                            if y > hmax {
                                y = hmax;
                            }
                        }
                        self.p_surfacemap[(200 * (zi as i64) + xi as i64) as usize] = y as u8;
                    }
                }
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
    }

    /// smoothterrain (NOCTIS-1.CPP:1530). 4-neighbour average (>>2).
    fn smoothterrain(&mut self, mut rounding: i32) {
        while rounding != 0 {
            let mut ptr = 0usize;
            while ptr < 39799 {
                let n = self.p_surfacemap[ptr] as i32
                    + self.p_surfacemap[ptr + 1] as i32
                    + self.p_surfacemap[ptr + 200] as i32
                    + self.p_surfacemap[ptr + 201] as i32;
                self.p_surfacemap[ptr] = (n >> 2) as u8;
                ptr += 1;
            }
            rounding -= 1;
        }
    }

    /// rockyground (NOCTIS-1.CPP:1545).
    fn rockyground(&mut self, roughness: i32, rounding: i32, level: i32) {
        for ptr in 0..40000 {
            self.p_surfacemap[ptr] = self.rng.random(roughness) as u8;
        }
        self.smoothterrain(rounding);
        for ptr in 0..40000 {
            if self.p_surfacemap[ptr] as i32 >= level.abs() {
                let mut v = self.p_surfacemap[ptr] as i32 + level;
                if v > 127 {
                    v = 127;
                }
                self.p_surfacemap[ptr] = v as u8;
            } else {
                self.p_surfacemap[ptr] = 0;
            }
        }
    }

    /// std_crater (NOCTIS-1.CPP:1561). `map` is 0 = p_surfacemap (align 200)
    /// or 1 = txtr/p_background (align 256).
    fn std_crater(
        &mut self,
        map: u8,
        cx: i32,
        cz: i32,
        r: i32,
        lim_h: i32,
        h_factor: f32,
        h_raiser: f32,
        align: i64,
    ) {
        let h: f32 = (r as f32) * h_factor;
        let r = r.abs();
        let fr: f32 = r as f32;
        let x0 = cx - r;
        let x1 = cx + r;
        let z0 = cz - r;
        let z1 = cz + r;
        let mut x = x0;
        while x < x1 {
            let mut z = z0;
            while z < z1 {
                if x > -1 && z > -1 && x < align as i32 && z < align as i32 {
                    let dx: f32 = (x - cx) as f32;
                    let dz: f32 = (z - cz) as f32;
                    let d: f32 = (((dx as f64) * (dx as f64) + (dz as f64) * (dz as f64)).sqrt()) as f32;
                    if d <= fr {
                        let mut y: f32 = ((((d as f64) / (fr as f64)) * PI).sin() * h as f64) as f32;
                        y = ((y as f64).powf(h_raiser as f64)) as f32;
                        let loc = (align * (z as i64) + x as i64) as usize;
                        let cur = match map {
                            0 => self.p_surfacemap[loc],
                            1 => self.p_background[loc],
                            _ => unreachable!(),
                        };
                        y = y + cur as f32;
                        if y < 0.0 {
                            y = 0.0;
                        }
                        if y > lim_h as f32 {
                            y = lim_h as f32;
                        }
                        match map {
                            0 => self.p_surfacemap[loc] = y as u8,
                            1 => self.p_background[loc] = y as u8,
                            _ => unreachable!(),
                        }
                    }
                }
                z += 1;
            }
            x += 1;
        }
    }

    /// srf_darkline (NOCTIS-1.CPP:1589). `map` 0/1 like std_crater.
    fn srf_darkline(&mut self, map: u8, mut length: i32, x_trend: i32, z_trend: i32, align: i64) {
        let mut fx = self.rng.random(align as i32) as i16 as i32;
        let mut fz = self.rng.random(align as i32) as i16 as i32;
        let mapsize = align * align;
        while length != 0 {
            fx = ((fx as i16).wrapping_add((self.rng.random(3) + x_trend) as i16)) as i32;
            fz = ((fz as i16).wrapping_add((self.rng.random(3) + z_trend) as i16)) as i32;
            let location = ((align * (fz as i64) + (fx as i64)) & 0xFFFF) as i64;
            if location > 0 && location < mapsize {
                let loc = location as usize;
                match map {
                    0 => self.p_surfacemap[loc] >>= 1,
                    1 => self.p_background[loc] >>= 1,
                    _ => unreachable!(),
                }
            }
            length -= 1;
        }
    }

    /// felisian_srf_darkline (NOCTIS-1.CPP:1605).
    fn felisian_srf_darkline(&mut self, mut length: i32, x_trend: i32, z_trend: i32, align: i64) {
        let mut fx = self.rng.random(align as i32) as i16 as i32;
        let mut fz = self.rng.random(align as i32) as i16 as i32;
        let mut deviation = self.rng.random(25) - 50;
        let mut variability = 2 + self.rng.random(10);
        let mapsize = align * align;
        while length != 0 {
            fx = ((fx as i16).wrapping_add((self.rng.random(3) + x_trend) as i16)) as i32;
            fz = ((fz as i16).wrapping_add((self.rng.random(3) + z_trend) as i16)) as i32;
            deviation += self.rng.random(variability) - (variability >> 1);
            // C uses `unsigned location` (16-bit), wraps large fz/fx.
            let location = ((align * (fz as i64) + (fx as i64)) & 0xFFFF) as i64;
            if location > 0 && location < mapsize {
                let loc = location as usize;
                let mut peak = self.p_surfacemap[loc] as i32;
                peak += deviation;
                if peak < 0 {
                    peak = 0;
                }
                if peak > 127 {
                    peak = 127;
                }
                let pk = peak as u8;
                // PORT-NOTE: the C writes map[location+-1] and map[location+-align]
                // unconditionally; location is in (0, mapsize=40000). For small
                // locations `location-align` underflows to a huge unsigned and
                // writes far memory that is NOT part of the hashed 80000-byte
                // region, so we only mirror writes that land inside the
                // heightmap (p_surfacemap[0..40000]) or in the objectschart
                // region (indices 40016..80015, which in C's heap layout is
                // directly after p_surfacemap+header).
                let loc_i = loc as i64;
                let neigh = [loc_i + 1, loc_i - 1, loc_i + align, loc_i - align];
                for &ni in &neigh {
                    if ni >= 0 && ni < 40000 {
                        self.p_surfacemap[ni as usize] = pk;
                    } else if ni >= 40000 && ni < 40016 {
                        // The 16-byte farmalloc header between p_surfacemap and
                        // objectschart. C scribbles over it here and the object
                        // loop later reads the scribbled bytes, so dropping these
                        // writes leaves the header at its initial value and changes
                        // the object chart (this is what made ~42 bodies differ).
                        self.oc_gap[(ni - 40000) as usize] = pk;
                    } else if ni >= 40016 && ni < 80016 {
                        // C writes past p_surfacemap land in the objectschart
                        // region (16-byte farmalloc header + objectschart data)
                        self.objectschart[(ni - 40016) as usize] = pk;
                    }
                }
                self.p_surfacemap[loc] = pk;
            }
            length -= 1;
        }
    }

    /// asterism (NOCTIS-1.CPP:1635). `map` = 1 (txtr).
    /// NB: `ad` and `ang` are PURE f32 (like C float). Accumulating `f(ang_f64 + ad_f64)`
    /// introduces double rounding and changes the number of loop iterations, which shifts
    /// the RNG state for all subsequent operations, breaking surftex matching.
    fn asterism(&mut self, x: i32, y: i32, base: i32, variation: i32, density: i32, size: i32, align: i64) {
        if density <= 0 {
            return;
        }
        let ad: f32 = f(PI * 2.0 / (density as f64));
        let mut ang: f32 = 0.0;
        while (ang as f64) < PI * 2.0 {
            let mut shift_d: f32 = f((self.rng.random(1000) as f64) / 1000.0);
            shift_d = f(shift_d as f64 * size as f64);
            if (shift_d as f64) >= 1.0 {
                let var: f32 = f((variation as f64) / shift_d as f64);
                let mut color: f32 = base as f32;
                let mut sd: f32 = shift_d;
                while sd > 0.0f32 {
                    let shift_x: i64 = ((ang as f64).cos() * sd as f64 + x as f64).trunc() as i64;
                    let shift_y: i64 = ((ang as f64).sin() * sd as f64 + y as f64).trunc() as i64;
                    let shift_x = shift_x as i32;
                    let shift_y = shift_y as i32;
                    if shift_x > 0 && shift_y > 0 && shift_x < align as i32 && shift_y < align as i32 {
                        let shift_p = shift_y as i64 * align + shift_x as i64;
                        self.p_background[shift_p as usize] = color as u8;
                    }
                    color = f(color as f64 + var as f64);
                    sd -= 1.0f32;
                }
            }
            ang += ad; // pure f32 accumulation, matching C
        }
    }

    // ===================== make_ruins (NOCTIS-1.CPP:1767) =====================

    fn average_of_y(&self, ic: i32, jc: i32, ra: i32) -> i32 {
        let mut av = 0i32;
        let mut ai = 1i32;
        let mut jp = jc - ra;
        while jp < jc + ra {
            let mut ip = ic - ra;
            while ip < ic + ra {
                av += self.p_surfacemap[self.m200[jp as usize] as usize + ip as usize] as i32;
                ai += 1;
                ip += 1;
            }
            jp += 1;
        }
        av / ai
    }

    #[allow(clippy::too_many_arguments)]
    fn make_ruins(&mut self, style1: i32, style2: i32, style3: i32, style4: i32, style5: i32, density: i32) {
        let mut buildings = 0i32;
        // PORT-NOTE: C `switch (sctype)` — OCEAN/PLAINS/DESERT/ICY (1..4).
        match self.sctype {
            OCEAN => {
                buildings = self.rng.random(4);
                if self.rng.random(10) == 0 {
                    buildings *= 2 + self.rng.random(2);
                }
            }
            PLAINS => {
                buildings = self.rng.random(8);
                if self.rng.random(5) == 0 {
                    buildings *= 3 + self.rng.random(2);
                }
            }
            DESERT => {
                buildings = self.rng.random(3);
            }
            ICY => {
                buildings = self.rng.random(2);
            }
            _ => {}
        }

        let mut peak: i32;
        let mut bstyle: i32;
        let mut ic: i32;
        let mut jc: i32;
        let mut ip: i32;
        let mut jp: i32;
        let mut ra: i32;
        let mut hr: i32;
        let mut pt: usize;
        let mut av: i32;

        buildings *= density;
        while buildings != 0 {
            let ai_sel = self.rng.random(5);
            bstyle = match ai_sel {
                0 => style1,
                1 => style2,
                2 => style3,
                3 => style4,
                _ => style5,
            };
            match bstyle {
                // Stili Felisiani antichi.
                0 | 3 => {
                    // cases 0 and 3 share the `like0` block.
                    hr = if bstyle == 0 {
                        10 + self.rng.random(25)
                    } else {
                        10 + self.rng.random(10)
                    };
                    ic = 20 + self.rng.random(160);
                    jc = 20 + self.rng.random(160);
                    ra = if bstyle == 0 { 1 + self.rng.random(3) } else { 1 + self.rng.random(2) };
                    av = self.average_of_y(ic, jc, ra);
                    jp = jc - ra;
                    while jp < jc + ra {
                        ip = ic - ra;
                        while ip < ic + ra {
                            pt = (self.m200[jp as usize] as usize) + ip as usize;
                            self.p_surfacemap[pt] = (av + hr) as u8;
                            if self.rng.random(5) == 0 {
                                self.p_surfacemap[pt] -= 1;
                            }
                            self.ruins_write(pt, AF1);
                            ip += 1;
                        }
                        jp += 1;
                    }
                }
                1 => {
                    hr = self.rng.random(8);
                    ic = 20 + self.rng.random(160);
                    jc = 20 + self.rng.random(160);
                    ra = 3 + self.rng.random(5);
                    ip = ic - ra;
                    while ip < ic + ra {
                        pt = (self.m200[(jc - ra) as usize] as usize) + ip as usize;
                        av = hr + self.rng.random(2);
                        self.p_surfacemap[pt] = (self.p_surfacemap[pt] as i32 + av) as u8;
                        if av != 0 {
                            self.ruins_write(pt, AF1);
                        }
                        pt = (self.m200[(jc + ra) as usize] as usize) + ip as usize;
                        av = hr + self.rng.random(3);
                        self.p_surfacemap[pt] = (self.p_surfacemap[pt] as i32 + av) as u8;
                        if av != 0 {
                            self.ruins_write(pt, AF1);
                        }
                        ip += 1;
                    }
                    jp = jc - ra;
                    while jp < jc + ra {
                        pt = (self.m200[jp as usize] as usize) + (ic - ra) as usize;
                        av = hr + self.rng.random(2);
                        self.p_surfacemap[pt] = (self.p_surfacemap[pt] as i32 + av) as u8;
                        if av != 0 {
                            self.ruins_write(pt, AF1);
                        }
                        pt = (self.m200[jp as usize] as usize) + (ic + ra) as usize;
                        av = hr + self.rng.random(3);
                        self.p_surfacemap[pt] = (self.p_surfacemap[pt] as i32 + av) as u8;
                        if av != 0 {
                            self.ruins_write(pt, AF1);
                        }
                        jp += 1;
                    }
                }
                2 => {
                    hr = 5 + self.rng.random(10);
                    ic = 30 + self.rng.random(140);
                    jc = 30 + self.rng.random(140);
                    ra = 15 + self.rng.random(15);
                    av = self.average_of_y(ic, jc, ra);
                    jp = jc - ra;
                    while jp < jc + ra {
                        ip = ic - ra;
                        while ip < ic + ra {
                            pt = (self.m200[jp as usize] as usize) + ip as usize;
                            self.p_surfacemap[pt] = av as u8;
                            ip += 1;
                        }
                        jp += 1;
                    }
                    ra /= 2;
                    if self.rng.random(3) != 0 {
                        let mut ai = 2 + self.rng.random(3);
                        ip = ic - ra;
                        while ip < ic + ra {
                            pt = (self.m200[(jc - ra) as usize] as usize) + ip as usize;
                            self.p_surfacemap[pt] = (av + hr + self.rng.random(2)) as u8;
                            self.ruins_write(pt, AF1);
                            pt = (self.m200[(jc + ra) as usize] as usize) + ip as usize;
                            self.p_surfacemap[pt] = (av + hr + self.rng.random(3)) as u8;
                            self.ruins_write(pt, AF1);
                            ip += ai;
                        }
                    }
                    if self.rng.random(3) != 0 {
                        let mut ai = 2 + self.rng.random(3);
                        jp = jc - ra;
                        while jp < jc + ra {
                            pt = (self.m200[jp as usize] as usize) + (ic - ra) as usize;
                            self.p_surfacemap[pt] = (av + hr + self.rng.random(2)) as u8;
                            self.ruins_write(pt, AF1);
                            pt = (self.m200[jp as usize] as usize) + (ic + ra) as usize;
                            self.p_surfacemap[pt] = (av + hr + self.rng.random(3)) as u8;
                            self.ruins_write(pt, AF1);
                            jp += ai;
                        }
                    }
                }
                4 => {
                    hr = 1 + self.rng.random(3);
                    ic = 25 + self.rng.random(150);
                    jc = 25 + self.rng.random(150);
                    ra = 5 + self.rng.random(5);
                    if self.rng.random(5) == 0 {
                        ra *= 2;
                    }
                    ip = ic - ra;
                    while ip < ic + ra {
                        pt = (self.m200[jc as usize] as usize) + ip as usize;
                        self.p_surfacemap[pt] =
                            (self.p_surfacemap[pt] as i32 + hr * (ra - (ip - ic).abs() + 1)) as u8;
                        self.ruins_write(pt, AF1);
                        ip += 1;
                    }
                    jp = jc - ra;
                    while jp < jc + ra {
                        pt = (self.m200[jp as usize] as usize) + ic as usize;
                        self.p_surfacemap[pt] =
                            (self.p_surfacemap[pt] as i32 + hr * (ra - (jp - jc).abs() + 1)) as u8;
                        self.ruins_write(pt, AF1);
                        jp += 1;
                    }
                }
                5 => {
                    hr = 10 + self.rng.random(10);
                    ic = 20 + self.rng.random(160);
                    jc = 20 + self.rng.random(160);
                    ra = 12 + self.rng.random(8);
                    av = self.average_of_y(ic, jc, ra);
                    jp = jc - ra;
                    while jp < jc + ra {
                        ip = ic - ra;
                        while ip < ic + ra {
                            pt = (self.m200[jp as usize] as usize) + ip as usize;
                            self.p_surfacemap[pt] = (av + hr) as u8;
                            self.ruins_write(pt, AF1);
                            ip += 1;
                        }
                        jp += 1;
                    }
                    ra = ra * 2;
                    ra /= 3;
                    let bstyle2 = self.rng.random(2);
                    jp = jc - ra;
                    while jp < jc + ra {
                        ip = ic - ra;
                        while ip < ic + ra {
                            pt = (self.m200[jp as usize] as usize) + ip as usize;
                            let ai2 = (jp - jc) * (jp - jc) + (ip - ic) * (ip - ic);
                            let ai2 = (ai2 as f64).sqrt();
                            // cos(M_PI_2 * (double)ai / (double)ra): ra is int.
                            let c: f64 = ((M_PI_2) * ai2 / ra as f64).cos();
                            peak = (3 * hr) as i32;
                            peak = (peak as f64 * c) as i32;
                            if peak > 0 {
                                self.p_surfacemap[pt] = (av + peak + 1) as u8;
                            }
                            if bstyle2 != 0 {
                                self.ruins_write(pt, AF1);
                            } else {
                                self.ruins_write(pt, 0);
                            }
                            ip += 1;
                        }
                        jp += 1;
                    }
                }
                _ => {}
            }
            buildings -= 1;
        }
    }

    // ===================== build_surface (NOCTIS-1.CPP:1948) =====================

    /// Replicate the harness `setup_landing` (NIVTEST.CPP:485) which feeds the
    /// landing-context globals into `build_surface`. This is harness setup,
    /// not generation logic: it mirrors exactly the assignments the C harness
    /// makes before calling `build_surface()`.
    pub fn setup_landing(&mut self, n: i32, lon: i32, lat: i32, albedo_arg: i32, sctype_arg: i32) {
        let n_idx = n as usize;
        self.ip_targetted = n;
        self.landing_pt_lon = lon;
        self.landing_pt_lat = lat;
        self.landing_point = 1;

        // latitude = |landing_pt_lat - 60| * 1.5  (float)
        self.latitude = f(((lat - 60).abs() as f64) * 1.5);

        // nightzone: C harness defaults to 0 (via argl(..., "-night", 0)).
        // The harness does NOT compute from terminator like the game does.
        self.nightzone = 0;

        // albedo: caller supplies 0..63; default 32
        self.albedo = if albedo_arg >= 0 { albedo_arg } else { 32 };

        // sun geometry (fixed, deterministic) as harness sets it
        self.dsd = 1000.0;
        self.dsd1 = 1000.0;
        self.dsd2 = 1000.0;
        self.nray1 = self.nearstar_ray as f64;
        self.nray2 = self.nearstar_ray as f64;
        self.sun_x = -1000.0;
        self.sun_y = 0.0;
        self.sun_z = 0.0;

        // global_surface_seed exactly as planetary_main derives it
        let seed_p_ray = self.p_ray[n_idx];
        let seed_p_orb_ray = self.p_orb_ray[n_idx];
        let seed_p_orb_orient = self.p_orb_orient[n_idx];
        self.global_surface_seed = ((seed_p_ray + seed_p_orb_ray + seed_p_orb_orient)
            * 4112.0)
            .trunc() as i64;
        if self.p_type[n_idx] as i32 == 3 {
            self.rng.srand((self.global_surface_seed + lon as i64) as u32);
            if (self.latitude as f64) > 25.0 + (self.global_surface_seed % 15) as f64 + self.rng.random(5) as f64 {
                self.global_surface_seed += 1;
            }
        }

        // scenario type
        if sctype_arg >= 1 && sctype_arg <= 4 {
            self.sctype = sctype_arg;
        } else if self.p_type[n_idx] as i32 == 3 {
            self.rng.srand((lon * lat) as u32);
            if self.rng.random(100) > 5 {
                let cpos = (555.0 * self.p_orb_orient[n_idx]) as i64;
                self.sctype = ((cpos % 4) + 1) as i32;
            } else {
                self.sctype = self.rng.random(4) + 1;
            }
            if self.albedo < 25 {
                self.sctype = SC_OCEAN;
            }
            if (self.latitude as f64) > 75.0 {
                self.sctype = SC_ICY;
            }
            if (self.latitude as f64) > 60.0 && self.rng.random(3) != 0 {
                self.sctype = SC_ICY;
            }
        } else {
            self.sctype = SC_PLAINS; // unused by non-felisian paths
        }
    }

    pub fn build_surface_with_gap(&mut self, gap_hex: &str) {
        if gap_hex.len() == 32 {
            let mut gap = [0u8; 16];
            for i in 0..16 {
                gap[i] = u8::from_str_radix(&gap_hex[2*i..2*i+2], 16).unwrap_or(self.oc_gap[i]);
            }
            eprintln!("[GAP] setting oc_gap from hex: {:02x?}", gap);
            self.oc_gap = gap;
        } else {
            eprintln!("[GAP] keeping default oc_gap: {:02x?} (hex={:?})", self.oc_gap, gap_hex);
        }
        self.build_surface();
    }

    pub fn build_surface(&mut self) {
        let n_idx = self.ip_targetted as usize;

        let mut cz: i32;
        let mut cx: i32;
        let mut cr: i32;
        let mut n: i32;
        let mut incl: i32;
        let mut hf: f32;
        let mut hr: f32;
        let mut ht: f32;
        let mut ptr1: u32;

        self.snowy = 0;
        self.frosty = 0;
        self.waswet = 0;
        self.waves_in = 0;
        self.waves_out = 0;
        self.t_scale = 32;
        self.quartz = 0;
        self.gtx = 1; // suolo con texture, per default

        // _fmemset(txtr, 16, 65535): due to far-pointer segment:offset wrapping
        // in Borland C, only 65532 bytes (indices 0..65531) are written. The
        // last 4 bytes (65532-65535) remain as farmalloc stale content (0x00
        // in DOSBox — not 0xFF from freed-page fill).
        for i in 0..65532 {
            self.p_background[i] = 16;
        }
        // Tail bytes 65532-65535: farmalloc stale in DOSBox = 0x00
        for i in 65532..65536 {
            self.p_background[i] = 0;
        }
        // _fmemset(p_surfacemap, 0, ps_bytes)
        for i in 0..40000 {
            self.p_surfacemap[i] = 0;
        }
        // _fmemset(objectschart, 0, oc_bytes)
        for i in 0..40000 {
            self.objectschart[i] = 0;
        }

        // regolazione dei parametri generali
        self.frng.srand(self.global_surface_seed as u32);
        self.rng.srand(self.global_surface_seed as u32);
        // trace: print seed and first 12 RNG values (NOT consuming from main rng)
        {
            let mut tr = BorlandRng::new();
            tr.srand(self.global_surface_seed as u32);
            let mut seq = Vec::with_capacity(12);
            for _ in 0..12 { seq.push(tr.rand()); }
            eprintln!("[TRACE] global_surface_seed={} rng_seq={:?}",
                self.global_surface_seed, seq);
        }

        self.groundflares = 0;

        cz = self.rng.random(2);
        cx = self.rng.random(100);
        if cx > 97 {
            self.groundflares = 2 + (2 * cz);
        }
        if cx > 45 && cx < 55 {
            if self.p_type[n_idx] as i32 == 3 && (self.latitude as f64) > 75.0 {
                self.groundflares = 2 + (2 * cz);
            }
        }

        let oc_bytes = 40000usize;
        let mut ptr = 0usize;
        while ptr < oc_bytes {
            self.set_obj_c0(ptr, ROCKS as u8);
            self.set_obj_c1(ptr, ROCKS as u8);
            self.set_obj_c2(ptr, ROCKS as u8);
            ptr += 1;
        }

        // gli alberi
        if (self.latitude as f64) > 45.0 {
            self.treepeaking = f(self.rng.flandom() * 0.9 + 0.1);
        } else {
            self.treepeaking = f(self.rng.flandom() + 0.75);
        }
        match self.rng.random(3) {
            0 => self.rootshade = 0x00,
            1 => self.rootshade = 0x80,
            _ => self.rootshade = 0xC0,
        }
        match self.rng.random(30) {
            7 => self.treeflares = 1,
            8 => self.treeflares = 2,
            9 => self.treeflares = 4,
            _ => self.treeflares = 0,
        }
        match self.rng.random(15) {
            7 => self.leafflares = 1,
            8 => self.leafflares = 2,
            9 => self.leafflares = 4,
            _ => self.leafflares = 0,
        }
        // C comma-operator fix: `{ float _r = flandom(); float _l = flandom(); ... }`
        // evaluates _r (right operand) FIRST, then _l (left operand) SECOND.
        let _ts_r = self.rng.flandom() as f32;  // right flandom (C _r), evaluated first
        let _ts_l = self.rng.flandom() as f32;  // left flandom (C _l), evaluated second
        self.treescaling = 3000.0f32 + _ts_l * 3000.0f32 - _ts_r * 1500.0f32;
        let _sp_l = self.rng.flandom();
        let _sp_r = self.rng.flandom();
        self.treespreads = f(0.75 + _sp_l * 0.50 - _sp_r * 0.50);
        self.branchwidth = f(0.05 + self.rng.flandom() * 0.15);
        self.rootheight = f(0.05 + self.rng.flandom());

        self.mushscaling = 4095;
        if (self.treescaling as f64) > 4096.0 {
            self.mushscaling = 8191;
        }

        if self.p_type[n_idx] as i32 == 3 {
            if self.sctype != ICY && self.sctype != DESERT {
                if self.rng.random(4) != 0 {
                    self.groundflares = 8;
                }
            }
        }

        // C computes landing_pt_lat * landing_pt_lon as 16-bit int
        // (Borland int is 16 bits). 100 * 359 = 35900 overflows signed
        // 16-bit to -29636, which is sign-extended to 32-bit 0xFFFF8C3C.
        let land_seed = ((self.landing_pt_lat as i16).wrapping_mul(self.landing_pt_lon as i16)) as i32 as u32;
        let ptype = self.p_type[n_idx] as i32;
        self.frng.srand(land_seed);
        self.rng.srand(land_seed);
        eprintln!("[TRACE] land_seed={} ptype={}", land_seed, ptype);
        match ptype {
            // case 0: vulcanico, non considerato.

            1 => {
                // rocciosi (stile luna)

                n = self.rng.random(5);
                if n <= 2 {
                    let a = 4 + self.rng.random(4);
                    self.rockyground(25, a, 0);
                }
                if n == 3 {
                    let a = 5 + self.rng.random(5);
                    self.rockyground(a, 1, 1);
                }
                if n == 4 {
                    let c = -self.rng.random(5);
                    self.rockyground(10, 2, c);
                }
                n = self.rng.random(48) + 32 - self.albedo;
                if n > 30 {
                    n = 30;
                }
                if n < 0 {
                    n = 0;
                }
                while n != 0 {
                    hf = f(self.rng.random(32) as f64 * 0.01);
                    hr = f((self.rng.random(20) + 5) as f64 * 0.075);
                    // std_crater(p_surfacemap, random(200), random(200),
                    //            random(50)+5, 127, hf, hr, 200)
                    // right-to-left: r, then cz, then cx.
                    let a_r = self.rng.random(50) + 5;
                    let a_cz = self.rng.random(200);
                    let a_cx = self.rng.random(200);
                    self.std_crater(0, a_cx, a_cz, a_r, 127, hf, hr, 200);
                    n -= 1;
                }
                n = self.rng.random(48) + 64 - self.albedo;
                if n < 0 {
                    n = 0;
                }
                hf = 0.35;
                if self.nightzone != 0 {
                    hf = 0.1;
                }
                while n != 0 {
                    cx = self.rng.random(200);
                    cz = self.rng.random(200);
                    cr = self.rng.random(32) + 10;
                    self.std_crater(1, cx, cz, cr, 31, f(hf as f64), 1.0, 256);
                    if cr % 2 != 0 {
                        let ccx = cx + cr / 3;
                        let ccz = cz + cr / 3;
                        let ccr = -cr;
                        self.std_crater(1, ccx, ccz, ccr, 31, f(hf as f64), 1.0, 256);
                    }
                    n -= 1;
                }
                n = self.rng.random(100);
                while n != 0 {
                    let r = self.rng.random(1000);
                    self.srf_darkline(1, r, -1, -1, 256);
                    n -= 1;
                }
                // C evaluates right-to-left: right random(2) first, then left.
                let _rd_r = self.rng.random(2);
                let _rd_l = self.rng.random(2);
                self.rockdensity = (15 + 16 * _rd_l) * _rd_r;
                self.rockscaling = f(150.0 + self.rng.random(500) as f64);
                self.rockpeaking = f(100.0 + self.rng.random(300) as f64);
            }

                2 => {
                // con spessa atmosfera (venusiani)
                self.rockyground(10, 1, 0);
                let rng_100 = self.rng.random(100);
                n = self.albedo + rng_100;
                eprintln!("[R_RNG] T2 random(100)={}", rng_100);
                let first_loop = n.min(1);
                if first_loop > 0 {
                    let a_h = self.rng.random(50) + 10;
                    let a_hmax = 0;
                    let a_r = self.rng.random(100) + 50;
                    let a_cz = self.rng.random(200);
                    let a_cx = self.rng.random(200);
                    eprintln!("[R_RNG] T3 random(50)={} random(100)={} random(200)={} random(200)={}",
                        a_h - 10, a_r - 50, a_cz, a_cx);
                    self.round_hill(a_cx, a_cz, a_r as u32, f(a_h as f64), f(a_hmax as f64), 1);
                    n -= 1;
                }
                while n != 0 {
                    let a_h = self.rng.random(50) + 10;
                    let a_hmax = 0;
                    let a_r = self.rng.random(100) + 50;
                    let a_cz = self.rng.random(200);
                    let a_cx = self.rng.random(200);
                    self.round_hill(a_cx, a_cz, a_r as u32, f(a_h as f64), f(a_hmax as f64), 1);
                    n -= 1;
                }
                let sw = self.rng.random(2);
                eprintln!("[R_RNG] T4 random(2)={}", sw);
                match sw {
                        0 => {
                        // C evaluates arithmetic left-to-right: random(200) first, then random(100)
                        // (same as case 7's identical expression at line ~1269; type-7 matches 100%).
                        // Commit 807e0a45 wrongly generalized function-arg RTL to binary -/+.
                        let _r200 = self.rng.random(200);
                        let _r100 = self.rng.random(100);
                        eprintln!("[R_RNG] T5 random(200)={} random(100)={}", _r200, _r100);
                        n = self.albedo + _r200 - _r100;
                        let hf_raw = self.rng.random(10);
                        eprintln!("[R_RNG] T6 random(10)={}", hf_raw);
                        hf = f(hf_raw as f64 * 0.02);
                        if n < 0 {
                            n = 0;
                        }
                        let first_cr = n.min(1);
                        if first_cr > 0 {
                            cx = self.rng.random(256);
                            cz = self.rng.random(256);
                            cr = self.rng.random(8) + 8;
                            let sw2 = self.rng.random(2);
                            eprintln!("[R_RNG] T7 random(256)={} random(256)={} random(8)={} random(2)={}",
                                cx, cz, cr - 8, sw2);
                            if sw2 != 0 {
                                self.std_crater(1, cx, cz, -cr, 31, hf, 1.0, 256);
                            } else {
                                self.std_crater(1, cx, cz, cr, 31, hf, 1.0, 256);
                            }
                            n -= 1;
                        }
                        while n != 0 {
                            cx = self.rng.random(256);
                            cz = self.rng.random(256);
                            cr = self.rng.random(8) + 8;
                            if self.rng.random(2) != 0 {
                                self.std_crater(1, cx, cz, -cr, 31, hf, 1.0, 256);
                            } else {
                                self.std_crater(1, cx, cz, cr, 31, hf, 1.0, 256);
                            }
                            n -= 1;
                        }
                    }
                    _ => {
                        n = self.albedo + self.rng.random(500);
                        ptr = self.rng.random(2000) as usize;
                        eprintln!("[R_RNG] T11 random(500)={} random(2000)={}", n - self.albedo, ptr);
                        while n != 0 {
                            let r = self.rng.random(ptr as i32);
                            self.srf_darkline(1, r, -1, -1, 256);
                            n -= 1;
                        }
                    }
                }
                let rs = self.rng.random(500);
                let rd = self.rng.random(2);
                let rp = self.rng.random(150);
                eprintln!("[R_RNG] T8 random(500)={} T9 random(2)={} T10 random(150)={}", rs, rd, rp);
                self.rockscaling = f(500.0 + rs as f64);
                self.rockdensity = 7 + 8 * rd;
                self.rockpeaking = f(50.0 + rp as f64);
            }

            3 => {
                // abitabili
                self.gtx = 0;

                match self.sctype {
                    SC_OCEAN => {
                        self.waves_in = 1;
                        self.waves_out = 1;
                        // Mirrors the C switch's `goto revert` / `goto addtrees`,
                        // which jump OUT of the OCEAN case and skip the sandy
                        // texture block below.
                        'ocean: {
                            if self.albedo > 20 {
                                self.waswet = 1;
                                self.waves_in = 0;
                                // goto revert (revert + addtrees + break)
                                self.oc_revert();
                                break 'ocean;
                            }
                            if self.albedo > 16 && self.rng.random(2) != 0 {
                                let a = self.rng.random(2);
                                self.rockyground(10, a, -5);
                                self.waves_in = 0;
                            }
                            if self.rng.random(3) == 0 {
                                cx = self.rng.random(100) + 50;
                                cz = self.rng.random(100) + 50;
                                let a_h = self.rng.random(10) + 1;
                                let a_r = self.rng.random(100) + 25;
                                let a_cz = cz + self.rng.random(15);
                                let a_cx = cx + self.rng.random(15);
                                self.round_hill(a_cx, a_cz, a_r as u32, f(a_h as f64), 0.0_f32, 1);
                                let b_h = self.rng.random(100) + 1;
                                let b_r = self.rng.random(100) + 25;
                                self.round_hill(cx, cz, b_r as u32, f(b_h as f64), 0.0_f32, 1);
                                self.waswet = 1;
                                // goto addtrees (addtrees + break)
                                self.oc_addtrees();
                                break 'ocean;
                            }
                            // texture sabbiosa
                            self.t_scale = 128;
                            n = self.rng.random(30) + 2;
                            ptr = 65535;
                            while ptr != 0 {
                                self.p_background[ptr] = self.rng.random(n) as u8;
                                ptr -= 1;
                            }
                            self.rockscaling = f(50.0 + self.rng.random(75) as f64);
                            self.rockpeaking = f(30.0 + self.rng.random(25) as f64);
                            if self.rng.random(3) != 0 {
                                self.rockdensity = 31;
                            } else {
                                self.rockdensity = 0;
                            }
                            self.liquid_water = 1;
                        }
                    }

                    SC_PLAINS => {
                        self.oc_revert();
                    }

                    SC_DESERT => {
                        n = self.rng.random(100);
                        self.rockyground(50 + n, 5 + (n >> 4), 0);
                        self.t_scale = 128;
                        ptr = 65535;
                        while ptr != 0 {
                            self.p_background[ptr] = self.rng.random(32) as u8;
                            ptr -= 1;
                        }
                        self.rockdensity = 0;
                        self.gtx = 1;
                    }

                    SC_ICY => {
                        self.snowy = 0;
                        self.frosty = 0;
                        match self.rng.random(4) {
                            0 => {
                                self.rockyground(15, 5, 0);
                                self.snowy = 1;
                            }
                            1 => {
                                let a_r2 = self.rng.random(2);
                                let a_r10 = self.rng.random(10);
                                self.rockyground(10 + a_r10, 1 + a_r2, 0);
                                self.frosty = 1;
                            }
                            2 => {
                                ptr = (self.rng.random(50) + 50) as usize;
                                while ptr != 0 {
                                    let a_h = self.rng.random(75) + 1;
                                    let a_r = self.rng.random(200) + 1;
                                    let a_cz = self.rng.random(200);
                                    let a_cx = self.rng.random(200);
                                    self.round_hill(a_cx, a_cz, a_r as u32, f(a_h as f64), 0.0_f32, 1);
                                    ptr -= 1;
                                }
                                self.snowy = 1;
                            }
                            _ => {
                                let a_40 = self.rng.random(40);
                                let a_3 = self.rng.random(3);
                                let a_50 = self.rng.random(50);
                                self.rockyground(50 + a_50, 3 + a_3, -(a_40 + 20));
                                self.frosty = 1;
                            }
                        }
                        self.rockscaling = f(200.0 + self.rng.random(500) as f64);
                        self.rockpeaking = f(150.0 + self.rng.random(250) as f64);
                        self.rockdensity = 2 * self.rng.random(2);
                        if self.snowy != 0 || self.frosty != 0 {
                            self.t_scale = 32;
                            n = self.rng.random(16) + 16;
                            ptr = 65535;
                            while ptr != 0 {
                                self.p_background[ptr] = self.rng.random(n) as u8;
                                ptr -= 1;
                            }
                            n = 1 + self.rng.random(3);
                            while n != 0 {
                                ptr = 65535 - 257;
                                while ptr != 0 {
                                    let cxv = self.p_background[ptr] as i32
                                        + self.p_background[ptr + 1] as i32
                                        + self.p_background[ptr + 256] as i32
                                        + self.p_background[ptr + 257] as i32;
                                    self.p_background[ptr] = (cxv >> 2) as u8;
                                    ptr -= 1;
                                }
                                n -= 1;
                            }
                        }
                        if self.frosty != 0 {
                            self.t_scale = 16 + self.rng.random(48) as i64;
                            n = self.rng.random(250);
                            while n != 0 {
                                let a_z = self.rng.random(2);
                                let a_x = self.rng.random(200);
                                self.srf_darkline(1, 100 + a_x, -a_z, 0, 256);
                                n -= 1;
                            }
                        }
                    }

                    _ => {}
                }

                if self.rng.random(5) != 0 {
                    self.quartz = 1;
                }
            }

            4 => {
                // medie dimensioni, pietroso, corrugato
                let a_l5 = self.rng.random(5);
                let a_r3 = self.rng.random(3);
                self.rockyground(15, 3 + a_r3, -a_l5);
                n = self.rng.random(15);
                while n != 0 {
                    hf = f((self.rng.random(15) + 7) as f64);
                    hr = f(hf as f64 * (self.rng.flandom() * 3.5 + 3.5));
                    ht = f(hr as f64 * (self.rng.flandom() * 0.2 + 0.3));
                    if (ht as f64) > 127.0 {
                        ht = f(127.0);
                    }
                    let a_r = hf as i32;
                    let a_cz = self.rng.random(200);
                    let a_cx = self.rng.random(200);
                    self.round_hill(a_cx, a_cz, a_r as u32, hr, ht, 0);
                    n -= 1;
                }
                let a_s2 = self.rng.random(2);
                self.smoothterrain(1 + a_s2);
    n = 64 - self.albedo;
    if self.nightzone != 0 {
        hf = f(0.50);
    } else {
        hf = f(0.25);
    }
                while n != 0 {
                    cx = self.rng.random(150) + 25;
                    cz = self.rng.random(150) + 25;
                    cr = self.rng.random(10) + 15;
                self.std_crater(1, cx, cz, -cr, 31, hf, 1.0, 256);
                n -= 1;
            }
                self.rockscaling = f(100.0 + self.rng.random(200) as f64);
                self.rockdensity = 3 + 4 * self.rng.random(2);
                self.rockpeaking = f(100.0 + self.rng.random(200) as f64);
            }

            5 => {
                // atmosfera sottile (marte)
                if self.rng.random(2) != 0 {
                    n = 5 + self.rng.random(10);
                    if self.albedo > 48 {
                        n /= 2;
                    }
                    self.rockyground(n, 1, 0);
                } else {
                    n = 15 + self.rng.random(32);
                    if self.albedo > 48 {
                        n /= 2;
                    }
                    let a_l24 = self.rng.random(24);
                self.rockyground(n, 1, -a_l24);
                }
                n = self.rng.random(68) - self.albedo;
                if n > 10 {
                    n = 10;
                }
                if n < 1 {
                    n = 1;
                }
                while n != 0 {
                    hf = f(self.rng.random(5) as f64 * 0.015);
                    hr = f((self.rng.random(10) + 10) as f64 * 0.27);
                    let a_r = self.rng.random(35) + 5;
                    let a_cz = self.rng.random(200);
                    let a_cx = self.rng.random(200);
                    self.std_crater(0, a_cx, a_cz, a_r, 127, hf, hr, 200);
                    n -= 1;
                }
                self.rockscaling = f(50.0 + self.rng.random(400) as f64);
                self.rockpeaking = f(50.0 + self.rng.random(250) as f64);
                self.rockdensity = 1 + 30 * self.rng.random(2);
                if self.albedo > 50 {
                    self.sky_brightness = (self.sky_brightness as f64 * 2.0) as i32;
                    if self.sky_brightness > 63 {
                        self.sky_brightness = 63;
                    }
                }
                if self.albedo > 40 && self.albedo <= 50 {
                    self.rockscaling = f(self.rockscaling as f64 * 2.0);
                    self.rockdensity = 15 + 16 * self.rng.random(2);
                    hf = f(self.rng.random(5) as f64 * 0.01);
                    hr = f((self.rng.random(5) + 5) as f64 * 0.5);
                    let a_cx = 90 + self.rng.random(20);
                    let a_cz = 90 + self.rng.random(20);
                    let a_r = 100 + self.rng.random(10);
                    self.std_crater(0, a_cx, a_cz, a_r, 127, hf, hr, 200);
                }
                ptr = (self.rng.random(1500) + 500) as usize;
                n = self.albedo * 5;
                while n != 0 {
                    let r = self.rng.random(ptr as i32);
                    self.srf_darkline(1, r, -1, -1, 256);
                    n -= 1;
                }
            }

            7 => {
                // gelidi, solcati di strie
                let a_100 = self.rng.random(100);
                self.rockyground(10 - (self.albedo / 8), 0, 20 + a_100);
                n = self.albedo - self.rng.random(self.albedo) + 10;
                while n != 0 {
                    let r = self.rng.random(500);
                    self.srf_darkline(0, r, -1, -1, 200);
                    n -= 1;
                }
                let _n_l = self.rng.random(200);
                let _n_r = self.rng.random(100);
                n = self.albedo + _n_l - _n_r;
                if n < 0 {
                    n = 0;
                }
                while n != 0 {
                    cx = self.rng.random(192) + 32;
                    cz = self.rng.random(192) + 32;
                    cr = self.rng.random(16) + 16;
                    self.std_crater(1, cx, cz, -cr, 31, 0.15, 1.0, 256);
                    n -= 1;
                }
                // C evaluates left-to-right: random(100) first, then random(50)
                let _n100 = self.rng.random(100);
                let _n50 = self.rng.random(50);
                n = self.albedo + _n100 - _n50;
                if n < 0 {
                    n = 0;
                }
                n /= 2;
                while n != 0 {
                    let a_z2 = self.rng.random(2);
                    let a_z1 = self.rng.random(2);
                    let a_x = self.rng.random(100);
                    self.srf_darkline(1, a_x, -a_z1, -a_z2, 256);
                    n -= 1;
                }
                self.rockscaling = f(50.0 + self.rng.random(400) as f64);
                self.rockpeaking = f(50.0 + self.rng.random(200) as f64);
                self.rockdensity = 3 + 4 * self.rng.random(2);
            }

            8 => {
                // lattiginosi (al quarzo)
                if self.albedo < 20 {
                    ptr = (100 - self.albedo) as usize;
                    while ptr != 0 {
                        hr = f(self.rng.random(300) as f64);
                        let a_h = hr as f64 + 1.0;
                        let a_r = self.rng.random(5) + 2;
                        let a_cz = self.rng.random(150) + 25;
                        let a_cx = self.rng.random(150) + 25;
                        self.round_hill(a_cx, a_cz, a_r as u32, f(a_h), 127.0, 0);
                        ptr -= 1;
                    }
                    let a_s3 = self.rng.random(3);
                    self.smoothterrain(2 + a_s3);
                }
                ptr = ((100 - self.albedo) * 2) as usize;
                while ptr != 0 {
                    let a_h = self.rng.random(25) + 1;
                    let a_r = self.rng.random(25) + 1;
                    let a_cz = self.rng.random(200);
                    let a_cx = self.rng.random(200);
                    self.round_hill(a_cx, a_cz, a_r as u32, f(a_h as f64), 0.0_f32, 1);
                    ptr -= 1;
                }
                self.quartz = 1;
                self.rockscaling = f(50.0 + self.rng.random(300) as f64);
                self.rockpeaking = f(50.0 + self.rng.random(300) as f64);
                self.rockdensity = 7 + 8 * self.rng.random(2);
                if self.albedo > 40 {
                    self.rockscaling = f(self.rockscaling as f64 * 0.5);
                    self.rockpeaking = self.rockscaling;
                    let a_r2 = self.rng.random(2);
                    self.rockdensity = 3 + 4 * a_r2;
                    let a_s10 = self.rng.random(10);
                    self.smoothterrain(1 + a_s10);
                }
                self.snowy = 0;
                self.frosty = 0;
                if self.rng.random(2) != 0 {
                    self.snowy = 1;
                } else {
                    self.frosty = 1;
                }
                if self.snowy != 0 || self.frosty != 0 {
                    self.t_scale = 32;
                    n = self.rng.random(16) + 16;
                    ptr = 65535;
                    while ptr != 0 {
                        self.p_background[ptr] = self.rng.random(n) as u8;
                        ptr -= 1;
                    }
                    n = 1 + self.rng.random(3);
                    while n != 0 {
                        ptr = 65535 - 257;
                        while ptr != 0 {
                            let cxv = self.p_background[ptr] as i32
                                + self.p_background[ptr + 1] as i32
                                + self.p_background[ptr + 256] as i32
                                + self.p_background[ptr + 257] as i32;
                            self.p_background[ptr] = (cxv >> 2) as u8;
                            ptr -= 1;
                        }
                        n -= 1;
                    }
                }
                if self.frosty != 0 {
                    self.t_scale = 16 + self.rng.random(48) as i64;
                    n = self.rng.random(250);
                    while n != 0 {
                        let a_z = self.rng.random(2);
                        let a_x = self.rng.random(200);
                        self.srf_darkline(1, 100 + a_x, -a_z, 0, 256);
                        n -= 1;
                    }
                }
                // shared "similar" tail (NOCTIS-1.CPP:2393): quartz may appear
                // "anche sui pianeti abitabili, ma solo a volte..."
                if self.rng.random(5) == 0 {
                    self.quartz = 1;
                }
            }

            _ => {}
        }

        n = self.rng.random(5);
        if n != 0 {
            while n != 0 {
                let r = self.rng.random(500);
                self.felisian_srf_darkline(r, -1, -1, 200);
                n -= 1;
            }
            ptr1 = 200;
            while ptr1 < 38800 {
                let pn = self.p_surfacemap[ptr1 as usize] as i32
                    + self.p_surfacemap[ptr1 as usize - 1] as i32
                    + self.p_surfacemap[ptr1 as usize + 1] as i32
                    + self.p_surfacemap[ptr1 as usize - 200] as i32
                    + self.p_surfacemap[ptr1 as usize + 200] as i32;
                self.p_surfacemap[ptr1 as usize] = (pn / 5) as u8;
                ptr1 += 1;
            }
        }

        // distribuzione oggetti in base alla pendenza
        ptr = 0;
        while ptr < oc_bytes {
            incl = (self.surf_h(ptr) as i32 - self.surf_h(ptr + 1) as i32).abs();
            incl += (self.surf_h(ptr) as i32 - self.surf_h(ptr + 200) as i32).abs();
            if incl < 20 {
                let r = self.rng.random(2) as u8;
                self.set_obj_nr(ptr, r);
            }
            if incl < 15 {
                let r = self.rng.random(3) as u8;
                self.set_obj_nr(ptr, r);
            }
            if incl < 10 {
                let r = self.rng.random(4) as u8;
                self.set_obj_nr(ptr, r);
            }
            ptr += 1;
        }

        // se c'è acqua liquida sulla superficie, non possono esserci
    // oggetti che vi galleggiano
    if self.liquid_water != 0 {
        ptr = 0;
        while ptr < oc_bytes {
            if self.p_surfacemap[ptr] == 0 {
                self.set_obj_nr(ptr, 0);
            }
            ptr += 1;
        }
    }

        // INIZIO MODIFICHE STORICHE: reimpostazione seeds con wrapping 16-bit
        let land_seed = ((self.landing_pt_lat as i16).wrapping_mul(self.landing_pt_lon as i16)) as i32 as u32;
        self.frng.srand(land_seed);
        self.rng.srand(land_seed);

        // PORT-NOTE: nearstar_identity is F80; multiply by 1E6/1E5/1E8 then
        // (long) truncation toward zero.
        use crate::f80::F80;
        let id6 = self.nearstar_identity.mul(&F80::from_f64(1e6)).trunc_i64();
        let id5 = self.nearstar_identity.mul(&F80::from_f64(1e5)).trunc_i64();
        let id8 = self.nearstar_identity.mul(&F80::from_f64(1e8)).trunc_i64();

        if id6 == -37828 {
            // Balastrackonastreya
            if self.ip_targetted == 3 {
                self.base_pp_pressure = 1.0;
                self.make_ruins(0, 1, 1, 2, 2, 3);
            } else if self.p_type[n_idx] as i32 == 3 {
                self.make_ruins(3, 3, 3, 1, 4, 1);
            }
        }
        if id5 == 1599551984 {
            // Fenia
            if self.ip_targetted == 2 {
                self.make_ruins(0, 0, 0, 1, 1, 2);
            }
            if self.ip_targetted == 3 {
                self.make_ruins(2, 2, 3, 3, 4, 2);
            }
            if self.ip_targetted == 6 {
                self.make_ruins(3, 4, 4, 4, 4, 1);
            }
        }
        if id8 == -11543634 {
            // Ylastravenia
            if self.ip_targetted == 3 {
                self.make_ruins(2, 4, 5, 5, 5, 2);
                if self.landing_pt_lon == 18 && self.landing_pt_lat == 60 {
                    let mut ptr2 = 112i32;
                    while ptr2 < 112 + 25 {
                        let mut ptr1b = 103i32;
                        while ptr1b < 103 + 25 {
                            self.p_surfacemap[(self.m200[ptr1b as usize] as usize) + ptr2 as usize] = 127;
                            if ptr1b == 103 + 1 || ptr1b == 103 + 2 {
                                self.ruins_write((self.m200[ptr1b as usize] as usize) + ptr2 as usize, AF1);
                            }
                            ptr1b += 1;
                        }
                        if ptr2 == 112 + 19 || ptr2 == 112 + 20 || ptr2 == 112 + 2 || ptr2 == 112 + 3 {
                            let mut ptr1b = 103i32;
                            while ptr1b < 103 + 25 {
                                self.ruins_write((self.m200[ptr1b as usize] as usize) + ptr2 as usize, AF1);
                                ptr1b += 1;
                            }
                        }
                        ptr2 += 1;
                    }
                }
            }
        }

        // ===================== FINE MODIFICHE STORICHE =====================

        // infine si calcola la mappa di shading (not hashed, but faithful).
        self.sh_delta = 0;
        if self.sun_x.abs() > 0.33 * self.dsd1 {
            if self.sun_x > 0.0 {
                self.sh_delta = 1;
            } else {
                self.sh_delta = -1;
            }
        }
        if self.sun_z.abs() > 0.33 * self.dsd1 {
            if self.sun_z > 0.0 {
                self.sh_delta += 200;
            } else {
                self.sh_delta -= 200;
            }
        }
    }

    // --- type-3 PLAINS/OCEAN shared blocks (labels `revert` / `addtrees`) ---

    fn oc_revert(&mut self) {
        let mut ptr = 0usize;
        let branch = self.rng.random(2);
        if branch != 0 {
            ptr = (self.rng.random(50) + 5) as usize;
            while ptr != 0 {
                let a_h = self.rng.random(30) + 1;
                let a_r = self.rng.random(200) + 1;
                let a_cz = self.rng.random(200);
                let a_cx = self.rng.random(200);
                self.round_hill(a_cx, a_cz, a_r as u32, f(a_h as f64), 0.0_f32, 1);
                ptr -= 1;
            }
        } else {
            ptr = (self.rng.random(25) + 10) as usize;
            while ptr != 0 {
                let a_h = self.rng.random(100) + 1;
                let a_r = self.rng.random(200) + 1;
                let a_cz = self.rng.random(200);
                let a_cx = self.rng.random(200);
                self.round_hill(a_cx, a_cz, a_r as u32, f(a_h as f64), 0.0_f32, 1);
                ptr -= 1;
            }
        }
        // fall through to addtrees
        self.oc_addtrees();
    }

    fn oc_addtrees(&mut self) {
        let mut n = self.rng.random(6);
        let oc_bytes = 40000usize;
        let mut ptr = 0usize;
        while ptr < oc_bytes {
            match self.p_surfacemap[ptr] as i32 / 25 {
                0 => {
                    self.set_obj_c1(ptr, VEGET as u8);
                    self.set_obj_c2(ptr, VEGET as u8);
                }
                1 => {
                    self.set_obj_c1(ptr, VEGET as u8);
                    self.set_obj_c2(ptr, TREES as u8);
                }
                2 => {
                    self.set_obj_c1(ptr, TREES as u8);
                    self.set_obj_c2(ptr, TREES as u8);
                }
                _ => {
                    self.set_obj_c0(ptr, TREES as u8);
                    self.set_obj_c1(ptr, TREES as u8);
                    self.set_obj_c2(ptr, TREES as u8);
                }
            }
            ptr += 1;
        }
        self.t_scale = 128;
        n = self.rng.random(15) + 2;
        ptr = 65535;
        while ptr != 0 {
            self.p_background[ptr] = self.rng.random(n) as u8;
            ptr -= 1;
        }
        n = 100 + self.rng.random(500);
        while n != 0 {
            // asterism(txtr, random(256), random(256), random(16),
            //          random(16), random(25)+6, random(15)+6, 256)
            let a_size = self.rng.random(15) + 6;
            let a_density = self.rng.random(25) + 6;
            let a_variation = self.rng.random(16);
            let a_base = self.rng.random(16);
            let a_y = self.rng.random(256);
            let a_x = self.rng.random(256);
            self.asterism(a_x, a_y, a_base, a_variation, a_density, a_size, 256);
            n -= 1;
        }
        // C: `if (!waswet || (waswet && !random(5)))`
        // C short-circuit: random(5) only called when waswet != 0
        let w = self.waswet;
        let run_jitter = if w == 0 {
            true
        } else {
            self.rng.random(5) == 0
        };
        if run_jitter {
            ptr = 0;
            while ptr < oc_bytes {
                self.p_surfacemap[ptr] = (self.p_surfacemap[ptr] as i32 + self.frng.fast_random(3) as i32) as u8;
                ptr += 1;
            }
        }
        self.rockscaling = f(100.0 + self.rng.random(200) as f64);
        self.rockpeaking = f(100.0 + self.rng.random(200) as f64);
        self.rockdensity = 3 + 4 * self.rng.random(2);
        if self.waswet != 0 {
            self.liquid_water = 1;
        } else {
            self.gtx = 1;
        }
    }

    // ── sky texture ─────────────────────────────────────────────────────

    /// cloudy_sky (NOCTIS-1.CPP:1703). Writes cloud patterns into s_background.
    fn cloudy_sky(&mut self, density: i32, smooths: i32) {
        let mut n = self.rng.random(density + self.albedo);
        let pqw = self.quadwords;
        self.quadwords = ST_BYTES as i32 / 4;
        while n > 0 {
            let cx = self.rng.random(360) as f64;
            let r = (self.rng.random(25) + 5) as f64;
            let cy = self.rng.random(50) as f64 + 25.0 + r;
            let mut y = -r;
            while y < r {
                let mut x = -2.0 * r;
                while x < 2.0 * r {
                    if (x * x * 0.2 + y * y).sqrt() < r {
                        let p = (x + cx + 360.0 * (y + cy)) as usize;
                        if p < ST_BYTES {
                            let mut b = (1.4142 / ((x + r) * (x + r) + (y + r) * (y + r)).sqrt()) * 64.0;
                            b += self.s_background[p] as f64;
                            if b > 63.0 { b = 63.0; }
                            self.s_background[p] = b as u8;
                        }
                    }
                    x += 1.0;
                }
                y += 1.0;
            }
            n -= 1;
        }
        // Swap so smoothing works on s_background
        std::mem::swap(&mut self.p_background, &mut self.s_background);
        let mut s = smooths;
        while s > 0 {
            self.ssmooth();
            s -= 1;
        }
        std::mem::swap(&mut self.p_background, &mut self.s_background);
        self.quadwords = pqw;
    }

    /// nebular_sky (NOCTIS-1.CPP:1674). Generates a nebula/alien sky texture
    /// in s_background using the `imul ax` pattern.
    fn nebular_sky(&mut self) {
        let seed = self.rng.random(10000) as u32;
        let pqw = self.quadwords;
        self.quadwords = ST_BYTES as i32 / 4;
        let mut cx = seed;
        for i in 0..ST_BYTES {
            let step = (ST_BYTES - i) as u32; // C decrements cx from st_bytes to 1
            cx = cx.wrapping_add(step);
            // imul ax: signed 16-bit square → dx:ax, keep low word
            let prod = (cx as i16 as i32).wrapping_mul(cx as i16 as i32) as u32;
            let lo = prod & 0xFFFF;
            let hi = prod >> 16;
            cx = lo.wrapping_add(hi); // add ax, dx
            self.s_background[i] = (cx as u8) & 0x3F;
        }
        // Swap buffers so smoothing works on s_background
        std::mem::swap(&mut self.p_background, &mut self.s_background);
        self.lssmooth();
        if self.rng.random(2) != 0 {
            self.ssmooth();
        }
        if self.rng.random(3) != 0 {
            self.psmooth_grays();
        }
        std::mem::swap(&mut self.p_background, &mut self.s_background);
        self.quadwords = pqw;
    }

    /// create_sky (NOCTIS-1.CPP:2736). Generates the sky texture, palette
    /// gradients, and temperature/pressure for the sector.
    /// NOTE: C re-seeds the RNG at the start (line 2803-2804):
    ///   fast_srand(global_surface_seed); srand(global_surface_seed);
    pub fn create_sky(&mut self, atmosphere: i32) {
        self.frng.srand(self.global_surface_seed as u32);
        self.rng.srand(self.global_surface_seed as u32);
        let br = self.sky_red_filter as f64 / 64.0;
        let bg = self.sky_grn_filter as f64 / 64.0;
        let bb = self.sky_blu_filter as f64 / 64.0;
        let tr = self.gnd_red_filter as f64 / 64.0;
        let tg = self.gnd_grn_filter as f64 / 64.0;
        let tb = self.gnd_blu_filter as f64 / 64.0;
        let mut fr = [0.0f64; 4];
        let mut fg = [0.0f64; 4];
        let mut fb = [0.0f64; 4];
        let al = self.albedo as f64 / 64.0;
        let mut dfs: f64;
        let mut sb: f64;
        let owner = self.p_owner[self.ip_targetted as usize];
        if owner == -1 {
            dfs = 1.0 - (self.ip_targetted as f64) * 0.05;
        } else {
            dfs = 1.0 - (owner as f64) * 0.05;
        }
        if atmosphere == 0 {
            sb = 1.0;
        } else {
            sb = self.sky_brightness as f64 / 24.0;
            if self.nightzone != 0 { dfs *= 0.5; }
        }
        if owner > 2 {
            sb *= dfs * dfs;
        } else {
            dfs = 1.0;
        }
        dfs *= match self.nearstar_class {
            0 => 1.0, 1 => 1.5, 2 => 0.5, 3 => 0.8, 4 => 1.2,
            5 => 0.1, 6 => 0.1, 7 => 0.4, 8 => 0.9, 9 => 1.3,
            10 => 0.5, 11 => 0.2, _ => 1.0,
        };
        let saturation = 1.0 - 0.15 * self.rainy;
        let ptype = self.p_type[self.ip_targetted as usize] as i32;
        match ptype {
            1 => {
                // rocky (moon-like)
                self.pp_pressure = 0.0;
                fr[0] = tr; fg[0] = tg; fb[0] = tb;
                fr[1] = 1.5; fg[1] = 1.5; fb[1] = 1.5;
                fr[2] = 2.0 * fr[0]; fg[2] = 2.0 * fg[0]; fb[2] = 2.0 * fb[0];
                fr[3] = 0.0; fg[3] = 0.0; fb[3] = 0.0;
            }
            2 => {
                // venusian (thick atmosphere)
                fr[0] = 1.2 - tr;
                fr[1] = tr + self.rng.flandom() * 0.15 - self.rng.flandom() * 0.15 + 0.3;
                fr[2] = tr + self.rng.flandom() * 0.30 - self.rng.flandom() * 0.30 + 0.2;
                fr[3] = tr + self.rng.flandom() * 0.45 - self.rng.flandom() * 0.45 + 0.1;
                fg[0] = 1.2 - tg;
                fg[1] = tg + self.rng.flandom() * 0.15 - self.rng.flandom() * 0.15 + 0.3;
                fg[2] = tg + self.rng.flandom() * 0.30 - self.rng.flandom() * 0.30 + 0.2;
                fg[3] = tg + self.rng.flandom() * 0.45 - self.rng.flandom() * 0.45 + 0.1;
                fb[0] = 1.2 - tb;
                fb[1] = tb + self.rng.flandom() * 0.15 - self.rng.flandom() * 0.15 + 0.3;
                fb[2] = tb + self.rng.flandom() * 0.30 - self.rng.flandom() * 0.30 + 0.2;
                fb[3] = tb + self.rng.flandom() * 0.45 - self.rng.flandom() * 0.45 + 0.1;
                self.nebular_sky();
                self.pp_pressure = self.frng.fast_flandom() * 20.0 + self.albedo as f64 + 1.0;
            }
            3 => {
                // felisian (habitable)
                fr[1] = br * 0.5 + 0.5 * self.rng.flandom();
                fg[1] = bg * 0.5 + 0.5 * self.rng.flandom();
                fb[1] = bb * 0.5 + 0.5 * self.rng.flandom();
                match self.sctype {
                    1 => { // OCEAN
                        fr[0] = 0.65 + 0.5 * self.rng.flandom();
                        fg[0] = 0.45 + 0.4 * self.rng.flandom();
                        fb[0] = 0.25 + 0.3 * self.rng.flandom();
                        if fg[0] < 0.6 { fg[0] *= 2.0; }
                        fr[2] = 0.8 * self.rng.flandom();
                        fg[2] = 0.8 * self.rng.flandom();
                        fb[2] = fb[0] * 2.0 + 0.4;
                        fr[3] = 0.2 + self.rng.flandom();
                        fg[3] = 0.4 + self.rng.flandom();
                        fb[3] = self.rng.flandom() * 0.6;
                        self.cloudy_sky(50, 1);
                    }
                    2 => { // PLAINS
                        fr[0] = 0.25 + 0.5 * self.rng.flandom();
                        fg[0] = 0.50 + 0.4 * self.rng.flandom();
                        fb[0] = 0.25 + 0.3 * self.rng.flandom();
                        if fg[0] < 0.75 { fg[0] *= 1.5; }
                        fr[2] = self.rng.flandom() * 0.4 + fr[0] * 0.3;
                        fg[2] = self.rng.flandom() * 0.7 + fg[0] * 0.3;
                        fb[2] = self.rng.flandom() * 0.2 + fb[0] * 0.3;
                        fr[3] = self.rng.flandom();
                        fg[3] = self.rng.flandom();
                        fb[3] = self.rng.flandom();
                        self.cloudy_sky(33, 1);
                    }
                    3 => { // DESERT
                        fr[0] = tr + self.rng.flandom() * 0.33;
                        fg[0] = tg + self.rng.flandom() * 0.25;
                        fb[0] = tb + self.rng.flandom() * 0.12;
                        fr[2] = tr; fg[2] = tg; fb[2] = tb;
                        fr[3] = 0.50 * self.rng.flandom();
                        fg[3] = 0.90 * self.rng.flandom();
                        fb[3] = 0.40 * self.rng.flandom();
                        self.cloudy_sky(10, 1);
                    }
                    _ => { // ICY
                        fr[0] = 0.25 + self.rng.flandom();
                        fg[0] = 0.55 + self.rng.flandom();
                        fb[0] = 1.00 + self.rng.flandom();
                        fr[2] = fr[0] * 0.6;
                        fg[2] = fg[0] * 0.8;
                        fb[2] = fb[0];
                        fr[3] = 0.95 * self.rng.flandom();
                        fg[3] = 0.95 * self.rng.flandom();
                        fb[3] = 0.95 * self.rng.flandom();
                        self.cloudy_sky(15, 1);
                    }
                }
                self.pp_pressure = self.frng.fast_flandom() * 0.8 + 0.6;
            }
            4 => {
                self.pp_pressure = self.frng.fast_flandom() * 0.1;
                // goto like1 (same as case 1)
                fr[0] = tr; fg[0] = tg; fb[0] = tb;
                fr[1] = 1.5; fg[1] = 1.5; fb[1] = 1.5;
                fr[2] = 2.0 * fr[0]; fg[2] = 2.0 * fg[0]; fb[2] = 2.0 * fb[0];
                fr[3] = 0.0; fg[3] = 0.0; fb[3] = 0.0;
            }
            5 => {
                // martian (thin atmosphere)
                fr[0] = tr + 0.33 * self.rng.flandom() * al;
                fg[0] = tg + 0.33 * self.rng.flandom() * al;
                fb[0] = tb + 0.33 * self.rng.flandom() * al;
                fr[1] = 0.8 * tb + 0.2 * self.rng.flandom() * al;
                fg[1] = 0.8 * tg + 0.2 * self.rng.flandom() * al;
                fb[1] = 0.8 * tr + 0.2 * self.rng.flandom() * al;
                fr[2] = 0.5 + fr[0] * 0.5 * al;
                fg[2] = 0.5 + fg[0] * 0.5 * al;
                fb[2] = 0.5 + fb[0] * 0.5 * al;
                fr[3] = 0.0; fg[3] = 0.0; fb[3] = 0.0;
                self.sky_brightness = ((self.sky_brightness as f64) * 0.65) as i32;
                self.cloudy_sky(10, 2);
                self.pp_pressure = self.frng.fast_flandom() * 0.05 + 0.01;
            }
            7 => {
                // icy (europa-like)
                self.pp_pressure = self.frng.fast_flandom() * 0.02;
                fr[0] = tr + self.rng.flandom() * al;
                fg[0] = tg + self.rng.flandom() * al;
                fb[0] = tb + self.rng.flandom() * al;
                fr[1] = 1.3; fg[1] = 1.4; fb[1] = 1.5;
                fr[2] = 0.5 + fr[0]; fg[2] = 0.5 + fg[0]; fb[2] = 0.5 + fb[0];
                fr[3] = 0.0; fg[3] = 0.0; fb[3] = 0.0;
            }
            8 => {
                self.pp_pressure = self.frng.fast_flandom() + 0.2;
                // goto like7 (same as case 7)
                fr[0] = tr + self.rng.flandom() * al;
                fg[0] = tg + self.rng.flandom() * al;
                fb[0] = tb + self.rng.flandom() * al;
                fr[1] = 1.3; fg[1] = 1.4; fb[1] = 1.5;
                fr[2] = 0.5 + fr[0]; fg[2] = 0.5 + fg[0]; fb[2] = 0.5 + fb[0];
                fr[3] = 0.0; fg[3] = 0.0; fb[3] = 0.0;
            }
            _ => {}
        }

        // Clamp negative gradients
        for i in 0..4 {
            if fr[i] < 0.0 { fr[i] = 0.0; }
            if fg[i] < 0.0 { fg[i] = 0.0; }
            if fb[i] < 0.0 { fb[i] = 0.0; }
        }

        // Saturation correction for habitable / martian
        if ptype == 3 || ptype == 5 {
            for i in 0..4 {
                fr[i] = (fr[i] - 0.5) * saturation + 0.5;
                fg[i] = (fg[i] - 0.5) * saturation + 0.5;
                fb[i] = (fb[i] - 0.5) * saturation + 0.5;
            }
        }

        // Scale by dfs/sb
        fr[0] *= 64.0 * dfs; fg[0] *= 64.0 * dfs; fb[0] *= 64.0 * dfs;
        fr[1] *= 64.0 * dfs * sb; fg[1] *= 64.0 * dfs * sb; fb[1] *= 64.0 * dfs * sb;
        fr[2] *= 64.0 * dfs; fg[2] *= 64.0 * dfs; fb[2] *= 64.0 * dfs;
        fr[3] *= 64.0 * dfs; fg[3] *= 64.0 * dfs; fb[3] *= 64.0 * dfs;

        if atmosphere == 0 {
            self.shade(64, 64, 0.0, 0.0, 0.0, 100.0, 110.0, 120.0);
        } else if self.nightzone != 0 {
            self.shade(64, 64, 0.0, 0.0, 0.0, 60.0, 62.0, 64.0);
            if ptype == 3 {
                self.shade(0, 64, 0.0, 0.0, 0.0, 64.0, 62.0, 60.0);
                self.shade(128, 64, 0.0, 0.0, 0.0, 64.0, 64.0, 64.0);
                self.shade(192, 64, 8.0, 12.0, 16.0, 56.0, 60.0, 64.0);
            } else {
                fr[0] *= 0.33; fg[0] *= 0.44; fb[0] *= 0.55;
                fr[2] *= 0.33; fg[2] *= 0.44; fb[2] *= 0.55;
                fr[3] *= 0.33; fg[3] *= 0.44; fb[3] *= 0.55;
                self.shade(64, 64, 0.0, 0.0, 0.0, fr[1], fg[1], fb[1]);
            }
        } else {
            self.shade(64, 64, 0.0, 0.0, 0.0, fr[1], fg[1], fb[1]);
        }

        // Ground gradient
        self.shade(0, 44, 0.0, 0.0, 0.0, fr[0], fg[0], fb[0]);
        self.shade(44, 20, fr[0], fg[0], fb[0], fr[1], fg[1], fb[1]);

        // Horizon gradient
        self.shade(128, 10, 0.0, 0.0, 0.0, fr[0], fg[0], fb[0]);
        self.shade(138, 44, fr[0], fg[0], fb[0], fr[2], fg[2], fb[2]);
        self.shade(182, 10, fr[2], fg[2], fb[2], fr[1], fg[1], fb[1]);

        // Vegetation gradient
        self.shade(192, 10, 0.0, 0.0, 0.0, fr[0], fg[0], fb[0]);
        self.shade(202, 44, fr[0], fg[0], fb[0], fr[3], fg[3], fb[3]);
        self.shade(246, 10, fr[3], fg[3], fb[3], fr[1], fg[1], fb[1]);

        // Temperature calculation
        self.pp_temp = 90.0 - self.dsd1 * 0.33;
        if atmosphere == 0 {
            self.pp_temp -= 44.0;
            self.pp_temp *= (self.pp_temp * 0.44).abs();
            if self.nightzone != 0 {
                self.pp_temp *= 0.3;
            } else {
                self.pp_temp *= 0.3 + self.exposure * 0.0077;
            }
        } else {
            if self.nightzone != 0 {
                self.pp_temp *= 0.6;
            } else {
                self.pp_temp *= 0.6 + self.exposure * 0.0044;
            }
        }
        self.pp_temp -= (0.5 + 0.5 * self.frng.fast_flandom()) * (self.landing_pt_lat - 60).abs() as f64;
        if self.pp_temp < -269.0 {
            self.pp_temp = -269.0 + 4.0 * self.frng.fast_flandom();
        }
        if ptype == 2 {
            self.pp_temp += self.frng.fast_flandom() * 150.0;
        }
        if ptype == 3 {
            let (lo, hi) = match self.sctype {
                1 => (10.0, 60.0),
                2 => (-10.0, 45.0),
                3 => (20.0, 80.0),
                _ => (-120.0, 4.0),
            };
            let mut _safety = 0i32;
            while self.pp_temp < lo && _safety < 10000 {
                self.pp_temp += self.frng.fast_flandom() * 5.0;
                _safety += 1;
            }
            _safety = 0;
            while self.pp_temp > hi && _safety < 10000 {
                self.pp_temp -= self.frng.fast_flandom() * 5.0;
                _safety += 1;
            }
        }
        self.base_pp_pressure = self.pp_pressure;
    }
}
