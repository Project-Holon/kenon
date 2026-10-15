# The `.bw` container

A `.bw` is the packed form of one mod. It is what a player drops in a folder
and the loader turns on. A mod is a directory laid out like the data root
today, so the `.bw` is a distribution format that packs that directory and
unpacks back to it. The loose directory is the development form; the `.bw`
is the form a player receives.

This document specifies the container as far as the port side fixes it, and
marks the rest as undecided. The port side is `docs/mods.md` (the data
surface that exists), `docs/mod-interface.md` (the interface contract, its
section 3 is the manifest and pack schema) and `README.md`, "The file layer".
Every claim about the port below is attributed to the file it comes from.

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
| `override` | a grammar line, repeatable | none | one address override |
| `subscribe` | an event name, repeatable | none | a bound event, declarative only |
| `settings` | relative path | none | the mod's settings declaration |

`order` keeps the existing rule: a value that is empty, not a number, or out
of range is reported on stderr and ignored, and the default `0` stands
rather than a guessed value (`sdk/vfs.c`, `read_manifest`; `docs/mods.md`,
"The per-mod manifest"). Mods load in ascending `order`; mods with equal
`order` keep the order they were added in, and a later mod wins
(`sdk/vfs.h`, "The mod list").

## The packed form

The current layer reads a directory root: `bw_data_path` joins a root and a
relative path, so the root is a directory (`sdk/vfs.c`, `bw_data_path`).
`docs/mod-interface.md`, section 3, states the consequence: "A packed
single-file mod (one archive with the same tree and manifest inside) is not a
thing the current layer reads", and item 13 makes a reader for it work still
to build. No `.bw` reader exists in the port.

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

The `.bw` is most likely a zip. No eidos document fixes a container, so the
choice is open.

Evidence for a zip:

- a zip carries a directory tree, so the whole-file layout and the member
  override directories both survive packing without a second format;
- the port's own first-run extractor already recognises zip as an archive
  kind when a player drops a ROM file (`docs/installer.md`, the archive
  cases), so the tooling's environment already treats zip as ordinary;
- the file layer needs no change if the launcher unpacks a zip to a
  directory.

Evidence against a fixed zip:

- nothing in the port reads a `.bw`, so any container needs a reader or an
  unpack step, and a zip is only one of several choices;
- a mod is small and a zip adds a central directory and compression, which
  the port does not need and which a pack must be able to remove losslessly;
- compression would change bytes, and the file layer compares a member's
  bytes in the game's own format (`docs/mods.md`, "A worked example"), so a
  pack must store members uncompressed or unpack them unchanged.

The format, the compression policy and the file extension are not decided.

### How a packed mod reaches the port is not decided

Two shapes reconcile the packed form with the directory root the port reads.

1. **The launcher unpacks a `.bw` to a directory and points a mod root at
   it.** The file layer needs no change: `bw_add_mod_root` appends one root
   and re-sorts, and `bw_set_mod_root` replaces the list (`sdk/vfs.h`,
   `docs/mods.md`, "Setting the roots"). The unpack is the tooling's job and
   the unpacked directory is the loose form.

2. **The port gains a reader for the packed form.** `docs/mod-interface.md`,
   item 13, is this work. It needs a reader that resolves a NitroFS path
   inside the container and serves whole files and member directories the
   way the directory root does, so `bw_vfs_file_layer` and `bw_data_path`
   answer for a packed root too.

Evidence for shape 1: the API and the flush exist, every change to the roots
drops the cached merged archives so the new view applies at the next open
(`sdk/vfs.c`, `bw_vfs_flush`; `docs/mods.md`, "The layering model"), and no
port work is needed. Evidence for shape 2: it keeps the mod a single file the
player drops in, which is the goal the run-time seam has
(`docs/modding.md`, "A player-side mod should not require recompiling the
port"), and it avoids an unpack step and a second copy on disk.

The choice is not decided.

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

## Changing priority and load order from the settings screen

The port reads the list from `bw_set_mod_root`, `bw_add_mod_root`,
`bw_clear_mods`, or the `BW_MOD_ROOT` environment variable, and re-sorts it
by each mod's manifest `order` (`sdk/vfs.h`, "The mod list"; `docs/mods.md`,
"Setting the roots"). `BW_MOD_ROOT` is read once, the first time the list is
consulted, unless a call set the list first (`sdk/vfs.c`, `mods_init`), so a
screen that changes the order must make an explicit call.

