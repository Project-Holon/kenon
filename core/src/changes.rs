//! What a mod changes, described for people: each changed file with a
//! title and the values that differ from the game.

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::lz;
use crate::tables::{self, Section};

/// One value that differs: what it is, the game's value and the mod's.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub label: String,
    pub old: String,
    pub new: String,
}

/// A changed file, described.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeInfo {
    pub path: String,
    /// The member, for a NARC member override.
    pub member: Option<(u16, usize)>,
    /// Where it shows in the editor: "Trainers", "Text", "Maps", ...
    pub category: String,
    pub title: String,
    pub lines: Vec<DiffLine>,
    /// Lines left out after the first `LINE_LIMIT`.
    pub more: usize,
    /// An asset path that shows it; `game/<path>` shows the game's version.
    pub preview: Option<String>,
    /// For a map piece: its tiles before and after.
    pub tiles: Option<TileDiff>,
}

/// A map piece's tiles, as (terrain id, flags), before and after.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileDiff {
    pub width: usize,
    pub height: usize,
    pub old: Vec<(u16, u16)>,
    pub new: Vec<(u16, u16)>,
}

/// What archives.json says of an archive: its label and group.
fn archive_label(arc: u16) -> Option<(String, String)> {
    static LIST: std::sync::OnceLock<Vec<(u16, String, String)>> = std::sync::OnceLock::new();
    let list = LIST.get_or_init(|| {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("archives.json")).unwrap_or_default();
        v["archives"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        Some((
                            x["id"].as_u64()? as u16,
                            x["label"].as_str()?.to_string(),
                            x["group"].as_str().unwrap_or("Files").to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    list.iter()
        .find(|(id, ..)| *id == arc)
        .map(|(_, l, g)| (l.clone(), g.clone()))
}

const LINE_LIMIT: usize = 40;

/// A change path's member: `a/0/1/2/<index>` or `a/0/1/2/<index>.<ext>`.
pub fn member_of(path: &str) -> Option<(u16, usize)> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() != 5 || parts[0] != "a" {
        return None;
    }
    let digit = |s: &str| s.parse::<u16>().ok().filter(|&d| d < 10);
    let arc = digit(parts[1])? * 100 + digit(parts[2])? * 10 + digit(parts[3])?;
    let index = parts[4].split('.').next()?.parse().ok()?;
    Some((arc, index))
}

/// Describes every change of the open project. Overrides that hold the
/// game's own bytes are dropped first: they change nothing.
pub fn describe_all(bridge: &Bridge) -> Vec<ChangeInfo> {
    prune(bridge);
    let Some(project) = bridge.project() else {
        return Vec::new();
    };
    project
        .changes()
        .iter()
        .map(|c| describe(bridge, &c.path))
        .collect()
}

/// Drops member overrides equal to the game's member. Returns how many.
pub fn prune(bridge: &Bridge) -> usize {
    let Some(project) = bridge.project() else {
        return 0;
    };
    let mut dropped = 0;
    for c in project.changes() {
        let Some((arc, index)) = member_of(&c.path) else {
            continue;
        };
        let same = bridge.base_archive(arc).ok().is_some_and(|a| {
            a.raw(index).ok().is_some_and(|raw| {
                project.member(arc, index).is_some_and(|mine| {
                    lz::decompress_if_compressed(raw) == lz::decompress_if_compressed(&mine)
                })
            })
        });
        if same && crate::catalog::revert(bridge, Member { arc, index }).is_ok() {
            dropped += 1;
        }
    }
    dropped
}

/// Describes one changed file.
pub fn describe(bridge: &Bridge, path: &str) -> ChangeInfo {
    let mut info = ChangeInfo {
        path: path.to_string(),
        member: member_of(path),
        category: "Files".into(),
        title: path.to_string(),
        lines: Vec::new(),
        more: 0,
        preview: None,
        tiles: None,
    };
    let Some((arc, index)) = info.member else {
        return info;
    };
    let base = bridge.base_archive(arc).ok().and_then(|a| {
        a.raw(index)
            .ok()
            .map(|r| lz::decompress_if_compressed(r).into_owned())
    });
    let current = bridge.member(Member { arc, index }).ok();
    let (Some(base), Some(current)) = (base, current) else {
        let (label, group) = archive_label(arc).unwrap_or_else(|| ("Files".into(), "Files".into()));
        info.category = group;
        info.title = format!("{label}: new entry {index}");
        return info;
    };
    let mut lines = match arc {
        12 => zone_headers(bridge, &base, &current, &mut info),
        8 => map_chunk(bridge, &base, &current, index, &mut info),
        125 => events(bridge, &base, &current, index, &mut info),
        2 | 3 => text(bridge, &base, &current, arc, index, &mut info),
        _ => table(bridge, &base, &current, arc, index, &mut info)
            .unwrap_or_else(|| bytes(bridge, &base, &current, arc, index, &mut info)),
    };
    if lines.len() > LINE_LIMIT {
        info.more = lines.len() - LINE_LIMIT;
        lines.truncate(LINE_LIMIT);
    }
    info.lines = lines;
    info
}

/// Drops one change: a member override or a whole file.
pub fn revert(bridge: &Bridge, path: &str) -> Result<(), Error> {
    if let Some((arc, index)) = member_of(path) {
        return crate::catalog::revert(bridge, Member { arc, index });
    }
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    project
        .revert_file(path)
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(())
}

fn line(label: impl Into<String>, old: impl Into<String>, new: impl Into<String>) -> DiffLine {
    DiffLine {
        label: label.into(),
        old: old.into(),
        new: new.into(),
    }
}

/// A value with its enum name, when it has one.
fn shown(bridge: &Bridge, enum_name: Option<&str>, v: i64) -> String {
    let name = enum_name
        .and_then(|e| tables::enum_values(bridge, e).ok())
        .and_then(|names| names.get(v as usize).cloned())
        .filter(|n| !n.is_empty());
    name.unwrap_or_else(|| v.to_string())
}

fn table(
    bridge: &Bridge,
    base: &[u8],
    current: &[u8],
    arc: u16,
    index: usize,
    info: &mut ChangeInfo,
) -> Option<Vec<DiffLine>> {
    let t = tables::schemas()
        .tables
        .iter()
        .find(|t| t.archive == arc && tables::editable(t) && t.arrays.is_none())?;
    let name = tables::entries(bridge, &t.id)
        .ok()
        .and_then(|e| e.iter().find(|e| e.index == index).map(|e| e.name.clone()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("#{index}"));
    info.category = t.label.clone();
    info.title = format!("{}: {name}", t.label);
    let seasons = t
        .seasons
        .as_ref()
        .map_or(1, |s| (current.len() / s.block_size).max(1));
    let mut out = Vec::new();
    for season in 0..seasons {
        let prefix = if seasons > 1 {
            format!(
                "{} ",
                ["Spring", "Summer", "Autumn", "Winter"]
                    .get(season)
                    .unwrap_or(&"")
            )
        } else {
            String::new()
        };
        let old = tables::record_of(bridge, &t.id, index, base, season).ok()?;
        let new = tables::record_of(bridge, &t.id, index, current, season).ok()?;
        for (a, b) in old.sections.iter().zip(&new.sections) {
            match (a, b) {
                (Section::Fields { values: va, .. }, Section::Fields { values: vb, .. }) => {
                    for (x, y) in va.iter().zip(vb) {
                        if x.value != y.value {
                            out.push(line(
                                format!("{prefix}{}", y.label),
                                shown(bridge, y.enum_name.as_deref(), x.value),
                                shown(bridge, y.enum_name.as_deref(), y.value),
                            ));
                        }
                    }
                }
                (
                    Section::List { rows: ra, .. },
                    Section::List {
                        rows: rb,
                        columns,
                        label,
                        ..
                    },
                ) => {
                    let row_text = |r: Option<&Vec<i64>>| {
                        r.map_or(String::new(), |r| {
                            columns
                                .iter()
                                .zip(r)
                                .map(|(c, &v)| {
                                    format!(
                                        "{} {}",
                                        c.label,
                                        shown(bridge, c.enum_name.as_deref(), v)
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                    };
                    for i in 0..ra.len().max(rb.len()) {
                        if ra.get(i) != rb.get(i) {
                            out.push(line(
                                format!("{prefix}{label} {}", i + 1),
                                row_text(ra.get(i)),
                                row_text(rb.get(i)),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Some(out)
}

fn text(
    bridge: &Bridge,
    base: &[u8],
    current: &[u8],
    arc: u16,
    index: usize,
    info: &mut ChangeInfo,
) -> Vec<DiffLine> {
    info.category = "Text".into();
    // Story banks are named by the maps that speak them.
    let mut places: Vec<String> = Vec::new();
    if arc == 3 {
        if let Ok(zs) = crate::world::zones(bridge) {
            for z in zs.iter() {
                if crate::scripts::zone_text_bank(bridge, z.id) == Some(index as u16)
                    && !places.contains(&z.place)
                {
                    places.push(z.place.clone());
                }
            }
        }
    }
    info.title = if !places.is_empty() {
        format!("Dialogue: {}", places.join(", "))
    } else if arc == 3 {
        format!("Story text {index}")
    } else {
        format!("Game text {index}")
    };
    let table = bridge.packed_table();
    let strings = |b: &[u8]| -> Vec<String> {
        crate::msg::Bank::parse(b).map_or_else(
            |_| Vec::new(),
            |bank| {
                bank.messages()
                    .iter()
                    .map(|m| crate::msg::display(&m.units, table.as_deref()))
                    .collect()
            },
        )
    };
    let (a, b) = (strings(base), strings(current));
    (0..a.len().max(b.len()))
        .filter(|&i| a.get(i) != b.get(i))
        .map(|i| {
            line(
                format!("Line {i}"),
                a.get(i).cloned().unwrap_or_default(),
                b.get(i).cloned().unwrap_or_default(),
            )
        })
        .collect()
}

/// Zone headers: one 48-byte record per map.
fn zone_headers(
    bridge: &Bridge,
    base: &[u8],
    current: &[u8],
    info: &mut ChangeInfo,
) -> Vec<DiffLine> {
    info.category = "Maps".into();
    info.title = "Map settings".into();
    let zones = crate::world::zones(bridge).ok();
    let place = |z: usize| {
        zones
            .as_ref()
            .and_then(|zs| zs.get(z).map(|x| format!("{} (map {z})", x.place)))
            .unwrap_or_else(|| format!("Map {z}"))
    };
    let u16_at = |b: &[u8], at: usize| {
        b.get(at..at + 2)
            .map_or(0, |x| u16::from_le_bytes([x[0], x[1]]))
    };
    let mut out = Vec::new();
    for z in 0..current.len().min(base.len()) / 48 {
        let (a, b) = (&base[z * 48..z * 48 + 48], &current[z * 48..z * 48 + 48]);
        if a == b {
            continue;
        }
        let mut named = false;
        for (i, season) in ["Spring", "Summer", "Autumn", "Winter"].iter().enumerate() {
            let at = 0x0C + i * 2;
            if a[at..at + 2] != b[at..at + 2] {
                named = true;
                out.push(line(
                    format!("{}: {season} music", place(z)),
                    u16_at(a, at).to_string(),
                    u16_at(b, at).to_string(),
                ));
            }
        }
        if !named {
            out.push(line(place(z), "", "settings changed"));
        }
    }
    out
}

fn map_chunk(
    bridge: &Bridge,
    base: &[u8],
    current: &[u8],
    index: usize,
    info: &mut ChangeInfo,
) -> Vec<DiffLine> {
    info.category = "Maps".into();
    let places = crate::zonemap::chunk_places(bridge, index);
    info.title = if places.is_empty() {
        format!("Map piece {index}")
    } else {
        let names: Vec<&str> = places.iter().map(|p| p.1.as_str()).collect();
        format!("Map tiles: {}", names.join(", "))
    };
    if let (Some((w, h, old)), Some((_, _, new))) = (
        crate::zonemap::chunk_tiles(base),
        crate::zonemap::chunk_tiles(current),
    ) {
        info.tiles = Some(TileDiff {
            width: w,
            height: h,
            old,
            new,
        });
    }
    let tiles = crate::zonemap::changed_tiles(base, current);
    vec![line("Tiles", "", format!("{tiles} changed"))]
}

fn events(
    bridge: &Bridge,
    base: &[u8],
    current: &[u8],
    index: usize,
    info: &mut ChangeInfo,
) -> Vec<DiffLine> {
    info.category = "Maps".into();
    let places: Vec<String> = crate::world::zones(bridge)
        .map(|zs| {
            zs.iter()
                .filter(|z| z.event_data as usize == index)
                .map(|z| z.place.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut places = places;
    places.dedup();
    info.title = if places.is_empty() {
        format!("Map events {index}")
    } else {
        format!("Map events: {}", places.join(", "))
    };
    crate::zonemap::diff_events(base, current)
        .into_iter()
        .map(|(label, old, new)| line(label, old, new))
        .collect()
}

fn bytes(
    bridge: &Bridge,
    base: &[u8],
    current: &[u8],
    arc: u16,
    index: usize,
    info: &mut ChangeInfo,
) -> Vec<DiffLine> {
    let kind = crate::catalog::kind_of(current);
    let (label, group) = archive_label(arc).unwrap_or_else(|| ("Files".into(), "Files".into()));
    info.category = group;
    let noun = match kind.as_str() {
        "NCGR" => "picture",
        "NCLR" => "palette",
        "NCER" => "sprite layout",
        "NSCR" => "screen",
        "NANR" => "animation",
        "BTX0" | "BMD0" => "model",
        _ => "entry",
    };
    // A picture the browser names (Charmander, Master Ball, ...).
    let named = crate::catalog::named_sheets(bridge, arc).ok();
    let sheet = named
        .as_ref()
        .and_then(|n| n.as_ref().as_ref())
        .and_then(|list| {
            list.iter()
                .find(|n| match kind.as_str() {
                    "NCGR" => n.chr == index || n.rig.is_some_and(|r| r[0] == index),
                    "NCLR" => n.pal == index || n.palettes.iter().any(|p| p.1 == index),
                    _ => false,
                })
                .cloned()
        });
    info.title = match &sheet {
        Some(n) => format!("{label}: {}", n.label),
        None => format!("{label}: {noun} {index}"),
    };
    info.preview = match (&sheet, kind.as_str()) {
        (Some(n), "NCGR" | "NCLR") => {
            let big = n.rig.filter(|r| r[0] == index && kind == "NCGR");
            let (chr, cells) = match big {
                Some(r) => (r[0], None),
                None => (n.chr, n.cells),
            };
            let pal = if kind == "NCLR" { index } else { n.pal };
            Some(format!(
                "preview/{arc}:{chr}/{arc}:{pal}?cells={}&paloffset={}",
                cells.map_or("none".to_string(), |c| c.to_string()),
                n.palette_offset
            ))
        }
        (None, "NCGR" | "NCLR") => {
            let want = if kind == "NCGR" { "NCLR" } else { "NCGR" };
            nearest(bridge, arc, index, want).map(|other| {
                let (chr, pal) = if kind == "NCGR" {
                    (index, other)
                } else {
                    (other, index)
                };
                format!("preview/{arc}:{chr}/{arc}:{pal}")
            })
        }
        (_, "BTX0") => Some(format!("texture/{arc}:{index}/0/0")),
        _ => None,
    };
    if info.preview.is_some() {
        return Vec::new();
    }
    let differ = base.iter().zip(current).filter(|(a, b)| a != b).count()
        + base.len().abs_diff(current.len());
    vec![line("Bytes", "", format!("{differ} changed"))]
}

/// The closest member of a kind to `index` in an archive, later first.
fn nearest(bridge: &Bridge, arc: u16, index: usize, kind: &str) -> Option<usize> {
    let members = crate::catalog::members(bridge, arc).ok()?;
    members
        .iter()
        .filter(|m| m.kind == kind)
        .min_by_key(|m| (m.index.abs_diff(index), m.index < index))
        .map(|m| m.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_paths() {
        assert_eq!(member_of("a/0/1/2/0"), Some((12, 0)));
        assert_eq!(member_of("a/1/2/5/31.bin"), Some((125, 31)));
        assert_eq!(member_of("a/0/1/3"), None);
        assert_eq!(member_of("wb_sound_data.sdat"), None);
    }
}
