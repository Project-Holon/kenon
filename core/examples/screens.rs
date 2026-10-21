//! Renders the first screens of an archive: `screens <rom> <out dir> <arc>...`.

use kenon_core::bridge::Bridge;
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    for arc in &a[3..] {
        let arc: u16 = arc.parse().unwrap();
        let list = kenon_core::catalog::screens(&bridge, arc).unwrap();
        println!("a{arc}: {} screens", list.len());
        for s in list.iter().take(3) {
            let path = format!("screen/{arc}:{}/{arc}:{}/{arc}:{}?zero=opaque", s.chr, s.pal, s.scr);
            let asset = bridge.serve(&path).unwrap();
            std::fs::write(format!("{}/{arc}_{}.png", a[2], s.scr), &asset.bytes).unwrap();
        }
    }
}
