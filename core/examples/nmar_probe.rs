//! Prints a multi-cell animation's sequences: `nmar_probe <rom> <arc> <member>`.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::nanr::Nanr;
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let m = Member {
        arc: a[2].parse().unwrap(),
        index: a[3].parse().unwrap(),
    };
    let multi = Nanr::parse_multi(&bridge.member(m).unwrap()).unwrap();
    for (i, s) in multi.sequences.iter().enumerate() {
        let f: Vec<String> = s
            .frames
            .iter()
            .map(|f| format!("g{}x{}", f.cell, f.duration))
            .collect();
        println!(
            "seq {i} loop {} mode {}: {}",
            s.loop_start,
            s.play_mode,
            f.join(" ")
        );
    }
}
