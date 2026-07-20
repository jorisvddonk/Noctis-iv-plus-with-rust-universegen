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
    """Deterministic pseudo-random landing coords from a planet seedval."""
    s = abs(int(round(seedval)))
    lon = s % 360
    lat = (s // 1000) % 121  # 0..120, within harness lat range
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


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("gen", "load", "run"):
        p = sub.add_parser(name)
        p.add_argument("star")
        p.add_argument("--force", action="store_true")
    args = ap.parse_args()
    conn = sqlite3.connect(DB_PATH)
    if args.cmd in ("gen", "run"):
        gen_star(conn, args.star, args.force)
    if args.cmd in ("load", "run"):
        load_star(conn, args.star)
    conn.close()


if __name__ == "__main__":
    main()
