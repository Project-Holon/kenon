# UI asset inventory for kenon's UI library

Read-only survey of what Pokemon Black's own UI needs, where each piece lives in the
extracted game data, which game code loads it, and whether a reader exists today.
Nothing from the game is copied here: only archive ids, member numbers, format names,
sizes, addresses and code locations.

Marks used in the tables:

- [V] checked in this pass, in the code or in the bytes of the data folder.
- [I] inferred from code shape or from how several screens reuse the same member; not
  proven.

Names in the form `aletheia:path` and `eidos:path` are paths inside those repositories.
Line numbers are from the pinned aletheia tree this pass read.

## 0. How the game addresses UI graphics

**Archive ids are the decimal spelling of the path [V].** `a/X/Y/Z` has id `100X+10Y+Z`.
The game's table of 235 archive paths (ARM9 0x020A6C58, `aletheia:src/game/arm9/arc_tool.c:169`)
holds exactly these names in order; all 235 entries match the rule. Examples used below:
id 83 (0x53) is `a/0/8/3`, id 23 (0x17) is `a/0/2/3`, id 5 is `a/0/0/5`.

**Container.** Every UI archive is a NARC (BTAF + GMIF). Some members are wrapped in
BIOS compression (type 0x10 LZ77 or 0x11 LZ11); the loaders take a "compressed" flag.
Message text is not in these archives: it is in `a/0/0/2` and `a/0/0/3` banks (see 2.4).

**Resource formats on disk** (magic is stored byte-reversed, so the file shows the
reverse of the name):

| name | on-disk magic | content here |
| --- | --- | --- |
| NCLR | `RLCN` | BGR555 palettes. Text palettes are one 16-colour palette (32 bytes); most others hold 16 sub-palettes (512 bytes). |
| NCGR | `RGCN` | 4bpp tiled character data (format 3) in every UI member I decoded; a few 8bpp. |
| NSCR | `RCSN` | tile map ("screen"); 8228-byte members are 64x64 entries, 2084-byte members 32x32. |
| NCER | `RECN` | cell (OAM group) bank. |
| NANR | `RNAN` | cell animation sequences. |
| NFTR | `RTFN` | fonts (`a/0/2/3`). |
| NMCR / NMAR | | multi-cell rigs, used by Pokemon and trainer battle sprites, not by plain UI. |

**Loader vocabulary in the decompiled source.** The `ARC_UTIL_*` names are sometimes
misleading; this is what they do [V, by their call arguments]:

- `ARC_UTIL_UploadPaletteFromArchive(Whole)` / `...FromHandle(Whole)`: an NCLR member into
  palette RAM (slot = byte offset / 32, "whole" = the full member).
- `ARC_UTIL_UploadBgCharFromArchive/Handle`, `ARC_UTIL_RegisterObjCharFrom*`: an NCGR
  member into tile memory.
- `ARC_UTIL_UploadBgPaletteFrom*`, `ARC_UTIL_LoadPaletteData*`, `ARC_UTIL_LoadScreenData*`:
  despite the "Palette" in the name, the ones that take a tile-map member (for example
  `OPTION_MENU_InitGfx` passes member 0xE to it) read an NSCR.
- `CLACT_RegisterPalette(Ex)`, `CLACT_RegisterCharacter`, `CLACT_RegisterCellAnim(arc,
  NCER, NANR)`: a sprite is four members: NCLR + NCGR + NCER + NANR.
- `GF_FONT_Create(archive, member, ...)`: opens an NFTR member.

**Misleading getters.** `aletheia:src/game/arm9/app_icon.c` names its member getters by
position, not by role. Examples: `APP_ICON_GetTypePaletteFileId(m)` returns an NCER
(`m+0x3b`), `APP_ICON_GetTypeCellFileId(t)` returns an NCGR (`t+0x22`),
`APP_ICON_GetSealCellFileId` returns an NCLR (11). Section 5 lists the real roles
from the NARC magic of each member and from the callers.

**Sets in triples.** Many sprite sets have three NCGR/NCER/NANR variants selected by the
OBJ VRAM mapping mode in use ("mapping" 0, 1, 2 in the getters) [V for the icon cells,
`aletheia:src/game/arm9/poke_tool_ext.c:503-545`]. A reader must pick the variant that
matches the layout it renders.

**Data that is in the code, not in an archive.** Several UI pieces live in ARM9 or
overlay data, so a data-folder-only reader cannot see them. They are listed in section 14.

## 1. Window frames and talk boxes

Primary archive: `a/0/0/5` (id 5), 13 members: 0-4 NCLR (each 16 sub-palettes), 5-12 NCGR
[V, decoded]. Dimensions below are tiles (w x h), all 4bpp tiled.

| element | archive / member | format | loaded by | layout facts | decoder |
| --- | --- | --- | --- | --- | --- |
| Field talk window frame palette | a/0/0/5 #0 | NCLR | `TALKMSGWIN_LoadFrameGraphics` (aletheia:src/game/ov021/talkmsgwin.c:1037) | one 32-byte palette at slot `framePalette` | NCLR: A battle_sprite.py, E g2d/resources.c |
| Field talk window frame tiles | a/0/0/5 #5 (16x1 tiles) | NCGR | same, talkmsgwin.c:1038 | `frameBaseTile` chosen by caller; frame drawn with tile pieces by `TALKMSGWIN_DrawSimpleFrame` / `DrawCorneredFrame` / `DrawFrameByType` | NCGR: A background.py, E resources.c |
| Speech balloon (tail) texture and palette | not in any archive: ov021 data 0x021DDA84 (one 4bpp tile, 0x20 bytes) and 0x021DDAA4 (16 colours) | raw | `TALKMSGWIN_UploadTexture` (talkmsgwin.c:1041-1051) | drawn as a 3D polygon; the shape comes from `TALKMSGWIN_BuildShape` / `CalcTail` (code, no asset); styles: plain window, upper balloon (tail below), lower balloon (tail above), auto | none (code data) |
| Message window bg frame tiles ("BG window") | a/0/0/5 #6 (6x3) | NCGR | `FIELD_MSGBG_LoadBgWinGraphics` (ov021/field_msgbg.c:1849-1855) | uploaded at tile 0x33; border drawn by `FIELD_MSGBG_DrawBgWinFrame` (field_msgbg.c:1857): 2 tiles left of x, 1 above y, width and height inside | NCGR as above |
| Message window bg palettes (4 window types) | a/0/0/5 #1 | NCLR | field_msgbg.c:1854 | copies 32 bytes from offset `winType << 5`; `winType >= 4` falls back to 0, so 4 types | NCLR |
| Special window palette | a/0/0/5 #2 | NCLR | `FIELD_MSGBG_LoadSpWinPalette` field_msgbg.c:1934 | one palette to `palSlot*32` | NCLR |
| Nine-slice bitmap frame tiles | a/0/0/5 #8 (8x3 = 24 tiles) | NCGR | `FIELD_MSGBG_LoadFrameTile` / `DrawFrameBmp` (field_msgbg.c:1939, 1964-) | tile index 9 centre, 1 top, 0x11 bottom ... blitted 8x8 into a bitmap; each frame set is 8 tiles (32 bytes each) | NCGR |
| System window frame | a/0/0/5 #10 (6x3 = 18 tiles) | NCGR | `FIELD_MSGBG_LoadSysWindowFrameGraphics` field_msgbg.c:2212 | tiles 0x1F..0x30, palette 13 (`DrawSysWindowFrame`) | NCGR |
| Menu list window frame (3x3 nine-slice) | a/0/0/5 #9 (and #11, same size) tiles + #3 (and #0) palette | NCGR + NCLR | `BMP_MENUWORK_LoadCharAndPltt` (aletheia:src/game/arm9/bmp_menuwork.c:173-215); ids from the byte tables at ARM9 0x0209E8C8 (char) and 0x0209E8CA (palette) [V bytes: 9,11 and 3,0] | `BMP_MENUWORK_DrawFrame` (bmp_menuwork.c) puts 8 tiles around a rectangle: corners tile+0,+2,+6,+8, edges +1,+7,+3,+5; every caller I found passes frame index 0, so #9 + #3 is the style in use; #11 + #0 looks unused [I] | NCGR/NCLR |
| Key-wait arrow in talk window | a/0/0/5 #12 (1 tile) | NCGR | `FIELD_MSGBG_Create` field_msgbg.c:416 | blitted 8x8 at (bmp w - 8, bmp h - 8 - offset); animation: 3 frames, advance every 4 ticks (`FIELD_MSGBG_AnimateCursor` field_msgbg.c:2175); offset table in ov021 data 0x021D3E50 | NCGR |
| Large 32x32-tile graphic, purpose unknown | a/0/0/5 #7 (32768 bytes) | NCGR | no immediate-id loader found [I: unused or loaded through a variable id] | | NCGR |
| Palette #4 | a/0/0/5 #4 | NCLR | no immediate-id loader found | | |
| Top-screen info bar (clock, link and signal marks) | a/0/5/0 (id 50): #0 NCLR, #1 NCGR (16x8 tiles) | NCLR + NCGR | `INFOWIN_LoadGraphics` (aletheia:src/game/arm9/infowin.c:480) | 32x2 tile strip; clock tiles written at x=9 (3x2), link mark at x=13 (6x2), a 7x2 block at x=19 (`INFOWIN_DrawTask`, infowin.c:375-389); clock from the RTC (`INFOWIN_UpdateClock`) | NCLR/NCGR |
| Wireless link icon (top right) | a/0/0/0 (id 0): #1 NCLR, #2 and #4 NCGR | NCLR + NCGR | `GFL_NET_WM_ICON_LoadGraphics`, aletheia:src/game/arm9/wm_icon.c:318-336 | uploaded as OBJ tiles; variant selects #4 or #2 | NCLR/NCGR |
| Save / wait hourglass icon sheet | a/1/9/8 (id 198) #0, LZ10, 255 bytes | raw tile sheet | `TIME_ICON_Create` (aletheia:src/game/arm9/time_icon.c:204, 224) via `BMP_CreateFromArchive(0xC6, 0, 1, heap)` | frames of a bitmap sheet, random start (`GFL_USE_GetRandom`) | LZ10 only (narc.py, g2d/lzss.c); sheet layout not decoded |

