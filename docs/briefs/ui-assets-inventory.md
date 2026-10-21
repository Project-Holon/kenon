# Brief: UI asset inventory for kenon's UI library

Read-only investigation. Write one report, nothing else.

## Goal

kenon (Tauri + Svelte + a Rust core) gets a UI library that looks and
behaves like Pokemon Black's own UI. The look is loaded at run time from the
user's own extracted game data (the eidos data folder); nothing from the game
is shipped. Find out whether everything such a library could ever need is in
that data, where each piece is, what format it is in, which game code uses
it, and whether aletheia or eidos already has a reader for the format.

Be thorough: list every UI element the game has, not only the common ones.

## Where to look

- Extracted game data (read-only, never copy it anywhere):
  aletheia's `extract/data` (the `a/x/y/z`
  NARC archives and named files).
- aletheia (the decompilation repository). Its asset
  converters are in `tools/assets/`, its notes in `docs/ASSETS.md`, the
  archive id table the game uses in `src/game` (ARC_TOOL / ARC_UTIL calls,
  the archive path table), and the decompiled UI code under `src/game/`:
  bmp_win, bmp_menu, bmp_menulist, printsys, gf_font, app_taskmenu,
  app_keycursor, touchbar, the field menu, the yes/no window, the bag,
  the party list, the summary screen, the name entry, the options menu, the
  battle UI (btlv_*), and any other UI you find.
- eidos (the port repository). Its file layer is
  `sdk/vfs.c`; its own UI (settings screen) loads game graphics in
  `engine/platform/settings_ui.c` and `engine/platform/mod_options_proc.c`;
  its 2D engine reads the formats in `engine/g2d/`.

## Categories to cover (add any you find)

Window frames and talk boxes (all styles, including the field talk window,
the information window, the battle message box), menu lists and the cursor,
yes/no, touch-screen buttons and their pressed states, tabs (bag pockets),
scroll arrows, the field menu (bottom screen), the C-Gear and touch bar,
fonts (every size and variant, with the glyph widths), palettes (per screen,
text colour sets), background patterns, type icons, status icons, item
icons, Pokemon icons and sprites, trainer graphics, HP and EXP bars, number
entry, the summary card, the trainer card, name entry keyboard, the options
screen, sounds the UI plays (cursor, confirm, cancel, error), and any
animation data (cell animations) the UI uses.

## For each element, record

- Archive and member (path or NARC id and index), as the game's code names
  it (the ARC id constant and the call that loads it, with file:function).
- Format (NCGR, NCLR, NSCR, NCER, NANR, NCBR, font format, SDAT for
  sounds, other).
- Layout facts the code gives (sizes, positions, padding, timing), with the
  source location.
- Whether a decoder exists: aletheia `tools/assets/...`, eidos `engine/...`
  or `sdk/...`, or none.

## Report

Write `docs/ui-assets-inventory.md` in this repository:
a table per category, then a "Gaps" section (missing decoders, elements not
found, formats not understood), then a short "Readers to port to Rust" list
in priority order. Paths, ids and format names only: no pixel data, no
copied text from the game, no extracted files.

Do not modify any other file. Do not spawn agents. Plain English, no em
dashes, no emoji.
