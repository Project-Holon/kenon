//! New content: moves and items after the game's last id.
//!
//! The port only raises a limit (eidos sdk/bw_limits.c): a mod declares the
//! last id it uses (`limit.moves = 560`) and supplies everything an id needs.
//! For a move or an item that is its record (a member past the end of the
//! archive), its text lines (`<bank>.msg` line files, eidos sdk/vfs_msg.h)
//! and, for an item, its row of the item icon table
//! (`eidos/tables/item_icons.txt`).
//!
//! Text banks with one line per id (counted in the ROM: a/0/0/2):
//!
//! | bank | lines per id | what                                       |
//! |------|--------------|--------------------------------------------|
//! | 203  | 1            | move names                                 |
//! | 286  | 1            | move names in capitals                     |
//! | 202  | 1            | move descriptions                          |
//! | 13   | 3            | "X used <move>!" (own, wild, foe Pokemon)  |
//! | 54   | 1            | item names                                 |
//! | 280  | 1            | item names, plural                         |
//! | 279  | 1            | item names with an article                 |
//! | 53   | 1            | item descriptions                          |

use std::collections::BTreeMap;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::catalog;
use crate::project::Project;
use crate::tables;

/// How a new id's text line is made from the entry it starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    /// The new name.
    Name,
    /// The new name in capitals.
    Caps,
    /// The source's line, with the source's name replaced by the new one.
    Copy,
}

struct Text {
    bank: usize,
    per_id: usize,
    line: Line,
}

/// A kind of content a mod can add.
pub struct Kind {
    /// The table (tables.rs) the records are in; for a kind without
    /// records (abilities), the name the interface uses.
    pub table: &'static str,
    /// Whether an id has a record in the table's archive.
    pub has_record: bool,
    /// The manifest key eidos raises the limit with; "" when there is none.
    pub limit: &'static str,
    /// The game's own last id.
    pub last: usize,
    /// The highest id the content's storage holds.
    pub ceiling: usize,
    /// The bank with the names (a/0/0/2).
    name_bank: usize,
    /// The schema enum its ids are (tables.rs, "move" or "item").
    pub enum_name: &'static str,
    texts: &'static [Text],
    /// The eidos table that needs a row per id, and the game's own table in
    /// the ARM9 image it extends (addresses.json).
    icons: Option<(&'static str, &'static str)>,
}

/// Every id of bank 13 has three lines (index id * 3 + k), and a bank holds
/// at most 65535 lines, so 21844 is the top move id.
const MOVES: Kind = Kind {
    table: "moves",
    has_record: true,
    limit: "limit.moves",
    last: 559,
    ceiling: 21844,
    name_bank: 203,
    enum_name: "move",
    texts: &[
        Text {
            bank: 203,
            per_id: 1,
            line: Line::Name,
        },
        Text {
            bank: 286,
            per_id: 1,
            line: Line::Caps,
        },
        Text {
            bank: 202,
            per_id: 1,
            line: Line::Copy,
        },
        Text {
            bank: 13,
            per_id: 3,
            line: Line::Copy,
        },
    ],
    icons: None,
};

const ITEMS: Kind = Kind {
    table: "items",
    has_record: true,
    limit: "limit.items",
    last: 626,
    ceiling: 0xFFFE,
    name_bank: 54,
    enum_name: "item",
    texts: &[
        Text {
            bank: 54,
            per_id: 1,
            line: Line::Name,
        },
        Text {
            bank: 280,
            per_id: 1,
            line: Line::Name,
        },
        Text {
            bank: 279,
            per_id: 1,
            line: Line::Copy,
        },
        Text {
            bank: 53,
            per_id: 1,
            line: Line::Copy,
        },
    ],
    icons: Some(("item_icons", "item_icons")),
};

