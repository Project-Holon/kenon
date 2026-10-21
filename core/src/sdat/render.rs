//! Renders a sequence to PCM: tempo map, instrument lookup, voices, mixer.
//!
//! The output is interleaved stereo `i16` at [`OUTPUT_RATE`] (32768 Hz, the
//! DS mixer's own rate).
//!
//! How a sequence becomes sound:
//!
//! 1. The SEQ record names the bank, and the bank record names up to four
//!    wave archives. A region's wave archive is a slot 0-3 of that table.
//! 2. Every track runs on its own (`sseq::run_track`) and yields notes in
//!    ticks. All tempo commands of all tracks form one tempo map (48 ticks
//!    to the quarter note, 120 BPM before the first command).
//! 3. Each note picks a region (`Sbnk::resolve`) and becomes a voice: a wave
//!    resampled by `2^((key - base note) / 12)` with linear interpolation
//!    and looping, or a PSG square or noise generator.
//! 4. The envelope follows the DS sound driver. It runs at 192 steps a
//!    second on a level in 1/1024 dB units, from -92544 (silent) to 0.
//!    Attack multiplies the level by `rate / 256` each step (rate from the
//!    attack byte: 127 is instant), decay and release subtract a fall (127
//!    is instant), and sustain is a level. The note-off at the end of the
//!    gate starts the release. A wave that is not looped ends where its
//!    data ends.
//! 5. Velocity, track volume, expression and the SEQ volume each scale the
//!    amplitude by `(x / 127)^2`, which is the driver's dB curve to a good
//!    approximation. Pan is a linear cross-fade about 64 that leaves the
//!    centre at unity.
//! 6. Voices are summed with no channel limit. If the sum peaks above 32000
//!    it is scaled down to that. The clip ends when the last voice ends.
//!
//! Approximations: the driver's exact dB tables, hardware channel limits and
//! priorities, portamento, modulation, sweep and tie are not modelled.
//!
//! References: eidos `engine/audio/player.c` and `mixer.c`, aletheia
//! `tools/assets/sseqsynth.py`. The envelope follows the public description
//! of the NitroSDK sound driver.

use std::collections::HashMap;

use crate::nitro::Error;

use super::container::Sdat;
use super::sbnk::{Region, Sbnk, Source};
use super::sseq::{self, Note, Sseq, TICKS_PER_QUARTER};
use super::swav::Swar;

/// The output rate, in Hz.
pub const OUTPUT_RATE: u32 = 32768;

const DEFAULT_BPM: f64 = 120.0;
const PEAK_LIMIT: f64 = 32000.0;
/// Envelope steps per second.
const ENV_RATE: f64 = 192.0;
const SILENT: i32 = -92544;
/// The longest a single voice may sound.
const MAX_VOICE_SECONDS: f64 = 30.0;
/// Level of a PSG tone against a full-scale wave.
const PSG_LEVEL: f64 = 0.25 * 32767.0;

/// What to render.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Stop after this many seconds (a looping sequence never ends).
    pub max_seconds: f64,
    /// Play with this bank slot instead of the one the SEQ record names.
    pub bank_override: Option<u32>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            max_seconds: 30.0,
            bank_override: None,
        }
    }
}

/// A rendered clip.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// Interleaved left and right samples.
    pub samples: Vec<i16>,
    pub rate: u32,
    /// The sequence loops on the DS (a track ended at a refused loop).
    pub looped: bool,
    /// The clip was cut at `max_seconds` with notes still sounding.
    pub truncated: bool,
    pub notes_rendered: usize,
    /// Notes with no region, no wave or no sound.
    pub notes_dropped: usize,
    /// Tracks that stopped at a command the decoder does not know.
    pub tracks_stopped_early: usize,
}

impl Clip {
    /// Frames (one left and one right sample each).
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    /// Length in seconds.
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate as f64
    }

    /// The largest absolute sample value.
    pub fn peak(&self) -> u32 {
        self.samples
            .iter()
            .map(|s| (*s as i32).unsigned_abs())
            .max()
            .unwrap_or(0)
    }
}

