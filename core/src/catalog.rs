//! What is in the game, for the browser: archives and their members, text
//! banks, sounds; and the edits that write a project's overrides.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error, Member};
use crate::ncer::Ncer;
use crate::ncgr::Ncgr;
use crate::nclr::Palette;
use crate::sdat::{Kind, Sdat};
use crate::{lz, msg, narc, nitro};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveInfo {
    pub id: u16,
    pub path: String,
    pub members: usize,
    /// Member format -> count, such as "NCGR" -> 12.
    pub kinds: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberInfo {
    pub index: usize,
    /// The format read from the magic (NCGR, NCLR, NSCR, NCER, NANR, NFTR,
    /// ...), "text" for a message bank, or "data".
    pub kind: String,
    pub size: usize,
    pub compressed: bool,
    /// The open project has its own version.
    pub changed: bool,
}

/// The format of a (decompressed) member from its magic.
pub fn kind_of(bytes: &[u8]) -> String {
    match nitro::magic_of(bytes) {
        Some(m) if m.iter().all(u8::is_ascii_uppercase) => {
            // Nitro magics are stored reversed.
            let mut name = m;
            name.reverse();
            String::from_utf8_lossy(&name).into_owned()
        }
        _ => "data".to_string(),
    }
}

/// Every archive of the game.
pub fn archives(bridge: &Bridge) -> Vec<ArchiveInfo> {
    (0..1000u16)
        .filter_map(|id| {
            let archive = bridge.base_archive(id).ok()?;
            let mut kinds = BTreeMap::new();
            for raw in archive.iter_raw() {
                let member = lz::decompress_if_compressed(raw);
                *kinds.entry(kind_of(&member)).or_insert(0) += 1;
            }
            Some(ArchiveInfo {
                id,
                path: narc::archive_path(id)?,
                members: archive.len(),
                kinds,
            })
        })
        .collect()
}

/// The members of one archive, listed once per game and edit.
pub fn members(bridge: &Bridge, arc: u16) -> Result<Arc<Vec<MemberInfo>>, Error> {
    bridge.cached(&format!("members/{arc}"), || {
        let archive = bridge.base_archive(arc)?;
        let project = bridge.project();
        Ok(archive
            .iter_raw()
            .enumerate()
            .map(|(index, raw)| {
                let changed = project.is_some_and(|p| p.member(arc, index).is_some());
                let unpacked = lz::decompress(raw);
                let compressed = unpacked.is_ok();
                let bytes = if changed {
                    bridge.member(Member { arc, index }).unwrap_or_default()
                } else {
                    unpacked.unwrap_or_else(|_| raw.to_vec())
                };
                MemberInfo {
                    index,
                    kind: kind_of(&bytes),
                    size: bytes.len(),
                    compressed,
                    changed,
                }
            })
            .collect())
    })
}

/// A sound the game can play.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundInfo {
    pub index: u32,
    pub name: String,
    /// music, effect, cry, jingle or other, from the name.
    pub group: &'static str,
}

pub const SOUND_ARCHIVE: &str = "wb_sound_data.sdat";

/// Every sequence of the sound archive, grouped by its name.
pub fn sounds(sdat: &Sdat) -> Vec<SoundInfo> {
    (0..sdat.count(Kind::Seq))
        .filter(|&i| sdat.seq_record(i).is_some())
        .map(|index| {
            let name = sdat
                .name(Kind::Seq, index)
                .map(str::to_string)
                .unwrap_or_else(|| format!("SEQ_{index}"));
            let group = match () {
                _ if name.starts_with("SEQ_BGM") => "music",
                _ if name.starts_with("SEQ_SE") => "effect",
                _ if name.starts_with("SEQ_PV") => "cry",
                _ if name.starts_with("SEQ_ME") => "jingle",
                _ => "other",
            };
            SoundInfo { index, name, group }
        })
        .collect()
}

/// A text bank's strings in the display form (`msg::Display`).
pub fn text_bank(bridge: &Bridge, arc: u16, bank: usize) -> Result<Vec<String>, Error> {
    let bytes = bridge.member(Member { arc, index: bank })?;
    let parsed = msg::Bank::parse(&bytes).map_err(|e| Error::Format(e.to_string()))?;
    let table = bridge.packed_table();
    Ok(parsed
        .messages()
        .iter()
        .map(|m| msg::display(&m.units, table.as_deref()))
        .collect())
}

