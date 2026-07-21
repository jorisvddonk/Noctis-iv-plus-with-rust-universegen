use nivgen::rng::{BorlandRng, FastRng};
use nivgen::system::SystemGen;
use nivgen::system::EngineY;
use nivgen::hash::{fnv1a, crc32, FNV_OFFSET};
use std::fs::OpenOptions;
use std::io::Write;

/// Number of surface-map bytes the globe renderer can actually sample.
///
/// The sphere projection (globe(), NOCTIS-0.CPP:3044) walks the surface map via
/// GLOBES.MAP and never reads beyond row 121 even at extreme rotation; rows
/// 0..119 cover the full visible latitude band. We hash rows 0..127 (46080
/// bytes) to include the worst-case rotation reach with margin while excluding
/// the deterministically-irrelevant scratch tail (rows 128..179), where the
/// moon far-heap OOB read (lssmooth reading past s_background) lands. Those tail
/// rows are never displayed, so hashing them would compare pure scratch memory.
const SAMPLED_BYTES: usize = 360 * 128; // 46080

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
        "planet-all" => cmd_planet_all(&args),
        "sector" => cmd_sector(&args),
        "surftex" => cmd_surftex(&args),
        "fill-db" => cmd_fill_db(&args),
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
    let json = args.iter().any(|a| a == "--json");
    let name = args.iter().position(|a| a == "--name").and_then(|p| args.get(p + 1));
    let starmap_path = args.iter().position(|a| a == "--starmap").and_then(|p| args.get(p + 1));

    let mut g = SystemGen::new();
    g.generate(x, EngineY::from_user_y(y), z);

    // If --name and --starmap both given, verify computed class matches starmap.
    if let (Some(n), Some(sp)) = (name, starmap_path) {
        if let Ok(bin) = std::fs::read(sp) {
            if let Some(expected) = nivgen::system::starmap_class(n, &bin) {
                if g.nearstar_class != expected {
                    panic!("CLASS MISMATCH: computed={} expected={} for {} (x={} y={} z={})",
                        g.nearstar_class, expected, n, x as i64, y as i64, z as i64);
                }
            }
        }
    }
    if json {
        emit_json_system(&mut f, &g);
    } else {
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
}

fn emit_json_system(f: &mut Option<std::fs::File>, g: &SystemGen) {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"nob\": {},\n", g.nearstar_nob));
    out.push_str(&format!("  \"nop\": {},\n", g.nearstar_nop));
    out.push_str("  \"bodies\": [\n");
    for n in 0..g.nearstar_nob as usize {
        out.push_str("    {");
        out.push_str(&format!("\"index\":{},\"type\":{},\"owner\":{},\"moonid\":{}", n, g.p_type[n], g.p_owner[n], g.p_moonid[n]));
        out.push_str("}");
        if n + 1 < g.nearstar_nob as usize { out.push_str(","); }
        out.push_str("\n");
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    print!("{}", out);
    if let Some(f) = f.as_mut() { let _ = f.write_all(out.as_bytes()); }
}

