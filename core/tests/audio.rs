//! Audio import against the user's ROM. Set `KENON_ROM` to run them;
//! without it they pass without doing anything. No game data is in this
//! repository.

use std::sync::Arc;

use kenon_core::audio_import::{self, read_wav};
use kenon_core::bridge::Bridge;
use kenon_core::catalog::SOUND_ARCHIVE;
use kenon_core::project::Project;
use kenon_core::rom::Rom;
use kenon_core::sdat::{Kind, Sdat};

/// A bridge over the ROM with an empty project in `dir`.
fn bridge(dir: &std::path::Path) -> Option<Bridge> {
    let rom = Rom::open(std::env::var_os("KENON_ROM")?.as_ref()).unwrap();
    let project = Arc::new(Project::create(&dir.join("mod"), "Audio", None).unwrap());
    Some(Bridge::new(rom).with_project(Some(project)))
}

fn wav(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
    let data = samples.len() * 2;
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + data as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2 * channels as u32).to_le_bytes());
    out.extend_from_slice(&(2 * channels).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data as u32).to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn sine(rate: u32, seconds: f64, hz: f64, amp: f64) -> Vec<i16> {
    (0..(rate as f64 * seconds) as usize)
        .map(|i| ((i as f64 / rate as f64 * hz * std::f64::consts::TAU).sin() * amp) as i16)
        .collect()
}

/// (rate, left channel) of a served stereo WAV.
fn decode_served(bytes: &[u8]) -> (u32, Vec<i16>) {
    let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    let left = bytes[44..44 + len]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|f| i16::from_le_bytes([f[0], f[1]]))
        .collect();
    (rate, left)
}

fn rms(x: &[i16]) -> f64 {
    (x.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

#[test]
fn a_cry_exported_and_imported_back_is_the_game_s() {
    let dir = tempfile::tempdir().unwrap();
    let Some(bridge) = bridge(dir.path()) else {
        return;
    };
    let exported = bridge.serve("cry/25").unwrap().bytes.clone();
    let (rate, samples) = decode_served(&exported);
    let summary = audio_import::import_cry(&bridge, 25, &exported).unwrap();
    assert_eq!(summary.format, "PCM8");
    assert!(summary.rate.abs_diff(rate) <= 1);
    // PCM8 holds the samples exactly, so nothing changed and no override
    // is kept.
    let again = decode_served(&bridge.serve("cry/25").unwrap().bytes).1;
    assert_eq!(again, samples);
    assert!(bridge.project().unwrap().file(SOUND_ARCHIVE).is_none());
}

#[test]
fn a_cry_replaced_by_a_sine_plays_and_the_rest_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let Some(bridge) = bridge(dir.path()) else {
        return;
    };
    // A stereo 44.1 kHz file, half a second.
    let mono = sine(44100, 0.5, 440.0, 20000.0);
    let stereo: Vec<i16> = mono.iter().flat_map(|&v| [v, v]).collect();
    let summary = audio_import::import_cry(&bridge, 25, &wav(&stereo, 44100, 2)).unwrap();
    assert_eq!(summary.format, "PCM8");
    assert_eq!(summary.source, "44100 Hz, 16-bit stereo");
    assert!((summary.seconds - 0.5).abs() < 0.01, "{summary:?}");

    let (rate, left) = decode_served(&bridge.serve("cry/25").unwrap().bytes);
    assert!(rate.abs_diff(summary.rate) <= 1);
    assert!((left.len() as f64 / rate as f64 - 0.5).abs() < 0.01);
    // A sine of amplitude 20000 has an RMS near 14142.
    assert!((rms(&left) - 14142.0).abs() < 600.0, "rms {}", rms(&left));

    let game = bridge.game().sdat().unwrap();
    let mine = bridge.sdat().unwrap();
    let id = game.wave_archive_file_id(25).unwrap();
    assert_ne!(game.file(id), mine.file(id));
    // The names, the records and every other file are the game's.
    assert_eq!(game.file_count(), mine.file_count());
    assert_eq!(game.bytes()[..8], mine.bytes()[..8]);
    assert_eq!(game.bytes()[0x0C..0x2C], mine.bytes()[0x0C..0x2C]);
    for f in 0..game.file_count() {
        if f != id {
            assert_eq!(game.file(f), mine.file(f), "file {f}");
        }
    }
    for kind in [Kind::Seq, Kind::Bank, Kind::WaveArc] {
        for i in 0..game.count(kind) {
            assert_eq!(game.name(kind, i), mine.name(kind, i));
        }
    }
    for i in 0..game.count(Kind::Seq) {
        assert_eq!(game.seq_record(i), mine.seq_record(i));
    }
    assert!(Sdat::parse(mine.bytes().to_vec()).is_ok());
    let changes = audio_import::changes(&bridge).unwrap();
    assert_eq!(changes.cries, vec![25]);

    // Back to the game's: the override goes away.
    audio_import::revert_cry(&bridge, 25).unwrap();
    assert!(bridge.project().unwrap().file(SOUND_ARCHIVE).is_none());
    assert_eq!(bridge.sdat().unwrap().bytes(), game.bytes());
}

#[test]
fn a_cry_too_long_is_brought_down_or_refused() {
    let dir = tempfile::tempdir().unwrap();
    let Some(bridge) = bridge(dir.path()) else {
        return;
    };
    // Two seconds at 32 kHz does not fit at that rate: it is brought down.
    let long = wav(&sine(32000, 2.0, 300.0, 10000.0), 32000, 1);
    let s = audio_import::import_cry(&bridge, 1, &long).unwrap();
    assert!(s.rate < 32000 && s.note.is_some(), "{s:?}");
    assert!(s.seconds > 1.9, "{s:?}");
    // Five seconds cannot be kept.
    let too_long = wav(&sine(16000, 5.0, 300.0, 10000.0), 16000, 1);
    let err = audio_import::import_cry(&bridge, 1, &too_long).unwrap_err();
    assert!(err.to_string().contains("long"), "{err}");
}

#[test]
fn a_sound_effect_wave_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let Some(bridge) = bridge(dir.path()) else {
        return;
    };
    let game = bridge.game().sdat().unwrap();
    // A sound effect that plays exactly one wave.
    let seq = (0..game.count(Kind::Seq))
        .filter(|&i| {
            game.name(Kind::Seq, i)
                .is_some_and(|n| n.starts_with("SEQ_SE"))
        })
        .find(|&i| audio_import::waves_of(&game, i).is_ok_and(|w| w.len() == 1))
        .expect("a one-wave sound effect");
    let path = format!("sound/{seq}?seconds=5");
    let before = bridge.serve(&path).unwrap();
    let input = wav(&sine(22050, 0.4, 880.0, 16000.0), 22050, 1);
    let summary = audio_import::import_sound(&bridge, seq, &input).unwrap();
    assert_eq!(summary.format, "IMA-ADPCM");
    assert!(summary.rate.abs_diff(22050) <= 2, "{summary:?}");
    assert!((summary.seconds - 0.4).abs() < 0.01, "{summary:?}");

    let after = bridge.serve(&path).unwrap();
    let (_, left) = decode_served(&after.bytes);
    assert!(rms(&left) > 500.0, "silent: {}", rms(&left));
    assert_ne!(before.bytes, after.bytes);
    assert!(audio_import::changes(&bridge)
        .unwrap()
        .sounds
        .contains(&seq));
    assert!(Sdat::parse(bridge.sdat().unwrap().bytes().to_vec()).is_ok());

    audio_import::revert_sound(&bridge, seq).unwrap();
    assert!(bridge.project().unwrap().file(SOUND_ARCHIVE).is_none());
    assert_eq!(bridge.serve(&path).unwrap().bytes, before.bytes);
}

