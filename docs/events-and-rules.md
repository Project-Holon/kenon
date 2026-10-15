# Events and rules

The port exposes a code API: a mod overrides a game function by DS address,
subscribes to events, and reads and writes game state (`docs/mod-interface.md`).
This document specifies a constrained surface over the same capabilities, so a
person who does not write C can express a rule such as "on faint, subtract
1000 from the player money".

Rules are code in a small predetermined language, not data. A rule is
compiled or checked before it runs, and a rule that fails to compile is
refused. This is a deliberate difference from the manifest and the JSON
tables, which are data the loader reads. A rule names events, reads named
state, and performs named actions; it is not a table of key and value lines.

The rule surface is a projection of the code API. Anything a rule cannot
express needs a code mod. The events, their meaning and the accepted writes
are the port's, and this document attributes each to `docs/mod-events.md` and
`docs/mod-interface.md`. The port now fixes the rule surface:
`docs/mod-interface.md`, section 3, carries the `rule` manifest key, and
`docs/mod-rules.md`, section 1, decides that a small interpreter in the port
runs a rule. The rule language and its field names are the tooling's work,
which the port's ABI excludes (`docs/mod-events.md`, section 5).

## The events

The event catalogue is `docs/mod-events.md`, section 1. It holds 18 events:
the 16 of `docs/mod-interface.md`, section 4, plus `overlay.load` and
`overlay.start`. An event is a semantic concept that maps onto one or more
addresses in the hookable set plus a typed wrapper. The event name is the
permanent binding key: renaming one later breaks every mod bound to it.

| Event | Declared-set address(es) | Region | Fires | Block | Mods |
|---|---|---|---|---|---|
| `encounter.start` | 0x0216E818 | ov010 | a wild or scripted battle task is created | yes | N S NS |
| `encounter.trainer` | 0x021AA5CC | ov021 | a trainer battle is set up | no | N S NS |
| `battle.start` | 0x02013090 | arm9 | the battle setup is created | yes | N S NS MP |
| `catch.attempt` | 0x021CBA40 | ov093 | a ball is thrown | yes | N S NS |
| `catch.result` | the return of 0x021CBA40 | ov093 | the throw returns | no | N S NS |
| `battle.faint` | 0x021C5200 | ov093 | a faint is processed | yes | N S NS |
| `heal.party` | 0x0215B5A8 | ov010 | a script heals the party | yes | N S NS |
| `whiteout.begin` | 0x021D1960, 0x02158A20 | ov093, ov010 | the player blacks out | yes | N S NS |
| `route.change` | 0x021643E8 | ov010 | the field loads a new zone | no | N S NS |
| `save.begin` | 0x02007150 | arm9 | a save starts | no | N S NS MP |
| `save.done` | 0x02007168 | arm9 | the save completes | no | N S NS MP |
| `save.load` | 0x02012C18 | arm9 | a save loads | no | N S NS MP |
| `link.up` | 0x0215CD24 | ov009 | the link becomes on | no | S NS MP |
| `link.down` | 0x0215CD24 | ov009 | the link becomes off | no | S NS MP |
| `net.send` | the Wm and DWC send points | not built | bytes leave the process | no | MP |
| `net.receive` | the Wm and DWC receive points | not built | bytes arrive | no | MP |
| `overlay.load` | 0x02078EB0 | arm9 | an overlay's data is loaded | no | N S NS MP |
| `overlay.start` | 0x0207902C | arm9 | an overlay's static initialisers run | no | N S NS MP |

The address column is the even function start, the form aletheia's `address:`
line records (`mod/declared_set.txt`; `docs/mod-events.md`, section 1). The
overlay tables key on the value a code pointer holds, which carries the Thumb
low bit, so a manifest that names an address must state which form it uses and
accept both (`docs/mod-interface.md`, still-to-build item 2).

