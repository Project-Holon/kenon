//! A kenon project: a mod folder in the layout the port reads
//! (`docs/bw-format.md`, "The loose directory").
//!
//! ```text
//! <mod>/eidos_mod.txt        the manifest the port reads (key = value)
//! <mod>/kenon.json           kenon's own notes: which ROM it is built on
//! <mod>/a/0/1/6/25           a member override: replaces member 25 of a/0/1/6
//! <mod>/<path>               a whole-file override at a NitroFS path
//! ```
//!
//! Reading through a project gives the mod's version of a file or member
//! when it has one, else the game's. Writing only ever touches the mod
//! folder; the ROM is never changed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::narc;

pub const MANIFEST: &str = "eidos_mod.txt";
pub const KENON_FILE: &str = "kenon.json";
/// The range the port accepts for `order` (eidos docs/mods.md).
pub const ORDER_LIMIT: i32 = 1_000_000;

/// The keys of `eidos_mod.txt` kenon edits; every other line is kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: String,
    pub order: i32,
    /// The port refuses a mod whose `api` is not its own interface version
    /// (eidos sdk/mod_manifest.c), so kenon writes it only when the mod
    /// already declares one (a mod with code); a data-only mod has none.
    pub api: Option<u32>,
    /// The lines kenon does not edit, kept in order.
    #[serde(skip)]
    pub other: Vec<String>,
}

impl Manifest {
    pub fn parse(text: &str, default_name: &str) -> Self {
        let mut m = Manifest {
            name: default_name.to_string(),
            order: 0,
            api: None,
            other: Vec::new(),
        };
        for line in text.lines() {
            let body = line.split('#').next().unwrap_or("");
            let Some((key, value)) = body.split_once('=') else {
                m.other.push(line.to_string());
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "name" => m.name = value.to_string(),
                // The port ignores a bad or out-of-range order and keeps 0.
                "order" => {
                    m.order = value
                        .parse()
                        .ok()
                        .filter(|o: &i32| o.abs() <= ORDER_LIMIT)
                        .unwrap_or(0)
                }
                "api" => m.api = value.parse().ok(),
                _ => m.other.push(line.to_string()),
            }
        }
        m
    }

    pub fn write(&self) -> String {
        let mut out = format!(
            "# Written by kenon.\nname = {}\norder = {}\n",
            self.name,
            self.order.clamp(-ORDER_LIMIT, ORDER_LIMIT)
        );
        if let Some(api) = self.api {
            out.push_str(&format!("api = {api}\n"));
        }
        for line in &self.other {
            if !line.starts_with("# Written by kenon.") {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }
}

/// kenon's own project notes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KenonFile {
    /// The ROM the mod is built on, as the user chose it.
    pub rom: Option<PathBuf>,
}

/// One changed asset in a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// The NitroFS path, with `/<index>` for a member.
    pub path: String,
    pub bytes: u64,
}

pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub kenon: KenonFile,
}

impl Project {
    /// Creates a project in an empty (or new) folder.
    pub fn create(root: &Path, name: &str, rom: Option<&Path>) -> io::Result<Self> {
        if root.exists() && fs::read_dir(root)?.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the folder is not empty",
            ));
        }
        fs::create_dir_all(root)?;
        let project = Project {
            root: root.to_path_buf(),
            manifest: Manifest::parse("", name),
            kenon: KenonFile {
                rom: rom.map(Path::to_path_buf),
            },
        };
        project.save_meta()?;
        Ok(project)
    }

    /// Opens an existing mod folder. A folder without a manifest is still a
    /// mod (the port applies the defaults).
    pub fn open(root: &Path) -> io::Result<Self> {
        if !root.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such folder"));
        }
        let default_name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "mod".to_string());
        let manifest = match fs::read_to_string(root.join(MANIFEST)) {
            Ok(text) => Manifest::parse(&text, &default_name),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Manifest::parse("", &default_name),
            Err(e) => return Err(e),
        };
        let kenon = fs::read(root.join(KENON_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ok(Project {
            root: root.to_path_buf(),
            manifest,
            kenon,
        })
    }

    pub fn save_meta(&self) -> io::Result<()> {
        write_atomic(&self.root.join(MANIFEST), self.manifest.write().as_bytes())?;
        let json = serde_json::to_vec_pretty(&self.kenon).map_err(io::Error::other)?;
        write_atomic(&self.root.join(KENON_FILE), &json)
    }

    fn member_dir(&self, arc: u16) -> Option<PathBuf> {
        narc::archive_path(arc).map(|rel| self.root.join(rel))
    }

    /// The mod's version of a NARC member, if it has one. The port also
    /// accepts `<index>.<ext>`.
    pub fn member(&self, arc: u16, index: usize) -> Option<Vec<u8>> {
        let dir = self.member_dir(arc)?;
        if !dir.is_dir() {
            return None;
        }
        if let Ok(bytes) = fs::read(dir.join(index.to_string())) {
            return Some(bytes);
        }
        let prefix = format!("{index}.");
        fs::read_dir(&dir)
            .ok()?
            .flatten()
            .find(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .and_then(|e| fs::read(e.path()).ok())
    }

    pub fn write_member(&self, arc: u16, index: usize, bytes: &[u8]) -> io::Result<()> {
        let dir = self
            .member_dir(arc)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad archive id"))?;
        fs::create_dir_all(&dir)?;
        // Two files naming one index stop the port opening the archive
        // (eidos sdk/vfs_narc.c): drop any other spelling first.
        remove_siblings(&dir, index, true)?;
        write_atomic(&dir.join(index.to_string()), bytes)
    }

    /// Drops the mod's version of a member, back to the game's.
    pub fn revert_member(&self, arc: u16, index: usize) -> io::Result<()> {
        let Some(dir) = self.member_dir(arc) else {
            return Ok(());
        };
        remove_siblings(&dir, index, false)
    }

    /// Drops a whole-file override.
    pub fn revert_file(&self, path: &str) -> io::Result<()> {
        let p = self.root.join(path.trim_start_matches('/'));
        if p.is_file() {
            fs::remove_file(p)?;
        }
        Ok(())
    }

    /// A whole-file override, if the mod has one. An archive path that is a
    /// folder of member overrides is not a whole-file override.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        let p = self.root.join(path.trim_start_matches('/'));
        p.is_file().then(|| fs::read(p).ok()).flatten()
    }

    /// Every asset the mod changes.
    pub fn changes(&self) -> Vec<Change> {
        let mut out = Vec::new();
        walk(&self.root, &self.root, &mut out);
        out.retain(|c| c.path != MANIFEST && c.path != KENON_FILE);
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Change>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            if rel
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('~'))
            {
                continue;
            }
            out.push(Change {
                path: rel.to_string_lossy().replace('\\', "/"),
                bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
            });
        }
    }
}

