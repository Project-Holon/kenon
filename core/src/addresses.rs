//! Where the code tables kenon reads sit in each cartridge. The tables move
//! between Black and White and between languages; `addresses.json` holds
//! their addresses per game code, made by `tools/code-addresses.py` from
//! the unpacked cartridges. A table missing for a cartridge is one kenon
//! does without there.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::code::Image;

const ADDRESSES: &str = include_str!("addresses.json");

/// The game code kenon falls back to when a source does not say: the
/// English Black cartridge.
pub const DEFAULT_GAME_CODE: &str = "IRBO";

type Table = HashMap<String, HashMap<String, String>>;

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(ADDRESSES).expect("addresses.json is valid"))
}

/// The image each table lives in.
fn image(name: &str) -> Image {
    match name {
        "script_ranges" => Image::Overlay(10),
        _ => Image::Arm9,
    }
}

/// The image and address of a table in a cartridge, by its game code.
pub fn lookup(game_code: &str, name: &str) -> Option<(Image, u32)> {
    let text = table().get(game_code)?.get(name)?;
    let addr = u32::from_str_radix(text.trim_start_matches("0x"), 16).ok()?;
    Some((image(name), addr))
}

/// True when kenon knows the cartridge's code tables.
pub fn known(game_code: &str) -> bool {
    table().contains_key(game_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cartridge_and_table() {
        for code in [
            "IRBO", "IRAO", "IRBF", "IRAF", "IRBD", "IRAD", "IRBI", "IRAI", "IRBS", "IRAS", "IRBJ",
            "IRAJ",
        ] {
            assert!(known(code), "{code}");
            assert_eq!(
                lookup(code, "item_icons").map(|t| t.0),
                Some(Image::Arm9),
                "{code}"
            );
        }
        assert_eq!(
            lookup("IRBO", "script_ranges"),
            Some((Image::Overlay(10), 0x0217_01A8))
        );
        assert_eq!(
            lookup("IRAO", "item_icons"),
            Some((Image::Arm9, 0x0209_EB84))
        );
        // The Japanese cartridges pack no strings.
        assert_eq!(lookup("IRBJ", "packed_chars"), None);
    }
}
