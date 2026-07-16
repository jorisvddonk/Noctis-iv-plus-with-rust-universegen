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
- `system` generation (T1C00–T1C11, 12 fixtures): all passing.
- Remaining fixture families (T2 planet, T3, T4, T5 sector/surface)
  still to be run and validated against the C reference.
