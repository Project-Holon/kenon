//! SDAT sound: the container, its sequences, banks and waves, and a renderer
//! that turns a sequence into PCM for the app's UI sound effects.
//!
//! - [`container`]: SDAT header, SYMB names, INFO records, FAT and FILE.
//! - [`sseq`]: the sequence bytecode and its track interpreter.
//! - [`sbnk`]: instrument banks (single, PSG, drum set, key split).
//! - [`swav`]: wave archives and PCM8, PCM16 and IMA-ADPCM decoding.
//! - [`render`]: tempo map, voices, envelopes and the mixer. Output is
//!   interleaved stereo `i16` at 32768 Hz.
//! - [`wav`]: a WAV file writer for the rendered clip.
//!
//! The sound archive is `wb_sound_data.sdat` in the data folder. A sequence
//! is found by number or by its SYMB name:
//!
//! ```no_run
//! use kenon_core::sdat::{self, Kind, Sdat};
//! let sdat = Sdat::open(std::path::Path::new("wb_sound_data.sdat")).unwrap();
//! let n = sdat.find(Kind::Seq, "SEQ_SE_SELECT1").unwrap();
//! let clip = sdat::render(&sdat, n, &sdat::Options::default()).unwrap();
//! let wav = sdat::to_wav(&clip.samples, clip.rate);
//! # let _ = wav;
//! ```

pub mod container;
pub mod render;
pub mod sbnk;
pub mod sseq;
pub mod swav;
pub mod wav;

pub use container::{BankRecord, Kind, PlayerRecord, Sdat, SeqRecord};
pub use render::{render, Clip, Options, OUTPUT_RATE};
pub use sbnk::{Region, Sbnk, Source};
pub use sseq::{Note, Sseq};
pub use swav::{Encoding, Swar, Swav};
pub use wav::to_wav;
