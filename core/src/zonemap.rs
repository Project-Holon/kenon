//! A zone's map, tile by tile, with everything placed on it: the per-tile
//! terrain grid of its map chunks and the events of its event file
//! (people and trainers, warps, signs and hidden items, step triggers).
//! Layouts from docs/game-data-map.md ("Map matrix and map chunks",
//! "Event data and trainer locations").
//!
//! ```text
//! matrix  a/0/0/9 member matrixId: u16 mode, u16, u16 width, u16 height,
//!         width*height u32 chunk ids (0xFFFFFFFF empty), then in mode 1
//!         width*height u32 zone ids
//! chunk   a/0/0/8 member: tag "WB"/"GC"/"RD" with the tile grid at the
//!         u32 at 8 ("NG" has none); grid u16 width, u16 height, then 8-byte
//!         records: u16 surface, u16 height index, u32 attribute (bits 0-15
//!         terrain id, bits 16-30 flags)
//! ```

use std::collections::HashMap;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::tables::{self, Field, Value};
use crate::world::{self, Zone};

const MATRICES: u16 = 9;
const OBJECT_CODES: u16 = 48;
const CHUNKS: u16 = 8;
const EVENT_FILES: u16 = 125;
const ZONE_HEADERS: u16 = 12;
const CHUNK_TILES: usize = 32;
const TILE_RECORD: usize = 8;

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

/// The byte offset of the first tile record of a chunk, and the grid size.
fn chunk_grid(bytes: &[u8]) -> Option<(usize, u16, u16)> {
    match bytes.get(..2)? {
        b"WB" | b"GC" | b"RD" => {
            let grid = rd32(bytes, 8)? as usize;
            Some((grid + 4, rd16(bytes, grid)?, rd16(bytes, grid + 2)?))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// "npc", "warp", "pos" (step trigger) or "bg" (sign, hidden item).
    pub kind: String,
    pub index: usize,
    /// Tile position on the zone's map (matrix tiles), and size in tiles.
    pub x: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    /// The trainer an NPC battles, when it is one.
    pub trainer: Option<u16>,
    /// For a warp: the zone it leads to.
    pub to_zone: Option<u16>,
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneMap {
    pub zone: Zone,
    /// Matrix tile of the map's top-left corner.
    pub origin_x: i32,
    pub origin_z: i32,
    pub width: usize,
    pub height: usize,
    /// Per tile: terrain id (-1 where no chunk is), flags.
    pub terrain: Vec<i32>,
    pub flags: Vec<u16>,
    pub events: Vec<Event>,
}

struct Matrix {
    mode: u16,
    width: usize,
    height: usize,
    chunks: Vec<u32>,
    zones: Vec<u32>,
}

fn matrix(bridge: &Bridge, id: u16) -> Result<Matrix, Error> {
    let b = bridge.member(Member {
        arc: MATRICES,
        index: id as usize,
    })?;
    let bad = || Error::Format(format!("matrix {id}"));
    let mode = rd16(&b, 0).ok_or_else(bad)?;
    let (w, h) = (
        rd16(&b, 4).ok_or_else(bad)? as usize,
        rd16(&b, 6).ok_or_else(bad)? as usize,
    );
    let read = |start: usize| -> Vec<u32> {
        (0..w * h)
            .map(|i| rd32(&b, start + i * 4).unwrap_or(u32::MAX))
            .collect()
    };
    Ok(Matrix {
        mode,
        width: w,
        height: h,
        chunks: read(8),
        zones: if mode == 1 {
            read(8 + 4 * w * h)
        } else {
            Vec::new()
        },
    })
}

/// The event file layout from schemas.json.
fn event_arrays() -> &'static [tables::EventArray] {
    tables::table("eventData")
        .ok()
        .and_then(|t| t.arrays.as_deref())
        .unwrap_or(&[])
}

/// (start offset, count) of each event array, in file order.
fn array_spans(file: &[u8]) -> Vec<(usize, usize)> {
    let mut at = 8;
    event_arrays()
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let count = file.get(4 + i).copied().unwrap_or(0) as usize;
            let span = (at, count);
            at += count * a.record_size;
            span
        })
        .collect()
}

