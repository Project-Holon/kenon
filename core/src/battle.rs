//! Battle facts read from the game: the type chart and the coded effects
//! (overlay 93), and who uses a move, an ability or a type.
//!
//! Coded effects. A move, an ability or a held item does what its data
//! says (power, status and chance, stat changes...), and some also have
//! battle code: the battle finds it in a registry by the move's, ability's
//! or item's own id (rows of {u16 id, u16 pad, u32 handler getter}):
//!
//! | registry                  | rows | aletheia                         |
//! |---------------------------|------|----------------------------------|
//! | gWazaHandlerRegistry      | 258  | ov093/hand_waza.c:1404           |
//! | gAbilityHandlerRegistry   | 158  | ov093/hand_tokusei.c:324         |
//! | gItemHandlerRegistry      | 172  | ov093/hand_item.c:438            |
//!
//! An id with no row has no code: it does only what its data says. A new id
//! a mod adds has no row, so a copy of a move with code is a plain move
//! until the port lets a new id use another's code.
//!
//! The type chart (gBtlCalcTypeAffinityTable) is 17 x 17 bytes, attacker
//! type by defender type: 0 no effect, 2 not very effective, 4 normal,
//! 8 super effective (ov093/btl_calc.c:264).

use std::sync::Arc;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};

use crate::{codenames, tables};

/// The 17 types of Black and White.
pub const TYPE_COUNT: usize = 17;

/// What has a registry of coded effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Move,
    Ability,
    Item,
}

impl Owner {
    pub fn parse(s: &str) -> Option<Owner> {
        match s {
            "move" | "moves" => Some(Owner::Move),
            "ability" | "abilities" => Some(Owner::Ability),
            "item" | "items" => Some(Owner::Item),
            _ => None,
        }
    }

    /// The code table (addresses.json) and its row count.
    fn registry(self) -> (&'static str, usize) {
        match self {
            Owner::Move => ("move_handlers", 0x102),
            Owner::Ability => ("ability_handlers", 0x9E),
            Owner::Item => ("item_handlers", 0xAC),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Owner::Move => "move",
            Owner::Ability => "ability",
            Owner::Item => "item",
        }
    }
}

/// The battle code one id has.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coded {
    pub id: u16,
    /// The handler getter's address in the cartridge.
    pub getter: u32,
    /// Its name in aletheia (`HAND_WAZA_GetThunderWaveHandlers`).
    pub function: Option<String>,
    /// The name in plain words, from the function's (`Thunder Wave`).
    pub label: Option<String>,
    /// The other ids that run the same code.
    pub shared_with: Vec<u16>,
}

/// The ids that have battle code, from the cartridge's registry.
pub fn coded(bridge: &Bridge, owner: Owner) -> Result<Arc<Vec<Coded>>, Error> {
    bridge.cached(&format!("battle/coded/{}", owner.key()), || {
        let code = bridge
            .code()
            .ok_or_else(|| Error::Missing("the game's code is not loaded".into()))?;
        let (name, rows) = owner.registry();
        let bytes = code
            .table(name, rows * 8)
            .ok_or_else(|| Error::Missing(format!("{name} is not known for this cartridge")))?;
        let mut out: Vec<Coded> = Vec::new();
        for row in bytes.as_chunks::<8>().0 {
            let id = u16::from_le_bytes([row[0], row[1]]);
            let getter = u32::from_le_bytes([row[4], row[5], row[6], row[7]]);
            if getter == 0 {
                continue;
            }
            let function = codenames::label(
                code.game_code(),
                &codenames::overlay_region(93),
                getter & !1,
            );
            let label = function.as_deref().map(plain_label);
            out.push(Coded {
                id,
                getter,
                function,
                label,
                shared_with: Vec::new(),
            });
        }
        let pairs: Vec<(u16, u32)> = out.iter().map(|c| (c.id, c.getter)).collect();
        for c in &mut out {
            c.shared_with = pairs
                .iter()
                .filter(|(id, g)| *g == c.getter && *id != c.id)
                .map(|(id, _)| *id)
                .collect();
        }
        Ok(out)
    })
}