## 2. Text: fonts, colours, speed

### 2.1 Fonts (`a/0/2/3`, id 23)

8 members: #0-#4 NFTR, #5-#7 NCLR (16 colours each) [V]. Font facts from
`aletheia:tools/assets/font.py list` [V, run this pass]:

| member | cell | line height | glyphs | use (callers of `GF_FONT_Create(0x17, n, ...)`) |
| --- | --- | --- | --- | --- |
| #0 | 12x15, 2bpp | 15 | 5064 | main font: talk windows, menus, options, bag, summary, yes/no, nearly every screen (gf_font users listed by `grep GF_FONT_Create`) |
| #1 | 10x10 | 10 | 5064 | small: `irc_rhythm.c:554` |
| #2 | 11x13 | 13 | 5064 | medium: battle sub font `btlv_core.c:583`, `pdc_main.c:194` |
| #3 | 13x8 | 8 | 32 | digits, `/ ? F I L M N`, gender marks: party list sub font `pokelist.c:1420`, `vs_multi_list.c:517`, `wifi_club.c:3665` |
| #4 | 12x12 | 12 | 41 | ordinal and circled digits: `scrcmd_msgwin.c:1497` (script "BG window" font) |

- Each glyph record is 3 metric bytes (bearing s8, width u8, advance u8) then a 2bpp
  bitmap, MSB first with no row padding; `cell_size - 3 = ceil(w*h*2/8)` holds for
  all 5 fonts [V, aletheia ASSETS.md "fonts located and decoded"]. Round trip: 15265
  of 15265 glyph records byte-identical.
- Character map: chunked CMAP. Printable ASCII is a direct range 0x20-0x7E at glyph
  index 0; hiragana, katakana, full-width forms are direct ranges; Latin-1 and symbol
  ranges are table maps; one 4530-entry sparse scan map covers the rest [V, font.py
  list].
- The width table chunk (CWDH) is located but its record layout is not decoded; layout
  uses the metric bytes inside the glyph records instead, which matches the game's
  renderer (`GF_FONT_DecodeGlyph`, aletheia:src/game/arm9/gf_font.c) [V].
- `isFixedWidth` (4th `GF_FONT_Create` argument) switches the width hook to a fixed
  advance; only `irc_ranking.c:871` sets it [V].
- Palettes #5, #6, #7 are text-window palettes: #5 is loaded into slot 11, 12, 14 or 15
  by almost every screen (`ARC_UTIL_UploadPaletteFromArchiveWhole(0x17, 5, ...)`, for
  example field_msgbg.c:2425 slot 0x160, startmenu.c:2068 slot 0x1E0); #7 into slot
  0x1C0 (field_msgbg.c:2426) [V]. #6 has no immediate-id loader [I].

Decoder: aletheia `tools/assets/font.py` (NFTR, glyph and map, round trip). eidos: none.
eidos runs the game's own `gf_font.c`, so it never parses NFTR itself.

### 2.2 Text colours [V]

Glyph pixels are 2bpp values 0..3. At draw time `GF_FONT_SetTextColor(r, g, b)`
(aletheia:src/game/arm9/gf_font.c, near line 780) fills a 256-entry table that maps
pixel value 0 to palette index 0 (untouched) and values 1, 2, 3 to the three requested
palette indices. A colour is a packed u16 `(p1 << 10) | (p2 << 5) | p3`
(`PRINTSYS_PrintQueColor`, printsys.c:586). The default is (1, 2, 0)
(`GF_FONT_SetDefaultTextColor`). So "colour sets" are choices of three indices into the
window's 16-colour palette, not RGB values. Most used packed values in the code (count
of literal call sites): 0x3C40 (about 53; indices 15, 2, 0), 0x3DC0 (about 39; 15, 14,
0), 0x2120 (9; 8, 9, 0), 0x0440 = 1088 (16; 1, 2, 0), 0x3C00, 0x39E3, 0x1167. The
actual RGB comes from the palette a screen uploaded (section 11).

### 2.3 Text speed [V/I]

`MSG_SPEED_GetWait` reads the saved text-speed setting (0-4, clamped) and indexes
`sMsgSpeedWaitTable` at ARM9 0x0209DFB0 (aletheia:src/game/arm9/msg_speed.c); the five
signed bytes there are 3, 1, -2, -4, 6 [V bytes]. What a negative value means (several
characters per frame) is [I]. Slower/faster neighbours are `MSG_SPEED_GetSlowerWait` /
`GetFasterWait`.

### 2.4 Message text banks

