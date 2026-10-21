//! NANR (on disk "RNAN"): cell animations.
//!
//! The ABNK block (on disk "KNBA") body:
//!
//! ```text
//! 0x00 u16 sequence count   0x02 u16 frame count (all sequences)
//! 0x04 u32 sequences offset  0x08 u32 frames offset  0x0C u32 frame data
//!   offset (all from this body)
//! sequence, 16 bytes: u16 frames, u16 loop start, u32 kind (low byte: frame
//!   data shape), u32 play mode, u32 offset of its first frame (from the
//!   frames array)
//! frame, 8 bytes: u32 offset into the frame data, u16 duration (in
//!   60 Hz frames), u16 0xBEEF
//! frame data: index (u16 cell), SRT (u16 cell, u16 rotation, s32 scale x,
//!   s32 scale y in 20.12, s16 x, s16 y) or T (u16 cell, u16, s16 x, s16 y)
//! ```
//!
//! Reference: eidos `g2d_nanr_parse`, `g2d_nanr_frame_data` and
//! `g2d_nanr_frame_at_time` (the walk the game runs at 0x020643CC).

use crate::nitro::{self, read_u16, read_u32, Error};

pub const MAGIC: &[u8; 4] = b"RNAN";
const ABNK: &[u8; 4] = b"KNBA";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub cell: u16,
    pub duration: u16,
    /// Rotation, 0x10000 is a full turn.
    pub rotation: u16,
    /// Scale in 20.12 fixed point, 4096 is 1.0.
    pub scale: (i32, i32),
    pub offset: (i16, i16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    pub loop_start: u16,
    pub kind: u32,
    pub play_mode: u32,
    pub frames: Vec<Frame>,
}

impl Sequence {
    /// The frame shown `t` 60 Hz frames after the start: durations add up,
    /// and past the end the sequence loops from `loop_start` (eidos
    /// `g2d_nanr_frame_at_time`; a zero duration counts as one).
    pub fn frame_at(&self, t: u32) -> usize {
        let n = self.frames.len();
        if n <= 1 {
            return 0;
        }
        let duration = |f: &Frame| (f.duration as u32).max(1);
        let loop_start = (self.loop_start as usize).min(n - 1);
        let loop_time: u32 = self.frames[..loop_start].iter().map(duration).sum();
        let total: u32 = self.frames.iter().map(duration).sum();
        let t = if t >= total {
            let len = total - loop_time;
            if len == 0 {
                loop_time
            } else {
                loop_time + (t - total) % len
            }
        } else {
            t
        };
        let mut acc = 0;
        for (i, f) in self.frames.iter().enumerate() {
            acc += duration(f);
            if t < acc {
                return i;
            }
        }
        n - 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nanr {
    pub sequences: Vec<Sequence>,
}

impl Nanr {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let body = nitro::block(bytes, MAGIC, ABNK)?;
        let count = read_u16(body, 0)? as usize;
        let seq_at = read_u32(body, 4)? as usize;
        let frames_at = read_u32(body, 8)? as usize;
        let data_at = read_u32(body, 12)? as usize;

        let sequences = (0..count)
            .map(|i| {
                let s = seq_at + i * 16;
                let n = read_u16(body, s)? as usize;
                let loop_start = read_u16(body, s + 2)?;
                let kind = read_u32(body, s + 4)?;
                let play_mode = read_u32(body, s + 8)?;
                let first = frames_at + read_u32(body, s + 12)? as usize;
                let frames = (0..n)
                    .map(|j| {
                        let f = first + j * 8;
                        let data = data_at + read_u32(body, f)? as usize;
                        let duration = read_u16(body, f + 4)?;
                        let cell = read_u16(body, data)?;
                        let mut frame = Frame {
                            cell,
                            duration,
                            rotation: 0,
                            scale: (4096, 4096),
                            offset: (0, 0),
                        };
                        match kind & 0xFF {
                            1 => {
                                frame.rotation = read_u16(body, data + 2)?;
                                frame.scale = (
                                    read_u32(body, data + 4)? as i32,
                                    read_u32(body, data + 8)? as i32,
                                );
                                frame.offset = (
                                    read_u16(body, data + 12)? as i16,
                                    read_u16(body, data + 14)? as i16,
                                );
                            }
                            2 => {
                                frame.offset = (
                                    read_u16(body, data + 4)? as i16,
                                    read_u16(body, data + 6)? as i16,
                                );
                            }
                            _ => {}
                        }
                        Ok(frame)
                    })
                    .collect::<Result<_, Error>>()?;
                Ok(Sequence {
                    loop_start,
                    kind,
                    play_mode,
                    frames,
                })
            })
            .collect::<Result<_, Error>>()?;
        Ok(Nanr { sequences })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One T sequence (kind 2) with three frames of cells 4, 5, 6.
    fn sample() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_le_bytes());
        body.extend_from_slice(&3u16.to_le_bytes());
        body.extend_from_slice(&16u32.to_le_bytes()); // sequences
        body.extend_from_slice(&32u32.to_le_bytes()); // frames
        body.extend_from_slice(&56u32.to_le_bytes()); // data
                                                      // Sequence: 3 frames, loop from 1, kind 2, mode 2, first frame 0.
        for v in [3u16, 1] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        for v in [2u32, 2, 0] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        for (i, d) in [(0u32, 4u16), (8, 2), (16, 0)] {
            body.extend_from_slice(&i.to_le_bytes());
            body.extend_from_slice(&d.to_le_bytes());
            body.extend_from_slice(&0xBEEFu16.to_le_bytes());
        }
        for (cell, x, y) in [(4u16, -3i16, 2i16), (5, 0, 0), (6, 1, 1)] {
            body.extend_from_slice(&cell.to_le_bytes());
            body.extend_from_slice(&0u16.to_le_bytes());
            body.extend_from_slice(&x.to_le_bytes());
            body.extend_from_slice(&y.to_le_bytes());
        }
        nitro::build(MAGIC, &[(ABNK, &body)])
    }

    #[test]
    fn parse_and_time() {
        let nanr = Nanr::parse(&sample()).unwrap();
        let seq = &nanr.sequences[0];
        assert_eq!(seq.frames.len(), 3);
        assert_eq!(seq.frames[0].cell, 4);
        assert_eq!(seq.frames[0].offset, (-3, 2));
        assert_eq!(seq.frames[2].duration, 0);
        // Durations 4, 2, 0 (counts as 1): total 7, loop from frame 1 at 4.
        let at: Vec<usize> = (0..12).map(|t| seq.frame_at(t)).collect();
        assert_eq!(at, vec![0, 0, 0, 0, 1, 1, 2, 1, 1, 2, 1, 1]);
    }
}
