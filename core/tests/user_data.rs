//! Checks against the user's own data folder. Set `KENON_DATA_ROOT` to the
//! extraction folder (or its `data/` folder) to run them; without it they
//! pass without doing anything. No game data is in this repository.

use std::path::PathBuf;

use kenon_core::data_root::{self, DataRoot, DataRootCheck};
use kenon_core::{lz, narc};

fn data_root() -> Option<DataRoot> {
    let path = PathBuf::from(std::env::var_os("KENON_DATA_ROOT")?);
    match data_root::check(&path) {
        DataRootCheck::Ready(root) => Some(root),
        other => panic!("KENON_DATA_ROOT is set but not usable: {other:?}"),
    }
}

#[test]
fn every_archive_parses() {
    let Some(root) = data_root() else {
        eprintln!("KENON_DATA_ROOT not set; skipped");
        return;
    };
    let mut archives = 0;
    let mut members = 0;
    let mut compressed = 0;
    for id in 0..1000u16 {
        let rel = narc::archive_path(id).unwrap();
        let path = root.file(&rel);
        if !path.is_file() {
            continue;
        }
        archives += 1;
        // The one archive that is not a NARC: 282 raw 10-byte records
        // (aletheia docs/ASSETS.md, "a/0/1/3: not a NARC").
        if rel == "a/0/1/3" {
            assert!(narc::Narc::open(&path).is_err());
            continue;
        }
        let archive = narc::Narc::open(&path).unwrap_or_else(|err| panic!("{rel}: {err}"));
        members += archive.len();
        compressed += archive
            .iter_raw()
            .filter(|raw| lz::decompress(raw).is_ok())
            .count();
    }
    eprintln!("{archives} archives, {members} members, {compressed} LZ members");
    // The game's path table has 235 archives (inventory section 0).
    assert_eq!(archives, 235);
}

#[test]
fn font_archive_holds_nftr_members() {
    let Some(root) = data_root() else {
        return;
    };
    let fonts = narc::Narc::open(&root.file("a/0/2/3")).unwrap();
    let nftr = fonts
        .iter_raw()
        .filter(|raw| raw.starts_with(b"RTFN"))
        .count();
    assert!(nftr > 0, "no NFTR member in a/0/2/3");
}

#[test]
fn every_2d_resource_parses() {
    use kenon_core::{ncgr::Ncgr, nclr::Palette, nscr::Nscr};
    let Some(root) = data_root() else {
        return;
    };
    let (mut palettes, mut sheets, mut screens) = (0, 0, 0);
    let mut odd_screens = Vec::new();
    for id in 0..1000u16 {
        let rel = narc::archive_path(id).unwrap();
        let Ok(archive) = narc::Narc::open(&root.file(&rel)) else {
            continue;
        };
        for (index, raw) in archive.iter_raw().enumerate() {
            let member = lz::decompress_if_compressed(raw);
            match member.get(..4) {
                Some(b"RLCN") => {
                    Palette::parse(&member).unwrap_or_else(|e| panic!("{rel} #{index}: {e}"));
                    palettes += 1;
                }
                Some(b"RGCN") => {
                    Ncgr::parse(&member).unwrap_or_else(|e| panic!("{rel} #{index}: {e}"));
                    sheets += 1;
                }
                Some(b"RCSN") => {
                    let screen =
                        Nscr::parse(&member).unwrap_or_else(|e| panic!("{rel} #{index}: {e}"));
                    if screen.entries.len() != screen.tiles_wide() * screen.tiles_high() {
                        odd_screens.push(format!(
                            "{rel} #{index}: {}x{} px, {} entries",
                            screen.width,
                            screen.height,
                            screen.entries.len()
                        ));
                    }
                    screens += 1;
                }
                _ => {}
            }
        }
    }
    eprintln!("{palettes} NCLR, {sheets} NCGR, {screens} NSCR");
    eprintln!(
        "{} NSCR with an entry count other than width x height:",
        odd_screens.len()
    );
    for line in &odd_screens {
        eprintln!("  {line}");
    }
    assert!(palettes > 0 && sheets > 0 && screens > 0);
}

