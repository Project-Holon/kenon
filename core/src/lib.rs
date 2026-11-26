//! kenon's core: every game file format and the data folder rules.
//!
//! This crate has no Tauri dependency, so tests and command-line tools can
//! use it directly. The front end never parses game formats; it asks the
//! app, and the app asks this crate.

pub mod addresses;
pub mod arm;
pub mod audio_import;
pub mod battle;
pub mod bridge;
pub mod catalog;
pub mod changes;
pub mod characters;
pub mod code;
pub mod codemod;
pub mod codenames;
pub mod codeview;
pub mod content;
pub mod cries;
pub mod data_root;
pub mod dex_search;
pub mod effscript;
pub mod fielditems;
pub mod grid;
pub mod g3d;
pub mod handlers;
pub mod import;
pub mod machines;
pub mod move_anims;
pub mod maprender;
pub mod nanr;
pub mod nftr;
pub mod nsbmd;
pub mod preview;
pub mod project;
pub mod regional_dex;
pub mod props;
pub mod rig;
pub mod romhack;
pub mod scripts;
pub mod sdat;
pub mod shops;
pub mod sound_add;
pub mod spa;
pub mod source;
pub mod species_add;
pub mod tables;
pub mod trainer_mons;
pub mod trainer_text;
pub mod types;
pub mod world;
pub mod zonemap;

// Mod folders without game data live in morphe (eidos-mod).
pub use eidos_mod::{modcode, modopts, package};

// The cartridge reader and the 2D formats live in morphe's `nds` crate.
pub use nds::{blz, image, lz, msg, narc, ncer, ncgr, nclr, nitro, nscr, rom};
