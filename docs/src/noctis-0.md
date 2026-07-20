# NOCTIS-0.CPP - Core System Functions

## Overview

NOCTIS-0.CPP contains the foundational low-level functions for the Noctis space simulator. It handles:
- Video graphics initialization and manipulation
- Memory management
- Mouse input
- Image processing operations
- File system utilities

## Key Functions

### Video Mode Functions

#### `_320_200_256()`
Initializes the standard VGA 320x200x256 color graphics mode (mode 0x13).

#### `_80_25_C()`
Returns to standard 80x25 text mode (mode 0x03).

### Memory Operations

#### `pcopy(dest, sorg)`
Ultra-fast page copy using `rep movsd` instruction. Copies the video buffer.

#### `pclear(target, pattern)`
Fast page clear using `rep stosd`. Fills buffer with specified byte pattern.

#### `areacopy(dest, sorg, x, y, l, h)`
Copies a rectangular region with automatic clipping.

#### `areaclear(dest, x, y, x2, y2, l, h, pattern)`
Clears a rectangular region with boundary clipping for MCGA compatibility.

### Image Processing

#### `psmooth_grays(target)`
Applies 4x4 pixel averaging for smooth grayscale transitions. Uses optimized 32-bit operations reducing 58 cycles to 12 per pixel.

#### `pfade(target, segshift, speed)`
Creates a rapid fade-to-black effect by reducing color intensities.

#### `psmooth_64(target, segshift)`
4-shade smoothing with 64 intensity levels each.

#### `smootharound_64(target, cx, cy, r, diffuse)`
Localized smoothing around a specific point.

### Input Functions

#### `test_and_init_mouse()`
Detects mouse driver presence and initializes it by clearing the movement buffer. Returns 1 if mouse found, 0 otherwise.

#### `mouse_input()`
Reads mouse position and button state into global variables:
- `mdltx, mdlty` - Delta movement
- `mx, my` - Accumulated position
- `mpul` - Button press state

#### `attendi_pressione_tasto()`
Waits for and returns a keypress using DOS interrupt 0x21.

#### `tasto_premuto()`
Returns 1 if a key is available in the keyboard buffer, 0 otherwise.

### Color Palette

#### `tavola_colori(nuova_tavolozza, colore_di_partenza, nr_colori, filtro_rosso, filtro_verde, filtro_blu)`
Loads color palette with optional RGB filtering. Outputs to VGA DAC registers.

### Directory Operations

#### `reach_your_dir()`
Determines the program's actual directory from `_argv[0]` and changes to that directory. Critical for ensuring correct file paths regardless of how the program was launched.

## Global Variables

### Video Buffers
```cpp
unsigned char far *adaptor  = 0xA0000000;  // VGA memory segment
unsigned char far *adapted  = 0xB0000000;  // Shadow buffer
```

### Palettes
```cpp
unsigned char tmppal[768];        // Temporary palette
char return_palette[768];         // Palette for mode restoration
char surface_palette[768];        // Surface-specific palette
```

### Color Lookup
```cpp
unsigned char range8088[64*3];    // 64-level RGB range table
```

## Constants

```cpp
int QUADWORDS = 16000;            // Buffer size in 32-bit words
const TEXTURE_XSIZE = 256;        // Texture width
const TEXTURE_YSIZE = 256;        // Texture height
```

## DOS/Windows Compatibility

The file uses preprocessor defines to handle both DOS (16-bit) and Windows (32-bit) modes:

- **DOS Mode**: Uses `far` and `huge` pointers, 16-bit int types
- **Windows Mode**: Uses flat memory model, 32-bit types

Key differences handled:
```cpp
#ifdef WINDOWS
    #define DOBUFFERCHECK
    #define FMEMCPY(x, y, z) memcpy(x,y,z)
#else
    #define FMEMCPY(x, y, z) _fmemcpy(x,y,z)
#endif
```