fn field<'a>(fields: &'a [Field], name: &str) -> Option<&'a Field> {
    fields.iter().find(|f| f.name == name)
}

fn get(file: &[u8], base: usize, fields: &[Field], name: &str) -> i64 {
    field(fields, name)
        .and_then(|f| tables::read_field(file, base, f))
        .unwrap_or(0)
}

/// A zone's map and events.
pub fn zone_map(bridge: &Bridge, zone_id: usize) -> Result<ZoneMap, Error> {
    let zones = world::zones(bridge)?;
    let zone = zones
        .get(zone_id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("zone {zone_id}")))?;
    let m = matrix(bridge, zone.matrix)?;
    // The cells this zone owns: in a shared matrix (mode 1) those that
    // name it, otherwise every cell with a chunk.
    let cells: Vec<(usize, usize)> = (0..m.width * m.height)
        .filter(|&i| m.chunks[i] != u32::MAX)
        .filter(|&i| m.mode != 1 || m.zones.get(i) == Some(&(zone_id as u32)))
        .map(|i| (i % m.width, i / m.width))
        .collect();
    let (cx0, cz0) = (
        cells.iter().map(|c| c.0).min().unwrap_or(0),
        cells.iter().map(|c| c.1).min().unwrap_or(0),
    );
    let (cx1, cz1) = (
        cells.iter().map(|c| c.0).max().unwrap_or(0),
        cells.iter().map(|c| c.1).max().unwrap_or(0),
    );
    let (wc, hc) = (cx1 + 1 - cx0, cz1 + 1 - cz0);
    let (width, height) = (wc * CHUNK_TILES, hc * CHUNK_TILES);
    let mut terrain = vec![-1i32; width * height];
    let mut flags = vec![0u16; width * height];
    for &(cx, cz) in &cells {
        let chunk = m.chunks[cz * m.width + cx] as usize;
        let Ok(bytes) = bridge.member(Member {
            arc: CHUNKS,
            index: chunk,
        }) else {
            continue;
        };
        let Some((first, gw, gh)) = chunk_grid(&bytes) else {
            continue;
        };
        for tz in 0..(gh as usize).min(CHUNK_TILES) {
            for tx in 0..(gw as usize).min(CHUNK_TILES) {
                let at = first + (tz * gw as usize + tx) * TILE_RECORD;
                let Some(attr) = rd32(&bytes, at + 4) else {
                    continue;
                };
                let i = ((cz - cz0) * CHUNK_TILES + tz) * width + (cx - cx0) * CHUNK_TILES + tx;
                terrain[i] = (attr & 0xFFFF) as i32;
                flags[i] = ((attr >> 16) & 0x7FFF) as u16;
            }
        }
    }

    let file = bridge.member(Member {
        arc: EVENT_FILES,
        index: zone.event_data as usize,
    })?;
    let mut events = Vec::new();
    for (array, (start, count)) in event_arrays().iter().zip(array_spans(&file)) {
        for index in 0..count {
            let base = start + index * array.record_size;
            let f = &array.fields;
            let g = |n: &str| get(&file, base, f, n);
            let rail = g("posType") != 0;
            let (x, z, w, h) = match array.name.as_str() {
                "npc" if !rail => (g("gridX"), g("gridZ"), 1, 1),
                "bg" if !rail => (g("x"), g("z"), 1, 1),
                // Warp positions are in sixteenths of a tile.
                "warp" if !rail => (
                    g("x") >> 4,
                    g("z") >> 4,
                    g("sizeX").max(1),
                    g("sizeZ").max(1),
                ),
                "pos" => (g("x"), g("z"), g("sizeX").max(1), g("sizeZ").max(1)),
                _ => (-1, -1, 0, 0),
            };
            let event_id = g("eventId") as u16;
            let trainer = (array.name == "npc")
                .then(|| match event_id {
                    3000..=4999 => Some(event_id - 3000),
                    5000..=6999 => Some(event_id - 5000),
                    _ => None,
                })
                .flatten();
            events.push(Event {
                kind: array.name.clone(),
                index,
                x: x as i32 - (cx0 * CHUNK_TILES) as i32,
                z: z as i32 - (cz0 * CHUNK_TILES) as i32,
                w: w as i32,
                h: h as i32,
                trainer,
                to_zone: (array.name == "warp").then(|| g("zoneId") as u16),
                values: tables::values(&file, base, f),
            });
        }
    }
    Ok(ZoneMap {
        zone,
        origin_x: (cx0 * CHUNK_TILES) as i32,
        origin_z: (cz0 * CHUNK_TILES) as i32,
        width,
        height,
        terrain,
        flags,
        events,
    })
}

