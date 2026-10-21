//! BIOS-style LZ77 (tag 0x10) and LZ11 (tag 0x11) compression, the wrapper
//! many NARC members arrive in.
//!
//! A stream is a 4-byte header (tag byte, 24-bit decompressed size), then
//! flag bytes. Each flag byte covers eight units, most significant bit
//! first: a clear bit is one literal byte, a set bit copies bytes from the
//! output already written. LZ11 adds two longer match forms, and a header
//! whose 24-bit size is 0 is followed by a 32-bit size.
//!
//! The end guard follows eidos `engine/g2d/lzss.c` and aletheia
//! `tools/assets/narc.py`: a real stream uses all but at most three bytes
//! (sub-file padding) of its input. A stream that ends much earlier means the
//! tag byte was data, so the member is not compressed.

use std::fmt;

/// The compression a member uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Lz10,
    Lz11,
}

impl Kind {
    pub const fn tag(self) -> u8 {
        match self {
            Kind::Lz10 => 0x10,
            Kind::Lz11 => 0x11,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The first byte is not 0x10 or 0x11, or the header is cut short.
    NotCompressed,
    /// The declared size is 0.
    EmptyOutput,
    /// The stream ends before the declared size is written.
    Truncated,
    /// A match points before the start of the output.
    BadDistance { at: usize, distance: usize },
    /// More than three bytes are left after the stream.
    TrailingData { left: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotCompressed => write!(f, "not an LZ10 or LZ11 stream"),
            Error::EmptyOutput => write!(f, "declared output size is 0"),
            Error::Truncated => write!(f, "stream ends before the declared size"),
            Error::BadDistance { at, distance } => {
                write!(f, "match at output {at} reaches back {distance} bytes")
            }
            Error::TrailingData { left } => {
                write!(f, "{left} bytes left after the stream")
            }
        }
    }
}

impl std::error::Error for Error {}

/// The kind and declared size, if `data` starts with an LZ header. This does
/// not prove the data is compressed; only [`decompress`] does.
pub fn sniff(data: &[u8]) -> Option<(Kind, usize)> {
    let kind = match *data.first()? {
        0x10 => Kind::Lz10,
        0x11 => Kind::Lz11,
        _ => return None,
    };
    let header = data.get(..4)?;
    let size = u32::from_le_bytes([header[1], header[2], header[3], 0]) as usize;
    if kind == Kind::Lz11 && size == 0 {
        let wide = data.get(4..8)?;
        return Some((kind, u32::from_le_bytes(wide.try_into().unwrap()) as usize));
    }
    Some((kind, size))
}

/// Decompresses an LZ10 or LZ11 stream.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, Error> {
    let (kind, size) = sniff(data).ok_or(Error::NotCompressed)?;
    if size == 0 {
        return Err(Error::EmptyOutput);
    }
    let mut sp = if kind == Kind::Lz11 && data[1..4] == [0, 0, 0] {
        8
    } else {
        4
    };
    let next = |sp: &mut usize| -> Result<u8, Error> {
        let byte = *data.get(*sp).ok_or(Error::Truncated)?;
        *sp += 1;
        Ok(byte)
    };

    let mut out = Vec::with_capacity(size);
    while out.len() < size {
        let flags = next(&mut sp)?;
        for bit in 0..8 {
            if out.len() >= size {
                break;
            }
            if flags & (0x80 >> bit) == 0 {
                out.push(next(&mut sp)?);
                continue;
            }
            let b0 = next(&mut sp)? as usize;
            let (len, distance) = match kind {
                Kind::Lz10 => {
                    let b1 = next(&mut sp)? as usize;
                    ((b0 >> 4) + 3, ((b0 & 0xF) << 8 | b1) + 1)
                }
                Kind::Lz11 => match b0 >> 4 {
                    0 => {
                        let b1 = next(&mut sp)? as usize;
                        let b2 = next(&mut sp)? as usize;
                        (
                            ((b0 & 0xF) << 4 | b1 >> 4) + 0x11,
                            ((b1 & 0xF) << 8 | b2) + 1,
                        )
                    }
                    1 => {
                        let b1 = next(&mut sp)? as usize;
                        let b2 = next(&mut sp)? as usize;
                        let b3 = next(&mut sp)? as usize;
                        (
                            ((b0 & 0xF) << 12 | b1 << 4 | b2 >> 4) + 0x111,
                            ((b2 & 0xF) << 8 | b3) + 1,
                        )
                    }
                    n => {
                        let b1 = next(&mut sp)? as usize;
                        (n + 1, ((b0 & 0xF) << 8 | b1) + 1)
                    }
                },
            };
            if distance > out.len() {
                return Err(Error::BadDistance {
                    at: out.len(),
                    distance,
                });
            }
            let base = out.len() - distance;
            for i in 0..len.min(size - out.len()) {
                out.push(out[base + i]);
            }
        }
    }

    let left = data.len() - sp;
    if left > 3 {
        return Err(Error::TrailingData { left });
    }
    Ok(out)
}

/// Decompresses `data` if it is a real LZ stream, else returns it unchanged.
/// This is what a loader with no "compressed" flag needs.
pub fn decompress_if_compressed(data: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    match decompress(data) {
        Ok(out) => out.into(),
        Err(_) => data.into(),
    }
}

