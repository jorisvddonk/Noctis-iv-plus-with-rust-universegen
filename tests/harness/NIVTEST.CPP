/*
	=======================================================================
	NIVTEST.CPP - Noctis IV Plus universe-generation test harness
	=======================================================================

	PURPOSE
	-------
	Drive the REAL, byte-for-byte unmodified Noctis IV Plus generation code
	(NOCTIS-0.CPP / NOCTIS-1.CPP, compiled to .OBJ) and emit small, stable
	fingerprints (and optionally raw dumps) of every generated artifact, so
	that a re-implementation in another language can be checked for exact,
	bug-for-bug equivalence.

	This file NEVER modifies generation logic. It only:
	   * allocates the same far/huge buffers main() would,
	   * seeds the same global inputs (Parsis coordinates, landing point...),
	   * calls the same generation functions,
	   * hashes / dumps the resulting global state and buffers.

	The harness is intentionally driven entirely by command-line arguments so
	it can be scripted from the macOS wrapper.

	COMMAND LINE
	------------
	  NIVTEST <command> [options]

	Global options (any command):
	  -secs <n>          set the global time counter `secs` (default 0).
	                     Some texture/cloud details depend on it; keep fixed.
	  -dump <DIR>        also write raw binary dumps of buffers to DIR (DOS
	                     8.3 dir on the mounted drive), in addition to hashes.
	  -o <FILE>          append textual results to FILE (default: stdout).

	Commands:
	  selftest
	      Emit hash self-test vectors + a fixed rand()/fast_random() sequence.
	      Lets a port verify the hashing + RNG before anything else.

	  system  -x <X> -y <Y> -z <Z>
	      Solar-system generation for the star at Parsis coordinates (X,Y,Z).
	      Runs extract_ap_target_infos()+prepare_nearstar() and dumps every
	      star & body property (class, ray, spin, nop, per-body type/ray/tilt/
	      spin/orbit/ring/moon relationships...).  (TEST SET 1)

	  planet  -x <X> -y <Y> -z <Z> -p <index>
	      Planet texture / global-map generation for body <index> of the star
	      at (X,Y,Z). Calls surface() exactly as planets() would and hashes
	      p_background (the 360x180 planetary surface map), the atmosphere
	      overlay, the palette, and the derived rotation/terminator values.
	      (TEST SET 2)

	  planet-all  -x <X> -y <Y> -z <Z>
	      Convenience: run `planet` for every body in the system.

	  sector  -x <X> -y <Y> -z <Z> -p <index> -lon <L> -lat <B> [-sc <type>]
	      Surface SECTOR generation for a landing at longitude L / latitude B.
	      Calls build_surface() and hashes the p_surfacemap heightmap and the
	      objectschart object map. (TEST SET 3)

	  surftex  -x <X> -y <Y> -z <Z> -p <index> -lon <L> -lat <B> [-sc <type>]
	      Same setup as `sector` but focuses on the surface TEXTURE buffer
	      (txtr) and terrain-detail parameters. (TEST SET 4)

	  starsystem-scan -x <X> -y <Y> -z <Z>
	      (TEST SET 5 / extra) One-line summary hash of a whole system, handy
	      for scanning many coordinates quickly.

	Notes on determinism
	--------------------
	The generator mixes two RNGs:
	   * random()/rand()  - Borland's LCG, seeded by srand()
	   * fast_random()    - Noctis' own self-multiplying generator
	Both are re-seeded deterministically from the inputs inside the
	generation functions, so given identical inputs the output is identical.
*/

#include "defs.h"      /* defines Word/Dword used by noctis-0.h */
#include "noctis-d.h"
#include "noctis-0.h"
#include "nivhash.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <math.h>
#include <alloc.h>

/* Functions living in NOCTIS-1.CPP but not declared in noctis-0.h. */
extern void build_surface(void);
extern void create_sky(char atmosphere);
extern char sctype;

/*
	nearstar_p_orb_seed is DEFINED in NOCTIS-0.CPP but (unlike the other
	nearstar_p_* arrays) is not declared in noctis-0.h. We declare it here
	rather than touch the original header.
*/
extern double nearstar_p_orb_seed[maxbodies];

