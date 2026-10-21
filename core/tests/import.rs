//! Importing pictures from PNG against the user's ROM. Set `KENON_ROM` to
//! run them; without it they pass without doing anything.

use std::sync::Arc;

use kenon_core::bridge::{encode_png, Bridge, Member};
use kenon_core::catalog::{self, Layout};
use kenon_core::image::Rgba;
use kenon_core::import::{decode_png, import_picture, PaletteMode};
use kenon_core::nclr::{bgr555_to_rgba, rgb_to_bgr555};
use kenon_core::project::Project;
use kenon_core::rom::Rom;

fn bridge() -> Option<(Bridge, tempfile::TempDir)> {
    let path = std::env::var_os("KENON_ROM")?;
    let dir = tempfile::tempdir().unwrap();
    let project = Arc::new(Project::create(&dir.path().join("m"), "Test", None).unwrap());
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap()).with_project(Some(project));
    Some((bridge, dir))
}

const CHR: Member = Member { arc: 4, index: 20 };
const PAL: Member = Member { arc: 4, index: 38 };
const GRID: Layout = Layout::Grid { cols: Some(16) };

fn png_of(bridge: &Bridge, layout: &Layout) -> Vec<u8> {
    encode_png(
        &catalog::sheet_data(bridge, CHR, PAL, layout)
            .unwrap()
            .render(),
    )
}

#[test]
fn unchanged_picture_writes_nothing() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    // The bytes the editor shows, served as a PNG, come back unchanged.
    let served = bridge
        .serve("preview/4:20/4:38?cols=16&paloffset=0&cells=none")
        .unwrap();
    let (chr0, pal0) = (bridge.member(CHR).unwrap(), bridge.member(PAL).unwrap());
    let summary = import_picture(
        &bridge,
        CHR,
        PAL,
        &GRID,
        &served.bytes,
        PaletteMode::Keep,
        0,
    )
    .unwrap();
    assert_eq!(summary.pixels, 0, "{summary:?}");
    assert_eq!(summary.colors_merged, 0);
    assert!(!summary.palette_changed);
    assert_eq!(bridge.member(CHR).unwrap(), chr0);
    assert_eq!(bridge.member(PAL).unwrap(), pal0);
    assert!(!catalog::members(&bridge, 4).unwrap()[20].changed);

    // Every layout: cells and screens too.
    let mut layouts = vec![];
    if let Some(cer) = bridge.cell_bank(CHR).unwrap() {
        layouts.push(Layout::Cells { cer: [4, cer] });
    }
    for s in catalog::screens(&bridge, 4).unwrap().iter().take(6) {
        let chr = Member {
            arc: 4,
            index: s.chr,
        };
        let pal = Member {
            arc: 4,
            index: s.pal,
        };
        let layout = Layout::Screen { scr: [4, s.scr] };
        let data = catalog::sheet_data(&bridge, chr, pal, &layout).unwrap();
        let (c0, p0) = (bridge.member(chr).unwrap(), bridge.member(pal).unwrap());
        let png = encode_png(&data.render());
        let sum = import_picture(&bridge, chr, pal, &layout, &png, PaletteMode::Keep, 0).unwrap();
        assert_eq!(sum.pixels, 0, "screen {}: {sum:?}", s.scr);
        assert_eq!(bridge.member(chr).unwrap(), c0);
        assert_eq!(bridge.member(pal).unwrap(), p0);
    }
    for layout in layouts {
        let sum = import_picture(
            &bridge,
            CHR,
            PAL,
            &layout,
            &png_of(&bridge, &layout),
            PaletteMode::Keep,
            0,
        )
        .unwrap();
        assert_eq!(sum.pixels, 0, "{sum:?}");
        assert_eq!(bridge.member(CHR).unwrap(), chr0);
    }
}

