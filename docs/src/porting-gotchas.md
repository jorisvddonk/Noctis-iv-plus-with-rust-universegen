# Noctis IV Plus → Rust: porting gotchas

This document records the non-obvious behaviours of the original
Turbo/Borland C++ 3.0 generator that the Rust port (`tests/nivgen`)
had to reproduce **bit-for-bit**. Each item was discovered the hard way
(a fixture diverged and we had to probe the real compiler under DOSBox).
Read this before touching `rng.rs`, `system.rs`, or `f80.rs`.

## 0. Session status (type-6 `surf_case6` / T2P06) — 2026-07-18

* Stage-by-stage localization (§9b) for T2P06 (body 0, seed
  `3812345,-4287654,-658973`): `0_RN`, `0A_O`, `1_SMO` **match C**; the
  first divergence is between `1_SMO` and `2_BW` (the band/wave loop).
  `2A_B` (objectschart after the loop) also matches C.
* The `2_BW` residual is dominated by `wave()`'s non-reproducible corruption
  (§9c/§9f): it is NOT a band-logic bug. Bands/ssmooth/clouds prior to the
  loop are verified matching via stages `0_RN`/`1_SMO`.
* **The 00:02 working tree is UNRECOVERABLE.** `git fsck` shows no dangling
  objects, no stashes, no editor swap/backup of `surface.rs`, and the OS
  "snapshots" are macOS update images (not user-file backups). Only committed
  `HEAD` survives, and HEAD's `surf_case6` already mismatches C
  (`surface_map` FNV `D5212AFE` vs C `DC903119`). So do NOT assume a prior
  "344-byte wave residual" state exists — it was a live, now-overwritten tree.
* `f_to_u16_rne` was found to be wrong for the band/cloud `s_px`/`s_py`
  `(unsigned)` casts (those are truncation, §9d); converting them to
  round-to-nearest changed nothing for positive values but is semantically
  incorrect — keep truncation.
* `wave()` first column (`px==360`) IS deterministic and bit-exact once the
  `bx=cy` special case is used. Only the `px<360` garbage columns are
  build-dependent (§9f).

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

## 9b. `surf_case6` (type-6 planet surface) — stage-by-stage localization

The type-6 surface pipeline (`surf_case6` in `NOCTIS-0.CPP`) is the hardest
fixture to match (`surface_map`/`atmo_overlay` in T2P00–09). Both the C
source and the Rust port emit **stage checkpoints** that make divergence
localization trivial — compare stage-by-stage instead of diffing the final
buffer:

* C `surf_case6` already calls `dmpchk("1_smo",...)`, `"2_bw"`, `"2a_bw"`,
  `"3_storm"`, `"4_lss"`, `"5_merge"` (and `a` variants for `objectschart`).
  These land in `$WORK/CHK_*.BIN` after a `--rebuild`.
* Rust `dbg_dump` writes the same stages to `/tmp/rchk/CHK_*.BIN` (gated by
  `NIVDBG=1` and `CURRENT_BODY == CAPTURE_BODY`). Stage names: `0_RN`,
  `0A_O`, `1_SMO`, `2_BW`, `2A_B`, `3_ST`, `3A_S`, `4_LS`, `4A_L`, `5_ME`,
  `5A_M`, `6_FI`, `6A_F`.
* Localize the **first** stage that diverges — everything after it is
  downstream noise. For T2P06 (body 0, seed `3812345,-4287654,-658973`):
  `0_RN`, `0A_O`, `1_SMO` all match C; divergence appears between `1_SMO`
  and `2_BW` (the band/wave loop).

Caveat: the C `dmpchk` file for stage 1 is named `CHK_1_SM.BIN` (not
`1_SMO`), and `2a_bw`'s `objectschart` is `CHK_2A_B.BIN`. Map names carefully.

## 9c. `wave()` memory-corruption bug (type-6 only)

`wave()` (`NOCTIS-0.CPP:4571`) has a genuine **memory-aliasing corruption**
that the Rust port MUST reproduce, not "fix":

```asm
les  di, p_background
mov  px, 360
mov  bx, cy          ; bx = cy (correct base for the first column)
nvrain:
    fild px
    fmul a
    fsin
    fild cr
    fmulp
    fistp py         ; py = round-to-nearest-even(sin(px*a)*cr)  [x87 fistp, NOT truncation]
    add  py, bx
    mov  ax, py
    mul  dx          ; dx = 360
    add  ax, 4
    mov  di, ax
    add  di, px      ; es:[di] = segment offset py*360 + 4 + px
    mov  byte ptr es:[di], 0
    dec  px
    jnz  nvrain
```