/// A decoded wave ready for resampling.
struct Wave {
    samples: Vec<i16>,
    loop_start: usize,
    looped: bool,
    rate: f64,
}

/// The bank a sequence plays with, with decoded waves cached by (slot, id).
struct BankCtx<'a> {
    sbnk: Sbnk<'a>,
    archives: [Option<Swar<'a>>; 4],
    cache: HashMap<(u16, u16), Option<Wave>>,
}

impl<'a> BankCtx<'a> {
    fn wave(&mut self, archive: u16, id: u16) -> Option<&Wave> {
        self.cache
            .entry((archive, id))
            .or_insert_with(|| {
                let swar = self.archives.get(archive as usize)?.as_ref()?;
                let w = swar.wave(id as usize).ok()?;
                let samples = w.decode();
                if samples.is_empty() {
                    return None;
                }
                Some(Wave {
                    loop_start: w.loop_start(),
                    looped: w.looped,
                    rate: w.native_rate(),
                    samples,
                })
            })
            .as_ref()
    }
}

/// Tempo map: (start tick, start second, bpm), ascending.
struct TempoMap(Vec<(u32, f64, f64)>);

impl TempoMap {
    fn new(events: &[(u32, u16)]) -> TempoMap {
        let mut sorted: Vec<(u32, f64)> = events
            .iter()
            .filter(|e| e.1 > 0)
            .map(|&(t, b)| (t, b as f64))
            .collect();
        sorted.sort_by_key(|e| e.0);
        // At one tick the last command wins.
        let mut ev: Vec<(u32, f64)> = Vec::new();
        for e in sorted {
            match ev.last_mut() {
                Some(last) if last.0 == e.0 => last.1 = e.1,
                _ => ev.push(e),
            }
        }
        if ev.first().map(|e| e.0) != Some(0) {
            ev.insert(0, (0, DEFAULT_BPM));
        }
        let mut segs: Vec<(u32, f64, f64)> = Vec::new();
        let mut time = 0.0;
        for (i, &(tick, bpm)) in ev.iter().enumerate() {
            if i > 0 {
                let (pt, ptime, pbpm) = segs[i - 1];
                time = ptime + (tick - pt) as f64 * 60.0 / (pbpm * TICKS_PER_QUARTER as f64);
            }
            segs.push((tick, time, bpm));
        }
        TempoMap(segs)
    }

    fn seconds(&self, tick: u32) -> f64 {
        let seg = self
            .0
            .iter()
            .rev()
            .find(|s| s.0 <= tick)
            .unwrap_or(&self.0[0]);
        seg.1 + (tick - seg.0) as f64 * 60.0 / (seg.2 * TICKS_PER_QUARTER as f64)
    }
}

/// The envelope rate of an attack byte (multiplier out of 256).
fn attack_rate(a: u8) -> i64 {
    const FAST: [i64; 19] = [
        0x00, 0x01, 0x05, 0x0E, 0x1A, 0x26, 0x33, 0x3F, 0x49, 0x54, 0x5C, 0x64, 0x6D, 0x74, 0x7B,
        0x7F, 0x84, 0x89, 0x8F,
    ];
    let a = a.min(127);
    if a >= 109 {
        FAST[(127 - a) as usize]
    } else {
        255 - a as i64
    }
}

/// How far a decay or release byte drops the level each step.
fn fall(v: u8) -> i64 {
    match v.min(127) {
        127 => 0xFFFF,
        126 => 0x3C00,
        v if v < 50 => v as i64 * 2 + 1,
        v => 0x1E00 / (126 - v as i64),
    }
}

