#!/usr/bin/env python3
"""Generate + store canonical SURFACE SECTOR hashes (heightmap + objectchart)
for every planet of a star, under BCPP 3.1 / DOSBox-X.

For each planet we store TWO sector sets:
  * default  : landing at lon=0, lat=60 (harness defaults)
  * pseudo    : landing coords derived deterministically from the planet's
                seedval (reproducible, no extra RNG state)

The Rust port (nivgen `sector`) must reproduce both to be byte-exact.

Usage:
  bulk_sectors.py gen   <STAR_NAME> [--force]
  bulk_sectors.py load  <STAR_NAME>
  bulk_sectors.py run   <STAR_NAME>      # gen + load
"""
import argparse
import glob
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, "source")
HAR = os.path.join(REPO, "tests", "harness")
DOSBOX = os.path.join(REPO, "tests", "dosbox")
TOOLCHAIN = os.environ.get("TOOLCHAIN", os.path.join(REPO, "niv-toolchain", "BCPP31"))
DB_PATH = os.path.join(HERE, "noctis.db")
SECT_DIR = os.path.join(REPO, "tests", "results", "sectors")
RECOMPUTE = os.path.join(REPO, "tests", "scripts", "recompute-sector.sh")


def safe_filename(name):
    return name.replace(" ", "_").replace("/", "_")


