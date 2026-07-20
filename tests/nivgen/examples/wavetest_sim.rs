// Throwaway simulation: replicate C `wave()` asm vs current Rust `wave()`.
// C: for px=360..1: py = rne(3 * sin(px * a_f32)) + cy ; write buf[py*360+px]
// where buf index is mod 65_536 (the +4 segment offset cancels the buffer base at 4).

fn rne(x: f64) -> i32 {
    // round half to even, like x87 fistp with default rounding
    let f = x.floor();
    let c = x.ceil();
    let fr = x - f;
    if fr < 0.5 {
        f as i32
    } else if fr > 0.5 {
        c as i32
    } else {
        // exactly .5 -> even
        if (f as i64) % 2 == 0 {
            f as i32
        } else {
            c as i32
        }
    }
}

// C-style: a as f32, product in 80-bit (approx via f64 of the f32 value), sin in f64
fn c_wave(a_f32: f32, cr: i32, cy: i32) -> Vec<u8> {
    let mut buf = vec![0u8; 64800];
    let a = a_f32 as f64; // f32 value widened to f64 (matches x87 fmul with 32-bit operand)
    let mut px = 360i32;
    while px > 0 {
        let prod = (cr as f64) * ((px as f64) * a).sin();
        let py = (rne(prod) + cy) & 0xFFFF;
        let di = (((py as i64 * 360) & 0xFFFF) + (px as i64) ) & 0xFFFF;
        if (di as usize) < buf.len() {
            buf[di as usize] = 0; // already 0, but mark
        }
        px -= 1;
    }
    buf
}

// Rust-style: same but note the C write is at py*360+px (mod 65536).
// The 344-byte residual: does differing on sin precision matter? count which px differ
// vs a higher-precision reference.
fn main() {
    let a_f32: f32 = 8.0 / 30.0; // (float)8/(float)30
    let cr = 3;
    let cy = 146;
    let buf = c_wave(a_f32, cr, cy);

    // Check: which distinct rows get written (py values), and how many px per row.
    let mut rows: std::collections::BTreeMap<i32, i32> = Default::default();
    let mut px = 360i32;
    let a = a_f32 as f64;
    while px > 0 {
        let prod = (cr as f64) * ((px as f64) * a).sin();
        let py = (rne(prod) + cy) & 0xFFFF;
        *rows.entry(py).or_insert(0) += 1;
        px -= 1;
    }
    println!("rows written (py): count of px each");
    for (py, n) in &rows {
        println!("  py={}  px_count={}", py, n);
    }
    println!("total distinct rows={}  total px={}", rows.len(), rows.values().sum::<i32>());

    // Compare to a reference with extra precision: compute arg as f64 but using the
    // f32-rounded a, and sin via f64 — that's what we have. The question is whether the
    // C 80-bit product * a vs f64 product * a differs. Compute the f64 product and the
    // "exact" product (px * f64(a)) — they are identical since px is exact and a is the
    // same f64 value. So the ONLY difference between C and f64 is x87 fsin vs libm sin.
    // Print px where sin is near a half-integer boundary (where rounding is sensitive).
    println!("\npx where 3*sin(px*a) is within 1e-3 of a half-integer (sin-precision sensitive):");
    let mut sens = 0;
    px = 360;
    while px > 0 {
        let v = (cr as f64) * ((px as f64) * a).sin();
        let frac = (v - v.floor()).abs();
        let dist = (frac - 0.5).abs();
        if dist < 1e-3 {
            println!("  px={} 3sin={:.6} py_raw={}", px, v, rne(v) + cy);
            sens += 1;
        }
        px -= 1;
    }
    println!("sensitive px count={}", sens);
}
