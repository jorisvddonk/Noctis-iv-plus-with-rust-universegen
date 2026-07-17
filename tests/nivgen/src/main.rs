use nivgen::rng::{BorlandRng, FastRng};
use nivgen::system::SystemGen;
use nivgen::hash::{fnv1a, crc32, FNV_OFFSET};
use std::fs::OpenOptions;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: nivgen <command> [opts]");
        std::process::exit(1);
    }
    let cmd = args[1].as_str();
    match cmd {
        "selftest" => cmd_selftest(&args),
        "system" => cmd_system(&args),
        "scan" => cmd_scan(&args),
        "planet" => cmd_planet(&args),
        "sector" => cmd_sector(&args),
        "surftex" => cmd_surftex(&args),
        "planet" => cmd_planet(&args),
        _ => {
            eprintln!("unknown command: {}", cmd);
            std::process::exit(1);
        }
    }
}

fn argd(args: &[String], key: &str, def: f64) -> f64 {
    if let Some(pos) = args.iter().position(|a| a == key) {
        if let Some(v) = args.get(pos + 1) {
            return v.parse().unwrap_or(def);
        }
    }
    def
}

fn argl(args: &[String], key: &str, def: i64) -> i64 {
    if let Some(pos) = args.iter().position(|a| a == key) {
        if let Some(v) = args.get(pos + 1) {
            return v.parse().unwrap_or(def);
        }
    }
    def
}

fn out_file(args: &[String]) -> Option<std::fs::File> {
    if let Some(pos) = args.iter().position(|a| a == "-o") {
        if let Some(path) = args.get(pos + 1) {
            if let Ok(f) = OpenOptions::new().create(true).write(true).truncate(true).open(path) {
                return Some(f);
            }
        }
    }
    None
}

macro_rules! emit {
    ($f:expr, $($arg:tt)*) => {
        {
            let s = format!($($arg)*).replace('\n', "\r\n");
            print!("{}", s);
            if let Some(f) = $f.as_mut() { let _ = f.write_all(s.as_bytes()); }
        }
    };
}

fn cmd_selftest(args: &[String]) {
    let mut f = out_file(args);
    emit!(f, "=== SELFTEST ===\n");
    emit!(f, "-- hash vectors --\n");
    for (s, fv, cv) in [
        ("", 0x811C9DC5u32, 0x00000000u32),
        ("a", 0xE40C292C, 0xE8B7BE43),
        ("abc", 0x1A47E90B, 0x352441C2),
        ("The quick brown fox jumps over the lazy dog", 0x048FFF90, 0x414FA339),
    ] {
        emit!(f, "  \"{}\" fnv={:08X} crc={:08X}\n", s, fnv1a(s.as_bytes()), crc32(s.as_bytes()));
        let _ = (fv, cv);
    }
    emit!(f, "-- Borland random() sequence, srand(1) --\n");
    let mut r = BorlandRng::new();
    r.srand(1);
    for _ in 0..8 {
        emit!(f, "  rand={}\n", r.rand());
    }
    emit!(f, "-- fast_random() sequence, fast_srand(12345) --\n");
    let mut fr = FastRng::new();
    fr.srand(12345);
    for _ in 0..8 {
        emit!(f, "  fast={}\n", fr.fast_random(0x7FFF));
    }
    emit!(f, "-- flandom()/fast_flandom() --\n");
    let mut r = BorlandRng::new();
    r.srand(1);
    emit!(f, "  flandom={:.8}\n", r.flandom());
    let mut fr = FastRng::new();
    fr.srand(1);
    emit!(f, "  fast_flandom={:.8}\n", fr.fast_flandom());
}

fn cmd_system(args: &[String]) {
    let mut f = out_file(args);
    let x = argd(args, "-x", 0.0);
    let y = argd(args, "-y", 0.0);
    let z = argd(args, "-z", 0.0);
    let mut g = SystemGen::new();
    g.generate(x, y, z);
    emit!(f, "=== SYSTEM ===\n");
    emit!(f, "STAR x={:.0} y={:.0} z={:.0}\n", g.ap_target_x, g.ap_target_y, g.ap_target_z);
    emit!(f, "  identity={:.6}\n", g.nearstar_identity);
    emit!(f, "  class={} ray={:.6} spin={} rgb={},{},{}\n",
        g.nearstar_class, g.nearstar_ray, g.nearstar_spin, g.nearstar_r, g.nearstar_g, g.nearstar_b);
    emit!(f, "  nop={} nob={} labeled={}\n", g.nearstar_nop, g.nearstar_nob, g.nearstar_labeled);
    for n in 0..g.nearstar_nob as usize {
        emit!(f, "  BODY {:2} type={} owner={} moonid={} ray={:.6} orb_ray={:.6}\n",
            n, g.p_type[n], g.p_owner[n], g.p_moonid[n], g.p_ray[n], g.p_orb_ray[n]);
        emit!(f, "          tilt={:.8} orb_tilt={:.8} orb_orient={:.8} orb_ecc={:.8} orb_seed={:.6} ring={:.6}\n",
            g.p_tilt[n], g.p_orb_tilt[n], g.p_orb_orient[n], g.p_orb_ecc[n], g.p_orb_seed[n], g.p_ring[n]);
    }
}

