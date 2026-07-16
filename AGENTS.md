# AGENTS.md

Rust port of the Noctis IV Plus universe generator, validated bit-for-bit
against the original Turbo/Borland C++ 3.0 source compiled under DOSBox.

## Layout
- `source/NOCTIS-0.CPP`, `source/NOCTIS-1.CPP` — the **unmodified**
  reference C source (ground truth). Do not "fix" it; port bugs faithfully.
- `tests/nivgen/` — the Rust port (`rng.rs`, `system.rs`, `f80.rs`,
  `hash.rs`). `cargo test` runs RNG/f80/hash regression vectors.
- `tests/harness/NIVTEST.CPP` — the C harness that drives the reference
  source and emits the fixtures in `tests/results/*.OUT` (CRLF).
- `tests/scripts/run-niv-tests.sh --rebuild` — rebuilds the C fixtures
  under DOSBox inside an Apple container.
- `docs/src/porting-gotchas.md` — **read this before touching the RNG
  or float math.** It documents every non-obvious Borland behaviour
  (16-bit `srand`, the LCG, `random(n)` signed overflow, x87 80-bit
  float, CRLF output, …) discovered while porting.

## Critical rules
1. Reproduce bugs, do not "correct" them. The Rust must match the C
   fixtures byte-for-byte (CRLF included).
2. The single most common divergence source is **16-bit `int`/`unsigned`
   arithmetic and the **80-bit x87** float precision. See the gotchas doc.
3. Never commit `tests/nivgen/target/` (already gitignored).

## Current status
- `system` generation (T1C00–T1C11, 12 fixtures): **all passing**.
- `starsystem-scan` (T5A/T5B): passing (reuses system-gen).
- `planet` / `sector` / `surftex` (T2P00–09, T3A–D, T4A–D):
  **NOT YET PORTED**. `cmd_planet` / `cmd_sector` / `cmd_surftex` in
  `tests/nivgen/src/main.rs` are stubs (`eprintln!("… not yet
  implemented")`). The real work — porting `surface()` / `build_surface()`
  from `source/NOCTIS-1.CPP` into `SystemGen` — is outstanding.
  These functions drive the planet texture (`p_background` 360×180 map,
  atmosphere overlay, palette) and the sector heightmap / surface texture,
  and exercise the `fast_random` RNG (see `rng.rs::FastRng`, already
  ported + validated) alongside the Borland LCG. Expect the same class of
  bugs as T1C (16-bit `int`/`unsigned` overflow via `i16()`, x87 80-bit
  float via `f80`) when porting them.

## Porting checklist for T2/T3/T4
1. Port `surface()` (NOCTIS-1.CPP) → a `SystemGen` method; emit the
   same CRLF lines + FNV/CRC hashes the harness does (see
   `tests/harness/NIVTEST.CPP` `cmd_planet`/`cmd_sector`/`cmd_surftex`).
2. `fast_srand`/`fast_random` are already bit-exact (`FastRng`); re-seed
   them exactly where the C code does (`seedval + 4112`, `seedval * 10`, …).
3. Watch 16-bit overflow on `random()`/`zrandom()` args → wrap with `i16()`.
4. Watch x87 80-bit float on any chained `double` expression that later
   feeds `srand`/`fast_srand` or an `int` cast → use `f80::F80`.
5. Validate each fixture with `tests/scripts/run-niv-tests.sh --rebuild`
   (regenerates the C reference) then diff the Rust output byte-for-byte
   (CRLF-sensitive).
