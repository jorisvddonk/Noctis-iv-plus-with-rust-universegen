/*
	NIVHASH.H - simple, portable hashing utilities for the Noctis IV Plus
	universe-generation test harness.

	These hashes exist so that the (potentially very large) buffers produced
	by the real, unmodified Noctis generation code can be reduced to a small,
	stable fingerprint that is trivial to reproduce in ANY other programming
	language when porting the generator.

	Two independent algorithms are provided on purpose:
	  * FNV-1a (32 bit) - dead simple, one screenful of code in any language.
	  * CRC-32 (IEEE, reflected, poly 0xEDB88420) - the ubiquitous zip/png CRC.

	Emitting BOTH lets a port catch bugs where one algorithm happens to
	collide. Neither algorithm depends on machine word size beyond 32 bits,
	so results are identical on 16-bit DOS and on modern 64-bit hosts.

	This file is part of the TEST HARNESS only. It does NOT touch, include or
	modify any of the original Noctis generation sources.
*/

#ifndef NIVHASH_H
#define NIVHASH_H

/* 32-bit unsigned type that is correct on 16-bit Borland C++ (where int==16). */
typedef unsigned long nh_u32;

/* --- FNV-1a ------------------------------------------------------------- */
#define NH_FNV_OFFSET 2166136261UL
#define NH_FNV_PRIME  16777619UL

/* Running/streaming interface: start with NH_FNV_OFFSET, feed bytes. */
nh_u32 nh_fnv1a_update(nh_u32 h, const void *data, unsigned long len);
nh_u32 nh_fnv1a(const void *data, unsigned long len);

/* --- CRC-32 (IEEE 802.3, reflected) ------------------------------------ */
/* Streaming: start with 0, finalize with nh_crc32_final(). */
nh_u32 nh_crc32_update(nh_u32 crc, const void *data, unsigned long len);
nh_u32 nh_crc32_final(nh_u32 crc);
nh_u32 nh_crc32(const void *data, unsigned long len);

/*
	Feeding a "huge" (>64K) buffer on 16-bit DOS requires far/huge pointers.
	These helpers accept a far pointer and a 32-bit length and walk it byte
	by byte using huge-pointer arithmetic so segment wrap is handled.
	On non-DOS builds NH_FAR is empty and they behave like the normal ones.
*/
#ifdef __TURBOC__
#define NH_FAR huge
#else
#define NH_FAR
#endif

nh_u32 nh_fnv1a_far(nh_u32 h, const unsigned char NH_FAR *data, nh_u32 len);
nh_u32 nh_crc32_far(nh_u32 crc, const unsigned char NH_FAR *data, nh_u32 len);

#endif /* NIVHASH_H */
