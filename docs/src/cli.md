# Adding Command-Line Arguments to Noctis IV Plus

## Overview

This guide covers adding CLI support to Noctis IV Plus for Borland C++ 3.1. The existing codebase already uses `_argv` in `reach_your_dir()`, so argument handling is a natural extension.

## Borland C++ 3.1 Compatibility

Borland C++ 3.1 has different conventions than modern compilers:

- **No `bool` type**: Use `char` or `int` (0/1)
- **No `std::string`**: Use C-style `char[]` arrays
- **No `strncpy`**: Use `strcpy` with manual bounds checking, or implement a safe version
- **No `stricmp`**: Use `strcmpi` (Borland extension) or manual case-insensitive comparison
- **No `atoi`/`atof` in some versions**: May need to use `sscanf` or custom conversion
- **`main()` returns `void`**: DOS programs typically use `void main()`

---

## Implementation

### 1. Define CLI Options Structure

Add to `NOCTIS-D.H`:

```c
/* Command-line options - Borland C++ 3.1 compatible */
#define CLI_MAX_PATH  64
#define CLI_MAX_DESC  32

typedef struct {
    /* Fuel/Physics (char for Borland compatibility) */
    char no_fuel_cost;
    char infinite_power;
    char no_radiation;
    char god_mode;           /* Combined: no fuel + no rad + max power */
    
    /* File overrides */
    char starmap_override[CLI_MAX_PATH];
    char guide_override[CLI_MAX_PATH];
    char surface_override[CLI_MAX_PATH];
    
    /* Starting position */
    long start_star_id;
    long start_planet_id;
    long start_moon_id;
    double start_x;
    double start_y;
    double start_z;
    
    /* Landing */
    char autoland;           /* Auto-land on startup */
    long land_planet_id;     /* Planet to land on */
    long land_latitude;      /* Landing latitude */
    long land_longitude;     /* Landing longitude */
    
    /* Export/Testing */
    char export_palette;     /* Export palette on startup */
    char export_textures;    /* Export textures on startup */
    char export_seed;        /* Export random seed info */
    char export_only;        /* Export and exit (no game) */
    char compare_mode;      /* Enable comparison output */
    
    /* Debug/Features */
    char show_debug;
    char skip_intro;
    char verbose;
} CLIOptions;

/* Global CLI instance */
extern CLIOptions cli_args;
```

### 2. Argument Parser (Borland C++ 3.1 Compatible)

Add to `NOCTIS-0.CPP`:

