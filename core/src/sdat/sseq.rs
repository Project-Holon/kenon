//! SSEQ sequences: the bytecode decoder and the per-track interpreter.
//!
//! ```text
//! "SSEQ" bom version size hdrsize blocks, then "DATA" u32 size at 0x10
//! 0x18  u32 offset of the payload (0x1C)
//! payload: track 0 starts with 0xFE (allocate tracks, u16 mask) and one
//!          0x93 (open track: u8 number, u24 offset) per other track; its
//!          own commands follow. Every offset (open track, jump, call) is
//!          from the start of the payload.
//! ```
//!
//! Commands: below 0x80 a note (`key; u8 velocity; varint length`), 0x80
//! rest (varint), 0x81 program change (varint), 0x93 open track, 0x94 jump
//! and 0x95 call (u24), 0xC0-0xD6 one u8 argument (C0 pan, C1 volume, C3
//! transpose as s8, C4 pitch bend as s8, C5 bend range, C7 note wait, D0-D3
//! attack, decay, sustain, release, D4 loop start, D5 expression), 0xE0 and
//! 0xE1 a u16 (E1 is the tempo), 0xE3 an s16, 0xB0-0xBD a variable command
//! (u8 variable, s16 value), 0xFC loop end, 0xFD return, 0xFE allocate and
//! 0xFF end of track. Lengths are MIDI-style varints, 7 bits per byte.
//!
//! The interpreter runs one track at a time on its own tick count (48 ticks
//! to the quarter note) and records the notes it plays. A backward jump is
//! taken once and refused the second time, and the track then ends with
//! `looped` set. The prefixes 0xA0, 0xA1 and 0xA2 (random, variable and
//! conditional) are skipped by the widths 5, 2 and 1 bytes, which is an
//! approximation; any other unknown command ends the track.
//!
//! References: eidos `engine/audio/sseq.c`, aletheia `tools/assets/sseq.py`
//! and `sseqsynth.py`.

use crate::nitro::Error;

/// Ticks in a quarter note.
pub const TICKS_PER_QUARTER: u32 = 48;

const MAX_STEPS: usize = 300_000;
const MAX_CALL_DEPTH: usize = 32;
const MAX_TRACKS: usize = 32;

/// A parsed sequence: a view of the payload all offsets are measured from.
#[derive(Debug, Clone, Copy)]
pub struct Sseq<'a> {
    payload: &'a [u8],
}

/// One decoded command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Note {
        key: u8,
        velocity: u8,
        length: u32,
    },
    Rest(u32),
    Program(u32),
    OpenTrack {
        track: u8,
        offset: u32,
    },
    Jump(u32),
    Call(u32),
    AllocTrack(u16),
    Return,
    LoopStart(u8),
    LoopEnd,
    End,
    Pan(u8),
    Volume(u8),
    Expression(u8),
    Transpose(i8),
    PitchBend(i8),
    BendRange(u8),
    NoteWait(bool),
    Attack(u8),
    Decay(u8),
    Sustain(u8),
    Release(u8),
    Tempo(u16),
    /// A command that is decoded so the bytecode stays in step, and then
    /// has no effect on the sound.
    Ignored,
}

/// Reads a MIDI-style varint at `pos`; returns the value and its width.
fn varint(data: &[u8], pos: usize) -> Option<(u32, usize)> {
    let mut value = 0u32;
    for width in 1..=5 {
        let b = *data.get(pos + width - 1)?;
        value = (value << 7) | (b & 0x7F) as u32;
        if b & 0x80 == 0 {
            return Some((value, width));
        }
    }
    None
}

fn u24(data: &[u8], pos: usize) -> Option<u32> {
    let b = data.get(pos..pos + 3)?;
    Some(b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16)
}

