//! The field scripts of a zone: which script file and message bank a zone
//! uses, and what each script says and does, found by walking its
//! bytecode. Layouts from docs/game-data-map.md ("Field scripts").
//!
//! ```text
//! zone header   a/0/1/2: u16 scriptArcId at 0x06 (member of a/0/5/7),
//!               u16 msgArcId at 0x0A (member of a/0/0/3)
//! script file   a/0/5/7 member: u32 offsets, one per script, each counted
//!               from the end of its own field; then u16 0xFD13; then code
//! code          u16 opcode (0..608), then operands; a u16 operand under
//!               0x4000 is a literal, above it a variable (work) number
//! script id     the ranges table of overlay 10 (46 rows of five u16: first
//!               id, last id, file, message archive 3 bank, ...) names the
//!               shared scripts, tried from the highest row down; below
//!               them id N is local script N - 1 of the zone's file
//! map scripts   the event file trailer: 6-byte rows (key, script id) up
//!               to a zero key; key 2 zone change, 3 recover, 4 field init,
//!               key 1 a scene list (see `map_scripts`)
//! ```
//!
//! Sources (aletheia): ov010/script_sys.c SCRIPT_SYS_GetScriptFileId,
//! SCRIPT_SYS_CreateVm, SCRIPT_SYS_FindSpecialScript, FindSceneScript;
//! ov010/script_work_cmd.c (jumps); ov021/scrcmd_*.c (operands).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::catalog;
use crate::code::Image;
use crate::tables;

const SCRIPT_ARCHIVE: u16 = 57;
const STORY_TEXT: u16 = 3;
const ZONE_HEADERS: u16 = 12;
const EVENT_FILES: u16 = 125;
const ZONE_RECORD: usize = 48;
const HEADER_END: u16 = 0xFD13;
const RANGE_TABLE: u32 = 0x0217_01A8;
const RANGE_ROWS: usize = 46;
const RANGE_ROW: usize = 10;
const ZONE_BANK_MARK: u16 = 0x400;
const LITERAL_LIMIT: u16 = 0x4000;
const UNKNOWN: u8 = 255;
const MAX_DEPTH: usize = 4;

const OP_HALT: u16 = 2;
const OP_CALL: u16 = 4;
const OP_RETURN: u16 = 5;
const OP_CHANGE_SCRIPT: u16 = 27;
const OP_CALL_SCRIPT: u16 = 28;
const OP_END: u16 = 29;
const OP_JUMP: u16 = 30;
const OP_IF_JUMP: u16 = 31;
const OP_IF_CALL: u16 = 32;
const OP_SET_FLAG: u16 = 35;
const OP_RESET_FLAG: u16 = 36;