```c
#include <string.h>
#include <stdlib.h>

CLIOptions cli_args;

/* Safe string copy (Borland-compatible) */
static void cli_strcpy(char* dest, const char* src, int maxlen)
{
    int i = 0;
    while (src[i] && i < maxlen - 1) {
        dest[i] = src[i];
        i++;
    }
    dest[i] = '\0';
}

/* Case-insensitive compare (Borland has strcmpi) */
static int cli_strcmp(const char* a, const char* b)
{
    /* Simple implementation if strcmpi unavailable */
    while (*a && *b) {
        if (*a != *b) {
            /* Simple lower-case conversion */
            char ca = (*a >= 'A' && *a <= 'Z') ? (*a + 32) : *a;
            char cb = (*b >= 'A' && *b <= 'Z') ? (*b + 32) : *b;
            if (ca != cb) return 1;
        }
        a++; b++;
    }
    return (*a != *b);
}

/* Parse integer from string */
static long cli_atoi(const char* s)
{
    long result = 0;
    char neg = 0;
    
    if (*s == '-') { neg = 1; s++; }
    while (*s >= '0' && *s <= '9') {
        result = result * 10 + (*s - '0');
        s++;
    }
    return neg ? -result : result;
}

/* Parse double from string */
static double cli_atof(const char* s)
{
    double result = 0.0;
    double fraction = 0.1;
    char neg = 0;
    char in_frac = 0;
    
    if (*s == '-') { neg = 1; s++; }
    while (*s) {
        if (*s == '.') { in_frac = 1; s++; continue; }
        if (*s < '0' || *s > '9') break;
        if (!in_frac) {
            result = result * 10 + (*s - '0');
        } else {
            result += (*s - '0') * fraction;
            fraction *= 0.1;
        }
        s++;
    }
    return neg ? -result : result;
}

void parse_arguments(int argc, char** argv)
{
    int i;
    
    /* Initialize defaults */
    cli_args.no_fuel_cost = 0;
    cli_args.infinite_power = 0;
    cli_args.no_radiation = 0;
    cli_args.god_mode = 0;
    cli_args.starmap_override[0] = '\0';
    cli_args.guide_override[0] = '\0';
    cli_args.surface_override[0] = '\0';
    cli_args.start_star_id = -1;
    cli_args.start_planet_id = -1;
    cli_args.start_moon_id = -1;
    cli_args.start_x = 0.0;
    cli_args.start_y = 0.0;
    cli_args.start_z = 0.0;
    cli_args.autoland = 0;
    cli_args.land_planet_id = -1;
    cli_args.land_latitude = 60;  /* Default */
    cli_args.land_longitude = 0;
    cli_args.export_palette = 0;
    cli_args.export_textures = 0;
    cli_args.export_seed = 0;
    cli_args.export_only = 0;
    cli_args.compare_mode = 0;
    cli_args.show_debug = 0;
    cli_args.skip_intro = 0;
    cli_args.verbose = 0;
    
    for (i = 1; i < argc; i++) {
        char* arg = argv[i];
        
        /* Skip if doesn't start with - or / */
        if (arg[0] != '-' && arg[0] != '/') continue;
        arg++;  /* Skip the - or / */
        
        /* Fuel/Physics options */
        if (cli_strcmp(arg, "nofuel") == 0 || cli_strcmp(arg, "nf") == 0) {
            cli_args.no_fuel_cost = 1;
        }
        else if (cli_strcmp(arg, "infinite-power") == 0 || cli_strcmp(arg, "ip") == 0) {
            cli_args.infinite_power = 1;
        }
        else if (cli_strcmp(arg, "norad") == 0 || cli_strcmp(arg, "nr") == 0) {
            cli_args.no_radiation = 1;
        }
        else if (cli_strcmp(arg, "god") == 0 || cli_strcmp(arg, "g") == 0) {
            cli_args.god_mode = 1;
        }
        
        /* File overrides */
        else if (i + 1 < argc && cli_strcmp(arg, "starmap") == 0) {
            cli_strcpy(cli_args.starmap_override, argv[++i], CLI_MAX_PATH - 1);
        }
        else if (i + 1 < argc && cli_strcmp(arg, "guide") == 0) {
            cli_strcpy(cli_args.guide_override, argv[++i], CLI_MAX_PATH - 1);
        }
        else if (i + 1 < argc && cli_strcmp(arg, "surface") == 0) {
            cli_strcpy(cli_args.surface_override, argv[++i], CLI_MAX_PATH - 1);
        }
        
        /* Start position */
        else if (i + 1 < argc && cli_strcmp(arg, "star") == 0) {
            cli_args.start_star_id = cli_atoi(argv[++i]);
        }
        else if (i + 1 < argc && cli_strcmp(arg, "planet") == 0) {
            cli_args.start_planet_id = cli_atoi(argv[++i]);
        }
        else if (i + 1 < argc && cli_strcmp(arg, "moon") == 0) {
            cli_args.start_moon_id = cli_atoi(argv[++i]);
        }
        else if (i + 3 < argc && cli_strcmp(arg, "pos") == 0) {
            cli_args.start_x = cli_atof(argv[++i]);
            cli_args.start_y = cli_atof(argv[++i]);
            cli_args.start_z = cli_atof(argv[++i]);
        }
        
        /* Landing */
        else if (cli_strcmp(arg, "autoland") == 0 || cli_strcmp(arg, "al") == 0) {
            cli_args.autoland = 1;
            if (i + 1 < argc && argv[i + 1][0] >= '0' && argv[i + 1][0] <= '9') {
                cli_args.land_planet_id = cli_atoi(argv[++i]);
            }
        }
        else if (i + 1 < argc && cli_strcmp(arg, "land") == 0) {
            cli_args.autoland = 1;
            cli_args.land_planet_id = cli_atoi(argv[++i]);
            if (i + 1 < argc && argv[i + 1][0] >= '0' && argv[i + 1][0] <= '9') {
                cli_args.land_latitude = cli_atoi(argv[++i]);
            }
            if (i + 1 < argc && argv[i + 1][0] >= '0' && argv[i + 1][0] <= '9') {
                cli_args.land_longitude = cli_atoi(argv[++i]);
            }
        }
        
        /* Export/Testing */
        else if (cli_strcmp(arg, "export-palette") == 0 || cli_strcmp(arg, "epal") == 0) {
            cli_args.export_palette = 1;
        }
        else if (cli_strcmp(arg, "export-textures") == 0 || cli_strcmp(arg, "etex") == 0) {
            cli_args.export_textures = 1;
        }
        else if (cli_strcmp(arg, "export-seed") == 0 || cli_strcmp(arg, "eseed") == 0) {
            cli_args.export_seed = 1;
        }
        else if (cli_strcmp(arg, "export") == 0 || cli_strcmp(arg, "exp") == 0) {
            cli_args.export_palette = 1;
            cli_args.export_textures = 1;
            cli_args.export_seed = 1;
            cli_args.export_only = 1;
        }
        else if (cli_strcmp(arg, "compare") == 0 || cli_strcmp(arg, "cmp") == 0) {
            cli_args.compare_mode = 1;
        }
        
        /* Debug */
        else if (cli_strcmp(arg, "debug") == 0 || cli_strcmp(arg, "d") == 0) {
            cli_args.show_debug = 1;
        }
        else if (cli_strcmp(arg, "verbose") == 0 || cli_strcmp(arg, "v") == 0) {
            cli_args.verbose = 1;
        }
        else if (cli_strcmp(arg, "skipintro") == 0) {
            cli_args.skip_intro = 1;
        }
        
        /* Help */
        else if (cli_strcmp(arg, "help") == 0 || cli_strcmp(arg, "h") == 0 || cli_strcmp(arg, "?") == 0) {
            printf("\nNoctis IV Plus CLI Options:\n");
            printf("----------------------------\n");
            printf("Fuel/Physics:\n");
            printf("  -nofuel, -nf        Skip fuel consumption\n");
            printf("  -infinite-power, -ip Unlimited power\n");
            printf("  -norad, -nr         Disable radiation damage\n");
            printf("  -god, -g            God mode (all of above)\n");
            printf("\nFiles:\n");
            printf("  -starmap <file>     Alternate star map\n");
            printf("  -guide <file>       Alternate guide\n");
            printf("  -surface <file>     Alternate surface\n");
            printf("\nStart Position:\n");
            printf("  -star <id>          Start at star ID\n");
            printf("  -planet <id>        Start at planet ID\n");
            printf("  -moon <id>          Start at moon ID\n");
            printf("  -pos <x> <y> <z>    Start at coordinates\n");
            printf("\nLanding:\n");
            printf("  -autoland [planet]   Auto-land on startup\n");
            printf("  -land <p> [lat] [lon] Auto-land at coordinates\n");
            printf("\nExport/Compare:\n");
            printf("  -export-palette     Export palette to file\n");
            printf("  -export-textures    Export textures to files\n");
            printf("  -export-seed        Export random seed info\n");
            printf("  -export             Export all and exit\n");
            printf("  -compare            Enable comparison output\n");
            printf("\nDebug:\n");
            printf("  -debug, -d          Show debug info\n");
            printf("  -verbose, -v        Verbose output\n");
            printf("  -skipintro          Skip intro sequence\n");
            printf("  -help, -h, -?       This help\n");
        }
    }
}
```

