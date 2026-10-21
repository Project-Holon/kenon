//! NCLR (on disk "RLCN"): palettes.
//!
//! The PLTT block (on disk "TTLP") body:
//!
//! ```text
//! 0x00  u32 format (3: 16-colour banks, 4: 256 colours)
//! 0x04  u32 extended-palette flag
//! 0x08  u32 colour data size in bytes
//! 0x0C  u32 colour data offset from the start of this body
//! ```
//!
//! Colours are BGR555. The expansion to 8 bits is `v * 255 / 31`,
//! truncating, the same integer rule as eidos `g2d_palette_append_nclr` and
//! aletheia's asset tools. Text palettes are one 16-colour bank (32 bytes);
//! most others hold 16 banks (512 bytes) (inventory section 0).

use crate::nitro::{self, read_u32, Error};

pub const MAGIC: &[u8; 4] = b"RLCN";
const PLTT: &[u8; 4] = b"TTLP";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// 3 for 16-colour banks, 4 for 256 colours.
    pub format: u32,
    pub extended: bool,
    /// The colours as stored (BGR555).
    pub colors: Vec<u16>,
}

impl Palette {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let body = nitro::block(bytes, MAGIC, PLTT)?;
        let format = read_u32(body, 0)?;
        let extended = read_u32(body, 4)? != 0;
        let size = read_u32(body, 8)? as usize;
        let offset = read_u32(body, 12)? as usize;
        if offset > body.len() {
            return Err(Error::Bad("PLTT colour offset"));
        }
        // Some files declare more than they hold; read what is there.
        let data = &body[offset..body.len().min(offset + size)];
        let colors = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        Ok(Palette {
            format,
            extended,
            colors,
        })
    }

    /// The file with its colours replaced (same count), every other byte kept.
    pub fn with_colors(file: &[u8], colors: &[u16]) -> Result<Vec<u8>, Error> {
        let at = nitro::block_offset(file, MAGIC, PLTT)?;
        let size = read_u32(&file[at..], 8)? as usize;
        let offset = read_u32(&file[at..], 12)? as usize;
        let start = at + offset;
        let len = size.min(file.len().saturating_sub(start)) / 2 * 2;
        if colors.len() * 2 != len {
            return Err(Error::Bad("new palette has another size"));
        }
        let mut out = file.to_vec();
        for (i, c) in colors.iter().enumerate() {
            out[start + i * 2..start + i * 2 + 2].copy_from_slice(&c.to_le_bytes());
        }
        Ok(out)
    }

    /// Colour `index` as RGBA; out of range gives magenta, so a wrong
    /// pairing shows instead of hiding.
    pub fn rgba(&self, index: usize) -> [u8; 4] {
        self.colors
            .get(index)
            .map_or([255, 0, 255, 255], |&c| bgr555_to_rgba(c))
    }

    /// The number of 16-colour banks.
    pub fn banks(&self) -> usize {
        self.colors.len() / 16
    }
}

/// The nearest BGR555 colour to 8-bit RGB.
pub fn rgb_to_bgr555(r: u8, g: u8, b: u8) -> u16 {
    let q = |v: u8| ((v as u16 * 31 + 127) / 255) & 0x1F;
    q(r) | q(g) << 5 | q(b) << 10
}

pub fn bgr555_to_rgba(c: u16) -> [u8; 4] {
    let expand = |v: u16| ((v as u32 & 0x1F) * 255 / 31) as u8;
    [expand(c), expand(c >> 5), expand(c >> 10), 255]
}

#[cfg(test)]
pub(crate) fn build(format: u32, colors: &[u16]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&format.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&((colors.len() * 2) as u32).to_le_bytes());
    body.extend_from_slice(&16u32.to_le_bytes());
    for c in colors {
        body.extend_from_slice(&c.to_le_bytes());
    }
    nitro::build(MAGIC, &[(PLTT, &body)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion() {
        assert_eq!(bgr555_to_rgba(0x0000), [0, 0, 0, 255]);
        assert_eq!(bgr555_to_rgba(0x7FFF), [255, 255, 255, 255]);
        assert_eq!(bgr555_to_rgba(0x001F), [255, 0, 0, 255]);
        assert_eq!(bgr555_to_rgba(0x03E0), [0, 255, 0, 255]);
        assert_eq!(bgr555_to_rgba(0x7C00), [0, 0, 255, 255]);
        // 16 * 255 / 31 = 131.6, truncated.
        assert_eq!(bgr555_to_rgba(16)[0], 131);
    }

    #[test]
    fn parse() {
        let colors: Vec<u16> = (0..32).collect();
        let pal = Palette::parse(&build(3, &colors)).unwrap();
        assert_eq!(pal.format, 3);
        assert_eq!(pal.colors, colors);
        assert_eq!(pal.banks(), 2);
        assert_eq!(pal.rgba(99), [255, 0, 255, 255]);
    }

    #[test]
    fn write_back_and_quantise() {
        let file = build(3, &[0, 1, 2]);
        let out = Palette::with_colors(&file, &[0x7FFF, 0, 0x001F]).unwrap();
        assert_eq!(
            Palette::parse(&out).unwrap().colors,
            vec![0x7FFF, 0, 0x001F]
        );
        assert!(Palette::with_colors(&file, &[0]).is_err());
        for c in [0u16, 0x7FFF, 0x1234 & 0x7FFF, 0x03E0] {
            let [r, g, b, _] = bgr555_to_rgba(c);
            assert_eq!(rgb_to_bgr555(r, g, b), c);
        }
    }
}
