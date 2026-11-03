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

## What this does and does not remove

Two branches carry the change:

- `Noctis-IV-Plus` `feat/derive-gap` — `derive_oc_gap()` in the engine, the
  `oc_gap` write fix in `felisian_srf_darkline`, `canonicalise_gap()` in the
  harness, the validation tooling and this document.
- `sheetbot_internal` `feat/no-orig` — the pipeline stops reading
  `orig_sect_*_gap` and stops passing `-gap`.

The C build remains the **source of truth**. What is removed is the *capture
dependency in generation*: ports no longer need 16 bytes harvested from a DOS
run in order to produce the same output. Verification still runs the C harness —
that is the whole point of the exercise.

Deployed 2026-11-03 23:49Z (`nivgen_common.ts` sha256 `eafa6669…`, served). The
`nivgen verify orig` task was briefly paused and then restored, so both engines
re-verify against C:

- rust `414f2822`: 355 stars stale, rolling refresh, 10 per cycle.
- orig `9371069c`: 379 stars stale, because `canonicalise_gap()` changed the
  harness source hash and therefore the engine version.

**Reverted — C is immutable.** An earlier iteration added `canonicalise_gap()` to
`tests/harness/NIVTEST.CPP` to pin the header. That is a change to the oracle, so
it has been reverted: `NIVTEST.CPP` is now byte-identical to upstream and a
rebuild reports the original engine version `aa7490d9`. The oracle is the
pristine C build, invoked one body per process; the ports must match it, never
the other way round. Everything below that depends on the derivation is
port-side (`derive_oc_gap()` and the `oc_gap` write in `felisian_srf_darkline`),
which needs no C change.

The five bodies that only reproduced under batched DOSBox sessions
(VENE'EREIB 20-23, TRIUMPHANT|35) were therefore re-captured by running the
pristine oracle **one body per process** — an invocation choice, not a source
change. All 8,418 bodies now match the pristine oracle for both ports (lino
except the two felisian bodies described below).

Historical note on the earlier (now reverted) trade-off: `canonicalise_gap()` changes the harness, so C's
output for the 47 non-canonical bodies is now pinned rather than inherited from
whatever the executable's memory layout happened to be. Evidence that this is
not "fitting the oracle to the port":

- for the 42 intrinsic bodies the pinned build reproduces the *existing* stored
  hashes exactly (`ANDROI|6` → `286F815D`, `TUCANO|9` → `573EB960`, …), and
- for the 5 contaminated ones the pinned build agrees with what a lone C run
  produces; only the batched captures disagreed.

Without the pin, "the source of truth" changes identity every time the harness is
rebuilt (one added `printf` shifted every body `0x51E9 → 0x51F3`), which makes
verification meaningless. With it, C produces a stable answer and the port can be
held to it.

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

## Verification matrix (no gap injected)

Both engines were run exactly as the pipeline now invokes them (no `-gap`) and
compared against the `orig_*` columns, which hold the C build's output.

| engine | scope | checks | mismatches |
|---|---|---|---|
| rust | 388 stars / 8,418 bodies, sector hashes | 33,672 | **0** |
| rust | 20 stars, sector + textures | 2,360 | **0** |
| lino | 388 stars / 7,982 bodies, sector hashes | 31,928 | **4** (2 bodies) |
| lino | 20 stars, sector + textures | 2,360 | **0** |
| lino | 120 stars, sector + textures | 18,440 | **2** (same `DUO|2`) |

Plus live C-vs-rust cross-checks (`crosscheck_c_vs_rust.ts`, sector and
surftex, 8 fixtures): 64/64.

## Lino: two genuine, non-gap failures

`DUO|2` and `HIGHWAY 18|41` differ on `sect_rand_hm` *and* `sect_rand_oc`. Both
are **type 3 (felisian)** bodies at high absolute latitude, and both already
failed in the corpus *before* the gap handling changed (they are two of lino's
long-standing accuracy errors), so this is not caused by dropping the gap.

`DUO|2` reproduced with identical context on both engines:

```
C   : SECTOR body=2 type=3 lon=354 lat=96 sctype=2 albedo=32 night=0
      global_surface_seed=3564502   heightmap fnv=4436CC17  objectchart fnv=9A540C29
lino: SECTOR body=2 type=3 lon=354 lat=96 sctype=2 albedo=32 night=0
      global_surface_seed=3564502   heightmap fnv=F6AAE59F  objectchart fnv=80255058
```

19,851 of 40,000 heightmap bytes differ and byte 0 already differs, i.e. the
divergence starts at the beginning of `build_surface`, not in the object pass.
Latitude sweep on the same body (lon 354) narrows it sharply:

```
lat    60  65  70  75  80  85  90  91  92 | 93  94  95  96  97  98  99
lino    ok  ok  ok  ok  ok  ok  ok  ok  ok | DIFF DIFF DIFF DIFF DIFF DIFF DIFF
rust    ok  ok  ok  ok  ok  ok  ok  ok  ok | ok  ok  ok  ok  ok  ok  ok
```

`latitude = |landing_lat - 60| * 1.5`, so the split sits between 48 and 49.5.
The only latitude branch in that region of `build_surface` is
`NOCTIS-1.CPP:2007` (`latitude > 45 ? treepeaking = flandom()*0.9+0.1 :
flandom()+0.75`), which is on the felisian tree path — consistent with only type
3 being affected, but the 48/49.5 boundary suggests lino also carries a subtly
different `latitude` value (or a different tree predicate), not just the branch.

**Where the source is.** I was wrong earlier to say it was missing: the lino game
source is in the repo, in Lino's text format under `work/` (`work/vhgame.txt`,
`work/vhground.txt`, `work/grnd.txt`, `work/vhnivgen.txt`, …). The harness source
is derived from it by `tools/nivtest.py::derive_main()` (splices `VHNIV run` into
`vhgame.txt` and emits `work/nivtestmain.txt`).

