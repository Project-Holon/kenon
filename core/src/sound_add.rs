//! New sounds: a WAV added to the sound archive as a new sequence, with a
//! bank and a wave archive of its own.
//!
//! The game plays a sound by its sequence number. It reads the number of
//! sequences, banks and wave archives from the INFO block it loads, and it
//! loads a sequence, its bank and the waves on demand into the heap of the
//! sequence's player (aletheia `nns_snd_arc.c`, `NNSi_SndArcLoadSeq`,
//! `NNSi_SndArcLoadBank`), so a new entry at the end of each table works with
//! no change to the game's code. An effect plays on a sound-effect player
//! whose heap holds the sequence, the bank and the whole wave archive; the
//! wave is capped to fit. Music plays on the music player, which has no heap
//! of its own and loads into the shared sound heap.
//!
//! The archive is rebuilt around the old one: each table that grows is
//! copied to the end of its block (SYMB names, INFO records), so no old
//! offset moves; the FAT grows, the FILE block moves by a whole number of
//! 32-byte steps and every old file keeps its alignment.
//!
//! The sequence is one track: a note at the wave's base key, held for the
//! length of the wave. A music sequence repeats it and its wave loops.

use std::fs;

use serde::Serialize;

use crate::audio_import::{encode_adpcm, read_wav, resample};
use crate::bridge::{Bridge, Error};
use crate::catalog::SOUND_ARCHIVE;
use crate::sdat::{Kind, Sdat};

/// The clock the channel timer divides (half the ARM7 bus clock).
const TIMER_CLOCK: f64 = 33_513_982.0 / 2.0;
/// The sound-effect player of most effects (heap 10200 bytes in the game).
const EFFECT_PLAYER: u8 = 2;
/// The music player (no heap of its own).
const MUSIC_PLAYER: u8 = 0;
const MAX_RATE: u32 = 32_000;
const MIN_RATE: u32 = 4_000;
/// The most wave data a music sequence takes: the shared sound heap is
/// about 600 KB and the game's own waves share it.
const MUSIC_DATA_MAX: usize = 160_000;
/// Heap kept free on an effect player beyond the three files.
const HEAP_MARGIN: usize = 0x100;
/// Ticks in a second at the tempo the sequence sets (120 per minute, 48
/// ticks to the quarter note).
const TICKS_PER_SECOND: f64 = 96.0;
/// The key the note plays and the bank's region is based on.
const NOTE_KEY: u8 = 60;

fn bad(msg: impl Into<String>) -> Error {
    Error::Format(msg.into())
}

fn fmt_err(e: crate::nitro::Error) -> Error {
    Error::Format(e.to_string())
}

/// A sound that was added.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSound {
    /// Its sequence number, the id maps and scripts use.
    pub id: u32,
    pub name: String,
    pub seconds: f64,
    pub rate: u32,
    pub bytes: usize,
    pub note: Option<String>,
}

// ---- the archive -------------------------------------------------------

fn rd32(b: &[u8], at: usize) -> Result<usize, Error> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()) as usize)
        .ok_or_else(|| bad("the sound archive is cut short"))
}

