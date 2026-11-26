//! The Unova (regional) Pokedex: each species' number in it, in
//! `eidos/tables/regional_dex.txt` (eidos docs/game-tables.md):
//!
//! ```text
//! <species> = <Unova number>
//! <species> = none
//! ```
//!
//! The game's table is a/0/1/6 member 668: one u16 per species 0..649,
//! numbered as the game shows them (Victini 0, Snivy 1 ... Genesect 155),
//! 999 for a species outside it. No line: a game species keeps its number,
//! a new species is national only.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::bridge::{Bridge, Member};
use crate::project::Project;

const TABLE: &str = "regional_dex";
const GAME_TABLE: Member = Member {
    arc: 16,
    index: 668,
};
/// The game's mark for a species outside the Unova Pokedex.
const NONE: u16 = 999;

/// A species' place: Some(number), or None for national only.
pub type Place = Option<u16>;

fn parse(v: &str) -> Option<Place> {
    let v = v.split('#').next()?.trim();
    if v == "none" {
        return Some(None);
    }
    v.parse::<u16>().ok().filter(|&n| n < NONE).map(Some)
}

/// The game's table, by species.
fn game_table(bridge: &Bridge) -> Vec<u16> {
    bridge
        .base_archive(GAME_TABLE.arc)
        .ok()
        .and_then(|a| a.raw(GAME_TABLE.index).ok().map(<[u8]>::to_vec))
        .map(|b| {
            b.chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect()
        })
        .unwrap_or_default()
}

/// A species' place in the game.
pub fn game(bridge: &Bridge, species: usize) -> Place {
    game_table(bridge)
        .get(species)
        .copied()
        .filter(|&n| n != NONE)
}

/// The mod's lines.
pub fn all(project: &Project) -> BTreeMap<usize, Place> {
    project
        .table_lines(TABLE)
        .into_iter()
        .filter_map(|(id, line)| Some((id, parse(&line)?)))
        .collect()
}

/// A species' place in this mod.
pub fn current(bridge: &Bridge, project: &Project, species: usize) -> Place {
    all(project)
        .get(&species)
        .copied()
        .unwrap_or_else(|| game(bridge, species))
}

/// The number after the highest in use: where a new species goes by default.
pub fn next_number(bridge: &Bridge, project: &Project) -> u16 {
    let mut places: BTreeMap<usize, Place> = game_table(bridge)
        .into_iter()
        .enumerate()
        .map(|(s, n)| (s, (n != NONE).then_some(n)))
        .collect();
    places.extend(all(project));
    places.values().flatten().max().map_or(0, |n| n + 1)
}

/// The species holding a number in this mod, other than `except`.
pub fn holder(bridge: &Bridge, project: &Project, number: u16, except: usize) -> Option<usize> {
    let mut places: BTreeMap<usize, Place> = game_table(bridge)
        .into_iter()
        .enumerate()
        .map(|(s, n)| (s, (n != NONE).then_some(n)))
        .collect();
    places.extend(all(project));
    places
        .into_iter()
        .find(|&(s, p)| s != except && p == Some(number))
        .map(|(s, _)| s)
}

fn write(project: &Project, rows: &BTreeMap<usize, Place>) -> Result<(), String> {
    let lines = rows
        .iter()
        .map(|(s, p)| (*s, p.map_or("none".to_string(), |n| n.to_string())))
        .collect();
    project
        .write_table_lines(
            TABLE,
            &lines,
            "<species> = <Unova Pokedex number, Victini 0> or none.",
        )
        .map_err(|e| e.to_string())
}

/// Sets a species' place; its default (the game's, or none for a new
/// species) drops the line.
pub fn set(bridge: &Bridge, project: &Project, species: usize, place: Place) -> Result<(), String> {
    if place.is_some_and(|n| n >= NONE) {
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
    pub next: u16,
}

pub fn info(bridge: &Bridge, project: &Project, species: usize) -> Info {
    Info {
        place: current(bridge, project, species),
        game: game(bridge, species),
        next: next_number(bridge, project),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!(parse("0"), Some(Some(0)));
        assert_eq!(parse(" 156 # new"), Some(Some(156)));
        assert_eq!(parse("none"), Some(None));
        assert_eq!(parse("999"), None);
        assert_eq!(parse("x"), None);
    }
}
