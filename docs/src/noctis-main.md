# NOCTIS.CPP - Main Game Module

## Overview

NOCTIS.CPP is the primary module containing the game's main loop, high-level gameplay logic, user interface, and system control. It integrates all other modules and manages the overall game state.

## Main Game Loop Structure

The main loop (starting ~line 4400) follows this flow:

1. **Input Processing** - Handle keyboard/mouse commands
2. **State Updates** - Update positions, physics, timers
3. **Targeting** - Handle star/planet selection and approach
4. **Rendering** - Draw the 3D scene, HUD, vehicle interior
5. **Module Communication** - Process GOES Net commands
6. **State Persistence** - Periodic save via `freeze()`

## Key Systems

### Flight Control System (FCS)

The FCS manages all flight operations:

```cpp
void fcs()           // Display flight status
void fcs_commands()  // Process flight commands
```

Commands include:
- Start/stop vimana (starship) flight
- Set remote/local targets
- Fine approach control
- Landing deployment

### Device Management

```cpp
void devices()       // Display device menus
void dev_commands() // Process device commands
```

Device pages:
- **Page 1**: Navigation instruments (starfield amplifier, tracking modes, planet finder)
- **Page 2**: Miscellaneous (internal light, target data, environment data)
- **Page 3**: Galactic cartography (labeling stars/planets, targets in range)
- **Page 4**: Emergency functions (system reset, help request, lithium collection)

### Preferences

```cpp
void prefs()        // Display preference options
void pfs_commands() // Process preference changes
```

Toggleable options:
- Auto screen sleep
- Reverse pitch controls
- Always-on menus
- Polarization/depolarization

### Onboard Computer System

The vehicle's internal computer provides multiple screens:

```cpp
void screen()           // Render main HUD screen
void clear_onboard_screen() // Clear computer display
void cline(line, text)  // Write single line
void other(text)        // Continue current line
```

Screen layout: 21x7 character display with command menus on the right.

## Vehicle Rendering

### Interior View

```cpp
void vehicle(opencapcount)  // Draw vehicle interior
```

Renders:
- Panoramic cupolas (openable for landing)
- Dashboard with screens
- GOES Net terminal
- Internal lighting (halogen lamp)

### Exterior View

```cpp
void other_vehicle_at(ovhx, ovhy, ovhz)  // Render external vehicle
```

Draws other vessels from outside perspective.

## GOES Net System

GOES Net is an in-game module system allowing external programs to interact with the game:

```cpp
void run_goesnet_module()  // Execute external module
```

Communication:
- Command line via `goesnet_command[]`
- Input via `COMM.BIN` file
- Output to `goesoutputfile`

### Supported Modules
- `ST` - Star map tree viewer
- `SL` - Star list
- `CAST` - Star catalogue
- `INBOX/OUTBOX` - Message systems
- `DL/REP` - Download/repair functions

## Navigation

### Stardrifter (Space) Mode
- Interstellar travel using vimana drive
- Lithium collection from stars
- Remote target selection via PARSIS coordinates
- Auto-navigation to stars

### Planetary Mode
- Atmosphere entry simulation
- Surface landing
- Terrain exploration with jetpack
- Local target approach

### Tracking Modes (sync variable)
1. Fixed-point chase
2. Far chase
3. Synchrone orbit
4. High-speed orbit
5. Near chase

## Target Systems

### Remote (Star) Targets
```cpp
void fix_remote_target()    // Set star destination
void update_star_label()    // Update star name display
```

### Local (Planet) Targets
```cpp
void fix_local_target()     // Set planet destination
void update_planet_label()  // Update planet name display
```

## State Persistence

### Freeze System
```cpp
void freeze()   // Save to disk
void unfreeze() // Load from disk
```

Saved state includes:
- Position (dzat_x/y/z)
- Target information
- Camera orientation
- Power/charge levels
- Options
- GOES Net command buffer

## HUD Elements

### Main Display
- Current coordinates (PARSIS universal)
- Target information
- Power meter
- Distance/range displays

### Warning Systems
- Status messages (top of screen)
- Crosshairs for targets
- Proximity indicators

### Computer Screens
- Flight control status
- Device status pages
- Environment data
- Planetary surface map

## Key Global State Variables

### Navigation
```cpp
double dzat_x, dzat_y, dzat_z;     // Current position
double ap_target_x, ap_target_y, ap_target_z;  // Remote target
float alfa, beta, gamma;            // Camera angles
```

### Flight State
```cpp
char stspeed;        // Stardrifter active
char ip_reaching;    // Approaching planet
char ip_reached;     // At planet
char sync;           // Tracking mode
int pwr;             // Available power
```

### Target State
```cpp
char ap_targetted;   // Remote target set
int ap_target_class; // Target star class
char ip_targetted;   // Local target planet index
```

## User Input Mapping

| Key | Action |
|-----|--------|
| 5 | Flight Control System menu |
| r | Devices menu |
| d | Preferences menu |
| x | Exit menus |
| 6-9 | Command execution |
| +/- | Lighting adjustment |
| TAB | Toggle antialiasing |
| M/* | Screenshot |
| ESC | Cancel/back |
