//! Lists trainers of zones whose name contains a word, with their
//! character and its frame count: `hidden_probe <rom> <word>`.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let tex = kenon_core::zonemap::object_textures(&bridge).unwrap();
    for z in kenon_core::world::zones(&bridge).unwrap().iter() {
        if !z.place.contains(a[2].as_str()) {
            continue;
        }
        let Ok(map) = kenon_core::zonemap::zone_map(&bridge, z.id) else {
            continue;
        };
        for e in map.events.iter().filter(|e| e.trainer.is_some()) {
            let v = |k: &str| e.values.iter().find(|v| v.key == k).map_or(0, |v| v.value);
            let code = v("objCode") as usize;
            let t = tex.get(code).copied().flatten();
            let frames = t.and_then(|t| {
                let b = bridge
                    .member(Member {
                        arc: 49,
                        index: t as usize,
                    })
                    .ok()?;
                let blocks = kenon_core::g3d::find_tex0(&b);
                Some(
                    blocks
                        .iter()
                        .filter_map(|b| kenon_core::g3d::Tex0::parse(b))
                        .map(|t| t.textures.len())
                        .sum::<usize>(),
                )
            });
            println!(
                "zone {} {}: trainer {:?} code {code} type {} move {} tex {t:?} frames {frames:?}",
                z.id,
                z.place,
                e.trainer,
                v("eventType"),
                v("moveCode")
            );
        }
    }
}
