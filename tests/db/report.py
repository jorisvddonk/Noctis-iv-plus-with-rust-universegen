"""Shared report queries for the verification DB.

Used by bulk_sectors.py (CLI pipeline) and server/app.py (web frontend).
"""

def sector_totals(conn):
    """Return (match, def*, r-b*, r-o*, total_bodies, n_stars)."""
    rows = conn.execute("""
        SELECT
          SUM(CASE WHEN sect_def_hm = rust_sect_def_hm AND sect_def_oc = rust_sect_def_oc
                        AND sect_rand_hm = rust_sect_rand_hm
                        AND sect_rand_oc = rust_sect_rand_oc THEN 1 ELSE 0 END) as ok,
          SUM(CASE WHEN (sect_def_hm != rust_sect_def_hm OR sect_def_oc != rust_sect_def_oc)
                        THEN 1 ELSE 0 END) as def,
          SUM(CASE WHEN sect_def_hm = rust_sect_def_hm AND sect_def_oc = rust_sect_def_oc
                        AND sect_rand_hm != rust_sect_rand_hm
                        AND sect_rand_oc != rust_sect_rand_oc THEN 1 ELSE 0 END) as rand_both,
          SUM(CASE WHEN sect_def_hm = rust_sect_def_hm AND sect_def_oc = rust_sect_def_oc
                        AND sect_rand_hm = rust_sect_rand_hm
                        AND sect_rand_oc != rust_sect_rand_oc THEN 1 ELSE 0 END) as rand_oc
        FROM planets WHERE sect_def_hm IS NOT NULL
    """).fetchone()
    ok, d, rb, ro = [rows[i] or 0 for i in range(4)]
    tot = ok + d + rb + ro
    n_stars = conn.execute(
        "SELECT COUNT(DISTINCT star_id) FROM planets WHERE sect_def_hm IS NOT NULL"
    ).fetchone()[0]
    return ok, d, rb, ro, tot, n_stars


def texture_totals(conn):
    """Return (sky_match, stex_match, total, unprocessed)."""
    tot = conn.execute(
        "SELECT COUNT(*) FROM planets WHERE sect_def_sky IS NOT NULL"
    ).fetchone()[0]
    if not tot:
        return 0, 0, 0, 0
    sky = conn.execute("""
        SELECT COUNT(*) FROM planets WHERE sect_def_sky IS NOT NULL
          AND sect_def_sky = rust_sect_def_sky
          AND sect_rand_sky = rust_sect_rand_sky
    """).fetchone()[0]
    stex = conn.execute("""
        SELECT COUNT(*) FROM planets WHERE sect_def_stex IS NOT NULL
          AND sect_def_stex = rust_sect_def_stex
          AND sect_rand_stex = rust_sect_rand_stex
    """).fetchone()[0]
    return sky, stex, tot, tot - sky

def surface_totals(conn):
    """Return (surf_match, atmo_match, pal_match, total, unprocessed)."""
    tot = conn.execute(
        "SELECT COUNT(*) FROM planets WHERE surf_fnv IS NOT NULL"
    ).fetchone()[0]
    if not tot:
        return 0, 0, 0, 0, 0
    surf = conn.execute(
        "SELECT COUNT(*) FROM planets WHERE surf_fnv = rust_surf_fnv AND surf_fnv IS NOT NULL"
    ).fetchone()[0]
    atmo = conn.execute(
        "SELECT COUNT(*) FROM planets WHERE atmo_fnv = rust_atmo_fnv AND atmo_fnv IS NOT NULL"
    ).fetchone()[0]
    pal = conn.execute(
        "SELECT COUNT(*) FROM planets WHERE pal_fnv = rust_pal_fnv AND pal_fnv IS NOT NULL"
    ).fetchone()[0]
    return surf, atmo, pal, tot, tot - surf
