//! Prints a rig's parts: `rig_probe <rom> <arc> <sheet> <cells> <anims> <multi>`.

use kenon_core::bridge::Bridge;
use kenon_core::bridge::Member;
use kenon_core::nanr::Nanr;
use kenon_core::ncer::Ncer;
use kenon_core::ncgr::Ncgr;
use kenon_core::rig::Nmcr;
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let arc: u16 = a[2].parse().unwrap();
    let get = |i: usize| {
        bridge
            .member(Member {
                arc,
                index: a[i].parse().unwrap(),
            })
            .unwrap()
    };
    let sheet = Ncgr::parse(&get(3)).unwrap();
    println!(
        "sheet {}x{} tiles, {:?}, mapping {:#x}, linear {}",
        sheet.tiles_wide, sheet.tiles_high, sheet.depth, sheet.mapping, sheet.linear
    );
    let cells = Ncer::parse(&get(4)).unwrap();
    println!("cells mapping {:#x}", cells.mapping);
    for (i, c) in cells.cells.iter().enumerate() {
        println!("cell {i}:");
        for o in &c.objs {
            println!("  {o:?}");
        }
    }
    let anims = Nanr::parse(&get(5)).unwrap();
    for (i, s) in anims.sequences.iter().enumerate() {
        println!("seq {i} kind {} mode {}:", s.kind, s.play_mode);
        for f in &s.frames {
            println!("  {f:?}");
        }
    }
    let rig = Nmcr::parse(&get(6)).unwrap();
    for (i, g) in rig.groups.iter().enumerate() {
        println!("group {i}: {g:?}");
    }
}
