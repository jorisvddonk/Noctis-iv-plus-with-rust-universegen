#!/usr/bin/env python3
"""Query the Noctis IV test fixtures database."""
import sqlite3, os, sys

DB_PATH = os.path.join(os.path.dirname(__file__), "noctis.db")

def query(conn, sql, params=()):
    cur = conn.execute(sql, params)
    cols = [d[0] for d in cur.description]
    rows = cur.fetchall()
    return cols, rows

def show_stars(conn):
    _, rows = query(conn, "SELECT id, name, class, x, y_user, z FROM stars ORDER BY name")
    print(f"{'ID':<4} {'Name':<25} {'Cls':<4} {'Coords'}")
    print("-" * 70)
    for r in rows:
        print(f"{r[0]:<4} {r[1]:<25} {r[2]:<4} -x {r[3]} -y {r[4]} -z {r[5]}")

def show_planets(conn, star_name=None):
    sql = """SELECT s.name, p.body_index, p.planet_type, p.name,
                    p.surf_fnv, p.atmo_fnv, p.pal_fnv, p.seedval
             FROM planets p JOIN stars s ON s.id = p.star_id"""
    params = ()
    if star_name:
        sql += " WHERE s.name LIKE ?"
        params = (f"%{star_name}%",)
    sql += " ORDER BY s.name, p.planet_type"
    _, rows = query(conn, sql, params)
    print(f"{'Star':<20} {'Body':<5} {'Type':<5} {'Planet':<15} {'Surf':<10} {'Atmo':<10} {'Pal':<10}")
    print("-" * 85)
    for r in rows:
        print(f"{r[0]:<20} {r[1]:<5} {r[2]:<5} {r[3] or '':<15} {r[4]:<10} {r[5]:<10} {r[6]:<10}")

if __name__ == "__main__":
    conn = sqlite3.connect(DB_PATH)
    if len(sys.argv) > 1 and sys.argv[1] == "stars":
        show_stars(conn)
    else:
        show_planets(conn, sys.argv[1] if len(sys.argv) > 1 else None)
    conn.close()
