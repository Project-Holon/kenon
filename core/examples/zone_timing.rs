//! Times what the map editor loads for a zone: `zone_timing <rom> <zone>...`.

use std::time::Instant;

use kenon_core::bridge::Bridge;
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    for z in &a[2..] {
        let zone: usize = z.parse().unwrap();
        for round in 0..2 {
            let t = Instant::now();
            let m = kenon_core::zonemap::zone_map(&bridge, zone).unwrap();
            let t1 = t.elapsed();
            let s = kenon_core::scripts::event_scripts(&bridge, zone).unwrap();
            let t2 = t.elapsed();
            let p = kenon_core::props::props(&bridge, zone).unwrap();
            let _ = kenon_core::props::prop_models(&bridge, zone).unwrap();
            let t3 = t.elapsed();
            let _ = kenon_core::zonemap::object_textures(&bridge).unwrap();
            let t4 = t.elapsed();
            println!(
                "zone {zone} round {round}: {}x{} map {t1:?}, scripts ({}) {:?}, props ({}) {:?}, objtex {:?}",
                m.width,
                m.height,
                s.len(),
                t2 - t1,
                p.len(),
                t3 - t2,
                t4 - t3
            );
        }
    }
}