/// Abilities have no record: an ability is its id (one byte in a Pokemon,
/// so 254 at most: 255 is the game's "none"), its texts, and the battle
/// code that tests the id.
/// Banks: 182 names, 285 names in capitals, 183 descriptions (165 lines
/// each in the ROM). eidos has no ability limit to raise.
const ABILITIES: Kind = Kind {
    table: "abilities",
    has_record: false,
    limit: "",
    last: 164,
    ceiling: 254,
    name_bank: 182,
    enum_name: "ability",
    texts: &[
        Text {
            bank: 182,
            per_id: 1,
            line: Line::Name,
        },
        Text {
            bank: 285,
            per_id: 1,
            line: Line::Caps,
        },
        Text {
            bank: 183,
            per_id: 1,
            line: Line::Copy,
        },
    ],
    icons: None,
};

/// The text archive (a/0/0/2).
const TEXT_ARC: u16 = 2;

/// The kind of content a table holds, when new entries can be added to it.
pub fn kind(table: &str) -> Option<&'static Kind> {
    [&MOVES, &ITEMS, &ABILITIES]
        .into_iter()
        .find(|k| k.table == table)
}

/// What a table's new entries look like to the interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    /// The game's last id; ids above it are the mod's.
    pub last: usize,
    /// The ids the mod adds, in order.
    pub ids: Vec<usize>,
    /// The highest id this kind can have.
    pub ceiling: usize,
}

/// The ids a mod has added to a table.
pub fn added(bridge: &Bridge, table: &str) -> Result<Added, Error> {
    let k = kind(table).ok_or_else(|| Error::BadPath(format!("{table} takes no new entries")))?;
    let count = count(bridge, k)?;
    Ok(Added {
        last: k.last,
        ids: (k.last + 1..count).collect(),
        ceiling: k.ceiling,
    })
}

/// How many entries a table has with the mod's: the game's, then the mod's
/// records and the ids its manifest declares.
pub fn count(bridge: &Bridge, k: &Kind) -> Result<usize, Error> {
    if !k.has_record {
        // The names bank with the mod's lines.
        return Ok(catalog::text_bank(bridge, TEXT_ARC, k.name_bank)?.len());
    }
    let t = tables::table(k.table)?;
    let base = bridge.base_archive(t.archive)?.len();
    let Some(p) = bridge.project() else {
        return Ok(base);
    };
    let members = p.last_member(t.archive).map_or(0, |i| i + 1);
    // The manifest on disk: add and remove write it there.
    let other =
        Project::open(&p.root).map_or_else(|_| p.manifest.other.clone(), |f| f.manifest.other);
    let declared = limit(&other, k.limit).map_or(0, |v| v + 1);
    Ok(base.max(members).max(declared))
}

/// Adds an entry after the last one, made from `from` (its record, its
/// description, its icon) with the name `name`. Returns the new id. The
/// manifest is written on disk: the caller reloads the project.
pub fn add(bridge: &Bridge, table: &str, from: usize, name: &str) -> Result<usize, Error> {
    let k = kind(table).ok_or_else(|| Error::BadPath(format!("{table} takes no new entries")))?;
    let project = project_of(bridge)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::BadPath("give the new entry a name".into()));
    }
    let id = count(bridge, k)?;
    if id > k.ceiling {
        return Err(Error::BadPath(format!(
            "{} cannot have more than {} entries",
            k.table,
            k.ceiling + 1
        )));
    }
    if from >= id {
        return Err(Error::BadPath(format!(
            "there is no entry {from} to start from"
        )));
    }
    if k.has_record {
        let t = tables::table(k.table)?;
        let record = bridge.member(Member {
            arc: t.archive,
            index: from,
        })?;
        catalog::write_member(
            bridge,
            Member {
                arc: t.archive,
                index: id,
            },
            &record,
        )?;
    }

    let old_name = line_of(bridge, k.name_bank, from)?;
    let display_name = crate::msg::Display(&name.encode_utf16().collect::<Vec<_>>()).to_string();
    for text in k.texts {
        let mut lines = project.msg_lines(TEXT_ARC, text.bank);
        for n in 0..text.per_id {
            let line = match text.line {
                Line::Name => display_name.clone(),
                Line::Caps => display_name.to_uppercase(),
                Line::Copy => {
                    let source = line_of(bridge, text.bank, from * text.per_id + n)?;
                    let caps = old_name.to_uppercase();
                    if !old_name.is_empty() && source.contains(&old_name) {
                        source.replace(&old_name, &display_name)
                    } else if !caps.is_empty() && source.contains(&caps) {
                        source.replace(&caps, &display_name.to_uppercase())
                    } else {
                        source
                    }
                }
            };
            lines.insert(id * text.per_id + n, line);
        }
        write_lines(project, text.bank, &lines)?;
    }

    if let Some((eidos_table, _)) = k.icons {
        let row = icon_row(bridge, k, from)?;
        let mut rows = project.table_rows(eidos_table);
        rows.insert(id, row);
        project
            .write_table_rows(eidos_table, &rows)
            .map_err(|e| Error::Missing(e.to_string()))?;
    }

    // The limit last: until it is written the port ignores the new id.
    set_limit(project, k, Some(id))?;
    bridge.invalidate();
    if k.table == ITEMS.table {
        sync_battle_pockets(bridge)?;
    }
    Ok(id)
}

