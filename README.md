# kenon

The modding suite for the native port of Pokémon Black and White.

## Status

Step 1 of `docs/briefs/kenon-plan.md` is done: the app shell, the Rust core,
the data folder setting and CI. The asset readers come next, in the order of
`docs/ui-assets-inventory.md`, section 17.

kenon reads the same data folder as [eidos](https://github.com/Project-Holon/eidos):
the extraction folder (with `data/` in it) or its `data/` folder. It reads
the user's own data at run time and ships none. Without a data folder it
uses a plain theme.

## Planned scope

- A script assembler and disassembler for the field and battle effect VMs
  (the opcode tables already exist in aletheia's tools).
- Editors for the JSON tables (species, moves, items, encounters, zones)
  and for sprite sheets and rigs, with validation against the engine.
- Mod packaging: a folder is a mod, a `.bw` is its distribution. The port
  manages mods in the game's own menu and unpacks or reads the `.bw`; the
  port owns the mod list, enable and disable, the load order, conflict
  reporting, display, controls and sound, and runs with no mods
  (`docs/bw-format.md`).
- Later: a rule surface over the engine's events for gameplay mods without
  recompiling. A rule is a small language the port fixes, run by a small
  interpreter in eidos; this repository writes rules in that grammar
  (`docs/events-and-rules.md`).

## Layout

| Path | What it is |
| --- | --- |
| `core/` | `kenon-core`: every file format and the data folder rules. Pure Rust, no Tauri, so tests and tools use it directly |
| `app/` | The Tauri 2 shell: settings and commands. It parses no game formats |
| `ui/` | The Svelte 5 + TypeScript front end. It asks the shell for everything |
| `docs/` | The mod contract, the `.bw` format, events and rules, the UI asset inventory, briefs |

## Building

Needs Rust (stable), Node 22 or later, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform.

```sh
npm ci
npm run tauri dev       # run the app
npm run check           # front-end type check
cargo test --workspace  # Rust tests
```

## Relationship to the other repositories

- [aletheia](https://github.com/Project-Holon/aletheia): the decompilation
  and the asset format knowledge kenon reuses.
- [eidos](https://github.com/Project-Holon/eidos): the engine kenon targets.

## License and legal

MIT (`LICENSE`). Kenon contains no game data and no reverse-engineered game
source; see `LEGAL.md`.
