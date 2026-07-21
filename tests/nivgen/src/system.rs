//! Star-system generation: `extract_ap_target_infos` + `prepare_nearstar`.
//!
//! Ported bug-for-bug from `source/NOCTIS-0.CPP`. The generator is a pure
//! function of the Parsis coordinates (x, y, z); the RNG is fully reseeded
//! from those coordinates inside these functions, so output is deterministic.

use crate::rng::BorlandRng;
use crate::f80::F80;

/// Full DOS segment size the surface buffers occupy (16-bit `di` range).
///
/// Both surface buffers are sized to a full 64 KB segment (not the 64800-byte
/// map) so the `.len()`-based bounds checks in `surface.rs` reproduce C's 16-bit
/// `di` addressing (`[0, 0xFFFF]`) exactly. `lssmooth`/`psmooth_grays` read a
/// few bytes past the 64800-byte map near the south pole; in C those land in the
/// adjacent far-heap allocation (`p_background`). We do NOT model that scratch
/// tail because it is never hashed: the globe renderer only samples rows 0..121,
/// and hashing is restricted to rows 0..127 (see `SAMPLED_BYTES` in main.rs).
pub const BG_SEGMENT: usize = 65536;
/// The surface map proper (360 x 180).
pub const BG_SURFACE: usize = 64800;

fn make_bg() -> Vec<u8> {
    vec![0u8; BG_SEGMENT]
}

/// Wrap a `double`/`f64` value the way Borland's 16-bit `int` would when a
/// C expression like `10 * nearstar_p_orb_seed[n]` (whose operands are `int`)
/// is passed to `random()`/`zrandom()`. The product is computed in `int` (which
/// is 16-bit under Turbo C++), so it wraps modulo 65536. E.g.
/// `10 * 3309 = 33091 -> (int)-32445`. Without this, large seed-derived
/// arguments would not overflow and the sign of the resulting zrandom would flip.
fn i16(x: f64) -> i32 {
    // Modulo-65536 wrap (Borland's 16-bit `int`), since Rust's `as
    // i16` *saturates* instead of wrapping. E.g. 33091 -> -32445.
    let t = (x as i32) & 0xFFFF;
    if t >= 32768 {
        t - 65536
    } else {
        t
    }
}

pub const STAR_CLASSES: usize = 12;
pub const PLANET_TYPES: usize = 10;
pub const MAXBODIES: usize = 20 * 4; // 20 * avgmoons

pub const CLASS_RGB: [u8; 3 * STAR_CLASSES] = [
    63, 58, 40, 30, 50, 63, 63, 63, 63, 63, 30, 20, 63, 55, 32, 32, 16, 10, 32, 28, 24, 10, 20, 63,
    63, 32, 16, 48, 32, 63, 40, 10, 10, 00, 63, 63,
];

pub const CLASS_RAY: [i32; STAR_CLASSES] = [
    5000, 15000, 300, 20000, 15000, 1000, 3000, 2000, 4000, 1500, 30000, 250,
];

pub const CLASS_RAYVAR: [i32; STAR_CLASSES] = [
    2000, 10000, 200, 15000, 5000, 1000, 3000, 500, 5000, 10000, 1000, 10,
];

pub const CLASS_PLANETS: [i32; STAR_CLASSES] = [12, 18, 8, 15, 20, 3, 0, 1, 7, 20, 2, 5];

pub const PLANET_RGB_AND_VAR: [u8; 44] = [
    60, 30, 15, 20, 40, 50, 40, 25, 32, 32, 32, 32, 16, 32, 48, 40, 32, 40, 32, 20, 32, 32, 32, 32,
    32, 32, 32, 32, 32, 40, 48, 24, 40, 40, 40, 30, 50, 25, 10, 20, 40, 40, 40, 40,
];

pub const PLANET_POSSIBLEMOONS: [i32; 11] = [1, 1, 2, 3, 2, 2, 18, 2, 3, 20, 20];

pub const PLANET_ORB_SCALING: f64 = 5.0;
pub const AVG_PLANET_SIZING: f64 = 2.4;
pub const MOON_ORB_SCALING: f64 = 12.8;
pub const AVG_MOON_SIZING: f64 = 1.8;