/// Decodes the command at `pos`; returns it and the position after it, or
/// None for an unknown command or a read past the end.
pub fn decode(data: &[u8], pos: usize) -> Option<(Command, usize)> {
    let op = *data.get(pos)?;
    let arg = |n: usize| data.get(pos + n).copied();
    Some(match op {
        0x00..=0x7F => {
            let velocity = arg(1)?;
            let (length, w) = varint(data, pos + 2)?;
            (
                Command::Note {
                    key: op,
                    velocity,
                    length,
                },
                pos + 2 + w,
            )
        }
        0x80 | 0x81 => {
            let (v, w) = varint(data, pos + 1)?;
            let c = if op == 0x80 {
                Command::Rest(v)
            } else {
                Command::Program(v)
            };
            (c, pos + 1 + w)
        }
        0x93 => (
            Command::OpenTrack {
                track: arg(1)?,
                offset: u24(data, pos + 2)?,
            },
            pos + 5,
        ),
        0x94 => (Command::Jump(u24(data, pos + 1)?), pos + 4),
        0x95 => (Command::Call(u24(data, pos + 1)?), pos + 4),
        0xFE => {
            let m = data.get(pos + 1..pos + 3)?;
            (
                Command::AllocTrack(u16::from_le_bytes([m[0], m[1]])),
                pos + 3,
            )
        }
        0xFC => (Command::LoopEnd, pos + 1),
        0xFD => (Command::Return, pos + 1),
        0xFF => (Command::End, pos + 1),
        0xC0..=0xD6 => {
            let v = arg(1)?;
            let c = match op {
                0xC0 => Command::Pan(v),
                0xC1 => Command::Volume(v),
                0xC3 => Command::Transpose(v as i8),
                0xC4 => Command::PitchBend(v as i8),
                0xC5 => Command::BendRange(v),
                0xC7 => Command::NoteWait(v != 0),
                0xD0 => Command::Attack(v),
                0xD1 => Command::Decay(v),
                0xD2 => Command::Sustain(v),
                0xD3 => Command::Release(v),
                0xD4 => Command::LoopStart(v),
                0xD5 => Command::Expression(v),
                _ => Command::Ignored,
            };
            (c, pos + 2)
        }
        0xE0 | 0xE1 | 0xE3 => {
            let b = data.get(pos + 1..pos + 3)?;
            let v = u16::from_le_bytes([b[0], b[1]]);
            let c = if op == 0xE1 {
                Command::Tempo(v)
            } else {
                Command::Ignored
            };
            (c, pos + 3)
        }
        0xB0..=0xB6 | 0xB8..=0xBD => {
            data.get(pos + 3)?;
            (Command::Ignored, pos + 4)
        }
        _ => return None,
    })
}

impl<'a> Sseq<'a> {
    /// Parses an SSEQ file.
    pub fn parse(bytes: &'a [u8]) -> Result<Sseq<'a>, Error> {
        if bytes.get(..4) != Some(b"SSEQ") {
            return Err(Error::WrongMagic {
                want: *b"SSEQ",
                found: bytes.get(..4).and_then(|m| m.try_into().ok()),
            });
        }
        if bytes.get(0x10..0x14) != Some(b"DATA") {
            return Err(Error::MissingBlock(*b"DATA"));
        }
        let at = crate::nitro::read_u32(bytes, 0x18)? as usize;
        let payload = bytes.get(at..).ok_or(Error::Truncated)?;
        if payload.is_empty() {
            return Err(Error::Truncated);
        }
        Ok(Sseq { payload })
    }

    /// The payload every offset counts from.
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    /// The tracks as (number, start offset): the open-track commands of the
    /// header in order, then track 0 right after the header.
    pub fn track_starts(&self) -> Vec<(u8, u32)> {
        let mut starts: Vec<(u8, u32)> = Vec::new();
        let mut pos = 0;
        let mut header_end = 0;
        while let Some((cmd, next)) = decode(self.payload, pos) {
            match cmd {
                Command::AllocTrack(_) => {}
                Command::OpenTrack { track, offset } => {
                    if let Some(i) = starts.iter().position(|s| s.0 == track) {
                        starts[i].1 = offset;
                    } else if starts.len() < MAX_TRACKS {
                        starts.push((track, offset));
                    }
                }
                _ => break,
            }
            pos = next;
            header_end = next;
        }
        match starts.iter_mut().find(|s| s.0 == 0) {
            Some(s) => s.1 = header_end as u32,
            None => starts.push((0, header_end as u32)),
        }
        starts
    }
}

/// One note as a track played it, with the controller state at that moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub tick: u32,
    pub length: u32,
    /// The key after transpose, 0-127.
    pub key: u8,
    pub velocity: u8,
    pub program: u32,
    pub volume: u8,
    pub expression: u8,
    pub pan: u8,
    /// Pitch bend in semitones.
    pub bend: f64,
    /// Envelope overrides the track set, if any.
    pub attack: Option<u8>,
    pub decay: Option<u8>,
    pub sustain: Option<u8>,
    pub release: Option<u8>,
    /// The track number it came from.
    pub track: u8,
}

