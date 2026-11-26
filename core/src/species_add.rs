//! New Pokemon: a species added after the game's own, copied from one that
//! exists, in every archive a species lives in.
//!
//! Ids. The game reads a species' files at member = id, and ids 650..714
//! are taken (eggs, alternate forms, the Substitute doll: eidos
//! docs/research/limits/species-data.md, species-assets.md). New species
//! start at 715, the first id whose sprite and icon members lie past the
//! end of their archives, so nothing has to move. Ids 652..714 stay unused.
//!
//! Forms. A new species' forms take slots after the mod's last species
//! (see [`Layout`]), in every archive, so species ids stay one run. Adding
//! a species moves every form slot up one; removing one moves them down.
//! The stats record's first-form fields point at them.
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
//! article, 260 category, 235 and 236 Pokedex entry. Form name, height and
//! weight (banks 244, 245, 268) have 727 lines, the game's species then its
//! forms; a slot the mod adds is line 727 + (slot - 715), as eidos reads it
//! (DEX_BANKS).
//!
//! eidos files: `limit.species` (the last species), `icon_palette.txt` (a
//! row per slot) and `cries.txt` (crate::cries).

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
/// Species ids are 11 bits in wild and trainer records.
pub const CEILING: usize = 2047;
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

/// The mod's slots: species FIRST_NEW..=last, one id each, then the form
/// slots of each species in species order. Form f (1 and up) of species S
/// is slot F + f - 1 in every archive, F being S's stats record's first
/// form (formStatsIndex, with formSpriteIndex equal to it: eidos's rule
/// for species past 714).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// The last species the mod adds (FIRST_NEW - 1 when none).
    pub last: usize,
    /// Each species' form slots, form 1 first.
    pub forms: BTreeMap<usize, Vec<usize>>,
}

impl Layout {
    /// One past the highest slot in use.
    fn end(&self) -> usize {
        self.forms
            .values()
            .flatten()
            .map(|s| s + 1)
            .max()
            .unwrap_or(0)
            .max(self.last + 1)
            .max(FIRST_NEW)
    }

    /// The species and form (1 and up) a form slot holds.
    pub fn form_at(&self, slot: usize) -> Option<(usize, usize)> {
        self.forms
            .iter()
            .find_map(|(&s, v)| v.iter().position(|&x| x == slot).map(|i| (s, i + 1)))
    }
}

/// `limit.species` in the mod's manifest, read fresh from disk.
fn limit_of(project: &Project) -> Option<usize> {
    let fresh = Project::open(&project.root).ok()?;
    fresh.manifest.other.iter().find_map(|line| {
        let (k, v) = line.split('#').next()?.split_once('=')?;
        if k.trim() == LIMIT_KEY {
            v.trim().parse().ok()
        } else {
            None
        }
    })
}

/// Where the mod's species and forms are.
pub fn layout(bridge: &Bridge) -> Layout {
    let Some(project) = bridge.project() else {
        return Layout {
            last: FIRST_NEW - 1,
            forms: BTreeMap::new(),
        };
    };
    let last = limit_of(project)
        .filter(|&l| l >= FIRST_NEW)
        .or_else(|| project.last_member(STATS_ARC).filter(|&m| m >= FIRST_NEW))
        .unwrap_or(FIRST_NEW - 1);
    let mut forms = BTreeMap::new();
    for s in FIRST_NEW..=last {
        let Ok(rec) = bridge.member(Member {
            arc: STATS_ARC,
            index: s,
        }) else {
            continue;
        };
        let n = rec.get(32).copied().unwrap_or(1) as usize;
        let first = rec
            .get(28..30)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
        if n > 1 && first > last {
            forms.insert(s, (first..first + n - 1).collect());
        }
    }
    Layout { last, forms }
}

/// The species ids the mod adds (715 and up), then its form slots.
pub fn added(bridge: &Bridge) -> Vec<usize> {
    let l = layout(bridge);
    let mut out: Vec<usize> = (FIRST_NEW..=l.last).collect();
    out.extend(l.forms.values().flatten());
    out
}

