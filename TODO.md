# TODO

## Done
- [x] crater (4 bugs: crays per-angle, 2nd block per-angle, borrow check, 16-bit px/py)
- [x] F80 add (exponent alignment, carry check)
- [x] fracture (f64 angle init, 16-bit py truncation)
- [x] psmooth_grays (packed 4-row dword average)
- [x] surf_case3 felisian (cx counter, imul low-word, word write)
- [x] terminator (16-bit di wrap)
- [x] shade (F80 gradient accumulation)
- [x] f64_to_u32_trunc (i64→u32, not rem_euclid)
- [x] type 2 = 100% (27/27)
- [x] type 9 = 100% (18/18)
- [x] BCPP 3.1 toolchain from archive.org
- [x] DOSBox-X local builds (`--local` flag)
- [x] SQLite test DB (15 stars, 279 planets, canonical hashes)
- [x] Compare script (`compare.py`)

## Type status (111/279 = 40%)
| Type | Match | Blocker |
|------|-------|---------|
| 2 | 100% | — |
| 9 | 100% | — |
| 3 | 71% | 4 moon bodies (INIX I #16, etc.) — sda loop edge case |
| 8 | 47% | permanent_storm edge cases |
| 4 | 46% | fracture moon bodies |
| 1 | 36% | FENIA moon bodies — crater surface diffs |
| 7 | 30% | fracture edge cases |
| 0 | 0% | contrast (F80/x87) |
| 5 | 0% | contrast (F80/x87) |
| 6 | 0% | wave() memory corruption |

## Remaining work

### contrast (types 0, 5)
The contrast function uses Borland x87 float operations. Our F80 software
emulation has ~2700 ULP diffs per body. No pure-Rust approach matches x87
at the ULP level without SoftFloat3 (which didn't compile cleanly).

Options:
- [ ] Build Berkeley SoftFloat3 as a static C library, FFI from Rust
- [ ] Port SoftFloat3 `extF80_add/sub/mul/lt` to Rust manually
- [ ] Accept F80 diffs and fix cascade (rng save/restore around randoface)

### moon bodies (types 1, 3, 4)
Many remaining diffs are moon bodies. The moon buffer swap is implemented
correctly (`gen_planet_surface` swaps p_background ↔ s_background).

- [ ] Compare `rndpat` output for moon bodies (initial s_background state)
- [ ] Compare C's farmalloc garbage vs Rust's zeroed Vec for s_background
- [ ] Verify moon seedval computation matches C exactly
- [ ] Check that `report_planet` reads s_background for moons and p_background for planets

### SoftFloat3 integration
The reference x87 softfloat used by QEMU. Contains `extF80_add`, `extF80_sub`,
`extF80_mul`, `extF80_lt`, `f64_to_extF80`, `extF80_to_f64`.

- [ ] Fix the SoftFloat3 build (template system, platform.h, specialization)
- [ ] Create a Rust FFI wrapper
- [ ] Replace F80 contrast with SoftFloat3
- [ ] Verify bit-exact output against BCPP 3.1

### DB improvements
- [ ] Store Rust hashes for ALL 279 planets (only ~40% populated)
- [ ] Add `planet_name` column (from starmap2.bin)
- [ ] Add moondar/star flags for moon filtering
- [ ] Add type 10 companion star handling
- [ ] Auto-generate PNGs for visual comparison
