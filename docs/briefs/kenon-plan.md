# Brief: kenon, first build

Read this first, then `README.md`, `LEGAL.md`, and the four docs in `docs/`.

## What kenon is

kenon is the modding suite for eidos, the native port of Pokemon Black and
White. It edits the game's data, writes scripts, and packs mods. It targets
the eidos data root (the unpacked game data) and the mod contract in
`docs/mod-contract.md`, `docs/bw-format.md` and `docs/events-and-rules.md`.

## Decisions already made

- **Stack:** Tauri 2, Svelte 5 (with TypeScript) for the front end, and a Rust
  core for every file format. No Electron. The front end never parses game
  formats. It asks the Rust core through Tauri commands.
- **UI library in the game's style.** kenon has its own Svelte component
  library that looks and behaves like Black and White's UI: window frames,
  talk boxes, menu lists with the cursor, yes/no, touch buttons with pressed
  states, pocket tabs, HP and EXP bars, type and status icons, the game font,
  and the UI sounds. Use a headless base (bits-ui, the base of shadcn-svelte)
  for behaviour and accessibility. The game's look is the skin on top.
- **No game data ships.** All graphics, fonts, palettes and sounds are read
  at run time from the user's own extracted data folder. The repository holds
  code, layout numbers taken from the decompiled code, and member maps
  (archive id and index). It holds no pixels, no game text, no extracted
  files. If the data folder is missing, the UI falls back to a plain theme.
- **The truth is the code.** Sizes, padding, colours, timings and cursor
  behaviour come from aletheia's decompiled UI code, with the source location
  noted. Do not guess from screenshots.

## Inputs

- `docs/ui-assets-inventory.md`: where every UI element lives (archive,
  member, format), the layout facts with source locations, the gaps, and the
  reader port order (its section 17). Treat it as the map.
- aletheia (the decompilation): format knowledge in `tools/assets/` and
  `docs/ASSETS.md`, the UI code in `src/game/`.
- eidos: working C readers in `sdk/narc.c`, `engine/g2d/` (NCLR, NCGR, NSCR,
  NCER, NANR, LZ), `engine/audio/` (SDAT).

## Order of work

1. **Scaffold.** A Tauri 2 + Svelte 5 + TypeScript app with a Cargo workspace:
   `core/` (a pure Rust library with no Tauri dependency, so tests and a CLI can
   use it) and `app/` (the Tauri shell). Add a settings screen to choose the
   data folder. Add CI that builds and runs `cargo test` and the front-end
   checks.
2. **Rust readers, in the inventory's order.** Start with NARC and LZ10/LZ11,
   then NCLR/NCGR/NSCR, then NFTR (font and glyph widths), the message bank
   decoder, NCER/NANR, and SDAT for the UI sounds. Each reader has unit tests
   on synthetic data made in the test, plus an optional test against the
   user's data folder that is skipped when no folder is set.
3. **Asset bridge.** A Tauri command (or a custom URI protocol) that serves
   decoded RGBA images, nine-slice frame parts, font atlases and sound clips to
   the front end, with an in-memory cache.
4. **UI library.** Build the components on top of the bridge. Start with the
   window frame, talk box, menu list with cursor, yes/no, button, tabs, bars and
   icons. Make a gallery page that shows each component, with a switch for
   real data and the fallback theme.
5. **First editors.** Table editors for species, moves, items, encounters and
   zones, with validation. Read and write the eidos data root in its
   human-readable form, as a mod overlay folder (`docs/bw-format.md`), never
   in place.
6. **Basic scripting.** A disassembler and assembler for the field script VM,
   built on the opcode tables in aletheia's tools. The text must round-trip
   byte for byte. Add an editor view with syntax highlighting.

Steps 2 to 4 give the visible result: a game-styled UI running on the user's
own data. Steps 5 and 6 make kenon useful.

## Open items from the inventory (gaps)

- No NFTR reader exists in eidos. aletheia `font.py` is the only reference.
  The width chunk (CWDH) is not decoded.
- Some UI data lives in code (the item icon table, the icon palette table,
  the cursor tile, the field menu and touch bar tables). Bake these to JSON
  at an install step from the user's ARM9 and overlay images, or keep them
  as numeric tables taken from the decompiled code. Copy no bytes.
- The talk balloon shape is code (a 3D polygon), so reimplement it from
  `TALKMSGWIN_BuildShape` and `CalcTail`.
- The name entry keyboard layout and the C-Gear skin records are not decoded.

## Rules

- Plain English. No em dashes, no emoji in code, docs or commits.
- No personal names, usernames or absolute paths in tracked files. The data
  folder is a setting, never a constant in the code.
- No game data, no ROMs, no extracted files in git. Test fixtures are made
  in the tests.
- No AI attribution lines in commits.
- Stage explicit files. Commit and push to `Project-Holon/kenon` after each
  working step.
- eidos and aletheia belong to other sessions. Do not edit them. If kenon
  needs something from them, write it down and tell the user.