/// Operand bytes of each opcode, read from the handlers in aletheia
/// (ov010 script_work*.c, ov021 scrcmd_*.c) and checked by walking all
/// scripts of the Black ROM (every reachable path ends cleanly). 255 marks
/// the nine opcodes with no handler source; 58 others never occur in the
/// data, so their sizes come from the source only.
static OPERAND_BYTES: [u8; 609] = [
    0, 0, 0, 2, 4, 0, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 2, 2, 4, 4, 2, 5, 2, 2, 2, 4, 4, 2, 2, 0, 4, 5,
    5, 2, 2, 2, 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 0, 0, 0, 0, 2, 4, 4, 0, 2, 3, 0, 8, 2, 10, 8, 0,
    0, 4, 0, 0, 4, 0, 4, 0, 2, 12, 12, 3, 0, 1, 3, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 5, 4,
    3, 3, 3, 3, 3, 3, 6, 0, 4, 6, 4, 12, 4, 2, 2, 10, 2, 4, 10, 6, 8, 4, 0, 2, 8, 0, 2, 6, 2, 8, 6,
    8, 2, 4, 2, 0, 4, 2, 2, 6, 8, 6, 6, 255, 4, 2, 0, 2, 0, 255, 255, 255, 4, 4, 8, 2, 2, 4, 2,
    255, 255, 4, 255, 255, 0, 2, 0, 2, 4, 2, 2, 4, 2, 0, 0, 2, 0, 4, 0, 6, 6, 6, 0, 0, 7, 8, 0, 6,
    6, 6, 6, 4, 4, 4, 2, 4, 8, 8, 10, 6, 8, 0, 10, 0, 2, 0, 2, 0, 2, 4, 2, 2, 2, 2, 4, 4, 2, 2, 4,
    4, 2, 2, 4, 2, 6, 0, 10, 4, 4, 6, 2, 2, 2, 0, 2, 4, 4, 2, 4, 2, 2, 2, 0, 0, 2, 2, 2, 2, 4, 4,
    4, 4, 4, 2, 4, 2, 2, 4, 4, 6, 4, 4, 2, 4, 4, 4, 0, 6, 2, 6, 4, 8, 6, 6, 8, 4, 18, 6, 6, 6, 4,
    4, 4, 6, 4, 4, 6, 6, 8, 6, 6, 2, 2, 4, 4, 4, 8, 18, 4, 0, 8, 8, 2, 4, 2, 6, 2, 8, 6, 2, 0, 0,
    2, 2, 0, 0, 4, 2, 6, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 22, 2, 0, 4, 2, 16, 4, 0, 0, 0, 4, 6, 4, 2,
    4, 0, 2, 4, 2, 2, 0, 4, 0, 8, 2, 0, 2, 0, 2, 2, 0, 0, 0, 4, 2, 0, 3, 2, 5, 5, 8, 2, 2, 4, 0, 2,
    0, 0, 2, 2, 2, 2, 0, 6, 0, 0, 2, 2, 0, 2, 4, 2, 2, 2, 0, 2, 2, 2, 2, 2, 2, 0, 0, 2, 2, 2, 16,
    2, 2, 2, 2, 2, 2, 0, 4, 2, 2, 4, 2, 2, 0, 2, 2, 2, 2, 2, 2, 2, 0, 0, 4, 4, 8, 8, 2, 0, 0, 0, 0,
    0, 0, 0, 255, 0, 0, 0, 0, 0, 4, 4, 0, 2, 4, 4, 6, 0, 3, 0, 0, 4, 2, 6, 0, 0, 4, 0, 4, 0, 2, 4,
    6, 0, 2, 0, 6, 0, 0, 2, 2, 4, 2, 4, 2, 4, 4, 4, 6, 8, 2, 2, 2, 4, 6, 6, 6, 4, 4, 8, 8, 10, 8,
    4, 4, 4, 4, 0, 8, 0, 0, 6, 12, 0, 0, 0, 2, 6, 4, 4, 0, 2, 2, 2, 2, 4, 2, 2, 4, 2, 8, 8, 4, 8,
    8, 4, 6, 8, 0, 2, 6, 4, 4, 2, 2, 2, 2, 1, 1, 4, 0, 2, 0, 8, 12, 8, 2, 2, 6, 2, 8, 4, 4, 2, 4,
    4, 4, 0, 4, 2, 2, 4, 4, 4, 2, 4, 6, 2, 2, 4, 4, 0, 4, 4, 0, 2, 2, 4, 4, 2, 4, 2, 8, 4, 8, 4, 2,
    2, 4, 0, 0, 0, 6, 0, 4, 2, 12, 6, 0, 0, 2, 10, 8, 12, 2, 0, 0, 0,
];

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

/// A message a script shows: bank `bank` of the story text archive
/// (a/0/0/3) and the message index in it, with its text in the display form
/// of `msg::Display`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    pub bank: u16,
    pub id: u16,
    pub text: String,
}

/// A warp a script starts. `x` and `z` are tiles; absent for a rail warp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptWarp {
    pub zone: u16,
    pub x: Option<u16>,
    pub z: Option<u16>,
}

/// A Pokemon a script gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gift {
    pub species: u16,
    pub level: u16,
}

/// An item a script gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemGift {
    pub item: u16,
    pub count: u16,
}

