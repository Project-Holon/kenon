//! The asset bridge: decoded images and text from the user's data folder,
//! addressed by short paths, so the front end never parses a game format.
//!
//! The app serves these paths on its own URI scheme. A resource is written
//! `arc:member`, with the archive id in decimal (`5:9` is member 9 of
//! `a/0/0/5`). Paths:
//!
//! ```text
//! sheet/{chr}/{pal}/{bank}                 an NCGR with its own size
//! tiles/{chr}/{pal}/{bank}/{cols}/{list}   tiles by index, `cols` per row;
//!                                          the list is comma separated and
//!                                          `-` leaves a tile blank
//! screen/{chr}/{pal}/{scr}                 an NSCR over its tiles
//! cell/{chr}/{pal}/{cer}/{cell}            one cell of an NCER
//! text/{font}/{pal}/{bank}/{colors}/{text} text in a game font; `colors` is
//!                                          three palette indices `1,2,0`
//! ```
//!
//! Every image is a PNG with colour 0 transparent. `screen` takes
//! `?zero=opaque` to draw colour 0 as a backdrop. Members are LZ-decompressed
//! when they are compressed.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use crate::data_root::DataRoot;
use crate::image::{self, Rgba, Zero};
use crate::ncer::Ncer;
use crate::ncgr::Ncgr;
use crate::nclr::Palette;
use crate::nftr::Font;
use crate::nscr::Nscr;
use crate::{lz, narc};

/// A served resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    /// For a cell: where its origin falls on the image.
    pub origin: Option<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The path does not name a resource.
    BadPath(String),
    /// The archive or member could not be read.
    Missing(String),
    /// A member is not the format the path asks for.
    Format(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadPath(s) => write!(f, "bad asset path: {s}"),
            Error::Missing(s) => write!(f, "missing: {s}"),
            Error::Format(s) => write!(f, "wrong format: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// A member reference, `arc:member`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Member {
    pub arc: u16,
    pub index: usize,
}

impl std::str::FromStr for Member {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        let bad = || Error::BadPath(format!("expected arc:member, got {s}"));
        let (arc, index) = s.split_once(':').ok_or_else(bad)?;
        Ok(Member {
            arc: arc.parse().map_err(|_| bad())?,
            index: index.parse().map_err(|_| bad())?,
        })
    }
}

/// The entries a cache keeps before it starts over.
const CACHE_LIMIT: usize = 1024;

/// Serves assets from one data folder, caching archives, fonts and results.
pub struct Bridge {
    root: DataRoot,
    archives: Mutex<HashMap<u16, Arc<narc::Narc>>>,
    fonts: Mutex<HashMap<Member, Arc<Font>>>,
    served: Mutex<HashMap<String, Arc<Asset>>>,
}

