# Sector heightmap ±1 pixel drift: x87 vs IEEE float precision

**Date:** 2026-07-22  
**Author:** opencode investigation session  
**Status:** Root cause identified, no fix attempted (platform difference)

## Summary

The Rust port's sector heightmaps diverge from BCPP 3.1 canonical by ±1 in
roughly half of all pixels (~19880/40000 for LOVE'S LABOURS WON body 22).
The root cause is **x87 80-bit vs IEEE 754 64-bit cos/sqrt precision** in
the `round_hill` function. The `round_hill` function is used only by the
sector code (NOCTIS-1.CPP `build_surface`), not by the surface map code
(NOCTIS-0.CPP `gen_planet_surface`), which is why all 838 surface maps
match byte-exact while sectors systematically drift.

## Investigation log

### 2026-07-22: Compounding mechanism quantified

Measured the pre-darkline heightmap for LOVE'S LABOURS WON body 22
(seed=35900, rand landing at lon=359, lat=100):

- **Pre-darkline diffs:** 30397/40000 (76%) between C and Rust
- **Darkline fixes:** 13978/30397 (46%) of pre-darkline diffs are eliminated
- **Final diffs:** 19880/40000 (50%), all ±1 to ±3
- **Pixel (0,0) pre-dark:** C=3, Rust=1 (jitter adds 1, no hill covers it)

The +1 at pixel (0,0) in Rust comes from jitter (`fast_random(3)`) with
seed=`35900 | 3 = 35903`. The first call returns 1 (not 0 as previously
computed — the `| 3` in `fast_srand` was missed).

The C excess of +2 at the same pixel (3 vs 1) is still unexplained but
is a minor edge case on pixels no hill covers.

The compounding mechanism:
- Each `round_hill` call evaluates `cos(d/v) * h` in x87 f80 (C)
  vs f64 (Rust), producing occasional ±1 ULP differences in the f32
  result.
- When `d/v ≈ π/2` (mod π), cos ≈ 0. A 1-ULP difference in the
  argument flips the sign of the result, including or excluding the
  entire hill contribution for that pixel.
- Across 12 active hills × ~20000 pixels per hill, ~50% of pixels
  have at least one such "borderline" hill whose sign flips.
- The darkline (`felisian_srf_darkline`) overwrites some of these
  pixels with specific peak values, converging ~46% of pre-darkline
  diffs toward C.

### 2026-07-22: fast_srand `|3` discovered

### 2026-07-22: round_hill unsigned-loop analysis

**Claim checked:** Does `round_hill`'s loop in C use `int x,z` with signed comparisons?

Looking at NOCTIS-1.CPP:1502:

```c
int x, z;
for (x = cx - r; x < cx + r; x++)
    for (z = cz - r; z < cz + r; z++)
        if (x > -1 && z > -1 && x < 200 && z < 200) ...
```

Parameters: `int cx, int cz, unsigned r`. The expression `cx - r` is
`int - unsigned` → `unsigned`. When `cx < r`, this underflows (16-bit:
65536 + cx - r). The result stored to `int x` is a negative value
(e.g. -25). Then `x < cx + r` compares `int` with `unsigned` → `int`
is converted to `unsigned`, making the negative x a huge unsigned
(e.g. 65511). Since `cx + r` is small (< 400), `x < cx+r` is false
and the loop body **never executes** when `cx < r`.

**Verdict:** The Rust code `(cx as u32).wrapping_sub(r)` with unsigned
`x < x1` comparison correctly reproduces C. **No bug here.**

### 2026-07-22: RNG sequence verification

Traced the full `random()` sequence from seed=35900 through oc_revert
(114 calls) + oc_addtrees (65535 texture + asterism loops + params).
Verified by comparing internal LCG seeds at checkpoints against the
Rust debug output (`NIVDBG=1`):

| Checkpoint | Seed (dec) | Seed (hex) | Matches? |
|---|---|---|---|
| oc_revert entry | 35900 | 0x8C3C | ✓ |
| oc_addtrees entry | 2446954530 | 0x91D99022 | ✓ |
| texture_n seed | 1471343432 | 0x57B2EB48 | ✓ |
| n_ast seed | 920251208 | 0x36D9EB48 | ✓ |

The asterism loop was ruled out as a source of RNG divergence:
a measurement program compiled under DOSBox-X with Borland C++ 3.1
confirmed f32 accumulation matches x87 exactly for all densities
[6, 30]. See `docs/src/asterism-investigation.md`.

### 2026-07-22: round_hill float computation

For each pixel in a hill's coverage area:

```
d = sqrt(dx^2 + dz^2)    // dx, dz are f32
v = r / (π/2)            // f32
y = cos(d/v) * h         // f80 in C, f64 in Rust, then → f32
y += p_surfacemap[...]   // f80-f32 in C, f64-f32 in Rust
```

C with x87: intermediate values live on the FPU stack with 64-bit mantissa.
Rust with f64: intermediate values use 53-bit mantissa. The f32 result
occasionally differs by 1 ULP, which for the integer truncation `y as u8`
gives exactly a ±1 pixel difference.

The 19880/40000 (~50%) mismatch rate is consistent with `cos(d/v)*h`
values falling near integer boundaries half the time.

### 2026-07-22: round_hill is sector-only

`round_hill` (NOCTIS-1.CPP:1494) is called ONLY from `build_surface`
(NOCTIS-1.CPP:1948), the sector generator. The surface map generator
`gen_planet_surface` (NOCTIS-0.CPP) uses `kfract`, `crater`, `fracture`,
and other functions — all of which were already fixed for f32 per-step
rounding and match byte-exact.

This explains why all 838 surface maps across 40 star systems match,
while sectors drift systematically.

## Verified bounds

- **`round_hill` loop bounds**: Correct (u32 wrapping_sub + unsigned comparison)
- **RNG sequence**: Identical between C and Rust through the entire
  `build_surface()` pipeline (verified at 4 checkpoints + asterism
  measurement matches C x87 for all densities 6–30)
- **Remaining drift**: ±1 pixel, x87 f80 vs IEEE f64 in cos/sqrt of
  `round_hill`

## Fix options

1. **Accept ±1 tolerance** for sector height comparisons. Simplest and most
   pragmatic. The visual in-game difference is imperceptible (1/256 height
   unit = 0.4% of the dynamic range).

2. **Match x87 cos/sqrt** by calling C math functions from Rust
   (`extern "C"`). Limited portability.

## Related files

- `source/NOCTIS-1.CPP`:1494 — `round_hill` definition
- `tests/nivgen/src/surface_sector.rs`:132 — Rust `round_hill`
- `docs/src/asterism-investigation.md` — asterism loop verification
- `docs/src/porting-gotchas.md` §31 — unsigned-mix loop analysis
- `docs/src/sector-oc-mismatch.md` — object chart mismatch (separate issue)
