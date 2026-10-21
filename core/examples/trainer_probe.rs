//! Prints trainers by name with class and sprite graphic: `trainer_probe <rom> <name>`.

use kenon_core::bridge::Bridge;
use kenon_core::rom::Rom;
use kenon_core::tables::{self, Section};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let code = bridge.code().unwrap();
    for e in tables::entries(&bridge, "trainers")
        .unwrap()
        .iter()
        .filter(|e| e.name == a[2])
    {
        let r = tables::record(&bridge, "trainers", e.index, 0).unwrap();
        let Section::Fields { values, .. } = &r.sections[0] else {
            continue;
        };
        let class = values
            .iter()
            .find(|v| v.key == "trainerClass")
            .unwrap()
            .value as u32;
        let g = code
            .read(kenon_core::code::Image::Arm9, 0x020A_023C + class, 1)
            .unwrap()[0];
        println!(
            "trainer {} class {class} graphic {g} (members {}..{})",
            e.index,
            8 * g as u32,
            8 * g as u32 + 7
        );
    }
}
