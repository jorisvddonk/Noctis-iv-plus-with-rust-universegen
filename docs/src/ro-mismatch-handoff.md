# r-o* mismatch handoff: 36 remaining sector object chart diffs

**Date:** 2026-07-22  
**Status:** Root cause identified at high level, exact mechanism unconfirmed  
**Mismatch count:** 36/3063 bodies (98.8% match)  
**Pattern:** All `r-o*` = rand-landing, object-chart-only (heightmaps match byte-exact)

## Summary

The Rust port's sector heightmaps and object charts match C canonical for
3027/3063 bodies. 36 bodies remain where **only the object chart differs**
on **rand landings only** (DEF landings match). 34/36 are moons. All
planet types (0-7) are represented.

## Symptom

The object chart byte differences fall into two categories:

1. **Class field diffs** (bits 2-7): Rust sets VEGET/TREES where C keeps ROCKS.
   The classification loop (`p_surfacemap[ptr] / 25` → `case 0:` → set VEGET)
   fires in Rust but appears not to in C for specific pixel positions.
   
2. **`nr_of_objects` diffs** (bits 0-1): Near the end of the buffer (indices
   39800+). Cascading from class diffs that change the object chart bytes,
   which are then read back via `surf_h(ptr+200)` in the object placement loop.

## What was verified

### Confirmed matching
- **RNG sequence**: Identical between C and Rust through the entire
  `build_surface()` pipeline at multiple checkpoints (global_surface_seed
  reseed, land_seed reseed, pre-jitter, pre-darkline).
- **fast_random sequence**: Fixed by 16-bit wrapping in `land_seed`
  computation (`((lat as i16).wrapping_mul(lon as i16)) as i32 as u32`).
- **Heightmaps**: Byte-exact match between C and Rust for ALL 3063 bodies
  at both DEF and RAND landings.
- **Pre-classification heightmap**: Identical between C and Rust at the
  `addtrees:` entry point (verified via DOSBox trace + NIVDUMP_PREOC).
