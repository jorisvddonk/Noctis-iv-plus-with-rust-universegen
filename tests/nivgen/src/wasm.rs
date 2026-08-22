//! Raw `wasm32-unknown-unknown` exports for the Noctis IV Plus generator.
//!
//! No wasm-bindgen: the JS side drives the engine through plain C ABI
//! functions and reads the output buffers directly out of linear memory.
//! All buffers are returned as base pointers into WASM memory; the JS page
//! slices them out of `memory.buffer`.
//!
//! A single global `SystemGen` instance is kept alive across calls (the
//! generator is deterministic, so a whole system + sector pipeline is just:
//! `niv_generate` -> `niv_planet_surface` -> `niv_sector` -> `niv_sky`).
//!
//! ## Surface / sector buffer sharing
//!
//! The engine aliases several outputs onto the same buffers the way the
//! original C game does (`txtr == p_background`). Because of that, copies of
//! the *surface map* and the *atmosphere overlay* must be taken by the JS
//! side before generating a sector, or they are overwritten by the
//! 256x256 surface texture. The per-buffer FNV-1a hash exports hash the
//! *current* buffer contents, so they must be called immediately after the
//! matching generator call.

use crate::hash::fnv1a;
use crate::rng::{BorlandRng, FastRng};
use crate::system::{EngineY, SystemGen};
use std::ptr;

// 360*180 surface map.
const SURFACE_BYTES: usize = 64800;
// Sampled surface bytes for hashing, matching the C harness convention
// (rows 0..127; the tail rows are never rendered by the globe).
const SAMPLED_BYTES: usize = 360 * 128;
// 200x200 sector buffers.
const SECTOR_BYTES: usize = 40000;
// Atmosphere overlay, first 32400 bytes of objectschart.
const ATMO_BYTES: usize = 32400;

static mut GEN: *mut SystemGen = ptr::null_mut();
static mut RNG: *mut BorlandRng = ptr::null_mut();
static mut FRNG: *mut FastRng = ptr::null_mut();
static mut CUR_BODY: i32 = -1;

fn gen() -> &'static mut SystemGen {
    unsafe { &mut *GEN }
}

fn rng() -> &'static mut BorlandRng {
    unsafe { &mut *RNG }
}

fn frng() -> &'static mut FastRng {
    unsafe { &mut *FRNG }
}

fn cur_body() -> i32 {
    unsafe { CUR_BODY }
}

fn set_cur_body(p: i32) {
    unsafe { CUR_BODY = p }
}

#[no_mangle]
pub extern "C" fn niv_init() {
    unsafe {
        GEN = Box::into_raw(Box::new(SystemGen::new()));
        RNG = Box::into_raw(Box::new(BorlandRng::new()));
        FRNG = Box::into_raw(Box::new(FastRng::new()));
        set_cur_body(-1);
    }
}

// ============================== star system ==============================

/// Generate the system at Parsis coordinates. `y` is the user-facing
/// (galactic-chart) y; the engine-facing convention is derived internally.
#[no_mangle]
pub extern "C" fn niv_generate(x: f64, y: f64, z: f64) {
    let g = gen();
    g.generate(x, EngineY::from_user_y(y), z);
    set_cur_body(-1);
}

#[no_mangle]
pub extern "C" fn niv_nob() -> i32 {
    gen().nearstar_nob
}

#[no_mangle]
pub extern "C" fn niv_nop() -> i32 {
    gen().nearstar_nop
}

#[no_mangle]
pub extern "C" fn niv_star_class() -> i32 {
    gen().nearstar_class
}

#[no_mangle]
pub extern "C" fn niv_star_ray() -> f64 {
    gen().nearstar_ray as f64
}

#[no_mangle]
pub extern "C" fn niv_star_spin() -> i32 {
    gen().nearstar_spin
}

#[no_mangle]
pub extern "C" fn niv_star_r() -> i32 {
    gen().nearstar_r
}

#[no_mangle]
pub extern "C" fn niv_star_g() -> i32 {
    gen().nearstar_g
}

#[no_mangle]
pub extern "C" fn niv_star_b() -> i32 {
    gen().nearstar_b
}

#[no_mangle]
pub extern "C" fn niv_star_identity() -> f64 {
    gen().nearstar_identity.to_f64()
}

#[no_mangle]
pub extern "C" fn niv_star_x() -> f64 {
    gen().nearstar_x
}

#[no_mangle]
pub extern "C" fn niv_star_y() -> f64 {
    gen().nearstar_y
}

#[no_mangle]
pub extern "C" fn niv_star_z() -> f64 {
    gen().nearstar_z
}

// ------------------------------ body accessors ------------------------------

#[no_mangle]
pub extern "C" fn niv_body_type(i: i32) -> i32 {
    gen().p_type[i as usize] as i32
}

