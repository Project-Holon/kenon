# Challenge needs

What mods need to run the popular Pokemon challenges, and what eidos offers today. Mods need general functions (reads and actions), not only events. A level cap per badge is the model case: the mod must read the badge count and the player's levels, and write the experience.

Scope: Pokemon Black and White. "eidos" means the API in `sdk/mod_api.h`, `sdk/mod_state.h`, `docs/mod-*.md` and the rule names in `sdk/mod_rule.c`. "aletheia" means the decompiled game source, `src/game`.

## 1. Sources

Rules were read from these pages. Where a page failed to load or a rule set was not found, the row says so.

- Nuzlocke University, rules: https://nuzlockeuniversity.ca/nuzlocke-rules/
- Nuzlocke University, variants: https://nuzlockeuniversity.ca/nuzlocke-variants/
- Nuzlocke University, Soul Link: https://nuzlockeuniversity.ca/nuzlocke-variants/soul-link-nuzlocke-rules/
- Nuzlocke University, 51 optional rules: https://nuzlockeuniversity.ca/2025/03/09/51-optional-nuzlocke-rules-to-customize-your-challenge/
- Smogon, introduction to Nuzlockes and challenges (Hardcore, level cap, Egglocke, Wonderlocke, Solo, Monotype): https://www.smogon.com/articles/introduction-nuzlockes
- Search results quoting Bulbapedia and community rule sets for dupes, shiny, static and gift clauses: https://m.bulbapedia.bulbagarden.net/wiki/Nuzlocke (the Bulbapedia page itself returned HTTP 403), https://pokemondb.net/pokebase/359664/what-is-exactly-a-nuzzlocke
- Wedlocke: https://bulbapedia.bulbagarden.net/wiki/User:MetalMetroid997/Wedlocke_Challenge and https://www.pokecommunity.com/threads/474651 (search snippets only)
- Kaizo Ironmon: https://bulbagarden.net/threads/a-new-version-of-the-popular-ironmon-challenge-dubbed-super-kaizo-debuts-on-twitch-streamer-and-ironmon-creator-iateyourpies-channel.301057/ (search snippets only), https://gonintendo.com/comments/14858
- Scalemons: https://www.smogon.com/articles/om-spotlight-metagamiate-scalemons
- Black/White gym ace levels: https://pokemondb.net/black-white/gymleaders-elitefour
- eidos's own list of modes: `docs/mod-modes.md` (it cites Nuzlocke University variants, Poketools and Bulbapedia).

Not found: no source for "Sisterlocke" exists in the results. Its rules are unverified, so it is not in the table. The nearest finds were Wedlocke and Setlocke (at most two sets of three Pokemon in the party). Re-check the name with the user before adding it.

Rule facts used below:

- Nuzlocke: catch only the first wild Pokemon of each area; a fainted Pokemon is dead (box or release it); nickname every catch. A whiteout ends the run.
- Dupes clause: a species already owned does not count, the next encounter does. Some sets cap the number of skips and some extend it to the evolution line.
- Shiny clause: a shiny may always be caught and does not use the area's encounter.
- Static/gift: static encounters are exempt from first-encounter; gifts are optional "free" encounters (Gift clause) or banned (optional rule 15).
- Hardcore: no healing items in battle, set mode, level caps. Cap = the highest level of the next boss's team. Going over during a battle is allowed.
- Optional rules from the 51-rule list that matter here: one use per Pokemon Center, ban Pokemon Centers, limit healing items, level cap plus N, team size equals the leader's team size, ban TMs, ban held items, ban types, Monolocke, Uniquelocke, single stage only, boxed Pokemon are never reused, deathless restart.
- Soul Link: two players, same game. First encounters per route are linked. A failed encounter, a faint or a boxing on one side is copied to the partner.
- Wedlocke: male and female pairs; partners only switch with each other.
- Egglocke: replace each catch with an egg; Wonderlocke: trade each catch for a random Pokemon; Randomizer Nuzlocke: Nuzlocke on randomized data; Solo: one Pokemon; Monotype: every team member shares a type.
- Kaizo Ironmon: one Pokemon, randomized moves and items, enemy levels raised by 50 percent, BST 600+ banned, a loss restarts the run with a new seed. Wild Pokemon may not be killed.
- Scalemons: non-HP base stats scaled so the BST is 600: stat * (600 - HP) / (BST - HP), floored.
- Black/White ace levels (Black version): Striaton 14, Nacrene 20, Castelia 23, Nimbasa 27, Driftveil 31, Mistralton 35, Icirrus 39, Opelucid 43, Elite Four 50 (Grimsley, Shauntal, Caitlin, Marshal), Alder 77.

## 2. What eidos offers today

Events with a refused outcome (`docs/mod-events.md`): `trainer.spotted`, `npc.talk`, `menu.open`, `field.move`, `pc.open`, `encounter.wild`, `mon.evolve`, `move.learn` (level-up, daycare, script), `mon.release`, `box.deposit`, `box.withdraw`, `party.swap`, `trade.ingame`, `battle.action` (fight, run, switch), `battle.item`, `battle.escape`, `catch.attempt`, `item.use`, `shop.buy`, `heal`. Modifiable payload fields: encounter species, level and shiny; `mon.exp.amount`; `mon.evolve.to`; `battle.start` format and `level_adjust`; `battle.damage.amount`; `catch.attempt.rate`; `heal.mask`; `shop.buy.price`; `money.change.delta`; `item.add.count`; `map.warp` destination; `menu.build` entries.

