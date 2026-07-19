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
* **T2P00–09 sweep result (2026-07-18):** ALL ten `planet` fixtures are the
  same type-6 body (body 0) for the same coords — the harness `cmd_planet`
  ignores `-n`, so T2P01/03/04/07/08/09 are byte-identical inputs to T2P00.
  Only T2P02 uses different coords (`-4224317 -468962`). Both coord sets are
  type 6, so **every T2P fixture exercises `wave()`** — there is NO fixture
  that validates the clean (non-wave) `surf_case6` stages. Hence T2P cannot
  confirm the bands/ssmooth/storm paths in isolation; those were validated via
  the stage dumps (`0_RN`/`1_SMO`/`2A_B` match C) instead.
* The C fixtures are themselves **non-deterministic run-to-run**: for the same
  coords, T2P00/03/05/06 give `surface_map=DC903119` but T2P01=`AA64B3EB`,
  T2P04=`AF74278D`, T2P07/08=`10989DFF`, T2P09=`325D07EE`. This is the `wave()`
  garbage-base bug surfacing — proof it cannot be reproduced bit-for-bit.
* `atmo_overlay` divergence (`3A_S` = 17805 diffs) is a **downstream effect of
  the wave base mismatch**, not an independent storm bug: `storm()` → `cirrus()`
  reads `p_background` (which `wave()` corrupted differently), so a wrong wave
  base poisons the overlay too. `2A_B` (objectschart right after the band/wave
  loop) matches C (0 diffs), confirming the storm *logic* is fine pre-wave.
* Net: Rust type-6 output is fully **deterministic** (always `D553C614` for
  coords A, `9D6323C1` for coords B) and internally consistent, but cannot
  match the non-deterministic C `surface_map`/`atmo_overlay` FNVs without
  pinning the exact C build. Do NOT block T2P validation on type-6 FNV.

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

## 10. `crater()` bugs (2026-07-18) — four divergences in a single function

The crater function (`NOCTIS-0.CPP:4514`, ~46 lines of mixed C and inline asm)
had four independent porting errors, all of which initially produced bit-exact
output for the *first* crater but desynced for later craters (or for craters
with small `cx` that trigger negative `px`). Finding them required stage-by-stage
dumps (`C1_SM`, `C1_C0`, `C1_CR`, `C1_LS`) and per-write CRAT tracing.

### 10a. Crays `random(crays)` runs ONCE PER ANGLE, not per `gr`

In C, the crays block (`crays && !random(crays)`) is **outside** the
`for (gr=0; gr<cr; gr++)` loop — it runs once per angle after the gr
loop completes. The Rust port had placed it **inside** the per-`gr` loop,
calling `random(crays)` `cr` times per angle instead of once. That consumed
`cr-1` extra RNG calls per angle (90 × `cr-1` per crater), desyncing the
Borland RNG for all subsequent random calls.

Symptom: `CRRC` trace (first 400 `random(4)` values) diverged from C at
index 90 (crater 1, first angle), while crater 0's `cr`/`cx`/`cy` params
all matched.

### 10b. Second asm block (`0x013E` write) runs ONCE PER ANGLE, not per `gr`

The second asm block (`mov ax,0x013E; mov es:[di],ax`) writes `0x3E` at
`di` and `0x01` at `di+1` using the LAST `vptr` from the gr loop. In C it
is after the `for gr` loop, inside the `for a` loop. The Rust port had it
inside the per-`gr` loop, so it wrote `0x3E`/`0x01` at EVERY `di` (per
gr) instead of only once per angle. This polluted the surface map with
extra `0x3E` bytes.

Symptom: `C1_C0.BIN` first diff at byte 21693 (py=60, px=93), where
Rust had consecutive `62,62,1,62` (=`0x3E,0x3E,0x01,0x3E`) but C had
`0,16,22,32`.

### 10c. Borrow check: `JNC` tests CARRY, not SIGN