/// How a sheet is laid out for viewing and editing.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Layout {
    /// Plain tiles in rows (the sheet's own width, else `cols`).
    Grid { cols: Option<usize> },
    /// The cells of a cell bank, assembled as the game draws them, side
    /// by side. A 1D sheet only makes a picture this way: its tiles are
    /// stored OBJ by OBJ.
    Cells { cer: [usize; 2] },
    /// One frame of a battle rig's animation: the sheet is the large
    /// battle picture, assembled as it moves (cells, animation and
    /// multi-cell banks).
    Rig {
        cer: [usize; 2],
        anr: [usize; 2],
        mcr: [usize; 2],
        /// A multi-cell animation that strings the groups together.
        #[serde(default)]
        mna: Option<[usize; 2]>,
        frame: usize,
    },
}

/// The data the pixel editor works on.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetData {
    pub bpp: u8,
    /// One 64-entry array of colour indices per tile.
    pub tiles: Vec<Vec<u8>>,
    /// The palette as 0xRRGGBB.
    pub colors: Vec<u32>,
    /// The picture: `width * height` entries of `tile * 64 + pixel`, or -1
    /// where nothing is drawn.
    pub width: usize,
    pub height: usize,
    pub map: Vec<i32>,
    /// The palette bank of each pixel (4bpp OBJs carry their own).
    pub banks: Vec<u8>,
    /// The sheet's own width in tiles, when it gives one.
    pub tiles_wide: Option<u16>,
}

pub fn sheet_data(
    bridge: &Bridge,
    chr: Member,
    pal: Member,
    layout: &Layout,
) -> Result<SheetData, Error> {
    let tiles = Ncgr::parse(&bridge.member(chr)?).map_err(|e| Error::Format(e.to_string()))?;
    let palette = Palette::parse(&bridge.member(pal)?).map_err(|e| Error::Format(e.to_string()))?;
    let count = tiles.tile_count();
    let (width, height, map, banks) = match layout {
        Layout::Grid { cols } => {
            let cols = match tiles.size_px() {
                Some((w, _)) => w / 8,
                None => cols.unwrap_or(16),
            }
            .clamp(1, count.max(1));
            let rows = count.div_ceil(cols).max(1);
            let (w, h) = (cols * 8, rows * 8);
            let mut map = vec![-1; w * h];
            for t in 0..count {
                for p in 0..64 {
                    let x = t % cols * 8 + p % 8;
                    let y = t / cols * 8 + p / 8;
                    map[y * w + x] = (t * 64 + p) as i32;
                }
            }
            (w, h, map, vec![0; w * h])
        }
        Layout::Cells { cer } => {
            let bank = Ncer::parse(&bridge.member(member_of(*cer))?)
                .map_err(|e| Error::Format(e.to_string()))?;
            let maps: Vec<_> = bank
                .cells
                .iter()
                .map(|c| crate::image::cell_map(&bank, c, &tiles))
                .filter(|m| m.width > 0)
                .collect();
            // Cells in rows no wider than 256 pixels, 8 pixels apart.
            let gap = 8;
            let mut placed = Vec::new();
            let (mut x, mut y, mut row_h, mut w) = (0, 0, 0, 0);
            for m in &maps {
                if x > 0 && x + m.width > 256 {
                    x = 0;
                    y += row_h + gap;
                    row_h = 0;
                }
                placed.push((x, y));
                x += m.width + gap;
                w = w.max(x - gap);
                row_h = row_h.max(m.height);
            }
            let h = (y + row_h).max(1);
            let w = w.max(1);
            let mut map = vec![-1; w * h];
            let mut banks = vec![0; w * h];
            for (m, (px, py)) in maps.iter().zip(placed) {
                for (i, entry) in m.pixels.iter().enumerate() {
                    if let Some((src, b)) = entry {
                        let at = (py + i / m.width) * w + px + i % m.width;
                        map[at] = *src as i32;
                        banks[at] = *b;
                    }
                }
            }
            (w, h, map, banks)
        }
        Layout::Rig {
            cer,
            anr,
            mcr,
            mna,
            frame,
        } => {
            let mut refs = vec![
                format!("{}:{}", chr.arc, chr.index),
                format!("{}:{}", pal.arc, pal.index),
                format!("{}:{}", cer[0], cer[1]),
                format!("{}:{}", anr[0], anr[1]),
                format!("{}:{}", mcr[0], mcr[1]),
            ];
            if let Some(m) = mna {
                refs.push(format!("{}:{}", m[0], m[1]));
            }
            let parts: Vec<&str> = refs.iter().map(String::as_str).collect();
            let anim = bridge.rig(&parts)?;
            let frame_map = anim
                .maps
                .get(*frame)
                .ok_or_else(|| Error::Missing(format!("rig frame {frame}")))?;
            let map = frame_map
                .iter()
                .map(|s| s.map_or(-1, |(src, _)| src as i32))
                .collect();
            let banks = frame_map.iter().map(|s| s.map_or(0, |(_, b)| b)).collect();
            (anim.width, anim.height, map, banks)
        }
    };
    let sized = |v: u16| (v != crate::ncgr::NO_SIZE).then_some(v);
    Ok(SheetData {
        bpp: match tiles.depth {
            crate::ncgr::Depth::Bpp4 => 4,
            crate::ncgr::Depth::Bpp8 => 8,
        },
        tiles: (0..count)
            .map(|t| tiles.tile(t, false, false).to_vec())
            .collect(),
        colors: palette
            .colors
            .iter()
            .map(|&c| {
                let [r, g, b, _] = crate::nclr::bgr555_to_rgba(c);
                (r as u32) << 16 | (g as u32) << 8 | b as u32
            })
            .collect(),
        width,
        height,
        map,
        banks,
        tiles_wide: sized(tiles.tiles_wide),
    })
}

