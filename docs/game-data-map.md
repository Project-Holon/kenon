# Game data map

Where each kind of editable data lives in the game files and how its records are laid out. This is the working reference behind `core/src/schemas.json`, which holds the same layouts in machine-readable form. The facts come from the aletheia decompilation (struct definitions and loader calls in `src/game`, the asset notes in `docs/ASSETS.md`, the decoders in `tools/assets`), from the eidos port (`docs/mod-events.md`, `docs/mods.md`), and from reading the extracted archives to confirm record sizes, member counts and a handful of values. No game bytes or text are copied here.

Tags: **[V]** verified (a struct or loader call in the decompiled code, or a value confirmed against the data), **[I]** inferred (consistent with the data or with names in the code, not pinned down). Source references name the decompiled file, for example `waza_tool.c:29` is `src/game/arm9/waza_tool.c`, line 29.

## Conventions

- **Archive ids.** The game opens an archive by a decimal id that spells its path: id = 100X + 10Y + Z for `a/X/Y/Z` (for example 16 is `a/0/1/6`; item data is 0x18 = 24 = `a/0/2/4`). [V: ASSETS.md "Item archive id closed by code"; `poke_personal.c:82` opens 0x10 for the personal data]
- **Members.** A NARC member is addressed by index. A mod writes one member as the file `a/X/Y/Z/<index>`. Member 0 is usually an empty or sentinel entry, so species, move, item and trainer ids equal the member index directly.
- **Compression.** Data tables and palettes are stored raw. Graphics tile sheets and some animation files in the battle sprite archives are LZ11 (first byte 0x11); everything else in the graphics archives listed below is raw. Detect by the first byte as kenon already does.
- **Little endian**, no alignment padding inside records. Bit fields are read from the little-endian word at the stated offset, bit 0 first.
- **Non-NARC files.** `a/0/1/3` (map area records) is a flat 2820-byte file and `wb_sound_data.sdat` is an SDAT; both are replaced as whole files. Some tables are not in any archive but in the ARM9 image (TM list, item icon map, trainer sprite map, icon palette selector). They are listed in "ARM9 tables" below.
- **Text.** Names and descriptions are entries of message banks in `a/0/0/2` (see "Text").
- **Forms.** Personal data, evolutions and learnsets have 668 members: record 0 is a sentinel, records 1..649 are species, records 650 and above are alternate forms. A form is reached through the base species record: form n (n >= 1) is member `formStatsIndex + n - 1` (`poke_personal.c`, the species-to-form resolver). [V]

## Pokemon stats: personal data

- Archive: `a/0/1/6` (id 16). Members: one member per species or form record: 0 is a 56-byte sentinel, 1..649 species, 650..667 extra form records, 668 is the form table (not a record) (669). Record size: 60 bytes. Compressed: no. Names: text bank 70. Source: `poke_personal_types.h:7, poke_personal.c:82`.

Reader: `POKE_PERSONAL_GetParam` has 44 getter cases over this record (`poke_personal.c`, struct at `poke_personal_types.h:7`). Field names below are the decompilation's. Sizes verified: 667 records of 60 bytes, plus a 56-byte sentinel (member 0) and a 1300-byte member 668 (the form table, below).

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u8 | `baseHp` | Base HP. | [V] |
| 0x01 | u8 | `baseAttack` | Base Attack. | [V] |
| 0x02 | u8 | `baseDefense` | Base Defense. | [V] |
| 0x03 | u8 | `baseSpeed` | Base Speed. | [V] |
| 0x04 | u8 | `baseSpAttack` | Base Sp. Atk. | [V] |
| 0x05 | u8 | `baseSpDefense` | Base Sp. Def. | [V] |
| 0x06 | u8 | `type1` | Type 1. type id; see enum type | [V] |
| 0x07 | u8 | `type2` | Type 2. enum type. equal to type1 for a mono-type species | [V] |
| 0x08 | u8 | `catchRate` | Catch rate. | [V] |
| 0x09 | u8 | `evoStage` | Evolution stage. 1 for a first stage, 2 for a middle or final stage (Bulbasaur 1, Pikachu 2), 3 seen on single-stage legendaries (Mewtwo, Giratina); exact rule not established | [I] |
| 0x0A | bits 0-1 of u16 | `evHp` | EV yield HP. 0..3 | [V] |
| 0x0A | bits 2-3 of u16 | `evAttack` | EV yield Attack. | [V] |
| 0x0A | bits 4-5 of u16 | `evDefense` | EV yield Defense. | [V] |
| 0x0A | bits 6-7 of u16 | `evSpeed` | EV yield Speed. | [V] |
| 0x0A | bits 8-9 of u16 | `evSpAttack` | EV yield Sp. Atk. | [V] |
| 0x0A | bits 10-11 of u16 | `evSpDefense` | EV yield Sp. Def. | [V] |
| 0x0A | bit 12 of u16 | `yieldFlag` | Yield flag. named yieldFlag in the decompilation (getter case 16); meaning not established | [I] |
| 0x0C | u16 | `heldItemCommon` | Held item (common). enum item. item id, 0 = none; 50 percent in the wild | [I] |
| 0x0E | u16 | `heldItemRare` | Held item (rare). enum item. item id; 5 percent in the wild | [I] |
| 0x10 | u16 | `heldItemDream` | Held item (dream). enum item. third slot; named heldItemDream in the decompilation | [I] |
| 0x12 | u8 | `genderRatio` | Gender ratio. female threshold out of 254: 0 always male, 254 always female, 255 genderless (31 = 12.5 percent female, 127 = 50 percent) | [V] |
| 0x13 | u8 | `eggCycles` | Egg cycles. | [V] |
| 0x14 | u8 | `baseFriendship` | Base friendship. | [V] |
| 0x15 | u8 | `growthRate` | Growth rate. enum growthRate | [V] |
| 0x16 | u8 | `eggGroup1` | Egg group 1. enum eggGroup | [V] |
| 0x17 | u8 | `eggGroup2` | Egg group 2. enum eggGroup | [V] |
| 0x18 | u8 | `ability1` | Ability 1. enum ability. ability id; 0 = none | [V] |
| 0x19 | u8 | `ability2` | Ability 2. enum ability. ability id; 0 = none | [V] |
| 0x1A | u8 | `hiddenAbility` | Hidden ability. enum ability. ability id; 0 = none | [V] |
| 0x1B | u8 | `escapeRate` | Escape rate. named escapeRate in the decompilation; 0 for every record checked (flee rate for Safari-style use) | [I] |
| 0x1C | u16 | `formStatsIndex` | Form record base. member index of this species' first extra form record; form n (n >= 1) lives at formStatsIndex + n - 1 (poke_personal.c 'species-to-form resolver'); 0 = none | [V] |
| 0x1E | u16 | `formSpriteIndex` | Form sprite base. base for the form's sprite blocks: battle sprite block 652 + formSpriteIndex + form - 1, icon member (formSpriteIndex + form - 1) * 2 + 1311, icon palette index formSpriteIndex + form + 651 | [V] |
| 0x20 | u8 | `formCount` | Form count. number of forms including the base form | [V] |
| 0x21 | bits 0-5 of u8 | `color` | Dex color. enum dexColor | [V] |
| 0x21 | bit 6 of u8 | `spriteFlip` | Sprite flip. named spriteFlip in the decompilation (getter 34) | [I] |
| 0x21 | bit 7 of u8 | `formSpriteShared` | Forms share sprite. 1 = the extra forms are palette-only (pokegra.c palOnlyForms): no separate sprite block | [V] |
| 0x22 | u16 | `baseExp` | Base experience. named baseExp in the decompilation; Bulbasaur 64 verified in data | [V] |
| 0x24 | u16 | `height` | Height (dm). decimetres; Bulbasaur 7 verified in data | [V] |
| 0x26 | u16 | `weight` | Weight (hg). hectograms; Bulbasaur 69 verified in data | [V] |
| 0x28 | u32 | `tmFlags1` | TM/HM flags 1. machine bits 0..31: TM01..TM32. Bit layout over the four words: bits 0..91 TM01..92, bits 92..94 TM93..95, bits 95..100 HM01..06 (verified against file_util.c GetId and a species with known TM and HM sets) | [V] |
| 0x2C | u32 | `tmFlags2` | TM/HM flags 2. machine bits 32..63 | [V] |
| 0x30 | u32 | `tmFlags3` | TM/HM flags 3. machine bits 64..95 | [V] |
| 0x34 | u32 | `tmFlags4` | TM/HM flags 4. machine bits 96..100 (HM bits 95..100 straddle words 3 and 4) | [V] |
| 0x38 | u32 | `tutorFlags` | Tutor flags. one bit per move-tutor entry; the tutor move list is not in this archive (gap) | [I] |

Enum `growthRate`: 0 Medium Fast, 1 Erratic, 2 Fluctuating, 3 Medium Slow, 4 Fast, 5 Slow. [I: three starters give 3, Pikachu 0, Mewtwo 5]
Enum `eggGroup`: 1 Monster, 2 Water 1, 3 Bug, 4 Flying, 5 Field, 6 Fairy, 7 Grass, 8 Human-Like, 9 Water 3, 10 Mineral, 11 Amorphous, 12 Water 2, 13 Ditto, 14 Dragon, 15 Undiscovered. [I: confirmed on the starters, Pikachu (5, 6), Mewtwo (15, 15)]
Enum `dexColor`: 0 Red, 1 Blue, 2 Yellow, 3 Green, 4 Black, 5 Brown, 6 Purple, 7 Gray, 8 White, 9 Pink. [I]
Enum `type`: 0 Normal through 16 Dark in the usual order (Normal, Fighting, Flying, Poison, Ground, Rock, Bug, Ghost, Steel, Fire, Water, Grass, Electric, Psychic, Ice, Dragon, Dark); 17 entries, no unknown type. Names come from text bank 199. [V count; order checked on Bulbasaur 11/3, Pikachu 12, Mewtwo 13, Giratina 7/15, Kyurem 15/14]