#[no_mangle]
pub extern "C" fn niv_body_owner(i: i32) -> i32 {
    gen().p_owner[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_moonid(i: i32) -> i32 {
    gen().p_moonid[i as usize] as i32
}

#[no_mangle]
pub extern "C" fn niv_body_ray(i: i32) -> f64 {
    gen().p_ray[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_orb_ray(i: i32) -> f64 {
    gen().p_orb_ray[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_orb_tilt(i: i32) -> f64 {
    gen().p_orb_tilt[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_orb_orient(i: i32) -> f64 {
    gen().p_orb_orient[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_orb_ecc(i: i32) -> f64 {
    gen().p_orb_ecc[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_tilt(i: i32) -> f64 {
    gen().p_tilt[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_ring(i: i32) -> f64 {
    gen().p_ring[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_rtperiod(i: i32) -> i32 {
    gen().p_rtperiod[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_rotation(i: i32) -> i32 {
    gen().p_rotation[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_term_start(i: i32) -> i32 {
    gen().p_term_start[i as usize]
}

#[no_mangle]
pub extern "C" fn niv_body_term_end(i: i32) -> i32 {
    gen().p_term_end[i as usize]
}

// ============================== planet surface ==============================

/// Generate the 360x180 surface map, atmosphere overlay and 64-colour
/// palette for body `p`. Afterwards `niv_surface_ptr`, `niv_atmo_ptr` and
/// `niv_pal_ptr` point at the results (JS must copy them before a sector
/// generation overwrites the shared buffers).
#[no_mangle]
pub extern "C" fn niv_planet_surface(p: i32) {
    let g = gen();
    if p < 0 || p as usize >= g.nearstar_nob as usize {
        return;
    }
    set_cur_body(p);
    g.gen_planet_surface(p);
}

/// Base pointer of the 360x180 surface map (64800 bytes). For a moon this
/// is the moon surface buffer; for a planet the planet buffer.
#[no_mangle]
pub extern "C" fn niv_surface_ptr() -> u32 {
    let g = gen();
    if cur_body() < 0 {
        return 0;
    }
    let b = if g.p_owner[cur_body() as usize] > -1 {
        &g.s_background
    } else {
        &g.p_background
    };
    b.as_ptr() as u32
}

/// Base pointer of the 360x180 atmosphere overlay (first 32400 bytes of
/// objectschart).
#[no_mangle]
pub extern "C" fn niv_atmo_ptr() -> u32 {
    gen().objectschart.as_ptr() as u32
}

/// Base pointer of the 256-colour palette (768 bytes). The planet surface
/// uses base 192*3, moons 128*3 (see `niv_pal_base`).
#[no_mangle]
pub extern "C" fn niv_pal_ptr() -> u32 {
    gen().tmppal.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn niv_pal_base() -> u32 {
    if cur_body() < 0 {
        return 192 * 3;
    }
    if gen().p_owner[cur_body() as usize] > -1 {
        128 * 3
    } else {
        192 * 3
    }
}

// ============================== landing sector ==============================

/// Build the landing sector for body `p` at (lon, lat). Runs the planet
/// surface first, then `setup_landing` + `build_surface`, producing the
/// 200x200 heightmap, 200x200 object chart and the 256x256 surface texture.
/// Returns nonzero on success.
#[no_mangle]
pub extern "C" fn niv_sector(p: i32, lon: i32, lat: i32, albedo: i32, sctype: i32) -> i32 {
    let g = gen();
    if p < 0 || p as usize >= g.nearstar_nob as usize {
        return 0;
    }
    set_cur_body(p);
    g.gen_planet_surface(p);
    g.setup_landing(p, lon, lat, albedo, sctype);
    g.build_surface();
    1
}

/// Base pointer of the 200x200 sector heightmap (40000 bytes).
#[no_mangle]
pub extern "C" fn niv_heightmap_ptr() -> u32 {
    gen().p_surfacemap.as_ptr() as u32
}

/// Base pointer of the 200x200 object chart (40000 bytes).
#[no_mangle]
pub extern "C" fn niv_objectchart_ptr() -> u32 {
    gen().objectschart.as_ptr() as u32
}

/// Base pointer of the 256x256 surface texture (p_background, 65536 bytes).
#[no_mangle]
pub extern "C" fn niv_surftex_ptr() -> u32 {
    gen().p_background.as_ptr() as u32
}

/// Base pointer of the sky texture (360x180, first 64800 bytes of
/// s_background). Valid only after `niv_sky`.
#[no_mangle]
pub extern "C" fn niv_sky_ptr() -> u32 {
    gen().s_background.as_ptr() as u32
}

/// Generate the sky texture + environment for the current landing sector,
/// mirroring the C harness `cmd_surftex` (per-type brightness/atmosphere
/// overrides, sky fill, `create_sky`). After this call the sky gradient
/// lives at palette entries 64..128.
#[no_mangle]
pub extern "C" fn niv_sky() {
    let g = gen();
    let p = cur_body() as usize;
    match g.p_type[p] {
        1 | 4 | 7 => {
            g.sky_brightness = 0;
            g.atmosphere = 0;
        }
        2 => {
            g.sky_brightness = 63 - g.nightzone * 31;
        }
        _ => {}
    }
    let sky_fill = g.sky_brightness as u8;
    for i in 0..SURFACE_BYTES {
        g.s_background[i] = sky_fill;
    }
    g.create_sky(g.atmosphere);
}

#[no_mangle]
pub extern "C" fn niv_sctype() -> i32 {
    gen().sctype
}

#[no_mangle]
pub extern "C" fn niv_albedo() -> i32 {
    gen().albedo
}

#[no_mangle]
pub extern "C" fn niv_latitude() -> f64 {
    gen().latitude as f64
}

#[no_mangle]
pub extern "C" fn niv_nightzone() -> i32 {
    gen().nightzone
}

#[no_mangle]
pub extern "C" fn niv_global_surface_seed() -> f64 {
    gen().global_surface_seed as f64
}

#[no_mangle]
pub extern "C" fn niv_t_scale() -> f64 {
    gen().t_scale as f64
}

#[no_mangle]
pub extern "C" fn niv_quartz() -> i32 {
    gen().quartz
}

#[no_mangle]
pub extern "C" fn niv_groundflares() -> i32 {
    gen().groundflares
}

#[no_mangle]
pub extern "C" fn niv_rockscaling() -> f64 {
    gen().rockscaling as f64
}

#[no_mangle]
pub extern "C" fn niv_rockpeaking() -> f64 {
    gen().rockpeaking as f64
}

#[no_mangle]
pub extern "C" fn niv_rockdensity() -> i32 {
    gen().rockdensity
}

#[no_mangle]
pub extern "C" fn niv_treescaling() -> f64 {
    gen().treescaling as f64
}

#[no_mangle]
pub extern "C" fn niv_env_temp() -> f64 {
    gen().pp_temp
}

#[no_mangle]
pub extern "C" fn niv_env_pressure() -> f64 {
    gen().pp_pressure
}

#[no_mangle]
pub extern "C" fn niv_sky_brightness() -> i32 {
    gen().sky_brightness
}

// ============================== FNV-1a hashes ==============================
// Determinism proof: hash each output the same way the verification DB does.

#[no_mangle]
pub extern "C" fn niv_surface_fnv() -> u32 {
    if cur_body() < 0 {
        return 0;
    }
    let g = gen();
    if g.p_owner[cur_body() as usize] > -1 {
        fnv1a(&g.s_background[..SAMPLED_BYTES])
    } else {
        fnv1a(&g.p_background[..SAMPLED_BYTES])
    }
}

#[no_mangle]
pub extern "C" fn niv_atmo_fnv() -> u32 {
    fnv1a(&gen().objectschart[..ATMO_BYTES])
}

#[no_mangle]
pub extern "C" fn niv_pal_fnv() -> u32 {
    let g = gen();
    let base = niv_pal_base() as usize;
    fnv1a(&g.tmppal[base..base + 192])
}

#[no_mangle]
pub extern "C" fn niv_heightmap_fnv() -> u32 {
    fnv1a(&gen().p_surfacemap[..SECTOR_BYTES])
}

#[no_mangle]
pub extern "C" fn niv_objectchart_fnv() -> u32 {
    fnv1a(&gen().objectschart[..SECTOR_BYTES])
}

/// 254x256 = 65024 bytes, matching the C harness surftex hashing rule.
#[no_mangle]
pub extern "C" fn niv_surftex_fnv() -> u32 {
    fnv1a(&gen().p_background[..65024])
}

#[no_mangle]
pub extern "C" fn niv_sky_fnv() -> u32 {
    fnv1a(&gen().s_background[..SAMPLED_BYTES])
}

// ============================== RNG lab ==============================
// Drive the exact Borland `rand()`/`fast_random()` sequences to demo the
// deterministic RNGs that back the whole generator.

#[no_mangle]
pub extern "C" fn niv_rng_srand(seed: u32) {
    rng().srand(seed);
}

#[no_mangle]
pub extern "C" fn niv_rng_rand() -> i32 {
    rng().rand()
}

#[no_mangle]
pub extern "C" fn niv_rng_random(n: i32) -> i32 {
    rng().random(n)
}

#[no_mangle]
pub extern "C" fn niv_rng_flandom() -> f64 {
    rng().flandom()
}

#[no_mangle]
pub extern "C" fn niv_frng_srand(seed: u32) {
    frng().srand(seed);
}

#[no_mangle]
pub extern "C" fn niv_frng_rand(mask: u32) -> u32 {
    frng().fast_random(mask)
}
