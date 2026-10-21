//! The SDAT container: header, SYMB (names), INFO (records), FAT and FILE.
//!
//! ```text
//! 0x00  "SDAT"  u16 bom  u16 version  u32 file size  u16 header size  u16 blocks
//! 0x10  u32 SYMB offset  u32 SYMB size   (size 0: no names)
//! 0x18  u32 INFO offset  u32 INFO size
//! 0x20  u32 FAT offset   u32 FAT size
//! 0x28  u32 FILE offset  u32 FILE size
//! SYMB  "SYMB" u32 size, then 8 u32 offsets (one per record kind), each to
//!       { u32 count; u32 string offset * count }. Offsets count from the
//!       start of the SYMB block; strings end in NUL.
//! INFO  "INFO" u32 size, then 8 u32 offsets, each to
//!       { u32 count; u32 record offset * count }. Offsets count from the
//!       start of the INFO block; a zero offset is an empty slot.
//! FAT   "FAT " u32 size, u32 count, then count { u32 offset; u32 size;
//!       8 bytes reserved }. Offsets are from the start of the SDAT file.
//! ```
//!
//! The eight kinds are, in order: SEQ, SEQARC, BANK, WAVEARC, PLAYER, GROUP,
//! PLAYER2, STRM. A SEQ record is `{ u32 file id; u16 bank; u8 volume;
//! u8 channel pressure; u8 player priority; u8 player }`. Its bank field is
//! the BANK slot itself, with no player indirection. A BANK record is
//! `{ u32 file id; u16 wave archive * 4 }` (0xFFFF: unused slot). A WAVEARC
//! record is a bare `u32` file id.
//!
//! References: eidos `engine/audio/sdat.c`, aletheia `tools/assets/sdat.py`.

use std::path::Path;

use crate::nitro::{read_u16, read_u32, Error};

/// The record kinds an SDAT holds, in the order of its tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Seq = 0,
    SeqArc = 1,
    Bank = 2,
    WaveArc = 3,
    Player = 4,
    Group = 5,
    Player2 = 6,
    Strm = 7,
}

const KINDS: usize = 8;

/// The INFO record of a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeqRecord {
    pub file_id: u32,
    /// The BANK slot the sequence plays with.
    pub bank: u16,
    pub volume: u8,
    pub channel_pressure: u8,
    pub player_priority: u8,
    pub player: u8,
}

/// The INFO record of an instrument bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankRecord {
    pub file_id: u32,
    /// Global WAVEARC slots, 0xFFFF for an unused one.
    pub wave_arcs: [u16; 4],
}

/// The INFO record of a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerRecord {
    pub max_sequences: u8,
    pub channel_mask: u16,
    pub heap_size: u32,
}

/// A parsed SDAT that owns its bytes.
#[derive(Debug, Clone)]
pub struct Sdat {
    bytes: Vec<u8>,
    info: usize,
    info_size: usize,
    symb: Option<(usize, usize)>,
    info_tables: [usize; KINDS],
    symb_tables: [usize; KINDS],
    counts: [u32; KINDS],
    fat: usize,
    fat_count: u32,
}

/// Checks a block's magic and size and returns where it starts and how big
/// it is.
fn section(bytes: &[u8], at: usize, magic: &[u8; 4]) -> Result<(usize, usize), Error> {
    let found = bytes.get(at..at + 4).ok_or(Error::Truncated)?;
    if found != magic {
        return Err(Error::WrongMagic {
            want: *magic,
            found: found.try_into().ok(),
        });
    }
    let size = read_u32(bytes, at + 4)? as usize;
    if size < 8 || at + size > bytes.len() {
        return Err(Error::Truncated);
    }
    Ok((at, size))
}

