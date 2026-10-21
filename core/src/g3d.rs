//! 3D resources: the BMD0 (models) and BTX0 (textures) containers and the
//! TEX0 texture block they carry. The overworld (map ground, buildings,
//! trees, characters) is 3D, and its look is these textures.
//!
//! Layouts follow eidos `engine/g3d/nsbmd.c` and `texture.c` (which follow
//! aletheia's docs/ASSETS.md):
//!
//! ```text
//! container   magic, u16 BOM, u16 version, u32 size, u16 header size,
//!             u16 block count, then a u32 offset per block
//! dictionary  u8 revision (0), u8 count, u16 size, u16 8, u16 8+(n+1)*4,
//!             (n+1) 4-byte nodes, u16 entry size, u16 names offset,
//!             n records, n 16-byte names
//! TEX0        0x3C-byte header (texel data offset at 0x14, palette data
//!             offset at 0x24), then the texture dictionary (8-byte
//!             records: u16 offset in 8-byte units, u16 upper TEXIMAGE
//!             parameters), then the palette dictionary (4-byte records:
//!             u16 offset in 8-byte units)
//! ```
//!
//! A texture's palette is the one named `<texture>_pl` when the set has
//! it, else the palette whose colours cover every index the texture uses.

use crate::image::Rgba;

/// The GX texture formats (TEXIMAGE_PARAM bits 26-28).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    None,
    A3i5,
    Pal4,
    Pal16,
    Pal256,
    Compressed,
    A5i3,
    Direct,
}

impl Format {
    fn from_code(c: u16) -> Self {
        match c & 7 {
            1 => Format::A3i5,
            2 => Format::Pal4,
            3 => Format::Pal16,
            4 => Format::Pal256,
            5 => Format::Compressed,
            6 => Format::A5i3,
            7 => Format::Direct,
            _ => Format::None,
        }
    }

    /// Bytes of texel data for a texture of this size.
    pub fn bytes(self, w: usize, h: usize) -> usize {
        match self {
            Format::Pal4 => w * h / 4,
            Format::Pal16 => w * h / 2,
            Format::A3i5 | Format::Pal256 | Format::A5i3 => w * h,
            Format::Direct => w * h * 2,
            Format::Compressed => w * h / 16 * 6,
            Format::None => 0,
        }
    }

    /// Colours the format's indices can reach.
    pub fn colors(self) -> usize {
        match self {
            Format::A3i5 => 32,
            Format::Pal4 => 4,
            Format::Pal16 => 16,
            Format::A5i3 => 8,
            _ => 256,
        }
    }
}

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

/// The block with `magic` inside a BMD0 or BTX0 container.
pub fn container_block<'a>(data: &'a [u8], magic: &[u8; 4]) -> Option<&'a [u8]> {
    let count = rd16(data, 0x0E)? as usize;
    (0..count).find_map(|i| {
        let off = rd32(data, 0x10 + i * 4)? as usize;
        let block = data.get(off..)?;
        (block.get(..4)? == magic).then(|| {
            let size = rd32(block, 4).unwrap_or(0) as usize;
            &block[..if size == 0 {
                block.len()
            } else {
                size.min(block.len())
            }]
        })
    })
}

/// Every TEX0 block in a member: in a BTX0 or BMD0 container at its start,
/// or in containers embedded further in (map chunks wrap a BMD0).
pub fn find_tex0(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 16 <= data.len() {
        let magic = &data[at..at + 4];
        if (magic == b"BTX0" || magic == b"BMD0") && rd16(data, at + 4) == Some(0xFEFF) {
            if let Some(b) = container_block(&data[at..], b"TEX0") {
                out.push(b);
            }
            let size = rd32(data, at + 8).unwrap_or(0) as usize;
            at += size.max(16);
            continue;
        }
        at += 4;
    }
    out
}

struct Dict<'a> {
    count: usize,
    entry: usize,
    records: &'a [u8],
    names: &'a [u8],
    size: usize,
}

