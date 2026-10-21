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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
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
const NO_WORK: u16 = 0xC000;
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
    /// True when every way the script reaches this line goes through a work
    /// variable whose value was set after a branch, so which line shows
    /// depends on a flag, badge or choice.
    pub conditional: bool,
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
/// A work variable operand is read from the values the walk knows on the
/// path (set, copy, add, stack arithmetic); with no known value it is left
/// out.
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

/// Counts of one kind of operand over a set of scripts (see
/// `resolve_stats`): each distinct place in the code counts once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperandCount {
    pub total: usize,
    /// The operand is a literal.
    pub literal: usize,
    /// The operand is a work variable with a known value on some path.
    pub resolved: usize,
    /// A work variable whose value no path fixes.
    pub unresolved: usize,
}

/// How many operands of each kind the walk can name, over every script of
/// the ROM. "Before" the variable tracking is `literal`; "after" is
/// `literal + resolved`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveStats {
    pub scripts: usize,
    pub messages: OperandCount,
    pub trainers: OperandCount,
    pub items: OperandCount,
    pub warps: OperandCount,
}

/// The kinds of operand the summary reads, for the counts.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Cat {
    Message,
    Bank,
    Trainer,
    Item,
    Warp,
    Other,
}

/// A script operand as the walk sees it: a literal, a work variable with a
/// known value (and whether the value depends on a branch), or unknown.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Opnd {
    Lit(u16),
    Var(u16, bool),
    Unknown,
    /// 0xC000 and up names no work variable: the game reads the number
    /// itself, and scripts use 0xFFFF for "none".
    None,
}

impl Opnd {
    fn value(self) -> Option<(u16, bool)> {
        match self {
            Opnd::Lit(v) => Some((v, false)),
            Opnd::Var(v, c) => Some((v, c)),
            Opnd::Unknown | Opnd::None => None,
        }
    }

    fn rank(self) -> u8 {
        match self {
            Opnd::Unknown => 0,
            Opnd::Var(..) => 1,
            Opnd::Lit(_) | Opnd::None => 2,
        }
    }
}

/// The work variables known on the path being walked.
///
/// `vars` maps a work number to its value and a flag that is true when the
/// value was set after a conditional branch (it then depends on the
/// branch). `stack` is the VM's value stack, `None` for an unknown value.
#[derive(Clone, Default, Hash, PartialEq, Eq)]
struct State {
    vars: BTreeMap<u16, (u16, bool)>,
    stack: Vec<Option<u32>>,
    branched: bool,
}

impl State {
    fn resolve(&self, word: u16) -> Opnd {
        if word < LITERAL_LIMIT {
            Opnd::Lit(word)
        } else if word >= NO_WORK {
            Opnd::None
        } else {
            self.vars
                .get(&word)
                .map_or(Opnd::Unknown, |&(v, c)| Opnd::Var(v, c))
        }
    }

    /// Sets a work variable; `None` forgets it. A literal number names no
    /// variable (the game ignores the store).
    fn set(&mut self, var: u16, value: Option<u16>) {
        if var < LITERAL_LIMIT {
            return;
        }
        match value {
            Some(v) => {
                self.vars.insert(var, (v, self.branched));
            }
            None => {
                self.vars.remove(&var);
            }
        }
    }

    fn push(&mut self, v: Option<u32>) {
        if self.stack.len() < STACK_LIMIT {
            self.stack.push(v);
        }
    }

    fn pop(&mut self) -> Option<u32> {
        self.stack.pop().flatten()
    }

    fn clear(&mut self) {
        self.vars.clear();
        self.stack.clear();
    }

    /// Forgets what a called region may have written. A called script keeps
    /// the local works 0x8020..=0x805E: the game saves and restores them.
    fn forget(&mut self, w: &Writes, script_call: bool) {
        let keep = |v: u16| script_call && (LOCAL_FIRST..=LOCAL_LAST).contains(&v);
        if w.all {
            self.vars.retain(|&v, _| keep(v));
        } else {
            self.vars.retain(|&v, _| keep(v) || !w.vars.contains(&v));
        }
    }
}

/// The work variables a region of code may write.
#[derive(Clone, Default)]
struct Writes {
    vars: HashSet<u16>,
    /// The region could not be followed to the end.
    all: bool,
}