/// Renders the options screen background (a/0/2/7: #0 NCGR, #1 NCLR, #2
/// NSCR; inventory 7.6) as raw RGBA to the folder in KENON_RENDER_OUT, for a
/// look by eye.
#[test]
fn render_options_background() {
    use kenon_core::image::{render_screen, Zero};
    use kenon_core::{ncgr::Ncgr, nclr::Palette, nscr::Nscr};
    let (Some(root), Some(out)) = (data_root(), std::env::var_os("KENON_RENDER_OUT")) else {
        return;
    };
    let archive = narc::Narc::open(&root.file("a/0/2/7")).unwrap();
    let get = |i| lz::decompress_if_compressed(archive.raw(i).unwrap()).into_owned();
    let tiles = Ncgr::parse(&get(0)).unwrap();
    let palette = Palette::parse(&get(1)).unwrap();
    for (name, index) in [("options_2", 2), ("options_9", 9), ("options_14", 14)] {
        let screen = Nscr::parse(&get(index)).unwrap();
        let img = render_screen(&screen, &tiles, &palette, Zero::Opaque);
        let path = PathBuf::from(&out).join(format!("{name}_{}x{}.rgba", img.width, img.height));
        std::fs::write(path, &img.pixels).unwrap();
    }
}

/// The five fonts of a/0/2/3: the CWDH advance table agrees with the
/// advance byte of every glyph record, and ASCII maps to itself.
#[test]
fn fonts() {
    use kenon_core::nftr::Font;
    let Some(root) = data_root() else {
        return;
    };
    let archive = narc::Narc::open(&root.file("a/0/2/3")).unwrap();
    let expect = [
        (12, 15, 15, 5064),
        (10, 10, 10, 5064),
        (11, 13, 13, 5064),
        (13, 8, 8, 32),
        (12, 12, 12, 41),
    ];
    for (index, (cw, ch, line, count)) in expect.into_iter().enumerate() {
        let font = Font::parse(archive.raw(index).unwrap()).unwrap();
        assert_eq!(
            (
                font.cell_width,
                font.cell_height,
                font.line_height,
                font.glyph_count()
            ),
            (cw, ch, line, count),
            "font {index}"
        );
        for g in 0..font.glyph_count() as u16 {
            assert_eq!(
                font.advance_of(g),
                font.glyph(g).unwrap().advance,
                "font {index} glyph {g}"
            );
        }
    }
    let main = Font::parse(archive.raw(0).unwrap()).unwrap();
    assert_eq!(main.glyph_index(b' ' as u16), 0);
    assert_eq!(main.glyph_index(b'A' as u16), 33);
    assert_eq!(main.glyph_index(b'?' as u16), main.missing_glyph);
    // Hiragana and katakana are mapped too.
    assert_ne!(main.glyph_index(0x3042), 0);
    assert_ne!(main.glyph_index(0x30A2), 0);
}

/// Every bank in a/0/0/2 and a/0/0/3 parses, writes back byte for byte,
/// and every string survives the display form.
#[test]
fn message_banks() {
    use kenon_core::msg::{parse_display, Bank, Display};
    let Some(root) = data_root() else {
        return;
    };
    let (mut banks, mut strings, mut identical) = (0, 0, 0);
    let mut different = Vec::new();
    for rel in ["a/0/0/2", "a/0/0/3"] {
        let archive = narc::Narc::open(&root.file(rel)).unwrap();
        for (index, raw) in archive.iter_raw().enumerate() {
            let bank = Bank::parse(raw).unwrap_or_else(|e| panic!("{rel} #{index}: {e}"));
            banks += 1;
            for (i, message) in bank.messages().iter().enumerate() {
                strings += 1;
                let text = Display(&message.units).to_string();
                assert_eq!(
                    parse_display(&text).unwrap(),
                    message.units,
                    "{rel} #{index} string {i}: {text}"
                );
            }
            if bank.build() == raw {
                identical += 1;
            } else if different.len() < 5 {
                let built = bank.build();
                let at = built.iter().zip(raw.iter()).position(|(a, b)| a != b);
                different.push(format!(
                    "{rel} #{index}: sizes {} vs {}, first difference at {at:?} of {}",
                    built.len(),
                    raw.len(),
                    raw.len()
                ));
            }
        }
    }
    eprintln!("{banks} banks, {strings} strings, {identical} written back byte for byte");
    eprintln!("first banks that differ: {different:?}");

    // aletheia's check: the species bank (a/0/0/2 #70) string 1.
    let archive = narc::Narc::open(&root.file("a/0/0/2")).unwrap();
    let species = Bank::parse(archive.raw(70).unwrap()).unwrap();
    assert_eq!(
        Display(&species.messages()[1].units).to_string(),
        "Bulbasaur"
    );
    assert_eq!(identical, banks);
}
