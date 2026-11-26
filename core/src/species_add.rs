//! New Pokemon: a species added after the game's own, copied from one that
//! exists, in every archive a species lives in.
//!
//! Ids. The game reads a species' files at member = id, and ids 650..714
//! are taken (eggs, alternate forms, the Substitute doll: eidos
//! docs/research/limits/species-data.md, species-assets.md). New species
//! start at 715, the first id whose sprite and icon members lie past the
//! end of their archives, so nothing has to move. Ids 652..714 stay unused.
//!
//! What one species is, by archive (member index for species `n`):
//!
//! | archive          | members              | what                         |
//! |------------------|----------------------|------------------------------|
//! | a/0/1/6 (16)     | n                    | stats (60 bytes)             |
//! | a/0/1/8 (18)     | n                    | level-up learnset            |
//! | a/0/1/9 (19)     | n                    | evolutions (42 bytes)        |
//! | a/1/2/3 (123)    | n                    | egg moves                    |
//! | a/0/2/0 (20)     | n                    | base species (u16)           |
//! | a/1/9/4 (194)    | n                    | cry record (4 bytes)         |
//! | a/0/0/4 (4)      | 20 n .. 20 n + 19    | battle sprites and palettes  |
//! | a/0/0/7 (7)      | 7 + 2 n, 8 + 2 n     | party icon                   |
//! | a/1/6/7 (167)    | n + 5                | Pokedex list icon            |
//!
//! Text lines (a/0/0/2): 70 name, 284 name in capitals, 281 name with its
//! article, 260 category, 235 and 236 Pokedex entry. Height and weight
//! (banks 244, 245, 268) have 727 lines, one per species and form; how the
//! Pokedex numbers a new species there is not known yet, so they are left
//! alone.
//!
//! Not in kenon's hands yet (eidos): `limit.species` (written now, read
//! once eidos lifts the limit), the icon palette table, the cry sound and
//! the Pokedex bits past 672.

use std::collections::BTreeMap;

use crate::bridge::{Bridge, Error, Member};
use crate::catalog;
use crate::content::Removed;
use crate::project::Project;
use crate::tables;

/// The first id a new species gets.
pub const FIRST_NEW: usize = 715;
/// The game's last species.
pub const LAST_GAME: usize = 649;
/// eidos's ceiling: the Pokedex keeps the last species viewed in 10 bits
/// (wild and trainer records hold 11).
pub const CEILING: usize = 1023;
const LIMIT_KEY: &str = "limit.species";

/// The archives with one member (or block) per species: (archive, members
/// per species, first member's offset).
const ARCHIVES: [(u16, usize, usize); 9] = [
    (16, 1, 0),
    (18, 1, 0),
    (19, 1, 0),
    (123, 1, 0),
    (20, 1, 0),
    (194, 1, 0),
    (4, 20, 0),
    (7, 2, 7),
    (167, 1, 5),
];

/// Text banks with one line per species, and how a new species' line is
/// made.
const TEXTS: [(usize, Line); 6] = [
    (70, Line::Name),
    (284, Line::Caps),
    (281, Line::Copy),
    (260, Line::Copy),
    (235, Line::Copy),
    (236, Line::Copy),
];

#[derive(Clone, Copy)]
enum Line {
    Name,
    Caps,
    Copy,
}

const TEXT_ARC: u16 = 2;
const STATS_ARC: u16 = 16;
const EVOLUTION_ARC: u16 = 19;
const BASE_SPECIES_ARC: u16 = 20;

fn project_of(bridge: &Bridge) -> Result<&std::sync::Arc<Project>, Error> {
    bridge.project().ok_or_else(|| {
        Error::BadPath("no project is open: a new Pokemon needs a mod to go in".into())
    })
}

/// The species ids the mod adds (715 and up).
pub fn added(bridge: &Bridge) -> Vec<usize> {
    let end = next_id(bridge);
    (FIRST_NEW..end).collect()
}

/// The id the next new species gets.
fn next_id(bridge: &Bridge) -> usize {
    let last = bridge
        .project()
        .and_then(|p| p.last_member(STATS_ARC))
        .filter(|&m| m >= FIRST_NEW);
    last.map_or(FIRST_NEW, |m| m + 1)
}

fn member(arc: u16, per: usize, base: usize, id: usize, k: usize) -> Member {
    Member {
        arc,
        index: base + id * per + k,
    }
}

