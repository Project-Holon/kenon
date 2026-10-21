//! A Nintendo DS cartridge image (`.nds`), read in place: the user picks
//! their own ROM and kenon reads the files it needs from it, with nothing
//! unpacked to disk.
//!
//! The layout follows eidos `install/ndsrom.c` (a transcription of
//! aletheia's `tools/ndsrom.py`):
//!
//! ```text
//! header  0x00 title (12)  0x0C game code (4)  0x10 maker (2)
//!         0x14 capacity (128 KiB << n)
//!         0x20 ARM9 offset, 0x24 entry, 0x28 load address, 0x2C size
//!         0x40 FNT offset, 0x44 size; 0x48 FAT offset, 0x4C size
//!         0x50 ARM9 overlay table offset, 0x54 size
//!         0x15E header CRC-16 (over 0x00..0x15E)
//! FAT     8 bytes per file: start, end
//! FNT     8 bytes per directory: u32 entry list offset (from the FNT),
//!         u16 first file id, u16 parent (the root: the directory count);
//!         an entry list holds a length byte (bit 7: directory, 0: end),
//!         the name, and for a directory its u16 id (0xF000 + index)
//! overlay table: 32-byte y9 entries (see `code::parse_overlay_table`)
//! ```
//!
//! The ARM9 binary and the overlays flagged as compressed are BLZ
//! compressed (`blz`).

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

use crate::blz;
use crate::code::{parse_overlay_table, OverlayEntry};

/// Where the image's bytes come from: a file on the desktop, memory in the
/// browser.
pub trait Storage: Send + Sync {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>>;
}

impl Storage for Vec<u8> {
    fn len(&self) -> u64 {
        self.as_slice().len() as u64
    }

    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let start = usize::try_from(offset).map_err(|_| short())?;
        self.get(start..start.checked_add(len).ok_or_else(short)?)
            .map(<[u8]>::to_vec)
            .ok_or_else(short)
    }
}

/// A ROM file read on demand.
pub struct FileStorage {
    file: Mutex<File>,
    len: u64,
}

impl FileStorage {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(FileStorage {
            file: Mutex::new(file),
            len,
        })
    }
}

impl Storage for FileStorage {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0; len];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }
}

fn short() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "past the end of the image")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Smaller than a header.
    TooSmall,
    /// The header CRC does not match: not a DS cartridge image.
    NotDs,
    /// A table runs past the image or is malformed.
    Bad(&'static str),
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TooSmall => write!(f, "the file is too small to be a DS game"),
            Error::NotDs => write!(f, "the file is not a Nintendo DS cartridge image"),
            Error::Bad(what) => write!(f, "the image is damaged: {what}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

/// Which game an image is, from its game code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Game {
    Black,
    White,
    Other,
}

/// The dumps kenon knows to be good (No-Intro): game code, size, SHA-1.
pub const KNOWN_DUMPS: &[(&str, u64, &str)] = &[
    (
        "IRBO",
        0x1000_0000,
        "26ad0b9967aa279c4a266ee69f52b9b2332399a5",
    ),
    (
        "IRAO",
        0x1000_0000,
        "bc696a0dfb448c7b3a8a206f0f8214411a039208",
    ),
];

/// What the header says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub title: String,
    pub game_code: String,
    pub game: Game,
    pub size: u64,
}

pub struct Rom {
    storage: Box<dyn Storage>,
    pub identity: Identity,
    arm9: (u64, usize),
    files: Vec<(u64, u64)>,
    paths: HashMap<String, u16>,
    overlays: Vec<OverlayEntry>,
    overlay_files: Vec<u32>,
}

