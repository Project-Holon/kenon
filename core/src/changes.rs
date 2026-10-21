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

/// Describes every change of the open project.
pub fn describe_all(bridge: &Bridge) -> Vec<ChangeInfo> {
    let Some(project) = bridge.project() else {
        return Vec::new();
    };
    project
        .changes()
        .iter()
        .map(|c| describe(bridge, &c.path))
        .collect()
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
        info.title = format!(
            "New file {index} in {}",
            path.rsplit_once('/').map_or(path, |p| p.0)
        );
        return info;
    };
    let mut lines = match arc {
        12 => zone_headers(bridge, &base, &current, &mut info),
        8 => map_chunk(&base, &current, index, &mut info),
        125 => events(bridge, &base, &current, index, &mut info),
        2 | 3 => text(bridge, &base, &current, arc, index, &mut info),
        _ => table(bridge, &base, &current, arc, index, &mut info)
            .unwrap_or_else(|| bytes(&base, &current, arc, index, &mut info)),
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
    info.title = format!(
        "{} text, bank {index}",
        if arc == 2 { "System" } else { "Story" }
    );
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

fn map_chunk(base: &[u8], current: &[u8], index: usize, info: &mut ChangeInfo) -> Vec<DiffLine> {
    info.category = "Maps".into();
    info.title = format!("Map piece {index}");
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
    base: &[u8],
    current: &[u8],
    arc: u16,
    index: usize,
    info: &mut ChangeInfo,
) -> Vec<DiffLine> {
    let kind = crate::catalog::kind_of(current);
    info.category = match kind.as_str() {
        "NCGR" | "NCLR" | "NCER" | "NSCR" | "NANR" => "Graphics".into(),
        "BTX0" | "BMD0" => "Textures".into(),
        _ => "Files".into(),
    };
    info.title = format!(
        "{} {index} in {}",
        match kind.as_str() {
            "NCGR" => "Picture",
            "NCLR" => "Palette",
            "NCER" => "Sprite layout",
            "NSCR" => "Screen",
            "NANR" => "Animation",
            _ => "File",
        },
        crate::narc::archive_path(arc).unwrap_or_default()
    );
    let differ = base.iter().zip(current).filter(|(a, b)| a != b).count()
        + base.len().abs_diff(current.len());
    vec![line("Bytes", "", format!("{differ} changed"))]
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