pub const AVG_PLANET_RAY: [f64; 11] = [
    0.007, 0.003, 0.010, 0.011, 0.010, 0.008, 0.064, 0.009, 0.012, 0.125, 5.000,
];

pub const PLANET_DESCRIPTION: [&str; 11] = [
    "medium size, internally hot, unstable surface, no atmosphere.",
    "small, solid, dusty, craterized, no atmosphere.",
    "medium size, solid, thick atmosphere, fully covered by clouds.",
    "medium size, felisian, breathable atmosphere, suitable for life.",
    "medium size, rocky, creased, no atmosphere.",
    "small, solid, thin atmosphere.",
    "large, not consistent, covered with dense clouds.",
    "small, solid, icy surface, no atmosphere.",
    "medium size, surface is mainly native quartz, oxygen atmosphere.",
    "very large, substellar object, not consistent.",
    "companion star - not a planet",
];

pub struct SystemGen {
    pub ap_target_x: f64,
    pub ap_target_y: f64,
    pub ap_target_z: f64,
    pub ap_target_class: i32,
    pub ap_target_ray: f64,
    pub ap_target_r: i32,
    pub ap_target_g: i32,
    pub ap_target_b: i32,
    pub ap_target_spin: i32,

    pub nearstar_class: i32,
    pub nearstar_x: f64,
    pub nearstar_y: f64,
    pub nearstar_z: f64,
    pub nearstar_ray: f32,
    pub nearstar_spin: i32,
    pub nearstar_r: i32,
    pub nearstar_g: i32,
    pub nearstar_b: i32,
    pub nearstar_identity: f64,
    pub nearstar_nop: i32,
    pub nearstar_nob: i32,
    pub nearstar_labeled: i32,

    pub p_type: [i8; MAXBODIES],
    pub p_owner: [i32; MAXBODIES],
    pub p_moonid: [i8; MAXBODIES],
    pub p_ring: [f64; MAXBODIES],
    pub p_tilt: [f64; MAXBODIES],
    pub p_ray: [f64; MAXBODIES],
    pub p_orb_ray: [f64; MAXBODIES],
    pub p_orb_tilt: [f64; MAXBODIES],
    pub p_orb_orient: [f64; MAXBODIES],
    pub p_orb_ecc: [f64; MAXBODIES],
    pub p_orb_seed: [f64; MAXBODIES],
    pub p_rtperiod: [i32; MAXBODIES],
    pub p_rotation: [i32; MAXBODIES],
    pub p_term_start: [i32; MAXBODIES],
    pub p_term_end: [i32; MAXBODIES],

    // auxiliary state used by surface()/build_surface()
    pub s_m: f64,
    pub secs: f64,

    // surface() buffers + scratch globals (ported from NOCTIS-0.CPP).
    // Both are sized to a full 64 KB segment (65536) to reproduce the DOS
    // far-heap layout: surface() uses a 16-bit `di` register, so it can read
    // and write anywhere in [0, 0xFFFF]. The surface map proper is the first
    // 64800 bytes; the tail 64800..65536 mirrors the bytes the harness's
    // farmalloc heap leaves after each buffer (see BG_TAIL_* below). This is
    // what lssmooth/psmooth_grays read out-of-bounds near the south pole.
    pub p_background: Vec<u8>, // 65536 (surface map = first 64800)
    pub s_background: Vec<u8>, // 65536 (surface map = first 64800)
    pub objectschart: Vec<u8>, // 32400 (atmosphere overlay, hashed as bytes)
    pub tmppal: Vec<u8>,       // 768

    // --- build_surface (surface SECTOR) buffers + landing context ---
    // `txtr` (the 256x256 surface-texture buffer) is aliased onto
    // `p_background` in the C harness (`txtr = p_background`), so the
    // `surface_sector` module writes texture bytes into `p_background`.
    pub p_surfacemap: Vec<u8>, // 40000 (200x200 heightmap)

    // Landing context globals (NOCTIS-1.CPP build_surface / harness setup_landing).
    pub ip_targetted: i32,
    pub landing_pt_lon: i32,
    pub landing_pt_lat: i32,
    pub landing_point: i32,
    pub latitude: f32,
    pub nightzone: i32,
    pub albedo: i32,
    pub global_surface_seed: i64,
    pub sctype: i32, // OCEAN=1 PLAINS=2 DESERT=3 ICY=4

