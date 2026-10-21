//! NSCR (on disk "RCSN"): a background screen (tile map).
//!
//! The SCRN block (on disk "NRCS") body is 12 bytes, then u16 entries:
//!
//! ```text
//! 0x00  u16 width in pixels   0x02  u16 height in pixels
//! 0x04  u32 unknown (0, or 1 in a few a/0/3/4 members; kept as is)
//! 0x08  u32 entry data size in bytes
//! ```
//!
//! An entry is the hardware text BG format: bits 0-9 tile, bit 10 horizontal
//! flip, bit 11 vertical flip, bits 12-15 palette bank (4bpp only).
//!
//! The game copies the entries straight to BG VRAM, so they are in hardware
//! order: a screen wider than 256 pixels is stored as 32x32-tile blocks, not
//! as one row-major map. The index rule is eidos `g2d_bg_screen_index`,
//! transcribed from the game's own helper.

use crate::nitro::{self, read_u16, read_u32, Error};

pub const MAGIC: &[u8; 4] = b"RCSN";
const SCRN: &[u8; 4] = b"NRCS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub tile: u16,
    pub hflip: bool,
    pub vflip: bool,
    pub palette: u8,
}

impl From<u16> for Entry {
    fn from(v: u16) -> Self {
        Entry {
            tile: v & 0x3FF,
            hflip: v & 0x400 != 0,
            vflip: v & 0x800 != 0,
            palette: (v >> 12) as u8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nscr {
    pub width: u16,
    pub height: u16,
    pub unknown: u32,
    /// The entries as stored, in hardware order.
    pub entries: Vec<u16>,
}

impl Nscr {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let body = nitro::block(bytes, MAGIC, SCRN)?;
        let width = read_u16(body, 0)?;
        let height = read_u16(body, 2)?;
        let unknown = read_u32(body, 4)?;
        let size = read_u32(body, 8)? as usize;
        let data = &body[12.min(body.len())..body.len().min(12 + size)];
        let entries = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        Ok(Nscr {
            width,
            height,
            unknown,
            entries,
        })
    }

    pub fn tiles_wide(&self) -> usize {
        self.width as usize / 8
    }

    pub fn tiles_high(&self) -> usize {
        self.height as usize / 8
    }

    /// The entry for tile column `x`, row `y`, if the data holds it.
    pub fn entry(&self, x: usize, y: usize) -> Option<Entry> {
        let index = screen_index(x, y, self.tiles_wide());
        self.entries.get(index).map(|&v| Entry::from(v))
    }
}

/// The hardware position of tile (x, y) in a map `tiles_wide` tiles wide.
/// Maps up to 32 tiles wide are row-major; wider ones are 32x32 blocks, left
/// to right, then top to bottom.
pub fn screen_index(x: usize, y: usize, tiles_wide: usize) -> usize {
    if tiles_wide <= 32 {
        return y * tiles_wide + x;
    }
    let blocks_wide = tiles_wide.div_ceil(32);
    let block = (y / 32) * blocks_wide + x / 32;
    block * 0x400 + (y % 32) * 32 + x % 32
}

#[cfg(test)]
pub(crate) fn build(width: u16, height: u16, entries: &[u16]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&width.to_le_bytes());
    body.extend_from_slice(&height.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&((entries.len() * 2) as u32).to_le_bytes());
    for e in entries {
        body.extend_from_slice(&e.to_le_bytes());
    }
    nitro::build(MAGIC, &[(SCRN, &body)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_bits() {
        let e = Entry::from(0xA805);
        assert_eq!(e.tile, 5);
        assert!(!e.hflip);
        assert!(e.vflip);
        assert_eq!(e.palette, 0xA);
        assert!(Entry::from(0x0400).hflip);
    }

    #[test]
    fn hardware_order() {
        // Matches eidos g2d_bg_screen_index for the 512x256 and 512x512 sizes.
        assert_eq!(screen_index(5, 3, 32), 3 * 32 + 5);
        assert_eq!(screen_index(31, 0, 64), 31);
        assert_eq!(screen_index(32, 0, 64), 0x400);
        assert_eq!(screen_index(33, 1, 64), 0x400 + 32 + 1);
        assert_eq!(screen_index(0, 32, 64), 0x800);
        assert_eq!(screen_index(32, 32, 64), 0xC00);
        for (x, y) in [(40, 5), (3, 20), (63, 31)] {
            let eidos = (x & 0x1f) + (y + (x & !0x1f)) * 32;
            assert_eq!(screen_index(x, y, 64), eidos, "512x256 at {x},{y}");
        }
    }

    #[test]
    fn parse() {
        let entries: Vec<u16> = (0..64 * 64).map(|i| i as u16).collect();
        let nscr = Nscr::parse(&build(512, 512, &entries)).unwrap();
        assert_eq!((nscr.tiles_wide(), nscr.tiles_high()), (64, 64));
        assert_eq!(nscr.entry(32, 0).unwrap().tile, 0x400 & 0x3FF);
        assert_eq!(nscr.entry(1, 1).unwrap().tile, 33);
        assert_eq!(nscr.entry(64, 64), None);
    }
}