/// What one script says and does, found by walking its code (jumps and
/// calls inside the file, and the scripts it calls, to a depth of four).
/// Operands that are variables, not literals, are left out.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInfo {
    /// The script id events use.
    pub id: u16,
    /// Member of a/0/5/7, and the script's index in it.
    pub file: u16,
    pub index: usize,
    /// The message bank the script's own text uses (a/0/0/3).
    pub text_bank: u16,
    pub messages: Vec<MessageRef>,
    /// Trainer ids of battles it starts.
    pub trainers: Vec<u16>,
    pub items: Vec<ItemGift>,
    pub gifts: Vec<Gift>,
    pub flags_set: Vec<u16>,
    pub flags_cleared: Vec<u16>,
    pub warps: Vec<ScriptWarp>,
    /// BGM sequence numbers it starts.
    pub music: Vec<u16>,
    /// Ids of the scripts it calls or switches to.
    pub calls: Vec<u16>,
    /// False when the walk met an opcode of unknown size or left the file.
    pub complete: bool,
}

/// A script the zone runs by itself: on entering, on recovery, at field
/// start, or by scene (a script chosen by a work value).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapScript {
    /// "zoneChange", "recover", "fieldInit" or "scene".
    pub kind: String,
    pub script_id: u16,
    /// For a scene: the work number tested and the value it must hold.
    pub work: Option<u16>,
    pub value: Option<u16>,
    pub script: Option<ScriptInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneScripts {
    pub zone: usize,
    /// Member of a/0/5/7 holding the zone's local scripts.
    pub script_file: u16,
    /// Bank of a/0/0/3 holding the zone's dialogue.
    pub text_bank: u16,
    /// Local scripts have ids 1..=count.
    pub scripts: Vec<ScriptInfo>,
    pub map_scripts: Vec<MapScript>,
}

/// The script one event runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventScript {
    pub kind: String,
    pub index: usize,
    /// The id the event record holds (NPC `eventId`, trigger or sign
    /// `scriptId`); none for kinds that run no script.
    pub script_id: Option<u16>,
    pub script: Option<ScriptInfo>,
}

struct Range {
    first: u16,
    last: u16,
    file: u16,
    bank: u16,
}

struct Loc {
    file: u16,
    bank: u16,
    index: usize,
}

#[derive(Default)]
struct Acc {
    messages: Vec<(u16, u16)>,
    trainers: Vec<u16>,
    items: Vec<ItemGift>,
    gifts: Vec<Gift>,
    flags_set: Vec<u16>,
    flags_cleared: Vec<u16>,
    warps: Vec<ScriptWarp>,
    music: Vec<u16>,
    calls: Vec<u16>,
    incomplete: bool,
}

fn push_new<T: PartialEq>(v: &mut Vec<T>, x: T) {
    if !v.contains(&x) {
        v.push(x);
    }
}

type Seen = HashSet<(u16, usize)>;

/// The zone's tables and the files read so far.
struct Ctx<'a> {
    bridge: &'a Bridge,
    script_file: u16,
    msg_bank: u16,
    ranges: Vec<Range>,
    files: HashMap<u16, Arc<Vec<u8>>>,
    banks: HashMap<u16, Vec<String>>,
}

fn literal(v: Option<u16>) -> Option<u16> {
    v.filter(|&x| x < LITERAL_LIMIT)
}

impl<'a> Ctx<'a> {
    fn new(bridge: &'a Bridge, zone: usize) -> Result<Self, Error> {
        let headers = bridge.member(Member {
            arc: ZONE_HEADERS,
            index: 0,
        })?;
        let r = headers
            .get(zone * ZONE_RECORD..(zone + 1) * ZONE_RECORD)
            .ok_or_else(|| Error::Missing(format!("zone {zone}")))?;
        Ok(Ctx {
            bridge,
            script_file: rd16(r, 0x06).unwrap_or(0),
            msg_bank: rd16(r, 0x0A).unwrap_or(0),
            ranges: ranges(bridge),
            files: HashMap::new(),
            banks: HashMap::new(),
        })
    }

