//! Audio import: puts a WAV file in place of a cry or of the wave a sound
//! effect plays, and writes the sound archive into the project as a
//! whole-file override.
//!
//! - A cry is wave archive `n` for species `n`, with one PCM8 wave. The
//!   game loads the whole file into a 0x5C30 byte buffer and plays the
//!   bytes after the wave header as PCM8 (aletheia `pm_voice.c`), so an
//!   imported cry is always mono PCM8, not looped, and the file ends where
//!   the samples end.
//! - A sound effect is a sequence. Its notes pick waves through its bank.
//!   When they all pick one wave, that wave is replaced by IMA-ADPCM at the
//!   WAV's rate. The pitch of the notes does not change: the wave keeps
//!   its place in the bank, only its samples differ.
//!
//! Only the replaced file and the offsets after it change in the archive
//! (`Sdat::replace_file`). Music (a sequence with many waves and a bank of
//! instruments) is not imported.

use std::collections::BTreeSet;
use std::fs;

use serde::Serialize;

use crate::bridge::{Bridge, Error};
use crate::catalog::SOUND_ARCHIVE;
use crate::sdat::sseq;
use crate::sdat::{Kind, Sbnk, Sdat, Source, Sseq, Swar};

/// The clock the channel timer divides: half the ARM7 bus clock
/// (`sdat::swav`).
const TIMER_CLOCK: f64 = 33_513_982.0 / 2.0;
/// The most a cry file may hold (the game's voice buffer).
const CRY_FILE_LIMIT: usize = 0x5C30;
/// Highest rate kept; faster WAVs are brought down to it.
const MAX_RATE: u32 = 32_000;
/// Lowest rate a cry is brought down to so that it fits.
const MIN_CRY_RATE: u32 = 8_000;
/// Where the wave record starts in a one-wave archive, and its header size.
const CRY_DATA_AT: usize = 0x40 + 12;

/// What an import did.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSummary {
    /// Length of the stored sound.
    pub seconds: f64,
    /// Stored sample rate, in Hz.
    pub rate: u32,
    /// `PCM8` or `IMA-ADPCM`.
    pub format: &'static str,
    /// Size of the stored wave data.
    pub bytes: usize,
    /// What the WAV was: rate, channels and bit depth.
    pub source: String,
    /// Anything the user should know (brought to a lower rate, shared).
    pub note: Option<String>,
}

/// Cries and sounds whose waves differ from the game's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AudioChanges {
    pub cries: Vec<u32>,
    pub sounds: Vec<u32>,
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Format(msg.into())
}

fn fmt_err(e: crate::nitro::Error) -> Error {
    Error::Format(e.to_string())
}

// ---- WAV reading -------------------------------------------------------

/// A decoded WAV file, mixed down to one channel.
#[derive(Debug, Clone, PartialEq)]
pub struct Wav {
    pub rate: u32,
    pub channels: u16,
    pub bits: u16,
    /// Samples in -1.0..1.0, the channels averaged.
    pub mono: Vec<f32>,
}

impl Wav {
    /// "44100 Hz, 16-bit stereo".
    pub fn describe(&self) -> String {
        let ch = match self.channels {
            1 => "mono".to_string(),
            2 => "stereo".to_string(),
            n => format!("{n} channels"),
        };
        format!("{} Hz, {}-bit {ch}", self.rate, self.bits)
    }