### 3. Entry Point Integration

In `NOCTIS.CPP`, find or add the entry point:

```c
/* DOS entry point - no arguments passed by DOS shell */
void main(int argc, char** argv)
{
    /* Parse command line */
    /* Note: In DOS, we need to get __argc/__argv from C runtime */
    
    #ifdef __BORLANDC__
        extern int _argc;
        extern char** _argv;
        parse_arguments(_argc, _argv);
    #else
        parse_arguments(argc, argv);
    #endif
    
    /* Continue with normal initialization... */
}
```

---

## Use Case Implementation

### Use Case 1: Skip Fuel Cost

In `NOCTIS.CPP` around line 3224:

```c
/* Inside ap_drive_mode where power is consumed: */
if (!cli_args.no_fuel_cost && !cli_args.god_mode) {
    pwr -= l_dsd * 1E-5;
}

/* At game start or in status update: */
if (cli_args.infinite_power || cli_args.god_mode) {
    pwr = 999999;  /* Maximum power */
}
```

Similar hooks needed in planet approach code where `pwr` decreases.

### Use Case 2: Load Different BIN Files

In `NOCTIS-D.H`, change `#define` to `extern char[]`:

```c
/* Before: */
#define starmap_file "..\\DATA\\STARMAP.BIN"

/* After: */
extern char starmap_file_path[CLI_MAX_PATH];
extern char guide_file_path[CLI_MAX_PATH];
```

In initialization (after `parse_arguments()`):

