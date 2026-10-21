//! NFTR (on disk "RTFN"): the game's fonts (`a/0/2/3` members 0-4).
//!
//! Game Freak's variant of the Nitro font. Every rule here follows the
//! decompiled `gf_font.c` (aletheia `src/game/arm9/gf_font.c`); the layout
//! notes are aletheia `tools/assets/font.py`.
//!
//! ```text
//! FINF ("FNIF") body, 20 bytes, read by GF_FONT_LoadResource at 0x18:
//!   0x00 u8 font type     0x01 u8 line height    0x02 u16 null glyph
//!   0x04 u8 default left  0x05 u8 default width  0x06 u8 default advance
//!   0x07 u8 encoding
//!   0x08 u32 CGLP body offset  0x0C u32 CWDH body offset  0x10 u32 CMAP
//!   (offsets from the start of the file)
//! CGLP ("PLGC") body: u8 cell width, u8 cell height, u16 record size,
//!   u8, u8, u16 bits per pixel (2); then one record per glyph:
//!   s8 left, u8 width, u8 advance, then the bitmap, 2 bits per pixel,
//!   most significant first, rows not padded.
//! CWDH ("HDWC") body: the advance width table that
//!   GF_FONT_GetProportionalWidth reads (see `advance_from_table`).
//! CMAP ("PAMC") bodies: u16 first, u16 last, u16 method, u16 reserved,
//!   u32 offset of the next CMAP body (0 ends), then the map data.
//! ```

use crate::nitro::{self, read_u16, read_u32, Error};

pub const MAGIC: &[u8; 4] = b"RTFN";
const FINF: &[u8; 4] = b"FNIF";

/// A character code: the game's text is UTF-16 code units.
pub type Code = u16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmapKind {
    /// Codes `first..=last` map to `base + (code - first)`.
    Direct { base: u16 },
    /// Codes `first..=last` map through a table.
    Table(Vec<u16>),
    /// Sorted (code, glyph) pairs, searched by binary search.
    Scan(Vec<(u16, u16)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmap {
    pub first: Code,
    pub last: Code,
    pub kind: CmapKind,
}

/// One glyph: its metrics and its pixels (values 0-3, row-major,
/// `cell_width * cell_height`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub left: i8,
    pub width: u8,
    pub advance: u8,
    pub pixels: Vec<u8>,
}

impl Glyph {
    /// The pixels as palette indices for a text colour. Pixel 0 stays 0
    /// (transparent); 1, 2 and 3 take the three indices, the mapping
    /// GF_FONT_SetTextColor builds. The game's default is `[1, 2, 0]`.
    pub fn colored(&self, colors: [u8; 3]) -> Vec<u8> {
        self.pixels
            .iter()
            .map(|&p| if p == 0 { 0 } else { colors[p as usize - 1] })
            .collect()
    }
}

/// The game's default text colour (GF_FONT_SetDefaultTextColor).
pub const DEFAULT_COLORS: [u8; 3] = [1, 2, 0];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub font_type: u8,
    pub line_height: u8,
    pub default_advance: u8,
    pub cell_width: u8,
    pub cell_height: u8,
    pub bits_per_pixel: u16,
    record_size: usize,
    records: Vec<u8>,
    width_table: Vec<u8>,
    pub cmaps: Vec<Cmap>,
    /// The glyph shown for a code a map covers but has no glyph for: the
    /// glyph of '?' (GF_FONT_LoadResource).
    pub missing_glyph: u16,
}

impl Font {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let info = nitro::block(bytes, MAGIC, FINF)?;
        if info.len() < 20 {
            return Err(Error::Bad("FINF size"));
        }
        let glyph_at = read_u32(info, 8)? as usize;
        let width_at = read_u32(info, 12)? as usize;
        let map_at = read_u32(info, 16)? as usize;

        let cglp = body_at(bytes, glyph_at, b"PLGC")?;
        let cell_width = cglp.first().copied().ok_or(Error::Truncated)?;
        let cell_height = *cglp.get(1).ok_or(Error::Truncated)?;
        let record_size = read_u16(cglp, 2)? as usize;
        let bits_per_pixel = read_u16(cglp, 6)?;
        if bits_per_pixel != 2 {
            return Err(Error::Bad("glyph bits per pixel (only 2 is used)"));
        }
        let bitmap_size = (cell_width as usize * cell_height as usize * 2).div_ceil(8);
        if record_size != 3 + bitmap_size {
            return Err(Error::Bad("glyph record size"));
        }
        let records = cglp[8..].to_vec();