State calls (`BwModState`): party, box and battle Pokemon refs with `get` and `set` by the game's own field id; `species_value`; `species_shiny`; `dex_caught`; `badge_count`, `badge_has`; `money`, `money_set`; `flag`, `flag_set`; `zone`, `player_pos`; `battle_style`; `mon_to_box`, `mon_take_item`; api 4: `mon_release`, `item_count`, `item_add` (queued moves now really applied).

Services: `message`, `confirm`, peer sessions (host, join, send, `on_session`), `mon_export`, `mon_import`, panel, timer and splits, `run_restart`, seeded random, external commands, input injection, frame tick, save handoff, `setting_get` (bool and choice only), mod saved data (`state_load` and the rest).

Rule language: `refuse`, `set`, `add`, `sub`, `message`, `send`; api 4 adds `release`, `give`, `item_count[item]`, `setting.<key>`. Mod cells `state.x[key]` (up to 1024 keys, saved with the game save). Flat expressions, no loops, 256 steps per firing. Rule state names: `money`, `badges`, `badge[n]`, `zone`, `party_count`, `species[m]`, `level[m]`, `hp[m]`, `max_hp[m]`, `type1[m]`, `type2[m]`, `shiny[m]`, `held_item[m]`, `species_type1[s]`, `species_type2[s]`, `species_bst[s]`, `caught[s]`, `item_count[i]`, `battle_style`, `in_battle`.

Hard limits found while reading:

- `party_get`, `party_set`, `box_get` and `battle_get` pass the field id straight to the game accessor (`POKE_TOOL_GetPartyValue`), so a C mod can already read any field id of `PokeToolParam` (`arm9/poke_tool_param.h`: personality 0x00, OT id 0x07, friendship 0x09, ability 0x0A, moves 0x36 to 0x39, IVs 0x46 to 0x4B, is-egg 0x4C, gender 0x6E, form 0x6F, nature 0x70, nickname 0x74, has-nickname 0x75). The ABI enum names only 13 of them and rules reach even fewer.
- `zone` is the zone id, not an "area". Several zones make up one route or city (interiors, cave floors). See the `area` item below.
- Settings have no number type (`bool`, `choice` only). A per-badge cap cannot be a setting row.
- The `mon.nickname` `force` field has no applier. The prompt row names no Pokemon.
- `heal.source` reads "script" for a Pokemon Centre; it cannot tell a Centre from another heal script.
- `encounter.wild.shiny` forces shiny; it does not tell whether the roll is shiny. `catch.attempt.shiny` does.
- `encounter.static` has no legendary kind.
- `battle.action` for a switch carries a party position only. Rules have no way to turn a position into a Pokemon.
- `mon.exp` names the first member that gains experience only.
- Rules cannot loop, so they cannot scan the party or boxes. Anything that needs a scan is a C mod or a new aggregate name.
- Rules cannot receive a peer message (`send` exists, receive does not).

## 3. Rule by rule

Legend for the last column: OK = possible today; PART = possible with a workaround or limit; MISS = missing (numbers refer to section 4). "R" means a rule is enough, "C" means it needs a code mod.

