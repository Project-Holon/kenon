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

use serde::Serialize;

use crate::narc;

// The manifest and the notes file are morphe's (eidos-mod).
use eidos_mod::manifest::{write_atomic, Folder};
pub use eidos_mod::manifest::{KenonFile, Manifest, KENON_FILE, MANIFEST, ORDER_LIMIT};

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
        Folder::create(root, name, rom).map(Self::from_folder)
    }

    /// Opens an existing mod folder. A folder without a manifest is still a
    /// mod (the port applies the defaults).
    pub fn open(root: &Path) -> io::Result<Self> {
        Folder::open(root).map(Self::from_folder)
    }

    fn from_folder(f: Folder) -> Self {
        Project {
            root: f.root,
            manifest: f.manifest,
            kenon: f.kenon,
        }
    }

    pub fn save_meta(&self) -> io::Result<()> {
        let folder = Folder {
            root: self.root.clone(),
            manifest: self.manifest.clone(),
            kenon: self.kenon.clone(),
        };
        folder.save_meta()
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

#[cfg(test)]
mod tests {
    use super::*;

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
