#!/usr/bin/env python3
"""Populate planets table from nivgen system gen (one subprocess per star)."""
import sqlite3, subprocess, json, sys, os

HERE = os.path.dirname(__file__)
DB = os.path.join(HERE, "noctis.db")
NIVGEN = os.path.join(HERE, "..", "nivgen", "target", "release", "nivgen")

conn = sqlite3.connect(DB)
stars = conn.execute("SELECT id, name, x, y, z FROM stars ORDER BY id").fetchall()
total = len(stars)

for idx, (sid, name, x, y, z) in enumerate(stars):
    r = subprocess.run(
        [NIVGEN, "system", "--json", "-x", str(x), "-y", str(y), "-z", str(z)],
        capture_output=True, text=True, timeout=30,
        cwd=os.path.join(HERE, "..", "nivgen"))
    if r.returncode != 0:
        print(f"[{idx+1}/{total}] {name}: FAIL {r.stderr[:80]}")
        continue
    try:
        data = json.loads(r.stdout)
    except json.JSONDecodeError:
        print(f"[{idx+1}/{total}] {name}: JSON FAIL")
        continue

    nob = data["nob"]
    for bi in range(nob):
        body = data["bodies"][bi]
        conn.execute(
            """INSERT OR IGNORE INTO planets
               (star_id, body_index, planet_type, seedval,
                is_moon, p_owner, p_moonid)
               VALUES (?,?,?,?,?,?,?)""",
            (sid, bi, body["type"], 0.0,
             body["isMoon"], body["owner"] if body["owner"] is not None else -1,
             body["moonId"] if body["moonId"] is not None else -1))
    conn.commit()
    print(f"[{idx+1}/{total}] {name}: {nob} bodies")

conn.close()
print(f"\nDone.")