/// Opcodes whose operands are all read, never written: control flow,
/// stack and register work, message windows, battles, music, warps.
fn reads_only(op: u16) -> bool {
    matches!(
        op,
        0..=9
            | 11..=17
            | 20..=33
            | 35
            | 36
            | 46..=70
            | 72..=75
            | 133..=135
            | 139
            | 140
            | 142
            | 148
            | 152
            | 158..=162
            | 166..=172
            | 175
            | 179
            | 180
            | 190..=196
            | 602
    )
}

/// The work variables an opcode may write. The set-work family names the
/// variable first (`ReadWorkPointer`); for other opcodes every operand
/// word that is a variable number counts, which can only lose a value, not
/// invent one.
fn written_by(op: u16, b: &[u8], at: usize, size: usize) -> Vec<u16> {
    match op {
        10 | 18 | 19 | 38 | 39 | 40 | 41 | 42 | 43 | 44 | 45 => rd16(b, at)
            .into_iter()
            .filter(|&v| v >= LITERAL_LIMIT)
            .collect(),
        _ if reads_only(op) => Vec::new(),
        _ => (0..size / 2)
            .filter_map(|i| rd16(b, at + i * 2))
            .filter(|&v| v >= LITERAL_LIMIT)
            .collect(),
    }
}

/// The effect of a non-control opcode on the known values: the set-work
/// family, the value stack, and a forget of every other written variable.
fn step(op: u16, b: &[u8], at: usize, size: usize, st: &mut State) {
    let w0 = rd16(b, at).unwrap_or(0);
    let w1 = rd16(b, at + 2).unwrap_or(0);
    match op {
        // Set to a constant, to a value or variable, copy from a variable.
        40 => st.set(w0, Some(w1)),
        42 => {
            let v = st.resolve(w1).value().map(|x| x.0);
            st.set(w0, v);
        }
        41 => {
            let v = (w1 >= LITERAL_LIMIT)
                .then(|| st.vars.get(&w1).map(|x| x.0))
                .flatten();
            st.set(w0, v);
        }
        // var = var op value: add, sub, mul, div, mod; and, or.
        38 | 39 | 43 | 44 | 45 | 18 | 19 => {
            let cur = (w0 >= LITERAL_LIMIT)
                .then(|| st.vars.get(&w0).map(|x| x.0))
                .flatten();
            let rhs = st.resolve(w1).value().map(|x| x.0);
            let out = match (cur, rhs) {
                (Some(a), Some(c)) => match op {
                    38 => Some(a.wrapping_add(c)),
                    39 => Some(a.wrapping_sub(c)),
                    43 => Some(a.wrapping_mul(c)),
                    44 => a.checked_div(c),
                    45 => a.checked_rem(c),
                    18 => Some(a & c),
                    _ => Some(a | c),
                },
                _ => None,
            };
            st.set(w0, out);
        }
        // Stack: push constant, push value, pop to variable, discard.
        8 => st.push(Some(w0 as u32)),
        9 => {
            let v = st.resolve(w0).value().map(|x| x.0 as u32);
            st.push(v);
        }
        10 => {
            let v = st.pop().map(|x| x as u16);
            st.set(w0, v);
        }
        11 => {
            st.pop();
        }
        12..=15 => {
            let top = st.pop();
            let below = st.pop();
            let out = match (below, top) {
                (Some(a), Some(c)) => match op {
                    12 => Some(a.wrapping_add(c)),
                    13 => Some(a.wrapping_sub(c)),
                    14 => Some(a.wrapping_mul(c)),
                    _ => a.checked_div(c),
                },
                _ => None,
            };
            st.push(out);
        }
        16 => st.push(None),
        17 => {
            let top = st.pop();
            let below = st.pop();
            let out = match (below, top) {
                (Some(a), Some(c)) => match w0 {
                    0 => Some(u32::from(a < c)),
                    1 => Some(u32::from(a == c)),
                    2 => Some(u32::from(a > c)),
                    3 => Some(u32::from(a <= c)),
                    4 => Some(u32::from(a >= c)),
                    5 => Some(u32::from(a != c)),
                    6 => Some(u32::from(a == 1 || c == 1)),
                    7 => Some(u32::from(a == 1 && c == 1)),
                    _ => Some(0),
                },
                _ => None,
            };
            st.push(out);
        }
        _ => {
            for v in written_by(op, b, at, size) {
                st.vars.remove(&v);
            }
        }
    }
}