    // Terrain tuning globals (written by build_surface; not hashed).
    pub t_scale: i64,
    pub quartz: i32,
    pub gtx: i32,
    pub groundflares: i32,
    pub waves_in: i32,
    pub waves_out: i32,
    pub treepeaking: f32,
    pub treescaling: f32,
    pub treespreads: f32,
    pub branchwidth: f32,
    pub rootheight: f32,
    pub rootshade: i32,
    pub treeflares: i32,
    pub leafflares: i32,
    pub rockscaling: f32,
    pub rockpeaking: f32,
    pub rockdensity: i32,
    pub mushscaling: i64,
    pub sky_brightness: i32,
    pub snowy: i32,
    pub frosty: i32,
    pub waswet: i32,
    pub liquid_water: i32,
    pub sh_delta: i32,

    // Sun / shading geometry (set by setup_landing; consumed by build_surface tail).
    pub dsd: f64,
    pub dsd1: f64,
    pub dsd2: f64,
    pub nray1: f64,
    pub nray2: f64,
    pub sun_x: f64,
    pub sun_y: f64,
    pub sun_z: f64,
    pub base_pp_pressure: f64,
    pub rainy: f64,

    // m200 lookup table: m200[i] = (unsigned)(i * 200). Used by make_ruins.
    pub m200: [u32; 200],
    pub lft_sin: [f64; 361],
    pub lft_cos: [f64; 361],
    pub quadwords: i32,
    // surface() scratch (16-bit-friendly)
    pub s_c: i32,
    pub s_gr: i32,
    pub s_r: i32,
    pub s_g: i32,
    pub s_b: i32,
    pub s_cr: i32,
    pub s_cx: i32,
    pub s_cy: i32,
    pub s_a: f64,
    pub s_kfract: f64,
    pub s_lawe: i32,
    pub s_cray: i32,
    pub s_px: u16,
    pub s_py: u16,
    pub s_plwp: i32,
    pub plx: f64,
    pub ply: f64,
    pub plz: f64,
    pub pixilating_effect: u8,
    pub pixel_spreads: u8,
    pub multicolormask: u8,

    /// Farmalloc gap bytes between p_surfacemap and objectschart in C.
    /// Read by the inclination loop; captured from C fixture output.
    pub oc_gap: [u8; 16],

    pub(crate) rng: BorlandRng,
    pub(crate) frng: crate::rng::FastRng,
}

