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
import os
import re
import sqlite3
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
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


def gen_star(conn, star, force=False):
    sid = conn.execute("SELECT id, x, y, z FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    _, x, y, z = sid
    bodies = conn.execute(
        "SELECT body_index, seedval FROM planets WHERE star_id=? ORDER BY body_index",
        (sid[0],)).fetchall()
    for body, seedval in bodies:
        lon_d, lat_d = 0, 60
        lon_r, lat_r = coords_from_seedval(seedval)
        for (kind, lon, lat) in (("def", lon_d, lat_d), ("rand", lon_r, lat_r)):
            out = os.path.join(SECT_DIR, fname(star, body, kind, lon, lat))
            if os.path.exists(out) and not force:
                print(f"  body {body:3d} {kind}: exists, skip")
                continue
            print(f"  body {body:3d} {kind}: lon={lon} lat={lat} ...", flush=True)
            r = subprocess.run(
                ["bash", RECOMPUTE, "-x", str(x), "-y", str(y), "-z", str(z),
                 "-p", str(body), "-lon", str(lon), "-lat", str(lat), "-o", out],
                capture_output=True, text=True)
            if r.returncode != 0 or not os.path.exists(out):
                print(f"    [!] FAILED:\n{r.stderr[-500:]}", flush=True)


HM_RE = re.compile(r"^heightmap\s+len=\d+\s+fnv=(\w+)", re.M)
OC_RE = re.compile(r"^objectchart\s+len=\d+\s+fnv=(\w+)", re.M)


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
            sets = ", ".join(f"{k}=?" for k in up)
            conn.execute(
                f"UPDATE planets SET {sets} WHERE star_id=? AND body_index=?",
                (*up.values(), sid, body))
    conn.commit()
    print(f"{star}: loaded sector hashes for {len(bodies)} bodies")


NIVGEN = os.path.join(REPO, "tests", "nivgen", "target", "debug", "nivgen")


def rust_sector_hashes(x, y, z, body, lon, lat):
    """Run `nivgen sector` for one landing and return (hm_fnv, oc_fnv)."""
    r = subprocess.run(
        [NIVGEN, "sector", "-x", str(x), "-y", str(y), "-z", str(z),
         "-p", str(body), "-lon", str(lon), "-lat", str(lat)],
        capture_output=True, text=True)
    if r.returncode != 0:
        return None, None
    hm = HM_RE.search(r.stdout)
    oc = OC_RE.search(r.stdout)
    return (hm.group(1) if hm else None,
            oc.group(1) if oc else None)


def rust_star(conn, star):
    """Compute Rust sector hashes for every body/landing and store them."""
    sid = conn.execute("SELECT id, x, y, z FROM stars WHERE name=?",
                       (star,)).fetchone()
    if not sid:
        print(f"star '{star}' not found"); sys.exit(1)
    _, x, y, z = sid
    bodies = conn.execute(
        "SELECT body_index, seedval FROM planets WHERE star_id=? "
        "ORDER BY body_index", (sid[0],)).fetchall()
    for body, seedval in bodies:
        lon_d, lat_d = 0, 60
        lon_r, lat_r = coords_from_seedval(seedval)
        up = {}
        for (kind, lon, lat, hm_col, oc_col) in (
            ("def", lon_d, lat_d, "rust_sect_def_hm", "rust_sect_def_oc"),
            ("rand", lon_r, lat_r, "rust_sect_rand_hm", "rust_sect_rand_oc"),
        ):
            hm, oc = rust_sector_hashes(x, y, z, body, lon, lat)
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
        "rust_sect_rand_hm, rust_sect_rand_oc FROM planets "
        "WHERE star_id=? ORDER BY body_index", (sid,)).fetchall()
    ok = 0
    for (b, cdm, cdo, crm, cro, rdm, rdo, rrm, rro) in bodies:
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
            print(f"  body {b:3d}: DIFF({' '.join(parts)})")
        else:
            ok += 1
    print(f"{star}: {ok}/{len(bodies)} bodies match (sectors)")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("gen", "load", "run", "rust", "cmp"):
        p = sub.add_parser(name)
        p.add_argument("star")
        p.add_argument("--force", action="store_true")
    args = ap.parse_args()
    conn = sqlite3.connect(DB_PATH)
    if args.cmd in ("gen", "run"):
        gen_star(conn, args.star, args.force)
    if args.cmd in ("load", "run"):
        load_star(conn, args.star)
    if args.cmd == "rust":
        rust_star(conn, args.star)
    if args.cmd == "cmp":
        cmp_star(conn, args.star)
    conn.close()


if __name__ == "__main__":
    main()