impl SheetData {
    /// The picture as RGBA, colour 0 transparent.
    pub fn render(&self) -> crate::image::Rgba {
        let mut img = crate::image::Rgba::new(self.width, self.height);
        for (i, &src) in self.map.iter().enumerate() {
            if src < 0 {
                continue;
            }
            let v = self.tiles[src as usize / 64][src as usize % 64] as usize;
            if v == 0 {
                continue;
            }
            let base = if self.bpp == 4 {
                self.banks[i] as usize * 16
            } else {
                0
            };
            let c = self.colors.get(base + v).copied().unwrap_or(0xFF00FF);
            img.pixels[i * 4..i * 4 + 4].copy_from_slice(&[
                (c >> 16) as u8,
                (c >> 8) as u8,
                c as u8,
                255,
            ]);
        }
        img
    }
}

fn member_of([arc, index]: [usize; 2]) -> Member {
    Member {
        arc: arc as u16,
        index,
    }
}

/// A background screen of an archive with the tiles and palette that draw
/// it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenInfo {
    pub scr: usize,
    pub chr: usize,
    pub pal: usize,
    pub width: u16,
    pub height: u16,
}

/// Every screen of an archive, paired as aletheia's background.py pairs
/// them: the nearest tile sheet (before it, else after) that holds every
/// tile the screen uses, and the nearest palette with every bank it uses.
pub fn screens(bridge: &Bridge, arc: u16) -> Result<Arc<Vec<ScreenInfo>>, Error> {
    bridge.cached(&format!("screens/{arc}"), || {
        let archive = bridge.base_archive(arc)?;
        let mut sheets = Vec::new();
        let mut palettes = Vec::new();
        let mut screens = Vec::new();
        for (i, raw) in archive.iter_raw().enumerate() {
            let bytes = lz::decompress_if_compressed(raw);
            match bytes.get(..4) {
                Some(b"RGCN") => {
                    if let Ok(t) = Ncgr::parse(&bytes) {
                        sheets.push((i, t.tile_count()));
                    }
                }
                Some(b"RLCN") => {
                    if let Ok(p) = Palette::parse(&bytes) {
                        palettes.push((i, p.banks().max(1)));
                    }
                }
                Some(b"RCSN") => {
                    if let Ok(s) = crate::nscr::Nscr::parse(&bytes) {
                        let tiles = s
                            .entries
                            .iter()
                            .map(|&e| (e & 0x3FF) as usize)
                            .max()
                            .unwrap_or(0);
                        let bank = s
                            .entries
                            .iter()
                            .map(|&e| (e >> 12) as usize)
                            .max()
                            .unwrap_or(0);
                        screens.push((i, s.width, s.height, tiles, bank));
                    }
                }
                _ => {}
            }
        }
        let nearest = |list: &[(usize, usize)], at: usize, need: usize| -> Option<usize> {
            let fits: Vec<usize> = list
                .iter()
                .filter(|(_, n)| *n > need)
                .map(|(i, _)| *i)
                .collect();
            fits.iter()
                .rev()
                .find(|&&i| i < at)
                .or_else(|| fits.iter().find(|&&i| i > at))
                .copied()
        };
        Ok(screens
            .into_iter()
            .filter_map(|(scr, width, height, tiles, bank)| {
                Some(ScreenInfo {
                    scr,
                    chr: nearest(&sheets, scr, tiles)?,
                    pal: nearest(&palettes, scr, bank)?,
                    width,
                    height,
                })
            })
            .collect())
    })
}

