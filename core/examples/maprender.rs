//! Writes zone map renders: `maprender <rom> <out dir> <px> <zone>...`. Set
//! `QUERY` to a maprender query string, `BENCH=n` to time a camera sweep, or
//! `OVERLAY=1` to tint the walkable tiles and mark events.

use kenon_core::bridge::Bridge;
use kenon_core::maprender::{self, Options};
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, rom, out, px, zones @ ..] = &a[..] else {
        eprintln!("usage: maprender <rom> <out dir> <px> <zone>...");
        std::process::exit(2);
    };
    let bridge = Bridge::new(Rom::open(rom.as_ref()).expect("ROM"));
    let px: usize = px.parse().expect("px");
    if zones.first().is_some_and(|z| z == "list") {
        for z in kenon_core::world::zones(&bridge).expect("zones").iter() {
            println!("{}	{}	matrix {}", z.id, z.place, z.matrix);
        }
        return;
    }
    // `query` is what follows `?` in a `maprender/{zone}` path, for example
    // `px=16&season=1` or `view=camera&x=20&z=20&time=night`.
    let query = std::env::var("QUERY").unwrap_or_else(|_| format!("px={px}"));
    for z in zones {
        let zone: usize = z.parse().expect("zone");
        if let Some(n) = std::env::var_os("BENCH") {
            // A camera sweep along the zone: the first call builds the scene.
            let n: usize = n.to_string_lossy().parse().unwrap_or(5);
            let map = kenon_core::zonemap::zone_map(&bridge, zone).expect("zone map");
            for i in 0..n {
                let (x, zz) = (
                    (map.width / 4 + i * 3).min(map.width - 1),
                    map.height / 2 + i % 3,
                );
                let path = format!("maprender/{zone}?view=camera&x={x}&z={zz}&w=384&h=288&{query}");
                let t = std::time::Instant::now();
                let asset = bridge.serve(&path).expect("camera");
                println!(
                    "zone {zone} camera #{i} ({x},{zz}): {:?} ({} bytes png)",
                    t.elapsed(),
                    asset.bytes.len()
                );
            }
            continue;
        }
        let t = std::time::Instant::now();
        if std::env::var_os("OVERLAY").is_none() {
            let path = format!("maprender/{zone}?{query}");
            match bridge.serve(&path) {
                Ok(asset) => {
                    let tag = query.replace(['&', '=', '/'], "_");
                    let file = std::path::Path::new(out).join(format!("zone{zone}_{tag}.png"));
                    std::fs::write(&file, &asset.bytes).unwrap();
                    println!("zone {zone}: {:?} -> {}", t.elapsed(), file.display());
                }
                Err(e) => println!("zone {zone}: {e}"),
            }
            continue;
        }
        match maprender::render(
            &bridge,
            zone,
            &Options {
                px,
                ..Options::default()
            },
        ) {
            Ok(mut r) => {
                overlay(&bridge, zone, &mut r);
                let file = std::path::Path::new(out).join(format!("zone{zone}_overlay.png"));
                std::fs::write(&file, kenon_core::bridge::encode_png(&r.image)).unwrap();
                println!("zone {zone}: {:?} -> {}", t.elapsed(), file.display());
            }
            Err(e) => println!("zone {zone}: {e}"),
        }
    }
}

/// Debug aid: tint tiles the game does not let you walk on, and mark events.
fn overlay(bridge: &Bridge, zone: usize, r: &mut maprender::Render) {
    let map = kenon_core::zonemap::zone_map(bridge, zone).expect("zone map");
    let px = r.layout.px;
    let w = r.image.width;
    let mut tint = |x: usize, z: usize, c: [u8; 3], a: u32| {
        for dy in 0..px {
            for dx in 0..px {
                let i = ((z * px + dy) * w + x * px + dx) * 4;
                for (k, &ck) in c.iter().enumerate() {
                    let o = r.image.pixels[i + k] as u32;
                    r.image.pixels[i + k] = ((o * (100 - a) + ck as u32 * a) / 100) as u8;
                }
                r.image.pixels[i + 3] = 255;
            }
        }
    };
    for z in 0..map.height {
        for x in 0..map.width {
            let i = z * map.width + x;
            if map.terrain[i] >= 0 && map.flags[i] & 1 == 0 {
                tint(x, z, [255, 0, 0], 35);
            }
        }
    }
    for e in &map.events {
        if e.x >= 0 && e.z >= 0 {
            let c = match e.kind.as_str() {
                "warp" => [0, 0, 255],
                "npc" => [255, 255, 0],
                _ => [255, 0, 255],
            };
            for dz in 0..e.h.max(1) {
                for dx in 0..e.w.max(1) {
                    let (x, z) = ((e.x + dx) as usize, (e.z + dz) as usize);
                    if x < map.width && z < map.height {
                        tint(x, z, c, 60);
                    }
                }
            }
        }
    }
}