impl Sdat {
    /// Parses an SDAT image.
    pub fn parse(bytes: Vec<u8>) -> Result<Sdat, Error> {
        if bytes.len() < 0x40 || &bytes[..4] != b"SDAT" {
            return Err(Error::WrongMagic {
                want: *b"SDAT",
                found: bytes.get(..4).and_then(|m| m.try_into().ok()),
            });
        }
        let symb_off = read_u32(&bytes, 0x10)? as usize;
        let symb_size = read_u32(&bytes, 0x14)? as usize;
        let info_off = read_u32(&bytes, 0x18)? as usize;
        let fat_off = read_u32(&bytes, 0x20)? as usize;
        let file_off = read_u32(&bytes, 0x28)? as usize;

        let (info, info_size) = section(&bytes, info_off, b"INFO")?;
        let symb = if symb_size != 0 {
            Some(section(&bytes, symb_off, b"SYMB")?)
        } else {
            None
        };
        let (fat, fat_size) = section(&bytes, fat_off, b"FAT ")?;
        let fat_count = read_u32(&bytes, fat + 8)?;
        if 0xC + fat_count as usize * 16 > fat_size {
            return Err(Error::Bad("FAT count"));
        }
        section(&bytes, file_off, b"FILE")?;

        let mut info_tables = [0usize; KINDS];
        let mut symb_tables = [0usize; KINDS];
        let mut counts = [0u32; KINDS];
        for i in 0..KINDS {
            let off = read_u32(&bytes, info + 8 + i * 4)? as usize;
            if off != 0 && off + 4 <= info_size {
                let count = read_u32(&bytes, info + off)?;
                if off + 4 + count as usize * 4 <= info_size {
                    info_tables[i] = off;
                    counts[i] = count;
                }
            }
            if let Some((symb, symb_size)) = symb {
                let off = read_u32(&bytes, symb + 8 + i * 4)? as usize;
                if off != 0 && off + 4 <= symb_size {
                    let count = read_u32(&bytes, symb + off)?;
                    // A table whose count disagrees with INFO (nested
                    // SEQARC names) is not a plain name list: skip it.
                    if off + 4 + count as usize * 4 <= symb_size && count == counts[i] {
                        symb_tables[i] = off;
                    }
                }
            }
        }
        Ok(Sdat {
            bytes,
            info,
            info_size,
            symb,
            info_tables,
            symb_tables,
            counts,
            fat,
            fat_count,
        })
    }

    /// Reads and parses the SDAT file at `path`.
    pub fn open(path: &Path) -> Result<Sdat, Error> {
        let bytes = std::fs::read(path).map_err(|_| Error::Bad("cannot read the SDAT file"))?;
        Sdat::parse(bytes)
    }

    /// How many records of `kind` the INFO block lists.
    pub fn count(&self, kind: Kind) -> u32 {
        self.counts[kind as usize]
    }

    /// The SYMB name of a slot, if the file has names and the slot has one.
    pub fn name(&self, kind: Kind, index: u32) -> Option<&str> {
        let (symb, symb_size) = self.symb?;
        let table = self.symb_tables[kind as usize];
        if table == 0 || index >= self.counts[kind as usize] {
            return None;
        }
        let str_off = read_u32(&self.bytes, symb + table + 4 + index as usize * 4).ok()? as usize;
        if str_off == 0 || str_off >= symb_size {
            return None;
        }
        let text = &self.bytes[symb + str_off..symb + symb_size];
        let end = text.iter().position(|&b| b == 0)?;
        std::str::from_utf8(&text[..end]).ok()
    }

    /// The slot whose SYMB name is `name`.
    pub fn find(&self, kind: Kind, name: &str) -> Option<u32> {
        (0..self.count(kind)).find(|&i| self.name(kind, i) == Some(name))
    }

    /// The INFO record bytes of a slot (at least `need` of them), or None for
    /// an empty slot.
    fn record(&self, kind: Kind, index: u32, need: usize) -> Option<&[u8]> {
        let table = self.info_tables[kind as usize];
        if table == 0 || index >= self.counts[kind as usize] {
            return None;
        }
        let ptr = read_u32(&self.bytes, self.info + table + 4 + index as usize * 4).ok()? as usize;
        if ptr == 0 || ptr + need > self.info_size {
            return None;
        }
        self.bytes.get(self.info + ptr..self.info + ptr + need)
    }

    /// The INFO record of sequence `index`.
    pub fn seq_record(&self, index: u32) -> Option<SeqRecord> {
        let r = self.record(Kind::Seq, index, 10)?;
        Some(SeqRecord {
            file_id: read_u32(r, 0).ok()?,
            bank: read_u16(r, 4).ok()?,
            volume: r[6],
            channel_pressure: r[7],
            player_priority: r[8],
            player: r[9],
        })
    }

    /// The INFO record of bank `index`.
    pub fn bank_record(&self, index: u32) -> Option<BankRecord> {
        let r = self.record(Kind::Bank, index, 12)?;
        let mut wave_arcs = [0u16; 4];
        for (i, slot) in wave_arcs.iter_mut().enumerate() {
            *slot = read_u16(r, 4 + i * 2).ok()?;
        }
        Some(BankRecord {
            file_id: read_u32(r, 0).ok()?,
            wave_arcs,
        })
    }

