//! Placed models on a map: buildings, trees, signs and other props that
//! sit in each map chunk's object section.
//!
//! A chunk (a/0/0/8) is a tagged wrapper: tag, u16 section count n, then
//! n + 1 u32 offsets (section starts, then the end). The object section is
//! the last one: a u32 count, then 16-byte records of fx32 x, y, z
//! relative to the chunk centre (z negated), u16 rotation (65536 is a full
//! turn) and the model id stored high byte first (aletheia
//! `ov021/field_buildmodel.c`, `field_g3d_map_block.c`). Models come from
//! the area's member of a/2/3/0 (outdoors) or a/2/2/9 (indoors).

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error, Member};
use crate::zonemap;

const CHUNKS: u16 = 8;
const ZONE_HEADERS: u16 = 12;
const BUILDINGS_OUTDOOR: u16 = 230;
const BUILDINGS_INDOOR: u16 = 229;
const CHUNK_TILES: f32 = 32.0;
const TILE: f32 = 16.0;
const RECORD: usize = 16;

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

/// One placed model. Positions are in map tiles of the zone (as
/// `zone_map` lays them out), fractional; `y` in tiles; `rot` in degrees.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prop {
    pub chunk: usize,
    pub index: usize,
    pub x: f32,
    pub z: f32,
    pub y: f32,
    pub rot: f32,
    pub model: u16,
}

/// The object section of a chunk: (start, end) byte offsets.
fn section(bytes: &[u8]) -> Option<(usize, usize, usize)> {
    let n = rd16(bytes, 2)? as usize;
    if n == 0 {
        return None;
    }
    let start = rd32(bytes, 4 + (n - 1) * 4)? as usize;
    let end = rd32(bytes, 4 + n * 4)? as usize;
    (start + 4 <= end && end <= bytes.len()).then_some((n, start, end))
}

fn fx(v: f32) -> u32 {
    (v * 4096.0).round() as i32 as u32
}

fn unfx(v: u32) -> f32 {
    v as i32 as f32 / 4096.0
}

/// Every placed model of a zone.
pub fn props(bridge: &Bridge, zone: usize) -> Result<Vec<Prop>, Error> {
    let mut out = Vec::new();
    for (cx, cz, chunk) in zonemap::zone_chunks(bridge, zone)? {
        let Ok(bytes) = bridge.member(Member {
            arc: CHUNKS,
            index: chunk,
        }) else {
            continue;
        };
        let Some((_, start, end)) = section(&bytes) else {
            continue;
        };
        let count = rd32(&bytes, start).unwrap_or(0) as usize;
        let (ox, oz) = (
            cx as f32 * CHUNK_TILES + CHUNK_TILES / 2.0,
            cz as f32 * CHUNK_TILES + CHUNK_TILES / 2.0,
        );
        for index in 0..count.min((end - start - 4) / RECORD) {
            let at = start + 4 + index * RECORD;
            let (Some(x), Some(y), Some(z), Some(rot), Some(id)) = (
                rd32(&bytes, at),
                rd32(&bytes, at + 4),
                rd32(&bytes, at + 8),
                rd16(&bytes, at + 12),
                rd16(&bytes, at + 14),
            ) else {
                continue;
            };
            out.push(Prop {
                chunk,
                index,
                x: ox + unfx(x) / TILE,
                z: oz - unfx(z) / TILE,
                y: unfx(y) / TILE,
                rot: rot as f32 / 65536.0 * 360.0,
                model: id.swap_bytes(),
            });
        }
    }
    Ok(out)
}

/// The chunk under a map tile and its cell origin (in tiles).
fn chunk_at(bridge: &Bridge, zone: usize, x: f32, z: f32) -> Result<(usize, f32, f32), Error> {
    let (cx, cz) = ((x / CHUNK_TILES).floor(), (z / CHUNK_TILES).floor());
    zonemap::zone_chunks(bridge, zone)?
        .into_iter()
        .find(|&(a, b, _)| a as f32 == cx && b as f32 == cz)
        .map(|(a, b, c)| (c, a as f32 * CHUNK_TILES, b as f32 * CHUNK_TILES))
        .ok_or_else(|| Error::Missing("no map piece there".into()))
}

fn record(x: f32, y: f32, z: f32, rot: f32, model: u16, ox: f32, oz: f32) -> [u8; RECORD] {
    let half = CHUNK_TILES / 2.0;
    let mut r = [0u8; RECORD];
    r[0..4].copy_from_slice(&fx((x - ox - half) * TILE).to_le_bytes());
    r[4..8].copy_from_slice(&fx(y * TILE).to_le_bytes());
    r[8..12].copy_from_slice(&fx(-(z - oz - half) * TILE).to_le_bytes());
    let turn = (rot.rem_euclid(360.0) / 360.0 * 65536.0).round() as u32 as u16;
    r[12..14].copy_from_slice(&turn.to_le_bytes());
    r[14..16].copy_from_slice(&model.swap_bytes().to_le_bytes());
    r
}

