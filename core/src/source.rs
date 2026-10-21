//! Where kenon reads the game from: the user's ROM (the normal way) or a
//! folder unpacked from it (eidos's extraction, or aletheia's `extract/`).

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;

use crate::code::CodeImages;
use crate::data_root::{self, DataRoot, DataRootCheck};
use crate::rom::{self, Game, Identity, Rom};

/// An opened game.
#[derive(Clone)]
pub enum Source {
    Rom(Arc<Rom>),
    Folder(DataRoot),
}

impl Source {
    /// The bytes of a NitroFS file, such as `a/0/2/3`.
    pub fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
        match self {
            Source::Rom(rom) => rom.file(path).map_err(std::io::Error::other),
            Source::Folder(root) => std::fs::read(root.file(path)),
        }
    }

    /// The code images (ARM9 and overlays), when the source has them.
    pub fn code(&self) -> Option<CodeImages> {
        match self {
            Source::Rom(rom) => Some(CodeImages::from_rom(rom.clone())),
            Source::Folder(root) => root
                .extraction_dir
                .as_deref()
                .filter(|_| root.has_arm9)
                .and_then(|dir| CodeImages::open(dir).ok()),
        }
    }
}

impl From<DataRoot> for Source {
    fn from(root: DataRoot) -> Self {
        Source::Folder(root)
    }
}

impl From<Rom> for Source {
    fn from(rom: Rom) -> Self {
        Source::Rom(Arc::new(rom))
    }
}

/// What the settings screen shows about the chosen ROM or folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum GameCheck {
    /// A ROM kenon can use.
    #[serde(rename_all = "camelCase")]
    Rom {
        identity: Identity,
        /// Black is the version kenon is built and tested against.
        supported: bool,
    },
    /// An unpacked folder kenon can use.
    #[serde(rename_all = "camelCase")]
    Folder { has_arm9: bool },
    /// Not usable, and why, in words for the user.
    #[serde(rename_all = "camelCase")]
    Problem { message: String },
}

/// Opens what the user chose: a `.nds` file or an unpacked folder.
pub fn open(path: &Path) -> (GameCheck, Option<Source>) {
    if path.is_file() {
        return match Rom::open(path) {
            Ok(rom) => match rom.identity.game {
                Game::Other => (
                    GameCheck::Problem {
                        message: format!(
                            "This is a DS game, but not Pokemon Black or White: its title is \
                             \"{}\" and its game code {}.",
                            rom.identity.title, rom.identity.game_code
                        ),
                    },
                    None,
                ),
                game => (
                    GameCheck::Rom {
                        identity: rom.identity.clone(),
                        supported: game == Game::Black,
                    },
                    Some(rom.into()),
                ),
            },
            Err(rom::Error::NotDs | rom::Error::TooSmall) => (
                GameCheck::Problem {
                    message: "This file is not a Nintendo DS game. Choose the .nds file of your \
                              Pokemon Black dump."
                        .to_string(),
                },
                None,
            ),
            Err(e) => (
                GameCheck::Problem {
                    message: format!("The ROM could not be read: {e}"),
                },
                None,
            ),
        };
    }
    match data_root::check(path) {
        DataRootCheck::Ready(root) => (
            GameCheck::Folder {
                has_arm9: root.has_arm9,
            },
            Some(root.into()),
        ),
        DataRootCheck::Missing { .. } => (
            GameCheck::Problem {
                message: "This file or folder no longer exists.".to_string(),
            },
            None,
        ),
        _ => (
            GameCheck::Problem {
                message: "No game here. Choose your Pokemon Black .nds file, or a folder \
                          unpacked from it."
                    .to_string(),
            },
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_a_rom_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.nds");
        std::fs::write(&path, crate::rom::tests::tiny_rom()).unwrap();
        let (check, source) = open(&path);
        assert!(
            matches!(
                check,
                GameCheck::Rom {
                    supported: true,
                    ..
                }
            ),
            "{check:?}"
        );
        assert_eq!(source.unwrap().read("x.bin").unwrap(), b"hi");
    }

    #[test]
    fn problems() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, [0u8; 1024]).unwrap();
        assert!(matches!(open(&path).0, GameCheck::Problem { .. }));
        assert!(matches!(open(dir.path()).0, GameCheck::Problem { .. }));
    }

    #[test]
    fn opens_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/a")).unwrap();
        std::fs::write(dir.path().join("data/a/x"), b"1").unwrap();
        let (check, source) = open(dir.path());
        assert_eq!(check, GameCheck::Folder { has_arm9: false });
        assert_eq!(source.unwrap().read("a/x").unwrap(), b"1");
    }
}
