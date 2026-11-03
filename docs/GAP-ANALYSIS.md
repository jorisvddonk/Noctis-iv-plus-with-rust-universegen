# Deriving the Borland `farmalloc` Gap and Full-Corpus Verification Report

**Status:** Complete and cross-checked (2026-11-03)  
**Branch:** `feat/derive-gap` (`Noctis-IV-Plus`)

---

## 1. Executive Summary

Both the Rust port (`Noctis-IV-Plus`) and the Linoleum port (`noctis-lino`) can generate planetary sectors without capturing or injecting the 16-byte Borland `farmalloc` gap (`-gap`) from the DOS C oracle (`orig`):

1. **Oracle immutability:** `tests/harness/NIVTEST.CPP` and `source/NOCTIS-*.CPP` are byte-identical to upstream (`srchash aa7490d9`). The C build is the immutable source of truth; the ports align to C, never the reverse.
2. **In-engine gap derivation:** The 16-byte Borland C++ 3.1 far-heap block header between `p_surfacemap` and `objectschart` is deterministic for a fresh process and is derived directly in both ports as `[00 × 12, C5 09, E9 51]`.
3. **Felisian out-of-bounds write fix (Rust):** All 42 bodies whose captured gap differed from `C509E951` in single-body runs were type-3 (Felisian) planets where `felisian_srf_darkline` wrote across the end of `p_surfacemap` (`40000..40016`) into the 16-byte heap header before `build_surface()` read `p_surfacemap[ptr + 200]`. Extending `felisian_srf_darkline` in `tests/nivgen/src/surface_sector.rs` to write indices `40000..40016` into `self.oc_gap` resolved every one of them without `-gap`.
4. **Batch-session contamination cleanup (5 bodies):** The remaining 5 bodies (`VENE'EREIB|20..23`, `TRIUMPHANT|35`) had stale `orig` fixtures captured when multiple bodies shared a single DOSBox process and inherited mutated gap bytes from an earlier body in the batch. Re-capturing them with the pristine C oracle invoked **one body per process** restored their canonical `C509E951` header, and both Rust and Lino match them with zero mismatches.
5. **Verification results (no `-gap` injected):**
   - **Rust (`nivgen`):** **0 mismatches** across all **388 stars / 8,418 bodies / 33,672 sector hashes**, plus **0 mismatches** across 20-star and 60-star sector+texture sweeps and 64/64 live C-vs-Rust checks.
   - **Lino (`nivlin`):** **386 / 388 stars match** (31,924 / 31,928 sector hashes). Only **2 bodies** fail (`DUO|2` at `lon=354, lat=96` and `HIGHWAY 18|41` at `lon=325, lat=108`), both with and without `-gap`.
6. **Root cause of the 2 Lino failures (verified by exact hash reproduction):** In `source/NOCTIS-1.CPP:2059`, `fast_srand(landing_pt_lat * landing_pt_lon)` evaluates `landing_pt_lat * landing_pt_lon` as a **16-bit signed `int`** and sign-extends it to 32-bit `dword` (`unsigned long`) before seeding `fast_srand`. When `lat * lon >= 32768`, the 16-bit signed product overflows negative and sign-extends high bits `0xFFFF0000` into `fast_srand`'s 32-bit state. Lino (`work/vhground.txt:387,457` and `work/vhnivgen.txt:310`) multiplies `VHGNDlat * VHGNDlon` in 32-bit arithmetic without 16-bit signed truncation + sign-extension (`0x0000809A` instead of `0xFFFF809A` at `DUO|2 lat=93`, and `0x000084CC` instead of `0xFFFF84CC` at `DUO|2 lat=96`). Removing the `i16 -> i32` sign-extension in Rust reproduces Lino's exact heightmap FNV hashes (`F6AAE59F` at `lat=96`, `57E915B4` at `lat=93`).

---

## 2. What the 16-Byte Gap Is

