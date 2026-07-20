# Movie and Screenshot System

## Overview

Noctis includes a built-in system for recording gameplay as screenshots and movies.

## Screenshot System

### Functions

```c
extern void snapshot(int forcenumber, char showdata);
```

Takes a screenshot and saves as BMP.

**Parameters:**
- `forcenumber` - Force specific image number (0 = auto)
- `showdata` - Include debug info overlay

### Output Format

Screenshots are saved as uncompressed BMP:

```c
// GALLERY/ directory
sprintf(snapfilename, "..\\GALLERY\\%08d.BMP", prog);
```

- **Filename**: 8-digit number (00000000-99999999)
- **Format**: 320x200, 256 colors + palette
- **Location**: `..\GALLERY\`

### Wide Screenshots

```c
// WIDE mode for panoramic views
sprintf(snapfilename, "..\\GALLERY\\WIDE%04d.BMP", forcenumber);
```

### BMP Header Structure

```c
// Standard BMP for 320x200x256
t[0] = 'B'; t[1] = 'M';        // Signature
// ... 54-byte header
// Then: palette (768 bytes)
// Then: pixel data (64000 bytes)
```

### Screenshot Hotkeys

| Key | Action |
|-----|---------|
| M or * | Take screenshot |
| B | Raw screenshot |
| N or / | Wide screenshot |

### Maximum Screenshots

```c
if (prog == 100000000) {
    // Rollover after 99,999,999
    prog = 0;
}
```

## Movie Recording

### Variables

```c
extern int moviefps;           // Frames per second
extern int moviefsec;          // Current frame
extern int moviestime;         // Recording duration
extern int moviedeck;          // Current deck (subfolder)
extern int movienr;            // Frame number
extern char movie;              // Recording active
extern char movieexists;       // Movie exists
extern char movieflashoff;     // Flash effect
extern char moviestat;         // Recording status
extern char moviepaused;       // Pause state
```

### Movie Structure

Movies are saved as numbered BMP sequences:

```
..\MOVIES\001\00000001.BMP
..\MOVIES\001\00000002.BMP
...
..\MOVIES\001\00099999.BMP
..\MOVIES\002\00000001.BMP
```

### Recording Controls

```c
// Start recording
movie = 1;
movienr = 0;
sprintf(snapfilename, "..\\MOVIES\\%03i\\%08d.BMP", moviedeck, movienr);

// Pause/Resume
if (c == 'p' && movie) {
    moviepaused = !moviepaused;
}
```

### Movie Hotkeys

| Key | Action |
|-----|---------|
| F3 | Toggle movie mode |
| P | Pause/Resume |

## Frame Rate Calculation

### FPS Calculation

```c
extern void CalculatemovieFPS(int moviefscap, int moviestime);
```

Calculates actual frames per second from capture data.

### Frame Timing

```c
moviefscap++;  // Increment frame count
moviestime += clock();  // Accumulate time
```

## Image Processing

### Palette Handling

The game converts internal palette to BMP format:

```c
// Convert 6-bit VGA palette to 8-bit BMP
b = tmppal[c+2] * 4;  // Blue
g = tmppal[c+1] * 4;  // Green  
r = tmppal[c+0] * 4;  // Red
a = 0;  // Alpha
```

### Upside-Down Correction

BMP stores images bottom-up, but video memory is top-down:

```c
// Write from bottom up
for (ptr = 63680; ptr < 64000; ptr -= 320)
    _write(ih, adapted + ptr, 320);
```

## Directory Structure

```
NOCTIS.EXE
├── GALLERY/
│   ├── 00000000.BMP
│   ├── 00000001.BMP
│   └── ...
├── MOVIES/
│   ├── 001/
│   │   ├── 00000001.BMP
│   │   └── ...
│   └── 002/
│       └── ...
```

## Memory Considerations

### Buffer Size

```c
// 320x200 = 64000 bytes
// + 4 bytes padding for polygon overflow
sc_bytes = 65540;
```

### Double Buffering

```c
// Draw to hidden buffer
adapted = farmalloc(sc_bytes);

// Flip to screen
pcopy(adaptor, adapted);
```

## Debug Overlay

### Show Data Option

When `showdata = 1`:

```c
areaclear(adapted, 2, 191, 0, 0, 316, 7, 64 + 63);

// Show location
sprintf(buffer, "LOCATION PARSIS: %s;%s;%s",
    alphavalue(parsis_x),
    alphavalue(-parsis_y),
    alphavalue(parsis_z));

wrouthud(3, 192, NULL, buffer);
```

### Raw Screenshots

B key bypasses the overlay:

```c
// Direct pixel dump without info
_write(ih, adapted + ptr, 320);
```

## CLI Export Integration

For comparison testing, screenshots can be automatically exported:

```c
if (cli_args.export_screenshot) {
    snapshot(0, 0);
}
```

## Technical Details

### Video Memory Layout

```
Segment: A000h
Offset 0:     Pixel (0,0)
Offset 320:    Pixel (0,1)
...
Offset 63360:  Pixel (0,197)
Offset 63380:  Pixel (0,198)
Offset 63400:  Pixel (0,199) [end of display]
Offset 63440:  Padding area
```

### BMP File Structure

```
Offset  Size  Description
0      2     'BM' signature
2      4     File size
10     4     Pixel data offset
14     4     Header size
18     4     Width
22     4     Height
28     2     Bits per pixel (8)
30     4     Compression (0)
...
54     768   Palette (256 * 3)
...
62222   64000  Pixel data
```

### Frame Rate

- Standard: 18.2 fps (BIOS timer)
- Target: ~15 fps for smooth playback
- Actual varies with scene complexity
