//! RGBA images built from the 2D resources: what the asset bridge serves.

use crate::ncer::{Cell, Ncer};
use crate::ncgr::{Depth, Ncgr};
use crate::nclr::Palette;
use crate::nscr::Nscr;

/// 8-bit RGBA, row-major, no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Rgba {
    pub fn new(width: usize, height: usize) -> Self {
        Rgba {
            width,
            height,
            pixels: vec![0; width * height * 4],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.width + x) * 4;
        self.pixels[i..i + 4].try_into().unwrap()
    }

    fn put(&mut self, x: usize, y: usize, rgba: [u8; 4]) {
        let i = (y * self.width + x) * 4;
        self.pixels[i..i + 4].copy_from_slice(&rgba);
    }
}

/// How colour index 0 of each bank is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zero {
    /// Transparent, as on a hardware BG or OBJ layer.
    Transparent,
    /// The palette's colour, as a backdrop would show it.
    Opaque,
}

/// Renders a screen against its tiles and palette. 4bpp tiles use the bank
/// in each entry; 8bpp tiles use the whole palette.
pub fn render_screen(screen: &Nscr, tiles: &Ncgr, palette: &Palette, zero: Zero) -> Rgba {
    let (tw, th) = (screen.tiles_wide(), screen.tiles_high());
    let mut out = Rgba::new(tw * 8, th * 8);
    for ty in 0..th {
        for tx in 0..tw {
            let Some(entry) = screen.entry(tx, ty) else {
                continue;
            };
            let tile = tiles.tile(entry.tile as usize, entry.hflip, entry.vflip);
            let base = match tiles.depth {
                Depth::Bpp4 => entry.palette as usize * 16,
                Depth::Bpp8 => 0,
            };
            for (i, &ci) in tile.iter().enumerate() {
                if ci == 0 && zero == Zero::Transparent {
                    continue;
                }
                out.put(
                    tx * 8 + i % 8,
                    ty * 8 + i / 8,
                    palette.rgba(base + ci as usize),
                );
            }
        }
    }
    out
}

/// Renders a sheet that has its own size with one palette bank (4bpp) or the
/// whole palette (8bpp). `None` when the sheet gives no size.
pub fn render_sheet(tiles: &Ncgr, palette: &Palette, bank: usize, zero: Zero) -> Option<Rgba> {
    let (w, h, indices) = tiles.indices()?;
    let base = match tiles.depth {
        Depth::Bpp4 => bank * 16,
        Depth::Bpp8 => 0,
    };
    let mut out = Rgba::new(w, h);
    for (i, &ci) in indices.iter().enumerate() {
        if ci == 0 && zero == Zero::Transparent {
            continue;
        }
        out.put(i % w, i / w, palette.rgba(base + ci as usize));
    }
    Some(out)
}

/// A cell drawn on its own canvas, and where the cell origin falls on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellImage {
    pub image: Rgba,
    pub origin: (i32, i32),
}

