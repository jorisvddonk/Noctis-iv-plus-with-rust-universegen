/* Throwaway probe: extract the far-heap bytes immediately after p_surfacemap
   so the Rust port can replicate the OOB alias read in build_surface's
   inclination loop. Does NOT modify the reference generation code. */
#include "defs.h"
#include "noctis-d.h"
#include "noctis-0.h"
#include "nivhash.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <math.h>
#include <alloc.h>

extern void build_surface(void);
extern char sctype;

static unsigned char far  *s_background;
static unsigned char far  *p_background;
static unsigned char huge *p_surfacemap;
static quadrant far       *objectschart;
static unsigned char far  *txtr;
static unsigned char far  *ruinschart;
static unsigned m200[200];

static void alloc_buffers(void) {
    int i;
    s_background = (unsigned char far *)  farmalloc(st_bytes);
    p_background = (unsigned char far *)  farmalloc(pl_bytes);
    p_surfacemap = (unsigned char huge *) farmalloc(ps_bytes);
    objectschart = (quadrant far *)       farmalloc(oc_bytes);
    ruinschart   = (unsigned char far *)  objectschart;
    txtr         = (unsigned char huge *) p_background;
    for (i = 0; i < 200; i++) m200[i] = (unsigned)(i * 200);
}

static void gen_system(double x, double y, double z) {
    ap_target_x = x; ap_target_y = y; ap_target_z = z;
    _delay = 0;
    extract_ap_target_infos();
    prepare_nearstar();
}

int main(int argc, char **argv) {
    double x = atof(argv[1]), y = atof(argv[2]), z = atof(argv[3]);
    int p = atoi(argv[4]);
    int lon = atoi(argv[5]), lat = atoi(argv[6]);
    gen_system(x, y, z);
    if (p < 0 || p >= nearstar_nob) return 2;
    alloc_buffers();
    gen_planet_surface(p);
    ip_targetted = (char)p;
    landing_pt_lon = lon;
    landing_pt_lat = lat;
    landing_point = 1;
    latitude = (float)(abs(landing_pt_lat - 60)) * 1.5;
    nightzone = 0;
    albedo = 32;
    dsd = 1000.0; dsd1 = 1000.0; dsd2 = 1000.0;
    nray1 = nearstar_ray; nray2 = nearstar_ray;
    sun_x = -1000.0; sun_y = 0.0; sun_z = 0.0;
    global_surface_seed = (long)((nearstar_p_ray[p] + nearstar_p_orb_ray[p] + nearstar_p_orb_orient[p]) * 4112.0);
    if (nearstar_p_type[p] == 3) {
        srand(global_surface_seed + landing_pt_lon);
        if (latitude > 25 + (global_surface_seed % 15) + random(5)) global_surface_seed++;
    }
    if (nearstar_p_type[p] == 3) {
        srand(landing_pt_lon * landing_pt_lat);
        if (random(100) > 5) { long cpos = (long)(555 * nearstar_p_orb_orient[p]); sctype = (char)((cpos % 4) + 1); }
        else sctype = (char)(random(4) + 1);
        if (albedo < 25) sctype = 1;
        if (latitude > 75) sctype = 4;
        if (latitude > 60 && random(3)) sctype = 4;
    } else sctype = 2;
    build_surface();
    FILE *f = fopen("D:\\HEAPPROBE.BIN", "wb");
    if (f) {
        unsigned char far *base = p_surfacemap + ps_bytes;
        for (int i = 0; i < 512; i++) fputc((int)base[i], f);
        fclose(f);
    }
    return 0;
}
