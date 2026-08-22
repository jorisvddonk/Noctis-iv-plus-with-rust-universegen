//! Tests for the `unsafe` pixel-smoothing loops in `surface.rs`
//! (`ssmooth`, `lssmooth`, `psmooth_grays`).
//!
//! Each unsafe function is compared byte-for-byte against an independent,
//! **safe** reference implementation that uses plain slice indexing instead of
//! raw pointers. The references are written to be drop-in replacements, so
//! once the unsafe loops are converted to safe Rust they can simply be swapped
//! in and these tests must keep passing unchanged.
//!
//! Cross-iteration aliasing is intentional and preserved: the C loops write
//! `buf[di]` while later iterations read it back via `di - 360` (etc.), so the
//! reference writes into the same slice and naturally sees prior writes.

use crate::system::SystemGen;

/// Deterministic pseudo-random byte fill (LCG, mirrors a surface-map feel).
fn fill_bg(g: &mut SystemGen, seed: u32) {
    let mut s = seed;
    for b in g.p_background.iter_mut() {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        *b = (s >> 24) as u8;
    }
}

/// Uniform fill — exercises the arithmetic-independent clamp paths.
fn fill_uniform(g: &mut SystemGen, v: u8) {
    g.p_background.fill(v);
}

/// Little-endian u32 read, safe slice version of the unsafe `w()` closure.
fn w(buf: &[u8], off: usize) -> u32 {
    buf[off] as u32
        | (buf[off + 1] as u32) << 8
        | (buf[off + 2] as u32) << 16
        | (buf[off + 3] as u32) << 24
}

/// Safe reference for `SystemGen::ssmooth` (NOCTIS-0.CPP:4380).
fn ref_ssmooth(buf: &mut [u8], q: i32) {
    let cx = (q * 4 - 360 * 4) as usize;
    let mut di = 360usize;
    for _ in 0..cx {
        let mut edx = w(buf, di - 360)
            .wrapping_add(w(buf, di))
            .wrapping_add(w(buf, di + 360))
            .wrapping_add(w(buf, di + 720));
        edx &= 0xFCFCFCFC;
        edx >>= 2;
        let mut al = (edx & 0xFF) as u8;
        al = al.wrapping_add(((edx >> 8) & 0xFF) as u8);
        edx >>= 16;
        al = al.wrapping_add((edx & 0xFF) as u8);
        al = al.wrapping_add(((edx >> 8) & 0xFF) as u8);
        al >>= 2;
        buf[di] = al;
        di += 1;
    }
}

/// Safe reference for `SystemGen::lssmooth` (NOCTIS-0.CPP:4416).
fn ref_lssmooth(buf: &mut [u8], q: i32) {
    let cx = (q * 4 - 80 * 4) as usize;
    let mut di = 0usize;
    for _ in 0..cx {
        let d0 = buf[di] as u32;
        let d1 = buf[di + 1] as u32;
        let b0 = buf[di + 360] as u32;
        let b1 = buf[di + 361] as u32;
        let dx = (d0 | (d1 << 8)) & 0x3F3F;
        let bx = (b0 | (b1 << 8)) & 0x3F3F;
        let mut al = (d0 as u8) & 0xC0;
        let mut dl = (d0 & 0x3F) as u8;
        dl = dl.wrapping_add((dx >> 8) as u8);
        dl = dl.wrapping_add((bx & 0xFF) as u8);
        dl = dl.wrapping_add(((bx >> 8) & 0xFF) as u8);
        dl >>= 2;
        al |= dl;
        buf[di] = al;
        di += 1;
    }
}

/// Safe reference for `SystemGen::psmooth_grays` (NOCTIS-0.CPP:480).
fn ref_psmooth_grays(buf: &mut [u8], q: i32) {
    let cx = (q * 4 - 320 * 4) as usize;
    let mut di = 320usize;
    for _ in 0..cx {
        let mut edx = w(buf, di - 320)
            .wrapping_add(w(buf, di))
            .wrapping_add(w(buf, di + 320))
            .wrapping_add(w(buf, di + 640));
        edx &= 0xFCFCFCFC;
        edx >>= 2;
        let mut al = (edx & 0xFF) as u8;
        al = al.wrapping_add(((edx >> 8) & 0xFF) as u8);
        edx >>= 16;
        al = al.wrapping_add((edx & 0xFF) as u8);
        al = al.wrapping_add(((edx >> 8) & 0xFF) as u8);
        al >>= 2;
        buf[di] = al;
        di += 1;
    }
}

fn eq_ssmooth(seed: u32, q: i32, label: &str) {
    let mut a = SystemGen::new();
    a.quadwords = q;
    fill_bg(&mut a, seed);
    let mut b = SystemGen::new();
    b.quadwords = q;
    fill_bg(&mut b, seed);

    a.ssmooth();
    ref_ssmooth(&mut b.p_background, q);

    assert_eq!(
        a.p_background, b.p_background,
        "ssmooth mismatch (seed={seed}, q={q}, {label})"
    );
}

