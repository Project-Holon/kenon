//! Where things are in the world: zones (maps), their place names, and the
//! trainers that stand in them.
//!
//! Layouts from docs/game-data-map.md, "Event data and trainer locations":
//!
//! ```text
//! zone headers  a/0/1/2 member 0: 48-byte records, record = zone id;
//!               u16 matrixId at 0x04, u16 eventDataId at 0x16, place name
//!               id in bits 0-9 of the u16 at 0x1A (text bank 89 of a/0/0/2)
//! event file    a/1/2/5 member eventDataId: u32 trailer offset, then u8
//!               counts of signs, NPCs, warps and position events; then the
//!               arrays in that order (20, 36, 20 and 22 bytes per record)
//! NPC record    u16 eventId at 0x0A; s32 posType at 0x18; when posType is
//!               0, u16 gridX at 0x1C and u16 gridZ at 0x1E (tiles in the
//!               zone's matrix, 32 per chunk)
//! ```
//!
//! A trainer NPC has eventId 3000 + trainer id (5000 + id for the second
//! of a pair): ov010/event_trainer.c:57-69. Values come from the user's ROM.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::tables::{bank_strings, BankRef};

const ZONE_HEADERS: u16 = 12;
const ZONE_RECORD: usize = 48;
const EVENT_FILES: u16 = 125;
const PLACE_NAMES: BankRef = BankRef {
    archive: 2,
    bank: 89,
};
const TRAINER_EVENT: u16 = 3000;
const TRAINER_EVENT_PAIR: u16 = 5000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub id: usize,
    pub place: String,
    pub matrix: u16,
    pub event_data: u16,
}

/// One place a trainer stands.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub zone: usize,
    pub place: String,
    /// The tile in the zone's matrix, when the NPC stands on the grid.
    pub tile: Option<(u16, u16)>,
    /// The second trainer of a pair (eventId 5000 + id).
    pub pair: bool,
}

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

/// Every zone with its place name.
pub fn zones(bridge: &Bridge) -> Result<Arc<Vec<Zone>>, Error> {
    bridge.cached("zones", || {
        let headers = bridge.member(Member {
            arc: ZONE_HEADERS,
            index: 0,
        })?;
        let names = bank_strings(bridge, &PLACE_NAMES);
        Ok(headers
            .chunks(ZONE_RECORD)
            .filter(|r| r.len() == ZONE_RECORD)
            .enumerate()
            .map(|(id, r)| {
                let place_id = (rd16(r, 0x1A).unwrap_or(0) & 0x3FF) as usize;
                Zone {
                    id,
                    place: names
                        .get(place_id)
                        .filter(|n| !n.is_empty())
                        .cloned()
                        .unwrap_or_else(|| format!("Zone {id}")),
                    matrix: rd16(r, 0x04).unwrap_or(0),
                    event_data: rd16(r, 0x16).unwrap_or(0),
                }
            })
            .collect())
    })
}

/// Trainer id -> where its NPCs stand, built once per game.
pub fn trainer_locations(bridge: &Bridge) -> Result<Arc<BTreeMap<u16, Vec<Location>>>, Error> {
    bridge.cached("trainer_locations", || {
        let mut out: BTreeMap<u16, Vec<Location>> = BTreeMap::new();
        for zone in zones(bridge)?.iter() {
            let Ok(file) = bridge.member(Member {
                arc: EVENT_FILES,
                index: zone.event_data as usize,
            }) else {
                continue;
            };
            let Some(counts) = file.get(4..8) else {
                continue;
            };
            let (signs, npcs) = (counts[0] as usize, counts[1] as usize);
            let first = 8 + 20 * signs;
            for n in 0..npcs {
                let at = first + 36 * n;
                let Some(event) = rd16(&file, at + 0x0A) else {
                    break;
                };
                let (id, pair) = match event {
                    e if (TRAINER_EVENT..TRAINER_EVENT_PAIR).contains(&e) => {
                        (e - TRAINER_EVENT, false)
                    }
                    e if (TRAINER_EVENT_PAIR..TRAINER_EVENT_PAIR + 2000).contains(&e) => {
                        (e - TRAINER_EVENT_PAIR, true)
                    }
                    _ => continue,
                };
                let tile = (rd32(&file, at + 0x18) == Some(0))
                    .then(|| Some((rd16(&file, at + 0x1C)?, rd16(&file, at + 0x1E)?)))
                    .flatten();
                out.entry(id).or_default().push(Location {
                    zone: zone.id,
                    place: zone.place.clone(),
                    tile,
                    pair,
                });
            }
        }
        Ok(out)
    })
}
