//! NARC, the container every `a/X/Y/Z` NitroFS file in the game is.
//!
//! ```text
//! 0x00  "NARC"  u16 bom (0xFFFE)  u16 version  u32 file size
//! 0x0C  u16 header size (0x10)  u16 chunk count
//! 0x10  chunks, each { u32 magic; u32 size; ... }, chained by size:
//!         "BTAF"  u16 count  u16 reserved  then count { u32 start; u32 end; }
//!         "BTNF"  the file name table (unused by the game's archives)
//!         "GMIF"  the member image; member offsets are from GMIF + 8
//! ```
//!
//! References: eidos `sdk/narc.c`, aletheia `tools/assets/narc.py`. Framing
//! only: no member format is read here.

use std::fmt;
use std::ops::Range;
use std::path::Path;

use crate::lz;

const NARC: &[u8; 4] = b"NARC";
const BTAF: &[u8; 4] = b"BTAF";
const GMIF: &[u8; 4] = b"GMIF";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file does not start with "NARC".
    NotNarc,
    /// No BTAF or no GMIF chunk.
    MissingChunk(&'static str),
    /// A chunk or table runs past the end of the file.
    Truncated,
    /// A member's range is reversed or outside the image.
    BadMember { index: usize },
    /// The member index is past the end of the table.
    NoMember { index: usize, count: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotNarc => write!(f, "not a NARC file"),
            Error::MissingChunk(name) => write!(f, "no {name} chunk"),
            Error::Truncated => write!(f, "the file is cut short"),
            Error::BadMember { index } => write!(f, "member {index} lies outside the image"),
            Error::NoMember { index, count } => {
                write!(f, "member {index} does not exist ({count} members)")
            }
        }
    }
}

impl std::error::Error for Error {}

/// A parsed NARC that owns its bytes.
#[derive(Debug, Clone)]
pub struct Narc {
    bytes: Vec<u8>,
    members: Vec<Range<usize>>,
}

impl Narc {
    /// Reads and parses a NARC file.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::parse(bytes).map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
    }

    /// Parses NARC bytes.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.get(..4) != Some(NARC) {
            return Err(Error::NotNarc);
        }
        let header_size = u16_at(&bytes, 0x0C)? as usize;

        let mut fat = None;
        let mut image = None;
        let mut pos = header_size;
        while pos + 8 <= bytes.len() {
            let magic = &bytes[pos..pos + 4];
            let size = u32_at(&bytes, pos + 4)? as usize;
            if size < 8 || size > bytes.len() - pos {
                return Err(Error::Truncated);
            }
            if magic == BTAF {
                fat = Some(pos..pos + size);
            } else if magic == GMIF {
                image = Some(pos + 8..pos + size);
            }
            pos += size;
        }
        let fat = fat.ok_or(Error::MissingChunk("BTAF"))?;
        let image = image.ok_or(Error::MissingChunk("GMIF"))?;

        let count = u16_at(&bytes, fat.start + 8)? as usize;
        let table = fat.start + 0x0C;
        if table + count * 8 > fat.end {
            return Err(Error::Truncated);
        }
        let members = (0..count)
            .map(|index| {
                let start = u32_at(&bytes, table + index * 8)? as usize;
                let end = u32_at(&bytes, table + index * 8 + 4)? as usize;
                if start > end || end > image.len() {
                    return Err(Error::BadMember { index });
                }
                Ok(image.start + start..image.start + end)
            })
            .collect::<Result<_, _>>()?;
        Ok(Narc { bytes, members })
    }

    /// The number of members.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// A member's bytes as stored (possibly compressed).
    pub fn raw(&self, index: usize) -> Result<&[u8], Error> {
        let range = self.members.get(index).ok_or(Error::NoMember {
            index,
            count: self.members.len(),
        })?;
        Ok(&self.bytes[range.clone()])
    }

    /// A member's bytes, decompressed when the loader says it is compressed.
    /// The game's loaders take this flag from the caller; they do not guess.
    pub fn member(&self, index: usize, compressed: bool) -> Result<Vec<u8>, MemberError> {
        let raw = self.raw(index)?;
        if compressed {
            Ok(lz::decompress(raw)?)
        } else {
            Ok(raw.to_vec())
        }
    }

    /// Every member as stored.
    pub fn iter_raw(&self) -> impl Iterator<Item = &[u8]> {
        self.members.iter().map(|range| &self.bytes[range.clone()])
    }
}

/// A member could not be read or decompressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberError {
    Narc(Error),
    Lz(lz::Error),
}

impl From<Error> for MemberError {
    fn from(err: Error) -> Self {
        MemberError::Narc(err)
    }
}

impl From<lz::Error> for MemberError {
    fn from(err: lz::Error) -> Self {
        MemberError::Lz(err)
    }
}