/// The animation bank that plays a cell bank: the nearest animation bank
/// after it (else before it, within 8 members) whose frames only name
/// cells the bank has.
pub fn anim_bank(bridge: &Bridge, arc: u16, cer: usize) -> Result<Option<usize>, Error> {
    let archive = bridge.base_archive(arc)?;
    let bank = Ncer::parse(&bridge.member(Member { arc, index: cer })?)
        .map_err(|e| Error::Format(e.to_string()))?;
    let fits = |i: usize| -> bool {
        let Ok(raw) = archive.raw(i) else {
            return false;
        };
        let bytes = lz::decompress_if_compressed(raw);
        if !bytes.starts_with(b"RNAN") {
            return false;
        }
        crate::nanr::Nanr::parse(&bytes).is_ok_and(|a| {
            !a.sequences.is_empty()
                && a.sequences
                    .iter()
                    .flat_map(|s| &s.frames)
                    .all(|f| (f.cell as usize) < bank.cells.len())
        })
    };
    let after = (cer + 1..(cer + 9).min(archive.len())).find(|&i| fits(i));
    Ok(after.or_else(|| (cer.saturating_sub(8)..cer).rev().find(|&i| fits(i))))
}

/// The cell bank of the archive that best assembles a sheet: the one whose
/// cells use the most of the sheet's tiles while their pieces stay inside
/// it (the nearest such bank on a tie). `None` when no bank fits.
pub fn best_cell_bank(bridge: &Bridge, chr: Member) -> Result<Option<usize>, Error> {
    let tiles = Ncgr::parse(&bridge.member(chr)?).map_err(|e| Error::Format(e.to_string()))?;
    let count = tiles.tile_count();
    let archive = bridge.base_archive(chr.arc)?;
    let mut best: Option<(usize, usize, usize)> = None;
    // Banks sit near the sheets they draw; looking further only costs time
    // in archives with thousands of members.
    let near = chr.index.saturating_sub(64)..(chr.index + 64).min(archive.len());
    for (index, raw) in archive
        .iter_raw()
        .enumerate()
        .skip(near.start)
        .take(near.len())
    {
        let bytes = lz::decompress_if_compressed(raw);
        if !bytes.starts_with(b"RECN") {
            continue;
        }
        let Ok(bank) = Ncer::parse(&bytes) else {
            continue;
        };
        let mut used = vec![false; count];
        let mut fits = true;
        for cell in &bank.cells {
            let map = crate::image::cell_map(&bank, cell, &tiles);
            let drawn = map.pixels.iter().flatten().count();
            for &(src, _) in map.pixels.iter().flatten() {
                used[src / 64] = true;
            }
            // cell_map drops pieces that reach past the sheet: a bank that
            // loses most of a cell does not belong to this sheet.
            let area: usize = cell
                .objs
                .iter()
                .map(|o| o.width as usize * o.height as usize)
                .sum();
            if drawn * 2 < area {
                fits = false;
            }
        }
        if !fits {
            continue;
        }
        let score = used.iter().filter(|&&u| u).count();
        let distance = index.abs_diff(chr.index);
        if score > 0 && best.is_none_or(|(s, d, _)| score > s || (score == s && distance < d)) {
            best = Some((score, distance, index));
        }
    }
    Ok(best.map(|(_, _, i)| i))
}