/// Adds a species made from `from` (stats, moves, sprites, icon, cry,
/// Pokedex text) under `name`. Returns its id.
pub fn add(bridge: &Bridge, from: usize, name: &str) -> Result<usize, Error> {
    let project = project_of(bridge)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::BadPath("give the new Pokemon a name".into()));
    }
    if from == 0 || (from > LAST_GAME && from < FIRST_NEW) {
        return Err(Error::BadPath(
            "start from a Pokemon of the game or one the mod adds".into(),
        ));
    }
    let id = next_id(bridge);
    if id > CEILING {
        return Err(Error::BadPath(format!("Pokemon ids stop at {CEILING}")));
    }
    if from >= id {
        return Err(Error::BadPath(format!(
            "there is no Pokemon {from} to start from"
        )));
    }
    for &(arc, per, base) in &ARCHIVES {
        for k in 0..per {
            let bytes = match arc {
                EVOLUTION_ARC => vec![0u8; 42],
                BASE_SPECIES_ARC => (id as u16).to_le_bytes().to_vec(),
                _ => bridge
                    .member(member(arc, per, base, from, k))
                    .unwrap_or_default(),
            };
            let bytes = if arc == STATS_ARC {
                own_stats(bytes)
            } else {
                bytes
            };
            project
                .write_member(arc, base + id * per + k, &bytes)
                .map_err(|e| Error::Missing(e.to_string()))?;
        }
    }
    let display = crate::msg::Display(&name.encode_utf16().collect::<Vec<_>>()).to_string();
    let old_name = line(bridge, 70, from)?;
    for &(bank, kind) in &TEXTS {
        let text = match kind {
            Line::Name => display.clone(),
            Line::Caps => display.to_uppercase(),
            Line::Copy => {
                let source = line(bridge, bank, from)?;
                if !old_name.is_empty() {
                    source
                        .replace(&old_name, &display)
                        .replace(&old_name.to_uppercase(), &display.to_uppercase())
                } else {
                    source
                }
            }
        };
        let mut lines = project.msg_lines(TEXT_ARC, bank);
        lines.insert(id, text);
        write_lines(project, bank, &lines)?;
    }
    // The icon's palette choice: the game keeps one nibble per species in
    // the ARM9 image; a new species gets a row of the eidos table
    // icon_palette ("<palette> <female palette>"), the source's values.
    let mut rows = project.table_rows(ICON_PALETTE);
    rows.insert(id, palette_row(icon_palette(bridge, from)));
    project
        .write_table_rows(ICON_PALETTE, &rows)
        .map_err(|e| Error::Missing(e.to_string()))?;
    set_limit(project, Some(id))?;
    bridge.invalidate();
    Ok(id)
}

/// The eidos table of icon palette choices (one value per species).
const ICON_PALETTE: &str = "icon_palette";

/// An icon_palette row (eidos docs/game-tables.md): "<palette> <female
/// palette>", from the game's packed byte (male low nibble, female high).
fn palette_row(byte: u8) -> Vec<u32> {
    vec![u32::from(byte & 0xF), u32::from(byte >> 4)]
}

/// A row's packed byte. One value is the palette of both (eidos's rule);
/// one value over 15 is a packed byte, the form kenon once wrote.
fn row_byte(row: &[u32]) -> u8 {
    match row {
        [m, f, ..] => ((*m & 0xF) | ((*f & 0xF) << 4)) as u8,
        [v] if *v > 15 => *v as u8,
        [v] => (*v | (*v << 4)) as u8,
        [] => 0,
    }
}

/// An icon's palettes (icon index n: species, eggs 650/651, forms from
/// 652), packed male low nibble, female high: the mod's row, else the
/// game's table.
pub fn icon_palette(bridge: &Bridge, species: usize) -> u8 {
    if let Some(b) = bridge.project().and_then(|p| {
        p.table_rows(ICON_PALETTE)
            .get(&species)
            .map(|r| row_byte(r))
    }) {
        return b;
    }
    bridge
        .code()
        .and_then(|c| {
            let (image, at) = c.table_at("icon_palette")?;
            c.read(image, at + species as u32, 1).map(|b| b[0])
        })
        .unwrap_or(0)
}

/// Sets an icon's palettes (eidos reads the row for any icon index).
/// The game's own values drop the row.
pub fn set_icon_palette(bridge: &Bridge, icon: usize, male: u8, female: u8) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    if male > 15 || female > 15 {
        return Err(Error::BadPath("a palette is 0 to 15".into()));
    }
    let game = bridge.code().and_then(|c| {
        let (image, at) = c.table_at("icon_palette")?;
        c.read(image, at + icon as u32, 1).map(|b| b[0])
    });
    let mut rows = project.table_rows(ICON_PALETTE);
    let byte = male | (female << 4);
    if game == Some(byte) {
        rows.remove(&icon);
    } else {
        rows.insert(icon, palette_row(byte));
    }
    project
        .write_table_rows(ICON_PALETTE, &rows)
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(())
}