fn dict(b: &[u8], off: usize) -> Option<Dict<'_>> {
    let count = *b.get(off + 1)? as usize;
    let size = rd16(b, off + 2)? as usize;
    if b[off] != 0 || rd16(b, off + 4)? != 8 || rd16(b, off + 6)? as usize != 8 + (count + 1) * 4 {
        return None;
    }
    let names = off + size - count * 16;
    let sub = off + 8 + (count + 1) * 4;
    let entry = rd16(b, sub)? as usize;
    Some(Dict {
        count,
        entry,
        records: b.get(sub + 4..sub + 4 + count * entry)?,
        names: b.get(names..names + count * 16)?,
        size,
    })
}

impl Dict<'_> {
    fn name(&self, i: usize) -> String {
        let raw = &self.names[i * 16..i * 16 + 16];
        let end = raw.iter().position(|&c| c == 0).unwrap_or(16);
        String::from_utf8_lossy(&raw[..end]).into_owned()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub format: Format,
    /// Colour 0 is transparent.
    pub color0: bool,
    #[serde(skip)]
    offset: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteInfo {
    pub name: String,
    #[serde(skip)]
    offset: usize,
}

/// A parsed TEX0 block.
pub struct Tex0<'a> {
    pub textures: Vec<TextureInfo>,
    pub palettes: Vec<PaletteInfo>,
    texel: &'a [u8],
    pal: &'a [u8],
}

impl<'a> Tex0<'a> {
    pub fn parse(block: &'a [u8]) -> Option<Self> {
        if block.get(..4)? != b"TEX0" {
            return None;
        }
        let tex_off = rd32(block, 0x14)? as usize;
        let pal_off = rd32(block, 0x24)? as usize;
        if tex_off > pal_off || pal_off > block.len() {
            return None;
        }
        let td = dict(block, 0x3C)?;
        let pd = dict(block, 0x3C + td.size)?;
        let textures = (0..td.count)
            .filter(|_| td.entry == 8)
            .map(|i| {
                let r = &td.records[i * 8..i * 8 + 8];
                let params = u16::from_le_bytes([r[2], r[3]]);
                TextureInfo {
                    name: td.name(i),
                    offset: u16::from_le_bytes([r[0], r[1]]) as usize * 8,
                    width: 8 << ((params >> 4) & 7),
                    height: 8 << ((params >> 7) & 7),
                    format: Format::from_code(params >> 10),
                    color0: (params >> 13) & 1 != 0,
                }
            })
            .collect();
        let palettes = (0..pd.count)
            .filter(|_| pd.entry == 4)
            .map(|i| PaletteInfo {
                name: pd.name(i),
                offset: u16::from_le_bytes([pd.records[i * 4], pd.records[i * 4 + 1]]) as usize * 8,
            })
            .collect();
        Some(Tex0 {
            textures,
            palettes,
            texel: &block[tex_off..pal_off],
            pal: &block[pal_off..],
        })
    }

    /// Texture `i` as RGBA, with its palette resolved (see the module
    /// note). `None` when it does not decode.
    pub fn decode(&self, i: usize) -> Option<Rgba> {
        let t = self.textures.get(i)?;
        let bytes = t.format.bytes(t.width, t.height);
        let texel = self.texel.get(t.offset..t.offset + bytes)?;
        if t.format == Format::Direct {
            return decode(t, texel, &[]);
        }
        let need = t.format.colors() * 2;
        let pal_at = |p: &PaletteInfo| -> &[u8] {
            let start = p.offset.min(self.pal.len());
            &self.pal[start..(start + need).min(self.pal.len())]
        };
        let wanted = format!("{}_pl", t.name);
        if let Some(p) = self.palettes.iter().find(|p| p.name == wanted) {
            return decode(t, texel, pal_at(p));
        }
        // Otherwise the palette that covers every index the texture uses.
        let used = max_index(t, texel);
        self.palettes
            .iter()
            .filter(|p| pal_at(p).len() / 2 > used)
            .min_by_key(|p| (pal_at(p).len() / 2).abs_diff(t.format.colors()))
            .or_else(|| self.palettes.first())
            .and_then(|p| decode(t, texel, pal_at(p)))
    }
}