Prime suspects, from reading those files:

- `work/vhground.txt:410-412` computes the landing latitude as integer
  `(|lat-60| * 3) / 2`, which truncates 49.5 to 49, while C keeps the float
  `|lat-60| * 1.5` (`NOCTIS-1.CPP:3484`). `work/vhnivgen.txt:317-329` does the
  same arithmetic with a doubling trick for the seed comparison.
- `work/grnd.txt:391-405` implements the C tree branch
  (`NOCTIS-1.CPP:2007`: `if (latitude > 45) treepeaking = flandom()*0.9+0.1;
  else treepeaking = flandom()+0.75;`) as `? A '<= 45 -> GR prol tree2`.

The 48 → 49.5 boundary in the sweep is exactly where a float-vs-truncated
comparison can flip, so the latitude arithmetic is the first place to look.

**Blocker: the build, not the source.** Rebuilding `build/nivtest` needs the
macOS x86_64 toolchain path: `build/compile_nivtest_linux.sh` requires
`build/macos-rtm01.bin` (not present in the repo) plus `xvfb-run`/`setarch`;
`tests/linoharness.py::build` shells out to PowerShell, which is not installed
here; the container path (`finch`, `lino-workspace`) has no VM on this machine
(`finch vm status` → `Nonexistent`). The Linux compiler in the tree is a 32-bit
ELF and compiling with `--sys:linux` instead of `--sys:macos` would change the
very allocator layout under test, so it is not a valid substitute.

So fixing these two bodies needs a machine that can run the lino build (or the
`lino-workspace` image). The verification work above is complete and does not
depend on that.

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

## Verification matrix (no gap injected)

Both engines were run exactly as the pipeline now invokes them (no `-gap`) and
compared against the `orig_*` columns, which hold the C build's output.

| engine | scope | checks | mismatches |
|---|---|---|---|
| rust | 388 stars / 8,418 bodies, sector hashes | 33,672 | **0** |
| rust | 20 stars, sector + textures | 2,360 | **0** |
| lino | 388 stars / 7,982 bodies, sector hashes | 31,928 | **4** (2 bodies) |
| lino | 20 stars, sector + textures | 2,360 | **0** |
| lino | 120 stars, sector + textures | 18,440 | **2** (same `DUO|2`) |