/* Scenario type constants (from noctis-1.cpp) - for readability in output. */
#define SC_OCEAN  1
#define SC_PLAINS 2
#define SC_DESERT 3
#define SC_ICY    4

/* ----------------------------------------------------------------------- */
/* output routing                                                          */
/* ----------------------------------------------------------------------- */

static FILE *g_out = NULL;
static char  g_dumpdir[64] = "";

static void out(const char *fmt, ...)
{
	va_list ap;
	va_start(ap, fmt);
	vfprintf(g_out ? g_out : stdout, fmt, ap);
	va_end(ap);
}

/* Write a raw far buffer to <dumpdir>\<name> if dumping is enabled. */
static void dump_far(const char *name, const unsigned char huge *buf, nh_u32 len)
{
	char path[96];
	FILE *f;
	nh_u32 i;
	if (!g_dumpdir[0]) return;
	sprintf(path, "%s\\%s", g_dumpdir, name);
	f = fopen(path, "wb");
	if (!f) { out("; WARN cannot open dump %s\n", path); return; }
	for (i = 0; i < len; i++)
		fputc((int)buf[i], f);
	fclose(f);
}

/* Emit both hashes of a far buffer on a single labelled line. */
static void hashline(const char *label, const unsigned char huge *buf, nh_u32 len)
{
	nh_u32 fnv = nh_fnv1a_far(NH_FNV_OFFSET, buf, len);
	nh_u32 crc = nh_crc32_final(nh_crc32_far(0xFFFFFFFFUL, buf, len));
	out("%-16s len=%-6lu fnv=%08lX crc=%08lX\n",
	    label, (unsigned long)len, (unsigned long)fnv, (unsigned long)crc);
}

/* ----------------------------------------------------------------------- */
/* buffer allocation - mirror the essential part of NOCTIS.CPP main()      */
/* ----------------------------------------------------------------------- */

static int alloc_buffers(void)
{
	int i;

	s_background = (unsigned char far *)  farmalloc(st_bytes);
	p_background = (unsigned char far *)  farmalloc(pl_bytes);
	p_surfacemap = (unsigned char huge *) farmalloc(ps_bytes);
	objectschart = (quadrant far *)       farmalloc(oc_bytes);
	ruinschart   = (unsigned char far *)  objectschart;   /* oc alias */
	txtr         = (unsigned char huge *) p_background;   /* txtr alias */

	if (!s_background || !p_background || !p_surfacemap || !objectschart)
		return 0;

	/* m200 lookup table (NOCTIS.CPP main sets this up). */
	for (i = 0; i < 200; i++) m200[i] = (unsigned)(i * 200);

	/* sine/cosine lookup tables (lens_flares_init in NOCTIS-0.CPP). */
	lens_flares_init();

	return 1;
}

/* ----------------------------------------------------------------------- */
/* argument helpers                                                        */
/* ----------------------------------------------------------------------- */

static double argd(int argc, char **argv, const char *key, double def)
{
	int i;
	for (i = 1; i < argc - 1; i++)
		if (strcmp(argv[i], key) == 0) return atof(argv[i + 1]);
	return def;
}
static long argl(int argc, char **argv, const char *key, long def)
{
	int i;
	for (i = 1; i < argc - 1; i++)
		if (strcmp(argv[i], key) == 0) return atol(argv[i + 1]);
	return def;
}
static int has(int argc, char **argv, const char *key)
{
	int i;
	for (i = 1; i < argc; i++)
		if (strcmp(argv[i], key) == 0) return 1;
	return 0;
}
static const char *args(int argc, char **argv, const char *key, const char *def)
{
	int i;
	for (i = 1; i < argc - 1; i++)
		if (strcmp(argv[i], key) == 0) return argv[i + 1];
	return def;
}

/* ----------------------------------------------------------------------- */
/* shared setup: place a star at (x,y,z) and generate its system           */
/* ----------------------------------------------------------------------- */

static void gen_system(double x, double y, double z)
{
	/* Feed the star's Parsis coordinates into the autopilot-target globals,
	   exactly the values the game would hold when a star is targeted, then
	   run the real two-stage generation. */
	ap_target_x = x;
	ap_target_y = y;
	ap_target_z = z;

	_delay = 0;               /* so prepare_nearstar copies ap_target_* */
	extract_ap_target_infos();/* -> ap_target_class/ray/rgb/spin        */
	prepare_nearstar();       /* -> nearstar_* + all bodies             */
}

