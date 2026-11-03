# The farmalloc gap: what it is, and why it can't be derived from game logic

Status: **investigated, wall reached** (2026-11-02). Written while trying to remove
the C build (`orig`) from the verification pipeline.

## What the bytes are

`build_surface()` has an out-of-bounds read. In the object-distribution loop
(`source/NOCTIS-1.CPP:2616`):

```c
incl += abs(p_surfacemap[ptr] - p_surfacemap[ptr + 200]);
```

`ptr` runs to `oc_bytes` (40000), so `ptr + 200` reaches 40199 — 200 bytes past the
end of the 40000-byte heightmap. In the DOS build those reads land, harmlessly, in
whatever memory follows the buffer:

```
[ p_surfacemap  40000 bytes ][ 16 bytes ][ objectschart  40000 bytes ]
                                ^ the "gap"
```

Both ports model it (`surface_sector.rs::surf_h`, `tools/nivtest.py` buffer layout
`height (98000,40000) / gap (138000,16) / objects (138016,40000)`). The harness
captures the 16 bytes from the C build and feeds them back via `-gap`.

## Decoded format

Probing the real BC++ 3.1 runtime (a small far-heap test program built with the
same toolchain flags as the game: `-ml -3 -f287 -O -Oe`) shows the header is:

```
[ 12 zero bytes ][ block size in paragraphs, LE ][ block segment, LE ]
```

Evidence — a probe that allocates three 40000-byte blocks and prints the 16 bytes
after each:

```
b after c alloc  (b+40000): 000000000000000000000000C5092B15
c after d=1000   (c+40000): 0000000000000000000000003F00F01E
c after d=2000   (c+40000): 0000000000000000000000007E00F01E

b = 0x152B0004  -> segment 0x152B, and the gap ends 2B 15  (LE) ✔
c = 0x1EF00004  -> segment 0x1EF0, and the gap ends F0 1E  (LE) ✔
40000 bytes = 2500 paragraphs + 1 header paragraph = 2501 = 0x09C5 -> bytes C5 09 ✔
1000 bytes = 63 paragraphs -> 3F 00 ✔
2000 bytes = 126 paragraphs -> 7E 00 ✔
```

So for `objectschart` (40000 bytes) the size field is always `C5 09`, and the only
variable part is the **segment the buffer landed at**. `…C509E951` means
"segment 0x51E9".

Corpus check (8,418 captured bodies): only 6 distinct segments, and one dominates.

| segment | bodies |
|---|---|
| 0x51E9 | 8,390 |
| 0x0000 | 15 |
| 0x5100 | 6 |
| 0x00E9 | 5 |
| 0x11E9 | 1 |
| 0x1CE9 | 1 |

## The wall

The segment is not a property of the planet or the game. It is the address the
DOS program's far heap happened to give the buffer. Two experiments prove it:

**1. The value tracks the *binary's* layout, not the planet.** Adding a single
`printf` to `NIVTEST.CPP` and rebuilding moved every body's segment from `0x51E9`
to `0x51F3`:

```
pristine harness, MIDORI|1 : gap=…C509E951  oc=2BC8CFD1   (== corpus)
instrumented build         : gap=…C509F351  oc=…           (segment 0x51F3)
```
Restoring the source and rebuilding restores `…C509E951` exactly.

**2. Within one binary, it also varies per body.** Same pristine binary, three
bodies:

```
ANDROI|6   gap=0000E900
SOKUN|76   gap=C509E951
IKTAYEL|9  gap=C5000000
```

Those three values are the canonical header with individual bytes zeroed
(`C509E951` → `0000E900` zeroes the size high byte and the segment high byte). So
the region is being partially overwritten by activity that depends on how that
particular body's generation ran.

A port running on modern hardware has no "correct" value to derive: the number is
an address inside a 1990s DOS heap, affected by code size, allocation history and
stray writes. There is no game-logic function `gap(planet)`. This is the wall.

## Second discovery: generation scribbles into the header

Canonicalising the header is necessary but not sufficient. Instrumenting
`build_surface` (temporary trace in `NOCTIS-1.CPP`) showed the bytes changing
*during terrain generation*, before the object loop ever reads them:

```
after memset           000000000000000000000000C509E951
before object loop     0000000000000000000000000000E900     <- zeroed here
after object loop      0000000000000000000000000000E900
```

The culprit is `felisian_srf_darkline` (`NOCTIS-1.CPP:1605`), which writes
`map[location ± 1]` and `map[location ± align]` with no upper bound. For locations
near the end of the map those writes land past the heightmap, in the header
region. The rust port mirrored those writes into `p_surfacemap` (0..40000) and
`objectschart` (40016..80016) but **silently dropped the ones landing in
40000..40016** — the header itself. So the port always saw the pristine header
while C saw a scribbled one.

That single missing branch accounted for the remaining 42 bodies. With it:

```
MIDORI|1   unchanged                       2BC8CFD1
ANDROI|6   was 286F815D -> 286F815D (matches C and the corpus)
```

