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

/// The members of one archive.
pub fn members(bridge: &Bridge, arc: u16) -> Result<Vec<MemberInfo>, Error> {
    let archive = bridge.base_archive(arc)?;
    let project = bridge.project();
    Ok(archive
        .iter_raw()
        .enumerate()
        .map(|(index, raw)| {
            let changed = project.is_some_and(|p| p.member(arc, index).is_some());
            let bytes = if changed {
                bridge.member(Member { arc, index }).unwrap_or_default()
            } else {
                lz::decompress_if_compressed(raw).into_owned()
            };
            MemberInfo {
                index,
                kind: kind_of(&bytes),
                size: bytes.len(),
                compressed: lz::decompress(raw).is_ok(),
                changed,
            }
        })
        .collect())
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
    Ok(parsed
        .messages()
        .iter()
        .map(|m| msg::Display(&m.units).to_string())
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

/// The cell bank of the archive that best assembles a sheet: the one whose
/// cells use the most of the sheet's tiles while their pieces stay inside
/// it (the nearest such bank on a tie). `None` when no bank fits.
pub fn best_cell_bank(bridge: &Bridge, chr: Member) -> Result<Option<usize>, Error> {
    let tiles = Ncgr::parse(&bridge.member(chr)?).map_err(|e| Error::Format(e.to_string()))?;
    let count = tiles.tile_count();
    let archive = bridge.base_archive(chr.arc)?;
    let mut best: Option<(usize, usize, usize)> = None;
    for (index, raw) in archive.iter_raw().enumerate() {
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
fn write(bridge: &Bridge, m: Member, bytes: &[u8]) -> Result<(), Error> {
    let project = project_of(bridge)?;
    let base = bridge.base_archive(m.arc)?;
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
    write(bridge, chr, &out)
}

/// Saves an edited palette (0xRRGGBB, quantised to the DS's 15-bit colour).
pub fn save_palette(bridge: &Bridge, pal: Member, colors: &[u32]) -> Result<(), Error> {
    let file = bridge.member(pal)?;
    let colors: Vec<u16> = colors
        .iter()
        .map(|&c| crate::nclr::rgb_to_bgr555((c >> 16) as u8, (c >> 8) as u8, c as u8))
        .collect();
    let out = Palette::with_colors(&file, &colors).map_err(|e| Error::Format(e.to_string()))?;
    write(bridge, pal, &out)
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
    let section = parsed
        .sections
        .first_mut()
        .ok_or_else(|| Error::Format("the bank has no text".into()))?;
    for (message, text) in section.messages.iter_mut().zip(strings) {
        message.units = msg::parse_display(text).map_err(Error::BadPath)?;
    }
    write(bridge, m, &parsed.build())
}

/// Drops the project's version of a member.
pub fn revert(bridge: &Bridge, m: Member) -> Result<(), Error> {
    project_of(bridge)?
        .revert_member(m.arc, m.index)
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(())
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