In `source/NOCTIS-1.CPP:2616`, the object-distribution slope loop at the end of `build_surface()` reads 200 bytes past the end of `p_surfacemap` (40,000 bytes):

```c
for (ptr = 0; ptr < oc_bytes; ptr++) {
    incl  = abs(p_surfacemap[ptr] - p_surfacemap[ptr + 1]);
    incl += abs(p_surfacemap[ptr] - p_surfacemap[ptr + 200]);
    ...
}
```

When `ptr` is in `39800..39999`, `ptr + 200` reaches `40000..40199`. In the DOS build (`NOCTIS-0.CPP:672–679`), the far heap is allocated sequentially at startup:

```
[ p_background  65535 B ]
[ p_surfacemap  40000 B ]
[ 16-byte farmalloc header ]  <-- indices 40000..40015 ("oc_gap")
[ objectschart  40000 B ]     <-- indices 40016..40199
```

Because `p_surfacemap` is allocated with `farmalloc(40000)` and rounded up to paragraph alignment (`40000 = 2500 * 16` bytes, normalized so the buffer starts at offset `0x0004` of its segment), the 200 out-of-bounds bytes starting at `p_surfacemap + 40000` consist of:
- **Bytes `40000..40011` (12 bytes):** trailing alignment slack of `p_surfacemap` (`0x00 × 12`),
- **Bytes `40012..40015` (4 bytes):** the Borland C++ 3.1 far-heap control header immediately preceding `objectschart`:
  - `C5 09` (`0x09C5 = 2501` paragraphs = `40,016` bytes: 40,000-byte payload + 16-byte header),
  - `E9 51` (`0x51E9`: segment address of the preceding heap block, deterministic for the fixed binary image and allocation sequence),
- **Bytes `40016..40199` (184 bytes):** `objectschart[0..184]`.

Thus the canonical 16-byte gap in a fresh process is:

```
00 00 00 00 00 00 00 00 00 00 00 00 C5 09 E9 51
```

Both ports initialize their gap buffer to this derived constant (`derive_oc_gap()` in `tests/nivgen/src/system.rs` and `DEFAULT_GAP` in `noctis-lino/tools/nivtest.py`).

---

## 3. Why 47 Bodies Had Non-Canonical Captured Gaps (and How All 47 Were Resolved)

Across the 8,418 bodies in the corpus, 8,371 had the canonical gap `C509E951` (last 4 bytes of the 16-byte gap), while 47 differed:

### 3.1. 42 bodies: intrinsic Felisian darkline writes into `40000..40015`

In `source/NOCTIS-1.CPP:1553–1596`, `felisian_srf_darkline()` draws dark river/crevasse lines on type-3 (Felisian) planets and writes four neighbours (`location + 1`, `location - 1`, `location + align`, `location - align`) using a **16-bit unsigned check**:

```c
i = location + align;
if (i < 80000) p_surfacemap[i] = k;
```

When `i` lands in:
- `0..39999`: it writes into `p_surfacemap`,
- `40000..40015`: it writes **directly into the 16-byte gap** between `p_surfacemap` and `objectschart`,
- `40016..79999`: it writes into `objectschart[i - 40016]`.

Previously, `tests/nivgen/src/surface_sector.rs` handled `0..40000` and `40016..80016`, but **silently dropped writes in `40000..40016`**. Routing `40000..40016` into `self.oc_gap[(ni - 40000) as usize]` (`surface_sector.rs:330–332`) reproduced all 42 intrinsic non-canonical bodies without `-gap`:
- When the overwrite landed in bytes `0..11` of the gap (30 bodies), the last 4 bytes remained `C509E951`, and now the modified leading bytes match C as well.
- When the overwrite landed in bytes `12..15` of the gap (42 bodies where the captured 4-byte tail was `3E3E3E51`, `C5094242`, `36363636`, etc.), `self.oc_gap` receives the exact same terrain height byte `k` as DOS memory, so the subsequent `p_surfacemap[ptr + 200]` slope read sees the identical bytes.

