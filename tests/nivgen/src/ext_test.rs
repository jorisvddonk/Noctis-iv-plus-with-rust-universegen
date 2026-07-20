#[test]
fn f32_contrast_accuracy() {
    let sm = std::fs::read("/var/folders/pz/q47m7vhj6fz11w2wy4w528dc0000gn/T/tmp.kwbJvyL9Am/C5_SM.BIN").unwrap();
    let ct_c = std::fs::read("/var/folders/pz/q47m7vhj6fz11w2wy4w528dc0000gn/T/tmp.kwbJvyL9Am/C5_CT.BIN").unwrap();
    
    // Body 1 args
    let kt: f32 = 0.7622222;
    let kq: f32 = 4.21;
    let thr: f32 = 27.0;
    
    let mut diffs = 0;
    for i in 0..64800 {
        let pixel = sm[i] as f32;
        let mut a = pixel - thr;
        if a > 0.0 { a = a * kt; } else { a = a * kq; }
        a = a + thr;
        if a < 0.0 { a = 0.0; }
        if a > 63.0 { a = 63.0; }
        let r = a as u8;
        if r != ct_c[i] { diffs += 1; }
        if i < 30 && r != ct_c[i] { println!("  {}: pixel={} C={} R={}", i, pixel, ct_c[i], r); }
    }
    println!("f32 diffs: {}", diffs);
    if diffs == 0 { println!("PERFECT MATCH!"); }
}