fn project_of(bridge: &Bridge) -> Result<&Arc<crate::project::Project>, Error> {
    bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open: changes need a mod to go in".into()))
}

/// Writes a member into the project, compressed as the game's was.
pub fn write_member(bridge: &Bridge, m: Member, bytes: &[u8]) -> Result<(), Error> {
    let project = project_of(bridge)?;
    let base = bridge.base_archive(m.arc)?;
    // Back to the game's bytes: drop the override instead of keeping a
    // copy that changes nothing.
    if base
        .raw(m.index)
        .is_ok_and(|raw| *lz::decompress_if_compressed(raw) == *bytes)
    {
        return revert(bridge, m);
    }
    let packed = match base.raw(m.index).ok().and_then(lz::sniff) {
        Some((kind, _)) if lz::decompress(base.raw(m.index).unwrap()).is_ok() => {
            lz::compress(kind, bytes)
        }
        _ => bytes.to_vec(),
    };
    project
        .write_member(m.arc, m.index, &packed)
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(())
}

/// Saves edited tiles (64 colour indices each, as `sheet_data` gave them).
pub fn save_sheet(bridge: &Bridge, chr: Member, tiles: &[Vec<u8>]) -> Result<(), Error> {
    let file = bridge.member(chr)?;
    let mut ncgr = Ncgr::parse(&file).map_err(|e| Error::Format(e.to_string()))?;
    for (t, pixels) in tiles.iter().enumerate().take(ncgr.tile_count()) {
        for (p, &v) in pixels.iter().enumerate().take(64) {
            ncgr.set_pixel(t, p % 8, p / 8, v);
        }
    }
    let out = Ncgr::with_data(&file, &ncgr.data).map_err(|e| Error::Format(e.to_string()))?;
    write_member(bridge, chr, &out)
}

/// Saves an edited palette (0xRRGGBB, quantised to the DS's 15-bit colour).
pub fn save_palette(bridge: &Bridge, pal: Member, colors: &[u32]) -> Result<(), Error> {
    let file = bridge.member(pal)?;
    let colors: Vec<u16> = colors
        .iter()
        .map(|&c| crate::nclr::rgb_to_bgr555((c >> 16) as u8, (c >> 8) as u8, c as u8))
        .collect();
    let out = Palette::with_colors(&file, &colors).map_err(|e| Error::Format(e.to_string()))?;
    write_member(bridge, pal, &out)
}

/// Saves a text bank from strings in the display form.
pub fn save_text_bank(
    bridge: &Bridge,
    arc: u16,
    bank: usize,
    strings: &[String],
) -> Result<(), Error> {
    let m = Member { arc, index: bank };
    let mut parsed =
        msg::Bank::parse(&bridge.member(m)?).map_err(|e| Error::Format(e.to_string()))?;
    let table = bridge.packed_table();
    let section = parsed
        .sections
        .first_mut()
        .ok_or_else(|| Error::Format("the bank has no text".into()))?;
    for (message, text) in section.messages.iter_mut().zip(strings) {
        message.units = msg::parse(text, table.as_deref()).map_err(Error::BadPath)?;
    }
    write_member(bridge, m, &parsed.build())
}

/// Drops the project's version of a member.
pub fn revert(bridge: &Bridge, m: Member) -> Result<(), Error> {
    project_of(bridge)?
        .revert_member(m.arc, m.index)
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(())
}

/// A picture the browser lists by name: its tiles, its palette and the
/// other palettes it can be shown with (shiny, female), and the palette
/// bank offset the game adds.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedSheet {
    pub label: String,
    pub chr: usize,
    pub pal: usize,
    /// Other palettes, by name.
    pub palettes: Vec<(String, usize)>,
    pub palette_offset: usize,
    /// The cell bank that draws it, when it is a sprite; `None` for a sheet
    /// that is a picture as it is.
    pub cells: Option<usize>,
    /// The animation bank that plays its cells, when it has one.
    pub anims: Option<usize>,
    /// A battle rig that animates the large picture: sheet, cell bank,
    /// animation bank, multi-cell bank (members of the same archive).
    /// A sixth member, when present, is a multi-cell animation (NMAR) that
    /// strings the rig's groups into one move.
    pub rig: Option<Vec<usize>>,
}

