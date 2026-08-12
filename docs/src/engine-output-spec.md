# Engine output specification (orig C vs rust)

Requirements source for `noctis-iv-lr` (and any future peer engine): what the
original Noctis-IV-Plus C harness (`NIVTEST.EXE`) and the Rust port (`nivgen`)
write, so a new engine can produce byte-compatible, verifiable output.

## 0. Shared foundations (any engine must reproduce)

- **Borland LCG**: `seed = (seed*22695477 + 1) & 0x7FFFFFFF`, then
  `rand() = (seed >> 16) & 0x7FFF`. Derived helpers:
  `random(n) = (long)rand() * n / 32768`,
  `flandom() = random(32767) * 0.000030518`.
  Also `fast_random()` / `fast_srand()` variants.
- **Hash line format (both engines)**: `{label:<16} len={n:<6} fnv={HEX8} crc={HEX8}`
  — parseable with `^<label>\s+len=\d+\s+fnv=(\w+)`.
- **Inputs**: star coordinates `(x, y_user, z)`; engines convert `y_user` →
  internal engine-y.
- **Per-planet buffers**: `p_background` (surface texture / surface map),
  `s_background` (sky), `p_surfacemap` (200×200 heightmap), `objectschart`
  (200×200 object chart), `tmppal` (768 bytes = 256 colors × 3, 6-bit values).
- **`surface_map` hash region**: first 46080 bytes = 360×128 rows (NOT the full
  map; rows 128+ are a scratch tail that must be excluded).

## 1. Orig (C harness `NIVTEST.EXE` — Borland C++ 3.1, run under DOSBox-X)

Commands: `selftest | system | scan | find | planet | planet-all | sector | surftex`.
Binary dumps via the **`-dump <DIR>`** argument (lowercase filenames written into DIR).

| Command | Text output (hash labels) | Binary dumps |
|---|---|---|
| `system -x -y -z` | `=== SYSTEM ===`; `STAR x= y= z=`; `identity=`; `class= ray= spin= rgb=, , `; `nop= nob= labeled=`; per body `BODY nn type= owner= moonid= ray= orb_ray=` + `tilt= orb_tilt= orb_orient= orb_ecc= orb_seed= ring=` | — |
| `scan -x -y -z` | `SCAN x= y= z= class= nop= nob= fnv=` | — |
| `find` | star-name lookup helper | — |
| `planet -p N` / `planet-all` | per body `PLANET body=… type=… (desc) owner=… seedval=…` + `rtperiod= rotation= term_start= term_end=`; then hashes **`surface_map`** (46080), **`atmo_overlay`** (32400), **`palette64`** (192; from `tmppal[base3..base3+192]`, base3 = is_moon ? 384 : 576). `planet-all` = `planet` for every body (full blocks incl. seedval). | `surfmap.bin` (64800 = 360×180), `atmover.bin` (32400) |
| `sector -p N -lon -lat` | `SECTOR body= type= lon= lat= sctype= albedo= night=` + `global_surface_seed=`; hashes **`heightmap`** (40000), **`objectchart`** (40000); then the **gap line** `gap len=16 <32hex>` (16 bytes read from `p_surfacemap[40000..40016]` — C-only output). | `height.bin` (40000), `objects.bin` (40000) |
| `surftex -p N -lon -lat` | `SURFTEX body= type= lon= lat= sctype= albedo=` + `T_SCALE= quartz= groundflares= gtx=` + `rockscaling= rockpeaking= rockdensity= treescaling=`; hashes **`surf_texture`** (65024), **`sky_texture`** (46080). | `surftex.bin` (65024 = 256 wide × 254 tall), `sky.bin` (64800 = 360×180) |

C specifics:

- `txtr` is **aliased onto `p_background`** (allocated 65552 bytes; the surface
  map is the first 64800 bytes of it). `surf_texture` hashes `p_background[0..65024]`.
- Per-type surftex overrides: types **1/4/7 → `sky_brightness = 0, atmosphere = 0`**;
  **type 2 → `sky_brightness = 63 - nightzone * 31`**. `s_background` is filled with
  `sky_brightness`, then `create_sky(atmosphere)` runs.
- `-o <file>` writes all text output to a file; without it, output goes to the console.

## 2. Rust (`tests/nivgen/target/release/nivgen`)

Commands: `selftest | system | scan | planet | planet-all | sector | surftex`
(plus `fill-db`, feature-gated `db`). Binary dumps via the **env var
`NIVDUMP=<DIR>`** (uppercase filenames written into DIR).

