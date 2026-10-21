//! kenon's core: every game file format and the data folder rules.
//!
//! This crate has no Tauri dependency, so tests and command-line tools can
//! use it directly. The front end never parses game formats; it asks the
//! app, and the app asks this crate.

pub mod blz;
pub mod bridge;
pub mod catalog;
pub mod changes;
pub mod code;
pub mod data_root;
pub mod fielditems;
pub mod g3d;
pub mod image;
pub mod import;
pub mod lz;
pub mod maprender;
pub mod modcode;
pub mod msg;
pub mod nanr;
pub mod narc;
pub mod ncer;
pub mod ncgr;
pub mod nclr;
pub mod nftr;
pub mod nitro;
pub mod nsbmd;
pub mod nscr;
pub mod package;
pub mod project;
pub mod props;
pub mod rig;
pub mod rom;
pub mod scripts;
pub mod sdat;
pub mod source;
pub mod tables;
pub mod trainer_text;
pub mod world;
pub mod zonemap;
