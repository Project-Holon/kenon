//! The Pokedex search values that are not in the species data: body shape
//! and name letter group, in `eidos/tables/pokedex.txt` (eidos
//! docs/game-tables.md, "New species"):
//!
//! ```text
//! 715 = shape 3 letter 21
//! 716.1 = shape 5          form 1 of species 716
//! ```
//!
//! The game keeps them in a/0/9/8: member 1 has one letter byte per species
//! 0..649 (A 0 .. Z 25), member 2 one shape byte per Pokedex record (727:
//! the species, then the game's forms). An added species with no letter
//! takes its name's first letter; with no shape it matches no shape filter,
//! and a form with none takes its species' shape.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bridge::Bridge;
use crate::cries::Key;
use crate::project::Project;

const FILE: &str = "eidos/tables/pokedex.txt";
const ARC: u16 = 98;
const LETTERS: usize = 1;
const SHAPES: usize = 2;

/// The body shapes in the game's order (the Pokedex search screen's).
pub const SHAPES_NAMED: [&str; 14] = [
    "Head",
    "Head and legs",
    "Fins",
    "Insectoid",
    "Quadruped",
    "Multiple wing pairs",
    "Multiple bodies",
    "Tentacles or many legs",
    "Head and base",
    "Tailed biped",
    "Tailless biped",
    "Pair of wings",
    "Serpentine",
    "Head and arms",
];

/// A species' or form's search values (None: not set).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Search {
    pub shape: Option<u8>,
    pub letter: Option<u8>,
}

fn parse_key(k: &str) -> Option<Key> {
    let k = k.trim();
    let (s, f) = k.split_once('.').unwrap_or((k, "0"));
    Some(Key {
        species: s.trim().parse().ok()?,
        form: f.trim().parse().ok()?,
    })
}

fn parse(v: &str) -> Option<Search> {
    let mut out = Search::default();
    let mut words = v.split('#').next()?.split_whitespace();
    while let Some(w) = words.next() {
        let n: u8 = words.next()?.parse().ok()?;
        match w {
            "shape" if (n as usize) < SHAPES_NAMED.len() => out.shape = Some(n),
            "letter" if n < 26 => out.letter = Some(n),
            _ => return None,
        }
    }
    Some(out)
}

fn format(s: Search) -> String {
    let mut parts = Vec::new();
    if let Some(n) = s.shape {
        parts.push(format!("shape {n}"));
    }
    if let Some(n) = s.letter {
        parts.push(format!("letter {n}"));
    }
    parts.join(" ")
}

/// The mod's lines.
pub fn all(project: &Project) -> BTreeMap<Key, Search> {
    let Ok(text) = std::fs::read_to_string(project.root.join(FILE)) else {
        return BTreeMap::new();
    };
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split('#').next()?.split_once('=')?;
            Some((parse_key(k)?, parse(v)?))
        })
        .collect()
}

fn write(project: &Project, rows: &BTreeMap<Key, Search>) -> Result<(), String> {
    let path = project.root.join(FILE);
    let rows: Vec<_> = rows
        .iter()
        .filter(|(_, s)| **s != Search::default())
        .collect();
    if rows.is_empty() {
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut text = String::from(
        "# Written by kenon: <species>[.<form>] = shape <0-13> letter <A 0 .. Z 25>.\n",
    );
    for (k, s) in rows {
        text.push_str(&format!("{k} = {}\n", format(*s)));
    }
    eidos_mod::manifest::write_atomic(&path, text.as_bytes()).map_err(|e| e.to_string())
}

fn game_byte(bridge: &Bridge, member: usize, at: usize) -> Option<u8> {
    let a = bridge.base_archive(ARC).ok()?;
    let raw = a.raw(member).ok()?;
    crate::lz::decompress_if_compressed(raw).get(at).copied()
}

/// A game species' values (forms of the game have no letter of their own).
pub fn game(bridge: &Bridge, species: usize) -> Search {
    if species > 649 {
        return Search::default();
    }
    Search {
        shape: game_byte(bridge, SHAPES, species),
        letter: game_byte(bridge, LETTERS, species).filter(|&l| l < 26),
    }
}

/// A key's values in this mod: the line, else the game's.
pub fn current(bridge: &Bridge, project: &Project, key: Key) -> Search {
    all(project).get(&key).copied().unwrap_or_else(|| {
        if key.form == 0 {
            game(bridge, key.species)
        } else {
            Search::default()
        }
    })
}

/// Sets a key's values; the game's own drop the line.
pub fn set(bridge: &Bridge, project: &Project, key: Key, s: Search) -> Result<(), String> {
    if s.shape.is_some_and(|n| n as usize >= SHAPES_NAMED.len())
        || s.letter.is_some_and(|n| n >= 26)
    {
        return Err("shape is 0 to 13 and letter 0 (A) to 25 (Z)".into());
    }
    let mut rows = all(project);
    let base = if key.form == 0 {
        game(bridge, key.species)
    } else {
        Search::default()
    };
    if s == base {
        rows.remove(&key);
    } else {
        rows.insert(key, s);
    }
    write(project, &rows)
}

/// After a species is removed: its lines go, later species move down one.
pub fn remove_species(project: &Project, id: usize) -> Result<(), String> {
    let rows = all(project)
        .into_iter()
        .filter(|(k, _)| k.species != id)
        .map(|(mut k, v)| {
            if k.species > id {
                k.species -= 1;
            }
            (k, v)
        })
        .collect();
    write(project, &rows)
}

/// After form `form` of `species` is removed: its line goes, its later
/// forms move down one.
pub fn remove_form(project: &Project, species: usize, form: usize) -> Result<(), String> {
    let rows = all(project)
        .into_iter()
        .filter(|(k, _)| !(k.species == species && k.form == form))
        .map(|(mut k, v)| {
            if k.species == species && k.form > form {
                k.form -= 1;
            }
            (k, v)
        })
        .collect();
    write(project, &rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines() {
        assert_eq!(
            parse("shape 3 letter 21"),
            Some(Search {
                shape: Some(3),
                letter: Some(21)
            })
        );
        assert_eq!(
            parse("shape 5"),
            Some(Search {
                shape: Some(5),
                letter: None
            })
        );
        assert_eq!(parse("shape 14"), None);
        assert_eq!(parse("letter 26"), None);
        assert_eq!(parse("size 1"), None);
        assert_eq!(
            format(Search {
                shape: Some(3),
                letter: Some(21)
            }),
            "shape 3 letter 21"
        );
        assert_eq!(
            parse_key("716.1"),
            Some(Key {
                species: 716,
                form: 1
            })
        );
    }
}
