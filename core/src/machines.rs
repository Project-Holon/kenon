//! Which move each TM and HM teaches: the game's table in the ARM9 image
//! (101 machines: TM01..TM95, then HM01..HM06) and the mod's lines in
//! `eidos/tables/machine_moves.txt` (eidos docs/game-tables.md):
//!
//! ```text
//! TM35 = 53
//! HM03 = 57
//! ```
//!
//! A TM item's description is its own text, so changing a machine's move
//! also rewrites that item's description to the move's.
//!
//! New TMs (eidos research new-tms.md): TM96 to TM122 are machines 101 to
//! 127, after the HMs, in the order of a Pokemon's TM bits (the bit is the
//! TM number plus 5; bits 101..127 are free in every record, past them is
//! the tutor word). Each is an item the mod adds, a copy of TM95's, named in
//! `eidos/tables/machine_items.txt`:
//!
//! ```text
//! TM96 = 639
//! ```

use std::collections::BTreeMap;
use std::fs;

use serde::Serialize;

use crate::bridge::{Bridge, Error};

const FILE: &str = "eidos/tables/machine_moves.txt";
const ITEMS: &str = "eidos/tables/machine_items.txt";
/// The game's machines: TM01..TM95, HM01..HM06.
pub const COUNT: usize = 101;
/// Every machine a mod can have: up to TM122 (bit 127).
pub const MAX: usize = 128;
/// TM95's item: a new TM's record is a copy of it.
const TM95_ITEM: usize = 620;

/// "TM01".."TM95", "HM01".."HM06", then "TM96".."TM122".
pub fn label(i: usize) -> String {
    if i < 95 {
        format!("TM{:02}", i + 1)
    } else if i < COUNT {
        format!("HM{:02}", i - 94)
    } else {
        format!("TM{:02}", i - 5)
    }
}

fn index_of(label: &str) -> Option<usize> {
    let l = label.trim().to_uppercase();
    let n: usize = l.get(2..)?.parse().ok()?;
    match &l[..2] {
        "TM" if (1..=95).contains(&n) => Some(n - 1),
        "TM" if (96..=122).contains(&n) => Some(n + 5),
        "HM" if (1..=6).contains(&n) => Some(94 + n),
        _ => None,
    }
}

fn read_lines(bridge: &Bridge, file: &str) -> BTreeMap<usize, usize> {
    let Some(text) = bridge
        .project()
        .and_then(|p| fs::read_to_string(p.root.join(file)).ok())
    else {
        return BTreeMap::new();
    };
    text.lines()
        .filter_map(|l| {
            let l = l.split('#').next()?;
            let (k, v) = l.split_once('=')?;
            Some((index_of(k)?, v.trim().parse().ok()?))
        })
        .collect()
}

fn write_lines(
    project: &crate::project::Project,
    file: &str,
    rows: &BTreeMap<usize, usize>,
    header: &str,
) -> Result<(), Error> {
    let path = project.root.join(file);
    if rows.is_empty() {
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| Error::Missing(e.to_string()))?;
        }
        return Ok(());
    }
    let mut text = format!("# Written by kenon: {header}\n");
    for (k, v) in rows {
        text.push_str(&format!("{} = {v}\n", label(*k)));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::Missing(e.to_string()))?;
    }
    eidos_mod::manifest::write_atomic(&path, text.as_bytes())
        .map_err(|e| Error::Missing(e.to_string()))
}

/// The mod's new TMs: machine index to item.
pub fn new_items(bridge: &Bridge) -> BTreeMap<usize, usize> {
    read_lines(bridge, ITEMS)
        .into_iter()
        .filter(|(i, _)| *i >= COUNT)
        .collect()
}

/// How many machines this mod has: the game's and its new TMs.
pub fn count(bridge: &Bridge) -> usize {
    COUNT + new_items(bridge).len()
}

/// The item of machine i in this mod.
pub fn item_of(bridge: &Bridge, i: usize) -> Option<usize> {
    if i < COUNT {
        Some(item(i))
    } else {
        new_items(bridge).get(&i).copied()
    }
}

