# Noctis IV Plus — WebAssembly demo

A single-page demo that compiles the deterministic universe generator
(`tests/nivgen`) to WebAssembly and renders every stage of the pipeline in
the browser — star systems, planet surface maps, landing sectors, sky
textures and the raw Borland RNG.

No wasm-bindgen and no build tooling beyond `cargo` + the wasm target: the
Rust side exports a plain C ABI and the JS side slices the output buffers
straight out of WASM linear memory.

## Run it

```bash
cd web
python3 -m http.server 8080     # or any static server
# open http://localhost:8080
```

`file://` will not work — the page fetches `nivgen.wasm`.

## Rebuild the wasm

```bash
cd tests/nivgen

# 1. make the db feature (rusqlite) opt-out so it does not get compiled
cargo build --release --target wasm32-unknown-unknown --lib --no-default-features

# 2. copy into the web demo
cp target/wasm32-unknown-unknown/release/nivgen.wasm ../../web/nivgen.wasm
```

## What it demonstrates

| Panel | What it shows |
|-------|---------------|
| 1. Parsis coordinates | x / y / z input, named-star presets from the verification DB |
| 2. Star system | star class/colour/ray/spin/identity, orbit map, planet & moon table |
| 3. Planet surface | 360×180 equirectangular surface map, atmosphere overlay, 64-colour VGA palette, FNV-1a hashes |
| 4. Landing sector | 200×200 heightmap + object chart, 256×256 surface texture, 360×180 sky, environment readout, FNV hashes |
| 5. RNG lab | drive the Borland `rand()` / Noctis `fast_random()` sequences and verify the canonical vectors |

The FNV-1a hashes match the canonical C fixture hashes in `tests/db/noctis.db`
byte-for-byte (same `SAMPLED_BYTES` / 65532 surftex conventions as the C
harness), so the page doubles as a determinism proof.

## Architecture

- `nivgen.wasm` — raw `cdylib` export from `tests/nivgen/src/wasm.rs`
- `demo.js` — instantiation + all canvas rendering
- `index.html` / `demo.css` — page layout

The engine keeps one global `SystemGen` alive across calls
(`niv_generate` → `niv_planet_surface` → `niv_sector` → `niv_sky`). Note that
`p_background` is shared between the planet surface map and the 256×256
surface texture (as in the original game), so the JS side copies each buffer
out of linear memory before the next generation step overwrites it.