## Consequence for dropping `orig`

The 16 bytes cannot be computed from first principles, so a port must **choose a
canonical value** and treat it as part of the specification. The natural choice is
the value the reference build produces for the overwhelming majority of bodies:

```
C509E951   (size 2501 paragraphs, segment 0x51E9)
```

This is now what both engines default to, and it is *derived*, not hardcoded:
`derive_oc_gap()` builds the header from `OC_BLOCK_BYTES` and `OC_BLOCK_SEGMENT`
in `system.rs`/`nivtest.py`.

Measured cost of that choice against the frozen corpus (`validate_gap.ts`,
sector hashes only):

- 8,418 bodies checked, 33,672 hashes
- mismatches only on bodies whose captured header is not the canonical one
- of the 47 non-canonical bodies, **42 reproduce solo** in the C build (they are
  intrinsic to that body under the canonical binary — mostly byte-zeroed variants
  of the canonical header) and **5 are contamination** from the harness having
  processed several bodies in one DOSBox session (VENE'EREIB 20–23,
  TRIUMPHANT|35: a solo run gives the canonical value).

So the two options are:

**A. Canonicalise (recommended).** Declare `C509E951` the specification, re-baseline
the ~42 affected bodies once with the canonical harness, and the corpus becomes
100% reproducible by the ports. `orig` then exists purely as a one-time oracle and
can be removed from the pipeline entirely.

**B. Bit-exact to the captured DOS artefacts.** Requires emulating the BC++ far
heap *and* the per-body stray writes that zero individual header bytes. Large
effort, no end-user benefit, and it would pin the ports to one specific build of a
test harness.

## Result (after both fixes)

- `validate_gap.ts` over all 388 stars: 8,418 bodies, 33,672 sector hashes, **0
  mismatches**. The port now computes the header itself and reproduces the corpus.
- `crosscheck_c_vs_rust.ts`: 8 hand-picked fixtures (including every former
  outlier family) run through both the C harness and the rust port live: **32/32
  checks agree**. This does not consult the corpus, so it validates the corpus
  and the derivation together.
- Five bodies (VENE'EREIB 20-23, TRIUMPHANT|35) were batch-contaminated captures;
  they were re-captured once with the canonical harness (`recapture_orig.ts`) and
  now match too.

The reference is now a property of the algorithm, not of the executing binary:
rebuilding the harness no longer changes any output.

## Validations run (all passing)

| check | scope | result |
|---|---|---|
| `validate_gap.ts` | 388 stars, 8,418 bodies, 33,672 sector hashes, engine run with **no** `-gap` | **0 mismatches** |
| `crosscheck_c_vs_rust.ts` (sector) | 8 fixtures covering every former outlier family, C and rust run live | **32/32** |
| `crosscheck_c_vs_rust.ts` (surftex) | same fixtures, `surf_texture`/`sky_texture` | **32/32** |
| harness wrapper path | rust, 5 stars, all 11 compared fields, no `-gap` | **1,605 comparisons, 0** |
| determinism | same body run 3x | identical |

## Retiring orig

Two branches carry the change:

- `Noctis-IV-Plus` `feat/derive-gap` — `derive_oc_gap()` in the engine, the
  `oc_gap` write fix in `felisian_srf_darkline`, `canonicalise_gap()` in the
  harness, the validation tooling and this document.
- `sheetbot_internal` `feat/no-orig` — the pipeline stops reading
  `orig_sect_*_gap` and stops passing `-gap`.

Activated 2026-11-03 23:49Z: the sheetbot branch was deployed to the server
(`nivgen_common.ts` sha256 `eafa6669…`, served) and `nivgen verify orig` was set
to PAUSED. The rust engine immediately started a full re-verification at
`414f2822` (355 stars stale, 10 per cycle) and every cycle reports matches. The
`orig_*` columns remain the frozen reference for accuracy; the C build no longer
runs.

One consequence worth knowing: `canonicalise_gap()` changed the harness's source
hash, so if `orig` is ever resumed it will rebuild and re-verify every star once
with the canonical layout (which is the intended behaviour).

## Tooling added

- `tests/nivgen/tools/validate_gap.ts` — runs the engine with **no** `-gap` (i.e.
  derived header) and compares all four sector hashes per body against the frozen
  corpus. Resumable, parallel, exits non-zero on any mismatch.
- `tests/nivgen/tools/probe_c_gap.ts` — runs the C harness alone, in a fresh
  DOSBox process, for every body whose header is non-canonical, to classify it as
  intrinsic vs batch contamination.
- `tests/nivgen/tools/recapture_orig.ts` — re-derives `orig` sector hashes for
  named bodies with the canonical harness (used for the five contaminated
  captures).
- `tests/nivgen/tools/crosscheck_c_vs_rust.ts` — runs C and rust side by side on
  fixed fixtures and compares hashes, independently of the corpus.