/// The item that is machine i (file_util.c FILE_UTIL_IsMachineItem):
/// TM01..TM92 are 328..419, HM01..HM06 420..425, TM93..TM95 618..620.
pub fn item(i: usize) -> usize {
    match i {
        0..=91 => 328 + i,
        92..=94 => 618 + (i - 92),
        _ => 420 + (i - 95),
    }
}

/// Where machine i (TM01..TM95, then HM01..HM06: the order of a Pokemon's
/// TM bits, FILE_UTIL_GetId) sits in the game's move table, which is in
/// item order (FILE_UTIL_GetMachineMove): TM01..TM92, HM01..HM06, then
/// TM93..TM95.
fn table_index(i: usize) -> usize {
    match i {
        0..=91 => i,
        92..=94 => 98 + (i - 92),
        _ => 92 + (i - 95),
    }
}

/// The game's moves, machine by machine (TM01..TM95, then HM01..HM06).
fn game(bridge: &Bridge) -> Result<Vec<u16>, Error> {
    let code = bridge
        .code()
        .ok_or_else(|| Error::Missing("the game's code is not loaded".into()))?;
    let t = code
        .table("machine_moves", COUNT * 2)
        .ok_or_else(|| Error::Missing("the TM table is not known for this cartridge".into()))?;
    Ok((0..COUNT)
        .map(|i| {
            let at = table_index(i) * 2;
            u16::from_le_bytes([t[at], t[at + 1]])
        })
        .collect())
}

/// The mod's lines: machine to move.
fn lines(bridge: &Bridge) -> BTreeMap<usize, u16> {
    read_lines(bridge, FILE)
        .into_iter()
        .map(|(k, v)| (k, v as u16))
        .collect()
}

/// The move each machine teaches in this mod: the game's machines, then
/// the new TMs.
pub fn moves(bridge: &Bridge) -> Result<Vec<u16>, Error> {
    let mut out = game(bridge)?;
    out.resize(count(bridge), 0);
    for (i, m) in lines(bridge) {
        if let Some(slot) = out.get_mut(i) {
            *slot = m;
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub label: String,
    pub item: usize,
    pub mv: u16,
    /// The game's move, when the mod changes it.
    pub game_move: Option<u16>,
}

pub fn list(bridge: &Bridge) -> Result<Vec<Machine>, Error> {
    let game = game(bridge)?;
    let now = moves(bridge)?;
    Ok((0..now.len())
        .map(|i| Machine {
            label: label(i),
            item: item_of(bridge, i).unwrap_or(0),
            mv: now[i],
            game_move: game.get(i).copied().filter(|&g| g != now[i]),
        })
        .collect())
}

/// Sets machine i's move (the game's own drops the line) and rewrites the
/// machine item's description to the move's.
pub fn set(bridge: &Bridge, i: usize, mv: u16) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    if i >= count(bridge) {
        return Err(Error::BadPath(format!("no machine {i}")));
    }
    let game = game(bridge)?;
    let mut rows: BTreeMap<usize, usize> = lines(bridge)
        .into_iter()
        .map(|(k, v)| (k, v as usize))
        .collect();
    if game.get(i) == Some(&mv) {
        rows.remove(&i);
    } else {
        rows.insert(i, mv as usize);
    }
    write_lines(
        project,
        FILE,
        &rows,
        "<TM or HM> = <move id> (eidos docs/game-tables.md).",
    )?;
    // The TM item's description (bank 53) becomes the move's (bank 202).
    let desc = crate::catalog::text_bank(bridge, 2, 202)?
        .get(mv as usize)
        .cloned()
        .unwrap_or_default();
    if let Some(it) = item_of(bridge, i) {
        crate::catalog::set_text_lines(bridge, 2, &[(53, it, desc)])?;
    }
    bridge.invalidate();
    Ok(())
}

/// Adds the next new TM (TM96, TM97...) teaching `mv`: its item, a copy of
/// TM95's named after it, with the move's description. Returns its machine
/// index. Its Pokemon get it through their TM bit (index = bit).
pub fn add(bridge: &Bridge, mv: u16) -> Result<usize, Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?
        .clone();
    let i = count(bridge);
    if i >= MAX {
        return Err(Error::BadPath("TMs stop at TM122".into()));
    }
    let it = crate::content::add(bridge, "items", TM95_ITEM, &label(i))?;
    let mut items = new_items(bridge);
    items.insert(i, it);
    write_lines(
        &project,
        ITEMS,
        &items,
        "<new TM> = <its item> (eidos research new-tms.md).",
    )?;
    bridge.invalidate();
    set(bridge, i, mv)?;
    Ok(i)
}