#[test]
fn changed_pixels_read_back() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    let before = catalog::sheet_data(&bridge, CHR, PAL, &GRID).unwrap();
    let mut img = before.render();
    // A pixel whose tile pixel the picture shows once, moved to the
    // palette's other colour.
    let uses = |src: i32| before.map.iter().filter(|&&m| m == src).count();
    let i = (0..before.map.len())
        .find(|&i| before.map[i] >= 0 && uses(before.map[i]) == 1)
        .unwrap();
    let src = before.map[i] as usize;
    let now = before.tiles[src / 64][src % 64] as usize;
    let base = before.banks[i] as usize * 16 * (before.bpp == 4) as usize;
    let other = (1..16)
        .find(|&k| k != now && before.colors[base + k] != before.colors[base + now])
        .unwrap();
    let c = before.colors[base + other];
    img.pixels[i * 4..i * 4 + 4].copy_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]);
    let summary = import_picture(
        &bridge,
        CHR,
        PAL,
        &GRID,
        &encode_png(&img),
        PaletteMode::Keep,
        0,
    )
    .unwrap();
    assert_eq!(summary.pixels, 1, "{summary:?}");
    let after = catalog::sheet_data(&bridge, CHR, PAL, &GRID).unwrap();
    assert_eq!(after.render(), img);
    assert_eq!(after.colors, before.colors);
    assert!(catalog::members(&bridge, 4).unwrap()[20].changed);
}

#[test]
fn replace_writes_the_png_colours() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    let before = catalog::sheet_data(&bridge, CHR, PAL, &GRID).unwrap();
    let (red, blue) = ([248, 0, 0, 255], [0, 0, 248, 255]);
    let mut img = Rgba::new(before.width, before.height);
    for (i, &m) in before.map.iter().enumerate() {
        if m >= 0 && before.banks[i] == 0 && (i % 3 != 0) {
            let c = if i % 2 == 0 { red } else { blue };
            img.pixels[i * 4..i * 4 + 4].copy_from_slice(&c);
        }
    }
    // The PNG and the picture agree on size; a PNG with its own palette
    // type decodes too.
    let png_bytes = encode_png(&img);
    assert_eq!(decode_png(&png_bytes).unwrap(), img);
    let summary = import_picture(
        &bridge,
        CHR,
        PAL,
        &GRID,
        &png_bytes,
        PaletteMode::Replace,
        0,
    )
    .unwrap();
    assert!(summary.palette_changed, "{summary:?}");
    assert_eq!(summary.colors_used, 2);
    assert_eq!(summary.colors_merged, 0);
    let after = catalog::sheet_data(&bridge, CHR, PAL, &GRID)
        .unwrap()
        .render();
    let snap = |c: [u8; 4]| {
        let [r, g, b, _] = bgr555_to_rgba(rgb_to_bgr555(c[0], c[1], c[2]));
        [r, g, b, 255]
    };
    for i in 0..before.map.len() {
        let want = if img.pixels[i * 4 + 3] == 0 {
            [0; 4]
        } else {
            snap(img.pixels[i * 4..i * 4 + 4].try_into().unwrap())
        };
        assert_eq!(after.pixels[i * 4..i * 4 + 4], want, "pixel {i}");
    }
}

#[test]
fn wrong_size_names_both_sizes() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    let before = catalog::sheet_data(&bridge, CHR, PAL, &GRID).unwrap();
    let wrong = encode_png(&Rgba::new(before.width + 3, before.height));
    let err = import_picture(&bridge, CHR, PAL, &GRID, &wrong, PaletteMode::Keep, 0)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(&format!("{}x{}", before.width + 3, before.height)),
        "{err}"
    );
    assert!(
        err.contains(&format!("{}x{}", before.width, before.height)),
        "{err}"
    );
    assert!(import_picture(&bridge, CHR, PAL, &GRID, b"not a png", PaletteMode::Keep, 0).is_err());

    // A whole multiple scales down.
    let big = Rgba {
        width: before.width * 2,
        height: before.height * 2,
        pixels: vec![0; before.width * before.height * 16],
    };
    let sum = import_picture(
        &bridge,
        CHR,
        PAL,
        &GRID,
        &encode_png(&big),
        PaletteMode::Keep,
        0,
    )
    .unwrap();
    assert!(sum.scaled);
}
