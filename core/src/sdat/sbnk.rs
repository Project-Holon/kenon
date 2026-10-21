//! SBNK instrument banks.
//!
//! ```text
//! "SBNK" bom version size hdrsize blocks, then "DATA" u32 size at 0x10
//! 0x38  u32 instrument count
//! 0x3C  u32 entry * count: low byte = record type, bits 8..24 = offset
//! ```
//!
//! The shared region record is 10 bytes: `u16 wave id; u16 wave archive;
//! u8 base note; u8 attack; u8 decay; u8 sustain; u8 release; u8 pan`. The
//! wave archive is a slot 0-3 in the bank's own table of four.
//!
//! Instrument types:
//! - 1: one region (the record).
//! - 2: PSG square wave; the first u16 is the duty (0-7), no wave.
//! - 3: PSG noise; no wave.
//! - 16: drum set. `u8 low; u8 high;` then one region per note in
//!   `low..=high`, each `u16 type; record` (12 bytes). The note picks the
//!   region directly and is clamped.
//! - 17: key split. Eight ascending split points (0 = unused), then one
//!   12-byte region per used point. The first region whose point is at or
//!   above the note wins; the last one catches higher notes.
//!
//! Offsets are from the start of the file. Types 0, 4 and 5 have no
//! playable region.
//!
//! References: eidos `engine/audio/sbnk.c`, aletheia `tools/assets/sbnk.py`.

use crate::nitro::{read_u16, read_u32, Error};

/// What a region makes sound with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A wave in the bank's wave archive slot `archive`.
    Wave { archive: u16, wave: u16 },
    /// A PSG square wave with the given duty (0-7, high for `(duty+1)/8`).
    Square { duty: u16 },
    /// PSG noise.
    Noise,
}

/// One playable region of an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub source: Source,
    pub base_note: u8,
    pub attack: u8,
    pub decay: u8,
    pub sustain: u8,
    pub release: u8,
    /// 0-127, 64 is centre.
    pub pan: u8,
}

/// A parsed bank.
#[derive(Debug, Clone, Copy)]
pub struct Sbnk<'a> {
    bytes: &'a [u8],
    count: u32,
}

