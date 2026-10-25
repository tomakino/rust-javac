// PORTING ADAPTER: jimage string table (modified UTF-8) and perfect-hash functions.
// Reference implementation: java.base/jdk/internal/jimage/ImageStringsReader.java
//   @ 982d53c (jdk-17.0.19-ga), extracted from the baseline JDK src.zip.

//! The jimage string table is a modified UTF-8 byte blob where every string is terminated by a
//! zero byte. Names are hashed with the same functions that jlink's PerfectHashBuilder used to
//! build the redirect table, so a table built by the JDK is looked up byte-for-byte the same way.

use crate::Error;

/// Java: ImageStringsReader.HASH_MULTIPLIER.
pub const HASH_MULTIPLIER: i32 = 0x0100_0193;

/// Java: ImageStringsReader.POSITIVE_MASK.
pub const POSITIVE_MASK: i32 = 0x7FFF_FFFF;

/// Java: ImageStringsReader.hashCode(String s) — s is iterated as UTF-16 code units.
pub fn hash_code(units: &[u16]) -> i32 {
    unmasked_hash_code(units, HASH_MULTIPLIER) & POSITIVE_MASK
}

/// Java: ImageStringsReader.hashCode(String s, int seed)
pub fn hash_code_seeded(units: &[u16], seed: i32) -> i32 {
    unmasked_hash_code(units, seed) & POSITIVE_MASK
}

/// Java: ImageStringsReader.hashCode(String module, String name)
pub fn hash_code_module_name(module: &str, name: &str) -> i32 {
    let mut seed = HASH_MULTIPLIER;
    seed = seed.wrapping_mul(HASH_MULTIPLIER) ^ ('/' as i32);
    seed = unmasked_hash_code_str(module, seed);
    seed = seed.wrapping_mul(HASH_MULTIPLIER) ^ ('/' as i32);
    seed = unmasked_hash_code_str(name, seed);
    seed & POSITIVE_MASK
}

/// Java: ImageStringsReader.unmaskedHashCode(String s, int seed)
pub fn unmasked_hash_code_str(s: &str, seed: i32) -> i32 {
    // Java String.charAt walks UTF-16 code units.
    let units: Vec<u16> = s.encode_utf16().collect();
    unmasked_hash_code(&units, seed)
}

/// Java: ImageStringsReader.unmaskedHashCode(String s, int seed)
pub fn unmasked_hash_code(units: &[u16], mut seed: i32) -> i32 {
    for &unit in units {
        let uch = unit as i32;

        if (uch & !0x7F) != 0 {
            let mut buffer = [0u8; 8];
            let mut mask: i32 = !0x3F;
            let mut n = 0usize;
            let mut value = uch;

            loop {
                buffer[n] = (0x80 | (value & 0x3F)) as u8;
                n += 1;
                value >>= 6;
                mask >>= 1;
                if (value & mask) == 0 {
                    break;
                }
            }
            buffer[n] = ((mask << 1) | value) as u8;

            loop {
                seed = seed.wrapping_mul(HASH_MULTIPLIER) ^ ((buffer[n] as i32) & 0xFF);
                if n == 0 {
                    break;
                }
                n -= 1;
            }
        } else if uch == 0 {
            seed = seed.wrapping_mul(HASH_MULTIPLIER) ^ 0xC0;
            seed = seed.wrapping_mul(HASH_MULTIPLIER) ^ 0x80;
        } else {
            seed = seed.wrapping_mul(HASH_MULTIPLIER) ^ uch;
        }
    }
    seed
}

/// Decode one modified UTF-8 character starting at *index.
///
/// Java: ImageStringsReader.charsFromByteBuffer (single character decode)
fn decode_one(bytes: &[u8], index: &mut usize) -> Option<u16> {
    let mut ch = *bytes.get(*index)?;
    *index += 1;

    let is_unicode = (ch & 0x80) != 0;
    let mut uch = (ch & 0x7F) as u32;

    if is_unicode {
        let mut mask: u32 = 0x40;
        while (uch & mask) != 0 {
            ch = *bytes.get(*index)?;
            *index += 1;
            if (ch & 0xC0) != 0x80 {
                return None;
            }
            uch = ((uch & !mask) << 6) | ((ch & 0x3F) as u32);
            mask <<= 5;
        }
    }

    if (uch & 0xFFFF) != uch {
        return None;
    }
    Some(uch as u16)
}

/// Java: ImageStringsReader.stringFromByteBuffer(ByteBuffer buffer, int offset)
pub fn string_from_bytes(bytes: &[u8], offset: usize) -> Result<String, Error> {
    if offset > bytes.len() {
        return Err(Error::OffsetOutOfBounds("strings", offset));
    }
    let mut units: Vec<u16> = Vec::new();
    let mut index = offset;
    loop {
        let ch = match bytes.get(index) {
            Some(&ch) => ch,
            // Java: InternalError("No terminating zero byte for modified UTF-8 byte sequence")
            None => return Err(Error::Corrupted("unterminated image string".to_string())),
        };
        if ch == 0 {
            break;
        }
        let value = decode_one(bytes, &mut index)
            .ok_or_else(|| Error::Corrupted("bad modified UTF-8 in image strings".to_string()))?;
        units.push(value);
    }
    // Java String may hold unpaired surrogates, Rust String may not. Every jimage string is a
    // resource name/path; a malformed table is rejected instead of being silently replaced.
    String::from_utf16(&units)
        .map_err(|_| Error::Corrupted("unpaired surrogate in image string".to_string()))
}

/// Java: ImageStringsReader.stringFromByteBufferMatches(buffer, offset, string, stringOffset)
///
/// Returns the number of UTF-16 code units of the table string that matched, or None when the
/// byte sequence does not match the Java string (including the case where the Java string ends
/// before the table string does, matching the JDK's limit checks).
pub fn match_string(
    bytes: &[u8],
    offset: usize,
    name: &[u16],
    mut name_offset: usize,
) -> Option<usize> {
    if offset > bytes.len() {
        return None;
    }
    let mut index = offset;
    let mut matched = 0usize;
    loop {
        let ch = *bytes.get(index)?;
        if ch == 0 {
            return Some(matched);
        }
        let value = decode_one(bytes, &mut index)?;
        if name_offset >= name.len() || name[name_offset] != value {
            return None;
        }
        name_offset += 1;
        matched += 1;
    }
}
