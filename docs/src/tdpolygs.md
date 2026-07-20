# TDPOLYGS.H - Polygon 3D Rendering Engine

## Overview

TDPOLYGS.H ("The Third Flare") is the primary 3D polygon rendering engine for Noctis. It's a full-3D engine capable of rendering filled color polygons with texture mapping at high speeds.

**Author**: Alessandro Ghignola  
**Date**: Started for Alpha Dream, Last revised December 21, 1997

## Key Features

- Filled polygon rendering (not wireframe)
- Texture mapping support
- Perspective-correct rendering
- Depth culling (back-face removal)
- Frustum clipping
- ~12,000 polygons/second on 75MHz Pentium

## Important Notes

> **CRITICAL**: Color 255 (0xFF) is reserved for polygon boundary marking. Never use this color in the working area when using these functions.

## Core Constants

```c
#define VERTICI_PER_POLIGONO 4    // Max vertices per polygon
#define larghezza 300             // View width
#define altezza 180               // View height
#define x_centro 160             // Screen center X
#define y_centro 100              // Screen center Y

const int MPIX = 319;            // Max pixel X
const int MPIY = 199;            // Max pixel Y
const int TEXTURE_XSIZE = 256;   // Texture width
const int TEXTURE_YSIZE = 256;   // Texture height
```

## Global Variables

### Camera

```c
float alfa = 0, beta = 0, gamma = 0;    // Rotation angles (radians)
float cam_x = 0, cam_y = 0, cam_z = 0;  // Camera position
float dpp = 200;                         // Distance to projection plane
float inv_dpp = 1 / dpp;                 // Pre-calculated inverse
```

### Texture Mapping

```c
long H_MATRIXS = 16;    // Horizontal texture repetitions (in 16ths)
long V_MATRIXS = 16;    // Vertical texture repetitions (in 16ths)
long XSIZE;             // Computed: TEXTURE_XSIZE * H_MATRIXS
long YSIZE;            // Computed: TEXTURE_YSIZE * V_MATRIXS
float XCOEFF = EMU_K / dpp;   // X projection coefficient
float YCOEFF = EMU_K / dpp;   // Y projection coefficient

unsigned char huge *txtr;     // Current texture pointer
unsigned int huge *wtxtr;    // Word-aligned texture (for faster access)
```

### Scanline Table

```c
unsigned riga[200];    // Video memory offsets for each scanline
```

Must be initialized with `initscanlines()` before rendering.

### Depth Limits

```c
float uneg = 100;      // Minimum Z distance (points behind camera)
```

## Core Functions

### Initialization

```c
void initscanlines()
```
Pre-calculates video memory offsets for each scanline (0-199). Must be called before any rendering.

```c
void _control87(MCW_EM, MCW_EM);  // Disables FPU exceptions
```

### Main Polygon Functions

```c
void poly3d(float *x, float *y, float *z, unsigned nrv, unsigned char colore)
```
Renders an N-vertex filled polygon with solid color.

```c
void polymap(float *x, float *y, float *z, char nv, unsigned char tinta)
```
Renders an N-vertex polygon with texture mapping.

### Depth Functions

```c
char getcoords(float x, float y, float z)
```
Checks if a 3D point is visible (within frustum). Returns 1 if visible.

```c
char facing(float *x, float *y, float *z)
```
Performs back-face culling - determines if polygon faces camera. Returns 1 if facing.

### Camera Controls

```c
void change_angle_of_view()
```
Recalculates projection matrices when camera angles (alfa, beta) change.

```c
void change_camera_lens()
```
Recalculates projection when field of view changes.

```c
void change_txm_repeating_mode()
```
Updates texture mapping coefficients when H_MATRIXS or V_MATRIXS change.

### Polygon Helper

```c
void setfx(char mode)
// Sets fill mode: 0=solid, 1=transparent, 2=additive, etc.

void resetfx()
// Restores default fill mode
```

## Texture Mapping Details

The texture system uses "sixteenths" for repetition counting:

| H_MATRIXS | V_MATRIXS | Effect |
|-----------|-----------|--------|
| 4 | 4 | Full texture (1x) |
| 8 | 8 | 2x repetition |
| 16 | 16 | 4x repetition |
| 32 | 32 | 8x repetition |

Example:
```c
H_MATRIXS = 3; V_MATRIXS = 3;  // Quarter texture (0.25x)
change_txm_repeating_mode();
```

## Rendering Pipeline

### 1. Vertex Transformation
```
World Coords → Camera Transform → Projection → Screen Coords
```

### 2. Frustum Culling
- Check against minimum Z (`uneg`)
- Check against screen boundaries

### 3. Back-Face Culling
```c
// Calculate face normal
// Dot product with view vector
// Discard if facing away
```

### 4. Depth Sorting
Polygons are sorted by average Z for correct painter's algorithm rendering.

### 5. Rasterization
- Convert to screen coordinates
- Fill polygon with color or texture
- Apply lighting effects

## Texture Coordinate System

Textures are stored as 256x256 byte arrays (8-bit indexed color).

```c
// Texture pixel access
texture[y * 256 + x]  // Returns color index
```

The engine handles:
- Perspective-correct texture mapping
- Texture wrapping/repetition
- Texture filtering (optional)

## Lighting Support

Polygons can have lighting applied via:

```c
char flares;  // Lighting mode
// 0 = no flares
// 1 = directional light
// 2 = point light
// etc.
```

## Performance Optimizations

1. **Fixed-point math** in inner loops
2. **Inline assembly** for screen writes
3. **Lookup tables** for trig functions
4. **Word-aligned textures** (`wtxtr`) for faster access
5. **Early rejection** of off-screen polygons

## Integration with Other Modules

TDPOLYGS.H works with:
- `FAST3D.H` - Line-based rendering (combined use)
- `TEXT3D.H` - 3D text rendering
- `GFX.H` - 2D primitives
- NOCTIS-1.CPP - Surface/terrain rendering
- NOCTIS.CPP - Vehicle and object rendering

## Common Usage Patterns

### Solid Color Polygon
```c
float vx[4] = {100, 200, 200, 100};
float vy[4] = {50, 50, 150, 150};
float vz[4] = {0, 0, 0, 0};

if (facing(vx, vy, vz)) {
    poly3d(vx, vy, vz, 4, 42);  // Draw with color 42
}
```

### Textured Polygon
```c
txtr = p_surfacemap;  // Set texture
H_MATRIXS = 4; V_MATRIXS = 4;
change_txm_repeating_mode();

polymap(vx, vy, vz, 4, 0);  // Draw with texture, base color 0
```

### Animated Texture
```c
txtr += 48;  // Offset into texture atlas
polymap(vx, vy, vz, 4, base_color);
```

## DOS VGA Considerations

- Uses VGA mode 0x13 (320x200x256)
- Direct video memory writes at 0xA000:0000
- 64KB memory bank limitation handled via bank switching if needed
