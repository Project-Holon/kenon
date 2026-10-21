//! NCER (on disk "RECN"): cell banks. A cell is a group of OBJ entries
//! (hardware sprites) drawn together: buttons, icons, cursors, bars.
//!
//! The CEBK block (on disk "KBEC") body:
//!
//! ```text
//! 0x00 u16 cell count   0x02 u16 cell record kind (0: 8 bytes, 1: 16 bytes)
//! 0x04 u32 cell records offset (from this body)
//! 0x08 u32 OBJ mapping (0: 2D; 1-4: 1D with 32, 64, 128, 256 KB reach)
//! cell record: u16 OBJ count, u16 attribute, u32 offset of its OBJ entries
//!   (from the end of the cell records); kind 1 adds s16 max x, max y,
//!   min x, min y
//! OBJ entry: the three hardware attribute words, 6 bytes
//! ```
//!
//! Reference: eidos `g2d_ncer_parse` and `oam_decode`. A sprite is four
//! members: NCLR + NCGR + NCER + NANR (inventory section 0); many sets come
//! in three variants, one per OBJ mapping, and a reader picks the one that
//! matches the mapping it renders with.

use crate::nitro::{self, read_u16, read_u32, Error};

pub const MAGIC: &[u8; 4] = b"RECN";
const CEBK: &[u8; 4] = b"KBEC";

/// Width and height of an OBJ by shape (square, wide, tall) and size.
const OBJ_SIZE: [[(u8, u8); 4]; 3] = [
    [(8, 8), (16, 16), (32, 32), (64, 64)],
    [(16, 8), (32, 8), (32, 16), (64, 32)],
    [(8, 16), (8, 32), (16, 32), (32, 64)],
];

/// One hardware OBJ entry, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Obj {
    /// Position relative to the cell origin, as stored (signed 8-bit y,
    /// signed 9-bit x).
    pub x: i16,
    pub y: i16,
    pub width: u8,
    pub height: u8,
    pub hflip: bool,
    pub vflip: bool,
    /// Rotation/scaling on (attr0 bit 8); the flip bits are then a
    /// parameter index, so they read as off.
    pub affine: bool,
    /// Affine with the double-size area (attr0 bits 8-9 both set).
    pub double_size: bool,
    /// 256-colour mode (attr0 bit 13).
    pub bpp8: bool,
    /// The character name: a tile index in units of the OBJ mapping.
    pub tile: u16,
    pub priority: u8,
    pub palette: u8,
}

impl Obj {
    pub fn decode(attr0: u16, attr1: u16, attr2: u16) -> Self {
        let affine = attr0 & 0x100 != 0;
        let shape = ((attr0 >> 14) & 3).min(2) as usize;
        let size = ((attr1 >> 14) & 3) as usize;
        let (width, height) = OBJ_SIZE[shape][size];
        let y = (attr0 & 0xFF) as i16;
        let x = (attr1 & 0x1FF) as i16;
        Obj {
            x: if x >= 256 { x - 512 } else { x },
            y: if y >= 128 { y - 256 } else { y },
            width,
            height,
            hflip: !affine && attr1 & 0x1000 != 0,
            vflip: !affine && attr1 & 0x2000 != 0,
            affine,
            double_size: affine && attr0 & 0x200 != 0,
            bpp8: attr0 & 0x2000 != 0,
            tile: attr2 & 0x3FF,
            priority: ((attr2 >> 10) & 3) as u8,
            palette: (attr2 >> 12) as u8,
        }
    }