fn eq_lssmooth(seed: u32, q: i32, label: &str) {
    let mut a = SystemGen::new();
    a.quadwords = q;
    fill_bg(&mut a, seed);
    let mut b = SystemGen::new();
    b.quadwords = q;
    fill_bg(&mut b, seed);

    a.lssmooth();
    ref_lssmooth(&mut b.p_background, q);

    assert_eq!(
        a.p_background, b.p_background,
        "lssmooth mismatch (seed={seed}, q={q}, {label})"
    );
}

fn eq_psmooth_grays(seed: u32, q: i32, label: &str) {
    let mut a = SystemGen::new();
    a.quadwords = q;
    fill_bg(&mut a, seed);
    let mut b = SystemGen::new();
    b.quadwords = q;
    fill_bg(&mut b, seed);

    a.psmooth_grays();
    ref_psmooth_grays(&mut b.p_background, q);

    assert_eq!(
        a.p_background, b.p_background,
        "psmooth_grays mismatch (seed={seed}, q={q}, {label})"
    );
}

#[test]
fn ssmooth_matches_safe_reference() {
    // Production quadwords is 16000; also cover a range of q values so the
    // loop extents (di - 360 .. di + 720) hit different buffer regions.
    for q in [16000, 3600, 1600, 720, 360] {
        for seed in [0, 1, 0xDEADBEEF, 0xFFFF_FFFE] {
            eq_ssmooth(seed, q, "random");
        }
        let mut a = SystemGen::new();
        a.quadwords = q;
        fill_uniform(&mut a, 0);
        let mut b = SystemGen::new();
        b.quadwords = q;
        fill_uniform(&mut b, 0);
        a.ssmooth();
        ref_ssmooth(&mut b.p_background, q);
        assert_eq!(a.p_background, b.p_background, "ssmooth uniform-0 q={q}");
        let mut a = SystemGen::new();
        a.quadwords = q;
        fill_uniform(&mut a, 0xFF);
        let mut b = SystemGen::new();
        b.quadwords = q;
        fill_uniform(&mut b, 0xFF);
        a.ssmooth();
        ref_ssmooth(&mut b.p_background, q);
        assert_eq!(a.p_background, b.p_background, "ssmooth uniform-FF q={q}");
    }
}

#[test]
fn lssmooth_matches_safe_reference() {
    for q in [16000, 3600, 1600, 720, 360] {
        for seed in [0, 1, 0xDEADBEEF, 0xFFFF_FFFE] {
            eq_lssmooth(seed, q, "random");
        }
        let mut a = SystemGen::new();
        a.quadwords = q;
        fill_uniform(&mut a, 0);
        let mut b = SystemGen::new();
        b.quadwords = q;
        fill_uniform(&mut b, 0);
        a.lssmooth();
        ref_lssmooth(&mut b.p_background, q);
        assert_eq!(a.p_background, b.p_background, "lssmooth uniform-0 q={q}");
    }
}

#[test]
fn psmooth_grays_matches_safe_reference() {
    for q in [16000, 3600, 1600, 720, 360] {
        for seed in [0, 1, 0xDEADBEEF, 0xFFFF_FFFE] {
            eq_psmooth_grays(seed, q, "random");
        }
        let mut a = SystemGen::new();
        a.quadwords = q;
        fill_uniform(&mut a, 0);
        let mut b = SystemGen::new();
        b.quadwords = q;
        fill_uniform(&mut b, 0);
        a.psmooth_grays();
        ref_psmooth_grays(&mut b.p_background, q);
        assert_eq!(a.p_background, b.p_background, "psmooth uniform-0 q={q}");
    }
}

#[test]
fn smoothers_are_deterministic() {
    // Same input -> same output across separate instances.
    let mut a = SystemGen::new();
    a.quadwords = 16000;
    fill_bg(&mut a, 42);
    let mut b = SystemGen::new();
    b.quadwords = 16000;
    fill_bg(&mut b, 42);

    a.ssmooth();
    a.lssmooth();
    a.psmooth_grays();
    b.ssmooth();
    b.lssmooth();
    b.psmooth_grays();

    assert_eq!(a.p_background, b.p_background, "smoothers not deterministic");
}

#[test]
fn production_quadwords_stays_in_bounds() {
    // Guard the raw-pointer loops: at q=16000 the maximum offset read must
    // stay inside the 65536-byte p_background buffer.
    let mut g = SystemGen::new();
    g.quadwords = 16000;
    fill_bg(&mut g, 7);
    // Would panic (OOB) if any offset escaped the buffer.
    g.ssmooth();
    g.lssmooth();
    g.psmooth_grays();
}