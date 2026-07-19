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
**85.4% of surface maps match (317/371 bodies)** across all 13 star systems
with bodies, comparing the Rust port to canonical BCPP 3.1 / DOSBox-X hashes.

Surface hashes cover **only rows 0-127 (46080 bytes)** of the 360×180 map —
the region the globe renderer can actually sample (rows 0-121). The scratch
tail (rows 128-179) is never displayed and is nondeterministic for moons
(far-heap OOB read), so it is excluded from hashing. See §19.

Match rate by planet type (planet / moon):
- **100%**: type 0, 1, 2, 4, 6, 7, 8 (planet), type 9 (planet-only),
  type 3 planet; moons of 0, 1, 2, 4, 7, 8 also 100%
- **~75-89%**: type 3 moon, type 6 moon (residual far-heap tail)
- **0%**: type 5 (both) — still diverges (contrast/F80 path)

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

### Remaining
- Type 5 (planets + moons): surf_map fully diverges (F80/contrast path)
- Type-3 moon, type-6 moon: one residual each (far-heap OOB tail)
- See `docs/test-fixtures.md` for test coordinates per type

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
