//! Wild Pokemon forms past the encounter record's form field (eidos
//! `eidos/tables/wild_forms.txt`, 0.2.8):
//!
//! ```text
//! 77.grass.3 = 40
//! 77.surf.* = 33
//! 12.fishing.0 = 2 season=winter
//! ```
//!
//! The key is the a/1/2/6 member, the slot group and the slot (`*` for
//! every slot); the value is the form, 0..255. A line wins over the slot's
//! own form field (31 included), a `season=` line over one without, and
//! a later line over an earlier one. A species without that form gets
//! form 0.
//!
//! The editor shows the form a slot plays as and kenon picks where it
//! goes: the record's field when the field can hold it and no other line
//! covers the slot, else a line of its own (with `season=` in a
//! four-season table). The field then holds the form for an eidos without
//! the table (0 past 31).

use std::fs;

use crate::bridge::{Bridge, Error};
use crate::project::Project;
use crate::tables::{self, Edit, Record, Remap, Section};

const TABLE: &str = "wild_forms";
const HEADER: &str = "# Written by kenon: <table>.<group>.<slot> = <form> [season=<season>]\n";
/// The slot groups of an encounter record.
pub const GROUPS: [&str; 7] = [
    "grass",
    "doubleGrass",
    "shakingGrass",
    "surf",
    "surfRipple",
    "fishing",
    "fishingRipple",
];
const SEASONS: [&str; 4] = ["spring", "summer", "autumn", "winter"];
/// The most a record's form field holds (31 rolls a random form).
const FIELD_MAX: i64 = 31;
pub const MAX_FORM: i64 = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub table: usize,
    pub group: String,
    /// None: every slot (`*`).
    pub slot: Option<usize>,
    pub season: Option<usize>,
    pub form: i64,
}

fn parse_line(text: &str) -> Option<Line> {
    let (key, value) = text.split('#').next()?.split_once('=')?;
    let mut k = key.trim().split('.');
    let table = k.next()?.trim().parse().ok()?;
    let group = k.next()?.trim();
    let slot = match k.next()?.trim() {
        "*" => None,
        s => Some(s.parse().ok()?),
    };
    if k.next().is_some() || !GROUPS.contains(&group) {
        return None;
    }
    let mut words = value.split_whitespace();
    let form: i64 = words.next()?.parse().ok()?;
    let mut season = None;
    for w in words {
        let (name, v) = w.split_once('=')?;
        if name != "season" {
            return None;
        }
        season = Some(
            SEASONS
                .iter()
                .position(|s| s.eq_ignore_ascii_case(v))
                .or_else(|| v.parse().ok().filter(|n| *n < 4))?,
        );
    }
    Some(Line {
        table,
        group: group.to_string(),
        slot,
        season,
        form: form.clamp(0, MAX_FORM),
    })
}

fn format_line(l: &Line) -> String {
    let slot = l.slot.map_or("*".to_string(), |s| s.to_string());
    let season = l
        .season
        .map_or(String::new(), |s| format!(" season={}", SEASONS[s]));
    format!("{}.{}.{slot} = {}{season}", l.table, l.group, l.form)
}

/// The mod's lines, in file order. Lines kenon cannot read are kept as
/// they are.
pub fn read(project: &Project) -> (Vec<Line>, Vec<String>) {
    let text = fs::read_to_string(project.table_path(TABLE)).unwrap_or_default();
    let mut lines = Vec::new();
    let mut other = Vec::new();
    for t in text.lines() {
        let bare = t.split('#').next().unwrap_or("").trim();
        if bare.is_empty() {
            continue;
        }
        match parse_line(t) {
            Some(l) => lines.push(l),
            None => other.push(t.to_string()),
        }
    }
    (lines, other)
}

fn write(project: &Project, lines: &[Line], other: &[String]) -> Result<(), Error> {
    let path = project.table_path(TABLE);
    let io = |e: std::io::Error| Error::Missing(e.to_string());
    if lines.is_empty() && other.is_empty() {
        if path.is_file() {
            fs::remove_file(&path).map_err(io)?;
        }
        return Ok(());
    }
    let mut out = HEADER.to_string();
    for l in other {
        out.push_str(l);
        out.push('\n');
    }
    for l in lines {
        out.push_str(&format_line(l));
        out.push('\n');
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    fs::write(path, out).map_err(io)
}

fn covers(l: &Line, table: usize, group: &str, slot: usize) -> bool {
    l.table == table && l.group == group && l.slot.is_none_or(|s| s == slot)
}

/// The form a slot plays as from the lines, when one covers it: a
/// `season=` line for `season` first, then one without; the later wins.
pub fn effective<'a>(
    lines: impl Iterator<Item = &'a Line> + Clone,
    table: usize,
    group: &str,
    slot: usize,
    season: Option<usize>,
) -> Option<i64> {
    let last = |want: Option<usize>| {
        lines
            .clone()
            .filter(|l| covers(l, table, group, slot) && l.season == want)
            .last()
            .map(|l| l.form)
    };
    season.and_then(|s| last(Some(s))).or_else(|| last(None))
}

/// The season a line of this block takes: None for a one-season table.
fn season_of(seasons: usize, season: usize) -> Option<usize> {
    (seasons > 1).then_some(season.min(seasons - 1))
}