    /// The INFO record of player `index`.
    pub fn player_record(&self, index: u32) -> Option<PlayerRecord> {
        let r = self.record(Kind::Player, index, 8)?;
        Some(PlayerRecord {
            max_sequences: r[0],
            channel_mask: read_u16(r, 2).ok()?,
            heap_size: read_u32(r, 4).ok()?,
        })
    }

    /// The file id of wave archive `index`.
    pub fn wave_archive_file_id(&self, index: u32) -> Option<u32> {
        read_u32(self.record(Kind::WaveArc, index, 4)?, 0).ok()
    }

    /// The bytes of FAT file `file_id`.
    pub fn file(&self, file_id: u32) -> Option<&[u8]> {
        if file_id >= self.fat_count {
            return None;
        }
        let at = self.fat + 0xC + file_id as usize * 16;
        let off = read_u32(&self.bytes, at).ok()? as usize;
        let size = read_u32(&self.bytes, at + 4).ok()? as usize;
        self.bytes.get(off..off.checked_add(size)?)
    }

    /// How many files the FAT lists.
    pub fn file_count(&self) -> u32 {
        self.fat_count
    }

    /// The SSEQ file of sequence `index`.
    pub fn seq_file(&self, index: u32) -> Option<&[u8]> {
        self.file(self.seq_record(index)?.file_id)
    }

    /// The SBNK file of bank `index`.
    pub fn bank_file(&self, index: u32) -> Option<&[u8]> {
        self.file(self.bank_record(index)?.file_id)
    }

    /// The SWAR file of wave archive `index`.
    pub fn wave_archive_file(&self, index: u32) -> Option<&[u8]> {
        self.file(self.wave_archive_file_id(index)?)
    }
}