/// Draws a cell of `bank` with its characters and palette. Character names
/// are in the bank's mapping units; a 1D bank lays each OBJ's tiles out in
/// a row, a 2D bank in rows of the sheet's width (32 tiles when the sheet
/// gives none). A lower OBJ priority value, then a lower OBJ index, draws
/// in front, as on hardware. Colour 0 is transparent. 4bpp OBJs use their
/// palette bank plus `palette_offset` (the offset a sprite actor adds, for
/// example per type for the type icons). Affine OBJs are drawn unrotated.
pub fn render_cell(
    bank: &Ncer,
    cell: &Cell,
    tiles: &Ncgr,
    palette: &Palette,
    palette_offset: usize,
) -> CellImage {
    if cell.objs.is_empty() {
        return CellImage {
            image: Rgba::new(0, 0),
            origin: (0, 0),
        };
    }
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for obj in &cell.objs {
        let (x, y) = obj.origin();
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + obj.width as i32);
        y1 = y1.max(y + obj.height as i32);
    }
    let mut image = Rgba::new((x1 - x0) as usize, (y1 - y0) as usize);

    let tile_size = tiles.depth.bytes_per_tile();
    let row_2d = match tiles.size_px() {
        Some((w, _)) => w / 8 * tile_size,
        None => 32 * tile_size,
    };
    let mut order: Vec<usize> = (0..cell.objs.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse((cell.objs[i].priority, i)));
    for i in order {
        let obj = &cell.objs[i];
        let (ox, oy) = obj.origin();
        let start = obj.tile as usize * bank.name_unit();
        let (tw, th) = (obj.width as usize / 8, obj.height as usize / 8);
        let base = match tiles.depth {
            Depth::Bpp4 => (obj.palette as usize + palette_offset) * 16,
            Depth::Bpp8 => 0,
        };
        for ty in 0..th {
            for tx in 0..tw {
                let at = if bank.is_1d() {
                    start + (ty * tw + tx) * tile_size
                } else {
                    start + ty * row_2d + tx * tile_size
                };
                let tile = tiles.tile_at(at, obj.hflip, obj.vflip);
                // A flipped OBJ also mirrors the tile order.
                let dx = if obj.hflip { tw - 1 - tx } else { tx };
                let dy = if obj.vflip { th - 1 - ty } else { ty };
                for (p, &ci) in tile.iter().enumerate() {
                    if ci == 0 {
                        continue;
                    }
                    let px = ox - x0 + (dx * 8 + p % 8) as i32;
                    let py = oy - y0 + (dy * 8 + p / 8) as i32;
                    image.put(px as usize, py as usize, palette.rgba(base + ci as usize));
                }
            }
        }
    }
    CellImage {
        image,
        origin: (-x0, -y0),
    }
}

/// Where each pixel of a drawn cell comes from: the tile and the pixel in
/// it (`tile * 64 + pixel`, flips applied), and the palette bank, or
/// `None` where no OBJ covers it. The editor paints through this map, so a
/// sprite is edited as the game shows it.
pub struct CellMap {
    pub width: usize,
    pub height: usize,
    pub origin: (i32, i32),
    pub pixels: Vec<Option<(usize, u8)>>,
}

