//! NCGR (on disk "RGCN"): character (tile) data.
//!
//! The CHAR block (on disk "RAHC") body is 24 bytes, then the pixel data:
//!
//! ```text
//! 0x00  u16 tiles high   0x02  u16 tiles wide   (0xFFFF: not given)
//! 0x04  u32 depth (3: 4bpp, 4: 8bpp)
//! 0x08  u32 mapping type
//! 0x0C  u32 layout (0: 8x8 tiles end to end, 1: a linear bitmap)
//! 0x10  u32 data size   0x14  u32 data offset from the start of this body
//! ```
//!
//! 4bpp stores the left pixel of each pair in the low nibble. A sheet with
//! 0xFFFF in either dimension has no size of its own; its tiles are
//! addressed by the cells or the screen that use it. Reference: eidos
//! `g2d_ncgr_parse`, which documents why the linear layout matters (the
//! rig archive's main sheet uses it).

use crate::nitro::{self, read_u16, read_u32, Error};

pub const MAGIC: &[u8; 4] = b"RGCN";
const CHAR: &[u8; 4] = b"RAHC";
/// The "dimension not given" value.
pub const NO_SIZE: u16 = 0xFFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    Bpp4,
    Bpp8,
}

impl Depth {
    pub const fn bytes_per_tile(self) -> usize {
        match self {
            Depth::Bpp4 => 32,
            Depth::Bpp8 => 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ncgr {
    pub tiles_high: u16,
    pub tiles_wide: u16,
    pub depth: Depth,
    pub mapping: u32,
    /// Layout 1: one linear bitmap `tiles_wide * 8` pixels wide.
    pub linear: bool,
    /// The pixel data as stored.
    pub data: Vec<u8>,
}

impl Ncgr {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let body = nitro::block(bytes, MAGIC, CHAR)?;
        let tiles_high = read_u16(body, 0)?;
        let tiles_wide = read_u16(body, 2)?;
        let depth = match read_u32(body, 4)? {
            4 => Depth::Bpp8,
            _ => Depth::Bpp4,
        };
        let mapping = read_u32(body, 8)?;
        let linear = read_u32(body, 12)? == 1;
        let size = read_u32(body, 16)? as usize;
        let offset = read_u32(body, 20)? as usize;
        if offset > body.len() {
            return Err(Error::Bad("CHAR data offset"));
        }
        let data = body[offset..body.len().min(offset + size)].to_vec();
        Ok(Ncgr {
            tiles_high,
            tiles_wide,
            depth,
            mapping,
            linear,
            data,
        })
    }

    pub fn tile_count(&self) -> usize {
        self.data.len() / self.depth.bytes_per_tile()
    }

    /// The sheet's own size in pixels, when it gives one.
    pub fn size_px(&self) -> Option<(usize, usize)> {
        (self.tiles_wide != NO_SIZE && self.tiles_high != NO_SIZE)
            .then(|| (self.tiles_wide as usize * 8, self.tiles_high as usize * 8))
    }

    /// Tile `index` as 64 colour indices, row-major, with the flips applied.
    /// A tile past the end of the data is blank (index 0), as on hardware.
    /// Tiles are counted in storage order, which is what screens and cells
    /// address.
    pub fn tile(&self, index: usize, hflip: bool, vflip: bool) -> [u8; 64] {
        let mut raw = [0u8; 64];
        let size = self.depth.bytes_per_tile();
        if let Some(src) = self.data.get(index * size..index * size + size) {
            match self.depth {
                Depth::Bpp8 => raw.copy_from_slice(src),
                Depth::Bpp4 => {
                    for (i, b) in src.iter().enumerate() {
                        raw[i * 2] = b & 0x0F;
                        raw[i * 2 + 1] = b >> 4;
                    }
                }
            }
        }
        let mut out = [0u8; 64];
        for y in 0..8 {
            for x in 0..8 {
                let sx = if hflip { 7 - x } else { x };
                let sy = if vflip { 7 - y } else { y };
                out[y * 8 + x] = raw[sy * 8 + sx];
            }
        }
        out
    }

    /// The whole sheet as colour indices, `width * height`, when it has a
    /// size. Tiled sheets place tile `t` at column `t % tiles_wide`.
    pub fn indices(&self) -> Option<(usize, usize, Vec<u8>)> {
        let (w, h) = self.size_px()?;
        let mut out = vec![0u8; w * h];
        if self.linear {
            let pixels: Vec<u8> = match self.depth {
                Depth::Bpp8 => self.data.clone(),
                Depth::Bpp4 => self.data.iter().flat_map(|b| [b & 0x0F, b >> 4]).collect(),
            };
            let n = pixels.len().min(out.len());
            out[..n].copy_from_slice(&pixels[..n]);
        } else {
            let wide = self.tiles_wide as usize;
            for t in 0..self.tile_count().min(wide * self.tiles_high as usize) {
                let (bx, by) = (t % wide * 8, t / wide * 8);
                let tile = self.tile(t, false, false);
                for row in 0..8 {
                    out[(by + row) * w + bx..][..8].copy_from_slice(&tile[row * 8..][..8]);
                }
            }
        }
        Some((w, h, out))
    }
}

#[cfg(test)]
pub(crate) fn build(tiles_wide: u16, tiles_high: u16, depth: Depth, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&tiles_high.to_le_bytes());
    body.extend_from_slice(&tiles_wide.to_le_bytes());
    let code: u32 = if depth == Depth::Bpp8 { 4 } else { 3 };
    body.extend_from_slice(&code.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&(data.len() as u32).to_le_bytes());
    body.extend_from_slice(&24u32.to_le_bytes());
    body.extend_from_slice(data);
    nitro::build(MAGIC, &[(CHAR, &body)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_bpp_tile_and_flips() {
        // Tile 0: pixel (x, y) = x for the left half, 0xF elsewhere.
        let mut data = vec![0u8; 64];
        for y in 0..8 {
            data[y * 4] = 0x10; // x0 = 0, x1 = 1
            data[y * 4 + 1] = 0x32;
            data[y * 4 + 2] = 0xFF;
            data[y * 4 + 3] = 0xFF;
        }
        data[32] = 0x05; // tile 1, pixel (0, 0) = 5
        let ncgr = Ncgr::parse(&build(2, 1, Depth::Bpp4, &data)).unwrap();
        assert_eq!(ncgr.tile_count(), 2);
        let t = ncgr.tile(0, false, false);
        assert_eq!(&t[..8], &[0, 1, 2, 3, 15, 15, 15, 15]);
        let h = ncgr.tile(0, true, false);
        assert_eq!(&h[..8], &[15, 15, 15, 15, 3, 2, 1, 0]);
        let v = ncgr.tile(1, false, true);
        assert_eq!(v[56], 5);
        assert_eq!(ncgr.tile(9, false, false), [0; 64]);

        let (w, hgt, px) = ncgr.indices().unwrap();
        assert_eq!((w, hgt), (16, 8));
        assert_eq!(px[8], 5);
        assert_eq!(px[16 + 3], 3);
    }

    #[test]
    fn eight_bpp_and_no_size() {
        let data: Vec<u8> = (0..64).collect();
        let ncgr = Ncgr::parse(&build(NO_SIZE, NO_SIZE, Depth::Bpp8, &data)).unwrap();
        assert_eq!(ncgr.depth, Depth::Bpp8);
        assert_eq!(ncgr.tile(0, false, false)[63], 63);
        assert_eq!(ncgr.size_px(), None);
        assert!(ncgr.indices().is_none());
    }
}
