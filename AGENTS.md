# AGENTS.md

Rust port of the Noctis IV Plus universe generator, validated bit-for-bit
against the original Borland C++ 3.1 source compiled under DOSBox-X.

## Layout
- `source/NOCTIS-0.CPP`, `source/NOCTIS-1.CPP` — the **unmodified**
  reference C source (ground truth). Do not "fix" it; port bugs faithfully.
- `tests/nivgen/` — the Rust port (`rng.rs`, `system.rs`, `f80.rs`,
  `hash.rs`). `cargo test` runs RNG/f80/hash regression vectors.
- `tests/harness/NIVTEST.CPP` — the C harness that drives the reference
  source and emits the fixtures in `tests/results/*.OUT` (CRLF).
- `tests/scripts/run-niv-tests.sh --local` — rebuilds the C fixtures
  using native DOSBox-X (BCPP 3.1 from archive.org).
- `tests/db/` — SQLite database of canonical BCPP 3.1 FNV hashes for
  15 star systems, 279 planets. Use `uv run python3 tests/db/query_db.py`
  to look up expected hashes for any body.
- `docs/src/porting-gotchas.md` — **read this before touching the RNG
  or float math.** Documents every non-obvious Borland behaviour discovered
  while porting, including the critical `imul ax` low-word replacement (§17).
- `docs/test-fixtures.md` — coordinates and body indices for every planet
  type across all star classes.

## Critical rules
1. Reproduce bugs, do not "correct" them. The Rust must match the C
   fixtures byte-for-byte (CRLF included).
2. The most common divergence sources are **16-bit `int`/`unsigned`
   arithmetic**, **`imul ax` low-word replacement**, and **x87 80-bit
   float precision**. See the gotchas doc.
3. Never commit `tests/nivgen/target/` (already gitignored).
4. Always compare against BCPP 3.1 output (`run-niv-tests.sh --local`),
   not the old Turbo C++ 3.0 container build.

## Current status (2026-07-19)
**100% of surface maps match (371/371 bodies)** across all 13 star systems
with bodies, comparing the Rust port to canonical BCPP 3.1 / DOSBox-X hashes.

Surface hashes cover **only rows 0-127 (46080 bytes)** of the 360×180 map —
the region the globe renderer can actually sample (rows 0-121). The scratch
tail (rows 128-179) is never displayed and is nondeterministic for moons
(far-heap OOB read), so it is excluded from hashing. See §19.

Match rate by planet type (planet / moon):
- **100% both planet and moon**: all types (0-9)

### Fixed
- **Crater**: crays per-angle (not per-gr), second block per-angle, borrow
  check (JNC=CARRY, not sign), 16-bit `unsigned px, py` truncation
- **Wave** (type 6): the inline-asm `add ax, 4` before the `es:[di]` write
  provably has NO effect on the written offset — verified byte-for-byte
  against the C surfmap, `di` is `py*360 + px`, not `+ 4`. Dropping the
  spurious +4 took type-6 planets to 100%.
- **Fracture**: the globals `a`, `px`, `py` are Borland `float` (32-bit) —
  the angle and position accumulators re-round to f32 every iteration
  (cos/sin promote back to double); `kfract` is also `float`. Accumulating
  in f64 drifted over the 100-200 fracture calls per body. 16-bit py
  `(unsigned)` cast before *360. This fix took types 0, 4, 7 to 100%.
- **psmooth_grays**: packed 4-row dword average (was wrong 2×2 block)
- **surf_case3** (felisian): `add ax, cx` (loop counter), `imul ax` low-word
  replacement, `mov word ptr` zeroing
- **Terminator**: 16-bit `di` wrap in darkside
- **F80 add**: exponent alignment, carry check (`s>>63==1` → `!= 0`)
- **Shade**: F80 gradient accumulation
- **Moon surfaces**: the moon far-heap OOB read (lssmooth past `s_background`)
  lands in scratch rows the game never renders. Fix = hash only the sampled
  rows 0-127 (`SAMPLED_BYTES=46080`), which is fully deterministic across
  single-body / planet-all / in-game. Moons now match at their planet-type
  rate (see §19).
- **Type 5** (`contrast`/`randoface`): Borland evaluates function arguments
  **right-to-left**, so multiple RNG calls within one call statement consume
  values in reverse argument order. Fixing `contrast(...)` and
  `randoface(...)` took type-5 planets to 100% (28/28). See §20.
- **Type 3** (`atm_cyclon`): the `float a` angle accumulator is stepped by
  the `double deg`. Borland computes `a += 6*deg` in double then rounds to
  f32; the port pre-rounded the step (`(6.0*DEG) as f32`), drifting `a` by
  ~1 ULP and flipping a `cirrus` write at an integer boundary. Fixing the
  initializer and the step took type-3 moons to 100% (9/9). See §22.
- **Type 5** (`contrast`): all operands/dest are Borland `float`, so the
  chain rounds to f32 per step. The port had emulated it with 80-bit `F80`,
  which drifted 1 ULP and broke a type-5 moon. Pure f32 per step took type-5
  moons to 100% (24/24). See §23.
- **`planet_xyz` moonorigin**: for a moon, C's `planet_xyz` adds the owner's
  orbital offset via `moonorigin()` (NOCTIS-0.CPP:1459). The port omitted it,
  leaving the moon at the wrong world position. This only surfaces when the
  moon's owner is a **type-10 companion star**, because only then does
  `cplx_planet_viewpoint` call `planet_xyz(logical_id)` and feed the stale
  `plx/plz` into the darkside terminator (`plwp`). One type-6 moon
  (TRIVIAPOLEN) had its terminator band misplaced. Fixing it took the port
  to **100% (371/371)**. See §24.

### Remaining
- None. All 371 surface maps match. See `docs/test-fixtures.md` for
  per-type test coordinates.

### Known non-surface gaps
- A few bodies still differ in the `palette64` hash (e.g. UNNAMED_CLASS1
  body 45). This is pre-existing and unrelated to the surface map; the
  palette RNG path is not yet byte-exact. `compare.py` counts surf+atmo+pal,
  so it will show <100% for those stars even though the **surface** matches.

## Canonical hashes (BCPP 3.1 / DOSBox-X only)
Canonical hashes MUST come from the real toolchain under DOSBox-X. Never
generate them any other way.
```bash
# recompute canonical for one star (planet-all, one DOSBox-X boot):
tests/scripts/recompute-canonical.sh -x X -y Y -z Z -o tests/results/canonical/NAME.out
uv run python3 tests/db/load_canonical.py NAME tests/results/canonical/NAME.out
# then recompute Rust hashes + compare:
uv run python3 tests/db/compare.py NAME
```

## Database
```bash
uv run python3 tests/db/query_db.py           # all planets
uv run python3 tests/db/query_db.py stars     # star systems  
uv run python3 tests/db/query_db.py FENIA     # filter by star
uv run python3 tests/db/seed_db.py            # re-seed from scratch
```
