# The `.bw` container

A `.bw` is the packed form of one mod. The port owns mod management: it
unpacks a `.bw` at startup, or reads it directly, and reads a mod directory as
it does today (`docs/mods.md`, "The layering model"). kenon owns the authoring
side: it packs a mod directory into a `.bw`, and its involvement ends when the
`.bw` exists. A mod is a directory laid out like the data root today, so the
`.bw` is a distribution format that packs that directory and unpacks back to
it. The loose directory is the development form; the `.bw` is the form a
player receives. The port runs with no mods at all, and its headless tests and
its demo do so.

This document specifies the container, which kenon writes. The port side is
`docs/mods.md` (the data surface that exists), `docs/mod-interface.md` (the
interface contract, its section 3 is the manifest and pack schema) and
`README.md`, "The file layer". Every claim about the port below is attributed
to the file it comes from.

## The loose directory, the form the port reads

A mod root is a directory laid out like the data root. There is one base
layer, the data root, and zero or more mod roots over it in a load order,
and a later layer wins (`docs/mods.md`, "The layering model"; `README.md`,
"The file layer"). A path resolves to the latest layer holding a regular
file there, the data root last (`sdk/vfs.c`, `bw_data_path`,
`bw_vfs_file_layer`).

Two granularities exist:

- A **regular file** at a NitroFS-relative path replaces that whole file
  (`docs/mods.md`, "Whole-file overrides").
- A **directory** at an archive's NitroFS path holds member overrides. Each
  file in it named `<index>` or `<index>.<ext>` replaces that NARC member,
  or adds it past the archive's end (`docs/mods.md`, "Member overrides";
  `sdk/vfs_narc.c`, `member_index`).

A mod root may hold a manifest, `eidos_mod.txt`. A missing manifest is not
an error; the defaults apply (`sdk/vfs.c`, `read_manifest`).

## The manifest

The manifest is the only metadata the port reads. The format is `key = value`
lines, `#` starts a comment, a line with no `=` is ignored, and an unknown
key is ignored on purpose so a later version can add keys (`sdk/vfs.c`,
`read_manifest`; `docs/mods.md`, "The per-mod manifest").

`docs/mod-interface.md`, section 3, fixes the schema the tooling writes and
the port reads:

| Key | Value | Default | Meaning |
|---|---|---|---|
| `name` | text | the directory name | display name |
| `order` | signed integer, -1000000 to 1000000 | 0 | load order key; a later mod wins |
| `api` | integer | 1 | the interface version the mod is built against |
| `code` | relative path | none | the mod's native library for this host |
| `state` | identifier | derived from `name` | the per-mod state namespace |
| `override` | a grammar line, repeatable | none | one (region, address) override |
| `rule` | a grammar line, repeatable | none | one rule bound to one event |
| `subscribe` | an event name, repeatable | none | a bound event, declarative only |
| `settings` | relative path | none | the mod's settings declaration |

`order` keeps the existing rule: a value that is empty, not a number, or out
of range is reported on stderr and ignored, and the default `0` stands
rather than a guessed value (`sdk/vfs.c`, `read_manifest`; `docs/mods.md`,
"The per-mod manifest"). Mods load in ascending `order`; mods with equal
`order` keep the order they were added in, and a later mod wins
(`sdk/vfs.h`, "The mod list").

The manifest keys cover two of the four kinds of the port's catalogue. The
catalogue of `docs/mod-events.md`, section 1, holds 359 entries in four kinds:
160 events, 27 override points, 81 state entries and 91 data tables. The
`override` key names an override point, and the `subscribe` and `rule` keys
name an event. A data table is not a manifest key: it is a whole-file or
member override in the data tree, keyed on the data layer rather than an
address (`docs/mod-events.md`, "The four kinds"; `docs/mods.md`). A game
state entry is reached through an accessor, not declared in the manifest; the
`state` key is the mod's own namespace, not a game state entry
(`docs/mod-events.md`, section 4).

## The packed form

