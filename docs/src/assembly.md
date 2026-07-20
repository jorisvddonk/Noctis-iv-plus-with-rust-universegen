# ASSEMBLY.H - Low-Level Assembly and VGA Utilities

## Overview

ASSEMBLY.H provides low-level hardware access functions and constants for VGA/EGA graphics, keyboard handling, and timing. It serves as the foundation for all graphics operations in Noctis.

## Video Memory Pointers

```c
unsigned char far * adaptor      = (unsigned char far *) 0xA0000000;  // VGA memory
unsigned char far * fake_adaptor = (unsigned char far *) 0;
unsigned char far * adapted      = (unsigned char far *) 0;  // Working buffer
```

## BIOS Data Area

```c
unsigned char far * ctrlkeys     = (unsigned char far *) 0x00000417; // Keyboard flags
unsigned char far * video_mode   = (unsigned char far *) 0x00000449; // Current video mode
unsigned char far * timer_ticks  = (unsigned char far *) 0x0000046C; // Timer ticks
```

## Constants

```c
#define colore_trasparente 20  // Transparent color value

int QUADWORDS = 16000;  // Buffer size (64KB / 4)
```

## Video Mode Functions

### Standard VGA Modes

```c
void _320_200_256()
// Sets VGA mode 0x13 (320x200x256 colors)

void _80_25_C()
// Sets text mode 0x03 (80x25x16 colors)
```

### Custom Video Mode Tables

The file contains custom video mode definitions for various resolutions:

#### 320x240 Mode
```c
vga_X320Y240 label word
  db 0e3h     // Dot clock
  dw 00d06h   // Vertical total
  dw 03e07h   // Overflow
  dw 04109h    // Cell height
  // ... more registers
```

#### 360x200 Mode
```c
vga_X360Y200 label word
  dw 06b00h   // Horizontal total
  dw 05901h   // Horizontal displayed
  // ...
```

#### 360x240 Mode
```c
vga_X360Y240 label word
  // Higher resolution version
```

#### 376x282 Mode
```c
vga_X376Y282 label word
  // Even higher resolution
```

## VGA Register Definitions

### CRTC (CRT Controller) Registers

| Register | Purpose |
|----------|---------|
| 0x00-0x07 | Horizontal timing |
| 0x08-0x0F | Vertical timing |
| 0x10-0x11 | Sync pulses |
| 0x12-0x13 | Screen format |
| 0x14-0x18 | Advanced |

### Sequencer Registers

Control character clock and display enable.

### Graphics Controller

Controls read/write modes and color dont't care.

### Attribute Controller

Controls palette and video enabled state.

## VGA Memory Organization

### Mode 0x13 Memory Map

```
Segment: A000h
Offset 0     = Pixel (0,0)
Offset 1     = Pixel (1,0)
Offset 2     = Pixel (2,0)
...
Offset 319   = Pixel (319,0)
Offset 320   = Pixel (0,1)
...
Offset 63999 = Pixel (319,199)
```

### Page Organization

The 64KB VGA memory is organized as a single bank in mode 0x13.

## Keyboard Utilities

### BIOS Keyboard Flags (0x417)

```
Bit 0: Right Shift pressed
Bit 1: Left Shift pressed
Bit 2: Ctrl pressed
Bit 3: Alt pressed
Bit 4: Scroll Lock
Bit 5: Num Lock
Bit 6: Caps Lock
Bit 7: Insert
```

### Keyboard Buffer

The BIOS maintains a circular buffer at 0x400:0x1E for keyboard input.

## Timer System

### BIOS Timer (0x46C)

- Location: BIOS data area segment 0x40, offset 0x6C
- Resolution: ~18.2 ticks per second (55ms)
- Used for: Frame timing, animation

## Color Palette

### Standard VGA Palette

The VGA DAC has 256 entries, each with 6-bit RGB:

```c
// Palette format: [R0,G0,B0,R1,G1,B1,...]
// Each component: 0-63
```

### Palette Registers

| Register | Purpose |
|---------|---------|
| 0x3C8 | Palette write index |
| 0x3C9 | Palette data (read/write) |
| 0x3C7 | Palette read index |

## Assembly Code Patterns

### Fast Memory Fill

```c
// Clear 64KB buffer
asm {
    les di, dword ptr buffer
    xor eax, eax
    mov cx, 16384
    rep stosd
}
```

### Fast Memory Copy

```c
// Copy 64KB
asm {
    push ds
    push es
    lds si, dword ptr source
    les di, dword ptr dest
    mov cx, 16000
    rep movsd
    pop es
    pop ds
}
```

### Port I/O

```c
// Out to VGA register
asm {
    mov dx, 0x3C0  // Attribute controller
    mov al, value
    out dx, al
}
```

## Video Mode Switching

### Enter Graphics Mode

```c
void set_video_mode_13h() {
    asm {
        mov ax, 0x13
        int 0x10
    }
}
```

### Return to Text Mode

```c
void set_video_mode_03h() {
    asm {
        mov ax, 0x03
        int 0x10
    }
}
```

## Performance Considerations

### Why Use Assembly

- **Speed**: Direct hardware access
- **Size**: Smaller code for tight loops
- **Control**: Access to special instructions

### Common Optimizations

1. **rep stosd/movsd**: Fast block operations
2. **32-bit operations**: Using 386+ instructions
3. **Register allocation**: Minimize memory access
4. **Loop unrolling**: Reduce branch overhead

## Memory Models

### Far Pointers (DOS)

```c
// 80x86 segment:offset addressing
// Video memory at A000:0000
unsigned char far *video = (unsigned char far *)0xA0000000L;
```

### Segment:Offset Calculation

```c
// To access pixel (x, y):
offset = y * 320 + x;
// Then: video[offset] = color;
```

## Hardware Interrupts Used

| IRQ | Purpose |
|-----|---------|
| 0x10 | Video services |
| 0x16 | Keyboard services |
| 0x33 | Mouse driver |
| 0x1C | Timer tick (user handler) |

## Error Handling

When accessing hardware:
- Always check return values
- Handle missing hardware gracefully
- Provide fallback modes

## Compatibility Notes

- Designed for IBM PC/AT and compatibles
- VGA required for graphics modes
- EGA compatible for some modes
- Works in real and protected modes

## Common Issues

1. **Screen flicker**: Use double buffering
2. **Color wrong**: Check palette initialization
3. **No video**: Verify VGA present
4. **Memory overwrite**: Check segment boundaries

## Double Buffering Pattern

```c
// Draw to offscreen buffer
unsigned char far *buffer = faralloc(64000);
unsigned char far *video = (unsigned char far *)0xA0000000L;

// Draw to buffer
buffer[320*y + x] = color;

// Flip to screen
farcopy(video, buffer);
```
