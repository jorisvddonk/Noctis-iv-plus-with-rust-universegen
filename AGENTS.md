# AGENTS.md

Rust port of Noctis IV Plus — a **100% standalone deterministic universe generator**.
No C code shipped, no fixtures required, no cheating (no peeking at C output to
derive constants). Every planet, every star, every sector is derived purely from
coordinates + the Borland RNG.

## Purpose

Generate all 2²⁰ × 10⁶ valid coordinate slots accurately without retaining any
C source or fixture files. The Rust port must be:

- **Deterministic** — same input → same output, always
- **Complete** — all star classes, all planet types, all surface/sector data
- **Portable** — no DOSBox, no Borland compiler, no 16-bit x87

## How it works

Input: three Parsis coordinates `(x, y, z)`.

```
SystemGen::generate(x, y, z)
  ├─ star class, body count, planet types, orbits ← Borland LCG
  ├─ per-body: surface map (360×180), atmosphere, palette
  └─ per-body per-landing: sector heightmap (200×200) + object chart
```

Everything is derived from the coordinates via the RNG. No external data needed.

### Key components

| Module | Purpose |
|--------|---------|
| `tests/nivgen/src/rng.rs` | Borland `rand()` / `srand()` and `fast_random()` / `fast_srand()` |
| `tests/nivgen/src/system.rs` | Star system generation (planet count, types, orbits) |
| `tests/nivgen/src/surface.rs` | 360×180 surface map + atmosphere + palette |
| `tests/nivgen/src/surface_sector.rs` | 200×200 sector heightmap + object chart |
| `tests/nivgen/src/f80.rs` | Noctis' x87 80-bit float emulation |
| `tests/nivgen/src/hash.rs` | FNV-1a / CRC-32 for test hashing |

### RNG rules

1. **Borland LCG**: `seed = (seed * 22695477 + 1) & 0x7FFFFFFF`, then `rand() = (seed >> 16) & 0x7FFF`.
2. `srand()` truncates to **16-bit** (`seed & 0xFFFF`) — the parameter is `unsigned int` in Borland.
3. `random(n)` = `(long)rand() * n / 32768`.
4. `flandom()` = `random(32767) * 0.000030518`.
5. Arguments to multi-parameter function calls are evaluated **right-to-left**.

### Float rules

1. All intermediate results are Borland `float` (32-bit) unless explicitly `double`.
2. `cos()`, `sin()`, `sqrt()` take `double` arguments but the C code's inputs are `float`, so the rounding to f32 happens before the promotion.
3. Borland's x87 uses 80-bit internal precision. The port has an `F80` type for cases where this matters (system gen seed derivation).
4. Every intermediate that would be `float` in C must be rounded to f32 via `f64 as f32`.

### Integer rules

1. `int` is **16-bit signed**; `unsigned` is **16-bit unsigned**; `long` is **32-bit signed**.
2. Mixed int/unsigned arithmetic promotes both to unsigned.
3. Right-to-left argument evaluation affects RNG call order.
4. Loop bounds with unsigned mixing (`x = cx - r` where `r` is unsigned) must be replicated exactly — see `round_hill`.

## The database (verification only)

The SQLite DB at `tests/db/noctis.db` stores canonical C fixture hashes for
**95 stars / 2570 bodies** drawn from the Feltyrion Explorer atlas. It is
purely a **verification tool** — not needed for generation.

### Pipeline

```
bulk_atlas.py ──import stars──► noctis.db ──► bulk_sectors.py (one_iter) ──► compare
                                                │
                                          nivgen (Rust binary) 
                                          computes hashes, DB stores them
```

### Commands

```bash
# Full verification for one star:
uv run python3 tests/db/bulk_sectors.py one_iter <STAR_NAME>

# Auto-select next unprocessed star:
uv run python3 tests/db/bulk_sectors.py one_iter

# Full status report:
uv run python3 tests/db/bulk_sectors.py db-report

# Condensed summary:
uv run python3 tests/db/bulk_sectors.py db-report --short

# Import new stars from Feltyrion Explorer atlas:
uv run python3 tests/db/bulk_atlas.py run --per-class 2

# Query the DB:
uv run python3 tests/db/query_db.py           # all planets
uv run python3 tests/db/query_db.py stars     # star systems
uv run python3 tests/db/query_db.py FENIA     # filter by star
```

### What the DB stores

| Column | Content |
|--------|---------|
| `planets.surf_fnv` | C's surface map FNV hash |
| `planets.atmo_fnv` | C's atmosphere FNV hash |
| `planets.pal_fnv` | C's palette FNV hash |
| `planets.sect_def_hm` / `sect_rand_hm` | C's sector heightmap FNV hash |
| `planets.sect_def_oc` / `sect_rand_oc` | C's sector object chart FNV hash |
| `planets.rust_surf_fnv` | Rust's surface map FNV hash (computed by `bulk_sectors.py`) |
| `planets.rust_sect_*` | Rust's sector hashes (computed by `bulk_sectors.py`) |

The `cmp*` columns compare Rust vs C after both are stored.

### Generator mode (no DB needed)

```bash
cd tests/nivgen && cargo run --release -- sector -x X -y Y -z Z -p BODY -lon LON -lat LAT
```

This does NOT touch the DB. All inputs are from coordinates. Output is the
sector text-framing + hashes to stdout (matching the C `NIVTEST.EXE` format).

## Current status

**Surface maps**: 10686/10686 match C (100%). Atmosphere: 10686/10686.
Palette: 10685/10686 (1 pre-existing TWENDZ body 13 fixture variance).

**Sectors**: 9321/9322 match C (1 known r-b\* mismatch — BALASTRACKONASTREYA
body 3, rand landing, ruinschart alias). All 117 former r-o\* mismatches fixed.

## References

- `docs/src/porting-gotchas.md` — every non-obvious Borland behaviour discovered
- `docs/src/sector-float-precision.md` — float math differences in `round_hill`
- `docs/src/type7-palette-investigation.md` — palette investigation
- `docs/src/twendz-palette-mismatch.md` — TWENDZ body 13 palette analysis
- `AGENTS_OLD.md` — archived investigation notes (2026-07-25)