The current layer reads a directory root: `bw_data_path` joins a root and a
relative path, so the root is a directory (`sdk/vfs.c`, `bw_data_path`).
`docs/mod-interface.md`, section 3, states the consequence: "A packed
single-file mod (one archive with the same tree and manifest inside) is not a
thing the current layer reads". The port owns the reader: it unpacks a `.bw`
at startup into its own cache, or reads the container directly, and reads a
mod directory as it does today. No `.bw` reader exists in the port yet, and
the choice between unpacking to a cache and reading directly is still open.

A `.bw` therefore holds the same tree the loose form holds, so that the two
forms differ only by packaging:

- `eidos_mod.txt` at the root, the manifest above.
- the data and asset tree at the NitroFS-relative layout, whole replacement
  files and member override directories alike.
- optionally the native library the `code` key names. The code loader is not
  built (`docs/mod-interface.md`, item 4): a dropped-in mod is a native shared
  library loaded at run time, and no such loader exists in the tree.
- the metadata, which is the manifest itself. `version` and `dependencies`
  are not keys the port reads; see "Metadata the port does not read".

### The container format is not decided

The `.bw` is most likely a zip. The container is kenon's to fix, because kenon
writes it; the port only reads or unpacks one.

Evidence for a zip:

- a zip carries a directory tree, so the whole-file layout and the member
  override directories both survive packing without a second format;
- the port's own first-run extractor already recognises zip as an archive
  kind when a player drops a ROM file (`docs/installer.md`, the archive
  cases), so the tooling's environment already treats zip as ordinary;
- the file layer needs no change if the port unpacks a zip into its own
  cache.

Evidence against a fixed zip:

- nothing in the port reads a `.bw`, so any container needs a reader or an
  unpack step, and a zip is only one of several choices;
- a mod is small and a zip adds a central directory and compression, which
  the port does not need and which a pack must be able to remove losslessly;
- compression would change bytes, and the file layer compares a member's
  bytes in the game's own format (`docs/mods.md`, "A worked example"), so a
  pack must store members uncompressed or unpack them unchanged.

The format, the compression policy and the file extension are not decided.

### How a packed mod reaches the port

The port owns the packed form. It unpacks a `.bw` at startup into its own
cache, or reads the container directly, and reads a mod directory as it does
today. Whether it unpacks to a cache or reads directly is still open. Either
way the unpacking is the port's, not kenon's. kenon produces the `.bw` and
stops.

The two shapes, with the evidence on each side:

- **The port unpacks the container into its own cache.** The file layer needs
  no change: `bw_data_path` joins a root and a relative path, and the roots
  are directories (`sdk/vfs.c`, `bw_data_path`; `sdk/vfs.h`, `bw_add_mod_root`,
  `bw_set_mod_root`). The unpack is the port's, at startup or at the point the
  mod roots are set.
- **The port reads the container directly.** The port gains a reader that
  resolves a NitroFS path inside the container and serves whole files and
  member directories the way a directory root does, so `bw_vfs_file_layer`
  and `bw_data_path` answer for a packed root too. The port's memory handle
  serves a merged NARC today, so this shape adds a memory form for whole
  files to `bw_data_path` and the readers that use it (`docs/mod-rules.md`,
  section 2).

The player drops a `.bw` in the mods folder and the port makes it work.

The container format, its compression policy and its extension are kenon's to
fix, because kenon writes the container. The port fixes only that it reads or
unpacks one.

## The port owns mod management

The port manages mods in the game's own menu, and a mod can be enabled without
restarting the game. The port owns the mod list, enable and disable, the load
order and conflict reporting. The port also owns display, controls and sound.
There is one place a player changes things, the game's own menu.

kenon owns the authoring side: the assembler and disassembler, the table
editors, and the packager that produces a `.bw`. Its involvement ends when
the `.bw` exists. kenon does not own the player's mod list, enable and
disable, the load order or conflict reporting.

The in-game management implies four constraints the tooling must know.

