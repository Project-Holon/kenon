//! The user's settings, kept as JSON in the app's config folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The user's ROM (a `.nds` file) or a folder unpacked from it. Never a
    /// constant in code. Older settings called it `dataFolder`.
    #[serde(alias = "dataFolder")]
    pub game: Option<PathBuf>,
    /// Recently opened projects, newest first.
    pub recent: Vec<PathBuf>,
    /// The folder eidos loads mods from, for the conflict hints.
    pub mods_folder: Option<PathBuf>,
    /// eidos's `mod/declared_set.txt`, when it is not found beside the mods
    /// folder.
    pub declared_set: Option<PathBuf>,
    /// Where the window was and how big, so it opens the same way.
    pub window: Option<WindowPlace>,
}

/// The main window's size and position (logical pixels) and whether it was
/// maximized. The size is the one it had before maximizing.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowPlace {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub maximized: bool,
}

impl Eq for WindowPlace {}

impl Settings {
    /// Reads the settings. A missing or unreadable file gives the defaults.
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Writes the settings through a temporary file and a rename.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config/settings.json");
        let settings = Settings {
            game: Some(dir.path().join("black.nds")),
            recent: vec![dir.path().join("mod")],
            mods_folder: Some(dir.path().join("mods")),
            declared_set: None,
            window: Some(WindowPlace {
                width: 1200.0,
                height: 800.0,
                x: 10.0,
                y: 20.0,
                maximized: true,
            }),
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
    }

    #[test]
    fn missing_or_broken_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        fs::write(&path, "not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"game": null, "later": 1}"#).unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        fs::write(&path, r#"{"dataFolder": "x"}"#).unwrap();
        assert_eq!(Settings::load(&path).game, Some(PathBuf::from("x")));
    }
}