/// A place where the game's code tests an ability's id itself, outside the
/// handler registry: in battle (Early Bird, No Guard...), or outside it
/// (wild encounters, eggs, after a battle). Found by reading aletheia's
/// code; the six abilities without a registry row (35, 48, 82, 99, 118,
/// 121) are all here.
#[derive(Debug, Clone, Serialize)]
pub struct CodeCheck {
    /// "battle" or "outside battle".
    pub place: &'static str,
    /// What it does, in plain words.
    pub what: &'static str,
    /// aletheia's function.
    pub function: &'static str,
}

const ABILITY_CHECKS: &[(u16, &str, &str, &str)] = &[
    (
        1,
        "outside battle",
        "First in the party: fewer wild Pokemon appear.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        8,
        "outside battle",
        "First in the party: fewer wild Pokemon appear in a sandstorm.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        9,
        "outside battle",
        "First in the party: wild Electric Pokemon appear more often (50%).",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        14,
        "outside battle",
        "First in the party: wild Pokemon hold items more often (60% and 20%).",
        "FIELD_ENCOUNT_RollHeldItem",
    ),
    (
        21,
        "outside battle",
        "First in the party: fish bite more often.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        22,
        "outside battle",
        "First in the party: wild Pokemon 5 or more levels lower stay away (50%).",
        "FIELD_ENCOUNT_IsRepelled",
    ),
    (
        28,
        "outside battle",
        "First in the party: wild Pokemon have its nature (50%).",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        30,
        "outside battle",
        "Status problems are cured after a battle.",
        "EVENT_BATTLE_RETURN_ApplyNaturalCure",
    ),
    (
        35,
        "outside battle",
        "First in the party: more wild Pokemon appear. It does nothing in battle.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        40,
        "outside battle",
        "In the party: eggs hatch in half the steps.",
        "FIELD_EVENT_CHECK_GetEggCycleDecrement",
    ),
    (
        42,
        "outside battle",
        "First in the party: wild Steel Pokemon appear more often (50%).",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        46,
        "outside battle",
        "First in the party: wild Pokemon are more often of a high level (50%).",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        48,
        "battle",
        "Wakes up twice as fast: the sleep counter goes down by two each turn.",
        "BTL_POKEPARAM_TickSleepTurn",
    ),
    (
        49,
        "outside battle",
        "In the party: eggs hatch in half the steps.",
        "FIELD_EVENT_CHECK_GetEggCycleDecrement",
    ),
    (
        51,
        "outside battle",
        "First in the party: wild Pokemon 5 or more levels lower stay away (50%).",
        "FIELD_ENCOUNT_IsRepelled",
    ),
    (
        53,
        "outside battle",
        "May pick up an item after a battle.",
        "EVENT_BATTLE_RETURN_ApplyPickupAndHoneyGather",
    ),
    (
        55,
        "outside battle",
        "First in the party: wild Pokemon are more often of a high level (50%).",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        56,
        "outside battle",
        "First in the party: wild Pokemon are of the other gender (67%).",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        60,
        "outside battle",
        "First in the party: fish bite more often.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        71,
        "outside battle",
        "First in the party: more wild Pokemon appear.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        72,
        "outside battle",
        "First in the party: wild Pokemon are more often of a high level (50%).",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        73,
        "outside battle",
        "First in the party: fewer wild Pokemon appear.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        81,
        "outside battle",
        "First in the party: fewer wild Pokemon appear in hail.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        82,
        "battle",
        "Eats pinch Berries early: at half its HP instead of a quarter.",
        "HAND_ITEM_IsHpAtOrBelowFraction",
    ),
    (
        95,
        "outside battle",
        "First in the party: fewer wild Pokemon appear.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        99,
        "battle",
        "Every move it uses, and every move used on it, hits.",
        "BTL_SERVER_FLOW_CheckNoGuardHit",
    ),
    (
        99,
        "outside battle",
        "First in the party: more wild Pokemon appear.",
        "FIELD_ENCOUNT_ApplyLeadAbility",
    ),
    (
        118,
        "outside battle",
        "May find Honey after a battle. It does nothing in battle.",
        "EVENT_BATTLE_RETURN_ApplyPickupAndHoneyGather",
    ),
    (
        121,
        "battle",
        "Arceus's type follows its held Plate; the ability cannot be copied or swapped.",
        "POKE_TOOL_GetBoxValue, BTL_ID_IsMultitypeAbility",
    ),
];

