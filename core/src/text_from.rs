//! Where an added move's or item's sentence lines come from in a game of
//! another language (eidos `eidos/tables/text_from.txt`, 0.2.8):
//!
//! ```text
//! move.<id> = <move id>
//! item.<id> = <item id>
//! ```
//!
//! eidos makes the "used" lines, the article, the plural and the capitals
//! from the source entry's own lines in the game's language, the name
//! swapped. kenon writes the entry it was copied from, which reads closer
//! than eidos's defaults (the move's handler base, else Pound; for items
//! only TMs, plates and drives).

use std::collections::BTreeMap;
use std::fs;

use crate::bridge::Error;
use crate::project::Project;

const FILE: &str = "eidos/tables/text_from.txt";

/// The mod's lines: (kind, id) to source.
pub fn all(project: &Project) -> BTreeMap<(String, usize), usize> {
    let text = fs::read_to_string(project.root.join(FILE)).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let body = l.split('#').next()?;
            let (k, v) = body.split_once('=')?;
            let (kind, id) = k.trim().split_once('.')?;
            Some(((kind.to_string(), id.parse().ok()?), v.trim().parse().ok()?))
        })
        .collect()
}

fn write(project: &Project, rows: &BTreeMap<(String, usize), usize>) -> Result<(), Error> {
    let path = project.root.join(FILE);
    if rows.is_empty() {
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| Error::Missing(e.to_string()))?;
        }
        return Ok(());
    }
    let mut text = String::from("# Written by kenon: <move|item>.<id> = <entry whose sentence lines it takes, the name swapped>.\n");
    for ((kind, id), v) in rows {
        text.push_str(&format!("{kind}.{id} = {v}\n"));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::Missing(e.to_string()))?;
    }
    eidos_mod::manifest::write_atomic(&path, text.as_bytes())
        .map_err(|e| Error::Missing(e.to_string()))
}

fn kind_of(table: &str) -> Option<&'static str> {
    match table {
        "moves" => Some("move"),
        "items" => Some("item"),
        _ => None,
    }
}

/// An added move or item takes its sentence lines from the entry it was
/// copied from.
pub fn added(project: &Project, table: &str, id: usize, from: usize) -> Result<(), Error> {
    let Some(kind) = kind_of(table) else {
        return Ok(());
    };
    let mut rows = all(project);
    rows.insert((kind.to_string(), id), from);
    write(project, &rows)
}

/// After an added move or item is removed: its line goes, later ids move
/// down, and lines that took the removed one's lines take none (eidos's
/// default).
pub fn removed(project: &Project, table: &str, id: usize) -> Result<bool, Error> {
    let Some(kind) = kind_of(table) else {
        return Ok(false);
    };
    let old = all(project);
    let shift = |v: usize| if v > id { v - 1 } else { v };
    let new: BTreeMap<(String, usize), usize> = old
        .iter()
        .filter(|((k, i), v)| !(k == kind && (*i == id || **v == id)))
        .map(|((k, i), v)| {
            if k == kind {
                ((k.clone(), shift(*i)), shift(*v))
            } else {
                ((k.clone(), *i), *v)
            }
        })
        .collect();
    if new == old {
        return Ok(false);
    }
    write(project, &new)?;
    Ok(true)
}

/// The language the mod's plain text files are written in (manifest
/// `lang`, eidos 0.2.8): the ROM's. English is eidos's default, so no line.
pub fn set_language(project: &Project, lang: Option<&str>) -> Result<(), Error> {
    let want = lang.filter(|l| *l != "en").map(|l| format!("lang = {l}"));
    let is_lang = |l: &String| {
        l.split('#')
            .next()
            .unwrap_or("")
            .split_once('=')
            .map(|(k, _)| k.trim())
            == Some("lang")
    };
    let current: Vec<&String> = project
        .manifest
        .other
        .iter()
        .filter(|l| is_lang(l))
        .collect();
    if current.len() == want.iter().count() && current.first().map(|s| s.trim()) == want.as_deref()
    {
        return Ok(());
    }
    let mut fresh = Project::open(&project.root).map_err(|e| Error::Missing(e.to_string()))?;
    fresh.manifest.other.retain(|l| !is_lang(l));
    if let Some(l) = want {
        fresh.manifest.other.push(l);
    }
    fresh.save_meta().map_err(|e| Error::Missing(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_follow_adds_and_removals() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::create(&dir.path().join("m"), "Test", None).unwrap();
        added(&project, "moves", 560, 85).unwrap();
        added(&project, "moves", 561, 560).unwrap();
        added(&project, "moves", 562, 53).unwrap();
        added(&project, "items", 627, 17).unwrap();
        added(&project, "abilities", 165, 22).unwrap();
        assert_eq!(all(&project).len(), 4);
        // 560 goes: 561 took its lines (none now), 562 becomes 561.
        assert!(removed(&project, "moves", 560).unwrap());
        let rows = all(&project);
        assert_eq!(rows.get(&("move".into(), 561)), Some(&53));
        assert_eq!(rows.get(&("item".into(), 627)), Some(&17));
        assert_eq!(rows.len(), 2);
    }
}
