# FAST3D.H - Line-Based 3D Rendering Engine

## Overview

FAST3D.H provides a lightweight 3D rendering engine using line primitives with phosphor-like glow effects. It's optimized for rendering wireframe geometry like stars, spacecraft, and UI elements.

**Author**: Alessandro Ghignola & Fottifoh  
**Date**: October 30, 1994 - February 4, 1996

## Key Features

- Line-based rendering (no polygon filling)
- Phosphor glow effect (evanescente fosforescenza)
- Full-screen rendering
- Compatible with TEXT3D.H for 3D text

## Video Memory Segments

```c
unsigned char far *adaptor = (unsigned char far *) 0xA0000000;  // VGA memory
unsigned char far *adapted = (unsigned char far *) 0x80000000;  // Shadow buffer
unsigned char far *fake_adaptor = (unsigned char far *) 0x80000000;
```

## Core Functions

### Video Mode Setup

```c
void _320_200_256()   // Initialize VGA 320x200x256 mode (0x13)
void _80_25_C()       // Return to 80x25 text mode (0x03)
```

### Input Functions

```c
int attendi_pressione_tasto()  // Wait for keypress, return keycode
int tasto_premuto()             // Check if key available (returns 1/0)
```

### Initialization

```c
void init()
```
Pre-calculates trigonometric lookup tables:
- `tsin[]`, `tcos[]` - 720 entries (0 to 2π)
- `tsinx[]`, `tcosx[]` - 722 entries for screen coordinates
- `tsiny[]`, `tcosy[]` - 722 entries for screen coordinates

### 3D Line Rendering

```c
void Line3D(double p_x, double p_y, double p_z,
            double x, double y, double z)
```
Draws a 3D line between two points with:
- Camera transformation (rotation by alfa, beta)
- Perspective projection
- Frustum clipping
- Optional explosion effect

### 2D Line Rendering

```c
void Segmento(unsigned long x, unsigned long y,
              unsigned long x2, unsigned long y2)
```
Draws a 2D line with phosphor glow effect.

### Coordinate Conversion

```c
int C32(double x, double y, double z)
```
Converts 3D coordinates to 2D screen position. Returns 1 if visible, 0 if behind camera.

### Relative Drawing Functions

All relative functions use `ox, oy, oz` (origin offsets) for positioning:

```c
void rel(double sx, double sy, double sz, 
         double fx, double fy, double fz)
// Line relative to origin

void rectrel(double cx, double cy, double cz,
             double hx, double hz, char orient)
// Rectangle relative to origin (3 orientations)

void boxrel(double cx, double cy, double cz,
            double hx, double hy, double hz)
// Box/parallelepiped relative to origin

void xrel(double px, double py, double pz)
// Single point/dot relative to origin
```

## Global Variables

### Camera

```c
double cam_x = 0, cam_y = 0, cam_z = 0;  // Camera position
int alfa = 0;  // Pitch angle (degrees)
int beta = 0;  // Yaw angle (degrees)
```

### Screen Constants

```c
int x_centro = 160;  // Screen center X
int y_centro = 100;  // Screen center Y
int lowerbound_x = -157, upperbound_x = 157;
int lowerbound_y = -97, upperbound_y = 97;
```

### Origin Offset (for relative drawing)

```c
double ox = 0, oy = 0, oz = 0;  // Origin point for relative drawing
```

### Math Tables

```c
float *tcos, *tsin;     // Trig tables (720 entries)
float *tcosx, *tsinx;   // Scaled trig for X projection
float *tcosy, *tsiny;   // Scaled trig for Y projection
```

## Rendering Pipeline

### 1. Camera Transform
```c
// Rotate by beta (yaw)
z2 = z * tcos[beta] - x * tsin[beta];
rz = z2 * tcos[alfa] + y * tsin[alfa];

// Rotate by alfa (pitch)
rx = x * tcosx[beta] + z * tsinx[beta];
ry = y * tcosy[alfa] - z2 * tsiny[alfa];
```

### 2. Perspective Projection
```c
fx = rx / rz;  // Screen X
fy = ry / rz;  // Screen Y
```

### 3. Frustum Clipping
Lines are clipped against screen boundaries before rendering.

### 4. Glow Rendering
The `Segmento()` function draws with phosphor glow effect by:
- Drawing primary line
- Adding adjacent pixel brightness
- Creating bloom effect

## Usage Example

```c
// Initialize
init();
_320_200_256();
initscanlines();

// Set camera
cam_x = 0; cam_y = 0; cam_z = -500;
alfa = 0; beta = 45;

// Draw line from (100,50,0) to (200,100,100)
Line3D(100, 50, 0, 200, 100, 100);

// Draw relative to origin
ox = 100; oy = 50; oz = 0;
rel(0, 0, 0, 100, 50, 100);  // Same as above
```

## Integration with TEXT3D.H

FAST3D.H can be combined with TEXT3D.H for rendering 3D text using line primitives.

## Performance Notes

- Lookup tables eliminate runtime trig calculations
- Uses fixed-point math in inner loops
- Inline assembly for screen writes
- ~12,000 lines/second on 75MHz Pentium

## DOS/Windows Differences

The Windows version uses different memory segments:
```c
// Windows
unsigned char far *adaptor = (unsigned char far *) 0xA0000000;
unsigned char far *adapted = (unsigned char far *) 0xB0000000;
```
