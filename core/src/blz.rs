//! BLZ ("backwards LZ"), the compression of the ARM9 binary and of most
//! ARM9 overlays.
//!
//! The container ends with an 8-byte footer: a u32 whose low 24 bits are
//! the encoded length (counted back from the end, footer included) and whose
//! high 8 bits are the footer length (8 to 11), then a u32 of how many bytes
//! the image grows by. A growth of 0 means the data is stored. Bytes before
//! the encoded part are kept as they are; the encoded part is read from its
//! end backwards and written from the end of the output backwards. Flag
//! bits are most significant first; a set bit is a match of
//! `(token >> 12) + 3` bytes, `(token & 0xFFF) + 3` back, from a big-endian
//! token. Reference: eidos `install/blz.c`, which refuses what aletheia's
//! `tools/blz.py` refuses.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    TooSmall,
    BadFooter(&'static str),
    Truncated,
    BadDistance,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TooSmall => write!(f, "BLZ: too small for a footer"),
            Error::BadFooter(why) => write!(f, "BLZ: {why}"),
            Error::Truncated => write!(f, "BLZ: the stream ends early"),
            Error::BadDistance => write!(f, "BLZ: a match reaches past the decoded data"),
        }
    }
}

impl std::error::Error for Error {}

/// Decompresses a BLZ container. A stored container comes back unchanged.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    let n = input.len();
    if n < 8 {
        return Err(Error::TooSmall);
    }
    let word = u32::from_le_bytes(input[n - 8..n - 4].try_into().unwrap());
    let grow = u32::from_le_bytes(input[n - 4..].try_into().unwrap()) as usize;
    if grow == 0 {
        return Ok(input.to_vec());
    }
    let encoded = (word & 0x00FF_FFFF) as usize;
    let header = (word >> 24) as usize;
    if !(8..=11).contains(&header) {
        return Err(Error::BadFooter("footer length outside 8..11"));
    }
    if encoded > n {
        return Err(Error::BadFooter("encoded length past the container"));
    }
    if encoded < header {
        return Err(Error::BadFooter("encoded length shorter than the footer"));
    }
    let kept = n - encoded;
    let packed = &input[kept..n - header];
    let size = n + grow;
    let want = size - kept;

    let mut out = vec![0u8; size];
    out[..kept].copy_from_slice(&input[..kept]);
    let mut made = 0usize;
    let mut sp = 0usize;
    let mut next = || -> Result<u8, Error> {
        let b = *packed
            .len()
            .checked_sub(1 + sp)
            .and_then(|i| packed.get(i))
            .ok_or(Error::Truncated)?;
        sp += 1;
        Ok(b)
    };
    while made < want {
        let flags = next()?;
        for bit in 0..8 {
            if made >= want {
                break;
            }
            if flags & (0x80 >> bit) == 0 {
                out[size - 1 - made] = next()?;
                made += 1;
                continue;
            }
            let hi = next()? as usize;
            let lo = next()? as usize;
            let token = hi << 8 | lo;
            let len = ((token >> 12) + 3).min(want - made);
            let distance = (token & 0xFFF) + 3;
            if distance > made {
                return Err(Error::BadDistance);
            }
            for _ in 0..len {
                out[size - 1 - made] = out[size - 1 - made + distance];
                made += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored() {
        let data = [1u8, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(decompress(&data).unwrap(), data);
    }

    #[test]
    fn hand_made_stream() {
        // Kept prefix "KP"; the stream, in reading order: a flag byte (three
        // literals, then a match), the literals z, y, x (written from the
        // end backwards), and a match of 12 + 3 = 15 bytes at distance 3.
        let read_order = [0b0001_0000u8, b'z', b'y', b'x', 0xC0, 0x00];
        let mut container = b"KP".to_vec();
        container.extend(read_order.iter().rev());
        let header = 8u32;
        let encoded = read_order.len() as u32 + header;
        container.extend_from_slice(&(encoded | header << 24).to_le_bytes());
        // 16 bytes in, 2 + 18 out.
        container.extend_from_slice(&4u32.to_le_bytes());
        let out = decompress(&container).unwrap();
        assert_eq!(out, [b"KP".to_vec(), b"xyz".repeat(6)].concat());
    }

    #[test]
    fn rejects_bad_footers() {
        assert_eq!(decompress(&[0; 4]), Err(Error::TooSmall));
        let mut bad = vec![0u8; 16];
        bad[8..12].copy_from_slice(&(16u32 | 3 << 24).to_le_bytes());
        bad[12..].copy_from_slice(&1u32.to_le_bytes());
        assert!(matches!(decompress(&bad), Err(Error::BadFooter(_))));
    }
}