### 3.2. 5 bodies: multi-body DOSBox batch contamination

Five non-Felisian bodies (`VENE'EREIB|20..23` and `TRIUMPHANT|35`) had non-canonical gaps in the database only because `NIVTEST.EXE` does not reset the 16 gap bytes between bodies in a batch run:
- `VENE'EREIB|19` is a type-3 Felisian body whose `felisian_srf_darkline` overwrites the gap (`C5092929`). When bodies `20..23` ran in the same DOSBox invocation, they inherited `VENE'EREIB|19`'s dirty gap bytes.
- `TRIUMPHANT|33` (type-3) similarly dirtied the gap before `TRIUMPHANT|35` ran in the same batch.

Running the **pristine** C oracle (`aa7490d9`) **one body per process** via `tests/nivgen/tools/recapture_orig.ts` confirmed that all five bodies have the canonical gap `C509E951` and match both Rust and Lino with zero mismatches.

---

## 4. Cross-Checking the 2 Lino Failures (`DUO|2` and `HIGHWAY 18|41`)

Across the entire 388-star corpus, Lino (`nivlin`) fails on only 2 bodies:

| Body | Type | Coords `(x, y, z)` | Landing `(lon, lat)` | C & Rust `hm / oc` | Lino `hm / oc` |
|---|---|---|---|---|---|
| `DUO\|2` | 3 (Felisian) | `(-21320432, 17757206, 38515440)` | `(354, 96)` | `4436CC17 / 9A540C29` | `F6AAE59F / 61B59822` |
| `HIGHWAY 18\|41` | 3 (Felisian) | `(1043528095, 1507257996, -247989340)` | `(325, 108)` | `A0C3F380 / 915731B9` | `B499A36B / A6FC6660` |

### 4.1. Cross-check of earlier hypotheses (ruled out)

1. **Ruled out — Commit `40b591d` (`VHGND felisian line` wrap into `objectschart`):**
   - *Flaw in earlier reasoning:* An earlier note suggested that `commit 40b591d` on `sector/hashes` (which wraps Felisian neighbour writes `40016..80016` into `objectschart`) could explain `DUO|2` and `HIGHWAY 18|41`.
   - *Why that cannot explain the heightmap diff:* `p_surfacemap` (`0..39999`, hashed as `heightmap`) is never written by indices `>= 40000`; `objectschart` and `oc_gap` only affect `objectschart` (either directly or via the `ptr + 200` slope read in `build_surface()`). Yet on both `DUO|2` and `HIGHWAY 18|41`, **the heightmap itself (`sect_rand_hm`) diverges** across ~75% of all 40,000 pixels. Therefore, missing `40016..80016` writes in `VHGND felisian line` cannot be the cause of the heightmap mismatch (though Lino does also need the `40000..80016` wrap for `objectschart` parity on Felisian planets whose rivers touch the bottom border).
2. **Ruled out — Integer truncation of `latitude = abs(landing_pt_lat - 60) * 1.5`:**
   - *Why it fails cross-check:* At `DUO|2` (`lat = 96`), `|96 - 60| * 1.5 = 36 * 1.5 = 54.0` is already an exact integer (`0.5` fractional part only occurs when `lat` is odd). Thus float and integer-truncated `latitude` are identical at `lat = 96` (`54`), yet `DUO|2` fails at `lat = 96`.

### 4.2. Verified Root Cause: 16-bit signed overflow in `fast_srand(landing_pt_lat * landing_pt_lon)`

#### The C behavior (`source/NOCTIS-1.CPP:2059`)
Immediately before the `switch (nearstar_p_type[ip_targetted])` in `build_surface()`, Noctis reseeds both RNG streams from the landing coordinates:

```c
fast_srand (landing_pt_lat * landing_pt_lon);
srand (landing_pt_lat * landing_pt_lon);
```