/// The id the next new species gets.
fn next_id(bridge: &Bridge) -> usize {
    layout(bridge).last + 1
}

/// Every member of one slot, in ARCHIVES order.
type SlotData = Vec<Vec<u8>>;

/// Where an archive's members start in a SlotData.
fn pos(arc: u16) -> usize {
    let mut at = 0;
    for &(a, per, _) in &ARCHIVES {
        if a == arc {
            return at;
        }
        at += per;
    }
    unreachable!("archive {arc} is not a species archive")
}

/// A slot's members as stored, so a copy keeps the game's packing (battle
/// sprites and Pokedex icons are LZ packed, party icons raw). The records
/// this module edits (stats, evolutions, base species) are raw in the game
/// and are read unpacked.
fn read_slot(bridge: &Bridge, slot: usize) -> SlotData {
    let mut out = Vec::new();
    for &(arc, per, base) in &ARCHIVES {
        for k in 0..per {
            let m = member(arc, per, base, slot, k);
            let bytes = if matches!(arc, STATS_ARC | EVOLUTION_ARC | BASE_SPECIES_ARC) {
                bridge.member(m)
            } else {
                bridge.member_stored(m)
            };
            out.push(bytes.unwrap_or_default());
        }
    }
    out
}

/// A slot's new contents: where they come from (the slot itself when
/// unmoved, so unchanged members are not written again) and its icon
/// palette row.
struct Planned {
    src: Option<usize>,
    data: SlotData,
    palette: Vec<u32>,
    /// Its lines in DEX_BANKS.
    dex: Vec<String>,
}

fn planned(bridge: &Bridge, project: &Project, slot: usize) -> Planned {
    Planned {
        src: Some(slot),
        data: read_slot(bridge, slot),
        palette: project
            .table_rows(ICON_PALETTE)
            .get(&slot)
            .cloned()
            .unwrap_or_else(|| palette_row(icon_palette(bridge, slot))),
        dex: dex_lines(bridge, slot),
    }
}

/// The Pokedex banks of 727 lines: the game's species 0..649, then its
/// forms (form name, height, weight). eidos reads a slot the mod adds
/// (715 and up) at line 727 + (slot - 715).
const DEX_BANKS: [usize; 3] = [244, 245, 268];
const DEX_GAME_LINES: usize = 727;

/// A slot's line in the DEX_BANKS: a game species' own, a mod slot's past
/// the game's lines.
fn dex_line(slot: usize) -> usize {
    if slot >= FIRST_NEW {
        DEX_GAME_LINES + slot - FIRST_NEW
    } else {
        slot
    }
}

fn dex_lines(bridge: &Bridge, slot: usize) -> Vec<String> {
    DEX_BANKS
        .iter()
        .map(|&b| line(bridge, b, dex_line(slot)).unwrap_or_default())
        .collect()
}

/// The plan for the layout as it is: every species and its forms.
fn plan_of(bridge: &Bridge, project: &Project, l: &Layout) -> Vec<(Planned, Vec<Planned>)> {
    (FIRST_NEW..=l.last)
        .map(|s| {
            let forms = l
                .forms
                .get(&s)
                .map(|v| v.iter().map(|&f| planned(bridge, project, f)).collect())
                .unwrap_or_default();
            (planned(bridge, project, s), forms)
        })
        .collect()
}

/// A stats record's form fields: the first form's slot (0: none) and the
/// number of forms, the base one included.
fn set_forms(stats: &mut [u8], first: usize, count: usize) {
    if stats.len() >= 34 {
        stats[28..30].copy_from_slice(&(first as u16).to_le_bytes()); // formStatsIndex
        stats[30..32].copy_from_slice(&(first as u16).to_le_bytes()); // formSpriteIndex
        stats[32] = count as u8; // formCount
        stats[33] &= !0x80; // formSpriteShared
    }
}