| Rule | Detect (events) | Read (state) | Do (actions) | Today |
|---|---|---|---|---|
| Nuzlocke, first encounter per area | `encounter.wild`, `encounter.static`, `catch.attempt`, `catch.result`, `battle.end` | area key, per-area "used" cell, shiny flag, whether the battle is wild | refuse `catch.attempt`, message, set cell | PART (zone key is wrong for multi-zone routes, M1) |
| Fainted is dead | `battle.faint`, `battle.end`, `whiteout` | mon identity (personality), side, place | record dead in cell, `mon_release` or box, refuse `box.withdraw`, refuse switch | PART (identity read M3, switch to mon M4) |
| Nicknames forced | `mon.receive`, `mon.nickname` | has-nickname | open the prompt | MISS (M9) |
| Whiteout ends run | `whiteout`, `battle.end` | outcome | `run_restart` with DELETE_SAVE | OK (C or R plus service) |
| Dupes clause | `encounter.wild` | species, owned count (party + box), evolution line root | refuse the encounter (the player walks on and re-rolls) | PART (`caught[s]` is the dex, not "owned"; M5, M13) |
| Shiny clause | `catch.attempt` | shiny | skip the refuse | OK (`catch.attempt.shiny`) |
| Static encounter exempt | `encounter.static`, `mon.receive` source | kind | skip the area cell | OK |
| Gift clause (free or banned) | `mon.receive` source GIFT | species, level | cell, or `mon_release` | OK |
| Species clause (no two alike) | `mon.receive`, `box.withdraw` | party species, owned | refuse, `mon_to_box` | PART (owned count M5) |
| Set mode | `battle.start`, `save.load` | `battle_style` | set it | OK (rule `set battle_style`; the options menu can still change it between battles) |
| Level cap per badge | `mon.exp`, `item.use`, `badge.get` | `badges`, level, exp, cap table | set `mon.exp.amount`, refuse Rare Candy | PART (see below) |
| No overleveling (no EXP while at cap) | `mon.exp` | level, cap | amount 0 | OK (coarse) |
| No items in battle | `battle.item` | item | refuse | OK |
| No healing items in battle | `battle.item`, `item.use` | item class (medicine, revive) | refuse | PART (list of ids vs the 256-step budget, M8) |
| Limit healing items | `item.use`, `battle.item` | counter | cell, refuse | PART (M8) |
| No Pokemon Center | `heal` | heal source or zone kind | refuse | MISS (M7) |
| Limited Centre uses | `heal` | counter | cell, refuse | MISS (M7) |
| No PC | `pc.open` | none | refuse | OK |
| No running | `battle.escape` | none | refuse | OK |
| Ban legendaries / banned species | `mon.receive`, `box.withdraw`, `encounter.wild`, `mon.evolve` | species, ban list | refuse, `mon_release`, `mon_to_box` | PART (ban list as a table, M2; `encounter.static` has no legendary kind) |
| BST cap | `mon.receive`, `mon.evolve` | `species_bst` | same | OK |
| Monotype / Monolocke | `mon.receive`, `box.withdraw`, `mon.evolve` | types, chosen type | `mon_to_box`, refuse | OK (type is a `choice` setting) |
| Uniquelocke | `mon.receive`, `box.withdraw`, `party.swap` | types of all party members | refuse | PART (scan, M5) |
| Solo run / starter only | `mon.receive`, `box.withdraw` | species | `mon_to_box`, refuse | OK |
| No evolution / Little Cup | `mon.evolve` | species stage | refuse | OK (stage read M13 for "not fully evolved") |
| No TMs | `item.use`, `move.learn` source | item id, TM list | refuse | OK (TM item ids are kenon data) |
| No held items | `mon.helditem` | item | `mon_take_item` | OK |
| No shops / poverty | `shop.buy`, `money.change` | money | refuse, `money_set` | OK |
| Team size = leader team size | `battle.start`, `badge.get` | party count, leader team size | message | PART (leader size from trainer data, M12) |
| Completionist / stay in gym | `trainer.spotted`, `battle.end`, `map.warp` | trainer defeated flags | refuse warp | PART (M12) |
| Hardcore Nuzlocke | union of the above | | | PART (union) |
| Wedlocke | `mon.receive`, `battle.action`, `battle.switch`, `battle.faint` | gender, pair, party position to Pokemon | refuse switch, release partner | MISS (M3, M4) |
| Egglocke | `catch.result`, `egg.hatch` | species | give an egg, raise the hatchling | MISS (M10) |
| Wonderlocke | `catch.result` | species | export, import another player's Pokemon | PART (services exist; creating the Pokemon, M10) |
| Randomizer Nuzlocke | data layer plus Nuzlocke | | | OK (kenon randomizer; no eidos API) |
| Soul Link | encounter, faint, box, release events | area key, mon identity | send/receive messages, release, box | MISS (section 5) |
| Kaizo Ironmon | `battle.start`, `battle.damage`, `battle.item`, `shop.buy`, `whiteout` | side of the target, wild flag, HP, species BST | set `level_adjust`, cap wild damage, refuse items, restart | PART (level percent, `side[m]`, M6) |
| Scalemon | none at run time | none | rewrite personal data | OK (kenon-side data transform) |
| Level scaling / mirror match | `battle.start` | party levels | set `level_adjust` | PART (max party level aggregate, M5; only a flat add) |
| Speedrun / Sleeplocke | `save.load`, `badge.get`, `game.cleared` | play time | timer, split | PART (game play time not readable, M11) |
| Trashlocke / theme / colour locke | `mon.receive`, `box.withdraw` | species, list | refuse | PART (list as a table, M2) |
| No super-effective / STAB / power cap | `battle.action` | move type, power, class, target types | refuse | MISS (M8) |
| Starlocke / Chesslocke / Tribelocke | `battle.switch`, `battle.action`, faint | roles per Pokemon | refuse | MISS (M3, M4) |

### Level cap per badge, worked through

Possible today as eight rules that kenon writes with the numbers inlined:

```
language 1
rule cap_2_badges on mon.exp
    when badges == 2 && level[event.mon] >= 20
    do set event.amount 0
```

and one rule on `item.use` for Rare Candy. The cap numbers come from the trainer data, so this part is kenon-side: the highest level in the leader's team, read at pack time from the ROM's trainer archive (`TR_TOOL_LoadTrainerInfo`, `TR_TOOL_BuildParty` in `arm9/tr_tool.h`) and shown in a per-badge table editor. Limits:

- A big experience gain can cross the cap in one battle. `amount 0` stops only Pokemon already at the cap. Clamping needs `exp_for_level` and a `min` operator (M14).
- After the eighth badge the cap becomes the Elite Four then Alder (50, then 77 for the champion); the rule reads the `badges == 8` case and a flag for the league. League progress flags are in the event flag bank (`flag()` call), flag ids are game data that kenon can list.
- `mon.exp` names the first party member that gains. A battle with several gainers fires once, so the others are not capped (documented in `mod-events.md`). Needs the event to fire per member (M15).
- If the mod edits trainer data later, kenon recomputes the caps.

