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
    let (mut palettes, mut sheets, mut screens, mut cells, mut anims) = (0, 0, 0, 0, 0);
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
                Some(b"RECN") => {
                    kenon_core::ncer::Ncer::parse(&member)
                        .unwrap_or_else(|e| panic!("{rel} #{index}: {e}"));
                    cells += 1;
                }
                Some(b"RNAN") => {
                    kenon_core::nanr::Nanr::parse(&member)
                        .unwrap_or_else(|e| panic!("{rel} #{index}: {e}"));
                    anims += 1;
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
    eprintln!("{palettes} NCLR, {sheets} NCGR, {screens} NSCR, {cells} NCER, {anims} NANR");
    eprintln!(
        "{} NSCR with an entry count other than width x height:",
        odd_screens.len()
    );
    for line in &odd_screens {
        eprintln!("  {line}");
    }
    assert!(palettes > 0 && sheets > 0 && screens > 0 && cells > 0 && anims > 0);
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

/// Renders every cell of the options screen sprites (a/0/2/7: palette #6;
/// main sprite NCGR #5 NCER #4, cursor NCGR #12 NCER #11; inventory 7.6) as
/// raw RGBA to KENON_RENDER_OUT.
#[test]
fn render_options_cells() {
    use kenon_core::image::render_cell;
    use kenon_core::{ncer::Ncer, ncgr::Ncgr, nclr::Palette};
    let (Some(root), Some(out)) = (data_root(), std::env::var_os("KENON_RENDER_OUT")) else {
        return;
    };
    let archive = narc::Narc::open(&root.file("a/0/2/7")).unwrap();
    let get = |i| lz::decompress_if_compressed(archive.raw(i).unwrap()).into_owned();
    let palette = Palette::parse(&get(6)).unwrap();
    for (name, char_index, cell_index) in [("main", 5, 4), ("cursor", 12, 11)] {
        let tiles = Ncgr::parse(&get(char_index)).unwrap();
        let bank = Ncer::parse(&get(cell_index)).unwrap();
        for (i, cell) in bank.cells.iter().enumerate() {
            let img = render_cell(&bank, cell, &tiles, &palette, 0).image;
            let path = PathBuf::from(&out)
                .join(format!("cell_{name}_{i}_{}x{}.rgba", img.width, img.height));
            std::fs::write(path, &img.pixels).unwrap();
        }
    }
}

/// The code images: the overlay table gives the load addresses the
/// inventory names, and the ARM9 archive path table (0x020A6C58, 235
/// pointers) spells a/X/Y/Z for every id, the rule narc::archive_path uses.
#[test]
fn code_images() {
    use kenon_core::code::{CodeImages, Image};
    let Some(root) = data_root() else {
        return;
    };
    let Some(dir) = root.extraction_dir.as_deref().filter(|_| root.has_arm9) else {
        return;
    };
    let code = CodeImages::open(dir).unwrap();
    assert_eq!(code.address(Image::Overlay(67)), Some(0x021B_6100));
    assert_eq!(code.address(Image::Overlay(21)), Some(0x0218_7F00));
    for id in 0..235u16 {
        let ptr = code
            .u32_at(Image::Arm9, 0x020A_6C58 + id as u32 * 4)
            .unwrap();
        let path = code.c_string(Image::Arm9, ptr, 32).unwrap();
        assert_eq!(Some(path), narc::archive_path(id), "archive {id}");
    }
    // Overlay 67 holds the touch bar button looks (inventory section 14).
    assert!(code.read(Image::Overlay(67), 0x021B_942C, 7 * 20).is_some());
}

/// The asset bridge on real data: a menu frame (a/0/0/5 #9 tiles, #3
/// palette; inventory section 1) and a line of text in the main font with
/// the text window palette (a/0/2/3 #0, #5). Written to KENON_RENDER_OUT.
#[test]
fn bridge_assets() {
    use kenon_core::bridge::Bridge;
    let Some(root) = data_root() else {
        return;
    };
    let bridge = Bridge::new(root);
    let paths = [
        ("frame", "tiles/5:9/5:3/0/3/0,1,2,3,4,5,6,7,8"),
        (
            "text",
            "text/23:0/23:5/0/1,2,0/Hello, kenon! \u{3042}\u{30A2}",
        ),
        ("options", "screen/27:0/27:1/27:2"),
        ("cell", "cell/27:5/27:6/27:4/3"),
    ];
    for (name, path) in paths {
        let asset = bridge.serve(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(asset.mime, "image/png");
        assert!(asset.bytes.starts_with(b"\x89PNG"));
        if let Some(out) = std::env::var_os("KENON_RENDER_OUT") {
            std::fs::write(
                PathBuf::from(out).join(format!("bridge_{name}.png")),
                &asset.bytes,
            )
            .unwrap();
        }
    }
    // Served twice, the second from the cache.
    let a = bridge.serve(paths[0].1).unwrap();
    let b = bridge.serve(paths[0].1).unwrap();
    assert!(std::sync::Arc::ptr_eq(&a, &b));
    assert_eq!(
        bridge.serve("cell/27:5/27:6/27:4/3").unwrap().origin,
        Some((8, 0))
    );
}

/// The sound archive: names, every sequence, bank and wave archive parses,
/// and the UI sound effects render to short, audible clips.
#[test]
fn sound_archive() {
    use kenon_core::sdat::{self, Kind, Sbnk, Sdat, Sseq, Swar};
    let Some(root) = data_root() else {
        return;
    };
    let path = root.file("wb_sound_data.sdat");
    if !path.is_file() {
        return;
    }
    let sdat = Sdat::open(&path).unwrap();
    assert_eq!(sdat.name(Kind::Seq, 1352), Some("SEQ_SE_SELECT1"));
    assert_eq!(sdat.name(Kind::Seq, 1356), Some("SEQ_SE_DECIDE1"));
    assert_eq!(sdat.name(Kind::Seq, 1361), Some("SEQ_SE_CANCEL1"));
    assert_eq!(sdat.find(Kind::Seq, "SEQ_SE_SELECT1"), Some(1352));

    let (mut seqs, mut banks, mut archives, mut waves) = (0, 0, 0, 0);
    for i in 0..sdat.count(Kind::Seq) {
        if sdat.seq_record(i).is_none() {
            continue;
        }
        let file = sdat.seq_file(i).unwrap_or_else(|| panic!("SEQ {i} file"));
        let sseq = Sseq::parse(file).unwrap_or_else(|e| panic!("SEQ {i}: {e}"));
        assert!(!sseq.track_starts().is_empty());
        seqs += 1;
    }
    for i in 0..sdat.count(Kind::Bank) {
        if sdat.bank_record(i).is_none() {
            continue;
        }
        let file = sdat.bank_file(i).unwrap_or_else(|| panic!("BANK {i} file"));
        Sbnk::parse(file).unwrap_or_else(|e| panic!("BANK {i}: {e}"));
        banks += 1;
    }
    for i in 0..sdat.count(Kind::WaveArc) {
        if sdat.wave_archive_file_id(i).is_none() {
            continue;
        }
        let file = sdat
            .wave_archive_file(i)
            .unwrap_or_else(|| panic!("WAVEARC {i} file"));
        let swar = Swar::parse(file).unwrap_or_else(|e| panic!("WAVEARC {i}: {e}"));
        for w in 0..swar.len() {
            let wave = swar
                .wave(w)
                .unwrap_or_else(|e| panic!("WAVEARC {i} wave {w}: {e}"));
            assert_eq!(wave.decode().len(), wave.decoded_len());
            waves += 1;
        }
        archives += 1;
    }
    eprintln!("{seqs} sequences, {banks} banks, {archives} wave archives, {waves} waves");
    assert!(seqs > 900 && banks > 900 && archives > 0 && waves > 0);

    for n in [1351, 1352, 1356, 1357, 1361, 1362, 1366, 1368] {
        let clip = sdat::render(&sdat, n, &sdat::Options::default()).unwrap();
        eprintln!(
            "SEQ {n} {}: {:.2} s, peak {}, {} notes, {} dropped",
            sdat.name(Kind::Seq, n).unwrap_or("?"),
            clip.seconds(),
            clip.peak(),
            clip.notes_rendered,
            clip.notes_dropped
        );
        assert!(clip.peak() > 500, "SEQ {n} is silent");
        assert!(clip.seconds() < 5.0, "SEQ {n} runs {} s", clip.seconds());
        assert!(!clip.truncated, "SEQ {n} was cut");
    }
}

/// Every sequence renders without a panic, and the numbers say how much of
/// the bytecode and the banks the renderer covers.
#[test]
fn sound_archive_renders_everything() {
    use kenon_core::sdat::{self, Kind, Sdat};
    let Some(root) = data_root() else {
        return;
    };
    let path = root.file("wb_sound_data.sdat");
    if !path.is_file() {
        return;
    }
    let sdat = Sdat::open(&path).unwrap();
    let opts = sdat::Options {
        max_seconds: 3.0,
        ..sdat::Options::default()
    };
    let (mut ok, mut failed, mut silent, mut stopped, mut dropped, mut notes) = (0, 0, 0, 0, 0, 0);
    for i in 0..sdat.count(Kind::Seq) {
        if sdat.seq_record(i).is_none() {
            continue;
        }
        match sdat::render(&sdat, i, &opts) {
            Ok(clip) => {
                ok += 1;
                silent += (clip.peak() == 0) as usize;
                stopped += clip.tracks_stopped_early;
                dropped += clip.notes_dropped;
                notes += clip.notes_rendered;
            }
            Err(_) => failed += 1,
        }
    }
    eprintln!(
        "{ok} rendered, {failed} failed, {silent} silent, {notes} notes sounded, \
         {dropped} dropped, {stopped} tracks stopped at an unknown command"
    );
    assert_eq!(failed, 0);
}

/// Reading straight from the ROM named by KENON_ROM: it is a known dump,
/// and when KENON_DATA_ROOT names an extraction of it, every NitroFS file,
/// the ARM9 binary and every overlay match the extracted copies.
#[test]
fn rom_matches_extraction() {
    use kenon_core::rom::{Game, Rom};
    let Some(path) = std::env::var_os("KENON_ROM") else {
        eprintln!("KENON_ROM not set; skipped");
        return;
    };
    let rom = Rom::open(path.as_ref()).unwrap();
    assert_ne!(rom.identity.game, Game::Other);
    assert!(rom.is_known_dump().unwrap(), "{:?}", rom.identity);
    let Some(root) = data_root() else {
        return;
    };
    let mut files = 0;
    for rel in rom.paths().map(str::to_string).collect::<Vec<_>>() {
        let extracted = std::fs::read(root.file(&rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert!(rom.file(&rel).unwrap() == extracted, "{rel}");
        files += 1;
    }
    eprintln!("{files} files match");
    let Some(dir) = root.extraction_dir.as_deref().filter(|_| root.has_arm9) else {
        return;
    };
    assert!(rom.arm9().unwrap() == std::fs::read(dir.join("arm9_dec.bin")).unwrap());
    for n in 0..rom.overlays().len() as u16 {
        let name = format!("y9_{n:04}.bin");
        let extracted = std::fs::read(dir.join("overlays_dec").join(&name))
            .or_else(|_| std::fs::read(dir.join("overlays").join(&name)))
            .unwrap();
        assert!(rom.overlay(n).unwrap() == extracted, "overlay {n}");
    }
}

/// The bridge straight from the ROM: animation frames, a message, the
/// cursor tile and a code table all need the ROM's files or code.
#[test]
fn bridge_from_rom() {
    use kenon_core::bridge::Bridge;
    use kenon_core::rom::Rom;
    let Some(path) = std::env::var_os("KENON_ROM") else {
        return;
    };
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap());
    // Touch bar back button: idle (sequence 1) and pressed (sequence 9).
    for seq in [1, 9] {
        let asset = bridge
            .serve(&format!("anim/83:20/83:19/83:21/83:24/{seq}/0"))
            .unwrap();
        assert!(asset.bytes.starts_with(b"\x89PNG"));
        if let Some(out) = std::env::var_os("KENON_RENDER_OUT") {
            std::fs::write(
                PathBuf::from(out).join(format!("back_{seq}.png")),
                &asset.bytes,
            )
            .unwrap();
        }
    }
    let info = String::from_utf8(bridge.serve("animinfo/83:24").unwrap().bytes.clone()).unwrap();
    assert!(info.starts_with("[{\"loopStart\""), "{info}");
    // a/0/0/2 bank 0xE9 (233): the yes/no labels.
    let yes = String::from_utf8(bridge.serve("msg/2/233/0").unwrap().bytes.clone()).unwrap();
    assert!(!yes.is_empty());
    eprintln!("bank 233 message 0: {yes}");
    assert!(bridge
        .serve("cursor/5:3/0")
        .unwrap()
        .bytes
        .starts_with(b"\x89PNG"));
    let types =
        String::from_utf8(bridge.serve("table/type_palette").unwrap().bytes.clone()).unwrap();
    assert_eq!(types.matches(',').count(), 17, "{types}");
}

/// The catalog and the edit path on the ROM: sound groups, and a text and
/// palette edit written to a project and read back through the bridge.
#[test]
fn catalog_and_edits() {
    use kenon_core::bridge::{Bridge, Member};
    use kenon_core::catalog;
    use kenon_core::project::Project;
    use kenon_core::rom::Rom;
    let Some(path) = std::env::var_os("KENON_ROM") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let project =
        std::sync::Arc::new(Project::create(&dir.path().join("m"), "Test", None).unwrap());
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap()).with_project(Some(project.clone()));

    let sounds = catalog::sounds(&bridge.sdat().unwrap());
    let mut groups = std::collections::BTreeMap::new();
    for s in &sounds {
        *groups.entry(s.group).or_insert(0) += 1;
    }
    eprintln!("sounds by group: {groups:?}");
    assert!(groups.get("music").copied().unwrap_or(0) > 0);
    assert!(bridge
        .serve("sound/SEQ_SE_DECIDE1")
        .unwrap()
        .bytes
        .starts_with(b"RIFF"));

    // Text: change the yes/no bank's first string.
    let mut yes_no = catalog::text_bank(&bridge, 2, 233).unwrap();
    yes_no[0] = "SURE".to_string();
    catalog::save_text_bank(&bridge, 2, 233, &yes_no).unwrap();
    assert_eq!(catalog::text_bank(&bridge, 2, 233).unwrap()[0], "SURE");
    let members = catalog::members(&bridge, 2).unwrap();
    assert!(members[233].changed && !members[232].changed);

    // Palette: the menu frame palette, first colour set to red.
    let pal = Member { arc: 5, index: 3 };
    let grid = catalog::Layout::Grid { cols: None };
    let sheet = catalog::sheet_data(&bridge, Member { arc: 5, index: 9 }, pal, &grid).unwrap();
    let mut colors = sheet.colors.clone();
    colors[1] = 0xFF0000;
    catalog::save_palette(&bridge, pal, &colors).unwrap();
    let again = catalog::sheet_data(&bridge, Member { arc: 5, index: 9 }, pal, &grid).unwrap();
    assert_eq!(again.colors[1], 0xFF0000);
    assert_eq!(project.changes().len(), 2);

    catalog::revert(&bridge, pal).unwrap();

    // Sprite sheets pick their cell bank: the ball icon (a/0/8/3 #112) is
    // drawn by a ball bank (#81-83 closed, #137-139 opening; both use its
    // tiles), the touch bar buttons (#20) by
    // #21-23.
    let ball = catalog::best_cell_bank(
        &bridge,
        Member {
            arc: 83,
            index: 112,
        },
    )
    .unwrap();
    assert!(matches!(ball, Some(81..=83 | 137..=139)), "{ball:?}");
    let bar = catalog::best_cell_bank(&bridge, Member { arc: 83, index: 20 }).unwrap();
    assert!(matches!(bar, Some(21..=23)), "{bar:?}");
    assert_eq!(
        catalog::sheet_data(&bridge, Member { arc: 5, index: 9 }, pal, &grid)
            .unwrap()
            .colors,
        sheet.colors
    );
}

/// Data tables and named pictures on the ROM: names come from its text,
/// values from its records, and an edit lands in the project.
#[test]
fn tables_and_names() {
    use kenon_core::bridge::Bridge;
    use kenon_core::catalog;
    use kenon_core::project::Project;
    use kenon_core::rom::Rom;
    use kenon_core::tables::{self, Section};
    let Some(path) = std::env::var_os("KENON_ROM") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let project = std::sync::Arc::new(Project::create(&dir.path().join("m"), "T", None).unwrap());
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap()).with_project(Some(project.clone()));

    let species = tables::entries(&bridge, "species").unwrap();
    assert_eq!(species[4].name, "Charmander");
    let value = |r: &tables::Record, key: &str| {
        r.sections
            .iter()
            .find_map(|s| match s {
                Section::Fields { values, .. } => {
                    values.iter().find(|v| v.key == key).map(|v| v.value)
                }
                _ => None,
            })
            .unwrap()
    };
    let charmander = tables::record(&bridge, "species", 4, 0).unwrap();
    assert_eq!(value(&charmander, "baseHp"), 39);
    let types = tables::enum_values(&bridge, "type").unwrap();
    assert_eq!(types[value(&charmander, "type1") as usize], "Fire");

    let ember = tables::entries(&bridge, "moves")
        .unwrap()
        .iter()
        .position(|e| e.name == "Ember")
        .unwrap();
    assert_eq!(
        value(
            &tables::record(&bridge, "moves", ember, 0).unwrap(),
            "power"
        ),
        40
    );

    // A learnset is a list ending at the terminator.
    let learn = tables::record(&bridge, "learnsets", 4, 0).unwrap();
    let Section::List { rows, .. } = &learn.sections[0] else {
        panic!()
    };
    assert!(rows.len() > 5 && rows[0][1] == 1, "{rows:?}");

    // Edit: Charmander's base HP to 45; saving again unchanged writes nothing new.
    let mut edit = tables::Edit::default();
    edit.values.insert("baseHp".into(), 45);
    tables::save(&bridge, "species", 4, &edit).unwrap();
    assert_eq!(
        value(&tables::record(&bridge, "species", 4, 0).unwrap(), "baseHp"),
        45
    );
    assert_eq!(project.changes().len(), 1);

    // Named pictures.
    let items = (*catalog::named_sheets(&bridge, 25).unwrap())
        .clone()
        .unwrap();
    assert!(
        items.iter().any(|s| s.label.starts_with("Master Ball")),
        "{:?}",
        &items[..5]
    );
    let battle = (*catalog::named_sheets(&bridge, 4).unwrap())
        .clone()
        .unwrap();
    assert!(battle
        .iter()
        .any(|s| s.label == "Charmander (front)" && s.palettes[0].0 == "Shiny"));
    let icons = (*catalog::named_sheets(&bridge, 7).unwrap())
        .clone()
        .unwrap();
    assert!(icons.iter().any(|s| s.label == "Pikachu"));
    // Trainer names are packed strings: they read as names, and every
    // message of every system bank survives the display form exactly.
    let names = tables::entries(&bridge, "trainers").unwrap();
    assert_eq!(names[1].name, "Jimmy");
    let table = bridge.packed_table();
    for bank in 0..bridge.base_archive(2).unwrap().len() {
        let raw = bridge
            .member(kenon_core::bridge::Member {
                arc: 2,
                index: bank,
            })
            .unwrap();
        let parsed = kenon_core::msg::Bank::parse(&raw).unwrap();
        for m in parsed.messages() {
            let text = kenon_core::msg::display(&m.units, table.as_deref());
            assert_eq!(
                kenon_core::msg::parse(&text, table.as_deref()).unwrap(),
                m.units,
                "bank {bank}: {text}"
            );
        }
    }
    let trainers = (*catalog::named_sheets(&bridge, 72).unwrap())
        .clone()
        .unwrap();
    // Battle rigs animate: Pikachu's front picture (block 500).
    let pika = bridge
        .rig(&["4:502", "4:518", "4:504", "4:505", "4:506"])
        .unwrap();
    eprintln!("Pikachu rig: {} frames", pika.frames.len());
    assert!(!pika.frames.is_empty());
    if let Some(out) = std::env::var_os("KENON_RENDER_OUT") {
        for (i, f) in pika.frames.iter().enumerate().take(6) {
            std::fs::write(
                PathBuf::from(&out).join(format!("rig_{i}_{}x{}.rgba", f.width, f.height)),
                &f.pixels,
            )
            .unwrap();
        }
    }
    eprintln!(
        "first trainer sprites: {:?}",
        trainers
            .iter()
            .take(4)
            .map(|s| &s.label)
            .collect::<Vec<_>>()
    );
}

/// Trainer locations from the maps' event data: most trainers stand in a
/// named zone.
#[test]
fn trainer_locations() {
    use kenon_core::bridge::Bridge;
    use kenon_core::rom::Rom;
    let Some(path) = std::env::var_os("KENON_ROM") else {
        return;
    };
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap());
    let zones = kenon_core::world::zones(&bridge).unwrap();
    assert_eq!(zones.len(), 427);
    let all = kenon_core::world::trainer_locations(&bridge).unwrap();
    let names = kenon_core::tables::entries(&bridge, "trainers").unwrap();
    eprintln!("{} trainers placed by their NPCs", all.len());
    for id in [1u16, 2, 3] {
        eprintln!("{}: {:?}", names[id as usize].name, all.get(&id));
    }
    assert!(all.len() > 300);
}

/// 3D textures: the map, building and overworld texture archives decode.
#[test]
fn textures_3d() {
    use kenon_core::bridge::{Bridge, Member};
    use kenon_core::g3d::{find_tex0, Tex0};
    use kenon_core::rom::Rom;
    let Some(path) = std::env::var_os("KENON_ROM") else {
        return;
    };
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap());
    // Map chunks (8) carry no textures: they use their area set (14).
    for arc in [14u16, 176, 177, 49] {
        let (mut ok, mut bad, mut members) = (0, 0, 0);
        for index in 0..bridge.base_archive(arc).unwrap().len() {
            let bytes = bridge.member(Member { arc, index }).unwrap();
            for block in find_tex0(&bytes) {
                members += 1;
                let tex = Tex0::parse(block).expect("TEX0 parses");
                for i in 0..tex.textures.len() {
                    match tex.decode(i) {
                        Some(_) => ok += 1,
                        None => bad += 1,
                    }
                }
            }
        }
        eprintln!("a{arc}: {members} texture blocks, {ok} textures decode, {bad} do not");
        assert!(ok > 0 && bad * 20 < ok + 1, "archive {arc}");
    }
}
