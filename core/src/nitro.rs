//! The common shape of every Nitro resource (NCLR, NCGR, NSCR, NCER, NANR,
//! NFTR and the rest).
//!
//! ```text
//! 0x00  magic (stored reversed: an NCLR file starts with "RLCN")
//! 0x04  u16 bom (0xFEFF)  u16 version  u32 file size
//! 0x0C  u16 header size  u16 block count
//! then blocks, each { magic (reversed); u32 size including these 8 bytes }
//! ```
//!
//! Blocks are walked by their own size, so optional trailing blocks (SOPC,
//! PCMP, LBAL and others) need not be known. Reference: eidos
//! `engine/g2d/resources.c`, `find_block`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file does not start with the expected magic.
    WrongMagic {
        want: [u8; 4],
        found: Option<[u8; 4]>,
    },
    /// The header or a block runs past the end of the file.
    Truncated,
    /// The block the format needs is not there.
    MissingBlock([u8; 4]),
    /// A field inside a block is out of range.
    Bad(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = |m: &[u8; 4]| String::from_utf8_lossy(m).into_owned();
        match self {
            Error::WrongMagic {
                want,
                found: Some(found),
            } => write!(f, "expected {}, found {}", name(want), name(found)),
            Error::WrongMagic { want, found: None } => {
                write!(f, "expected {}, file too short", name(want))
            }
            Error::Truncated => write!(f, "the file is cut short"),
            Error::MissingBlock(magic) => write!(f, "no {} block", name(magic)),
            Error::Bad(what) => write!(f, "bad field: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// One block: its magic as stored and its body (after the 8-byte head).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block<'a> {
    pub magic: [u8; 4],
    pub body: &'a [u8],
}

/// The magic a file starts with, if it has four bytes.
pub fn magic_of(bytes: &[u8]) -> Option<[u8; 4]> {
    bytes.get(..4).map(|m| m.try_into().unwrap())
}

/// Splits a resource into its blocks after checking the file magic.
pub fn blocks<'a>(bytes: &'a [u8], magic: &[u8; 4]) -> Result<Vec<Block<'a>>, Error> {
    let found = magic_of(bytes);
    if found.as_ref() != Some(magic) {
        return Err(Error::WrongMagic {
            want: *magic,
            found,
        });
    }
    let mut pos = read_u16(bytes, 0x0C)? as usize;
    let mut out = Vec::new();
    while pos + 8 <= bytes.len() {
        let magic: [u8; 4] = bytes[pos..pos + 4].try_into().unwrap();
        let size = read_u32(bytes, pos + 4)? as usize;
        if size < 8 {
            return Err(Error::Truncated);
        }
        // A block may declare more than the file holds: some compressed
        // members (the a/0/0/4 battle sprite sheets) store only the start
        // of their pixel data, and the rest reads as zero. Keep what is
        // there; it can only be the last block.
        let end = bytes.len().min(pos + size);
        out.push(Block {
            magic,
            body: &bytes[pos + 8..end],
        });
        pos = end;
    }
    Ok(out)
}

/// The body of the first block with `want` as its magic.
pub fn block<'a>(bytes: &'a [u8], magic: &[u8; 4], want: &[u8; 4]) -> Result<&'a [u8], Error> {
    blocks(bytes, magic)?
        .into_iter()
        .find(|b| &b.magic == want)
        .map(|b| b.body)
        .ok_or(Error::MissingBlock(*want))
}

/// Where the body of the first `want` block starts in the file.
pub fn block_offset(bytes: &[u8], magic: &[u8; 4], want: &[u8; 4]) -> Result<usize, Error> {
    let body = block(bytes, magic, want)?;
    Ok(body.as_ptr() as usize - bytes.as_ptr() as usize)
}

pub(crate) fn read_u16(bytes: &[u8], at: usize) -> Result<u16, Error> {
    let b = bytes.get(at..at + 2).ok_or(Error::Truncated)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

pub(crate) fn read_u32(bytes: &[u8], at: usize) -> Result<u32, Error> {
    let b = bytes.get(at..at + 4).ok_or(Error::Truncated)?;
    Ok(u32::from_le_bytes(b.try_into().unwrap()))
}

/// Builds a resource from blocks, for tests and writers.
pub fn build(magic: &[u8; 4], blocks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let total = 0x10 + blocks.iter().map(|(_, b)| 8 + b.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(magic);
    out.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x01]);
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&0x10u16.to_le_bytes());
    out.extend_from_slice(&(blocks.len() as u16).to_le_bytes());
    for (magic, body) in blocks {
        out.extend_from_slice(*magic);
        out.extend_from_slice(&((8 + body.len()) as u32).to_le_bytes());
        out.extend_from_slice(body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_blocks() {
        let file = build(b"RLCN", &[(b"TTLP", &[1, 2, 3, 4]), (b"PMCP", &[9])]);
        let all = blocks(&file, b"RLCN").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].magic, *b"TTLP");
        assert_eq!(all[1].body, &[9]);
        assert_eq!(block(&file, b"RLCN", b"PMCP").unwrap(), &[9]);
        assert_eq!(
            block(&file, b"RLCN", b"RAHC"),
            Err(Error::MissingBlock(*b"RAHC"))
        );
        assert!(matches!(
            blocks(&file, b"RGCN"),
            Err(Error::WrongMagic { .. })
        ));
        // A short last block keeps what the file holds.
        let short = blocks(&file[..file.len() - 1], b"RLCN").unwrap();
        assert_eq!(short[1].body, &[] as &[u8]);
        assert!(blocks(&file[..0x12], b"RLCN").unwrap().is_empty());
    }
}
