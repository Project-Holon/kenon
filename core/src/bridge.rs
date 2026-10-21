//! The asset bridge: decoded images and text from the user's data folder,
//! addressed by short paths, so the front end never parses a game format.
//!
//! The app serves these paths on its own URI scheme. A resource is written
//! `arc:member`, with the archive id in decimal (`5:9` is member 9 of
//! `a/0/0/5`). Paths:
//!
//! ```text
//! sheet/{chr}/{pal}/{bank}                 an NCGR (16 tiles wide, or
//!                                          `?cols=N`, when it gives no size)
//! tiles/{chr}/{pal}/{bank}/{cols}/{list}   tiles by index, `cols` per row;
//!                                          the list is comma separated and
//!                                          `-` leaves a tile blank
//! screen/{chr}/{pal}/{scr}                 an NSCR over its tiles
//! cell/{chr}/{pal}/{cer}/{cell}            one cell of an NCER
//! anim/{chr}/{pal}/{cer}/{anr}/{seq}/{n}   the cell shown by frame n of an
//!                                          NANR sequence
//! animinfo/{anr}                           JSON: per sequence, the loop
//!                                          start, play mode, and each
//!                                          frame's cell and duration
//! text/{font}/{pal}/{bank}/{colors}/{text} text in a game font; `colors` is
//!                                          three palette indices `1,2,0`
//! msg/{arc}/{bank}/{index}                 a message in the display form
//!                                          of `msg::Display`, as text
//! cursor/{pal}/{bank}                      the list-menu cursor tile (ARM9
//!                                          data; index 15 is transparent)
//! table/{name}                             JSON: a UI table read from code
//!                                          (see `UI_TABLES`)
//! sound/{seq or name}                      a sequence rendered to WAV
//!                                          (`?seconds=N`, default 30)
//! ```
//!
//! Every image is a PNG with colour 0 transparent. `screen` takes
//! `?zero=opaque` to draw colour 0 as a backdrop; `cell` and `anim` take
//! `?paloffset=N`. Members are LZ-decompressed when they are compressed.
//! `cursor` and `table` need the game's code images.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use crate::code::{CodeImages, Image};
use crate::image::{self, Rgba, Zero};
use crate::msg;
use crate::nanr::Nanr;
use crate::ncer::Ncer;
use crate::ncgr::Ncgr;
use crate::nclr::Palette;
use crate::nftr::Font;
use crate::nscr::Nscr;
use crate::project::Project;
use crate::source::Source;
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

/// UI tables that live in code (inventory section 14): name, image,
/// address, length in bytes.
pub const UI_TABLES: &[(&str, Image, u32, usize)] = &[
    // _PokeTypePltOffset: the palette offset of each type icon.
    ("type_palette", Image::Arm9, 0x0209_FCF8, 18),
    // _WazaKindPltOffset: the palette offset of each move-kind icon.
    ("move_kind_palette", Image::Arm9, 0x0209_FCEC, 3),
    // The text speed table.
    ("text_speed", Image::Arm9, 0x0209_DFB0, 5),
];

/// The list-menu cursor tile (BMP_CURSOR_SetTileBitmap), one 4bpp tile.
const CURSOR_TILE: (Image, u32) = (Image::Arm9, 0x0209_E8CC);

/// The entries a cache keeps before it starts over.
const CACHE_LIMIT: usize = 1024;

/// Serves assets from one data folder, caching archives, fonts and results.
pub struct Bridge {
    source: Source,
    /// The open project, whose files and members win over the game's.
    project: Option<Arc<Project>>,
    code: Option<CodeImages>,
    archives: Mutex<HashMap<u16, Arc<narc::Narc>>>,
    sdat: std::sync::OnceLock<Option<Arc<crate::sdat::Sdat>>>,
    fonts: Mutex<HashMap<Member, Arc<Font>>>,
    served: Mutex<HashMap<String, Arc<Asset>>>,
}

