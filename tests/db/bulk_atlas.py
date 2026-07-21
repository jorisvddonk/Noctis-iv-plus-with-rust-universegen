#!/usr/bin/env python3
"""Bulk-import class-diverse star systems from the Feltyrion Explorer atlas.

For each selected star:
  1. fetch /stars/<name>.txt -> parse class + Paris coords (x,y,z)
  2. insert into `stars`
  3. recompute-canonical.sh (BCPP 3.1 / DOSBox-X) -> tests/results/canonical/<NAME>.out
  4. load_canonical.py -> canonical surf/atmo/pal FNV
  5. populate is_moon/p_owner from .out owner= headers
  6. compare.py -> Rust vs canonical (surface-only should be 100%)

Usage:
  bulk_atlas.py select [--per-class N]     # print class-diverse selection
  bulk_atlas.py run [--per-class N] [--only NAME ...]
"""
import argparse
import os
import re
import sqlite3
import subprocess
import sys
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
DB_PATH = os.path.join(HERE, "noctis.db")
CANON_DIR = os.path.join(REPO, "tests", "results", "canonical")
RECOMPUTE = os.path.join(REPO, "tests", "scripts", "recompute-canonical.sh")
BASE = "https://jorisvddonk.github.io/feltyrion-explorer"

INDEX_RE = re.compile(
    r"^(.*?)\s+ S(\d+) \s+(\d+) bodies"
)


def fetch(url):
    with urllib.request.urlopen(url, timeout=30) as r:
        return r.read().decode("utf-8", "replace")


def load_index():
    """Return list of (name, cls:int, nbodies:int)."""
    text = fetch(f"{BASE}/index.txt")
    out = []
    for line in text.splitlines():
        m = INDEX_RE.match(line)
        if not m:
            continue
        name, cls, nb = m.group(1), int(m.group(2)), int(m.group(3))
        out.append((name, cls, nb))
    return out


def select(entries, per_class, existing_names):
    """Pick up to per_class stars per class (with >=1 body, not already in DB).

    Prefers a spread of body counts within each class for edge coverage.
    """
    by_class = {}
    for name, cls, nb in entries:
        if nb < 1:
            continue
        if name.upper() in existing_names:
            continue
        by_class.setdefault(cls, []).append((name, cls, nb))
    chosen = []
    for cls in sorted(by_class):
        lst = sorted(by_class[cls], key=lambda e: e[2])  # by body count
        if len(lst) <= per_class:
            picks = lst
        else:
            # even spread across the sorted-by-bodycount list
            step = (len(lst) - 1) / (per_class - 1) if per_class > 1 else 0
            picks = [lst[round(i * step)] for i in range(per_class)]
        chosen.extend(picks)
    return chosen


COORD_RE = re.compile(r"Paris coordinates:\s*(-?\d+),\s*(-?\d+),\s*(-?\d+)")
CLASS_RE = re.compile(r"Star class:\s*S(\d+)")


def fetch_star(name):
    """Return (cls, x, y, z) from the per-star .txt."""
    url = f"{BASE}/stars/{urllib.parse.quote(name)}.txt"
    text = fetch(url)
    c = CLASS_RE.search(text)
    co = COORD_RE.search(text)
    if not c or not co:
        raise ValueError(f"could not parse coords/class for {name}")
    return int(c.group(1)), int(co.group(1)), int(co.group(2)), int(co.group(3))


def safe_filename(name):
    return name.replace(" ", "_").replace("/", "_")


def existing(conn):
    return {r[0].upper() for r in conn.execute("SELECT name FROM stars")}


def populate_moons_from_out(conn, sid, out_path):
    """Parse owner= headers from the .out and set is_moon/p_owner."""
    text = open(out_path).read()
    # PLANET body=N type=T ... owner=O
    for m in re.finditer(r"^PLANET body=(\d+).*?owner=(-?\d+)", text, re.M):
        body = int(m.group(1))
        owner = int(m.group(2))
        conn.execute(
            "UPDATE planets SET is_moon=?, p_owner=? WHERE star_id=? AND body_index=?",
            (1 if owner > -1 else 0, owner, sid, body),
        )
    conn.commit()


