# Sector Objectchart Mismatch — Analysis

**Date**: 2026-07-21
**Status**: Under investigation — 10 bodies diverge on seedval pseudo landing only.
All default landings (lon=0, lat=60) match byte-exact.

## Summary

10 sector bodies produce mismatching objectcharts on seedval-derived landing
coordinates. The heightmap also diverges for 3 of them. None diverge on the
default landing. The mismatches are **coordinate-sensitive** — triggered by
specific lat/lon pairs that route through different branches in
`setup_landing` or `build_surface`.

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

Key observation: **all default landings match**. The divergence is purely
coordinate-sensitive — different lat/lon → different `setup_landing` branches
(terrain type, inclination calc, ruins placement) → different random() calls.

## Binary diff pattern (from VOIDAL LAMP body 24, type 2 moon)

The AGENTS.md documents 6 differing bytes across 40000:

- **Bytes 150-151**: object class bits differ.
  - C: `0x1F` = `0b00011111` → nr_of_objects=3, object0_class=3(invalid),
    object1_class=1(VEGET), object2_class=0(ROCKS)
  - Rust: `0x03` = `0b00000011` → nr_of_objects=3, all classes=ROCKS(0)

- **Bytes 39966-39973**: `nr_of_objects` nibbles differ near buffer end
  (downstream of the class placement issue).

## What it is NOT

- **NOT a bitfield packing issue.** `struct quadrant` (NOCTIS-D.H:171) packs
  4×2-bit fields into 1 byte. Borland C++ 3.1 confirms `sizeof=1` (the
  `for (ptr=0; ptr<oc_bytes; ptr++)` loop iterates byte-by-byte over
  `objectschart`). The Rust bitfield accessors at `surface_sector.rs:80-91`
  match C's layout byte-for-byte.

- **NOT the `addtrees:` code path.** For types 1/2 (most mismatch bodies),
  `addtrees:` is unreachable (reached only from `case 3:` via
  `switch(sctype)` → `case OCEAN:` / `case PLAINS:`). The type-1/2/9
  `switch` cases all `break` before the `addtrees:` block.

- **NOT a far-heap OOB issue** (distinct from the type-7 palette fix).
  `build_surface` reseeds both `fast_srand` and `srand` from
  `global_surface_seed` at its start, so no carryover LCG state from surface
  generation can affect the sector. The 0xFF fix does not apply here.

## What it likely IS

### The inclination loop OOB read

(NOTE: The LCG random() call sequence through ALL terrain generation was
simulated step-by-step and MATCHES Rust byte-for-byte. Every seed from the
re-seed at `srand(lat*lon)` through rockyground (40k calls), both std_crater
loops, the srf_darkline loop, and the post-switch felisian loop was verified
against Rust's trace seeds. The simulation produces the SAME seed at the
inclination loop entry as Rust. Despite this, the objectchart diverges,
confirming the issue is in the inclination loop's `surf_h` OOB reads or the
loop body itself, not in any prior random() call sequence.)

The `build_surface` loop at NOCTIS-1.CPP:2660-2730 processes all 40000
objectchart quadrants and calls `surf_h(ptr+200)` at line 2687 for the
"inclination" calculation. For `ptr >= 39800`:

```
surf_h(ptr+200) for ptr=39800 → reads objectschart[40000] = oc_gap[0]
surf_h(ptr+200) for ptr=39816 → reads objectschart[40016] = oc_gap[16]
surf_h(ptr+200) for ptr=39984 → reads objectschart[40184]
```

Wait — `objectschart` is 40000 bytes, and `surf_h` reads `objectschart[ptr]`
within bounds. The OOB read is through `p_surfacemap[ptr]` access at the
buffer boundary. Let me re-examine:

Actually, looking more carefully at the C code (NOCTIS-1.CPP), the inclination
calculation uses `p_surfacemap[ptr+200]` at some points — this reads from
`p_surfacemap` (heightmap), not `objectschart`. The heightmap `p_surfacemap`
is 40000 bytes. For `ptr >= 39800`, `ptr+200 >= 40000`, which reads past the
heightmap into the next allocation.

In the C far-heap layout:
- `p_surfacemap = farmalloc(40016)` (aligned to paragraph: 40016+padding)
- `objectschart = farmalloc(40000)` (adjacent, allocated right after)

So `p_surfacemap[40000..40015]` reads the **farmalloc header** of
`objectschart` — 16 bytes of heap metadata (`oc_gap`). And
`p_surfacemap[40016..40199]` reads the first 184 bytes of `objectschart`
itself (quadrants 0-183, which already have `nr_of_objects` set by the loop).

The inclination calculation at `ptr ~39800` reads these OOB bytes, computes
a different `incl` → changes which `random()` calls fire → cascades through
remaining iterations.

### The oc_gap mismatch

The farmalloc header bytes between `p_surfacemap` and `objectschart` are
specific to the DOSBox memory layout. The Rust model uses a hardcoded
`oc_gap` (default: `[0,0,0,0,0,0,0,0,0,0,0,0,197,9,181,84]`), but the
actual farmalloc header may differ per star/per body depending on allocation
order and DOSBox's far-heap state.

If the `oc_gap` is wrong, the OOB read returns different values → different
`incl` → diverging random() calls → different `nr_of_objects` at the end.

### The `make_ruins` raw byte writes

`make_ruins` writes raw 0x40 bytes via `ruinschart[pt] = AF1`. This is a
**raw byte write**, not a bitfield write. In C, writing `0x40` sets specific
bits in the byte. In Rust, the objectchart is accessed through bitfield
setters. If the bitfield setter for raw 0x40 writes a different byte value,
the ruins output would differ.

But `make_ruins` is only called for specific named stars (Balastrackonastreya),
and most mismatch bodies are not in that list. So this is likely not the
general cause — though it may contribute to the BALASTRACKONASTREYA mismatch.

## Next steps

1. **Binary diff one of the 10 bodies** (e.g., REDING body 24) to confirm the
   same 6-byte pattern at bytes 150-151 and 39966-39973. If the pattern
   differs, the root cause may be different per body.

2. **Verify `oc_gap` bytes.** Run the C binary under DOSBox-X with a
   diagnostic that dumps the 16 farmalloc header bytes between `p_surfacemap`
   and `objectschart`. Compare against the Rust hardcoded default. If they
   differ, update the gap bytes in `oc_gap`.

3. **Verify `m200` lookup table** used by `make_ruins`: `m200[j] = j * 200`.
   If computed as `unsigned` (16-bit) in Borland C++, values >= 328 would
   overflow. Check that Rust's `m200` matches.

4. **If the gap is correct:** the issue is in how `incl` is computed from the
   OOB data. Trace the inclination computation for one diverging body and
   compare the `incl` values and random() call sequence between C and Rust.

## Reference

- AGENTS.md "Known non-surface gaps" — original description of the 6-byte
  diff pattern.
- `source/NOCTIS-1.CPP:2660-2730` — the inclination loop.
- `tests/nivgen/src/surface_sector.rs` — Rust sector implementation.
- `docs/src/porting-gotchas.md §29` — bitfield packing analysis.
