//! Map renders against the user's ROM. Set `KENON_ROM` to run them; without
//! it they pass without doing anything. No game data is in this repository.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::maprender::{self, CameraOptions, Options};
use kenon_core::rom::Rom;
use kenon_core::{nsbmd, zonemap};

fn bridge() -> Option<Bridge> {
    let path = std::env::var_os("KENON_ROM")?;
    Some(Bridge::new(Rom::open(path.as_ref()).unwrap()))
}

fn opaque_share(img: &kenon_core::image::Rgba) -> f64 {
    let n = img.pixels.chunks(4).filter(|p| p[3] == 255).count();
    n as f64 / (img.width * img.height) as f64
}

/// Every map chunk model parses to triangles of sane size (a few models,
/// such as the long bridges, reach well past their own chunk).
#[test]
fn chunk_models_parse() {
    let Some(bridge) = bridge() else { return };
    let count = bridge.base_archive(8).unwrap().len();
    let mut with_draws = 0;
    for index in 0..count {
        let bytes = bridge.member(Member { arc: 8, index }).unwrap();
        let off = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let model = nsbmd::parse(&bytes[off..]).unwrap_or_else(|| panic!("chunk {index}"));
        assert!(!model.draws.is_empty(), "chunk {index} draws nothing");
        with_draws += 1;
        for d in &model.draws {
            for t in &model.shapes[d.shape] {
                for v in t {
                    let p = model.nodes[d.node].apply(v.pos.map(|c| c * model.pos_scale));
                    assert!(p[0].abs() < 4096.0 && p[2].abs() < 4096.0, "chunk {index}");
                }
            }
        }
    }
    assert_eq!(with_draws, count);
}

/// The render lines up with `zone_map` and fills a whole chunk.
#[test]
fn render_matches_zone_map() {
    let Some(bridge) = bridge() else { return };
    // Nuvema Town (zone 389) is one chunk of ground, houses and the sea.
    let map = zonemap::zone_map(&bridge, 389).unwrap();
    let info = maprender::info(&bridge, 389, 16).unwrap();
    assert_eq!((info.width, info.height), (map.width, map.height));
    assert_eq!((info.origin_x, info.origin_z), (map.origin_x, map.origin_z));
    assert_eq!(info.image_width, map.width * 16);
    let r = maprender::render(&bridge, 389, &Options::default()).unwrap();
    assert_eq!(
        (r.image.width, r.image.height),
        (map.width * 16, map.height * 16)
    );
    assert!(opaque_share(&r.image) > 0.95);
    // The bridge path serves the same picture and its JSON.
    let png = bridge.serve("maprender/389?px=16").unwrap();
    assert_eq!(png.mime, "image/png");
    let json = bridge.serve("maprender/389/info").unwrap();
    let v: serde_json::Value = serde_json::from_slice(&json.bytes).unwrap();
    assert_eq!(v["imageWidth"], 512);
    assert_eq!(v["originX"], map.origin_x);
}

/// A building stands where its door warp is: house doors on Nuvema's
/// ground are not covered by the building's own roof.
#[test]
fn buildings_are_drawn() {
    let Some(bridge) = bridge() else { return };
    let with = maprender::render(&bridge, 389, &Options::default()).unwrap();
    let without = maprender::render(
        &bridge,
        389,
        &Options {
            buildings: false,
            ..Options::default()
        },
    )
    .unwrap();
    let diff = with
        .image
        .pixels
        .chunks(4)
        .zip(without.image.pixels.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(diff > 5000, "{diff} pixels differ");
}

/// Every zone renders (small tiles keep it quick) and agrees with its map.
#[test]
fn every_zone_renders() {
    let Some(bridge) = bridge() else { return };
    let zones = kenon_core::world::zones(&bridge).unwrap();
    let mut largest = 0;
    for z in zones.iter() {
        let map = zonemap::zone_map(&bridge, z.id).unwrap();
        if map.terrain.iter().all(|&t| t < 0) {
            continue;
        }
        let r = maprender::render(
            &bridge,
            z.id,
            &Options {
                px: 2,
                buildings: true,
                supersample: 1,
            },
        )
        .unwrap_or_else(|e| panic!("zone {}: {e}", z.id));
        assert_eq!(r.image.width, map.width * 2, "zone {}", z.id);
        assert_eq!(r.image.height, map.height * 2, "zone {}", z.id);
        largest = largest.max(map.width * map.height);
    }
    eprintln!("largest zone: {largest} tiles");
}

/// The camera view draws ground under the camera.
#[test]
fn camera_view() {
    let Some(bridge) = bridge() else { return };
    let img = maprender::render_camera(
        &bridge,
        389,
        &CameraOptions {
            x: 16.0,
            z: 20.0,
            width: 256,
            height: 192,
            ..CameraOptions::default()
        },
    )
    .unwrap();
    assert_eq!((img.width, img.height), (256, 192));
    assert!(opaque_share(&img) > 0.9);
    let png = bridge.serve("maprender/389?view=camera&x=16&z=20&w=256&h=192");
    assert!(png.is_ok());
}
