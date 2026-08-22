# Remaining heightmap mismatches (3 type-2 bodies)

**Date:** 2026-07-30  
**Status:** Root cause unidentified; float precision ruled out

## Problem

Three bodies (4, 13, 21 of ZUNGDARK BETA — all **type 2 venusian**, rand
landings only) produce sector heightmaps that differ between C (DOSBox-X +
Borland C 3.1) and Rust by **1225 / 40000 pixels (3%)**. The def landings
(lon=0, lat=60) match perfectly for all three.

```
body   4: DIFF lon=73 lat=15 rand-hm=44B28ECB/F0DD723B  (C / Rust)
body  13: DIFF lon=49 lat=22 rand-hm=DB4E10D6/F8B6F1F9
body  21: DIFF lon=91 lat=53 rand-hm=B2AB4661/F45061CF
```

The objectchart diverges even more (74%) — a secondary cascade from
different height inclinations → different RNG call counts in the object
placement loop.

## What's been ruled out

### 1. RNG sequence (ruled out)
The Borland LCG (`rand()`, `srand()`, `random()`) and the fast PRNG
(`fast_random()`, `fast_srand()`, `flandom()`) have been verified to
produce **identical** output between C and Rust:
- The full `random()` sequence seeded with `global_surface_seed=2173702`
  matches through the global-parameter preamble (12 calls: `random(2)`,
  `random(100)`, `random(3)`, `flandom()`, etc.)
- After `srand(land_seed=1095)`, the 40000 `random(10)` calls in
  `rockyground(10,1,0)` produce identical values
- The `random(100)=67` used for the hill loop count (`n=99`) matches
- The `random(50)`, `random(100)`, `random(200)` calls for round_hill
  parameters match
- The surface detail crater-loop RNG calls match
- Rockscaling/rockdensity/rockpeaking values match

The RNG state is confirmed to be identical at every traced checkpoint.

### 2. Unsigned loop bounds in round_hill (ruled out)
The C `round_hill` function uses mixed int/unsigned arithmetic for loop
bounds:
```c
int x, z;
for (x = cx - r; x < cx + r; x++)    // cx is int, r is unsigned
    for (z = cz - r; z < cz + r; z++)
```

When `cx < r`, `cx - r` underflows to a huge unsigned value (16-bit:
`0x10000 + cx - r`). This is stored to `int x` (wrapping to negative in
16-bit). The comparison `x < cx + r` promotes the negative `int` to
`unsigned`, giving a huge unsigned on the LHS → comparison false → loop
body never executes.

Both C and Rust use **unsigned** comparison (confirmed: signed comparison
gives 97% mismatch).  Only 33 of 99 candidate hills execute (those with
both `cx >= cr` AND `cz >= cr`).  The same 33 hills execute in both.

### 3. F80 vs f64 cos precision (ruled out)
x87 computes `cos(d/v)` with 80-bit extended precision (64-bit mantissa),
while Rust uses f64 (53-bit). A software F80 implementation was built
(cos via 20-term Taylor series with full range reduction, sqrt via Newton,
direct F80→f32 rounding) and integrated into round_hill.

Result: **identical** heightmap (1225 differences vs C) — F80 cos gives
the same f32 output as f64 cos for every pixel.  The per-pixel error from
64-bit vs 53-bit cos computation is ≤1e-15 relative, far below the u8
quantum of 1.0, confirming float precision is not the cause.

### 4. F80 sqrt precision (ruled out)
Same conclusion as cos — sqrt differences between 64-bit and 53-bit
are < 1e-15 and have no measurable effect on the u8 heightmap.

### 5. Right-to-left argument evaluation (ruled out)
The Rust code correctly emulates RTL argument evaluation for
`round_hill(random(200), random(200), random(100)+50, random(50)+10, 0, 1)`
by evaluating `h` first, then `r`, then `cz`, then `cx`.

### 6. smoothterrain (ruled out)
The 4-neighbor averaging loop (`(n + n+1 + n+200 + n+201) >> 2`) iterates
`ptr in 0..39799` and has no read-after-write hazards.  Identical in C and
Rust.

### 7. 16-bit integer overflow in land_seed (ruled out)
The `land_seed = lat * lon` computation correctly uses 16-bit wrapping
multiplication.  For the 3 problem bodies the products are small (1095,
1078, 4823) and do not overflow, so the sign-extension fix is irrelevant
here.

### 8. Object encoding (ruled out)
The `quadrant` bitfield layout (nr_of_objects 0-1, object0_class 2-3,
object1_class 4-5, object2_class 6-7) and accessor functions match C.

## What's NOT been ruled out