/// Builds a small SDAT for tests in this crate: named records of the three
/// kinds the player needs, and the FAT members they point at.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    pub struct Entries<'a> {
        pub seqs: &'a [(&'a str, SeqRecord)],
        pub banks: &'a [(&'a str, BankRecord)],
        pub wave_archives: &'a [(&'a str, u32)],
        pub files: &'a [Vec<u8>],
    }

    fn put32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    /// One block with a table per kind. `records[k]` is a list of
    /// (name, INFO record bytes).
    fn tables(records: &[Vec<(String, Vec<u8>)>; KINDS]) -> (Vec<u8>, Vec<u8>) {
        let mut info = vec![0u8; 8 + KINDS * 4];
        info[..4].copy_from_slice(b"INFO");
        let mut symb = vec![0u8; 8 + KINDS * 4];
        symb[..4].copy_from_slice(b"SYMB");
        let mut name_slots: Vec<(usize, &str)> = Vec::new();
        for (k, recs) in records.iter().enumerate() {
            if recs.is_empty() {
                continue;
            }
            let at = info.len() as u32;
            info[8 + k * 4..12 + k * 4].copy_from_slice(&at.to_le_bytes());
            put32(&mut info, recs.len() as u32);
            let ptrs = info.len();
            info.resize(ptrs + recs.len() * 4, 0);
            for (i, (_, rec)) in recs.iter().enumerate() {
                let at = info.len() as u32;
                info[ptrs + i * 4..ptrs + i * 4 + 4].copy_from_slice(&at.to_le_bytes());
                info.extend_from_slice(rec);
            }
            let at = symb.len() as u32;
            symb[8 + k * 4..12 + k * 4].copy_from_slice(&at.to_le_bytes());
            put32(&mut symb, recs.len() as u32);
            let sptrs = symb.len();
            symb.resize(sptrs + recs.len() * 4, 0);
            for (i, (name, _)) in recs.iter().enumerate() {
                name_slots.push((sptrs + i * 4, name));
            }
        }
        for (slot, text) in name_slots {
            let at = symb.len() as u32;
            symb[slot..slot + 4].copy_from_slice(&at.to_le_bytes());
            symb.extend_from_slice(text.as_bytes());
            symb.push(0);
        }
        for block in [&mut symb, &mut info] {
            while block.len() % 4 != 0 {
                block.push(0);
            }
            let len = block.len() as u32;
            block[4..8].copy_from_slice(&len.to_le_bytes());
        }
        (symb, info)
    }

    pub fn build(entries: &Entries) -> Vec<u8> {
        let mut records: [Vec<(String, Vec<u8>)>; KINDS] = Default::default();
        for (name, r) in entries.seqs {
            let mut v = Vec::new();
            put32(&mut v, r.file_id);
            v.extend_from_slice(&r.bank.to_le_bytes());
            v.extend_from_slice(&[r.volume, r.channel_pressure, r.player_priority, r.player]);
            v.extend_from_slice(&[0, 0]);
            records[Kind::Seq as usize].push((name.to_string(), v));
        }
        for (name, r) in entries.banks {
            let mut v = Vec::new();
            put32(&mut v, r.file_id);
            for w in r.wave_arcs {
                v.extend_from_slice(&w.to_le_bytes());
            }
            records[Kind::Bank as usize].push((name.to_string(), v));
        }
        for (name, id) in entries.wave_archives {
            let mut v = Vec::new();
            put32(&mut v, *id);
            put32(&mut v, 0);
            records[Kind::WaveArc as usize].push((name.to_string(), v));
        }
        let (symb, info) = tables(&records);

        let fat_size = 12 + entries.files.len() * 16;
        let symb_off = 0x40;
        let info_off = symb_off + symb.len();
        let fat_off = info_off + info.len();
        let file_off = fat_off + fat_size;
        let mut fat = Vec::new();
        fat.extend_from_slice(b"FAT ");
        put32(&mut fat, fat_size as u32);
        put32(&mut fat, entries.files.len() as u32);
        let mut body = Vec::new();
        for f in entries.files {
            put32(&mut fat, (file_off + 8 + body.len()) as u32);
            put32(&mut fat, f.len() as u32);
            put32(&mut fat, 0);
            put32(&mut fat, 0);
            body.extend_from_slice(f);
        }
        let file_size = 8 + body.len();

        let mut out = vec![0u8; 0x40];
        out[..4].copy_from_slice(b"SDAT");
        for (at, value) in [
            (0x10, symb_off),
            (0x14, symb.len()),
            (0x18, info_off),
            (0x1C, info.len()),
            (0x20, fat_off),
            (0x24, fat.len()),
            (0x28, file_off),
            (0x2C, file_size),
        ] {
            out[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
        out.extend_from_slice(&symb);
        out.extend_from_slice(&info);
        out.extend_from_slice(&fat);
        out.extend_from_slice(b"FILE");
        put32(&mut out, file_size as u32);
        out.extend_from_slice(&body);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{build, Entries};
    use super::*;

    fn sample() -> Sdat {
        let seq = SeqRecord {
            file_id: 1,
            bank: 0,
            volume: 90,
            channel_pressure: 0,
            player_priority: 64,
            player: 2,
        };
        let bank = BankRecord {
            file_id: 0,
            wave_arcs: [0, 0xFFFF, 0xFFFF, 0xFFFF],
        };
        let bytes = build(&Entries {
            seqs: &[("SEQ_A", seq), ("SEQ_B", seq)],
            banks: &[("BANK_A", bank)],
            wave_archives: &[("WAVE_A", 2)],
            files: &[b"bank".to_vec(), b"seq".to_vec(), b"wave".to_vec()],
        });
        Sdat::parse(bytes).unwrap()
    }

    #[test]
    fn finds_names_and_records() {
        let s = sample();
        assert_eq!(s.count(Kind::Seq), 2);
        assert_eq!(s.name(Kind::Seq, 1), Some("SEQ_B"));
        assert_eq!(s.find(Kind::Seq, "SEQ_B"), Some(1));
        assert_eq!(s.find(Kind::Seq, "nope"), None);
        assert_eq!(s.name(Kind::Bank, 0), Some("BANK_A"));
        let r = s.seq_record(0).unwrap();
        assert_eq!(
            (r.file_id, r.volume, r.player_priority, r.player),
            (1, 90, 64, 2)
        );
        assert_eq!(s.bank_record(0).unwrap().wave_arcs[1], 0xFFFF);
        assert_eq!(s.seq_file(0), Some(&b"seq"[..]));
        assert_eq!(s.bank_file(0), Some(&b"bank"[..]));
        assert_eq!(s.wave_archive_file(0), Some(&b"wave"[..]));
        assert_eq!(s.file_count(), 3);
        assert!(s.seq_record(2).is_none());
        assert!(s.file(3).is_none());
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Sdat::parse(vec![0; 0x100]).is_err());
        assert!(Sdat::parse(b"SDAT".to_vec()).is_err());
        let mut bytes = build(&Entries {
            seqs: &[],
            banks: &[],
            wave_archives: &[],
            files: &[],
        });
        bytes.truncate(0x50);
        assert!(Sdat::parse(bytes).is_err());
    }
}