fn load(bridge: &Bridge, chunk: usize) -> Result<Vec<u8>, Error> {
    bridge.member(Member {
        arc: CHUNKS,
        index: chunk,
    })
}

fn store(bridge: &Bridge, chunk: usize, bytes: &[u8]) -> Result<(), Error> {
    crate::catalog::write_member(
        bridge,
        Member {
            arc: CHUNKS,
            index: chunk,
        },
        bytes,
    )
}

/// Appends a record to a chunk's object section; returns its index.
fn append(bytes: &mut Vec<u8>, rec: &[u8; RECORD]) -> Result<usize, Error> {
    let (n, start, end) =
        section(bytes).ok_or_else(|| Error::Format("map piece has no object list".into()))?;
    let count = rd32(bytes, start).unwrap_or(0) as usize;
    let at = start + 4 + count * RECORD;
    if at > end {
        return Err(Error::Format("object list is short".into()));
    }
    bytes.splice(at..at, rec.iter().copied());
    bytes[start..start + 4].copy_from_slice(&((count + 1) as u32).to_le_bytes());
    // The object section is the last: only the end offset moves.
    let new_end = (end + RECORD) as u32;
    bytes[4 + n * 4..8 + n * 4].copy_from_slice(&new_end.to_le_bytes());
    Ok(count)
}

fn drop_record(bytes: &mut Vec<u8>, index: usize) -> Result<(), Error> {
    let (n, start, end) =
        section(bytes).ok_or_else(|| Error::Format("map piece has no object list".into()))?;
    let count = rd32(bytes, start).unwrap_or(0) as usize;
    if index >= count {
        return Err(Error::Missing(format!("object {index}")));
    }
    let at = start + 4 + index * RECORD;
    bytes.drain(at..at + RECORD);
    bytes[start..start + 4].copy_from_slice(&((count - 1) as u32).to_le_bytes());
    let new_end = (end - RECORD) as u32;
    bytes[4 + n * 4..8 + n * 4].copy_from_slice(&new_end.to_le_bytes());
    Ok(())
}

/// Moves, turns or raises a placed model. When it moves into another map
/// piece it changes lists; the result names where it is now.
pub fn set_prop(
    bridge: &Bridge,
    zone: usize,
    prop: &Prop,
    x: f32,
    z: f32,
    y: f32,
    rot: f32,
) -> Result<(usize, usize), Error> {
    let (target, ox, oz) = chunk_at(bridge, zone, x, z)?;
    let rec = record(x, y, z, rot, prop.model, ox, oz);
    if target == prop.chunk {
        let mut bytes = load(bridge, prop.chunk)?;
        let (_, start, _) =
            section(&bytes).ok_or_else(|| Error::Format("map piece has no object list".into()))?;
        let at = start + 4 + prop.index * RECORD;
        bytes
            .get_mut(at..at + RECORD)
            .ok_or_else(|| Error::Missing(format!("object {}", prop.index)))?
            .copy_from_slice(&rec);
        store(bridge, prop.chunk, &bytes)?;
        return Ok((prop.chunk, prop.index));
    }
    let mut from = load(bridge, prop.chunk)?;
    drop_record(&mut from, prop.index)?;
    let mut to = load(bridge, target)?;
    let index = append(&mut to, &rec)?;
    store(bridge, prop.chunk, &from)?;
    store(bridge, target, &to)?;
    Ok((target, index))
}

/// Places a model at a map tile; returns (chunk, index).
pub fn add_prop(
    bridge: &Bridge,
    zone: usize,
    model: u16,
    x: f32,
    z: f32,
    y: f32,
    rot: f32,
) -> Result<(usize, usize), Error> {
    let (chunk, ox, oz) = chunk_at(bridge, zone, x, z)?;
    let mut bytes = load(bridge, chunk)?;
    let index = append(&mut bytes, &record(x, y, z, rot, model, ox, oz))?;
    store(bridge, chunk, &bytes)?;
    Ok((chunk, index))
}

/// Removes a placed model. Later ones in its list move down one index.
pub fn remove_prop(bridge: &Bridge, chunk: usize, index: usize) -> Result<(), Error> {
    let mut bytes = load(bridge, chunk)?;
    drop_record(&mut bytes, index)?;
    store(bridge, chunk, &bytes)
}