Nuzlocke abbreviates to N, Soullink to S, Nuzlocke plus Soullink to NS, and
Multiplayer to MP. A mod in the S or NS column needs every N event too,
because both are Nuzlocke supersets (`docs/modding.md`, "What the four
example mods need").

The first sixteen rows are the catalogue of `docs/mod-interface.md`, section
4. `overlay.load` and `overlay.start` are the port's addition
(`docs/mod-events.md`, section 1): `docs/mod-interface.md`, section 1, names
`FS_LoadOverlayImage` and `FS_StartOverlay` as lifecycle rows a mod that
writes overlay data needs, and section 4 makes the overlay lifecycle the load
and unload point of a mod. Whether they are events or a separate lifecycle
callback is not decided (`docs/mod-events.md`, still-to-build item 10).

### Declared addresses with no event

The declared set is the hookable set, not the event set (`docs/mod-events.md`,
"Declared addresses with no event"). Several declared addresses are a concept
a mod reaches by a function override or a state read rather than an event, and
several are part of a concept above whose binding is not decided. The port
names them:

| Address | Function | Region | Bound to |
|---|---|---|---|
| 0x0216EAB0 | EVENT_BATTLE_CreateEncounterTaskWithMon | ov010 | `encounter.start` (undecided) |
| 0x0201347C | BTL_SETUP_SetupWild | arm9 | `battle.start` (undecided) |
| 0x0216EE20 | EVENT_BATTLE_DispatchBattleKindNotify | ov010 | none |
| 0x02188CE8 | FIELDMAP_GetEncount | ov021 | none |
| 0x021CBD48 | BTL_SERVER_FLOW_GetBallCatchRate | ov093 | `catch.attempt` override point |
| 0x021D5B68 | BTL_POKEPARAM_IsFainted | ov093 | none (a predicate) |
| 0x02158AD4 | SCRIPT_SYS_RunFieldRecoverScript | ov010 | `whiteout.begin` (undecided) |
| 0x02018198 | POKE_TOOL_RecoverAllPP | arm9 | `heal.party` (undecided) |
| 0x021643AC | FIELD_MAP_CHANGE_ApplyZoneHeader | ov010 | `route.change` (undecided) |
| 0x021841C0 | EVENT_MAPCHANGE_ChangeFieldEvent | ov020 | `route.change` (undecided) |
| 0x021666C8 | WCM_GetLinkLevel | ov009 | none |
| 0x02166710 | WCM_CalcLinkLevel | ov009 | none |
| 0x02012BF8 | GAME_DATA_CheckSaveFlag | arm9 | the save-flag state read |

Two concepts span more than one declared address. `whiteout.begin` spans the
battle-screen half (0x021D1960) and the field-recover half (0x02158A20), and
whether one event name binds both or two events exist is not decided
(`docs/mod-events.md`, still-to-build item 1). `link.up` and `link.down` are
one address (0x0215CD24) and two events, distinguished by the predicate's
return. `route.change` and `heal.party` each have a declared address the four
mods name that the catalogue does not bind (`docs/mod-events.md`, section 1).

### What each event means, and what a rule may do

The port's exact semantics are `docs/mod-events.md`, section 1, and
`docs/mod-interface.md`, section 4. Each entry below adds what a rule may do
at that event.

- **`encounter.start`.** Fires once when `EVENT_BATTLE_CreateEncounterTask`
  is entered. The payload names the battle kind and the task. A handler may
  block, which suppresses the encounter; a `replace` changes the species the
  task builds. This is Nuzlocke's "first encounter of a route" point. A rule
  may block the encounter, or read the current map to key the used-route set.
- **`encounter.trainer`.** Fires when a trainer battle is set up. The payload
  names the trainer. Read-only; the encounter already exists. A rule may read
  it and record it.
- **`battle.start`.** Fires when `BTL_SETUP_Create` is entered. The setup
  struct is not yet filled, so a `wrap` handler runs before or after the
  original and may adjust the fields the original derives. A rule may read
  and adjust the setup fields.
- **`catch.attempt`.** Fires when a ball is thrown. The payload names the
  ball and the target. A block forbids the catch, which is Nuzlocke's rule
  against a second catch on a route. A rule may block the catch.
- **`catch.result`.** Fires when the `catch.attempt` slot's original returns.
  The payload is the result. Read-only. A rule may read the result and record
  the caught Pokemon.
- **`battle.faint`.** Fires when a faint is processed. The payload names the
  fainted Pokemon. A block may stop the reward step. Soullink mirrors the
  faint to the peer here. A rule may block the reward step, record the
  Pokemon as dead, or act on the linked Pokemon.
- **`heal.party`.** Fires when a script heals the party. A block forbids the
  heal, which is Nuzlocke's rule against healing a Pokemon the run has
  declared dead. A rule may block the heal.
- **`whiteout.begin`.** Fires when the party blacks out. It spans two
  addresses, the battle-screen half and the field-recover half, because one
  semantic concept is not one address (`docs/modding.md`, "Name and key the
  events"). A block suppresses the return to the last heal point, which
  Nuzlocke uses to continue after a whiteout. A rule may block the return.
  Whether one event name binds both addresses or two events exist is not
  decided (`docs/mod-events.md`, still-to-build item 1).
- **`route.change`.** Fires when the field loads a new zone. The payload
  names the map. Read-only. Nuzlocke keys the used-route set on it. A rule
  may read the map.
- **`save.begin`, `save.done`.** Fire around a game save. A mod writes its
  state here, through its state namespace. A rule may persist its state.
- **`save.load`.** Fires when a save loads. A mod reads its state here. A
  rule may read its state.
- **`link.up`, `link.down`.** Fire when the link state changes, from the one
  predicate `SOCL_LinkIsOn`. Soullink and multiplayer use them to open and
  close the exchange. A rule may open or close its exchange.
- **`net.send`, `net.receive`.** Fire at the Wm and DWC entry points. These
  are the multiplayer transport seam and are not built
  (`docs/modding.md`, "Networking seam: missing"). The names are fixed so a
  mod does not rebind when the transport lands. A rule may not use them
  today.
- **`overlay.load`.** Fires when `FS_LoadOverlayImage` (0x02078EB0, arm9)
  runs. The overlay's data is reset on every load, so a mod that writes
  overlay data needs this event and `overlay.start` (`docs/mod-events.md`,
  section 1). Whether the handler runs before or after the port's own load
  work is not decided (still-to-build item 10).
- **`overlay.start`.** Fires when `FS_StartOverlay` (0x0207902C, arm9) runs
  the overlay's static initialisers. Whether the handler runs before or after
  the initialisers is not decided (`docs/mod-events.md`, section 1).

## What a rule may do

A rule acts at one event. The actions are a subset of what the code API
allows at that event.

- **Block or allow.** The code API returns `BW_EVENT_CONTINUE` or
  `BW_EVENT_BLOCK` from a handler (`docs/mod-interface.md`, section 4). A
  block is the return of a block on a slot that defines one, not a state
  write. Blocks compose, and the port must fix whether any handler blocks or
  every handler must allow (`docs/mod-events.md`, still-to-build item 8). A
  rule may block at the events that define a block: `encounter.start`,
  `catch.attempt`, `battle.faint`, `heal.party`, `whiteout.begin`.
- **Read state.** Party and box records, species, level and HP, the current
  map, and the save flags, through `bw_ds_ptr` or the matched accessors
  (`docs/mod-interface.md`, section 4).
- **Write state.** The same bytes. The port does not arbitrate a write, and
  the game's later write wins (`docs/mod-interface.md`, section 4). A rule
  that must stick writes at the event the game derives the state from, or
  through the override's return.
- **Show the player a message.** Partial: the mod-facing screen and message
  surface is not built (`docs/modding.md`, "Settings and UI: partial";
  `docs/mod-interface.md`, item 11). A rule may request a message; whether
  and how it is shown is not decided.
- **Persist state.** At `save.begin`, through the mod's `state` namespace,
  written whole and atomically (`sdk/save.h`; `docs/mod-interface.md`,
  section 4). The exact callback that reads state before the game runs and
  writes it at `save.begin` is not built (`docs/mod-interface.md`, item 10).
- **Send an event to another instance.** Not built. The transport does not
  exist (`docs/modding.md`, "Networking seam: missing").

### The example, worked

"On faint, subtract 1000 from the player money" is a rule at
`battle.faint`. The port names the field. Player money is `money`, the save
block with id 0x34 at offset +0x00, a u32 capped at 9,999,999, read by
`BEACON_GetPoints` (0x0200C274) and written by `BEACON_AddPoints`
(0x0200C278) or `BEACON_SubPoints` (0x0200C29C) (`docs/mod-events.md`,
section 4; `docs/mod-interface.md`, section 4). The block is at a run-time
address, not a fixed DS address, reached through `SAVE_CONTROL_GetBlock`
(0x020071CC) and `GAME_DATA_GetMiscSave` (0x02012F08). The money accessors
are state accessors, not override points, so they are not in the declared set
(`docs/mod-interface.md`, section 4). A rule names `money` and the port makes
the same call. The port does not arbitrate the write, so the game's later
write wins; a change that must stick is made at the event the game derives
the money from (`docs/mod-events.md`, section 4).

## The rule substrate

The port decided the substrate. A small interpreter in the port runs a rule,
not the game's bytecode VM (`docs/mod-rules.md`, section 1, "Decision"). The
rule is not native code and not the game's script bytecode.

The reasons the port gives: the VM cannot bind to a port event, cannot reach
the per-mod state namespace, and cannot express block and allow as a value,
and the port has not wired the VM at all (`docs/mod-rules.md`, section 1,
"What the VM cannot express"). The port's ABI excludes the rule language and
its field names, which are the tooling's work (`docs/mod-events.md`, section
5).

What a rule compiles to: the input the port's interpreter runs, a small
bytecode over the port's field table and the event payload
(`docs/mod-rules.md`, section 1; `docs/mod-interface.md`, sections 3 and 4).
The port runs a compiled rule at the event slot that fires the event, in the
same place it runs a mod's C handlers (`docs/mod-interface.md`, "The rule
surface"). The rule stays out of host code. The exact encoding is part of the
grammar still to be fixed (`docs/mod-rules.md`, still-to-build item 1).

Who owns the language: the tooling. The port owns the interpreter, the field
table it exposes, and the substrate decision. The rule grammar, the state
field names and the actions are the tooling's (`docs/mod-events.md`, section
5, which excludes the rule language from the port ABI). The grammar is still
to be fixed (`docs/mod-rules.md`, still-to-build item 1).