/// The pixel map of a cell, with the same placement and drawing order as
/// [`render_cell`].
pub fn cell_map(bank: &Ncer, cell: &Cell, tiles: &Ncgr) -> CellMap {
    if cell.objs.is_empty() {
        return CellMap {
            width: 0,
            height: 0,
            origin: (0, 0),
            pixels: Vec::new(),
        };
    }
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for obj in &cell.objs {
        let (x, y) = obj.origin();
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + obj.width as i32);
        y1 = y1.max(y + obj.height as i32);
    }
    let (width, height) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut pixels = vec![None; width * height];
    let tile_size = tiles.depth.bytes_per_tile();
    let row_2d = match tiles.size_px() {
        Some((w, _)) => w / 8 * tile_size,
        None => 32 * tile_size,
    };
    let mut order: Vec<usize> = (0..cell.objs.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse((cell.objs[i].priority, i)));
    for i in order {
        let obj = &cell.objs[i];
        let (ox, oy) = obj.origin();
        let start = obj.tile as usize * bank.name_unit();
        let (tw, th) = (obj.width as usize / 8, obj.height as usize / 8);
        for ty in 0..th {
            for tx in 0..tw {
                let at = if bank.is_1d() {
                    start + (ty * tw + tx) * tile_size
                } else {
                    start + ty * row_2d + tx * tile_size
                };
                let tile = at / tile_size;
                if tile >= tiles.tile_count() {
                    continue;
                }
                let dx = if obj.hflip { tw - 1 - tx } else { tx };
                let dy = if obj.vflip { th - 1 - ty } else { ty };
                for p in 0..64 {
                    let (lx, ly) = (p % 8, p / 8);
                    let sx = if obj.hflip { 7 - lx } else { lx };
                    let sy = if obj.vflip { 7 - ly } else { ly };
                    let px = (ox - x0) as usize + dx * 8 + lx;
                    let py = (oy - y0) as usize + dy * 8 + ly;
                    pixels[py * width + px] = Some((tile * 64 + sy * 8 + sx, obj.palette));
                }
            }
        }
    }
    CellMap {
        width,
        height,
        origin: (-x0, -y0),
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ncer, ncgr, nclr, nscr};

    #[test]
    fn screen_uses_entry_bank_and_flip() {
        // One 4bpp tile: index 1 at (0, 0), 0 elsewhere.
        let mut data = vec![0u8; 32];
        data[0] = 0x01;
        let tiles = Ncgr::parse(&ncgr::build(1, 1, Depth::Bpp4, &data)).unwrap();
        let mut colors = vec![0u16; 32];
        colors[1] = 0x001F; // bank 0, colour 1: red
        colors[17] = 0x7C00; // bank 1, colour 1: blue
        colors[16] = 0x03E0; // bank 1, colour 0: green
        let palette = Palette::parse(&nclr::build(3, &colors)).unwrap();
        // Two tiles: plain, then bank 1 with horizontal flip.
        let screen = Nscr::parse(&nscr::build(16, 8, &[0x0000, 0x1400])).unwrap();

        let img = render_screen(&screen, &tiles, &palette, Zero::Transparent);
        assert_eq!((img.width, img.height), (16, 8));
        assert_eq!(img.get(0, 0), [255, 0, 0, 255]);
        assert_eq!(img.get(15, 0), [0, 0, 255, 255]);
        assert_eq!(img.get(8, 0), [0, 0, 0, 0]);

        let img = render_screen(&screen, &tiles, &palette, Zero::Opaque);
        assert_eq!(img.get(8, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn sheet() {
        let data = vec![0x11u8; 32];
        let tiles = Ncgr::parse(&ncgr::build(1, 1, Depth::Bpp4, &data)).unwrap();
        let palette = Palette::parse(&nclr::build(3, &[0, 0x7FFF])).unwrap();
        let img = render_sheet(&tiles, &palette, 0, Zero::Transparent).unwrap();
        assert_eq!(img.get(7, 7), [255, 255, 255, 255]);
    }

    #[test]
    fn cell_with_two_objs_and_flip() {
        // Four 4bpp tiles; tile t is filled with colour t + 1.
        let data: Vec<u8> = (0..4u8).flat_map(|t| [(t + 1) * 0x11; 32]).collect();
        let tiles = Ncgr::parse(&ncgr::build(4, 1, Depth::Bpp4, &data)).unwrap();
        let colors: Vec<u16> = (0..16).map(|i| i * 0x0421).collect();
        let palette = Palette::parse(&nclr::build(3, &colors)).unwrap();
        // 1D 32K: OBJ 0 is 16x8 at (-8, -4) from tile 0 with hflip;
        // OBJ 1 is 8x8 at (0, 4) from tile 3, behind OBJ 0.
        let bank = Ncer::parse(&ncer::build(
            0,
            &[&[[0x4000 | 0xFC, 0x1000 | 0x1F8, 0], [4, 0, 3]]],
        ))
        .unwrap();
        let cell = render_cell(&bank, &bank.cells[0], &tiles, &palette, 0);
        assert_eq!((cell.image.width, cell.image.height), (16, 16));
        assert_eq!(cell.origin, (8, 4));
        // hflip: tile 1 (colour 2) now on the left, tile 0 (colour 1) right.
        assert_eq!(cell.image.get(0, 0), palette.rgba(2));
        assert_eq!(cell.image.get(15, 0), palette.rgba(1));
        assert_eq!(cell.image.get(8, 8), palette.rgba(4));
        assert_eq!(cell.image.get(0, 15), [0, 0, 0, 0]);
    }

    #[test]
    fn cell_map_matches_render() {
        let data: Vec<u8> = (0..4u8).flat_map(|t| [(t + 1) * 0x11; 32]).collect();
        let tiles = Ncgr::parse(&ncgr::build(4, 1, Depth::Bpp4, &data)).unwrap();
        let bank = Ncer::parse(&ncer::build(
            0,
            &[&[[0x4000 | 0xFC, 0x1000 | 0x1F8, 0], [4, 0, 3]]],
        ))
        .unwrap();
        let map = cell_map(&bank, &bank.cells[0], &tiles);
        assert_eq!((map.width, map.height, map.origin), (16, 16, (8, 4)));
        // hflip: the left half of OBJ 0 comes from tile 1, mirrored.
        assert_eq!(map.pixels[0], Some((64 + 7, 0)));
        assert_eq!(map.pixels[15], Some((0, 0)));
        assert_eq!(map.pixels[8 * 16 + 8], Some((3 * 64, 0)));
        assert_eq!(map.pixels[15 * 16], None);
    }
}
