# Sector Objectchart Mismatch — Analysis

**Date**: 2026-07-21
**Status**: Still open — 10 bodies diverge on seedval pseudo landing only.
All default landings (lon=0, lat=60) match byte-exact. LCG call sequence
verified correct to inclination loop entry.

## Summary

10 sector bodies produce mismatching objectcharts on seedval-derived landing
coordinates. The heightmap also diverges for 3 of them. None diverge on the
default landing. The mismatches are **coordinate-sensitive** — triggered by
specific lat/lon pairs.

## Affected bodies

| Body | Type | HM | OC | Pattern |
|------|------|----|----|---------|
| BALASTRACKONASTREYA body 3 | 3 planet | ❌ | ❌ | r-b* |
| BALASTRACKONASTREYA body 19 | 3 moon | ❌ | ❌ | r-b* |
| TERA body 0 | 9 planet | ❌ | ❌ | r-b* |
| BOTHO'ES body 42 | 3 moon | ✅ | ❌ | r-o* |
| GALARETKA body 6 | 2 moon | ✅ | ❌ | r-o* |
| INIX I body 12 | 1 moon | ✅ | ❌ | r-o* |
| RAVALISS body 37 | 1 moon | ✅ | ❌ | r-o* |
| REDING body 24 | 1 moon | ✅ | ❌ | r-o* |
| REENBOHW body 23 | 1 moon | ✅ | ❌ | r-o* |
| SADA'UKEIRIT body 63 | 2 moon | ✅ | ❌ | r-o* |

## Confirmed: LCG call sequence matches C

A step-by-step LCG simulation was run from the re-seed at `srand(lat*lon)`
through the entire type-1 terrain generation, verifying every intermediate
seed against Rust trace dumps. **All steps match:**

| Step | Description | Sim seed | Rust seed | Match |
|------|-------------|----------|-----------|-------|
| A | After 1st `random(5)`, n=1 | 0x282434CF | 0x282434CF | ✅ |
| B1 | After `rockyground(25,...)` 40k calls | 0x73EE0A9C | 0x73EE0A9C | ✅ |
| C | After 1st `while(n)` std_crater loop | 0x4EDA9817 | 0x4EDA9817 | ✅ |
| D | After 2nd `while(n)` std_crater loop | 0xE6A1D672 | 0xE6A1D672 | ✅ |
| E | After `srf_darkline` loop | 0x6E9112D5 | 0x6E9112D5 | ✅ |
| F | After rockdensity/rockscaling/rockpeaking | 0xC8C6B581 | 0xC8C6B581 | ✅ |
| G | After post-switch `random(5)`, n=4 | 0x6CCAE1B6 | 0x6CCAE1B6 | ✅ |
| H | After `felisian_srf_darkline` loop | 0x322D279D | 0x322D279D | ✅ |

The simulation was run with the CORRECTED `random_and_advance` semantics (C
advances the seed BEFORE computing the return value). The buggy version
(compute from current seed, then advance) gave a divergent seed at step C.

**Conclusion:** every `random()` call between the re-seed and the inclination
loop is consumed in the same order and count in C and Rust. The LCG state at
the inclination loop entry is IDENTICAL.

## What the issue IS

The inclination loop (NOCTIS-1.CPP:2606-2630) computes:
```c
incl = abs(p_surfacemap[ptr] - p_surfacemap[ptr+1])
     + abs(p_surfacemap[ptr] - p_surfacemap[ptr+200]);
```

For `ptr >= 39800`, `ptr+200 >= 40000`, which reads past the 40000-byte
`p_surfacemap` into the adjacent far-heap allocation. In C's far-heap layout:
- `p_surfacemap` = farmalloc(40000) (rounded to 40016 by paragraph alignment)
- 16 bytes of farmalloc metadata (the `oc_gap`)
- `objectschart` = farmalloc(40000)

So `p_surfacemap[40000..40015]` reads the 16-byte farmalloc header, and
`p_surfacemap[40016..40199]` reads `objectschart[0..183]` (which already has
`nr_of_objects` set from earlier loop iterations).

These OOB reads affect `incl`, which affects which `random()` calls fire,
which cascades through the remaining loop iterations.

## The oc_gap hypothesis

The Rust code models the 16 farmalloc header bytes as `oc_gap`:
```rust
pub oc_gap: [u8; 16],
// default: [0,0,0,0,0,0,0,0,0,0,0,0,197,9,181,84]
```

**Changing the gap bytes changes the objectchart output** (tested with
all-zeros and all-ones gaps — both produce different hashes than the
default, confirming the gap data matters). However, the default gap bytes
produce the WRONG objectchart for 10 bodies.

The correct gap bytes depend on the specific DOSBox farmalloc header, which
may differ per allocation run. Extracting the true gap bytes requires
running the C binary under DOSBox-X with a memory dump.

## Fixes applied (2026-07-21)

- **6 RTL compound expressions** fixed (lines 772-773 flandom order, line
  868 rockdensity `random(2)*random(2)` order, lines 891/1192/1205
  `albedo + random ± random` order). These match C's right-to-left
  evaluation order. They do NOT affect type-1 bodies (most mismatches).
- **Re-seed confirmed unconditional**: C `NOCTIS-1.CPP:2051-2052`
  unconditionally re-seeds both RNGs from `lat*lon` before the type switch.
  Rust already matches this. A conditional fix was reverted.

## Next steps

1. **Extract the true farmalloc header** from the C binary under DOSBox-X
   for one mismatching body. Modify `NOCTIS-1.CPP` to dump 16 bytes at
   `p_surfacemap + 40000`, rebuild, run for REDING body 24, and capture
   the hex. Compare against the default `oc_gap`.

2. **If the gap matches**: the issue is in the inclination loop's readback
   from `objectschart[0..183]` (indices 40016+). The loop writes
   `nr_of_objects` for early quadrants, then reads those same quadrants
   back OOB at high `ptr`. If the loop body produces different
   `nr_of_objects` for any early quadrant (due to a subtle `incl` diff),
   the OOB read feeds stale data into the end-of-buffer iterations.

3. **Binary diff** one mismatching body to confirm the 6-byte pattern
   (bytes 150-151 and 39966-39973) generalizes to all 10 bodies.

## Reference

- AGENTS.md "Known non-surface gaps" — describes the 6-byte diff pattern.
- `source/NOCTIS-1.CPP:2051-2052` — the unconditional re-seed.
- `source/NOCTIS-1.CPP:2606-2630` — the inclination loop.
- `tests/nivgen/src/surface_sector.rs:110-122` — `surf_h()` OOB read.
- `tests/nivgen/src/system.rs:293` — `oc_gap` field declaration.
