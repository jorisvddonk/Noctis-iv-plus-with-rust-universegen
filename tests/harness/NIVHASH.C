/*
	NIVHASH.C - implementation of the portable hashing utilities.
	See NIVHASH.H for rationale.

	Reference values (verified against zlib/gzip, so a port can self-check
	the hash itself, independent of Noctis):

	   input bytes         FNV-1a(32)     CRC-32
	   ------------------  -----------    ----------
	   ""  (empty)         0x811C9DC5     0x00000000
	   "a"                 0xE40C292C     0xE8B7BE43
	   "abc"               0x1A47E90B     0x352441C2
	   "123456789"         0xBB86B11C     0xCBF43926   <- the classic CRC check
	   "The quick brown fox jumps over the lazy dog"
	                       0x048FFF90     0x414FA339

	CRC-32 here is the standard IEEE 802.3 CRC (poly 0x04C11DB7, input and
	output reflected, init 0xFFFFFFFF, final XOR 0xFFFFFFFF) - i.e. exactly
	what zlib.crc32 / gzip produce. It is implemented MSB-first with an
	explicit bit reflection per byte, which is the formulation that is easy
	to reproduce byte-for-byte in any language.
*/

#include "nivhash.h"

/* ---- FNV-1a ----------------------------------------------------------- */

nh_u32 nh_fnv1a_update(nh_u32 h, const void *data, unsigned long len)
{
	const unsigned char *p = (const unsigned char *)data;
	unsigned long i;
	for (i = 0; i < len; i++) {
		h ^= (nh_u32)p[i];
		h *= NH_FNV_PRIME;
		h &= 0xFFFFFFFFUL; /* keep 32 bits on platforms with >32-bit long */
	}
	return h;
}

nh_u32 nh_fnv1a(const void *data, unsigned long len)
{
	return nh_fnv1a_update(NH_FNV_OFFSET, data, len);
}

/* ---- CRC-32 (IEEE, reflected) ----------------------------------------- */

static nh_u32 nh__reflect(nh_u32 v, int bits)
{
	nh_u32 r = 0;
	int i;
	for (i = 0; i < bits; i++) {
		if (v & (1UL << i))
			r |= (1UL << (bits - 1 - i));
	}
	return r & 0xFFFFFFFFUL;
}

nh_u32 nh_crc32_update(nh_u32 crc, const void *data, unsigned long len)
{
	const unsigned char *p = (const unsigned char *)data;
	unsigned long i;
	int k;
	for (i = 0; i < len; i++) {
		crc ^= (nh__reflect((nh_u32)p[i], 8) << 24);
		crc &= 0xFFFFFFFFUL;
		for (k = 0; k < 8; k++) {
			if (crc & 0x80000000UL)
				crc = ((crc << 1) ^ 0x04C11DB7UL) & 0xFFFFFFFFUL;
			else
				crc = (crc << 1) & 0xFFFFFFFFUL;
		}
	}
	return crc;
}

nh_u32 nh_crc32_final(nh_u32 crc)
{
	return nh__reflect(crc, 32) ^ 0xFFFFFFFFUL;
}

nh_u32 nh_crc32(const void *data, unsigned long len)
{
	return nh_crc32_final(nh_crc32_update(0xFFFFFFFFUL, data, len));
}

/* ---- far/huge variants for large DOS buffers -------------------------- */

nh_u32 nh_fnv1a_far(nh_u32 h, const unsigned char NH_FAR *data, nh_u32 len)
{
	nh_u32 i;
	for (i = 0; i < len; i++) {
		h ^= (nh_u32)data[i];
		h *= NH_FNV_PRIME;
		h &= 0xFFFFFFFFUL;
	}
	return h;
}

nh_u32 nh_crc32_far(nh_u32 crc, const unsigned char NH_FAR *data, nh_u32 len)
{
	nh_u32 i;
	int k;
	for (i = 0; i < len; i++) {
		crc ^= (nh__reflect((nh_u32)data[i], 8) << 24);
		crc &= 0xFFFFFFFFUL;
		for (k = 0; k < 8; k++) {
			if (crc & 0x80000000UL)
				crc = ((crc << 1) ^ 0x04C11DB7UL) & 0xFFFFFFFFUL;
			else
				crc = (crc << 1) & 0xFFFFFFFFUL;
		}
	}
	return crc;
}
