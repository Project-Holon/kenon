//! kenon's core: every game file format and the data folder rules.
//!
//! This crate has no Tauri dependency, so tests and command-line tools can
//! use it directly. The front end never parses game formats; it asks the
//! app, and the app asks this crate.

pub mod addresses;
pub mod audio_import;
pub mod bridge;
pub mod catalog;
pub mod changes;
pub mod characters;
pub mod code;
pub mod codemod;
pub mod data_root;
pub mod fielditems;
pub mod g3d;
pub mod import;
pub mod maprender;
pub mod msg;
pub mod nanr;
pub mod nftr;
pub mod nsbmd;
pub mod project;
pub mod props;
pub mod rig;
pub mod scripts;
pub mod sdat;
pub mod sound_add;
pub mod source;
pub mod tables;
pub mod trainer_text;
pub mod world;
pub mod zonemap;

// Mod folders without game data live in morphe (eidos-mod).
pub use eidos_mod::{modcode, modopts, package};

// The cartridge reader and the 2D formats live in morphe's `nds` crate.
pub use nds::{blz, image, lz, narc, ncer, ncgr, nclr, nitro, nscr, rom};
