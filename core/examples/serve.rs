//! Writes bridge assets to files: `serve <rom> <out dir> <path>...`.

use kenon_core::bridge::Bridge;
use kenon_core::rom::Rom;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, rom, out, paths @ ..] = &args[..] else {
        eprintln!("usage: serve <rom> <out dir> <path>...");
        std::process::exit(2);
    };
    let bridge = Bridge::new(Rom::open(rom.as_ref()).expect("ROM"));
    for (i, path) in paths.iter().enumerate() {
        match bridge.serve(path) {
            Ok(asset) => {
                let file = std::path::Path::new(out).join(format!("{i:02}.png"));
                std::fs::write(&file, &asset.bytes).unwrap();
                println!("{path} -> {}", file.display());
            }
            Err(e) => println!("{path}: {e}"),
        }
    }
}