impl fmt::Display for MemberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemberError::Narc(err) => err.fmt(f),
            MemberError::Lz(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for MemberError {}

/// The NitroFS path of an archive id. The id is the decimal spelling of the
/// path: `a/X/Y/Z` has id `100X + 10Y + Z` (inventory section 0; the game's
/// 235-entry path table at `arc_tool.c` follows this rule).
pub fn archive_path(id: u16) -> Option<String> {
    (id < 1000).then(|| format!("a/{}/{}/{}", id / 100, id / 10 % 10, id % 10))
}

/// Builds a NARC from members, laid out as the game's archives are: BTAF,
/// an empty BTNF, then GMIF with each member aligned to 4 bytes. Tests and
/// the mod packer use it.
pub fn build(members: &[&[u8]]) -> Vec<u8> {
    let mut image = Vec::new();
    let mut table = Vec::with_capacity(members.len() * 8);
    for member in members {
        let start = image.len() as u32;
        image.extend_from_slice(member);
        table.extend_from_slice(&start.to_le_bytes());
        table.extend_from_slice(&(image.len() as u32).to_le_bytes());
        while image.len() % 4 != 0 {
            image.push(0xFF);
        }
    }

    let btaf_size = 12 + table.len();
    // An empty name table: one root directory entry, no names.
    let btnf: [u8; 16] = [
        b'B', b'T', b'N', b'F', 16, 0, 0, 0, // magic, size
        4, 0, 0, 0, 0, 0, 1, 0, // first offset, first file, directory count
    ];
    let gmif_size = 8 + image.len();
    let total = 0x10 + btaf_size + btnf.len() + gmif_size;

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(NARC);
    out.extend_from_slice(&[0xFE, 0xFF, 0x00, 0x01]);
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&[0x10, 0x00, 0x03, 0x00]);
    out.extend_from_slice(BTAF);
    out.extend_from_slice(&(btaf_size as u32).to_le_bytes());
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&table);
    out.extend_from_slice(&btnf);
    out.extend_from_slice(GMIF);
    out.extend_from_slice(&(gmif_size as u32).to_le_bytes());
    out.extend_from_slice(&image);
    out
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, Error> {
    let b = bytes.get(at..at + 2).ok_or(Error::Truncated)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, Error> {
    let b = bytes.get(at..at + 4).ok_or(Error::Truncated)?;
    Ok(u32::from_le_bytes(b.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_paths() {
        assert_eq!(archive_path(5).as_deref(), Some("a/0/0/5"));
        assert_eq!(archive_path(23).as_deref(), Some("a/0/2/3"));
        assert_eq!(archive_path(83).as_deref(), Some("a/0/8/3"));
        assert_eq!(archive_path(234).as_deref(), Some("a/2/3/4"));
        assert_eq!(archive_path(1000), None);
    }

    #[test]
    fn build_and_parse() {
        let packed = lz::compress(lz::Kind::Lz10, &[7u8; 64]);
        let members: [&[u8]; 4] = [b"RLCN first", b"", b"odd", &packed];
        let narc = Narc::parse(build(&members)).unwrap();
        assert_eq!(narc.len(), 4);
        for (i, member) in members.iter().enumerate() {
            assert_eq!(narc.raw(i).unwrap(), *member);
        }
        assert_eq!(narc.member(3, true).unwrap(), vec![7u8; 64]);
        assert_eq!(narc.member(0, false).unwrap(), b"RLCN first");
        assert!(matches!(narc.member(0, true), Err(MemberError::Lz(_))));
        assert_eq!(narc.raw(4), Err(Error::NoMember { index: 4, count: 4 }));
        assert_eq!(narc.iter_raw().count(), 4);
    }

    #[test]
    fn header_fields() {
        let bytes = build(&[b"abcd"]);
        assert_eq!(u32_at(&bytes, 8).unwrap() as usize, bytes.len());
        assert_eq!(u16_at(&bytes, 0x0E).unwrap(), 3);
    }

    #[test]
    fn rejects_bad_files() {
        assert_eq!(Narc::parse(b"RLCN".to_vec()).unwrap_err(), Error::NotNarc);

        let good = build(&[b"abcd"]);
        let mut cut = good.clone();
        cut.truncate(good.len() - 2);
        assert_eq!(Narc::parse(cut).unwrap_err(), Error::Truncated);

        // Member end past the image.
        let mut bad = good.clone();
        bad[0x10 + 0x0C + 4] = 0x40;
        assert_eq!(Narc::parse(bad).unwrap_err(), Error::BadMember { index: 0 });

        // Rename GMIF so it is not found.
        let mut no_image = good;
        let at = no_image.windows(4).position(|w| w == GMIF).unwrap();
        no_image[at] = b'X';
        assert_eq!(
            Narc::parse(no_image).unwrap_err(),
            Error::MissingChunk("GMIF")
        );
    }
}
