# NOCTIS-1.CPP - Planetary Surface Rendering

## Overview

NOCTIS-1.CPP handles the procedural generation and rendering of planetary surfaces, including terrain, vegetation, atmosphere, and weather effects. This module transforms mathematical representations into the 3D landscapes players can explore.

## Terrain System

### Height Map Calculation

```cpp
float hpoint(long px, long pz)
```
Calculates terrain height at a given coordinate using:
- Bilinear interpolation between quadrant heights
- 16,384 intermediate gradations per quadrant side
- Triangle division handling (isosceles triangles)

The surface is divided into 200x200 logical quadrants, each with 128x128 height samples.

### Biome Types

```cpp
char sctype;  // Scenario type
#define OCEAN   1   // Water world
#define PLAINS  2   // Grassy terrain
#define DESERT  3   // Arid landscape  
#define ICY     4   // Frozen world
```

### Terrain Features

#### Mountains and Hills
Generated through fractal noise with configurable:
- `rockscaling` - Base rock size (default: 500)
- `rockpeaking` - Height variation (default: 250)
- `rockdensity` - Grouping frequency (default: 15)

#### Oceans and Water
- Mirror effects via `mirror` flag
- Wave simulation with `waves_in`/`waves_out` flags
- Depth-based coloring

## Vegetation System

### Tree Generation

```cpp
void build_fractal_tree(x, y, z, scaling, reduction, globalwidth, 
                       layers, divisions, distance_from_perfection,
                       rootcolormask, leafcolormask, branchdetail, 
                       isrootnode, occurrence)
```

Recursive fractal tree generation with parameters:
- `scaling` - Trunk height
- `reduction` - Branch length reduction per level
- `layers` - Recursion depth
- `divisions` - Branching factor
- `distance_from_perfection` - Irregularity coefficient

#### Tree Parameters
```cpp
float treescaling    = 4096;   // Base tree size
float treespreads    = 0.75;   // Branch spread
float treepeaking    = 1.25;   // Divergence angle
float branchwidth    = 0.15;   // Branch thickness ratio
float rootheight     = 0.50;   // Trunk height ratio
```

### Ground Cover

```cpp
void greenmush(x, y, z, mask_1, mask_2, scaling, 
              colorgrade, colormask, noseed)
```
Renders distant vegetation as clusters of green pixels, providing depth cue without individual geometry.

## Surface Rendering

### Fragment Processing

The main surface renderer processes terrain in tiles:
1. Calculate view frustum
2. Apply depth culling
3. Determine visible quadrants
4. Render each visible polygon with:
   - Texture mapping
   - Lighting/shading
   - Atmospheric effects
   - Object placement

### Shading System

```cpp
int sh_delta;  // Shading pointer shift
```

Light calculation based on:
- Sun position (`sun_x, sun_y, sun_z`)
- Surface normal
- Atmospheric scattering
- Latitude/exposure angles

## Atmosphere and Weather

### Star Radiation

```cpp
float dsd1, dsd2;    // Distance from primary/secondary stars
float nray1, nray2;  // Star radii
```

### Weather Effects

Atmospheric sounds and effects:
- Wind (various intensities)
- Rain
- Thunder
- Subsonic vibrations

### Eclipse Handling

When planetary shadows occlude stars:
```cpp
float eclipse;  // Shadow intensity (0.0 - 1.0)
```
Affects lighting, temperature readings, and radiation levels.

## Planet Types

```cpp
#define planet_types 10  // Number of planet classes
```

Each type has unique characteristics:
- Surface color palettes
- Temperature ranges
- Atmosphere composition
- Available resources

## Object Placement

### Surface Objects Map

```cpp
struct quadrant {
    unsigned nr_of_objects : 2;
    unsigned object0_class : 2;
    unsigned object1_class : 2;
    unsigned object2_class : 2;
};
```

Object classes:
```cpp
#define ROCKS   0
#define VEGET   1
#define TREES   2
#define NOTHING 3
```

### Random Generation

Using seeded random functions:
```cpp
int detail_seed = 12345;  // Current terrain seed
```

The seed ensures consistent terrain generation for the same coordinates.

## Landing System

### Approach Rendering

When approaching a planet:
1. Render atmospheric entry effects
2. Display surface map preview
3. Show landing coordinates
4. Calculate approach trajectory

### Surface Map Display

```cpp
void show_planetary_map()
```
Renders 48x32 pixel map showing:
- Longitude/latitude grid
- Terrain type colors
- Current position indicator

### Landing Coordinates

```cpp
int landing_pt_lon;  // Landing longitude (0-359)
int landing_pt_lat;  // Landing latitude (1-119)
```

## Environmental Data

Displayed in onboard computer:
- Temperature (Kelvin, Celsius, Fahrenheit)
- Radiation levels
- Lithium ion concentration
- Atmospheric composition

## Surface Mode Controls

| Key | Action |
|-----|--------|
| 0-9 | Movement speed |
| J | Jump |
| L | Land on surface |
| Space | Jetpack |
| C | Disengage jetpack |
| M/* | Screenshot |
| N// | Wide screenshot |
| F3 | Moviemaker |
| Up | Toggle mouselook |

## Constants

```cpp
#define avgmoons    4        // Average moons per planet
#define maxbodies   80       // Max bodies per system (20 * avgmoons)
#define log2avgmoons 2       // For bit operations
```

## Global Variables

### Environment State
```cpp
float latitude;      // Player latitude (0-90)
float exposure;      // Longitude relative to day center
char mirror;         // Water reflection enabled
char waves_in;       // Wave rendering active
long T_SCALE;        // Texture scale for terrain
```

### Physics
```cpp
float base_pp_temp;      // Base surface temperature
float base_pp_pressure;   // Base atmospheric pressure
```

### Rendering
```cpp
char groundflares;    // Ground lighting mode
char treeflares;      // Tree lighting mode
char leafflares;      // Leaf lighting mode
```
