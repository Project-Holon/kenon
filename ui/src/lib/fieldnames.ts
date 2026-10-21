// Names for the numbers of a map event, from the game's own code and data
// (aletheia src/game): what a movement code does, what an event type is,
// how a script id or an event flag is used, and what a sound sequence is.

/// Direction numbers of a person's facing and of the move sets: the game's
/// direction order (ov021 fldmmdl_movecode.c, gMmdlDirSets).
export const FACING = ["Up", "Down", "Left", "Right"];

const DIR_SETS: Record<string, string> = {
  "0123": "any way",
  "02": "up or left",
  "03": "up or right",
  "12": "down or left",
  "13": "down or right",
  "012": "up, down or left",
  "013": "up, down or right",
  "023": "up, left or right",
  "123": "down, left or right",
  "01": "up or down",
  "23": "left or right",
};

// The turn sets, in the order of FLDMMDL_MOVECODE_InitTurnRandomSet0..8
// (codes 2 and 6 to 13); sets 9 and 10 (codes 45 and 46) are up or down and
// left or right.
const TURN_SETS = ["0123", "02", "03", "12", "13", "012", "013", "023", "123"];

// Movement codes 0..84. The code is an index into the move table of overlay
// 21 (FLDMMDL_GetMoveProcList); each name comes from the init function in
// that table (fldmmdl_movecode.c and fldmmdl_rail_move.c).
function buildMoves(): Record<number, string> {
  const m: Record<number, string> = {
    0: "Does nothing",
    1: "Does nothing",
    3: "Wanders (walks any way)",
    4: "Walks up and down",
    5: "Walks left and right",
    14: "Stands facing up",
    15: "Stands facing down",
    16: "Stands facing left",
    17: "Stands facing right",
    18: "Spins left",
    19: "Spins right",
    20: "Walks back and forth",
    47: "Not used",
    48: "Turns to face you",
    49: "Swings side to side",
    50: "Looks at other people",
    51: "Hidden (style 1)",
    52: "Hidden (style 2)",
    53: "Hidden (style 2, alternate)",
    54: "Hidden (style 1, alternate)",
    55: "Faces up",
    56: "Faces down",
    57: "Faces left",
    58: "Faces right",
    59: "Faces up, then the way you face",
    60: "Faces down, then the way you face",
    61: "Faces left, then the way you face",
    62: "Faces right, then the way you face",
    63: "Follows the wall (left hand)",
    64: "Follows the wall (right hand)",
    65: "Follows the wall (left hand, wide)",
    66: "Follows the wall (right hand, wide)",
    67: "Walks left and right (bumps)",
    68: "Another player online",
    69: "Stands still (rail map)",
    82: "Wanders (rail map)",
    83: "Wanders (rail map, alternate)",
    84: "Paces back and forth (rail map)",
  };
  TURN_SETS.forEach((set, i) => {
    const code = i === 0 ? 2 : i + 5;
    m[code] = i === 0 ? "Looks around" : `Looks ${DIR_SETS[set]}`;
  });
  m[45] = `Looks ${DIR_SETS["01"]}`;
  m[46] = `Looks ${DIR_SETS["23"]}`;
  for (let i = 0; i < 16; i++) m[21 + i] = `Walks route ${i + 1}`;
  for (let i = 0; i < 8; i++) m[37 + i] = `Paces route ${i + 1}`;
  for (let i = 0; i <= 10; i++) m[70 + i] = `Looks around (rail map, style ${i + 1})`;
  return m;
}
const MOVES = buildMoves();

export function moveName(code: number): string {
  return MOVES[code] ?? `Movement ${code}`;
}

/// Every movement code with a name, for a choice list.
export function moveOptions(): [number, string][] {
  return Object.keys(MOVES)
    .map(Number)
    .filter((c) => c !== 47)
    .sort((a, b) => a - b)
    .map((c) => [c, MOVES[c]]);
}

// Event types of a person. FLDMMDL_NormalizeEventType folds 4 to 8 and 11 to
// 13 into 1; EVENT_TRAINER_EYE sees along the facing for 1 and in all four
// ways for 2; 7, 8, 17 and 18 mirror the player's turns (IsMirrorEventType);
// 13 and 15 to 19 run with the player (IsDashEventType).
const EVENT_TYPES: Record<number, string> = {
  0: "Normal person",
  1: "Trainer, sees ahead",
  2: "Trainer, sees all four ways",
  4: "Trainer, sees ahead (type 4)",
  5: "Trainer, sees ahead (type 5)",
  6: "Trainer, sees ahead (type 6)",
  7: "Trainer, mirrors your turns (left side)",
  8: "Trainer, mirrors your turns (right side)",
  11: "Trainer, sees ahead (type 11)",
  12: "Trainer, sees ahead (type 12)",
  13: "Trainer, runs when you run",
  15: "Runs when you run (type 15)",
  16: "Runs when you run (type 16)",
  17: "Mirrors your turns, left side, runs when you run",
  18: "Mirrors your turns, right side, runs when you run",
  19: "Runs when you run (type 19)",
};

