//! SWAR wave archives and SWAV waves, and the three sample encodings.
//!
//! ```text
//! SWAR  "SWAR" bom version size hdrsize blocks, then "DATA" u32 size at 0x10
//!       0x38  u32 wave count
//!       0x3C  u32 absolute file offset * count
//! SWAV record (also at 0x18 of a standalone "SWAV" file)
//!       u8 type (0 PCM8, 1 PCM16, 2 IMA-ADPCM)  u8 loop flag
//!       u16 sample rate  u16 timer  u16 loop start (4-byte words)
//!       u32 non-loop length (4-byte words)
//!       then (loop start + non-loop length) * 4 bytes of sample data
//! ```
//!
//! A wave's length is stored in its own record, so the archive needs no end
//! offsets. PCM8 is signed 8-bit, widened by 256. ADPCM data starts with a
//! 4-byte seed (s16 predictor, u8 step index, u8 reserved); the nibbles
//! follow, low nibble first, and the seed itself is not a decoded sample.
//!
//! References: eidos `engine/audio/swav.c`, aletheia `tools/assets/swav.py`.

use crate::nitro::{read_u16, read_u32, Error};

const IMA_STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

const IMA_INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// The clock the channel timer divides: half the ARM7 bus clock.
const TIMER_CLOCK: f64 = 33_513_982.0 / 2.0;

/// How a wave's samples are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Pcm8,
    Pcm16,
    ImaAdpcm,
}

/// One wave: its header fields and a borrowed view of its sample bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swav<'a> {
    pub encoding: Encoding,
    pub looped: bool,
    pub sample_rate: u16,
    pub timer: u16,
    /// Loop start, in 4-byte words from the start of the sample data.
    pub loop_offset_words: u16,
    pub nonloop_words: u32,
    pub data: &'a [u8],
}

impl<'a> Swav<'a> {
    /// Reads the wave record at `at` in `bytes`.
    fn record(bytes: &'a [u8], at: usize) -> Result<Swav<'a>, Error> {
        let head = bytes.get(at..at + 12).ok_or(Error::Truncated)?;
        let encoding = match head[0] {
            0 => Encoding::Pcm8,
            1 => Encoding::Pcm16,
            2 => Encoding::ImaAdpcm,
            _ => return Err(Error::Bad("wave type")),
        };
        let loop_offset_words = read_u16(head, 6)?;
        let nonloop_words = read_u32(head, 8)?;
        let len = (loop_offset_words as usize + nonloop_words as usize) * 4;
        let data = bytes.get(at + 12..at + 12 + len).ok_or(Error::Truncated)?;
        Ok(Swav {
            encoding,
            looped: head[1] != 0,
            sample_rate: read_u16(head, 2)?,
            timer: read_u16(head, 4)?,
            loop_offset_words,
            nonloop_words,
            data,
        })
    }

    /// Parses a standalone SWAV file.
    pub fn parse_file(bytes: &'a [u8]) -> Result<Swav<'a>, Error> {
        if bytes.get(..4) != Some(b"SWAV") {
            return Err(Error::WrongMagic {
                want: *b"SWAV",
                found: bytes.get(..4).and_then(|m| m.try_into().ok()),
            });
        }
        if bytes.get(0x10..0x14) != Some(b"DATA") {
            return Err(Error::MissingBlock(*b"DATA"));
        }
        Swav::record(bytes, 0x18)
    }

    /// How many samples [`Swav::decode`] produces.
    pub fn decoded_len(&self) -> usize {
        match self.encoding {
            Encoding::Pcm8 => self.data.len(),
            Encoding::Pcm16 => self.data.len() / 2,
            Encoding::ImaAdpcm => self.data.len().saturating_sub(4) * 2,
        }
    }

    /// The samples as signed 16-bit PCM, whatever the stored encoding.
    pub fn decode(&self) -> Vec<i16> {
        match self.encoding {
            Encoding::Pcm8 => self.data.iter().map(|&b| (b as i8 as i16) * 256).collect(),
            Encoding::Pcm16 => self
                .data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c))
                .collect(),
            Encoding::ImaAdpcm => {
                if self.data.len() < 4 {
                    return Vec::new();
                }
                let mut predictor = i16::from_le_bytes([self.data[0], self.data[1]]) as i32;
                let mut index = (self.data[2] as i32).clamp(0, 88);
                let mut out = Vec::with_capacity(self.decoded_len());
                for &byte in &self.data[4..] {
                    for nibble in [byte & 0xF, byte >> 4] {
                        let step = IMA_STEP[index as usize];
                        let mut diff = step >> 3;
                        if nibble & 4 != 0 {
                            diff += step;
                        }
                        if nibble & 2 != 0 {
                            diff += step >> 1;
                        }
                        if nibble & 1 != 0 {
                            diff += step >> 2;
                        }
                        if nibble & 8 != 0 {
                            predictor -= diff;
                        } else {
                            predictor += diff;
                        }
                        predictor = predictor.clamp(-32768, 32767);
                        index = (index + IMA_INDEX[nibble as usize]).clamp(0, 88);
                        out.push(predictor as i16);
                    }
                }
                out
            }
        }
    }