/// Where the game tests an ability's id itself (not through its battle
/// code).
pub fn ability_checks(id: u16) -> Vec<CodeCheck> {
    ABILITY_CHECKS
        .iter()
        .filter(|c| c.0 == id)
        .map(|&(_, place, what, function)| CodeCheck {
            place,
            what,
            function,
        })
        .collect()
}

/// The battle code of one id, when it has some.
pub fn coded_of(bridge: &Bridge, owner: Owner, id: u16) -> Result<Option<Coded>, Error> {
    Ok(coded(bridge, owner)?.iter().find(|c| c.id == id).cloned())
}

/// `HAND_WAZA_GetThunderWaveHandlers` -> `Thunder Wave`;
/// `HAND_TOKUSEI_AddStatic` -> `Static`.
fn plain_label(function: &str) -> String {
    let base = function.split('+').next().unwrap_or(function);
    let mut s = base;
    for prefix in ["HAND_WAZA_", "HAND_TOKUSEI_", "HAND_ITEM_"] {
        s = s.strip_prefix(prefix).unwrap_or(s);
    }
    for prefix in ["Get", "Add"] {
        s = s.strip_prefix(prefix).unwrap_or(s);
    }
    s = s.strip_suffix("Handlers").unwrap_or(s);
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            out.push(' ');
            continue;
        }
        let split = i > 0
            && c.is_uppercase()
            && (chars[i - 1].is_lowercase()
                || chars[i - 1].is_ascii_digit()
                || chars.get(i + 1).is_some_and(|n| n.is_lowercase())
                    && chars[i - 1].is_uppercase());
        if split && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// The type chart: `chart[attacker][defender]`, 0 no effect, 2 not very
/// effective, 4 normal, 8 super effective.
pub fn type_chart(bridge: &Bridge) -> Result<Vec<Vec<u8>>, Error> {
    let code = bridge
        .code()
        .ok_or_else(|| Error::Missing("the game's code is not loaded".into()))?;
    let bytes = code
        .table("type_chart", TYPE_COUNT * TYPE_COUNT)
        .ok_or_else(|| Error::Missing("the type chart is not known for this cartridge".into()))?;
    Ok(bytes.chunks(TYPE_COUNT).map(<[u8]>::to_vec).collect())
}

// ---------------------------------------------------------------- uses

/// A species (or form record) that learns a move at a level.
#[derive(Debug, Clone, Serialize)]
pub struct LevelUp {
    pub species: usize,
    pub level: u16,
}

/// Who learns a move.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveUsers {
    pub level_up: Vec<LevelUp>,
    pub egg: Vec<usize>,
    /// The TM or HM that teaches it ("TM24", "HM03") and who can use it.
    pub machine: Option<String>,
    pub machine_species: Vec<usize>,
    /// Taught by a move tutor, and who can learn it there.
    pub tutor: bool,
    pub tutor_species: Vec<usize>,
    /// Trainers whose parties have it.
    pub trainers: Vec<usize>,
}

/// The move each tutor bit of a species record (+56) teaches: Grass,
/// Fire and Water Pledge, Frenzy Plant, Blast Burn, Hydro Cannon (ov021
/// SCRCMD_POKEMON_FLD_GetTutorMoveId), Draco Meteor (bit 6, from the data:
/// the 31 Dragon types that have it).
pub const TUTOR_MOVES: [u16; 7] = [520, 519, 518, 338, 307, 308, 434];