fn event_file(bridge: &Bridge, zone_id: usize) -> Result<(Member, Vec<u8>), Error> {
    let zones = world::zones(bridge)?;
    let zone = zones
        .get(zone_id)
        .ok_or_else(|| Error::Missing(format!("zone {zone_id}")))?;
    let m = Member {
        arc: EVENT_FILES,
        index: zone.event_data as usize,
    };
    Ok((m, bridge.member(m)?))
}

fn array_index(kind: &str) -> Result<usize, Error> {
    event_arrays()
        .iter()
        .position(|a| a.name == kind)
        .ok_or_else(|| Error::BadPath(format!("no event kind {kind}")))
}

/// Changes fields of one event and writes the zone's event file.
pub fn save_event(
    bridge: &Bridge,
    zone_id: usize,
    kind: &str,
    index: usize,
    values: &HashMap<String, i64>,
) -> Result<(), Error> {
    let (m, mut file) = event_file(bridge, zone_id)?;
    let a = array_index(kind)?;
    let array = &event_arrays()[a];
    let (start, count) = array_spans(&file)[a];
    if index >= count {
        return Err(Error::Missing(format!("{kind} {index}")));
    }
    let base = start + index * array.record_size;
    for f in &array.fields {
        if let Some(&v) = values.get(&f.name) {
            if f.when
                .as_ref()
                .is_none_or(|w| get(&file, base, &array.fields, &w.field) == w.equals)
            {
                let (min, max) = tables::range(f);
                tables::write_field(&mut file, base, f, v.clamp(min, max));
            }
        }
    }
    crate::catalog::write_member(bridge, m, &file)
}

/// Adds an event at a map tile (copying the zone's first event of that
/// kind as a template, or zeros), and returns its index.
pub fn add_event(
    bridge: &Bridge,
    zone_id: usize,
    kind: &str,
    x: i32,
    z: i32,
) -> Result<usize, Error> {
    let map = zone_map(bridge, zone_id)?;
    let (m, mut file) = event_file(bridge, zone_id)?;
    let a = array_index(kind)?;
    let array = &event_arrays()[a];
    let (start, count) = array_spans(&file)[a];
    if count >= 255 {
        return Err(Error::BadPath(
            "this zone holds the most events of that kind".into(),
        ));
    }
    let size = array.record_size;
    let mut record = if count > 0 {
        file[start..start + size].to_vec()
    } else {
        vec![0; size]
    };
    let (mx, mz) = ((x + map.origin_x) as i64, (z + map.origin_z) as i64);
    let set = |rec: &mut Vec<u8>, name: &str, v: i64| {
        if let Some(f) = field(&array.fields, name) {
            tables::write_field(rec, 0, f, v);
        }
    };
    set(&mut record, "posType", 0);
    match kind {
        "npc" => {
            set(&mut record, "gridX", mx);
            set(&mut record, "gridZ", mz);
            set(&mut record, "objId", count as i64);
            // A plain person: no script, no trainer, always present.
            set(&mut record, "eventType", 0);
            set(&mut record, "eventId", 0);
            set(&mut record, "eventFlag", 0);
        }
        "warp" => {
            set(&mut record, "x", mx << 4);
            set(&mut record, "z", mz << 4);
            set(&mut record, "sizeX", 1);
            set(&mut record, "sizeZ", 1);
        }
        _ => {
            set(&mut record, "x", mx);
            set(&mut record, "z", mz);
            set(&mut record, "sizeX", 1);
            set(&mut record, "sizeZ", 1);
            set(&mut record, "scriptId", 0);
        }
    }
    let at = start + count * size;
    file.splice(at..at, record);
    file[4 + a] = (count + 1) as u8;
    // The header's offset of what follows the arrays moves with them.
    let trailer = rd32(&file, 0).unwrap_or(0) + size as u32;
    file[0..4].copy_from_slice(&trailer.to_le_bytes());
    crate::catalog::write_member(bridge, m, &file)?;
    Ok(count)
}