impl Rom {
    /// Opens a ROM file; its bytes are read when needed.
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::new(Box::new(FileStorage::open(path)?))
    }

    /// Reads a ROM held in memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Self::new(Box::new(bytes))
    }

    pub fn new(storage: Box<dyn Storage>) -> Result<Self, Error> {
        if storage.len() < 0x200 {
            return Err(Error::TooSmall);
        }
        let header = storage.read_at(0, 0x200)?;
        let h32 = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        if crc16(&header[..0x15E]) != u16::from_le_bytes([header[0x15E], header[0x15F]]) {
            return Err(Error::NotDs);
        }
        let text = |r: std::ops::Range<usize>| {
            String::from_utf8_lossy(&header[r])
                .trim_end_matches('\0')
                .to_string()
        };
        let game_code = text(0x0C..0x10);
        let game = match game_code.get(..3) {
            Some("IRB") => Game::Black,
            Some("IRA") => Game::White,
            _ => Game::Other,
        };
        let identity = Identity {
            title: text(0x00..0x0C),
            game_code,
            game,
            size: storage.len(),
        };

        let table = |offset: u32, size: u32, what: &'static str| -> Result<Vec<u8>, Error> {
            if offset as u64 + size as u64 > storage.len() {
                return Err(Error::Bad(what));
            }
            Ok(storage.read_at(offset as u64, size as usize)?)
        };
        let fat = table(h32(0x48), h32(0x4C), "file allocation table")?;
        let files: Vec<(u64, u64)> = fat
            .as_chunks::<8>()
            .0
            .iter()
            .map(|e| {
                let start = u32::from_le_bytes(e[..4].try_into().unwrap()) as u64;
                let end = u32::from_le_bytes(e[4..].try_into().unwrap()) as u64;
                (start, end)
            })
            .collect();
        if files.iter().any(|&(s, e)| s > e || e > storage.len()) {
            return Err(Error::Bad("a file lies outside the image"));
        }
        let fnt = table(h32(0x40), h32(0x44), "file name table")?;
        let paths = parse_fnt(&fnt)?;
        let y9 = table(h32(0x50), h32(0x54), "overlay table")?;
        let overlays = parse_overlay_table(&y9);
        let overlay_files = y9
            .as_chunks::<32>()
            .0
            .iter()
            .map(|e| u32::from_le_bytes(e[24..28].try_into().unwrap()))
            .collect();
        let arm9 = (h32(0x20) as u64, h32(0x2C) as usize);
        if arm9.0 + arm9.1 as u64 > storage.len() {
            return Err(Error::Bad("the ARM9 binary lies outside the image"));
        }
        Ok(Rom {
            storage,
            identity,
            arm9,
            files,
            paths,
            overlays,
            overlay_files,
        })
    }

    /// Whether the image is a dump kenon knows. Hashes the whole image, so
    /// it takes a moment.
    pub fn is_known_dump(&self) -> Result<bool, Error> {
        let size = self.storage.len();
        if !KNOWN_DUMPS
            .iter()
            .any(|&(code, s, _)| code == self.identity.game_code && s == size)
        {
            return Ok(false);
        }
        let mut sha = sha1_smol::Sha1::new();
        let mut at = 0u64;
        while at < size {
            let n = (size - at).min(1 << 22) as usize;
            sha.update(&self.storage.read_at(at, n)?);
            at += n as u64;
        }
        let hex = sha.digest().to_string();
        Ok(KNOWN_DUMPS
            .iter()
            .any(|&(code, _, h)| code == self.identity.game_code && h == hex))
    }

    /// The bytes of a NitroFS file by path, such as `a/0/2/3`.
    pub fn file(&self, path: &str) -> Result<Vec<u8>, Error> {
        let id = *self
            .paths
            .get(path.trim_start_matches('/'))
            .ok_or_else(|| Error::Io(format!("{path}: no such file in the ROM")))?;
        self.file_by_id(id as u32)
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.paths.contains_key(path.trim_start_matches('/'))
    }

    fn file_by_id(&self, id: u32) -> Result<Vec<u8>, Error> {
        let &(start, end) = self
            .files
            .get(id as usize)
            .ok_or(Error::Bad("file id past the table"))?;
        Ok(self.storage.read_at(start, (end - start) as usize)?)
    }

    /// The decompressed ARM9 binary.
    pub fn arm9(&self) -> Result<Vec<u8>, Error> {
        let raw = self.storage.read_at(self.arm9.0, self.arm9.1)?;
        blz::decompress(&raw).map_err(|e| Error::Io(format!("ARM9: {e}")))
    }

    pub fn overlays(&self) -> &[OverlayEntry] {
        &self.overlays
    }

    /// An overlay as it sits in memory (decompressed when compressed).
    pub fn overlay(&self, n: u16) -> Result<Vec<u8>, Error> {
        let entry = self
            .overlays
            .get(n as usize)
            .ok_or(Error::Bad("no such overlay"))?;
        let raw = self.file_by_id(self.overlay_files[n as usize])?;
        if entry.compressed {
            blz::decompress(&raw).map_err(|e| Error::Io(format!("overlay {n}: {e}")))
        } else {
            Ok(raw)
        }
    }

    /// Every file path, for listings.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.paths.keys().map(String::as_str)
    }
}

