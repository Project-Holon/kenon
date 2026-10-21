//! RGBA images built from the 2D resources: what the asset bridge serves.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ncgr, nclr, nscr};

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
}