        // GF_FONT_LoadResource reads from the CWDH body to the CMAP body.
        let width_table = bytes
            .get(width_at..map_at.max(width_at))
            .ok_or(Error::Truncated)?
            .to_vec();

        let mut cmaps = Vec::new();
        let mut at = map_at;
        // In every font of a/0/2/3 the last map covers 0x0000-0xFFFF, so
        // the game never follows its next offset, which points just past
        // the end of the file. Stop there too.
        while at != 0 && at < bytes.len() {
            if cmaps.len() > 1024 {
                return Err(Error::Bad("CMAP chain loops"));
            }
            let body = body_at(bytes, at, b"PAMC")?;
            let first = read_u16(body, 0)?;
            let last = read_u16(body, 2)?;
            let method = read_u16(body, 4)?;
            let data = body.get(12..).ok_or(Error::Truncated)?;
            let words = |n: usize| -> Result<Vec<u16>, Error> {
                (0..n).map(|i| read_u16(data, i * 2)).collect()
            };
            let kind = match method {
                0 => CmapKind::Direct {
                    base: read_u16(data, 0)?,
                },
                1 => CmapKind::Table(words(last.saturating_sub(first) as usize + 1)?),
                2 => {
                    let count = read_u16(data, 0)? as usize;
                    let flat = words(1 + count * 2)?;
                    CmapKind::Scan(flat[1..].chunks(2).map(|p| (p[0], p[1])).collect())
                }
                _ => return Err(Error::Bad("CMAP method")),
            };
            cmaps.push(Cmap { first, last, kind });
            at = read_u32(body, 8)? as usize;
        }

        let mut font = Font {
            font_type: info[0],
            line_height: info[1],
            default_advance: info[6],
            cell_width,
            cell_height,
            bits_per_pixel,
            record_size,
            records,
            width_table,
            cmaps,
            missing_glyph: 0,
        };
        font.missing_glyph = font.glyph_index(b'?' as Code);
        Ok(font)
    }

    pub fn glyph_count(&self) -> usize {
        self.records.len() / self.record_size
    }

    /// GF_FONT_GetGlyphIndex: the first map whose range holds the code
    /// decides. A code it holds but has no glyph for gives the missing
    /// glyph; a code no map holds gives glyph 0.
    pub fn glyph_index(&self, code: Code) -> u16 {
        let Some(map) = self
            .cmaps
            .iter()
            .find(|m| m.first <= code && code <= m.last)
        else {
            return 0;
        };
        match &map.kind {
            CmapKind::Direct { base } => base.wrapping_add(code - map.first),
            CmapKind::Table(table) => table[(code - map.first) as usize],
            CmapKind::Scan(pairs) => pairs
                .binary_search_by_key(&code, |&(c, _)| c)
                .map_or(self.missing_glyph, |i| pairs[i].1),
        }
    }

    /// Glyph `index`, or `None` past the end.
    pub fn glyph(&self, index: u16) -> Option<Glyph> {
        let start = index as usize * self.record_size;
        let record = self.records.get(start..start + self.record_size)?;
        let count = self.cell_width as usize * self.cell_height as usize;
        let bits = &record[3..];
        let pixels = (0..count)
            .map(|i| (bits[i / 4] >> (6 - (i % 4) * 2)) & 3)
            .collect();
        Some(Glyph {
            left: record[0] as i8,
            width: record[1],
            advance: record[2],
            pixels,
        })
    }

    /// The glyph for a character code.
    pub fn glyph_for(&self, code: Code) -> Option<Glyph> {
        self.glyph(self.glyph_index(code))
    }

    /// The advance of a character as the game measures text with a
    /// proportional font (GF_FONT_GetCharWidth).
    pub fn advance(&self, code: Code) -> u8 {
        self.advance_of(self.glyph_index(code))
    }

    /// The advance of glyph `index` from the CWDH table.
    pub fn advance_of(&self, index: u16) -> u8 {
        advance_from_table(&self.width_table, index as u32).unwrap_or(self.default_advance)
    }

    /// The width of a run of codes in pixels.
    pub fn measure(&self, text: &[Code]) -> u32 {
        text.iter().map(|&c| self.advance(c) as u32).sum()
    }
}