C's first asm block does `sub al,ah ; jnc entro ; xor al,al`. The `JNC`
instruction is "jump if no CARRY", i.e. it tests the unsigned borrow
flag — equivalent to the C expression `al >= ah` (unsigned). The Rust
port incorrectly used `al as i8 >= 0`, which tests the **sign** flag.
For values like `al=200, ah=50`: `200 - 50 = 150`, no borrow, so C keeps
`150`. But `150 as i8 = -106 < 0`, so Rust set it to `0`.

Symptom: crater pixels with `al ≥ 128 - ah` and `al ≥ ah` fell through
to `0` in Rust but kept their attenuated value in C. These appeared in the
`C1_C0` diff as Rust `0x3E` where C had intermediate values.

Fix: compare the **original** byte against `ah` *before* the subtraction:
`if al0 >= ah { al0 - ah } else { 0 }`.

### 10d. `unsigned px, py` are 16-bit — negative values must wrap mod 2^16

The global `unsigned px, py` (line 4446 in NOCTIS-0.CPP) are **16-bit**
`unsigned` (Turbo C++ 3.0). Assignments like `px = cx + cos(a)*gr` compute
a `double`, truncate it toward zero (C assignment), then store it in a
16-bit `unsigned`, i.e. wrap modulo 65536. For craters with small `cx`,
the cosine term can make `px` negative, e.g. `px = 12 + (-0.6)*53 ≈ -20`.
C gives `(unsigned)(-20) = 65516` (16-bit), while a naive 32-bit
truncation gives `0xFFFFFFEC = 4294967275`. The difference in `vptr =
px + 360*py` (mod 65536) results in an off-by-one `di`, causing the `r2`
crays sub-block writes to land at adjacent-but-wrong pixels.

Symptom: craters 0–22 matched byte-for-bit; crater 23 (cx=12, the first
crater small enough to produce a negative px) diverged at byte 25539.

Fix: added `f64_to_u16_trunc` that truncates toward zero and masks to 16
bits (`((x.trunc() as i32) & 0xFFFF) as u32`). Use it for **all** px/py
computations where the C variable is `unsigned` (16-bit): `crater`,
`permanent_storm`, `spot`, `volcano`, `atm_cyclon`, `storm`. Keep
`f64_to_u32_trunc` for the `srand`/`fast_srand` seeds which need 32-bit
truncation.

### 10e. `contrast()` uses x87 80-bit float — must use `F80`

The `contrast()` function (NOCTIS-0.CPP:4633) computes `(pixel - thr) * k + thr`
in 80-bit x87 extended precision. A plain f64 chain diverges from x87
rounding by ~1-2 LSB on some pixels, producing a surface_map mismatch.

Fix: compute `contrast`'s inner loop using the `F80` module (software
x87 `long double` emulation — `add`/`sub`/`mul` in 80-bit with
round-to-nearest-even).

### Status (2026-07-19)

* Bodies 0 (type 8), 2 (type 4), 3 (type 1), 4 (type 1), 10 (type 1), 13 (type 2):
  **byte-for-bit identical** (6/10).
* Body 1 (type 5): surface ~2700 diffs (F80 contrast rounding ≤2 LSB), atmo+palette OK.
* Bodies 5, 8 (type 5): surface + palette diverge (color computation issue, TBD).
* Body 9 (type 7): surface ~2 diffs (fracture x87 sin drift), atmo+palette OK.

---

## 11. BCPP 3.1 toolchain (2026-07-19)

The actual compiler used by Noctis IV is **Borland C++ 3.1** (not Turbo C++ 3.0).
The toolchain is available at `https://archive.org/download/bcpp31/BCPP31.ZIP`.

* BCPP 3.1 uses `BCC.EXE` instead of `TCC.EXE`, but library names (`C0L.OBJ`,
  `FP87.LIB`, `MATHL.LIB`, `CL.LIB`) are identical.