/// Writes the mod's slots as `plan` lays them out: species at 715 on, then
/// each species' forms. Fixes each stats record's form fields, each base
/// species member and the icon palette rows, reverts the slots past the
/// new end and sets `limit.species`. All of `plan` is read before this.
fn apply(
    bridge: &Bridge,
    project: &Project,
    plan: Vec<(Planned, Vec<Planned>)>,
    old_end: usize,
) -> Result<(), Error> {
    let io = |e: std::io::Error| Error::Missing(e.to_string());
    let count = plan.len();
    let mut next = FIRST_NEW + count;
    let mut out: Vec<(usize, Planned)> = Vec::new();
    for (i, (mut sp, forms)) in plan.into_iter().enumerate() {
        let id = FIRST_NEW + i;
        let first = if forms.is_empty() { 0 } else { next };
        set_forms(&mut sp.data[pos(STATS_ARC)], first, forms.len() + 1);
        sp.data[pos(BASE_SPECIES_ARC)] = (id as u16).to_le_bytes().to_vec();
        out.push((id, sp));
        for mut f in forms {
            set_forms(&mut f.data[pos(STATS_ARC)], 0, 1);
            f.data[pos(BASE_SPECIES_ARC)] = (id as u16).to_le_bytes().to_vec();
            out.push((next, f));
            next += 1;
        }
    }
    let mut rows: BTreeMap<usize, Vec<u32>> = project
        .table_rows(ICON_PALETTE)
        .into_iter()
        .filter(|(i, _)| *i < FIRST_NEW)
        .collect();
    for (slot, p) in &out {
        let same = (p.src == Some(*slot)).then(|| read_slot(bridge, *slot));
        let mut at = 0;
        for &(arc, per, base) in &ARCHIVES {
            for k in 0..per {
                let bytes = &p.data[at];
                if same.as_ref().map_or(true, |s| &s[at] != bytes) {
                    project
                        .write_member(arc, base + slot * per + k, bytes)
                        .map_err(io)?;
                }
                at += 1;
            }
        }
        rows.insert(*slot, p.palette.clone());
    }
    for slot in next..old_end {
        for &(arc, per, base) in &ARCHIVES {
            for k in 0..per {
                project.revert_member(arc, base + slot * per + k).map_err(io)?;
            }
        }
    }
    project.write_table_rows(ICON_PALETTE, &rows).map_err(io)?;
    // The Pokedex lines: the game's 727, then one per slot in slot order.
    for (k, &bank) in DEX_BANKS.iter().enumerate() {
        let mut lines: BTreeMap<usize, String> = project
            .msg_lines(TEXT_ARC, bank)
            .into_iter()
            .filter(|(i, _)| *i < DEX_GAME_LINES)
            .collect();
        for (slot, p) in &out {
            lines.insert(dex_line(*slot), p.dex.get(k).cloned().unwrap_or_default());
        }
        write_lines(project, bank, &lines)?;
    }
    set_limit(project, (count > 0).then(|| FIRST_NEW + count - 1))
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
    let l = layout(bridge);
    let id = l.last + 1;
    if l.end() + 1 > CEILING + 1 {
        return Err(Error::BadPath(format!("Pokemon ids stop at {CEILING}")));
    }
    if from >= id {
        return Err(Error::BadPath(format!(
            "there is no Pokemon {from} to start from"
        )));
    }
    // The new species goes after the others; the form slots after it move
    // up one.
    let mut plan = plan_of(bridge, project, &l);
    let mut data = read_slot(bridge, from);
    data[pos(EVOLUTION_ARC)] = vec![0u8; 42];
    plan.push((
        Planned {
            src: None,
            data,
            palette: palette_row(icon_palette(bridge, from)),
            // The source's height and weight; a species has no form name.
            dex: {
                let mut d = dex_lines(bridge, from);
                d[0].clear();
                d
            },
        },
        Vec::new(),
    ));
    apply(bridge, project, plan, l.end())?;
    // Its cry: the source's.
    let cry = crate::cries::current(project, crate::cries::Key { species: from, form: 0 });
    crate::cries::set(project, crate::cries::Key { species: id, form: 0 }, Some(cry))
        .map_err(Error::Missing)?;
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
    bridge.invalidate();
    Ok(id)
}

/// The tables whose records name a species and its form.
const FORM_TABLES: [&str; 4] = ["encounters", "trainerParties", "battleFacilityMons", "trades"];