The game's own field and battle effect VMs still exist, and kenon still plans
an assembler and disassembler for them (`README.md`). The port decision is
about the rule substrate only, not about the VM tools.

## What is not decided

- The rule language: its grammar, its state field names, and its actions
  (`docs/mod-rules.md`, still-to-build item 1). The tooling owns it, and the
  port does not fix it.
- The interpreter seam: where the port runs a rule at a slot, and how it
  reads and writes the field table (`docs/mod-rules.md`, still-to-build item
  2).
- The full table of named state fields. The port names `money` and the other
  fields of `docs/mod-events.md`, section 4, but the port's mod-facing wrapper
  name and signature for each are still to be fixed (`docs/mod-events.md`,
  still-to-build item 4).
- The message surface a rule may use (`docs/mod-interface.md`, item 11).
- Whether `overlay.load` and `overlay.start` are events or a separate
  lifecycle callback, and whether the handler runs before or after the port's
  own load work (`docs/mod-events.md`, still-to-build item 10).
- Whether `whiteout.begin` is one event on two addresses or two events, and
  which declared address each multi-address concept binds (`docs/mod-events.md`,
  still-to-build item 1).
- Block composition, whether any handler blocks or every handler must allow
  (`docs/mod-events.md`, still-to-build item 8).
- The authority model for Soullink: which instance decides a shared outcome,
  how two conflicting events are ordered, and what happens when one instance
  is ahead (`docs/modding.md`, "What is still to build", item 3).
- The transport, and therefore every action that sends an event to another
  instance (`docs/modding.md`, "Networking seam: missing").

## Related documents

- `docs/bw-format.md`: the `.bw` container and the manifest.
- `docs/mod-contract.md`: the tooling side of the port contract.
- eidos `docs/mod-events.md`: the 18-event catalogue, the state API and the
  ABI surface.
- eidos `docs/mod-rules.md`: the rule substrate decision and the container
  decision.
- eidos `docs/mod-interface.md`, section 4: the event and state API, and the
  typed wrapper.
- eidos `docs/mod-interface.md`, section 5: the trust model.
- eidos `docs/modding.md`: what the four example mods need, including the
  events Nuzlocke is built on.
- aletheia's `tools/assets/scripts.py` and `tools/assets/btleff.py`: the
  field script VM and the battle effect VM opcode tables.
