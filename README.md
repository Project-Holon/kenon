# kenon

The modding suite for the native port of Pokémon Black and White. Kenon is
the void: the empty form that takes either truth or ideals, which is what a
mod is before it is filled.

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
- Mod packaging: a folder is a mod, a zip is its distribution, conflicts are
  reported per file.
- Later: a Lua binding over the engine's events for gameplay mods without
  recompiling.

## Relationship to the other repositories

- [aletheia](https://github.com/Project-Holon/aletheia): the decompilation
  and the asset format knowledge kenon reuses.
- [eidos](https://github.com/Project-Holon/eidos): the engine kenon targets.

## License and legal

MIT (`LICENSE`). Kenon contains no game data and no reverse-engineered game
source; see `LEGAL.md`.
