#!/usr/bin/env python3
"""Seed the Noctis IV test fixtures database."""
import sqlite3, os, sys

DB_PATH = os.path.join(os.path.dirname(__file__), "noctis.db")

def init_db(conn):
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS stars (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            class INTEGER NOT NULL,
            x INTEGER NOT NULL,
            y INTEGER NOT NULL,
            z INTEGER NOT NULL,
            UNIQUE(x, y, z)
        );
        CREATE TABLE IF NOT EXISTS planets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            star_id INTEGER NOT NULL REFERENCES stars(id),
            name TEXT,
            body_index INTEGER NOT NULL,
            planet_type INTEGER NOT NULL,
            seedval REAL NOT NULL,
            surf_fnv TEXT,
            atmo_fnv TEXT,
            pal_fnv TEXT,
            UNIQUE(star_id, body_index)
        );
        CREATE TABLE IF NOT EXISTS test_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            star_id INTEGER NOT NULL REFERENCES stars(id),
            toolchain TEXT NOT NULL,
            timestamp TEXT DEFAULT (datetime('now')),
            notes TEXT
        );
    """)
    conn.commit()

def insert_star(conn, name, cls, x, y, z):
    conn.execute(
        "INSERT OR IGNORE INTO stars (name, class, x, y, z) VALUES (?,?,?,?,?)",
        (name, cls, x, y, z)
    )
    conn.commit()
    return conn.execute("SELECT id FROM stars WHERE x=? AND y=? AND z=?", (x,y,z)).fetchone()[0]

def insert_planet(conn, star_id, name, body_idx, ptype, seedval, surf, atmo, pal):
    conn.execute(
        """INSERT OR REPLACE INTO planets
           (star_id, name, body_index, planet_type, seedval, surf_fnv, atmo_fnv, pal_fnv)
           VALUES (?,?,?,?,?,?,?,?)""",
        (star_id, name, body_idx, ptype, seedval, surf, atmo, pal)
    )
    conn.commit()

def seed_fenia(conn):
    """Seed FENIA (class 9, main test system)."""
    x, y, z = 3612345, -4487654, -858973
    sid = insert_star(conn, "FENIA", 9, x, y, z)
    print(f"FENIA: star_id={sid}")

    # Planet data from NT6_P.OUT (BCPP 3.1 reference)
    planets = [
        # (body, type, seedval, surf_fnv, atmo_fnv, pal_fnv)
        (23, 0, 607248.307690, "40C612E0", "8BA1CB05", "1757389C"),
        (3,  1, 557668930.431149, "5352D512", "8BA1CB05", "013A8C79"),
        (13, 2, -6556585412.312880, "CDD965D9", "4BC84216", "8B58313F"),
        (2,  4, 250999248.897013, "12E61688", "8BA1CB05", "AA96D63A"),
        (1,  5, 16876215.603686, "FEE10CE2", "8BA1CB05", "69A81410"),
        (6,  6, 135446154259.788254, "3BC2E4DC", "93DD708B", "C99A1180"),
        (9,  7, -12755869666.515738, "5CD1BDAE", "8BA1CB05", "37D82989"),
        (0,  8, 71310043.923042, "E456C9AC", "8BA1CB05", "633E4346"),
        (7,  9, 125293807827.114243, "F7FAEA82", "6B15A4F5", "ED7CBF78"),
    ]
    for body, ptype, seedval, surf, atmo, pal in planets:
        insert_planet(conn, sid, None, body, ptype, seedval, surf, atmo, pal)
        print(f"  body {body} type {ptype}: surf={surf} pal={pal}")

def seed_balastrackonastreya(conn):
    """Seed Balastrackonastreya (class 0, home of Felisians)."""
    x, y, z = -18928, -29680, -67336
    sid = insert_star(conn, "BALASTRACKONASTREYA", 0, x, y, z)
    print(f"BALASTRACKONASTREYA: star_id={sid}")

    # From BCPP 3.1 reference
    planets = [
        (3, 3, 148122381.349600, "6CC505C6", "DF7C8DB9", "08C0F967"),
    ]
    for body, ptype, seedval, surf, atmo, pal in planets:
        name = "FELYSIA" if body == 3 else None
        insert_planet(conn, sid, name, body, ptype, seedval, surf, atmo, pal)
        print(f"  body {body} type {ptype} ({name or ''}): surf={surf} pal={pal}")

if __name__ == "__main__":
    conn = sqlite3.connect(DB_PATH)
    init_db(conn)
    seed_fenia(conn)
    seed_balastrackonastreya(conn)
    conn.close()
    print(f"\nDatabase: {DB_PATH}")