    fn file(&mut self, id: u16) -> Option<Arc<Vec<u8>>> {
        if let Some(f) = self.files.get(&id) {
            return Some(f.clone());
        }
        let bytes = self
            .bridge
            .member(Member {
                arc: SCRIPT_ARCHIVE,
                index: id as usize,
            })
            .ok()?;
        let f = Arc::new(bytes);
        self.files.insert(id, f.clone());
        Some(f)
    }

    /// Where a script id lives (SCRIPT_SYS_GetScriptFileId).
    fn locate(&self, id: u16) -> Option<Loc> {
        for r in &self.ranges {
            if id < r.first {
                continue;
            }
            if id > r.last {
                return None;
            }
            return Some(Loc {
                file: r.file,
                bank: r.bank,
                index: (id - r.first) as usize,
            });
        }
        (id >= 1).then(|| Loc {
            file: self.script_file,
            bank: self.msg_bank,
            index: id as usize - 1,
        })
    }

    fn text(&mut self, bank: u16, id: u16) -> String {
        let bridge = self.bridge;
        self.banks
            .entry(bank)
            .or_insert_with(|| {
                catalog::text_bank(bridge, STORY_TEXT, bank as usize).unwrap_or_default()
            })
            .get(id as usize)
            .cloned()
            .unwrap_or_default()
    }

    fn walk(&mut self, loc: &Loc, start: usize, depth: usize, acc: &mut Acc, seen: &mut Seen) {
        let Some(b) = self.file(loc.file) else {
            acc.incomplete = true;
            return;
        };
        let mut pending = vec![start];
        while let Some(mut pc) = pending.pop() {
            loop {
                if !seen.insert((loc.file, pc)) {
                    break;
                }
                let Some(op) = rd16(&b, pc) else {
                    acc.incomplete = true;
                    break;
                };
                let size = OPERAND_BYTES.get(op as usize).copied().unwrap_or(UNKNOWN);
                let at = pc + 2;
                let next = at + size as usize;
                if size == UNKNOWN || next > b.len() {
                    acc.incomplete = true;
                    break;
                }
                let arg = |n: usize| literal(rd16(&b, at + n));
                let target = |from: usize| -> Option<usize> {
                    let rel = rd32(&b, from)? as i32;
                    usize::try_from(next as i64 + rel as i64).ok()
                };
                match op {
                    OP_HALT | OP_RETURN | OP_END => break,
                    OP_JUMP => {
                        match target(at) {
                            Some(t) => pending.push(t),
                            None => acc.incomplete = true,
                        }
                        break;
                    }
                    OP_CALL => match target(at) {
                        Some(t) => pending.push(t),
                        None => acc.incomplete = true,
                    },
                    OP_IF_JUMP | OP_IF_CALL => match target(at + 1) {
                        Some(t) => pending.push(t),
                        None => acc.incomplete = true,
                    },
                    OP_CHANGE_SCRIPT | OP_CALL_SCRIPT => {
                        if let Some(id) = arg(0) {
                            push_new(&mut acc.calls, id);
                            self.follow(id, depth, acc, seen);
                        }
                        if op == OP_CHANGE_SCRIPT {
                            break;
                        }
                    }
                    _ => effect(op, loc.bank, &arg, acc),
                }
                pc = next;
            }
        }
    }

    /// Walks a called script, within the depth limit.
    fn follow(&mut self, id: u16, depth: usize, acc: &mut Acc, seen: &mut Seen) {
        if depth >= MAX_DEPTH {
            return;
        }
        let Some(loc) = self.locate(id) else { return };
        match self.entry(loc.file, loc.index) {
            Some(start) => self.walk(&loc, start, depth + 1, acc, seen),
            None => acc.incomplete = true,
        }
    }

