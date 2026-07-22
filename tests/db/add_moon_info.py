#!/usr/bin/env python3
"""Add is_moon and p_owner columns to planets and populate from Rust system gen."""
import json, os, sqlite3, subprocess, sys

DB_PATH = os.path.join(os.path.dirname(__file__), "noctis.db")
NIVGEN_DIR = os.path.join(os.path.dirname(__file__), "../nivgen")

def add_columns(conn):
    """Add is_moon, p_owner, p_moonid columns if they don't exist."""
    existing = {r[1] for r in conn.execute("PRAGMA table_info(planets)")}
    for col in ("is_moon", "p_owner", "p_moonid"):
        if col not in existing:
            conn.execute(f"ALTER TABLE planets ADD COLUMN {col} INTEGER")
    conn.commit()

def get_system_bodies(x, y_user, z):
    """Run nivgen system --json and return list of body dicts."""
    binary = os.path.join(NIVGEN_DIR, "target/debug/nivgen")
    r = subprocess.run(
        [binary, "system", "--json",
         "-x", str(x), "-y", str(y), "-z", str(z)],
        capture_output=True, text=True, timeout=30
    )
    out = r.stdout
    if not out.strip():
        out = r.stderr
    # Find JSON object in output
    start = out.find('{')
    end = out.rfind('}')
    if start == -1 or end == -1:
        return []
    try:
        return json.loads(out[start:end+1])["bodies"]
    except:
        return []

def populate(conn):
    """For each star, run system gen and populate is_moon/p_owner/p_moonid."""
    stars = conn.execute("SELECT id, x, y_user, z FROM stars ORDER BY id").fetchall()
    for sid, x, y_user, z in stars:
        bodies = get_system_bodies(x, y_user, z)
        if not bodies:
            print(f"  SKIP star {sid}: no system output", file=sys.stderr)
            continue
        updated = 0
        for b in bodies:
            idx = b["index"]
            is_moon = 1 if b["owner"] > -1 else 0
            res = conn.execute(
                "UPDATE planets SET is_moon=?, p_owner=?, p_moonid=? WHERE star_id=? AND body_index=?",
                (is_moon, b["owner"], b["moonid"], sid, idx)
            )
            if res.rowcount > 0:
                updated += 1
        conn.commit()
        print(f"  star {sid:2d}: {updated}/{len(bodies)} bodies matched in DB")

if __name__ == "__main__":
    conn = sqlite3.connect(DB_PATH)
    add_columns(conn)
    populate(conn)

    # Show summary
    moon_match = moon_total = plan_match = plan_total = 0
    for r in conn.execute("""
        SELECT p.is_moon, p.surf_fnv, p.rust_surf_fnv
        FROM planets p JOIN stars s ON s.id = p.star_id
        WHERE p.surf_fnv IS NOT NULL AND p.rust_surf_fnv IS NOT NULL AND p.is_moon IS NOT NULL
    """):
        is_moon, expected, rust = r
        if is_moon:
            moon_total += 1
            if expected == rust: moon_match += 1
        else:
            plan_total += 1
            if expected == rust: plan_match += 1

    print(f"\nSummary: planets {plan_match}/{plan_total}  moons {moon_match}/{moon_total}")
    conn.close()
