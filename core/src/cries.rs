//! Cries: which wave archive (WAVARC slot) a species or form plays, in
//! `eidos/tables/cries.txt` (eidos docs/game-tables.md):
//!
//! ```text
//! <species>[.<form>] = <WAVARC slot>
//! ```
//!
//! Slots 1..649 are the game's species' cries (slot n is species n), 650 is
//! Shaymin's Sky Forme, and 651 and up are sound effects, then the wave
//! archives a mod appends. No line: a game species and its forms play their
//! own, a new species plays no cry (0 here), and a new species' form plays
//! its species' line, else none (eidos sdk/cries.c). The volume stays in a/1/9/4 at the slot.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

use crate::project::Project;

const FILE: &str = "eidos/tables/cries.txt";
const LAST_GAME: usize = 649;

/// A species, or one of its forms (1 and up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Key {
    pub species: usize,
    pub form: usize,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.form == 0 {
            write!(f, "{}", self.species)
        } else {
            write!(f, "{}.{}", self.species, self.form)
        }
    }
}

fn parse_key(k: &str) -> Option<Key> {
    let k = k.trim();
    let (s, f) = k.split_once('.').unwrap_or((k, "0"));
    Some(Key {
        species: s.trim().parse().ok()?,
        form: f.trim().parse().ok()?,
    })
}

/// The mod's lines.
pub fn all(project: &Project) -> BTreeMap<Key, u32> {
    let Ok(text) = std::fs::read_to_string(project.root.join(FILE)) else {
        return BTreeMap::new();
    };
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split('#').next()?.split_once('=')?;
            Some((parse_key(k)?, v.trim().parse().ok()?))
        })
        .collect()
}

fn write(project: &Project, rows: &BTreeMap<Key, u32>) -> Result<(), String> {
    let path = project.root.join(FILE);
    if rows.is_empty() {
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut text = String::from("# Written by kenon: <species>[.<form>] = <wave archive slot>.\n");
    for (k, v) in rows {
        text.push_str(&format!("{k} = {v}\n"));
    }
    eidos_mod::manifest::write_atomic(&path, text.as_bytes()).map_err(|e| e.to_string())
}

/// The slot a key plays with no line of its own.
pub fn default_for(project: &Project, key: Key) -> u32 {
    if key.species <= LAST_GAME {
        // The game's own: its form cries are its species' (Shaymin Sky aside).
        return if key.species == 492 && key.form == 1 {
            650
        } else {
            key.species as u32
        };
    }
    if key.form > 0 {
        return current(
            project,
            Key {
                species: key.species,
                form: 0,
            },
        );
    }
    0
}

/// The slot a key plays in this mod.
pub fn current(project: &Project, key: Key) -> u32 {
    all(project)
        .get(&key)
        .copied()
        .unwrap_or_else(|| default_for(project, key))
}

/// Sets a key's cry; None, or its default, drops the line.
pub fn set(project: &Project, key: Key, slot: Option<u32>) -> Result<(), String> {
    if slot == Some(0) {
        return Err("slot 0 is not a cry".into());
    }
    let mut rows = all(project);
    rows.remove(&key);
    if let Some(s) = slot.filter(|&s| s != default_for(project, key)) {
        rows.insert(key, s);
    }
    write(project, &rows)
}

/// After a species is removed: its lines go, later species move down one.
pub fn remove_species(project: &Project, id: usize) -> Result<(), String> {
    let rows = all(project)
        .into_iter()
        .filter(|(k, _)| k.species != id)
        .map(|(mut k, v)| {
            if k.species > id {
                k.species -= 1;
            }
            (k, v)
        })
        .collect();
    write(project, &rows)
}

/// After form `form` of `species` is removed: its line goes, its later
/// forms move down one.
pub fn remove_form(project: &Project, species: usize, form: usize) -> Result<(), String> {
    let rows = all(project)
        .into_iter()
        .filter(|(k, _)| !(k.species == species && k.form == form))
        .map(|(mut k, v)| {
            if k.species == species && k.form > form {
                k.form -= 1;
            }
            (k, v)
        })
        .collect();
    write(project, &rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        assert_eq!(
            parse_key("715"),
            Some(Key {
                species: 715,
                form: 0
            })
        );
        assert_eq!(
            parse_key(" 715.2 "),
            Some(Key {
                species: 715,
                form: 2
            })
        );
        assert_eq!(
            Key {
                species: 715,
                form: 2
            }
            .to_string(),
            "715.2"
        );
        assert!(parse_key("x").is_none());
    }
}
