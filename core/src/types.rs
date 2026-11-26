//! Types: the chart and new types, in eidos files (eidos item 7):
//!
//! ```text
//! eidos_mod.txt                  limit.types = <last type id>
//! eidos/tables/type_chart.txt    <attacker> = <d0> <d1> ... <dN>
//! ```
//!
//! Chart values are the game's bytes (ov093's table): 0 no effect, 2 half,
//! 4 normal, 8 double, one per defending type 0..limit. A row the mod
//! leaves alone has no line; a new type's pairs with no value are 4.
//! Names are text bank 199, line = type id, so a new name line puts the
//! type in every type list.

use std::collections::BTreeMap;

use crate::bridge::{Bridge, Error};
use crate::project::Project;

const TABLE: &str = "type_chart";
const LIMIT_KEY: &str = "limit.types";
/// The game's types: Normal 0 .. Dark 16.
pub const GAME_TYPES: usize = crate::battle::TYPE_COUNT;
/// The type ids the game's records hold (5 bits in item records, with 31
/// for none).
pub const CEILING: usize = 30;
const NAMES: usize = 199;
const VALUES: [u8; 4] = [0, 2, 4, 8];

fn project_of(bridge: &Bridge) -> Result<&std::sync::Arc<Project>, Error> {
    bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))
}

/// `limit.types` in the mod's manifest, read fresh from disk.
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

/// How many types the mod has.
pub fn count(bridge: &Bridge) -> usize {
    bridge
        .project()
        .and_then(|p| limit_of(p))
        .filter(|&l| l >= GAME_TYPES)
        .map_or(GAME_TYPES, |l| l + 1)
}

fn parse(line: &str) -> Option<Vec<u8>> {
    line.split('#')
        .next()?
        .split_whitespace()
        .map(|w| w.parse::<u8>().ok().filter(|v| VALUES.contains(v)))
        .collect()
}

/// The game's chart widened to `n` types (new pairs 4).
fn game_chart(bridge: &Bridge, n: usize) -> Result<Vec<Vec<u8>>, Error> {
    let game = crate::battle::type_chart(bridge)?;
    Ok((0..n)
        .map(|a| {
            (0..n)
                .map(|d| game.get(a).and_then(|r| r.get(d)).copied().unwrap_or(4))
                .collect()
        })
        .collect())
}

/// The chart in this mod: `chart[attacker][defender]`.
pub fn chart(bridge: &Bridge) -> Result<Vec<Vec<u8>>, Error> {
    let n = count(bridge);
    let mut out = game_chart(bridge, n)?;
    if let Some(p) = bridge.project() {
        for (a, line) in p.table_lines(TABLE) {
            if let (Some(row), Some(parsed)) = (out.get_mut(a), parse(&line)) {
                for (d, v) in parsed.into_iter().enumerate().take(n) {
                    row[d] = v;
                }
            }
        }
    }
    Ok(out)
}

fn write_chart(bridge: &Bridge, project: &Project, chart: &[Vec<u8>]) -> Result<(), Error> {
    let n = chart.len();
    let game = game_chart(bridge, n)?;
    let mut rows: BTreeMap<usize, String> = BTreeMap::new();
    for (a, row) in chart.iter().enumerate() {
        if row.len() != n || row.iter().any(|v| !VALUES.contains(v)) {
            return Err(Error::BadPath("chart values are 0, 2, 4 or 8, one per type".into()));
        }
        if *row != game[a] {
            rows.insert(a, row.iter().map(u8::to_string).collect::<Vec<_>>().join(" "));
        }
    }
    project
        .write_table_lines(
            TABLE,
            &rows,
            "<attacking type> = <0 no effect, 2 half, 4 normal, 8 double, per defending type>.",
        )
        .map_err(|e| Error::Missing(e.to_string()))
}

/// Writes the chart (rows equal to the game's have no line).
pub fn set_chart(bridge: &Bridge, chart: &[Vec<u8>]) -> Result<(), Error> {
    let project = project_of(bridge)?;
    if chart.len() != count(bridge) {
        return Err(Error::BadPath(format!("the chart has {} types", count(bridge))));
    }
    write_chart(bridge, project, chart)?;
    bridge.invalidate();
    Ok(())
}