/// What a removal changed.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    /// The added entries after the removed one, each one id down: (old, new).
    pub renumbered: Vec<(usize, usize)>,
    /// The mod's records that named the removed entry, where it is now none
    /// (or, in a learnset, egg move list or evolution, the row is gone).
    pub cleared: Vec<String>,
    /// The mod's records whose ids were renumbered.
    pub updated: Vec<String>,
}

/// Removes an entry the mod adds. The added entries after it move down one
/// id each, so no id is left empty (an empty record would be read as a move
/// or an item), and every id of the mod's own records follows: the tables,
/// evolutions, item balls, hidden items and items scripts give. A place that
/// named the removed entry is set to none.
pub fn remove(bridge: &Bridge, table: &str, id: usize) -> Result<Removed, Error> {
    let k = kind(table).ok_or_else(|| Error::BadPath(format!("{table} takes no new entries")))?;
    let project = project_of(bridge)?;
    let count = count(bridge, k)?;
    if id <= k.last || id >= count {
        return Err(Error::BadPath(format!("{id} is not an entry the mod adds")));
    }
    let last = count - 1;
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

    // The references first: they are read with the old ids.
    remap_references(bridge, project, k, &remap, &mut report)?;
    if let Some(kind) = crate::handlers::kind_of(k.table) {
        if crate::handlers::remove_id(project, kind, id).map_err(Error::Missing)? {
            report.updated.push(format!("What {kind}s do in battle"));
        }
    }
    if k.table == "items" && crate::machines::item_removed(project, id)? {
        report.updated.push("New TMs' items".into());
    }
    if k.table == "moves" && crate::move_anims::remove_id(project, id).map_err(Error::Missing)? {
        report.updated.push("Move animations".into());
    }

    // The records: each later one moves down, then the last goes.
    if k.has_record {
        let t = tables::table(k.table)?;
        for v in id + 1..=last {
            // As stored: a packed record stays packed one index down.
            let bytes = bridge.member_stored(Member {
                arc: t.archive,
                index: v,
            })?;
            project
                .write_member(t.archive, v - 1, &bytes)
                .map_err(|e| Error::Missing(e.to_string()))?;
        }
        project
            .revert_member(t.archive, last)
            .map_err(|e| Error::Missing(e.to_string()))?;
    }

    // The text lines: the removed id's go, the later ids' move down.
    for text in k.texts {
        let lines = project.msg_lines(TEXT_ARC, text.bank);
        let (lo, hi) = (id * text.per_id, (id + 1) * text.per_id);
        let shifted: BTreeMap<usize, String> = lines
            .into_iter()
            .filter(|(i, _)| !(lo..hi).contains(i))
            .map(|(i, s)| (if i >= hi { i - text.per_id } else { i }, s))
            .collect();
        write_lines(project, text.bank, &shifted)?;
    }
    if let Some((eidos_table, _)) = k.icons {
        let old = project.table_rows(eidos_table);
        let gone = old.get(&id).cloned();
        let rows: BTreeMap<usize, Vec<u32>> = old
            .into_iter()
            .filter(|(i, _)| *i != id)
            .map(|(i, r)| (if i > id { i - 1 } else { i }, r))
            .collect();
        project
            .write_table_rows(eidos_table, &rows)
            .map_err(|e| Error::Missing(e.to_string()))?;
        // Its own picture (set_item_icon) goes with it.
        let base_len = bridge.base_archive(ICON_ARC)?.len() as u32;
        if let Some(row) = gone.filter(|r| r.len() >= 2 && r[0] >= base_len) {
            if !rows.values().any(|r| r.first() == row.first()) {
                for m in [row[0], row[1]] {
                    project
                        .revert_member(ICON_ARC, m as usize)
                        .map_err(|e| Error::Missing(e.to_string()))?;
                }
            }
        }
    }
    set_limit(project, k, (last - 1 > k.last).then_some(last - 1))?;
    bridge.invalidate();
    if k.table == ITEMS.table {
        sync_battle_pockets(bridge)?;
    }
    Ok(report)
}