### A. x87 FCOS vs libm cos: different implementation
x87 uses the `FCOS` instruction (proprietary Intel microcode), while Rust
uses the system libm `cos()` on ARM64.  Although both are high-precision,
they may give genuinely different results for specific arguments by up to
1 ULP of 80-bit.  When rounded to f32, this difference disappears for
99.9999% of inputs, but the 3% mismatch rate rules this out as the primary
cause (it could only account for << 0.001%).

### B. Unexplored: integer overflow in `dx*dx + dz*dz`
The distance-squared computation `(xi - cx)^2 + (zi - cz)^2` uses `i64`
in Rust.  In C, `dx = xi - cx` is `int` (16-bit, may overflow for dx > 32767
— but max dx is ±199, so this is fine).  The product `dx*dx` is `int`
multiplication (16-bit), could overflow for dx > 181 (since 182² = 33124 >
32767).  But with max dx = 199, `dx*dx` overflows 16-bit signed!

In C: `dx = x - cx` (int, could be ±199). `dx*dx` is int multiplication:
199² = 39601 > 32767 → 16-bit signed overflow → UB or wrap to
`39601 - 65536 = -25935`.  Then `d² = dx*dx + dz*dz` may be negative,
and `sqrt(d²)` of a negative number → NaN!

**But** this would cause NaN in BOTH C and Rust (if Rust uses i32 for
`dx`/`dz`), so it wouldn't explain the C-vs-Rust difference.  Also, `d`
is computed from `dx` and `dz` as f32, which are exact for small integers.

**Correction**: `d = sqrt((dx as f64)² + (dz as f64)²)` uses f64, and
`dx` is f32 (exact for small ints).  No 16-bit overflow in the f32/f64
path because it's computed as f32 → f64.

### C. Unexplored: `_fmemset` byte count for txtr
C: `_fmemset(txtr, 16, 65535)`  
Rust: `for i in 0..65532 { p_background[i] = 16; }`

The C memset fills 65535 bytes; Rust fills 65532 and then zeroes the
remaining 4.  The texturing code writes to `txtr` (alias p_background)
at positions determined by the terrain.  If the texturing reads from
uninitialized bytes in [65533, 65535] in C but gets 0 in Rust, this
could affect the surftex hash but NOT the heightmap hash (which covers
p_surfacemap, not p_background).

### D. Unexplored: compiler optimization differences
The Borland C 3.1 compiler may reorder operations or use different
intermediate precision for sub-expressions of `cos(d/v) * h` compared to
Rust's explicit F80 sequence.  For example, `cos` computes the argument
in 80-bit on the FPU stack, while F80 computes the argument as F80
division followed by the Taylor series.  The argument `d/v` might differ
by a few ULPs of F80 due to:
- F80 mul/sub for division vs F80 from_f64 + F80 div
- Different rounding at each step

These could compound into ~1-3 ULPs of f32 at the final rounding step,
affecting ~3% of pixels if the values happen to fall near u8 thresholds.

## How the RNG state diverges (objectchart cascade)

Even if the heightmap differences are tiny (±1 ULP), they affect the
object-placement loop at lines 2610-2622 of NOCTIS-1.CPP:

```c
for (ptr = 0; ptr < oc_bytes; ptr ++) {
    incl  = abs (p_surfacemap[ptr] - p_surfacemap[ptr +   1]);
    incl += abs (p_surfacemap[ptr] - p_surfacemap[ptr + 200]);
    if (incl < 20)
        objectschart[ptr].nr_of_objects = random (2);   // 1-2 RNG calls
    if (incl < 15)
        objectschart[ptr].nr_of_objects = random (3);   // 2-3 RNG calls
    if (incl < 10)
        objectschart[ptr].nr_of_objects = random (4);   // 3-4 RNG calls
}
```

A heightmap difference of ±1 can change `incl` by 1-2, which can cross a
threshold (10, 15, or 20), changing the number of `random()` calls for that
pixel.  This causes the RNG state to diverge from that pixel onward, and
all subsequent pixels get different `nr_of_objects` values.  Result: ~74%
of object bytes differ.

## Reproducing

```bash
# Build C harness (recompiles under DOSBox-X):
uv run python3 tests/db/bulk_sectors.py one_iter "ZUNGDARK BETA" --force --build

# Or run the Rust binary directly for one body:
cd tests/nivgen
cargo run --release -- sector \
  -x -130128 -y 137501 -z 111395356 -p 4 -lon 73 -lat 15

# Compare C and Rust height buffers:
mkdir -p /tmp/c /tmp/r
# (run C harness with -dump D:\c_dir, Rust with NIVDUMP=/tmp/r)
cmp -l /tmp/c/HEIGHT.BIN /tmp/r/HEIGHT.bin | head
```

