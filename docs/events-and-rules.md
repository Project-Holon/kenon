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
are the port's, and this document attributes each to `docs/mod-interface.md`.
The four eidos documents do not mention rules, so the language and the
compiler or interpreter are this repository's work over the port's API.

## The events

The event catalogue is `docs/mod-interface.md`, section 4. An event is a
semantic concept that maps onto one or more addresses in the hookable set
plus a typed wrapper. The event name is the permanent binding key: renaming
one later breaks every mod bound to it.

| Event name | Source address(es) | When it fires | Mods |
|---|---|---|---|
| `encounter.start` | 0x0216E818 | a wild or scripted battle task is created | N S NS |
| `encounter.trainer` | 0x021AA5CC | a trainer battle is set up | N S NS |
| `battle.start` | 0x02013090 | the battle setup is created | N S NS MP |
| `catch.attempt` | 0x021CBA40 | a ball is thrown | N S NS |
| `catch.result` | the return of 0x021CBA40 | the throw returns | N S NS |
| `battle.faint` | 0x021C5200 | a faint is processed | N S NS |
| `heal.party` | 0x0215B5A8 | a script heals the party | N S NS |
| `whiteout.begin` | 0x021D1960, 0x02158A20 | the player blacks out | N S NS |
| `route.change` | 0x021643E8 | the field loads a new zone | N S NS |
| `save.begin` | 0x02007150 | a save starts | N S NS MP |
| `save.done` | 0x02007168 | the save completes | N S NS MP |
| `save.load` | 0x02012C18 | a save loads | N S NS MP |
| `link.up` | 0x0215CD24 | the link becomes on | S NS MP |
| `link.down` | 0x0215CD24 | the link becomes off | S NS MP |
| `net.send` | the Wm and DWC send points | bytes leave the process | MP |
| `net.receive` | the Wm and DWC receive points | bytes arrive | MP |