/// What running one track produced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackRun {
    pub notes: Vec<Note>,
    /// (tick, beats per minute) pairs.
    pub tempos: Vec<(u32, u16)>,
    pub end_tick: u32,
    /// The track ended at a backward jump it refused: the sequence loops.
    pub looped: bool,
    /// The track stopped at a command it does not know.
    pub unknown: bool,
}

/// Runs the track that starts at `start` and records its notes.
pub fn run_track(payload: &[u8], start: usize, track: u8) -> TrackRun {
    let mut run = TrackRun::default();
    let mut pc = start;
    let mut calls: Vec<usize> = Vec::new();
    let mut loops: Vec<(usize, u32)> = Vec::new();
    let mut seen_jumps: Vec<usize> = Vec::new();
    let mut tick = 0u32;
    let mut program = 0u32;
    let (mut volume, mut expression, mut pan) = (127u8, 127u8, 64u8);
    let mut transpose = 0i32;
    let mut bend = 0i32;
    let mut bend_range = 2u8;
    let mut note_wait = true;
    let mut adsr: [Option<u8>; 4] = [None; 4];

    for _ in 0..MAX_STEPS {
        let Some((cmd, next)) = decode(payload, pc) else {
            match payload.get(pc) {
                Some(0xA0) => pc += 5,
                Some(0xA1) => pc += 2,
                Some(0xA2) => pc += 1,
                Some(_) => {
                    run.unknown = true;
                    break;
                }
                None => break,
            }
            continue;
        };
        pc = next;
        match cmd {
            Command::Note {
                key,
                velocity,
                length,
            } => {
                run.notes.push(Note {
                    tick,
                    length,
                    key: (key as i32 + transpose).clamp(0, 127) as u8,
                    velocity,
                    program,
                    volume,
                    expression,
                    pan,
                    bend: bend as f64 / 127.0 * bend_range as f64,
                    attack: adsr[0],
                    decay: adsr[1],
                    sustain: adsr[2],
                    release: adsr[3],
                    track,
                });
                if note_wait {
                    tick = tick.saturating_add(length);
                }
            }
            Command::Rest(n) => tick = tick.saturating_add(n),
            Command::Program(p) => program = p,
            Command::Pan(v) => pan = v,
            Command::Volume(v) => volume = v,
            Command::Expression(v) => expression = v,
            Command::Transpose(v) => transpose = v as i32,
            Command::PitchBend(v) => bend = v as i32,
            Command::BendRange(v) => bend_range = v,
            Command::NoteWait(on) => note_wait = on,
            Command::Attack(v) => adsr[0] = Some(v),
            Command::Decay(v) => adsr[1] = Some(v),
            Command::Sustain(v) => adsr[2] = Some(v),
            Command::Release(v) => adsr[3] = Some(v),
            Command::Tempo(bpm) => run.tempos.push((tick, bpm)),
            Command::LoopStart(count) => {
                if loops.len() < MAX_CALL_DEPTH {
                    // Zero means forever: play the body once and note the loop.
                    if count == 0 {
                        run.looped = true;
                    }
                    loops.push((pc, (count as u32).saturating_sub(1)));
                }
            }
            Command::LoopEnd => {
                if let Some(top) = loops.last_mut() {
                    if top.1 > 0 {
                        top.1 -= 1;
                        pc = top.0;
                    } else {
                        loops.pop();
                    }
                }
            }
            Command::Jump(target) => {
                let target = target as usize;
                let site = next - 4;
                if target > site {
                    pc = target;
                } else if seen_jumps.contains(&site) {
                    run.looped = true;
                    break;
                } else {
                    seen_jumps.push(site);
                    pc = target;
                }
            }
            Command::Call(target) => {
                if calls.len() >= MAX_CALL_DEPTH {
                    break;
                }
                calls.push(pc);
                pc = target as usize;
            }
            Command::Return => match calls.pop() {
                Some(back) => pc = back,
                None => break,
            },
            Command::End => break,
            Command::AllocTrack(_) | Command::OpenTrack { .. } | Command::Ignored => {}
        }
    }
    run.end_tick = tick;
    run
}