    /// Where the OBJ's pixels start relative to the cell origin. The
    /// double-size area centres the sprite in twice its size (eidos
    /// `oam_decode`).
    pub fn origin(&self) -> (i32, i32) {
        if self.double_size {
            (
                self.x as i32 + self.width as i32 / 2,
                self.y as i32 + self.height as i32 / 2,
            )
        } else {
            (self.x as i32, self.y as i32)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub attribute: u16,
    /// min x, min y, max x, max y, when the bank stores them.
    pub bounds: Option<[i16; 4]>,
    pub objs: Vec<Obj>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ncer {
    /// 0: 2D mapping; 1-4: 1D mapping with a 32, 64, 128 or 256 KB reach.
    pub mapping: u32,
    pub cells: Vec<Cell>,
}

impl Ncer {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let body = nitro::block(bytes, MAGIC, CEBK)?;
        let count = read_u16(body, 0)? as usize;
        let extended = read_u16(body, 2)? == 1;
        let records = read_u32(body, 4)? as usize;
        let mapping = read_u32(body, 8)?;
        let stride = if extended { 16 } else { 8 };
        let objs_at = records + count * stride;

        let cells = (0..count)
            .map(|i| {
                let at = records + i * stride;
                let n = read_u16(body, at)? as usize;
                let attribute = read_u16(body, at + 2)?;
                let first = objs_at + read_u32(body, at + 4)? as usize;
                let bounds = if extended {
                    let v = |o| read_u16(body, at + o).map(|v| v as i16);
                    Some([v(12)?, v(14)?, v(8)?, v(10)?])
                } else {
                    None
                };
                let objs = (0..n)
                    .map(|j| {
                        let o = first + j * 6;
                        Ok(Obj::decode(
                            read_u16(body, o)?,
                            read_u16(body, o + 2)?,
                            read_u16(body, o + 4)?,
                        ))
                    })
                    .collect::<Result<_, Error>>()?;
                Ok(Cell {
                    attribute,
                    bounds,
                    objs,
                })
            })
            .collect::<Result<_, Error>>()?;
        Ok(Ncer { mapping, cells })
    }

    /// Bytes per character-name unit: 32 for 2D and 1D 32 KB mapping,
    /// doubling with each larger 1D reach.
    pub fn name_unit(&self) -> usize {
        32 << self.mapping.saturating_sub(1).min(3)
    }

    pub fn is_1d(&self) -> bool {
        self.mapping != 0
    }
}

#[cfg(test)]
pub(crate) fn build(mapping: u32, cells: &[&[[u16; 3]]]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&(cells.len() as u16).to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&16u32.to_le_bytes());
    body.extend_from_slice(&mapping.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // VRAM transfer data: none
    let mut objs = Vec::new();
    for cell in cells {
        body.extend_from_slice(&(cell.len() as u16).to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&(objs.len() as u32).to_le_bytes());
        for attrs in *cell {
            for a in attrs {
                objs.extend_from_slice(&a.to_le_bytes());
            }
        }
    }
    body.extend_from_slice(&objs);
    nitro::build(MAGIC, &[(CEBK, &body)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_decode() {
        // y = -8, x = -16, wide shape size 1 (32x8), hflip, tile 5,
        // priority 2, palette 3.
        let o = Obj::decode(0x4000 | 0xF8, 0x4000 | 0x1000 | 0x1F0, 0x3805);
        assert_eq!((o.x, o.y, o.width, o.height), (-16, -8, 32, 8));
        assert!(o.hflip && !o.vflip && !o.affine);
        assert_eq!((o.tile, o.priority, o.palette), (5, 2, 3));
        // Affine double size: flips read as off, origin moves by half.
        let a = Obj::decode(0x0300 | 10, 0x3000 | 20, 0);
        assert!(a.affine && a.double_size && !a.hflip && !a.vflip);
        assert_eq!(a.origin(), (24, 14));
    }

    #[test]
    fn parse_bank() {
        let ncer = Ncer::parse(&build(
            2,
            &[&[[0, 0, 1]], &[[0, 0, 2], [0x4000, 0x4000, 3]]],
        ))
        .unwrap();
        assert_eq!(ncer.cells.len(), 2);
        assert_eq!(ncer.cells[1].objs[1].tile, 3);
        assert_eq!(ncer.cells[1].objs[1].width, 32);
        assert_eq!(ncer.name_unit(), 64);
        assert!(ncer.is_1d());
    }
}