const SPECIES_ARC: u16 = 16;
const LEARNSET_ARC: u16 = 18;
const EGG_MOVE_ARC: u16 = 123;
/// Real species records (1..649); the members after are forms.
const SPECIES_LAST: usize = 649;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn members(bridge: &Bridge, arc: u16) -> usize {
    let base = bridge.base_archive(arc).map(|a| a.len()).unwrap_or(0);
    let mine = bridge
        .project()
        .and_then(|p| p.last_member(arc))
        .map_or(0, |i| i + 1);
    base.max(mine)
}

/// Who learns `mv`: by level, as an egg move, from a TM or HM, and the
/// trainers who use it.
pub fn move_users(bridge: &Bridge, mv: u16) -> Result<MoveUsers, Error> {
    let mut out = MoveUsers::default();
    for sp in 1..members(bridge, LEARNSET_ARC) {
        let Ok(b) = bridge.member(Member {
            arc: LEARNSET_ARC,
            index: sp,
        }) else {
            continue;
        };
        let mut at = 0;
        while let (Some(m), Some(level)) = (u16_at(&b, at), u16_at(&b, at + 2)) {
            if m == 0xFFFF {
                break;
            }
            if m == mv {
                out.level_up.push(LevelUp { species: sp, level });
            }
            at += 4;
        }
    }
    for sp in 1..members(bridge, EGG_MOVE_ARC) {
        let Ok(b) = bridge.member(Member {
            arc: EGG_MOVE_ARC,
            index: sp,
        }) else {
            continue;
        };
        let count = u16_at(&b, 0).unwrap_or(0) as usize;
        if (0..count).any(|i| u16_at(&b, 2 + i * 2) == Some(mv)) {
            out.egg.push(sp);
        }
    }
    if let Some(slot) = machine_slot(bridge, mv) {
        out.machine = Some(if slot >= 95 {
            format!("HM{:02}", slot - 94)
        } else {
            format!("TM{:02}", slot + 1)
        });
        for sp in 1..=SPECIES_LAST.min(members(bridge, SPECIES_ARC).saturating_sub(1)) {
            let Ok(b) = bridge.member(Member {
                arc: SPECIES_ARC,
                index: sp,
            }) else {
                continue;
            };
            let word = 40 + (slot / 32) * 4;
            let bits = b
                .get(word..word + 4)
                .map_or(0, |x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
            if bits >> (slot % 32) & 1 == 1 {
                out.machine_species.push(sp);
            }
        }
    }
    if let Some(bit) = TUTOR_MOVES.iter().position(|&m| m == mv) {
        out.tutor_species = species_records(bridge)
            .into_iter()
            .filter(|(_, b)| {
                let bits = b
                    .get(56..60)
                    .map_or(0, |x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
                bits >> bit & 1 == 1
            })
            .map(|(sp, _)| sp)
            .collect();
        out.tutor = true;
    }
    out.trainers = trainers_with(bridge, "move", mv)?;
    Ok(out)
}

/// The move of each TM/HM slot (TM01..TM95, HM01..HM06), from the ARM9
/// table `machine_moves` (arm9/file_util.c FILE_UTIL_GetMachineMove).
pub fn machine_moves(bridge: &Bridge) -> Result<Vec<u16>, Error> {
    let code = bridge
        .code()
        .ok_or_else(|| Error::Missing("the game's code is not loaded".into()))?;
    let t = code
        .table("machine_moves", 101 * 2)
        .ok_or_else(|| Error::Missing("the TM table is not known for this cartridge".into()))?;
    Ok((0..101).filter_map(|i| u16_at(t, i * 2)).collect())
}

/// The TM/HM slot (0..100: TM01..TM95, then HM01..HM06) that teaches a move.
fn machine_slot(bridge: &Bridge, mv: u16) -> Option<usize> {
    let code = bridge.code()?;
    let t = code.table("machine_moves", 101 * 2)?;
    (0..101).find(|&i| u16_at(t, i * 2) == Some(mv))
}

/// Trainers whose party names `id` of enum `kind` ("move" or "item").
fn trainers_with(bridge: &Bridge, kind: &str, id: u16) -> Result<Vec<usize>, Error> {
    let t = tables::table("trainerParties")?;
    let mut out = Vec::new();
    for index in 1..members(bridge, t.archive) {
        let found = tables::remap_member(bridge, "trainerParties", index, kind, &|v| {
            if v == id as i64 {
                tables::Remap::Drop
            } else {
                tables::Remap::Keep
            }
        })
        .ok()
        .flatten()
        .is_some();
        if found {
            out.push(index);
        }
    }
    Ok(out)
}

/// Where an item is found or used.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemUsers {
    /// Wild Pokemon that may hold it: (species, slot 0 common 1 rare
    /// 2 shaking grass).
    pub wild: Vec<(usize, u8)>,
    /// Trainers that use it in battle (their bag of up to four items).
    pub trainer_bag: Vec<usize>,
    /// Trainers whose Pokemon hold it.
    pub trainer_held: Vec<usize>,
    /// Trainers that give it after the battle.
    pub trainer_gift: Vec<usize>,
    /// In-game trades whose Pokemon holds it.
    pub trades: Vec<usize>,
    /// Item balls and hidden items on maps: (place, "ball" or "hidden").
    pub on_maps: Vec<(String, String)>,
}

/// Where `item` is found or used, across the game and the mod.
pub fn item_users(bridge: &Bridge, item: u16) -> Result<ItemUsers, Error> {
    let mut out = ItemUsers::default();
    for (sp, b) in species_records(bridge) {
        for (slot, at) in [12usize, 14, 16].iter().enumerate() {
            if u16_at(&b, *at) == Some(item) {
                out.wild.push((sp, slot as u8));
            }
        }
    }
    let trainers = tables::table("trainers")?;
    for t in 1..members(bridge, trainers.archive) {
        let Ok(b) = bridge.member(Member {
            arc: trainers.archive,
            index: t,
        }) else {
            continue;
        };
        if (0..4).any(|k| u16_at(&b, 4 + k * 2) == Some(item)) {
            out.trainer_bag.push(t);
        }
        if u16_at(&b, 18) == Some(item) {
            out.trainer_gift.push(t);
        }
    }
    out.trainer_held = trainers_with(bridge, "item", item)?;
    let trades = tables::table("trades")?;
    for t in 0..members(bridge, trades.archive) {
        if let Ok(b) = bridge.member(Member {
            arc: trades.archive,
            index: t,
        }) {
            if u16_at(&b, 76) == Some(item) {
                out.trades.push(t);
            }
        }
    }
    out.on_maps = item_places(bridge)?
        .iter()
        .filter(|(i, _, _)| *i == item)
        .map(|(_, place, kind)| (place.clone(), kind.clone()))
        .collect();
    Ok(out)
}

/// One item on the maps: (item, place, kind).
type ItemPlace = (u16, String, String);

/// Every item ball (NPC with script 7000..7799) and hidden item (sign
/// event with script 8000..8799) on the maps.
fn item_places(bridge: &Bridge) -> Result<Arc<Vec<ItemPlace>>, Error> {
    use crate::fielditems::{event_item, BALL_FIRST, BALL_LAST, HIDDEN_FIRST, HIDDEN_LAST};
    bridge.cached("battle/item-places", || {
        let mut out = Vec::new();
        for zone in crate::world::zones(bridge)?.iter() {
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
            let mut found = |script: u16, kind: &str| {
                if let Some(e) = event_item(bridge, script) {
                    out.push((e.item, zone.place.clone(), kind.to_string()));
                }
            };
            for n in 0..signs {
                if let Some(s) = u16_at(&file, 8 + 20 * n) {
                    if (HIDDEN_FIRST..=HIDDEN_LAST).contains(&s) {
                        found(s, "hidden");
                    }
                }
            }
            let first = 8 + 20 * signs;
            for n in 0..npcs {
                if let Some(s) = u16_at(&file, first + 36 * n + 0x0A) {
                    if (BALL_FIRST..=BALL_LAST).contains(&s) {
                        found(s, "ball");
                    }
                }
            }
        }
        Ok(out)
    })
}

/// The map event files (a/1/2/5).
const EVENT_FILES: u16 = 125;

/// The species that have an ability, by slot.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AbilityUsers {
    pub first: Vec<usize>,
    pub second: Vec<usize>,
    pub hidden: Vec<usize>,
}

