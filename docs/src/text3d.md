# TEXT3D.H - Vector Font System

## Overview

TEXT3D.H provides a 3D vector font system for rendering text as wireframe 3D geometry. Unlike bitmap fonts, vector fonts are drawn as line segments in 3D space, allowing them to scale and rotate with the scene.

## Font Data Structure

The font is stored as stroke definitions in the `symbol` array:

```c
char symbol[65][41] = { ... };
```

Each row (0-64) represents one character:
- Index 0: Character width
- Indices 1-40: Stroke definitions (pairs of X,Y offsets)

### Stroke Encoding

Each stroke is encoded as 4 values:
```
[start_x, start_y, end_x, end_y]
```

Values are relative coordinates scaled during rendering. The format allows for multi-stroke characters.

### Character Set

The font includes 65 characters (ASCII 32-96):
- Space to at-sign (32-64): Special characters
- A-Z (65-90): Uppercase letters
- Some punctuation and symbols

## Main Rendering Function

### `Txt()`

```c
void Txt(char *testo, double x, double y, double z,
         double scala_x, double scala_y, int alfa, int beta)
```

Renders a string in 3D space.

**Parameters:**
- `testo` - Null-terminated string
- `x, y, z` - Position in 3D space
- `scala_x, scala_y` - Scale factors
- `alfa, beta` - Rotation angles (pitch, yaw)

**How it works:**
1. Iterates through each character in string
2. For each character, reads stroke data from `symbol[]`
3. Applies 3D transformation (rotation, scaling)
4. Calls `Line3D()` to draw each stroke

### Stroke Processing

```c
// For each stroke in character:
fxx = symbol[t][f+0]*scala_x;  // Start X
fyy = symbol[t][f+1]*scala_y;  // Start Y
pxx = symbol[t][f+2]*scala_x;  // End X
pyy = symbol[t][f+3]*scala_y;  // End Y

// Transform to 3D and draw line
Line3D(sx, sy, sz, lsx, lsy, lsz);
```

## Character Definitions

### Numbers (ASCII 48-57)
```
0:  9, -1, -2, 1, -2, 1, -2, 2, -1, 2, -1, 2, 1, 2, 1, 1, 2...
```

### Letters (ASCII 65-90)
Each letter is drawn using multiple strokes. For example:
- 'A': Two diagonal strokes + one horizontal crossbar
- 'B': Vertical + three curved strokes
- 'O': Circle (approximated with multiple segments)

## Scale Calculation

The font uses perspective-correct scaling:

```c
double icoeff = sqrt(sx*sx + sy*sy + sz*sz);
double scala_d = scala_x * mediumwidth;
long incr = icoeff / (50*scala_d);
```

This adjusts character spacing based on distance from camera.

## Integration with FAST3D

TEXT3D.H uses FAST3D.H's `Line3D()` function for rendering, which provides:
- 3D transformation
- Perspective projection
- Glow effects

## Usage Example

```c
#include "TEXT3D.H"
#include "FAST3D.H"

// After initializing FAST3D:
init();  // Setup trig tables

// Render text in 3D
Txt("HELLO", 100, 50, 0,  1.0, 1.0,  0, 45);
```

## Font Metrics

```c
char blank = 4;       // Space character width
char mediumwidth = 4; // Average character width
```

- Maximum stroke segments per character: 10
- Coordinates stored as signed chars (-128 to 127)
- Scale factors modify during rendering

## Advantages

1. **Resolution independent** - Scales without pixelation
2. **Truly 3D** - Can be rotated, positioned in depth
3. **Consistent stroke width** - No varying line weights
4. **Small footprint** - ~2.6KB for entire font

## Limitations

1. **Wireframe only** - No filled interiors
2. **Limited character set** - 65 characters
3. **No lowercase** - Uppercase only
4. **Fixed stroke count** - Some complex letters simplified

## Customization

### Adding Characters

To add a new character:
1. Choose ASCII index (32-96)
2. Define stroke width in `symbol[index][0]`
3. Add stroke pairs (x1,y1,x2,y2) for each segment

Example:
```c
// Adding '[' character at index 33 (already exists as example)
symbol[33][0] = 3;           // Width
symbol[33][1] = -2; symbol[33][2] = 0;  // Vertical line start
symbol[33][3] =  2; symbol[33][4] = 0;  // Vertical line end
// Add horizontal strokes as needed
```

### Modifying Scale

```c
// Make text larger
Txt("TEXT", x, y, z, 2.0, 2.0, alfa, beta);

// Make text smaller
Txt("TEXT", x, y, z, 0.5, 0.5, alfa, beta);
```

## Historical Notes

This font system was common in early 3D games and demoscene productions due to:
- Minimal memory requirements
- Fast rendering (line primitives)
- Retro aesthetic appeal