/// Adds a type named `name`, matching up like `from` (its chart row and
/// column copied). Returns its id.
pub fn add(bridge: &Bridge, name: &str, from: usize) -> Result<usize, Error> {
    let project = project_of(bridge)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::BadPath("give the new type a name".into()));
    }
    let id = count(bridge);
    if id > CEILING {
        return Err(Error::BadPath(format!("types stop at {}", CEILING + 1)));
    }
    if from >= id {
        return Err(Error::BadPath(format!("there is no type {from}")));
    }
    let old = chart(bridge)?;
    let mut new: Vec<Vec<u8>> = old.iter().map(|r| {
        let mut r = r.clone();
        r.push(r[from]);
        r
    }).collect();
    let mut row = new[from].clone();
    row[id] = old[from][from];
    new.push(row);
    let mut lines = project.msg_lines(2, NAMES);
    lines.insert(id, name.to_string());
    project
        .write_msg_lines(2, NAMES, &lines)
        .map_err(|e| Error::Missing(e.to_string()))?;
    set_limit(project, Some(id))?;
    write_chart(bridge, project, &new)?;
    bridge.invalidate();
    Ok(id)
}

/// Removes a type the mod adds: later types move down one, in the chart,
/// the names and every record that names a type (Pokemon, moves, items...);
/// a record with the removed type gets Normal.
pub fn remove(bridge: &Bridge, id: usize) -> Result<crate::content::Removed, Error> {
    let project = project_of(bridge)?;
    let n = count(bridge);
    if id < GAME_TYPES || id >= n {
        return Err(Error::BadPath(format!("{id} is not a type the mod adds")));
    }
    let mut report = crate::content::Removed {
        renumbered: (id + 1..n).map(|v| (v, v - 1)).collect(),
        ..Default::default()
    };
    let remap = |v: i64| -> crate::tables::Remap {
        let v = v as usize;
        if v == id {
            crate::tables::Remap::To(0)
        } else if v > id && v < n {
            crate::tables::Remap::To((v - 1) as i64)
        } else {
            crate::tables::Remap::Keep
        }
    };
    for t in &crate::tables::schemas().tables {
        if !crate::tables::editable(t) {
            continue;
        }
        for index in project.members(t.archive) {
            // Types, and items' Natural Gift type (31 is none: past every
            // type, so it stays).
            for kind in ["type", "naturalGiftType"] {
                if let Some(bytes) = crate::tables::remap_member(bridge, &t.id, index, kind, &remap)? {
                    crate::catalog::write_member(
                        bridge,
                        crate::bridge::Member {
                            arc: t.archive,
                            index,
                        },
                        &bytes,
                    )?;
                    report.updated.push(format!("{} {index}", t.label));
                }
            }
        }
    }
    let old = chart(bridge)?;
    let new: Vec<Vec<u8>> = old
        .iter()
        .enumerate()
        .filter(|(a, _)| *a != id)
        .map(|(_, r)| r.iter().enumerate().filter(|(d, _)| *d != id).map(|(_, v)| *v).collect())
        .collect();
    let lines: BTreeMap<usize, String> = project
        .msg_lines(2, NAMES)
        .into_iter()
        .filter(|(i, _)| *i != id)
        .map(|(i, s)| (if i > id { i - 1 } else { i }, s))
        .collect();
    project
        .write_msg_lines(2, NAMES, &lines)
        .map_err(|e| Error::Missing(e.to_string()))?;
    set_limit(project, (n - 1 > GAME_TYPES).then(|| n - 2))?;
    write_chart(bridge, project, &new)?;
    crate::type_pictures::remove_type(project, id)?;
    bridge.invalidate();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows() {
        assert_eq!(parse("4 2 8 0"), Some(vec![4, 2, 8, 0]));
        assert_eq!(parse("4 3"), None);
    }
}
