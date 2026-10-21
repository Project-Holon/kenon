//! The user's data folder: where kenon finds the game's unpacked data.
//!
//! kenon reads the same folder as eidos (eidos `docs/installer.md`, "The
//! flow"). The user may name either the extraction folder (the one with
//! `eidos_manifest.json` and `data/`) or its `data/` folder. The NitroFS
//! tree is under `data/`, and the archives the UI reads are under `data/a`
//! (for example `a/0/2/3`, the fonts). `arm9_dec.bin` sits beside `data/`;
//! kenon needs it later for the UI tables that live in code.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// The marker file an eidos extraction writes beside `data/`.
pub const MANIFEST_NAME: &str = "eidos_manifest.json";
/// The decompressed ARM9 image an eidos extraction writes beside `data/`.
pub const ARM9_NAME: &str = "arm9_dec.bin";

/// The result of checking a folder the user chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DataRootCheck {
    /// The path does not exist.
    #[serde(rename_all = "camelCase")]
    Missing { path: PathBuf },
    /// The path exists but is not a folder.
    #[serde(rename_all = "camelCase")]
    NotAFolder { path: PathBuf },
    /// The folder has no `data/a` and is not itself a `data/` folder.
    #[serde(rename_all = "camelCase")]
    NoGameData { path: PathBuf },
    /// The folder holds the game's data.
    #[serde(rename_all = "camelCase")]
    Ready(DataRoot),
}

/// A data folder that passed the check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataRoot {
    /// The folder the user chose.
    pub chosen: PathBuf,
    /// The `data/` folder: the NitroFS root.
    pub data_dir: PathBuf,
    /// The folder that holds `data/`, when it has one the check could see.
    pub extraction_dir: Option<PathBuf>,
    /// `eidos_manifest.json` is beside `data/`.
    pub has_manifest: bool,
    /// `arm9_dec.bin` is beside `data/`.
    pub has_arm9: bool,
}

impl DataRoot {
    /// The full path of a NitroFS path such as `a/0/2/3`.
    pub fn file(&self, rel: &str) -> PathBuf {
        self.data_dir.join(rel)
    }
}

/// Checks a folder the user chose and finds the `data/` folder in it.
pub fn check(chosen: &Path) -> DataRootCheck {
    let chosen = chosen.to_path_buf();
    if !chosen.exists() {
        return DataRootCheck::Missing { path: chosen };
    }
    if !chosen.is_dir() {
        return DataRootCheck::NotAFolder { path: chosen };
    }

    // The extraction folder first, then the data folder itself.
    let nested = chosen.join("data");
    let (data_dir, extraction_dir) = if is_nitrofs_root(&nested) {
        (nested, Some(chosen.clone()))
    } else if is_nitrofs_root(&chosen) {
        (chosen.clone(), chosen.parent().map(Path::to_path_buf))
    } else {
        return DataRootCheck::NoGameData { path: chosen };
    };

    let beside = |name: &str| {
        extraction_dir
            .as_ref()
            .is_some_and(|dir| dir.join(name).is_file())
    };
    DataRootCheck::Ready(DataRoot {
        has_manifest: beside(MANIFEST_NAME),
        has_arm9: beside(ARM9_NAME),
        chosen,
        data_dir,
        extraction_dir,
    })
}

/// The folder eidos extracts to by default on this platform, when it can be
/// worked out (eidos `docs/installer.md`, "Where the data goes"). kenon
/// offers it as a suggestion; it never uses it without the user's choice.
pub fn eidos_default() -> Option<PathBuf> {
    eidos_home().map(|home| home.join("extract"))
}

fn eidos_home() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        var("LOCALAPPDATA").map(|dir| dir.join("eidos"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|dir| dir.join("Library/Application Support/eidos"))
    } else {
        // A relative XDG_DATA_HOME is ignored, as the spec says.
        var("XDG_DATA_HOME")
            .filter(|dir| dir.is_absolute())
            .or_else(|| var("HOME").map(|dir| dir.join(".local/share")))
            .map(|dir| dir.join("eidos"))
    }
}

fn is_nitrofs_root(dir: &Path) -> bool {
    dir.join("a").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn extraction(with_manifest: bool, with_arm9: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/a/0/2")).unwrap();
        if with_manifest {
            fs::write(dir.path().join(MANIFEST_NAME), "{}").unwrap();
        }
        if with_arm9 {
            fs::write(dir.path().join(ARM9_NAME), [0u8; 4]).unwrap();
        }
        dir
    }

    #[test]
    fn missing_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nothing");
        assert_eq!(check(&path), DataRootCheck::Missing { path });
    }

    #[test]
    fn file_is_not_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rom.nds");
        fs::write(&path, [0u8; 4]).unwrap();
        assert_eq!(check(&path), DataRootCheck::NotAFolder { path });
    }

    #[test]
    fn empty_folder_has_no_game_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        assert_eq!(check(&path), DataRootCheck::NoGameData { path });
    }

    #[test]
    fn extraction_folder() {
        let dir = extraction(true, true);
        let DataRootCheck::Ready(root) = check(dir.path()) else {
            panic!("expected a data root");
        };
        assert_eq!(root.data_dir, dir.path().join("data"));
        assert_eq!(root.extraction_dir.as_deref(), Some(dir.path()));
        assert!(root.has_manifest);
        assert!(root.has_arm9);
        assert_eq!(root.file("a/0/2/3"), dir.path().join("data/a/0/2/3"));
    }

    #[test]
    fn data_folder_itself() {
        let dir = extraction(false, true);
        let data = dir.path().join("data");
        let DataRootCheck::Ready(root) = check(&data) else {
            panic!("expected a data root");
        };
        assert_eq!(root.chosen, data);
        assert_eq!(root.data_dir, data);
        assert!(!root.has_manifest);
        assert!(root.has_arm9);
    }

    #[test]
    fn plain_nitrofs_tree_without_eidos_files() {
        // aletheia's own extract/ has the tree but no eidos manifest.
        let dir = extraction(false, false);
        let DataRootCheck::Ready(root) = check(dir.path()) else {
            panic!("expected a data root");
        };
        assert!(!root.has_manifest);
        assert!(!root.has_arm9);
    }

    #[test]
    fn check_serializes_with_a_state_tag() {
        let dir = tempfile::tempdir().unwrap();
        let json = serde_json::to_value(check(dir.path())).unwrap();
        assert_eq!(json["state"], "noGameData");

        let dir = extraction(true, false);
        let json = serde_json::to_value(check(dir.path())).unwrap();
        assert_eq!(json["state"], "ready");
        assert_eq!(json["hasManifest"], true);
        assert_eq!(json["hasArm9"], false);
    }
}
