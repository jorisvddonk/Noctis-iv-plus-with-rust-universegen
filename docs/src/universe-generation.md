# Universe Generation - Star Systems

## Overview

Noctis generates procedurally created star systems using deterministic random number generation. Given the same seed, the universe is always identical.

## Key Concepts

### PARSIS Coordinates

The game uses PARSIS (Parallax Autonomous Reference System for Interstellar Surveys) as its universal coordinate system:

- **X, Y, Z**: Floating-point coordinates in space
- **Scale**: 1 unit ≈ 1 meter
- **Origin**: Arbitrary reference point

### Identity Codes

Each celestial body has a unique identity derived from its position:

```c
double star_id = x / 100000 * y / 100000 * z / 100000;
double planet_id = nearstar_identity + planet_index + 1;
```

## Star System Generation

### Initial Seed

The universe seed is derived from current time:

```c
secs = (long)time(NULL);  // Seconds since epoch
fast_srand(secs);          // Initialize RNG
```

### Nearstar Structure

When approaching a star, the system generates:

```c
double nearstar_x, nearstar_y, nearstar_z;  // Star position
int nearstar_class;                         // Star class (0-11)
int nearstar_nop;                          // Number of planets
float nearstar_ray;                        // Star radius
char nearstar_r, nearstar_g, nearstar_b;   // Star color
```

### Star Classes

| Class | Name | Color Range | Temperature |
|-------|------|-------------|-------------|
| 0 | Blue Giant | Blue | Hot |
| 1 | White | White | Hot |
| 2 | Yellow | Yellow | Medium |
| 3 | Orange | Orange | Warm |
| 4 | Red Dwarf | Red | Cool |
| 5 | Brown Dwarf | Dark Red | Very Cool |
| 6 | Proto-Star | Orange-Brown | Forming |
| ... | ... | ... | ... |
| 11 |的特殊 | Various | Variable |

## Planet Generation

### Per-Planet Properties

```c
nearstar_p_type[planet]      // Planet type (0-9)
nearstar_p_ray[planet]     // Radius
nearstar_p_owner[planet]   // Parent planet (-1 if star)
nearstar_p_moonid[planet] // Moon ID
nearstar_p_atmosphere[planet]  // Atmosphere type
```

### Planet Types

| Type | Description |
|------|-------------|
| 0 | Molten/Rocky |
| 1 | Desert |
| 2 | Terrestrial |
| 3 | Ocean |
| 4 | Gas Giant |
| 5 | Ice Giant |
| ... | ... |

### Generation Algorithm

```c
// Seed with star position + planet index
fast_srand(nearstar_identity + planet_index);

// Generate deterministic properties
type = fast_random(10);
radius = 100 + fast_random(1000);
atmosphere = fast_random(64);
```

## Atmosphere Effects

### Sky Color

```c
surlight = nearstar_r + nearstar_g + nearstar_b;  // Light intensity
dsd = distance_to_star;  // Distance from star
```

### Weather Systems

```c
if (sctype == OCEAN) {
    // Generate waves
    waves_in = fast_random(2);
}
if (sctype == PLAINS) {
    // Generate vegetation
    treescaling = fast_random(4096);
}
```

## Surface Terrain

### Height Map

Surface terrain uses height maps stored in `p_surfacemap`:

```c
// Each quadrant has 128x128 height samples
// Value: 0-255 (0 = lowest, 255 = highest)
height = -((long)p_surfacemap[quadrant] << 11);
```

### Biome Selection

```c
switch (nearstar_p_type[planet]) {
    case 0: // Molten - lava, rocks
        groundflares = 0;
        rockscaling = 800;
        break;
    case 1: // Desert - sand dunes
        groundflares = 0;
        rockscaling = 200;
        break;
    case 2: // Terrestrial - grass, trees
        groundflares = 1;
        treescaling = 4096;
        break;
    case 3: // Ocean - water, waves
        mirror = 1;
        waves_in = 1;
        break;
    case 4: // Icy - ice, snow
        groundflares = 2;
        rockscaling = 300;
        break;
}
```

## Star Field Background

### Background Stars

The star field uses a different seed from the current system:

```c
fast_srand(secs);  // Reset to universe seed

// Generate distant background stars
for (int i = 0; i < 1000; i++) {
    star_x = fast_random(1000000);
    star_y = fast_random(1000000);
    star_z = fast_random(1000000);
    star_class = fast_random(star_classes);
}
```

### Magnitude

Stars are placed at various distances:

```c
distance = 50000 + fast_random(500000);
```

## Reproduction

### Exporting Seed Information

To reproduce a universe:

```c
// In CLI export
fprintf(f, "secs: %ld\n", (long)secs);
fprintf(f, "nearstar_identity: %.0f\n", nearstar_identity);
```

### Reproducing Generation

```c
// Load saved game state
unfreeze();

// Or manually:
secs = saved_secs;
fast_srand(secs);

// Regenerate - will be identical
generate_star_system();
```

## Star Map Database

### STARMAP.BIN

The game maintains a database of labeled stars:

```c
// Search for star by ID
long search_id_code(double star_id, char type);

// Label a star
long star_label_pos = tell(sm_handle);
_write(sm_handle, &star_id, 32);
```

### Consolidation

The first 4 bytes of STARMAP.BIN contain a consolidation marker:

```c
_read(smh, &sm_consolidated, 4);
if (sm_consolidated == 0) {
    // Database needs rebuilding
    lseek(smh, 0, SEEK_SET);
    sm_consolidated = filelength(smh);
    _write(smh, &sm_consolidated, 4);
}
```

## Time and Space

### Cosmic Time

```c
double secs;  // Global time counter
```

Increments during gameplay. Used for:
- Planet orbital positions
- Star rotation
- Animation timing

### Position Tracking

```c
double dzat_x, dzat_y, dzat_z;  // Current ship position
double ap_target_x, ...;        // Autopilot target
```

## CLI Export for Verification

For comparing universe generation between implementations:

```json
{
  "seed": 1234567890,
  "nearstar_identity": 12345000,
  "nearstar_class": 3,
  "nearstar_nop": 5,
  "planets": [
    {"type": 2, "radius": 500},
    {"type": 3, "radius": 1200},
    ...
  ]
}
```
