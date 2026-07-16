//! Portable FNV-1a + CRC-32 hashing, bit-exact with `tests/harness/NIVHASH.C`.
//!
//! CRC-32 is the standard IEEE 802.3 CRC (poly 0x04C11DB7, input and output
//! reflected, init 0xFFFFFFFF, final XOR 0xFFFFFFFF) - identical to
//! `zlib.crc32` / `gzip`. It is implemented MSB-first with an explicit bit
//! reflection per byte, matching the C reference.

pub const FNV_OFFSET: u32 = 0x811C_9DC5;
pub const FNV_PRIME: u32 = 0x0100_0193;

pub fn fnv1a(data: &[u8]) -> u32 {
    fnv1a_update(FNV_OFFSET, data)
}

pub fn fnv1a_update(mut h: u32, data: &[u8]) -> u32 {
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn reflect(mut v: u32, bits: u32) -> u32 {
    let mut r = 0u32;
    for i in 0..bits {
        if v & (1 << i) != 0 {
            r |= 1 << (bits - 1 - i);
        }
    }
    r
}

pub fn crc32(data: &[u8]) -> u32 {
    crc32_final(crc32_update(0xFFFF_FFFF, data))
}

pub fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &b in data {
        crc ^= reflect(b as u32, 8) << 24;
        for _ in 0..8 {
            if crc & 0x8000_0000 != 0 {
                crc = (crc << 1) ^ 0x04C1_1DB7;
            } else {
                crc = crc << 1;
            }
        }
    }
    crc
}

pub fn crc32_final(crc: u32) -> u32 {
    reflect(crc, 32) ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors() {
        assert_eq!(fnv1a(b""), 0x811C_9DC5);
        assert_eq!(fnv1a(b"a"), 0xE40C_292C);
        assert_eq!(fnv1a(b"abc"), 0x1A47_E90B);
        assert_eq!(fnv1a(b"The quick brown fox jumps over the lazy dog"), 0x048F_FF90);

        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"abc"), 0x3524_41C2);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }
}