def ensure_moon_cols(conn):
    existing_cols = {r[1] for r in conn.execute("PRAGMA table_info(planets)")}
    for col in ("is_moon", "p_owner", "p_moonid",
                "rust_surf_fnv", "rust_atmo_fnv", "rust_pal_fnv"):
        if col not in existing_cols:
            conn.execute(f"ALTER TABLE planets ADD COLUMN {col} TEXT" if col.startswith("rust")
                         else f"ALTER TABLE planets ADD COLUMN {col} INTEGER")
    conn.commit()


def run_one(conn, name, no_compare=False):
    fname = safe_filename(name)
    cls, x, y_user, z = fetch_star(name)
    y_eng = -y_user
    print(f"[{name}] S{cls:02d} @ ({x},{y_user},{z})", flush=True)

    conn.execute(
        "INSERT OR IGNORE INTO stars (name, class, x, y_user, z) VALUES (?,?,?,?,?)",
        (name, cls, x, y_eng, z),
    )
    conn.commit()
    row = conn.execute("SELECT id FROM stars WHERE x=? AND y_user=? AND z=?", (x, y_eng, z)).fetchone()
    if not row:
        print(f"  [!] insert failed (dup coords?) — skipping", flush=True)
        return None
    sid = row[0]

    out_path = os.path.join(CANON_DIR, f"{fname}.out")
    if not os.path.exists(out_path):
        print(f"  recompute-canonical ...", flush=True)
        r = subprocess.run(
            ["bash", RECOMPUTE, "-x", str(x), "-y", str(y), "-z", str(z), "-o", out_path],
            capture_output=True, text=True,
        )
        if r.returncode != 0 or not os.path.exists(out_path):
            print(f"  [!] canonical FAILED:\n{r.stderr[-600:]}", flush=True)
            return None
    else:
        print(f"  canonical exists, reusing", flush=True)

    subprocess.run([sys.executable, os.path.join(HERE, "load_canonical.py"), name, out_path],
                   check=True)
    populate_moons_from_out(conn, sid, out_path)

    if no_compare:
        return "loaded"
    r = subprocess.run([sys.executable, os.path.join(HERE, "compare.py"), name],
                       capture_output=True, text=True)
    print(r.stdout.rstrip(), flush=True)
    tail = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    return tail


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    sp = sub.add_parser("select")
    sp.add_argument("--per-class", type=int, default=2)
    sp.add_argument("--target", type=int, default=0,
                    help="spread this many systems evenly across all classes")
    rp = sub.add_parser("run")
    rp.add_argument("--per-class", type=int, default=2)
    rp.add_argument("--target", type=int, default=0,
                    help="spread this many systems evenly across all classes")
    rp.add_argument("--only", nargs="*")
    rp.add_argument("--no-compare", action="store_true",
                    help="skip per-star compare during ingestion (do aggregate later)")
    args = ap.parse_args()

    conn = sqlite3.connect(DB_PATH)
    ensure_moon_cols(conn)
    ex = existing(conn)

    if getattr(args, "only", None):
        targets = [(n, None, None) for n in args.only]
    else:
        per_class = args.per_class
        if args.target:
            ncls = len({c for _, c, nb in load_index() if nb >= 1})
            per_class = max(1, args.target // ncls)
        print("loading index ...", file=sys.stderr)
        entries = load_index()
        targets = select(entries, per_class, ex)

    if args.cmd == "select":
        for name, cls, nb in targets:
            print(f"S{cls:02d}  {nb:4d} bodies  {name}")
        print(f"\n{len(targets)} stars selected", file=sys.stderr)
        return

    results = []
    for name, *_ in targets:
        try:
            res = run_one(conn, name, no_compare=args.no_compare)
            results.append((name, res))
        except Exception as e:
            print(f"  [!] {name}: {e}")
            results.append((name, f"ERROR: {e}"))
    print("\n=== SUMMARY ===")
    for name, res in results:
        print(f"  {name:28s} {res}")
    conn.close()


if __name__ == "__main__":
    main()