impl Bridge {
    pub fn new(source: impl Into<Source>) -> Self {
        let source = source.into();
        Bridge {
            code: source.code(),
            source,
            project: None,
            sdat: std::sync::OnceLock::new(),
            archives: Mutex::new(HashMap::new()),
            fonts: Mutex::new(HashMap::new()),
            served: Mutex::new(HashMap::new()),
        }
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Reads through a project: its overrides win over the game's files.
    pub fn with_project(mut self, project: Option<Arc<Project>>) -> Self {
        self.project = project;
        self
    }

    pub fn project(&self) -> Option<&Arc<Project>> {
        self.project.as_ref()
    }

    /// The sound archive, read once.
    pub fn sdat(&self) -> Option<Arc<crate::sdat::Sdat>> {
        self.sdat
            .get_or_init(|| {
                let bytes = self.source.read(crate::catalog::SOUND_ARCHIVE).ok()?;
                crate::sdat::Sdat::parse(bytes).ok().map(Arc::new)
            })
            .clone()
    }

    /// Forgets cached results, after the project changed something.
    pub fn invalidate(&self) {
        lock(&self.served).clear();
        lock(&self.fonts).clear();
    }

    /// The game's archive, without the project's overrides.
    pub fn base_archive(&self, arc: u16) -> Result<Arc<narc::Narc>, Error> {
        self.archive(arc)
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
        let query_number = |key: &str| {
            query
                .split('&')
                .find_map(|q| q.strip_prefix(key)?.strip_prefix('='))
                .and_then(|v| v.parse::<usize>().ok())
        };
        let json = |body: String| Asset {
            mime: "application/json",
            bytes: body.into_bytes(),
            origin: None,
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
                let bank = number(3)?;
                // A sheet with no size of its own is laid out 16 tiles wide
                // (or `?cols=N`).
                let img = match image::render_sheet(&tiles, &palette, bank, Zero::Transparent) {
                    Some(img) => img,
                    None => {
                        let list: Vec<Option<usize>> = (0..tiles.tile_count()).map(Some).collect();
                        let cols = query_number("cols").unwrap_or(16).clamp(1, 64);
                        render_tiles(&tiles, &palette, bank, cols.min(list.len().max(1)), &list)
                    }
                };
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
                let offset = query_number("paloffset").unwrap_or(0);
                let drawn = image::render_cell(&bank, cell, &tiles, &palette, offset);
                Ok(Asset {
                    origin: Some(drawn.origin),
                    ..png(drawn.image)
                })
            }
            Some("anim") if parts.len() == 7 => {
                let tiles = self.ncgr(member(1)?)?;
                let palette = self.nclr(member(2)?)?;
                let bank = Ncer::parse(&self.member(member(3)?)?)
                    .map_err(|e| Error::Format(format!("{path}: {e}")))?;
                let anims = Nanr::parse(&self.member(member(4)?)?)
                    .map_err(|e| Error::Format(format!("{path}: {e}")))?;
                let frame = anims
                    .sequences
                    .get(number(5)?)
                    .and_then(|s| s.frames.get(number(6).unwrap_or(0)))
                    .ok_or_else(|| Error::Missing(path.to_string()))?;
                let cell = bank
                    .cells
                    .get(frame.cell as usize)
                    .ok_or_else(|| Error::Missing(format!("{path}: cell {}", frame.cell)))?;
                let offset = query_number("paloffset").unwrap_or(0);
                let drawn = image::render_cell(&bank, cell, &tiles, &palette, offset);
                Ok(Asset {
                    origin: Some((
                        drawn.origin.0 + frame.offset.0 as i32,
                        drawn.origin.1 + frame.offset.1 as i32,
                    )),
                    ..png(drawn.image)
                })
            }
            Some("animinfo") if parts.len() == 2 => {
                let anims = Nanr::parse(&self.member(member(1)?)?)
                    .map_err(|e| Error::Format(format!("{path}: {e}")))?;
                let sequences: Vec<String> = anims
                    .sequences
                    .iter()
                    .map(|s| {
                        let frames: Vec<String> = s
                            .frames
                            .iter()
                            .map(|f| format!(r#"{{"cell":{},"duration":{}}}"#, f.cell, f.duration))
                            .collect();
                        format!(
                            r#"{{"loopStart":{},"playMode":{},"frames":[{}]}}"#,
                            s.loop_start,
                            s.play_mode,
                            frames.join(",")
                        )
                    })
                    .collect();
                Ok(json(format!("[{}]", sequences.join(","))))
            }
            Some("msg") if parts.len() == 4 => {
                let bank = msg::Bank::parse(&self.member(Member {
                    arc: number(1)? as u16,
                    index: number(2)?,
                })?)
                .map_err(|e| Error::Format(format!("{path}: {e}")))?;
                let message = bank
                    .messages()
                    .get(number(3)?)
                    .ok_or_else(|| Error::Missing(path.to_string()))?;
                Ok(Asset {
                    mime: "text/plain; charset=utf-8",
                    bytes: msg::Display(&message.units).to_string().into_bytes(),
                    origin: None,
                })
            }
            Some("cursor") if parts.len() == 3 => {
                let palette = self.nclr(member(1)?)?;
                let raw = self
                    .code_bytes(CURSOR_TILE.0, CURSOR_TILE.1, 32)
                    .ok_or_else(|| Error::Missing("the game's code".to_string()))?;
                let base = number(2)? * 16;
                let mut img = Rgba::new(8, 8);
                for (i, b) in raw.iter().enumerate() {
                    for (k, ci) in [b & 0xF, b >> 4].into_iter().enumerate() {
                        // BMP_CURSOR_Draw blits with 15 as the transparent index.
                        if ci != 0xF {
                            let p = (i * 2 + k) * 4;
                            img.pixels[p..p + 4].copy_from_slice(&palette.rgba(base + ci as usize));
                        }
                    }
                }
                Ok(png(img))
            }
            Some("table") if parts.len() == 2 => {
                let &(_, image, address, len) = UI_TABLES
                    .iter()
                    .find(|t| t.0 == parts[1])
                    .ok_or_else(|| Error::BadPath(path.to_string()))?;
                let bytes = self
                    .code_bytes(image, address, len)
                    .ok_or_else(|| Error::Missing("the game's code".to_string()))?;
                let values: Vec<String> = bytes.iter().map(u8::to_string).collect();
                Ok(json(format!("[{}]", values.join(","))))
            }
            Some("sound") if parts.len() == 2 => {
                let sdat = self
                    .sdat()
                    .ok_or_else(|| Error::Missing("the sound archive".to_string()))?;
                let seq = parts[1].trim_end_matches(".wav");
                let index = match seq.parse::<u32>() {
                    Ok(n) => n,
                    Err(_) => sdat
                        .find(crate::sdat::Kind::Seq, seq)
                        .ok_or_else(|| Error::Missing(path.to_string()))?,
                };
                let seconds = query_number("seconds").unwrap_or(30) as f64;
                let clip = crate::sdat::render(
                    &sdat,
                    index,
                    &crate::sdat::Options {
                        max_seconds: seconds,
                        ..Default::default()
                    },
                )
                .map_err(|e| Error::Format(format!("{path}: {e}")))?;
                Ok(Asset {
                    mime: "audio/wav",
                    bytes: crate::sdat::to_wav(&clip.samples, clip.rate),
                    origin: None,
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

    fn code_bytes(&self, image: Image, address: u32, len: usize) -> Option<Vec<u8>> {
        Some(self.code.as_ref()?.read(image, address, len)?.to_vec())
    }

    fn archive(&self, arc: u16) -> Result<Arc<narc::Narc>, Error> {
        if let Some(a) = lock(&self.archives).get(&arc) {
            return Ok(a.clone());
        }
        let rel =
            narc::archive_path(arc).ok_or_else(|| Error::BadPath(format!("archive {arc}")))?;
        let bytes = self
            .source
            .read(&rel)
            .map_err(|e| Error::Missing(format!("{rel}: {e}")))?;
        let archive =
            Arc::new(narc::Narc::parse(bytes).map_err(|e| Error::Format(format!("{rel}: {e}")))?);
        lock(&self.archives).insert(arc, archive.clone());
        Ok(archive)
    }

    /// A member's bytes, decompressed when compressed.
    pub fn member(&self, m: Member) -> Result<Vec<u8>, Error> {
        if let Some(bytes) = self.project.as_ref().and_then(|p| p.member(m.arc, m.index)) {
            return Ok(lz::decompress_if_compressed(&bytes).into_owned());
        }
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
