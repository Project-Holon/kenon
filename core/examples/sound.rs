//! Renders one sequence to a WAV file:
//! `sound <data folder> <sequence number or name> <out.wav>`.

use kenon_core::data_root::{self, DataRootCheck};
use kenon_core::sdat::{self, Kind, Sdat};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, folder, which, out] = &args[..] else {
        eprintln!("usage: sound <data folder> <sequence number or name> <out.wav>");
        std::process::exit(2);
    };
    let DataRootCheck::Ready(root) = data_root::check(folder.as_ref()) else {
        eprintln!("no game data in {folder}");
        std::process::exit(1);
    };
    let sdat = Sdat::open(&root.file("wb_sound_data.sdat")).expect("wb_sound_data.sdat");
    let number = match which.parse::<u32>() {
        Ok(n) => n,
        Err(_) => sdat.find(Kind::Seq, which).unwrap_or_else(|| {
            eprintln!("no sequence named {which}");
            std::process::exit(1);
        }),
    };
    let clip = sdat::render(&sdat, number, &sdat::Options::default()).expect("render");
    println!(
        "sequence {number} ({}): {:.2} s, peak {}, {} notes rendered, {} dropped{}",
        sdat.name(Kind::Seq, number).unwrap_or("?"),
        clip.seconds(),
        clip.peak(),
        clip.notes_rendered,
        clip.notes_dropped,
        if clip.looped { ", loops" } else { "" }
    );
    std::fs::write(out, sdat::to_wav(&clip.samples, clip.rate)).expect("write the WAV file");
}
