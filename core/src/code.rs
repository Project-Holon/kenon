//! The game's code images, for the UI data that lives in code rather than
//! in archives (inventory section 14: the item icon table, the menu cursor
//! tile, the field menu and touch bar tables, and others).
//!
//! An eidos extraction writes them beside `data/`: `arm9_dec.bin` (the
//! decompressed ARM9, loaded at 0x02004000, eidos `BW_ARM9_LOAD_ADDRESS`),
//! `y9.bin` (the overlay table), `overlays/y9_NNNN.bin` (each overlay as
//! stored) and `overlays_dec/y9_NNNN.bin` (the compressed ones,
//! decompressed). kenon reads values from the user's own images at run
//! time; it copies no bytes into the repository.
//!
//! y9 entry, 32 bytes: u32 id, u32 load address, u32 size, u32 BSS size,
//! u32 static initialiser start and end, u32 file id, u32 flags (bit 24:
//! compressed).

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::rom::Rom;

/// Where the ARM9 image is loaded.
pub const ARM9_ADDRESS: u32 = 0x0200_4000;

/// Which image an address belongs to: overlays share address ranges, so an
/// address alone does not say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Image {
    Arm9,
    Overlay(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayEntry {
    pub id: u32,
    pub address: u32,
    pub size: u32,
    pub bss_size: u32,
    pub compressed: bool,
}

/// Parses `y9.bin`.
pub fn parse_overlay_table(bytes: &[u8]) -> Vec<OverlayEntry> {
    bytes
        .as_chunks::<32>()
        .0
        .iter()
        .map(|e| {
            let word = |i: usize| u32::from_le_bytes(e[i * 4..i * 4 + 4].try_into().unwrap());
            OverlayEntry {
                id: word(0),
                address: word(1),
                size: word(2),
                bss_size: word(3),
                compressed: word(7) & 0x0100_0000 != 0,
            }
        })
        .collect()
}

/// Where the images come from.
enum Backing {
    /// An extraction folder (the one that holds `data/`).
    Dir(PathBuf),
    /// The ROM itself.
    Rom(Arc<Rom>),
}

/// The code images of one game, loaded on first use.
pub struct CodeImages {
    backing: Backing,
    table: Vec<OverlayEntry>,
    loaded: HashMap<Image, OnceLock<Vec<u8>>>,
}

impl CodeImages {
    /// Opens the images in an extraction folder (the one that holds
    /// `data/`). Fails when `y9.bin` is missing; the ARM9 and the overlays
    /// are read when first asked for.
    pub fn open(extraction_dir: &Path) -> io::Result<Self> {
        let table = parse_overlay_table(&fs::read(extraction_dir.join("y9.bin"))?);
        Ok(Self::with(
            Backing::Dir(extraction_dir.to_path_buf()),
            table,
        ))
    }

    /// The images inside a ROM.
    pub fn from_rom(rom: Arc<Rom>) -> Self {
        let table = rom.overlays().to_vec();
        Self::with(Backing::Rom(rom), table)
    }

    fn with(backing: Backing, table: Vec<OverlayEntry>) -> Self {
        let loaded = std::iter::once(Image::Arm9)
            .chain((0..table.len()).map(|n| Image::Overlay(n as u16)))
            .map(|image| (image, OnceLock::new()))
            .collect();
        CodeImages {
            backing,
            table,
            loaded,
        }
    }

    pub fn overlays(&self) -> &[OverlayEntry] {
        &self.table
    }

    /// The load address of an image.
    pub fn address(&self, image: Image) -> Option<u32> {
        match image {
            Image::Arm9 => Some(ARM9_ADDRESS),
            Image::Overlay(n) => self.table.get(n as usize).map(|e| e.address),
        }
    }

    /// The bytes of an image as it sits in memory.
    pub fn bytes(&self, image: Image) -> io::Result<&[u8]> {
        let cell = self
            .loaded
            .get(&image)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such overlay"))?;
        if let Some(bytes) = cell.get() {
            return Ok(bytes);
        }
        let bytes = match (&self.backing, image) {
            (Backing::Dir(dir), Image::Arm9) => fs::read(dir.join("arm9_dec.bin"))?,
            (Backing::Dir(dir), Image::Overlay(n)) => {
                let name = format!("y9_{n:04}.bin");
                fs::read(dir.join("overlays_dec").join(&name))
                    .or_else(|_| fs::read(dir.join("overlays").join(&name)))?
            }
            (Backing::Rom(rom), Image::Arm9) => rom.arm9().map_err(io::Error::other)?,
            (Backing::Rom(rom), Image::Overlay(n)) => rom.overlay(n).map_err(io::Error::other)?,
        };
        Ok(cell.get_or_init(|| bytes))
    }

    /// `len` bytes at a memory address in an image. `None` when the range is
    /// outside the image (or in its BSS, which has no stored bytes).
    pub fn read(&self, image: Image, address: u32, len: usize) -> Option<&[u8]> {
        let base = self.address(image)?;
        let bytes = self.bytes(image).ok()?;
        let at = address.checked_sub(base)? as usize;
        bytes.get(at..at + len)
    }

    pub fn u16_at(&self, image: Image, address: u32) -> Option<u16> {
        self.read(image, address, 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32_at(&self, image: Image, address: u32) -> Option<u32> {
        self.read(image, address, 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }

    /// A NUL-terminated ASCII string at an address, up to `max` bytes.
    pub fn c_string(&self, image: Image, address: u32, max: usize) -> Option<String> {
        let base = self.address(image)?;
        let bytes = self.bytes(image).ok()?;
        let at = address.checked_sub(base)? as usize;
        let tail = bytes.get(at..)?;
        let end = tail.iter().take(max).position(|&b| b == 0)?;
        Some(String::from_utf8_lossy(&tail[..end]).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u32, address: u32, size: u32, flags: u32) -> Vec<u8> {
        [id, address, size, 0, 0, 0, id, flags]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect()
    }

    #[test]
    fn images_from_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut y9 = entry(0, 0x0215_42C0, 8, 0);
        y9.extend(entry(1, 0x021B_6100, 8, 0x0100_0000 | 0x20));
        fs::write(dir.path().join("y9.bin"), y9).unwrap();
        let mut arm9 = vec![0u8; 16];
        arm9[8..14].copy_from_slice(b"a/0/2\0");
        arm9[4..8].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        fs::write(dir.path().join("arm9_dec.bin"), arm9).unwrap();
        fs::create_dir(dir.path().join("overlays")).unwrap();
        fs::create_dir(dir.path().join("overlays_dec")).unwrap();
        fs::write(dir.path().join("overlays/y9_0000.bin"), [1u8; 8]).unwrap();
        fs::write(dir.path().join("overlays/y9_0001.bin"), [9u8; 4]).unwrap();
        fs::write(dir.path().join("overlays_dec/y9_0001.bin"), [2u8; 8]).unwrap();

        let code = CodeImages::open(dir.path()).unwrap();
        assert_eq!(code.overlays().len(), 2);
        assert!(code.overlays()[1].compressed);
        assert_eq!(code.u32_at(Image::Arm9, 0x0200_4004), Some(0x1234_5678));
        assert_eq!(
            code.c_string(Image::Arm9, 0x0200_4008, 16).as_deref(),
            Some("a/0/2")
        );
        assert_eq!(code.read(Image::Arm9, 0x0200_3FFF, 1), None);
        assert_eq!(
            code.read(Image::Overlay(0), 0x0215_42C0, 2),
            Some(&[1u8, 1][..])
        );
        // The decompressed copy wins.
        assert_eq!(code.u16_at(Image::Overlay(1), 0x021B_6106), Some(0x0202));
        assert_eq!(code.read(Image::Overlay(1), 0x021B_6108, 1), None);
        assert_eq!(code.address(Image::Overlay(7)), None);
    }
}