pub fn ability_users(bridge: &Bridge, ability: u8) -> Result<AbilityUsers, Error> {
    let mut out = AbilityUsers::default();
    for sp in species_records(bridge) {
        let (index, b) = sp;
        if b.get(24) == Some(&ability) {
            out.first.push(index);
        }
        if b.get(25) == Some(&ability) && b.get(24) != Some(&ability) {
            out.second.push(index);
        }
        if b.get(26) == Some(&ability) {
            out.hidden.push(index);
        }
    }
    Ok(out)
}

/// Species and moves of a type.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TypeUsers {
    pub species: Vec<usize>,
    pub moves: Vec<usize>,
}

pub fn type_users(bridge: &Bridge, ty: u8) -> Result<TypeUsers, Error> {
    let mut out = TypeUsers::default();
    for (index, b) in species_records(bridge) {
        if b.get(6) == Some(&ty) || b.get(7) == Some(&ty) {
            out.species.push(index);
        }
    }
    let moves = crate::content::kind("moves").expect("moves take new entries");
    for mv in 1..crate::content::count(bridge, moves)? {
        if let Ok(b) = bridge.member(Member { arc: 21, index: mv }) {
            if b.first() == Some(&ty) {
                out.moves.push(mv);
            }
        }
    }
    Ok(out)
}