* `BUILD.BAT` auto-detects `BCC.EXE` or `TCC.EXE`.
* `run-niv-tests.sh` auto-detects toolchain layout (BCPP31/ or TC/ subdirectory).
* BCPP 3.1 produces **identical output** to TC 3.0 for all tested planet types
  (1, 2, 4, 5, 7, 8). The `wave()` garbage base differs between builds as
  documented in §9c/§9f.
* Use `run-niv-tests.sh --local --toolchain niv-toolchain/BCPP31` for native
  DOSBox-X builds on macOS.

## 12. `psmooth_grays` — packed 4-row dword average (2026-07-19)

C's `psmooth_grays` (NOCTIS-0.CPP:480) uses the **same packed 32-bit dword
technique as `ssmooth`**, not a simple 2×2 block average:

```asm
mov dx, [di-320]    ; row -1  (4 pixels packed in dword)
add dx, [di]        ; row  0
add dx, [di+320]    ; row +1
add dx, [di+640]    ; row +2
and edx, 0xFCFCFCFC ; mask low 2 bits
shr dx, 2           ; average of 4 rows
mov al, dl          ; sum 4 column bytes
add al, dh
shr dx, 16
add al, dl
add al, dh
shr al, 2           ; average of 4 columns
mov [di], al
```

The Rust port originally did a plain 2×2 block average over 360×180 pixels.
This produced completely different output (body 13 diverged).

Fix: model after `ssmooth` — use raw pointer reads for 4-row dword sums,
mask with `0xFCFCFCFC`, shift right by 2, then sum the 4 resulting column
bytes and average into one output byte per position.

## 13. F80 `add` bugs (2026-07-19)

Two bugs found in the software x87 `long double` addition:

### 13a. Exponent alignment when `self.exp < other.exp`

When the first operand has a smaller exponent, its mantissa is right-shifted
to align. The base exponent for the result must be the **larger** exponent
(`other.exp`), not the smaller one (`self.exp`). The subtraction path
recovered via `ea.max(eb)`, but the same-sign addition path used `ea`
directly.

Fix: `ea = ob.exp` (line 171 in `f80.rs`).

### 13b. Integer-bit normalisation check: `s>>63 == 1` misses carry

When two equal-magnitude mantissas add, the sum may have **2 integer bits**
(`s>>63 == 2`). The check `s>>63 == 1` missed this case and fell into the
shift-left normalization branch, which shifted the integer bit past bit 128
and lost it.

Fix: `(s>>63) != 0` (any integer bit present), then handle 1-bit and 2-bit
cases.

Both bugs made `add` return garbage for many inputs (25+25=2, etc.),
which manifested as `contrast()` producing ~26000 byte diffs vs C.
After fix the diffs drop to ~2700 (pure F80-vs-x87 rounding residuals).

## 14. `fracture` precision bugs (2026-07-19)

### 14a. Initial angle computed in f32, not f64

C fracture initializes `a = random(360) * deg` where `a` is `double` and
`deg` is `const double = M_PI/180.0`. The Rust port computed `random(360)
as f32 * DEG as f32` then upcast to f64, losing ~7 mantissa bits. This
drifted the cos/sin accumulation across iterations and produced 29 byte
diffs in body 2 (type 4).

Fix: `self.s_a = self.rng.random(360) as f64 * DEG;` (compute in f64).

### 14b. `(unsigned)py` 16-bit cast

C fracture casts `py` to `(unsigned)` (16-bit) before multiplying by 360.
Rust used `py as i32` (32-bit). For negative py values the wrapping differs:
C wraps mod 65536 while Rust wraps mod 4294967296. Body 2 didn't hit
negative py, but the fix is needed for other bodies.

Fix: use `f64_to_u16_trunc(py)`.

## 15. `shade` F80 gradient accumulation (2026-07-19)

The `shade()` palette gradient function accumulates `delta_r`/`delta_g`/`delta_b`
over 64 iterations. x87 accumulates in 80-bit extended precision; f64 alone
drifts by enough to flip occasional u8 truncations in the 64-color ramp.

