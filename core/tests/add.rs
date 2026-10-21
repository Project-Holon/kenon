//! Adding characters and sounds against the user's ROM. Set `KENON_ROM` to
//! run them; without it they pass without doing anything.

use std::sync::Arc;

use kenon_core::bridge::{encode_png, Bridge, Member};
use kenon_core::catalog;
use kenon_core::characters::{add_character, added_characters, FIRST_NEW_CODE};
use kenon_core::g3d::{find_tex0, Tex0};
use kenon_core::project::Project;
use kenon_core::rom::Rom;
use kenon_core::sdat::Kind;
use kenon_core::sound_add::{add_music, add_sound_effect};
use kenon_core::zonemap::{object_sprites, object_textures};

fn bridge() -> Option<(Bridge, tempfile::TempDir)> {
    let path = std::env::var_os("KENON_ROM")?;
    let dir = tempfile::tempdir().unwrap();
    let project = Arc::new(Project::create(&dir.path().join("m"), "Test", None).unwrap());
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap()).with_project(Some(project));
    Some((bridge, dir))
}

fn frames_of(bridge: &Bridge, tex: u16) -> Vec<kenon_core::image::Rgba> {
    let bytes = bridge
        .member(Member {
            arc: 49,
            index: tex as usize,
        })
        .unwrap();
    let t = Tex0::parse(find_tex0(&bytes)[0]).unwrap();
    (0..t.textures.len())
        .map(|i| t.decode(i).unwrap())
        .collect()
}

#[test]
fn a_character_is_added_and_reads_back() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    // Character 3 has four 32x32 pictures.
    let like = 3u16;
    let sprites = object_sprites(&bridge).unwrap();
    let source = sprites[like as usize].unwrap();
    let originals = frames_of(&bridge, source.tex);
    let pngs: Vec<Vec<u8>> = originals.iter().map(encode_png).collect();

    let before = object_textures(&bridge).unwrap().len();
    let added = add_character(&bridge, &pngs, like).unwrap();
    assert!(added.code >= FIRST_NEW_CODE);
    assert_eq!(added.frames, originals.len());
    assert!(before <= added.code as usize + 1);

    // The listing sees it with the same number of frames.
    let sprites = object_sprites(&bridge).unwrap();
    let mine = sprites[added.code as usize].unwrap();
    assert_eq!(mine.tex, added.tex);
    assert_eq!(mine.frames as usize, originals.len());
    assert_eq!(added_characters(&bridge).unwrap(), vec![added.clone()]);

    // The pictures decode back to what was given.
    let back = frames_of(&bridge, added.tex);
    assert_eq!(back.len(), originals.len());
    for (a, b) in originals.iter().zip(&back) {
        assert_eq!((a.width, a.height), (b.width, b.height));
        for (p, q) in a
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(b.pixels.as_chunks::<4>().0)
        {
            assert_eq!(p[3] >= 128, q[3] >= 128);
            if p[3] >= 128 {
                assert_eq!(&p[..3], &q[..3]);
            }
        }
    }

    // The copied character is untouched, and the record is a copy.
    let table = bridge.member(Member { arc: 48, index: 0 }).unwrap();
    let n = u32::from_le_bytes(table[0..4].try_into().unwrap()) as usize;
    let rec = |i: usize| &table[4 + i * 28..4 + (i + 1) * 28];
    let like_rec = rec(like as usize);
    let new_rec = rec(n - 1);
    assert_eq!(&new_rec[2..0x10], &like_rec[2..0x10]);
    assert_eq!(&new_rec[0x12..], &like_rec[0x12..]);
    assert_eq!(frames_of(&bridge, source.tex).len(), originals.len());

    // A second one gets the next code and the next member.
    let second = add_character(&bridge, &pngs, like).unwrap();
    assert_eq!(second.code, added.code + 1);
    assert_eq!(second.tex, added.tex + 1);
}

#[test]
fn a_character_needs_its_pictures() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    let sprites = object_sprites(&bridge).unwrap();
    let source = sprites[3].unwrap();
    let pngs: Vec<Vec<u8>> = frames_of(&bridge, source.tex)
        .iter()
        .map(encode_png)
        .collect();
    assert!(add_character(&bridge, &pngs[1..], 3).is_err());
    let small = encode_png(&kenon_core::image::Rgba::new(16, 16));
    let wrong: Vec<Vec<u8>> = pngs.iter().map(|_| small.clone()).collect();
    assert!(add_character(&bridge, &wrong, 3).is_err());
    assert!(add_character(&bridge, &pngs, 0xFFF0).is_err());
    assert!(added_characters(&bridge).unwrap().is_empty());
}

