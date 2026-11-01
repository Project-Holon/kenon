//! Checks on every cartridge kenon knows. Set `KENON_ROMS` to a folder of
//! ROMs (any of black.nds, white.nds, black_fr.nds ... white_ja.nds, the
//! names aletheia's docs/ROMS.md uses); the cartridges found there are
//! opened and their code tables read.

use std::path::PathBuf;

use kenon_core::bridge::Bridge;
use kenon_core::rom::{Game, Language, Rom};

const NAMES: &[&str] = &[
    "black", "white", "black_fr", "white_fr", "black_de", "white_de", "black_it", "white_it",
    "black_es", "white_es", "black_ja", "white_ja",
];

#[test]
fn every_cartridge_reads_its_code_tables() {
    let Some(dir) = std::env::var_os("KENON_ROMS").map(PathBuf::from) else {
        eprintln!("KENON_ROMS not set; skipped");
        return;
    };
    let mut seen = 0;
    for name in NAMES {
        let path = dir.join(format!("{name}.nds"));
        if !path.is_file() {
            continue;
        }
        seen += 1;
        let rom = Rom::open(&path).unwrap();
        let id = rom.identity.clone();
        assert_eq!(
            id.game,
            if name.starts_with("black") {
                Game::Black
            } else {
                Game::White
            },
            "{name}"
        );
        if name.ends_with("_ja") {
            assert_eq!(id.language, Language::Japanese, "{name}");
        }
        assert!(rom.is_known_dump().unwrap(), "{name}: not a known dump");
        let bridge = Bridge::new(rom);
        // The item icon table starts with the Master Ball's members.
        let icons =
            String::from_utf8(bridge.serve("table/item_icons").unwrap().bytes.clone()).unwrap();
        assert!(
            icons.starts_with("[233,3,234,3,"),
            "{name}: {}",
            &icons[..40]
        );
        let types =
            String::from_utf8(bridge.serve("table/type_palette").unwrap().bytes.clone()).unwrap();
        assert_eq!(types.matches(',').count(), 17, "{name}");
        assert!(bridge
            .serve("cursor/5:3/0")
            .unwrap()
            .bytes
            .starts_with(b"\x89PNG"));
        // Species names, bank 70.
        let bulbasaur =
            String::from_utf8(bridge.serve("msg/2/70/1").unwrap().bytes.clone()).unwrap();
        assert!(!bulbasaur.is_empty(), "{name}");
        eprintln!(
            "{name}: {} {:?} {:?}, species 1 {bulbasaur}",
            id.game_code, id.game, id.language
        );
    }
    eprintln!("{seen} cartridges checked");
}