**Form table (member 668).** A 1300-byte array of 650 u16, read by `POKE_PERSONAL_LoadFormTable` (`poke_personal.c:371`); 999 means no entry, 156 entries are in use. [V that it is a u16 array with the 999 sentinel; I for the mapping, which only matters for the code that walks forms]

**TM and HM compatibility** are the four u32 `tmFlags` words plus `tutorFlags` above. The machine bit layout (verified against `FILE_UTIL_GetId` and a species with known TM and HM sets): bits 0..91 are TM01..TM92, bits 92..94 are TM93..TM95, bits 95..100 are HM01..HM06. The move each machine teaches is in an ARM9 table (below). [V]

## Moves

- Archive: `a/0/2/1` (id 21). Members: one member per move id (member 0 is the empty move) (560). Record size: 36 bytes. Compressed: no. Names: text bank 203. Source: `waza_tool.c:29, waza_tool.c:178`.

Reader: `WAZA_TOOL_GetDataParam` / `GetDataFlag` (`waza_tool.c:198`, struct at `waza_tool.c:29`). The accessor ids and the struct agree with the data (checked on three moves and the 560 constant marker). Note: aletheia's Python decoder `tools/assets/waza.py` places `type` at 0x01; the struct and the data (a Fire move reads type 9 at offset 0, damage class 2 at offset 2) show type at 0x00, so follow the struct. [V]

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u8 | `type` | Type. enum type | [V] |
| 0x01 | u8 | `effectCategory` | Effect category. named category in the decompilation; 14 values (0..13) in data, groups moves by battle-effect family; value meanings not enumerated | [I] |
| 0x02 | u8 | `damageClass` | Damage class. enum damageClass. 0 status, 1 physical, 2 special | [V] |
| 0x03 | u8 | `power` | Power. | [V] |
| 0x04 | u8 | `accuracy` | Accuracy. 101 is stored for 'never misses' and read back as 100 (waza_tool.c getter case 4) | [V] |
| 0x05 | u8 | `pp` | PP. | [V] |
| 0x06 | s8 | `priority` | Priority. | [V] |
| 0x07 | bits 0-3 of u8 | `hitMin` | Min hits. multi-hit minimum | [V] |
| 0x07 | bits 4-7 of u8 | `hitMax` | Max hits. multi-hit maximum | [V] |
| 0x08 | u16 | `sickId` | Status condition. inflicted condition id (named sickId; getter case 11) | [I] |
| 0x0A | u8 | `sickPercent` | Status chance (%). | [V] |
| 0x0B | u8 | `sickContType` | Status duration type. 0 none, 1..4 observed | [I] |
| 0x0C | u8 | `sickTurnMin` | Status turns min. | [V] |
| 0x0D | u8 | `sickTurnMax` | Status turns max. | [V] |
| 0x0E | u8 | `criticalRank` | Critical-hit rank. 6 is read back as 0 (always-crit marker handled elsewhere) | [V] |
| 0x0F | u8 | `shrinkPercent` | Flinch chance (%). | [V] |
| 0x10 | u16 | `effectSequence` | Battle effect sequence. index of the battle-effect script (value range 0..337 in data) | [V] |
| 0x12 | s8 | `drainRatio` | Drain / recoil (%). positive = drain, negative = recoil (getter cases 25 and 30) | [V] |
| 0x13 | s8 | `healRatio` | Heal / self-damage (%). positive = heal, negative = damage to the user (cases 26 and 31) | [V] |
| 0x14 | u8 | `target` | Target. targeting mode, 14 values (0 selected target, 4 all adjacent, 5 all foes, 7 user ...); no enum fixed | [I] |
| 0x15 | u8 | `rankType1` | Stat change 1 stat. which stat stage 1 changes | [V] |
| 0x16 | u8 | `rankType2` | Stat change 2 stat. | [V] |
| 0x17 | u8 | `rankType3` | Stat change 3 stat. | [V] |
| 0x18 | s8 | `rankValue1` | Stat change 1 stages. | [V] |
| 0x19 | s8 | `rankValue2` | Stat change 2 stages. | [V] |
| 0x1A | s8 | `rankValue3` | Stat change 3 stages. | [V] |
| 0x1B | u8 | `rankPercent1` | Stat change 1 chance (%). | [V] |
| 0x1C | u8 | `rankPercent2` | Stat change 2 chance (%). | [V] |
| 0x1D | u8 | `rankPercent3` | Stat change 3 chance (%). | [V] |
| 0x1E | u16 | `pad1E` | Marker (constant 0x5353). constant in all 560 records; keep unchanged | [V] |
| 0x20 | bit 0 of u32 | `flagContact` | Makes contact. bit 0 | [V] |
| 0x20 | bit 1 of u32 | `flagCharge` | Charge turn. bit 1; set on 12 moves (two-turn moves) | [I] |
| 0x20 | bit 2 of u32 | `flagRecharge` | Recharge turn. bit 2; set on 7 moves | [I] |
| 0x20 | bit 3 of u32 | `flagProtect` | Blocked by Protect. bit 3 | [V] |
| 0x20 | bit 4 of u32 | `flagMagicCoat` | Reflected by Magic Coat. bit 4 | [I] |
| 0x20 | bit 5 of u32 | `flagSnatch` | Stolen by Snatch. bit 5 | [I] |
| 0x20 | bit 6 of u32 | `flagMirrorMove` | Copyable by Mirror Move. bit 6 | [I] |
| 0x20 | bit 7 of u32 | `flagPunch` | Punch move. bit 7; set on 15 moves | [I] |
| 0x20 | bit 8 of u32 | `flagSound` | Sound move. bit 8; set on 18 moves | [I] |
| 0x20 | bit 9 of u32 | `flagGravity` | Disabled by Gravity. bit 9 | [I] |
| 0x20 | bit 10 of u32 | `flagDefrost` | Thaws the user. bit 10 | [I] |
| 0x20 | bit 11 of u32 | `flagDistanceTriple` | Reaches far in triple battle. bit 11 | [I] |
| 0x20 | bit 12 of u32 | `flagHeal` | Heal move (Heal Block). bit 12 | [I] |
| 0x20 | bit 13 of u32 | `flagIgnoreSubstitute` | Bypasses Substitute. bit 13 | [I] |

Enum `damageClass`: 0 status, 1 physical, 2 special. [V on Pound 1, Ember 2, Recover 0]
Observed `target` values (14): 0 (402 moves, the default single target), 1, 2, 3, 4 (12 moves, for example all adjacent), 5 (31), 6, 7 (77, the user), 8, 9, 10, 11, 12, 13. Names are not fixed by the code reviewed. [I]
Observed `priority` range -7..+5. `accuracy` 101 is read as 100. `criticalRank` 6 is read as 0.
Flags: the data confirm bits 0 (contact, 181 moves), 3 (protect, 443) and 6 (mirror move, 433); the other names are the community meanings of the same bit positions, with the counts of set bits agreeing in magnitude (charge 12, recharge 7, punch 15, sound 18). aletheia's notes call bit 5 "heal"; Recover and Softboiled also have bit 12, so bit 5 is more likely Snatch and bit 12 the heal flag. [I]

TM contents (which move a TM teaches) are not in this archive; see "ARM9 tables". Move tutor lists are a gap.

## Items

- Archive: `a/0/2/4` (id 24). Members: one member per item id; ids above 626 read as 0 (file_util.c:324) (627). Record size: 36 bytes. Compressed: no. Names: text bank 54. Source: `file_util.c:110, file_util.c:54, file_util.c:400`.