/* Emit the full per-body property table for the current nearstar. */
static void dump_system_props(void)
{
	int n;

	out("STAR x=%.0f y=%.0f z=%.0f\n", ap_target_x, ap_target_y, ap_target_z);
	out("  identity=%.6f\n", nearstar_identity);
	out("  class=%d ray=%.6f spin=%d rgb=%d,%d,%d\n",
	    (int)nearstar_class, (double)nearstar_ray, (int)nearstar_spin,
	    (int)nearstar_r, (int)nearstar_g, (int)nearstar_b);
	out("  nop=%d nob=%d labeled=%d\n",
	    nearstar_nop, nearstar_nob, nearstar_labeled);

	for (n = 0; n < nearstar_nob; n++) {
		out("  BODY %2d type=%d owner=%d moonid=%d "
		    "ray=%.6f orb_ray=%.6f\n",
		    n, (int)nearstar_p_type[n], nearstar_p_owner[n],
		    (int)nearstar_p_moonid[n],
		    (double)nearstar_p_ray[n], (double)nearstar_p_orb_ray[n]);
		out("          tilt=%.8f orb_tilt=%.8f orb_orient=%.8f "
		    "orb_ecc=%.8f orb_seed=%.6f ring=%.6f\n",
		    (double)nearstar_p_tilt[n], (double)nearstar_p_orb_tilt[n],
		    (double)nearstar_p_orb_orient[n], (double)nearstar_p_orb_ecc[n],
		    (double)nearstar_p_orb_seed[n], (double)nearstar_p_ring[n]);
	}
}

/* Compute the exact seedval planets() passes to surface() for body n. */
static double surface_seedval_for(int n)
{
	int is_moon = (nearstar_p_owner[n] > -1);
	if (is_moon) {
		if (nearstar_p_type[n])
			return 1000000.0 * nearstar_ray * nearstar_p_type[n]
			     * nearstar_p_orb_orient[n];
		else
			return 2000000.0 * n * nearstar_ray
			     * nearstar_p_orb_orient[n];
	} else {
		if (nearstar_p_type[n])
			return 1000000.0 * nearstar_p_type[n]
			     * nearstar_p_orb_seed[n] * nearstar_p_orb_tilt[n]
			     * nearstar_p_orb_ecc[n] * nearstar_p_orb_orient[n];
		else
			return 2000000.0 * n
			     * nearstar_p_orb_seed[n] * nearstar_p_orb_tilt[n]
			     * nearstar_p_orb_ecc[n] * nearstar_p_orb_orient[n];
	}
}

/* ----------------------------------------------------------------------- */
/* TEST SET 1 : solar system                                               */
/* ----------------------------------------------------------------------- */

static int cmd_system(int argc, char **argv)
{
	double x = argd(argc, argv, "-x", 0);
	double y = argd(argc, argv, "-y", 0);
 	double z = argd(argc, argv, "-z", 0);
	gen_system(x, y, z);
 	out("=== SYSTEM ===\n");
	dump_system_props();
	return 0;
}

