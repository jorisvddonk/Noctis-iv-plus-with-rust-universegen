import os, sys, json, tempfile, subprocess, struct
from pathlib import Path
from io import BytesIO

from fastapi import FastAPI, Query, HTTPException
from fastapi.responses import Response, HTMLResponse
from fastapi.staticfiles import StaticFiles
from jinja2 import Environment, FileSystemLoader
from PIL import Image
import sqlite3

REPO = Path(__file__).resolve().parent.parent
DB = REPO / "tests" / "db" / "noctis.db"
NIVGEN = REPO / "tests" / "nivgen" / "target" / "release" / "nivgen"
SAMPLED_BYTES = 360 * 128  # 46080

app = FastAPI(title="Noctis IV Visual Verifier")
templates = Environment(loader=FileSystemLoader(REPO / "server" / "templates"))

# ── helpers ──────────────────────────────────────────────────────────────────

def db():
    conn = sqlite3.connect(str(DB))
    conn.row_factory = sqlite3.Row
    return conn

def nivgen_run(*args):
    env = os.environ.copy()
    tmp = tempfile.mkdtemp()
    env["NIVDUMP"] = tmp
    try:
        subprocess.run([str(NIVGEN)] + list(args), env=env,
                       capture_output=True, text=True, check=True, timeout=60)
        return tmp
    except subprocess.CalledProcessError as e:
        import shutil; shutil.rmtree(tmp, ignore_errors=True)
        raise HTTPException(500, f"nivgen failed: {e.stderr[:500]}")
    except subprocess.TimeoutExpired:
        import shutil; shutil.rmtree(tmp, ignore_errors=True)
        raise HTTPException(500, "nivgen timed out")

PLANET_TYPES = {
    0: "Internally hot", 1: "Dusty", 2: "Thick atmosphere",
    3: "Felisian", 4: "Rocky creased", 5: "Thin atmosphere",
    6: "Gas giant", 7: "Icy", 8: "Quartz",
    9: "Substellar object", 10: "Companion star",
}

def db_report():
    import sys as _sys
    _sys.path.insert(0, str(REPO / "tests" / "db"))
    from report import sector_totals, surface_totals, texture_totals
    with db() as conn:
        ok, d, rb, ro, tot, stars = sector_totals(conn)
        surf, atmo, pal, s_tot, unproc = surface_totals(conn)
        sky_def, sky_rand, stex_def, stex_rand, tex_tot = texture_totals(conn)
    return {
        "sector_match": ok, "sector_total": tot,
        "sector_def": d, "sector_rb": rb, "sector_ro": ro,
        "stars": stars,
        "surf_match": surf, "surf_total": s_tot,
        "atmo_match": atmo, "pal_match": pal,
        "surf_unprocessed": unproc,
        "sky_def": sky_def, "sky_rand": sky_rand,
        "stex_def": stex_def, "stex_rand": stex_rand,
        "texture_total": tex_tot,
    }

def has_surface(ptype: int) -> bool:
    return ptype != 10

def no_data_png(text="no data"):
    img = Image.new("RGB", (200, 100), (32, 32, 32))
    from PIL import ImageDraw
    d = ImageDraw.Draw(img)
    d.text((10, 40), text, fill=(100, 100, 100))
    buf = BytesIO()
    img.save(buf, "PNG")
    return buf.getvalue()

SCTYPE_NAMES = {1: "Ocean", 2: "Plains", 3: "Desert", 4: "Icy"}

# ── renderers ────────────────────────────────────────────────────────────────

def render_surface(raw: bytes, palette=None):
    if len(raw) == 65536:
        raw = raw[:64800]
    w, h = 360, len(raw) // 360
    if h < 1:
        h = 180
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            v = raw[y * w + x]
            if palette:
                i = v * 3
                r = min(palette[i] * 4, 255)
                g = min(palette[i+1] * 4, 255)
                b = min(palette[i+2] * 4, 255)
            else:
                r = g = b = v
            px[x, y] = (r, g, b)
    return img

def render_heightmap(raw: bytes, w=200, h=200):
    vals = [raw[y * w + x] for y in range(h) for x in range(w)]
    lo, hi = min(vals), max(vals)
    rng = max(hi - lo, 1)
    colors = [(0,0,0),(50,120,50),(90,180,80),(140,220,110),(180,240,140),
              (210,250,170),(230,250,200),(240,240,210),(250,245,220),(255,255,240)]
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            v = raw[y * w + x]
            idx = (v - lo) * (len(colors) - 1) // rng
            px[x, y] = colors[min(idx, len(colors) - 1)]
    return img

