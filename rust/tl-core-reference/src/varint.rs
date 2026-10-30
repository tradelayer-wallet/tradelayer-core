//! TradeLayer compressed integers (`src/tradelayer/varint.cpp`).
//!
//! Wire format: little-endian base-128, least significant 7-bit group first, MSB set on every
//! byte except the last. `CompressInteger` in C++ is reproduced exactly by [`compress`].
//!
//! The C++ decoder (`DecompressInteger` + `CMPTransaction::GetNextVarIntBytes`) is lenient: it
//! accepts non-canonical encodings, silently drops bits above bit 63, reads zero bytes past the
//! end of the packet and has undefined behaviour beyond ten bytes. [`read_strict`] is the
//! reference decoder used by [`crate::decode`]; [`decompress_cpp`] reproduces the lenient C++
//! arithmetic for differential tests. Differences are listed in the README.

/// Maximum number of bytes a `u64` needs (`ceil(64 / 7)`).
pub const MAX_VARINT_LEN: usize = 10;

/// Errors from the strict varint reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarintError {
    /// Input ended before a byte without the continuation bit was seen.
    Truncated,
    /// More than [`MAX_VARINT_LEN`] bytes with the continuation bit set.
    TooLong,
    /// The value does not fit in 64 bits.
    Overflow,
    /// A redundant trailing zero group (for example `80 00`); `compress` never emits these.
    NonCanonical,
}

/// Appends the compressed form of `value` to `out` (C++ `CompressInteger`).
pub fn push_compressed(out: &mut Vec<u8>, mut value: u64) {
    while value > 127 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// Returns the compressed form of `value` (C++ `CompressInteger`).
pub fn compress(value: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(MAX_VARINT_LEN);
    push_compressed(&mut out, value);
    out
}

/// Strictly reads one varint from `input` starting at `*pos`, advancing `*pos` on success.
///
/// Rejects truncation, more than ten bytes, 64-bit overflow and non-canonical encodings.
/// On error `*pos` is left unchanged.
pub fn read_strict(input: &[u8], pos: &mut usize) -> Result<u64, VarintError> {
    let mut value: u64 = 0;
    let mut i = *pos;
    let mut n = 0usize;
    loop {
        let Some(&byte) = input.get(i) else {
            return Err(VarintError::Truncated);
        };
        if n == MAX_VARINT_LEN {
            return Err(VarintError::TooLong);
        }
        let group = u64::from(byte & 0x7f);
        if n == MAX_VARINT_LEN - 1 && group > 1 {
            return Err(VarintError::Overflow);
        }
        value |= group << (7 * n);
        i += 1;
        n += 1;
        if byte & 0x80 == 0 {
            if n > 1 && byte == 0 {
                return Err(VarintError::NonCanonical);
            }
            *pos = i;
            return Ok(value);
        }
    }
}

/// The arithmetic of C++ `DecompressInteger` applied to a byte slice that has already been cut
/// at the first byte without the MSB (`GetNextVarIntBytes`).
///
/// Bits that would be shifted past bit 63 are dropped, exactly as in C++. Returns `None` for more
/// than ten bytes, where C++ shifts a 64-bit value by 70 or more bits (undefined behaviour).
pub fn decompress_cpp(bytes: &[u8]) -> Option<u64> {
    if bytes.len() > MAX_VARINT_LEN {
        return None;
    }
    let mut value = 0u64;
    for (n, b) in bytes.iter().enumerate() {
        value |= u64::from(b & 0x7f) << (7 * n);
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_encodings() {
        assert_eq!(compress(0), [0x00]);
        assert_eq!(compress(127), [0x7f]);
        assert_eq!(compress(128), [0x80, 0x01]);
        assert_eq!(compress(300), [0xac, 0x02]);
        assert_eq!(compress(u64::MAX).len(), 10);
    }

    #[test]
    fn strict_roundtrip_and_errors() {
        for v in [0u64, 1, 127, 128, 16383, 16384, u32::MAX as u64, 1 << 63, u64::MAX] {
            let bytes = compress(v);
            let mut pos = 0;
            assert_eq!(read_strict(&bytes, &mut pos), Ok(v));
            assert_eq!(pos, bytes.len());
            for cut in 0..bytes.len() {
                let mut p = 0;
                assert_eq!(read_strict(&bytes[..cut], &mut p), Err(VarintError::Truncated));
                assert_eq!(p, 0);
            }
        }
        let mut p = 0;
        assert_eq!(read_strict(&[0x80, 0x00], &mut p), Err(VarintError::NonCanonical));
        assert_eq!(read_strict(&[0xff, 0x00], &mut p), Err(VarintError::NonCanonical));
        let mut over = vec![0xff; 9];
        over.push(0x02);
        assert_eq!(read_strict(&over, &mut p), Err(VarintError::Overflow));
        let mut long = vec![0x80; 10];
        long.push(0x01);
        assert_eq!(read_strict(&long, &mut p), Err(VarintError::TooLong));
    }

    #[test]
    fn cpp_lenient_arithmetic() {
        assert_eq!(decompress_cpp(&[0x80, 0x00]), Some(0));
        assert_eq!(decompress_cpp(&[0xff, 0x00]), Some(127));
        assert_eq!(decompress_cpp(&[0x80; 9].iter().chain(&[0x7f]).copied().collect::<Vec<_>>()), Some(1 << 63));
        assert_eq!(decompress_cpp(&[0x80; 11]), None);
    }
}