/// Named pictures for the archives whose member layout is known
/// (docs/game-data-map.md): Pokemon icons, battle sprites, item icons and
/// trainer sprites. Names come from the ROM's text banks and tables;
/// `None` for other archives.
pub fn named_sheets(bridge: &Bridge, arc: u16) -> Result<Arc<Option<Vec<NamedSheet>>>, Error> {
    bridge.cached(&format!("named/{arc}"), || make_named_sheets(bridge, arc))
}

fn make_named_sheets(bridge: &Bridge, arc: u16) -> Result<Option<Vec<NamedSheet>>, Error> {
    let bank = |b: usize| {
        crate::tables::bank_strings(
            bridge,
            &crate::tables::BankRef {
                archive: 2,
                bank: b,
            },
        )
    };
    let archive = bridge.base_archive(arc)?;
    let size = |i: usize| archive.raw(i).map_or(0, <[u8]>::len);
    let code = bridge.code();
    let species = bank(70);
    let name = |n: usize| match n {
        650 => "Egg".to_string(),
        651 => "Manaphy Egg".to_string(),
        n => species
            .get(n)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Form {}", n.saturating_sub(651))),
    };
    let sheets = match arc {
        // Pokemon icons: species n at 7 + 2n, the female icon after it
        // when that member is not empty; the palette bank comes from the
        // nibble table in the ARM9 image (low nibble male, high female).
        7 => {
            let mut out = Vec::new();
            let mut n = 0;
            while 7 + 2 * n < archive.len() {
                let nibble = code
                    .and_then(|c| c.read(crate::code::Image::Arm9, 0x0209_E474 + n as u32, 1))
                    .map_or(0, |b| b[0]);
                out.push(NamedSheet {
                    label: name(n),
                    chr: 7 + 2 * n,
                    pal: 0,
                    palettes: Vec::new(),
                    palette_offset: (nibble & 0xF) as usize,
                    cells: Some(2),
                    anims: Some(1),
                    rig: None,
                });
                if size(8 + 2 * n) > 0 {
                    out.push(NamedSheet {
                        label: format!("{} (female)", name(n)),
                        chr: 8 + 2 * n,
                        pal: 0,
                        palettes: Vec::new(),
                        palette_offset: (nibble >> 4) as usize,
                        cells: Some(2),
                        anims: Some(1),
                        rig: None,
                    });
                }
                n += 1;
            }
            out
        }
        // Battle sprites: 20 members per species; front sheet +0 (female
        // +1), back sheet +9 (female +10), palette +18, shiny palette +19.
        4 => {
            let mut out = Vec::new();
            let mut n = 0;
            while 20 * n + 19 < archive.len() {
                let block = 20 * n;
                let shiny = vec![("Shiny".to_string(), block + 19)];
                for (role, side) in [
                    (0, "front"),
                    (1, "front, female"),
                    (9, "back"),
                    (10, "back, female"),
                ] {
                    if size(block + role) > 0 {
                        // The small sheets are 12x12 tiles in 1D order; the
                        // one-cell banks at members 4 and 13 (block 0) lay
                        // out the whole 96x96 picture for every species. The
                        // per-species banks at +4 and +13 cut the large
                        // battle texture into animated parts instead.
                        let cells = if role < 9 { 4 } else { 13 };
                        // The large picture (+2 front, +11 back; female
                        // +3, +12 when present) and its rig: cells +4,
                        // animation +5, multi-cell +6 (back +13..+15).
                        let female = role == 1 || role == 10;
                        let big = if role < 9 { block + 2 } else { block + 11 } + female as usize;
                        let parts = if role < 9 { block + 4 } else { block + 13 };
                        let rig = (size(big) > 0).then(|| vec![big, parts, parts + 1, parts + 2]);
                        out.push(NamedSheet {
                            label: format!("{} ({side})", name(n)),
                            chr: block + role,
                            pal: block + 18,
                            palettes: shiny.clone(),
                            palette_offset: 0,
                            cells: Some(cells),
                            anims: None,
                            rig,
                        });
                    }
                }
                n += 1;
            }
            out
        }
        // Item icons: the ARM9 table pairs each item with a tile and a
        // palette member; items sharing a picture are listed together.
        25 => {
            let Some(code) = code else {
                return Ok(None);
            };
            let items = bank(54);
            let mut by_tiles: Vec<(usize, usize, Vec<String>)> = Vec::new();
            for (item, label) in items.iter().enumerate().take(627) {
                let Some(pair) =
                    code.read(crate::code::Image::Arm9, 0x0209_EB6C + item as u32 * 4, 4)
                else {
                    break;
                };
                let chr = u16::from_le_bytes([pair[0], pair[1]]) as usize;
                let pal = u16::from_le_bytes([pair[2], pair[3]]) as usize;
                let label = label.clone();
                if label.is_empty() || label == "???" {
                    continue;
                }
                match by_tiles.iter_mut().find(|(c, p, _)| *c == chr && *p == pal) {
                    Some((_, _, names)) => names.push(label),
                    None => by_tiles.push((chr, pal, vec![label])),
                }
            }
            by_tiles
                .into_iter()
                .map(|(chr, pal, names)| NamedSheet {
                    label: if names.len() > 1 {
                        format!("{} (+{} more)", names[0], names.len() - 1)
                    } else {
                        names[0].clone()
                    },
                    chr,
                    pal,
                    palettes: Vec::new(),
                    palette_offset: 0,
                    cells: Some(1),
                    anims: Some(0),
                    rig: None,
                })
                .collect()
        }
        // Trainer sprites: 8 members per graphic, sheet +0, palette +7; the
        // classes that use a graphic name it (class -> graphic table in the
        // ARM9 image).
        72 => {
            let classes = bank(191);
            let mut names: Vec<Vec<String>> = vec![Vec::new(); archive.len() / 8];
            if let Some(code) = code {
                for (class, label) in classes.iter().enumerate().take(106) {
                    if let Some(g) =
                        code.read(crate::code::Image::Arm9, 0x020A_023C + class as u32, 1)
                    {
                        if let Some(list) = names.get_mut(g[0] as usize) {
                            if !label.is_empty() && !list.contains(label) {
                                list.push(label.clone());
                            }
                        }
                    }
                }
            }
            names
                .into_iter()
                .enumerate()
                .map(|(g, list)| NamedSheet {
                    label: if list.is_empty() {
                        format!("Trainer graphic {g}")
                    } else {
                        list.join(" / ")
                    },
                    chr: 8 * g,
                    pal: 8 * g + 7,
                    palettes: Vec::new(),
                    palette_offset: 0,
                    // The small sheet is laid out by the shared one-cell
                    // bank; a graphic's own bank at +2 is either a copy of
                    // it or, for some (N), the rig of the battle texture.
                    cells: Some(2),
                    anims: None,
                    // The battle picture +1 with cells +2, animation +3,
                    // multi-cell +4. Only animated graphics have a bank of
                    // their own (2D, laid on the picture's frames); the rest
                    // carry the shared one-cell bank, which does not fit the
                    // picture.
                    rig: archive
                        .raw(8 * g + 2)
                        .ok()
                        .and_then(|raw| {
                            crate::ncer::Ncer::parse(&crate::lz::decompress_if_compressed(raw)).ok()
                        })
                        .filter(|bank| bank.mapping == 4)
                        .map(|_| vec![8 * g + 1, 8 * g + 2, 8 * g + 3, 8 * g + 4, 8 * g + 5]),
                })
                .collect()
        }
        // Back sprites (the player throwing a ball): the same 8-member
        // layout as trainer sprites, but every one is animated, so the sheet
        // shown is the battle picture (a plain bitmap of the frames) with
        // its rig.
        73 => (0..archive.len() / 8)
            .map(|g| NamedSheet {
                label: format!("Back sprite {}", g + 1),
                chr: 8 * g + 1,
                pal: 8 * g + 7,
                palettes: Vec::new(),
                palette_offset: 0,
                cells: None,
                anims: None,
                rig: Some(vec![8 * g + 1, 8 * g + 2, 8 * g + 3, 8 * g + 4, 8 * g + 5]),
            })
            .collect(),
        _ => return Ok(None),
    };
    Ok(Some(sheets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(kind_of(b"RGCN...."), "NCGR");
        assert_eq!(kind_of(b"\x10\x00\x00\x00"), "data");
        assert_eq!(kind_of(b""), "data");
    }
}
