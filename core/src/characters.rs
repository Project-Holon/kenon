//! New overworld characters: a picture set and an object record added next
//! to the game's own.
//!
//! A character is a record in the object code table (a/0/4/8 member 0: a
//! u32 count, then 28-byte records) and a billboard texture member in
//! a/0/4/9 whose id sits in the record at 0x10. `add_character` copies a
//! record of an existing character (draw type, sizes, shadow, animation),
//! gives the copy a new code and the texture member that new pictures
//! build, and appends both. The texture member is the copied character's own
//! BTX0 with every frame's texels and the palette replaced, so the layout,
//! names and sizes stay the game's.
//!
//! The game looks a record up by index, not by its code field
//! (`FLDMMDL_ObjCodeToTableIndex`, aletheia `fldmmdl_save.c`): codes
//! 0..0xDE, 0x1000..0x1235 and 0x2000..0x2009 map to records 0..798, and any
//! other code maps to record 10. A record added at the end is therefore not
//! reached by any code until something maps a code to it. The zone preload
//! caches (a/0/5/8) match by code, but they are loaded after a map's people
//! are placed, and 387 of the 427 zones use none. New codes start at
//! 0x3000, which the game's table does not use.

use std::collections::HashSet;

use crate::bridge::{Bridge, Error, Member};
use crate::catalog;
use crate::g3d::{find_tex0, Format, Tex0};
use crate::import::decode_png;
use crate::nclr::{bgr555_to_rgba, rgb_to_bgr555};

const OBJECT_CODES: u16 = 48;
const OBJECT_TEXTURES: u16 = 49;
const RECORD: usize = 28;
/// New codes start here, above the game's own (0..0xDE, 0x1000..0x1235,
/// 0x2000..0x2009).
pub const FIRST_NEW_CODE: u16 = 0x3000;

fn bad(msg: impl Into<String>) -> Error {
    Error::Format(msg.into())
}

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

/// A character that was added.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCharacter {
    pub code: u16,
    /// Its a/0/4/9 member.
    pub tex: u16,
    pub frames: usize,
}

/// The records of the object code table: (code, bytes).
fn records(table: &[u8]) -> Vec<(u16, &[u8])> {
    let count = rd32(table, 0).unwrap_or(0) as usize;
    (0..count)
        .map_while(|i| {
            let r = table.get(4 + i * RECORD..4 + (i + 1) * RECORD)?;
            Some((rd16(r, 0)?, r))
        })
        .collect()
}

/// The codes added with `add_character`, with their texture members.
pub fn added_characters(bridge: &Bridge) -> Result<Vec<NewCharacter>, Error> {
    let table = bridge.member(Member {
        arc: OBJECT_CODES,
        index: 0,
    })?;
    let mut out = Vec::new();
    for (code, r) in records(&table) {
        if code < FIRST_NEW_CODE {
            continue;
        }
        let tex = rd16(r, 0x10).unwrap_or(0);
        let frames = bridge
            .member(Member {
                arc: OBJECT_TEXTURES,
                index: tex as usize,
            })
            .ok()
            .and_then(|b| {
                let block = find_tex0(&b).into_iter().next()?;
                Tex0::parse(block).map(|t| t.textures.len())
            })
            .unwrap_or(0);
        out.push(NewCharacter { code, tex, frames });
    }
    Ok(out)
}

