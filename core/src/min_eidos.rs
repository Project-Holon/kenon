//! The oldest eidos a mod works on: `eidos = X.Y.Z` in eidos_mod.txt
//! ("this eidos or later"; 0.2.8 and later refuse a mod that needs a newer
//! one). kenon writes it from what the mod uses, by eidos's own list of
//! the first version that reads each thing (eidos sdk/features.json):
//!
//! ```text
//! {"format": 1, "features": {"tables/types.txt": "0.2.8",
//!   "tables/types.txt name": "0.2.8", "manifest limit.types": "0.2.8",
//!   "a/0/6/6": "0.2.8", ...}}
//! ```
//!
//! A feature is a file under eidos/ (`tables/types.txt`), a word of one of
//! its lines (`tables/move_handlers.txt geomancy`; for `key=value` the
//! key), a manifest key (`manifest limit.types`), or an archive folder the
//! mod ships members in (`a/0/6/6`). Anything the list does not name has no
//! minimum.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::project::Project;

/// The manifest key.
pub const KEY: &str = "eidos";
const BUNDLED: &str = include_str!("features.json");

/// An eidos version, compared number by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Version(pub [u32; 3]);

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let mut parts = s.trim().split('.');
        let mut v = [0; 3];
        for slot in &mut v {
            *slot = parts.next()?.parse().ok()?;
        }
        parts.next().is_none().then_some(Version(v))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0[0], self.0[1], self.0[2])
    }
}

/// Feature to the first eidos that reads it.
#[derive(Debug, Clone, Default)]
pub struct Features(pub BTreeMap<String, Version>);

impl Features {
    pub fn parse(text: &str) -> Result<Features, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v.get("format").and_then(Value::as_u64) != Some(1) {
            return Err("features.json: format 1 expected".into());
        }
        let map = v
            .get("features")
            .and_then(Value::as_object)
            .ok_or("features.json: no features")?
            .iter()
            .map(|(k, v)| {
                let ver = v
                    .as_str()
                    .and_then(Version::parse)
                    .ok_or_else(|| format!("features.json: {k}: bad version"))?;
                Ok((k.clone(), ver))
            })
            .collect::<Result<_, String>>()?;
        Ok(Features(map))
    }

    /// kenon's copy, joined with the one the last eidos run left (newer
    /// features; on a disagreement the higher version wins).
    pub fn load(eidos_copy: Option<&Path>) -> Result<Features, String> {
        let mut f = Features::parse(BUNDLED)?;
        if let Some(text) = eidos_copy.and_then(|p| std::fs::read_to_string(p).ok()) {
            for (k, v) in Features::parse(&text)?.0 {
                let e = f.0.entry(k).or_insert(v);
                *e = (*e).max(v);
            }
        }
        Ok(f)
    }
}