/// GF_FONT_GetProportionalWidth. The table starts with three class widths,
/// a pad byte, then three u32 offsets into the table: the class bits (2
/// per glyph, most significant first; class 3 means "look further"), a
/// 512-entry s8 hash (a value of 0 or more is the width; a negative value
/// `-n` selects chain `n - 1`), and the chains (a u8 count, then count
/// records of u16 big-endian glyph index and u8 width). `None` is the
/// font's default advance.
pub fn advance_from_table(table: &[u8], glyph: u32) -> Option<u8> {
    let class_at = read_u32(table, 4).ok()? as usize;
    let hash_at = read_u32(table, 8).ok()? as usize;
    let chain_at = read_u32(table, 12).ok()? as usize;

    let bit = glyph as usize * 2;
    let packed = *table.get(class_at + bit / 8)?;
    let class = (packed >> (6 - (bit & 7))) & 3;
    if class < 3 {
        return Some(table[class as usize]);
    }

    let fold = ((glyph >> 12) ^ (glyph >> 11) ^ (glyph >> 10) ^ (glyph >> 9)) & 1;
    let slot = ((glyph & 0x1FF) ^ (fold << 3)) & 0x1FF;
    let entry = *table.get(hash_at + slot as usize)? as i8;
    if entry >= 0 {
        return Some(entry as u8);
    }

    let mut at = chain_at;
    for _ in 0..(-(entry as i16) - 1) {
        let count = *table.get(at)? as usize;
        at += 1 + count * 3;
    }
    let count = *table.get(at)? as usize;
    (0..count).find_map(|i| {
        let r = table.get(at + 1 + i * 3..at + 4 + i * 3)?;
        (u16::from_be_bytes([r[0], r[1]]) as u32 == glyph).then_some(r[2])
    })
}