#[test]
fn music_with_many_waves_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let Some(bridge) = bridge(dir.path()) else {
        return;
    };
    let game = bridge.game().sdat().unwrap();
    let song = (0..game.count(Kind::Seq))
        .find(|&i| {
            game.name(Kind::Seq, i)
                .is_some_and(|n| n.starts_with("SEQ_BGM"))
                && audio_import::waves_of(&game, i).is_ok_and(|w| w.len() > 1)
        })
        .expect("a song");
    let input = wav(&sine(22050, 0.4, 880.0, 16000.0), 22050, 1);
    let err = audio_import::import_sound(&bridge, song, &input).unwrap_err();
    assert!(err.to_string().contains("waves"), "{err}");
    assert!(bridge.project().unwrap().file(SOUND_ARCHIVE).is_none());
}

#[test]
fn wav_variants_read() {
    let mut f = wav(&[0, 16384, -16384, 32767], 8000, 1);
    let w = read_wav(&f).unwrap();
    assert_eq!((w.rate, w.channels, w.bits, w.mono.len()), (8000, 1, 16, 4));
    assert!((w.mono[1] - 0.5).abs() < 1e-4);
    // A chunk before the data, with an odd size and its pad byte.
    let mut with_list = f[..36].to_vec();
    with_list.extend_from_slice(b"LIST\x03\x00\x00\x00abc\x00");
    with_list.extend_from_slice(&f[36..]);
    assert_eq!(read_wav(&with_list).unwrap().mono.len(), 4);
    // Text that is not a WAV, and a compressed format.
    assert!(read_wav(b"not a wav file at all").is_err());
    f[20] = 2;
    assert!(read_wav(&f).unwrap_err().to_string().contains("compressed"));
}
