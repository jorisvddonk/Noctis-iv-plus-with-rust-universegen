# GOES Net Module System

## Overview

GOES Net (Galactic Operating & Emergency System Network) is Noctis's in-game module system, allowing external programs to interact with the game, provide additional functionality, and extend the experience.

## Architecture

```
+-----------+      +------------+      +----------+
|  NOCTIS   | <--> | COMM.BIN   | <--> |  MODULE  |
|  (host)   |      | (protocol) |      | (external)|
+-----------+      +------------+      +----------+
       |                                     |
       v                                     v
  GOESOUTPUT.TXT                   Various EXE/COM files
  (screen output)                   (ST.EXE, SL.EXE, etc.)
```

## Communication Protocol

### Input (COMM.BIN)

The host game writes commands to `COMM.BIN`:

```c
// Written by NOCTIS.CPP when module returns data
_write(fh, &ip_targetted, 2);      // 2 bytes: planet target
_write(fh, &ap_target_x, 8);       // 8 bytes: double
_write(fh, &ap_target_y, 8);       // 8 bytes: double  
_write(fh, &ap_target_z, 8);       // 8 bytes: double
```

### Output (GOESOUTPUT.TXT)

Modules write to `GOESOUTPUT.TXT` which is read by the game's vehicle() function and displayed on the onboard computer screens.

### Command Line

The module is launched with command line arguments via DOS `system()`:
```c
system("ST.EXE >GOESOUTPUT.TXT");
```

## Available Modules

### ST.EXE - Star Tree Viewer
Displays hierarchical star system navigation tree.

### SL.EXE - Star List
Simple alphabetical/list view of stars.

### CAST.EXE - Star Catalogue
Detailed star information database.

### INBOX.EXE - Message Inbox
Receives and displays messages.

### OUTBOX.EXE - Message Outbox
Composes and sends messages.

### DL.EXE - Download
Downloads data from remote sources.

### REP.EXE - Repair
Repair/refuel module.

### PRI.EXE - Priority
Priority message system.

### CLEAN.EXE - Cleanup
Cleans temporary files.

### DELE.EXE - Delete
File deletion utility.

## Module Structure

Each module is a standalone DOS executable that:
1. Reads from current directory for game data
2. Writes output to `GOESOUTPUT.TXT`
3. Optionally writes to `COMM.BIN` for bidirectional communication

### Example Module (Pseudo-code)

```c
#include <stdio.h>

void main() {
    FILE *out = fopen("GOESOUTPUT.TXT", "w");
    
    // Read game state if needed
    FILE *comm = fopen("..\\DATA\\COMM.BIN", "rb");
    if (comm) {
        // Read coordinates, targets, etc.
        fclose(comm);
    }
    
    // Output to onboard computer screen
    fprintf(out, "                    STAR LIST\n");
    fprintf(out, "-----------------------------------------\n");
    fprintf(out, "  #  NAME              CLASS   DISTANCE\n");
    fprintf(out, "-----------------------------------------\n");
    // ... listing code ...
    
    fclose(out);
    return 0;
}
```

## Integration with Main Game

### Launching Modules

```c
// From NOCTIS.CPP - run_goesnet_module()
void run_goesnet_module() {
    // Save game state
    freeze();
    
    // Free memory for module
    farfree(adapted);
    
    // Build command
    goesnet_command[gnc_pos] = '\0';
    strcat(goesnet_command, " >GOESOUTPUT.TXT");
    
    // Launch module
    system(goesnet_command);
    
    // Reallocate memory
    adapted = farmalloc(sc_bytes);
    
    // Process response
    process_communication();
}
```

### Screen Display

The vehicle's computer screens display module output:

```c
// In vehicle() function - NOCTIS.CPP
screenfile = _open("GOESOUTPUT.TXT", 0);
if (screenfile > -1) {
    lseek(screenfile, goesfile_pos, SEEK_SET);
    n = _read(screenfile, osscreen[1], 7*21);
    _close(screenfile);
}
```

## Screen Format

GOESOUTPUT.TXT is displayed on 21x7 character screens:
- 21 characters per line
- 7 lines per screen
- Scrollable with arrow keys

## Special Commands

### CLR Command
```c
// Clears GOES output
if (!memcmp(goesnet_command, "CLR", 3)) {
    remove(goesoutputfile);
}
```

## Data Files

### COMM.BIN Format

| Size | Content |
|------|---------|
| 2 bytes | Planet target (int16) |
| 24 bytes | Remote target coordinates (3x float64) |

### STARMAP.BIN

The main star database used by modules.

### GUIDE.BIN

Navigation guide data.

## Creating Custom Modules

### 1. Create Standalone EXE
```c
// mymodule.c
#include <stdio.h>
void main() {
    FILE *f = fopen("GOESOUTPUT.TXT", "w");
    fprintf(f, "My custom module output\n");
    fclose(f);
}
```

### 2. Compile with Borland C
```bash
bcc -mydmodule.c -o mymodule.exe
```

### 3. Place in MODULES directory

### 4. Run from GOES Net
```
GOES> MYMODULE
```

## Event-Driven Communication

The game can trigger module launches based on events:

```c
// In planetary approach
if (filelength(comm) == 2) {
    // Planet selected
    _read(comm, &ip_targetted, 2);
    ip_targetted--;
    fix_local_target();
}
```

## Debugging Modules

### Testing Output
```c
// In module
FILE *debug = fopen("DEBUG.TXT", "w");
fprintf(debug, "Position: %f, %f, %f\n", x, y, z);
fclose(debug);
```

### Common Issues
- Module needs to be in correct directory
- Output file must be named exactly `GOESOUTPUT.TXT`
- Working directory must have access to DATA files
- Some modules require specific file formats

## Video Mode Notes

Modules should:
- Not change video mode permanently
- Clean up before exit
- Use text mode or mode 0x13 (320x200x256)

## Future Extension Ideas

- Network modules for multiplayer
- Database import/export
- Procedural generation tools
- Mission scripting
- Recording/playback utilities