export function eventTypeName(t: number): string {
  return EVENT_TYPES[t] ?? `Type ${t}`;
}

export function eventTypeOptions(): [number, string][] {
  return Object.keys(EVENT_TYPES).map((k) => [Number(k), EVENT_TYPES[Number(k)]]);
}

/// Script id ranges the game treats alike (the ranges table of overlay 10;
/// ov010 script_sys.c SCRIPT_SYS_GetScriptFileId, event_trainer.c).
export function scriptKind(id: number): string {
  if (id === 0) return "No script";
  if (id < 2000) return `Script ${id} of this map`;
  if (id >= 3000 && id < 5000) return `Trainer ${id - 3000} (battle script)`;
  if (id >= 5000 && id < 7000) return `Trainer ${id - 5000} (second of a pair)`;
  if (id >= 7000 && id < 7800) return `Item ball script ${id - 7000}`;
  if (id >= 8000 && id < 8800) return `Hidden item script ${id - 8000}`;
  return `Shared script ${id}`;
}

/// What an event flag does for this event, from where the game reads it.
/// A person is placed on the map only while its flag is clear
/// (FLDMMDL_AddMmdlHeaderList); a trainer's defeated flag is 1420 + id
/// (EVENT_TRAINER_CheckDefeatedFlag); a hidden item's flag is 900 + its
/// index (HIDDEN_ITEM_GetFlag).
export function flagNote(flag: number, kind: "npc" | "hidden" | "item" | "trainer"): string {
  if (flag === 0) return "No flag";
  const base = `Flag ${flag}`;
  if (kind === "hidden") return `${base}, set when it is picked up`;
  if (kind === "item") return `${base}, set when the item is taken`;
  if (flag >= 1420 && flag < 1420 + 1000) {
    return `${base}, set when trainer ${flag - 1420} is beaten`;
  }
  return `${base}, the person is gone once it is set`;
}

/// Object codes 162 to 177 are not a picture: the game reads the picture
/// from a script variable (FLDMMDL_GetEventWorkObjCode).
export const isVariableCharacter = (code: number) => code >= 162 && code <= 177;

export interface SoundLabel {
  group: "music" | "effect" | "cry" | "jingle" | "other";
  /// "Plays sound Select", "Plays music Pokemon League", ...
  text: string;
  /// The readable name alone.
  name: string;
}

// The jingles are named in Japanese romaji; these are what they play for.
const JINGLES: Record<string, string> = {
  ASA: "Morning",
  LVUP: "Level up",
  ITEM: "Item",
  KEYITEM: "Key item",
  POKEGET: "Pokemon caught",
  SHINKAOME: "Evolution",
  BADGE: "Badge",
  WAZA: "Move learned",
  ACCE: "Accessory",
  WASURE: "Move forgotten",
  KINOMI: "Berry",
  HYOUKA1: "Rating 1",
  HYOUKA2: "Rating 2",
  HYOUKA3: "Rating 3",
  HYOUKA4: "Rating 4",
  HYOUKA5: "Rating 5",
  HYOUKA6: "Rating 6",
  TAMAGO_GET: "Egg received",
  BPGET: "BP received",
  AISHOU_H: "Compatibility high",
  AISHOU_M: "Compatibility medium",
  AISHOU_L: "Compatibility low",
  AISHOU_KIDOU: "Compatibility start",
  MISSION_START: "Mission start",
  MISSION_CLEAR: "Mission cleared",
  MISSION_FAILED: "Mission failed",
  ITEM_MYSTERY: "Mystery Gift item",
  CALL: "Call",
  QUIZ: "Quiz",
};

const sentence = (s: string) => {
  const t = s.toLowerCase().replaceAll("_", " ").trim();
  return t.charAt(0).toUpperCase() + t.slice(1);
};

/// A sequence name made readable: SEQ_SE_SELECT1 is a sound called
/// "Select1", SEQ_BGM_POKECEN is music "Pokecen", SEQ_ME_ITEM a jingle.
export function soundLabel(seqName: string): SoundLabel {
  const m = /^SEQ_(BGM|SE|ME|PV)_?(.*)$/.exec(seqName);
  if (!m) return { group: "other", text: `Plays ${sentence(seqName.replace(/^SEQ_/, ""))}`, name: sentence(seqName.replace(/^SEQ_/, "")) };
  const [, tag, rest] = m;
  if (tag === "ME") {
    const name = JINGLES[rest] ?? sentence(rest);
    return { group: "jingle", text: `Plays jingle ${name}`, name };
  }
  const name = sentence(rest) || "Default";
  if (tag === "BGM") return { group: "music", text: `Plays music ${name}`, name };
  if (tag === "PV") return { group: "cry", text: `Plays cry ${name}`, name };
  return { group: "effect", text: `Plays sound ${name}`, name };
}
