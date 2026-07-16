# Noctis IV Plus → Rust: porting gotchas

This document records the non-obvious behaviours of the original
Turbo/Borland C++ 3.0 generator that the Rust port (`tests/nivgen`)
had to reproduce **bit-for-bit**. Each item was discovered the hard way
(a fixture diverged and we had to probe the real compiler under DOSBox).
Read this before touching `rng.rs`, `system.rs`, or `f80.rs`.

## 1. `srand` takes a **16-bit** `unsigned int` 🔴 (the big one)

Borland is a 16-bit compiler, so `srand`'s parameter is a 16-bit
`unsigned int`. The seed is **truncated to its low 16 bits** on the way in.
A 32-bit `long` passed to `srand` loses its top half.

```c
srand((long)seed_d);   /* seed_d = -7863.954 -> (long) = -7863 = 0xFFFFE149 */
                       /* srand stores only 0x0000E149 = 5737            */
```

Symptom: small validated seeds (1, 7665, 8556) all fit in 16 bits so
they *hide the bug*; only large seeds (the real star coordinates) break,
flipping the whole RNG sequence and the star class.

Fix: `BorlandRng::srand` does `self.seed = seed & 0xFFFF;`. **All**
`srand(...)` call sites in the C source (`extract_ap_target_infos`,
`prepare_nearstar`, `fast_srand` uses) are covered by this one mask.

## 2. Borland `rand()` is the classic LCG — **no 31-bit mask**

```c
/* Borland RTL */
long _randseed;
int rand(void) {
    _randseed = _randseed * 0x15A4E35L + 1;   /* 32-bit, may go negative */
    return (_randseed >> 16) & 0x7FFF;            /* NO & 0x7FFFFFFF mask */
}
```

`_randseed` is a 32-bit **signed** `long`. The multiply may overflow into
the sign bit; do **not** mask to 31 bits first. Use wrapping 32-bit
signed arithmetic, then `(seed >> 16) & 0x7FFF`.

Validated sequence (use as a regression check): `srand(1)` →
`346, 130, 10982, 1090, 11656, 7117, …`.

## 3. Borland `random(n)` = `(long)rand() * n / 32768`, **signed**

From `<dos.h>`:
`#define random(num) (int)(((long)rand()*(num))/(RAND_MAX+1))`
with `RAND_MAX == 0x7FFF`, so `rand()*n/32768` computed in **signed**
`long`, result cast to 16-bit `int`.

Consequences that MUST be reproduced exactly:
* It is **not** `rand() % n` (no modulo bias).
* `rand()` is consumed for **every** call, including `n <= 0`.
* For `n == 0` → result `0`. For `n < 0` the signed product is
  non-positive, so the result is **negative** (e.g. `random(-32445)`
  can return a negative value). The 16-bit `int` cast then wraps.
* Expressions that feed `random()` are 16-bit `int` arithmetic in Borland,
  so they can **overflow**: `10 * nearstar_p_orb_seed[n]` where
  `10*3309 = 33091` overflows `int` to `-32445`. Reproduce with a
  modulo-65536 wrap (Rust's `as i16` **saturates** — do NOT use it;
  use an explicit `& 0xFFFF` wrap into `i32`).

Helper used in Rust:
```rust
fn i16(x: f64) -> i32 {
    let t = (x as i32) & 0xFFFF;
    if t >= 32768 { t - 65536 } else { t }
}
```
Apply `i16(...)` to every `int` argument of `random()`/`zrandom()`
(`10*orb_seed`, `300*ray`, `300*p_ray`, `orb_seed + 10*|orb_tilt|`,
`CLASS_RAYVAR[class]`, …).

## 4. `zrandom(range)` = `random(range) - random(range)` (float)

Straightforward, but note it consumes **two** `rand()`s.

## 5. Float math is **x87 80-bit extended** (`-f287`)

With `-f287`, all `float`/`double` arithmetic is performed in 80-bit
extended precision (64-bit mantissa) on the FPU. For the star **seed
expression** and `nearstar_identity`:

```c
srand(ap_target_x/100000 * ap_target_y/100000 * ap_target_z/100000);
nearstar_identity = nearstar_x/100000 * nearstar_y/100000 * nearstar_z/100000;
```

the 80-bit accumulation differs from naive IEEE `f64` by enough at
large coordinates to flip the integer truncation of `srand`. Rust has no
native `long double` on macOS (SSE2 `f64`), so `tests/nivgen/src/f80.rs`
implements a software x87 `long double` (64-bit mantissa, round-to-nearest-
even) and the seed/identity expressions are evaluated through it.

Key invariants when touching `f80.rs`:
* Stored mantissa has its integer bit at **bit 63**; the value is
  `mantissa × 2^(exp - BIAS - 63)`, i.e. `to_f64` treats the top 53
  bits as the `1.x` significand and uses exponent `exp - BIAS`.
* `mul` exponent = `self.exp + other.exp - BIAS + 1` (before normalising).
* `div`  exponent = `self.exp - other.exp + BIAS`.
* Both `mul` and `div` round the dropped low bits to nearest-even (x87
  semantics) — **truncation here causes fixture divergence**.
* The final `(long) seed_d` truncation happens on the **64-bit double**
  (after `f80::to_f64`), then the 16-bit `srand` mask (#1) applies.

## 6. `nearstar_ray` is a C `float`

`prepare_nearstar` stores `nearstar_ray` as a `float`. The body
`orb_ray` math multiplies it, so keep `nearstar_ray: f32` in Rust and
cast to `f64` only inside the arithmetic — otherwise ULP drift shows up in
the bodies. (This was the T1C00-era fix; bodies matched with `f64`
alone only because the seed was already wrong-by-coincidence for small
coordinates.)

## 7. Output is **CRLF**, and `-o` truncates

The emitter writes `"\r\n"` (fixtures are CRLF). The `-o <file>` mode
**truncates** the target file (it was accidentally append at one point and
that corrupted multi-system dumps).

## 8. `int` is 16-bit everywhere in the C source

Any computation written as `int` in the original is 16-bit and wraps
modulo 65536. When porting arithmetic that "looks like it should be
32-bit", check whether the C type is `int` (→ wrap) or `long` (→ keep
32-bit). Hand-rolled 32-bit self-multipliers (the `fast_* ` RNG) use
`0x66`-prefixed 32-bit opcodes inside 16-bit codegen and are unaffected
by the `-2` (80286) target.

## 9. How to verify against the real compiler

The ground truth is the **unmodified C source** built with the TC 3.0
toolchain under DOSBox (driven by `tests/scripts/run-niv-tests.sh
--rebuild`). Fixtures in `tests/results/*.OUT` are CRLF and committed as
the reference. To probe a specific behaviour of the real RTL, drop a tiny
`probe.c` next to `tests/dosbox/`, link it with the working
`BUILD.BAT`/`LINK.RSP` (must include `FP87.LIB MATHL.LIB CL.LIB` or you
get `F_FTOL@`/`FIDRQQ` undefined-symbol link errors), and have it
`printf(...)` to **stdout redirected to a file** via the dosbox
`autoexec` (`probe.exe > D:\OUT.TXT`); the bind-mounted work dir then
surfaces `OUT.TXT` on the macOS side. Declarations must precede
statements (C89), and `void main` is fine.