## 4. Missing functions, prioritized

Format follows eidos: a C state call, then the rule read name. "Where" gives the game location, from aletheia unless said. Priority tiers: **A** fits the open api 4 (reads and names; low risk, no new hooks), **B** next interface version, **C** later.

Items already covered by the coordinator's api 4 update (`mon_release`, `item_count`, `item_add`, `release`, `give`, `item_count[i]`, `setting.<key>`, queued moves applied) are not repeated.

| # | Tier | Need | Needed by |
|---|---|---|---|
| M1 | A | area key | Nuzlocke family, Soul Link, Routelocke |
| M2 | A | mod data tables | level caps, ban lists, id lists, gym trainer ids, zone to area |
| M3 | A | more Pokemon fields by name | Nuzlocke death tracking, Wedlocke, Soul Link, nickname checks |
| M4 | A | party position to Pokemon; side and place of a ref | switch refusal, Wedlocke, Soul Link, Ironmon |
| M5 | A | party and box aggregates | dupes, species clause, Uniquelocke, level scaling |
| M6 | A | `side[m]`, `place[m]`, wild flag in rules | Ironmon (wild survives), hitless |
| M7 | A | Pokemon Centre detection | No Centre, limited Centre uses |
| M8 | A | item and move data reads | healing-item limits, move bans |
| M9 | A | nickname set, get and forced prompt | all Nuzlocke |
| M10 | B | create and give a Pokemon | Egglocke, Wonderlocke, Ironmon starter, Genlocke |
| M11 | A | game play time | Speedrun, Sleeplocke, run panel |
| M12 | A | trainer data reads and defeated flags | gym leader identity, team size rule, completionist |
| M13 | A | species data names | evolution line, stage, gender ratio, legendary |
| M14 | B | `min`, `max`, `exp_for_level` | exact level cap |
| M15 | B | per-member `mon.exp` | exact level cap, shared experience |
| M16 | B | rule actions `box`, `take_item`, `heal_mon`, `prompt` | rules without a code mod |
| M17 | B | `session.message` event | Soul Link in rules |
| M18 | C | wild encounter table read | Professor Oak challenge, dupes by reroll |
| M19 | C | `item.add` source, ground vs shop | "found items only" rule |
| M20 | C | `money.change` source (coordinator: planned after freeze) | poverty runs |

### M1 area key (A)

```c
u16 (*area)(void);               /* key of the place the player is in; 0 when none */
u16 (*area_of)(u16 zone);        /* key of any zone */
```
Rule reads: `area`, `area_of[z]`.

Where: `ZONEDATA_GetPlaceNameId` (`arm9/zonedata.c`) is the place-name message id of a zone; zones with the same name are one route, city or cave. `ZONEDATA_GetParentZoneId` gives an interior's outdoor zone. `ZONEDATA_GetAreaId` is the tileset (`ZONEDATA_CreateAreaData(areaId, season)`), so it is not usable as a route key. Evidence is the code comment near `ZONEDATA_GetPlaceNameId` (it substitutes 0x70 for two neighbouring values) and the signatures in `zonedata.h`; the exact behaviour of interiors and caves must be confirmed by running a zone sweep in eidos. A mod could also apply its own zone-to-area table once M2 exists, so the minimum request is the tables in M2 and a zone list (kenon can build the grouping from the zone headers in the ROM, a pure kenon task). Add the call anyway: the game already knows its own place names.
Needed by: every first-encounter rule, Soul Link (both sides must use the same key, so the key must be stable and independent of the save).

### M2 mod data tables (A)

A manifest key `table <name> <path>`; the file maps integer keys to integers (`key value` per line). Reads in rules: `table.<name>[key]` (0 when absent); in C: `int (*table_get)(const char *name, s32 key, s32 *out)`.

Where: eidos only (the rule layer). Cells already hold keyed values, so the storage exists; this is read-only data that ships with the mod.
Needed by: per-badge level caps (`table.cap[badges]`), banned species lists, TM item lists, medicine item lists, gym trainer ids, zone to area, cap tables per challenge. It removes the need for the 256-step `||` chains and the lack of numeric settings. kenon writes the files from its editors. This is the most valuable single addition.

### M3 more Pokemon fields by name (A)

