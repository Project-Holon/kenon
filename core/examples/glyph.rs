//! Prints glyphs of a font as text: `glyph <data folder> <font member> <text>`.

use kenon_core::{data_root, narc, nftr::Font};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, folder, member, text] = &args[..] else {
        eprintln!("usage: glyph <data folder> <font member 0-4> <text>");
        std::process::exit(2);
    };
    let data_root::DataRootCheck::Ready(root) = data_root::check(folder.as_ref()) else {
        eprintln!("no game data in {folder}");
        std::process::exit(1);
    };
    let archive = narc::Narc::open(&root.file("a/0/2/3")).expect("a/0/2/3");
    let font = Font::parse(archive.raw(member.parse().unwrap()).unwrap()).expect("font");
    for code in text.encode_utf16() {
        let glyph = font.glyph_for(code).unwrap();
        println!(
            "U+{code:04X} glyph {} left {} width {} advance {}",
            font.glyph_index(code),
            glyph.left,
            glyph.width,
            glyph.advance
        );
        for row in glyph.pixels.chunks(font.cell_width as usize) {
            let line: String = row
                .iter()
                .map(|&p| [' ', '#', '+', '.'][p as usize])
                .collect();
            println!("  |{line}|");
        }
    }
}