/// A mono 16-bit WAV of a sine wave.
fn sine(seconds: f64, rate: u32) -> Vec<u8> {
    let n = (seconds * rate as f64) as usize;
    let mut data = Vec::new();
    for i in 0..n {
        let v = (i as f64 * 440.0 * std::f64::consts::TAU / rate as f64).sin() * 0.6;
        data.extend_from_slice(&((v * 32767.0) as i16).to_le_bytes());
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

/// Seconds in a WAV the bridge served.
fn wav_seconds(bytes: &[u8]) -> f64 {
    let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as f64;
    let channels = u16::from_le_bytes(bytes[22..24].try_into().unwrap()) as f64;
    let data = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as f64;
    data / 2.0 / channels / rate
}

#[test]
fn a_sound_effect_is_added_and_plays() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    let old = bridge.sdat().unwrap();
    let (seqs, banks, waves, files) = (
        old.count(Kind::Seq),
        old.count(Kind::Bank),
        old.count(Kind::WaveArc),
        old.file_count(),
    );
    let before = bridge.serve("sound/1379").unwrap().bytes.clone();

    let added = add_sound_effect(&bridge, "Test beep", &sine(0.75, 22050)).unwrap();
    assert_eq!(added.id, seqs);
    assert_eq!(added.name, "SEQ_SE_TEST_BEEP");

    let sdat = bridge.sdat().unwrap();
    assert_eq!(sdat.count(Kind::Seq), seqs + 1);
    assert_eq!(sdat.count(Kind::Bank), banks + 1);
    assert_eq!(sdat.count(Kind::WaveArc), waves + 1);
    assert_eq!(sdat.file_count(), files + 3);
    // Every old file is where it was, byte for byte, and still 32-aligned
    // as far as it was.
    for id in 0..files {
        assert_eq!(sdat.file(id), old.file(id), "file {id}");
    }
    for q in 0..seqs {
        assert_eq!(sdat.seq_record(q), old.seq_record(q), "sound {q}");
        assert_eq!(sdat.name(Kind::Seq, q), old.name(Kind::Seq, q));
    }
    assert_eq!(bridge.serve("sound/1379").unwrap().bytes, before);

    // It is listed as an effect and renders at the length given.
    let listed = catalog::sounds(&sdat);
    let me = listed.iter().find(|s| s.index == added.id).unwrap();
    assert_eq!((me.group, me.name.as_str()), ("effect", "SEQ_SE_TEST_BEEP"));
    let clip = bridge.serve(&format!("sound/{}", added.id)).unwrap();
    let seconds = wav_seconds(&clip.bytes);
    assert!((seconds - 0.75).abs() < 0.1, "{seconds}");
    assert!(clip.bytes[44..].iter().any(|&b| b != 0));
    // By name too.
    let by_name = bridge.serve("sound/SEQ_SE_TEST_BEEP").unwrap();
    assert_eq!(by_name.bytes, clip.bytes);

    // The same name twice, and a sound that cannot fit, are refused.
    assert!(add_sound_effect(&bridge, "test beep", &sine(0.5, 22050)).is_err());
    assert!(add_sound_effect(&bridge, "long", &sine(30.0, 22050)).is_err());

    // A second sound lands after the first.
    let second = add_sound_effect(&bridge, "Second", &sine(0.3, 44100)).unwrap();
    assert_eq!(second.id, seqs + 1);
    assert!(bridge.sdat().unwrap().file(files + 3).is_some());
    let again = bridge.serve(&format!("sound/{}", added.id)).unwrap();
    assert_eq!(again.bytes, clip.bytes);
}

#[test]
fn a_music_loop_is_added_and_plays() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    let seqs = bridge.sdat().unwrap().count(Kind::Seq);
    let added = add_music(&bridge, "Test loop", &sine(2.0, 22050)).unwrap();
    assert_eq!(added.id, seqs);
    let sdat = bridge.sdat().unwrap();
    let me = catalog::sounds(&sdat)
        .into_iter()
        .find(|s| s.index == added.id)
        .unwrap();
    assert_eq!(me.group, "music");
    assert_eq!(sdat.seq_record(added.id).unwrap().player, 0);
    let clip = bridge.serve(&format!("sound/{}", added.id)).unwrap();
    assert!(wav_seconds(&clip.bytes) >= 1.9);
}