/// The level of a sustain byte.
fn sustain_level(s: u8) -> i64 {
    let s = s.min(127);
    if s == 127 {
        0
    } else if s == 0 {
        SILENT as i64
    } else {
        ((40.0 * (s as f64 / 127.0).log10() * 1024.0) as i64).max(SILENT as i64)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Attack,
    Decay,
    Sustain,
    Release,
}

struct Envelope {
    stage: Stage,
    level: i64,
    attack: i64,
    decay: i64,
    sustain: i64,
    release: i64,
}

impl Envelope {
    fn new(region: &Region, note: &Note) -> Envelope {
        Envelope {
            stage: Stage::Attack,
            level: SILENT as i64,
            attack: attack_rate(note.attack.unwrap_or(region.attack)),
            decay: fall(note.decay.unwrap_or(region.decay)),
            sustain: sustain_level(note.sustain.unwrap_or(region.sustain)),
            release: fall(note.release.unwrap_or(region.release)),
        }
    }

    fn step(&mut self) {
        match self.stage {
            Stage::Attack => {
                self.level = self.level * self.attack / 256;
                if self.level >= 0 || self.attack == 0 {
                    self.level = 0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level -= self.decay;
                if self.level <= self.sustain {
                    self.level = self.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {}
            Stage::Release => self.level = (self.level - self.release).max(SILENT as i64),
        }
    }

    fn done(&self) -> bool {
        self.level <= SILENT as i64 && self.stage != Stage::Attack
    }

    /// Linear amplitude of the current level.
    fn amplitude(&self) -> f64 {
        if self.level <= SILENT as i64 {
            0.0
        } else {
            10f64.powf(self.level as f64 / 1024.0 / 20.0)
        }
    }
}

/// Left and right sums being built, in sample units.
struct Mix {
    left: Vec<f64>,
    right: Vec<f64>,
    limit: usize,
    /// Frames written so far (the buffers may be longer).
    used: usize,
}

impl Mix {
    fn add(&mut self, frame: usize, l: f64, r: f64) {
        if frame >= self.limit {
            return;
        }
        if frame >= self.left.len() {
            let want = (frame + 1).max(self.left.len() * 2).min(self.limit);
            self.left.resize(want, 0.0);
            self.right.resize(want, 0.0);
        }
        self.left[frame] += l;
        self.right[frame] += r;
        self.used = self.used.max(frame + 1);
    }
}

/// Where a voice reads its signal from.
enum Generator<'a> {
    Wave {
        wave: &'a Wave,
        step: f64,
        pos: f64,
    },
    Square {
        duty: f64,
        step: f64,
        phase: f64,
    },
    Noise {
        step: f64,
        clock: f64,
        lfsr: u32,
        level: f64,
    },
}

impl Generator<'_> {
    /// Whether the signal ran out (a wave that is not looped, at its end).
    fn exhausted(&self) -> bool {
        match self {
            Generator::Wave { wave, pos, .. } => !wave.looped && *pos >= wave.samples.len() as f64,
            _ => false,
        }
    }

    fn next(&mut self) -> f64 {
        match self {
            Generator::Wave { wave, step, pos } => {
                let len = wave.samples.len();
                let mut i0 = *pos as usize;
                let i1 = if wave.looped && i0 >= wave.loop_start {
                    let span = len - wave.loop_start;
                    i0 = wave.loop_start + (i0 - wave.loop_start) % span.max(1);
                    if i0 + 1 >= len {
                        wave.loop_start
                    } else {
                        i0 + 1
                    }
                } else if i0 + 1 >= len {
                    i0 = len - 1;
                    i0
                } else {
                    i0 + 1
                };
                let frac = *pos - pos.floor();
                let s0 = wave.samples[i0] as f64;
                let s1 = wave.samples[i1] as f64;
                *pos += *step;
                s0 + (s1 - s0) * frac
            }
            Generator::Square { duty, step, phase } => {
                let out = if phase.fract() < *duty { 1.0 } else { -1.0 };
                *phase += *step;
                out * PSG_LEVEL
            }
            Generator::Noise {
                step,
                clock,
                lfsr,
                level,
            } => {
                *clock += *step;
                while *clock >= 1.0 {
                    *clock -= 1.0;
                    let carry = *lfsr & 1;
                    *lfsr >>= 1;
                    if carry != 0 {
                        *lfsr ^= 0x6000;
                    }
                    *level = if carry != 0 { -1.0 } else { 1.0 };
                }
                *level * PSG_LEVEL
            }
        }
    }
}

fn square(x: f64) -> f64 {
    x * x
}

fn pan_gains(note_pan: u8, region_pan: u8) -> (f64, f64) {
    let pan = (note_pan as i32 - 64 + region_pan as i32).clamp(0, 127);
    let p = ((pan - 64) as f64 / 64.0).clamp(-1.0, 1.0);
    ((1.0 - p).min(1.0), (1.0 + p).min(1.0))
}

/// Everything a render needs, resolved from the SDAT.
struct Prepared<'a> {
    notes: Vec<Note>,
    tempos: TempoMap,
    bank: BankCtx<'a>,
    seq_gain: f64,
    looped: bool,
    stopped_early: usize,
}

fn prepare<'a>(sdat: &'a Sdat, seq: u32, opts: &Options) -> Result<Prepared<'a>, Error> {
    let rec = sdat.seq_record(seq).ok_or(Error::Bad("no such sequence"))?;
    let file = sdat
        .file(rec.file_id)
        .ok_or(Error::Bad("sequence file is not in the FAT"))?;
    let sseq = Sseq::parse(file)?;

    let bank_slot = opts.bank_override.unwrap_or(rec.bank as u32);
    let brec = sdat
        .bank_record(bank_slot)
        .ok_or(Error::Bad("no such bank"))?;
    let bank_file = sdat
        .file(brec.file_id)
        .ok_or(Error::Bad("bank file is not in the FAT"))?;
    let sbnk = Sbnk::parse(bank_file)?;
    let mut archives: [Option<Swar<'a>>; 4] = [None, None, None, None];
    for (slot, &arc) in archives.iter_mut().zip(brec.wave_arcs.iter()) {
        if arc == 0xFFFF {
            continue;
        }
        *slot = sdat
            .wave_archive_file(arc as u32)
            .and_then(|f| Swar::parse(f).ok());
    }

    let mut notes = Vec::new();
    let mut tempos = Vec::new();
    let mut looped = false;
    let mut stopped_early = 0;
    for (track, start) in sseq.track_starts() {
        let run = sseq::run_track(sseq.payload(), start as usize, track);
        looped |= run.looped;
        stopped_early += run.unknown as usize;
        notes.extend(run.notes);
        tempos.extend(run.tempos);
    }
    notes.sort_by_key(|n| n.tick);
    Ok(Prepared {
        notes,
        tempos: TempoMap::new(&tempos),
        bank: BankCtx {
            sbnk,
            archives,
            cache: HashMap::new(),
        },
        seq_gain: if rec.volume == 0 {
            1.0
        } else {
            square(rec.volume as f64 / 127.0)
        },
        looped,
        stopped_early,
    })
}