/*
	find - scan a coordinate grid and print coordinates of systems matching
	criteria. Used to build reproducible, exhaustive test fixtures that cover
	every star class and every planet type. Purely a discovery helper; it
	does not itself test anything.

	Options:
	  -x0/-y0/-z0     grid origin (defaults from ship start position)
	  -step           grid spacing (default 100000, the star-cell size)
	  -n              number of steps per axis (default 12)
	  -class <c>      only report systems of star class c (-1 = any)
	  -ptype <t>      only report systems containing >=1 body of planet type t
	  -minnop <k>     only report systems with at least k planets
	  -max <m>        stop after m matches (default 40)
*/
static int cmd_find(int argc, char **argv)
{
	double x0 = argd(argc, argv, "-x0", 3800000.0);
	double y0 = argd(argc, argv, "-y0", -4300000.0);
	double z0 = argd(argc, argv, "-z0", -900000.0);
	double step = argd(argc, argv, "-step", 100000.0);
	int n    = (int)argl(argc, argv, "-n", 12);
	int wclass = (int)argl(argc, argv, "-class", -1);
	int wptype = (int)argl(argc, argv, "-ptype", -1);
	int minnop = (int)argl(argc, argv, "-minnop", 0);
	int maxm   = (int)argl(argc, argv, "-max", 40);
	int ix, iy, iz, k, found = 0;
	double x, y, z;

	out("=== FIND (class=%d ptype=%d minnop=%d) ===\n",
	    wclass, wptype, minnop);
	for (ix = 0; ix < n && found < maxm; ix++)
	for (iy = 0; iy < n && found < maxm; iy++)
	for (iz = 0; iz < n && found < maxm; iz++) {
		int has_ptype = 0;
		x = x0 + ix * step;
		y = y0 + iy * step;
		z = z0 + iz * step;
		gen_system(x, y, z);
		if (wclass >= 0 && (int)nearstar_class != wclass) continue;
		if (nearstar_nop < minnop) continue;
		if (wptype >= 0) {
			for (k = 0; k < nearstar_nob; k++)
				if ((int)nearstar_p_type[k] == wptype) has_ptype = 1;
			if (!has_ptype) continue;
		}
		out("FOUND x=%.0f y=%.0f z=%.0f class=%d nop=%d nob=%d\n",
		    x, y, z, (int)nearstar_class, nearstar_nop, nearstar_nob);
		found++;
	}
	out("; %d matches\n", found);
	return 0;
}

/* Compact one-line-ish system fingerprint (TEST SET 5). */
static int cmd_scan(int argc, char **argv)
{
	double x = argd(argc, argv, "-x", 0);
	double y = argd(argc, argv, "-y", 0);
	double z = argd(argc, argv, "-z", 0);
	nh_u32 h = NH_FNV_OFFSET;
	int n;
	char rec[64];

	gen_system(x, y, z);

	/* hash the essential, port-relevant scalar outputs */
	sprintf(rec, "%d %d %d %d",
	        (int)nearstar_class, nearstar_nop, nearstar_nob,
	        (int)nearstar_spin);
	h = nh_fnv1a_update(h, rec, strlen(rec));
	for (n = 0; n < nearstar_nob; n++) {
		sprintf(rec, "%d %d %d", (int)nearstar_p_type[n],
		        nearstar_p_owner[n], (int)nearstar_p_moonid[n]);
		h = nh_fnv1a_update(h, rec, strlen(rec));
	}
	out("SCAN x=%.0f y=%.0f z=%.0f class=%d nop=%d nob=%d fnv=%08lX\n",
	    x, y, z, (int)nearstar_class, nearstar_nop, nearstar_nob,
	    (unsigned long)h);
	return 0;
}

/* ----------------------------------------------------------------------- */
/* TEST SET 2 : planet textures / global surface map                       */
/* ----------------------------------------------------------------------- */

static void gen_planet_surface(int n)
{
	int is_moon = (nearstar_p_owner[n] > -1);
	unsigned char colorbase = is_moon ? 128 : 192;
	double seedval = surface_seedval_for(n);

	/* surface() writes to p_background for planets, s_background for moons;
	   planets() swaps p_background accordingly. We mirror that here. */
	if (is_moon) {
		unsigned char far *save = p_background;
		p_background = s_background;
		surface(n, nearstar_p_type[n], seedval, colorbase);
		p_background = save;
	} else {
		surface(n, nearstar_p_type[n], seedval, colorbase);
	}
}

