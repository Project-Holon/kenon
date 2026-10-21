//! Lists members of an archive with magic and size: `members_probe <rom> <arc> <first> <count>`.

use kenon_core::bridge::Bridge;
use kenon_core::lz;
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let arc: u16 = a[2].parse().unwrap();
    let first: usize = a[3].parse().unwrap();
    let count: usize = a[4].parse().unwrap();
    let archive = bridge.base_archive(arc).unwrap();
    for (i, raw) in archive.iter_raw().enumerate().skip(first).take(count) {
        let compressed = raw.first().is_some_and(|&b| b == 0x10 || b == 0x11);
        let bytes = lz::decompress_if_compressed(raw);
        let magic = String::from_utf8_lossy(&bytes[..4.min(bytes.len())]).to_string();
        println!(
            "{i}: {magic} {} bytes{}",
            bytes.len(),
            if compressed { " (lz)" } else { "" }
        );
    }
}
