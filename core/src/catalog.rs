//! What is in the game, for the browser: archives and their members, text
//! banks, sounds; and the edits that write a project's overrides.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
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

/// The data the pixel editor works on: a sheet's colour indices tile by
/// tile and its palette.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetData {
    pub tiles_wide: Option<u16>,
    pub tiles_high: Option<u16>,
    pub bpp: u8,
    /// One 64-entry array of colour indices per tile.
    pub tiles: Vec<Vec<u8>>,
    /// The palette as 0xRRGGBB.
    pub colors: Vec<u32>,
}

pub fn sheet_data(bridge: &Bridge, chr: Member, pal: Member) -> Result<SheetData, Error> {
    let tiles = Ncgr::parse(&bridge.member(chr)?).map_err(|e| Error::Format(e.to_string()))?;
    let palette = Palette::parse(&bridge.member(pal)?).map_err(|e| Error::Format(e.to_string()))?;
    let sized = |v: u16| (v != crate::ncgr::NO_SIZE).then_some(v);
    Ok(SheetData {
        tiles_wide: sized(tiles.tiles_wide),
        tiles_high: sized(tiles.tiles_high),
        bpp: match tiles.depth {
            crate::ncgr::Depth::Bpp4 => 4,
            crate::ncgr::Depth::Bpp8 => 8,
        },
        tiles: (0..tiles.tile_count())
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
    })
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