```c
void init_cli_paths(void)
{
    /* Defaults */
    strcpy(starmap_file_path, "..\\DATA\\STARMAP.BIN");
    strcpy(guide_file_path, "..\\DATA\\GUIDE.BIN");
    
    /* CLI overrides */
    if (cli_args.starmap_override[0]) {
        strcpy(starmap_file_path, cli_args.starmap_override);
    }
    if (cli_args.guide_override[0]) {
        strcpy(guide_file_path, cli_args.guide_override);
    }
}
```

### Use Case 3: Start in Orbit / Specific Coordinates

```c
void apply_start_position(void)
{
    /* Direct coordinate start */
    if (cli_args.start_x != 0.0 || cli_args.start_y != 0.0 || cli_args.start_z != 0.0) {
        dzat_x = cli_args.start_x;
        dzat_y = cli_args.start_y;
        dzat_z = cli_args.start_z;
    }
    
    /* Planet orbit start */
    if (cli_args.start_planet_id >= 0) {
        ip_targetted = cli_args.start_planet_id;
        ip_reached = 1;
        
        /* Get planet position */
        planet_xyz(cli_args.start_planet_id);
        
        /* Position in orbit */
        dzat_x = plx + nearstar_p_ray[cli_args.start_planet_id] * 2.0;
        dzat_y = ply;
        dzat_z = plz;
    }
    
    /* Star orbit start */
    if (cli_args.start_star_id >= 0) {
        /* Set as remote target */
        ap_targetted = 1;
        ap_target_class = cli_args.start_star_id;
        /* Position near star */
    }
}
```

### Use Case 4: Autoland at Specific Coordinates

This is more involved as it requires landing sequence integration:

```c
void apply_autoland(void)
{
    if (!cli_args.autoland) return;
    
    /* Set target planet */
    if (cli_args.land_planet_id >= 0) {
        ip_targetted = cli_args.land_planet_id;
    }
    
    /* Set landing coordinates */
    landing_point = 1;
    landing_pt_lat = cli_args.land_latitude;
    landing_pt_lon = cli_args.land_longitude;
    
    /* Enable auto-approach */
    ip_reaching = 1;
    
    /* If in orbit, trigger approach */
    if (ip_reached) {
        status("AUTOLAND", 50);
    }
}
```

Call `apply_autoland()` after `apply_start_position()`.

### Use Case 5: Export Palettes and Textures

This is critical for verifying universe reproduction in alternate implementations.