Reader: `FILE_UTIL_GetItemRecordParam` and `FILE_UTIL_GetItemExtParam` (`file_util.c:538`, structs at `file_util.c:110` and `file_util.c:54`). The record is a 16-byte header followed, when `extType` is 1 (208 of 627 items), by an 18-byte medicine block that ends at 0x22; the last 2 bytes are padding. Checked on Potion, Super Potion, Hyper Potion and Antidote. Item ids are used directly (id 0 is empty, ids above 626 read as item 0). Bag pocket selects one of five bag arrays (310, 48, 109, 64, 83 slots). [V]

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u16 | `price` | Price / 10. the game reads price * 10 (file_util.c GetItemRecordParam case 0); sell price is half | [V] |
| 0x02 | u8 | `holdEffect` | Hold effect. | [V] |
| 0x03 | u8 | `holdEffectParam` | Hold effect parameter. | [V] |
| 0x04 | u8 | `pluckEffect` | Pluck effect. | [V] |
| 0x05 | u8 | `flingEffect` | Fling effect. | [V] |
| 0x06 | u8 | `flingPower` | Fling power. | [V] |
| 0x07 | u8 | `naturalGiftPower` | Natural Gift power. | [V] |
| 0x08 | bits 0-4 of u16 | `naturalGiftType` | Natural Gift type. enum type. type id; 31 = none | [V] |
| 0x08 | bit 5 of u16 | `preventToss` | Cannot be tossed. | [V] |
| 0x08 | bit 6 of u16 | `selectable` | Selectable in bag. | [V] |
| 0x08 | bits 7-10 of u16 | `pocket` | Bag pocket. enum itemPocket (verified: 0 items/balls, 1 medicine, 2 machines, 3 berries, 4 key items) | [V] |
| 0x08 | bits 11-15 of u16 | `sortGroup` | Bag sort group. | [V] |
| 0x0A | u8 | `fieldUse` | Field use. field-use handler id (0 none, 1, 6, 7, 8, 19, 20, 21 seen) | [I] |
| 0x0B | u8 | `battleUse` | Battle use. 0 none, 1..3 seen | [I] |
| 0x0C | u8 | `extType` | Extension present. 1 = the 18-byte medicine block at 0x10 applies; 0 = byte 0x10 is a plain value | [V] |
| 0x0D | u8 | `partyUse` | Party-screen use. | [I] |
| 0x0E | u8 | `partyUseParam` | Party-screen parameter. | [I] |
| 0x0F | u8 | `partyUseOption` | Party-screen option. | [I] |
| 0x10 | u8 | `extByte` | Plain value (extType 0). read only when extType is 0 Only when extType = 0. | [V] |
| 0x10 | bit 0 of u8 | `cureSleep` | Cures sleep. Only when extType = 1. | [V] |
| 0x10 | bit 1 of u8 | `curePoison` | Cures poison. Only when extType = 1. | [V] |
| 0x10 | bit 2 of u8 | `cureBurn` | Cures burn. Only when extType = 1. | [V] |
| 0x10 | bit 3 of u8 | `cureFreeze` | Cures freeze. Only when extType = 1. | [V] |
| 0x10 | bit 4 of u8 | `cureParalysis` | Cures paralysis. Only when extType = 1. | [V] |
| 0x10 | bit 5 of u8 | `cureConfusion` | Cures confusion. Only when extType = 1. | [V] |
| 0x10 | bit 6 of u8 | `cureInfatuation` | Cures infatuation. Only when extType = 1. | [V] |
| 0x10 | bit 7 of u8 | `guardSpec` | Guard Spec.. Only when extType = 1. | [V] |
| 0x11 | bit 0 of u8 | `revive` | Revives. Only when extType = 1. | [V] |
| 0x11 | bit 1 of u8 | `reviveAll` | Revives all. Only when extType = 1. | [V] |
| 0x11 | bit 2 of u8 | `levelUp` | Raises level. Only when extType = 1. | [V] |
| 0x11 | bit 3 of u8 | `evolve` | Evolution stone effect. Only when extType = 1. | [V] |
| 0x11 | bits 4-7 of u8 | `atkStage` | Attack stage boost. Only when extType = 1. | [V] |
| 0x12 | bits 0-3 of u8 | `defStage` | Defense stage boost. Only when extType = 1. | [V] |
| 0x12 | bits 4-7 of u8 | `spAtkStage` | Sp. Atk stage boost. Only when extType = 1. | [V] |
| 0x13 | bits 0-3 of u8 | `spDefStage` | Sp. Def stage boost. Only when extType = 1. | [V] |
| 0x13 | bits 4-7 of u8 | `accStage` | Accuracy stage boost. Only when extType = 1. | [V] |
| 0x14 | bits 0-3 of u8 | `critStage` | Crit stage boost. Only when extType = 1. | [V] |
| 0x14 | bits 4-5 of u8 | `ppRestoreKind` | PP restore kind. Only when extType = 1. | [I] |
| 0x14 | bit 6 of u8 | `ppUp` | PP Up. Only when extType = 1. | [V] |
| 0x14 | bit 7 of u8 | `ppUpMax` | PP Max. Only when extType = 1. | [V] |
| 0x15 | bit 0 of u8 | `ppRestore` | Restores PP (one move). Only when extType = 1. | [V] |
| 0x15 | bit 1 of u8 | `ppRestoreAll` | Restores PP (all moves). Only when extType = 1. | [V] |
| 0x15 | bit 2 of u8 | `hpRestore` | Restores HP. Only when extType = 1. | [V] |
| 0x15 | bit 3 of u8 | `hpEvUp` | HP EV up. Only when extType = 1. | [V] |
| 0x15 | bit 4 of u8 | `atkEvUp` | Attack EV up. Only when extType = 1. | [V] |
| 0x15 | bit 5 of u8 | `defEvUp` | Defense EV up. Only when extType = 1. | [V] |
| 0x15 | bit 6 of u8 | `speedEvUp` | Speed EV up. Only when extType = 1. | [V] |
| 0x15 | bit 7 of u8 | `spAtkEvUp` | Sp. Atk EV up. Only when extType = 1. | [V] |
| 0x16 | bit 0 of u8 | `spDefEvUp` | Sp. Def EV up. Only when extType = 1. | [V] |
| 0x16 | bit 1 of u8 | `friendshipLow` | Friendship change (low tier). Only when extType = 1. | [V] |
| 0x16 | bit 2 of u8 | `friendshipMid` | Friendship change (mid tier). Only when extType = 1. | [V] |
| 0x16 | bit 3 of u8 | `friendshipHigh` | Friendship change (high tier). Only when extType = 1. | [V] |
| 0x16 | bit 4 of u8 | `evLimitIgnore` | Ignores EV cap. Only when extType = 1. | [V] |
| 0x17 | s8 | `hpEv` | HP EV change. Only when extType = 1. | [V] |
| 0x18 | s8 | `atkEv` | Attack EV change. Only when extType = 1. | [V] |
| 0x19 | s8 | `defEv` | Defense EV change. Only when extType = 1. | [V] |
| 0x1A | s8 | `speedEv` | Speed EV change. Only when extType = 1. | [V] |
| 0x1B | s8 | `spAtkEv` | Sp. Atk EV change. Only when extType = 1. | [V] |
| 0x1C | s8 | `spDefEv` | Sp. Def EV change. Only when extType = 1. | [V] |
| 0x1D | u8 | `hpRestoreAmount` | HP restored. 255 = full restore; Potion 20, Super Potion 50, Hyper Potion 200 verified in data Only when extType = 1. | [V] |
| 0x1E | u8 | `ppRestoreAmount` | PP restored. Only when extType = 1. | [V] |
| 0x1F | s8 | `friendshipDeltaLow` | Friendship delta (low). Only when extType = 1. | [V] |
| 0x20 | s8 | `friendshipDeltaMid` | Friendship delta (mid). Only when extType = 1. | [V] |
| 0x21 | s8 | `friendshipDeltaHigh` | Friendship delta (high). Only when extType = 1. | [V] |

Machine items: ids 328..419 are TM01..TM92 (0x148..0x1A3), 420..425 are HM01..HM06, 618..620 are TM93..TM95 (`FILE_UTIL_IsMachineItem`, `file_util.c:687`). Shop prices are `price * 10`; the sell price is derived at run time. Shop stock lists are in script/overlay data, not here (gap). Item descriptions are text bank 53, names bank 54.

## Learnsets, egg moves and evolutions

### Level-up learnsets

- Archive: `a/0/1/8` (id 18). Members: one member per species or form record (same index as personal data); member 0 is empty (668). Compressed: no. Names: text bank 70. Source: `poke_personal.c:139 (POKE_PERSONAL_LoadDataDirect), sodateya.c learnset walks`.

Each member is a list of (move u16, level u16) pairs ended by the pair (0xFFFF, 0xFFFF). Level 1 entries are the moves known at hatch/first meeting. The game reads it through `POKE_PERSONAL_LoadDataDirect` with the form-resolved member id (`poke_personal.c:139`), into a buffer of 26 entries; the longest member in the data has 22 moves plus the terminator. A mod that lengthens a learnset beyond 25 moves overruns that buffer in the original code. [V]

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u16 | `move` | Move. enum move. move id | [V] |
| 0x02 | u16 | `level` | Level. level at which the move is learned | [V] |

### Egg moves

- Archive: `a/1/2/3` (id 123). Members: one member per species id (member 0 empty); species only, not forms (650). Compressed: no. Names: text bank 70. Source: `sodateya.c:1304`.

Layout: `u16 count` followed by `count` u16 move ids. Members 1..649 are species; 260 of 650 have moves. Reader `SODATEYA_AddEggMoves` (`sodateya.c:1304`). [V]

### Evolutions

- Archive: `a/0/1/9` (id 19). Members: one member per species or form record (same index as personal data); member 0 is zero (668). Record size: 42 bytes. Compressed: no. Names: text bank 70. Source: `shinka_check.c:35, shinka_check.c:408`.

Seven rows of three u16. A row is unused when `species` is 0. The shinka reader returns method, param and species for a row and evaluates them in `SHINKA_CHECK_CheckEvolution` (`shinka_check.c:111-340`); the file stores 42 bytes while the in-game struct reserves 44 (a trailing pad the data omits). [V]

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u16 x7 (stride 6) | `method` | Method. enum evolutionMethod; row is unused when species is 0 | [V] |
| 0x02 | u16 x7 (stride 6) | `param` | Parameter. level, item id, move id, species id, friendship/beauty threshold, depending on method (table below) | [V] |
| 0x04 | u16 x7 (stride 6) | `species` | Target species. enum species. species id, 0 = no row | [V] |