fn form_column(columns: &[tables::Column]) -> Option<usize> {
    columns.iter().position(|c| c.name == "form")
}

/// Shows the forms the lines give in an encounter record's slots.
pub fn overlay(project: &Project, index: usize, season: usize, record: &mut Record) {
    let (lines, _) = read(project);
    if lines.is_empty() {
        return;
    }
    let season = season_of(record.seasons, season);
    for s in &mut record.sections {
        let Section::List {
            key, columns, rows, ..
        } = s
        else {
            continue;
        };
        let Some(fc) = form_column(columns) else {
            continue;
        };
        for (i, row) in rows.iter_mut().enumerate() {
            if let Some(f) = effective(lines.iter(), index, key, i, season) {
                row[fc] = f;
            }
        }
    }
}

/// Splits the forms of an encounter edit between the record's fields and
/// the mod's lines, and writes the lines. `seasons` is how many season
/// blocks the member holds.
pub fn store(bridge: &Bridge, index: usize, seasons: usize, edit: &mut Edit) -> Result<(), Error> {
    let Some(project) = bridge.project() else {
        return Ok(());
    };
    let (mut lines, other) = read(project);
    let before = lines.clone();
    let season = season_of(seasons, edit.season);
    let record = tables::record(bridge, "encounters", index, edit.season)?;
    for s in &record.sections {
        let Section::List { key, columns, .. } = s else {
            continue;
        };
        let (Some(fc), Some(rows)) = (form_column(columns), edit.lists.get_mut(key)) else {
            continue;
        };
        for (slot, row) in rows.iter_mut().enumerate() {
            let Some(f) = row.get(fc).copied() else {
                continue;
            };
            let f = f.clamp(0, MAX_FORM);
            // The slot's own line goes; then the others decide.
            lines.retain(|l| {
                !(l.slot == Some(slot) && l.season == season && covers(l, index, key, slot))
            });
            let rest = effective(lines.iter(), index, key, slot, season);
            if rest != Some(f) && (rest.is_some() || f > FIELD_MAX) {
                lines.push(Line {
                    table: index,
                    group: key.clone(),
                    slot: Some(slot),
                    season,
                    form: f,
                });
            }
            row[fc] = if f > FIELD_MAX { 0 } else { f };
        }
    }
    if lines != before {
        write(project, &lines, &other)?;
    }
    Ok(())
}

/// Renumbers the forms of `species` in the mod's lines by `map` (a
/// dropped form becomes 0). A line follows the species its slots hold.
pub fn remap(bridge: &Bridge, species: i64, map: &dyn Fn(i64) -> Remap) -> Result<bool, Error> {
    let Some(project) = bridge.project() else {
        return Ok(false);
    };
    let (mut lines, other) = read(project);
    let mut changed = false;
    for l in &mut lines {
        let seasons = tables::record(bridge, "encounters", l.table, 0).map_or(1, |r| r.seasons);
        let blocks: Vec<usize> = match l.season {
            Some(s) if seasons > 1 => vec![s.min(seasons - 1)],
            _ => (0..seasons).collect(),
        };
        let mut holds = false;
        for b in blocks {
            let Ok(r) = tables::record(bridge, "encounters", l.table, b) else {
                continue;
            };
            for s in &r.sections {
                let Section::List {
                    key, columns, rows, ..
                } = s
                else {
                    continue;
                };
                let Some(sc) = columns.iter().position(|c| c.name == "species") else {
                    continue;
                };
                if *key == l.group
                    && rows
                        .iter()
                        .enumerate()
                        .any(|(i, r)| l.slot.is_none_or(|s| s == i) && r.get(sc) == Some(&species))
                {
                    holds = true;
                }
            }
        }
        if !holds {
            continue;
        }
        let to = match map(l.form) {
            Remap::Keep => continue,
            Remap::To(v) => v,
            Remap::Drop => 0,
        };
        if to != l.form {
            l.form = to;
            changed = true;
        }
    }
    if changed {
        write(project, &lines, &other)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_precedence() {
        let l = parse_line("12.fishing.0 = 2 season=winter").unwrap();
        assert_eq!(
            (l.table, l.slot, l.season, l.form),
            (12, Some(0), Some(3), 2)
        );
        assert_eq!(format_line(&l), "12.fishing.0 = 2 season=winter");
        assert_eq!(parse_line("77.surf.* = 33").unwrap().slot, None);
        assert_eq!(
            parse_line("77.grass.3 = 4 season=1").unwrap().season,
            Some(1)
        );
        assert!(parse_line("77.lava.3 = 4").is_none());
        let lines = [
            parse_line("77.grass.* = 33").unwrap(),
            parse_line("77.grass.3 = 40").unwrap(),
            parse_line("77.grass.3 = 5 season=winter").unwrap(),
        ];
        assert_eq!(effective(lines.iter(), 77, "grass", 3, None), Some(40));
        assert_eq!(effective(lines.iter(), 77, "grass", 3, Some(3)), Some(5));
        assert_eq!(effective(lines.iter(), 77, "grass", 2, Some(3)), Some(33));
        assert_eq!(effective(lines.iter(), 77, "surf", 2, None), None);
    }
}
