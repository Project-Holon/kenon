//! Prints object code records and what they point at: `objcode_probe <rom> <code>...`.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let table = bridge.member(Member { arc: 48, index: 0 }).unwrap();
    for code in &a[2..] {
        let i: usize = code.parse().unwrap();
        let rec = &table[2 + i * 28..2 + i * 28 + 28];
        println!("code {i}: {:02x?}", rec);
        let res = &rec[16..28];
        for k in 0..6 {
            let id = u16::from_le_bytes([res[k * 2], res[k * 2 + 1]]) as usize;
            if let Ok(m) = bridge.member(Member { arc: 49, index: id }) {
                println!(
                    "   res[{k}] = {id}: {} {} bytes",
                    String::from_utf8_lossy(&m[..4]),
                    m.len()
                );
            }
        }
    }
}
