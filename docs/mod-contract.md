# The mod contract, from the tooling side

This document is the tooling side of the contract the port defines. The port
side is six documents in eidos: `docs/mod-interface.md` (the interface
contract), `docs/mod-events.md` (the 18-event catalogue and the state API),
`docs/mod-rules.md` (the rule substrate and the container decision),
`docs/mod-overrides.md` (the function override keyed on a DS address),
`docs/mods.md` (the data surface that exists) and `docs/modding.md` (what the
four example mods need and what is missing). This document says what a mod may
declare and do from the tooling's point of view, and where the two sides must
agree. It points at the port documents rather than restating them.

The tooling is the authoring side of the modding suite this repository plans:
a script assembler and disassembler for the field and battle effect VMs,
editors for the JSON tables and sprite sheets with validation, and a packager
that writes a mod folder to a `.bw`. The port manages mods in the game's own
menu, so kenon's involvement ends when the `.bw` exists (`README.md`). Later a
rule surface over the engine's events for gameplay mods without recompiling
(`README.md`, "Planned scope"; `docs/mod-rules.md`, section 1).

## The key is the DS address

An override is keyed on the function's DS address, never on its name
(`docs/mod-overrides.md`, "Override by DS address, never by name"). A name is
not stable: aletheia renames and re-splits units on every cleanup wave, so a
name-based key breaks on the next wave with no warning. A DS address is fixed
for the life of the ROM.

The tooling must therefore key an override on the address. `tools/closure.py`
and `tools/bootlink.py` resolve by address, and `tools/ovlink.py`'s
trampolines ask `bw_ovl_resident_at` which overlay is resident over an address
(`docs/mod-overrides.md`). The address is the key the rest of the repository
already trusts.

An address has two forms. aletheia's `address:` line records the even
function start, and the overlay tables key on the value a code pointer holds,
which carries the Thumb low bit (`docs/mod-interface.md`, section 1;
`sdk/overlay_image.h`, `bw_ovl_code_entry`). The manifest must state which
form it uses and accept both (`docs/mod-interface.md`, section 1, item 2).
The tooling must accept both forms and must not assume one.

## The declared set is the interface

The port exposes a declared set of DS addresses. For each address in the set
it generates an indirection, a slot. An address outside the set is called
directly, as the build calls it today, and no mod can reach it
(`docs/mod-interface.md`, section 1).

The port declares the set. It publishes 44 rows in `mod/declared_set.txt`,
one per address, with the address, the aletheia function name, the region
(`arm9` or the overlay id) and the example mods that need it
(`docs/mod-interface.md`, section 1). The table in section 1 is the prose
account of the same set. The cost of a set that is too large is one rename
site and one maintenance item per address at every aletheia wave
(`docs/hooks.md`, "Limits").

The tooling must treat only the declared addresses as hookable, and it reads
the set from the port's `mod/declared_set.txt` rather than carrying a second
copy that can drift. An editor that offers an address the port does not
declare promises something the port does not do.

The set is the hookable set, not the event set. `docs/mod-events.md`,
"Declared addresses with no event", names the declared addresses that bind to
no event and the concepts that span more than one address.

## What a mod declares

The manifest is `<mod>/eidos_mod.txt`. The port reads it with `read_manifest`
(`sdk/vfs.c`), which reads `key = value` lines, drops a `#` comment, ignores
a line with no `=`, and ignores an unknown key on purpose so a later version
can add keys (`docs/mod-interface.md`, section 3). The tooling writes the
same file.

| Declaration | Key | Value the port reads |
|---|---|---|
| display name | `name` | text; default the directory name |
| load order | `order` | signed integer, -1000000 to 1000000; default 0 |
| interface version | `api` | integer; default 1 |
| native library | `code` | relative path; the loader is not built (`docs/mod-interface.md`, item 4) |
| state namespace | `state` | identifier; default derived from `name` |
| one (region, address) override | `override` | a grammar line, repeatable |
| one rule bound to one event | `rule` | a grammar line, repeatable |
| one bound event | `subscribe` | an event name, repeatable, declarative only |
| settings declaration | `settings` | relative path; the surface is not built (`docs/mod-interface.md`, item 11) |

### The override declaration

The `override` value grammar (`docs/mod-interface.md`, section 3):

```
<region>:<address> <mode> <symbol> [sig="<prototype>"] [switch=<name>]
```

- `<region>:<address>` is the pair `(region, address)`. `<region>` is `arm9`
  or an overlay id, the spelling `mod/declared_set.txt` uses, and `<address>`
  is a `0x` prefixed DS address. Both the even start and the Thumb-bit value
  are accepted (`docs/mod-interface.md`, sections 2 and 1, item 2). The bare
  address is not a key: two overlays share one RAM window, so `0x021C5200`
  alone names two functions (`docs/mod-interface.md`, section 1).