#[cfg(test)]
pub(crate) mod testing {
    /// An SSEQ file around a payload.
    pub fn file(payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; 0x1C];
        out[..4].copy_from_slice(b"SSEQ");
        out[0x10..0x14].copy_from_slice(b"DATA");
        out[0x18..0x1C].copy_from_slice(&0x1Cu32.to_le_bytes());
        out.extend_from_slice(payload);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_commands() {
        assert_eq!(
            decode(&[60, 100, 0x81, 0x00], 0),
            Some((
                Command::Note {
                    key: 60,
                    velocity: 100,
                    length: 128
                },
                4
            ))
        );
        assert_eq!(decode(&[0x80, 48], 0), Some((Command::Rest(48), 2)));
        assert_eq!(decode(&[0xC4, 0xFF], 0), Some((Command::PitchBend(-1), 2)));
        assert_eq!(
            decode(&[0xE1, 0x78, 0x00], 0),
            Some((Command::Tempo(120), 3))
        );
        assert_eq!(
            decode(&[0x93, 1, 0x10, 0, 0], 0),
            Some((
                Command::OpenTrack {
                    track: 1,
                    offset: 16
                },
                5
            ))
        );
        assert_eq!(decode(&[0xB0, 1, 2, 0], 0), Some((Command::Ignored, 4)));
        assert_eq!(decode(&[0x90], 0), None);
        assert_eq!(decode(&[60, 100], 0), None);
        assert_eq!(decode(&[], 0), None);
    }

    #[test]
    fn header_and_tracks() {
        // Track 0 header: alloc, open track 1 at offset 12; track 0 body at 8.
        let payload = [
            0xFE, 0x03, 0x00, 0x93, 1, 12, 0, 0, // 0..8
            60, 100, 48, 0xFF, // 8..12 track 0: one note, end
            72, 90, 24, 0xFF, // 12..16 track 1
        ];
        let bytes = testing::file(&payload);
        let q = Sseq::parse(&bytes).unwrap();
        assert_eq!(q.track_starts(), vec![(1, 12), (0, 8)]);
        let t0 = run_track(q.payload(), 8, 0);
        assert_eq!(t0.notes.len(), 1);
        assert_eq!(t0.end_tick, 48);
        let t1 = run_track(q.payload(), 12, 1);
        assert_eq!(t1.notes[0].key, 72);
        assert_eq!(t1.notes[0].track, 1);
        assert!(Sseq::parse(&bytes[..0x10]).is_err());
    }

    #[test]
    fn controllers_tempo_and_rests() {
        let payload = [
            0xE1, 0x3C, 0x00, // tempo 60
            0xC1, 80, 0xC0, 10, 0xC3, 2, // volume, pan, transpose +2
            0xD0, 5, // attack override
            60, 127, 24, // note
            0x80, 24, // rest
            0xC7, 0, 62, 127, 10, // note wait off: tick does not advance
            64, 127, 10, 0xFF,
        ];
        let run = run_track(&payload, 0, 3);
        assert_eq!(run.tempos, vec![(0, 60)]);
        let n = &run.notes[0];
        assert_eq!((n.key, n.volume, n.pan, n.attack), (62, 80, 10, Some(5)));
        assert_eq!(run.notes[1].tick, 48);
        assert_eq!(run.notes[2].tick, 48);
        assert_eq!(run.end_tick, 48);
    }

    #[test]
    fn jumps_calls_and_loops() {
        // A backward jump is taken once, then refused.
        let payload = [60, 100, 10, 0x94, 0, 0, 0];
        let run = run_track(&payload, 0, 0);
        assert_eq!(run.notes.len(), 2);
        assert!(run.looped);
        // Call and return.
        let payload = [0x95, 5, 0, 0, 0xFF, 60, 100, 10, 0xFD];
        let run = run_track(&payload, 0, 0);
        assert_eq!(run.notes.len(), 1);
        assert!(!run.looped);
        // A counted loop plays its body that many times.
        let payload = [0xD4, 3, 60, 100, 4, 0xFC, 0xFF];
        assert_eq!(run_track(&payload, 0, 0).notes.len(), 3);
        // An unknown command stops the track.
        let run = run_track(&[60, 100, 4, 0x90, 60, 100, 4], 0, 0);
        assert!(run.unknown);
        assert_eq!(run.notes.len(), 1);
    }
}
