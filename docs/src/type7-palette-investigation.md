# Type-7 Moon Palette Divergence — Root Cause Analysis

**Date**: 2026-07-21
**Status**: **FIXED** — confirmed via 0xFF far-heap fill (588/589 pal match).

## Summary

Type-7 moon palettes diverged between C (BCPP 3.1/DOSBox-X) and the Rust port
because **Rust consumed exactly 2 fewer LCG calls during randoface**, leaving
the LCG 2 steps behind C's at the palette `random(24)` entry point. This shifted
the entire 18-value `random(24)` stream by 2 positions, changing every colour
component computed by `shade()`.

**Root cause**: `lssmooth()` reads past the 64800-byte surface buffer
(position di+361 up to 65200) for the last ~400 iterations. In C's far-heap,
the next farmalloc'd block (the next star's `p_background`) contains **0xFF**
(the DOSBox default page-fill byte). Rust's `Vec<u8>` (65536 bytes) had the
extra 736 bytes (64800-65535) zeroed, so the lssmooth OOB reads returned 0x00
instead of 0xFF. This changed 1 pixel's value in the tail (rows 128-179), which
caused randoface to match 1 fewer pixel (2 fewer `random()` calls).

**Fix**: Fill `Vec` positions 64800-65535 with `0xFF` in `make_bg()`.

## What the divergence is NOT

The AGENTS.md description attributing this to "uninitialized far-heap in
`s_background` rows 128–179" was **incorrect**. Proof:

1. `rndpat` (the inline-ASM random-pattern filler at `surface()`:4818)
   fills **all 64800 bytes** (rows 0–179) deterministically from the planet
   seed. It does not leave the tail uninitialized.
2. `_fmemset(overlay, 0, 32400)` at line 4838 zeros the object chart.
3. The tail is NOT "far-heap garbage" — it is deterministically filled by
   `rndpat` and is identical in both C and Rust.
4. The difference comes from `lssmooth` READing past the buffer, not from an
   uninitialized buffer. C reads the next farmalloc block's content (0xFF);
   Rust read zeros.

## LCG trace evidence (BUENKIT body 24, seed=59787)

| Trace | C | Rust |
|-------|---|------|
| LCG5 (`random(0x7FFF)` × 5 after `srand`) | `27957 17257 27443 9365 30152` | same — **match** |
| LCG18 (`random(24)` × 18 at palette entry) | `4 18 23 17 7 5 8 0 13 11 15 7 13 21 0 14 5 8` | `20 21 4 18 23 17 7 5 8 0 13 11 15 7 13 21 0 14` |

- LCG5 match proves both start from the same `srand(59787)` state.
- LCG18 shows `Rust[2..] == C[0..16]`. Rust was **2 LCG calls behind** C.

## Precise call counting

Borland LCG `seed = seed × 0x15A4E35 + 1` (mod 2^32):

| | Rust (before fix) | C |
|--|-------------------|---|
| Fracture starts from | `0xE98B` (srand) | `0xE98B` (srand) |
| Fracture ends at | `0x8C50C2AB` (7008 steps) | `0x8C50C2AB` (7008 steps) |
| Randoface ends at | `0x87D87CED` (129582 steps) | `0xF4EB6EBB` (129584 steps) |
| Total from srand to palette | **136590** | **136592** |

Both start randoface from the **same** seed `0x8C50C2AB`. C consumed 2 more
calls inside randoface because **1 extra pixel matched** (gr >= 1).

## How `lssmooth` OOB read caused the difference

`lssmooth` reads 16-bit words at `di` and `di+360`. With `QUADWORDS=16200`:
- Iterations: `(16200 - 80) × 4 = 64480`
- Max `di`: 64479
- Read at `di+360`: up to 64839 (past 64800 for di ≥ 64440, ~40 iterations)
- Read at `di+361`: up to 64840 (past 64800 for di ≥ 64439, ~41 iterations)
- Read at `di+720`: up to 65199 (past 64800 for di ≥ 64080, ~400 iterations)

In C's far-heap, the data after the 64800-byte `farmalloc`'d buffer is the
**next** `farmalloc` block (the next star's `p_background`), which DOSBox
initializes to **0xFF** (the default page-fill byte). In Rust's `Vec<u8>` of
65536 bytes (extra 736 bytes), the tail was zeroed.

The single differing byte in the tail changes one pixel from 0 (Rust) to
non-zero (C), which causes randoface to consume 2 extra `random()` calls.

## Fix

In `system.rs::make_bg()`, fill `Vec` positions 64800-65535 with `0xFF`:

```rust
fn make_bg() -> Vec<u8> {
    let mut v = vec![0u8; BG_SEGMENT];
    for b in &mut v[BG_SURFACE..] {
        *b = 0xFF;
    }
    v
}
```

This models the DOSBox far-heap's page-fill byte. Unlike the previous
"uninitialized far-heap" theory, this is a **deterministic, reproducible**
value: DOSBox fills new pages with 0xFF in its default configuration.

## Verification

Before fix: `575/589 pal MATCH` (14 type-7 moon palette divergences).
After fix:  `588/589 pal MATCH` (1 remaining: TWENDZ body 13 = type-7 planet
data artifact, not a moon palette issue).

All 13 type-7 moon palette divergences are resolved.

## 2-dummy-call test (confirms root cause)

Adding 2 dummy `self.rng.random(0x7FFF)` calls after randoface (advancing
Rust's LCG to match C's palette-entry state) made the palette hash match the
canonical C value.

**YOLINX I body 61** (type 7 moon, canonical pal=`DB4BE7FF`):
- Rust without fix: `pal=CD47B093` ❌
- Rust with 2 dummy calls after randoface: `pal=DB4BE7FF` ✅
- Surface hash (rows 0-127): `F7BB9703` — already matched both ways

## Cleanup applied (2026-07-21)

- Removed buggy `C7_FRAC_END` and `C7_LSSM_END` debug traces from
  `source/NOCTIS-0.CPP` (consumed 2 random(0x7FFF) each and re-seeded with
  `srand(seed)` mid-fill-code, which would corrupt the LCG for randoface).
- Removed all Rust debug traces from `surface.rs` and `RUN.BAT`.
- Applied the 0xFF far-heap fix.
- Updated canonical hashes in DB for all 1004 stars.