1. **The mod roots and the override registry are rebuildable at the menu.**
   The port sets the mod roots with `bw_set_mod_root`, `bw_add_mod_root` and
   `bw_clear_mods`, and every change drops the cached merged archives so the
   new view applies at the next open (`sdk/vfs.h`; `docs/mods.md`, "Setting
   the roots"). Enabling or disabling a mod at the menu rebuilds the roots
   and the override registry rather than installing them once at startup.
2. **Disabling a code mod reverts its slots before its library is unloaded.**
   The game holds pointers into a loaded mod, so the port restores each slot
   to its original before the library is unloaded.
3. **A mod's effects persist after it is disabled.** State a mod wrote, and
   anything it changed through an override's return, stays until the game
   writes over it or the port restarts. Disabling a mod stops its future
   effects; it does not undo past ones.
4. **The port starts with no mods at all.** A build with no mods defines
   every address the way a build without the override machinery does, and the
   resolver answers from the data root alone (`docs/mod-interface.md`, section
   5; `docs/mods.md`, "The layering model").

The escape hatch is the port's. The port runs with no mods, and its headless
tests and its demo do so (`docs/mods.md`, "The layering model"; `README.md`,
"Running the game's boot"). A `--no-mods` switch, and an enable list the port
reads before it loads anything, let a player turn off a mod that breaks
startup without the game.

## Metadata the port does not read

Two fields a loader usually needs are not in the port schema.

- **`version`.** The port has no version key. The manifest parser reads one
  key per line and ignores an unknown key on purpose so a later version can
  add keys (`sdk/vfs.c`, `read_manifest`), so a `version` line can sit in
  `eidos_mod.txt` today without breaking the port. Whether the port ever
  reads it, and what a version is compared against, is not decided.
- **`dependencies`.** The port has no dependency key and no resolution. The
  same forward-compatible rule lets a `dependencies` line sit in the
  manifest. Whether the loader orders by dependencies, refuses a missing
  one, or ignores the field is not decided. The `order` key is the only
  ordering the port has (`sdk/vfs.h`, "The mod list").

`priority` is the tooling's word for the port's `order`. They are one value.
A higher `order` loads later and wins more (`sdk/vfs.h`, `docs/mods.md`).

## Changing priority and load order from the game's menu

The port manages mods in the game's own menu. The port reads the list from
`bw_set_mod_root`, `bw_add_mod_root`, `bw_clear_mods`, or the `BW_MOD_ROOT`
environment variable, and re-sorts it by each mod's manifest `order`
(`sdk/vfs.h`, "The mod list"; `docs/mods.md`, "Setting the roots").
`BW_MOD_ROOT` is read once, the first time the list is consulted, unless a
call set the list first (`sdk/vfs.c`, `mods_init`), so a menu that changes
the order makes an explicit call.

The menu that changes it is not built. The settings screen exists and the
title menu reaches it (`engine/platform/settings_ui.c`,
`engine/platform/mod_options_proc.c`, `sdk/mod_title.c`; `docs/settings-design.md`),
but `docs/modding.md`, "Settings and UI: partial", states that the
mod-facing part is missing: the row table and the UI text are still eidos's
own, and there is no mod-supplied screen or menu. `docs/mod-interface.md`,
item 11, states the port must fix whether the `settings` key names a file, a
row table, or nothing.

The port owns the mod list and the order. Two shapes for the menu, with the
evidence on each side:

- **A menu row per mod that re-sorts at run time.** The row changes a mod's
  order and calls `bw_add_mod_root` or `bw_set_mod_root`, and the cached
  merged archives drop so the new view applies at the next open (`sdk/vfs.h`,
  `bw_vfs_flush`). Evidence for: the API exists and the flush exists, and the
  port owns mod management, so a mod can be enabled without restarting the
  game. Evidence against: the mod-facing row table does not exist
  (`docs/modding.md`, "Settings and UI: partial"), so the row has nowhere to
  live yet.
- **The menu writes the order to the port's own settings file.** The menu
  persists a player-chosen order, and the port reads it before it loads
  anything. The port's own settings file lives in the eidos home next to the
  extraction (`engine/platform/settings.c`, `eidos_settings_path`;
  `install/setup.c`, `bw_setup_home`; `docs/modding.md`, "Per-mod state
  persistence: partial"), which is a place such a file could live. Evidence
  for: it needs no run-time re-sort, and the port owns the settings. Evidence
  against: no file of that shape exists, and the settings file the port owns
  holds eidos's own options, not a mod order.

Neither shape is decided. The port owns both the menu and the settings, so
nothing is split across programs. Writing `order` back into a mod's own
manifest is a third option, but it edits the mod rather than the player's
choice, and nothing in the port writes a manifest.

## Pack and unpack, the tooling contract

The tooling packs a loose directory to a `.bw`. The port unpacks it, or reads
it directly. The tooling also unpacks a `.bw` for a round-trip check, and both
directions are lossless: a round trip reproduces the input bytes. `README.md`
of this repository fixes the packaging goal: a folder is a mod, a `.bw` is its
distribution.

A pack must preserve:

- the manifest at the container root;
- every whole replacement file at its NitroFS-relative path;
- every member override directory and its `<index>` and `<index>.<ext>`
  names, because a directory at an archive's path is a member override and
  its member names are the replacement indices (`docs/mods.md`, "Member
  overrides"; `sdk/vfs_narc.c`, `member_index`);
- the native library the `code` key names, when present.

A pack must not reorder, rename or recompress a member, because a member file
holds the game's own bytes and the port serves them unchanged (`docs/mods.md`,
"A worked example"; `README.md`, "The file layer").

## What is decided and what is not

Decided:

- the loose directory is the mod root the port reads, and a path resolves to
  the latest layer holding a regular file there (`sdk/vfs.c`);
- the manifest is `eidos_mod.txt`, `key = value`, `#` comments, an unknown
  key ignored on purpose (`sdk/vfs.c`, `read_manifest`);
- the `order` key, ascending load order, later wins, equal order keeps the
  add order (`sdk/vfs.h`, "The mod list");
- the `api` key is the interface version the mod is built against; the port refuses a mismatch, so a data-only mod leaves it out
  (`docs/mod-interface.md`, section 3);
- the `code` key names the mod's native library for this host; the loader is
  not built (`docs/mod-interface.md`, item 4);
- member overrides are directories of `<index>` files (`sdk/vfs_narc.c`;
  `docs/mods.md`, "Member overrides");
- the port owns mod management, in the game's own menu, and owns the mod
  list, enable and disable, the load order and conflict reporting, plus
  display, controls and sound;
- the port unpacks a `.bw` into its own cache or reads it directly, and reads
  a mod directory as it does today;
- the port runs with no mods, and a `--no-mods` switch and an enable list it
  reads before anything loads turn off a mod that breaks startup
  (`docs/mods.md`, "The layering model");
- kenon owns the authoring side and produces the `.bw`; its involvement ends
  when the `.bw` exists.

Not decided:

- whether the port unpacks a `.bw` to its own cache or reads the container
  directly;
- the container format, the compression policy and the file extension of a
  `.bw`, which are kenon's to fix;
- the `version` and `dependencies` keys, which the port does not read;
- the menu that changes a mod's priority (`docs/mod-interface.md`, item 11;
  `docs/modding.md`, "Settings and UI: partial");
- the layout of a mod's state under the save directory (`docs/mod-interface.md`,
  item 10).

## Related documents

- `docs/mod-contract.md`: the tooling side of the port contract, including
  the manifest keys and the override grammar.
- `docs/events-and-rules.md`: the event catalogue and the rule surface.
- eidos `docs/mod-interface.md`, section 3: the manifest and pack schema.
- eidos `docs/mod-rules.md`, section 2: where a packed mod is read.
- eidos `docs/mods.md`: the data surface that exists today.
- eidos `README.md`, "The file layer": the design account of the data
  surface.