/// The body of the chunk whose body starts at `at`, checking its magic.
fn body_at<'a>(bytes: &'a [u8], at: usize, magic: &'static [u8; 4]) -> Result<&'a [u8], Error> {
    let head = at.checked_sub(8).ok_or(Error::Bad("chunk offset"))?;
    if bytes.get(head..head + 4) != Some(magic) {
        return Err(Error::MissingBlock(*magic));
    }
    let size = read_u32(bytes, head + 4)? as usize;
    bytes
        .get(at..(head + size).min(bytes.len()))
        .ok_or(Error::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A font with 6 glyphs of 4x2 cells; glyph g has pixel i = (g + i) % 4.
    /// Maps: 'A'..'C' direct to 0, 'a'..'c' by table (b has no glyph),
    /// and a scan map over 0x3000..0x3100.
    fn sample() -> Vec<u8> {
        let (cw, ch) = (4u8, 2u8);
        let record = 3 + 2; // 8 pixels at 2 bits
        let mut cglp = vec![cw, ch, record as u8, 0, 0, 0, 2, 0];
        for g in 0..6u8 {
            cglp.extend_from_slice(&[(g as i8 - 1) as u8, 3, 4 + g]);
            let px: Vec<u8> = (0..8).map(|i| (g + i) % 4).collect();
            for quad in px.chunks(4) {
                cglp.push(quad[0] << 6 | quad[1] << 4 | quad[2] << 2 | quad[3]);
            }
        }

        // Width table: classes [4, 5, 6]; glyphs 0-2 use them, 3 and 5 go
        // to the hash (3 directly, 5 through chain 0), 4 is class 3 with a
        // chain that does not hold it, so it gets the default.
        let mut cwdh = vec![4, 5, 6, 0];
        let class_at = 16u32;
        let hash_at = class_at + 2;
        let chain_at = hash_at + 512;
        cwdh.extend_from_slice(&class_at.to_le_bytes());
        cwdh.extend_from_slice(&hash_at.to_le_bytes());
        cwdh.extend_from_slice(&chain_at.to_le_bytes());
        cwdh.extend_from_slice(&[0b0001_1011, 0b1111_0000]); // 0,1,2,3 | 3,3
        let mut hash = vec![0u8; 512];
        hash[3] = 7;
        hash[4] = (-1i8) as u8;
        hash[5] = (-2i8) as u8;
        cwdh.extend_from_slice(&hash);
        cwdh.extend_from_slice(&[1, 0x00, 0x09, 1]); // chain 0: glyph 9 -> 1
        cwdh.extend_from_slice(&[1, 0x00, 0x05, 8]); // chain 1: glyph 5 -> 8

        let cmap = |first: u16, last: u16, method: u16, data: &[u16]| {
            let mut body = Vec::new();
            for v in [first, last, method, 0] {
                body.extend_from_slice(&v.to_le_bytes());
            }
            body.extend_from_slice(&0u32.to_le_bytes()); // next, patched below
            for v in data {
                body.extend_from_slice(&v.to_le_bytes());
            }
            body
        };
        let maps = [
            cmap(0x41, 0x43, 0, &[0]),
            cmap(0x61, 0x63, 1, &[3, 0xFFFF, 5]),
            cmap(0x3000, 0x3100, 2, &[2, 0x3005, 1, 0x30FF, 4]),
        ];

        // Lay out: header, FINF, CGLP, CWDH, CMAPs.
        let finf_at = 0x10 + 8;
        let cglp_at = finf_at + 20 + 8;
        let cwdh_at = cglp_at + cglp.len() + 8;
        let mut map_at = vec![cwdh_at + cwdh.len() + 8];
        for m in &maps[..maps.len() - 1] {
            map_at.push(map_at.last().unwrap() + m.len() + 8);
        }
        let mut finf = vec![0, 9, 0, 0, 0, 3, 6, 1];
        for v in [cglp_at, cwdh_at, map_at[0]] {
            finf.extend_from_slice(&(v as u32).to_le_bytes());
        }
        let mut blocks: Vec<(&[u8; 4], Vec<u8>)> =
            vec![(b"FNIF", finf), (b"PLGC", cglp), (b"HDWC", cwdh)];
        for (i, mut m) in maps.into_iter().enumerate() {
            let next = map_at.get(i + 1).copied().unwrap_or(0) as u32;
            m[8..12].copy_from_slice(&next.to_le_bytes());
            blocks.push((b"PAMC", m));
        }
        let refs: Vec<(&[u8; 4], &[u8])> = blocks.iter().map(|(m, b)| (*m, &b[..])).collect();
        nitro::build(MAGIC, &refs)
    }

    #[test]
    fn header_and_glyphs() {
        let font = Font::parse(&sample()).unwrap();
        assert_eq!(font.line_height, 9);
        assert_eq!((font.cell_width, font.cell_height), (4, 2));
        assert_eq!(font.glyph_count(), 6);
        let g = font.glyph(2).unwrap();
        assert_eq!((g.left, g.width, g.advance), (1, 3, 6));
        assert_eq!(g.pixels, vec![2, 3, 0, 1, 2, 3, 0, 1]);
        assert_eq!(g.colored(DEFAULT_COLORS), vec![2, 0, 0, 1, 2, 0, 0, 1]);
        assert_eq!(font.glyph(6), None);
        assert_eq!(font.cmaps.len(), 3);
    }

    #[test]
    fn character_maps() {
        let font = Font::parse(&sample()).unwrap();
        // '?' is in no map, so the missing glyph is 0.
        assert_eq!(font.missing_glyph, 0);
        assert_eq!(font.glyph_index(0x41), 0);
        assert_eq!(font.glyph_index(0x43), 2);
        assert_eq!(font.glyph_index(0x61), 3);
        assert_eq!(font.glyph_index(0x62), 0xFFFF);
        assert_eq!(font.glyph_index(0x63), 5);
        assert_eq!(font.glyph_index(0x3005), 1);
        assert_eq!(font.glyph_index(0x30FF), 4);
        assert_eq!(font.glyph_index(0x3006), font.missing_glyph);
        assert_eq!(font.glyph_index(0x7000), 0);
    }

    #[test]
    fn advance_table() {
        let font = Font::parse(&sample()).unwrap();
        let widths: Vec<u8> = (0..6).map(|g| font.advance_of(g)).collect();
        // Classes 4, 5, 6; hash 7; chain miss -> default 6; chain hit 8.
        assert_eq!(widths, vec![4, 5, 6, 7, 6, 8]);
        assert_eq!(font.measure(&[0x41, 0x42, 0x63]), 4 + 5 + 8);
    }
}
