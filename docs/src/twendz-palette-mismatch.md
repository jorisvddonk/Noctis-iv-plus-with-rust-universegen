# TWENDZ Body 13 Palette Mismatch — RESOLVED

## Status

**Fixed.** Root cause: C BSS adjacency of `p_background`/`s_background` not
replicated in Rust's Vec layout.

## Symptom (before fix)

```
Canonical (BCPP 3.1/DOSBox-X):  palette64 fnv=B30F9135
Rust (before fix)              :  palette64 fnv=FD5AAB45
Surface                       :  surface_map fnv=7C3C12A4  (MATCHES)
```

## Investigation summary

### Step 1: LCG state shift confirmed

The 18 `random(c)` palette values from the canonical trace:

```
C13_INLINE body=13: 13 21 20 8 6 8 1 6 20 8 15 19 13 7 22 16 6 22
```

Inverse LCG gives C state `0xC0CE82EC`. Rust state is `0x7312EDC2`.
`lcg_back(Rust, 2) = C` → Rust makes 2 more `rand()` calls during surface gen.

### Step 2: Call count breakdown

| Sub-function | LCG calls | Notes |
|---|---|---|
| Fracture (20×) | 5688 | `1 + 2·(gr+1)` per fracture |
| Randoface | 129584 | 2 calls per eligible pixel (64792 × 2) |
| Palette | 18 | 18 × `random(pc)` |
| **Total** | **135272** | C = 135270 → diff = **2** |

### Step 3: Eligible pixel counter

Added a 16-bit counter to C's `randoface()`:

| Body | C `rf_eligible` | Rust eligible | Pixel diff |
|---|---|---|---|
| 13 | 64791 | 64792 | **1** |
| 24 | 64800 | 64800 | **0** |

C has exactly 1 fewer eligible pixel for body 13.

### Step 4: Binary-diff pre-randoface row 179

Dumped the pre-randoface pixel buffer from C and Rust. The **first 40 columns**
of row 179 are systematically different:

```
Col 17: C=0 (ineligible)   Rust=32 (eligible)
```

This is the single pixel causing the 2-call shift.

### Step 5: Root cause — `lssmooth` OOB read

`lssmooth()` processes `(QUADWORDS-80)*4 = 64480` pixels. For the last 40
iterations (columns 0–39 of row 179), it reads `di+360` and `di+361` which
exceed the 64800-byte surface map.

**In C:** `p_background[64800]` = `s_background[0]` — the arrays are adjacent
globals in the BSS segment:

```c
unsigned char p_background[64800];
unsigned char s_background[64800];
```

**In Rust:** each Vec is sized to 65536 with the tail filled by `0xFF` in
`make_bg()` (documented as "empirically matched"). This does not match C's
adjacency — lssmooth reads `s_background[0..80]`, not `0xFF`.

### Earlier hypotheses ruled out

1. **cos/sin precision** — 1-ULP difference between x87 `FCOS`/`FSIN` and
   IEEE 754 `cos`/`sin` is ~2e-16, 12 orders of magnitude below f32 resolution
   at 179.0 (~2e-5). Verified: f32, libm, and f64 cos/sin all produce
   identical surface hashes. **Ruled out.**

2. **`step_back(2)`** — LCG modular inverse corrects the state but cannot be
   applied universally (body 24 breaks). Hardcoding `logical_id` is fragile.
   **Rejected for general use.**

## Fix

In `surface()`, before the `match ptype` dispatch, copy the first 80 bytes of
`s_background` into `p_background`'s tail — but **only for planets** (the
OOB region for moons maps to a different BSS area that `0xFF` already matches):

```rust
if self.p_owner[logical_id as usize] == -1 {
    let copy_len = self.s_background.len().min(80);
    let end = BG_SURFACE + copy_len;
    if end <= self.p_background.len() {
        self.p_background[BG_SURFACE..end]
            .copy_from_slice(&self.s_background[..copy_len]);
    }
}
```

The fix is **general and scalable**:
- No hardcoded body indices
- No architecture-dependent cos/sin emulation
- No LCG `step_back` kludge
- Works for ALL planets (not just TWENDZ body 13)

## Verification

| Body | Surface | Palette | Match |
|---|---|---|---|
| 13 | `7C3C12A4` | `B30F9135` | ✅ (was mismatch) |
| 24 | `08106792` | `BB3E0223` | ✅ (unchanged) |
| 18 | `9547A8BA` | `3C37BFDA` | ✅ (unchanged) |
| 27 | `DD662830` | `03AB1317` | ✅ (unchanged) |
| All 80 TWENDZ | — | — | ✅ 0/80 mismatches |

Full corpus: surface 10685/10685, atmosphere 10685/10685, palette 10681/10685
(remaining 4 palette mismatches are pre-existing in other stars, not regressions).
