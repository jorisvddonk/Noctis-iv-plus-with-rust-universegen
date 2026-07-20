# NOCTIS-0.CPP - Random Number Generation

## Overview

Noctis uses custom fast random number generation for procedural universe creation. The standard C `rand()` is too slow for real-time generation, so custom implementations are used.

## Functions

### `fast_srand()`

```c
extern void fast_srand(long seed);
```

Initializes the fast random number generator with a seed.

**Parameters:**
- `seed` - Initial seed value

**Usage:**
```c
fast_srand(secs);  // Seed with current time
fast_srand(nearstar_identity);  // Seed with star position
```

### `fast_random()`

```c
extern long fast_random(long mask);
```

Returns a random number with the given bit mask.

**Parameters:**
- `mask` - Bit mask for range (e.g., 31 for 0-31)

**Returns:**
- Random value from 0 to mask

**Usage:**
```c
int value = fast_random(255);  // 0-255
int planet_type = fast_random(9);  // 0-9 (planet types)
```

### `ranged_fast_random()`

```c
extern int ranged_fast_random(int range);
```

Returns a random value in a specific range.

**Parameters:**
- `range` - Maximum value (exclusive)

**Returns:**
- Random value from 0 to range-1

### `flandom()`

```c
extern float flandom(void);
```

Returns a floating-point random value.

**Returns:**
- Random float from 0.0 to 1.0

**Usage:**
```c
float height = flandom() * 1000.0;
float temperature = -200 + flandom() * 400.0;  // -200 to 200
```

## Seed Sources

The game uses various seed sources for deterministic universe generation:

### Time-based
```c
fast_srand(secs);        // Current time
fast_srand(clock());     // Clock ticks
```

### Position-based
```c
fast_srand((long)nearstar_x % 10000);
fast_srand((long)nearstar_y % 10000);
fast_srand((long)nearstar_z % 10000);
```

### ID-based
```c
fast_srand(ap_target_id);  // Star/planet unique ID
```

## Deterministic Generation

The key to reproducible universes is seeding based on position:

```c
// Generate star system at specific coordinates
double star_id = x / 100000 * y / 100000 * z / 100000;
fast_srand((long)star_id);

// Now all random calls will produce the same results
int planet_type = fast_random(10);  // Same every time
```

## Comparison with Standard Library

| Function | Standard | Noctis Fast |
|----------|----------|--------------|
| Seed | `srand()` | `fast_srand()` |
| Range | `rand() % n` | `fast_random(n-1)` |
| Float | `(float)rand()/RAND_MAX` | `flandom()` |
| Speed | ~100 cycles | ~10 cycles |

## Internal Implementation

The fast random uses a linear congruential generator (LCG):

```c
// Typical implementation pattern
static long rng_state;

void fast_srand(long seed) {
    rng_state = seed;
}

long fast_random(long mask) {
    rng_state = rng_state * 1103515245 + 12345;
    return (rng_state >> 16) & mask;
}
```

This is similar to the ANSI C `rand()` implementation but stripped down for speed.

## Usage in Universe Generation

### Planet Properties
```c
fast_srand(nearstar_identity + planet_index);

nearstar_p_type[planet_index] = fast_random(10);     // Planet type
nearstar_p_ray[planet_index] = 100 + fast_random(500); // Radius
nearstar_p_atmosphere[planet_index] = fast_random(64); // Atmosphere
```

### Terrain Generation
```c
fast_srand(detail_seed);  // Seed for terrain

// Each call produces deterministic result for same seed
int mountain_height = fast_random(1000);
```

### Color Generation
```c
fast_srand(star_position);

star_r = fast_random(64);
star_g = fast_random(64);
star_b = fast_random(64);
```

## CLI Integration

For universe reproduction, the seed must be exported:

```c
// In export code
fprintf(f, "seed: %ld\n", (long)secs);
fprintf(f, "detail_seed: %d\n", detail_seed);
```

To reproduce:
1. Capture `secs` value from original game
2. Call `fast_srand(secs)` before generating
3. Universe will be identical