/// Removes the last new TM: its lines, its item, and its bit in the mod's
/// Pokemon records. Returns its label.
pub fn remove_last(bridge: &Bridge) -> Result<String, Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?
        .clone();
    let i = count(bridge)
        .checked_sub(1)
        .filter(|&i| i >= COUNT)
        .ok_or_else(|| Error::BadPath("the mod adds no TM".into()))?;
    let it = new_items(bridge)[&i];
    // The bit, in every species record the mod writes.
    let word = format!("tmFlags{}", i / 32 + 1);
    let t = crate::tables::table("species")?;
    for index in project.members(t.archive) {
        let rec = crate::tables::record(bridge, "species", index, 0)?;
        let v = rec.sections.iter().find_map(|s| match s {
            crate::tables::Section::Fields { values, .. } => {
                values.iter().find(|v| v.key == word).map(|v| v.value)
            }
            _ => None,
        });
        if let Some(v) = v.filter(|v| v >> (i % 32) & 1 == 1) {
            let edit = crate::tables::Edit {
                values: [(word.clone(), v & !(1 << (i % 32)))].into_iter().collect(),
                ..Default::default()
            };
            crate::tables::save(bridge, "species", index, &edit)?;
        }
    }
    let mut moves: BTreeMap<usize, usize> = lines(bridge)
        .into_iter()
        .map(|(k, v)| (k, v as usize))
        .collect();
    moves.remove(&i);
    write_lines(
        &project,
        FILE,
        &moves,
        "<TM or HM> = <move id> (eidos docs/game-tables.md).",
    )?;
    let mut items = new_items(bridge);
    items.remove(&i);
    write_lines(
        &project,
        ITEMS,
        &items,
        "<new TM> = <its item> (eidos research new-tms.md).",
    )?;
    bridge.invalidate();
    crate::content::remove(bridge, "items", it)?;
    Ok(label(i))
}

/// After an added item is removed: new TMs whose items came after it
/// follow (their item numbers move down one); a TM whose item it was goes.
pub fn item_removed(project: &crate::project::Project, id: usize) -> Result<bool, Error> {
    let path = project.root.join(ITEMS);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(false);
    };
    let rows: BTreeMap<usize, usize> = text
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split('#').next()?.split_once('=')?;
            Some((index_of(k)?, v.trim().parse().ok()?))
        })
        .collect();
    let new: BTreeMap<usize, usize> = rows
        .iter()
        .filter(|(_, &it)| it != id)
        .map(|(&k, &it)| (k, if it > id { it - 1 } else { it }))
        .collect();
    if new == rows {
        return Ok(false);
    }
    write_lines(
        project,
        ITEMS,
        &new,
        "<new TM> = <its item> (eidos research new-tms.md).",
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_items() {
        assert_eq!(label(0), "TM01");
        assert_eq!(label(94), "TM95");
        assert_eq!(label(95), "HM01");
        assert_eq!(label(100), "HM06");
        for i in 0..COUNT {
            assert_eq!(index_of(&label(i)), Some(i));
        }
        assert_eq!(index_of("tm35"), Some(34));
        assert_eq!(index_of("TM96"), Some(101));
        assert_eq!(label(101), "TM96");
        assert_eq!(label(127), "TM122");
        assert_eq!(index_of("TM123"), None);
        assert_eq!(
            (item(0), item(91), item(92), item(94), item(95), item(100)),
            (328, 419, 618, 620, 420, 425)
        );
    }
}