    /// The code offset of script `index` of a file.
    fn entry(&mut self, file: u16, index: usize) -> Option<usize> {
        let b = self.file(file)?;
        let mut p = 0;
        let mut n = 0;
        while rd16(&b, p)? != HEADER_END {
            if n == index {
                return usize::try_from(p as i64 + 4 + rd32(&b, p)? as i64).ok();
            }
            p += 4;
            n += 1;
        }
        None
    }

    fn count(&mut self, file: u16) -> usize {
        let Some(b) = self.file(file) else { return 0 };
        let mut p = 0;
        while rd16(&b, p).is_some_and(|v| v != HEADER_END) {
            p += 4;
        }
        if rd16(&b, p).is_some() {
            p / 4
        } else {
            0
        }
    }

    fn info(&mut self, id: u16) -> Option<ScriptInfo> {
        let loc = self.locate(id)?;
        let start = self.entry(loc.file, loc.index)?;
        let mut acc = Acc::default();
        let mut seen = Seen::new();
        self.walk(&loc, start, 0, &mut acc, &mut seen);
        let messages = acc
            .messages
            .iter()
            .map(|&(bank, id)| MessageRef {
                bank,
                id,
                text: self.text(bank, id),
            })
            .collect();
        Some(ScriptInfo {
            id,
            file: loc.file,
            index: loc.index,
            text_bank: loc.bank,
            messages,
            trainers: acc.trainers,
            items: acc.items,
            gifts: acc.gifts,
            flags_set: acc.flags_set,
            flags_cleared: acc.flags_cleared,
            warps: acc.warps,
            music: acc.music,
            calls: acc.calls,
            complete: !acc.incomplete,
        })
    }
}

/// What one opcode with operands means for the summary. Argument offsets
/// follow the handlers: ReadWorkValue and VM_ReadU16 are two bytes each, in
/// the order the handler reads them.
fn effect(op: u16, zone_bank: u16, arg: &dyn Fn(usize) -> Option<u16>, acc: &mut Acc) {
    let bank_of = |a: Option<u16>| match a {
        Some(ZONE_BANK_MARK) | None => zone_bank,
        Some(b) => b,
    };
    let mut msg = |bank: u16, id: Option<u16>| {
        if let Some(id) = id {
            push_new(&mut acc.messages, (bank, id));
        }
    };
    match op {
        // Info, plain, sub, background and special windows: the message id
        // comes first, from the zone's bank.
        52 | 53 | 56 | 58 | 67 | 69 | 74 => msg(zone_bank, arg(0)),
        // Menu item: text, then help text.
        175 => {
            msg(zone_bank, arg(0));
            msg(zone_bank, arg(2));
        }
        // Balloon window: file, message.
        60 | 61 => msg(bank_of(arg(0)), arg(2)),
        72 => {
            let bank = bank_of(arg(0));
            msg(bank, arg(2));
            msg(bank, arg(4));
        }
        // The second argument is skipped in the Black build.
        73 => msg(bank_of(arg(0)), arg(4)),
        // Trainer battles: single, multi (ally first), with a Pokemon.
        133 | 134 | 148 => {
            let first = usize::from(op == 134);
            let n = if op == 148 { 4 } else { 3 };
            for i in first..first + n {
                if let Some(t) = arg(i * 2).filter(|&t| t != 0) {
                    push_new(&mut acc.trainers, t);
                }
            }
        }
        // Item, count, result variable.
        181 => {
            if let Some(item) = arg(0) {
                let count = arg(2).unwrap_or(1);
                push_new(&mut acc.items, ItemGift { item, count });
            }
        }
        // Party Pokemon: result variable, species, form, level.
        268 => {
            if let (Some(species), Some(level)) = (arg(2), arg(6)) {
                push_new(&mut acc.gifts, Gift { species, level });
            }
        }
        OP_SET_FLAG => {
            if let Some(f) = arg(0) {
                push_new(&mut acc.flags_set, f);
            }
        }
        OP_RESET_FLAG => {
            if let Some(f) = arg(0) {
                push_new(&mut acc.flags_cleared, f);
            }
        }
        // Warps: zone, then x, z (190, 191, 193, 194), a rail position
        // (192, 602) or x, y, z (196).
        190 | 191 | 193 | 194 | 196 => {
            if let Some(zone) = arg(0) {
                let z_at = if op == 196 { 6 } else { 4 };
                let w = ScriptWarp {
                    zone,
                    x: arg(2),
                    z: arg(z_at),
                };
                push_new(&mut acc.warps, w);
            }
        }
        192 | 602 => {
            if let Some(zone) = arg(0) {
                let w = ScriptWarp {
                    zone,
                    x: None,
                    z: None,
                };
                push_new(&mut acc.warps, w);
            }
        }
        // BGM: fade out and play, push and play (wait), push and play.
        152 | 159 | 169 => {
            if let Some(m) = arg(0) {
                push_new(&mut acc.music, m);
            }
        }
        _ => {}
    }
}

