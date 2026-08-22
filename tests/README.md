# Noctis IV Plus — Universe Generation Test Harness

This directory holds a harness that compiles the **unmodified** Noctis IV Plus
generation code (Borland C++ 3.1 for DOS) and emits stable hashes of the
generated universe data, so the generator can later be ported to other
languages bug-for-bug.

## Layout

```
tests/
  harness/        test-only C/C++ that wraps the real generation code
    NIVHASH.H/.C  FNV-1a + CRC32 (verified against zlib reference vectors)
    NIVSTUBS.C    stub symbols NOCTIS.CPP would provide (not linked)
    NIVTEST.CPP   the harness: selftest/system/scan/find/planet/sector/surftex
  dosbox/
    BUILD.BAT     compile NOCTIS-0/1 + harness, link NIVTEST.EXE
    LINK.RSP      TLINK response file (large model, FP87/MATHL/CL libs)
    RUN.BAT       runs all 5 test sets, writes .OUT hash files
  scripts/
    fetch-toolchain.sh   download + reconstruct Borland C++ 3.1 (archive.org bcpp31)
    run-niv-tests.sh      macOS wrapper: builds in DOSBox (Apple container) + copies results
  results/        committed .OUT hash reference fixtures
  niv-toolchain/  (git-ignored) assembled Borland C++ 3.1 toolchain
```

The real generation sources (`source/NOCTIS-0.CPP`, `source/NOCTIS-1.CPP` and
their headers) are copied verbatim into the build dir; they are **not** modified.
The only addition is an `extern double nearstar_p_orb_seed[maxbodies];`
declaration in the harness, since that variable is defined but not declared in
the original `NOCTIS-0.H`.

## Test sets

1. **Solar systems** — `system` for one representative star of each class 0..11.
2. **Planet textures** — `planet` for one body of each planet type 0..9.
3. **Sector heightmaps** — `sector` (`build_surface`) across a few classes.
4. **Surface textures** — `surftex` (`surface`) for planet, gas giant, moon.
5. **Extra** — `scan` compact fingerprints (incl. the degenerate 500,-600,700
   origin) plus the `selftest` RNG/CRC self-check that gates the whole suite.

## Run

```sh
tests/scripts/run-niv-tests.sh            # uses tests/niv-toolchain (fetched if absent)
tests/scripts/run-niv-tests.sh --dump     # also copy raw .DMP binaries
```

Requires the Apple `container` CLI, `curl`, and network access for the first
toolchain fetch (archive.org `bcpp31`). The harness runs under plain `dosbox`
0.74-3 inside a `debian:bookworm-slim` Linux/arm64 container with
`SDL_VIDEODRIVER=dummy`.

## Determinism

The generator is a pure function of `(x, y, z)`. `cmd_find` can rediscover
coordinate fixtures (`find -class N`, `find -ptype T`) if the suite needs to be
extended. Reference hashes: FNV-1a and CRC32 (MSB-first, poly 0x04C11DB7,
reflected I/O) — see `NIVHASH.C` and the `selftest` vector block.