Nuzlocke abbreviates to N, Soullink to S, Nuzlocke plus Soullink to NS, and
Multiplayer to MP. A mod in the S or NS column needs every N event too,
because both are Nuzlocke supersets (`docs/modding.md`, "What the four
example mods need").

### What each event means, and what a rule may do

The port's exact semantics are `docs/mod-interface.md`, section 4. Each
entry below adds what a rule may do at that event.

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
  decided (`docs/mod-interface.md`, item 8).
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

## What a rule may do

A rule acts at one event. The actions are a subset of what the code API
allows at that event.

- **Block or allow.** The code API returns `BW_EVENT_CONTINUE` or
  `BW_EVENT_BLOCK` from a handler (`docs/mod-interface.md`, section 4). A
  block is the return of a block on a slot that defines one, not a state
  write. Blocks compose, and the port must fix whether any handler blocks or
  every handler must allow (`docs/mod-interface.md`, item 9). A rule may
  block at the events that define a block: `encounter.start`,
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
`battle.faint`. The rule reads the player money field, subtracts 1000, and
writes the new value. The port does not arbitrate the write, so the game's
later write wins (`docs/mod-interface.md`, section 4). The rule must write at
the event the game derives the money from if the change must stick.

Player money is not named in the four eidos documents and is not in the
declared hookable set (`docs/mod-interface.md`, section 1). The field name
and its address are not decided. The rule language needs a table of named
state fields, each mapped to an address or an accessor the port exposes.
That table does not exist. Until it does, the example names a field the
tooling cannot resolve.

## The game's own script VMs as a candidate substrate

The game already contains field and battle script VMs. Aletheia has decoded
both, and they are the substrate the planned assembler and disassembler in
this repository would serve.

- **The field script VM.** The interpreter is `vm.c` at
  `src/matched/unit_0201126C.c` and `src/matched/unit_02011330.c`. It
  dispatches through a 609-entry handler table at 0x0217062C in overlay 10.
  A bytecode is one little-endian `u16` opcode, then its operand bytes. The
  field-script archive is `a/0/5/7` (899 sub-files). The stop opcodes are
  {2, 29, 140, 342, 359, 378}, opcode 4 is a call, and the flow-end opcodes
  are {5, 30, 31} (aletheia's `tools/assets/scripts.py`, its header).
- **The battle effect VM.** It is the same engine as the field script VM,
  with its own handler set. Every one of its 78 opcodes takes 4-byte `u32`
  operands only. Opcode 77 is a stop, opcode 70 is a call, opcode 71 is a
  return, and opcodes 60, 61, 72 and 74 are branches. Its data archive is
  `a/0/6/6` (601 sub-files), and its object constructor is
  `func_021F94F8` (`btlv_effvm.c`, overlay 94) (aletheia's
  `tools/assets/btleff.py`, its header).

`README.md` of this repository plans "a script assembler and disassembler
for the field and battle effect VMs (the opcode tables already exist in
aletheia's tools)". So the VM is the substrate the assembler and
disassembler serve, and it is a candidate substrate for rules.

Two facts limit it as a rule substrate today. The port has not wired the
script VM: eidos Milestone 7 lists "script VM" as matched but not wired into
the port (`README.md`, "Planned Milestones"; `docs/status.md`, "Milestone 7,
gameplay"). And the VM is the game's own object, with handlers that act on
game state, not on the port's event and state API
(`docs/mod-interface.md`, section 4).

## How a rule compiles or interprets

The choice is not decided. Three shapes are open, with the evidence on each
side.

1. **A rule compiles to the game's script VM bytecode, and the VM runs it.**
   Evidence for: the VM and its opcode tables exist, and the assembler this
   repository plans targets exactly this bytecode (`README.md`). Evidence
   against: the port has not run the script VM (`docs/status.md`, Milestone
   7); the VM's handlers act on game state and know nothing of the port's
   events, so a rule bound to `battle.faint` has no bytecode that reaches
   it; and the VM has no access to the mod state namespace (`sdk/save.h`).

2. **A rule compiles to a native shared library, the same as a code mod,
   with the rule language as a front end.** Evidence for: the code loader is
   the seam the port needs anyway, a dropped-in mod is a native library
   loaded at run time (`docs/mod-interface.md`, item 4); the rule then uses
   the same event and state API a code mod uses (`docs/mod-interface.md`,
   section 4); and the assembler and disassembler are not needed for it.
   Evidence against: a native library is trusted to the same degree as any
   mod, so the small language is a constraint on the author, not a safety
   boundary (`docs/mod-interface.md`, section 5).

3. **A small interpreter in the port runs the rule over a fixed opcode
   set.** Evidence for: the rule stays out of host code and the surface
   stays small. Evidence against: the port promises no sandboxing and no
   bounded resource use (`docs/mod-interface.md`, section 5); no interpreter
   seam exists in the port; and this is new port work the four eidos
   documents do not list.

The port side of the rule surface is not in the manifest schema or the
still-to-build list. `docs/mod-interface.md`, section 3, has a `settings`
key and no rule key, and item 11 is the settings surface. So a rule that runs
inside the port needs a port-side entry point that does not exist, or it
compiles to a native library (shape 2) and uses the loader item 4.

## What is not decided

- The rule language: its grammar, its state field names, and its actions.
- The compiler or interpreter, and whether it is a tool in this repository,
  a port component, or both.
- Whether a rule runs as generated native code or through an interpreter.
- The table of named state fields, and how each maps to an address or an
  accessor. Player money is an example with no field and no address.
- The message surface a rule may use (`docs/mod-interface.md`, item 11).
- Whether `whiteout.begin` is one event on two addresses or two events
  (`docs/mod-interface.md`, item 8).
- Block composition, whether any handler blocks or every handler must allow
  (`docs/mod-interface.md`, item 9).
- The authority model for Soullink: which instance decides a shared outcome,
  how two conflicting events are ordered, and what happens when one instance
  is ahead (`docs/modding.md`, "What is still to build", item 3).
- The transport, and therefore every action that sends an event to another
  instance (`docs/modding.md`, "Networking seam: missing").

## Related documents

- `docs/bw-format.md`: the `.bw` container and the manifest.
- `docs/mod-contract.md`: the tooling side of the port contract.
- eidos `docs/mod-interface.md`, section 4: the event and state API, and the
  typed wrapper.
- eidos `docs/mod-interface.md`, section 5: the trust model.
- eidos `docs/modding.md`: what the four example mods need, including the
  events Nuzlocke is built on.
- aletheia's `tools/assets/scripts.py` and `tools/assets/btleff.py`: the
  field script VM and the battle effect VM opcode tables.
