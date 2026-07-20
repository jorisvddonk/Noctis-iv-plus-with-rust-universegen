# GFX.H - 2D Graphics Primitives

## Overview

GFX.H provides low-level 2D graphics primitives for Noctis, including line drawing, shapes, text rendering, and bitmap operations. These functions build upon the VGA 320x200x256 graphics mode.

## Video Memory

```c
// Defined in ASSEMBLY.H
extern unsigned char far *adaptor;   // VGA memory segment (0xA0000000)
extern unsigned char far *adapted;  // Working buffer
```

## Core Drawing Functions

### Line Drawing

```c
void Segmento(long x, long y, long x2, long y2,
               unsigned char colore, char abrush)
```
Draws a line between two points.

**Parameters:**
- `x, y` - Start coordinates
- `x2, y2` - End coordinates  
- `colore` - Color index (0-255)
- `abrush` - Brush width (0 = 1 pixel, >0 = multi-pixel width)

**Example:**
```c
Segmento(10, 10, 100, 50, 63, 0);  // Single pixel line
Segmento(10, 10, 100, 50, 63, 3);  // 3-pixel wide line
```

### Rectangle Outline

```c
void Rettangolo(long x, long y, long l, long a, unsigned char colore)
void psico_Rettangolo(long x, long y, long l, long a, 
                      unsigned char colore, char abrush)
```
Draws rectangular outline.

**Parameters:**
- `x, y` - Top-left corner
- `l` - Width
- `a` - Height (italiano)
- `colore` - Color
- `abrush` - Brush width (psico_ version only)

### Filled Rectangle

```c
void Blocco(long x, long y, long l, long a, unsigned char colore)
void psico_Blocco(long x, long y, long l, long a, unsigned char colore)
```
Draws filled rectangle.

The `psico_` version applies vertical gradient (shading):
```c
psico_Blocco(10, 10, 50, 30, 32);
// Color increases by 1 for each scanline (creates gradient effect)
```

### Text Rendering

```c
void Testo(long x, long y, char far *messaggio,
            unsigned char colore, unsigned stile)
```

**Styles:**
```c
#define NORMALE  0   // Normal
#define ITALICO  1   // Italic
#define NERTO    2   // Bold
#define CENTRATO 3    // Centered
#define ALL_DX   4   // Right-aligned
```

**Example:**
```c
Testo(10, 10, "HELLO WORLD", 63, NORMALE);
Testo(160, 100, "CENTERED TEXT", 63, CENTRATO);
```

### Text with Shadow

```c
void tx(int x, int y, char *t, char attrib)
```
Draws text with black shadow offset by (+1, +1).

### Bitmap Copy

```c
void Copia(unsigned char far *dest, unsigned char far *sorg,
           unsigned x, unsigned y, int l, int a)
```
Copies bitmap rectangle from source to destination.

**Parameters:**
- `dest` - Destination buffer
- `sorg` - Source buffer  
- `x, y` - Position
- `l` - Width (in pixels)
- `a` - Height (in pixels)

### Half-Brite Effect

```c
void halfbrite(int x, int y, int l, int a, char incr)
```
Brightens/darkens rectangular area.

```c
halfbrite(0, 0, 320, 200, 32);  // Brighten entire screen
```

## Internal Font

GFX.H includes a built-in 6x6 uppercase font (65 characters).

```c
unsigned char font[65][6] = { ... };
```

Character range: ASCII 32-96 (space through backtick).

Character set:
- Space (32) through at-sign (64)
- A-Z (65-90)
- Some special characters

## Clipping

Functions automatically clip to screen boundaries:
```c
int lwx = 0, upx = 319;  // X bounds
int lwy = 0, upy = 199;  // Y bounds
```

## Drawing Primitives for HUD

The HUD system uses these functions extensively:

```c
// Typical HUD drawing pattern
Rettangolo(x, y, w, h, color);           // Box outline
Blocco(x, y, w, h, color);               // Filled box
Testo(x, y, "STATUS", color, NORMALE);   // Text
```

## Performance Notes

- Direct memory writes to video buffer
- Uses integer arithmetic where possible
- 16-bit addressing for speed on 8086/80286
- Assembly-optimized inner loops

## Integration

GFX.H is used by:
- NOCTIS.CPP - HUD rendering
- Vehicle interior rendering
- Screen effects
- Menu displays

## Color Palette

The standard 256-color VGA palette is used. Common HUD colors:
- `63` - White/bright
- `32` - Gray
- `0` - Black
- Various other palette entries for themed elements