/// Rewrites the ids of `k`'s enum in every record the mod writes.
fn remap_references(
    bridge: &Bridge,
    project: &Project,
    k: &Kind,
    remap: &dyn Fn(i64) -> tables::Remap,
    report: &mut Removed,
) -> Result<(), Error> {
    let dropped = |v: i64| remap(v) == tables::Remap::Drop;
    for t in &tables::schemas().tables {
        if !tables::editable(t) {
            continue;
        }
        for index in project.members(t.archive) {
            let Some(bytes) = tables::remap_member(bridge, &t.id, index, k.enum_name, remap)?
            else {
                continue;
            };
            // Whether the removed entry itself was named: the same rewrite
            // with only the drop applied changes the record too.
            let named = tables::remap_member(bridge, &t.id, index, k.enum_name, &|v| {
                if dropped(v) {
                    tables::Remap::Drop
                } else {
                    tables::Remap::Keep
                }
            })?
            .is_some();
            catalog::write_member(
                bridge,
                Member {
                    arc: t.archive,
                    index,
                },
                &bytes,
            )?;
            let place = format!("{} {index}", t.label);
            if named {
                report.cleared.push(place);
            } else {
                report.updated.push(place);
            }
        }
    }
    if k.enum_name == "item" {
        remap_script_items(bridge, project, remap, report)?;
    }
    Ok(())
}

