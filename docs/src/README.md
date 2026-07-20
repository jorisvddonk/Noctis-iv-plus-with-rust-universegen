# Noctis IV Plus - Source Documentation Index

## Quick Start

This documentation provides a comprehensive guide to the Noctis IV Plus source code.

## Documentation Files

### Core Architecture

| File | Description |
|------|-------------|
| [initialization.md](initialization.md) | Project structure and initialization process |
| [noctis-0.md](noctis-0.md) | Low-level graphics and system functions |
| [noctis-main.md](noctis-main.md) | Main game loop, flight control, UI |
| [noctis-1.md](noctis-1.md) | Planetary surface rendering and terrain |

### Graphics Engines

| File | Description |
|------|-------------|
| [fast3d.md](fast3d.md) | Line-based 3D rendering (wireframe) |
| [tdpolygs.md](tdpolygs.md) | Polygon 3D engine (filled polygons + textures) |
| [text3d.md](text3d.md) | Vector font system for 3D text |
| [pitagora.md](pitagora.md) | Triangle-based rendering engine |
| [gfx.md](gfx.md) | 2D graphics primitives |
| [assembly.md](assembly.md) | Low-level VGA/keyboard/timer utilities |

### Systems

| File | Description |
|------|-------------|
| [goes-net.md](goes-net.md) | GOES Net module system |

### Data & Formats

| File | Description |
|------|-------------|
| [cli.md](cli.md) | Command-line argument implementation |
| [file-formats.md](file-formats.md) | Binary file format specifications |

---

## Architecture Overview

```
                         +------------------+
                         |   NOCTIS.CPP     |
                         |  (Main Loop)     |
                         +--------+---------+
                                  |
         +-------------------+-----+----+-------------------+
         |                   |           |                   |
+--------v-------+  +--------v-------+  |  +--------v-------+
|   NOCTIS-0.CPP |  |   NOCTIS-1.CPP |  |  |  GOES NET     |
|  System I/O    |  | Surface Render  |  |  |   MODULES     |
+--------+-------+  +----------------+  +  +----------------+
         |                                           |
         v                                           v
   +-----+------+                              +-----+-----+
   |  FAST3D   |                              | ST.EXE   |
   |  TDPOLYGS |                              | SL.EXE   |
   |    GFX    |                              |  ...     |
   +-----------+                              +----------+
         |
         v
   +-----+------+    +------+----+
   | TEXT3D.H  |    |PITAGORA.H|
   |  (Fonts)  |    |(Triangles)|
   +-----------+    +-----------+
```

## Key Initialization Sequence

1. **DOS/Windows Mode Detection** (defs.h)
2. **Directory Setup** (`reach_your_dir()`)
3. **Video Mode** (`_320_200_256()`)
4. **Memory Allocation** (~550KB buffers)
5. **Trig Tables** (`init()` in FAST3D)
6. **Scanlines** (`initscanlines()`)
7. **Mouse Detection** (`test_and_init_mouse()`)
8. **State Restoration** (`unfreeze()`)
9. **Main Loop Entry**

## Memory Layout

| Area | Size | Purpose |
|------|------|---------|
| om_bytes | 7,340 | Offset map (QT-VR) |
| gl_bytes | 22,586 | Globe textures |
| st_bytes | 64,800 | Star background |
| pl_bytes | 65,552 | Planetary map |
| ps_bytes | 40,000 | Surface altimetry |
| oc_bytes | 40,000 | Surface objects |
| sc_bytes | 65,540 | Video buffer |
| pv_bytes | 20,480 | Polygon files |

**Total**: ~334KB dynamic + 222KB code = ~550KB

## Important Functions by Module

### NOCTIS-0.CPP
- `reach_your_dir()` - Directory setup
- `_320_200_256()` - VGA video init
- `pcopy()` / `pclear()` - Buffer operations
- `psmooth_grays()` - Image processing
- `test_and_init_mouse()` - Mouse setup

### NOCTIS.CPP
- `freeze()` / `unfreeze()` - State management
- `fcs()` / `fcs_commands()` - Flight control
- `devices()` / `dev_commands()` - Device menus
- `vehicle()` - Interior rendering

### NOCTIS-1.CPP
- `hpoint()` - Terrain height calculation
- `build_fractal_tree()` - Vegetation generation
- `greenmush()` - Ground cover

### FAST3D.H
- `init()` - Trig table initialization
- `Line3D()` - 3D line rendering
- `Segmento()` - 2D line with glow

### TDPOLYGS.H
- `initscanlines()` - Scanline setup
- `poly3d()` - Solid polygon
- `polymap()` - Textured polygon
- `facing()` - Back-face culling

### GFX.H
- `Segmento()` - Line drawing
- `Blocco()` / `Rettangolo()` - Shapes
- `Testo()` - Text rendering
- `Copia()` - Bitmap copy

## DOS/Windows Differences

The codebase uses preprocessor defines:
- `WINDOWS` - 32-bit mode
- `__DOS__` or default - 16-bit DOS mode

Key differences:
- Pointer types (`far` vs flat)
- Memory allocation (`farmalloc` vs `malloc`)
- File I/O (DOS int vs stdio)
- Entry point (`void main()`)

## External Dependencies

### DOS Interrupts
- `0x10` - Video services
- `0x21` - DOS functions
- `0x33` - Mouse driver

### File Paths
```
..\data\STARMAP.BIN   - Star database
..\data\GUIDE.BIN     - Navigation data
..\data\supports.nct  - Embedded resources
..\modules\           - GOES Net modules
```

## Build Information

- **Compiler**: Borland C++ 3.1 (original) / Modern C++ (Windows port)
- **Output**: NOCTIS.EXE + SUPPORTS.NCT
- **Target**: DOS 16-bit / Windows 32-bit

## CLI Documentation

See [cli.md](cli.md) for:
- Adding command-line arguments
- Fuel/power options (-nofuel, -infinite-power)
- File overrides (-starmap, -guide)
- Start position (-pos, -planet, -star)
- Autoland (-autoland, -land)
- Export/compare (-export-palette, -export-textures, -export-seed)

## File Formats

See [file-formats.md](file-formats.md) for:
- Current.BIN save file structure
- Surface.BIN landing data
- STARMAP.BIN star database
- Palette export formats
- Texture file layouts
