# Noctis IV Plus - File Formats

This document describes the binary file formats used by Noctis IV Plus for saving game state, surface data, and other persistent information.

---

## Current.BIN - Game State

The main save file containing all game state. Location: `..\DATA\Current.BIN`

### Data Structure

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 1 | char | `sync` - Tracking mode (0-5) |
| 1 | 1 | char | `anti_rad` - Radiation protection |
| 2 | 1 | char | `pl_search` - Planet finder active |
| 3 | 1 | char | `field_amplificator` - Starfield amplifier |
| 4 | 1 | char | `ilight` - Internal light power (0-63) |
| 5 | 1 | char | `ilightv` - Light variation direction |
| 6 | 1 | char | `charge` - Lithium amount (grams) |
| 7 | 1 | char | `revcontrols` - Reverse pitch flag |
| 8 | 1 | char | `ap_targetting` - Selecting remote target |
| 9 | 1 | char | `ap_targetted` - Remote target type (0=none, 1=star, -1=coords) |
| 10 | 1 | char | `ip_targetting` - Selecting local target |
| 11 | 1 | char | `ip_targetted` - Local target planet index |
| 12 | 1 | char | `ip_reaching` - Approaching planet |
| 13 | 1 | char | `ip_reached` - At planet |
| 14 | 1 | char | `ap_target_spin` - Target star spin phase |
| 15 | 1 | char | `ap_target_r` - Target star R component |
| 16 | 1 | char | `ap_target_g` - Target star G component |
| 17 | 1 | char | `ap_target_b` - Target star B component |
| 18 | 1 | char | `nearstar_spin` - Current star spin |
| 19 | 1 | char | `nearstar_r` - Current star R |
| 20 | 1 | char | `nearstar_g` - Current star G |
| 21 | 1 | char | `nearstar_b` - Current star B |
| 22 | 1 | char | `gburst` - Emergency beacon |
| 23 | 1 | char | `menusalwayson` - Always show menus |
| 24 | 1 | char | `depolarize` - Polarization state |
| 25 | 2 | int | `sys` - Current system menu |
| 27 | 2 | int | `pwr` - Available power (kilodyams) |
| 29 | 2 | int | `dev_page` - Device menu page |
| 31 | 2 | int | `ap_target_class` - Target star class |
| 33 | 2 | int | `f_ray_elapsed` - Flight ray elapsed |
| 35 | 2 | int | `nearstar_class` - Current star class |
| 37 | 2 | int | `nearstar_nop` - Number of planets |
| 39 | 4 | float | `pos_x` - Position X |
| 43 | 4 | float | `pos_y` - Position Y |
| 47 | 4 | float | `pos_z` - Position Z |
| 51 | 4 | float | `user_alfa` - User pitch |
| 55 | 4 | float | `user_beta` - User yaw |
| 59 | 4 | float | `navigation_beta` - Navigation angle |
| 63 | 4 | float | `ap_target_ray` - Target star radius |
| 67 | 4 | float | `nearstar_ray` - Current star radius |
| 71 | 8 | double | `dzat_x` - Universe X position |
| 79 | 8 | double | `dzat_y` - Universe Y position |
| 87 | 8 | double | `dzat_z` - Universe Z position |
| 95 | 8 | double | `ap_target_x` - Target X |
| 103 | 8 | double | `ap_target_y` - Target Y |
| 111 | 8 | double | `ap_target_z` - Target Z |
| 119 | 8 | double | `nearstar_x` - Near star X |
| 127 | 8 | double | `nearstar_y` - Near star Y |
| 135 | 8 | double | `nearstar_z` - Near star Z |
| 143 | 8 | double | `helptime` - Help request time |
| 151 | 8 | double | `ip_target_initial_d` - Approach distance |
| 159 | 8 | double | `requested_approach_coefficient` |
| 167 | 8 | double | `current_approach_coefficient` |
| 175 | 8 | double | `reaction_time` |
| 183 | 11 | char[11] | `fcs_status` - Status message |
| 194 | 2 | int | `fcs_status_delay` |
| 196 | 2 | int | `psys` - Previous system |
| 198 | 8 | double | `ap_target_initial_d` |
| 206 | 8 | double | `requested_vimana_coefficient` |
| 214 | 8 | double | `current_vimana_coefficient` |
| 222 | 8 | double | `vimana_reaction_time` |
| 230 | 1 | char | `lithium_collector` - Collection active |
| 231 | 1 | char | `autoscreenoff` - Auto screen off |
| 232 | 1 | char | `ap_reached` - At target |
| 233 | 1 | char | `lifter` - Jetpack active |
| 234 | 8 | double | `secs` - Time seed |
| 242 | 2 | int | Extended state... |

**Total size**: ~245 bytes (variable)

---

## Surface.BIN - Planetary Surface

Save file for planetary surface state. Created during landing, deleted on takeoff.

### Data Structure

