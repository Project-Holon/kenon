//! Prints the model names in a building set: `propnames_probe <rom> <arc> <member>`.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let b = bridge
        .member(Member {
            arc: a[2].parse().unwrap(),
            index: a[3].parse().unwrap(),
        })
        .unwrap();
    let n = b[2] as usize / 2;
    let off = |i: usize| u32::from_le_bytes(b[4 + i * 4..8 + i * 4].try_into().unwrap()) as usize;
    for i in 0..n {
        let id = u16::from_le_bytes([b[off(i)], b[off(i) + 1]]);
        let name = kenon_core::nsbmd::parse(&b[off(n + i)..])
            .map(|m| m.name)
            .unwrap_or_default();
        println!("{id}: {name}");
    }
}