/// The header checksum: CRC-16 (MODBUS form, initial 0xFFFF).
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// Walks the file name table into path -> file id.
fn parse_fnt(fnt: &[u8]) -> Result<HashMap<String, u16>, Error> {
    let bad = || Error::Bad("file name table");
    let rd16 = |at: usize| -> Result<u16, Error> {
        fnt.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(bad)
    };
    let dir_count = rd16(6)? as usize;
    let mut paths = HashMap::new();
    let mut stack = vec![(0usize, String::new())];
    let mut seen = vec![false; dir_count];
    while let Some((dir, prefix)) = stack.pop() {
        if dir >= dir_count || std::mem::replace(&mut seen[dir], true) {
            return Err(bad());
        }
        let entry = dir * 8;
        let mut at = fnt
            .get(entry..entry + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .ok_or_else(bad)?;
        let mut file_id = rd16(entry + 4)?;
        loop {
            let t = *fnt.get(at).ok_or_else(bad)?;
            if t == 0 {
                break;
            }
            let len = (t & 0x7F) as usize;
            let name = fnt.get(at + 1..at + 1 + len).ok_or_else(bad)?;
            let name = String::from_utf8_lossy(name);
            let path = format!("{prefix}{name}");
            at += 1 + len;
            if t & 0x80 != 0 {
                let id = rd16(at)?;
                at += 2;
                stack.push(((id & 0x0FFF) as usize, format!("{path}/")));
            } else {
                paths.insert(path, file_id);
                file_id += 1;
            }
        }
    }
    Ok(paths)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny image: files "a/0" (id 0) and "x.bin" (id 1), and one
    /// uncompressed overlay (file id 2) loaded at 0x02100000.
    pub(crate) fn tiny_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x1000];
        rom[..12].copy_from_slice(b"POKEMON B\0\0\0");
        rom[0x0C..0x10].copy_from_slice(b"IRBO");
        // FNT at 0x200: two directories (root, "a").
        let mut fnt = Vec::new();
        fnt.extend_from_slice(&16u32.to_le_bytes());
        fnt.extend_from_slice(&1u16.to_le_bytes()); // root's first file: x.bin
        fnt.extend_from_slice(&2u16.to_le_bytes()); // directory count
        fnt.extend_from_slice(&0u32.to_le_bytes()); // patched below
        fnt.extend_from_slice(&0u16.to_le_bytes()); // a's first file: a/0
        fnt.extend_from_slice(&0xF000u16.to_le_bytes());
        // Root entries: x.bin, then directory "a" (id 0xF001).
        fnt.push(5);
        fnt.extend_from_slice(b"x.bin");
        fnt.push(0x81);
        fnt.push(b'a');
        fnt.extend_from_slice(&0xF001u16.to_le_bytes());
        fnt.push(0);
        let a_list = fnt.len() as u32;
        fnt[8..12].copy_from_slice(&a_list.to_le_bytes());
        fnt.push(1);
        fnt.push(b'0');
        fnt.push(0);
        rom[0x200..0x200 + fnt.len()].copy_from_slice(&fnt);
        // FAT at 0x300: three files.
        let files = [(0x800u32, 0x804u32), (0x900, 0x902), (0xA00, 0xA08)];
        for (i, (s, e)) in files.iter().enumerate() {
            rom[0x300 + i * 8..0x304 + i * 8].copy_from_slice(&s.to_le_bytes());
            rom[0x304 + i * 8..0x308 + i * 8].copy_from_slice(&e.to_le_bytes());
        }
        rom[0x800..0x804].copy_from_slice(b"NARC");
        rom[0x900..0x902].copy_from_slice(b"hi");
        rom[0xA00..0xA08].copy_from_slice(b"overlay!");
        // Overlay table at 0x400: one entry, file id 2.
        let y9 = [0u32, 0x0210_0000, 8, 0, 0, 0, 2, 0];
        for (i, w) in y9.iter().enumerate() {
            rom[0x400 + i * 4..0x404 + i * 4].copy_from_slice(&w.to_le_bytes());
        }
        // ARM9 at 0xB00, 16 stored bytes (a zero growth word: stored).
        rom[0xB00..0xB04].copy_from_slice(b"ARM9");
        let put = |rom: &mut Vec<u8>, at: usize, v: u32| {
            rom[at..at + 4].copy_from_slice(&v.to_le_bytes())
        };
        put(&mut rom, 0x20, 0xB00);
        put(&mut rom, 0x28, 0x0200_4000);
        put(&mut rom, 0x2C, 16);
        put(&mut rom, 0x40, 0x200);
        put(&mut rom, 0x44, fnt.len() as u32);
        put(&mut rom, 0x48, 0x300);
        put(&mut rom, 0x4C, 24);
        put(&mut rom, 0x50, 0x400);
        put(&mut rom, 0x54, 32);
        let crc = crc16(&rom[..0x15E]);
        rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
        rom
    }

    #[test]
    fn reads_files_overlays_and_arm9() {
        let rom = Rom::from_bytes(tiny_rom()).unwrap();
        assert_eq!(rom.identity.game, Game::Black);
        assert_eq!(rom.identity.title, "POKEMON B");
        assert_eq!(rom.file("a/0").unwrap(), b"NARC");
        assert_eq!(rom.file("x.bin").unwrap(), b"hi");
        assert!(rom.file("a/1").is_err());
        assert_eq!(rom.overlays()[0].address, 0x0210_0000);
        assert_eq!(rom.overlay(0).unwrap(), b"overlay!");
        assert_eq!(&rom.arm9().unwrap()[..4], b"ARM9");
        // Not one of the known dumps (wrong size).
        assert!(!rom.is_known_dump().unwrap());
    }

    #[test]
    fn rejects_non_ds_files() {
        let mut bytes = tiny_rom();
        bytes[0] ^= 1;
        assert!(matches!(Rom::from_bytes(bytes), Err(Error::NotDs)));
        assert!(matches!(Rom::from_bytes(vec![0; 16]), Err(Error::TooSmall)));
    }

    #[test]
    fn crc() {
        // The standard CRC-16/MODBUS check value.
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }
}
