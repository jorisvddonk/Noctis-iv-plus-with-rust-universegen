# PV System - Polygon 3D Models

## Overview

The PV (Polygon Vertex) system handles loading, rendering, and manipulating 3D polygon models in Noctis. It's used for spacecraft, creatures, and other 3D objects.

## Data Structures

### `pvlist` - Polygon Definition

```c
struct pvlist {
    unsigned polygon_id : 12;  // Which polygon this vertex belongs to
    unsigned vtxflag_0  : 1;   // Vertex 0 active
    unsigned vtxflag_1  : 1;   // Vertex 1 active
    unsigned vtxflag_2  : 1;   // Vertex 2 active
    unsigned vtxflag_3  : 1;   // Vertex 3 active
};
```

Each entry describes a vertex belonging to a polygon.

### Model Handles

```c
#define vehicle_handle  0
#define bird_base      0
#define bird_result    1
#define mamm_base      2
#define mamm_result    3
```

Pre-defined model slots for quick access.

## Functions

### Loading Models

```c
extern char loadpv(int handle, long virtual_file_position,
                 float xscale, float yscale, float zscale,
                 float xmove, float ymove, float zmove,
                 unsigned char base_color, char depth_sort);
```

Loads a 3D model from a virtual file.

**Parameters:**
- `handle` - Model slot (0-15 typically)
- `virtual_file_position` - Offset in PV file
- `xscale, yscale, zscale` - Scale factors
- `xmove, ymove, zmove` - Position offset
- `base_color` - Base color for model
- `depth_sort` - Enable depth sorting

**Returns:**
- 1 on success, 0 on failure

### Unloading Models

```c
extern void unloadpv(int handle);
extern void unloadallpv(void);
```

Unload specific model or all models from memory.

### Rendering Models

```c
extern void drawpv(int handle, int mode, int rm_iterations,
                  float center_x, float center_y, float center_z,
                  char use_depth_sort);
```

Renders a loaded 3D model.

**Parameters:**
- `handle` - Model to render
- `mode` - Render mode (0=normal, 1=wireframe, etc.)
- `rm_iterations` - Smooth shading iterations
- `center_x, center_y, center_z` - Rotation center
- `use_depth_sort` - Enable depth sorting

### Modifying Models

```c
extern void modpv(int handle, int polygon_id, int vertex_id,
                 float x_scale, float y_scale, float z_scale,
                 float x_angle, float y_angle, float z_angle,
                 pvlist *vertexs_to_affect);
```

Applies transformation to specific vertices.

### Copying Models

```c
extern void copypv(int dest_handle, int src_handle);
```

Copies a model from one slot to another.

## Model File Format (.NCC)

NCC files contain 3D model data:

```
Header:
  - Number of vertices
  - Number of polygons
  - Bounding box

Vertex Data:
  - X, Y, Z coordinates (float)
  - Normal vector (for lighting)

Polygon Data:
  - Vertex indices (4 per quad)
  - Color/texture info
```

### Known NCC Files

| File | Description |
|------|-------------|
| MAMMAL.NCC | Creature/mammal model |
| BIRDY.NCC | Bird creature |
| VEHICLE.NCC | Player spacecraft |
| FACE.NCC | Humanoid face |
| ALPHABET.NCC | Letter models |

## Internal Data

### Vertex Buffers

```c
extern float far *vhxm, *vhym, *vhzm, *vh_mdq;
extern unsigned char far *vhnv, *vhcolor, *vhindex;
```

Coordinate and color buffers for loaded models.

### Inverse Transforms

```c
extern unsigned char far *inv, *icolor;
extern int alpha_n;
extern unsigned char far *alpha_nv;
extern float far *alpha_x, *alpha_y, *alpha_z;
```

Alpha blending and inverse transform data.

## Usage in Noctis

### Loading Vehicle

```c
vehicle_handle = loadpv(0, vehicle_offset,
                       1.0, 1.0, 1.0,  // Scale
                       0, 0, 0,        // Position
                       32,              // Base color
                       1);              // Depth sort
```

### Rendering Vehicle

```c
// In vehicle interior view
drawpv(vehicle_handle, 2, 2, 0.0, 0.0, 0.0, 0);

// In external view
drawpv(vehicle_handle, 0, 0, ovhx, ovhy, ovhz, 1);
```

### Render Modes

| Mode | Description |
|------|-------------|
| 0 | Solid fill |
| 1 | Wireframe |
| 2 | With effects |
| 3 | Transparent |

## Depth Sorting

Models can use depth sorting for correct rendering order:

```c
// Enable depth sort for complex scenes
drawpv(handle, 0, 2, x, y, z, 1);

// Disable for simple objects
drawpv(handle, 0, 0, x, y, z, 0);
```

## QuickSort for Depth

```c
extern void QuickSort(int far *index, float far *mdist, 
                    int start, int end);
```

Sorts polygons by distance for painter's algorithm.

## Memory Management

Model data is allocated from the `pv_bytes` buffer (20,480 bytes). Multiple models can be loaded simultaneously using different handles.

## Creating Custom Models

### Using Polyvert

Polyvert is a tool for creating NCC models:

1. Create vertices
2. Define polygons
3. Set colors
4. Save as .NCC

### Manual Format

```c
// Simple quad
float vertices[] = {
    -10.0f, -10.0f, 0.0f,  // V0
     10.0f, -10.0f, 0.0f,  // V1
     10.0f,  10.0f, 0.0f,  // V2
    -10.0f,  10.0f, 0.0f   // V3
};

int polygon[] = {0, 1, 2, 3};  // Quad
```

## Performance Tips

1. **Use depth sorting** only when needed
2. **Limit polygon count** per model
3. **Pre-scale models** rather than scaling at runtime
4. **Cache frequently used models** in dedicated slots