In Borland C++ 3.1 (16-bit target):
- `landing_pt_lat` and `landing_pt_lon` are 16-bit signed `int` (`i16`).
- Their multiplication `landing_pt_lat * landing_pt_lon` is performed in **16-bit signed `int`** (`IMUL`).
- `srand(unsigned)` takes a 16-bit `unsigned`, so only the low 16 bits matter (`seed & 0xFFFF`).
- **`fast_srand(dword)` takes a 32-bit `unsigned long` (`dword`)!** When the 16-bit signed `int` product `landing_pt_lat * landing_pt_lon` is passed to `fast_srand(dword)`, C **sign-extends** the 16-bit `int` to 32 bits (`CWD` -> `DX:AX`) before `fast_srand` executes `or word ptr flat_rnd_seed, 3`:
  ```
  flat_rnd_seed = (s & 0xFFFF0000) | ((s & 0xFFFF) | 3)
  ```
- Whenever `landing_pt_lat * landing_pt_lon >= 32768` (and `< 65536`), the 16-bit signed `int` overflows negative (`bit 15 = 1`), so sign-extension sets the upper 16 bits of `flat_rnd_seed` to `0xFFFF` instead of `0x0000`!

#### Why `DUO|2` transitions between `lat = 92` (match) and `lat = 93..96` (fail)
On `DUO|2`, `lon = 354`:
- At `lat = 92`: `92 * 354 = 32,568 <= 32,767` (fits in positive `i16` -> `0x00007F38` in both C and 32-bit arithmetic). **Lino matches C (`hm = E0BA8671`).**
- At `lat = 93`: `93 * 354 = 32,922 >= 32,768` (overflows `i16` to `-32,614` -> sign-extended in C to `0xFFFF809A`, whereas 32-bit unsigned/signed multiplication gives `0x0000809A`). **Lino diverges!**
- At `lat = 96`: `96 * 354 = 33,984 >= 32,768` (overflows `i16` to `-31,552` -> `0xFFFF84C0` in C vs `0x000084C0` in Lino). **Lino diverges!**
- On `HIGHWAY 18|41` (`lon = 325, lat = 108`): `108 * 325 = 35,100 >= 32,768` (overflows `i16` to `-30,436` -> `0xFFFF891C` in C vs `0x0000891C` in Lino). **Lino diverges!**

#### Why the heightmap diff is symmetric `±1..3` across 30,014 / 40,000 pixels
In `source/NOCTIS-1.CPP:2289–2291` (inside `case 3`, Felisian planets, when `sctype != ICY && sctype != DESERT`):

```c
if (!waswet || (waswet && !random(5))) {
    for (ptr = 0; ptr < oc_bytes; ptr++)
        p_surfacemap[ptr] += fast_random(3);
}
```

Every previous terrain pass in `case 3` (`smooth_surface`, `waved_ground`, `felisian_srf_darkline`, etc.) uses `random()` (seeded by `srand`, which only uses the low 16 bits, so C and Lino stay 100% synchronized!). The **very first consumer of `fast_random` on the heightmap** is `p_surfacemap[ptr] += fast_random(3)`, which adds a uniform `0, 1, 2, or 3` to each of the 40,000 pixels.

When two independent uniform `{0, 1, 2, 3}` streams are subtracted (`C - Lino`), the theoretical difference distribution over 40,000 pixels is:
- `0`: `4/16 × 40000 = 10,000` identical bytes (`30,000` differing bytes),
- `±1`: `3/16 × 40000 = 7,500` each,
- `±2`: `2/16 × 40000 = 5,000` each,
- `±3`: `1/16 × 40000 = 2,500` each.

The empirically measured `C - Lino` byte difference on `DUO|2` at `lat = 93` is:
- Differing bytes: **30,014 / 40,000** (expected: 30,000)
- `(-1: 7553, +1: 7428, -2: 5035, +2: 4997, -3: 2532, +3: 2469)` — matching the theoretical `7500 / 5000 / 2500` triangle distribution to within `< 1%`!

#### Code location in `noctis-lino` and exact hash reproduction
In `noctis-lino/work/vhground.txt:387,457` and `work/vhnivgen.txt:310`:

```linoleum
[VHGNDlocseed] = [VHGNDlat]; A = [VHGNDlocseed]; A '* [VHGNDlon]; [VHGNDlocseed] = A;
...
A = [VHGNDlocseed]; => SU fast srand;
```

Because Linoleum registers are 32-bit, `A '* [VHGNDlon]` computes the 32-bit product (`33984 = 0x000084C0`) without truncating to 16-bit signed `i16` and sign-extending to 32-bit (`0xFFFF84C0`).

By comparison, Rust (`tests/nivgen/src/surface_sector.rs:845`) explicitly models the 16-bit signed overflow and sign-extension:

```rust
let land_seed = ((self.landing_pt_lat as i16).wrapping_mul(self.landing_pt_lon as i16)) as i32 as u32;
```

**Direct experimental verification:** Temporarily replacing `surface_sector.rs:845` in Rust with `let land_seed = (self.landing_pt_lat * self.landing_pt_lon) as u32;` (emulating Lino's 32-bit multiply) and running `nivgen sector` produces **Lino's exact heightmap FNV hashes**:

| Body & Latitude | `lat * lon` | C & Rust (`i16 -> i32`) `hm` | Rust with 32-bit mul `hm` | Lino `hm` | Match? |
|---|---|---|---|---|---|
| `DUO\|2` (`lat=92, lon=354`) | `32,568` (`< 32768`) | `E0BA8671` | `E0BA8671` | `E0BA8671` | **100% identical** |
| `DUO\|2` (`lat=93, lon=354`) | `32,922` (`>= 32768`) | `7991E63A` | `57E915B4` | `57E915B4` | **100% identical** |
| `DUO\|2` (`lat=96, lon=354`) | `33,984` (`>= 32768`) | `4436CC17` | `F6AAE59F` | `F6AAE59F` | **100% identical** |

#### Fix required in `noctis-lino` (once a Linoleum compiler environment is available)
In `work/vhground.txt` (line 387) and `work/vhnivgen.txt` (line 310), sign-extend the low 16 bits of `VHGNDlocseed` to 32 bits before storing:

```linoleum
A = [VHGNDlat]; A '* [VHGNDlon]; A < 16; A '> 16; [VHGNDlocseed] = A;
```
*(or equivalently `A & 0FFFFh; ? A < 8000h -> ...; A | 0FFFF0000h;`)*, and cherry-pick `commit 40b591d` (extended to `40000..80016` including the 16-byte gap) so `VHGND felisian line` also wraps neighbour writes into `oc_gap` and `objectschart`.

---

## 5. Summary Verification Matrix (Pristine Oracle `aa7490d9`, No `-gap`)

| Engine | Scope | Stars | Bodies | Hashes Checked | Mismatches |
|---|---|---:|---:|---:|---:|
| `rust` (`nivgen`) | Full corpus (`sector`: `def_hm`, `def_oc`, `rand_hm`, `rand_oc`) | 388 | 8,418 | 33,672 | **0** |
| `rust` (`nivgen`) | Sector + surface/sky textures (`--fields all`, 20 + 60 stars) | 80 | 1,316 | 6,444 | **0** |
| `rust` (`nivgen`) | Live C (`NIVTEST.EXE`) vs Rust (`sector` + `surftex`) | 10 | 32 | 64 | **0** |
| `lino` (`nivlin`) | Full corpus (`sector`: `def_hm`, `def_oc`, `rand_hm`, `rand_oc`) | 388 | 7,982 | 31,928 | **4** (`DUO\|2`, `HIGHWAY 18\|41`) |
| `lino` (`nivlin`) | Post-recapture `VENE'EREIB` (80) + `TRIUMPHANT` (41) | 2 | 121 | 484 | **0** |
| `lino` (`nivlin`) | Sector + surface/sky textures (`--fields all`, 120 stars) | 120 | 2,305 | 18,440 | **2** (`DUO\|2`) |