/// Item balls, hidden items and the items scripts give, in the script files
/// the mod writes.
fn remap_script_items(
    bridge: &Bridge,
    project: &Project,
    remap: &dyn Fn(i64) -> tables::Remap,
    report: &mut Removed,
) -> Result<(), Error> {
    use crate::fielditems::{self, BALL_FIRST, BALL_LAST, HIDDEN_FIRST, HIDDEN_LAST};
    let files = project.members(crate::scripts::SCRIPT_MEMBERS);
    if files.is_empty() {
        return Ok(());
    }
    for script in (BALL_FIRST..=BALL_LAST).chain(HIDDEN_FIRST..=HIDDEN_LAST) {
        let Some(e) = fielditems::event_item(bridge, script) else {
            continue;
        };
        let to = match remap(e.item as i64) {
            tables::Remap::Keep => continue,
            tables::Remap::To(v) => v as u16,
            tables::Remap::Drop => 0,
        };
        fielditems::write_event_item(bridge, script, to, None)?;
        let place = format!(
            "{} item script {script}",
            if e.kind == "ball" {
                "Item ball"
            } else {
                "Hidden item"
            }
        );
        if to == 0 {
            report.cleared.push(place);
        } else {
            report.updated.push(place);
        }
    }
    for file in files {
        let sites = crate::scripts::item_literals(bridge, file as u16)?;
        if sites.is_empty() {
            continue;
        }
        let m = Member {
            arc: crate::scripts::SCRIPT_MEMBERS,
            index: file,
        };
        let mut bytes = bridge.member(m)?;
        let (mut changed, mut named) = (false, false);
        for at in sites {
            let Some(v) = bytes
                .get(at..at + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
            else {
                continue;
            };
            let to = match remap(v as i64) {
                tables::Remap::Keep => continue,
                tables::Remap::To(x) => x as u16,
                tables::Remap::Drop => {
                    named = true;
                    0
                }
            };
            bytes[at..at + 2].copy_from_slice(&to.to_le_bytes());
            changed = true;
        }
        if changed {
            catalog::write_member(bridge, m, &bytes)?;
            let place = format!("Script file {file}");
            if named {
                report.cleared.push(place);
            } else {
                report.updated.push(place);
            }
        }
    }
    Ok(())
}

/// The item icon archive (a/0/2/5) and its cell bank (member 1: every icon
/// is one 32 x 32 cell).
const ICON_ARC: u16 = 25;
const ICON_CELLS: usize = 1;

/// Gives an added item a picture of its own: two new members at the end of
/// a/0/2/5 (the tiles and the palette, made from the icon it shows now),
/// the PNG laid over them with its own colours, and the item's icon row
/// pointed at them. A second call replaces the picture in place.
pub fn set_item_icon(bridge: &Bridge, item: usize, png: &[u8]) -> Result<Vec<u32>, Error> {
    let k = &ITEMS;
    let project = project_of(bridge)?;
    if item <= k.last || item >= count(bridge, k)? {
        return Err(Error::BadPath(format!(
            "{item} is not an item the mod adds; change a game item's icon in Item icons"
        )));
    }
    let base_len = bridge.base_archive(ICON_ARC)?.len();
    let row = icon_row(bridge, k, item)?;
    let (chr, pal) = (row[0] as usize, row[1] as usize);
    // Its own members already (past the game's, used by no other row).
    let rows = project.table_rows("item_icons");
    let shared = rows
        .iter()
        .any(|(id, r)| *id != item && r.first().map(|&c| c as usize) == Some(chr));
    let (chr, pal) = if chr >= base_len && pal == chr + 1 && !shared {
        (chr, pal)
    } else {
        let at = base_len.max(project.last_member(ICON_ARC).map_or(0, |i| i + 1));
        for (from, to) in [(chr, at), (pal, at + 1)] {
            let bytes = bridge.member(Member {
                arc: ICON_ARC,
                index: from,
            })?;
            catalog::write_member(
                bridge,
                Member {
                    arc: ICON_ARC,
                    index: to,
                },
                &bytes,
            )?;
        }
        (at, at + 1)
    };
    crate::import::import_picture(
        bridge,
        Member {
            arc: ICON_ARC,
            index: chr,
        },
        Member {
            arc: ICON_ARC,
            index: pal,
        },
        &catalog::Layout::Cells {
            cer: [ICON_ARC as usize, ICON_CELLS],
        },
        png,
        crate::import::PaletteMode::Replace,
        0,
    )?;
    let mut rows = rows;
    let new_row = vec![chr as u32, pal as u32];
    rows.insert(item, new_row.clone());
    project
        .write_table_rows("item_icons", &rows)
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(new_row)
}

/// An item's icon row: tile member and palette member in a/0/2/5. The
/// mod's row, else the game's (ARM9 table `item_icons`), else item 0's,
/// which is what the port gives an id with no row.
pub fn icon_row(bridge: &Bridge, k: &Kind, id: usize) -> Result<Vec<u32>, Error> {
    let Some((eidos_table, arm9_table)) = k.icons else {
        return Err(Error::BadPath(format!("{} have no icons", k.table)));
    };
    if let Some(row) = bridge
        .project()
        .and_then(|p| p.table_rows(eidos_table).remove(&id))
    {
        return Ok(row);
    }
    let code = bridge
        .code()
        .ok_or_else(|| Error::Missing("the game's code is not loaded".into()))?;
    let at = if id <= k.last { id } else { 0 };
    let (image, base) = code
        .table_at(arm9_table)
        .ok_or_else(|| Error::Missing(format!("no address for {arm9_table}")))?;
    let pair = code
        .read(image, base + at as u32 * 4, 4)
        .ok_or_else(|| Error::Missing(format!("{arm9_table} cannot be read")))?;
    Ok(vec![
        u16::from_le_bytes([pair[0], pair[1]]) as u32,
        u16::from_le_bytes([pair[2], pair[3]]) as u32,
    ])
}

/// The value of a `key = value` line of the manifest's other lines.
pub fn limit(other: &[String], key: &str) -> Option<usize> {
    other.iter().rev().find_map(|line| {
        let body = line.split('#').next()?;
        let (k, v) = body.split_once('=')?;
        (k.trim() == key).then(|| v.trim().parse().ok()).flatten()
    })
}

/// Sets (or with `None` removes) the kind's limit line in the manifest on
/// disk.
fn set_limit(project: &Project, k: &Kind, value: Option<usize>) -> Result<(), Error> {
    if k.limit.is_empty() {
        return Ok(());
    }
    let mut fresh = Project::open(&project.root).map_err(|e| Error::Missing(e.to_string()))?;
    fresh.manifest.other.retain(|line| {
        let body = line.split('#').next().unwrap_or("");
        body.split_once('=').map(|(key, _)| key.trim()) != Some(k.limit)
    });
    if let Some(v) = value {
        fresh.manifest.other.push(format!("{} = {v}", k.limit));
    }
    fresh.save_meta().map_err(|e| Error::Missing(e.to_string()))
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

/// One line of a bank in the display form, with the mod's lines.
fn line_of(bridge: &Bridge, bank: usize, index: usize) -> Result<String, Error> {
    Ok(catalog::text_bank(bridge, TEXT_ARC, bank)?
        .get(index)
        .cloned()
        .unwrap_or_default())
}

/// a/2/3/2 (battle_pocket.dat): one byte per item, the battle bag pockets it
/// shows in. The battle bag reads only this (B_BAG_MAIN_BuildPocketLists);
/// the same bits sit in each item record (bits 11-15 of the halfword at 8),
/// and in the game the two agree for every item.
const BATTLE_POCKET_ARC: u16 = 232;

/// Rebuilds a/2/3/2 from the item records, so the record's pocket bits are
/// what the battle bag uses and new items have a byte (without one the bag
/// reads past the table's end). The game's own member is kept when nothing
/// differs.
pub fn sync_battle_pockets(bridge: &Bridge) -> Result<(), Error> {
    let Some(project) = bridge.project() else {
        return Ok(());
    };
    let t = tables::table(ITEMS.table)?;
    let n = count(bridge, &ITEMS)?;
    let mut out = Vec::with_capacity(n);
    for index in 0..n {
        let r = bridge.member(Member {
            arc: t.archive,
            index,
        })?;
        out.push(
            r.get(8..10)
                .map_or(0, |h| (u16::from_le_bytes([h[0], h[1]]) >> 11) as u8),
        );
    }
    let base = bridge
        .base_archive(BATTLE_POCKET_ARC)?
        .member(0, false)
        .map_err(|e| Error::Missing(e.to_string()))?;
    let r = if out == base {
        project.revert_member(BATTLE_POCKET_ARC, 0)
    } else {
        project.write_member(BATTLE_POCKET_ARC, 0, &out)
    };
    r.map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(())
}

fn project_of(bridge: &Bridge) -> Result<&std::sync::Arc<Project>, Error> {
    bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open: new entries need a mod to go in".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_lines() {
        let other = vec![
            "# a comment".to_string(),
            "limit.moves = 560".to_string(),
            "limit.items=700 # more".to_string(),
        ];
        assert_eq!(limit(&other, "limit.moves"), Some(560));
        assert_eq!(limit(&other, "limit.items"), Some(700));
        assert_eq!(limit(&other, "limit.species"), None);
    }

    #[test]
    fn kinds() {
        assert!(kind("moves").is_some());
        assert!(kind("items").is_some());
        assert!(kind("species").is_none());
        // Bank 13 has three lines per move: the top id keeps the bank
        // under 65535 lines.
        const { assert!(MOVES.ceiling * 3 + 2 < 65535) };
    }
}