def render_palette(raw: bytes):
    n = len(raw) // 3
    img = Image.new("RGB", (n * 8, 32))
    px = img.load()
    for i in range(n):
        r = min(raw[i * 3] * 4, 255)
        g = min(raw[i * 3 + 1] * 4, 255) if i * 3 + 1 < len(raw) else 0
        b = min(raw[i * 3 + 2] * 4, 255) if i * 3 + 2 < len(raw) else 0
        for x in range(8):
            for y in range(32):
                px[i * 8 + x, y] = (r, g, b)
    return img

def render_atmo(raw: bytes, w=360):
    h = 32400 // w
    img = Image.new("L", (w, h))
    img.frombytes(bytes(raw[:w*h]))
    return img

# ── routes ────────────────────────────────────────────────────────────────────

@app.get("/", response_class=HTMLResponse)
def index():
    with db() as conn:
        stars = conn.execute("""
            SELECT s.*, (SELECT COUNT(*) FROM planets WHERE star_id=s.id) AS nob
            FROM stars s ORDER BY s.name
        """).fetchall()
    return templates.get_template("index.html").render(stars=stars)

@app.get("/api/report")
def api_report():
    return db_report()

@app.get("/api/mismatches")
def api_mismatches():
    with db() as conn:
        rows = conn.execute("""
            SELECT s.name, p.body_index, p.planet_type,
                   p.sect_def_hm, p.rust_sect_def_hm,
                   p.sect_def_oc, p.rust_sect_def_oc,
                   p.sect_rand_hm, p.rust_sect_rand_hm,
                   p.sect_rand_oc, p.rust_sect_rand_oc,
                   p.surf_fnv, p.rust_surf_fnv,
                   p.atmo_fnv, p.rust_atmo_fnv,
                   p.pal_fnv, p.rust_pal_fnv,
                   p.sect_def_sky, p.rust_sect_def_sky,
                   p.sect_rand_sky, p.rust_sect_rand_sky,
                   p.sect_def_stex, p.rust_sect_def_stex,
                   p.sect_rand_stex, p.rust_sect_rand_stex
            FROM planets p JOIN stars s ON s.id=p.star_id
            WHERE (p.sect_def_hm IS NOT NULL
              AND NOT (p.sect_def_hm = p.rust_sect_def_hm
                       AND p.sect_def_oc = p.rust_sect_def_oc
                       AND p.sect_rand_hm = p.rust_sect_rand_hm
                       AND p.sect_rand_oc = p.rust_sect_rand_oc))
               OR (p.surf_fnv IS NOT NULL
                   AND (p.surf_fnv != p.rust_surf_fnv
                        OR p.atmo_fnv != p.rust_atmo_fnv
                        OR p.pal_fnv != p.rust_pal_fnv))
               OR (p.sect_def_sky IS NOT NULL
                   AND (p.sect_def_sky != p.rust_sect_def_sky
                        OR p.sect_rand_sky != p.rust_sect_rand_sky))
               OR (p.sect_def_stex IS NOT NULL
                   AND (p.sect_def_stex != p.rust_sect_def_stex
                        OR p.sect_rand_stex != p.rust_sect_rand_stex))
            ORDER BY s.name, p.body_index
        """).fetchall()
    ms = []
    for r in rows:
        sname, bi, pt = r[0], r[1], r[2]
        diffs = []
        if r[3] is not None and r[3] != r[4]: diffs.append(f"def-hm {r[3][:8]} vs {r[4][:8]}")
        if r[5] is not None and r[5] != r[6]: diffs.append(f"def-oc {r[5][:8]} vs {r[6][:8]}")
        if r[7] is not None and r[7] != r[8]: diffs.append(f"rand-hm {r[7][:8]} vs {r[8][:8]}")
        if r[9] is not None and r[9] != r[10]: diffs.append(f"rand-oc {r[9][:8]} vs {r[10][:8]}")
        if r[11] is not None and r[11] != r[12]: diffs.append(f"surf {r[11][:8]} vs {r[12][:8]}")
        if r[13] is not None and r[13] != r[14]: diffs.append(f"atmo {r[13][:8]} vs {r[14][:8]}")
        if r[15] is not None and r[15] != r[16]: diffs.append(f"pal {r[15][:8]} vs {r[16][:8]}")
        if r[17] is not None and r[17] != r[18]: diffs.append(f"def-sky {r[17][:8]} vs {r[18][:8]}")
        if r[19] is not None and r[19] != r[20]: diffs.append(f"rand-sky {r[19][:8]} vs {r[20][:8]}")
        if r[21] is not None and r[21] != r[22]: diffs.append(f"def-stex {r[21][:8]} vs {r[22][:8]}")
        if r[23] is not None and r[23] != r[24]: diffs.append(f"rand-stex {r[23][:8]} vs {r[24][:8]}")
        ms.append({"star": sname, "body": bi, "type": pt, "diffs": diffs})
    return ms