/// The highest colour index a paletted texture uses.
fn max_index(t: &TextureInfo, texel: &[u8]) -> usize {
    let n = t.width * t.height;
    match t.format {
        Format::Pal4 => (0..n)
            .map(|i| (texel[i / 4] >> ((i % 4) * 2) & 3) as usize)
            .max(),
        Format::Pal16 => (0..n)
            .map(|i| (texel[i / 2] >> ((i % 2) * 4) & 15) as usize)
            .max(),
        Format::Pal256 => texel.iter().map(|&b| b as usize).max(),
        Format::A3i5 => texel.iter().map(|&b| (b & 0x1F) as usize).max(),
        Format::A5i3 => texel.iter().map(|&b| (b & 7) as usize).max(),
        _ => Some(0),
    }
    .unwrap_or(0)
}

fn color(pal: &[u8], i: usize) -> [u8; 3] {
    match pal.get(i * 2..i * 2 + 2) {
        Some(c) => {
            let [r, g, b, _] = crate::nclr::bgr555_to_rgba(u16::from_le_bytes([c[0], c[1]]));
            [r, g, b]
        }
        None => [255, 0, 255],
    }
}

/// Decodes texel data with a palette (eidos `g3d_texture_decode`).
fn decode(t: &TextureInfo, texel: &[u8], pal: &[u8]) -> Option<Rgba> {
    let (w, h) = (t.width, t.height);
    let mut img = Rgba::new(w, h);
    let put = |img: &mut Rgba, i: usize, c: [u8; 3], a: u8| {
        img.pixels[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], a]);
    };
    match t.format {
        Format::Direct => {
            for i in 0..w * h {
                let v = u16::from_le_bytes([texel[i * 2], texel[i * 2 + 1]]);
                let [r, g, b, _] = crate::nclr::bgr555_to_rgba(v & 0x7FFF);
                put(
                    &mut img,
                    i,
                    [r, g, b],
                    if v & 0x8000 != 0 { 255 } else { 0 },
                );
            }
        }
        Format::A3i5 | Format::A5i3 => {
            for (i, &b) in texel.iter().enumerate().take(w * h) {
                let (idx, a) = if t.format == Format::A3i5 {
                    ((b & 0x1F) as usize, (b >> 5) as u32 * 255 / 7)
                } else {
                    ((b & 7) as usize, (b >> 3) as u32 * 255 / 31)
                };
                put(&mut img, i, color(pal, idx), a as u8);
            }
        }
        Format::Pal4 | Format::Pal16 | Format::Pal256 => {
            let bits = match t.format {
                Format::Pal4 => 2,
                Format::Pal16 => 4,
                _ => 8,
            };
            let per = 8 / bits;
            for i in 0..w * h {
                let idx = ((texel[i / per] >> ((i % per) * bits)) as usize) & ((1 << bits) - 1);
                let a = if t.color0 && idx == 0 { 0 } else { 255 };
                put(&mut img, i, color(pal, idx), a);
            }
        }
        Format::Compressed => {
            // 4x4 blocks: a u32 of 2-bit selectors, then per block a u16
            // palette address and mode (eidos decode_comp; unused in this
            // game's data, kept from the hardware documentation).
            let blocks = w / 4 * (h / 4);
            let idx = texel.get(blocks * 4..blocks * 6)?;
            for b in 0..blocks {
                let word = u32::from_le_bytes(texel[b * 4..b * 4 + 4].try_into().ok()?);
                let iw = u16::from_le_bytes([idx[b * 2], idx[b * 2 + 1]]);
                let addr = (iw & 0x3FFF) as usize * 2;
                let mode = iw >> 14;
                let c0 = color(pal, addr);
                let c1 = color(pal, addr + 1);
                let mix = |wa: u32, wb: u32| {
                    [0, 1, 2].map(|k| ((c0[k] as u32 * wa + c1[k] as u32 * wb) / (wa + wb)) as u8)
                };
                let (c, alpha3) = match mode {
                    0 => ([c0, c1, mix(5, 3), mix(3, 5)], 255),
                    1 => ([c0, c1, mix(1, 1), [0; 3]], 0),
                    2 => ([c0, c1, color(pal, addr + 2), color(pal, addr + 3)], 255),
                    _ => ([c0, c1, mix(5, 3), [0; 3]], 0),
                };
                let (bx, by) = (b % (w / 4) * 4, b / (w / 4) * 4);
                for p in 0..16 {
                    let sel = ((word >> (p * 2)) & 3) as usize;
                    let i = (by + p / 4) * w + bx + p % 4;
                    put(&mut img, i, c[sel], if sel == 3 { alpha3 } else { 255 });
                }
            }
        }
        Format::None => return None,
    }
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BTX0 with one 8x8 PAL4 texture "t" and its palette "t_pl".
    fn sample() -> Vec<u8> {
        let mk_dict = |entry: usize, records: &[u8], names: &[&str]| {
            let n = names.len();
            let size = 8 + (n + 1) * 4 + 4 + records.len() + n * 16;
            let mut d = vec![0u8, n as u8];
            d.extend_from_slice(&(size as u16).to_le_bytes());
            d.extend_from_slice(&8u16.to_le_bytes());
            d.extend_from_slice(&((8 + (n + 1) * 4) as u16).to_le_bytes());
            d.extend(vec![0u8; (n + 1) * 4]);
            d.extend_from_slice(&(entry as u16).to_le_bytes());
            d.extend_from_slice(&((4 + records.len()) as u16).to_le_bytes());
            d.extend_from_slice(records);
            for name in names {
                let mut raw = [0u8; 16];
                raw[..name.len()].copy_from_slice(name.as_bytes());
                d.extend_from_slice(&raw);
            }
            d
        };
        // PAL4 = 2, 8x8: size codes 0, colour 0 transparent.
        let params: u16 = (2 << 10) | (1 << 13);
        let mut trec = vec![0u8, 0];
        trec.extend_from_slice(&params.to_le_bytes());
        trec.extend_from_slice(&[0; 4]);
        let td = mk_dict(8, &trec, &["t"]);
        let pd = mk_dict(4, &[0, 0, 0, 0], &["t_pl"]);
        let tex_off = 0x3C + td.len() + pd.len();
        let texel: Vec<u8> = (0..16)
            .map(|i| if i == 0 { 0b1110_0100 } else { 0x55 })
            .collect();
        let pal_off = tex_off + texel.len();
        let pal: [u16; 4] = [0, 0x001F, 0x03E0, 0x7C00];
        let mut block = b"TEX0".to_vec();
        block.extend(vec![0u8; 0x3C - 4]);
        block[0x14..0x18].copy_from_slice(&(tex_off as u32).to_le_bytes());
        block[0x24..0x28].copy_from_slice(&(pal_off as u32).to_le_bytes());
        block.extend(td);
        block.extend(pd);
        block.extend(texel);
        for c in pal {
            block.extend_from_slice(&c.to_le_bytes());
        }
        let total = (block.len() + 8) as u32;
        let block_len = block.len() as u32;
        block[4..8].copy_from_slice(&block_len.to_le_bytes());
        let mut file = b"BTX0".to_vec();
        file.extend_from_slice(&0xFEFFu16.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&(total + 12).to_le_bytes());
        file.extend_from_slice(&16u16.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&20u32.to_le_bytes());
        file.extend(block);
        file
    }

    #[test]
    fn decodes_a_pal4_texture() {
        let file = sample();
        let blocks = find_tex0(&file);
        assert_eq!(blocks.len(), 1);
        let tex = Tex0::parse(blocks[0]).unwrap();
        assert_eq!(tex.textures[0].name, "t");
        assert_eq!((tex.textures[0].width, tex.textures[0].height), (8, 8));
        assert_eq!(tex.textures[0].format, Format::Pal4);
        let img = tex.decode(0).unwrap();
        // First byte 0b11_10_01_00: pixels 0..3 are colours 0, 1, 2, 3.
        assert_eq!(img.get(0, 0), [0, 0, 0, 0]);
        assert_eq!(img.get(1, 0), [255, 0, 0, 255]);
        assert_eq!(img.get(2, 0), [0, 255, 0, 255]);
        assert_eq!(img.get(3, 0), [0, 0, 255, 255]);
    }
}