- **Gap bytes**: Per-landing gap handling fixed (`rust_star` reads gap from
  each landing's fixture file). DEF and RAND gaps differ for some bodies.
- **Asterism loop**: Iteration count matches C x87 for all densities [6,30]
  (verified via DOSBox measurement program).
- **liquid_water cleanup**: Fixed to check `liquid_water`, not `waswet`.
- **`round_hill` unsigned loop**: Confirmed correct (u32 wrapping_sub +
  unsigned comparison matches C's 16-bit int/unsigned mix).

### Traced but inconclusive
- **C classification loop trace** (DOSBox, TRACE4/TRACE5): At `addtrees:`,
  `p_surfacemap[ptr] / 25` evaluates to 0 for pixels at positions 64-76.
  `case 0:` should fire, setting `object1_class = VEGET (1)` and
  `object2_class = VEGET (1)`. But the final object chart shows ROCKS (0)
  at these positions.
- **Assembly output**: For a minimal test case, the bitfield write code is
  correct (`AND 0xCF, OR 0x10` then `AND 0x3F, OR 0x40`). Full assembly
  could not be generated ("Compiler table limit exceeded" with -S flag).

## The 16-bit `int` loop theory (most likely root cause)

The most promising lead involves C's 16-bit `int` for the `ptr` loop
variable in the classification loop:

```c
int ptr;
for (ptr = 0; ptr < oc_bytes; ptr++) {  // oc_bytes = 40000
    switch (p_surfacemap[ptr] / 25) { ... }
}
```

`int` is 16-bit in Borland C++ 3.1, so `ptr` ranges from 0 to 32767,
then wraps to -32768 (0x8000). The assembly comparison `cmp ax, -25536; jb`
uses **unsigned** comparison (`jb`). The full iteration count is:

1. **Pass 1:** ptr = 0..32767 (32K iterations, jl NOT taken, jne NOT taken,
   jb taken)
2. **Pass 2:** ptr = 32768..65535 (32K iterations, jl taken for negative
   ptr values, all written to valid objectschart indices via huge pointer
   arithmetic)
3. **Pass 3:** ptr wraps to 0..39999 (40K iterations, jl NOT taken,
   jne NOT taken, jb taken until ptr=40000 where jb exits)

**Total: ~105K iterations instead of 40K.** The last pass writes the
final values (same class decisions as pass 1). Rust's `usize` loop runs
exactly 40K.

### How this could cause the mismatch

The C loop's triple pass means each pixel's `p_surfacemap[ptr] / 25` is
evaluated 3 times. The third evaluation (pass 3) should give the same
result as the first. But there are two subtle effects:

1. **`p_surfacemap` aliasing?** If any code MODIFIES `p_surfacemap`
   between pass 1 and pass 3, the third evaluation might differ. But
   the classification loop itself doesn't modify `p_surfacemap`. However,
   `objectschart` and `p_surfacemap` are both `huge` far pointers.
   The Borland compiler with `-O -Z` might alias them? They're separate
   `farmalloc`'d allocations, so they shouldn't overlap.

2. **Compiler register pressure:** In a 3000-line function like
   `build_surface()`, the compiler may spill/reload registers between
   loop passes. If `p_surfacemap`'s far pointer (segment:offset) is
   reloaded from a stale location in pass 3, it could point to the
   wrong memory. This is a compiler bug hypothesis — hard to verify
   without the full -S output.

### Predictions if the theory is correct

- The 36 bodies are those where a specific pixel's `p_surfacemap` value
  differs between pass 1 and pass 3 in C (due to register spilling or
  aliasing), causing the classification to NOT set VEGET in pass 3,
  while Rust's single-pass always sets VEGET.
- Bodies where `case 0` fires for a different `p_surfacemap / 25` value
  might not be affected (if the compiler doesn't spill for those branches).
- Moon bias (34/36) might be because moons have specific `p_surfacemap`
  patterns from the far-heap tail.

## What to try next

### 1. Get the full `build_surface` assembly output

The full -S output couldn't be generated due to "table limit exceeded".
Try:
- Extract just the `addtrees:` section of `build_surface` into a
  standalone `.C` file that includes the same context (globals, same
  struct definitions, same nested switch depth).
- Compile with `-S` under DOSBox.
- Look for the `objectschart[ptr].object1_class = VEGET` bitfield write.
  Check if the AND mask is `207` (0xCF) or something else.
  Check if there are TWO separate reads from `p_surfacemap` (pass 1 vs pass 3).

### 2. Trace p_surfacemap[ptr] on each C loop pass

Add a trace inside the classification loop that dumps `ptr`, `p_surfacemap[ptr]`,
and `p_surfacemap[ptr] / 25` for specific pixel positions (e.g., 64-76).
Use `fopen("D:\\TRACE_N.TXT", "a")` with a counter to capture all 105K
iterations. Compare pass 1 vs pass 3 values.

### 3. Reproduce with a `usize`-style loop in Rust

The Rust code already uses `usize` (64-bit on modern systems). To match
C's buggy behavior:
```rust
let mut ptr: i16 = 0i16;
loop {
    match self.p_surfacemap[ptr as usize] as i32 / 25 {
        // ... same as current code ...
    }
    ptr = ptr.wrapping_add(1);
    if ptr as u16 >= 40000 { break; }
}
```
This simulates the 16-bit wrapping + unsigned comparison. If this matches
C's output, the 16-bit loop theory is confirmed.

### 4. Cross-reference the exact positions

For several r-o* bodies, dump the diff positions. If the positions
correlate with `p_surfacemap` values near 25 (the classification
boundary), it supports the aliasing/register theory. If the positions
are random, look elsewhere.

## Bodies affected

All 36 are in `tests/db/noctis.db`. Query:
```sql
SELECT s.name, p.body_index, p.planet_type, p.is_moon
FROM planets p JOIN stars s ON p.star_id = s.id
WHERE p.sect_rand_hm = p.rust_sect_rand_hm
  AND p.sect_rand_oc != p.rust_sect_rand_oc
  AND p.sect_def_hm = p.rust_sect_def_hm
  AND p.sect_def_oc = p.rust_sect_def_oc;
```

Keys: SADA'UKEIRIT body 63, EKA24 FERTILE CHILD bodies 52/59/67,
LOVE'S LABOURS WON body 79, RAVALISS body 37, BLUE CRUSADER body 3,
and 30 more.

## Files

- `source/NOCTIS-1.CPP` — reference C source (lines 2235-2294: addtrees)
- `tests/nivgen/src/surface_sector.rs` — Rust port
  - `oc_addtrees()` at line 1472 (classification loop)
  - `oc_revert()` at line 1458 (round_hill + addtrees)
  - `liquid_water` cleanup at line 1353
- `tests/db/bulk_sectors.py` — `rust_star()` at line 393
- `tests/results/sectors/*.out` — canonical fixtures
- `/tmp/NOCTIS-1_TRACE3.CPP` through `_TRACE6.CPP` — instrumented C copies
- `/tmp/eka52/` — DOSBox dumps for EKA24 FERTILE CHILD body 52
- `/tmp/ro_r37/` — DOSBox dumps for RAVALISS body 37
- `docs/src/asterism-investigation.md` — asterism loop verification
- `docs/src/sector-float-precision.md` — float precision investigation
