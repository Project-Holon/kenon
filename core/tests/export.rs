//! What the right-click menus save, against the user's ROM. Set `KENON_ROM`
//! to run them; without it they pass without doing anything.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::rom::Rom;

fn bridge() -> Option<Bridge> {
    let path = std::env::var_os("KENON_ROM")?;
    Some(Bridge::new(Rom::open(path.as_ref()).unwrap()))
}

#[test]
fn assets_serve_as_files() {
    let Some(bridge) = bridge() else { return };
    // A picture is a PNG, a cry and a song are WAV.
    let png = bridge
        .serve("preview/4:20/4:38?cols=16&paloffset=0")
        .unwrap();
    assert_eq!(png.mime, "image/png");
    assert_eq!(&png.bytes[1..4], b"PNG");
    let cry = bridge.serve("cry/1").unwrap();
    assert_eq!(&cry.bytes[..4], b"RIFF");
    let song = bridge.serve("sound/1?seconds=5").unwrap();
    assert_eq!(&song.bytes[..4], b"RIFF");
    assert_eq!(&song.bytes[8..12], b"WAVE");
}

#[test]
fn raw_members_decompress_to_the_served_bytes() {
    let Some(bridge) = bridge() else { return };
    let mut compressed = 0;
    for index in 0..bridge.base_archive(4).unwrap().len().min(120) {
        let m = Member { arc: 4, index };
        let raw = bridge.member_raw(m).unwrap();
        let plain = bridge.member(m).unwrap();
        if raw != plain {
            compressed += 1;
            assert_eq!(kenon_core::lz::decompress(&raw).unwrap(), plain);
        }
    }
    assert!(compressed > 0, "expected some LZ members in a/0/0/4");
}