/// The shared palette of a set of frames: up to 15 colours (index 0 stays
/// transparent), as 15-bit colours. `hist` is (colour, pixel count).
fn palette_of(mut hist: Vec<(u16, u32)>) -> Vec<u16> {
    hist.sort_unstable();
    if hist.len() <= 15 {
        return hist.into_iter().map(|(c, _)| c).collect();
    }
    // Median cut: split the box with the widest channel at the weighted
    // median until there are 15, then average each box.
    let channel = |c: u16, k: usize| ((c >> (5 * k)) & 31) as i32;
    let mut boxes: Vec<Vec<(u16, u32)>> = vec![hist];
    while boxes.len() < 15 {
        let spread = |b: &Vec<(u16, u32)>| {
            (0..3)
                .map(|k| {
                    let lo = b.iter().map(|&(c, _)| channel(c, k)).min().unwrap_or(0);
                    let hi = b.iter().map(|&(c, _)| channel(c, k)).max().unwrap_or(0);
                    (hi - lo, k)
                })
                .max()
                .unwrap_or((0, 0))
        };
        let Some((at, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by_key(|(_, b)| {
                spread(b).0 * b.iter().map(|&(_, n)| n as i32).sum::<i32>().min(1 << 20)
            })
        else {
            break;
        };
        let mut b = boxes.swap_remove(at);
        let (_, k) = spread(&b);
        b.sort_by_key(|&(c, _)| channel(c, k));
        let total: u64 = b.iter().map(|&(_, n)| n as u64).sum();
        let (mut run, mut cut) = (0u64, 1);
        for (i, &(_, n)) in b.iter().enumerate() {
            run += n as u64;
            if run * 2 >= total {
                cut = (i + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let right = b.split_off(cut);
        boxes.push(b);
        boxes.push(right);
    }
    let mut out: Vec<u16> = boxes
        .iter()
        .map(|b| {
            let total: u64 = b.iter().map(|&(_, n)| n as u64).sum::<u64>().max(1);
            let avg = |k: usize| {
                (b.iter()
                    .map(|&(c, n)| channel(c, k) as u64 * n as u64)
                    .sum::<u64>()
                    + total / 2)
                    / total
            };
            (avg(0) | avg(1) << 5 | avg(2) << 10) as u16
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The nearest palette entry (1-based) to a colour.
fn nearest(palette: &[u16], c: u16) -> u8 {
    let [r, g, b, _] = bgr555_to_rgba(c);
    palette
        .iter()
        .enumerate()
        .min_by_key(|&(_, &p)| {
            let [pr, pg, pb, _] = bgr555_to_rgba(p);
            let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
            d(r, pr) + d(g, pg) + d(b, pb)
        })
        .map(|(i, _)| i as u8 + 1)
        .unwrap_or(1)
}

/// Where each texture's texels lie in a BTX0 member, and where its
/// palettes are: absolute offsets into the member.
struct Layout {
    texels: Vec<usize>,
    palettes: Vec<usize>,
}

fn layout(data: &[u8]) -> Result<Layout, Error> {
    let block = find_tex0(data)
        .into_iter()
        .next()
        .ok_or_else(|| bad("the character has no texture block"))?;
    let base = block.as_ptr() as usize - data.as_ptr() as usize;
    let truncated = || bad("the texture block is cut short");
    let tex_off = rd32(block, 0x14).ok_or_else(truncated)? as usize;
    let pal_off = rd32(block, 0x24).ok_or_else(truncated)? as usize;
    // Texture dictionary at 0x3C: count at +1, size at +2; the records
    // follow the node list, 8 bytes each (u16 offset in 8-byte units).
    let records = |dict: usize, entry: usize| -> Result<Vec<usize>, Error> {
        let count = *block.get(dict + 1).ok_or_else(truncated)? as usize;
        let size = rd16(block, dict + 2).ok_or_else(truncated)? as usize;
        let sub = dict + 8 + (count + 1) * 4;
        if rd16(block, sub).ok_or_else(truncated)? as usize != entry {
            return Err(bad("the texture dictionary has an unknown record size"));
        }
        let first = sub + 4;
        let list = (0..count)
            .map(|i| rd16(block, first + i * entry).map(|v| v as usize * 8))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(truncated)?;
        Ok([list, vec![size]].concat())
    };
    let mut t = records(0x3C, 8)?;
    let size = t.pop().unwrap();
    let mut p = records(0x3C + size, 4)?;
    p.pop();
    Ok(Layout {
        texels: t.into_iter().map(|o| base + tex_off + o).collect(),
        palettes: p.into_iter().map(|o| base + pal_off + o).collect(),
    })
}

/// The member a new texture goes to: the first index no archive member or
/// project file uses.
fn next_texture_member(bridge: &Bridge) -> Result<usize, Error> {
    let mut index = bridge.base_archive(OBJECT_TEXTURES)?.len();
    while bridge
        .project()
        .is_some_and(|p| p.member(OBJECT_TEXTURES, index).is_some())
    {
        index += 1;
    }
    Ok(index)
}

/// Builds the texture member of a character that looks like `like_member`:
/// the same container with `frames` as its textures.
pub fn build_texture(like_member: &[u8], frames: &[Vec<u8>]) -> Result<Vec<u8>, Error> {
    let block = find_tex0(like_member)
        .into_iter()
        .next()
        .ok_or_else(|| bad("the character has no texture block"))?;
    let tex = Tex0::parse(block).ok_or_else(|| bad("the texture block does not parse"))?;
    if tex.textures.is_empty() || tex.textures.iter().any(|t| t.format != Format::Pal16) {
        return Err(bad("only characters with 16-colour pictures can be copied"));
    }
    if frames.len() != tex.textures.len() {
        return Err(bad(format!(
            "this character has {} pictures; {} were given",
            tex.textures.len(),
            frames.len()
        )));
    }
    let pos = layout(like_member)?;
    let mut images = Vec::new();
    for (i, png) in frames.iter().enumerate() {
        let img = decode_png(png)?;
        let t = &tex.textures[i];
        if (img.width, img.height) != (t.width, t.height) {
            return Err(bad(format!(
                "picture {} is {}x{}; this character's pictures are {}x{}",
                i + 1,
                img.width,
                img.height,
                t.width,
                t.height
            )));
        }
        images.push(img);
    }
    // One palette for the whole set, as the game's own characters have.
    let mut counts: std::collections::BTreeMap<u16, u32> = Default::default();
    for img in &images {
        for px in img.pixels.as_chunks::<4>().0 {
            if px[3] >= 128 {
                *counts
                    .entry(rgb_to_bgr555(px[0], px[1], px[2]))
                    .or_default() += 1;
            }
        }
    }
    let colors = palette_of(counts.into_iter().collect());
    let mut out = like_member.to_vec();
    for (i, img) in images.iter().enumerate() {
        let at = *pos
            .texels
            .get(i)
            .ok_or_else(|| bad("a texture is missing"))?;
        let n = img.width * img.height;
        let dest = out
            .get_mut(at..at + n / 2)
            .ok_or_else(|| bad("a texture runs past the member"))?;
        for (k, px) in img.pixels.as_chunks::<4>().0.iter().enumerate() {
            let idx = if px[3] < 128 {
                0
            } else {
                nearest(&colors, rgb_to_bgr555(px[0], px[1], px[2]))
            };
            let byte = &mut dest[k / 2];
            *byte = if k % 2 == 0 {
                (*byte & 0xF0) | idx
            } else {
                (*byte & 0x0F) | idx << 4
            };
        }
    }
    for &at in &pos.palettes {
        let dest = out
            .get_mut(at..at + 32)
            .ok_or_else(|| bad("a palette runs past the member"))?;
        dest.fill(0);
        for (k, c) in colors.iter().enumerate() {
            dest[(k + 1) * 2..(k + 1) * 2 + 2].copy_from_slice(&c.to_le_bytes());
        }
    }
    Ok(out)
}

/// Adds a character that looks like character `like`: its record copied,
/// its pictures `frames` (PNG files, 32x32 or whatever size the copied
/// character's are, in its order and count).
pub fn add_character(
    bridge: &Bridge,
    frames: &[Vec<u8>],
    like: u16,
) -> Result<NewCharacter, Error> {
    let table_at = Member {
        arc: OBJECT_CODES,
        index: 0,
    };
    let mut table = bridge.member(table_at)?;
    let all = records(&table)
        .into_iter()
        .map(|(c, r)| (c, r.to_vec()))
        .collect::<Vec<_>>();
    let (_, original) = all
        .iter()
        .find(|(code, r)| *code == like && r.get(2) == Some(&1))
        .ok_or_else(|| bad(format!("character {like} is not a picture character")))?;
    let tex_like = rd16(original, 0x10).unwrap_or(0);
    let mut record = original.to_vec();
    let used: HashSet<u16> = all.iter().map(|(c, _)| *c).collect();
    let code = (FIRST_NEW_CODE..=u16::MAX)
        .find(|c| !used.contains(c))
        .ok_or_else(|| bad("no object code is free"))?;
    let like_member = bridge.member(Member {
        arc: OBJECT_TEXTURES,
        index: tex_like as usize,
    })?;
    let texture = build_texture(&like_member, frames)?;
    let index = next_texture_member(bridge)?;
    if index > u16::MAX as usize {
        return Err(bad("the texture archive is full"));
    }
    record[0..2].copy_from_slice(&code.to_le_bytes());
    record[0x10..0x12].copy_from_slice(&(index as u16).to_le_bytes());
    table.extend_from_slice(&record);
    let count = (all.len() + 1) as u32;
    table[0..4].copy_from_slice(&count.to_le_bytes());
    catalog::write_member(
        bridge,
        Member {
            arc: OBJECT_TEXTURES,
            index,
        },
        &texture,
    )?;
    catalog::write_member(bridge, table_at, &table)?;
    Ok(NewCharacter {
        code,
        tex: index as u16,
        frames: frames.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn few_colours_are_kept() {
        let p = palette_of(vec![(1, 5), (2, 5), (3, 1)]);
        assert_eq!(p, vec![1, 2, 3]);
    }

    #[test]
    fn many_colours_make_fifteen() {
        let hist: Vec<(u16, u32)> = (0..200u16).map(|i| (i * 3, 1 + (i as u32 % 5))).collect();
        let p = palette_of(hist);
        assert!(p.len() <= 15 && p.len() >= 8, "{}", p.len());
        assert_eq!(nearest(&p, p[3]), 4);
    }
}
