# Noctis IV Plus - Source Code Documentation

## Project Overview

Noctis IV Plus is a real-time 3D space exploration simulator originally created by Alessandro Ghignola. This modified version enhances the original game with additional features while preserving its core architecture.

The project is written in C++ with heavy use of inline assembly for performance-critical graphics operations, designed to run on DOS/Windows systems.

## Project Structure

```
source/
├── NOCTIS.CPP      - Main game loop and high-level gameplay logic
├── NOCTIS-0.CPP    - Low-level graphics, hardware, and system initialization
├── NOCTIS-1.CPP    - Planetary surface rendering and terrain generation
├── NOCTIS-2.H      - Additional function declarations
├── NOCTIS-D.H      - Data definitions, memory layouts, and constants
├── NOCTIS-0.H      - External function and variable declarations
├── defs.h          - Common definitions and macros
├── ASSEMBLY.H      - Assembly language utilities
├── GFX.H           - Graphics-related definitions
├── FAST3D.H        - 3D rendering engine definitions
└── [resource files] - Textures, sounds, and game data
```

## Initialization Process

### 1. System Initialization (NOCTIS-0.CPP)

The initialization sequence begins with low-level system setup:

#### 1.1 Directory Setup
```cpp
void reach_your_dir()
```
Locates the program's actual directory and changes the working directory to ensure correct file paths for game resources.

#### 1.2 Video Mode Setup
```cpp
void _320_200_256()  // Initialize VGA 320x200x256 graphics mode
void _80_25_C()      // Return to text mode 80x25 colors
```
Transitions between VGA graphics mode and text mode.

#### 1.3 Memory Initialization
The game allocates approximately 550KB of memory for various buffers:

| Buffer Name | Size (bytes) | Purpose |
|-------------|--------------|---------|
| `om_bytes`  | 7,340        | Offset map for QT-VR |
| `gl_bytes`  | 22,586       | Globe textures (sea surface) |
| `st_bytes`  | 64,800       | Star background map |
| `pl_bytes`  | 65,552       | Planetary map from orbit |
| `ps_bytes`  | 40,000       | Surface altimetry map |
| `oc_bytes`  | 40,000       | Surface objects map |
| `sc_bytes`  | 65,540       | Video page buffer |
| `pv_bytes`  | 20,480       | PV polygon file buffer |

#### 1.4 Mouse Support
```cpp
char test_and_init_mouse()
```
Detects and initializes the mouse driver, clearing the movement buffer.

#### 1.5 Palette Management
```cpp
void tavola_colori(unsigned char *nuova_tavolozza, 
                   unsigned colore_di_partenza, 
                   unsigned nr_colori,
                   char filtro_rosso, char filtro_verde, char filtro_blu)
```
Loads and applies color palettes with optional RGB filtering for atmospheric effects.

### 2. Main Entry Point

The main game loop resides in `NOCTIS.CPP` with the key function starting around line 4400. The entry sequence:

1. **Graphics Initialization** - Sets up VGA mode
2. **Memory Allocation** - Allocates all game buffers
3. **File Loading** - Loads star maps, surface textures, vehicle models
4. **State Restoration** - Attempts to restore previous game state via `unfreeze()`
5. **Main Loop** - Enters the game loop handling input, rendering, and physics

### 3. State Management (NOCTIS.CPP)

#### Freeze/Unfreeze System
```cpp
void freeze()   // Save current game state to disk
void unfreeze() // Restore game state from disk
```

The freeze function saves:
- Current position and velocity (`dzat_x`, `dzat_y`, `dzat_z`)
- Target information
- Camera angles
- System options
- Power/charge levels
- Navigation state

### 4. Planetary Initialization (NOCTIS-1.CPP)

When approaching a planet, the surface rendering system initializes:

#### 4.1 Terrain Generation
```cpp
float hpoint(long px, long pz)
```
Calculates terrain height using bilinear interpolation with 16,384 intermediate gradations between logical units.

#### 4.2 Surface Features
The system generates:
- **Oceans** (OCEAN, PLAINS, DESERT, ICY biomes)
- **Vegetation** - Procedurally generated trees and plants
- **Rocks** - Randomly placed rock formations
- **Atmospheric effects** - Weather, lighting, fog

### 5. Key Global Variables

#### Position and Navigation
- `dzat_x, dzat_y, dzat_z` - Current position in space
- `alfa, beta, gamma` - Camera rotation angles
- `cam_x, cam_y, cam_z` - Camera position offset

#### Game State
- `sync` - Tracking mode (0=inactive, 1-5=various chase modes)
- `ip_targetted` - Current local (planet) target index
- `ap_targetted` - Current remote (star) target status
- `stspeed` - Stardrifter flight active flag
- `pwr` - Available power/energy

### 6. Build System

The project uses a Makefile-based build system:
- `NOCTIS.MAK` - Main makefile
- Output: `NOCTIS.EXE` with appended `SUPPORTS.NCT` resource file

### 7. External Modules

The game supports GOES Net - an in-game module system:
- Modules located in `/modules/`
- Communication via `COMM.BIN` file
- Modules include: ST, SL, CAST, INBOX, OUTBOX, etc.

## Graphics Engine

The 3D engine (defined in `FAST3D.H` and `TDPOLYGS.H`) uses:
- Software polygon rendering optimized with inline assembly
- Depth culling for performance
- Texture mapping with repeating modes
- Dynamic lighting calculations

## Data Files

| File | Location | Description |
|------|----------|-------------|
| `STARMAP.BIN` | data/ | Star database |
| `GUIDE.BIN` | data/ | Navigation guide |
| `SUPPORTS.NCT` | data/ | Embedded resources |

## Platform Notes

- Originally designed for DOS (16-bit mode with far pointers)
- Windows port uses different memory models (32-bit flat)
- Heavy use of x86 assembly for performance
- VGA hardware registers accessed directly