/// Renders sequence number `seq` of `sdat`.
pub fn render(sdat: &Sdat, seq: u32, opts: &Options) -> Result<Clip, Error> {
    let mut p = prepare(sdat, seq, opts)?;
    let rate = OUTPUT_RATE as f64;
    let limit = (opts.max_seconds.max(0.0) * rate) as usize;
    let mut mix = Mix {
        left: Vec::new(),
        right: Vec::new(),
        limit,
        used: 0,
    };
    let (mut rendered, mut dropped) = (0usize, 0usize);
    let mut truncated = false;

    for note in &p.notes {
        let start_time = p.tempos.seconds(note.tick);
        let off_time = p.tempos.seconds(note.tick.saturating_add(note.length));
        let start = (start_time * rate).round() as usize;
        if start >= limit {
            truncated = true;
            continue;
        }
        let Some(region) = p.bank.sbnk.resolve(note.program, note.key) else {
            dropped += 1;
            continue;
        };
        let semis = (note.key as f64 - region.base_note as f64) + note.bend;
        let freq = 440.0 * 2f64.powf((note.key as f64 + note.bend - 69.0) / 12.0);
        let mut generator = match region.source {
            Source::Wave { archive, wave } => match p.bank.wave(archive, wave) {
                Some(w) => Generator::Wave {
                    wave: w,
                    step: 2f64.powf(semis / 12.0) * w.rate / rate,
                    pos: 0.0,
                },
                None => {
                    dropped += 1;
                    continue;
                }
            },
            Source::Square { duty } => Generator::Square {
                duty: (duty + 1) as f64 / 8.0,
                step: freq / rate,
                phase: 0.0,
            },
            Source::Noise => Generator::Noise {
                step: freq / rate,
                clock: 0.0,
                lfsr: 0x7FFF,
                level: 1.0,
            },
        };
        let gain = p.seq_gain
            * square(note.velocity as f64 / 127.0)
            * square(note.volume as f64 / 127.0)
            * square(note.expression as f64 / 127.0);
        let (gl, gr) = pan_gains(note.pan, region.pan);
        let off_frame = ((off_time - start_time) * rate).round().max(0.0) as usize;
        let max_frames = (MAX_VOICE_SECONDS * rate) as usize;

        let mut env = Envelope::new(&region, note);
        let mut steps = 1.0; // take the first envelope step before sound
        let mut amp = 0.0;
        let mut frames = 0usize;
        for i in 0..max_frames {
            if start + i >= limit {
                truncated = true;
                break;
            }
            if i == off_frame {
                env.stage = Stage::Release;
            }
            steps += ENV_RATE / rate;
            while steps >= 1.0 {
                steps -= 1.0;
                env.step();
                amp = env.amplitude();
            }
            if env.done() || generator.exhausted() {
                break;
            }
            let s = generator.next() * amp * gain;
            mix.add(start + i, s * gl, s * gr);
            frames = i + 1;
        }
        if frames > 0 {
            rendered += 1;
        } else {
            dropped += 1;
        }
    }

    let frames = mix.used;
    let peak = mix.left[..frames]
        .iter()
        .chain(mix.right[..frames].iter())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    let scale = if peak > PEAK_LIMIT {
        PEAK_LIMIT / peak
    } else {
        1.0
    };
    let mut samples = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        for v in [mix.left[i], mix.right[i]] {
            samples.push((v * scale).round().clamp(-32768.0, 32767.0) as i16);
        }
    }
    // A clip cut short ends in a short fade so it does not click.
    if truncated {
        let fade = (OUTPUT_RATE as usize / 20).min(frames);
        for i in 0..fade {
            let m = 1.0 - i as f64 / fade as f64;
            let at = (frames - fade + i) * 2;
            samples[at] = (samples[at] as f64 * m) as i16;
            samples[at + 1] = (samples[at + 1] as f64 * m) as i16;
        }
    }
    Ok(Clip {
        samples,
        rate: OUTPUT_RATE,
        looped: p.looped,
        truncated,
        notes_rendered: rendered,
        notes_dropped: dropped,
        tracks_stopped_early: p.stopped_early,
    })
}