fn cmd_scan(args: &[String]) {
    let mut f = out_file(args);
    let x = argd(args, "-x", 0.0);
    let y = argd(args, "-y", 0.0);
    let z = argd(args, "-z", 0.0);
    let mut g = SystemGen::new();
    g.generate(x, EngineY::from_user_y(y), z);
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

// Process ALL bodies sequentially (matching C's cmd_planet_all) for state-sharing validation.
fn cmd_planet_all(args: &[String]) {
    let mut f = out_file(args);
    let x = argd(args, "-x", 0.0);
    let y = argd(args, "-y", 0.0);
    let z = argd(args, "-z", 0.0);
    let secs = argd(args, "-secs", 0.0);

    let mut g = SystemGen::new();
    g.secs = secs;
    g.generate(x, EngineY::from_user_y(y), z);
    emit!(f, "=== PLANET TEXTURES (all {} bodies) ===\n", g.nearstar_nob);
    for n in 0..g.nearstar_nob as i32 {
        g.gen_planet_surface(n);
        report_planet_compact(&mut f, &g, n);
    }
}

fn report_planet_compact(f: &mut Option<std::fs::File>, g: &SystemGen, p: i32) {
    let p = p as usize;
    let is_moon = g.p_owner[p] > -1;
    let surf = if is_moon { &g.s_background[..SAMPLED_BYTES] } else { &g.p_background[..SAMPLED_BYTES] };
    let surf_fnv = fnv1a(surf);
    let atmo_fnv = fnv1a(&g.objectschart[0..32400]);
    let base3 = (if is_moon { 128 } else { 192 }) * 3;
    let pal_fnv = fnv1a(&g.tmppal[base3..base3 + 192]);
    emit!(f, "PLANET {} type={} is_moon={} surf={:08X} atmo={:08X} pal={:08X}\n",
        p, g.p_type[p], is_moon as i32, surf_fnv, atmo_fnv, pal_fnv);
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
    g.generate(x, EngineY::from_user_y(y), z);
    if p < 0 || p as usize >= g.nearstar_nob as usize {
        emit!(f, "; ERROR body index {} out of range (nob={})\n", p, g.nearstar_nob);
        return;
    }
    emit!(f, "=== PLANET TEXTURE ===\n");
    g.gen_planet_surface(p);
    report_planet(&mut f, &g, p);
}

fn cmd_sector(args: &[String]) {
    let mut f = out_file(args);
    let x = argd(args, "-x", 0.0);
    let y = argd(args, "-y", 0.0);
    let z = argd(args, "-z", 0.0);
    let p = argl(args, "-p", 0) as i32;
    let lon = argl(args, "-lon", 0) as i32;
    let lat = argl(args, "-lat", 60) as i32;
    let albedo = argl(args, "-albedo", 32) as i32;
    let sc = argl(args, "-sc", -1) as i32;
    let gap_hex = if let Some(pos) = args.iter().position(|a| a == "-gap") {
        args.get(pos + 1).cloned().unwrap_or_default()
    } else { String::new() };

    let mut g = SystemGen::new();
    g.generate(x, EngineY::from_user_y(y), z);
    if p < 0 || p as usize >= g.nearstar_nob as usize {
        emit!(f, "; ERROR body index {} out of range (nob={})\n", p, g.nearstar_nob);
        return;
    }
    g.gen_planet_surface(p);
    g.setup_landing(p, lon, lat, albedo, sc);
    emit!(f, "=== SURFACE SECTOR (heightmap) ===\n");
    g.build_surface_with_gap(&gap_hex);

    let pt = p as usize;
    let ptype = g.p_type[pt] as i32;
    emit!(f, "SECTOR body={} type={} lon={} lat={} sctype={} albedo={} night={}\n",
        p, ptype, lon, lat, g.sctype, g.albedo, g.nightzone);
    emit!(f, "  global_surface_seed={}\n", g.global_surface_seed);
    hashline(&mut f, "heightmap", &g.p_surfacemap[0..40000]);
    hashline(&mut f, "objectchart", &g.objectschart[0..40000]);

    if let Ok(dir) = std::env::var("NIVDUMP") {
        let _ = std::fs::write(format!("{}/HEIGHT.bin", dir), &g.p_surfacemap[0..40000]);
        let _ = std::fs::write(format!("{}/OBJECTS.bin", dir), &g.objectschart[0..40000]);
    }
}

fn cmd_surftex(_args: &[String]) { eprintln!("surftex: not yet implemented"); }

fn cmd_fill_db(args: &[String]) {
    let db_path = args.iter().position(|a| a == "--db").and_then(|p| args.get(p + 1))
        .map(|s| s.as_str()).unwrap_or("db/noctis.db");
    let conn = rusqlite::Connection::open(db_path).expect("open DB");
    conn.execute_batch("DELETE FROM planets").expect("clear planets");
    let mut stmt = conn.prepare("SELECT id, x, y_user, z FROM stars ORDER BY id")
        .expect("prepare stmt");
    let stars: Vec<(i64, f64, f64, f64)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }).expect("query stars").filter_map(|r| r.ok()).collect();

    let mut update_star = conn.prepare("UPDATE stars SET class = ?1 WHERE id = ?2")
        .expect("prepare update star");
    let mut insert = conn.prepare(
        "INSERT OR IGNORE INTO planets \
         (star_id, body_index, planet_type, seedval, is_moon, p_owner, p_moonid) \
         VALUES (?1,?2,?3,0.0,?4,?5,?6)"
    ).expect("prepare insert");

    for (i, (sid, x, y, z)) in stars.iter().enumerate() {
        let mut g = nivgen::system::SystemGen::new();
        g.generate(*x, nivgen::system::EngineY::from_user_y(*y), *z);
        update_star.execute(rusqlite::params![g.nearstar_class, sid])
            .expect("update star class");
        for bi in 0..g.nearstar_nob as usize {
            insert.execute(rusqlite::params![
                sid, bi as i32, g.p_type[bi] as i32,
                (g.p_owner[bi] > -1) as i32,
                g.p_owner[bi] as i32,
                g.p_moonid[bi] as i32,
            ]).expect("insert planet");
        }
        if (i + 1) % 100 == 0 || i == stars.len() - 1 {
            println!("[{}/{}] star_id={} cls={} nob={}",
                i + 1, stars.len(), sid, g.nearstar_class, g.nearstar_nob);
        }
    }
    println!("Done.");
}

fn hashline(f: &mut Option<std::fs::File>, label: &str, buf: &[u8]) {
    let fnv = fnv1a(buf);
    let crc = crc32(buf);
    emit!(f, "{:<16} len={:<6} fnv={:08X} crc={:08X}\n", label, buf.len(), fnv, crc);
}

fn report_planet(f: &mut Option<std::fs::File>, g: &SystemGen, p: i32) {
    let p = p as usize;
    let is_moon = g.p_owner[p] > -1;
    let surf = if is_moon { &g.s_background[..SAMPLED_BYTES] } else { &g.p_background[..SAMPLED_BYTES] };
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