/// A copy's stats record is its own species: no alternate forms.
fn own_stats(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.len() >= 34 {
        bytes[28..30].copy_from_slice(&0u16.to_le_bytes()); // formStatsIndex
        bytes[30..32].copy_from_slice(&0u16.to_le_bytes()); // formSpriteIndex
        bytes[32] = 1; // formCount
        bytes[33] &= !0x80; // formSpriteShared
    }
    bytes
}

/// Removes a species the mod adds: the later ones move down one id in every
/// archive and text bank, and every species id in the mod's records
/// follows (wild encounters, trainer parties, evolutions, trades...).
pub fn remove(bridge: &Bridge, id: usize) -> Result<Removed, Error> {
    let project = project_of(bridge)?;
    let end = next_id(bridge);
    if id < FIRST_NEW || id >= end {
        return Err(Error::BadPath(format!(
            "{id} is not a Pokemon the mod adds"
        )));
    }
    let last = end - 1;
    let remap = |v: i64| -> tables::Remap {
        let v = v as usize;
        if v == id {
            tables::Remap::Drop
        } else if v > id && v <= last {
            tables::Remap::To((v - 1) as i64)
        } else {
            tables::Remap::Keep
        }
    };
    let mut report = Removed {
        renumbered: (id + 1..=last).map(|v| (v, v - 1)).collect(),
        ..Removed::default()
    };
    // References first, read with the old ids.
    for t in &tables::schemas().tables {
        if !tables::editable(t) {
            continue;
        }
        for index in project.members(t.archive) {
            let Some(bytes) = tables::remap_member(bridge, &t.id, index, "species", &remap)? else {
                continue;
            };
            catalog::write_member(
                bridge,
                Member {
                    arc: t.archive,
                    index,
                },
                &bytes,
            )?;
            report.updated.push(format!("{} {index}", t.label));
        }
    }
    // The files of each later species move down, then the last one's go.
    for &(arc, per, base) in &ARCHIVES {
        for v in id + 1..=last {
            for k in 0..per {
                let bytes = bridge
                    .member(member(arc, per, base, v, k))
                    .unwrap_or_default();
                let mut bytes = bytes;
                if arc == BASE_SPECIES_ARC {
                    bytes = ((v - 1) as u16).to_le_bytes().to_vec();
                }
                project
                    .write_member(arc, base + (v - 1) * per + k, &bytes)
                    .map_err(|e| Error::Missing(e.to_string()))?;
            }
        }
        for k in 0..per {
            project
                .revert_member(arc, base + last * per + k)
                .map_err(|e| Error::Missing(e.to_string()))?;
        }
    }
    for &(bank, _) in &TEXTS {
        let lines: BTreeMap<usize, String> = project
            .msg_lines(TEXT_ARC, bank)
            .into_iter()
            .filter(|(i, _)| *i != id)
            .map(|(i, s)| (if i > id && i <= last { i - 1 } else { i }, s))
            .collect();
        write_lines(project, bank, &lines)?;
    }
    let rows: BTreeMap<usize, Vec<u32>> = project
        .table_rows(ICON_PALETTE)
        .into_iter()
        .filter(|(i, _)| *i != id)
        .map(|(i, r)| (if i > id && i <= last { i - 1 } else { i }, r))
        .collect();
    project
        .write_table_rows(ICON_PALETTE, &rows)
        .map_err(|e| Error::Missing(e.to_string()))?;
    set_limit(project, (last > FIRST_NEW).then_some(last - 1))?;
    bridge.invalidate();
    Ok(report)
}

fn line(bridge: &Bridge, bank: usize, index: usize) -> Result<String, Error> {
    Ok(catalog::text_bank(bridge, TEXT_ARC, bank)?
        .get(index)
        .cloned()
        .unwrap_or_default())
}

fn write_lines(
    project: &Project,
    bank: usize,
    lines: &BTreeMap<usize, String>,
) -> Result<(), Error> {
    project
        .write_msg_lines(TEXT_ARC, bank, lines)
        .map_err(|e| Error::Missing(e.to_string()))
}

fn set_limit(project: &Project, value: Option<usize>) -> Result<(), Error> {
    let mut fresh = Project::open(&project.root).map_err(|e| Error::Missing(e.to_string()))?;
    fresh.manifest.other.retain(|line| {
        let body = line.split('#').next().unwrap_or("");
        body.split_once('=').map(|(k, _)| k.trim()) != Some(LIMIT_KEY)
    });
    if let Some(v) = value {
        fresh.manifest.other.push(format!("{LIMIT_KEY} = {v}"));
    }
    fresh.save_meta().map_err(|e| Error::Missing(e.to_string()))
}