@app.get("/api/stars")
def api_stars():
    with db() as conn:
        rows = conn.execute("""
            SELECT s.*, (SELECT COUNT(*) FROM planets WHERE star_id=s.id) AS nob
            FROM stars s ORDER BY s.name
        """).fetchall()
    return [dict(r) for r in rows]

@app.get("/api/stars/{name}")
def api_star(name: str):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404, f"star '{name}' not found")
        planets = conn.execute(
            "SELECT * FROM planets WHERE star_id=? ORDER BY body_index",
            (star["id"],)
        ).fetchall()
    return {"star": dict(star), "planets": [dict(p) for p in planets]}

@app.get("/api/stars/{name}/bodies/{body}")
def api_body(name: str, body: int):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404)
        p = conn.execute(
            "SELECT * FROM planets WHERE star_id=? AND body_index=?",
            (star["id"], body)
        ).fetchone()
        if not p:
            raise HTTPException(404)
        return dict(p)

# ── rendered images ──────────────────────────────────────────────────────────

@app.get("/stars/{name}/bodies/{body}/surface.png")
def surface_png(name: str, body: int,
                lon: int = Query(0), lat: int = Query(60),
                albedo: int = Query(32), sc: int = Query(-1)):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404)
        p = conn.execute("SELECT * FROM planets WHERE star_id=? AND body_index=?",
                         (star["id"], body)).fetchone()
    if not p:
        raise HTTPException(404)
    if not has_surface(p["planet_type"]):
        return Response(no_data_png("companion star - no surface"), media_type="image/png")
    dump = nivgen_run("planet", "-x", str(star["x"]), "-y", str(star["y_user"]),
                      "-z", str(star["z"]), "-p", str(body))
    try:
        is_moon = bool(p["is_moon"]) or (p["p_owner"] > -1)
        sf = os.path.join(dump, "moonsurf.raw" if is_moon else "surface.raw")
        raw = open(sf, "rb").read()
        pal = open(os.path.join(dump, "palette.raw"), "rb").read()
    except FileNotFoundError:
        import shutil; shutil.rmtree(dump, ignore_errors=True)
        return Response(no_data_png("no surface data"), media_type="image/png")
    import shutil; shutil.rmtree(dump, ignore_errors=True)

    base3 = (128 if is_moon else 192) * 3
    pal64 = pal[base3:base3 + 192]
    img = render_surface(raw, palette=pal64)
    buf = BytesIO()
    img.save(buf, "PNG")
    return Response(buf.getvalue(), media_type="image/png")

@app.get("/stars/{name}/bodies/{body}/sector.png")
def sector_png(name: str, body: int,
               lon: int = Query(21), lat: int = Query(87),
               albedo: int = Query(32), sc: int = Query(-1)):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404)
        p = conn.execute("SELECT * FROM planets WHERE star_id=? AND body_index=?",
                         (star["id"], body)).fetchone()
    if not p:
        raise HTTPException(404)
    if not has_surface(p["planet_type"]):
        return Response(no_data_png("companion star - no surface"), media_type="image/png")
    dump = nivgen_run("sector", "-x", str(star["x"]), "-y", str(star["y_user"]),
                      "-z", str(star["z"]), "-p", str(body),
                      "-lon", str(lon), "-lat", str(lat),
                      "-albedo", str(albedo), "-sc", str(sc))
    try:
        raw = open(os.path.join(dump, "HEIGHT.bin"), "rb").read(40000)
    except FileNotFoundError:
        import shutil; shutil.rmtree(dump, ignore_errors=True)
        return Response(no_data_png("no sector data"), media_type="image/png")
    import shutil; shutil.rmtree(dump, ignore_errors=True)

    img = render_heightmap(raw)
    buf = BytesIO()
    img.save(buf, "PNG")
    return Response(buf.getvalue(), media_type="image/png")

@app.get("/stars/{name}/bodies/{body}/palette.png")
def palette_png(name: str, body: int,
                lon: int = Query(0), lat: int = Query(60),
                albedo: int = Query(32), sc: int = Query(-1)):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404)
        p = conn.execute("SELECT * FROM planets WHERE star_id=? AND body_index=?",
                         (star["id"], body)).fetchone()
    if not p:
        raise HTTPException(404)
    if not has_surface(p["planet_type"]):
        return Response(no_data_png("companion star - no surface"), media_type="image/png")
    dump = nivgen_run("planet", "-x", str(star["x"]), "-y", str(star["y_user"]),
                      "-z", str(star["z"]), "-p", str(body))
    try:
        pal = open(os.path.join(dump, "palette.raw"), "rb").read()
    except FileNotFoundError:
        import shutil; shutil.rmtree(dump, ignore_errors=True)
        return Response(no_data_png("no palette"), media_type="image/png")
    import shutil; shutil.rmtree(dump, ignore_errors=True)

    is_moon = bool(p["is_moon"]) or (p["p_owner"] > -1)
    base3 = (128 if is_moon else 192) * 3
    pal64 = pal[base3:base3 + 192]
    img = render_palette(pal64)
    buf = BytesIO()
    img.save(buf, "PNG")
    return Response(buf.getvalue(), media_type="image/png")

