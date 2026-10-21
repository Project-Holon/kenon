//! Writes zone map renders: `maprender <rom> <out dir> <px> <zone>...`.

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
    for z in zones {
        let zone: usize = z.parse().expect("zone");
        let t = std::time::Instant::now();
        if let Some(at) = std::env::var_os("CAMERA") {
            let at = at.to_string_lossy().to_string();
            let (x, zz) = at.split_once(',').expect("CAMERA=x,z");
            let opts = maprender::CameraOptions {
                x: x.parse().expect("x"),
                z: zz.parse().expect("z"),
                ..maprender::CameraOptions::default()
            };
            let img = maprender::render_camera(&bridge, zone, &opts).expect("camera");
            let file = std::path::Path::new(out).join(format!("zone{zone}_camera.png"));
            std::fs::write(&file, kenon_core::bridge::encode_png(&img)).unwrap();
            println!(
                "zone {zone}: camera {:?} -> {}",
                t.elapsed(),
                file.display()
            );
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
                if std::env::var_os("OVERLAY").is_some() {
                    overlay(&bridge, zone, &mut r);
                }
                let file = std::path::Path::new(out).join(format!("zone{zone}.png"));
                std::fs::write(&file, kenon_core::bridge::encode_png(&r.image)).unwrap();
                println!(
                    "zone {zone}: {}x{} tiles, {}x{} px, {:?} -> {}",
                    r.layout.width,
                    r.layout.height,
                    r.layout.image_width,
                    r.layout.image_height,
                    t.elapsed(),
                    file.display()
                );
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