/// The shared-script ranges from overlay 10 (Black addresses). Empty when
/// the image is missing or the table does not look right, so only local
/// scripts resolve.
fn ranges(bridge: &Bridge) -> Vec<Range> {
    let Some(raw) = bridge
        .code()
        .and_then(|c| c.read(Image::Overlay(10), RANGE_TABLE, RANGE_ROWS * RANGE_ROW))
    else {
        return Vec::new();
    };
    let rows: Vec<Range> = raw
        .chunks(RANGE_ROW)
        .map(|r| Range {
            first: rd16(r, 0).unwrap_or(0),
            last: rd16(r, 2).unwrap_or(0),
            file: rd16(r, 4).unwrap_or(0),
            bank: rd16(r, 8).unwrap_or(0),
        })
        .collect();
    let sane = rows.iter().all(|r| r.first <= r.last && r.file < 1000)
        && rows.windows(2).all(|w| w[0].first > w[1].last);
    if sane {
        rows
    } else {
        Vec::new()
    }
}

/// The zone's script file and dialogue bank, its local scripts and its
/// The dialogue bank (a/0/0/3) a zone's scripts speak from.
pub fn zone_text_bank(bridge: &Bridge, zone: usize) -> Option<u16> {
    let headers = bridge
        .member(Member {
            arc: ZONE_HEADERS,
            index: 0,
        })
        .ok()?;
    rd16(
        headers.get(zone * ZONE_RECORD..(zone + 1) * ZONE_RECORD)?,
        0x0A,
    )
}

/// map scripts.
pub fn zone_scripts(bridge: &Bridge, zone: usize) -> Result<ZoneScripts, Error> {
    let mut ctx = Ctx::new(bridge, zone)?;
    let count = ctx.count(ctx.script_file);
    let scripts = (1..=count as u16).filter_map(|id| ctx.info(id)).collect();
    let map_scripts = map_scripts(&mut ctx, bridge, zone)?;
    Ok(ZoneScripts {
        zone,
        script_file: ctx.script_file,
        text_bank: ctx.msg_bank,
        scripts,
        map_scripts,
    })
}

/// One script by id, as the zone resolves it (shared ids included).
pub fn script_info(
    bridge: &Bridge,
    zone: usize,
    script_id: u16,
) -> Result<Option<ScriptInfo>, Error> {
    Ok(Ctx::new(bridge, zone)?.info(script_id))
}

fn event_file(bridge: &Bridge, zone: usize) -> Result<Vec<u8>, Error> {
    let zones = crate::world::zones(bridge)?;
    let z = zones
        .get(zone)
        .ok_or_else(|| Error::Missing(format!("zone {zone}")))?;
    bridge.member(Member {
        arc: EVENT_FILES,
        index: z.event_data as usize,
    })
}

