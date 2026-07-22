#!/usr/bin/env python3
"""Run Rust nivgen against a star system and compare to DB canonical hashes."""
import subprocess, re, sqlite3, os, sys

DB_PATH = os.path.join(os.path.dirname(__file__), "noctis.db")
BINARY = os.path.join(os.path.dirname(__file__), "../nivgen/target/debug/nivgen")

def planet_hashes_all(star_x, star_y, star_z):
    """Run `nivgen planet-all` once and return {body_index: (surf, atmo, pal)}.

    Canonical fixtures are produced with `planet-all` (state is shared across
    bodies -- e.g. the darkside terminator for a moon of a non-type-10 owner
    depends on prior bodies' `plx/plz`). Per-body `planet -p N` runs do NOT
    share that state and will diverge, so comparison MUST use planet-all.
    """
    r = subprocess.run(
        ["cargo", "run", "--quiet", "--", "planet-all",
         "-x", str(star_x), "-y", str(star_y), "-z", str(star_z)],
        cwd=os.path.join(os.path.dirname(__file__), "../nivgen"),
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


def planet_hashes(star_x, star_y, star_z, body):
    """Convenience: single-body hash (NOT suitable for canonical comparison)."""
    return planet_hashes_all(star_x, star_y, star_z).get(body, (None, None, None))

def compare_star(conn, star_name):
    """Run Rust for all planets of a given star and update DB."""
    star = conn.execute(
        "SELECT id, x, y_user, z FROM stars WHERE name = ?",
        (star_name,)
    ).fetchone()
    if not star:
        star = conn.execute(
            "SELECT id, x, y_user, z FROM stars WHERE name LIKE ?",
            (f"%{star_name}%",)
        ).fetchone()
    if not star:
        print(f"Star '{star_name}' not found")
        return
    sid, x, y_user, z = star

    # Generate all Rust bodies in one planet-all run (state-shared, matches
    # the canonical fixture path). Single-body `planet -p N` runs diverge for
    # non-type-10 moons whose terminator depends on prior bodies' plx/plz.
    rust = planet_hashes_all(x, y_user, z)

    planets = conn.execute(
        "SELECT id, body_index FROM planets WHERE star_id=? AND surf_fnv IS NOT NULL",
        (sid,)
    ).fetchall()

    if not planets:
        print(f"No planets for '{star_name}'")
        return

    ok = 0
    for pid, body in planets:
        h = rust.get(body)
        if not h:
            print(f"  body {body:3d}: NO OUTPUT (type 10?)")
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
        status = "OK" if match else f"DIFF(surf={c_surf[:8]}/{surf[:8]})"
        print(f"  body {body:3d}: {status}")
    
    conn.commit()
    print(f"  {ok}/{len(planets)} match")

if __name__ == "__main__":
    conn = sqlite3.connect(DB_PATH)
    if len(sys.argv) > 1:
        compare_star(conn, sys.argv[1])
    else:
        # Show current comparison state
        for r in conn.execute("""
            SELECT s.name, p.body_index, p.planet_type,
                   p.surf_fnv, p.rust_surf_fnv
            FROM planets p JOIN stars s ON s.id = p.star_id
            WHERE p.rust_surf_fnv IS NOT NULL
            ORDER BY s.name, p.planet_type
        """):
            match = r[3] == r[4]
            print(f"{r[0]:20s} body {r[1]:3d} type {r[2]}: {'OK' if match else 'DIFF'}")
    conn.close()