/// Removes an event. Later events of the kind move down one index.
pub fn remove_event(
    bridge: &Bridge,
    zone_id: usize,
    kind: &str,
    index: usize,
) -> Result<(), Error> {
    let (m, mut file) = event_file(bridge, zone_id)?;
    let a = array_index(kind)?;
    let size = event_arrays()[a].record_size;
    let (start, count) = array_spans(&file)[a];
    if index >= count {
        return Err(Error::Missing(format!("{kind} {index}")));
    }
    let at = start + index * size;
    file.drain(at..at + size);
    file[4 + a] = (count - 1) as u8;
    let trailer = rd32(&file, 0).unwrap_or(0).saturating_sub(size as u32);
    file[0..4].copy_from_slice(&trailer.to_le_bytes());
    crate::catalog::write_member(bridge, m, &file)
}

/// One tile change: map tile (as `zone_map` lays them out), terrain id
/// and flags.
pub type TileWrite = (usize, usize, u16, u16);

/// Writes tiles, each with its own terrain and flags, writing each map
/// chunk once.
pub fn write_tiles(bridge: &Bridge, zone_id: usize, tiles: &[TileWrite]) -> Result<(), Error> {
    let zones = world::zones(bridge)?;
    let zone = zones
        .get(zone_id)
        .ok_or_else(|| Error::Missing(format!("zone {zone_id}")))?;
    let map = zone_map(bridge, zone_id)?;
    let m = matrix(bridge, zone.matrix)?;
    let mut chunks: HashMap<usize, Vec<u8>> = HashMap::new();
    for &(x, z, terrain, flags) in tiles {
        let (mx, mz) = (x + map.origin_x as usize, z + map.origin_z as usize);
        let (cx, cz) = (mx / CHUNK_TILES, mz / CHUNK_TILES);
        let Some(&chunk) = m.chunks.get(cz * m.width + cx).filter(|&&c| c != u32::MAX) else {
            continue;
        };
        let bytes = match chunks.entry(chunk as usize) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(bridge.member(Member {
                arc: CHUNKS,
                index: chunk as usize,
            })?),
        };
        let Some((first, gw, _)) = chunk_grid(bytes) else {
            continue;
        };
        let at = first + ((mz % CHUNK_TILES) * gw as usize + mx % CHUNK_TILES) * TILE_RECORD + 4;
        let Some(old) = rd32(bytes, at) else {
            continue;
        };
        // Bit 31 belongs to the slope search, not the attribute: keep it.
        let attr = (old & 0x8000_0000) | ((flags as u32 & 0x7FFF) << 16) | terrain as u32;
        bytes[at..at + 4].copy_from_slice(&attr.to_le_bytes());
    }
    for (index, bytes) in chunks {
        crate::catalog::write_member(bridge, Member { arc: CHUNKS, index }, &bytes)?;
    }
    Ok(())
}

/// Sets many tiles to one terrain id and flags.
pub fn set_tiles(
    bridge: &Bridge,
    zone_id: usize,
    tiles: &[(usize, usize)],
    terrain: u16,
    flags: u16,
) -> Result<(), Error> {
    let writes: Vec<TileWrite> = tiles.iter().map(|&(x, z)| (x, z, terrain, flags)).collect();
    write_tiles(bridge, zone_id, &writes)
}

/// Sets a tile's terrain id and flags.
pub fn set_tile(
    bridge: &Bridge,
    zone_id: usize,
    x: usize,
    z: usize,
    terrain: u16,
    flags: u16,
) -> Result<(), Error> {
    write_tiles(bridge, zone_id, &[(x, z, terrain, flags)])
}

