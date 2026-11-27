//! The game's own text in another language, shown beside a mod's
//! translation: from a ROM of that language when the user set one (every
//! bank), else from kenon's bundled copy of Black and White's names and
//! descriptions (reference.json.gz, tools/make_reference.py, from PokeAPI's
//! data of the games; German, Italian and Spanish descriptions are X and
//! Y's, which PokeAPI has where it has none of Black and White's).

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::Value;

use crate::bridge::{Bridge, Error};

const BUNDLED: &[u8] = include_bytes!("reference.json.gz");

/// Where a bank's reference lines came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// A ROM in that language.
    Rom,
    /// kenon's copy of Black and White's text.
    Bundled,
    /// kenon's copy, from X and Y (no Black and White text in PokeAPI).
    BundledLater,
}

#[derive(Debug, Clone, Serialize)]
pub struct Lines {
    pub source: Source,
    pub lines: BTreeMap<usize, String>,
}

fn bundled() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut text = String::new();
        flate2::read::GzDecoder::new(BUNDLED)
            .read_to_string(&mut text)
            .expect("reference.json.gz unpacks");
        serde_json::from_str(&text).expect("reference.json.gz is JSON")
    })
}

/// The bundled lines of a bank of a/0/0/2 in `lang` (en, fr, de, it, es).
pub fn bundled_lines(lang: &str, bank: usize) -> Option<Lines> {
    let data = bundled();
    let lines = data["languages"][lang][bank.to_string()].as_object()?;
    let later = data["later"][lang]
        .as_array()
        .is_some_and(|b| b.iter().any(|v| v.as_str() == Some(&bank.to_string())));
    Some(Lines {
        source: if later {
            Source::BundledLater
        } else {
            Source::Bundled
        },
        lines: lines
            .iter()
            .filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_string())))
            .collect(),
    })
}

/// A reference ROM's lines of a text bank (a/0/0/2 or a/0/0/3).
pub fn rom_lines(rom: &Bridge, arc: u16, bank: usize) -> Result<Lines, Error> {
    let lines = crate::catalog::text_bank(rom, arc, bank)?
        .into_iter()
        .enumerate()
        .filter(|(_, s)| !s.is_empty())
        .collect();
    Ok(Lines {
        source: Source::Rom,
        lines,
    })
}

/// The language code (en, fr, de, it, es) of a ROM, when it is one eidos
/// runs.
pub fn language_of(rom: &crate::rom::Rom) -> Option<&'static str> {
    use crate::rom::Language as L;
    match rom.identity.language {
        L::English => Some("en"),
        L::French => Some("fr"),
        L::German => Some("de"),
        L::Italian => Some("it"),
        L::Spanish => Some("es"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_text_reads() {
        let fairy_wind = bundled_lines("fr", 203).unwrap();
        assert_eq!(fairy_wind.source, Source::Bundled);
        assert_eq!(fairy_wind.lines[&1], "Écras'Face");
        assert_eq!(bundled_lines("fr", 199).unwrap().lines[&9], "Feu");
        assert_eq!(
            bundled_lines("de", 202).unwrap().source,
            Source::BundledLater
        );
        assert!(bundled_lines("fr", 3).is_none());
    }
}