Method ids and what `param` means (from the `switch` in `shinka_check.c`): 1 friendship (param unused); 2 friendship in daytime; 3 friendship at night; 4 level (param = level); 5 trade; 6 trade holding an item (param = item id); 7 trade against a specific partner (param unused; the code checks one fixed species pair, Karrablast and Shelmet, and that neither holds the 0x40 hold-effect item); 8 use item (param = item id); 9 level with Attack > Defense; 10 Attack = Defense; 11 Attack < Defense (param = level for 9..11); 12 and 13 level with personality below or above the split (param = level); 14 level (param = level; the Nincada to Ninjask row); 15 the row that spawns the extra Pokemon (it returns no species itself, only the method id); 16 beauty (param = threshold); 17 use item if male; 18 use item if female (param = item id); 19 hold item in daytime; 20 hold item at night (param = item id); 21 know move (param = move id); 22 species in party (param = species id); 23 level if male; 24 level if female (param = level); 25 level in a magnetic area; 26 level at the moss rock; 27 level at the ice rock; 28 level in an extra area (25..28 are matched against zone id lists in the ARM9 image). Method 4 plus param 16 for Bulbasaur (level 16 into Ivysaur) was checked against the data. [V for 1..28 as code branches; I for the descriptive names of 14 and 15]

## Trainers

### Trainer records

- Archive: `a/0/9/2` (id 92). Members: one member per trainer id (member 0 is a 16-byte sentinel) (616). Record size: 20 bytes. Compressed: no. Names: text bank 190. Source: `tpoke_data_types.h (TpokeDataRecord), tpoke_data.c:328, tr_tool.c:146`.

Reader: `TPOKE_DATA_ReadRecord` / `TPOKE_DATA_GetRecordField` (`tpoke_data.c:328`, struct `TpokeDataRecord` in `tpoke_data_types.h`). 615 records of 20 bytes plus a 16-byte sentinel. Trainer names are text bank 190 (indexed by trainer id, `tr_tool.c:146`), class names text bank 191 (105 classes). [V]

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u8 | `partyType` | Party type. bit 0 = custom moves, bit 1 = held item (0 plain, 1 moves, 2 item, 3 item and moves); selects the party entry size | [V] |
| 0x01 | u8 | `trainerClass` | Trainer class. enum trainerClass. class id, 2..102 in data; indexes the class-name bank, the class graphics table and the gender table (named genderClass in the decompilation) | [V] |
| 0x02 | u8 | `battleFormat` | Battle format. 0 single, 1 double, 2 triple, 3 rotation (values 0..3 in data; named pad02 in the decompilation) | [I] |
| 0x03 | u8 | `partyCount` | Party size. 1..6 | [V] |
| 0x04 | u16 | `item1` | Bag item 1. enum item. AI-usable battle items | [V] |
| 0x06 | u16 | `item2` | Bag item 2. enum item. | [V] |
| 0x08 | u16 | `item3` | Bag item 3. enum item. | [V] |
| 0x0A | u16 | `item4` | Bag item 4. enum item. | [V] |
| 0x0C | u32 | `aiFlags` | AI flags. battle AI bit set; common values 1, 7, 5, 0, 3, 129 | [V] |
| 0x10 | bit 0 of u8 | `flag10` | Flag at 0x10. 0 or 1 (6 records); meaning not established | [I] |
| 0x11 | u8 | `byte11` | Byte at 0x11. values 8, 10, 25 ... (looks like a prize-money factor) | [I] |
| 0x12 | u16 | `half12` | Value at 0x12. 0 except 10 records holding an item-id-sized value (151, 158) | [I] |

### Trainer parties

- Archive: `a/0/9/3` (id 93). Members: one member per trainer id, paired with the trainers table (same index) (616). Compressed: no. Names: text bank 190. Source: `tr_tool.c:84, tpoke_data.c:333`.

Reader `TR_TOOL_BuildParty` (`tr_tool.c:187`). The member holds `partyCount` entries, each of the size picked by `partyType & 3`: 0 is 8 bytes, 1 is 16 (custom moves), 2 is 10 (held item), 3 is 18 (item and moves). All 615 members match this in the data. [V]

Base entry (all types), offsets from the entry start:

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u8 | `difficulty` | Difficulty value. IV = value * 31 / 255 in all six stats (tr_tool.c) | [V] |
| 0x01 | u8 | `abilityFlags` | Ability / gender flags. low nibble gender adjust, high nibble ability slot choice (0x30 = special path) | [I] |
| 0x02 | u16 | `level` | Level. | [V] |
| 0x04 | u16 | `species` | Species. enum species. | [V] |
| 0x06 | u16 | `form` | Form. | [V] |

Type 1 adds four u16 moves at 0x08; type 2 adds a u16 item at 0x08; type 3 adds the item at 0x08 and the four moves at 0x0A. The trainer's class also selects its gender through a small table in the ARM9 image (`gTpokeDataGenderValueTable`) and its battle sprite (below).

## Wild encounters

- Archive: `a/1/2/6` (id 126). Members: one member per encounter table (112); each is 232 bytes (all seasons) or 928 bytes (four 232-byte blocks, season index 0..3) (112). Record size: 232 bytes. Compressed: no. Source: `field_encount_types.h:30, field_encount_st.c:81, eventdata_system.c:248`.

A zone header's `encountId` picks the member (0xFFFF = none). A member is one 232-byte block when it applies all year or four consecutive blocks when it varies by season; the loader divides the member size by 232, accepts only 1 or 4 and, for 4, reads block `seasonIndex` (`field_encount_st.c:81-101`). Block 3 is winter [I: Cubchoo and Vanillite appear only there]; blocks 0..2 are identical in the data. [V]

Block layout (`EncountData`, `field_encount_types.h:30`):

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u8 x7 (stride 1) | `rate` | Encounter rate (per method). index 0 grass, 1 double grass, 2 shaking grass/dust cloud, 3 surf, 4 surf ripple, 5 fishing, 6 fishing ripple | [V] |
| 0x07 | bits 0-6 of u8 | `effectKind` | Effect encounter kind. 0..2 | [V] |
| 0x07 | bit 7 of u8 | `enabled` | Enabled (set by the loader). the loader ORs 0x80 into this byte (field_encount_st.c:99) | [V] |

Slot groups (each slot is 4 bytes):

| Offset | Count | Group | Encounter rate index |
|---|---|---|---|
| 0x08 | 12 | grass | 0 |
| 0x38 | 12 | double grass (dark grass) | 1 |
| 0x68 | 12 | shaking grass / dust cloud | 2 |
| 0x98 | 5 | surf | 3 |
| 0xAC | 5 | surf ripple | 4 |
| 0xC0 | 5 | fishing | 5 |
| 0xD4 | 5 | fishing ripple | 6 |

Slot: bits 0-10 of the first u16 are the species (0 = empty), bits 11-15 the form (31 picks a random form), then `minLevel` u8 at +2 and `maxLevel` u8 at +3. [V] Which game feature each rate index and group belongs to is from the struct comment; the swarm, Hidden Grotto, roamer, static and gift encounters are not in this archive (script data, gap).

## Map headers, areas and tiles

### Zone (map) headers

- Archive: `a/0/1/2` (id 12). Members: ONE member (0) holding 427 consecutive 48-byte records; record index = zone id (1). Record size: 48 bytes. Compressed: no. Source: `zonedata.c:54, zonedata.c:385`.

All 427 records are one 20496-byte member, so a mod replaces member 0 as a whole (or patches the 48-byte record in place). Reader: `ZONEDATA_GetHeader` (`zonedata.c:385`, struct at `zonedata.c:54`). [V]

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u8 | `mapType` | Map type. | [V] |
| 0x01 | u8 | `mapSubType` | Map sub-type. | [V] |
| 0x02 | u16 | `areaId` | Area id. record index in the area table a/0/1/3 (0..273 in data) | [V] |
| 0x04 | u16 | `matrixId` | Matrix id. member of a/0/0/9 (the map matrix); 0..254 in data | [V] |
| 0x06 | u16 | `scriptArcId` | Script id. member of the script archive a/0/5/7 | [V] |
| 0x08 | u16 | `initScriptArcId` | Init script id. member of the script archive a/0/5/7 | [V] |
| 0x0A | u16 | `msgArcId` | Message bank id. message file id in a/0/0/3 | [V] |
| 0x0C | u16 | `bgmSpring` | BGM (season 0). SDAT sequence number (1000..1178 are BGM sequences) | [V] |
| 0x0E | u16 | `bgmSummer` | BGM (season 1). | [V] |
| 0x10 | u16 | `bgmAutumn` | BGM (season 2). | [V] |
| 0x12 | u16 | `bgmWinter` | BGM (season 3). | [V] |
| 0x14 | u16 | `encountId` | Wild encounter table. member of a/1/2/6; 0xFFFF = none | [V] |
| 0x16 | u16 | `eventDataId` | Event data. member of a/1/2/5 | [V] |
| 0x18 | u16 | `parentZoneId` | Parent zone. | [V] |
| 0x1A | bits 0-9 of u16 | `placeNameId` | Place name. index into text bank 89 | [V] |
| 0x1A | bits 10-15 of u16 | `placeNameWindow` | Place name window style. | [V] |
| 0x1C | bits 0-5 of u16 | `weatherId` | Weather. | [V] |
| 0x1C | bits 6-8 of u16 | `mapChangeType` | Map change type. | [V] |
| 0x1C | bits 9-15 of u16 | `cameraId` | Camera. | [V] |
| 0x1E | bits 0-4 of u16 | `expObjSetId` | Object set. | [V] |
| 0x1E | bits 5-9 of u16 | `fieldEffectSetId` | Field effect set. | [V] |
| 0x1E | bit 10 of u16 | `bikeEnabled` | Bike allowed. | [V] |
| 0x1E | bit 11 of u16 | `dashEnabled` | Running allowed. | [V] |
| 0x1E | bit 12 of u16 | `weatherEnabled` | Weather enabled. | [V] |
| 0x1E | bit 13 of u16 | `flyEnabled` | Fly allowed. | [V] |
| 0x1E | bit 14 of u16 | `rideBgmEnabled` | Bike music. | [V] |
| 0x1E | bit 15 of u16 | `palaceEnabled` | Palace allowed. | [V] |
| 0x20 | u16 | `cameraAreaId` | Camera area. | [V] |
| 0x24 | s32 | `posX` | Start X. start position; small grid-style integers in the data (5, 0, 4 for zone 1); for rail maps a rail location, see ZONEDATA_GetStartRailPos | [I] |
| 0x28 | s32 | `posY` | Start Y. | [V] |
| 0x2C | s32 | `posZ` | Start Z. | [V] |

