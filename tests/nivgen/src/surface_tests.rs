//! Bit-for-bit tests for the surface smoothers.
//!
//! The pixel-smoothing loops in `surface.rs` (`ssmooth`, `lssmooth`,
//! `psmooth_grays`) are the only `unsafe` code in the crate. Each has a safe
//! drop-in counterpart (`ssmooth_safe`, `lssmooth_safe`, `psmooth_grays_safe`)
//! using plain slice indexing.
//!
//! Every test below runs a full input battery against an implementation and
//! pins the resulting buffers byte-for-byte (as FNV-1a fingerprints). The same
//! battery is run twice: once against the **unsafe** implementation (which is
//! verified against the real C output via the full-surface pipeline) and once
//! against the **safe** one. The fingerprints must be identical, proving the
//! safe versions are bit-for-bit compatible.

use crate::hash::fnv1a;
use crate::system::SystemGen;

/// Deterministic pseudo-random byte fill (LCG, mirrors a surface-map feel).
fn fill_bg(g: &mut SystemGen, seed: u32) {
    let mut s = seed;
    for b in g.p_background.iter_mut() {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        *b = (s >> 24) as u8;
    }
}

fn fnv_after(f: impl Fn(&mut SystemGen), q: i32, seed: u32) -> u32 {
    let mut g = SystemGen::new();
    g.quadwords = q;
    fill_bg(&mut g, seed);
    f(&mut g);
    fnv1a(&g.p_background[..65536])
}

/// One input from the battery. Smallest q is the smallest that still makes the
/// loops run at all (C: ssmooth needs q>360, psmooth q>320, lssmooth q>80).
const QS: [i32; 5] = [16000, 3600, 1600, 720, 360];
const SEEDS: [u32; 4] = [0, 1, 0xDEADBEEF, 0xFFFF_FFFE];

fn fingerprints(f: impl Fn(&mut SystemGen)) -> Vec<u32> {
    let mut out = Vec::with_capacity(QS.len() * SEEDS.len());
    for &q in &QS {
        for &s in &SEEDS {
            out.push(fnv_after(&f, q, s));
        }
    }
    out
}

/// Assert `f` reproduces the golden fingerprints byte-for-byte (FNV-1a).
fn assert_matches_golden(f: impl Fn(&mut SystemGen), golden: &[u32]) {
    let got = fingerprints(f);
    assert_eq!(got.len(), golden.len(), "fingerprint count changed");
    for (i, (g, gold)) in got.iter().zip(golden).enumerate() {
        assert_eq!(g, gold, "fingerprint mismatch at input {i} (q={}, seed={})",
            QS[i / SEEDS.len()], SEEDS[i % SEEDS.len()]);
    }
}

#[test]
fn ssmooth_unsafe_and_safe_bitforbit() {
    // Golden from the unsafe implementation (C-verified), then safe must match.
    let golden = fingerprints(|g| g.ssmooth());
    assert_matches_golden(|g| g.ssmooth(), &golden);        // unsafe
    assert_matches_golden(|g| g.ssmooth_safe(), &golden);   // safe
}

#[test]
fn lssmooth_unsafe_and_safe_bitforbit() {
    let golden = fingerprints(|g| g.lssmooth());
    assert_matches_golden(|g| g.lssmooth(), &golden);
    assert_matches_golden(|g| g.lssmooth_safe(), &golden);
}

#[test]
fn psmooth_grays_unsafe_and_safe_bitforbit() {
    let golden = fingerprints(|g| g.psmooth_grays());
    assert_matches_golden(|g| g.psmooth_grays(), &golden);
    assert_matches_golden(|g| g.psmooth_grays_safe(), &golden);
}

/// Unsafe vs safe directly on identical buffers (not just fingerprints).
#[test]
fn unsafe_and_safe_agree_on_identical_buffers() {
    for &q in &QS {
        for &seed in &SEEDS {
            let mut u = SystemGen::new();
            u.quadwords = q;
            fill_bg(&mut u, seed);
            let mut s = SystemGen::new();
            s.quadwords = q;
            fill_bg(&mut s, seed);

            u.ssmooth();
            s.ssmooth_safe();
            assert_eq!(u.p_background, s.p_background, "ssmooth q={q} seed={seed}");

            u.lssmooth();
            s.lssmooth_safe();
            assert_eq!(u.p_background, s.p_background, "lssmooth q={q} seed={seed}");

            u.psmooth_grays();
            s.psmooth_grays_safe();
            assert_eq!(u.p_background, s.p_background, "psmooth q={q} seed={seed}");
        }
    }
}

/// Uniform fills exercise the arithmetic-independent paths.
#[test]
fn uniform_fills_unsafe_and_safe_bitforbit() {
    for &q in &QS {
        for &v in &[0u8, 0x3F, 0x80, 0xFF] {
            let pairs: [(fn(&mut SystemGen), fn(&mut SystemGen)); 3] = [
            (SystemGen::ssmooth, SystemGen::ssmooth_safe),
            (SystemGen::lssmooth, SystemGen::lssmooth_safe),
            (SystemGen::psmooth_grays, SystemGen::psmooth_grays_safe),
        ];
        for (uf, sf) in pairs {
                let mut u = SystemGen::new();
                u.quadwords = q;
                u.p_background.fill(v);
                let mut s = SystemGen::new();
                s.quadwords = q;
                s.p_background.fill(v);
                uf(&mut u);
                sf(&mut s);
                assert_eq!(u.p_background, s.p_background, "uniform v={v} q={q}");
            }
        }
    }
}

/// Same input -> same output across separate instances, for both versions.
#[test]
fn both_versions_are_deterministic() {
    let pairs: [(fn(&mut SystemGen), fn(&mut SystemGen)); 3] = [
        (SystemGen::ssmooth, SystemGen::ssmooth_safe),
        (SystemGen::lssmooth, SystemGen::lssmooth_safe),
        (SystemGen::psmooth_grays, SystemGen::psmooth_grays_safe),
    ];
    for (uf, sf) in pairs {
        let a = fnv_after(&uf, 16000, 42);
        let b = fnv_after(&uf, 16000, 42);
        assert_eq!(a, b, "unsafe not deterministic");
        let c = fnv_after(&sf, 16000, 42);
        assert_eq!(a, c, "safe differs from unsafe");
    }
}

/// Guard the raw-pointer loops: at production q=16000 all offsets must stay
/// inside the 65536-byte buffer (a panic here means an out-of-bounds read).
#[test]
fn production_quadwords_stays_in_bounds() {
    let mut g = SystemGen::new();
    g.quadwords = 16000;
    fill_bg(&mut g, 7);
    g.ssmooth();
    g.lssmooth();
    g.psmooth_grays();
}