impl SystemGen {
    pub fn new() -> Self {
        SystemGen {
            ap_target_x: 0.0,
            ap_target_y: 0.0,
            ap_target_z: 0.0,
            ap_target_class: 0,
            ap_target_ray: 0.0,
            ap_target_r: 0,
            ap_target_g: 0,
            ap_target_b: 0,
            ap_target_spin: 0,
            nearstar_class: 0,
            nearstar_x: 0.0,
            nearstar_y: 0.0,
            nearstar_z: 0.0,
            nearstar_ray: 0.0,
            nearstar_spin: 0,
            nearstar_r: 0,
            nearstar_g: 0,
            nearstar_b: 0,
            nearstar_identity: 0.0,
            nearstar_nop: 0,
            nearstar_nob: 0,
            nearstar_labeled: 0,
            p_type: [0; MAXBODIES],
            p_owner: [-1; MAXBODIES],
            p_moonid: [0; MAXBODIES],
            p_ring: [0.0; MAXBODIES],
            p_tilt: [0.0; MAXBODIES],
            p_ray: [0.0; MAXBODIES],
            p_orb_ray: [0.0; MAXBODIES],
            p_orb_tilt: [0.0; MAXBODIES],
            p_orb_orient: [0.0; MAXBODIES],
            p_orb_ecc: [0.0; MAXBODIES],
            p_orb_seed: [0.0; MAXBODIES],
            p_rtperiod: [0; MAXBODIES],
            p_rotation: [0; MAXBODIES],
            p_term_start: [0; MAXBODIES],
            p_term_end: [0; MAXBODIES],
            s_m: 0.0,
            secs: 0.0,
            p_background: make_bg(),
            s_background: make_bg(),
            objectschart: vec![0u8; 40000], // oc_bytes = 40000 (object map); atmo overlay uses [0..32400]
            tmppal: vec![0u8; 768],
            p_surfacemap: vec![0u8; 40000],
            ip_targetted: -1,
            landing_pt_lon: 0,
            landing_pt_lat: 60,
            landing_point: 0,
            latitude: 0.0,
            nightzone: 0,
            albedo: 32,
            global_surface_seed: 0,
            sctype: 2,
            t_scale: 32,
            quartz: 0,
            gtx: 1,
            groundflares: 0,
            waves_in: 0,
            waves_out: 0,
            treepeaking: 1.25,
            treescaling: 4096.0,
            treespreads: 0.75,
            branchwidth: 0.15,
            rootheight: 0.50,
            rootshade: 0x00,
            treeflares: 0x00,
            leafflares: 0x00,
            rockscaling: 500.0,
            rockpeaking: 250.0,
            rockdensity: 15,
            mushscaling: 8191,
            sky_brightness: 0,
            snowy: 0,
            frosty: 0,
            waswet: 0,
            liquid_water: 0,
            sh_delta: 0,
            dsd: 1000.0,
            dsd1: 1000.0,
            dsd2: 1000.0,
            nray1: 0.0,
            nray2: 0.0,
            sun_x: -1000.0,
            sun_y: 0.0,
            sun_z: 0.0,
            base_pp_pressure: 0.0,
            rainy: 0.0,
            m200: {
                let mut m = [0u32; 200];
                for i in 0..200 {
                    m[i] = (i * 200) as u32;
                }
                m
            },
            lft_sin: [0.0; 361],
            lft_cos: [0.0; 361],
            quadwords: 16000,
            s_c: 0,
            s_gr: 0,
            s_r: 0,
            s_g: 0,
            s_b: 0,
            s_cr: 0,
            s_cx: 0,
            s_cy: 0,
            s_a: 0.0,
            s_kfract: 0.0,
            s_lawe: 0,
            s_cray: 0,
            s_px: 0,
            s_py: 0,
            s_plwp: 0,
            plx: 0.0,
            ply: 0.0,
            plz: 0.0,
            pixilating_effect: 0,
            pixel_spreads: 0,
            multicolormask: 0,
            oc_gap: [0,0,0,0,0,0,0,0,0,0,0,0,197,9,181,84],
            rng: BorlandRng::new(),
            frng: crate::rng::FastRng::new(),
        }
    }

    pub fn generate(&mut self, x: f64, y: f64, z: f64) {
        self.ap_target_x = x;
        // y can be user-facing (negated) or engine-facing depending on caller.
        // The engine/starmap convention is positive y, so normalize the sign.
        self.ap_target_y = y.abs();
        self.ap_target_z = z;
        self.extract_ap_target_infos();
        self.prepare_nearstar();
    }

    fn extract_ap_target_infos(&mut self) {
        // The C expression is ((x/100000 * y)/100000 * z)/100000. The starmap
        // binary stores |y|, so sign-cancelling to fabs(y) makes the seed
        // match regardless of input convention and reproduces the starmap
        // class for all named stars. The identity formula keeps the raw y.
        let seed = (self.ap_target_x / 100000.0)
            * (self.ap_target_y / 100000.0)
            * (self.ap_target_z / 100000.0);
        let seed_i = seed as i32;
        self.rng.srand(seed_i as u32);
        self.ap_target_class = self.rng.random(STAR_CLASSES as i32);
        self.ap_target_ray =
            ((self.rng.random(i16(CLASS_RAYVAR[self.ap_target_class as usize] as f64)) as f64)
                + CLASS_RAY[self.ap_target_class as usize] as f64)
                * 0.001;
        self.ap_target_r = CLASS_RGB[3 * self.ap_target_class as usize + 0] as i32;
        self.ap_target_g = CLASS_RGB[3 * self.ap_target_class as usize + 1] as i32;
        self.ap_target_b = CLASS_RGB[3 * self.ap_target_class as usize + 2] as i32;
        self.ap_target_spin = 0;
        if self.ap_target_class == 11 {
            self.ap_target_spin = self.rng.random(30) + 1;
        }
        if self.ap_target_class == 7 {
            self.ap_target_spin = self.rng.random(12) + 1;
        }
        if self.ap_target_class == 2 {
            self.ap_target_spin = self.rng.random(4) + 1;
        }
    }