The BGM fields are SDAT sequence numbers: the data holds values from 1000 to 1170, inside the `SEQ_BGM_*` range (1000..1178). The place name is an entry of text bank 89. The script, message, encounter and event ids index the archives `a/0/5/7`, `a/0/0/3`, `a/1/2/6` and `a/1/2/5`. [V for struct and sizes; I for the bank 89 match, which agrees with the names seen in that bank]

### Map areas

- Archive: `a/0/1/3` (id 13). Members: NOT a NARC: a flat file of 282 records, replaced as a whole file (282). Record size: 10 bytes. Compressed: no. Source: `zonedata.c:120, aletheia ASSETS.md (reader sub_0201407C)`.

Selected by the zone header's `areaId`. A flat file (not a NARC), 282 records of 10 bytes. Reader `ZONEDATA_CreateAreaData` (`zonedata.c:120`). The first record (index 0) is the empty area; records 2..209 come in four seasonal siblings (`sub_0201407C`). [V]

| Offset | Type | Name | Meaning | |
|---|---|---|---|---|
| 0x00 | u16 | `modelSetId` | Model set. 0..43 in data | [V] |
| 0x02 | u16 | `texSetId` | Texture set. equals the record index in 280 of 282 records; names the texture member of a/0/1/4 | [I] |
| 0x04 | u8 | `itaAnimeId` | Texture animation (ita). 0xFF = none | [V] |
| 0x05 | u8 | `itpAnimeId` | Palette animation (itp). 0xFF = none | [V] |
| 0x06 | u8 | `innerOuter` | Inside or outside. | [V] |
| 0x07 | u8 | `lightType` | Light type. | [V] |
| 0x08 | u8 | `edgeMarkType` | Edge mark type. | [V] |
| 0x09 | u8 | `billboardColor` | Billboard color. | [V] |

### Map matrix and map chunks (map tiles)

- `a/0/0/9` (id 9), 255 members, one per `matrixId`. Header: u16 mode, u16 pad, u16 width, u16 height (each at most 30, product at most 900); then width*height u32 chunk ids (an index into `a/0/0/8`; 0xFFFFFFFF = empty); then, if mode is 1, width*height further u32 values holding the zone ids (0xFFFFFFFF = none). Member 0 is 6272 = 8 + 783*8 bytes (29 x 27); members 1.. are 12 to 16 bytes (1x1 and 1x2 matrices, mode 0). Reader `MAP_MATRIX_Load` / `SetupFromFile` (`map_matrix.c:55-100`). [V; the u32 width of the zone-id array is inferred from the size]
- `a/0/0/8` (id 8), 649 members, the map chunks (one 3D tile block each). Each member is a small wrapper (size-table header with a 2-letter tag WB, GC, NG or RD) around a BMD0 model, then a grid of tile records, then a tail. Per-tile record (8 bytes): u16 surface (bits 0-1 surface kind, bits 2-15 normal index), u16 height index, u32 attribute (bits 0-15 terrain id, bits 16-31 flags: walkable ground, alternate encounter set, terrain-class gate). The grid opens with u16 width, u16 height and the first record is 4 bytes in. Tile records are the editable per-tile collision and terrain data; the models are 3D and not pixel-editable. The terrain id values and class switch are in aletheia `tools/assets/map_grid.py` (reader `func_021D14C4` in ov021). [V per the aletheia code trace; the wrapper tags WB/GC/NG/RD and grid offsets are described in `map_grid.py`]
- `a/0/1/4` (id 14), 282 members of BTX0 textures, member = `texSetId` of the area record. `a/0/1/1` (222 members) are the building/model sets. [I]
- Pixel-level map editing in the sense of a 2D tileset does not exist: the overworld is 3D models plus textures, with a separate per-tile attribute grid.

## Graphics

### Pokemon battle sprites (`a/0/0/4`, id 4)

`POKEGRA_GetArcID` returns 4 (`pokegra.c:73`). The archive has 14285 members. One sprite block is 20 members and the block index is the species id, with eggs at 650 (651 for the Manaphy egg) and the forms that have their own sprite at 652 + `formSpriteIndex` + form - 1 (`pokegra.c:451-496`). Member = 20 * block + role offset:

| Offset | Role | Format | Compressed |
|---|---|---|---|
| 0 | front, small tile sheet (+1 is the female variant, empty when there is none) | NCGR | LZ11 |
| 2 | front, battle bitmap (+3 female) | NCGR, 16384 bytes of 4bpp tiles | LZ11 |
| 4 | front cell bank | NCER | no |
| 5 | front cell animation | NANR | LZ11 |
| 6 | front multi-cell resource | NMCR | no |
| 7 | front multi-cell animation | NMAR | no |
| 8 | front cell header | 64 bytes, no NITRO magic | no |
| 9, 10 | back small tile sheet (male, female) | NCGR | LZ11 |
| 11, 12 | back battle bitmap (male, female) | NCGR | LZ11 |
| 13 to 17 | back cell bank, cell animation, multi-cell resource, multi-cell animation, cell header | as 4 to 8 | as 4 to 8 |
| 18 | normal palette | NCLR, 16 colors | no |
| 19 | shiny palette | NCLR | no |

The battle renderer builds its member list in `MCSS_TOOL_MakeAddWorkFromParam` (`mcss.c:3207`): texture = the offset-2 style member (`POKEGRA_GetCbrArcIndex`), palette, cell, cell animation, multi-cell, multi-cell animation and cell header. The offset-0 style member is the small tile sheet used by 2D screens (`POKEGRA_LoadCharData`). The sex argument is 0 male, 1 female (the +1 member is used only if non-empty), 2 genderless (`pokegra.c:498-512`). Palette-only forms (`formSpriteShared` = 1) use palette members 14253 + (`formSpriteIndex` + form - 1) * 2 + shiny. [V for the member arithmetic; role names from the function names; I for what the small sheet is used for]

For a pixel editor: the editable image is the offset-2 (or 11) NCGR together with the cell/multi-cell structure that arranges its tiles. The sprite is a poseable multi-part rig (cell bank plus multi-cell resource plus animation), not one flat bitmap, so editing means editing tiles and keeping the cell references valid. aletheia records that its own compositor still does not reproduce the full pose, so a faithful preview needs the NCER/NMCR/NANR path in kenon.

### Trainer battle sprites (`a/0/7/2` front, `a/0/7/3` back)

Archive 0x48 = 72 for the front view and 0x49 = 73 for the back view (`pokegra.c:530-535`). Member = 8 * graphic + role, with roles 0 small tile sheet (NCGR, LZ11), 1 battle bitmap (NCGR, LZ11), 2 cell bank, 3 cell animation (LZ11), 4 multi-cell resource, 5 multi-cell animation, 6 cell header, 7 palette. For the front archive `graphic` = `TrainerGraIndexTable[trainerClass]` (u8 table, 106 entries, ARM9 0x020A023C; class ids up to 0x69, 95 distinct graphics in 760 members); for the back archive the graphic index is passed directly (6 graphics: the player characters, Juniper, the rivals, per aletheia). [V]

### Pokemon icons (`a/0/0/7`, id 7)

1431 members. Member 0 is a shared palette of three 16-color sub-palettes (NCLR); members 1..6 are the shared animation and cell files. Icon tiles are at 7 + 2n for species n (n = 650 egg, 651 Manaphy egg, forms with their own icon at (`formSpriteIndex` + form - 1 + 652)), and the odd member after each is the female variant (size 0 when absent). Each NCGR is 1072 bytes: 1024 bytes of 4bpp tiles, which is two 32x32 frames (the bounce animation), uncompressed. The sub-palette is chosen from a nibble table in the ARM9 image (low nibble male, high nibble female, index = species). The game copies 0x200 bytes (one 32x32 frame) into the box icon. `POKE_TOOL_CalcGraphicIndex` / `POKE_TOOL_CalcIconPalette` (`poke_tool_ext.c:352-470`, `worldtrade_box.c:1303`). [V; "two frames" is I]

The separate archive `a/1/6/7` (655 members, 650 LZ77 icons of 16x16 4bpp, monochrome palette) is a different small icon set used by another screen; its consumer is not identified (gap).

### Item icons (`a/0/2/5`, id 25)

1005 members. Member 0 (NANR) and 1 (NCER) are shared. For item `i`, the tile member is `item_gfx_member[i].lo` and the palette member is `.hi`, a u16 pair table of 627 entries in the ARM9 image at 0x0209EB6C (item 0 and 0xFFFF use 1001/1002 and 1003/1004). Tiles are NCGR with 512 bytes of 4bpp data (one 32x32 icon), palettes NCLR, both raw. Several items share an icon (the pair repeats). `FILE_UTIL_LoadItemRecord` (`file_util.c:395-410`). [V]