/// Compresses `data` with a greedy longest-match search. The output is a
/// valid stream for the BIOS decoders, not byte-identical to the game's own
/// compressor.
pub fn compress(kind: Kind, data: &[u8]) -> Vec<u8> {
    let (min_len, max_len) = match kind {
        Kind::Lz10 => (3, 0x12),
        Kind::Lz11 => (3, 0x10110),
    };
    const WINDOW: usize = 0x1000;

    let mut out = Vec::with_capacity(data.len() + data.len() / 8 + 8);
    out.push(kind.tag());
    if kind == Kind::Lz11 && data.len() > 0xFF_FFFF {
        out.extend_from_slice(&[0, 0, 0]);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    } else {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes()[..3]);
    }

    let mut pos = 0;
    while pos < data.len() {
        let flag_at = out.len();
        out.push(0);
        for bit in 0..8 {
            if pos >= data.len() {
                break;
            }
            let (len, distance) = longest_match(data, pos, WINDOW, max_len);
            if len < min_len {
                out.push(data[pos]);
                pos += 1;
                continue;
            }
            out[flag_at] |= 0x80 >> bit;
            let d = distance - 1;
            match kind {
                Kind::Lz10 => {
                    out.push(((len - 3) << 4 | d >> 8) as u8);
                    out.push(d as u8);
                }
                Kind::Lz11 if len <= 0x10 => {
                    out.push(((len - 1) << 4 | d >> 8) as u8);
                    out.push(d as u8);
                }
                Kind::Lz11 if len <= 0x110 => {
                    let l = len - 0x11;
                    out.push((l >> 4) as u8);
                    out.push(((l & 0xF) << 4 | d >> 8) as u8);
                    out.push(d as u8);
                }
                Kind::Lz11 => {
                    let l = len - 0x111;
                    out.push((0x10 | l >> 12) as u8);
                    out.push((l >> 4) as u8);
                    out.push(((l & 0xF) << 4 | d >> 8) as u8);
                    out.push(d as u8);
                }
            }
            pos += len;
        }
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

fn longest_match(data: &[u8], pos: usize, window: usize, max_len: usize) -> (usize, usize) {
    let limit = max_len.min(data.len() - pos);
    let mut best = (0, 0);
    // Distance 1 is allowed, but the BIOS decoders copy byte by byte, so an
    // overlapping match is fine in both directions.
    for distance in 1..=window.min(pos) {
        let start = pos - distance;
        let mut len = 0;
        while len < limit && data[start + len] == data[pos + len] {
            len += 1;
        }
        if len > best.0 {
            best = (len, distance);
            if len == limit {
                break;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz10_hand_made_stream() {
        // "abc" as literals, then a match of 6 at distance 3, then "d".
        // Flags: 0 0 0 1 0 -> 0b0001_0000.
        let stream = [
            0x10, 10, 0, 0, // header: size 10
            0x10, b'a', b'b', b'c', // flags, literals
            0x30, 0x02, // len 6, distance 3
            b'd', 0, // literal, padding
        ];
        assert_eq!(decompress(&stream).unwrap(), b"abcabcabcd");
    }

    #[test]
    fn lz11_all_three_match_forms() {
        let mut stream = vec![0x11, 0, 0, 0]; // size 0: a 32-bit size follows
        let size: u32 = 1 + 0x10 + 0x20 + 0x200;
        stream.extend_from_slice(&size.to_le_bytes());
        stream.push(0b0111_0000);
        stream.push(b'x');
        // Short form: length 0x10, distance 1.
        stream.extend_from_slice(&[0xF0, 0x00]);
        // Middle form: length 0x20 = 0x11 + 0xF, distance 1.
        stream.extend_from_slice(&[0x00, 0xF0, 0x00]);
        // Long form: length 0x200 = 0x111 + 0xEF, distance 1.
        stream.extend_from_slice(&[0x10, 0x0E, 0xF0, 0x00]);
        let out = decompress(&stream).unwrap();
        assert_eq!(out.len(), size as usize);
        assert!(out.iter().all(|&b| b == b'x'));
    }

    #[test]
    fn rejects_bad_streams() {
        assert_eq!(decompress(b"NARC"), Err(Error::NotCompressed));
        assert_eq!(decompress(&[0x10, 0, 0, 0]), Err(Error::EmptyOutput));
        assert_eq!(decompress(&[0x10, 8, 0, 0, 0, b'a']), Err(Error::Truncated));
        assert_eq!(
            decompress(&[0x10, 4, 0, 0, 0x80, 0x00, 0x00, 0]),
            Err(Error::BadDistance { at: 0, distance: 1 })
        );
        // A tag byte that is really data: the stream ends long before the input.
        let mut fake = vec![0x10, 1, 0, 0, 0, b'a'];
        fake.extend_from_slice(&[0xAA; 16]);
        assert_eq!(decompress(&fake), Err(Error::TrailingData { left: 16 }));
        assert_eq!(&*decompress_if_compressed(&fake), &fake[..]);
    }

    fn sample() -> Vec<u8> {
        let mut data = Vec::new();
        let mut seed = 0x1234_5678u32;
        for i in 0..20_000u32 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            // Runs, repeats and noise, so every match form appears.
            let byte = match (i / 700) % 3 {
                0 => (i % 7) as u8,
                1 => 0x55,
                _ => (seed >> 24) as u8,
            };
            data.push(byte);
        }
        data
    }

    #[test]
    fn round_trip_both_kinds() {
        let data = sample();
        for kind in [Kind::Lz10, Kind::Lz11] {
            let packed = compress(kind, &data);
            assert_eq!(packed.len() % 4, 0);
            assert!(packed.len() < data.len());
            assert_eq!(sniff(&packed), Some((kind, data.len())));
            assert_eq!(decompress(&packed).unwrap(), data, "{kind:?}");
        }
    }

    #[test]
    fn round_trip_tiny_inputs() {
        for len in 1..40 {
            let data: Vec<u8> = (0..len).map(|i| (i % 3) as u8).collect();
            for kind in [Kind::Lz10, Kind::Lz11] {
                assert_eq!(decompress(&compress(kind, &data)).unwrap(), data);
            }
        }
    }
}