Add to the ABI enum (ids from the game's own accessor, `arm9/poke_tool_param.h`) and to the rule names, all read-only:

```
BW_MON_FIELD_PERSONALITY 0x00   rule: personality[m]
BW_MON_FIELD_OT_ID       0x07   rule: ot_id[m]
BW_MON_FIELD_FRIENDSHIP  0x09   rule: friendship[m]
BW_MON_FIELD_ABILITY     0x0A   rule: ability[m]
BW_MON_FIELD_MOVE1..4    0x36..0x39   rule: move1[m] .. move4[m]
BW_MON_FIELD_IS_EGG      0x4C   rule: is_egg[m]
BW_MON_FIELD_GENDER      0x6E   rule: gender[m]
BW_MON_FIELD_FORM        0x6F   rule: form[m]
BW_MON_FIELD_NATURE      0x70   rule: nature[m]
BW_MON_FIELD_HAS_NICKNAME 0x75  rule: has_nickname[m]
```
`exp[m]` is in the enum (0x08) but not in the rule names; add it. Calls already pass these ids through; this is naming and rule checking, plus the surface hash.
Needed by: a stable identity for dead and linked Pokemon (personality plus OT id is unique), Wedlocke gender, egg handling, nickname check.

### M4 party position to Pokemon (A)

Rule read: `party_mon[n]` returns a `mon` for party position `n` (the call `party_ref` already exists). Lets a rule inspect `battle.action.switch_to`, `party.swap.slot_a`, `box.deposit` and so on. Also `box_mon[b, s]` is optional (two subscripts, C only today via `box_ref`).
Where: `live_member()` in eidos `mod_state.c` already does the lookup; this exposes it.
Needed by: refusing a switch to a dead Pokemon, Wedlocke, Chesslocke.

### M5 party and box aggregates (A)

```c
u32 (*owned)(u16 species);         /* in party and boxes */
u32 (*party_max_level)(void);
u32 (*party_alive)(void);          /* members with HP > 0, eggs not counted */
u32 (*party_has_type)(u32 type);   /* how many members have it */
```
Rule reads: `owned[s]`, `party_max_level`, `party_alive`, `party_types[t]`.
Where: party scan `POKE_TOOL_GetPartyValue` over `party_count`; box scan `BOX_SAVEDATA_GetUsableBoxCount` and `POKE_TOOL_GetBoxValue` (both used by `mod_state.c`).
Needed by: dupes clause (the dex `caught[s]` stays true after a release), species and type clauses, level scaling, "all dead" checks.

### M6 rule reads for battle refs (A)

Rule reads `side[m]` and `place[m]` from `BwMonRef.side`/`.place`; `wild` as `event.wild` already exists on `battle.start`/`battle.end`, add `wild_battle` as a state name for use at other events.
Where: `BwMonRef` is already filled; this only exposes it.
Needed by: Ironmon (wild Pokemon cannot be killed: clamp `battle.damage.amount` when the target is wild), hitless, Soul Link (own side only).

### M7 Pokemon Centre detection (A)

```c
u8 (*zone_kind)(u16 zone);          /* BW_ZONE_FIELD, _CENTRE, _MART, _GYM, _CAVE, _OTHER */
```
Rule read: `zone_kind[z]`, `in_centre` (for the current zone).
And on `heal`: `source` BW_HEAL_CENTRE when the zone is a Centre and the script is the nurse's.
Where: `SCRCMD_PROC_HealParty` (ov010) is the script command both the nurse and other scripts run, so the call alone cannot tell (documented in `mod-events.md`). The zone headers hold a map type and sub type (`ZONEDATA_GetMapType`, `ZONEDATA_GetMapSubType`); whether one of them marks a Centre is not verified. A table from M2 (`table.centre[zone]`, written by kenon from the zone list) is the fallback and needs nothing from the game. `GPOWER_UseHpRestore` (arm9) is the Pass Power heal, already `BW_HEAL_POWER`.
Needed by: No Pokemon Centre, limited uses, "heal only by items".
Black/White note: Pokemon Centres are the nurse script; Join Avenue and Entralink Pass Powers are separate heals and should be separate rules (a power is its own source).

### M8 item and move data reads (A)

```c
int (*item_value)(u16 item, u32 field, u32 *out);   /* price, pocket, hold effect, medicine flags */
int (*move_value)(u16 move, u32 field, u32 *out);   /* type, power, class, priority, accuracy */
int (*type_effect)(u8 attack_type, u8 defend_type1, u8 defend_type2); /* 0, 5, 10, 20, 40 */
```
Rule reads: `item_pocket[i]`, `item_is_medicine[i]`, `item_is_ball[i]`, `item_is_revive[i]`, `move_type[mv]`, `move_power[mv]`, `move_class[mv]`, `move_priority[mv]`, `effect[mv_type, ...]`.
Where: `FILE_UTIL_GetItemRecordParam` and `FILE_UTIL_GetItemExtParam` (`arm9/file_util.c:538`; medicine block present when `extType` is 1, 208 of 627 items); `WAZA_TOOL_GetDataParam` (`arm9/waza_tool.c:198`). Record layouts are in `docs/game-data-map.md`. The type chart is the game's own battle table; its function is not located here and must be found in ov093 before this is signed off.
Needed by: healing-item limits without a 200-id list, TM bans, STAB, super-effective and base-power bans.

### M9 nickname control (A)

```c
int (*mon_nickname)(BwMonRef mon, char *out, u32 size);    /* UTF-8 */
int (*mon_set_nickname)(BwMonRef mon, const char *name);
```
And make `mon.nickname.force` effective on the catch screen row (`ZUKAN_NICKNAME_Create`, ov207), or add the event `mon.received_unnamed` for the catch path. Rule read `has_nickname[m]` (M3), action `nickname <mon> <text>`.
Where: `POKE_TOOL_PARAM_NICKNAME` 0x74 and `HAS_NICKNAME` 0x75 on `POKE_TOOL_SetPartyValue`, `EVENT_NAMEIN_Create` (ov021) is the name-entry screen.
Needed by: the Nuzlocke nickname rule (a mod can check after the fact and message, but cannot force the prompt today), Wedlocke and Soul Link pairs named alike.

### M10 create and give a Pokemon (B)

```c
int (*mon_give)(u16 species, u8 level, u32 flags, BwMonRef *out);  /* flags: EGG, SHINY, TO_BOX */
```
Where: `SCRCMD_PROC_AddMonToParty` / `AddMonToBox` / `AddEggToParty` (ov010, the same path as gifts, binding `mon.receive` with source GIFT) and `POKE_TOOL_CreateEx` (arm9). The rule action `give` is the item form; add `give_mon`.
Needed by: Egglocke, Wonderlocke and Genlocke import, Ironmon starter, "starter swap" challenges. `mon_import` exists but needs a file first.

### M11 game play time (A)

```c
u32 (*play_seconds)(void);
```
Rule read `play_seconds`.
Where: `GAME_DATA_GetPlayTime` returns `struct PlayTime *` (`arm9/game_data.h:142`); `PLAYTIME_GetHours`, `GetMinutes`, `GetSeconds` (`arm9/playtime.h`) convert it.
Needed by: Speedrun and Sleeplocke (the existing `timer_ms` is the mod's own timer, not the game's), the run panel, "battle by minute" rules.

### M12 trainer data reads and defeated flags (A)

```c
int (*trainer_value)(u16 trainer, u32 field, u32 *out);  /* class, party size, format, highest level */
int (*trainer_defeated)(u16 trainer);
```
Rule reads: `trainer_class[t]`, `trainer_size[t]`, `trainer_max_level[t]`, `trainer_defeated[t]`.
Where: `TR_TOOL_LoadTrainerInfo` and `TR_TOOL_BuildParty` (`arm9/tr_tool.h:21`). The defeated flag is an event flag set by `SCRCMD_WORK_CmdSetTrainerDefeatedFlag` (ov010); the flag base is not read here and `flag()` is the call to use once kenon or eidos knows the base. `battle.start` already carries `trainer_id[2]`.
Gym leader identity can be done today with M2 (a table of gym trainer ids built by kenon from the trainer data). `trainer_max_level` makes the cap table derive from the game with no kenon data.
Needed by: level caps from the actual leader party, team-size rule, completionist, "no grinding".

### M13 species data names (A)

Rule reads over `species_value` fields: `species_stage[s]` (the personal record's `evoStage`, `docs/game-data-map.md`; its exact rule is inferred, not verified), `species_gender_ratio[s]`, `species_egg_group1[s]`, `species_growth[s]`, `species_catch_rate[s]`, `species_ability1[s]`, plus `species_base[s]` (first stage of the line, from the evolution archive) and `species_is_legendary[s]`.
Where: `POKE_PERSONAL_GetParam` (44 getter cases, `arm9/poke_personal.c`); the evolution archive for the line root. There is no "legendary" flag in the personal data that was verified; use a kenon table through M2.
Needed by: dupes by line, Little Cup, single-stage rules, Wedlocke pairing (gender ratio), legendary bans.

### M14 min, max, exp_for_level (B)

Rule operators `min(a, b)`, `max(a, b)`, `abs(a)`. The rule grammar has one subscript, so the experience for a level is a C call and a rule name taking the level as the subscript of the current mon: `exp_for_level[level]` (species taken from `event.mon`).
```c
int (*exp_for_level)(u16 species, u8 level, u32 *out);
```
Where: the growth-rate tables used by `POKE_TOOL` (`growthRate` in the personal record, `docs/game-data-map.md`).
Needed by: exact level cap (amount = min(amount, exp_for_level(cap) - exp)).

### M15 per-member mon.exp (B)

Fire `mon.exp` once per gaining member (`BTL_SERVER_FLOW_CalcPartyExpShares` computes the share per member), or add `mon.exp_each`. Where: `BTL_SERVER_FLOW_CalcPartyExpShares` (ov093). Needed by: exact level cap with several gainers.

### M16 rule actions (B)

`box <mon>` (the `mon_to_box` call), `take_item <mon>`, `heal_mon <mon>`, `prompt <text> yes <action>` (confirm), `nickname <mon> <text>`, `export`/`import` aliases. Calls exist for all but the prompt; the gap is rule syntax only.

### M17 session.message event (B)

An event `session.message` with `event.peer`, `event.text` (up to 256 bytes) and `event.kind` (message, join, leave), so a rule can receive. Where: `BwSessionHandler` (`on_session`) already delivers the same data to C mods.

### M18 to M20 (C)

- M18 `encounter_species[zone, method, slot]`: encounter tables per zone, `ZONEDATA_GetEncountId` (arm9) selects the table, `FIELD_ENCOUNT_CreateZoneWildMons` (ov021) rolls it. Needed by Professor Oak and dupes-by-reroll.
- M19 `item.add.source` (shop, ground, gift, script): `MYITEM_SAVEDATA_AddItem` has sixteen callers (`mod-events.md`); the source is the caller. Needed by "found healing items only".
- M20 `money.change.source`: coordinator says planned after the freeze.

## 5. Soul Link (out of game, two instances)

Needs: a link between two eidos instances; a pairing table keyed by area; deaths, boxes and failed encounters sent both ways; a way to apply a partner's fate.

What exists: `session_host(code)`, `session_join(code)`, `session_send(peer, buf, size)`, `on_session(handler)` with MESSAGE, JOIN, LEAVE (`docs/mod-services.md`; the relay model follows Archipelago). `send <expr>` as a rule action. `mon_release` and `mon_to_box` apply a fate. `state_store` keeps the link table with the save, and `state_in_step` tells whether the mod's file and the save agree.

Minimal design:

1. Pair key: `area` (M1), same game and same ROM on both sides, so equal ids. Version check at JOIN: send a hello with the interface version and the ROM id; refuse a mismatch.
2. Link table in saved data: `link[area] = {state, my_personality, my_ot, partner_species, partner_dead}`. State: open, encountered, caught, failed, linked, dead, boxed.
3. Messages (UTF-8 lines, so a rule can send them): `E <area> <caught|failed> <species> <seq>`, `D <area> <seq>`, `B <area> <boxed|party> <seq>`, `R <hello> <last-seen-seq>`. A receiver ignores a message with a `seq` it has seen. Both sides keep a per-peer outbox in saved data until acknowledged.
4. Apply rules: on `E failed` from the partner, the local encounter for that area is marked failed (the local player may not catch it, a `catch.attempt` refuse). On `D`, release or box the local Pokemon whose `personality` is in `link[area]` using `mon_release` (applied at the next quiet moment, also works for a Pokemon in a battle). On `B`, `mon_to_box`.
5. Reconcile at JOIN: each side sends its whole link table and `last-seen-seq`; the side that is behind applies the missing messages. This covers one player offline and a restore from a save.
6. Save safety: one side loading an older save desyncs. The mod compares `state_in_step` at `save.load`; if false, it shows a message and asks the peer for the table.

eidos work: M1 (area key), M3 (personality), M4 (party position to Pokemon), M5 (lookup by personality, a small `find_mon(personality)` call is the clean form: `int (*find_mon)(u32 personality, BwMonRef *out)` over party, boxes), M17 (receive in rules). Without M17 it is a code mod (`on_session` handler); with it a rule set can do most of it. The relay and the code exchange exist.
kenon work: a Soul Link preset (links the rules, the settings and the message template), a link table viewer if the mod publishes it.

## 6. Pure kenon versus eidos

Pure kenon (data or UI, no eidos change):

- Level cap table per badge: a table editor (badge, cap, source: leader ace level read from the ROM's trainer data), and rule generation with the numbers inlined (works today); later it writes an M2 table file.
- Elite Four and champion caps, and "cap + N" offsets.
- Banned species lists (legendaries, BST over a limit, tier lists), as rule sets or tables.
- TM, medicine, revive, ball and held-item id lists, built from the item archive (`docs/game-data-map.md`, items), for item bans.
- Gym trainer id list and each leader's team size and ace level.
- Zone to area grouping from the zone headers (a fallback for M1).
- Scalemon: rewrite the personal archive base stats with the Smogon formula (HP unchanged, the rest scaled to BST 600, floor). The ban list for Scalemon (items and moves) is a rule set.
- Randomizer Nuzlocke, Ironmon randomization (starter, wild tables, trainer teams, moves, shop stock): kenon data transforms. The Ironmon "enemy levels plus 50 percent" is a trainer-data rewrite, so it needs no eidos change.
- Monotype type picker (a `choice` setting with 17 rows), and the preset picker for the whole challenge family (Nuzlocke, Hardcore, Monotype, Solo).
- Challenge presets: a named bundle of rules, settings and tables that a user picks.
- Rule linting at pack time (already planned in `docs/mod-rules-language.md`, section 5).

eidos-side (needs the game process): M1, M3 to M13, M14 to M17 as listed. M2 has a kenon half (the editor and the file writer) and an eidos half (the manifest key and the rule op).

## 7. Black/White notes

- Seasons: `route.change` carries `season`. Zones are not duplicated per season, so an area key survives the season change. Check the seasonal zones before signing off M1.
- Dream World and Entralink: no run in the original rules touches them. Entralink Pass Powers include an HP restore (`GPOWER_UseHpRestore`, `BW_HEAL_POWER`), so a "no healing" rule must also refuse the power. Join Avenue shops are shops (`shop.buy`).
- Encounter methods unique to B/W: dark grass double battles (`double_` in `encounter.wild`), shaking grass, rustling grass, dust clouds, rippling water. A set of rules often treats the first of these in an area as the area's encounter; the choice is a setting.
- Set mode: B/W has the game option (`battle_style`: shift or set); a rule may force it on `battle.start`, but the player can change it in the options menu between battles (`CONFIG_GetBattleStyle`, `arm9/config.h:19`). Forcing it at `menu.open` is a second guard.
- Pokemon Centre: the nurse is a script that calls the same heal command as scripts (see M7).
- Gym order: Striaton, Nacrene, Castelia, Nimbasa, Driftveil, Mistralton, Icirrus, Opelucid, and the cap follows the badge count unless the player skips; badges are bits of the misc save (`MISC_GetBadge` 0x0200C234, `MISC_CountBadges` 0x0200C258), so a cap by badge count is right even when the gyms are visited out of order, but the "next leader" for a skipped gym would be wrong. A table keyed by badge count is the intended shape.

## 8. Rules a challenge mod must be able to express

eidos and kenon provide the functions; the mod itself is written by its author, not shipped by kenon. Each rule below must be possible to write, as a setting of such a mod. The Setting column is the author's plan for their own mod.

| Rule | Setting | Needs |
|---|---|---|
| Dupes clause with reroll: a dupe first encounter is rolled again | on by default | M21, M5 |
| Dupes by evolution line: owning any member of a line makes the line a dupe | on by default | M13 (`species_base`), M5 |
| Dark grass double battle: one of the two wild Pokemon counts, picked at random | on by default | M22 |
| Story and gift Pokemon (Zekrom or Reshiram, Victini, the gift monkey, Darmanitan): allowed or not | option | M23 |
| No Dream World, Entralink or traded Pokemon | option | M24, M3 (`ot_id`) |
| Graveyard box: the dead go to a chosen box instead of release; optional revive token per badge | option | M25 |
| No revives: a fainted Pokemon cannot get HP back | on in Nuzlocke presets | `heal.hp` with `hp == 0` (api 4), plus M26 for revive items used outside `heal.hp` |
| Held item lost on faint | option | `mon_take_item` (today) |
| Level cap: disobedience above the cap | option | M27 |
| Level cap: no Rare Candy or EXP items above the cap | option | M28 |
| No grinding: reduced or no EXP from wild battles | option | M29 |
| Stronger trainers: level scaling, max IVs | option | M30 |
| Inverse or custom type chart | option | M31 |
| Wonderlocke and Ironmon swaps | option | M32 |
| Genlocke: carry survivors to the next game | option | M33 |
| Every Pokemon must battle (bench rules) | option | M34 |
| No Day Care, trading, Entralink or Dream World use | option | M35 |
| One shop purchase per city, no healing items in shops | option | `shop.buy` (today), M1 |
| Limited Pokemon Centre visits per badge | option | M7 or a table (M2) |
| Reload counter and penalty | option | `save.load` `state_in_step` (today), rule cells |
| Soul Link type clause | option | M3, M36 |

A challenge mod needs a numeric setting for counts and caps (Centre visits, cap offset). Settings are bool or choice today: M37.

### Missing functions for these rules

| # | Need | Proposed shape |
|---|---|---|
| M21 | Writable species and level on the wild encounter, at roll time, before the battle starts | `encounter.wild` payload `species *mod`, `level *mod`; or a new refusable `encounter.roll` that fires again on refuse (reroll) |
| M22 | Double wild battle: both Pokemon in the payload, and which one is first | `encounter.wild` `double`, `species2`, `level2` |
| M23 | Kind of encounter: wild, static (story), gift, egg, trade | `catch.attempt` and a new `mon.received` event with `kind` |
| M24 | Origin reads | rule `met_location[m]`, `origin_game[m]`, `met_level[m]`, `fateful[m]` (POKE_TOOL_PARAM ids) |
| M25 | Move a Pokemon to a given box, and back to the party | `mon_to_box_n(BwMonRef, u8 box)`, `mon_from_box(u8 box, u8 slot)`; rule actions `box <mon> <n>`, `withdraw <box> <slot>` |
| M26 | Status-only revive paths | confirm every revive (Revive, Max Revive, Revival Herb, Sacred Ash, Entralink power) goes through `heal.hp` with `hp == 0`; if not, a `mon.revive` refusable event |
| M27 | Obedience check hook | refusable `battle.obey` event or a writable `obeys` flag, on the game's obedience check |
| M28 | EXP source | `mon.exp` `source`: battle, rare candy, item, day care |
| M29 | Battle kind on EXP | `mon.exp` `battle_kind`: wild, trainer, gym leader, elite four (M12 trainer class) |
| M30 | Trainer party edit at battle start | `battle.trainer_party` event: each member writable (species, level, moves, item, ability, IVs) before the battle builds it |
| M31 | Type effectiveness hook | `battle.type_effect` event: attack type, defender types, `multiplier *mod` |
| M32 | Replace a Pokemon in place and write its fields | `mon_set(BwMonRef, field, value)` for species, form, ability, moves, nature, IVs, with stats recalculated |
| M33 | Export and import a full Pokemon | `mon_export(BwMonRef, u8 *buf, u32 size)`, `mon_import(const u8 *buf, u32 size, BwMonRef *out)`; the file format is the game's own party record |
| M34 | Took-part flag per Pokemon | `battle.end` payload `took_part` bit mask by party position |
| M35 | Refuse events for systems | refusable `daycare.deposit`, `trade.start` (in game and link), `entralink.enter`, `dreamworld.sync` |
| M36 | Refuse moving a Pokemon into the party | refusable `party.add` (from box, gift, catch) with the Pokemon |
| M37 | Numeric mod settings | settings type `int <min> <max> <default>` |