/// Adds a form to a species the mod adds, made from `from`: a game
/// species (its base form) or any slot the mod uses (a species or a
/// form). Returns the new form's number (1 and up). The form slots after
/// it move up one.
pub fn add_form(bridge: &Bridge, species: usize, from: usize) -> Result<usize, Error> {
    let project = project_of(bridge)?;
    let l = layout(bridge);
    if !(FIRST_NEW..=l.last).contains(&species) {
        return Err(Error::BadPath(format!(
            "{species} is not a Pokemon the mod adds"
        )));
    }
    let mod_slot = from >= FIRST_NEW && from < l.end() && (from <= l.last || l.form_at(from).is_some());
    if from == 0 || !(from <= LAST_GAME || mod_slot) {
        return Err(Error::BadPath(
            "start from a Pokemon of the game or a Pokemon or form the mod adds".into(),
        ));
    }
    if l.end() > CEILING {
        return Err(Error::BadPath(format!("Pokemon ids stop at {CEILING}")));
    }
    let mut plan = plan_of(bridge, project, &l);
    let mut data = read_slot(bridge, from);
    data[pos(EVOLUTION_ARC)] = vec![0u8; 42];
    let forms = &mut plan[species - FIRST_NEW].1;
    if forms.len() >= 254 {
        return Err(Error::BadPath("a Pokemon has at most 255 forms".into()));
    }
    forms.push(Planned {
        src: None,
        data,
        palette: palette_row(icon_palette(bridge, from)),
        // The source's form name (empty for a species), height and weight.
        dex: dex_lines(bridge, from),
    });
    let form = forms.len();
    apply(bridge, project, plan, l.end())?;
    bridge.invalidate();
    Ok(form)
}

/// Removes form `form` (1 and up) of a species the mod adds. Its later
/// forms move down one, in the mod's records too (wild Pokemon, trainer
/// parties, trades): a record with the removed form gets the base form.
pub fn remove_form(bridge: &Bridge, species: usize, form: usize) -> Result<Removed, Error> {
    let project = project_of(bridge)?;
    let l = layout(bridge);
    let count = l.forms.get(&species).map_or(0, Vec::len);
    if !(FIRST_NEW..=l.last).contains(&species) || form == 0 || form > count {
        return Err(Error::BadPath(format!("Pokemon {species} has no form {form}")));
    }
    let mut report = Removed::default();
    // References first, read with the old form numbers.
    let map = |v: i64| -> tables::Remap {
        let v = v as usize;
        if v == form {
            tables::Remap::Drop
        } else if v > form {
            tables::Remap::To((v - 1) as i64)
        } else {
            tables::Remap::Keep
        }
    };
    for id in FORM_TABLES {
        let t = tables::table(id)?;
        for index in project.members(t.archive) {
            if let Some(bytes) = tables::remap_forms(bridge, id, index, species as i64, &map)? {
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
    }
    let mut plan = plan_of(bridge, project, &l);
    plan[species - FIRST_NEW].1.remove(form - 1);
    apply(bridge, project, plan, l.end())?;
    crate::cries::remove_form(project, species, form).map_err(Error::Missing)?;
    bridge.invalidate();
    Ok(report)
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
    // The slots: the species and its forms go, the later species and every
    // form slot move down.
    bridge.invalidate();
    let l = layout(bridge);
    let mut plan = plan_of(bridge, project, &l);
    plan.remove(id - FIRST_NEW);
    apply(bridge, project, plan, l.end())?;
    crate::cries::remove_species(project, id).map_err(Error::Missing)?;
    crate::regional_dex::remove_species(project, id).map_err(Error::Missing)?;
    for &(bank, _) in &TEXTS {
        let lines: BTreeMap<usize, String> = project
            .msg_lines(TEXT_ARC, bank)
            .into_iter()
            .filter(|(i, _)| *i != id)
            .map(|(i, s)| (if i > id && i <= last { i - 1 } else { i }, s))
            .collect();
        write_lines(project, bank, &lines)?;
    }
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