/// How many distinct states a code address may be walked in before the
/// walk gives up tracking values there, and the instructions one script
/// walk may take before it does the same everywhere.
const STATES_PER_PC: usize = 16;
const STEP_BUDGET: usize = 60_000;
const STACK_LIMIT: usize = 16;
const LOCAL_FIRST: u16 = 0x8020;
const LOCAL_LAST: u16 = 0x805E;

/// The (address, state) pairs a walk has taken, so each is walked once.
/// Past a cap the state is dropped (all values unknown), which still
/// reaches every address once: structure is never lost, only values.
#[derive(Default)]
struct Seen {
    at: HashMap<(u16, usize), Vec<u64>>,
    steps: usize,
}

impl Seen {
    fn admit(&mut self, file: u16, pc: usize, st: &mut State) -> bool {
        self.steps += 1;
        if self.steps > STEP_BUDGET {
            st.clear();
        }
        let known = self.at.entry((file, pc)).or_default();
        let hash = |st: &State| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            st.hash(&mut h);
            h.finish()
        };
        if known.len() >= STATES_PER_PC {
            st.clear();
        }
        let key = hash(st);
        if known.contains(&key) {
            return false;
        }
        known.push(key);
        true
    }
}

/// What a walk collects.
#[derive(Default)]
struct Acc {
    /// Bank, id, and whether the value depends on a branch.
    messages: Vec<(u16, u16, bool)>,
    trainers: Vec<u16>,
    items: Vec<ItemGift>,
    gifts: Vec<Gift>,
    flags_set: Vec<u16>,
    flags_cleared: Vec<u16>,
    warps: Vec<ScriptWarp>,
    music: Vec<u16>,
    calls: Vec<u16>,
    incomplete: bool,
    /// For `resolve_stats`: the best view of each operand place so far.
    sites: Option<HashMap<(Cat, u16, usize, usize), Opnd>>,
}

fn push_new<T: PartialEq>(v: &mut Vec<T>, x: T) {
    if !v.contains(&x) {
        v.push(x);
    }
}

impl Acc {
    /// Reads operand `n` of the instruction in the state, and notes the
    /// place for the counts.
    fn operand(&mut self, cat: Cat, ops: &Operands, n: usize) -> Option<(u16, bool)> {
        let o = ops.get(n);
        if let Some(sites) = &mut self.sites {
            let best = sites
                .entry((cat, ops.file, ops.pc, n))
                .or_insert(Opnd::Unknown);
            if o.rank() > best.rank() {
                *best = o;
            }
        }
        o.value()
    }

    fn message(&mut self, bank: Option<(u16, bool)>, id: Option<(u16, bool)>) {
        let (Some((bank, bc)), Some((id, ic))) = (bank, id) else {
            return;
        };
        let cond = bc || ic;
        match self.messages.iter_mut().find(|m| m.0 == bank && m.1 == id) {
            Some(m) => m.2 = m.2 && cond,
            None => self.messages.push((bank, id, cond)),
        }
    }
}

/// The operands of one instruction, read in the state of the path.
struct Operands<'a> {
    b: &'a [u8],
    at: usize,
    state: &'a State,
    file: u16,
    pc: usize,
}

impl Operands<'_> {
    /// The `n`-th byte offset's u16: a literal, or a variable's known value.
    fn get(&self, n: usize) -> Opnd {
        rd16(self.b, self.at + n).map_or(Opnd::Unknown, |w| self.state.resolve(w))
    }
}

/// An instruction's place and size.
struct Insn {
    op: u16,
    at: usize,
    size: usize,
    next: usize,
}

/// Reads the instruction at `pc`; `None` for an opcode of unknown size or one
/// that runs past the end of the file.
fn decode(b: &[u8], pc: usize) -> Option<Insn> {
    let op = rd16(b, pc)?;
    let size = OPERAND_BYTES.get(op as usize).copied().unwrap_or(UNKNOWN);
    let at = pc + 2;
    let next = at + size as usize;
    if size == UNKNOWN || next > b.len() {
        return None;
    }
    Some(Insn {
        op,
        at,
        size: size as usize,
        next,
    })
}