/// Moves an event so its corner is on map tile (x, z), in each kind's own
/// units (warps keep their sixteenths within the tile).
pub fn move_event(
    bridge: &Bridge,
    zone_id: usize,
    kind: &str,
    index: usize,
    x: i32,
    z: i32,
) -> Result<(), Error> {
    let map = zone_map(bridge, zone_id)?;
    let ev = map
        .events
        .iter()
        .find(|e| e.kind == kind && e.index == index)
        .ok_or_else(|| Error::Missing(format!("{kind} {index}")))?;
    let (mx, mz) = ((x + map.origin_x) as i64, (z + map.origin_z) as i64);
    let value = |key: &str| {
        ev.values
            .iter()
            .find(|v| v.key == key)
            .map_or(0, |v| v.value)
    };
    let mut values = HashMap::new();
    match kind {
        "npc" => {
            values.insert("gridX".to_string(), mx);
            values.insert("gridZ".to_string(), mz);
        }
        "warp" => {
            values.insert("x".to_string(), (mx << 4) | (value("x") & 15));
            values.insert("z".to_string(), (mz << 4) | (value("z") & 15));
        }
        _ => {
            values.insert("x".to_string(), mx);
            values.insert("z".to_string(), mz);
        }
    }
    save_event(bridge, zone_id, kind, index, &values)
}

/// Makes a person battle a trainer (or stop battling, with `None`).
pub fn set_trainer(
    bridge: &Bridge,
    zone_id: usize,
    index: usize,
    trainer: Option<u16>,
) -> Result<(), Error> {
    let mut values = HashMap::new();
    match trainer {
        Some(t) => {
            values.insert("eventType".to_string(), 1);
            values.insert("eventId".to_string(), 3000 + t as i64);
        }
        None => {
            values.insert("eventType".to_string(), 0);
            values.insert("eventId".to_string(), 0);
        }
    }
    save_event(bridge, zone_id, "npc", index, &values)
}

/// Copies an event to map tile (x, z) and returns the copy's index.
pub fn copy_event(
    bridge: &Bridge,
    zone_id: usize,
    kind: &str,
    index: usize,
    x: i32,
    z: i32,
) -> Result<usize, Error> {
    let (m, mut file) = event_file(bridge, zone_id)?;
    let a = array_index(kind)?;
    let size = event_arrays()[a].record_size;
    let (start, count) = array_spans(&file)[a];
    if index >= count {
        return Err(Error::Missing(format!("{kind} {index}")));
    }
    if count >= 255 {
        return Err(Error::BadPath(
            "this zone holds the most events of that kind".into(),
        ));
    }
    let record = file[start + index * size..start + (index + 1) * size].to_vec();
    let at = start + count * size;
    file.splice(at..at, record);
    file[4 + a] = (count + 1) as u8;
    let trailer = rd32(&file, 0).unwrap_or(0) + size as u32;
    file[0..4].copy_from_slice(&trailer.to_le_bytes());
    crate::catalog::write_member(bridge, m, &file)?;
    move_event(bridge, zone_id, kind, count, x, z)?;
    Ok(count)
}

/// Sets a zone's seasonal music (zone header 0x0C..0x14), in place in the
/// one-member zone header archive.
pub fn set_music(bridge: &Bridge, zone_id: usize, music: [u16; 4]) -> Result<(), Error> {
    let m = Member {
        arc: ZONE_HEADERS,
        index: 0,
    };
    let mut headers = bridge.member(m)?;
    let at = zone_id * 48 + 0x0C;
    if at + 8 > headers.len() {
        return Err(Error::Missing(format!("zone {zone_id}")));
    }
    for (i, v) in music.iter().enumerate() {
        headers[at + i * 2..at + i * 2 + 2].copy_from_slice(&v.to_le_bytes());
    }
    crate::catalog::write_member(bridge, m, &headers)
}

/// For each object code (a/0/4/8), the a/0/4/9 member of its billboard
/// texture: one 32x32 frame per texture, three per facing (up, down,
/// left, right). `None` for codes drawn another way.
pub fn object_textures(bridge: &Bridge) -> Result<Vec<Option<u16>>, Error> {
    let table = bridge.member(Member {
        arc: OBJECT_CODES,
        index: 0,
    })?;
    // A u32 count, then 28-byte records that start with their code.
    let count = rd32(&table, 0).unwrap_or(0) as usize;
    let mut out: Vec<Option<u16>> = Vec::new();
    for i in 0..count {
        let at = 4 + i * 28;
        let (Some(code), Some(&draw), Some(tex)) =
            (rd16(&table, at), table.get(at + 2), rd16(&table, at + 0x10))
        else {
            break;
        };
        // drawType 1: a billboard with a frame texture.
        if draw == 1 {
            let code = code as usize;
            if out.len() <= code {
                out.resize(code + 1, None);
            }
            out[code] = Some(tex);
        }
    }
    Ok(out)
}