## Key coordinates

| Body | Type | lon | lat | land_seed | global_surface_seed |
|------|------|-----|-----|-----------|---------------------|
| 4    | 2    | 73  | 15  | 1095      | 2173702             |
| 13   | 2    | 49  | 22  | 1078      | 2173702             |
| 21   | 2    | 91  | 53  | 4823      | 2173702             |

Star: ZUNGDARK BETA, Parsons x=-130128, y_user=137501, z=111395356

All three are type-2 (venusian / thick atmosphere).  The def landing
(lon=0, lat=60, land_seed=0) matches for all bodies.  The matching def
landing tells us the terrain system works correctly for land_seed=0;
the problem is specific to certain non-zero land_seed values.

## Suggested next investigations

1. **Binary-diff C vs Rust height buffers** after rockyground but before
   round_hill, to pinpoint where the first divergence occurs.  Requires
   modifying C source to add an intermediate dump.

2. **Check 16-bit overflow in `dx*dx + dz*dz` when computed as int**
   (the `round_hill` loop uses `int x, z` in C but these are small
   range [−199..199]).  Verify the sqrt input is always positive.

3. **Compare F80 `cos` with x87 `FCOS` on known test vectors** — run
   both on a range of arguments and find any where the sign differs
   (the Taylor series vs FCOS microcode may disagree at the 80-bit ULP
   level near π/2).

4. **Test on x86_64 hardware** where `long double` gives native 80-bit
   precision, to see if the mismatches persist.

5. **Add a debug flag to dump every pixel's `cos(d/v)` value** from
   round_hill, then compare C and Rust outputs at the pixel level
   to find the exact first divergence.

## Recent investigation (2026-07-30)

### Attempts that did NOT fix the issue:

1. **Computing d using F80 sqrt instead of f64 sqrt**: No change. This suggests
   that the difference between 80-bit FSQRT and 64-bit sqrt is not the cause.
   
2. **Using native f64 cos() instead of F80 Taylor series cos()**: No change.
   This confirms the doc's earlier finding that cos precision is not the issue.

3. **Computing d/v as f32 division before promoting to f64/F80**: This actually
   broke other bodies that were passing, suggesting the current F80 division
   approach is correct.

### Key observations:

- The three failing bodies (4, 13, 21) all produce **exactly the same hashes**
  regardless of whether we use F80 or f64 arithmetic: `F0DD723B`, `F8B6F1F9`,
  `F45061CF` respectively.
  
- This hash stability across different floating-point approaches suggests the
  bug is **not** in floating-point precision differences.

- The def landing (land_seed=0) works perfectly for all bodies, including
  the three that fail on rand landings. This means the terrain generation
  code itself is fundamentally correct.

- The RNG sequences have been confirmed identical between C and Rust at all
  checkpoints, so the inputs to `round_hill` are the same.

### Working hypothesis:

Given that:
- Float precision (f64 vs F80) doesn't matter
- RNG is identical
- The same 33/99 hills execute in both C and Rust
- def landing works but certain rand landings fail

The bug may be in a **subtle interaction** between specific parameter values
that only occur with certain land_seed values. Possible candidates:

1. **Edge case in loop bounds**: Although unsigned comparison was confirmed,
   there might be an edge case when `cx == r` or `cz == r` (equality) that's
   handled differently.

2. **Signed zero or NaN handling**: The check `if (y >= 0)` might behave
   differently between C and Rust if `y` is -0.0 or NaN in edge cases.

3. **Unknown compiler behavior**: Borland C 3.1 might have a quirk in how
   it compiles the specific sequence of operations in `round_hill` that we
   haven't identified yet.

4. **Memory aliasing or undefined behavior in C**: The C code might rely on
   undefined behavior that coincidentally works but produces different results
   when reimplemented in well-defined Rust.

## Relevant files

| File | Purpose |
|------|---------|
| `source/NOCTIS-1.CPP:1494-1528` | C `round_hill` definition |
| `source/NOCTIS-1.CPP:1948-2148` | C `build_surface` + type-2 case |
| `source/NOCTIS-1.CPP:2610-2622` | C object-placement loop |
| `tests/nivgen/src/surface_sector.rs:132-171` | Rust `round_hill` |
| `tests/nivgen/src/surface_sector.rs:705-991` | Rust `build_surface` + type-2 case |
| `tests/nivgen/src/f80.rs` | F80 emulation (cos, sqrt, neg, abs, cmp, pi) |
| `tests/harness/NIVTEST.CPP` | C test harness |
| `tests/db/bulk_sectors.py` | Pipeline runner |