/// The script id field of each event of a zone, in file order.
fn event_ids(file: &[u8]) -> Vec<(String, usize, Option<u16>)> {
    let arrays = tables::table("eventData")
        .ok()
        .and_then(|t| t.arrays.as_deref())
        .unwrap_or(&[]);
    let mut out = Vec::new();
    let mut at = 8;
    for (i, a) in arrays.iter().enumerate() {
        let count = file.get(4 + i).copied().unwrap_or(0) as usize;
        let name = match a.name.as_str() {
            "npc" => Some("eventId"),
            "pos" | "bg" => Some("scriptId"),
            _ => None,
        };
        let field = name.and_then(|n| a.fields.iter().find(|f| f.name == n));
        for index in 0..count {
            let base = at + index * a.record_size;
            let id = field
                .and_then(|f| tables::read_field(file, base, f))
                .map(|v| v as u16);
            out.push((a.name.clone(), index, id));
        }
        at += count * a.record_size;
    }
    out
}

fn trainer_of(event_id: u16) -> Option<u16> {
    match event_id {
        3000..=4999 => Some(event_id - 3000),
        5000..=6999 => Some(event_id - 5000),
        _ => None,
    }
}

fn resolve_event(ctx: &mut Ctx, kind: &str, id: Option<u16>) -> Option<ScriptInfo> {
    let id = id?;
    let mut info = ctx.info(id)?;
    if kind == "npc" {
        if let Some(t) = trainer_of(id) {
            push_new(&mut info.trainers, t);
        }
    }
    Some(info)
}

/// The script an event runs, resolved through its script id. `kind` is
/// "npc", "pos" or "bg" as in `zonemap::Event`; `None` for other kinds, a
/// missing event, or an id that names no script.
pub fn event_script(
    bridge: &Bridge,
    zone: usize,
    kind: &str,
    index: usize,
) -> Result<Option<ScriptInfo>, Error> {
    let file = event_file(bridge, zone)?;
    let Some((_, _, id)) = event_ids(&file)
        .into_iter()
        .find(|(k, i, _)| k == kind && *i == index)
    else {
        return Ok(None);
    };
    let mut ctx = Ctx::new(bridge, zone)?;
    Ok(resolve_event(&mut ctx, kind, id))
}

/// The scripts of all events of a zone (people, signs, triggers; warps
/// carry no script).
pub fn event_scripts(bridge: &Bridge, zone: usize) -> Result<Vec<EventScript>, Error> {
    let file = event_file(bridge, zone)?;
    let mut ctx = Ctx::new(bridge, zone)?;
    Ok(event_ids(&file)
        .into_iter()
        .map(|(kind, index, script_id)| {
            let script = resolve_event(&mut ctx, &kind, script_id);
            EventScript {
                kind,
                index,
                script_id,
                script,
            }
        })
        .collect())
}

/// The zone's own scripts from the event file trailer
/// (SCRIPT_SYS_FindSpecialScript, SCRIPT_SYS_FindSceneScript).
fn map_scripts(ctx: &mut Ctx, bridge: &Bridge, zone: usize) -> Result<Vec<MapScript>, Error> {
    let file = event_file(bridge, zone)?;
    let mut out = Vec::new();
    let Some(mut p) = rd32(&file, 0).map(|t| t as usize + 4) else {
        return Ok(out);
    };
    while let Some(key) = rd16(&file, p).filter(|&k| k != 0) {
        let kind = match key {
            2 => Some("zoneChange"),
            3 => Some("recover"),
            4 => Some("fieldInit"),
            _ => None,
        };
        if let (Some(kind), Some(id)) = (kind, rd16(&file, p + 2)) {
            out.push(MapScript {
                kind: kind.into(),
                script_id: id,
                work: None,
                value: None,
                script: ctx.info(id),
            });
        } else if key == 1 {
            // A scene list: rows of (work, value, script id), starting
            // after this row at the offset in the u32 following the key.
            let mut q = p + 6 + rd32(&file, p + 2).unwrap_or(0) as usize;
            while let Some(work) = rd16(&file, q).filter(|&w| w != 0) {
                if let Some(id) = rd16(&file, q + 4) {
                    out.push(MapScript {
                        kind: "scene".into(),
                        script_id: id,
                        work: Some(work),
                        value: rd16(&file, q + 2),
                        script: ctx.info(id),
                    });
                }
                q += 6;
            }
        }
        p += 6;
    }
    Ok(out)
}