fn put32(b: &mut [u8], at: usize, v: usize) {
    b[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
}

fn align(n: usize, to: usize) -> usize {
    n.div_ceil(to) * to
}

/// One record to add to a table: its name and its INFO bytes.
struct Entry {
    kind: Kind,
    name: String,
    record: Vec<u8>,
}

/// Grows the table of `kind` in a SYMB or INFO block by `items`, copying the
/// table to the end of the block: `block` is the bytes of the whole block
/// (header included), `item` is what the table points at (a record or a
/// string). Returns nothing; the block gets longer.
fn grow_table(block: &mut Vec<u8>, kind: Kind, items: &[Vec<u8>]) -> Result<(), Error> {
    let slot = 8 + kind as usize * 4;
    let old = rd32(block, slot)?;
    let (count, ptrs) = if old == 0 {
        (0, Vec::new())
    } else {
        let count = rd32(block, old)?;
        let ptrs = block
            .get(old + 4..old + 4 + count * 4)
            .ok_or_else(|| bad("a table of the sound archive is cut short"))?
            .to_vec();
        (count, ptrs)
    };
    while !block.len().is_multiple_of(4) {
        block.push(0);
    }
    let table = block.len();
    block.extend_from_slice(&((count + items.len()) as u32).to_le_bytes());
    block.extend_from_slice(&ptrs);
    let mut at = table + 4 + (count + items.len()) * 4;
    let mut body = Vec::new();
    for item in items {
        block.extend_from_slice(&(at as u32).to_le_bytes());
        at += item.len();
        body.extend_from_slice(item);
    }
    block.extend_from_slice(&body);
    put32(block, slot, table);
    Ok(())
}

/// Adds records to the table of `kind` in the INFO block, in place: the
/// table's pointer list grows, and every offset after it moves with the
/// text. INFO records hold no offsets of their own, so the tables are all
/// there is to fix. The game reads this block into its sound heap, so it
/// is kept free of dead bytes.
fn grow_info(info: &mut Vec<u8>, kind: Kind, records: &[Vec<u8>]) -> Result<(), Error> {
    let slot = 8 + kind as usize * 4;
    let table = rd32(info, slot)?;
    if table == 0 {
        return Err(bad("the sound archive has no such table"));
    }
    let count = rd32(info, table)?;
    let n = records.len();
    let at = table + 4 + count * 4;
    if at > info.len() {
        return Err(bad("a table of the sound archive is cut short"));
    }
    info.splice(at..at, vec![0u8; n * 4]);
    // Table offsets after this one, and every record pointer after `at`
    // (old coordinates), move by the room made.
    for j in 0..8 {
        let off = rd32(info, 8 + j * 4)?;
        if off > table {
            put32(info, 8 + j * 4, off + n * 4);
        }
    }
    for j in 0..8 {
        let off = rd32(info, 8 + j * 4)?;
        if off == 0 {
            continue;
        }
        let c = rd32(info, off)?;
        for i in 0..c {
            let p = off + 4 + i * 4;
            // The new slots (this table, from `at`) are zero and stay so.
            let v = rd32(info, p)?;
            if v >= at {
                put32(info, p, v + n * 4);
            }
        }
    }
    while !info.len().is_multiple_of(4) {
        info.push(0);
    }
    put32(info, table, count + n);
    for (i, r) in records.iter().enumerate() {
        let here = info.len();
        put32(info, at + i * 4, here);
        info.extend_from_slice(r);
    }
    Ok(())
}

/// The SDAT with the files and records of `entries` added: file ids follow
/// the old ones in the order of `files`; the record bytes name them by
/// the ids the caller worked out.
fn append(old: &[u8], files: &[Vec<u8>], entries: &[Entry]) -> Result<Vec<u8>, Error> {
    const FILE_ALIGN: usize = 32;
    let (symb_at, symb_size) = (rd32(old, 0x10)?, rd32(old, 0x14)?);
    let (info_at, info_size) = (rd32(old, 0x18)?, rd32(old, 0x1C)?);
    let (fat_at, fat_size) = (rd32(old, 0x20)?, rd32(old, 0x24)?);
    let (file_at, file_size) = (rd32(old, 0x28)?, rd32(old, 0x2C)?);
    if info_at + info_size > fat_at
        || fat_at + fat_size > file_at
        || file_at + file_size > old.len()
    {
        return Err(bad("the sound archive blocks are not in the usual order"));
    }

    let mut info = old[info_at..info_at + info_size].to_vec();
    let mut symb = old[symb_at..symb_at + symb_size].to_vec();
    for kind in [Kind::Seq, Kind::Bank, Kind::WaveArc] {
        let of_kind: Vec<&Entry> = entries
            .iter()
            .filter(|e| e.kind as u8 == kind as u8)
            .collect();
        if of_kind.is_empty() {
            continue;
        }
        // The records go after the table; their pointers are block offsets.
        let records: Vec<Vec<u8>> = of_kind.iter().map(|e| e.record.clone()).collect();
        grow_info(&mut info, kind, &records)?;
        if symb_size != 0 {
            let names: Vec<Vec<u8>> = of_kind
                .iter()
                .map(|e| [e.name.as_bytes(), &[0]].concat())
                .collect();
            grow_table(&mut symb, kind, &names)?;
        }
    }
    while !info.len().is_multiple_of(4) {
        info.push(0);
    }
    while symb_size != 0 && !symb.len().is_multiple_of(4) {
        symb.push(0);
    }
    let n = info.len();
    put32(&mut info, 4, n);
    if symb_size != 0 {
        let n = symb.len();
        put32(&mut symb, 4, n);
    }

    // The FAT: the old entries, then one per new file.
    let old_count = rd32(old, fat_at + 8)?;
    let new_count = old_count + files.len();
    let new_fat_size = 12 + new_count * 16;
    let new_info_at = symb_at + symb.len();
    let new_fat_at = new_info_at + info.len();
    let new_file_at = new_fat_at + new_fat_size;

    // Where the old files start; they move by a whole number of steps.
    let first_old = (0..old_count)
        .map(|i| rd32(old, fat_at + 12 + i * 16))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|&o| o != 0)
        .min()
        .unwrap_or(file_at + 16);
    let new_first = align(new_file_at + 16, FILE_ALIGN) + (first_old % FILE_ALIGN);
    let shift = new_first as isize - first_old as isize;
    let old_end = file_at + file_size;
    let data = old
        .get(first_old..old_end)
        .ok_or_else(|| bad("the sound archive files lie outside the file block"))?;

    // File data is addressed from the start of the archive; the block's
    // 16-byte header comes first.
    let data_at = new_file_at + 16;
    let mut file_block = vec![0u8; new_first - data_at];
    file_block.extend_from_slice(data);
    let mut fat = Vec::with_capacity(new_fat_size);
    fat.extend_from_slice(b"FAT ");
    fat.extend_from_slice(&(new_fat_size as u32).to_le_bytes());
    fat.extend_from_slice(&(new_count as u32).to_le_bytes());
    for i in 0..old_count {
        let at = fat_at + 12 + i * 16;
        let entry = old
            .get(at..at + 16)
            .ok_or_else(|| bad("the FAT is cut short"))?;
        let off = rd32(entry, 0)?;
        let moved = if off == 0 {
            0
        } else {
            (off as isize + shift) as usize
        };
        fat.extend_from_slice(&(moved as u32).to_le_bytes());
        fat.extend_from_slice(&entry[4..]);
    }
    for f in files {
        while !(data_at + file_block.len()).is_multiple_of(FILE_ALIGN) {
            file_block.push(0);
        }
        fat.extend_from_slice(&((data_at + file_block.len()) as u32).to_le_bytes());
        fat.extend_from_slice(&(f.len() as u32).to_le_bytes());
        fat.extend_from_slice(&[0; 8]);
        file_block.extend_from_slice(f);
    }
    while !(data_at + file_block.len()).is_multiple_of(4) {
        file_block.push(0);
    }

    let mut out = old[..0x40].to_vec();
    out.extend_from_slice(&symb);
    out.extend_from_slice(&info);
    out.extend_from_slice(&fat);
    out.extend_from_slice(b"FILE");
    out.extend_from_slice(&((file_block.len() + 16) as u32).to_le_bytes());
    out.extend_from_slice(&(new_count as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&file_block);
    let size = out.len();
    put32(&mut out, 0x08, size);
    put32(&mut out, 0x10, 0x40);
    put32(&mut out, 0x14, if symb_size == 0 { 0 } else { symb.len() });
    put32(&mut out, 0x18, new_info_at);
    put32(&mut out, 0x1C, info.len());
    put32(&mut out, 0x20, new_fat_at);
    put32(&mut out, 0x24, fat.len());
    put32(&mut out, 0x28, new_file_at);
    put32(&mut out, 0x2C, file_block.len() + 16);
    Ok(out)
}

// ---- the files ---------------------------------------------------------

/// Wave archive, bank and sequence files for one wave.
fn swar_file(data: &[u8], rate: u32, looped: bool) -> Vec<u8> {
    let total_words = data.len() / 4;
    let timer = (TIMER_CLOCK / rate as f64).floor().clamp(1.0, 65535.0) as u16;
    let mut v = vec![0u8; 0x3C];
    v[..4].copy_from_slice(b"SWAR");
    v[4..8].copy_from_slice(&[0xFF, 0xFE, 0, 1]);
    v[0x0C..0x10].copy_from_slice(&[0x10, 0, 1, 0]);
    v[0x10..0x14].copy_from_slice(b"DATA");
    v[0x38..0x3C].copy_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(&0x40u32.to_le_bytes());
    // IMA-ADPCM: the 4-byte seed counts as the first word before the loop.
    v.extend_from_slice(&[2, looped as u8]);
    v.extend_from_slice(&(rate.min(65535) as u16).to_le_bytes());
    v.extend_from_slice(&timer.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&((total_words - 1) as u32).to_le_bytes());
    v.extend_from_slice(data);
    let size = v.len();
    put32(&mut v, 8, size);
    put32(&mut v, 0x14, size - 0x10);
    v
}

fn sbnk_file(wave_arc_slot: u16) -> Vec<u8> {
    let mut v = vec![0u8; 0x3C];
    v[..4].copy_from_slice(b"SBNK");
    v[4..8].copy_from_slice(&[0xFF, 0xFE, 0, 1]);
    v[0x0C..0x10].copy_from_slice(&[0x10, 0, 1, 0]);
    v[0x10..0x14].copy_from_slice(b"DATA");
    v[0x38..0x3C].copy_from_slice(&1u32.to_le_bytes());
    // One instrument: type 1 (a wave), its region at 0x40.
    v.extend_from_slice(&[1, 0x40, 0, 0]);
    // Region: wave 0, archive slot, base key, attack, decay, sustain,
    // release, pan.
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&wave_arc_slot.to_le_bytes());
    v.extend_from_slice(&[NOTE_KEY, 127, 127, 127, 123, 64]);
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
    let size = v.len();
    put32(&mut v, 8, size);
    put32(&mut v, 0x14, size - 0x10);
    v
}

