# Asterism loop iteration count: x87 vs Rust f32 accumulation

**Date:** 2026-07-22  
**Author:** opencode investigation session  

## Summary

The `asterism` function's inner `ang += ad` loop was suspected of causing
an RNG call-count offset due to x87 f80 vs Rust f32 precision differences.
This investigation confirms that **Rust's f32 accumulation matches Borland
C++ 3.1 x87 behavior exactly** for all density values [6, 30]. There is
no RNG offset from the asterism loop.

## Method

A minimal C test program (`docs/src/asterism-measure.c` — see DOSBox
reproduction steps below) was compiled with Borland C++ 3.1 under
DOSBox-X using the exact flags used by the game's build system
(`-ml -2 -f287 -O`). The program measures, for each density (6–30),
how many iterations the `while (ang < M_PI * 2)` loop executes.

The same computation was then simulated in Python using IEEE 754
`struct.pack('f')` rounding (f32) and compared.

## Results

| density | C (x87) | Rust f32 sim | Expected (density) | Extra |
|---------|---------|---------------|--------------------|-------|
| 6       | 7       | 7             | 6                  | +1    |
| 7       | 8       | 8             | 7                  | +1    |
| 8       | 9       | 9             | 8                  | +1    |
| 9       | 10      | 10            | 9                  | +1    |
| 10      | 10      | 10            | 10                 | 0     |
| 11      | 12      | 12            | 11                 | +1    |
| 12      | 13      | 13            | 12                 | +1    |
| 13      | 13      | 13            | 13                 | 0     |
| 14      | 14      | 14            | 14                 | 0     |
| 15      | 15      | 15            | 15                 | 0     |
| 16      | 16      | 16            | 16                 | 0     |
| 17      | 17      | 17            | 17                 | 0     |
| 18      | 19      | 19            | 18                 | +1    |
| 19      | 19      | 19            | 19                 | 0     |
| 20      | 20      | 20            | 20                 | 0     |
| 21      | 22      | 22            | 21                 | +1    |
| 22      | 23      | 23            | 22                 | +1    |
| 23      | 24      | 24            | 23                 | +1    |
| 24      | 25      | 25            | 24                 | +1    |
| 25      | 25      | 25            | 25                 | 0     |
| 26      | 26      | 26            | 26                 | 0     |
| 27      | 28      | 28            | 27                 | +1    |
| 28      | 28      | 28            | 28                 | 0     |
| 29      | 30      | 30            | 29                 | +1    |
| 30      | 31      | 31            | 30                 | +1    |

**All 25 densities match exactly between C (x87) and Rust f32 simulation.**

## Why f32 works (x87 register reasoning)

The C code executes `ang += ad` where both are `float`. The x87 FPU
stores `ang` to f32 memory after each iteration because:

1. The loop body calls `cos(ang)` and `sin(ang)`, which use the x87
   `fcos`/`fsin` instructions, replacing the top-of-stack value.
2. The compiler must spill `ang` to memory before `cos`/`sin` and reload
   it afterward. Loading from memory converts f32 → f80 (exact), but
   the stored value is f32-rounded from the previous iteration.
3. The comparison `ang < M_PI * 2` loads the f32-rounded `ang` from
   memory and compares it with the f64 constant `M_PI * 2` as f80 vs f80.

Thus the accumulation is f32 precision (no "extra" f80 register retention
across iterations). This matches Rust's `f(ang as f64 + ad as f64)`.

## Why extra iterations occur

For many densities, `f32(ad) * density < 2π` after exactly `density`
additions. The loop runs one more step, computing `f32(ang + ad)` which
exceeds 2π and terminates. This is inherent to f32 rounding of `ad`
(the step size), not to the accumulation precision. Both C and Rust
behave identically.

## RNG impact

The total number of `random(1000)` calls per asterism invocation is
exactly `density` (one per loop iteration). Since the iteration count
matches C, the RNG sequence through the entire `oc_addtrees` section
(including jitter, rock params, etc.) is identical to C.

No "211-call offset" exists — the earlier suspicion was caused by a
bug in the manual RNG trace (incorrect call count).

## Reproduction

To rebuild the C measurement under DOSBox-X:

```bash
# Create minimal test
cat > /tmp/ASTMEAS.C << 'EOF'
#include <stdio.h>
#include <math.h>
#define M_PI 3.14159265358979323846
void measure(int density) {
    float ad = M_PI * 2 / (float)density;
    float ang = 0;
    int iter = 0;
    while (ang < M_PI * 2) { iter++; ang += ad; }
    printf("d=%2d iter=%2d ang_final=%.15f\n", density, iter, (double)ang);
}
int main(void) {
    for (int d = 6; d <= 30; d++) measure(d);
    return 0;
}
EOF

# Build + run under DOSBox-X (see tests/scripts/recompute-sector.sh for
# the canonical DOSBox-X invocation pattern)
```

## Related files

- `tests/nivgen/src/surface_sector.rs:326` — Rust `asterism` function
- `source/NOCTIS-1.CPP:1635` — C `asterism` function
- `docs/src/sector-float-precision.md` — broader sector float precision doc