```c
#include <stdio.h>

/* Export the main palette */
void export_palette(void)
{
    FILE* f;
    int i;
    
    f = fopen("noctis_palette.raw", "wb");
    if (!f) return;
    
    /* surface_palette is 768 bytes (256 colors x 3 RGB) */
    fwrite(surface_palette, 1, 768, f);
    fclose(f);
    
    /* Also export as text for comparison */
    f = fopen("noctis_palette.txt", "w");
    if (f) {
        for (i = 0; i < 256; i++) {
            fprintf(f, "%3d: %3d %3d %3d\n", i,
                surface_palette[i*3],
                surface_palette[i*3+1],
                surface_palette[i*3+2]);
        }
        fclose(f);
    }
}

/* Export texture maps */
void export_textures(void)
{
    FILE* f;
    char filename[32];
    int i;
    
    /* Export s_background (star field texture) */
    f = fopen("tex_starfield.raw", "wb");
    if (f) {
        fwrite(s_background, 1, st_bytes, f);
        fclose(f);
    }
    
    /* Export p_background (planet texture) */
    f = fopen("tex_planet.raw", "wb");
    if (f) {
        fwrite(p_background, 1, pl_bytes, f);
        fclose(f);
    }
    
    /* Export p_surfacemap (terrain heightmap) */
    f = fopen("tex_terrain.raw", "wb");
    if (f) {
        fwrite(p_surfacemap, 1, ps_bytes, f);
        fclose(f);
    }
    
    /* Export surface palette as separate file */
    f = fopen("tex_palette.raw", "wb");
    if (f) {
        fwrite(surface_palette, 1, 768, f);
        fclose(f);
    }
}

/* Export random seed information */
void export_seed_info(void)
{
    FILE* f;
    time_t t;
    
    f = fopen("seed_info.txt", "w");
    if (!f) return;
    
    /* Current seed values used */
    fprintf(f, "Random seed info:\n");
    fprintf(f, "-----------------\n");
    fprintf(f, "secs (time): %ld\n", (long)secs);
    fprintf(f, "detail_seed: %d\n", detail_seed);
    fprintf(f, "nearstar_identity: %.0f\n", nearstar_identity);
    
    /* Random function state if accessible */
    fprintf(f, "\nTo reproduce this universe:\n");
    fprintf(f, "1. Use secs value as initial seed\n");
    fprintf(f, "2. Call srand(%ld)\n", (long)secs);
    fprintf(f, "3. Generate star system in same order\n");
    
    fclose(f);
}

/* Export comparison-friendly format */
void export_comparison_data(void)
{
    FILE* f;
    
    /* JSON-like format for easier parsing */
    f = fopen("comparison.json", "w");
    if (f) {
        fprintf(f, "{\n");
        fprintf(f, "  \"seed\": %ld,\n", (long)secs);
        fprintf(f, "  \"detail_seed\": %d,\n", detail_seed);
        fprintf(f, "  \"nearstar_id\": %.0f,\n", nearstar_identity);
        fprintf(f, "  \"nearstar_class\": %d,\n", nearstar_class);
        fprintf(f, "  \"nearstar_nop\": %d,\n", nearstar_nop);
        fprintf(f, "  \"palette_size\": 768,\n");
        fprintf(f, "  \"starfield_size\": %d,\n", st_bytes);
        fprintf(f, "  \"planet_size\": %d,\n", pl_bytes);
        fprintf(f, "  \"terrain_size\": %d\n", ps_bytes);
        fprintf(f, "}\n");
        fclose(f);
    }
}

/* Main export function */
void run_exports(void)
{
    if (cli_args.verbose) {
        printf("Exporting data for comparison...\n");
    }
    
    if (cli_args.export_palette || cli_args.export_only) {
        export_palette();
        if (cli_args.verbose) printf("  Palette exported.\n");
    }
    
    if (cli_args.export_textures || cli_args.export_only) {
        export_textures();
        if (cli_args.verbose) printf("  Textures exported.\n");
    }
    
    if (cli_args.export_seed || cli_args.export_only) {
        export_seed_info();
        if (cli_args.verbose) printf("  Seed info exported.\n");
    }
    
    if (cli_args.compare_mode) {
        export_comparison_data();
        if (cli_args.verbose) printf("  Comparison data exported.\n");
    }
    
    if (cli_args.export_only) {
        /* Exit without starting game */
        if (cli_args.verbose) printf("Export complete. Exiting.\n");
        _80_25_C();
        exit(0);
    }
}
```

---

## Usage Examples

```bash
# Basic usage
NOCTIS -pos 50000 10000 -30000

# Orbit around planet 3
NOCTIS -planet 3

# God mode exploration
NOCTIS -god -pos 0 0 100000

# Autoland on planet 2 at coordinates 45, 120
NOCTIS -land 2 45 120

# Autoland at default coordinates on planet 1
NOCTIS -autoland 1

# Load custom star map
NOCTIS -starmap ..\custom\mystars.bin

# Export data for comparison, then exit
NOCTIS -export

# Export only palette
NOCTIS -export-palette

# Verbose export with debug
NOCTIS -export -verbose -debug

# Compare mode (creates comparison.json)
NOCTIS -compare -pos 10000 0 0
```

---

## DOS Batch File Launcher

For easier testing, create a batch file:

```batch
@echo off
REM Noctis Launcher with CLI options
REM Usage: launch.bat [options]

if "%1"=="" goto normal
NOCTIS.EXE %*
goto end

:normal
NOCTIS.EXE

:end
```

---

## Additional Export Ideas

### Star System Database Export
```c
void export_star_system(void)
{
    FILE* f = fopen("star_system.bin", "wb");
    if (f) {
        /* Export current star system data */
        fwrite(&nearstar_x, sizeof(nearstar_x), 1, f);
        fwrite(&nearstar_y, sizeof(nearstar_y), 1, f);
        fwrite(&nearstar_z, sizeof(nearstar_z), 1, f);
        fwrite(&nearstar_class, sizeof(nearstar_class), 1, f);
        fwrite(&nearstar_nop, sizeof(nearstar_nop), 1, f);
        /* Export planets */
        fwrite(nearstar_p_type, sizeof(nearstar_p_type), MAX_PLANETS, f);
        fwrite(nearstar_p_ray, sizeof(nearstar_p_ray), MAX_PLANETS, f);
        /* etc */
        fclose(f);
    }
}
```

### Frame-Accurate Reproduction
```c
/* Export initial state for exact reproduction */
void export_initial_state(void)
{
    /* Save everything needed to reproduce this exact frame */
}
```