#[cfg(test)]
mod tests {
    use super::super::container::testing::{build, Entries};
    use super::super::container::{BankRecord, SeqRecord};
    use super::super::{sbnk, sseq as sq, swav};
    use super::*;

    /// A one-sequence SDAT whose bank has a looped PCM8 square-ish wave in
    /// program 0 and a PSG square in program 1.
    fn sdat_with(payload: &[u8], seq_volume: u8) -> Sdat {
        let wave: Vec<u8> = (0..64).map(|i| if i < 32 { 100 } else { 156 }).collect();
        let swar = swav::testing::archive(&[swav::testing::record(0, true, 16000, 0, &wave)]);
        let adsr = [127, 127, 127, 127];
        let bank = sbnk::testing::bank(&[
            (1, sbnk::testing::region(0, 0, 60, adsr, 64)),
            (2, sbnk::testing::region(3, 0, 60, adsr, 64)),
        ]);
        let seq = SeqRecord {
            file_id: 2,
            bank: 0,
            volume: seq_volume,
            channel_pressure: 0,
            player_priority: 0,
            player: 0,
        };
        let brec = BankRecord {
            file_id: 1,
            wave_arcs: [0, 0xFFFF, 0xFFFF, 0xFFFF],
        };
        Sdat::parse(build(&Entries {
            seqs: &[("SEQ_TEST", seq)],
            banks: &[("BANK_TEST", brec)],
            wave_archives: &[("WAVE_TEST", 0)],
            files: &[swar, bank, sq::testing::file(payload)],
        }))
        .unwrap()
    }

    fn sdat_with_default(payload: &[u8]) -> Sdat {
        sdat_with(payload, 127)
    }