fn varint(mut n: u32) -> Vec<u8> {
    let mut out = vec![(n & 0x7F) as u8];
    n >>= 7;
    while n > 0 {
        out.push((n & 0x7F) as u8 | 0x80);
        n >>= 7;
    }
    out.reverse();
    out
}

fn sseq_file(seconds: f64, repeat: bool) -> Vec<u8> {
    let ticks = (seconds * TICKS_PER_SECOND).ceil() as u32 + 4;
    let mut p = vec![0xFE, 0x01, 0x00, 0xE1, 120, 0, 0x81, 0, 0xC1, 127];
    let label = p.len();
    p.extend_from_slice(&[NOTE_KEY, 127]);
    p.extend(varint(ticks));
    if repeat {
        p.push(0x94);
        p.extend_from_slice(&(label as u32).to_le_bytes()[..3]);
    } else {
        p.push(0xFF);
    }
    while !(p.len() + 0x1C).is_multiple_of(4) {
        p.push(0);
    }
    let mut v = vec![0u8; 0x1C];
    v[..4].copy_from_slice(b"SSEQ");
    v[4..8].copy_from_slice(&[0xFF, 0xFE, 0, 1]);
    v[0x0C..0x10].copy_from_slice(&[0x10, 0, 1, 0]);
    v[0x10..0x14].copy_from_slice(b"DATA");
    v[0x18..0x1C].copy_from_slice(&0x1Cu32.to_le_bytes());
    v.extend_from_slice(&p);
    let size = v.len();
    put32(&mut v, 8, size);
    put32(&mut v, 0x14, size - 0x10);
    v
}