### Overworld characters and objects

Field NPCs and objects are not sprite sheets keyed by character. Each placed object has an object code; `a/0/4/8` (id 48) is a single member holding a u16 count (799) and 799 parameter records of 28 bytes (`MmdlObjCodeParam`, `fldmmdl_types.h:32`): draw type, shadow and footprint sizes, offsets, and 12 bytes of resource ids that select models, textures and animations in `a/0/4/9` (id 49, 764 members: BMD0, BTX0 and animation files). The fields are in the schema file. The mapping from the three resource ids to `a/0/4/9` members was not independently confirmed. aletheia's 2D sprite archives `a/0/3/1` and `a/0/3/2` (walk-cycle character sheets) were identified by picture only and have no confirmed reader in the decompiled code; treat them as a gap. [V for `a/0/4/8` layout; I for the resource semantics]

### Other graphics archives used by the UI

Type, move-category and ball icons, window frames, the touch bar and C-Gear are in `a/0/8/3`-style UI archives, with `APP_ICON_GetArchiveId` = 0x53 (`a/0/8/3`) for the common icon file (`app_icon.c:54`). kenon's `docs/ui-assets-inventory.md` covers these.

## Sound

All sound is in `wb_sound_data.sdat`, a whole file of about 51 MB (not a NARC): 2076 sequences, 1986 banks, 1986 wave archives, 7 players, no streams. The SYMB block names 940 sequences. [V: aletheia sdat.py summary]

- **Music.** BGM sequences are numbered 1000..1178 (`SEQ_BGM_*`, 179). The zone header carries four per zone (one per season). Jingles (`SEQ_ME_*`) are 1300..1331. [V for the zone-to-sequence mapping (data values 1000..1170); the use of 1300 as the jingle block is the naming range]
- **Sound effects.** `SEQ_SE_*` is 1350..2075 (726 named). [I as a naming range]
- **Cries.** Wave archive number = cry index, counted from 1 (`PM_VOICE_GetCryIndex`, `pm_voice.c:772`: species id for 1..649, 650 for Sky Forme Shaymin which is species 492 form 1; anything outside 1..649 plays species 1). The wave comes from `NNS_SndArcGetWaveArcInfo(cryIndex)` and the file read (`pm_voice.c:442-481`). Wave archive names are `WAVE_ARC_PV###` for the first 494 and `WAVE_ARC_PMWB_###` for the rest, with 650 named `..._SKY`. Each archive holds one wave; the sample rate and loop come from the wave record. Playback parameters are per species in `a/1/9/4` (id 194, 652 members of 4 bytes: u16 rate-like value, u8 volume 97..127, u8 pad). The Chatot cry (species 441) can be replaced by the player's recorded sample (`pm_voice.c:801-815`). [V]
- kenon's `core/src/sdat` reads the container, sequences, banks and wave archives and can build a new SDAT, so an edit writes `wb_sound_data.sdat` as a whole file.

## Text