Plus live C-vs-rust cross-checks (`crosscheck_c_vs_rust.ts`, sector and
surftex, 8 fixtures): 64/64.

## Lino: two genuine, non-gap failures

`DUO|2` and `HIGHWAY 18|41` differ on `sect_rand_hm` *and* `sect_rand_oc`. Both
are **type 3 (felisian)** bodies at high absolute latitude, and both already
failed in the corpus *before* the gap handling changed (they are two of lino's
long-standing accuracy errors), so this is not caused by dropping the gap.

`DUO|2` reproduced with identical context on both engines:

```
C   : SECTOR body=2 type=3 lon=354 lat=96 sctype=2 albedo=32 night=0
      global_surface_seed=3564502   heightmap fnv=4436CC17  objectchart fnv=9A540C29
lino: SECTOR body=2 type=3 lon=354 lat=96 sctype=2 albedo=32 night=0
      global_surface_seed=3564502   heightmap fnv=F6AAE59F  objectchart fnv=80255058
```

19,851 of 40,000 heightmap bytes differ and byte 0 already differs, i.e. the
divergence starts at the beginning of `build_surface`, not in the object pass.
Latitude sweep on the same body (lon 354) narrows it sharply:

```
lat    60  65  70  75  80  85  90  91  92 | 93  94  95  96  97  98  99
lino    ok  ok  ok  ok  ok  ok  ok  ok  ok | DIFF DIFF DIFF DIFF DIFF DIFF DIFF
rust    ok  ok  ok  ok  ok  ok  ok  ok  ok | ok  ok  ok  ok  ok  ok  ok
```

`latitude = |landing_lat - 60| * 1.5`, so the split sits between 48 and 49.5.
The only latitude branch in that region of `build_surface` is
`NOCTIS-1.CPP:2007` (`latitude > 45 ? treepeaking = flandom()*0.9+0.1 :
flandom()+0.75`), which is on the felisian tree path — consistent with only type
3 being affected, but the 48/49.5 boundary suggests lino also carries a subtly
different `latitude` value (or a different tree predicate), not just the branch.

**Blocker:** the lino game source is not on this machine. `build/build_nivtest.sh`
compiles it inside a container (`NOLINO_RUNTIME=finch`, `NOLINO_CTR=lino-workspace`,
sources at `/workspace/nivtest-src`); the Finch VM does not exist here
(`finch vm status` → `Nonexistent`) and the source is absent from both
`noctis-lino` and `noctis-lino-beta20` and from eland. Fixing these two bodies
needs either the workspace image or a machine that can run the lino build.

### What the divergence looks like numerically

At `lat=93` the two heightmaps differ in 30,014 of 40,000 bytes, spread over the
whole surface (x 0..199, z 0..199), with this distribution of `C - lino`:

```
(-1, 7553) (1, 7428) (-2, 5035) (2, 4997) (-3, 2532) (3, 2469)
```

Symmetric ±1..3 offsets applied globally. A different branch (a tree or ruin pass
taking the other path) would change isolated regions *and* desynchronise the RNG,
producing large scattered differences instead. This is a small value difference
applied per pixel, which is what a latitude-derived quantity looks like when
`latitude` loses its half: `work/vhground.txt:410-412` computes
`(|lat-60| * 3) / 2` (integer, truncating 49.5 -> 49) while C keeps the float
`|lat-60| * 1.5` (`NOCTIS-1.CPP:3484`).

That is the cheapest thing to try first once a lino build is available: keep the
half (e.g. carry `latitude*2` as `3*|lat-60|` and halve only where a float is
genuinely needed), then re-run `validate_gap.ts --engine lino --star DUO`.

### Ruled out: integer truncation of latitude

Tempting, but wrong. `lat=96` gives `|96-60| * 1.5 = 54` exactly, so a
float-vs-integer latitude is identical for both engines there, yet they still
diverge. Confirmed by patching the C harness to truncate the same way
(`latitude = (int)(abs(lat-60) * 1.5)`) and rebuilding:

