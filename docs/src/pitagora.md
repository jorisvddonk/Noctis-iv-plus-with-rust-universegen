# PITAGORA.H - Triangle 3D Engine

## Overview

PITAGORA.H is an advanced 3D rendering engine based on "intelligent triangles." It's designed for high-performance polygon rendering with texture mapping support.

**Author**: Home Sweet Pixel  
**Date**: 1999  
**Purpose**: Born for Noctis - a space simulation program

## Philosophy

The core idea is using **only 3D triangles** to render everything. This allows:
- Massive optimization opportunities
- Reduced CPU usage
- Very fast rendering (~500,000 triangles/second on Pentium)

## Key Features

- Triangle-based rendering (not quads or arbitrary polygons)
- Texture mapping with perspective correction
- Multiple fill modes (solid, textured, gradient, brilliant)
- VESA/VGA support
- Scanline-based rendering with span optimization
- Optimized for MCGA (320x200x256)

## Video System

### Video Memory

```c
unsigned char huge *display = (unsigned char huge *) 0xA0000000;
unsigned char huge *vid_map = (unsigned char huge *) 0xA0000000;
```

### VESA Support

```c
char openvesa(unsigned mode_code)
char video_page_select(char videopage)
void closevesa()
```

Supports VESA modes:
- `LORES565` (0x10E) - 640x200 direct color
- `HIRES565` (0x111) - 2048x768 direct color

## Camera System

### View Parameters

```c
double alpha=0, beta=0, gamma=0;  // Pitch/yaw/roll
double cam_x=0, cam_y=0, cam_z=0;  // Position
double dpp=100;                     // Distance to projection plane
```

### Movement

```c
void forward(double delta)
// Moves viewpoint by delta units in facing direction

void force(double delta)
// Calculates movement without applying (for physics)
```

### Angle Updates

```c
void change_angle_of_view()
// Recalculates optimized trig tables when angles change

void change_camera_lens(double DPP)
// Adjusts field of view (perspective)
```

## Texture System

### Parameters

```c
const TXTR_XSIZE = 256;    // Texture width
const TXTR_YSIZE = 128;    // Texture height
long H_MATRIXS = 16;       // Horizontal repeats (16ths)
long V_MATRIXS = 16;       // Vertical repeats (16ths)
```

### Changing Texture Scale

```c
void change_txm_repeating_mode(long h_matrixs, long v_matrixs)
// H_MATRIXS/V_MATRIXS 16ths ( in16 = 1x, 32 = 2x, etc.)
```

## Fill Modes

The engine supports multiple rendering modes controlled by `mapeffect`:

| Mode | Value | Description |
|------|-------|-------------|
| PLAIN | 0 | Solid color fill |
| TEXTURE | 8 | Texture mapped |
| GRADIENT | 64 | Color gradient |
| BRILLIANT | 128 | Additive lighting |
| BACKWARD | 4 | Fill direction |

### Fill Router Table

```c
unsigned long filling_router[allocated_codes] = {
    plain,           // 0x00
    forw_texture,   // 0x08
    back_texture,    // 0x0C
    forw_gradient,  // 0x48
    back_gradient,   // 0x4C
    brilliant,       // 0x80
    // ... more entries
};
```

## Rendering Functions

### Triangle Fill

```c
void trifill(long x1, long y1, long x2, long y2, long x3, long y3)
// Fills triangle defined by three vertices
```

### Line Fill (Scanline)

```c
void linefill()
// Renders a single scanline using the selected fill mode
```

## Texture Mapping Details

### How It Works

1. **Calculate UV coordinates** at each vertex
2. **Interpolate** across scanlines
3. **Apply perspective correction** (important for depth)
4. **Map to texture** using lookup

### Span-Based Rendering

The engine divides each polygon into horizontal "spans":

```c
#define spanlength 16    // Pixels per span
#define spanshifts 4    // log2(spanlength)
```

Longer spans = faster but less accurate.

## Optimization Techniques

### Pre-calculated Tables

```c
// Scanline offsets pre-calculated
unsigned long scan[200] = { 0L*2, 320L*2, 640L*2, ... };
```

### Register Optimization

Assembly handlers minimize register usage and keep FPU busy.

### Memory Access Patterns

Texture data accessed in predictable patterns for cache efficiency.

## Viewport Control

```c
void setviewport(long x_center, long y_center, 
                long vp_width, long vp_height)
// Sets rendering viewport (default: full screen)
```

## Display Functions

### Clear Screen

```c
void cleardisplay()
// Fills video memory with zeros
```

### Show Frame

```c
void showframe(unsigned char huge *source)
// Copies buffer to video memory with page flipping
```

### Low to High Resolution

```c
void LOW2HIGH(unsigned char huge *source)
// Scales 320x200 to higher resolution displays
```

## Vertex Ordering Convention

**CRITICAL**: Vertices must be passed in clockwise order:

```
       2
       *
    .       .
 .               .
1 *...............* 3
```

This determines:
- Which side is visible (back-face culling)
- Texture orientation
- Lighting direction

## Performance Notes

- **~500,000 triangles/second** on Pentium
- **~12,000 textured polygons/second** on 75MHz Pentium
- Span-based rendering reduces per-pixel overhead
- Optimized for 16-bit real mode execution

## Integration with Noctis

PITAGORA was designed to work with Noctis but doesn't include its own depth sorting. The host application must handle:
- Z-ordering
- Polygon sorting
- Frustum culling

## Differences from TDPOLYGS.H

| Feature | TDPOLYGS | PITAGORA |
|---------|-----------|----------|
| Polygon type | Quads | Triangles |
| Fill modes | Basic | Advanced |
| VESA support | No | Yes |
| Complexity | Lower | Higher |
| Speed | Fast | Faster |

## Usage Example

```c
// Initialize
initpitagora();
setviewport(160, 100, 320, 200);

// Set camera
cam_x = 0; cam_y = 0; cam_z = -500;
alpha = 0; beta = 45;
change_angle_of_view();

// Set texture
txm = my_texture;
H_MATRIXS = 16; V_MATRIXS = 16;
change_txm_repeating_mode(16, 16);

// Draw triangle
mapeffect = TEXTURE;
trifill(x1, y1, x2, y2, x3, y3);
```

## Assembly Handlers

PITAGORA uses near-call assembly handlers for rendering:

```c
void near plain();        // Solid color
void near forw_texture(); // Forward texture map
void near back_texture(); // Backward texture map
void near forw_gradient(); // Gradient fill
void near back_gradient(); // Backward gradient
void near brilliant();    // Additive light
```

Each handler follows strict conventions for speed.
