#!/usr/bin/env python3
"""Parse a `planet-all` canonical .OUT (BCPP 3.1 / DOSBox-X) and write the
canonical surf/atmo/pal FNV hashes into the DB for the matching star.

Usage:
  load_canonical.py <star_name_substring> <path_to_star.out>
"""
import sqlite3, os, re, sys

DB_PATH = os.path.join(os.path.dirname(__file__), "noctis.db")


def parse(path):
    """Yield (body, ptype, seedval, surf, atmo, pal) per PLANET block."""
    text = open(path).read()
    blocks = re.split(r'^PLANET body=', text, flags=re.M)[1:]
    for b in blocks:
        hdr = re.match(r'(\d+) type=(\d+).*?seedval=([-\d.]+)', b)
        if not hdr:
            continue
        body = int(hdr.group(1)); ptype = int(hdr.group(2)); seed = float(hdr.group(3))

        def f(label):
            m = re.search(label + r'\s+len=\d+\s+fnv=(\w+)', b)
            return m.group(1) if m else None
        yield body, ptype, seed, f("surface_map"), f("atmo_overlay"), f("palette64")


def main():
    if len(sys.argv) != 3:
        print(__doc__); sys.exit(2)
    star_name, path = sys.argv[1], sys.argv[2]
    conn = sqlite3.connect(DB_PATH)
    star = conn.execute("SELECT id, x, y, z FROM stars WHERE name = ?",
                        (star_name,)).fetchone()
    if not star:
        star = conn.execute("SELECT id, x, y, z FROM stars WHERE name LIKE ?",
                            (f"%{star_name}%",)).fetchone()
    if not star:
        print(f"star '{star_name}' not found"); sys.exit(1)
    sid = star[0]

    updated = inserted = 0
    for body, ptype, seed, surf, atmo, pal in parse(path):
        if surf is None:  # type 10 companion star (no surface)
            continue
        row = conn.execute(
            "SELECT id FROM planets WHERE star_id=? AND body_index=?",
            (sid, body)).fetchone()
        if row:
            conn.execute(
                "UPDATE planets SET planet_type=?, seedval=?, surf_fnv=?, atmo_fnv=?, pal_fnv=? WHERE id=?",
                (ptype, seed, surf, atmo, pal, row[0]))
            updated += 1
        else:
            conn.execute(
                """INSERT INTO planets (star_id, body_index, planet_type, seedval, surf_fnv, atmo_fnv, pal_fnv)
                   VALUES (?,?,?,?,?,?,?)""",
                (sid, body, ptype, seed, surf, atmo, pal))
            inserted += 1
    conn.commit()
    print(f"{star_name}: {updated} updated, {inserted} inserted")
    conn.close()


if __name__ == "__main__":
    main()