/// The name of every species or form record of a/0/1/6 (and of the
/// learnsets and evolutions, which share its numbering): the species name
/// for 1..649, "Deoxys (form 1)" for a form record, found through each
/// species' formStatsIndex (+28) and formCount (+32): form n >= 1 of a
/// species is record formStatsIndex + n - 1 (poke_personal.c
/// POKE_PERSONAL_GetFormDataId).
pub fn record_names(bridge: &Bridge) -> Result<Arc<Vec<String>>, Error> {
    bridge.cached("battle/record-names", || {
        let species = tables::bank_strings(
            bridge,
            &tables::BankRef {
                archive: 2,
                bank: 70,
            },
        );
        let count = members(bridge, SPECIES_ARC);
        let mut out: Vec<String> = (0..count)
            .map(|i| {
                species
                    .get(i)
                    .filter(|s| !s.trim().is_empty())
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        for (sp, b) in species_records(bridge) {
            let first = u16_at(&b, 28).unwrap_or(0) as usize;
            let forms = b.get(32).copied().unwrap_or(0) as usize;
            if first == 0 || forms < 2 {
                continue;
            }
            for n in 1..forms {
                let record = first + n - 1;
                // 650 and 651 are egg names in the bank, Deoxys forms here.
                if record > SPECIES_LAST && record < out.len() {
                    out[record] = format!(
                        "{} (form {n})",
                        species.get(sp).cloned().unwrap_or_default()
                    );
                }
            }
        }
        if let Some(last) = out.last_mut() {
            if last.is_empty() && count > SPECIES_LAST + 1 {
                *last = "Regional Pokedex order (not a Pokemon)".into();
            }
        }
        Ok(out)
    })
}

/// The species records 1..649 with their bytes.
fn species_records(bridge: &Bridge) -> Vec<(usize, Vec<u8>)> {
    (1..=SPECIES_LAST)
        .filter_map(|sp| {
            bridge
                .member(Member {
                    arc: SPECIES_ARC,
                    index: sp,
                })
                .ok()
                .map(|b| (sp, b))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(
            plain_label("HAND_WAZA_GetThunderWaveHandlers"),
            "Thunder Wave"
        );
        assert_eq!(plain_label("HAND_TOKUSEI_AddStatic"), "Static");
        assert_eq!(
            plain_label("HAND_TOKUSEI_GetIntimidateHandlers"),
            "Intimidate"
        );
        assert_eq!(
            plain_label("HAND_ITEM_GetLeftoversHandlers+0x4"),
            "Leftovers"
        );
    }
}