Fix: compute deltas and accumulate `sr`/`sg`/`sb` using `F80`, converting
to `f64` only for the final `as u8` truncation (which is a C `(unsigned char)`
cast = truncation toward zero, matched by Rust's `as u8`).

## 16. Terminator (darkside) 16-bit `di` wrap (2026-07-19)

The darkside terminator loop uses `di` as a **16-bit register** in C.
`inc di` wraps at 0x10000. Rust used `usize` (64-bit), which skips writes
beyond array bounds that C would wrap to low offsets.

Fix: use `u16` with `wrapping_add` for the `di` accumulator.

## 17. `imul ax` replaces AX with the low word of the product (2026-07-19)

This is the single most important x86 quirk to internalise. When the asm does:

```asm
add  ax, cx        ; ax = ax + cx
imul ax            ; dx:ax = (int16_t)ax * (int16_t)ax
add  ax, dx        ; ax = ax + dx
```

the `imul ax` instruction **replaces AX with the low 16 bits of the signed
32-bit product**. Then `add ax, dx` adds the high word (DX) to that new AX.

The naive Rust translation:
```rust
ax = ax.wrapping_add(cx);      // ax = ax + cx
let prod = (ax as i16 as i32) * (ax as i16 as i32);
let dx = (prod >> 16) & 0xFFFF;
ax = ax.wrapping_add(dx);      // ❌ adds dx to (ax+cx), NOT to prod_low!
```

is **wrong** — it misses the step where the low word of `prod` replaces
`ax`. The correct translation:

```rust
ax = ax.wrapping_add(cx);
let prod = (ax as i16 as i32) * (ax as i16 as i32);
let dx = (prod >> 16) & 0xFFFF;
let ax_low = (prod & 0xFFFF) as u16;
ax = ax_low.wrapping_add(dx);   // ✅ ax = prod_low + prod_high
```

This bug caused an entirely wrong felisian surface (`surf_case3`, body 3
type 3 — Felysia) despite the seed and threshold matching C. Every
`imul ax` → `add ax, dx` pattern in the C source must follow this
three-step structure: add→imul(replace AX)→add dx.

Also found in the same `surf_case3` loop:
- `add ax, cx` uses the **loop counter** (decrementing 64000→1), not a
  constant 64000.
- The clamp writes `mov word ptr [di], 0x003E` which zeroes the **next**
  byte (di+1) — a C bug that must be reproduced.

## 18. `f64_to_u32_trunc`: truncate as i64, not `rem_euclid` (2026-07-19)

C's `(long)(double)` for negative values truncates toward zero first
(removes the fractional part), THEN stores the integer in a 32-bit
`long`. The naive `rem_euclid(2^32)` wraps the **fractional** part:

```rust
// ❌ WRONG: rem_euclid wraps the fractional part
fn f64_to_u32_trunc(x: f64) -> u32 {
    (x.rem_euclid(4294967296.0)) as u32
}
```

For `x = -578621324.65`:
- `rem_euclid`: `2^32 - 578621325 = 3716345971` (off by 1 — wraps fraction)
- C `(long)`: `-578621324` as u32 = `3716345972` (truncates first)

This only matters for bodies with **negative `seedval`** (which is common —
many star coordinates produce negative seedvals). The error cascades:
wrong frng seed → different `fast_random(0xFFFF)` → different Borland
`srand` seed → different palette random values → palette diverges.

The fix: truncate as `i64` first, then cast to `u32`:

```rust
fn f64_to_u32_trunc(x: f64) -> u32 {
    (x.trunc() as i64) as u32
}
```

This handles both positive and negative correctly:
- Positive `713100430.0`: truncate → `713100430_i64` → `713100430_u32` ✓
- Negative `-578621324.65`: truncate → `-578621324_i64` → `3716345972_u32` ✓

## 19. Moon surface OOB read is nondeterministic — hash only sampled rows (2026-07-19)

**Every moon (types 1, 4, 8, …) appeared to diverge while the identical
planet type passed.** The apparent bug was real but *cosmetically
irrelevant*, and the correct fix is to **hash only the rows the game
actually renders**, not the full 64800-byte buffer.

### The out-of-bounds read

In `NOCTIS-D.H` the two surface buffers are `farmalloc`d at different sizes
(`p_background` = `pl_bytes` = 65552; `s_background` = `st_bytes` = 64800),
and are allocated **adjacently** (`s_background` first, then `p_background`
— see NOCTIS.CPP:2142-2143 and the harness `alloc_buffers`). Planets render
into `p_background`, moons into `s_background` (the game repoints
`p_background = s_background` for a moon, NOCTIS-0.CPP:5326).

`lssmooth` (NOCTIS-0.CPP:4416) walks `di = 0..64479` reading a WORD at
`es:[di+360]` — up to byte 64840, i.e. 40 bytes past the 64800-byte map.
Only iterations `di = 64440..64479` read OOB, and they write to
`di = 64440..64479` = **row 179 (south pole), cols 0-39**. Repeated smooth
passes bleed this up to ~row 170. For a moon this OOB read pulls bytes from
the adjacent `p_background` allocation (by design — from a moon you can see
its parent planet, so the two maps are kept separate; NOCTIS-0.CPP:4462).
That content depends on player trajectory / render order, so it is **not
deterministic**.

### Why it does not matter: the globe never samples those rows

The sphere renderer `globe()` (NOCTIS-0.CPP:3044) does **not** sample the
map linearly. It walks it via the precomputed `GLOBES.MAP` offset stream.
Parsing that stream shows the maximum surface byte offset read is ~43562 =
**row 121**, and pixels are only ever sampled from **rows 0-119** (both
polar caps included — they are *not* at the physical top/bottom of the
buffer). Rows 122-179 are pure scratch space that is never displayed.

The OOB nondeterminism lands at rows 170-179 — ~50 rows below anything the
game can render. So it only ever perturbed the *full-buffer hash*, never a
visible pixel.

### The fix: hash rows 0-127 only

Both the C harness and the Rust port now hash only the first **46080 bytes
(rows 0-127)** of the surface map:

```rust
const SAMPLED_BYTES: usize = 360 * 128; // 46080
```

Row 127 sits comfortably above the worst-case rotation reach (row 121) and
well below the OOB smear (row 170+), so the hashed region is fully
deterministic. Consequences:

- Single-body (`planet -p`), sequential (`planet-all`), and in-game
  rendering all produce **identical** hashes for rows 0-127 — the execution
  mode / heap state no longer matters. Verified: FENIA body 16 moon =
  `78837023` from both single-body and planet-all.
- No need to model the far-heap tail at all. `s_background`/`p_background`
  are still sized to a full 64 KB segment (`BG_SEGMENT = 65536`) purely so
  the `.len()` bounds checks in `surface.rs` reproduce C's 16-bit `di`
  addressing (`[0, 0xFFFF]`); the tail is left zero and never hashed.

After switching to sampled-row hashing and recomputing every canonical hash
from BCPP 3.1 / DOSBox-X (`planet-all` per star, one boot each, via
`tests/scripts/recompute-canonical.sh` + `tests/db/load_canonical.py`), the
DB match rate rose from 39.8% to **71.2%**, and moons now match at the same
rate as their planet types (types 1, 2, 8, 9 = 100% for both planets and
moons).

**Lesson:** when the reference buffer is larger than what the algorithm
consumes, only fingerprint the consumed region. Hashing scratch/pad bytes
turns a deterministic port into a false failure.

## 20. Borland evaluates function arguments RIGHT-TO-LEFT (2026-07-19)

Borland C++ 3.1 pushes call arguments onto the stack **right-to-left**, and
it evaluates side-effecting subexpressions in that same order. When several
arguments each call the RNG, the *last* argument consumes the *first* random
value. Porting the calls left-to-right desyncs the entire RNG stream from
that point on.

Type 5 (`"small, solid, thin atmosphere."`) had two such calls, and both
were the sole reason the type diverged:

```c
/* contrast(kt, kq, thrshld): the three ranged_fast_random calls fire
   in the order thrshld -> kq -> kt */
contrast ((float)ranged_fast_random(200)/900 + 0.6,
          (float)ranged_fast_random(350)/100 + 4.0,
          25 + ranged_fast_random(3));

/* randoface(range, upon): the `upon` (2nd) ranged() fires before `range` */
randoface (5 + ranged_fast_random(3), -20 * (ranged_fast_random(3) + 1));
```

Correct Rust: pull the RNG values in **reverse argument order**, then pass
them positionally.

```rust
let c3 = (25 + self.frng.ranged(3)) as f32 as f64;          // thrshld first
let c2 = ((self.frng.ranged(350) as f32 / 100.0) as f64 + 4.0) as f32 as f64;
let c1 = ((self.frng.ranged(200) as f32 / 900.0) as f64 + 0.6) as f32 as f64;
self.contrast(c1, c2, c3);

let rf2 = -20 * (self.frng.ranged(3) + 1);                  // upon first
let rf1 = 5 + self.frng.ranged(3);
self.randoface(rf1, rf2);
```

Fixing both took type-5 **planets to 100% (28/28)** and moons to 23/24
(the last is the usual far-heap tail, §19), and the overall match rate from
85.4% to **99.2%**.

**Lesson:** whenever a single C statement calls the RNG in more than one
argument, assume right-to-left evaluation and consume the values in reverse.
`contrast`'s params are also Borland `float`, so each argument expression
rounds to f32 (`as f32 as f64`).

## 21. `wave()` inline-asm `add ax, 4` is a no-op on the write offset (2026-07-19)

In `wave()` (type 6) the inline asm does `add ax, 4` before writing to
`es:[di]`, but `di` is recomputed from `py*360 + px` and the `+4` never
reaches the store. Verified byte-for-byte against the C surfmap: the written
offset is `py*360 + px`, **not** `+4`. Dropping the spurious +4 took type-6
planets to 100%.

## 22. `float += double` accumulates in double, then rounds to float (2026-07-19)

Borland promotes a `float` operand to `double` for arithmetic, computes in
double precision, and rounds the result back to `float` on assignment. When
porting a `float` accumulator that is stepped by a `double` constant, you
must add in f64 and round to f32 *once per step* — not pre-round the step.

Type 3 (`atm_cyclon`) has a `float a` angle accumulator stepped by the
`double deg`:

```c
a = ranged_fast_random(360) * deg;   /* int * double -> double -> (float)a */
...
a += 6 * deg;                        /* (double)a + double -> (float)a     */
```

The Rust port had pre-rounded the increment to f32:

```rust
a += (6.0 * DEG) as f32;                  // WRONG: rounds the step first
```

which drifted `a` by ~1 ULP after enough iterations. On this body the drift
landed `cx + cr*cos(a)` on the wrong side of an integer boundary
(`-4.9999…` vs `-5.0000…`), so `(unsigned)` truncation gave 65531 instead of
65532, shifting a single `cirrus` write in `objectschart` by one byte. After
the atmosphere/terrain merge this flipped 4 surface bytes (row 27) and broke
the hash. Correct:

```rust
self.s_a = (self.frng.ranged(360) as f64 * DEG) as f32 as f64; // init
...
a = (a as f64 + 6.0 * DEG) as f32;                              // step
```

Fixing both the initializer and the step took type-3 **moons to 100% (9/9)**
and the overall match rate to **99.5%**.

**Lesson:** for a `float` accumulator advanced by a `double`, always
`(f_acc as f64 + step_f64) as f32`. Pre-casting the step to f32 changes the
rounding and desyncs floating-point-dependent addressing at boundaries.
