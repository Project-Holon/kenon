# kenon

The modding suite for the native port of Pokémon Black and White.

## Status

Not started. It begins once [eidos](https://github.com/Project-Holon/eidos)
runs the game from the unpacked data folder. The design it will build on is
in aletheia's `docs/DISTRIBUTION.md`: a human-readable data root, per-file
mod overlays with a load order, script text that round-trips, and a
documented C API for the engine's loaders.

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
  recompiling. A rule is a small language this repository owns, run by a
  small interpreter in eidos (`docs/events-and-rules.md`).

## Relationship to the other repositories

- [aletheia](https://github.com/Project-Holon/aletheia): the decompilation
  and the asset format knowledge kenon reuses.
- [eidos](https://github.com/Project-Holon/eidos): the engine kenon targets.

## License and legal

MIT (`LICENSE`). Kenon contains no game data and no reverse-engineered game
source; see `LEGAL.md`.
