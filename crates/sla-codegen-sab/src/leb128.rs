//! LEB128 primitives used by the SAB v4 wire format.
//!
//! Faithful port of the write side of sci `sab.zig`
//! (`writeUleb128` / `writeSleb128`, lines 104-129) plus a minimal read
//! side used by this crate's own structural tests. Every multi-byte
//! integer in a SAB module (counts, pool ids, line numbers, immediate
//! values) goes through these two encodings.

/// Append `value` as unsigned LEB128 (7 bits per byte, low group first,
/// continuation bit 0x80 on all but the last byte).
pub fn write_uleb128(out: &mut Vec<u8>, value: u64) {
    let mut rest = value;
    loop {
        let mut byte = (rest & 0x7f) as u8;
        rest >>= 7;
        if rest != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if rest == 0 {
            break;
        }
    }
}

/// Append `value` as signed LEB128 (two's complement, sign bit 0x40 of
/// the final byte decides termination), matching `writeSleb128`.
pub fn write_sleb128(out: &mut Vec<u8>, value: i64) {
    let mut rest = value;
    loop {
        let mut byte = (rest & 0x7f) as u8;
        rest >>= 7; // arithmetic shift, as in the Zig original
        let sign_bit_set = byte & 0x40 != 0;
        let done = (rest == 0 && !sign_bit_set) || (rest == -1 && sign_bit_set);
        if !done {
            byte |= 0x80;
        }
        out.push(byte);
        if done {
            break;
        }
    }
}

/// Read an unsigned LEB128 starting at `*pos`, advancing `*pos`.
/// Returns `None` on truncation or overflow; used by structural tests.
pub fn read_uleb128(bytes: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*pos)?;
        *pos += 1;
        if shift >= 64 {
            return None;
        }
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift >= 70 {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uleb(value: u64) -> Vec<u8> {
        let mut v = Vec::new();
        write_uleb128(&mut v, value);
        v
    }

    fn sleb(value: i64) -> Vec<u8> {
        let mut v = Vec::new();
        write_sleb128(&mut v, value);
        v
    }

    #[test]
    fn uleb_known_vectors() {
        assert_eq!(uleb(0), vec![0x00]);
        assert_eq!(uleb(1), vec![0x01]);
        assert_eq!(uleb(63), vec![0x3f]);
        assert_eq!(uleb(128), vec![0x80, 0x01]);
        assert_eq!(uleb(300), vec![0xac, 0x02]);
        assert_eq!(uleb(16_384), vec![0x80, 0x80, 0x01]);
        assert_eq!(uleb(u32::MAX as u64), vec![0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn sleb_known_vectors() {
        assert_eq!(sleb(0), vec![0x00]);
        assert_eq!(sleb(-1), vec![0x7f]);
        assert_eq!(sleb(63), vec![0x3f]);
        assert_eq!(sleb(64), vec![0xc0, 0x00]);
        assert_eq!(sleb(-64), vec![0x40]);
        assert_eq!(sleb(-65), vec![0xbf, 0x7f]);
        assert_eq!(sleb(-12_345), vec![0xc7, 0x9f, 0x7f]);
    }

    #[test]
    fn uleb_round_trip() {
        for value in [0u64, 1, 127, 128, 300, 65_535, 1 << 35, u64::MAX] {
            let bytes = uleb(value);
            let mut pos = 0;
            assert_eq!(read_uleb128(&bytes, &mut pos), Some(value));
            assert_eq!(pos, bytes.len());
        }
    }
}