    fn prepare_nearstar(&mut self) {
        let deg = std::f64::consts::PI / 180.0;

        self.nearstar_class = self.ap_target_class;
        self.nearstar_x = self.ap_target_x;
        self.nearstar_y = self.ap_target_y;
        self.nearstar_z = self.ap_target_z;
        self.nearstar_ray = self.ap_target_ray as f32;
        self.nearstar_spin = self.ap_target_spin;
        self.nearstar_r = self.ap_target_r;
        self.nearstar_g = self.ap_target_g;
        self.nearstar_b = self.ap_target_b;

        self.s_m = (4.0 * std::f64::consts::PI / 3.0)
            * (self.nearstar_ray as f64)
            * (self.nearstar_ray as f64)
            * (self.nearstar_ray as f64)
            * 0.01e-7;

        self.nearstar_identity = F80::from_f64(self.nearstar_x)
            .div(&F80::from_f64(100000.0))
            .mul(&F80::from_f64(self.nearstar_y))
            .div(&F80::from_f64(100000.0))
            .mul(&F80::from_f64(self.nearstar_z))
            .div(&F80::from_f64(100000.0))
            .to_f64();

        // NB: Borland `long` is 32-bit. The multiplication chain
        // `((long)x%10000 * (long)y % 10000 * (long)z % 10000)` can overflow
        // 32-bit signed int; the result wraps modulo 2^32 exactly as Borland
        // does. Cast through i64 to force the 32-bit signed overflow, then
        // reduce mod 2^32.
        // NB: Borland `long` is 32-bit. The C expression is
        // `(long)x%10000 * (long)y % 10000 * (long)z % 10000` with `%` and `*`
        // at equal precedence, left-to-right: `(((x%10000)*y)%10000)*z)%10000`.
        // The intermediate `*` is 32-bit signed and WILL overflow for large y;
        // the result wraps modulo 2^32 exactly as Borland does. We must
        // multiply the FULL (long)y / (long)z, not their reduced forms.
        let sx = self.nearstar_x as i32 % 10000;
        let y = self.nearstar_y as i32;
        let z = self.nearstar_z as i32;
        let step1 = sx as i64 * y as i64; // 32-bit signed overflow on wrap
        let step1 = step1 as i32 % 10000;
        let step2 = step1 as i64 * z as i64;
        let seed = step2 as i32 % 10000;
        self.rng.srand(seed as u32);

        self.nearstar_nop = self.rng.random(CLASS_PLANETS[self.nearstar_class as usize] + 1);

        for n in 0..self.nearstar_nop as usize {
            self.p_owner[n] = -1;
            self.p_orb_orient[n] = deg * self.rng.random(360) as f64;
            self.p_orb_seed[n] = 3.0 * (n * n + 1) as f64 * (self.nearstar_ray as f64)
                + (self.rng.random(i16(300.0 * self.nearstar_ray as f64)) as f64) / 100.0;
            self.p_tilt[n] = self.rng.zrandom(i16(10.0 * self.p_orb_seed[n])) / 500.0;
            self.p_orb_tilt[n] = self.rng.zrandom(i16(10.0 * self.p_orb_seed[n])) / 5000.0;
            self.p_orb_ecc[n] = 1.0
                - (self.rng.random(i16(self.p_orb_seed[n] + 10.0 * self.p_orb_tilt[n].abs()))
                    as f64)
                    / 2000.0;
            self.p_ray[n] = (self.rng.random(i16(self.p_orb_seed[n])) as f64) * 0.001 + 0.01;
            self.p_ring[n] =
                self.rng.zrandom((self.p_ray[n]) as i32) as f64 * (1.0 + (self.rng.random(1000) as f64) / 100.0);
            if self.nearstar_class != 8 {
                self.p_type[n] = self.rng.random(PLANET_TYPES as i32) as i8;
            } else {
                if self.rng.random(2) != 0 {
                    self.p_type[n] = 10;
                    self.p_orb_tilt[n] *= 100.0;
                } else {
                    self.p_type[n] = self.rng.random(PLANET_TYPES as i32) as i8;
                }
            }
            if self.nearstar_class == 2 || self.nearstar_class == 7 || self.nearstar_class == 15 {
                self.p_orb_seed[n] *= 10.0;
            }
        }

        // Habitable boost on class 0.
        if self.nearstar_class == 0 {
            if self.rng.random(4) == 2 {
                self.p_type[2] = 3;
            }
            if self.rng.random(4) == 2 {
                self.p_type[3] = 3;
            }
            if self.rng.random(4) == 2 {
                self.p_type[4] = 3;
            }
        }

        // Phase 1: impossible planets for star class.
        for n in 0..self.nearstar_nop as usize {
            match self.nearstar_class {
                2 => {
                    while self.p_type[n] == 3 {
                        self.p_type[n] = self.rng.random(10) as i8;
                    }
                }
                5 => {
                    while self.p_type[n] == 6 || self.p_type[n] == 9 {
                        self.p_type[n] = self.rng.random(10) as i8;
                    }
                }
                7 => {
                    self.p_type[n] = 9;
                }
                9 => {
                    while self.p_type[n] != 0 && self.p_type[n] != 6 && self.p_type[n] != 9 {
                        self.p_type[n] = self.rng.random(10) as i8;
                    }
                }
                11 => {
                    while self.p_type[n] != 1 && self.p_type[n] != 7 {
                        self.p_type[n] = self.rng.random(10) as i8;
                    }
                }
                _ => {}
            }
        }

        // Phase 2: impossible for orbital distance.
        for n in 0..self.nearstar_nop as usize {
            match self.p_type[n] {
                0 => {
                    if self.rng.random(8) != 0 {
                        self.p_type[n] += 1;
                    }
                }
                3 => {
                    if (n < 2) || (n > 6) || (self.nearstar_class != 0 && self.rng.random(4) != 0) {
                        if self.rng.random(2) != 0 {
                            self.p_type[n] += 1;
                        } else {
                            self.p_type[n] -= 1;
                        }
                    }
                }
                7 => {
                    if n < 7 {
                        if self.rng.random(2) != 0 {
                            self.p_type[n] -= 1;
                        } else {
                            self.p_type[n] -= 2;
                        }
                    }
                }
                _ => {}
            }
        }

        self.nearstar_nob = self.nearstar_nop;

        if self.nearstar_class == 2 || self.nearstar_class == 7 || self.nearstar_class == 15 {
            // no moons
        } else {
            let mut n = 0;
            while n < self.nearstar_nop as usize {
                let s = self.p_type[n] as i32;
                let t = if n < 2 {
                    let mut t = 0;
                    if s == 10 {
                        t = self.rng.random(3);
                    }
                    t
                } else {
                    self.rng.random(PLANET_POSSIBLEMOONS[s as usize] + 1)
                };
                let mut t = t;
                if self.nearstar_nob + t > MAXBODIES as i32 {
                    t = MAXBODIES as i32 - self.nearstar_nob;
                }
                for c in 0..t as usize {
                    let q = self.nearstar_nob as usize + c;
                    self.p_owner[q] = n as i32;
                    self.p_moonid[q] = c as i8;
                    self.p_orb_orient[q] = deg * self.rng.random(360) as f64;
                    self.p_orb_seed[q] =
                        (c * c + 4) as f64 * self.p_ray[n] as f64
                            + (self.rng.zrandom(i16(300.0 * self.p_ray[n] as f64)) as f64) / 100.0;
                    self.p_tilt[q] = self.rng.zrandom(i16(10.0 * self.p_orb_seed[q])) / 50.0;
                    self.p_orb_tilt[q] = self.rng.zrandom(i16(10.0 * self.p_orb_seed[q])) / 500.0;
                    self.p_orb_ecc[q] = 1.0
                        - (self.rng.random(i16(self.p_orb_seed[q] + 10.0 * self.p_orb_tilt[q].abs()))
                            as f64)
                            / 2000.0;
                    self.p_ray[q] =
                        (self.rng.random(i16(self.p_orb_seed[n])) as f64) * 0.05 + 0.1;
                    self.p_ring[q] = 0.0;
                    let mut r = self.rng.random(PLANET_TYPES as i32);
                    if r == 9 && s != 10 {
                        r = 2;
                    }
                    if r == 6 && s < 9 {
                        r = 5;
                    }
                    if n > 7 && self.rng.random(c as i32) != 0 {
                        r = 7;
                    }
                    if n > 9 && self.rng.random(c as i32) != 0 {
                        r = 7;
                    }
                    if r == 2 || r == 3 || r == 4 || r == 8 {
                        if s != 6 && s < 9 {
                            r = 1;
                        }
                    }
                    if r == 3 && s < 9 {
                        if n > 7 {
                            r = 7;
                        }
                        if self.nearstar_class != 0 && self.rng.random(4) != 0 {
                            r = 5;
                        }
                        if self.nearstar_class == 2
                            || self.nearstar_class == 7
                            || self.nearstar_class == 11
                        {
                            r = 8;
                        }
                    }
                    if r == 7 && n <= 5 {
                        r = 1;
                    }
                    if (self.nearstar_class == 2
                        || self.nearstar_class == 5
                        || self.nearstar_class == 7
                        || self.nearstar_class == 11)
                        && self.rng.random(n as i32) != 0
                    {
                        r = 7;
                    }
                    self.p_type[q] = r as i8;
                }
                self.nearstar_nob += t;
                n += 1;
            }
        }

        // Kepler-normalisation of planet orbits.
        let mut key_radius = (self.nearstar_ray as f64) * PLANET_ORB_SCALING;
        if self.nearstar_class == 8 {
            key_radius *= 2.0;
        }
        if self.nearstar_class == 2 {
            key_radius *= 16.0;
        }
        if self.nearstar_class == 7 {
            key_radius *= 18.0;
        }
        if self.nearstar_class == 11 {
            key_radius *= 20.0;
        }
        for n in 0..self.nearstar_nop as usize {
            self.p_ray[n] = AVG_PLANET_RAY[self.p_type[n] as usize]
                + AVG_PLANET_RAY[self.p_type[n] as usize] * self.rng.zrandom(100) / 200.0;
            self.p_ray[n] *= AVG_PLANET_SIZING;
            self.p_orb_ray[n] = key_radius + key_radius * self.rng.zrandom(100) / 500.0;
            self.p_orb_ray[n] += key_radius * AVG_PLANET_RAY[self.p_type[n] as usize];
            if n < 8 {
                key_radius += self.p_orb_ray[n];
            } else {
                key_radius += 0.22 * self.p_orb_ray[n];
            }
        }

        // Moon orbit normalisation.
        let mut n = self.nearstar_nop as usize;
        while n < self.nearstar_nob as usize {
            let mut q = 0usize;
            let c = self.p_owner[n] as usize;
            let mut key_radius = self.p_ray[c] * MOON_ORB_SCALING;
            while n < self.nearstar_nob as usize && self.p_owner[n] == c as i32 {
                self.p_ray[n] = AVG_PLANET_RAY[self.p_type[n] as usize]
                    + AVG_PLANET_RAY[self.p_type[n] as usize] * self.rng.zrandom(100) / 200.0;
                self.p_ray[n] *= AVG_MOON_SIZING;
                self.p_orb_ray[n] = key_radius + key_radius * self.rng.zrandom(100) / 250.0;
                self.p_orb_ray[n] += key_radius * AVG_PLANET_RAY[self.p_type[n] as usize];
                if q < 2 {
                    key_radius += self.p_orb_ray[n];
                }
                if q >= 2 && q < 8 {
                    key_radius += 0.12 * self.p_orb_ray[n];
                }
                if q >= 8 {
                    key_radius += 0.025 * self.p_orb_ray[n];
                }
                q += 1;
                n += 1;
            }
        }

        // Ring elimination.
        for n in 0..self.nearstar_nop as usize {
            self.p_ring[n] = 0.75 * self.p_ray[n] * (2.0 + self.rng.random(3) as f64);
            let s = self.p_type[n] as i32;
            if s != 6 && s != 9 {
                if self.rng.random(5) != 0 {
                    self.p_ring[n] = 0.0;
                }
            } else {
                if self.rng.random(2) != 0 {
                    self.p_ring[n] = 0.0;
                }
            }
        }

        // Label count (search_id_code not reproduced; no named stars in fixtures).
        self.nearstar_labeled = 0;

        for n in 0..self.nearstar_nob as usize {
            self.p_rtperiod[n] = 0;
        }
    }
}