Message banks are members of `a/0/0/2` (id 2, 288 members, decoded by kenon's `msg.rs`). `MSGDATA_CreateFromArchive(0, 2, bank, heap)` opens bank `bank` (`msgdata.c:121`). Another archive, `a/0/0/3` (id 3), holds the map and script message banks that zone headers select with `msgArcId`. Each bank is a list of strings; the entry index is the id used by the game (species id, move id, ...). Counts below include entry 0.

| Content | Bank | Entries | |
|---|---|---|---|
| species names, index = species id | 70 | 652 | [V] |
| same in capitals | 284 | 652 | [I] |
| move names, index = move id | 203 | 560 | [V] |
| move descriptions | 202 | 560 | [I] |
| capitals | 286 | 560 | [I] |
| item names, index = item id | 54 | 627 | [V] |
| item descriptions | 53 | 627 | [I] |
| article and name templates | 279 | 627 | [I] |
| plural names | 280 | 627 | [I] |
| ability names, index = ability id | 182 | 165 | [I] |
| ability descriptions | 183 | 165 | [I] |
| capitals | 285 | 165 | [I] |
| type names, index = type id (17 entries, 0 Normal ... 16 Dark) | 199 | 17 | [V] |
| capitals | 287 | 17 | [I] |
| nature names | 24 | 25 | [I] |
| trainer names, index = trainer id (tr_tool.c:146) | 190 | 616 | [V] |
| trainer class names, index = class id | 191 | 105 | [V] |
| article forms | 283 | 105 | [I] |
| place names, zone header placeNameId (10 bits) indexes this bank | 89 | 117 | [V] |
| Pokedex flavor text (banks 235..243 are the per-version and per-language copies) | 235 | 650 | [I] |
| species category text, for example 'Seed Pokemon' style; banks 260..267 per language | 260 | 650 | [I] |
| height strings (245..252 per unit and language) | 244 | 727 | [I] |
| weight strings (269..275 per unit and language) | 268 | 727 | [I] |

Notes. The species, move, item, type, trainer and place-name banks are tied to code or to the table sizes (species 652, moves 560, items 627, types 17, trainer classes 105 matching the class ids 2..102, trainers 616 matching the trainer table). Banks 100..125 are letter-grouped word lists for the phrase input (Easy Chat), not data text. Several of the banks between 235 and 275 are the same species-indexed text in other languages or units (the English and the non-English copies are all present in the data). Strings use the control codes `0xFFFE` newline, `0xF000` escape (kind, argument count, arguments) and 0xFFFF terminator, enciphered per string (`msgdata.c`, aletheia `text.py`). [V for the code-tied banks; I for the rest]

## ARM9 tables

Data in the ARM9 image, not in a NARC. Addresses are Black addresses; `extract/arm9_dec.bin` is loaded at 0x02004000 (file offset = address - 0x02004000). A mod changes these through the port's override mechanism or an ARM9 patch, not through a member override.

| Table | Address | Shape | Use |
|---|---|---|---|
| TM and HM moves | 0x0209EAA0 | u16 x 101 | machine n teaches this move (index 0..91 TM01..92, 92..97 HM01..06, 98..100 TM93..95); `FILE_UTIL_GetMachineMove` [V] |
| Item icon map | 0x0209EB6C | (u16, u16) x 627 | item to (tile member, palette member) in `a/0/2/5` [V] |
| Trainer class to sprite | 0x020A023C | u8 x 106 | class to graphic block in `a/0/7/2` [V] |
| Icon palette selector | 0x0209E474 | nibble pairs, one byte per icon index | low nibble male, high nibble female sub-palette in `a/0/0/7` [V] |
| Trainer class gender | `gTpokeDataGenderValueTable` | u8 per class | gender of the trainer class (address not reviewed) [I] |

## Other useful archives

- **Battle facility.** `a/2/1/4` (id 214): 1000 records of 16 bytes (species u16, four u16 moves, EV spread mask u8, nature u8, item u16, form u16), the Pokemon sets of the battle facilities; `a/2/1/5` (id 215): 315 group lists `[u16 group][u16 count][u16 set id * count]` indexing `a/2/1/4`. The reader is `fld_btl_inst_tool.c` in ov010. `a/1/6/9` and `a/1/6/8` have the same shapes but no reader in the image. [V for 214/215]
- **Cry parameters** `a/1/9/4`, see Sound. [V]
- **Species base index** `a/0/2/0` (id 20): 650 members of one u16 each, read as an evolution-family base species index by an ov021 function; the meaning is not pinned. [I]
- **Event data** `a/1/2/5` (id 125): 428 members, per-zone event tables (a u32 offset to a trailing block, four u8 counts, then arrays of background events 20, NPCs 36, warps 20 and position events 22 bytes). Layouts and the trainer-to-location rule are in "Event data and trainer locations" and in `schemas.json` (`eventData`, `trainerLocation`). [V]
- **Scripts** `a/0/5/7` (id 57): 899 members of field script bytecode; the instruction set is documented in aletheia `scripts.py` and eidos. Dialogue is in `a/0/0/3`. Gift, static, starter and shop contents live in scripts and overlay data. [V that scripts are here; contents not mapped]
- **Battle visual effect scripts** `a/0/6/6` and `a/0/6/7` (ids 66, 67): bytecode run by the battle effect VM (`btlv_effvm.c:890`, `:899`); 601 and 98 members. Which of these a move's `effectSequence` selects is not established. [V that the VM loads them; I for the rest]
- **Fonts**: `gfl_font.dat`; kenon reads it with `nftr.rs`.
- **Title demo and cinematic frames**: `a/1/3/4` is a 670-frame opening cinematic (256x256 16bpp LZ11 frames), not sprites.

## eidos data-table catalogue

`docs/mod-events.md` (appendix "data tables") lists 91 tables a mod replaces through the data layer. Mapping of the main ones to this document: wild encounters, fishing, surfing, swarm (Wild encounters; swarm data separate); base stats, types, abilities, hidden ability, catch rate, EV yield (Pokemon stats); evolution target and method (Evolutions); move properties and physical/special split (Moves, damage class); level-up learnset, egg moves (Learnsets and egg moves); TM contents (ARM9 table), TM and HM compatibility (personal flags), move tutor moves (gap); trainer party species, level, held items, movesets, AI flags, class, battle format (Trainers); shop price (Items `price`), ball modifier and held item effect (item hold effect, code); wild held item table (personal held items); area music (zone `bgm` fields); pokemon sprites, icons, trainer sprites, cries, music, SE (Graphics and Sound); message text (Text). Tables with no archive layout yet: hidden grotto, static and gift Pokemon, starter choice, in-game trade, ground items and hidden items (scripts and event data), shop inventory and the exchange corner, type chart, level cap, ban list.

## Event data and trainer locations

Everything here was read from the aletheia decompilation (source paths below are relative to its `src/game`) and cross-checked against the extracted archives (all 427 event files, the 899 script members, the 615 trainer records). Sources are given as `file:line`. [V] = verified in code and data, [I] = inferred.

### Zone to event file to place name

- A zone header (`a/0/1/2`, 48 bytes per zone) holds `scriptArcId` at 0x06, `eventDataId` at 0x16 and `placeNameId` in bits 0-9 of the u16 at 0x1A (`arm9/zonedata.c:54`). The event file of a zone is member `eventDataId` of `a/1/2/5` (id 125): the event system opens archive 0x7D (`ov010/eventdata_system.c:247`) and reads the member named by `ZONEDATA_GetEventDataId` (`ov010/eventdata_system.c:362`). [V]
- In the data all 427 zones have distinct `eventDataId` values (0..426), so a zone and its event file are one to one. Member 427 of the archive is a 4-byte sentinel that no zone uses. [V, data]
- Place names are entries of text bank 89 of `a/0/0/2` (117 entries). The field place-name popup opens it with `MSGDATA_CreateFromArchive(0, 2, 0x59, ...)` (`ov021/field_place_name.c:1081`; 0x59 is 89) and indexes it with `ZONEDATA_GetPlaceNameId` (`arm9/zonedata.c:1023`). The data uses ids 0..116 (111 distinct values; 3 zones have id 0). Reading the bank confirms the entries are place names (the zones of Black City, Driftveil City, Twist Mountain and Giant Chasm resolve to those names). One quirk in the Black build: ids 0x70 and 0x71 both display entry 0x70 (`arm9/zonedata.c:1023-1031`). [V]
- Many zones share one place name (the rooms and floors of one building have the same `placeNameId`). A trainer's place is therefore the place name of the zone whose event file holds the NPC. [V]

### Event file layout (`a/1/2/5` member)

All integers are little-endian. The loader reads the whole member and splits it into four arrays that follow one another (`ov010/eventdata_system.c:362-404`, struct `EventFileData` at `:48`). [V]

| Offset | Type | Name | Meaning |
|---|---|---|---|
| 0x00 | u32 | `trailerOffset` | Offset of the trailer, counted from offset 4 of the file. The trailer starts at `4 + trailerOffset` and runs to the end of the file (the `specialScripts` pointer, `:376`). |
| 0x04 | u8 | `bgCount` | Number of background events (signs, hidden items). |
| 0x05 | u8 | `npcCount` | Number of NPC (object) records. |
| 0x06 | u8 | `connectCount` | Number of warps (connections). |
| 0x07 | u8 | `posCount` | Number of position (trigger) events. |
| 0x08 | | arrays | `bgCount` records of 0x14 bytes, then `npcCount` of 0x24, then `connectCount` of 0x14, then `posCount` of 0x16. |

Check on the data: `8 + 20*bg + 36*npc + 20*connect + 22*pos` equals `4 + trailerOffset` in all 428 members (0 exceptions). The trailer is 4 bytes in 266 files and a few dozen bytes at most elsewhere; the game keeps a pointer to it (`EVENTDATA_SYSTEM_GetSpecialScriptData`) and its contents are not decoded here. [V for the split; the trailer is a gap]

Totals in the game: 662 background events, 2257 NPCs, 886 warps, 373 position events. [V, data]

Coordinate units. NPC, background and position events use grid cells (one cell is one tile of the 32 by 32 map chunk). Positions are in the zone's whole map matrix, not per chunk: the outdoor matrix 0 is 29 by 27 chunks and NPC `gridX` there reaches 818 and `gridZ` 764; 2215 of the 2217 grid NPCs lie inside `32 * matrixWidth` by `32 * matrixHeight` of their zone's matrix (`a/0/0/9`). The chunk of an NPC is `(gridX / 32, gridZ / 32)`; in a mode 1 matrix the zone id of that chunk can differ from the zone that owns the event file. Warps are the exception: their x, y and z are in 1/16 tile (tile = value >> 4; the hit tests compare `pos >> 12`, `:1051-1067`, and 806 of 809 grid warps fall inside the matrix when divided by 16). Heights: NPC `posY` is 16.16 fixed point, tile height = `posY >> 16` (`ov010/fldmmdl.c:585-610`; data range -6 to 9, 1623 of 2217 at 0). [V]

### NPC (object) record, 0x24 bytes (`MmdlHeader`, `ov010/fldmmdl_types.h:52`)

| Offset | Type | Name | Meaning |
|---|---|---|---|
| 0x00 | u16 | `objId` | Object id of the NPC inside the zone (the id scripts use to address it). [V] |
| 0x02 | u16 | `objCode` | Object code (appearance); index into the 799 parameter records of `a/0/4/8` (`MmdlObjCodeParam`). [V] |
| 0x04 | u16 | `moveCode` | Movement behaviour code. [V] |
| 0x06 | u16 | `eventType` | How the object reacts. 1 is a trainer that sees the player; 4 to 8 and 11 to 13 count as 1 (`FLDMMDL_NormalizeEventType`, `ov010/fldmmdl.c:2178`); 0 is a normal NPC; 2, 10, 19, 20 occur on a few special objects. [V for the normalisation, I for the meaning of 2, 10, 19, 20] |
| 0x08 | u16 | `eventFlag` | Event flag tied to the object (appearance condition). [I] |
| 0x0A | u16 | `eventId` | Script id run when the player talks to the object or it sees the player. For trainers it encodes the trainer id, see below. [V] |
| 0x0C | u16 | `dirHeader` | Initial facing. [V] |
| 0x0E | u16 | `param0` | Movement parameter. [V] |
| 0x10 | u16 | `param1` | Movement parameter. [V] |
| 0x12 | u16 | `param2` | Movement parameter. [V] |
| 0x14 | s16 | `moveLimitX` | Movement range in x. [V] |
| 0x16 | s16 | `moveLimitZ` | Movement range in z. [V] |
| 0x18 | s32 | `posType` | 0 = grid position (2217 records), other = rail position (40). [V] |
| 0x1C | u16 | `gridX` | Grid x (when `posType` is 0). [V] |
| 0x1E | u16 | `gridZ` | Grid z (the second horizontal axis; the game names it z). [V] |
| 0x20 | s32 | `posY` | Height, 16.16 fixed point. [V] |

For `posType` 1 the same bytes are `railIndex` (u16 at 0x1C), `lineGrid` (u16 at 0x1E) and `widthGrid` (s16 at 0x20), read by `FLDMMDL_MOVE_RequestRailMove` (`ov010/fldmmdl_move.c:1512`). Rail records have no tile on the chunk grid. [V]

### Warp (connect) record, 0x14 bytes (`ConnectData`, `ov010/eventdata_system.c:82`)

| Offset | Type | Name | Meaning |
|---|---|---|---|
| 0x00 | u16 | `zoneId` | Destination zone, 0xFFFF when disabled. All 886 data values are valid zone ids or 0xFFFF. [V] |
| 0x02 | u16 | `exitId` | Destination exit (0x100 marks a special exit). [V] |
| 0x04 | u8 | `exitDir` | Exit direction 0 to 4. [V] |
| 0x05 | u8 | `exitType` | Exit type. [V] |
| 0x06 | u16 | `posType` | 0 grid (809), 1 rail (77). [V] |
| 0x08 | s16 | `x` | Grid area x, 1/16 tile. [V] |
| 0x0A | s16 | `y` | Height, 1/16 tile. [V] |
| 0x0C | s16 | `z` | Grid area z, 1/16 tile. [V] |
| 0x0E | u16 | `sizeX` | Width of the trigger area in tiles. [V] |
| 0x10 | u16 | `sizeZ` | Depth of the trigger area in tiles. [V] |

For `posType` 1 the area words are `railIndex` (0x08), `lineGrid` (0x0A), `widthGrid` (0x0C), `lineSize` (0x0E), `widthSize` (0x10) and `flipMode` (0x12). [V]

### Position event (trigger) record, 0x16 bytes (`EventRec`, `ov010/eventdata_system.c:53`)

| Offset | Type | Name | Meaning |
|---|---|---|---|
| 0x00 | u16 | `scriptId` | Script id started by the trigger. [V] |
| 0x02 | u16 | `workValue` | The trigger fires when the event work variable equals this. [V] |
| 0x04 | u16 | `workId` | Event work variable to compare; 0 means no check. [V] |
| 0x06 | u16 | `checkType` | 1 to 4 fire when the player faces up, down, left, right (`EVENTDATA_SYSTEM_DirToPosEventType`, `:650`); 6 fires on stepping in; 0 also occurs (232 of 373). [V for 1 to 4 and 6, I for 0] |
| 0x08 | u16 | `posType` | 0 grid (363), 1 rail (10). [V] |
| 0x0A | u16 | `x` | Area x in tiles. [V] |
| 0x0C | u16 | `z` | Area z in tiles. [V] |
| 0x0E | u16 | `sizeX` | Area width in tiles. [V] |
| 0x10 | u16 | `sizeZ` | Area depth in tiles. [V] |
| 0x12 | s16 | `y` | Height, 1/16 tile. [V] |
| 0x14 | u16 | spare | Unused. [I] |

The hit test is `IsPosEventGridHit` (`:1157-1173`). [V]

### Background event (sign) record, 0x14 bytes (`BgEventData`, `ov010/eventdata_system.c:109`)

| Offset | Type | Name | Meaning |
|---|---|---|---|
| 0x00 | u16 | `scriptId` | Script id run when the player interacts. [V] |
| 0x02 | u16 | `type` | 2 = hidden item (184; the script id is the hidden item id, `:781`); 0 (351) and 1 (127) are other interactables such as signs. [V for 2, I for 0 and 1] |
| 0x04 | u16 | `dir` | Which facings fire it: 0 up, 1 down, 2 left, 3 right, 4 any, 5 left or right, 6 up or down (`:786-816`). [V] |
| 0x06 | u16 | `posType` | 0 grid (637), 1 rail (25). [V] |
| 0x08 | s32 | `x` | Grid x (tile). [V] |
| 0x0C | s32 | `z` | Grid z (tile). [V] |
| 0x10 | s32 | `y` | Height, 1/16 tile. [V] |

For `posType` 1: `railIndex` u16 at 0x08, `lineGrid` u16 at 0x0A, `widthGrid` s16 at 0x0C. [V]

Correction to the older aletheia notes (`tables.py`, ASSETS.md): they label the four arrays warp (20), NPC (36), trigger (20), unknown (22). The real order is background (20), NPC (36), warp (20), position (22). Sizes and counts agree; only the labels and the guessed NPC x/y offsets differed. [V]

### Trainer id to NPC rule

1. **Script id range.** `EVENT_TRAINER_GetPrimaryScriptId(id)` returns `id + 3000` and `EVENT_TRAINER_GetSecondaryScriptId(id)` returns `id + 5000`; `EVENT_TRAINER_GetTrainerIdFromScriptId(s)` returns `s - 3000` below 5000 and `s - 5000` otherwise (`ov010/event_trainer.c:57-69`). The trainer-eye code takes a trainer NPC's trainer id from `FLDMMDL_GetEventId`, which is the NPC record's `eventId` at 0x0A (`ov021/event_trainer_eye.c:683`). So an NPC with `eventId` in 3000..4999 is the trainer `eventId - 3000`. [V]
2. **Secondary ids.** `eventId` 5000..6999 is the same trainer id as the second member of a pair. The script range table sends both ranges to the same script member (`a/0/5/7` member 863; range table read by `SCRIPT_SYS_GetScriptFileId`, `ov010/script_sys.c:624`, rows read from the overlay 10 image at 0x021701A8). In the data 14 NPCs use ids in 5000..6999 and each such trainer id also has a 3000-range NPC, so no trainer is placed only by a secondary id. [V]
3. **The battle itself** is not named in the script. The shared trainer script (member 863) asks for the trainer id with `SCRCMD_GetScriptTrainerId` (opcode 132, `ov021/scrcmd_trainer_audio_item.c:331`) and passes it to the battle command (opcode 133, `:349`) through script variables (the operands in member 863 are 0x8020 and up). This is why trainer NPCs carry no trainer id in bytecode. [V]
4. **Defeated flag.** The "trainer beaten" event flag is `1420 + trainerId` (`ov010/event_trainer.c:88`). [V]
5. **Scripted battles.** Gym leaders, rivals, bosses and some others start battles from a zone's own script with opcode 133 (trainer, second, third; `:349-368`), opcode 134 (multi battle: ally first, then up to three opponents, `:369`) or opcode 148 (battle with a Pokemon, `:497`). Operand values below 0x4000 are literals; larger values are variables (`SCRIPT_WORK_GetWorkValue`, `ov010/script_work.c:300`). To locate them, scan the zone's script member (`scriptArcId`, `a/0/5/7`) for those opcodes. The numbers are the indexes of `SCRCMD_StartTrainerBattle`, `SCRCMD_StartMultiTrainerBattle` and `SCRCMD_StartTrainerBattleWithMon` in the 609-entry handler table at 0x0217062C of overlay 10. [V]
6. **Linking a scripted battle to a tile.** Inside a zone's own script member, entry `k` is local script id `k + 1` (`SCRIPT_SYS_GetScriptFileId` returns `scriptId - 1` for local scripts, `ov010/script_sys.c:624-650`). An NPC (`eventId`), position event (`scriptId`) or background event (`scriptId`) of the same zone with that script id is what starts it. [V for the id arithmetic, I for the chain]

### Coverage (615 trainers, ids 1 to 615)

- NPC rule (3000 + id or 5000 + id): **327** trainers placed. 341 NPC records have a 3000-range id and 14 have an id in 5000..6999. 326 trainer ids have exactly one NPC; trainer 363 has 15 NPCs in 5 zones. [V, data]
- Script scan (a linear sweep of the script members that are a zone's own script, looking for opcodes 133, 134, 148 with literal operands): **119** trainers, one of them also an NPC trainer. Of these 119, **111** have a script id that an event of the same zone points at, and 63 of those point at an NPC record (giving a tile); 48 more only have a position event (a rectangle). [I: the sweep stops 323 times at bytes it cannot size and can miss battles reached through subroutines]
- Total placed in at least one zone: **445 of 615** (72.4%). With a tile from an NPC record: **389**. Trainers in more than one zone: trainer 363 (5 zones) and trainer 1 (2 zones).
- Zones holding at least one placed trainer: 117 of 427.
- Nine battle operands are variables (opcode 133 in members 0, 104, 863 and 896): shared helper scripts that a static scan cannot resolve. [V]

### Not placed (170 trainers)

Ids: 5-6, 43, 53-55, 78-80, 82-83, 96, 126, 129-130, 132, 136, 146-147, 149, 169, 171-177, 185, 189, 191-192, 197, 204-211, 219-220, 232, 268, 273, 277, 280-286, 315-316, 320-321, 325-326, 330-331, 335-336, 339, 341, 343, 347, 349, 354-362, 364-388, 391-400, 406, 461-490, 497, 510-512, 521-525, 527, 534-538, 543, 547-548, 558-562, 597, 599, 614.

They have no NPC with a 3000 or 5000 script id and no literal battle operand in any zone script. 96 of the 170 sit in the blocks 354 to 400, 461 to 490 and 510 to 562, which hold the trainer classes of the battle facilities and tournaments (backpackers, clerks, school kids, doctors and nurses, ace trainers, veterans, the Riches; the two Subway Bosses are 406 and 614). Facility code chooses those at runtime from tables, so they are not placed on a map. [I] The other 74 (for example 5-6, 43, 53-55, 78-83, 126, 129-136, 146-149, 169-177, 185-211, 219-220, 232) are probably fights started with variable operands or through subroutines, or minigame and Join Avenue trainers. They could not be tied to a zone from static data and are a gap. [I]

### Using this for the trainer editor

1. For trainer `t`: find NPCs with `eventId == 3000 + t` or `5000 + t` in every event file; the owning zone is the zone whose `eventDataId` is that member.
2. Zone name: text bank 89, entry `placeNameId` of that zone header. Tile: `(gridX, gridZ)` of the NPC; chunk `(gridX / 32, gridZ / 32)` in the zone's matrix.
3. For scripted trainers: scan `a/0/5/7` member `scriptArcId` (or `scriptArcId + 1`: exactly one member of that pair has an entry table and is the script) for opcodes 133, 134, 148 and read the literal operands.
4. Show "not placed on a map" for the rest.

## Gaps

1. **Move tutors, TM shop and shop stock, static/gift/starter/trade/hidden grotto/hidden item contents**: stored in scripts or overlay data; not located as tables. The type chart is also in code/overlay data (no archive).
2. **Wild encounter details**: the swarm table, roamers, Hidden Grotto and dust cloud special species are outside `a/1/2/6`. Season order of blocks 0..2 is unknown (they are identical); only block 3 is winter by data.
3. **Item fields**: `fieldUse`, `battleUse`, `partyUse*`, `holdEffect` ids, `pluckEffect`, `flingEffect` have no enum; the tail 2 bytes are padding. The `extType` = 0 case holds one plain byte.
4. **Move fields**: `effectCategory`, `target`, `sickContType` and `rankType` meanings are not enumerated; flag bit names are inferred.
5. **Personal data**: `yieldFlag`, `spriteFlip`, `escapeRate` semantics; the member 668 form table mapping; `tutorFlags` (no tutor list located); `evoStage` rule.
6. **Trainer record**: bytes at 0x10, 0x11 and 0x12 and the `battleFormat` byte names are inferred; `abilityFlags` in party entries only partly decoded.
7. **Graphics**: pixel layout of the battle bitmap (member offset 2) inside the cell/multi-cell rig: aletheia could not yet compose a recognisable pose, so the NCER/NMCR/NANR relationship is documented but not independently confirmed here. The 64-byte "cell header" members have no decoder. No confirmed 2D overworld character sprite archive (a/0/3/x sheets are unconfirmed); overworld objects are 3D.
8. **Map data**: terrain id enum, the surface/height tables (live in overlay 21, not in the archive), a/0/1/1 model set indexing, and the use of `texSetId` are partly inferred. Scripts `a/0/5/7` have no schema here; the event data trailer (the bytes after the four arrays) is not decoded.
9. **Sound**: sequence names beyond the SYMB block, the unnamed sequences, and the mapping of in-game sound ids to sequences outside zone BGM and cries (battle music per trainer class, fanfares) are in code and not mapped.
10. **Text**: bank ids for items, abilities, moves, types and natures are checked against counts and content, not against every code reference; the Pokedex language/version bank ranges are estimated.
11. **White version**: all values here are Black. The decompilation notes (poke_personal.c "version sensitivity") say some member ids and counts (668, 441, 652) may differ in other versions; the archive paths are the same for White.