    pub fn seconds(&self) -> f64 {
        self.mono.len() as f64 / self.rate as f64
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Reads a WAV file: PCM of 8, 16, 24 or 32 bits or float of 32 or 64
/// bits, any rate, any number of channels.
pub fn read_wav(bytes: &[u8]) -> Result<Wav, Error> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad("not a WAV file"));
    }
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4).unwrap_or(0) as usize;
        let body = at + 8;
        // A size past the end (a streamed file) means "to the end".
        let end = body.saturating_add(size).min(bytes.len());
        if id == b"fmt " {
            let b = &bytes[body..end];
            let mut tag = u16_at(b, 0).ok_or_else(|| bad("the WAV header is cut short"))?;
            let channels = u16_at(b, 2).ok_or_else(|| bad("the WAV header is cut short"))?;
            let rate = u32_at(b, 4).ok_or_else(|| bad("the WAV header is cut short"))?;
            let bits = u16_at(b, 14).ok_or_else(|| bad("the WAV header is cut short"))?;
            if tag == 0xFFFE {
                // Extensible: the real format is the start of the GUID.
                tag = u16_at(b, 24).ok_or_else(|| bad("the WAV header is cut short"))?;
            }
            fmt = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            data = Some(&bytes[body..end]);
        }
        at = body.saturating_add(size).saturating_add(size & 1);
    }
    let (tag, channels, rate, bits) = fmt.ok_or_else(|| bad("the WAV file has no format block"))?;
    let data = data.ok_or_else(|| bad("the WAV file has no sound data"))?;
    if channels == 0 || rate == 0 {
        return Err(bad("the WAV header is not valid"));
    }
    let step = (bits as usize).div_ceil(8);
    let sample = |b: &[u8]| -> Result<f32, Error> {
        Ok(match (tag, bits) {
            (1, 8) => (b[0] as f32 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
            (1, 24) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            (1, 32) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            (3, 64) => f64::from_le_bytes(b[..8].try_into().unwrap()) as f32,
            _ => {
                return Err(bad(format!(
                    "this WAV is {}-bit {}; use PCM or float",
                    bits,
                    if tag == 1 || tag == 3 {
                        "PCM"
                    } else {
                        "compressed"
                    }
                )))
            }
        })
    };
    let frame = step * channels as usize;
    let mut mono = Vec::with_capacity(data.len() / frame);
    for f in data.chunks_exact(frame) {
        let mut sum = 0.0;
        for c in 0..channels as usize {
            sum += sample(&f[c * step..(c + 1) * step])?;
        }
        mono.push((sum / channels as f32).clamp(-1.0, 1.0));
    }
    if mono.is_empty() {
        return Err(bad("the WAV file holds no samples"));
    }
    Ok(Wav {
        rate,
        channels,
        bits,
        mono,
    })
}

// ---- resampling and encoding -------------------------------------------

/// `x` at rate `from` as a signal at rate `to`. Going down averages the
/// samples each output covers, going up interpolates.
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return x.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = ((x.len() as f64 / ratio).round() as usize).max(1);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if ratio > 1.0 {
            let (a, b) = (
                i as f64 * ratio,
                ((i + 1) as f64 * ratio).min(x.len() as f64),
            );
            let (ia, ib) = (a.floor() as usize, (b.ceil() as usize).min(x.len()));
            let (mut sum, mut weight) = (0.0f64, 0.0f64);
            for (k, &v) in x.iter().enumerate().take(ib).skip(ia) {
                let w = (b.min((k + 1) as f64) - a.max(k as f64)).max(0.0);
                sum += v as f64 * w;
                weight += w;
            }
            out.push(if weight > 0.0 {
                (sum / weight) as f32
            } else {
                0.0
            });
        } else {
            let p = i as f64 * ratio;
            let k = p.floor() as usize;
            let t = (p - k as f64) as f32;
            let a = x[k.min(x.len() - 1)];
            let b = x[(k + 1).min(x.len() - 1)];
            out.push(a + (b - a) * t);
        }
    }
    out
}