@app.get("/stars/{name}/bodies/{body}/surftex.png")
def surftex_png(name: str, body: int,
                lon: int = Query(21), lat: int = Query(87),
                albedo: int = Query(32), sc: int = Query(-1)):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404)
        p = conn.execute("SELECT * FROM planets WHERE star_id=? AND body_index=?",
                         (star["id"], body)).fetchone()
    if not p:
        raise HTTPException(404)
    if not has_surface(p["planet_type"]):
        return Response(no_data_png("companion star - no surface"), media_type="image/png")
    dump = nivgen_run("surftex", "-x", str(star["x"]), "-y", str(star["y_user"]),
                      "-z", str(star["z"]), "-p", str(body),
                      "-lon", str(lon), "-lat", str(lat),
                      "-albedo", str(albedo), "-sc", str(sc))
    try:
        raw = open(os.path.join(dump, "SURFTEX.bin"), "rb").read(65536)
    except FileNotFoundError:
        import shutil; shutil.rmtree(dump, ignore_errors=True)
        return Response(no_data_png("no surftex data"), media_type="image/png")
    import shutil; shutil.rmtree(dump, ignore_errors=True)

    # Render 256x256 grayscale
    img = Image.new("RGB", (256, 256))
    px = img.load()
    for y in range(256):
        for x in range(256):
            v = raw[y * 256 + x]
            # Scale 6-bit (0-63) to 8-bit with *4
            r = g = b = min(v * 4, 255)
            px[x, y] = (r, g, b)
    buf = BytesIO()
    img.save(buf, "PNG")
    return Response(buf.getvalue(), media_type="image/png")

@app.get("/stars/{name}/bodies/{body}/atmo.png")
def atmo_png(name: str, body: int,
             lon: int = Query(0), lat: int = Query(60),
             albedo: int = Query(32), sc: int = Query(-1)):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404)
        p = conn.execute("SELECT * FROM planets WHERE star_id=? AND body_index=?",
                         (star["id"], body)).fetchone()
    if not p:
        raise HTTPException(404)
    if not has_surface(p["planet_type"]):
        return Response(no_data_png("companion star - no surface"), media_type="image/png")
    dump = nivgen_run("planet", "-x", str(star["x"]), "-y", str(star["y_user"]),
                      "-z", str(star["z"]), "-p", str(body))
    try:
        raw = open(os.path.join(dump, "atmo.raw"), "rb").read(32400)
    except FileNotFoundError:
        import shutil; shutil.rmtree(dump, ignore_errors=True)
        raise HTTPException(500)
    import shutil; shutil.rmtree(dump, ignore_errors=True)

    img = render_atmo(raw)
    buf = BytesIO()
    img.save(buf, "PNG")
    return Response(buf.getvalue(), media_type="image/png")

@app.get("/stars/{name}/bodies/{body}/sky.png")
def sky_png(name: str, body: int,
            lon: int = Query(21), lat: int = Query(87),
            albedo: int = Query(32), sc: int = Query(-1)):
    with db() as conn:
        star = conn.execute("SELECT * FROM stars WHERE name=?", (name,)).fetchone()
        if not star:
            raise HTTPException(404)
        p = conn.execute("SELECT * FROM planets WHERE star_id=? AND body_index=?",
                         (star["id"], body)).fetchone()
    if not p:
        raise HTTPException(404)
    if not has_surface(p["planet_type"]):
        return Response(no_data_png("companion star - no surface"), media_type="image/png")
    dump = nivgen_run("surftex", "-x", str(star["x"]), "-y", str(star["y_user"]),
                      "-z", str(star["z"]), "-p", str(body),
                      "-lon", str(lon), "-lat", str(lat),
                      "-albedo", str(albedo), "-sc", str(sc))
    try:
        raw = open(os.path.join(dump, "SKY.bin"), "rb").read(64800)
    except FileNotFoundError:
        import shutil; shutil.rmtree(dump, ignore_errors=True)
        return Response(no_data_png("no sky data"), media_type="image/png")
    import shutil; shutil.rmtree(dump, ignore_errors=True)

    # Render 360x180 grayscale
    img = Image.new("L", (360, 180))
    img.frombytes(raw)
    buf = BytesIO()
    img.save(buf, "PNG")
    return Response(buf.getvalue(), media_type="image/png")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=4274)