impl<'a> Sbnk<'a> {
    /// Parses a bank file.
    pub fn parse(bytes: &'a [u8]) -> Result<Sbnk<'a>, Error> {
        if bytes.get(..4) != Some(b"SBNK") {
            return Err(Error::WrongMagic {
                want: *b"SBNK",
                found: bytes.get(..4).and_then(|m| m.try_into().ok()),
            });
        }
        if bytes.get(0x10..0x14) != Some(b"DATA") {
            return Err(Error::MissingBlock(*b"DATA"));
        }
        let count = read_u32(bytes, 0x38)?;
        if 0x3C + count as usize * 4 > bytes.len() {
            return Err(Error::Truncated);
        }
        Ok(Sbnk { bytes, count })
    }

    /// How many instruments (programs) the bank lists.
    pub fn len(&self) -> u32 {
        self.count
    }

    /// True when the bank lists no instruments.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The record type of a program (0, 1, 2, 3, 4, 5, 16 or 17).
    pub fn instrument_type(&self, program: u32) -> Option<u8> {
        if program >= self.count {
            return None;
        }
        Some(read_u32(self.bytes, 0x3C + program as usize * 4).ok()? as u8)
    }

    /// The 10-byte record at `off` and its leading u16 (a wave id, or the
    /// duty of a square instrument).
    fn record(&self, off: usize) -> Option<(u16, Region)> {
        let r = self.bytes.get(off..off + 10)?;
        let lead = read_u16(r, 0).ok()?;
        let archive = read_u16(r, 2).ok()?;
        Some((
            lead,
            Region {
                source: Source::Wave {
                    archive,
                    wave: lead,
                },
                base_note: r[4],
                attack: r[5],
                decay: r[6],
                sustain: r[7],
                release: r[8],
                pan: r[9],
            },
        ))
    }

    /// The region `program` uses for `note`, or None when the program is
    /// empty, out of range or its record runs off the file.
    pub fn resolve(&self, program: u32, note: u8) -> Option<Region> {
        if program >= self.count {
            return None;
        }
        let entry = read_u32(self.bytes, 0x3C + program as usize * 4).ok()?;
        let kind = entry as u8;
        let off = ((entry >> 8) & 0xFFFF) as usize;
        match kind {
            1 => self.record(off).map(|(_, r)| r),
            2 => {
                let (duty, mut r) = self.record(off)?;
                r.source = Source::Square { duty: duty & 7 };
                Some(r)
            }
            3 => {
                let (_, mut r) = self.record(off)?;
                r.source = Source::Noise;
                Some(r)
            }
            16 => {
                let low = *self.bytes.get(off)? as i32;
                let high = *self.bytes.get(off + 1)? as i32;
                let count = high - low + 1;
                if count <= 0 {
                    return None;
                }
                let idx = (note as i32 - low).clamp(0, count - 1) as usize;
                self.record(off + 2 + idx * 12 + 2).map(|(_, r)| r)
            }
            17 => {
                let splits = self.bytes.get(off..off + 8)?;
                let used: Vec<u8> = splits.iter().copied().filter(|&s| s != 0).collect();
                if used.is_empty() {
                    return None;
                }
                let chosen = used
                    .iter()
                    .position(|&s| note <= s)
                    .unwrap_or(used.len() - 1);
                self.record(off + 8 + chosen * 12 + 2).map(|(_, r)| r)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    /// A 10-byte region record.
    pub fn region(lead: u16, archive: u16, base: u8, adsr: [u8; 4], pan: u8) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&lead.to_le_bytes());
        v.extend_from_slice(&archive.to_le_bytes());
        v.push(base);
        v.extend_from_slice(&adsr);
        v.push(pan);
        v
    }

    /// A bank from (type, payload) pairs; payloads are placed after the
    /// instrument table and the offsets patched in.
    pub fn bank(instruments: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let table = 0x3C + instruments.len() * 4;
        let mut out = vec![0u8; 0x3C];
        out[..4].copy_from_slice(b"SBNK");
        out[0x10..0x14].copy_from_slice(b"DATA");
        out[0x38..0x3C].copy_from_slice(&(instruments.len() as u32).to_le_bytes());
        let mut body = Vec::new();
        for (kind, payload) in instruments {
            let entry = (*kind as u32) | (((table + body.len()) as u32) << 8);
            out.extend_from_slice(&entry.to_le_bytes());
            body.extend_from_slice(payload);
        }
        out.extend_from_slice(&body);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    const ADSR: [u8; 4] = [1, 2, 3, 4];

    #[test]
    fn single_square_noise() {
        let b = bank(&[
            (1, region(7, 2, 60, ADSR, 64)),
            (2, region(3, 0, 60, ADSR, 64)),
            (3, region(0, 0, 60, ADSR, 40)),
            (0, vec![]),
        ]);
        let b = Sbnk::parse(&b).unwrap();
        assert_eq!(b.len(), 4);
        let r = b.resolve(0, 60).unwrap();
        assert_eq!(
            r.source,
            Source::Wave {
                archive: 2,
                wave: 7
            }
        );
        assert_eq!((r.attack, r.sustain, r.release, r.pan), (1, 3, 4, 64));
        assert_eq!(b.resolve(1, 70).unwrap().source, Source::Square { duty: 3 });
        let n = b.resolve(2, 70).unwrap();
        assert_eq!(n.source, Source::Noise);
        assert_eq!(n.pan, 40);
        assert_eq!(b.instrument_type(3), Some(0));
        assert!(b.resolve(3, 60).is_none());
        assert!(b.resolve(4, 60).is_none());
    }

    #[test]
    fn drum_set_and_key_split() {
        let mut drum = vec![36, 38];
        for wave in [10u16, 11, 12] {
            drum.extend_from_slice(&1u16.to_le_bytes());
            drum.extend_from_slice(&region(wave, 0, 60, ADSR, 64));
        }
        let mut split = vec![50, 80, 0, 0, 0, 0, 0, 0];
        for wave in [20u16, 21] {
            split.extend_from_slice(&1u16.to_le_bytes());
            split.extend_from_slice(&region(wave, 0, 60, ADSR, 64));
        }
        let b = bank(&[(16, drum), (17, split)]);
        let b = Sbnk::parse(&b).unwrap();
        let wave = |p, n| match b.resolve(p, n).unwrap().source {
            Source::Wave { wave, .. } => wave,
            _ => panic!(),
        };
        assert_eq!(wave(0, 36), 10);
        assert_eq!(wave(0, 37), 11);
        assert_eq!(wave(0, 38), 12);
        assert_eq!(wave(0, 10), 10);
        assert_eq!(wave(0, 120), 12);
        assert_eq!(wave(1, 50), 20);
        assert_eq!(wave(1, 51), 21);
        assert_eq!(wave(1, 127), 21);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Sbnk::parse(&[0; 0x40]).is_err());
        let mut b = bank(&[(1, region(0, 0, 60, ADSR, 64))]);
        b.truncate(0x3E);
        assert!(Sbnk::parse(&b).is_err());
        // A record that runs off the end is no region.
        let mut b = bank(&[(1, region(0, 0, 60, ADSR, 64))]);
        b.truncate(b.len() - 3);
        assert!(Sbnk::parse(&b).unwrap().resolve(0, 60).is_none());
    }
}