/// Removes every file of `dir` that names member `index` (`N` or `N.ext`),
/// except the bare `N` when `keep_bare`.
fn remove_siblings(dir: &Path, index: usize, keep_bare: bool) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let bare = index.to_string();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = name.split('.').next().unwrap_or("");
        if stem == bare && !(keep_bare && name == bare) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // A name the port cannot read as a member index ("~25.kenon"), so a
    // crash before the rename leaves nothing it would load.
    let tmp = path.with_file_name(format!(
        "~{}.kenon",
        path.file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned())
    ));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_keeps_other_lines() {
        let m = Manifest::parse(
            "name = Hard mode\norder = x\n# note\nrule = foo bar\n",
            "dir",
        );
        assert_eq!(m.name, "Hard mode");
        assert_eq!(m.order, 0);
        assert_eq!(m.api, None);
        assert!(!m.write().contains("api"));
        assert_eq!(Manifest::parse("order = 2000000\napi = 3", "d").order, 0);
        assert!(Manifest::parse("api = 3", "d")
            .write()
            .contains("api = 3\n"));
        let text = m.write();
        assert!(text.contains("name = Hard mode\n"));
        assert!(text.contains("rule = foo bar\n"));
        assert!(text.contains("# note\n"));
        assert_eq!(Manifest::parse(&text, "dir").name, "Hard mode");
    }

    #[test]
    fn create_write_read_revert() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("mymod");
        let p = Project::create(&root, "My mod", Some(Path::new("black.nds"))).unwrap();
        assert!(root.join(MANIFEST).is_file());
        assert_eq!(p.member(16, 1), None);
        p.write_member(16, 1, b"new").unwrap();
        assert_eq!(p.member(16, 1).as_deref(), Some(&b"new"[..]));
        assert!(root.join("a/0/1/6/1").is_file());
        let changes = p.changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "a/0/1/6/1");
        // A whole-file read of the archive path is not the member folder.
        assert_eq!(p.file("a/0/1/6"), None);

        let again = Project::open(&root).unwrap();
        assert_eq!(again.manifest.name, "My mod");
        assert_eq!(again.kenon.rom.as_deref(), Some(Path::new("black.nds")));
        again.revert_member(16, 1).unwrap();
        assert!(again.changes().is_empty());
        assert!(Project::create(&root, "x", None).is_err());
    }

    #[test]
    fn member_with_extension() {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::create(dir.path(), "m", None).unwrap();
        fs::create_dir_all(dir.path().join("a/0/0/7")).unwrap();
        fs::write(dir.path().join("a/0/0/7/9.ncgr"), b"x").unwrap();
        assert_eq!(p.member(7, 9).as_deref(), Some(&b"x"[..]));
        // Writing member 9 replaces the other spelling instead of adding a
        // second file for the same index.
        p.write_member(7, 9, b"y").unwrap();
        assert!(!dir.path().join("a/0/0/7/9.ncgr").exists());
        assert_eq!(p.member(7, 9).as_deref(), Some(&b"y"[..]));
        p.revert_member(7, 9).unwrap();
        assert_eq!(p.member(7, 9), None);
    }
}