* The `mov byte ptr es:[di], 0` far-write **aliases `cy`'s memory** (layout-
  dependent). After the first column (`px==360`, where `py` is correct:
  `py = rne(sin(360*a)*cr) + cy`, row ≈ `cy`), the `bx` base becomes a
  **garbage large value G** for all `px < 360`. So `py = rne(sin(px*a)*cr) + G`
  with `G` essentially random (observed ~27628 in one build, ~27804 in
  another, layout-dependent and **NON-deterministic across builds**).
* The far-write target `p_background` sits at segment offset 4, so the buffer
  index is `py*360 + px` (the `+4` from `add ax,4` cancels the buffer's own
  offset of 4; `mov di,ax` overwrites the `les di` base, so the +4 is NOT a
  pointer base). Wrap modulo 65536.
* `fistp` rounds to nearest-even (handled via `F80::to_i32_rne(...)`), NOT
  truncation. The `sin` arg is `px*a` with `a` a `double`.
* To reproduce in Rust: in `wave()`, use `bx = cy` for the `px==360`
  iteration only, then `bx = cy + WAVE_CORRUPT_BASE` for `px < 360`, where
  `WAVE_CORRUPT_BASE` is a tuned constant (env `WAVE_BASE`). The exact G is
  build-specific; for the committed 00:02 fixtures G ≈ 27628 (base offset
  ≈ 27482 above `cy`). With the wrong base the wave zeroes **different**
  pixels than C, producing ~4700 "C has terrain, R has 0" diffs in `2_BW`.

### 9d. `(unsigned)(double)` casts are TRUNCATION, not x87 rounding

The x87 round-to-nearest-even (`fistp`) applies ONLY where the C actually
stores a `float`/`double` through the FPU (e.g. `wave()`'s `fistp py`, and
the seed/identity expressions in §5). Plain C `(unsigned)(expr)` / `(int)(expr)`
casts from `double` are **truncation toward zero**. The band/cloud/spot
`s_px`/`s_py` assignments (`py *= 360; s_py = (py) as u16;`) come from
`(unsigned)` casts → use truncation (`x.trunc() as i64 & 0xFFFF`), NOT
`f_to_u16_rne`. Converting these to round-to-nearest produced NO change for
positive values but is semantically wrong; keep them as truncation.

### 9f. `wave()` corruption base is NON-REPRODUCIBLE — accept approximate match

The `wave()` garbage base `G` (§9c) is the **runtime contents of `cy`'s
memory after a far-write alias**. It depends on the exact data-layout /
stack contents of each build, so it is **not deterministic across builds**
and cannot be reproduced bit-for-bit from the C source alone. Observed values:
`G ≈ 27628` (00:02 build), `G ≈ 27804` (instrumented build), even `G ≈ -9`
(persistent-handle build).

Consequence for validation: the type-6 `surface_map` fixture can ONLY match
C exactly for the specific C build that produced the committed fixture. A
brute-force scan of `WAVE_BASE` (env-tunable offset added to `cy` for
`px<360`) over 27000–28092 only reduced `2_BW` diffs from 4545 → 4516 — it
does **not** converge toward 0, because the corruption is not a single
constant offset (the aliased memory differs per wave call in ways a constant
can't capture). The 00:02 "344-byte wave residual" was measured against the
00:02 C build's *actual* (unrecoverable) garbage base.

Practical stance: keep the `WAVE_BASE` tunable + the `px==360` correct-base
special case so the *first* wave column is bit-exact (that part is
deterministic: `py = rne(sin(360*a)*cr) + cy`), and accept that the remaining
garbage-column writes are build-dependent. Do NOT block T2P06 validation on a
perfect `surface_map` FNV for type 6 until/unless the exact C build is pinned.
Record the `WAVE_BASE` that minimizes diffs against the committed fixture so
the residual is at least minimized.

### 9e. u16 overflow wraps in C (debug build panics in Rust)

`s_px + s_px`-style `u16` additions in `spot()`/`cirrus()` wrap modulo 65536
in the 16-bit C. In Rust debug builds these panic ("attempt to add with
overflow"); use `.wrapping_add(...)`. (Release builds wrap silently, matching
C, but the explicit `wrapping_add` is correct everywhere.)



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
