//! Map renders against the user's ROM. Set `KENON_ROM` to run them; without
//! it they pass without doing anything. No game data is in this repository.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::image::Rgba;
use kenon_core::maprender::{self, CameraOptions, Options, Roofs};
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
                supersample: 1,
                ..Options::default()
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

fn differing(a: &Rgba, b: &Rgba) -> usize {
    a.pixels
        .chunks(4)
        .zip(b.pixels.chunks(4))
        .filter(|(p, q)| p != q)
        .count()
}

fn brightness(img: &Rgba) -> f64 {
    let sum: u64 = img
        .pixels
        .chunks(4)
        .map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64)
        .sum();
    sum as f64 / (img.width * img.height * 3) as f64
}

fn camera(bridge: &Bridge, zone: usize, f: impl FnOnce(&mut CameraOptions)) -> Rgba {
    let mut opts = CameraOptions {
        x: 16.0,
        z: 20.0,
        width: 192,
        height: 144,
        ..CameraOptions::default()
    };
    f(&mut opts);
    maprender::render_camera(bridge, zone, &opts).unwrap()
}

/// People show in both views and can be turned off.
#[test]
fn people_are_drawn() {
    let Some(bridge) = bridge() else { return };
    // Route 1 has people on it.
    let map = zonemap::zone_map(&bridge, 317).unwrap();
    assert!(map.events.iter().any(|e| e.kind == "npc" && e.x >= 0));
    let render = |people| {
        maprender::render(
            &bridge,
            317,
            &Options {
                px: 8,
                people,
                ..Options::default()
            },
        )
        .unwrap()
        .image
    };
    let diff = differing(&render(true), &render(false));
    assert!(diff > 200, "{diff} pixels differ from above");
    // The camera follows the first person on the route.
    let npc = map
        .events
        .iter()
        .find(|e| e.kind == "npc" && e.x >= 0)
        .unwrap();
    let at = |people| {
        camera(&bridge, 317, |o| {
            o.x = npc.x as f32;
            o.z = npc.z as f32;
            o.people = people;
        })
    };
    let diff = differing(&at(true), &at(false));
    assert!(diff > 100, "{diff} pixels differ in the camera");
}

/// The field light shades surfaces and follows the time of day.
#[test]
fn lighting_and_time() {
    let Some(bridge) = bridge() else { return };
    let lit = camera(&bridge, 389, |_| {});
    let flat = camera(&bridge, 389, |o| o.light = false);
    assert!(differing(&lit, &flat) > 1000);
    let night = camera(&bridge, 389, |o| o.hour = 22.0);
    assert!(
        brightness(&night) < brightness(&lit) * 0.9,
        "night {} day {}",
        brightness(&night),
        brightness(&lit)
    );
}

/// A season swaps the map's texture set and light table.
#[test]
fn seasons_differ() {
    let Some(bridge) = bridge() else { return };
    let spring = camera(&bridge, 389, |_| {});
    let winter = camera(&bridge, 389, |o| o.season = 3);
    assert!(differing(&spring, &winter) > 5000);
    // The same season twice is the same picture (a kept scene).
    let again = camera(&bridge, 389, |_| {});
    assert_eq!(spring.pixels, again.pixels);
    bridge.invalidate();
    let rebuilt = camera(&bridge, 389, |_| {});
    assert_eq!(spring.pixels, rebuilt.pixels);
}

/// Tall roofs can be cut from the top-down view of a tall city.
#[test]
fn roofs_can_be_cut() {
    let Some(bridge) = bridge() else { return };
    let draw = |roofs| {
        maprender::render(
            &bridge,
            31,
            &Options {
                px: 4,
                roofs,
                ..Options::default()
            },
        )
        .unwrap()
        .image
    };
    let all = draw(Roofs::All);
    let cut = draw(Roofs::Cut(48.0));
    assert!(differing(&all, &cut) > 0);
    // Castelia is tall, so Auto cuts; a small town is drawn whole.
    assert_eq!(draw(Roofs::Auto).pixels, draw(Roofs::Cut(96.0)).pixels);
    let town = |roofs| {
        maprender::render(
            &bridge,
            389,
            &Options {
                px: 4,
                roofs,
                ..Options::default()
            },
        )
        .unwrap()
        .image
    };
    assert_eq!(town(Roofs::Auto).pixels, town(Roofs::All).pixels);
}

/// The new query parameters reach the renderer.
#[test]
fn bridge_parameters() {
    let Some(bridge) = bridge() else { return };
    for q in [
        "view=camera&x=16&z=20&w=96&h=72&season=2&time=night&people=0&light=1",
        "view=camera&x=16&z=20&w=96&h=72&time=17:30&light=0",
        "px=4&people=1&season=1&time=evening",
        "px=4&roofs=0&roofh=2",
        "px=4&roofs=1",
    ] {
        let png = bridge.serve(&format!("maprender/389?{q}")).unwrap();
        assert_eq!(png.mime, "image/png", "{q}");
    }
}

/// Every prop model of two zones renders a thumbnail and reports a size.
#[test]
fn prop_models_render() {
    let Some(bridge) = bridge() else { return };
    for zone in [389usize, 317] {
        let models = kenon_core::props::prop_models(&bridge, zone).unwrap();
        assert!(!models.is_empty(), "zone {zone}");
        for model in models {
            let img = maprender::render_prop(&bridge, zone, model, 64)
                .unwrap_or_else(|e| panic!("zone {zone} model {model}: {e}"));
            assert_eq!((img.width, img.height), (64, 64));
            let drawn = img.pixels.chunks(4).filter(|p| p[3] > 0).count();
            let info = maprender::prop_info(&bridge, zone, model).unwrap();
            // A few ids hold no geometry; they give an empty picture.
            if info.width + info.depth + info.height > 0.0 {
                assert!(drawn > 10, "zone {zone} model {model}: {drawn} pixels");
            }
            assert!(info.width < 64.0 && info.depth < 64.0 && info.height < 64.0);
        }
    }
    let png = bridge.serve("propmodel/389/1?px=32").unwrap();
    assert_eq!(png.mime, "image/png");
    let json = bridge.serve("propmodel/389/1/info").unwrap();
    let v: serde_json::Value = serde_json::from_slice(&json.bytes).unwrap();
    assert!(v["width"].as_f64().unwrap() > 0.0);
    assert!(bridge.serve("propmodel/389/65000").is_err());
}
