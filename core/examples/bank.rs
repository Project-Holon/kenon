//! Prints strings of a text bank: `bank <rom> <archive> <bank> [first] [count]`.

use kenon_core::bridge::Bridge;
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).expect("ROM"));
    let arc: u16 = a[2].parse().unwrap();
    let bank: usize = a[3].parse().unwrap();
    let first: usize = a.get(4).map_or(0, |s| s.parse().unwrap());
    let count: usize = a.get(5).map_or(10, |s| s.parse().unwrap());
    let strings = kenon_core::catalog::text_bank(&bridge, arc, bank).unwrap();
    println!("{} strings", strings.len());
    for (i, s) in strings.iter().enumerate().skip(first).take(count) {
        println!("{i}: {s}");
    }
}