def coords_from_seedval(seedval):
    """Deterministic pseudo-random landing coords from a planet seedval.

    Clamped to the in-game landable range enforced by NOCTIS.CPP:951-955
    (lat in [1,119], lon wraps to [0,359]). The previous `% 121` formula
    could emit lat 0 / 120, which are landings the game can never make.
    """
    s = abs(int(round(seedval)))
    lon = s % 360                       # [0, 359] — game wraps longitude
    lat = 1 + ((s // 1000) % 119)       # [1, 119] — game clamps latitude
    return lon, lat


def fname(star, body, kind, lon, lat):
    base = star.replace(" ", "_").replace("/", "_")
    return f"{base}_b{body}_{kind}_{lon}_{lat}.out"


EXE_DIR = os.path.join(REPO, "tests", ".harness_build")
EXE_PATH = os.path.join(EXE_DIR, "NIVTEST.EXE")
HASH_FILE = os.path.join(EXE_DIR, "NIVTEST.EXE.srchash")

SRC_FILES = [
    os.path.join(HAR, "NIVTEST.CPP"),
    os.path.join(HAR, "NIVHASH.C"), os.path.join(HAR, "NIVHASH.H"), os.path.join(HAR, "NIVSTUBS.C"),
    os.path.join(SRC, "NOCTIS-0.CPP"), os.path.join(SRC, "NOCTIS-1.CPP"),
    os.path.join(DOSBOX, "BUILD.BAT"), os.path.join(DOSBOX, "LINK.RSP"),
]
for pat in ("*.h", "*.H"):
    SRC_FILES.extend(glob.glob(os.path.join(SRC, pat)))


def compute_source_hash():
    """SHA256 of all source files that go into the C harness EXE."""
    import hashlib
    h = hashlib.sha256()
    for f in sorted(SRC_FILES):
        if os.path.exists(f):
            with open(f, "rb") as fh:
                h.update(fh.read())
        else:
            h.update(b"MISSING:" + f.encode())
    return h.hexdigest()


def source_hash_matches():
    """True if current source hash matches the saved hash file."""
    if not os.path.exists(HASH_FILE):
        return False
    with open(HASH_FILE) as f:
        saved = f.read().strip()
    return saved == compute_source_hash()


def save_source_hash():
    """Write current source hash to hash file."""
    os.makedirs(EXE_DIR, exist_ok=True)
    with open(HASH_FILE, "w") as f:
        f.write(compute_source_hash() + "\n")


def sources_changed():
    return not source_hash_matches()


def gen_star(conn, star, force=False, build_flag=False, keep_fixtures=False):
    gen_stars_batch(conn, [star], force, build_flag, keep_fixtures)

# ---------- helpers ----------

def _star_cmds(conn, star, force, star_idx=0):
    """Collect pending (body, lon, lat, dosname, outpath, x, y_user, z) for one star."""
    sid = conn.execute("SELECT id, x, y_user, z FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        return [], None, None, None, None
    sid, x, y_usr, z = sid
    bodies = conn.execute(
        "SELECT body_index, seedval FROM planets WHERE star_id=? ORDER BY body_index",
        (sid,)).fetchall()
    cmds = []
    for body, seedval in bodies:
        lon_r, lat_r = coords_from_seedval(seedval)
        for kind, lon, lat in (("def", 0, 60), ("rand", lon_r, lat_r)):
            name = fname(star, body, kind, lon, lat)
            out = os.path.join(SECT_DIR, name)
            if os.path.exists(out) and not force:
                continue
            if force and os.path.exists(out):
                os.remove(out)  # Delete so gen produces a fresh file (no append)
            suf = "D" if kind == "def" else "R"
            cmds.append((body, lon, lat, f"s{star_idx}_{body}{suf}.out", out, x, y_usr, z, "sector"))
            stex_out = os.path.join(SECT_DIR, fname(star, body, f"{kind}_stex", lon, lat))
            stex_dosname = f"t{star_idx}_{body}{suf}.out"
            cmds.append((body, lon, lat, stex_dosname, stex_out, x, y_usr, z, "surftex"))
    return cmds, x, y_usr, z, star

def gen_stars_batch(conn, stars, force=False, build_flag=False, keep_fixtures=False, dry_run=False):
    """Generate C canonical sector fixtures for multiple stars in one DOSBox-X boot."""
    all_cmds = []
    info = {}
    for si, s in enumerate(stars):
        cmds, _, _, _, _ = _star_cmds(conn, s, force, star_idx=si)
        for c in cmds:
            all_cmds.append((s,) + c)  # prepend star name
            info[(s, c[0], c[3])] = (s, c[8])  # (star, body, dosname) -> (star, type)

    if not all_cmds:
        print(f"{', '.join(stars[:3])}{'...' if len(stars)>3 else ''}: nothing to generate")
        return

    if dry_run:
        for star_name, body, lon, lat, dosname, outpath, cx, cy, cz, cmd_type in all_cmds:
            snm = info.get((star_name, body, dosname), ("?", ""))
            print(f"  {snm[0]} body {body:3d}: {cmd_type} lon={lon} lat={lat} dosname={dosname}")
        print(f"[dry-run] {len(all_cmds)} commands for {len(stars)} stars")
        return

    # Toolchain
    if os.path.isfile(os.path.join(TOOLCHAIN, "BIN", "BCC.EXE")):
        tc_mount = TOOLCHAIN
    elif os.path.isfile(os.path.join(TOOLCHAIN, "TC", "BIN", "BCC.EXE")):
        tc_mount = os.path.join(TOOLCHAIN, "TC")
    else:
        print("[sector] toolchain not found"); sys.exit(1)

    dosbox_x = os.environ.get("DOSBOX_X") or shutil.which("dosbox-x")
    if not dosbox_x:
        print("[sector] dosbox-x not found"); sys.exit(1)

    work = tempfile.mkdtemp(prefix="sector_")
    try:
        # Wipe stale fixture .out files for the stars being generated
        if not keep_fixtures:
            nuked = 0
            for star_name in stars:
                prefix = safe_filename(star_name) + "_"
                for f in glob.glob(os.path.join(SECT_DIR, prefix + "*.out")):
                    os.remove(f)
                    nuked += 1
            if nuked:
                print(f"  cleaned {nuked} stale fixture(s)")

        need_build = build_flag or not os.path.exists(EXE_PATH) or sources_changed()

        if need_build:
            os.makedirs(EXE_DIR, exist_ok=True)
            for f in ["NOCTIS-0.CPP", "NOCTIS-1.CPP"]:
                shutil.copy2(os.path.join(SRC, f), work)
            for pat in ("*.h", "*.H"):
                for f in glob.glob(os.path.join(SRC, pat)):
                    shutil.copy2(f, work)
            for f in ["NIVTEST.CPP", "NIVHASH.C", "NIVHASH.H", "NIVSTUBS.C"]:
                shutil.copy2(os.path.join(HAR, f), work)
            for f in ["BUILD.BAT", "LINK.RSP"]:
                shutil.copy2(os.path.join(DOSBOX, f), work)
            build_conf = (
                "[cpu]\r\ncputype=486\r\ncycles=max\r\n[dos]\r\nxms=true\r\nems=true\r\n"
                "[autoexec]\r\n"
                f"mount c {tc_mount}\r\n"
                f"mount d {work}\r\n"
                f"mount e {EXE_DIR}\r\n"
                "path C:\\BIN\r\n"
                "d:\r\n"
                "call BUILD.BAT\r\n"
                "if exist BUILD.FAIL goto end\r\n"
                "copy D:\\NIVTEST.EXE E:\\\r\n"
                ":end\r\n"
                "exit\r\n"
            )
            with open(os.path.join(work, "build.conf"), "w") as f:
                f.write(build_conf)
            env = os.environ.copy()
            env["SDL_VIDEODRIVER"] = "dummy"
            env["SDL_AUDIODRIVER"] = "dummy"
            subprocess.run(
                [dosbox_x, "-conf", os.path.join(work, "build.conf"), "-exit"],
                capture_output=True, text=True, env=env)
            if not os.path.exists(EXE_PATH):
                print("  [!] BUILD FAILED"); return
            save_source_hash()

        # RUN.BAT: planet-all for each star, then all sector landings
        run_lines = ["@echo off"]
        seen_stars = []
        for star_name, body, lon, lat, dosname, outpath, cx, cy, cz, cmd_type in all_cmds:
            if star_name not in seen_stars:
                seen_stars.append(star_name)
                canon_idx = len(seen_stars) - 1
                canon_name = f"PLANET{canon_idx}.OUT"
                run_lines.append(
                    f"NIVTEST.EXE planet-all -x {cx} -y {cy} -z {cz}"
                    f" -o D:\\{canon_name}"
                )
            if cmd_type == "sector":
                run_lines.append(
                    f"NIVTEST.EXE sector -x {cx} -y {cy} -z {cz} -p {body}"
                    f" -lon {lon} -lat {lat} -o D:\\{dosname}"
                )
            else:
                run_lines.append(
                    f"NIVTEST.EXE surftex -x {cx} -y {cy} -z {cz} -p {body}"
                    f" -lon {lon} -lat {lat} -o D:\\{dosname}"
                )
        run_lines.append("exit")
        with open(os.path.join(work, "RUN.BAT"), "w") as f:
            f.write("\r\n".join(run_lines) + "\r\n")

        build_section = "call BUILD.BAT\r\n" if need_build else f"copy E:\\NIVTEST.EXE D:\\NIVTEST.EXE >NUL\r\n"
        conf = (
            "[cpu]\r\ncputype=486\r\ncycles=max\r\ncore=dynamic\r\n"
            "[dos]\r\nxms=true\r\nems=true\r\n"
            "[sdl]\r\nstartup_verbosity=quiet\r\n"
            "[autoexec]\r\n"
            f"mount c {tc_mount}\r\n"
            f"mount d {work}\r\n"
            f"mount e {EXE_DIR}\r\n"
            "path C:\\BIN;E:\\\r\n"
            "d:\r\n"
            + build_section
            + "if exist BUILD.FAIL goto end\r\n"
            "call RUN.BAT\r\n"
            ":end\r\n"
            "exit\r\n"
        )
        with open(os.path.join(work, "niv.conf"), "w") as f:
            f.write(conf)

        # Print progress
        for star_name, body, lon, lat, dosname, outpath, cx, cy, cz, cmd_type in all_cmds:
            snm = info.get((star_name, body, dosname), ("?", ""))
            print(f"  {snm[0]} body {body:3d}: {cmd_type} lon={lon} lat={lat} ...", flush=True)

        env = os.environ.copy()
        env["SDL_VIDEODRIVER"] = "dummy"
        env["SDL_AUDIODRIVER"] = "dummy"
        r = subprocess.run(
            [dosbox_x, "-conf", os.path.join(work, "niv.conf"), "-exit"],
            capture_output=True, text=True, env=env)

        fail_flag = os.path.join(work, "BUILD.FAIL")
        if os.path.exists(fail_flag):
            log = os.path.join(work, "BUILD.LOG")
            if os.path.exists(log):
                with open(log) as lf:
                    print(f"  [!] BUILD FAILED:\n{lf.read()[-1000:]}", flush=True)
            print(f"  [!] stderr: {r.stderr[-500:]}", flush=True)
            return

        # Copy outputs, grouped by star
        star_ok = {}
        for star_name, body, lon, lat, dosname, outpath, cx, cy, cz, cmd_type in all_cmds:
            snm = info.get((star_name, body, dosname), ("?", ""))
            src = os.path.join(work, dosname)
            if os.path.exists(src):
                shutil.copy2(src, outpath)
                star_ok[snm] = star_ok.get(snm, 0) + 1
            else:
                print(f"  body {body:3d}: missing output", flush=True)

        for snm, n in star_ok.items():
            n_cmds = sum(1 for c in all_cmds if info.get((c[0], c[1], c[4])) == snm)
            print(f"{snm[0]}: {n}/{n_cmds} landings generated")
        print(f"Booted DOSBox-X once for {len(stars)} stars")

        # Load canonical surface hashes from planet-all outputs
        canon_dir = os.path.join(REPO, "tests", "results", "canonical")
        load_script = os.path.join(REPO, "tests", "db", "load_canonical.py")
        for idx, star_name in enumerate(seen_stars):
            canon_name = f"PLANET{idx}.OUT"
            canon_src = os.path.join(work, canon_name)
            canon_dst = os.path.join(canon_dir, f"{safe_filename(star_name)}.out")
            if os.path.exists(canon_src):
                shutil.copy2(canon_src, canon_dst)
                r = subprocess.run(
                    [sys.executable, load_script, star_name, canon_dst],
                    capture_output=True, text=True
                )
                if r.returncode == 0:
                    print(f"  [canon] {star_name}: {r.stdout.strip()}")
                else:
                    print(f"  [canon] {star_name}: FAILED: {r.stderr[-200:]}")
            else:
                print(f"  [canon] {star_name}: no planet-all output", flush=True)

    finally:
        shutil.rmtree(work, ignore_errors=True)


HM_RE = re.compile(r"^heightmap\s+len=\d+\s+fnv=(\w+)", re.M)
OC_RE = re.compile(r"^objectchart\s+len=\d+\s+fnv=(\w+)", re.M)
SKY_RE = re.compile(r"^sky_texture\s+len=\d+\s+fnv=(\w+)", re.M)
STEX_RE = re.compile(r"^surf_texture\s+len=\d+\s+fnv=(\w+)", re.M)
GAP_RE = re.compile(r"^gap\s+len=16\s+([0-9A-F]{32})", re.M)


def load_star(conn, star):
    sid = conn.execute("SELECT id FROM stars WHERE name=?", (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    sid = sid[0]
    bodies = conn.execute(
        "SELECT body_index, seedval FROM planets WHERE star_id=? ORDER BY body_index",
        (sid,)).fetchall()
    for body, seedval in bodies:
        lon_d, lat_d = 0, 60
        lon_r, lat_r = coords_from_seedval(seedval)
        up = {}
        for (kind, lon, lat, hm_col, oc_col, sky_col, stex_col) in (
            ("def", lon_d, lat_d, "sect_def_hm", "sect_def_oc", "sect_def_sky", "sect_def_stex"),
            ("rand", lon_r, lat_r, "sect_rand_hm", "sect_rand_oc", "sect_rand_sky", "sect_rand_stex"),
        ):
            out = os.path.join(SECT_DIR, fname(star, body, kind, lon, lat))
            if not os.path.exists(out):
                continue
            t = open(out).read()
            hm = HM_RE.findall(t)
            oc = OC_RE.findall(t)
            if hm:
                if len(hm) > 1:
                    print(f"  WARN {star} body {body} {kind}: {len(hm)} heightmap blocks in file, using first")
                up[hm_col] = hm[0]
            if oc:
                if len(oc) > 1:
                    print(f"  WARN {star} body {body} {kind}: {len(oc)} objectchart blocks in file, using first")
                up[oc_col] = oc[0]
            # Capture gap from this landing's fixture (DEF and RAND may differ)
            m = GAP_RE.findall(t)
            if m:
                gap_kind = f"sect_{kind}_gap"
                up[gap_kind] = m[0]
            # Surftex fixture (separate file)
            stex_out = os.path.join(SECT_DIR, fname(star, body, f"{kind}_stex", lon, lat))
            if os.path.exists(stex_out):
                tt = open(stex_out).read()
                sk = SKY_RE.findall(tt)
                st = STEX_RE.findall(tt)
                if sk: up[sky_col] = sk[0]
                if st: up[stex_col] = st[0]
        if up:
            update_planet(conn, sid, body, up)


NIVGEN = os.path.join(REPO, "tests", "nivgen", "target", "release", "nivgen")


def rust_sector_hashes(x, y, z, body, lon, lat, star_name=None, gap=None):
    """Run `nivgen sector` for one landing and return (hm_fnv, oc_fnv, sky_fnv, stex_fnv)."""
    starmap_bin = os.path.join(REPO, "data", "STARMAP.BIN")
    cmd = [NIVGEN, "sector", "-x", str(x), "-y", str(y), "-z", str(z),
           "-p", str(body), "-lon", str(lon), "-lat", str(lat)]
    if star_name:
        cmd += ["--name", star_name, "--starmap", starmap_bin]
    if gap:
        cmd += ["-gap", gap]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        return None, None, None, None
    hm = HM_RE.search(r.stdout)
    oc = OC_RE.search(r.stdout)
    sk = SKY_RE.search(r.stdout)
    st = STEX_RE.search(r.stdout)
    return (hm.group(1) if hm else None,
            oc.group(1) if oc else None,
            sk.group(1) if sk else None,
            st.group(1) if st else None)


def rust_star(conn, star):
    """Compute Rust sector hashes for every body/landing and store them."""
    sid = conn.execute("SELECT id, x, y_user, z FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    _, x, y_usr, z = sid
    bodies = conn.execute(
        "SELECT body_index, seedval FROM planets WHERE star_id=? "
        "ORDER BY body_index", (sid[0],)).fetchall()
    for body, seedval in bodies:
        lon_d, lat_d = 0, 60
        lon_r, lat_r = coords_from_seedval(seedval)
        up = {}
        for (kind, lon, lat, hm_col, oc_col, sky_col, stex_col) in (
            ("def", lon_d, lat_d, "rust_sect_def_hm", "rust_sect_def_oc",
             "rust_sect_def_sky", "rust_sect_def_stex"),
            ("rand", lon_r, lat_r, "rust_sect_rand_hm", "rust_sect_rand_oc",
             "rust_sect_rand_sky", "rust_sect_rand_stex"),
        ):
            fpath = os.path.join(SECT_DIR, fname(star, body, kind, lon, lat))
            gap = None
            if os.path.exists(fpath):
                m = GAP_RE.search(open(fpath).read())
                if m:
                    gap = m.group(1)
            if not gap:
                gap_col = f"sect_{kind}_gap"
                row = conn.execute(
                    f"SELECT {gap_col} FROM planets WHERE star_id=? AND body_index=?",
                    (sid[0], body),
                ).fetchone()
                if row and row[0]:
                    gap = row[0]
            hm, oc, _, _ = rust_sector_hashes(x, y_usr, z, body, lon, lat, star, gap)
            if hm:
                up[hm_col] = hm
            if oc:
                up[oc_col] = oc
            stex_cmd = [NIVGEN, "surftex", "-x", str(x), "-y", str(y_usr), "-z", str(z),
                        "-p", str(body), "-lon", str(lon), "-lat", str(lat)]
            if gap:
                stex_cmd += ["-gap", gap]
            r2 = subprocess.run(stex_cmd, capture_output=True, text=True, timeout=30)
            if r2.returncode == 0:
                sk = SKY_RE.search(r2.stdout)
                st = STEX_RE.search(r2.stdout)
                if sk: up[sky_col] = sk.group(1)
                if st: up[stex_col] = st.group(1)
        if up:
            update_planet(conn, sid[0], body, up)
        print(f"  body {body}: sky={up.get(sky_col,'?')} stex={up.get(stex_col,'?')}", flush=True)
    conn.commit()
    print(f"{star}: computed Rust sector hashes for {len(bodies)} bodies")


def rust_surface_hashes(x, y_user, z):
    """Run `nivgen planet-all` once and return {body_index: (surf, atmo, pal)}."""
    r = subprocess.run(
        [NIVGEN, "planet-all", "-x", str(x), "-y", str(y_user), "-z", str(z)],
        capture_output=True, text=True, timeout=120
    )
    t = r.stdout + r.stderr
    out = {}
    for m in re.finditer(
        r"^PLANET (\d+) type=\d+ is_moon=\d+ surf=(\w+) atmo=(\w+) pal=(\w+)",
        t, re.M,
    ):
        out[int(m.group(1))] = (m.group(2), m.group(3), m.group(4))
    return out


def surf_star(conn, star):
    """Compute Rust surface hashes (surf/atmo/pal) for every body and store them."""
    sid = conn.execute("SELECT id, x, y_user, z FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    sid, x, y_user, z = sid
    bodies = conn.execute(
        "SELECT id, body_index FROM planets WHERE star_id=? AND surf_fnv IS NOT NULL "
        "ORDER BY body_index", (sid,)).fetchall()
    if not bodies:
        print(f"{star}: no canonical surface data")
        return
    rust = rust_surface_hashes(x, y_user, z)
    ok = 0
    for pid, body in bodies:
        h = rust.get(body)
        if not h:
            print(f"  body {body:3d}: NO OUTPUT")
            continue
        surf, atmo, pal = h
        conn.execute(
            "UPDATE planets SET rust_surf_fnv=?, rust_atmo_fnv=?, rust_pal_fnv=? WHERE id=?",
            (surf, atmo, pal, pid)
        )
        c_surf = conn.execute("SELECT surf_fnv FROM planets WHERE id=?", (pid,)).fetchone()[0]
        c_atmo = conn.execute("SELECT atmo_fnv FROM planets WHERE id=?", (pid,)).fetchone()[0]
        c_pal = conn.execute("SELECT pal_fnv FROM planets WHERE id=?", (pid,)).fetchone()[0]
        match = (surf == c_surf and atmo == c_atmo and pal == c_pal)
        if match: ok += 1
        status = "OK" if match else f"DIFF(surf={c_surf[:8]}/{surf[:8]} atmo={c_atmo[:8]}/{atmo[:8]} pal={c_pal[:8]}/{pal[:8]})"
        print(f"  body {body:3d}: {status}")
    conn.commit()
    print(f"{star}: {ok}/{len(bodies)} surface bodies match")


def cmp_star(conn, star):
    """Compare Rust vs canonical sector+stex hashes; report OK/DIFF per body."""
    sid = conn.execute("SELECT id FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    sid = sid[0]
    bodies = conn.execute(
        "SELECT body_index, sect_def_hm, sect_def_oc, sect_rand_hm, "
        "sect_rand_oc, rust_sect_def_hm, rust_sect_def_oc, "
        "rust_sect_rand_hm, rust_sect_rand_oc, seedval, "
        "sect_def_sky, rust_sect_def_sky, sect_def_stex, rust_sect_def_stex, "
        "sect_rand_sky, rust_sect_rand_sky, sect_rand_stex, rust_sect_rand_stex "
        "FROM planets "
        "WHERE star_id=? ORDER BY body_index", (sid,)).fetchall()
    ok = 0
    no_canon = 0
    for row in bodies:
        b, cdm, cdo, crm, cro, rdm, rdo, rrm, rro, sv = row[:10]
        cds, rds, cdt, rdt, crs, rrs, crt, rrt = row[10:]
        if all(x is None for x in (cdm, cdo, crm, cro, cds, rds, cdt, rdt, crs, rrs, crt, rrt)):
            print(f"  body {b:3d}: NO CANONICAL DATA")
            no_canon += 1
            continue
        parts = []
        for (canon, rust, lbl) in ((cdm, rdm, "def-hm"), (cdo, rdo, "def-oc"),
                                   (crm, rrm, "rand-hm"), (cro, rro, "rand-oc")):
            if canon is None: continue
            if rust is None: parts.append(f"{lbl}=MISSING")
            elif canon != rust: parts.append(f"{lbl}={canon}/{rust}")
        for (canon, rust, lbl) in ((cds, rds, "def-sky"), (cdt, rdt, "def-stex"),
                                   (crs, rrs, "rand-sky"), (crt, rrt, "rand-stex")):
            if canon is None: continue
            if rust is None: parts.append(f"{lbl}=MISSING")
            elif canon != rust: parts.append(f"{lbl}={canon}/{rust}")
        if parts:
            lon, lat = coords_from_seedval(sv)
            print(f"  body {b:3d}: DIFF lon={lon} lat={lat} {' '.join(parts)}")
        else:
            ok += 1
    label = f"{ok}/{len(bodies)} bodies match" if no_canon == 0 else f"{ok}/{len(bodies)} bodies match ({no_canon} no canonical)"
    print(f"{star}: {label}")


def report_all(conn, short=False):
    rows = conn.execute("""
        SELECT planet_type, is_moon,
               SUM(CASE WHEN sect_def_hm = rust_sect_def_hm AND sect_def_oc = rust_sect_def_oc
                             AND sect_rand_hm = rust_sect_rand_hm
                             AND sect_rand_oc = rust_sect_rand_oc THEN 1 ELSE 0 END) as ok,
               SUM(CASE WHEN (sect_def_hm != rust_sect_def_hm OR sect_def_oc != rust_sect_def_oc)
                             THEN 1 ELSE 0 END) as def,
               SUM(CASE WHEN sect_def_hm = rust_sect_def_hm AND sect_def_oc = rust_sect_def_oc
                             AND sect_rand_hm != rust_sect_rand_hm
                             AND sect_rand_oc != rust_sect_rand_oc THEN 1 ELSE 0 END) as rand_both,
               SUM(CASE WHEN sect_def_hm = rust_sect_def_hm AND sect_def_oc = rust_sect_def_oc
                             AND sect_rand_hm = rust_sect_rand_hm
                             AND sect_rand_oc != rust_sect_rand_oc THEN 1 ELSE 0 END) as rand_oc
        FROM planets WHERE sect_def_hm IS NOT NULL
        GROUP BY planet_type, is_moon ORDER BY planet_type, is_moon
    """).fetchall()

    if short:
        from report import sector_totals, surface_totals, texture_totals
        ok, d, rb, ro, tot, n_stars = sector_totals(conn)
        surf, atmo, pal, s_tot, unproc = surface_totals(conn)
        sky_def, sky_rand, stex_def, stex_rand, tex_tot = texture_totals(conn)
        print(f'SECTOR: {ok}/{tot} MATCH | def* {d} MISMATCH | r-b* {rb} MISMATCH | r-o* {ro} MISMATCH | {n_stars} stars')
        if s_tot:
            print(f'SURFACE: {surf}/{s_tot} surf MATCH | {atmo}/{s_tot} atmo MATCH | {pal}/{s_tot} pal MATCH | {unproc} unprocessed')
        else:
            print('SURFACE: no canonical data')
        if tex_tot:
            print(f'TEXTURE: sky {sky_def}/{tex_tot} def MATCH | {sky_rand}/{tex_tot} rand MATCH')
            print(f'         stex {stex_def}/{tex_tot} def MATCH | {stex_rand}/{tex_tot} rand MATCH')
        else:
            print('TEXTURE: no canonical data')
        return

    print(f'Planet-type breakdown (bodies with sector data):')
    print(f'{"Typ":>3} {"P/M":>3} {"match":>5} {"def*":>5} {"r-b*":>5} {"r-o*":>5} {"tot":>5}')
    print('-' * 36)
    tot_p, tot_m, tot_ok, tot_def, tot_rb, tot_ro = 0,0,0,0,0,0
    for t, m, ok, df, rh, ro in rows:
        print(f'{t:3d} {"M" if m else "P":>3} {ok:5d} {df:5d} {rh:5d} {ro:5d} {ok+df+rh+ro:5d}')
        if m: tot_m += ok+df+rh+ro
        else: tot_p += ok+df+rh+ro
        tot_ok += ok; tot_def += df; tot_rb += rh; tot_ro += ro
    print('-' * 36)
    print(f'{"ALL":>3} {"":>3} {tot_ok:5d} {tot_def:5d} {tot_rb:5d} {tot_ro:5d} {tot_ok+tot_def+tot_rb+tot_ro:5d}')
    print()
    print(f'  MATCH    = all 4 hashes match ({tot_ok} bodies)')
    print(f'  def*     = def-hm or def-oc differs ({tot_def} bodies)')
    print(f'  r-b*     = rand-hm + rand-oc differ ({tot_rb} bodies)')
    print(f'  r-o*     = only rand-oc differs ({tot_ro} bodies)')
    print()
    surf_tot = conn.execute('SELECT COUNT(*) FROM planets WHERE surf_fnv IS NOT NULL').fetchone()[0]
    if surf_tot > 0:
        surf_rows = conn.execute('''
            SELECT planet_type, is_moon,
                   SUM(CASE WHEN surf_fnv = rust_surf_fnv THEN 1 ELSE 0 END) as surf_ok,
                   SUM(CASE WHEN atmo_fnv = rust_atmo_fnv THEN 1 ELSE 0 END) as atmo_ok,
                   SUM(CASE WHEN pal_fnv = rust_pal_fnv THEN 1 ELSE 0 END) as pal_ok,
                   COUNT(*) as tot
            FROM planets WHERE surf_fnv IS NOT NULL
            GROUP BY planet_type, is_moon ORDER BY planet_type, is_moon
        ''').fetchall()

        print('Surface hash breakdown (bodies with canonical data):')
        print(f'{"Typ":>3} {"P/M":>3} {"surf":>5} {"atmo":>5} {"pal":>5} {"tot":>5}')
        print('-' * 33)
        st_p = st_m = 0
        surf_ok = atmo_ok = pal_ok = 0
        for t, m, s, a, p, tot in surf_rows:
            print(f'{t:3d} {"M" if m else "P":>3} {s:5d} {a:5d} {p:5d} {tot:5d}')
            if m: st_m += tot
            else: st_p += tot
            surf_ok += s; atmo_ok += a; pal_ok += p
        print('-' * 33)
        print(f'{"ALL":>3} {"":>3} {surf_ok:5d} {atmo_ok:5d} {pal_ok:5d} {st_p+st_m:5d}')
        print()
        print(f'  surf/atmo/pal columns: count of bodies where Rust hash == canonical hash')
        print(f'  Rust surface hashes computed: {surf_ok+atmo_ok+pal_ok > 0}')
        if surf_ok == st_p+st_m and atmo_ok == st_p+st_m and pal_ok == st_p+st_m:
            print(f'  ALL SURFACE HASHES MATCH ({st_p+st_m}/{st_p+st_m})')
        else:
            print(f'  SURFACE MISMATCH: surf={surf_ok}/{st_p+st_m} atmo={atmo_ok}/{st_p+st_m} pal={pal_ok}/{st_p+st_m}')
    else:
        print('Surface hash breakdown: no canonical surface data in DB')
        print('(run load_canonical.py after fill-db to populate surf/atmo/pal hashes)')
    print()

    # Summary
    n_stars = conn.execute('SELECT COUNT(DISTINCT star_id) FROM planets WHERE sect_def_hm IS NOT NULL').fetchone()[0]
    print(f'Stars with data: {n_stars}')
    print(f'  Planets: {tot_p}  Moons: {tot_m}  Total: {tot_p+tot_m}')
    sector_label = 'MATCH' if tot_def == 0 and tot_rb == 0 and tot_ro == 0 else 'MISMATCH'
    print(f'  SECTOR: {sector_label} ({tot_ok} full match, {tot_def} def*, {tot_rb} r-b*, {tot_ro} r-o*)')

    # Star class breakdown
    print()
    print('Star-class breakdown (stars with sector data):')
    sc = conn.execute('''
        SELECT s.class, COUNT(DISTINCT s.id)
        FROM stars s JOIN planets p ON p.star_id = s.id
        WHERE p.sect_def_hm IS NOT NULL
        GROUP BY s.class ORDER BY s.class
    ''').fetchall()
    for cls, cnt in sc:
        print(f'  S{cls:02d}: {cnt} stars')



def update_planet(conn, star_id, body_index, cols):
    sets = ", ".join(f"{k}=?" for k in cols)
    conn.execute(
        f"UPDATE planets SET {sets} WHERE star_id=? AND body_index=?",
        (*cols.values(), star_id, body_index))


# ---------- system gen hashing ----------

FNV_OFFSET = 0x811C_9DC5
FNV_PRIME = 0x0100_0193

def fnv1a(data: bytes) -> str:
    h = FNV_OFFSET
    for b in data:
        h ^= b
        h = (h * FNV_PRIME) & 0xFFFF_FFFF
    return f"{h:08X}"

def system_output_hash(text: str) -> str:
    """FNV-1a of the canonical system output (trimmed, normalized)."""
    lines = [l.rstrip() for l in text.strip().splitlines() if l.strip()]
    return fnv1a("\n".join(lines).encode("ascii", errors="replace"))

def ensure_syshash_table(conn):
    conn.execute("""
        CREATE TABLE IF NOT EXISTS system_hashes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            star_id INTEGER NOT NULL UNIQUE,
            canon_sys TEXT,
            canon_fn TEXT,
            rust_sys TEXT,
            rust_fn TEXT,
            FOREIGN KEY (star_id) REFERENCES stars(id)
        )
    """)
    conn.commit()


def syshash_star_rust(conn, star):
    """Run nivgen system and store Rust system gen hash."""
    sid_row = conn.execute("SELECT id, x, y_user, z FROM stars WHERE name=?", (star,)).fetchone()
    if not sid_row:
        print(f"star '{star}' not found"); return
    sid, x, y_usr, z = sid_row
    r = subprocess.run(
        [NIVGEN, "system", "-x", str(x), "-y", str(y_usr), "-z", str(z)],
        capture_output=True, text=True, timeout=30)
    if r.returncode != 0:
        print(f"  {star}: nivgen system FAILED")
        return
    sys_text = r.stdout + r.stderr
    sys_hash = system_output_hash(sys_text)
    ensure_syshash_table(conn)
    existing = conn.execute("SELECT id FROM system_hashes WHERE star_id=?", (sid,)).fetchone()
    if existing:
        conn.execute("UPDATE system_hashes SET rust_sys=?, rust_fn=? WHERE star_id=?",
                     (sys_text[:1000], sys_hash, sid))
    else:
        conn.execute(
            "INSERT INTO system_hashes (star_id, rust_sys, rust_fn) VALUES (?,?,?)",
            (sid, sys_text[:1000], sys_hash))
    conn.commit()
    print(f"  {star}: Rust sys_hash={sys_hash}")


def syshash_star_canon(conn, star, build_flag=False):
    """Run nivtest.exe system under DOSBox and store C canonical system hash."""
    sid_row = conn.execute("SELECT id, x, y_user, z FROM stars WHERE name=?", (star,)).fetchone()
    if not sid_row:
        print(f"star '{star}' not found"); return
    sid, x, y_usr, z = sid_row

    import tempfile, glob

    tc_mount = os.environ.get("TOOLCHAIN", os.path.join(REPO, "niv-toolchain", "BCPP31"))
    if os.path.isfile(os.path.join(tc_mount, "BIN", "BCC.EXE")):
        pass
    elif os.path.isfile(os.path.join(tc_mount, "TC", "BIN", "BCC.EXE")):
        tc_mount = os.path.join(tc_mount, "TC")
    else:
        print("[syshash] toolchain not found"); return

    dosbox_x = os.environ.get("DOSBOX_X") or shutil.which("dosbox-x")
    if not dosbox_x:
        print("[syshash] dosbox-x not found"); return

    work = tempfile.mkdtemp(prefix="syshash_")
    try:
        run_lines = [
            "@echo off",
            f"NIVTEST.EXE system -x {x} -y {y_usr} -z {z} -o D:\\sys.out",
            "exit"
        ]
        with open(os.path.join(work, "RUN.BAT"), "w") as f:
            f.write("\r\n".join(run_lines) + "\r\n")

        need_build = build_flag or not os.path.exists(EXE_PATH) or sources_changed()
        build_section = "call BUILD.BAT\r\n" if need_build else f"copy E:\\NIVTEST.EXE D:\\NIVTEST.EXE >NUL\r\n"

        conf = (
            "[cpu]\r\ncputype=486\r\ncycles=max\r\ncore=dynamic\r\n"
            "[dos]\r\nxms=true\r\nems=true\r\n"
            "[sdl]\r\nstartup_verbosity=quiet\r\n"
            "[autoexec]\r\n"
            f"mount c {tc_mount}\r\n"
            f"mount d {work}\r\n"
            f"mount e {EXE_DIR}\r\n"
            "path C:\\BIN;E:\\\r\n"
            "d:\r\n"
            + build_section
            + "if exist BUILD.FAIL goto end\r\n"
            "call RUN.BAT\r\n"
            ":end\r\n"
            "exit\r\n"
        )
        with open(os.path.join(work, "sys.conf"), "w") as f:
            f.write(conf)

        if need_build:
            os.makedirs(EXE_DIR, exist_ok=True)
            for f in ["NOCTIS-0.CPP", "NOCTIS-1.CPP"]:
                shutil.copy2(os.path.join(SRC, f), work)
            for pat in ("*.h", "*.H"):
                for f in glob.glob(os.path.join(SRC, pat)):
                    shutil.copy2(f, work)
            for f in ["NIVTEST.CPP", "NIVHASH.C", "NIVHASH.H", "NIVSTUBS.C"]:
                shutil.copy2(os.path.join(HAR, f), work)
            for f in ["BUILD.BAT", "LINK.RSP"]:
                shutil.copy2(os.path.join(DOSBOX, f), work)
            # remove -o from recompilation (it conflicts with -o in RUN.BAT)
            # Actually the build process doesn't use -o

        env = os.environ.copy()
        env["SDL_VIDEODRIVER"] = "dummy"
        env["SDL_AUDIODRIVER"] = "dummy"
        subprocess.run(
            [dosbox_x, "-conf", os.path.join(work, "sys.conf"), "-exit"],
            capture_output=True, text=True, env=env)

        fail_flag = os.path.join(work, "BUILD.FAIL")
        if os.path.exists(fail_flag):
            log = os.path.join(work, "BUILD.LOG")
            if os.path.exists(log):
                with open(log) as lf:
                    print(f"  [!] BUILD FAILED:\n{lf.read()[-500:]}")
            return

        out_path = os.path.join(work, "sys.out")
        if not os.path.exists(out_path) or os.path.getsize(out_path) < 20:
            print(f"  {star}: no canonical system output (empty/failed)")
            return

        text = open(out_path).read()
        canon_hash = system_output_hash(text)
        ensure_syshash_table(conn)
        existing = conn.execute("SELECT id FROM system_hashes WHERE star_id=?", (sid,)).fetchone()
        if existing:
            conn.execute("UPDATE system_hashes SET canon_sys=?, canon_fn=? WHERE star_id=?",
                         (text[:1000], canon_hash, sid))
        else:
            conn.execute(
                "INSERT INTO system_hashes (star_id, canon_sys, canon_fn) VALUES (?,?,?)",
                (sid, text[:1000], canon_hash))
        conn.commit()
        print(f"  {star}: C canon sys_hash={canon_hash}")
    finally:
        shutil.rmtree(work, ignore_errors=True)


def syshash_cmp(conn, star):
    """Compare Rust vs C canonical system hashes."""
    sid = conn.execute("SELECT id FROM stars WHERE name=?", (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); return
    sid = sid[0]
    row = conn.execute(
        "SELECT canon_fn, rust_fn, canon_sys, rust_sys FROM system_hashes WHERE star_id=?",
        (sid,)).fetchone()
    if not row:
        print(f"  {star}: no system hash data"); return
    canon_fn, rust_fn, canon_sys, rust_sys = row
    if canon_fn and rust_fn:
        if canon_fn == rust_fn:
            print(f"  {star}: MATCH ({canon_fn})")
        else:
            print(f"  {star}: DIFF canon={canon_fn} rust={rust_fn}")
    elif canon_fn:
        print(f"  {star}: C canonical only ({canon_fn})")
    elif rust_fn:
        print(f"  {star}: Rust only ({rust_fn})")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("gen", "load", "run", "rust", "cmp", "one_iter", "db-report",
                 "syshash", "syscmp", "syshash-canon"):
        p = sub.add_parser(name)
        p.add_argument("star", nargs="?")
        p.add_argument("--force", action="store_true")
        p.add_argument("--build", action="store_true", help="force recompile C harness")
        p.add_argument("--keep-fixtures", action="store_true", help="don't wipe stale .out files")
        p.add_argument("--batch", type=int, default=1,
            help="batch N stars in one DOSBox-X boot (one_iter only)")
        p.add_argument("--dry-run", action="store_true",
            help="print what would be generated without running DOSBox-X")
        p.add_argument("--short", action="store_true",
            help="condensed report for notifications")
        p.add_argument("--skip-sectors", action="store_true",
            help="skip sector generation/comparison")
        p.add_argument("--skip-surf", action="store_true",
            help="skip surface hash generation/comparison")
        p.add_argument("--skip-pal", action="store_true",
            help="skip palette hash generation/comparison")
    args = ap.parse_args()
    conn = sqlite3.connect(DB_PATH)

    # If no star name given, pick the next star without C canonical data.
    star = args.star
    if not star and args.cmd != "db-report":
        if args.cmd == "rust" and args.force:
            row = conn.execute(
                "SELECT name FROM stars ORDER BY id LIMIT 1"
            ).fetchone()
        else:
            row = conn.execute(
                "SELECT name FROM stars WHERE id IN "
                "(SELECT star_id FROM planets GROUP BY star_id "
                "HAVING COUNT(sect_def_hm) = 0) "
                "ORDER BY id LIMIT 1"
            ).fetchone()
        if not row:
            print("no star without canonical data")
            return
        star = row[0]
        if args.cmd != "one_iter":
            print(f"[{args.cmd}] auto-selected: {star}")

    if args.cmd in ("gen", "run"):
        gen_star(conn, star, args.force, args.build, args.keep_fixtures)
    if args.cmd in ("load", "run"):
        load_star(conn, star)
        conn.commit()
    if args.cmd == "rust":
        if args.force:
            rows = conn.execute("SELECT name FROM stars ORDER BY id").fetchall()
            n = len(rows)
            for i, (sname,) in enumerate(rows):
                print(f"[{i+1}/{n}] {sname}")
                rust_star(conn, sname)
                surf_star(conn, sname)
                conn.commit()
        else:
            rust_star(conn, star)
    if args.cmd == "cmp":
        cmp_star(conn, star)
    if args.cmd == "db-report":
        report_all(conn, short=args.short)
    if args.cmd == "syshash":
        ensure_syshash_table(conn)
        if args.star:
            syshash_star_rust(conn, args.star)
        else:
            rows = conn.execute("SELECT name FROM stars ORDER BY id").fetchall()
            for (name,) in rows:
                syshash_star_rust(conn, name)
    if args.cmd == "syshash-canon":
        ensure_syshash_table(conn)
        if args.star:
            syshash_star_canon(conn, args.star, getattr(args, 'build', False))
        else:
            rows = conn.execute("SELECT name FROM stars ORDER BY id").fetchall()
            for (name,) in rows:
                syshash_star_canon(conn, name)
    if args.cmd == "syscmp":
        if args.star:
            syshash_cmp(conn, args.star)
        else:
            rows = conn.execute(
                "SELECT s.name FROM stars s JOIN system_hashes h ON h.star_id = s.id "
                "WHERE h.canon_fn IS NOT NULL AND h.rust_fn IS NOT NULL ORDER BY s.id"
            ).fetchall()
            ok = diff = 0
            for (name,) in rows:
                sid = conn.execute("SELECT id FROM stars WHERE name=?", (name,)).fetchone()[0]
                row = conn.execute(
                    "SELECT canon_fn, rust_fn FROM system_hashes WHERE star_id=?", (sid,)).fetchone()
                if row[0] == row[1]:
                    ok += 1
                else:
                    diff += 1
                    print(f"  {name}: DIFF canon={row[0]} rust={row[1]}")
            print(f"syshash: {ok} match, {diff} differ")
    if args.cmd == "one_iter":
        batch_size = getattr(args, 'batch', 1)
        skip_sectors = getattr(args, 'skip_sectors', False)
        skip_surf = getattr(args, 'skip_surf', False)
        skip_pal = getattr(args, 'skip_pal', False)
        if args.star:
            batch = [args.star]
        else:
            # First: try to find a star from starmap2.bin whose name is NOT in the DB
            import struct
            starmap_path = os.path.join(REPO, "data", "starmap2.bin")
            existing_names = {r[0] for r in conn.execute("SELECT name FROM stars")}
            candidate = None
            with open(starmap_path, "rb") as fh:
                data = fh.read()
            ENTRY = 44
            for i in range(0, len(data) - ENTRY, ENTRY):
                ent = data[i:i+ENTRY]
                idx = struct.unpack("<i", ent[12:16])[0]
                bt = struct.unpack("<i", ent[16:20])[0]
                name = ent[20:40].rstrip(b" ").decode("ascii", errors="replace")
                if idx == 0 and bt == 0 and name and name not in existing_names:
                    candidate = name
                    break
            import_ok = False
            if candidate:
                print(f"importing new star {candidate} from starmap2.bin ...")
                imp = subprocess.run(
                    [sys.executable, os.path.join(HERE, "bulk_atlas.py"),
                     "run", "--only", candidate, "--no-compare"],
                    capture_output=True, text=True, timeout=600)
                if imp.returncode != 0:
                    print(imp.stderr[-300:])
                    print("starmap import failed, falling back to pending stars")
                else:
                    batch = [candidate]
                    import_ok = True
            if not import_ok:
                # Fall back to existing pending stars in DB
                if skip_sectors:
                    rows = conn.execute(
                        "SELECT name FROM stars WHERE id IN "
                        "(SELECT star_id FROM planets WHERE surf_fnv IS NOT NULL "
                        "GROUP BY star_id HAVING COUNT(rust_surf_fnv) = 0) "
                        "ORDER BY id LIMIT ?", (batch_size,)).fetchall()
                else:
                    rows = conn.execute(
                        "SELECT name FROM stars WHERE id IN "
                        "(SELECT star_id FROM planets GROUP BY star_id "
                        "HAVING COUNT(sect_def_hm) = 0) "
                        "ORDER BY id LIMIT ?", (batch_size,)).fetchall()
                if not rows:
                    print("no star needs processing")
                    return
                batch = [r[0] for r in rows]
        if not skip_sectors:
            gen_stars_batch(conn, batch, args.force, args.build, args.keep_fixtures, args.dry_run)
        if not args.dry_run:
            for s in batch:
                if not skip_sectors:
                    load_star(conn, s)
                    conn.commit()
                    rust_star(conn, s)
                if not skip_surf:
                    surf_star(conn, s)
                if not skip_pal:
                    pass  # pal is part of surf_star now
                if not skip_sectors:
                    cmp_star(conn, s)
    conn.close()


if __name__ == "__main__":
    main()