static void report_planet(int n)
{
	int is_moon = (nearstar_p_owner[n] > -1);
	unsigned char far *surf = is_moon ? s_background : p_background;

	out("PLANET body=%d type=%d (%s) owner=%d seedval=%.6f\n",
	    n, (int)nearstar_p_type[n],
	    (nearstar_p_type[n] <= 10 ?
	        planet_description[nearstar_p_type[n]] : "?"),
	    nearstar_p_owner[n], surface_seedval_for(n));
	out("  rtperiod=%d rotation=%d term_start=%d term_end=%d\n",
	    nearstar_p_rtperiod[n], nearstar_p_rotation[n],
	    nearstar_p_term_start[n], nearstar_p_term_end[n]);

	if (nearstar_p_type[n] == 10) {
		out("  (companion star - surface() is a no-op)\n");
		return;
	}

	/* The surface map buffer is 360*180 = 64800 bytes, but the globe renderer
	   (globe(), NOCTIS-0.CPP:3044, via GLOBES.MAP) never samples beyond row 121.
	   We hash only rows 0..127 (46080 bytes): this covers the full visible band
	   with rotation margin and excludes the scratch tail (rows 128..179) where
	   the moon far-heap OOB read (lssmooth past s_background) lands. That tail is
	   never displayed and is nondeterministic, so it must not enter the hash. */
	hashline("surface_map", surf, 46080UL);
	/* atmosphere overlay lives in objectschart (32400 bytes used). */
	hashline("atmo_overlay", (unsigned char huge *)objectschart, 32400UL);
	/* the two 64-entry palette blocks surface() rewrote (192.. or 128..). */
	{
		int base3 = (is_moon ? 128 : 192) * 3;
		hashline("palette64", (unsigned char huge *)(tmppal + base3), 192UL);
	}

	dump_far("surfmap.bin", surf, 64800UL);
	dump_far("atmover.bin", (unsigned char huge *)objectschart, 32400UL);
}

static int cmd_planet(int argc, char **argv)
{
	double x = argd(argc, argv, "-x", 0);
	double y = argd(argc, argv, "-y", 0);
	double z = argd(argc, argv, "-z", 0);
	int    p = (int)argl(argc, argv, "-p", 0);

	gen_system(x, y, z);
	if (p < 0 || p >= nearstar_nob) {
		out("; ERROR body index %d out of range (nob=%d)\n", p, nearstar_nob);
		return 2;
	}
	out("=== PLANET TEXTURE ===\n");
	gen_planet_surface(p);
	report_planet(p);
	return 0;
}

static int cmd_planet_all(int argc, char **argv)
{
	double x = argd(argc, argv, "-x", 0);
	double y = argd(argc, argv, "-y", 0);
	double z = argd(argc, argv, "-z", 0);
	int n;

	gen_system(x, y, z);
	out("=== PLANET TEXTURES (all %d bodies) ===\n", nearstar_nob);
	for (n = 0; n < nearstar_nob; n++) {
		gen_planet_surface(n);
		report_planet(n);
	}
	return 0;
}

/* ----------------------------------------------------------------------- */
/* TEST SETS 3 & 4 : surface sector heightmap + surface textures           */
/* ----------------------------------------------------------------------- */

/*
	build_surface() consumes a set of "landing context" globals that the game
	computes in planets()/planetary_main() from the landing longitude/latitude
	and the planet's albedo. To exercise build_surface() deterministically we
	reproduce exactly those *input* assignments (this is harness setup, not a
	re-implementation of any generation algorithm):

	   latitude          = |landing_pt_lat - 60| * 1.5          (planetary_main)
	   global_surface_seed = (ray+orb_ray+orb_orient)*4112 ...   (planetary_main)
	   sctype            = derived from orbit orientation/albedo (planetary_main)
	   albedo, nightzone = derived by planets() from the surface map

	Because those derivations themselves live in the generation modules, the
	harness lets the caller either pass explicit values (fully controlled,
	reproducible) OR fall back to computing the minimal ones it needs.
*/
static void setup_landing(int argc, char **argv, int n)
{
	int lon = (int)argl(argc, argv, "-lon", 0);
	int lat = (int)argl(argc, argv, "-lat", 60);
	long albedo_arg = argl(argc, argv, "-albedo", -1);
	int  sctype_arg = (int)argl(argc, argv, "-sc", -1);
	extern float latitude;
	extern float dsd1, dsd2, sun_x, sun_y, sun_z, nray1, nray2;

	ip_targetted   = (char)n;
	landing_pt_lon = lon;
	landing_pt_lat = lat;
	landing_point  = 1;

	/* latitude as planetary_main computes it */
	latitude = (float)(abs(landing_pt_lat - 60)) * 1.5;

	/* nightzone: default day unless caller overrides */
	nightzone = (int)argl(argc, argv, "-night", 0);

	/* albedo: caller supplies it for full reproducibility (0..63). If not
	   given, use a fixed mid value so results stay stable & documented. */
	albedo = (albedo_arg >= 0) ? (int)albedo_arg : 32;

	/* a plausible, fixed sun geometry so shading delta is deterministic */
	dsd  = 1000.0;
	dsd1 = 1000.0; dsd2 = 1000.0;
	nray1 = nearstar_ray; nray2 = nearstar_ray;
	sun_x = -1000.0; sun_y = 0.0; sun_z = 0.0;

	/* global_surface_seed exactly as planetary_main derives it */
	global_surface_seed = (long)((nearstar_p_ray[n]
	                    + nearstar_p_orb_ray[n]
	                    + nearstar_p_orb_orient[n]) * 4112.0);
	if (nearstar_p_type[n] == 3) {
		srand(global_surface_seed + landing_pt_lon);
		if (latitude > 25 + (global_surface_seed % 15) + random(5))
			global_surface_seed++;
	}

	/* scenario type: for felisian (type 3) worlds derive it as
	   planetary_main does, unless the caller forces one via -sc. */
	if (sctype_arg >= 1 && sctype_arg <= 4) {
		sctype = (char)sctype_arg;
	} else if (nearstar_p_type[n] == 3) {
		long cpos;
		srand(landing_pt_lon * landing_pt_lat);
		if (random(100) > 5) {
			cpos = (long)(555 * nearstar_p_orb_orient[n]);
			sctype = (char)((cpos % 4) + 1);
		} else {
			sctype = (char)(random(4) + 1);
		}
		if (albedo < 25) sctype = SC_OCEAN;
		if (latitude > 75) sctype = SC_ICY;
		if (latitude > 60 && random(3)) sctype = SC_ICY;
	} else {
		sctype = SC_PLAINS; /* unused by non-felisian build_surface paths */
	}
}