```
lat 96, C with truncated latitude : fnv=4436CC17   (unchanged, still differs from lino)
lat 92, C with truncated latitude : fnv=E0BA8671   (unchanged, still matches lino)
```

So the divergence is not in `latitude`'s fractional part. The harness was
reverted afterwards and verified (`MIDORI|1` → `47283EBA / 2BC8CFD1`), and the
corpus was not touched by the patched build (latest `orig_updated_at` is
05:15:44Z, before the patch window; harness srchash is back to `9371069c`).

What remains: at `lat=96` both engines have identical seed (3564502), identical
`sctype` (2), identical `albedo` (32), and an integral latitude (54), yet the
whole heightmap is offset by symmetric ±1..3. That is a numeric difference inside
the generation passes, not an input difference — the next step is a trace inside
`work/grnd.txt` (or a float-precision audit of the passes that feed
`p_surfacemap`) on a machine that can rebuild lino.

### Post-revert re-validation (pristine oracle, no gap)

After restoring `NIVTEST.CPP` byte-identical to upstream (rebuild reports
`aa7490d9`, the original engine version), both ports were re-checked against the
corpus with no `-gap`:

| engine | scope | checks | mismatches |
|---|---|---|---|
| rust | 60 stars / 1,021 bodies, sector | 4,084 | **0** |
| lino | 60 stars / 1,021 bodies, sector | 4,084 | **0** |
| rust | VENE'EREIB (80 bodies) + TRIUMPHANT (41 bodies) | 484 | **0** |
| lino | VENE'EREIB (80 bodies) + TRIUMPHANT (41 bodies) | 484 | **0** |

The earlier full-corpus runs (rust 0/33,672, lino 4/31,928) still stand: only the
five batch-ambiguous bodies changed in the corpus, and those are covered by the
targeted runs above.

The oracle's engine stamp was set back to `aa7490d9` to match the pristine build,
so the pipeline does not re-verify the corpus for no reason.

### Branch survey (both lino repos, all 36 refs each)

- No `macos-rtm01.bin` / `rtm01.bin` in **any** ref of either repo. The only
  runtime image on disk is untracked: `noctis-lino-beta20/src/linoleum_macos64/rtm01.bin`
  (Mach-O x86_64, Aug 18). Everything in git under `build/` is
  `*/accepted/compiler114m.exe`.
- Our checkout matches `upstream/master` for `work/vhground.txt`,
  `work/grnd.txt`, `work/vhnivgen.txt`.

**A fix for felisian bodies exists on `sector/hashes` and is not in our tree.**
Commit `40b591d` ("vhground: fix type-1 heightmap craters + felisian objectschart
wrap") adds, for the felisian darkline neighbour writes:

```
( neighbor writes: location+1, -1, +align, -align, with the C wrap
  objectschart if 40016<=i<80016 (C far-heap layout), else skip. )
? A '< 40016 -> VHGND felisian put done;
? A '>= 80016 -> VHGND felisian put done;
( i in 40016..80016: objectschart[i-40016] )
A - 40016; A + ROBJ; [MBptr] = A; [MBval] = [VHGNDtmp]; => MEM put byte;
```

`grep -n "40016\|80016" work/vhground.txt` in our tree returns **nothing**, i.e.
the shipped `build/nivtest` predates that wrap and simply drops those writes.
That is the same class of bug I had to fix in the rust port (which now writes them
into `oc_gap` for `40000..40016` and then `objectschart`). Note the branch's fix
starts at `40016` and therefore still skips the 16-byte header, so even it may not
be complete.

**Next step, once a lino build is available:** build `build/nivtest` from
`sector/hashes` (or cherry-pick `40b591d` onto the current source) and re-run
`validate_gap.ts --engine lino --star DUO` and `--star "HIGHWAY 18"`. If the
heightmap still differs, extend the wrap to `40000..40016` the way the rust port
does.