fn cmd_scan(args: &[String]) {
    let mut f = out_file(args);
    let x = argd(args, "-x", 0.0);
    let y = argd(args, "-y", 0.0);
    let z = argd(args, "-z", 0.0);
    let mut g = SystemGen::new();
    g.generate(x, y, z);
    let mut h = FNV_OFFSET;
    let rec = format!("{} {} {} {}", g.nearstar_class, g.nearstar_nop, g.nearstar_nob, g.nearstar_spin);
    h = fnv1a_update_str(h, &rec);
    for n in 0..g.nearstar_nob as usize {
        let rec = format!("{} {} {}", g.p_type[n], g.p_owner[n], g.p_moonid[n]);
        h = fnv1a_update_str(h, &rec);
    }
    emit!(f, "SCAN x={:.0} y={:.0} z={:.0} class={} nop={} nob={} fnv={:08X}\n",
        x, y, z, g.nearstar_class, g.nearstar_nop, g.nearstar_nob, h);
}

fn fnv1a_update_str(mut h: u32, s: &str) -> u32 {
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(nivgen::hash::FNV_PRIME);
    }
    h
}

// Placeholder commands; implemented in later stages.
fn cmd_planet(args: &[String]) {
    let mut f = out_file(args);
    let x = argd(args, "-x", 0.0);
    let y = argd(args, "-y", 0.0);
    let z = argd(args, "-z", 0.0);
    let p = argl(args, "-p", 0) as i32;
    let secs = argd(args, "-secs", 0.0);

    let mut g = SystemGen::new();
    g.secs = secs;
    g.generate(x, y, z);
    if p < 0 || p as usize >= g.nearstar_nob as usize {
        emit!(f, "; ERROR body index {} out of range (nob={})\n", p, g.nearstar_nob);
        return;
    }
    emit!(f, "=== PLANET TEXTURE ===\n");
    g.gen_planet_surface(p);
    report_planet(&mut f, &g, p);
}

fn cmd_sector(_args: &[String]) { eprintln!("sector: not yet implemented"); }
fn cmd_surftex(_args: &[String]) { eprintln!("surftex: not yet implemented"); }

fn hashline(f: &mut Option<std::fs::File>, label: &str, buf: &[u8]) {
    let fnv = fnv1a(buf);
    let crc = crc32(buf);
    emit!(f, "{:<16} len={:<6} fnv={:08X} crc={:08X}\n", label, buf.len(), fnv, crc);
}

fn report_planet(f: &mut Option<std::fs::File>, g: &SystemGen, p: i32) {
    let p = p as usize;
    let is_moon = g.p_owner[p] > -1;
    let surf = if is_moon { &g.s_background } else { &g.p_background };
    let seedval = g.surface_seedval_for(p as i32);
    let desc = if g.p_type[p] as i32 <= 10 {
        nivgen::system::PLANET_DESCRIPTION[g.p_type[p] as usize]
    } else {
        "?"
    };
    emit!(f, "PLANET body={} type={} ({}) owner={} seedval={:.6}\n",
        p, g.p_type[p], desc, g.p_owner[p], seedval);
    emit!(f, "  rtperiod={} rotation={} term_start={} term_end={}\n",
        g.p_rtperiod[p], g.p_rotation[p], g.p_term_start[p], g.p_term_end[p]);

    if g.p_type[p] as i32 == 10 {
        emit!(f, "  (companion star - surface() is a no-op)\n");
        return;
    }

    hashline(f, "surface_map", surf);
    hashline(f, "atmo_overlay", &g.objectschart[0..32400]);
    let base3 = (if is_moon { 128 } else { 192 }) * 3;
    hashline(f, "palette64", &g.tmppal[base3..base3 + 192]);
}