| Offset | Size | Type | Range | Description |
|--------|------|------|-------|-------------|
| 0 | 2 | int | 0-359 | Longitude of landing point |
| 2 | 2 | int | 1-119 | Latitude (60 = equator) |
| 4 | 4 | int | 0-3276800 | Capsule X (aligned to 16384) |
| 8 | 4 | int | 0-3276800 | Capsule Z |
| 12 | 4 | int | 0-16384 | X within sub-quadrant |
| 16 | 4 | int | 0-16384 | Z within sub-quadrant |
| 20 | 4 | float | 0-3276800 | Player X position |
| 24 | 4 | float | ~-260000-0 | Player Y (0 = sea level) |
| 28 | 4 | float | 0-3276800 | Player Z position |
| 32 | 4 | float | -45 to +45 | Alpha (yaw) angle |
| 36 | 4 | float | -180 to +180 | Beta (pitch) angle |
| 40 | 2 | int | -5, 0, +5 | Radiation visor animation delta |
| 42 | 2 | int | 0-170 | Radiation visor animation count |
| 44 | 1 | char | 0-1 | Visor: 0=open, 1=closed |

**Total size**: 45 bytes (fixed)

---

## STARMAP2.BIN - Star Map (NoctisMapper format)

The NoctisMapper starmap format. Source of truth for star name, class, and Parsis coordinates.

### File Structure

- No file header
- Fixed-length entries, 44 bytes each

### Entry Format

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 4 | int32 | `star_x` - Parsis X coordinate |
| 4 | 4 | int32 | `star_y` - Parsis Y coordinate (engine-facing; negate for user-facing) |
| 8 | 4 | int32 | `star_z` - Parsis Z coordinate |
| 12 | 4 | int32 | `index` - 0 for stars, >0 for planets/moons |
| 16 | 4 | int32 | `body_type` - 0 for stars, planet type for bodies |
| 20 | 20 | char[20] | `name` - Star or body name (space-padded, no null terminator) |
| 40 | 4 | char[4] | `type_str` - `" S##"` for stars (class), `" P##"` for bodies (number) |

### Star Entry

- `index` == 0
- `body_type` == 0
- `type_str` format: `" S"` followed by two-digit class (e.g. `" S00"`, `" S05"`)

### Coordinate Convention

- File stores **engine-facing** Y (starmap convention: negated from galactic chart)
- To get user-facing Y (what the player sees on the galactic chart): `user_y = -engine_y`
- The C harness negates at the boundary: `ap_target_y = -y` (where `y` is user-facing CLI input)
- The Rust CLI negates at the boundary: `EngineY::from_user_y(y)`
- The DB column `y_user` stores **user-facing** Y

### Example

```
Entry for BALASTRACKONASTREYA:
  x = -18928, y = -29680 (engine), z = -67336
  user-facing: (-18928, 29680, -67336)
  class = S00
```

---

## GUIDE.BIN - Navigation Guide

Additional navigation and reference data (~4MB).

---

## SUPPORTS.NCT - Embedded Resources

Resources embedded in the executable. Contains:

| Resource | Offset | Size | Description |
|----------|--------|------|-------------|
| DIGIMAP2 | -60776 | 9360 | Digit map for display |
| MAMMAL | -51416 | ~25768 | Mammal sprite |
| BIRDY | -36784 | ~1002 | Bird sprite |
| VEHICLE | -35782 | ~5808 | Vehicle model |
| HEADER_BMP | -29980 | ~54 | BMP header |
| OFFSETS_MAP | -29926 | ~7340 | QT-VR offsets |
| GLOBES_MAP | -22586 | ~22586 | Globe textures |

(Offsets are negative from end of file)

---

## GOES Net Files

### GOESOUTPUT.TXT

Module output displayed on onboard computer:
- 21 characters × 7 lines
- Scrolling via arrow keys

### COMM.BIN

Communication between game and modules:

| Size | Content |
|------|---------|
| 2 bytes | Planet target (int16) |
| 24 bytes | Remote target (3× float64) |

---

## Palette Files

### Format

Raw 768-byte palette:
```
[R0,G0,B0, R1,G1,B1, ..., R255,G255,B255]
```

Each component: 0-63 (6-bit DAC values)

### Text Format (for comparison)

```
000:  63  63  63
001:  59  59  59
...
```

---

## Texture Files

### Raw Format

Raw binary texture data:
- `st_bytes`: 64,800 bytes (starfield)
- `pl_bytes`: 65,552 bytes (planet surface)
- `ps_bytes`: 40,000 bytes (terrain heightmap)

### Structure

```
Pixel data: [y * 256 + x] = color_index
```

---

## Save/Load Process

### Freeze (Save)

```c
void freeze() {
    fh = _creat("Current.BIN", 0);
    _write(fh, &sync, 245);
    _write(fh, &gnc_pos, 1);
    _write(fh, &goesfile_pos, 4);
    _write(fh, goesnet_command, 120);
    // ... more fields
    _close(fh);
}
```

### Unfreeze (Load)

```c
void unfreeze() {
    fh = _open("Current.BIN", 0);
    if (fh > -1) {
        _read(fh, &sync, 244);
        // ... restore state
        _close(fh);
    }
}
```

---

## CLI Export Formats

### comparison.json

```json
{
  "seed": 1234567890,
  "detail_seed": 12345,
  "nearstar_id": 12345000,
  "nearstar_class": 3,
  "nearstar_nop": 5,
  "palette_size": 768,
  "starfield_size": 64800,
  "planet_size": 65552,
  "terrain_size": 40000
}
```

### seed_info.txt

```
Random seed info:
-----------------
secs (time): 1234567890
detail_seed: 12345
nearstar_identity: 12345000

To reproduce this universe:
1. Use secs value as initial seed
2. Call srand(1234567890)
3. Generate star system in same order
```