- `<mode>` is `replace` or `wrap`. `wrap` calls the original, which the slot
  keeps callable (`docs/mod-overrides.md`, "What an override declares";
  `docs/mod-interface.md`, section 2).
- `<symbol>` is the C symbol the mod exports for this address.
- `sig=` names the prototype the caller expects, quoted. When present, the
  generator checks it with the same normaliser the trampolines use and
  refuses a mismatch rather than calling through the wrong type
  (`tools/ovlink.py`, `signature`; `docs/mod-overrides.md`, "What an override
  declares"). The tooling must emit the prototype the caller expects, not the
  replaced function's own, and must let the port refuse a mismatch.
- `switch=` names the on/off key `bw_hook_enabled` answers, defaulting to a
  name derived from the address (`sdk/mod_hooks.h`).

A repeated `override` line is the mechanism for the conflict rule. Mods load
in ascending `order`, and for one address the later mod wins and the earlier
is reported as shadowed (`docs/mod-overrides.md`, "Two mods on one address").
The tooling reports the same conflict per address, and per file for data
(`README.md`, "Planned scope").

The port requires one symbol per object for the static-archive rule to work,
so two overrides in one object would break it (`docs/mod-overrides.md`,
"still to build" item 8). The tooling must emit one object per address, or
leave the packing to the port's registry.

### The event declaration

A mod can register a handler in its code, and a `subscribe` line lets the
port report a conflict before the mod runs. The work is to fix which form the
port reads (`docs/mod-interface.md`, item 7). The event name is the permanent
binding key: renaming one later breaks every mod bound to it
(`docs/mod-interface.md`, section 4). The catalogue is the port's, 18 events
in `docs/mod-events.md`, section 1, and the tooling uses those names and does
not rename them. `docs/events-and-rules.md` follows the same catalogue.

A mod that does not write C declares a rule under the `rule` key instead. The
port decided the rule substrate: a small interpreter in the port runs the
rule, and the tooling owns the language (`docs/mod-rules.md`, section 1;
`docs/mod-events.md`, section 5).

## What a mod may do

- **Override a function**, by address, in `replace` or `wrap` mode
  (`docs/mod-interface.md`, section 3).
- **Subscribe to an event.** The port fires an event when a declared slot is
  entered, calls the mod's handler with a fixed struct, and then calls the
  original unless the handler blocks (`docs/mod-interface.md`, section 4).
- **Declare a rule.** A rule is code in a small language the tooling owns,
  run by a small interpreter in the port at the event slot
  (`docs/mod-rules.md`, section 1; `docs/mod-interface.md`, "The rule
  surface").
- **Read game state.** Party and box records, species, level and HP, the
  current map, and the save flags. A mod reaches them through the DS address
  registry (`bw_ds_ptr`, `sdk/dsmem.h`) or through the matched accessors the
  port defines (`POKE_TOOL_*`, `GAME_DATA_*`) (`docs/mod-interface.md`,
  section 4).
- **Write the same bytes.** The port does not arbitrate a write. When a mod
  writes something the game then contradicts, the game's later write wins
  (`docs/mod-interface.md`, section 4).
- **Block an action.** A block is the return of a block on a slot that
  defines one, not a state write (`docs/mod-interface.md`, section 4;
  `docs/modding.md`, "What block an action means").
- **Keep state across sessions.** A mod's state lives under its `state`
  namespace, written whole and atomically (`sdk/save.h`, `bw_save_path`,
  `bw_save_read`, `bw_save_write_atomic`; `docs/mod-interface.md`, section 4).
- **Show the player something.** Partial: a mod can replace or add whole NARC
  members today, but there is no mod-supplied screen or menu
  (`docs/modding.md`, "Settings and UI: partial").
- **Exchange events between instances.** Not built. `sdk/arm7/` has no Wm or
  DWC responder and there are no sockets in the tree
  (`docs/modding.md`, "Networking seam: missing").

Two write facts change what a mod can rely on (`docs/mod-interface.md`,
section 4):

- an ARM9 save-block field written at `save.begin` persists until
  `bw_ds_reset` or a later game write, so it is durable state;
- an overlay's data is reset on every load, so a write into an overlay's data
  does not survive the next load of that overlay or of any overlay sharing
  its window.

A change that must stick is made at the event the game derives the state from,
or through the override's return, not as a write the game recomputes.

## What the port promises and does not promise

The trust model is `docs/mod-interface.md`, section 5. A mod is native C,
loaded into the port's process, and runs with the port's full privilege. It
can read and write any memory the process holds, call any function, corrupt
state, hang, and crash the process. A mod can defeat the on/off switch and
the conflict report, because both are in the same address space it runs in.

The port promises, and only promises:

- a deterministic load order, a later-wins rule, and a visible conflict
  report (`sdk/vfs.h`, "The mod list"; `docs/mod-overrides.md`, "Two mods on
  one address");
- the `bw_hook_enabled` switch over a slot, so one mod's override can be
  isolated while debugging (`sdk/mod_hooks.h`);
- that a build with no mods defines every address the way a build without
  the override machinery does, because matched files are never edited
  (`README.md`, portability rule 1; `docs/mod-overrides.md`, "The base is
  never edited").

The port does not promise isolation, sandboxing, bounded resource use,
validation of a mod's reads or writes, or recovery from a bad write. It does
not promise a stable ABI across `api` versions: a mismatch is refused, not
translated (`docs/mod-interface.md`, section 5, item 5). The correct model is
that a mod is a build of the port's own code by another author, trusted to
the same degree.

The consequence for the tooling: the tooling may validate a mod against the
engine, and `README.md` says the editors validate. The port does not validate
a mod. A validation failure is the tooling's refusal, not a port guarantee,
and the tooling must not present validation as a sandbox.

## Where the two must agree

The two sides meet at these points. A change on one side is a change to the
other.

| Point | Port side | Tooling side |
|---|---|---|
| the key | the DS address (`docs/mod-overrides.md`) | the tooling keys on the address, never the name |
| the address form | even start and Thumb-bit value accepted, or one fixed and converted (`docs/mod-interface.md`, item 2) | the tooling accepts both and states which one a manifest uses |
| the hookable set | the declared set, published at `mod/declared_set.txt` (`docs/mod-interface.md`, section 1) | the tooling reads the set from the port and treats only declared addresses as hookable |
| the manifest | `eidos_mod.txt`, `key = value`, unknown key ignored (`sdk/vfs.c`, `read_manifest`) | the tooling writes the same file and the same keys |
| the override grammar | `<region>:<address> <mode> <symbol> [sig=] [switch=]` (`docs/mod-interface.md`, section 3) | the tooling emits this grammar and one object per address |
| the event names | the permanent binding key, 18 events (`docs/mod-events.md`, section 1) | the tooling uses those names and never renames one |
| the rule substrate | a small interpreter in the port runs a rule (`docs/mod-rules.md`, section 1) | the tooling owns the language and compiles to the interpreter (`docs/events-and-rules.md`) |
| the interface version | `api`, and the port fixes what a mismatch does (`docs/mod-interface.md`, item 5) | the tooling records the `api` the mod was built against |
| the conflict rule | later in the load order wins, the earlier is reported as shadowed at run time (`docs/mod-overrides.md`) | the tooling reports a conflict it can see at pack time; the port owns the run-time report |
| the mod list | the port owns mod management in the game's own menu, and enable and disable without a restart | kenon produces the `.bw` and stops; it does not own the list or the order |
| the state namespace | `state`, one file or folder under the save directory (`docs/mod-interface.md`, section 3, item 10) | the tooling names the namespace and does not share it between mods |

Two open points the tooling must watch, because a tool that assumes an answer
will be wrong when the port fixes it:

- **`whiteout.begin` spans two addresses.** One concept, two functions, the
  battle-screen half and the field-recover half. The port must fix whether
  one event name binds both or two events exist
  (`docs/mod-events.md`, section 1, still-to-build item 1). The tooling must
  not assume one binding.
- **Block composition.** The port must fix whether any handler blocks or
  every handler must allow. The conservative rule is any-blocks, and the port
  reports both (`docs/mod-events.md`, still-to-build item 8). The tooling must
  not assume one rule.

## What the tooling does not do

The tooling does not add a port capability. It assembles and disassembles
scripts, edits the JSON tables and sprite sheets with validation, packages a
mod folder to a `.bw`, and compiles a rule for the port's interpreter
(`README.md`; `docs/mod-rules.md`, section 1). It does not manage the player's
mods: the port owns the mod list, enable and disable, the load order and the
run-time conflict report (`docs/bw-format.md`). It must not claim a port
capability the eidos documents do not describe.

Where a mod needs something the port does not expose, the gap is on the port
side. `docs/modding.md`, "The gap list, in priority order", and
`docs/mod-interface.md`, "Still to build", carry those gaps: the run-time
seam, the transport, the event and state API, per-mod persistent state, the
mod-facing UI, composition, the address-keyed override, the code loader, and
the packed single-file mod.

## Related documents

- `docs/bw-format.md`: the `.bw` container and the manifest.
- `docs/events-and-rules.md`: the event catalogue and the rule surface.
- eidos `docs/mod-interface.md`: the hookable set, the slot, the manifest
  schema, the event and state API, and the trust model.
- eidos `docs/mod-events.md`: the 18-event catalogue, the state API and the
  ABI surface.
- eidos `docs/mod-rules.md`: the rule substrate decision and the container
  decision.
- eidos `docs/mod-overrides.md`: the function override keyed on a DS address.
- eidos `docs/modding.md`: what the four example mods need and what is
  missing.
- eidos `docs/mods.md`: the data surface that exists today.