`MSGDATA_CreateFromArchive(0, N, bank, heap)` (msgdata.c:121): N=2 reads `a/0/0/2`
(system and UI text; the UI uses banks such as 0x22 field menu names, 0x9D party list,
0xE9 yes/no) and N=3 reads `a/0/0/3` (field text) [V for N=2, I for N=3 = `a/0/0/3`,
per ASSETS.md]. Each string is XOR-ciphered with key `(index+3)*0x2983`, rotated 3 bits
left per unit. Decoder: aletheia `tools/assets/text.py` (round trip). eidos: none
standalone (the game's `MSGDATA_*` code runs).

## 3. Menu lists, cursors, yes/no

| element | location | format | loaded by | layout facts | decoder |
| --- | --- | --- | --- | --- | --- |
| List-menu cursor arrow | ARM9 data 0x0209E8CC, 32 bytes (one 4bpp tile) | raw 4bpp tile | `BMP_CURSOR_SetTileBitmap` (aletheia:src/game/arm9/bmp_cursor.c:110-112) | 8x8, blitted at (x+2, y+2) with 0xF as the transparent index (`BMP_CURSOR_Draw`, bmp_cursor.c:98) | none; read the 32 bytes from the ARM9 image |
| Message "next page" mark (generic) | pixels passed by the caller, 2x2 tiles | raw | `APP_KEYCURSOR_Create` (aletheia:src/game/arm9/app_keycursor.c, line ~255) | `APP_KEYCURSOR_DrawMark`: fills a 10x7 rectangle at (bmp w - 11, bmp h - 9 + phase); phase steps 0,1,2 every 8 ticks (`APP_KEYCURSOR_Animate`) | none |
| Bottom-screen task menu (vertical big buttons: Summary, Switch, ...) | a/0/8/3 #31 NCLR (2 sub-palettes, 64 bytes loaded), #32 NCGR (29 tiles) | NCLR + NCGR | `APP_TASKMENU_RES_Create` (aletheia:src/game/arm9/app_taskmenu.c:721-722) | hit table per entry: top = (posY<<3) + height*8*i, bottom = +height*8, left = posX<<3, right = ((posX+width)<<3)-1 (`APP_TASKMENU_UpdateTouch`, app_taskmenu.c:~655); cursor wraps; key map: up 0x40, down 0x80, A 1, B 2; B jumps to the last entry and decides | NCLR/NCGR |
| Yes/No (full-screen two-button) | a/0/8/3 #163 NCLR (4 sub-palettes, 0x40 loaded), #164 NCGR (1088 B), #165 NSCR; cursor sprite set a/0/1/1 #116 NCLR, #119 NCGR, #120 NCER, #121 NANR | NCLR + NCGR + NSCR | `YESNO_MENU_Create` (aletheia:src/game/ov067/yesno_menu.c:199-220) | upper text window 32x6 tiles at y=5, lower 32x6 at y=11; each label centred in 256 px at y=0x10, colour 0x3C40; touch rectangles: Yes top 32 bottom 79, No top 80 bottom 127, both left 8 right 247 (data 0x021B94C4 in ov067) [V bytes]; flash animation: 10 steps from table 0x021B94D0; cursor actors from a/0/1/1 (descriptor fields `charId` 0x74, `plttId` 0x77, `cellId` 0x78, `animId` 0x79 are rotated against the member magics: 0x74 is the NCLR, 0x77 the NCGR) | NCLR/NCGR/NSCR; NCER/NANR for the cursor |
| Yes/No (small list version) | text from `a/0/0/2` bank 0xE9 | text | `BMP_MENU_CreateYesNo` (aletheia:src/game/arm9/bmp_menu.c:481-515) | uses the menu frame of section 1 and the cursor arrow above | |
| Frame list (scrolling list with frame cursor) | no archive loaded in `frame_list.c` | | aletheia:src/game/ov067/frame_list.c | cursor sounds 0x548 and 0x645 | |
| Touch bar sprite buttons | see section 4 | | | | |

Note on the Yes/No sprite set: the four ids are the `desc` fields written at
yesno_menu.c:228-232 with `arcId = 0xb`. The battle party list loads the same four
members as its "tool" icon (b_plist_draw.c:2554-2558: palette 116, char 119, cell 120,
anim 121) [V]. The art itself was not decoded.

## 4. Touch screen buttons, field menu, C-Gear, shortcuts

### 4.1 Touch bar (back / arrows / action buttons) [V]

`TOUCHBAR_LoadGfx` (aletheia:src/game/ov067/touchbar.c:389-407) loads from `a/0/8/3`:

| part | members | format |
| --- | --- | --- |
| bar background palette | #27 | NCLR (1 sub-palette) |
| bar background tiles | #28 (6 tiles) uploaded at char base 0x800 | NCGR |
| bar background tile map | #29 (and #30 used by the party list) | NSCR; `APP_ICON_LoadTouchBar` (app_icon.c) copies the last 3 rows (0x60 cells) and rebases each entry by character offset and palette number |
| button sprite palette | #19 | NCLR |
| button sprite tiles | #20 (3504 B) | NCGR |
| button cell bank | #21 + iconSet (iconSet 0..2 = mapping variants) | NCER |
| button animations | #24 + iconSet | NANR |

Seven built-in button kinds (`gTouchBarButtonLooks`, ov067 data 0x021B942C, 20 bytes
each) [V bytes]: each has idle, disabled, pressed animation numbers, a sound id and a
key mask:

| kind | idle / disabled / pressed anim | sound id | key | meaning [I] |
| --- | --- | --- | --- | --- |
| 0 | 0 / 14 / 8 | 0x556 | 0x400 (X) | X-button action |
| 1 | 1 / 15 / 9 | 0x551 | 0x002 (B) | back / return |
| 2 | 2 / 16 / 10 | 0x54C | 0x080 (Down) | down arrow |
| 3 | 3 / 17 / 11 | 0x54C | 0x040 (Up) | up arrow |
| 4 | 4 / 18 / 12 | 0x54C | 0x020 (Left) | left arrow |
| 5 | 5 / 19 / 13 | 0x54C | 0x010 (Right) | right arrow |
| 6 | 6 / 21 / 7 (alt 22) | 0x646 | 0x800 (Y) | Y-button action, style 1 |

Labels for the yes/no/cancel touch captions come from `a/0/0/2` bank 0xA3
(`TOUCHBAR_PrintYesNoCancelLabels`, touchbar.c:704). The pressed state is a second NANR
sequence, not a separate image. Scroll arrows are these arrow kinds; I found no other
scroll-arrow asset.

### 4.2 Field menu (bottom screen start menu) [V]

`FIELD_MENU_*` (aletheia:src/game/ov021/field_menu.c) uses `a/0/7/1` (id 71, 39 members:
#0-1 NCLR, #2-13 NCGR, #14-18 NSCR, #19-28 NCER, #29-38 NANR) plus the common sheet:

| part | members | notes |
| --- | --- | --- |
| BG palette | #0 | field_menu.c:719 |
| BG tiles | #2 | field_menu.c:720 |
| BG tile map | one of #16, #17, #18 chosen by layout type (table `gFieldMenuScreenIds`, ov021 data 0x021D8B44: values 0x10, 0x12, 0x12, 0x11, 0x12 [V bytes]); #14 for the scrolling layer | field_menu.c:721-722 |
| sprite palette / char / cell+anim | #1, #13, cell #28 with anim #38 | the cursor (field_menu.c:723-725) |
| per-button icon char/cell/anim | table `gFieldMenuIconRes` (ov021 data); female-player variants swap in char #7 cell #22 anim #32 and char #12 cell #27 anim #37 | field_menu.c:849-864 |
| common cursor/touch sprites | a/0/8/3 #19, #20, #21, #24 (touch-bar set) | field_menu.c:726-729 |

Layout: seven slots. Slot j (0-5): cursor x = (j%2)*128 + 0x40, y = (j/2)*0x30 + 0x2C,
icon at x - 0x20. Slot 6 (back) at x=0x80, y=0xB4. Two BG layers scroll in from below
(`FIELD_MENU_SCROLL_HIDDEN`). A colour task animates two sprite-palette entries with a
sine (`FIELD_MENU_ColorTask`, field_menu.c:~880: phase += 6 per frame). Button names come
from `a/0/0/2` bank 0x22; the player's own name replaces the trainer-card entry.

### 4.3 C-Gear [V for archive, I for role of members]

`a/0/3/0` (id 30, 35 members): #0 NCGR, #1-3 NSCR, #4-5 NCGR, #6 NCLR, #7-11 NCGR,
#12-13 NCLR, #14-18 NCGR, #19 NCLR, #20 NANR, #21 NCER, #22 NCGR, #23-24 NCLR, #25-34
raw 36-byte skin records (10 skins). Loaders: `C_GEAR_Init` opens it
(aletheia:src/game/ov029/c_gear.c:2343); `C_GEAR_LoadSkin` picks a palette by theme
(table 0x021F31E0 in ov029) and a character member by [skin][theme] (table 0x021F32E0);
`C_GEAR_LoadSkinMember` reads skin record `skinIndex + 0x19` (c_gear.c:2988). The "no
gear" screen uses `gNoGearPaletteFileIds` / `gNoGearCharFileIds` from the same archive
(aletheia:src/game/ov030/no_gear.c:116-120). Related: C-Gear help screen `cg_help.c`
(archive 29 and 30), power on/off `c_gear_power_onoff.c` (archives 23 and 30). The
36-byte skin record layout is not decoded.

### 4.4 Shortcut (Y-button) menu

`SHORTCUT_MENU_Create` (aletheia:src/game/ov021/shortcut_menu.c:288-309): `a/1/2/8` (id
128, 8 members: #0 NCLR, #1 NANR, #2 NCER, #3 NCGR, #4 NCLR, #5 NCGR, #6 NCLR, #7 NSCR)
[V]; texts from bank 0xAD.

## 5. Common sprite sheet `a/0/8/3` (id 83, 172 members): member map

This is the shared UI sheet ("app common resources",
`aletheia:src/game/arm9/app_icon.c:159`). Role column is what the callers show; the
source's own getter names are in parentheses where they mislead.

| members | formats | role | evidence |
| --- | --- | --- | --- |
| 0, 1 | NCLR | palettes for the small marker sets (member 0 is the "none" palette) | p_status.c:727, 734; b_plist_draw.c:2578 |
| 2-4, 5-7, 8-10 | NCGR (336/368/432 B), NCER, NANR | "gear" small marker set, 3 mapping variants (src name Gear) [I role] | app_icon.c:429-443; p_status.c:728-730 |
| 11, 12, 13-15, 16-18 | NCLR, NCGR (1088 B), NCER, NANR | status-condition icon (src name Seal): animation number from `APP_ICON_GetStatusIconAnim` (app_icon.c:333): 6 = fainted (HP 0), 1..5 = the five status conditions, 8 = none | p_sta_sub.c:438, 548; btlv_gauge.c:321-330; b_plist_draw.c:2480 |
| 19 | NCLR | touch-bar button palette | touchbar.c:403 |
| 20 | NCGR (3504 B) | touch-bar button tiles | touchbar.c:404 |
| 21-23 / 24-26 | NCER / NANR | touch-bar button cells and animations | touchbar.c:405-406 |
| 27, 28, 29, 30 | NCLR, NCGR, NSCR, NSCR | touch bar background (palette, tiles, map, extra map) | touchbar.c:398-400; pokelist.c:1469-1471 |
| 31, 32 | NCLR, NCGR | task-menu window | app_taskmenu.c:721-722 |
| 33 | NCLR | type and move-kind icon palette (sub-palette chosen by `_PokeTypePltOffset` ARM9 0x0209FCF8 and `_WazaKindPltOffset` 0x0209FCEC) | b_plist_draw.c:2500; p_status.c:737-738 |
| 34-51 | NCGR (304 B each) | type icon tiles, index = type id (18 entries incl. the "???" type) [I for the exact order] | `APP_ICON_GetTypeCellFileId` = t+0x22 |
| 52-55 | NCGR (304 B) | four more 304-byte tile sets, not referenced by an immediate id [I] | |
| 56-58 | NCGR (304 B) | move-kind icons, index = kind (0 status, 1 physical, 2 special by a/0/2/1 data) | `APP_ICON_GetMoveKindCellFileId` = k+0x38; p_status.c:741-743 |
| 59-61 / 62-64 | NCER / NANR | type and move-kind icon cell and animation, 3 mapping variants | b_plist_draw.c:2501; itemmenu.c:2828 |
| 65, 66, 67-69, 70-72 | NCGR (448), NCLR, NCER, NANR | held-item marker on party plates: anim 0 normal, 1 mail (src name Item) | plist_plate.c:486-491; b_plist_draw.c:2536 |
| 73, 74, 75-77, 78-80 | NCLR, NCGR (560), NCER, NANR | second item marker set (role not confirmed) | app_icon.c:289-307 |
| 81-83 / 84-86 | NCER / NANR | ball icon cell and animation | app_icon.c:356-367 |
| 87-111 | NCLR (25) | ball icon palettes, index = ball id (0 maps to ball 4) | `APP_ICON_GetBallThrowCellFileId` |
| 112-136 | NCGR (25, 192 B) | ball icon tiles, index = ball id | `APP_ICON_GetBallThrowAnimFileId` |
| 137-139 / 140-142 | NCER / NANR | "open ball" cell and animation (shared by all balls) | app_icon.c:389-403 |
| 143, 144-146, 147-149, 150-152 | NCLR, NCGR, NCER, NANR | effect sprite set (src name Effect) | app_icon.c:409-427 |
| 153, 154-156, 157-159, 160-162 | NCLR, NCGR, NCER, NANR | second "gear" marker set (src name Gear extra) [I role] | app_icon.c:441-460 |
| 163, 164, 165 | NCLR, NCGR (1088), NSCR | Yes/No button art | yesno_menu.c:210-212 |
| 166-168 / 169-171 | NCER / NANR | ball cell/anim, second set (src names Ball cell/frame) | app_icon.c:373-387 |

Decoder: all NCLR/NCGR/NSCR/NCER/NANR as in the format table; aletheia has parsers for each
(see section 15). No code in either repo composes this sheet by member map; the map above
is new.

## 6. Icons

| element | archive / members | format | loaded by | notes | decoder |
| --- | --- | --- | --- | --- | --- |
| Item icons (bag, party, shops) | a/0/2/5 (id 25, 1005 members): #0 NANR, #1 NCER (one 32x32 cell), #2-1000 NCGR (16 tiles each, 451 of them) and NCLR (552 of them) interleaved; #1001/#1002 default icon (item 0), #1003/#1004 for id 0xFFFF [V] | NCGR + NCLR + shared NCER/NANR | `FILE_UTIL_Get(item, which)` (aletheia:src/game/arm9/file_util.c:322-355), `FILE_UTIL_LoadItemRecord` | table `item_gfx_member` at ARM9 0x0209EB6C: 627 pairs of u16 {NCGR member, NCLR member}, one per item id (item 1 -> 2 and 3) [V bytes]; a second table `item_gfx_member_x` at 0x0209E9E8 (looked up through `ITEM_GFX_GetRowIndex`) gives `which` 3 and 4 (other icon variant) [I] | NCGR round trip 451 of 451 (aletheia overworld.py `--archive a/0/2/5`); no table reader |
| Pokemon icons (party, box, lists) | a/0/0/7 (id 7, 1431 members): #0 NCLR (3 banks of 16 colours), #1-6 three NANR+NCER pairs, then NCGR per icon from #7 | NCGR + NCLR + NCER + NANR | `POKE_TOOL_GetGraphicIndex` (aletheia:src/game/arm9/poke_tool_ext.c:332-380), `CLACT_RegisterCharacter(arc, index)` in `PARTY_SELECT_LIST_LoadMonIconResources` | member = 7 + 2*species (+1 if a female icon exists, size > 0); eggs at index 650 (+1 for species 490); alternate forms from index 1311 + 2*(firstForm + form - 1) [V]. Palette bank 0..2 from nibble table `_IconPaletteTable` ARM9 0x0209E474 (`POKE_TOOL_GetIconPalette`). Cell = NCER #2/#4/#6 and anim = NANR #1/#3/#5 chosen by OBJ mapping (poke_tool_ext.c:503-545). Icon animation sequence on party plates: 0 healthy, 2 red, 3 yellow, 4 green, 5 status, 1 other (plist_plate.c:495-504) | NCGR round trip 715 of 715 (overworld.py); `a/0/0/7` NANR timeline in walk.py/ASSETS |
| Status condition icon | a/0/8/3 #11-18 | see section 5 | | | |
| Type icons | a/0/8/3 #33-51, #59-64 | see section 5 | | | |
| Move-kind icons | a/0/8/3 #56-58 | see section 5 | | | |
| Ball icons | a/0/8/3 #81-142 | see section 5 | | | |
| Species mini icons (650 species/form slots) | a/1/6/7 (id 167, 655 members): #0 NCLR, #1 and #3 NANR, #2 and #4 NCER, #5-654 LZ10 NCGR of 304 bytes (8 tiles) | NCGR + NCLR + NCER + NANR | no immediate-id loader found, so the screen that uses it is not identified [I: a list or dex view] | aletheia ASSETS.md "Icon archive a/1/6/7" | `tools/assets/icons.py` (decode, render, round trip) |

## 7. Menu screens (bag, party, summary, trainer card, name entry, options, title)

### 7.1 Bag (`a/0/8/8` id 88, 40 members) [V call args]

`ITEMMENU_SetupGraphics`, `LoadBagGraphics`, `LoadTabGraphics`, `LoadPocketGraphics`
(aletheia:src/game/ov070/itemmenu.c:2781-2920, 3134-3156). Members: #0 NCGR, #1 NCLR, #2
NSCR (female player), #3 NCGR, #4 NCLR, #5 NSCR (male player), #6 NCGR, #7 NSCR, #8 NCLR
(main bag background), #9 NANR, #10 NCER, #11 NCGR, #12 NCLR (pocket-tab set, female),
#13 NANR, #14 NCER, #15 NCGR, #16 NCLR (pocket-tab set, male), #17 NCGR, #18 NANR, #19
NCER, #20 NCGR (pocket label sprites, 8 copies drawn as 12x2-tile bitmaps), #21-22 NCLR,
#23-24 NSCR (list backgrounds; #24 for the sell/other mode), #25 NANR, #26 NCER, #27
NCGR, #28 NCLR (bag icon), #29 NANR, #30 NCER, #31 NCGR, #32 NCLR (pocket icon), #33 NCGR,
#34 NCLR, #35 NSCR (sell palette screen), #36 NANR, #37 NCER, #38 NCGR, #39 NCLR (list
cursor / input guard shown at x=0x92, y=0xB0). The player's sex picks the female or male
background and tab set. Other bag facts: 8 pockets; windows created with
`BMP_WIN_Create(6, ...)`: price window x=0x14,y=8 6x3 tiles, description x=2,y=0xC 29x7;
text colour 0x3DC0; list sound ids 0x548 move, 0x54C select, 0x551 cancel, 0x7CD register
(itemmenu_types.h:66-69). Item icons use `a/0/2/5` (section 6) through `0x19`; the
dragged item uses `FILE_UTIL_Get(id, 1/2)`.

Battle bag: `B_BAG_MAIN_*` (aletheia:src/game/ov197/b_bag_main.c): `a/0/9/9` (id 99, 17
members: #0 LZ10, #1 NCLR, #2-13 LZ10, #14 NCLR, #15 NCER, #16 NANR) for graphics, and
`a/2/3/2` (id 232, one raw member) loaded at b_bag_main.c:844 as
`ARC_TOOL_LoadMember(0xE8, 0, ...)` for the pocket item lists [I that the LZ10 members are
tiles and screens; the pocket-list role is read from `B_BAG_MAIN_BuildPocketLists`].

### 7.2 Party list (`a/0/7/6` id 76)

`POKELIST_LoadGraphics` (aletheia:src/game/ov091/pokelist.c:1439-1485): 29 members: #0-6
NCLR, #7-15 NCGR, #16-20 NSCR, #21-23 NCER, #24-26 NANR, #27-28 particle wrapper
(" APS"). Palettes #6, #0, #5, #1; tiles #15, #12, #14, #9; screens #20, #18, #19, #17, #16
(`PLIST_MENU_LoadResources` picks #17 or #16 by mode). Plates: `PLIST_PLATE_*`
(plist_plate.c): HP bar is 48 dots (`BGWINFRM_ScaleRatio(hp, maxHp, 0x30)`), drawn as two
1-pixel rows at x=0x3D, y=0x1B and 0x1C with palette indices 14/15 (green or full),
12/13 (yellow), 10/11 (red) (plist_plate.c:512-538). Colour ranks
(`BGWINFRM_GetScaleRank`, aletheia:src/game/arm9/bgwinfrm.c:532): above 50% = 0 (green),
above 20% = 1 (yellow), above 0 = 2 (red), 0 = 3 (empty) [V]. Fonts: #0 main, #3 for
digits. Menu: common touch bar (section 4.1), task menu (section 3).

### 7.3 Summary screen (`a/0/7/8` id 78, 179 members)

`P_STATUS_LoadObjResource` (aletheia:src/game/ov131/p_status.c:700-772) plus the
`p_sta_*` files: #0-6 NCLR, #7-61 NCGR, #62-76 NSCR, #77-127 NCER, #128-178 NANR. Verified
uses: palettes #0, #1, #2-#6; BG tiles #8, #9, #10; screens #65, #70, #71; sprite chars
#11, #13, #14, #15; cells (78, 129), (79, 130), (80, 131), (81, 132), (82, 133). Member
#65 is the EXP bar screen (p_sta_info.c:119); the EXP bar is a 3-pixel-high rectangle
of palette index 5 filled at (0, 3) (p_sta_info.c:399). Common sheet users: types,
move kinds, balls, status icon (section 5). Text uses fonts #0 and palette `a/0/2/3` #5
slot 14. `ov173` poke_summary_main.c is a different screen (move re-learner,
`gMoveRelearnProcData`) that uses `a/1/2/4` (id 124, 29 members).

### 7.4 Trainer card [I identification]

`TRCARD_SYS` (aletheia:src/game/ov010/trcard_sys.c) pushes the proc at ov010 data
0x021725FC whose three functions are `VIEWER_MAIN_Create/Main/Delete` in ov111
(0x021C3CC0, 0x021C3F54, 0x021C40DC; function_addrs.tsv) [V addresses, I that "viewer"
is the trainer card]. It uses `a/0/4/0` (id 40: #0-24 NCLR, #25-32 NCGR, #33-44 NSCR,
#45-46 NCER, #47-48 NANR, #49-50 BMD0, #51-52 BCA0) and `a/0/4/1` (id 41: 16 NCLR, 16
NCGR, 1 NSCR; one per card colour/star level [I]) with text bank 0xBB/0xBF. 3D parts
(BMD0/BCA0) are the card flip model [I]. Badge art was not separately located.

### 7.5 Name entry (`a/1/1/8` id 118, `a/1/1/9` id 119)

`NAMEIN_GRAPHIC_LoadBgPalette` (aletheia:src/game/ov194/namein_graphic_main.c:350-366):
`a/1/1/8` has 11 members: #0 NSCR, #1 NCGR, #2 NCLR, #3-5 NSCR, #6 NANR, #7 NCER, #8 NCGR,
#9 NCLR, #10 NSCR. `a/1/1/9` (9 raw members) holds the keyboard layout tables, read by
`NAMEIN_GRAPHIC_LoadArcMember/LoadCharMember/LoadPairMember` (namein_data.c:87-194, via
archive 0x77 = 119) [I: layout record format not decoded]. Icon actors at namein_graphic.c:1381.

### 7.6 Options (`a/0/2/7` id 27, 17 members) [V]

`OPTION_MENU_InitGfx` / `InitSprites` (aletheia:src/game/ov068/option_menu.c:579-613,
766-790): #0 NCGR, #1 NCLR, #2 NSCR, #3 NANR, #4 NCER, #5 NCGR, #6 NCLR, #7 NCGR, #8
NCLR, #9 NSCR, #10 NANR, #11 NCER, #12-13 NCGR, #14 NSCR, #15-16 NCGR. Windows (tile
units): description 30x4 at (1, 0x13) on layer 4 palette 4; message 30x4 at (1,1) layer
3 palette 15; title 16x3 at (0,0) layer 0 palette 8. Choice sprites: main char #5 cell
#4 anim #3; cursor char #12 cell #11 anim #10; both with palette #6; cursor starts at
(0x80, 0x60); choice positions from `gOptionMenuChoices`. Text banks: 0x1D labels, bank
0x9E from archive 3 for help text. Sounds: 0x548 cursor, 0x54C confirm, 0x551 cancel,
0x558 apply. eidos already reuses this screen's art (eidos:engine/platform/mod_options_proc.c:10).

### 7.7 Title and start menu

`a/0/2/6` (id 26: 15 members, mostly LZ10), `a/0/3/5` (id 35), `a/1/6/4` (id 164),
`a/1/7/0` area for intro demos (id 170); start menu rows come from a frame list loaded
with `ARC_UTIL_LoadPaletteDataFromHandle(..., gStartMenuRowInfo[i].frameId, 1, ...)`
(aletheia:src/game/ov088/startmenu.c:2398). Graphics load at startmenu.c:2056-2069 from
`a/0/3/5` (handle 0x23): palettes #4 and #5, tiles #3 and #1, screens #2 and #0 (the
LZ10 members), plus `a/0/2/3` #5. Sub-element detail
not decoded.

## 8. Battle UI

### 8.1 Battle message box and name plates [V]

`BTLV_SCU_SetupDisplay` (aletheia:src/game/ov093/btlv_scu.c:576-605) loads from `a/0/1/1`
(id 11, 222 members: #0-87 3D models and animations, #88+ UI graphics):

| part | member | format |
| --- | --- | --- |
| message frame tiles | #89 (0x59), palette #90 (0x5A) | NCGR 320 B, NCLR |
| message box tiles | #213 (0xD5) | NCGR |
| message box tile map (sub-screen / main) | #215 (0xD7), #216 (0xD8) | NSCR |
| name plate tiles | #221 (0xDD) | NCGR |
| extra palette | #214 (0xD6) | NCLR |

Layout: message window `BMP_WIN_Create(1, 1, 0x13, 0x1E, 4, 0, 0)` = layer 1, x=1, y=19,
30x4 tiles, cleared to colour 0xC; name window layer 3 at (0x15, 0x25) 10x12 tiles.

### 8.2 HP and EXP gauges [V]

`BTLV_GAUGE_Create` (aletheia:src/game/ov094/btlv_gauge.c:302-330) opens `a/0/1/1`; six
copies of palette #162 (0xA2) at 0x20 spacing; gauge tile data #164 (0xA4, 4bpp, 2x32 tiles); status icon from the common sheet (section 5). Bars: `BTLV_GAUGE_DrawBar` draws
6 tiles = 48 pixels (HP kind 0, EXP kind 1); fill pixels = `CalcBarPixels(hp, maxHp, 48)`;
colour set chosen by `BGWINFRM_GetScaleRank` with palette offsets 0, 0x120, 0x240
(btlv_gauge.c:1340-1350). Max HP below 48 is scaled by 256 for sub-pixel steps. EXP tick
sound 0x56D = 1389 (`BTLV_GAUGE_SE_EXP_TICK`, btlv_gauge_types.h:26; SDAT symbol SEQ_SE_EXP); name text wider than 48 px is shrunk
(btlv_gauge.c:1267).

### 8.3 Battle command screens [V call args]

`BTLV_INPUT_*` (aletheia:src/game/ov094/btlv_input.c): `a/0/1/1` handle; BG tiles #91
(0x5B, 4bpp, 32x32 tiles), #113 (0x71, 8bpp, 40x32 tiles); screens (all NSCR 8228 B, 64x64 entries):
action #104 (0x68) plus a per-state file, fight/waza #93 (0x5D) and #111 (0x6F),
target #106 (0x6A) and #105 (0x69), wait #110 (0x6E), list #93/#111/#100 (0x64), select
#98 (0x62) and #109 (0x6D), confirm #99 (0x63) and #107 (0x6B); target buttons use
palette blocks #211 (0xD3) and #212 (0xD4); tile map #114 (0x72). Move buttons draw type
icons from the common sheet (section 5) and a Pokemon icon sheet from `a/0/0/7`. Finger
cursor sprite: `BTLV_FINGER_CURSOR_Create` (btlv_finger_cursor.c:123) from `a/0/1/1`
[members not traced]. Battle party list: `B_PLIST_*` uses `a/1/0/0` (id 100, 20 members,
19 LZ10 + 1 NCLR; mode palettes via `B_PLIST_MAIN_LoadModePalette`), common sheet icon
sets 1-5, and `a/0/1/1` #116-121 (tool icon). Timer: btlv_timer.c.

## 9. Pokemon and trainer graphics (battle, summary art)

| element | archive | format | loaded by | notes | decoder |
| --- | --- | --- | --- | --- | --- |
| Pokemon front/back battle sprites | a/0/0/4 (id 4, `POKEGRA_GetArcID`) | NCGR + NCLR + NCER + NANR + NMCR + NMAR rig per species/form/sex/direction | `POKEGRA_*` (aletheia:src/game/arm9/pokegra.c) | member index from `POKEGRA_CalcArcIndex`; 712 of 714 species composed | aletheia rig.py (poses, GIF export), battle_sprite.py; eidos engine/g2d/rig.c + resources.c (NMCR) |
| Trainer battle sprites | a/0/7/2 front (id 72), a/0/7/3 back (id 73) (`POKEGRA_GetTrainerArcID` returns 0x48 / 0x49) | 8-member block per trainer class: char = class*8, +1 CBR, +7 palette ... | `POKEGRA_GetTrainerCgrArcIndex` (pokegra.c:540-) with table `TrainerGraIndexTable` | class > 0x69 maps through the table | aletheia rig.py + trainer.py (block detection); one no-magic member (48-byte per-cell records) not decoded |
| Pokemon icon | section 6 | | | | |
| Trainer overworld sprites | a/0/3/1 and siblings | NCGR + NANR walk cycles | | not UI | aletheia walk.py |
| Trainer portrait on the trainer card, VS screens | not located | | | the trainer card art in `a/0/4/x` may include it [I] | none |

## 10. Pokedex and other full screens (archive map)

Reader file = where the loads are. Decoders: the same NCLR/NCGR/NSCR/NCER/NANR parsers.

| screen | archive (id) | members | reader |
| --- | --- | --- | --- |
| Pokedex lists/search/detail | a/1/5/9 (159) | 0-9 NCLR, 10-25 NCGR, 26-30 NCER, 31-42 NSCR, 43-47 NANR, 48-51 NCLR, 52-59 NCGR, 60-61 NCER, 62-69 NSCR, 70-71 NANR, 72-76 NCLR, 77-82 NCGR, 83-85 NCER, 86-98 NSCR, 99-101 NANR, 102-104 NCLR, 105-107 NCGR, 108-112 NSCR | ov210 zknsearch_*, ov208 zukan_detail_* |
| Pokedex area map / touch bar | a/0/8/5 (85) | 0-4 NCLR, 5-11 NCGR, 12-13 NCER, 14-18 NSCR, 19-20 NANR | zukan_detail_area.c, zukan_detail_touchbar.c, townmap.c |
| Town map | a/0/8/5 (85), a/0/8/6 (86, raw 1 member) | | ov072 townmap_grh.c |
| Place-name banner | a/0/8/4 (84): NCGR, NCLR, NSCR, NCLR | member #1 palette read by `FIELD_PLACE_NAME_LoadPaletteFile` (field_place_name.c:459-461); text bank 0x59 | ov021 field_place_name.c, place_name_letter.c |
| Pokemon box | a/1/1/6 (116, 79 members): #0-3 LZ10, #4 NCLR, #5-6 LZ10, #7 NCLR, #8-37 LZ10, #38-61 NCLR, #62-64 LZ10/NCLR, #65-69 NCER/NANR, #70-78 mixed | ov170 box2_main/seq/ui (grid, wallpapers, box art; LZ10 members are tiles and screens [I]) |
| Trade, GTS, Wi-Fi club, mystery gift, mail | a/1/0/2 (102), a/1/0/3 (103), a/1/4/3-5 (143-145), a/0/3/3 (33), a/0/3/4 (34), a/1/5/5 (155) | NCLR/NCGR/NSCR/NCER/NANR | ov119, ov120, ov123, ov099, ov121, ov136 |
| Poke Transfer / Xtransceiver | a/1/3/1 (131), a/1/7/2 (172), a/1/7/3 (173) | | ov174 local_tvt_sys, ov172 |
| Research (Entralink survey) | a/1/9/2 (192) | | ov217 |
| Hall of Fame, awards | a/2/1/6 (216), a/2/0/1 (201), a/2/0/2 (202) | | ov180, ov220-222 |
| Staff roll | a/2/2/0 (220) | | ov181 |
| Game manual | a/2/2/3 (223) | | ov225 |
| Pokemon list (battle subway etc.) | a/1/9/9 (199), a/1/9/6 (196) | LZ10 mixes | ov069, ov034 |
| Entralink (union) subscreens | a/0/8/9 (89) | | ov031, ov019 |
| Musical | a/0/4/6 (46), a/0/4/7 (47), a/2/1/3 (213) | | ov132, ov133 |
| Wireless setup (DWC) | `dwc/utility.bin` (flat) | Nintendo library bundle with its own NCGR/NSCR/NCLR/NFTR/SDAT inside | not part of this UI |

## 11. Palettes and backgrounds

- Text palette: `a/0/2/3` #5 (see 2.1). Window palette: `a/0/0/5` #0-#3 (section 1).
  Common sheet palettes: section 5. Palettes are placed by slot number in each screen's
  loader (the `palSlot << 5` or fixed byte offsets in the `ARC_UTIL_Upload*` calls); there
  is no global slot map, so a UI library needs the per-screen assignments from the
  loader calls listed above.
- Background patterns: bag `a/0/8/8` NSCR (#2, #5, #7, #23, #24); field menu
  `a/0/7/1` NSCR #14-#18; field sub-screen backdrop `a/0/7/1` char #3 at tile 0x64 with
  screen #15 (`FIELD_SUBSCREEN_LoadBackdrop`, aletheia:src/game/ov021/field_subscreen.c:
  710-713); title and menus `a/0/2/6`, `a/1/6/4`; touch-bar background `a/0/8/3` #27-30.
  Decoder for NSCR + NCGR + NCLR composition and PNG output: aletheia background.py
  (`compose`, round trip), eidos engine/g2d/bg.c (`g2d_nscr_parse`, per-layer rasteriser
  with windows and blends).

## 12. Sounds the UI plays [V ids, I mapping]

`GFL_USE_PlaySeDefaultVolume(seqNo)` takes an SDAT sequence number (aletheia:src/game/arm9/
gfl_use.c:2205) in `wb_sound_data.sdat` (2076 SEQ entries, names in the SYMB block).
Ids used by the UI code:

| id | decimal | symbol (from the SDAT symbol table) | used for |
| --- | --- | --- | --- |
| 0x548 | 1352 | SEQ_SE_SELECT1 | cursor move (task menu, yes/no, options, bag, lists, touch arrows) |
| 0x54C | 1356 | SEQ_SE_DECIDE1 | confirm (options, bag select, touch buttons) |
| 0x54D | 1357 | SEQ_SE_DECIDE2 | yes/no confirm |
| 0x551 | 1361 | SEQ_SE_CANCEL1 | cancel / back (options, bag, back button) |
| 0x552 | 1362 | SEQ_SE_CANCEL2 | yes/no cancel |
| 0x556 | 1366 | SEQ_SE_CLOSE1 | touch-bar X button |
| 0x558 | 1368 | SEQ_SE_SAVE | options apply |
| 0x55C | 1372 | SEQ_SE_PC_LOGIN | register result in bag |
| 0x645 | 1605 | SEQ_SE_SYS_06 | frame list alt move |
| 0x646 / 0x647 | 1606 / 1607 | SEQ_SE_SYS_07 / 08 | touch-bar Y button, bag |
| 0x7CD | 1997 | SEQ_SE_SYS_92 | bag register |
| 1351 | | SEQ_SE_MESSAGE | text tick [I, name only] |
| 1364, 1365 | | SEQ_SE_OPEN1 / OPEN2 | menu open [I, not traced to a caller] |

The id to name mapping assumes the sequence number is the SSEQ index, which fits every
name above (select, decide, cancel). Decoders: aletheia sdat.py, sseq.py, sbnk.py,
swav.py, sseq2midi.py, sseqsynth.py (software synth); eidos engine/audio/ (sdat.c,
sseq.c, sbnk.c, swav.c, player.c, mixer.c).

## 13. Cell animations

Sprites everywhere use NCER (cells = OAM groups) + NANR (sequences). Facts verified in
this pass: sequences are selected by number (`CLACT_SetActorAnimSeq`), pressed or
disabled states are separate sequences (touch bar), the animation can auto-play
(`CLACT_SetActorAutoAnim`). Frame duration units are from the NANR frame table;
the DS refresh-rate assumption (60 Hz, 1 unit = 1 frame) is not confirmed inside
aletheia (ASSETS.md "NDS frame-rate assumption re-checked, still unconfirmed in-project").
Decoders: aletheia rig.py (`Nanr`, `Ncer`, `Nmcr`), walk.py (`Nanr2`); eidos
engine/g2d/resources.c (`g2d_ncer_parse`, `g2d_nanr_parse`, `g2d_nanr_frame_at_time`,
`g2d_nmcr_parse`), compose.c, rig.c.

## 14. UI data that lives in code, not in archives

A reader limited to the data folder cannot see these; kenon must read them from the ARM9
or overlay images or bake them in:

| item | where | size / form |
| --- | --- | --- |
| archive path table | ARM9 0x020A6C58 | 235 pointers (rule in section 0 makes it unnecessary) |
| menu list cursor tile | ARM9 0x0209E8CC | 32 bytes |
| menu frame char/palette id tables | ARM9 0x0209E8C8 / 0x0209E8CA | 2-byte tables |
| item icon table | ARM9 0x0209EB6C, second table 0x0209E9E8 | 627 x {u16, u16} |
| Pokemon icon palette table | ARM9 0x0209E474 | nibble pairs |
| type / move-kind sub-palette offsets | ARM9 0x0209FCF8 / 0x0209FCEC | bytes |
| text speed table | ARM9 0x0209DFB0 | 5 bytes |
| balloon texture and palette | ov021 0x021DDA84 / 0x021DDAA4 | 32 bytes each |
| field menu tables (screens, layouts, icon ids, window positions, msg ids) | ov021 0x021D8B44 and neighbours (`gFieldMenuScreenIds`, `gFieldMenuLayouts`, `gFieldMenuIconRes`, `gFieldMenuWindowPos`) | small arrays |
| touch bar button looks | ov067 0x021B942C | 7 x 20 bytes |
| yes/no hit rectangles, flash steps, button offsets | ov067 0x021B94C4, 0x021B94D0, 0x021B94B8, 0x021B94BC | few bytes |
| option choice positions | ov068 `gOptionMenuChoices` | |
| C-Gear theme and skin tables | ov029 0x021F31E0, 0x021F32E0 | |
| Start menu row info | ov088 `gStartMenuRowInfo` | |

Overlay load addresses for those come from the overlay table (`y9.bin`; ov067 loads at
0x021B6100, ov021 at 0x02187F00).

## 15. Reader coverage

| format / data | aletheia | eidos |
| --- | --- | --- |
| NARC | `tools/assets/narc.py` (also LZ77, LZ11, Huffman, RLE; encoders lz77.py, lz11.py) | `sdk/narc.c`, `sdk/vfs.c`, `sdk/vfs_narc.c`, `sdk/fs.c`; LZ10/LZ11 in `engine/g2d/lzss.c` |
| NCLR | `battle_sprite.py` (`parse_nclr_full`) | `engine/g2d/resources.c` (`g2d_palette_append_nclr`) |
| NCGR | `background.py` (`NcgrTiles`), `rig.py` (`Ncgr`, with encoder), `overworld.py` (`NcgrFile`, round trip) | `engine/g2d/resources.c` (`g2d_ncgr_parse`) |
| NSCR | `background.py` (`parse_nscr`, `compose`, round trip) | `engine/g2d/bg.c` (`g2d_nscr_parse`, compositor) |
| NCER / NANR | `rig.py` (`Ncer`, `Nanr`), `walk.py`, `overworld.py` | `engine/g2d/resources.c` |
| NMCR / NMAR | `rig.py` | NMCR in resources.c; NMAR none |
| NFTR | `font.py` | none |
| message banks | `text.py` | none standalone |
| SDAT / SSEQ / SBNK / SWAV | `sdat.py`, `sseq.py`, `sbnk.py`, `swav.py` | `engine/audio/*` |
| Hardware 2D composition | | `engine/g2d/hwcompose.c`, `compose.c`, `window.c` |
| ARM9 data window | | `sdk/armdata.c`, `sdk/narc.c` |
| 3D models (BMD0) | `nsbmd.py` | `engine/g3d/nsbmd.c` |

eidos's own settings screen (`engine/platform/settings_ui.c`) draws with its built-in
bitmap font and shapes and loads no game graphics; only `mod_options_proc.c` reuses
game art, and it does so by running the game's own graphics functions, not by parsing
formats.

## 16. Gaps

1. **No NFTR decoder in eidos, and no Rust-ready width model.** The width chunk
   (CWDH) is not decoded in aletheia either; layout metrics come from the glyph records.
   Text control codes inside strings (colour, wait, name tags) are handled by the game's
   print code; their byte-level list is not extracted here.
2. **`gfl_font.dat` has no reader.** A byte search of the ARM9 image and every overlay
   finds no reference to this name, nor to `skb.narc`, `soundstatus.narc` or
   `titledemo.narc` [V]. Their glyph order/purpose is unresolved; treat them as
   unused by the UI.
3. **Embedded assets (section 14)** cannot be loaded from the data folder alone; eidos
   has an ARM9 data window but kenon's Rust core would need one for the ARM9 and
   overlay images, or baked tables.
4. **Role of several members is inferred, not proven:** `a/0/0/5` #4 and #7; `a/0/8/3` #52-55, #73-80, #153-162, #2-10 marker sets; `a/0/2/3` #6.
5. **Trainer card** is identified through the process data and archive 40/41 only; badge
   art, star colours and layout positions were not read. The VS-trainer portraits were
   not located.
6. **Battle UI**: the 64x64 NSCR screens of `a/0/1/1` were mapped by call argument, not
   by content; the state each screen belongs to (for example 0x6E "wait") is read from
   the function names. The finger cursor and target button art members are not traced.
7. **Name-entry keyboard layout** (`a/1/1/9` raw members) and the **C-Gear skin
   records** (`a/0/3/0` #25-34, 36 bytes) are not decoded.
8. **Particle/effect wrapper** (" APS12_1" members in `a/0/7/6` #27-28 and others) is
   decoded by aletheia trainer.py only for its own container; its use in UI is not
   rendered.
9. **Scroll arrows** are only the touch-bar arrow kinds; no separate scroll-bar asset
   was found. **Number entry** is plain text drawn by print code (bag quantity); no
   widget graphics found.
10. **Timing**: frame duration units, key-repeat rates, balloon open/close easing and
    palette-flash timings were read for the field menu, key-wait marks and yes/no only.
11. **Sound mapping** SE id to SDAT index is inferred (names match); the player/bank
    selection for UI sounds and volume is not documented here.
12. **White version** was not compared; the icon index constants in
    `POKE_TOOL_CalcGraphicIndex` are noted by the source as version sensitive
    (poke_tool_ext.c:300-310).
13. **Talk balloon geometry** is code (3D polygon), not data: kenon has to reimplement
    it from `TALKMSGWIN_BuildShape` and `CalcTail`.

## 17. Readers to port to Rust, in priority order

1. **NARC + LZ10/LZ11** (member access by index; archive id rule from section 0). Needed by
   everything. Reference: eidos sdk/narc.c and engine/g2d/lzss.c, aletheia narc.py.
2. **NCLR, NCGR, NSCR** (BGR555 expansion, 4bpp/8bpp tiles, 16-bit tile map entries with
   flips and palette number, the sentinel-sized character blocks). Reference:
   eidos engine/g2d/resources.c and bg.c (`g2d_nscr_parse`), aletheia background.py.
3. **NFTR font reader and glyph renderer** with the 2bpp-to-three-index colour mapping
   and CMAP method 0/1/2. New in Rust: aletheia font.py is the only reference.
4. **Message bank decoder** (XOR cipher, string table, control tags). Reference: text.py.
5. **NCER + NANR** (cells, sequences, mapping-mode variant choice, auto play) for buttons,
   icons, cursors, bars. Reference: eidos resources.c, compose.c.
6. **Code-table reader** (ARM9 and overlay data at fixed addresses, or a baked JSON
   produced by an install step): item icon table, Pokemon icon palette table, type
   offset tables, field menu and touch bar tables, cursor tile, balloon texture.
7. **SDAT** (SYMB name lookup, INFO, FAT, SSEQ, SBNK, SWAV) for the eight UI sounds
   above. Reference: eidos engine/audio/*, aletheia sdat.py and friends.
8. **Common sheet member map** (section 5) and **icon index rules** (section 6) as data
   tables in kenon, since they are not in any archive.
9. Later: NMCR/NMAR rigs (battle sprites), NSBMD (3D parts of the trainer card and balloon
   texture use), APS wrapper.