| Command | Text output (hash labels) | Binary dumps (NIVDUMP) |
|---|---|---|
| `system -x -y -z [--json]` | same `=== SYSTEM ===` block as C (STAR / identity / class / ray / spin / rgb / nop / nob / labeled + BODY lines). `--json` emits `{"nob","nop","bodies":[{index,type,owner,moonid}]}` | — |
| `scan` | `SCAN … fnv=` (same as C) | — |
| `planet -p N` | `PLANET body=… type=… (desc) owner=… seedval=…` + rtperiod etc. + `surface_map` / `atmo_overlay` / `palette64` hashes (same labels) | `surface.raw` (planet; **65536** = whole p_background) or `moonsurf.raw` (moon; s_background), `palette.raw` (768), `atmo.raw` (32400) |
| `planet-all` | `=== PLANET TEXTURES (all N bodies) ===` then **compact** lines: `PLANET n type= t is_moon= m surf= atmo= pal=` — **no seedval, no block labels, no dumps** | — |
| `sector -p N -lon -lat [-gap HEX]` | `SECTOR body= type= lon= lat= sctype= albedo= night=` + `global_surface_seed=`; `heightmap` (40000), `objectchart` (40000); **[TRACE]/[GAP] debug lines go to stderr** | `HEIGHT.bin` (40000), `OBJECTS.bin` (40000) |
| `surftex -p N -lon -lat [-gap HEX]` | `SURFTEX …` + `surf_texture` (65024), `sky_texture` (46080) — same labels as C | `SURFTEX.bin` (65536 = 256×256 full buffer), `SKY.bin` (64800), `palette.raw` (768), `SURFTEX_BEFORE.bin` (65536) |

Rust specifics:

- `p_background` / `s_background` are both **65536**; the surface map is the
  first 64800 bytes of `p_background`; surftex is all 65536. Same logical data
  as C, different buffer size.
- Takes **`-gap <32hex>`** as an *input* to sector/surftex (must replay the C
  gap) — it does not emit a gap line.
- Seedval comes from `surface_seedval_for(body)` (reported by `planet`, not
  `planet-all`).
- Same per-type surftex overrides as C; sky gradient written to `tmppal`
  entries 64..128 (bytes 192..384), ground gradient entries 0..63 (bytes 0..192).

## 3. Parity rules (what must match between engines)

- FNV-1a (and CRC-32) over the same byte ranges; hashline labels must be
  identical strings.
- Byte contents of `surface_map`, `atmo_overlay`, `palette64`, `heightmap`,
  `objectchart`, `surf_texture`, `sky_texture` must be **identical** across
  engines. This is the verification criterion (reference: 10686/10686 surface,
  9321/9322 sectors, 74/74 sky).
- The **gap** (16 bytes) is produced by C, consumed by rust (`-gap`).
- Palette slice semantics for rendering:
  - surface map → `tmppal[576..768]` (planet) / `tmppal[384..576]` (moon);
  - surftex → ground gradient `tmppal[0..192]`;
  - sky → sky gradient `tmppal[192..384]`;
  - all 6-bit values scaled ×4.05 to 8-bit for display.

## 4. Sheet integration contract (per planet, per engine)

Stored in the `nivgen_planets` sheet (SheetBot), one row per planet:

- Identity: `type`, `is_moon`, `seedval`, `rand_lon` / `rand_lat`
  (`lon = abs(round(seedval)) % 360`,
  `lat = 1 + ((abs(round(seedval)) // 1000) % 119)`).
- Hashes: `surf`, `atmo`, `pal`, `sect_def_hm`, `sect_def_oc`, `sect_rand_hm`,
  `sect_rand_oc`, `sect_def_stex`, `sect_def_sky`, `sect_rand_stex`,
  `sect_rand_sky` (plus C-only `sect_def_gap` / `sect_rand_gap`).
- Rendered artefacts: surface PNG (360×180), surftex PNG
  (256×254 / 256×256), sky PNG (360×180), all palette-applied.

## 5. Must-haves for `noctis-iv-lr`

1. The four hash families at the exact byte lengths: surface
   (`surface_map` 46080, `atmo_overlay` 32400, `palette64` 192), sector
   (`heightmap` 40000, `objectchart` 40000), surftex (`surf_texture` 65024),
   sky (`sky_texture` 46080).
2. Compact `PLANET n type=… is_moon=… surf=… atmo=… pal=…` output for
   `planet-all`, plus the full `planet` block (with `seedval`) for
   per-body metadata.
3. The `-gap <32hex>` input contract, so C/orig gaps can be replayed.
4. Identical hashline labels and byte ranges as the two reference engines.