static void report_sector_heightmap(int n)
{
	out("SECTOR body=%d type=%d lon=%d lat=%d sctype=%d albedo=%d night=%d\n",
	    n, (int)nearstar_p_type[n], landing_pt_lon, landing_pt_lat,
	    (int)sctype, albedo, nightzone);
	out("  global_surface_seed=%ld\n", global_surface_seed);

	/* p_surfacemap is the 200x200 = 40000 byte height field. */
	hashline("heightmap", p_surfacemap, 40000UL);
	/* objectschart is the 40000 byte object placement map. */
	hashline("objectchart", (unsigned char huge *)objectschart, 40000UL);

	dump_far("height.bin", p_surfacemap, 40000UL);
	dump_far("objects.bin", (unsigned char huge *)objectschart, 40000UL);
}

static void report_surface_texture(int n)
{
	extern long  T_SCALE;
	extern char  quartz, groundflares, gtx;
	extern float rockscaling, rockpeaking, treescaling;
	extern int   rockdensity;

	out("SURFTEX body=%d type=%d lon=%d lat=%d sctype=%d albedo=%d\n",
	    n, (int)nearstar_p_type[n], landing_pt_lon, landing_pt_lat,
	    (int)sctype, albedo);
	out("  T_SCALE=%ld quartz=%d groundflares=%d gtx=%d\n",
	    T_SCALE, (int)quartz, (int)groundflares, (int)gtx);
	out("  rockscaling=%.3f rockpeaking=%.3f rockdensity=%d treescaling=%.3f\n",
	    rockscaling, rockpeaking, rockdensity, treescaling);

	/* txtr is aliased onto p_background (pl_bytes = 65552). The surface
	   texture occupies a 256x256 = 65536 byte region. */
	hashline("surf_texture", txtr, 65536UL);
	dump_far("surftex.bin", txtr, 65536UL);
}

static int cmd_sector(int argc, char **argv)
{
	double x = argd(argc, argv, "-x", 0);
	double y = argd(argc, argv, "-y", 0);
	double z = argd(argc, argv, "-z", 0);
	int    p = (int)argl(argc, argv, "-p", 0);

	gen_system(x, y, z);
	if (p < 0 || p >= nearstar_nob) {
		out("; ERROR body index %d out of range (nob=%d)\n", p, nearstar_nob);
		return 2;
	}
	/* build_surface reads planet rotation/terminator too; make sure the
	   planetary map (and thus rtperiod etc.) exists first, like the game. */
	gen_planet_surface(p);
	setup_landing(argc, argv, p);
	out("=== SURFACE SECTOR (heightmap) ===\n");
	build_surface();
	report_sector_heightmap(p);
	return 0;
}