impl Bridge {
    pub fn new(root: DataRoot) -> Self {
        Bridge {
            root,
            archives: Mutex::new(HashMap::new()),
            fonts: Mutex::new(HashMap::new()),
            served: Mutex::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> &DataRoot {
        &self.root
    }

    /// Serves a path (with an optional `?query`), from the cache when it can.
    pub fn serve(&self, path: &str) -> Result<Arc<Asset>, Error> {
        let path = path.trim_start_matches('/');
        if let Some(asset) = lock(&self.served).get(path) {
            return Ok(asset.clone());
        }
        let asset = Arc::new(self.render(path)?);
        let mut served = lock(&self.served);
        if served.len() >= CACHE_LIMIT {
            served.clear();
        }
        served.insert(path.to_string(), asset.clone());
        Ok(asset)
    }

    fn render(&self, path: &str) -> Result<Asset, Error> {
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let parts: Vec<&str> = path.split('/').collect();
        let member = |i: usize| -> Result<Member, Error> {
            parts
                .get(i)
                .ok_or_else(|| Error::BadPath(path.to_string()))?
                .parse()
        };
        let number = |i: usize| -> Result<usize, Error> {
            parts
                .get(i)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| Error::BadPath(path.to_string()))
        };
        let png = |img: Rgba| Asset {
            mime: "image/png",
            bytes: encode_png(&img),
            origin: None,
        };
        match parts.first().copied() {
            Some("sheet") if parts.len() == 4 => {
                let tiles = self.ncgr(member(1)?)?;
                let palette = self.nclr(member(2)?)?;
                let img = image::render_sheet(&tiles, &palette, number(3)?, Zero::Transparent)
                    .ok_or_else(|| Error::Format(format!("{path}: the sheet gives no size")))?;
                Ok(png(img))
            }
            Some("tiles") if parts.len() == 6 => {
                let tiles = self.ncgr(member(1)?)?;
                let palette = self.nclr(member(2)?)?;
                let cols = number(4)?.max(1);
                let list = parts[5]
                    .split(',')
                    .map(|t| match t {
                        "-" => Ok(None),
                        t => t
                            .parse()
                            .map(Some)
                            .map_err(|_| Error::BadPath(path.to_string())),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(png(render_tiles(&tiles, &palette, number(3)?, cols, &list)))
            }
            Some("screen") if parts.len() == 4 => {
                let tiles = self.ncgr(member(1)?)?;
                let palette = self.nclr(member(2)?)?;
                let screen = Nscr::parse(&self.member(member(3)?)?)
                    .map_err(|e| Error::Format(format!("{path}: {e}")))?;
                let zero = if query.split('&').any(|q| q == "zero=opaque") {
                    Zero::Opaque
                } else {
                    Zero::Transparent
                };
                Ok(png(image::render_screen(&screen, &tiles, &palette, zero)))
            }
            Some("cell") if parts.len() == 5 => {
                let tiles = self.ncgr(member(1)?)?;
                let palette = self.nclr(member(2)?)?;
                let bank = Ncer::parse(&self.member(member(3)?)?)
                    .map_err(|e| Error::Format(format!("{path}: {e}")))?;
                let cell = bank
                    .cells
                    .get(number(4)?)
                    .ok_or_else(|| Error::Missing(path.to_string()))?;
                let drawn = image::render_cell(&bank, cell, &tiles, &palette);
                Ok(Asset {
                    origin: Some(drawn.origin),
                    ..png(drawn.image)
                })
            }
            Some("text") if parts.len() >= 6 => {
                let font = self.font(member(1)?)?;
                let palette = self.nclr(member(2)?)?;
                let bank = number(3)?;
                let colors = parse_colors(parts[4])
                    .ok_or_else(|| Error::BadPath(format!("{path}: colours")))?;
                // The text may itself hold '/'.
                let text = parts[5..].join("/");
                let units: Vec<u16> = text.encode_utf16().collect();
                Ok(png(render_text(&font, &palette, bank, colors, &units)))
            }
            _ => Err(Error::BadPath(path.to_string())),
        }
    }

    fn archive(&self, arc: u16) -> Result<Arc<narc::Narc>, Error> {
        if let Some(a) = lock(&self.archives).get(&arc) {
            return Ok(a.clone());
        }
        let rel =
            narc::archive_path(arc).ok_or_else(|| Error::BadPath(format!("archive {arc}")))?;
        let archive = Arc::new(
            narc::Narc::open(&self.root.file(&rel))
                .map_err(|e| Error::Missing(format!("{rel}: {e}")))?,
        );
        lock(&self.archives).insert(arc, archive.clone());
        Ok(archive)
    }

    /// A member's bytes, decompressed when compressed.
    pub fn member(&self, m: Member) -> Result<Vec<u8>, Error> {
        let archive = self.archive(m.arc)?;
        let raw = archive
            .raw(m.index)
            .map_err(|e| Error::Missing(format!("{}:{}: {e}", m.arc, m.index)))?;
        Ok(lz::decompress_if_compressed(raw).into_owned())
    }

    fn ncgr(&self, m: Member) -> Result<Ncgr, Error> {
        Ncgr::parse(&self.member(m)?)
            .map_err(|e| Error::Format(format!("{}:{}: {e}", m.arc, m.index)))
    }

    fn nclr(&self, m: Member) -> Result<Palette, Error> {
        Palette::parse(&self.member(m)?)
            .map_err(|e| Error::Format(format!("{}:{}: {e}", m.arc, m.index)))
    }

    /// A font, parsed once.
    pub fn font(&self, m: Member) -> Result<Arc<Font>, Error> {
        if let Some(f) = lock(&self.fonts).get(&m) {
            return Ok(f.clone());
        }
        let font = Arc::new(
            Font::parse(&self.member(m)?)
                .map_err(|e| Error::Format(format!("{}:{}: {e}", m.arc, m.index)))?,
        );
        lock(&self.fonts).insert(m, font.clone());
        Ok(font)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn parse_colors(s: &str) -> Option<[u8; 3]> {
    let v: Vec<u8> = s
        .split(',')
        .map(|c| c.parse().ok())
        .collect::<Option<_>>()?;
    v.try_into().ok()
}

/// Tiles by index into rows of `cols`, colour 0 transparent.
pub fn render_tiles(
    tiles: &Ncgr,
    palette: &Palette,
    bank: usize,
    cols: usize,
    list: &[Option<usize>],
) -> Rgba {
    let rows = list.len().div_ceil(cols).max(1);
    let mut out = Rgba::new(cols * 8, rows * 8);
    let base = match tiles.depth {
        crate::ncgr::Depth::Bpp4 => bank * 16,
        crate::ncgr::Depth::Bpp8 => 0,
    };
    for (n, index) in list.iter().enumerate() {
        let Some(index) = index else { continue };
        let tile = tiles.tile(*index, false, false);
        for (p, &ci) in tile.iter().enumerate() {
            if ci != 0 {
                let x = n % cols * 8 + p % 8;
                let y = n / cols * 8 + p / 8;
                let i = (y * out.width + x) * 4;
                out.pixels[i..i + 4].copy_from_slice(&palette.rgba(base + ci as usize));
            }
        }
    }
    out
}

/// Text as the game prints it (printsys.c): each glyph is drawn at the pen
/// plus its left bearing, `width` columns of the cell, colour 0 left
/// transparent; the pen moves by the glyph's advance. A newline (U+000A or
/// the game's 0xFFFE) starts a new line one line height lower.
pub fn render_text(
    font: &Font,
    palette: &Palette,
    bank: usize,
    colors: [u8; 3],
    text: &[u16],
) -> Rgba {
    let lines: Vec<&[u16]> = text.split(|&c| c == 0x000A || c == 0xFFFE).collect();
    let width = lines
        .iter()
        .map(|l| font.measure(l) as usize)
        .max()
        .unwrap_or(0)
        .max(1);
    let line_height = font.line_height as usize;
    let mut out = Rgba::new(width, (line_height * lines.len()).max(1));
    let cw = font.cell_width as usize;
    for (row, line) in lines.iter().enumerate() {
        let mut pen = 0i32;
        for &code in *line {
            let Some(glyph) = font.glyph_for(code) else {
                continue;
            };
            let indices = glyph.colored(colors);
            for y in 0..(font.cell_height as usize).min(line_height) {
                for x in 0..(glyph.width as usize).min(cw) {
                    let ci = indices[y * cw + x];
                    if ci == 0 {
                        continue;
                    }
                    let px = pen + glyph.left as i32 + x as i32;
                    let py = row * line_height + y;
                    if px < 0 || px as usize >= out.width {
                        continue;
                    }
                    let i = (py * out.width + px as usize) * 4;
                    out.pixels[i..i + 4].copy_from_slice(&palette.rgba(bank * 16 + ci as usize));
                }
            }
            pen += glyph.advance as i32;
        }
    }
    out
}

/// Encodes RGBA as PNG.
pub fn encode_png(img: &Rgba) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder =
            png::Encoder::new(&mut out, img.width.max(1) as u32, img.height.max(1) as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header to memory");
        if img.width == 0 || img.height == 0 {
            writer
                .write_image_data(&[0; 4])
                .expect("PNG data to memory");
        } else {
            writer
                .write_image_data(&img.pixels)
                .expect("PNG data to memory");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ncgr, nclr};

    #[test]
    fn member_refs() {
        assert_eq!(
            "5:9".parse::<Member>().unwrap(),
            Member { arc: 5, index: 9 }
        );
        assert!("5".parse::<Member>().is_err());
        assert!("x:1".parse::<Member>().is_err());
    }

    #[test]
    fn png_round_numbers() {
        let mut img = Rgba::new(2, 1);
        img.pixels[..4].copy_from_slice(&[255, 0, 0, 255]);
        let bytes = encode_png(&img);
        assert_eq!(&bytes[1..4], b"PNG");
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut buf).unwrap();
        assert_eq!(&buf[..8], &[255, 0, 0, 255, 0, 0, 0, 0]);
    }

    #[test]
    fn tiles_layout() {
        let data: Vec<u8> = (0..2u8).flat_map(|t| [(t + 1) * 0x11; 32]).collect();
        let tiles = Ncgr::parse(&ncgr::build(2, 1, ncgr::Depth::Bpp4, &data)).unwrap();
        let palette = Palette::parse(&nclr::build(3, &[0, 0x001F, 0x03E0])).unwrap();
        let img = render_tiles(&tiles, &palette, 0, 2, &[Some(1), None, Some(0)]);
        assert_eq!((img.width, img.height), (16, 16));
        assert_eq!(img.get(0, 0), palette.rgba(2));
        assert_eq!(img.get(8, 0), [0, 0, 0, 0]);
        assert_eq!(img.get(0, 8), palette.rgba(1));
    }

    #[test]
    fn bad_paths() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/a")).unwrap();
        let crate::data_root::DataRootCheck::Ready(root) = crate::data_root::check(dir.path())
        else {
            panic!()
        };
        let bridge = Bridge::new(root);
        assert!(matches!(bridge.serve("nothing"), Err(Error::BadPath(_))));
        assert!(matches!(
            bridge.serve("sheet/5:9/5:3/x"),
            Err(Error::Missing(_)) | Err(Error::BadPath(_))
        ));
        assert!(matches!(
            bridge.serve("sheet/5:9/5:3/0"),
            Err(Error::Missing(_))
        ));
    }
}