    #[test]
    fn plays_a_note_and_ends() {
        // Header (alloc), then one note of a quarter at 120 BPM = 0.5 s.
        let s = sdat_with_default(&[0xFE, 1, 0, 60, 127, 48, 0xFF]);
        let clip = render(&s, 0, &Options::default()).unwrap();
        assert_eq!(clip.rate, OUTPUT_RATE);
        assert_eq!(clip.notes_rendered, 1);
        assert!(clip.peak() > 1000);
        // 0.5 s of gate plus a few ms of instant release.
        assert!(
            clip.seconds() > 0.49 && clip.seconds() < 0.56,
            "{}",
            clip.seconds()
        );
        assert!(!clip.truncated && !clip.looped);
    }

    #[test]
    fn tempo_changes_the_length() {
        let slow = sdat_with_default(&[0xFE, 1, 0, 0xE1, 60, 0, 60, 127, 48, 0xFF]);
        let clip = render(&slow, 0, &Options::default()).unwrap();
        assert!(
            clip.seconds() > 0.99 && clip.seconds() < 1.06,
            "{}",
            clip.seconds()
        );
    }

    #[test]
    fn pan_and_psg() {
        // Full left pan, program 1 (PSG square).
        let s = sdat_with_default(&[0xFE, 1, 0, 0xC0, 0, 0x81, 1, 69, 127, 24, 0xFF]);
        let clip = render(&s, 0, &Options::default()).unwrap();
        let (mut l, mut r) = (0i64, 0i64);
        for f in clip.samples.chunks(2) {
            l += (f[0] as i64).abs();
            r += (f[1] as i64).abs();
        }
        assert!(l > 0);
        assert!(r < l / 50, "left {l} right {r}");
    }

    #[test]
    fn looping_sequences_are_cut_at_the_limit() {
        // Track body jumps back to itself after a note.
        let s = sdat_with_default(&[0xFE, 1, 0, 60, 127, 24, 0x94, 3, 0, 0]);
        let clip = render(&s, 0, &Options::default()).unwrap();
        assert!(clip.looped);
        let opts = Options {
            max_seconds: 0.2,
            ..Options::default()
        };
        let cut = render(&s, 0, &opts).unwrap();
        assert!(cut.truncated);
        assert!(cut.seconds() <= 0.2 + 1e-6);
    }

    #[test]
    fn dropped_notes_and_errors() {
        // Program 9 does not exist.
        let s = sdat_with_default(&[0xFE, 1, 0, 0x81, 9, 60, 127, 24, 0xFF]);
        let clip = render(&s, 0, &Options::default()).unwrap();
        assert_eq!((clip.notes_rendered, clip.notes_dropped), (0, 1));
        assert!(clip.samples.is_empty());
        assert!(render(&s, 5, &Options::default()).is_err());
    }

    #[test]
    fn seq_volume_scales_the_output() {
        let payload = [0xFE, 1, 0, 60, 127, 48, 0xFF];
        let loud = render(&sdat_with(&payload, 127), 0, &Options::default()).unwrap();
        let soft = render(&sdat_with(&payload, 64), 0, &Options::default()).unwrap();
        assert!(soft.peak() * 3 < loud.peak() * 2);
    }

    #[test]
    fn envelope_tables() {
        assert_eq!(attack_rate(127), 0);
        assert_eq!(attack_rate(109), 0x8F);
        assert_eq!(attack_rate(0), 255);
        assert_eq!(fall(127), 0xFFFF);
        assert_eq!(fall(10), 21);
        assert_eq!(fall(100), 0x1E00 / 26);
        assert_eq!(sustain_level(127), 0);
        assert_eq!(sustain_level(0), SILENT as i64);
        assert!(sustain_level(64) < 0 && sustain_level(64) > sustain_level(10));
    }

    #[test]
    fn tempo_map_segments() {
        let m = TempoMap::new(&[(96, 240), (0, 120)]);
        assert!((m.seconds(48) - 0.5).abs() < 1e-9);
        assert!((m.seconds(96) - 1.0).abs() < 1e-9);
        assert!((m.seconds(144) - 1.25).abs() < 1e-9);
    }
}
