//! Prints map chunk wrapper headers: `chunk_probe <rom> <chunk>...`.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    for c in &a[2..] {
        let b = bridge
            .member(Member {
                arc: 8,
                index: c.parse().unwrap(),
            })
            .unwrap();
        let words: Vec<u32> = (0..6)
            .map(|i| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()))
            .collect();
        println!(
            "{c}: {} u16@2={} words {:x?} len {}",
            String::from_utf8_lossy(&b[..2]),
            u16::from_le_bytes([b[2], b[3]]),
            &words[1..],
            b.len()
        );
    }
}
