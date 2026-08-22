# Noctis IV Plus

This is the home of Noctis IV Plus, a basic modification of Alessandro Ghignola's excellent [Noctis IV](https://80.style/#/hsp/noctis_iv/noctis_iv_download_JmsLdos_onlyK) space exploration simulator.

![](gallery/SAMPLE.BMP)

For a list of changes, see [source/NIVPLUS_CHANGES.TXT](source/NIVPLUS_CHANGES.TXT)

For the manual, see [manual/noctis_iv_manual.html](manual/noctis_iv_manual.html)

## Deterministic Rust port (universe generator)

This repository also contains a **100% standalone Rust reimplementation** of the
game's universe generator (`tests/nivgen`). It derives every planet, moon,
star, surface, sector and sky texture purely from the Parsis coordinates and
the Borland RNG — no C code, no fixtures, no DOSBox. The output is verified
byte-for-byte against canonical hashes captured from the original game.

```
SystemGen::generate(x, y, z)
  ├─ star class, body count, planet types, orbits ← Borland LCG
  ├─ per-body: surface map (360×180), atmosphere, palette
  └─ per-body per-landing: sector heightmap (200×200) + object chart
       └─ surftex (256×256) + sky (360×180) for the landing view
```

### Modules

| Module | Purpose |
|--------|---------|
| `tests/nivgen/src/rng.rs` | Borland `rand()` / `srand()` and `fast_random()` / `fast_srand()` |
| `tests/nivgen/src/system.rs` | Star system generation (planet count, types, orbits) |
| `tests/nivgen/src/surface.rs` | 360×180 surface map + atmosphere + palette |
| `tests/nivgen/src/surface_sector.rs` | 200×200 sector heightmap + object chart + surftex/sky |
| `tests/nivgen/src/f80.rs` | Noctis' x87 80-bit float emulation |
| `tests/nivgen/src/wasm.rs` | C-ABI exports for the browser demo |

Key Borland behaviours faithfully replicated: 16-bit `int`/`unsigned`
arithmetic, right-to-left argument evaluation, 32-bit `float` intermediates
with x87 80-bit internal precision, and the `seed & 0xFFFF` srand truncation.

### CLI

```bash
cd tests/nivgen
cargo run --release -- <command> [opts]
```

| Command | What it prints |
|---------|----------------|
| `selftest` | hash vectors, Borland RNG / `fast_random()` sequences, `flandom()` |
| `system -x X -y Y -z Z [--json]` | star class/colour/ray/spin + full body table |
| `scan -x X -y Y -z Z` | condensed per-star FNV hash (system-level) |
| `planet -x X -y Y -z Z -p BODY` | one body's surface/atmosphere/palette FNV hashes |
| `planet-all -x X -y Y -z Z` | all bodies sequentially (state-sharing validation) |
| `sector -x X -y Y -z Z -p BODY -lon LON -lat LAT` | sector heightmap + object chart hashes |
| `surftex -x X -y Y -z Z -p BODY -lon LON -lat LAT` | 256×256 surface texture + sky hashes |
| `fill-db [--db PATH]` | populate the verification DB (needs the `db` feature) |

Set `NIVDUMP=<dir>` to also write raw `.bin` dumps for binary-level diffing.

### Verification pipeline

`tests/db/noctis.db` stores canonical C fixture hashes for **95 stars / 2570
bodies** drawn from the Feltyrion Explorer atlas. It is only a verification
tool — generation itself never touches it.

```bash
uv run python3 tests/db/bulk_sectors.py one_iter        # process next unprocessed star
uv run python3 tests/db/bulk_sectors.py db-report --short  # condensed status
uv run python3 tests/db/bulk_sectors.py cmp <STAR_NAME>    # per-body hash comparison
```

See `AGENTS.md` for the full workflow and hashing conventions
(`SAMPLED_BYTES` = 46080 for surfaces, 65024 for surftex, 32400 for
atmosphere).

### WebAssembly demo

`web/` is a browser demo compiling the generator to WASM (raw C ABI, no
wasm-bindgen). It renders star systems, planet surface maps, landing sectors,
sky textures and an RNG lab, with FNV hashes shown live.

```bash
cd web && python3 -m http.server 8080    # open http://localhost:8080
```

See [web/README.md](web/README.md) for rebuild instructions.

### Current status

- **Surface maps**: 10686/10686 match C (100%)
- **Atmosphere**: 10686/10686 match
- **Palette**: 10685/10686 match (1 known pre-existing TWENDZ variance)
- **Sectors**: 9321/9322 match (1 known r-b* heightmap mismatch)
- **Sky**: 74/74 match · **Surftex**: all type-0 bodies match

The original C source is in `source/`, with documentation in `docs/src/`.

## Getting Noctis IV Plus to run on modern computers

Noctis IV was made for MS-DOS and early Windows versions that still supported 16-bit MS-DOS executables natively, and as such, you'll need to be able to run DOS on your computer. To run Noctis IV plus on modern computers, you have three main options, in order of simplest to most complicated to setup:

1. [DosBOS-X](https://dosbox-x.com/) (all modern operating systems)
2. [DOSEMU](http://www.dosemu.org/) (Linux only)
3. A [FreeDOS](https://www.freedos.org/) or MS-DOS environment running inside of a virtual machine like [VirtualBox](https://www.virtualbox.org/)

The author of Noctis IV Plus recommends DosBOS-X, using the "dynamic_x86" CPU core, simulating a Pentium III 866MHz EB (~407000 cycles). Note that with this setup, a completely empty Stardrifter floating in space may actually give you *lower* frames per second than one that's orbiting a planet in a star system. You may want to play around with the cycle count a bit to find the right number for your computer.

Once you've got an MS-DOS setup running, simply run the following from within the MS-DOS environment:

```batch
cd <directory where NIVPlus is installed>
cd modules
noctis.exe
```

## Compiling

To compile Noctis IV Plus, follow these steps:

1. Get a working MS-DOS setup, as per the above.
2. Install Borland C++ 3.1 for DOS to `c:\bc.31`
3. Run the following from within the MS-DOS environment:
```batch
cd <directory where NIVPlus is installed>
cd source
compile.bat
```
