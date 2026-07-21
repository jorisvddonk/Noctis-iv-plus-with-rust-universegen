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


def gen_star(conn, star, force=False, build_flag=False):
    sid = conn.execute("SELECT id, x, y_user, z FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    sid, x, y_eng, z = sid
    y_usr = -y_eng  # DB stores engine-facing; CLI expects user-facing
    bodies = conn.execute(
        "SELECT body_index, seedval FROM planets WHERE star_id=? ORDER BY body_index",
        (sid,)).fetchall()

    # Collect pending combos
    cmds = []  # (body, lon, lat, outpath, outname, dosname)
    for body, seedval in bodies:
        lon_r, lat_r = coords_from_seedval(seedval)
        for (kind, lon, lat) in (("def", 0, 60), ("rand", lon_r, lat_r)):
            name = fname(star, body, kind, lon, lat)
            out = os.path.join(SECT_DIR, name)
            if os.path.exists(out) and not force:
                print(f"  body {body:3d} {kind}: exists, skip")
                continue
            print(f"  body {body:3d} {kind}: lon={lon} lat={lat} ...", flush=True)
            # DOS 8.3-safe name: s{BODY}{D/R}.out
            suf = "D" if kind == "def" else "R"
            dosname = f"s{body}{suf}.out"
            cmds.append((body, lon, lat, out, name, dosname))

    if not cmds:
        print(f"{star}: nothing to generate")
        return

    # Determine toolchain mount point
    if os.path.isfile(os.path.join(TOOLCHAIN, "BIN", "BCC.EXE")):
        tc_mount = TOOLCHAIN
    elif os.path.isfile(os.path.join(TOOLCHAIN, "TC", "BIN", "BCC.EXE")):
        tc_mount = os.path.join(TOOLCHAIN, "TC")
    else:
        print(f"[sector] toolchain not found at {TOOLCHAIN}"); sys.exit(1)

    dosbox_x = os.environ.get("DOSBOX_X") or shutil.which("dosbox-x")
    if not dosbox_x:
        print("[sector] dosbox-x not found"); sys.exit(1)

    work = tempfile.mkdtemp(prefix="sector_")
    try:
        exe_dir = os.path.join(REPO, "tests", ".harness_build")
        exe_path = os.path.join(exe_dir, "NIVTEST.EXE")
        need_build = build_flag or not os.path.exists(exe_path)

        if need_build:
            os.makedirs(exe_dir, exist_ok=True)
            # Copy sources into work for compilation
            for f in ["NOCTIS-0.CPP", "NOCTIS-1.CPP"]:
                shutil.copy2(os.path.join(SRC, f), work)
            for pat in ("*.h", "*.H"):
                for f in glob.glob(os.path.join(SRC, pat)):
                    shutil.copy2(f, work)
            for f in ["NIVTEST.CPP", "NIVHASH.C", "NIVHASH.H", "NIVSTUBS.C"]:
                shutil.copy2(os.path.join(HAR, f), work)
            for f in ["BUILD.BAT", "LINK.RSP"]:
                shutil.copy2(os.path.join(DOSBOX, f), work)

        # Build the EXE if needed (one DOSBox-X boot).
        if need_build:
            build_conf = (
                "[cpu]\r\ncputype=486\r\ncycles=max\r\n[dos]\r\nxms=true\r\nems=true\r\n"
                "[autoexec]\r\n"
                f"mount c {tc_mount}\r\n"
                f"mount d {work}\r\n"
                f"mount e {exe_dir}\r\n"
                "path C:\\BIN\r\n"
                "d:\r\n"
                "call BUILD.BAT\r\n"
                "if exist BUILD.FAIL goto end\r\n"
                f"copy D:\\NIVTEST.EXE E:\\\r\n"
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
            if not os.path.exists(exe_path):
                print("  [!] BUILD FAILED"); return

        # Write RUN.BAT — one sector command per output file
        run_lines = ["@echo off"]
        for body, lon, lat, outpath, name, dosname in cmds:
            run_lines.append(
                f"NIVTEST.EXE sector -x {x} -y {y_usr} -z {z} -p {body}"
                f" -lon {lon} -lat {lat} -o D:\\{dosname}"
            )
        run_lines.append("exit")
        with open(os.path.join(work, "RUN.BAT"), "w") as f:
            f.write("\r\n".join(run_lines) + "\r\n")

        # Write niv.conf
        if need_build:
            build_section = "call BUILD.BAT\r\n"
        else:
            build_section = f"copy E:\\NIVTEST.EXE D:\\NIVTEST.EXE >NUL\r\n"
        conf = (
            "[cpu]\r\n"
            "cputype=486\r\n"
            "cycles=max\r\n"
            "core=dynamic\r\n"
            "[dos]\r\n"
            "xms=true\r\n"
            "ems=true\r\n"
            "[sdl]\r\n"
            "startup_verbosity=quiet\r\n"
            "[autoexec]\r\n"
            f"mount c {tc_mount}\r\n"
            f"mount d {work}\r\n"
            f"mount e {exe_dir}\r\n"
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

        # Boot DOSBox-X once: compiles + runs all sector commands
        env = os.environ.copy()
        env["SDL_VIDEODRIVER"] = "dummy"
        env["SDL_AUDIODRIVER"] = "dummy"
        r = subprocess.run(
            [dosbox_x, "-conf", os.path.join(work, "niv.conf"), "-exit"],
            capture_output=True, text=True, env=env)

        # Check build status
        fail_flag = os.path.join(work, "BUILD.FAIL")
        if os.path.exists(fail_flag):
            log = os.path.join(work, "BUILD.LOG")
            if os.path.exists(log):
                with open(log) as lf:
                    print(f"  [!] BUILD FAILED:\n{lf.read()[-1000:]}", flush=True)
            print(f"  [!] stderr: {r.stderr[-500:]}", flush=True)
            return

        # Copy output files
        ok = 0
        for body, lon, lat, outpath, name, dosname in cmds:
            src = os.path.join(work, dosname)
            if os.path.exists(src):
                shutil.copy2(src, outpath)
                ok += 1
            else:
                print(f"  body {body:3d}: missing output {name}", flush=True)
        print(f"{star}: {ok}/{len(cmds)} landings generated (DOSBox-X booted once)", flush=True)
    finally:
        shutil.rmtree(work, ignore_errors=True)


HM_RE = re.compile(r"^heightmap\s+len=\d+\s+fnv=(\w+)", re.M)
OC_RE = re.compile(r"^objectchart\s+len=\d+\s+fnv=(\w+)", re.M)
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
        for (kind, lon, lat, hm_col, oc_col) in (
            ("def", lon_d, lat_d, "sect_def_hm", "sect_def_oc"),
            ("rand", lon_r, lat_r, "sect_rand_hm", "sect_rand_oc"),
        ):
            out = os.path.join(SECT_DIR, fname(star, body, kind, lon, lat))
            if not os.path.exists(out):
                continue
            t = open(out).read()
            hm = HM_RE.search(t)
            oc = OC_RE.search(t)
            if hm: up[hm_col] = hm.group(1)
            if oc: up[oc_col] = oc.group(1)
        if up:
            # also capture gap from def landing (same for both landings)
            def_out = os.path.join(SECT_DIR, fname(star, body, "def", lon_d, lat_d))
            if os.path.exists(def_out):
                m = GAP_RE.search(open(def_out).read())
                if m: up["sect_gap"] = m.group(1)
            sets = ", ".join(f"{k}=?" for k in up)
            conn.execute(
                f"UPDATE planets SET {sets} WHERE star_id=? AND body_index=?",
                (*up.values(), sid, body))
    conn.commit()
    print(f"{star}: loaded sector hashes for {len(bodies)} bodies")


NIVGEN = os.path.join(REPO, "tests", "nivgen", "target", "release", "nivgen")


def rust_sector_hashes(x, y, z, body, lon, lat, star_name=None, gap=None):
    """Run `nivgen sector` for one landing and return (hm_fnv, oc_fnv)."""
    starmap_bin = os.path.join(REPO, "data", "STARMAP.BIN")
    cmd = [NIVGEN, "sector", "-x", str(x), "-y", str(y), "-z", str(z),
           "-p", str(body), "-lon", str(lon), "-lat", str(lat)]
    if star_name:
        cmd += ["--name", star_name, "--starmap", starmap_bin]
    if gap:
        cmd += ["-gap", gap]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        return None, None
    hm = HM_RE.search(r.stdout)
    oc = OC_RE.search(r.stdout)
    return (hm.group(1) if hm else None,
            oc.group(1) if oc else None)


def rust_star(conn, star):
    """Compute Rust sector hashes for every body/landing and store them."""
    sid = conn.execute("SELECT id, x, y_user, z FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    _, x, y_eng, z = sid
    y_usr = -y_eng  # DB stores engine-facing; CLI expects user-facing
    bodies = conn.execute(
        "SELECT body_index, seedval, sect_gap FROM planets WHERE star_id=? "
        "ORDER BY body_index", (sid[0],)).fetchall()
    for body, seedval, gap in bodies:
        lon_d, lat_d = 0, 60
        lon_r, lat_r = coords_from_seedval(seedval)
        up = {}
        for (kind, lon, lat, hm_col, oc_col) in (
            ("def", lon_d, lat_d, "rust_sect_def_hm", "rust_sect_def_oc"),
            ("rand", lon_r, lat_r, "rust_sect_rand_hm", "rust_sect_rand_oc"),
        ):
            hm, oc = rust_sector_hashes(x, y_usr, z, body, lon, lat, star, gap)
            if hm:
                up[hm_col] = hm
            if oc:
                up[oc_col] = oc
        if up:
            sets = ", ".join(f"{k}=?" for k in up)
            conn.execute(
                f"UPDATE planets SET {sets} WHERE star_id=? AND body_index=?",
                (*up.values(), sid[0], body))
    conn.commit()
    print(f"{star}: computed Rust sector hashes for {len(bodies)} bodies")


def cmp_star(conn, star):
    """Compare Rust vs canonical sector hashes; report OK/DIFF per body."""
    sid = conn.execute("SELECT id FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    sid = sid[0]
    bodies = conn.execute(
        "SELECT body_index, sect_def_hm, sect_def_oc, sect_rand_hm, "
        "sect_rand_oc, rust_sect_def_hm, rust_sect_def_oc, "
        "rust_sect_rand_hm, rust_sect_rand_oc, seedval "
        "FROM planets "
        "WHERE star_id=? ORDER BY body_index", (sid,)).fetchall()
    ok = 0
    no_canon = 0
    for (b, cdm, cdo, crm, cro, rdm, rdo, rrm, rro, sv) in bodies:
        # If no canonical data at all, report that instead of "match"
        if cdm is None and cdo is None and crm is None and cro is None:
            print(f"  body {b:3d}: NO CANONICAL DATA")
            no_canon += 1
            continue
        parts = []
        for (canon, rust, lbl) in ((cdm, rdm, "def-hm"), (cdo, rdo, "def-oc"),
                                   (crm, rrm, "rand-hm"), (cro, rro, "rand-oc")):
            if canon is None:
                continue
            if rust is None:
                parts.append(f"{lbl}=MISSING")
            elif canon != rust:
                parts.append(f"{lbl}={canon}/{rust}")
        if parts:
            lon, lat = coords_from_seedval(sv)
            print(f"  body {b:3d}: DIFF lon={lon} lat={lat} {' '.join(parts)}")
        else:
            ok += 1
    label = f"{ok}/{len(bodies)} bodies match" if no_canon == 0 else f"{ok}/{len(bodies)} bodies match ({no_canon} no canonical)"
    print(f"{star}: {label}")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("gen", "load", "run", "rust", "cmp", "one_iter"):
        p = sub.add_parser(name)
        p.add_argument("star", nargs="?")
        p.add_argument("--force", action="store_true")
        p.add_argument("--build", action="store_true", help="force recompile C harness")
        p.add_argument("--batch", type=int, default=1,
            help="batch N stars in one DOSBox-X boot (one_iter only)")
    args = ap.parse_args()
    conn = sqlite3.connect(DB_PATH)

    # If no star name given, pick the next star without C canonical data.
    star = args.star
    if not star:
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
        gen_star(conn, star, args.force, args.build)
    if args.cmd in ("load", "run"):
        load_star(conn, star)
    if args.cmd == "rust":
        rust_star(conn, star)
    if args.cmd == "cmp":
        cmp_star(conn, star)
    if args.cmd == "one_iter":
        # Pick batch_size stars without canonical data
        batch_size = getattr(args, 'batch', 1)
        rows = conn.execute(
            "SELECT name FROM stars WHERE id IN "
            "(SELECT star_id FROM planets GROUP BY star_id "
            "HAVING COUNT(sect_def_hm) = 0) "
            "ORDER BY id LIMIT ?", (batch_size,)).fetchall()
        if not rows:
            print("no star without canonical data")
            return
        batch = [r[0] for r in rows]
        for s in batch:
            print(f"[one_iter] star: {s}")
            gen_star(conn, s, args.force, args.build)
            load_star(conn, s)
            rust_star(conn, s)
            cmp_star(conn, s)
    conn.close()


if __name__ == "__main__":
    main()
