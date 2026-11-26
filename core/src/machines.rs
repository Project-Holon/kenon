//! Which move each TM and HM teaches: the game's table in the ARM9 image
//! (101 machines: TM01..TM95, then HM01..HM06) and the mod's lines in
//! `eidos/tables/machine_moves.txt` (eidos docs/game-tables.md):
//!
//!     TM35 = 53
//!     HM03 = 57
//!
//! A TM item's description is its own text, so changing a machine's move
//! also rewrites that item's description to the move's.

use std::collections::BTreeMap;
use std::fs;

use serde::Serialize;

use crate::bridge::{Bridge, Error};

const FILE: &str = "eidos/tables/machine_moves.txt";
pub const COUNT: usize = 101;

/// "TM01".."TM95", "HM01".."HM06".
pub fn label(i: usize) -> String {
    if i < 95 {
        format!("TM{:02}", i + 1)
    } else {
        format!("HM{:02}", i - 94)
    }
}

fn index_of(label: &str) -> Option<usize> {
    let l = label.trim().to_uppercase();
    let n: usize = l.get(2..)?.parse().ok()?;
    match &l[..2] {
        "TM" if (1..=95).contains(&n) => Some(n - 1),
        "HM" if (1..=6).contains(&n) => Some(94 + n),
        _ => None,
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
    let Some(text) = bridge
        .project()
        .and_then(|p| fs::read_to_string(p.root.join(FILE)).ok())
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

/// The move each machine teaches in this mod.
pub fn moves(bridge: &Bridge) -> Result<Vec<u16>, Error> {
    let mut out = game(bridge)?;
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
    Ok((0..COUNT)
        .map(|i| Machine {
            label: label(i),
            item: item(i),
            mv: now[i],
            game_move: (now[i] != game[i]).then_some(game[i]),
        })
        .collect())
}

/// Sets machine i's move (the game's own drops the line) and rewrites the
/// machine item's description to the move's.
pub fn set(bridge: &Bridge, i: usize, mv: u16) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    if i >= COUNT {
        return Err(Error::BadPath(format!("no machine {i}")));
    }
    let game = game(bridge)?;
    let mut rows = lines(bridge);
    if game[i] == mv {
        rows.remove(&i);
    } else {
        rows.insert(i, mv);
    }
    let path = project.root.join(FILE);
    if rows.is_empty() {
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| Error::Missing(e.to_string()))?;
        }
    } else {
        let mut text = String::from(
            "# Written by kenon: <TM or HM> = <move id> (eidos docs/game-tables.md).\n",
        );
        for (k, m) in &rows {
            text.push_str(&format!("{} = {m}\n", label(*k)));
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| Error::Missing(e.to_string()))?;
        }
        eidos_mod::manifest::write_atomic(&path, text.as_bytes())
            .map_err(|e| Error::Missing(e.to_string()))?;
    }
    // The TM item's description (bank 53) becomes the move's (bank 202).
    let desc = crate::catalog::text_bank(bridge, 2, 202)?
        .get(mv as usize)
        .cloned()
        .unwrap_or_default();
    crate::catalog::set_text_lines(bridge, 2, &[(53, item(i), desc)])?;
    bridge.invalidate();
    Ok(())
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
        assert_eq!(index_of("TM96"), None);
        assert_eq!(
            (item(0), item(91), item(92), item(94), item(95), item(100)),
            (328, 419, 618, 620, 420, 425)
        );
    }
}