/// The size the heap of an effect player takes for a file: the file and a
/// header rounded to 32 bytes, plus a block header (`NNS_SndHeapAlloc`).
fn heap_cost(file_len: usize) -> usize {
    align(file_len + 0x20, 0x20) + 0x20
}

// ---- adding ------------------------------------------------------------

/// Letters, digits and underscores, upper case.
fn symbol(name: &str) -> Result<String, Error> {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        return Err(bad("give the sound a name"));
    }
    Ok(s)
}

/// Writes the sound archive into the project, or drops the override when
/// it is the game's own.
fn store(bridge: &Bridge, bytes: Vec<u8>) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open: changes need a mod to go in".into()))?;
    let io = |e: std::io::Error| Error::Missing(e.to_string());
    // The temporary name starts with '~', which the port and the change
    // list both skip.
    let path = project.root.join(SOUND_ARCHIVE);
    let tmp = path.with_file_name(format!("~{SOUND_ARCHIVE}.kenon"));
    fs::write(&tmp, &bytes).map_err(io)?;
    fs::rename(&tmp, &path).map_err(io)?;
    bridge.invalidate();
    Ok(())
}

fn add(bridge: &Bridge, name: &str, wav: &[u8], music: bool) -> Result<NewSound, Error> {
    let wav = read_wav(wav)?;
    let sdat = bridge
        .sdat()
        .ok_or_else(|| Error::Missing("the sound archive".into()))?;
    let stem = symbol(name)?;
    let kind = if music { "BGM" } else { "SE" };
    let (seq_name, bank_name, wave_name) = (
        format!("SEQ_{kind}_{stem}"),
        format!("BANK_{kind}_{stem}"),
        format!("WAVE_{kind}_{stem}"),
    );
    if sdat.find(Kind::Seq, &seq_name).is_some() {
        return Err(bad(format!("a sound named {seq_name} exists")));
    }
    let player = if music { MUSIC_PLAYER } else { EFFECT_PLAYER };

    // How much wave data the player can hold.
    let bank = sbnk_file(0);
    let probe = sseq_file(wav.seconds().min(600.0), music);
    let data_max = if music {
        MUSIC_DATA_MAX
    } else {
        let heap = sdat
            .player_record(player as u32)
            .map(|p| p.heap_size as usize)
            .filter(|&h| h > 0)
            .ok_or_else(|| bad("the sound player has no heap"))?;
        let left = heap
            .checked_sub(heap_cost(bank.len()) + heap_cost(probe.len()) + HEAP_MARGIN)
            .ok_or_else(|| bad("the sound player is too small"))?;
        let file_max = (left.saturating_sub(0x20) & !0x1F).saturating_sub(0x20);
        file_max.saturating_sub(0x4C) & !3
    };
    let mut rate = wav.rate.min(MAX_RATE);
    let mut note = (rate != wav.rate).then(|| format!("Brought down to {rate} Hz."));
    let cost = |rate: u32| {
        (wav.mono.len() as f64 * rate as f64 / wav.rate as f64 / 2.0).ceil() as usize + 8
    };
    if cost(rate) > data_max {
        rate = (((data_max - 8) as f64 * 2.0 * wav.rate as f64) / wav.mono.len() as f64).floor()
            as u32;
        if rate < MIN_RATE {
            return Err(bad(format!(
                "this sound is {:.1} s long; a {} can be {:.1} s",
                wav.seconds(),
                if music { "music loop" } else { "sound effect" },
                (data_max - 8) as f64 * 2.0 / MIN_RATE as f64
            )));
        }
        note = Some(format!("Brought down to {rate} Hz to fit the game's heap."));
    }
    let samples: Vec<i16> = resample(&wav.mono, wav.rate, rate)
        .iter()
        .map(|&v| (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect();
    let data = encode_adpcm(&samples);
    let seconds = samples.len() as f64 / rate as f64;

    let file_id = sdat.file_count() as usize;
    let (seq_id, bank_id, wave_id) = (
        sdat.count(Kind::Seq),
        sdat.count(Kind::Bank),
        sdat.count(Kind::WaveArc),
    );
    if bank_id > 0xFFFE || wave_id > 0xFFFE {
        return Err(bad("the sound archive has no room for another bank"));
    }
    let swar = swar_file(&data, rate, music);
    let bank = sbnk_file(0);
    let sseq = sseq_file(seconds, music);
    let le32 = |v: usize| (v as u32).to_le_bytes().to_vec();
    let entries = [
        Entry {
            kind: Kind::WaveArc,
            name: wave_name,
            record: le32(file_id),
        },
        Entry {
            kind: Kind::Bank,
            name: bank_name,
            record: [
                le32(file_id + 1),
                (wave_id as u16).to_le_bytes().to_vec(),
                vec![0xFF; 6],
            ]
            .concat(),
        },
        Entry {
            kind: Kind::Seq,
            name: seq_name.clone(),
            record: [
                le32(file_id + 2),
                (bank_id as u16).to_le_bytes().to_vec(),
                vec![127, 104, 64, player, 0, 0],
            ]
            .concat(),
        },
    ];
    let bytes = append(sdat.bytes(), &[swar.clone(), bank, sseq], &entries)?;
    // The new archive must read back as what was meant.
    let check = Sdat::parse(bytes.clone()).map_err(fmt_err)?;
    if check.count(Kind::Seq) != seq_id + 1
        || check.find(Kind::Seq, &seq_name) != Some(seq_id)
        || check.wave_archive_file(wave_id) != Some(&swar[..])
    {
        return Err(bad("the new sound did not land in the archive"));
    }
    store(bridge, bytes)?;
    Ok(NewSound {
        id: seq_id,
        name: seq_name,
        seconds,
        rate,
        bytes: data.len(),
        note,
    })
}

/// Adds the WAV as a sound effect and returns it. `name` is what the user
/// calls it; the archive gets `SEQ_SE_<NAME>`.
pub fn add_sound_effect(bridge: &Bridge, name: &str, wav: &[u8]) -> Result<NewSound, Error> {
    add(bridge, name, wav, false)
}

/// Adds the WAV as a music loop: a sequence on the music player that plays
/// the wave over and over. The wave must fit the shared sound heap with the
/// game's own data; a loop of a few seconds does.
pub fn add_music(bridge: &Bridge, name: &str, wav: &[u8]) -> Result<NewSound, Error> {
    add(bridge, name, wav, true)
}
