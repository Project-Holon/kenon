//! The Unova (regional) Pokedex: whether each species is in it, in
//! `eidos/tables/regional_dex.txt` (eidos docs/game-tables.md, "New
//! species"):
//!
//! ```text
//! <species> = unova      in it, numbered after the highest number
//! <species> = <number>   in it, at this number
//! <species> = none       National Pokedex only
//! ```
//!
//! The game's table is a/0/1/6 member 668: one u16 per species 0..649,
//! numbered as the game shows them (Victini 0, Snivy 1 ... Genesect 155),
//! 999 for a species outside it. No line: a game species keeps its place, an
//! added species is National only. The `unova` species take the numbers
//! after the highest one in use, in species id order.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Member};
use crate::project::Project;

const TABLE: &str = "regional_dex";
const GAME_TABLE: Member = Member {
    arc: 16,
    index: 668,
};
/// The game's mark for a species outside the Unova Pokedex.
const NONE: u16 = 999;

/// A species' place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "number", rename_all = "camelCase")]
pub enum Place {
    /// In the Unova Pokedex, numbered after the others.
    Unova,
    /// In it at this number.
    Number(u16),
    /// National Pokedex only.
    National,
}

fn parse(v: &str) -> Option<Place> {
    match v.split('#').next()?.trim() {
        "none" => Some(Place::National),
        "unova" => Some(Place::Unova),
        n => n
            .parse::<u16>()
            .ok()
            .filter(|&n| n < NONE)
            .map(Place::Number),
    }
}

fn format(p: Place) -> String {
    match p {
        Place::Unova => "unova".into(),
        Place::Number(n) => n.to_string(),
        Place::National => "none".into(),
    }
}

/// The game's table, by species.
fn game_table(bridge: &Bridge) -> Vec<u16> {
    bridge
        .base_archive(GAME_TABLE.arc)
        .ok()
        .and_then(|a| a.raw(GAME_TABLE.index).ok().map(<[u8]>::to_vec))
        .map(|b| {
            b.as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect()
        })
        .unwrap_or_default()
}

/// A species' place in the game (National for an added species).
pub fn game(bridge: &Bridge, species: usize) -> Place {
    match game_table(bridge).get(species) {
        Some(&n) if n != NONE => Place::Number(n),
        _ => Place::National,
    }
}

/// The mod's lines.
pub fn all(project: &Project) -> BTreeMap<usize, Place> {
    project
        .table_lines(TABLE)
        .into_iter()
        .filter_map(|(id, line)| Some((id, parse(&line)?)))
        .collect()
}

/// Every species' place in this mod, the game's under the mod's lines.
fn places(bridge: &Bridge, project: &Project) -> BTreeMap<usize, Place> {
    let mut out: BTreeMap<usize, Place> = (0..game_table(bridge).len())
        .map(|s| (s, game(bridge, s)))
        .collect();
    out.extend(all(project));
    out
}

/// A species' place in this mod.
pub fn current(bridge: &Bridge, project: &Project, species: usize) -> Place {
    all(project)
        .get(&species)
        .copied()
        .unwrap_or_else(|| game(bridge, species))
}

/// The Unova number each species shows: its own, or for `unova` species
/// the numbers after the highest one in use, in species id order.
pub fn numbers(bridge: &Bridge, project: &Project) -> BTreeMap<usize, u16> {
    let places = places(bridge, project);
    let mut next = places
        .values()
        .filter_map(|p| match p {
            Place::Number(n) => Some(*n),
            _ => None,
        })
        .max()
        .map_or(0, |n| n + 1);
    places
        .into_iter()
        .filter_map(|(s, p)| match p {
            Place::Number(n) => Some((s, n)),
            Place::Unova => {
                next += 1;
                Some((s, next - 1))
            }
            Place::National => None,
        })
        .collect()
}

fn write(project: &Project, rows: &BTreeMap<usize, Place>) -> Result<(), String> {
    let lines = rows.iter().map(|(s, p)| (*s, format(*p))).collect();
    project
        .write_table_lines(
            TABLE,
            &lines,
            "<species> = unova, <Unova Pokedex number (Victini 0)> or none.",
        )
        .map_err(|e| e.to_string())
}

/// Sets a species' place; its default (the game's place, or National for
/// an added species) drops the line.
pub fn set(bridge: &Bridge, project: &Project, species: usize, place: Place) -> Result<(), String> {
    if matches!(place, Place::Number(n) if n >= NONE) {
        return Err(format!("Unova Pokedex numbers go up to {}", NONE - 1));
    }
    let mut rows = all(project);
    if place == game(bridge, species) {
        rows.remove(&species);
    } else {
        rows.insert(species, place);
    }
    write(project, &rows)
}

/// After a species the mod adds is removed: its line goes, later species'
/// lines move down one.
pub fn remove_species(project: &Project, id: usize) -> Result<(), String> {
    let rows = all(project)
        .into_iter()
        .filter(|(s, _)| *s != id)
        .map(|(s, p)| (if s > id { s - 1 } else { s }, p))
        .collect();
    write(project, &rows)
}

/// What the species screen shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub place: Place,
    pub game: Place,
    /// The number it shows in the Unova Pokedex (None: National only).
    pub number: Option<u16>,
    /// Another species with that number, if any.
    pub shared_with: Option<usize>,
}

pub fn info(bridge: &Bridge, project: &Project, species: usize) -> Info {
    let numbers = numbers(bridge, project);
    let number = numbers.get(&species).copied();
    Info {
        place: current(bridge, project, species),
        game: game(bridge, species),
        number,
        shared_with: number.and_then(|n| {
            numbers
                .iter()
                .find(|&(&s, &m)| s != species && m == n)
                .map(|(&s, _)| s)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!(parse("0"), Some(Place::Number(0)));
        assert_eq!(parse(" 156 # new"), Some(Place::Number(156)));
        assert_eq!(parse("none"), Some(Place::National));
        assert_eq!(parse("unova"), Some(Place::Unova));
        assert_eq!(parse("999"), None);
        assert_eq!(format(Place::Unova), "unova");
    }
}