/// The places whose maps use a chunk, as (zone, place name), one per
/// place name.
pub fn chunk_places(bridge: &Bridge, chunk: usize) -> Vec<(usize, String)> {
    let Ok(zones) = world::zones(bridge) else {
        return Vec::new();
    };
    let mut seen: HashMap<u16, Option<Matrix>> = HashMap::new();
    let mut out: Vec<(usize, String)> = Vec::new();
    for z in zones.iter() {
        let m = seen
            .entry(z.matrix)
            .or_insert_with(|| matrix(bridge, z.matrix).ok());
        let Some(m) = m else { continue };
        let uses = (0..m.chunks.len()).any(|i| {
            m.chunks[i] as usize == chunk && (m.mode != 1 || m.zones.get(i) == Some(&(z.id as u32)))
        });
        if uses && !out.iter().any(|(_, p)| *p == z.place) {
            out.push((z.id, z.place.clone()));
        }
    }
    out
}

/// A chunk's tile grid: width, height and (terrain id, flags) per tile.
pub type ChunkTiles = (usize, usize, Vec<(u16, u16)>);

/// Reads a chunk's tile grid.
pub fn chunk_tiles(bytes: &[u8]) -> Option<ChunkTiles> {
    let (first, w, h) = chunk_grid(bytes)?;
    let (w, h) = (w as usize, h as usize);
    let tiles = (0..w * h)
        .map(|i| {
            let attr = rd32(bytes, first + i * TILE_RECORD + 4).unwrap_or(0);
            ((attr & 0xFFFF) as u16, ((attr >> 16) & 0x7FFF) as u16)
        })
        .collect();
    Some((w, h, tiles))
}

/// How many tile records differ between two versions of a map chunk.
pub fn changed_tiles(base: &[u8], current: &[u8]) -> usize {
    let (Some((fa, wa, ha)), Some((fb, _, _))) = (chunk_grid(base), chunk_grid(current)) else {
        return 0;
    };
    (0..wa as usize * ha as usize)
        .filter(|i| {
            let (a, b) = (fa + i * TILE_RECORD, fb + i * TILE_RECORD);
            base.get(a..a + TILE_RECORD) != current.get(b..b + TILE_RECORD)
        })
        .count()
}

/// The events that differ between two versions of an event file:
/// (what, the game's, the mod's).
pub fn diff_events(base: &[u8], current: &[u8]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let (sa, sb) = (array_spans(base), array_spans(current));
    for (a, array) in event_arrays().iter().enumerate() {
        let ((ba, ca), (bb, cb)) = (sa[a], sb[a]);
        let (label, many) = match array.name.as_str() {
            "npc" => ("Person", "People"),
            "warp" => ("Warp", "Warps"),
            "pos" => ("Trigger", "Triggers"),
            _ => ("Sign", "Signs"),
        };
        if ca != cb {
            out.push((many.to_string(), ca.to_string(), cb.to_string()));
        }
        for i in 0..ca.min(cb) {
            let (x, y) = (ba + i * array.record_size, bb + i * array.record_size);
            let (Some(ra), Some(rb)) = (
                base.get(x..x + array.record_size),
                current.get(y..y + array.record_size),
            ) else {
                continue;
            };
            if ra == rb {
                continue;
            }
            let va = tables::values(ra, 0, &array.fields);
            let vb = tables::values(rb, 0, &array.fields);
            for (p, q) in va.iter().zip(&vb) {
                if p.value != q.value {
                    out.push((
                        format!("{label} {}: {}", i + 1, q.label),
                        p.value.to_string(),
                        q.value.to_string(),
                    ));
                }
            }
        }
    }
    out
}