/// The zone's tables and the files read so far.
struct Ctx<'a> {
    bridge: &'a Bridge,
    script_file: u16,
    msg_bank: u16,
    ranges: Vec<Range>,
    files: HashMap<u16, Arc<Vec<u8>>>,
    banks: HashMap<u16, Vec<String>>,
    writes: HashMap<(u16, usize), Writes>,
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
            writes: HashMap::new(),
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

    /// The variables the code at `start` (and everything it calls) may
    /// write, found without values: every path, calls followed as jumps.
    fn writes_from(&mut self, file: u16, start: usize, depth: usize) -> Writes {
        if let Some(w) = self.writes.get(&(file, start)) {
            return w.clone();
        }
        let mut w = Writes::default();
        let Some(b) = self.file(file) else {
            w.all = true;
            return w;
        };
        let mut seen: HashSet<usize> = HashSet::new();
        let mut pending = vec![start];
        while let Some(mut pc) = pending.pop() {
            loop {
                if !seen.insert(pc) {
                    break;
                }
                let Some(i) = decode(&b, pc) else {
                    w.all = true;
                    break;
                };
                let target = |from: usize| -> Option<usize> {
                    let rel = rd32(&b, from)? as i32;
                    usize::try_from(i.next as i64 + rel as i64).ok()
                };
                match i.op {
                    OP_HALT | OP_RETURN | OP_END => break,
                    OP_JUMP | OP_CALL => {
                        match target(i.at) {
                            Some(t) => pending.push(t),
                            None => w.all = true,
                        }
                        if i.op == OP_JUMP {
                            break;
                        }
                    }
                    OP_IF_JUMP | OP_IF_CALL => match target(i.at + 1) {
                        Some(t) => pending.push(t),
                        None => w.all = true,
                    },
                    OP_CHANGE_SCRIPT | OP_CALL_SCRIPT => {
                        let sub = match rd16(&b, i.at) {
                            Some(id) if depth < MAX_DEPTH => self.script_writes(id, depth + 1),
                            _ => Writes {
                                all: true,
                                ..Writes::default()
                            },
                        };
                        // A called script restores the local works.
                        let restore = i.op == OP_CALL_SCRIPT;
                        let keep = |v: u16| restore && (LOCAL_FIRST..=LOCAL_LAST).contains(&v);
                        w.all |= sub.all;
                        w.vars.extend(sub.vars.into_iter().filter(|&v| !keep(v)));
                        if i.op == OP_CHANGE_SCRIPT {
                            break;
                        }
                    }
                    op => w.vars.extend(written_by(op, &b, i.at, i.size)),
                }
                pc = i.next;
            }
        }
        self.writes.insert((file, start), w.clone());
        w
    }

    fn script_writes(&mut self, id: u16, depth: usize) -> Writes {
        let unknown = Writes {
            all: true,
            ..Writes::default()
        };
        let Some(loc) = self.locate(id) else {
            return unknown;
        };
        match self.entry(loc.file, loc.index) {
            Some(start) => self.writes_from(loc.file, start, depth),
            None => unknown,
        }
    }

    fn walk(
        &mut self,
        loc: &Loc,
        start: usize,
        depth: usize,
        acc: &mut Acc,
        seen: &mut Seen,
        init: State,
    ) {
        let Some(b) = self.file(loc.file) else {
            acc.incomplete = true;
            return;
        };
        let mut pending = vec![(start, init)];
        while let Some((mut pc, mut st)) = pending.pop() {
            loop {
                if !seen.admit(loc.file, pc, &mut st) {
                    break;
                }
                let Some(i) = decode(&b, pc) else {
                    acc.incomplete = true;
                    break;
                };
                let target = |from: usize| -> Option<usize> {
                    let rel = rd32(&b, from)? as i32;
                    usize::try_from(i.next as i64 + rel as i64).ok()
                };
                match i.op {
                    OP_HALT | OP_RETURN | OP_END => break,
                    OP_JUMP => {
                        match target(i.at) {
                            Some(t) => pending.push((t, st.clone())),
                            None => acc.incomplete = true,
                        }
                        break;
                    }
                    OP_CALL => match target(i.at) {
                        Some(t) => {
                            pending.push((t, st.clone()));
                            let w = self.writes_from(loc.file, t, depth);
                            st.forget(&w, false);
                        }
                        None => acc.incomplete = true,
                    },
                    OP_IF_JUMP | OP_IF_CALL => {
                        // Condition 0xFF takes its answer from the stack.
                        if i.op == OP_IF_JUMP && b.get(i.at) == Some(&0xFF) {
                            st.pop();
                        }
                        match target(i.at + 1) {
                            Some(t) => {
                                let mut taken = st.clone();
                                taken.branched = true;
                                pending.push((t, taken));
                                if i.op == OP_IF_CALL {
                                    let w = self.writes_from(loc.file, t, depth);
                                    st.forget(&w, false);
                                }
                            }
                            None => acc.incomplete = true,
                        }
                        st.branched = true;
                    }
                    OP_CHANGE_SCRIPT | OP_CALL_SCRIPT => {
                        if let Some(id) = rd16(&b, i.at).filter(|&id| id < LITERAL_LIMIT) {
                            push_new(&mut acc.calls, id);
                            self.follow(id, depth, acc, seen, st.clone());
                            if i.op == OP_CALL_SCRIPT {
                                let w = self.script_writes(id, depth + 1);
                                st.forget(&w, true);
                            }
                        } else if i.op == OP_CALL_SCRIPT {
                            st.forget(
                                &Writes {
                                    all: true,
                                    ..Writes::default()
                                },
                                true,
                            );
                        }
                        if i.op == OP_CHANGE_SCRIPT {
                            break;
                        }
                    }
                    op => {
                        let ops = Operands {
                            b: &b,
                            at: i.at,
                            state: &st,
                            file: loc.file,
                            pc,
                        };
                        effect(op, loc.bank, &ops, acc);
                        step(op, &b, i.at, i.size, &mut st);
                    }
                }
                pc = i.next;
            }
        }
    }

    /// Walks a called script, within the depth limit.
    fn follow(&mut self, id: u16, depth: usize, acc: &mut Acc, seen: &mut Seen, st: State) {
        if depth >= MAX_DEPTH {
            return;
        }
        let Some(loc) = self.locate(id) else { return };
        match self.entry(loc.file, loc.index) {
            Some(start) => self.walk(&loc, start, depth + 1, acc, seen, st),
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
        let mut seen = Seen::default();
        self.walk(&loc, start, 0, &mut acc, &mut seen, State::default());
        let messages = acc
            .messages
            .iter()
            .map(|&(bank, id, conditional)| MessageRef {
                bank,
                id,
                text: self.text(bank, id),
                conditional,
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
/// the order the handler reads them. An operand that is a work variable is
/// read from the state of the path.
fn effect(op: u16, zone_bank: u16, ops: &Operands, acc: &mut Acc) {
    // A bank operand: 0x400 is the zone's bank.
    let bank = |acc: &mut Acc, n: usize| {
        acc.operand(Cat::Bank, ops, n)
            .map(|(b, c)| (if b == ZONE_BANK_MARK { zone_bank } else { b }, c))
    };
    match op {
        // Info, plain, sub, background and special windows: the message id
        // comes first, from the zone's bank.
        52 | 53 | 56 | 58 | 67 | 69 | 74 => {
            let id = acc.operand(Cat::Message, ops, 0);
            acc.message(Some((zone_bank, false)), id);
        }
        // Menu item: text, then help text.
        175 => {
            let id = acc.operand(Cat::Message, ops, 0);
            acc.message(Some((zone_bank, false)), id);
            let id = acc.operand(Cat::Message, ops, 2);
            acc.message(Some((zone_bank, false)), id);
        }
        // Balloon window: file, message.
        60 | 61 => {
            let b = bank(acc, 0);
            let id = acc.operand(Cat::Message, ops, 2);
            acc.message(b, id);
        }
        72 => {
            let b = bank(acc, 0);
            let id = acc.operand(Cat::Message, ops, 2);
            acc.message(b, id);
            let id = acc.operand(Cat::Message, ops, 4);
            acc.message(b, id);
        }
        // The second argument is skipped in the Black build.
        73 => {
            let b = bank(acc, 0);
            let id = acc.operand(Cat::Message, ops, 4);
            acc.message(b, id);
        }
        // Trainer battles: single, multi (ally first), with a Pokemon.
        133 | 134 | 148 => {
            let first = usize::from(op == 134);
            let n = if op == 148 { 4 } else { 3 };
            for i in first..first + n {
                if let Some((t, _)) = acc.operand(Cat::Trainer, ops, i * 2).filter(|v| v.0 != 0) {
                    push_new(&mut acc.trainers, t);
                }
            }
        }
        // Item, count, result variable.
        181 => {
            if let Some((item, _)) = acc.operand(Cat::Item, ops, 0) {
                let count = acc.operand(Cat::Other, ops, 2).map_or(1, |v| v.0);
                push_new(&mut acc.items, ItemGift { item, count });
            }
        }
        // Party Pokemon: result variable, species, form, level.
        268 => {
            if let (Some((species, _)), Some((level, _))) = (
                acc.operand(Cat::Other, ops, 2),
                acc.operand(Cat::Other, ops, 6),
            ) {
                push_new(&mut acc.gifts, Gift { species, level });
            }
        }
        OP_SET_FLAG => {
            if let Some((f, _)) = acc.operand(Cat::Other, ops, 0) {
                push_new(&mut acc.flags_set, f);
            }
        }
        OP_RESET_FLAG => {
            if let Some((f, _)) = acc.operand(Cat::Other, ops, 0) {
                push_new(&mut acc.flags_cleared, f);
            }
        }
        // Warps: zone, then x, z (190, 191, 193, 194), a rail position
        // (192, 602) or x, y, z (196).
        190 | 191 | 193 | 194 | 196 => {
            if let Some((zone, _)) = acc.operand(Cat::Warp, ops, 0) {
                let z_at = if op == 196 { 6 } else { 4 };
                let w = ScriptWarp {
                    zone,
                    x: acc.operand(Cat::Other, ops, 2).map(|v| v.0),
                    z: acc.operand(Cat::Other, ops, z_at).map(|v| v.0),
                };
                push_new(&mut acc.warps, w);
            }
        }
        192 | 602 => {
            if let Some((zone, _)) = acc.operand(Cat::Warp, ops, 0) {
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
            if let Some((m, _)) = acc.operand(Cat::Other, ops, 0) {
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

/// Counts the operands of the summary kinds over every script of the ROM
/// (each script of each member of a/0/5/7, walked from its entry), each
/// distinct place in the code once. `literal` is what the walk named before
/// it tracked work variables; `literal + resolved` is what it names now.
pub fn resolve_stats(bridge: &Bridge) -> Result<ResolveStats, Error> {
    let mut ctx = Ctx::new(bridge, 0)?;
    let files = catalog::members(bridge, SCRIPT_ARCHIVE)?.len();
    let mut stats = ResolveStats::default();
    let mut sites = HashMap::new();
    for file in 0..files as u16 {
        for index in 0..ctx.count(file) {
            let Some(start) = ctx.entry(file, index) else {
                continue;
            };
            stats.scripts += 1;
            let loc = Loc {
                file,
                bank: 0,
                index,
            };
            let mut acc = Acc {
                sites: Some(std::mem::take(&mut sites)),
                ..Acc::default()
            };
            let mut seen = Seen::default();
            ctx.walk(&loc, start, 0, &mut acc, &mut seen, State::default());
            sites = acc.sites.take().unwrap_or_default();
        }
    }
    for ((cat, ..), operand) in sites {
        let count = match cat {
            Cat::Message => &mut stats.messages,
            Cat::Trainer => &mut stats.trainers,
            Cat::Item => &mut stats.items,
            Cat::Warp => &mut stats.warps,
            Cat::Bank | Cat::Other => continue,
        };
        count.total += 1;
        match operand {
            Opnd::Lit(_) => count.literal += 1,
            Opnd::Var(..) => count.resolved += 1,
            Opnd::Unknown => count.unresolved += 1,
            Opnd::None => count.total -= 1,
        }
    }
    Ok(stats)
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
