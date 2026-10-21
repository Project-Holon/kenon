//! Lists the cell banks of an archive that fit a sheet: `probe_cells <rom> <arc> <sheet>`.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::ncer::Ncer;
use kenon_core::ncgr::Ncgr;
use kenon_core::rom::Rom;
use kenon_core::{image, lz};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let arc: u16 = a[2].parse().unwrap();
    let sheet: usize = a[3].parse().unwrap();
    let tiles = Ncgr::parse(&bridge.member(Member { arc, index: sheet }).unwrap()).unwrap();
    println!(
        "sheet {sheet}: {}x{} tiles, {} tiles of data",
        tiles.tiles_wide,
        tiles.tiles_high,
        tiles.tile_count()
    );
    let archive = bridge.base_archive(arc).unwrap();
    let mut seen = std::collections::HashMap::new();
    for (i, raw) in archive.iter_raw().enumerate() {
        let bytes = lz::decompress_if_compressed(raw);
        if !bytes.starts_with(b"RECN") {
            continue;
        }
        let bank = Ncer::parse(&bytes).unwrap();
        let key = format!(
            "{:?}",
            bank.cells.iter().map(|c| c.objs.len()).collect::<Vec<_>>()
        );
        let first = *seen.entry(bytes.to_vec()).or_insert(i);
        let mut used = std::collections::HashSet::new();
        let mut drawn = 0;
        for c in &bank.cells {
            let m = image::cell_map(&bank, c, &tiles);
            for &(src, _) in m.pixels.iter().flatten() {
                used.insert(src / 64);
                drawn += 1;
            }
        }
        if i < 200 || (sheet.saturating_sub(30)..sheet + 30).contains(&i) {
            println!("ncer {i} (same as {first}): mapping {}, cells {}, objs {key}, tiles used {}, px {drawn}", bank.mapping, bank.cells.len(), used.len());
        }
    }
}