static int cmd_surftex(int argc, char **argv)
{
	double x = argd(argc, argv, "-x", 0);
	double y = argd(argc, argv, "-y", 0);
	double z = argd(argc, argv, "-z", 0);
	int    p = (int)argl(argc, argv, "-p", 0);

	gen_system(x, y, z);
	if (p < 0 || p >= nearstar_nob) {
		out("; ERROR body index %d out of range (nob=%d)\n", p, nearstar_nob);
		return 2;
	}
	gen_planet_surface(p);
	setup_landing(argc, argv, p);
	out("=== SURFACE SECTOR (texture) ===\n");
	build_surface();
	report_surface_texture(p);
	return 0;
}

/* ----------------------------------------------------------------------- */
/* selftest : hash vectors + RNG sequences                                 */
/* ----------------------------------------------------------------------- */

static int cmd_selftest(int argc, char **argv)
{
	static const char *v[] = { "", "a", "abc",
		"The quick brown fox jumps over the lazy dog" };
	int i, k;
	(void)argc; (void)argv;

	out("=== SELFTEST ===\n");
	out("-- hash vectors --\n");
	for (i = 0; i < 4; i++) {
		nh_u32 f = nh_fnv1a(v[i], (unsigned long)strlen(v[i]));
		nh_u32 c = nh_crc32(v[i], (unsigned long)strlen(v[i]));
		out("  \"%s\" fnv=%08lX crc=%08lX\n", v[i],
		    (unsigned long)f, (unsigned long)c);
	}

	out("-- Borland random() sequence, srand(1) --\n");
	srand(1);
	for (k = 0; k < 8; k++) out("  rand=%d\n", rand());

	out("-- fast_random() sequence, fast_srand(12345) --\n");
	fast_srand(12345L);
	for (k = 0; k < 8; k++) out("  fast=%ld\n", fast_random(0x7FFFL));

	out("-- flandom()/fast_flandom() --\n");
	srand(1);
	out("  flandom=%.8f\n", (double)flandom());
	fast_srand(1L);
	out("  fast_flandom=%.8f\n", (double)fast_flandom());
	return 0;
}

/* ----------------------------------------------------------------------- */
/* main                                                                    */
/* ----------------------------------------------------------------------- */

static void usage(void)
{
	printf("NIVTEST - Noctis IV Plus generation test harness\n");
	printf("commands: selftest | system | planet | planet-all |\n");
	printf("          sector | surftex | scan\n");
	printf("see NIVTEST.CPP header for full option list.\n");
}

int main(int argc, char **argv)
{
	const char *ofile;
	const char *cmd;
	int rc;

	if (argc < 2) { usage(); return 1; }
	cmd = argv[1];

	/* output file */
	ofile = args(argc, argv, "-o", (const char *)0);
	if (ofile) {
		g_out = fopen(ofile, "a");
		if (!g_out) { printf("cannot open output %s\n", ofile); return 1; }
	}
	/* dump dir */
	{
		const char *d = args(argc, argv, "-dump", (const char *)0);
		if (d) strcpy(g_dumpdir, d);
	}
	/* fixed time counter for reproducibility */
	secs = (double)argl(argc, argv, "-secs", 0L);

	if (strcmp(cmd, "selftest") == 0) {
		rc = cmd_selftest(argc, argv);
	} else {
		if (!alloc_buffers()) {
			out("; FATAL not enough memory to allocate buffers\n");
			if (g_out) fclose(g_out);
			return 3;
		}
		if      (strcmp(cmd, "system")     == 0) rc = cmd_system(argc, argv);
		else if (strcmp(cmd, "scan")       == 0) rc = cmd_scan(argc, argv);
		else if (strcmp(cmd, "find")       == 0) rc = cmd_find(argc, argv);
		else if (strcmp(cmd, "planet")     == 0) rc = cmd_planet(argc, argv);
		else if (strcmp(cmd, "planet-all") == 0) rc = cmd_planet_all(argc, argv);
		else if (strcmp(cmd, "sector")     == 0) rc = cmd_sector(argc, argv);
		else if (strcmp(cmd, "surftex")    == 0) rc = cmd_surftex(argc, argv);
		else { usage(); rc = 1; }
	}

	if (g_out) fclose(g_out);
	return rc;
}
