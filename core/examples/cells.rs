//! Lists the cells of an NCER member: `cells <data folder> <archive id> <member>`.

use kenon_core::{data_root, lz, narc, ncer::Ncer, ncgr::Ncgr};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, folder, id, member, rest @ ..] = &args[..] else {
        eprintln!("usage: cells <data folder> <archive id> <NCER member> [NCGR member]");
        std::process::exit(2);
    };
    let data_root::DataRootCheck::Ready(root) = data_root::check(folder.as_ref()) else {
        eprintln!("no game data in {folder}");
        std::process::exit(1);
    };
    let path = narc::archive_path(id.parse().unwrap()).unwrap();
    let archive = narc::Narc::open(&root.file(&path)).unwrap();
    let get = |i: usize| lz::decompress_if_compressed(archive.raw(i).unwrap()).into_owned();
    let bank = Ncer::parse(&get(member.parse().unwrap())).unwrap();
    println!(
        "{path} #{member}: mapping {}, {} cells",
        bank.mapping,
        bank.cells.len()
    );
    for (i, cell) in bank.cells.iter().enumerate() {
        println!(
            "  cell {i}: attribute {:#x}, bounds {:?}",
            cell.attribute, cell.bounds
        );
        for obj in &cell.objs {
            println!("    {obj:?}");
        }
    }
    if let Some(m) = rest.first() {
        let tiles = Ncgr::parse(&get(m.parse().unwrap())).unwrap();
        println!(
            "NCGR #{m}: {}x{} tiles, {:?}, mapping {}, linear {}, {} bytes",
            tiles.tiles_wide,
            tiles.tiles_high,
            tiles.depth,
            tiles.mapping,
            tiles.linear,
            tiles.data.len()
        );
    }
}