The screen that would change it is not built. The settings screen exists and
the title menu reaches it (`engine/platform/settings_ui.c`,
`engine/platform/mod_options_proc.c`, `sdk/mod_title.c`; `docs/settings-design.md`),
but `docs/modding.md`, "Settings and UI: partial", states that the
mod-facing part is missing: the row table and the UI text are still eidos's
own, and there is no mod-supplied screen or menu. `docs/mod-interface.md`,
item 11, states the port must fix whether the `settings` key names a file, a
row table, or nothing.

So changing a mod's priority from the settings screen is not decided. Two
candidate shapes, with the evidence on each side:

- **A settings row per mod that re-sorts at run time.** The row changes a
  mod's order and calls `bw_add_mod_root` or `bw_set_mod_root`, and the
  cached merged archives drop so the new view applies at the next open
  (`sdk/vfs.h`, `bw_vfs_flush`). Evidence for: the API exists and the flush
  exists. Evidence against: the mod-facing row table does not exist
  (`docs/modding.md`, "Settings and UI: partial"), so the row has nowhere to
  live yet.
- **The screen writes the order to a file the launcher reads.** The screen
  persists a player-chosen order, and the launcher passes it to
  `bw_set_mod_root` before the game runs. The port's own settings file lives
  in the eidos home next to the extraction (`engine/platform/settings.c`,
  `eidos_settings_path`; `install/setup.c`, `bw_setup_home`;
  `docs/modding.md`, "Per-mod state persistence: partial"), which is a place
  such a file could live. Evidence for: it needs no run-time re-sort. Evidence
  against: no port or tooling file of that shape exists, and the settings
  file the port owns holds eidos's own options, not a mod order.

Neither shape is decided. Writing `order` back into a mod's own manifest from
the screen is a third option, but it edits the mod rather than the player's
choice, and nothing in the port writes a manifest.

## Pack and unpack, the tooling contract

The tooling packs a loose directory to a `.bw` and unpacks a `.bw` to a loose
directory. Both directions are lossless, and a round trip reproduces the
input bytes. `README.md` of this repository fixes the packaging goal: a folder
is a mod, a zip is its distribution, conflicts are reported per file.

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

Decided, on the port side:

- the loose directory is the mod root the port reads, and a path resolves to
  the latest layer holding a regular file there (`sdk/vfs.c`);
- the manifest is `eidos_mod.txt`, `key = value`, `#` comments, an unknown
  key ignored on purpose (`sdk/vfs.c`, `read_manifest`);
- the `order` key, ascending load order, later wins, equal order keeps the
  add order (`sdk/vfs.h`, "The mod list");
- the `api` key is the interface version the mod is built against, default 1
  (`docs/mod-interface.md`, section 3);
- the `code` key names the mod's native library for this host; the loader is
  not built (`docs/mod-interface.md`, item 4);
- member overrides are directories of `<index>` files (`sdk/vfs_narc.c`;
  `docs/mods.md`, "Member overrides").

Not decided:

- the container format, the compression policy and the file extension of a
  `.bw`;
- whether the port reads a `.bw` directly or the launcher unpacks it
  (`docs/mod-interface.md`, item 13);
- the `version` and `dependencies` keys, which the port does not read;
- the settings screen that changes a mod's priority (`docs/mod-interface.md`,
  item 11; `docs/modding.md`, "Settings and UI: partial");
- the layout of a mod's state under the save directory (`docs/mod-interface.md`,
  item 10).

## Related documents

- `docs/mod-contract.md`: the tooling side of the port contract, including
  the manifest keys and the override grammar.
- `docs/events-and-rules.md`: the event catalogue and the rule surface.
- eidos `docs/mod-interface.md`, section 3: the manifest and pack schema.
- eidos `docs/mods.md`: the data surface that exists today.
- eidos `README.md`, "The file layer": the design account of the data
  surface.