/// The features a mod folder uses, in the list's key shape.
pub fn uses(project: &Project) -> Vec<String> {
    let mut out = Vec::new();
    for line in &project.manifest.other {
        if let Some((k, _)) = line.split('#').next().unwrap_or("").split_once('=') {
            let k = k.trim();
            if k != KEY && !k.is_empty() {
                out.push(format!("manifest {k}"));
            }
        }
    }
    let tables = project.root.join("eidos/tables");
    if let Ok(dir) = std::fs::read_dir(&tables) {
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".txt") {
                continue;
            }
            let file = format!("tables/{name}");
            out.push(file.clone());
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            for line in text.lines() {
                let body = line.split('#').next().unwrap_or("");
                let Some((_, value)) = body.split_once('=') else {
                    continue;
                };
                for word in value.split(|c: char| c.is_whitespace() || c == '+') {
                    let w = word.split('=').next().unwrap_or("");
                    if !w.is_empty() {
                        out.push(format!("{file} {w}"));
                    }
                }
            }
        }
    }
    // Archive folders with members past the game's: a/X/Y/Z. Only the
    // move scripts (a/0/6/6) are in eidos's list; a member there counts
    // when it is an added move's.
    let a = project.root.join("a");
    for x in 0..10 {
        for y in 0..10 {
            for z in 0..10 {
                let d = a.join(format!("{x}/{y}/{z}"));
                let past = |name: &str| {
                    (x, y, z) != (0, 6, 6)
                        || name
                            .parse::<usize>()
                            .is_ok_and(|n| n > crate::move_anims::LAST as usize)
                };
                if std::fs::read_dir(&d)
                    .is_ok_and(|it| it.flatten().any(|e| past(&e.file_name().to_string_lossy())))
                {
                    out.push(format!("a/{x}/{y}/{z}"));
                }
            }
        }
    }
    // Text in a language of its own (`<bank>.<lang>.msg`).
    for arc in [2u16, 3] {
        if crate::project::LANGUAGES
            .iter()
            .any(|l| !project.msg_banks(arc, Some(l)).is_empty())
        {
            out.push(format!("a/0/0/{arc} lang"));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// What a mod needs: the highest version among what it uses, and the
/// features at that version.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Need {
    pub version: Option<Version>,
    pub because: Vec<String>,
}

pub fn need(project: &Project, features: &Features) -> Need {
    let found: Vec<(String, Version)> = uses(project)
        .into_iter()
        .filter_map(|u| features.0.get(&u).map(|v| (u, *v)))
        .collect();
    let version = found.iter().map(|(_, v)| *v).max();
    Need {
        version,
        because: found
            .into_iter()
            .filter(|(_, v)| Some(*v) == version)
            .map(|(u, _)| u)
            .collect(),
    }
}

/// Writes `eidos = <version>` for what the mod uses (no line when nothing
/// needs one). Returns what it needs.
pub fn refresh(project: &Project, features: &Features) -> Result<Need, String> {
    let n = need(project, features);
    let mut fresh = Project::open(&project.root).map_err(|e| e.to_string())?;
    let line = n.version.map(|v| format!("{KEY} = {v}"));
    let current: Vec<&String> = fresh
        .manifest
        .other
        .iter()
        .filter(|l| {
            l.split('#')
                .next()
                .unwrap_or("")
                .split_once('=')
                .map(|(k, _)| k.trim())
                == Some(KEY)
        })
        .collect();
    if current.len() == line.iter().count() && current.first().map(|s| s.trim()) == line.as_deref()
    {
        return Ok(n);
    }
    fresh.manifest.other.retain(|l| {
        l.split('#')
            .next()
            .unwrap_or("")
            .split_once('=')
            .map(|(k, _)| k.trim())
            != Some(KEY)
    });
    if let Some(l) = line {
        fresh.manifest.other.push(l);
    }
    fresh.save_meta().map_err(|e| e.to_string())?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(Version::parse("0.2.8"), Some(Version([0, 2, 8])));
        assert!(Version::parse("0.2.10") > Version::parse("0.2.9"));
        assert_eq!(Version::parse("0.2"), None);
        assert_eq!(Version::parse("0.2.8-rc1"), None);
        assert_eq!(Version([1, 0, 3]).to_string(), "1.0.3");
    }

    #[test]
    fn bundled_list_reads() {
        Features::parse(BUNDLED).unwrap();
    }

    #[test]
    fn a_mod_gets_the_highest_version_it_uses() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("m");
        let project = Project::create(&root, "Test", None).unwrap();
        let features = Features::parse(
            r#"{"format": 1, "features": {"tables/types.txt": "0.2.7",
                "tables/types.txt name": "0.2.8", "tables/move_handlers.txt type": "0.2.8",
                "manifest limit.types": "0.2.8", "a/0/6/6": "0.2.9"}}"#,
        )
        .unwrap();
        // Nothing used: no line.
        assert_eq!(refresh(&project, &features).unwrap().version, None);

        std::fs::create_dir_all(root.join("eidos/tables")).unwrap();
        std::fs::write(
            root.join("eidos/tables/types.txt"),
            "17 = icon 172 colors F6A5D5\n",
        )
        .unwrap();
        assert_eq!(need(&project, &features).version, Version::parse("0.2.7"));
        std::fs::write(
            root.join("eidos/tables/types.txt"),
            "# name in a comment\n17 = name fairy icon 172\n",
        )
        .unwrap();
        std::fs::write(
            root.join("eidos/tables/move_handlers.txt"),
            "45 = flower_shield type=fairy + 1\n",
        )
        .unwrap();
        let n = refresh(&project, &features).unwrap();
        assert_eq!(n.version, Version::parse("0.2.8"));
        assert_eq!(
            n.because,
            vec!["tables/move_handlers.txt type", "tables/types.txt name"]
        );
        let text = std::fs::read_to_string(root.join("eidos_mod.txt")).unwrap();
        assert_eq!(text.matches("eidos = 0.2.8").count(), 1, "{text}");

        // A member in a/0/6/6 raises it; again the line is replaced, not added.
        std::fs::create_dir_all(root.join("a/0/6/6")).unwrap();
        std::fs::write(root.join("a/0/6/6/560"), [0u8]).unwrap();
        let project = Project::open(&root).unwrap();
        assert_eq!(
            refresh(&project, &features).unwrap().because,
            vec!["a/0/6/6"]
        );
        let text = std::fs::read_to_string(root.join("eidos_mod.txt")).unwrap();
        assert!(
            text.contains("eidos = 0.2.9") && !text.contains("eidos = 0.2.8"),
            "{text}"
        );

        // A game move's own script (Pound, 1) is not past the game's.
        std::fs::remove_file(root.join("a/0/6/6/560")).unwrap();
        std::fs::write(root.join("a/0/6/6/1"), [0u8]).unwrap();
        assert_eq!(need(&project, &features).version, Version::parse("0.2.8"));
    }
}

#[cfg(test)]
mod lang_tests {
    use super::*;

    #[test]
    fn a_translation_needs_the_language_feature() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::create(&dir.path().join("m"), "Test", None).unwrap();
        let lines = [(17usize, "Fée".to_string())].into_iter().collect();
        project
            .write_msg_lines_in(2, 199, Some("fr"), &lines)
            .unwrap();
        assert_eq!(project.msg_banks(2, Some("fr")), vec![199]);
        assert!(project.msg_banks(2, None).is_empty());
        assert_eq!(project.msg_lines_in(2, 199, Some("fr"))[&17], "Fée");
        assert!(uses(&project).contains(&"a/0/0/2 lang".to_string()));
        assert!(project.members(2).is_empty(), "a line file is not a member");
    }
}