/// The raw record of a placed model, to restore it after a delete.
pub fn prop_record(bridge: &Bridge, chunk: usize, index: usize) -> Result<Vec<u8>, Error> {
    let bytes = load(bridge, chunk)?;
    let (_, start, end) =
        section(&bytes).ok_or_else(|| Error::Format("map piece has no object list".into()))?;
    let count = rd32(&bytes, start).unwrap_or(0) as usize;
    let at = start + 4 + index * RECORD;
    if index >= count || at + RECORD > end {
        return Err(Error::Missing(format!("object {index}")));
    }
    Ok(bytes[at..at + RECORD].to_vec())
}

/// Puts a removed record back at its index in a chunk's object list.
pub fn insert_prop(
    bridge: &Bridge,
    chunk: usize,
    index: usize,
    record: &[u8],
) -> Result<(), Error> {
    if record.len() != RECORD {
        return Err(Error::Format(format!("object record is {RECORD} bytes")));
    }
    let mut bytes = load(bridge, chunk)?;
    let (n, start, end) =
        section(&bytes).ok_or_else(|| Error::Format("map piece has no object list".into()))?;
    let count = rd32(&bytes, start).unwrap_or(0) as usize;
    if index > count || start + 4 + count * RECORD > end {
        return Err(Error::Missing(format!("object {index}")));
    }
    let at = start + 4 + index * RECORD;
    bytes.splice(at..at, record.iter().copied());
    bytes[start..start + 4].copy_from_slice(&((count + 1) as u32).to_le_bytes());
    let new_end = (end + RECORD) as u32;
    bytes[4 + n * 4..8 + n * 4].copy_from_slice(&new_end.to_le_bytes());
    store(bridge, chunk, &bytes)
}

/// The building set member a zone's area uses.
fn building_set(bridge: &Bridge, zone: usize) -> Result<Vec<u8>, Error> {
    let headers = bridge.member(Member {
        arc: ZONE_HEADERS,
        index: 0,
    })?;
    let area = rd16(&headers, zone * 48 + 2).unwrap_or(0);
    let area_file = bridge
        .source()
        .read("a/0/1/3")
        .map_err(|e| Error::Missing(format!("a/0/1/3: {e}")))?;
    let indoor = area_file
        .get(area as usize * 10 + 6)
        .is_some_and(|&b| b != 0);
    let arc = if indoor {
        BUILDINGS_INDOOR
    } else {
        BUILDINGS_OUTDOOR
    };
    let count = bridge.base_archive(arc)?.len();
    let m = if area >= 0xD2 {
        area as usize - 0xD2
    } else if area >= 2 {
        (area as usize - 2) / 4
    } else {
        0
    };
    let member = if m < count { m } else { 0 };
    let bytes = bridge.member(Member { arc, index: member })?;
    Ok(bytes)
}

/// The models a zone's area offers, by id (the ids props use).
pub fn prop_models(bridge: &Bridge, zone: usize) -> Result<Vec<u16>, Error> {
    let bytes = building_set(bridge, zone)?;
    let n = bytes.get(2).copied().unwrap_or(0) as usize / 2;
    Ok((0..n)
        .filter_map(|i| {
            let def = rd32(&bytes, 4 + i * 4)? as usize;
            rd16(&bytes, def)
        })
        .collect())
}

/// The area's models with the name stored in each model file (pc01,
/// stair_u01, ...), by id.
pub fn prop_model_names(bridge: &Bridge, zone: usize) -> Result<Vec<(u16, String)>, Error> {
    let bytes = building_set(bridge, zone)?;
    let n = bytes.get(2).copied().unwrap_or(0) as usize / 2;
    Ok((0..n)
        .filter_map(|i| {
            let id = rd16(&bytes, rd32(&bytes, 4 + i * 4)? as usize)?;
            let file = rd32(&bytes, 4 + (n + i) * 4)? as usize;
            let name = crate::nsbmd::parse(bytes.get(file..)?)
                .map(|m| m.name)
                .unwrap_or_default();
            Some((id, name))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trip() {
        let r = record(40.5, 1.0, 20.25, 90.0, 0x1234, 32.0, 0.0);
        let (x, y, z) = (
            unfx(rd32(&r, 0).unwrap()),
            unfx(rd32(&r, 4).unwrap()),
            unfx(rd32(&r, 8).unwrap()),
        );
        assert_eq!(32.0 + 16.0 + x / TILE, 40.5);
        assert_eq!(16.0 - z / TILE, 20.25);
        assert_eq!(y / TILE, 1.0);
        assert_eq!(rd16(&r, 12), Some(16384));
        assert_eq!(rd16(&r, 14).map(u16::swap_bytes), Some(0x1234));
    }
}