fn to_i16(x: &[f32]) -> Vec<i16> {
    x.iter()
        .map(|&v| (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect()
}

/// The channel timer value and the rate it plays at (the game's own
/// waves store the rate rounded down).
fn timer_for(rate: u32) -> (u16, u16) {
    let timer = (TIMER_CLOCK / rate as f64).round().clamp(1.0, 65535.0);
    let stored = (TIMER_CLOCK / timer).floor().min(65535.0);
    (timer as u16, stored as u16)
}

const IMA_STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const IMA_INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// One decoder step, exactly the one in `sdat::swav` and the game's.
fn ima_step(predictor: i32, index: i32, nibble: u8) -> (i32, i32) {
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
    let p = if nibble & 8 != 0 {
        predictor - diff
    } else {
        predictor + diff
    };
    (
        p.clamp(-32768, 32767),
        (index + IMA_INDEX[nibble as usize]).clamp(0, 88),
    )
}

/// IMA-ADPCM sample data as the DS reads it: a 4-byte seed (predictor,
/// step index), then two nibbles a byte, low nibble first. Every nibble
/// is the one whose decoded value is closest to its sample. The data is
/// padded with the last sample to a whole number of 4-byte words.
pub fn encode_adpcm(samples: &[i16]) -> Vec<u8> {
    let mut samples = samples.to_vec();
    if samples.is_empty() {
        samples.push(0);
    }
    let last = *samples.last().unwrap();
    while !samples.len().is_multiple_of(8) {
        samples.push(last);
    }
    let mut predictor = samples[0] as i32;
    let mut index = 0i32;
    let mut out = Vec::with_capacity(4 + samples.len() / 2);
    out.extend_from_slice(&(predictor as i16).to_le_bytes());
    out.extend_from_slice(&[index as u8, 0]);
    for pair in samples.as_chunks::<2>().0 {
        let mut byte = 0u8;
        for (k, &target) in pair.iter().enumerate() {
            let (mut best, mut best_err) = (0u8, i32::MAX);
            for nibble in 0..16u8 {
                let (p, _) = ima_step(predictor, index, nibble);
                let err = (p - target as i32).abs();
                if err < best_err {
                    (best, best_err) = (nibble, err);
                }
            }
            (predictor, index) = ima_step(predictor, index, best);
            byte |= best << (4 * k);
        }
        out.push(byte);
    }
    out
}

/// A SWAV record: header and sample data.
fn wave_record(kind: u8, rate: u32, data: &[u8]) -> Vec<u8> {
    assert_eq!(data.len() % 4, 0);
    let (timer, stored) = timer_for(rate);
    let mut v = vec![kind, 0];
    v.extend_from_slice(&stored.to_le_bytes());
    v.extend_from_slice(&timer.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&((data.len() / 4) as u32).to_le_bytes());
    v.extend_from_slice(data);
    v
}

// ---- wave archives -----------------------------------------------------

fn rd32(b: &[u8], at: usize) -> Result<usize, Error> {
    u32_at(b, at)
        .map(|v| v as usize)
        .ok_or_else(|| bad("the wave archive is cut short"))
}

/// Where each wave's record sits in a SWAR: (index, start, end), by start.
/// A wave ends where the next starts; the last ends where its own
/// header says its data does.
fn swar_spans(bytes: &[u8]) -> Result<Vec<(usize, usize, usize)>, Error> {
    Swar::parse(bytes).map_err(fmt_err)?;
    let count = rd32(bytes, 0x38)?;
    let mut spans: Vec<(usize, usize, usize)> = (0..count)
        .map(|i| rd32(bytes, 0x3C + i * 4).map(|o| (i, o, o)))
        .collect::<Result<_, _>>()?;
    spans.sort_by_key(|s| s.1);
    for k in 0..spans.len() {
        let start = spans[k].1;
        let natural =
            start + 12 + (rd32(bytes, start + 6)? & 0xFFFF) * 4 + rd32(bytes, start + 8)? * 4;
        let end = match spans.get(k + 1) {
            Some(next) if next.1 < natural => return Err(bad("waves overlap")),
            Some(next) => next.1,
            None => natural,
        };
        if end > bytes.len() || end <= start {
            return Err(bad("a wave runs past the archive"));
        }
        spans[k].2 = end;
    }
    if spans.windows(2).any(|w| w[0].1 == w[1].1) {
        return Err(bad("two waves share their data"));
    }
    Ok(spans)
}

/// The bytes of wave `index`'s record in a SWAR.
fn wave_bytes(bytes: &[u8], index: usize) -> Result<&[u8], Error> {
    let spans = swar_spans(bytes)?;
    let s = spans
        .iter()
        .find(|s| s.0 == index)
        .ok_or_else(|| bad("no such wave"))?;
    Ok(&bytes[s.1..s.2])
}

/// The SWAR with wave `index` replaced by `record`. The other waves are
/// kept as they are, and the size fields follow.
fn swar_replace(old: &[u8], index: usize, record: &[u8]) -> Result<Vec<u8>, Error> {
    let spans = swar_spans(old)?;
    if !spans.iter().any(|s| s.0 == index) {
        return Err(bad("no such wave"));
    }
    let mut out = old[..spans[0].1].to_vec();
    for &(i, start, end) in &spans {
        let at = out.len();
        out[0x3C + i * 4..0x40 + i * 4].copy_from_slice(&(at as u32).to_le_bytes());
        if i == index {
            out.extend_from_slice(record);
        } else {
            out.extend_from_slice(&old[start..end]);
        }
    }
    let total = out.len() as u32;
    out[8..12].copy_from_slice(&total.to_le_bytes());
    out[0x14..0x18].copy_from_slice(&(total - 0x10).to_le_bytes());
    Ok(out)
}

// ---- cries -------------------------------------------------------------

/// A one-wave cry archive holding `wav`, built on the old archive's
/// header, with what the import did.
pub fn cry_archive(old: &[u8], wav: &Wav) -> Result<(Vec<u8>, AudioSummary), Error> {
    if Swar::parse(old).map_err(fmt_err)?.len() != 1 {
        return Err(bad("a cry archive holds one wave"));
    }
    let cap = CRY_FILE_LIMIT - CRY_DATA_AT;
    let mut rate = wav.rate.min(MAX_RATE);
    let mut note = (rate != wav.rate).then(|| format!("Brought down to {rate} Hz."));
    if (wav.mono.len() as f64 * rate as f64 / wav.rate as f64).round() as usize > cap {
        // Too long for the game's buffer: a lower rate keeps all of it.
        let fit = (cap as f64 / wav.seconds()).floor() as u32;
        if fit < MIN_CRY_RATE {
            return Err(bad(format!(
                "this sound is {:.1} s long; a cry can be {:.1} s",
                wav.seconds(),
                cap as f64 / MIN_CRY_RATE as f64
            )));
        }
        rate = fit;
        note = Some(format!(
            "Brought down to {rate} Hz to fit the game's cry buffer."
        ));
    }
    let samples = resample(&wav.mono, wav.rate, rate);
    let mut data: Vec<u8> = samples
        .iter()
        .take(cap)
        .map(|&v| (v * 128.0).round().clamp(-128.0, 127.0) as i8 as u8)
        .collect();
    while !data.len().is_multiple_of(4) {
        data.push(0);
    }
    let record = wave_record(0, rate, &data);
    let (_, stored) = timer_for(rate);
    let archive = swar_replace(old, 0, &record)?;
    let summary = AudioSummary {
        seconds: data.len() as f64 / stored as f64,
        rate: stored as u32,
        format: "PCM8",
        bytes: data.len(),
        source: wav.describe(),
        note,
    };
    Ok((archive, summary))
}

// ---- sounds ------------------------------------------------------------

/// The waves a sequence plays, each as (SWAR file id, index in it).
pub fn waves_of(sdat: &Sdat, seq: u32) -> Result<BTreeSet<(u32, u16)>, Error> {
    let rec = sdat
        .seq_record(seq)
        .ok_or_else(|| Error::Missing(format!("sound {seq}")))?;
    let file = sdat
        .file(rec.file_id)
        .ok_or_else(|| bad("the sequence file is missing"))?;
    let sseq = Sseq::parse(file).map_err(fmt_err)?;
    let brec = sdat
        .bank_record(rec.bank as u32)
        .ok_or_else(|| bad("the sound has no bank"))?;
    let bank_file = sdat
        .file(brec.file_id)
        .ok_or_else(|| bad("the bank file is missing"))?;
    let bank = Sbnk::parse(bank_file).map_err(fmt_err)?;
    let mut waves = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for (track, start) in sseq.track_starts() {
        for n in sseq::run_track(sseq.payload(), start as usize, track).notes {
            if !seen.insert((n.program, n.key)) {
                continue;
            }
            let Some(region) = bank.resolve(n.program, n.key) else {
                continue;
            };
            if let Source::Wave { archive, wave } = region.source {
                let global = brec
                    .wave_arcs
                    .get(archive as usize)
                    .copied()
                    .unwrap_or(0xFFFF);
                if let Some(file_id) = sdat.wave_archive_file_id(global as u32) {
                    waves.insert((file_id, wave));
                }
            }
        }
    }
    Ok(waves)
}

/// The SWAR with a wave of `wav`, and what it holds.
fn sound_archive(old: &[u8], index: usize, wav: &Wav) -> Result<(Vec<u8>, AudioSummary), Error> {
    let rate = wav.rate.min(MAX_RATE);
    let samples = to_i16(&resample(&wav.mono, wav.rate, rate));
    let data = encode_adpcm(&samples);
    let record = wave_record(2, rate, &data);
    let (_, stored) = timer_for(rate);
    let archive = swar_replace(old, index, &record)?;
    let summary = AudioSummary {
        seconds: samples.len() as f64 / stored as f64,
        rate: stored as u32,
        format: "IMA-ADPCM",
        bytes: data.len(),
        source: wav.describe(),
        note: (rate != wav.rate).then(|| format!("Brought down to {rate} Hz.")),
    };
    Ok((archive, summary))
}

// ---- the project -------------------------------------------------------

/// Writes the sound archive into the project, or drops the override when
/// it is the game's own.
fn store(bridge: &Bridge, bytes: Vec<u8>) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open: changes need a mod to go in".into()))?;
    let game = bridge
        .game()
        .sdat()
        .ok_or_else(|| Error::Missing("the sound archive".into()))?;
    let io = |e: std::io::Error| Error::Missing(e.to_string());
    if game.bytes() == &bytes[..] {
        project.revert_file(SOUND_ARCHIVE).map_err(io)?;
    } else {
        // The temporary name starts with '~', which the port and the
        // change list both skip.
        let path = project.root.join(SOUND_ARCHIVE);
        let tmp = path.with_file_name(format!("~{SOUND_ARCHIVE}.kenon"));
        fs::write(&tmp, &bytes).map_err(io)?;
        fs::rename(&tmp, &path).map_err(io)?;
    }
    bridge.invalidate();
    Ok(())
}

fn need_sdat(bridge: &Bridge) -> Result<std::sync::Arc<Sdat>, Error> {
    bridge
        .sdat()
        .ok_or_else(|| Error::Missing("the sound archive".into()))
}

/// Replaces the cry of species `species` with the WAV in `wav`.
pub fn import_cry(bridge: &Bridge, species: u32, wav: &[u8]) -> Result<AudioSummary, Error> {
    let wav = read_wav(wav)?;
    let sdat = need_sdat(bridge)?;
    let file_id = sdat
        .wave_archive_file_id(species)
        .ok_or_else(|| Error::Missing(format!("cry {species}")))?;
    let old = sdat
        .file(file_id)
        .ok_or_else(|| bad("the cry file is missing"))?;
    let (archive, summary) = cry_archive(old, &wav)?;
    store(
        bridge,
        sdat.replace_file(file_id, &archive).map_err(fmt_err)?,
    )?;
    Ok(summary)
}

/// The one wave a sound plays, as (file id, index), or why there is not
/// exactly one.
fn single_wave(sdat: &Sdat, seq: u32) -> Result<(u32, u16), Error> {
    let waves = waves_of(sdat, seq)?;
    match waves.len() {
        1 => Ok(*waves.iter().next().unwrap()),
        0 => Err(bad("this sound plays no wave")),
        n => Err(bad(format!(
            "this sound plays {n} waves; only a sound with one wave can be replaced"
        ))),
    }
}

/// Replaces the wave a sound effect plays with the WAV in `wav`.
pub fn import_sound(bridge: &Bridge, seq: u32, wav: &[u8]) -> Result<AudioSummary, Error> {
    let wav = read_wav(wav)?;
    let sdat = need_sdat(bridge)?;
    let (file_id, wave) = single_wave(&sdat, seq)?;
    let old = sdat
        .file(file_id)
        .ok_or_else(|| bad("the wave file is missing"))?;
    let (archive, mut summary) = sound_archive(old, wave as usize, &wav)?;
    let sharing = (0..sdat.count(Kind::Seq))
        .filter(|&s| s != seq && sdat.seq_record(s).is_some())
        .filter(|&s| waves_of(&sdat, s).is_ok_and(|w| w.contains(&(file_id, wave))))
        .count();
    if sharing > 0 {
        let also = format!(
            "{sharing} other sound{} play this wave too.",
            if sharing == 1 { "" } else { "s" }
        );
        summary.note = Some(match summary.note {
            Some(n) => format!("{n} {also}"),
            None => also,
        });
    }
    store(
        bridge,
        sdat.replace_file(file_id, &archive).map_err(fmt_err)?,
    )?;
    Ok(summary)
}

/// Puts the game's cry back.
pub fn revert_cry(bridge: &Bridge, species: u32) -> Result<(), Error> {
    let sdat = need_sdat(bridge)?;
    let game = bridge
        .game()
        .sdat()
        .ok_or_else(|| Error::Missing("the sound archive".into()))?;
    let file_id = sdat
        .wave_archive_file_id(species)
        .ok_or_else(|| Error::Missing(format!("cry {species}")))?;
    let original = game
        .file(file_id)
        .ok_or_else(|| bad("the cry file is missing"))?;
    store(
        bridge,
        sdat.replace_file(file_id, original).map_err(fmt_err)?,
    )
}

/// Puts the game's wave back in a sound.
pub fn revert_sound(bridge: &Bridge, seq: u32) -> Result<(), Error> {
    let sdat = need_sdat(bridge)?;
    let game = bridge
        .game()
        .sdat()
        .ok_or_else(|| Error::Missing("the sound archive".into()))?;
    let (file_id, wave) = single_wave(&sdat, seq)?;
    let original = wave_bytes(
        game.file(file_id)
            .ok_or_else(|| bad("the wave file is missing"))?,
        wave as usize,
    )?;
    let current = sdat
        .file(file_id)
        .ok_or_else(|| bad("the wave file is missing"))?;
    let archive = swar_replace(current, wave as usize, original)?;
    store(
        bridge,
        sdat.replace_file(file_id, &archive).map_err(fmt_err)?,
    )
}

/// The cries and sounds the project has changed.
pub fn changes(bridge: &Bridge) -> Result<AudioChanges, Error> {
    let mut out = AudioChanges::default();
    let Some(project) = bridge.project() else {
        return Ok(out);
    };
    if project.file(SOUND_ARCHIVE).is_none() {
        return Ok(out);
    }
    let sdat = need_sdat(bridge)?;
    let game = bridge
        .game()
        .sdat()
        .ok_or_else(|| Error::Missing("the sound archive".into()))?;
    // Waves whose records differ, found file by file.
    let mut waves: BTreeSet<(u32, u16)> = BTreeSet::new();
    let mut files = BTreeSet::new();
    for arc in 0..sdat.count(Kind::WaveArc) {
        let Some(id) = sdat.wave_archive_file_id(arc) else {
            continue;
        };
        let (Some(mine), Some(theirs)) = (sdat.file(id), game.file(id)) else {
            continue;
        };
        if mine == theirs {
            continue;
        }
        if files.insert(id) {
            let count = Swar::parse(mine).map(|s| s.len()).unwrap_or(0);
            for i in 0..count {
                if wave_bytes(mine, i).ok() != wave_bytes(theirs, i).ok() {
                    waves.insert((id, i as u16));
                }
            }
        }
        if (1..=649).contains(&arc) {
            out.cries.push(arc);
        }
    }
    if waves.is_empty() {
        return Ok(out);
    }
    for seq in 0..sdat.count(Kind::Seq) {
        if sdat.seq_record(seq).is_some()
            && waves_of(&sdat, seq).is_ok_and(|w| w.iter().any(|k| waves.contains(k)))
        {
            out.sounds.push(seq);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdat::container::testing::{build, Entries};
    use crate::sdat::swav::testing::{archive, record};

    fn sine(n: usize) -> Vec<i16> {
        (0..n)
            .map(|i| ((i as f64 * 0.05).sin() * 12000.0) as i16)
            .collect()
    }

    #[test]
    fn adpcm_decodes_close_to_the_input() {
        let input = sine(4000);
        let data = encode_adpcm(&input);
        assert_eq!(data.len() % 4, 0);
        let rec = wave_record(2, 16000, &data);
        let a = archive(&[rec]);
        let out = Swar::parse(&a).unwrap().wave(0).unwrap().decode();
        assert_eq!(out.len(), 4000);
        let err: f64 = input
            .iter()
            .zip(&out)
            .map(|(&a, &b)| (a as f64 - b as f64).powi(2))
            .sum::<f64>()
            / 4000.0;
        let signal: f64 = input.iter().map(|&a| (a as f64).powi(2)).sum::<f64>() / 4000.0;
        // Better than 25 dB.
        assert!(signal / err > 300.0, "snr {}", signal / err);
    }

    #[test]
    fn resample_keeps_length_and_level() {
        let x: Vec<f32> = (0..4410).map(|i| (i as f32 * 0.01).sin()).collect();
        let down = resample(&x, 44100, 11025);
        assert_eq!(down.len(), 1103);
        let up = resample(&x, 11025, 44100);
        assert_eq!(up.len(), 17640);
        assert!((down[100] - x[400]).abs() < 0.05);
    }

    #[test]
    fn replacing_a_wave_keeps_the_others() {
        let a = archive(&[
            record(0, false, 8000, 0, &[1, 2, 3, 4]),
            record(0, false, 8000, 0, &[5, 6, 7, 8, 9, 10, 11, 12]),
            record(0, false, 8000, 0, &[13, 14, 15, 16]),
        ]);
        let new = wave_record(0, 8000, &[9; 12]);
        let b = swar_replace(&a, 1, &new).unwrap();
        let s = Swar::parse(&b).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s.wave(0).unwrap().data, &[1, 2, 3, 4]);
        assert_eq!(s.wave(1).unwrap().data, &[9; 12]);
        assert_eq!(s.wave(2).unwrap().data, &[13, 14, 15, 16]);
        assert_eq!(
            u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize,
            b.len()
        );
        assert_eq!(
            u32::from_le_bytes(b[0x14..0x18].try_into().unwrap()) as usize,
            b.len() - 0x10
        );
        assert!(swar_replace(&a, 3, &new).is_err());
    }

    #[test]
    fn a_replaced_file_moves_only_the_later_ones() {
        let files: Vec<Vec<u8>> = vec![vec![1; 10], vec![2; 40], vec![3; 7]];
        // Files of whole 32-byte slots, as the game's are.
        let padded: Vec<Vec<u8>> = files
            .iter()
            .map(|f| {
                let mut f = f.clone();
                f.resize(f.len().div_ceil(32) * 32, 0);
                f
            })
            .collect();
        let mut image = build(&Entries {
            seqs: &[],
            banks: &[],
            wave_archives: &[],
            files: &padded,
        });
        // The test builder leaves the size field in the header empty.
        let size = image.len() as u32;
        image[8..12].copy_from_slice(&size.to_le_bytes());
        let s = Sdat::parse(image).unwrap();
        let out = s.replace_file(1, &[9; 70]).unwrap();
        let t = Sdat::parse(out).unwrap();
        assert_eq!(t.file(0), s.file(0));
        assert_eq!(t.file(1), Some(&[9u8; 70][..]));
        assert_eq!(t.file(2), s.file(2));
        let back = t.replace_file(1, s.file(1).unwrap()).unwrap();
        assert_eq!(back, s.bytes());
    }
}
