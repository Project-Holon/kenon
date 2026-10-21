//! The item an item ball or a hidden item gives. The game keeps no item
//! table for them: the event's script id selects a tiny script of the
//! shared script files, and that script stores the item and the count in
//! work variables before it calls the common pick-up routine.
//!
//! ```text
//! item ball     script id 7000..=7799, file 864 of a/0/5/7
//!               SetWork 0x800C item; SetWork 0x800D count; Call common
//! hidden item   script id 8000..=8799, file 865 of a/0/5/7 (bg event of
//!               type 2; its flag is 900 + id - 8000)
//!               SetWork 0x8000 item; SetWork 0x8001 count;
//!               SetWork 0x8002 flag; Call common
//! ```
//!
//! Sources (aletheia): the ranges table of overlay 10 (see `scripts`),
//! ov010/hidden_item.c (HIDDEN_ITEM_GetIndex, HIDDEN_ITEM_GetFlag,
//! HIDDEN_ITEM_SCRIPT_FIRST/LAST, HIDDEN_ITEM_FLAG_BASE),
//! ov010/eventdata_system.c (SearchBgEventScriptId, type 2),
//! ov021/scrcmd_trainer_audio_item.c (SCRCMD_AddItem, SCRCMD_CanAddItem).
//! The work numbers are read from the data of the Black ROM: every script of
//! both ranges has this shape.

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::catalog;
use crate::scripts::{
    decode, script_code, LITERAL_MAX, OP_SET_CONST, OP_SET_VALUE, SCRIPT_MEMBERS,
};

pub const BALL_FIRST: u16 = 7000;
pub const BALL_LAST: u16 = 7799;
pub const HIDDEN_FIRST: u16 = 8000;
pub const HIDDEN_LAST: u16 = 8799;
/// Event flag of hidden item script `HIDDEN_FIRST + n` is `n + this`.
pub const HIDDEN_FLAG_BASE: u16 = 900;

/// Work numbers the script sets: item, count, and (hidden items) flag.
struct Works {
    item: u16,
    count: u16,
    flag: Option<u16>,
}

const BALL_WORKS: Works = Works {
    item: 0x800C,
    count: 0x800D,
    flag: None,
};
const HIDDEN_WORKS: Works = Works {
    item: 0x8000,
    count: 0x8001,
    flag: Some(0x8002),
};

/// What an item ball or hidden item gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventItem {
    /// "ball" or "hidden".
    pub kind: String,
    pub item: u16,
    pub count: u16,
    /// Hidden items: the event flag set when it is taken.
    pub flag: Option<u16>,
}

/// Where the literals sit in the script file.
struct Spots {
    file: u16,
    bytes: Vec<u8>,
    item: usize,
    count: usize,
    flag: Option<usize>,
}

fn works_of(script_id: u16) -> Option<(&'static str, Works)> {
    match script_id {
        BALL_FIRST..=BALL_LAST => Some(("ball", BALL_WORKS)),
        HIDDEN_FIRST..=HIDDEN_LAST => Some(("hidden", HIDDEN_WORKS)),
        _ => None,
    }
}

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

/// Reads the script's leading SetWork instructions, up to its first jump or
/// call, and notes where each literal sits.
fn find(bridge: &Bridge, script_id: u16, works: &Works) -> Option<Spots> {
    let (file, bytes, start) = script_code(bridge, 0, script_id)?;
    let (mut item, mut count, mut flag) = (None, None, None);
    let mut pc = start;
    // Item balls run a few set-work instructions first; a bound keeps a
    // malformed script from running on.
    for _ in 0..16 {
        let i = decode(&bytes, pc)?;
        if i.op == OP_SET_CONST || i.op == OP_SET_VALUE {
            let (var, value) = (rd16(&bytes, i.at)?, rd16(&bytes, i.at + 2)?);
            if value < LITERAL_MAX {
                if var == works.item {
                    item = Some(i.at + 2);
                } else if var == works.count {
                    count = Some(i.at + 2);
                } else if Some(var) == works.flag {
                    flag = Some(i.at + 2);
                }
            }
        } else if matches!(i.op, 4 | 27 | 28 | 29 | 30 | 2 | 5) {
            break;
        }
        pc = i.next;
    }
    Some(Spots {
        file,
        bytes,
        item: item?,
        count: count?,
        flag,
    })
}

/// The item a script id gives, when the id is in the item ball or hidden
/// item range and its script has the usual shape.
pub fn event_item(bridge: &Bridge, script_id: u16) -> Option<EventItem> {
    let (kind, works) = works_of(script_id)?;
    let s = find(bridge, script_id, &works)?;
    Some(EventItem {
        kind: kind.into(),
        item: rd16(&s.bytes, s.item)?,
        count: rd16(&s.bytes, s.count)?,
        flag: s.flag.and_then(|at| rd16(&s.bytes, at)),
    })
}

/// Makes a script id give `item` (and `count`, when given). Only those
/// literals in the script file change.
pub fn set_event_item(
    bridge: &Bridge,
    script_id: u16,
    item: u16,
    count: Option<u16>,
) -> Result<(), Error> {
    let bad = || Error::BadPath(format!("script {script_id} is not an item script"));
    let (_, works) = works_of(script_id).ok_or_else(bad)?;
    let mut s = find(bridge, script_id, &works).ok_or_else(bad)?;
    if item == 0 || item >= LITERAL_MAX || count.is_some_and(|c| c == 0 || c >= LITERAL_MAX) {
        return Err(Error::BadPath("item or count out of range".into()));
    }
    s.bytes[s.item..s.item + 2].copy_from_slice(&item.to_le_bytes());
    if let Some(c) = count {
        s.bytes[s.count..s.count + 2].copy_from_slice(&c.to_le_bytes());
    }
    catalog::write_member(
        bridge,
        Member {
            arc: SCRIPT_MEMBERS,
            index: s.file as usize,
        },
        &s.bytes,
    )
}

/// A hidden item script no event uses yet, so a new hidden item can have its
/// own pick-up flag. `None` when all of them are taken: the game's table has
/// 177 entries and all 177 are used (the table, in overlay 10, sets the flag
/// and the dowsing machine's spot).
pub fn free_hidden_script(bridge: &Bridge) -> Result<Option<u16>, Error> {
    let zones = crate::world::zones(bridge)?.len();
    let mut used = std::collections::HashSet::new();
    for zone in 0..zones {
        for (kind, _, id) in crate::scripts::event_script_ids(bridge, zone)? {
            if let (true, Some(id)) = (kind == "bg", id) {
                used.insert(id);
            }
        }
    }
    Ok((HIDDEN_FIRST..=HIDDEN_LAST)
        .find(|id| !used.contains(id) && event_item(bridge, *id).is_some()))
}