    /// The loop point as an index into the decoded samples: 4 samples per
    /// word for PCM8, 2 for PCM16, and 8 for ADPCM less the seed word.
    pub fn loop_start(&self) -> usize {
        let words = self.loop_offset_words as usize;
        let start = match self.encoding {
            Encoding::Pcm8 => words * 4,
            Encoding::Pcm16 => words * 2,
            Encoding::ImaAdpcm => words.saturating_sub(1) * 8,
        };
        start.min(self.decoded_len())
    }

    /// The sample rate the wave was recorded at, in Hz. The timer value is
    /// the more exact source; the stored rate is the fallback.
    pub fn native_rate(&self) -> f64 {
        if self.timer != 0 {
            TIMER_CLOCK / self.timer as f64
        } else if self.sample_rate != 0 {
            self.sample_rate as f64
        } else {
            16000.0
        }
    }
}

/// A wave archive: a table of waves.
#[derive(Debug, Clone)]
pub struct Swar<'a> {
    bytes: &'a [u8],
    offsets: Vec<u32>,
}

impl<'a> Swar<'a> {
    /// Parses a SWAR file.
    pub fn parse(bytes: &'a [u8]) -> Result<Swar<'a>, Error> {
        if bytes.get(..4) != Some(b"SWAR") {
            return Err(Error::WrongMagic {
                want: *b"SWAR",
                found: bytes.get(..4).and_then(|m| m.try_into().ok()),
            });
        }
        if bytes.get(0x10..0x14) != Some(b"DATA") {
            return Err(Error::MissingBlock(*b"DATA"));
        }
        let count = read_u32(bytes, 0x38)? as usize;
        if 0x3C + count * 4 > bytes.len() {
            return Err(Error::Truncated);
        }
        let offsets = (0..count)
            .map(|i| read_u32(bytes, 0x3C + i * 4))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Swar { bytes, offsets })
    }

    /// How many waves the archive holds.
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// True when the archive holds no waves.
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// Wave `index`.
    pub fn wave(&self, index: usize) -> Result<Swav<'a>, Error> {
        let at = *self.offsets.get(index).ok_or(Error::Bad("wave index"))?;
        Swav::record(self.bytes, at as usize)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    /// A wave record with the given type, loop word offset and data.
    pub fn record(kind: u8, looped: bool, rate: u16, loop_words: u16, data: &[u8]) -> Vec<u8> {
        assert_eq!(data.len() % 4, 0);
        let total = data.len() as u32 / 4;
        let mut v = vec![kind, looped as u8];
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&loop_words.to_le_bytes());
        v.extend_from_slice(&(total - loop_words as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    /// A SWAR holding the given wave records.
    pub fn archive(waves: &[Vec<u8>]) -> Vec<u8> {
        let table = 0x3C + waves.len() * 4;
        let mut out = vec![0u8; 0x3C];
        out[..4].copy_from_slice(b"SWAR");
        out[0x10..0x14].copy_from_slice(b"DATA");
        out[0x38..0x3C].copy_from_slice(&(waves.len() as u32).to_le_bytes());
        let mut body = Vec::new();
        for w in waves {
            out.extend_from_slice(&((table + body.len()) as u32).to_le_bytes());
            body.extend_from_slice(w);
        }
        out.extend_from_slice(&body);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn pcm8_and_pcm16_decode() {
        let a = archive(&[
            record(0, false, 8000, 0, &[0x00, 0x01, 0xFF, 0x80]),
            record(1, true, 16000, 1, &[0x00, 0x80, 0xFF, 0x7F]),
        ]);
        let s = Swar::parse(&a).unwrap();
        assert_eq!(s.len(), 2);
        let w0 = s.wave(0).unwrap();
        assert_eq!(w0.decode(), vec![0, 256, -256, -32768]);
        assert_eq!(w0.native_rate(), 8000.0);
        let w1 = s.wave(1).unwrap();
        assert_eq!(w1.decode(), vec![-32768, 32767]);
        assert!(w1.looped);
        assert_eq!(w1.loop_start(), 2);
        assert!(s.wave(2).is_err());
    }

    #[test]
    fn adpcm_decodes_a_known_ramp() {
        // Seed: predictor 100, index 0. Nibble 0x4 adds step 7 >> 3 + 7 = 7.
        let data = [100, 0, 0, 0, 0x44, 0x04];
        let mut padded = data.to_vec();
        padded.extend_from_slice(&[0, 0]);
        let a = archive(&[record(2, false, 8000, 0, &padded)]);
        let w = Swar::parse(&a).unwrap().wave(0).unwrap();
        assert_eq!(w.decoded_len(), 8);
        let pcm = w.decode();
        assert_eq!(pcm.len(), 8);
        assert_eq!(pcm[0], 107);
        assert!(pcm[1] > pcm[0] && pcm[2] > pcm[1]);
    }

    #[test]
    fn standalone_file_and_errors() {
        let rec = record(0, false, 8000, 0, &[1, 2, 3, 4]);
        let mut file = vec![0u8; 0x18];
        file[..4].copy_from_slice(b"SWAV");
        file[0x10..0x14].copy_from_slice(b"DATA");
        file.extend_from_slice(&rec);
        assert_eq!(Swav::parse_file(&file).unwrap().decoded_len(), 4);
        assert!(Swav::parse_file(&file[..0x20]).is_err());
        assert!(Swar::parse(&[0; 0x40]).is_err());
        let mut cut = archive(&[rec]);
        cut.truncate(cut.len() - 2);
        assert!(Swar::parse(&cut).unwrap().wave(0).is_err());
    }
}
