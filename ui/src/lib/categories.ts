// The asset categories of the sidebar. Archive numbers and member roles come
// from docs/ui-assets-inventory.md and docs/game-data-map.md.

export type Category =
  | { id: string; label: string; kind: "graphics"; archive: number | null; hint?: string }
  | { id: string; label: string; kind: "sound"; group: "music" | "effect" | "cry" | "jingle" | "other" }
  | { id: string; label: string; kind: "text"; archive: number }
  | { id: string; label: string; kind: "table"; tables: string[] }
  | { id: string; label: string; kind: "files" }
  | { id: string; label: string; kind: "cries" }
  | { id: string; label: string; kind: "worldmap" }
  | { id: string; label: string; kind: "mod"; view: "settings" | "hooks" | "rules" | "tables" | "code" | "conflicts" }
  | { id: string; label: string; kind: "textures"; archive: number; noun: string };

export interface Group {
  label: string;
  items: Category[];
}

export const GROUPS: Group[] = [
  {
    label: "Pokemon",
    items: [
      { id: "species", label: "Pokemon stats and moves", kind: "table", tables: ["species", "learnsets", "eggMoves", "evolutions"] },
      { id: "battle-sprites", label: "Pokemon battle sprites", kind: "graphics", archive: 4 },
      { id: "pokemon-icons", label: "Pokemon icons", kind: "graphics", archive: 7 },
    ],
  },
  {
    label: "Moves and items",
    items: [
      { id: "moves", label: "Moves", kind: "table", tables: ["moves"] },
      { id: "items", label: "Items", kind: "table", tables: ["items"] },
      { id: "item-icons", label: "Item icons", kind: "graphics", archive: 25 },
    ],
  },
  {
    label: "World",
    items: [
      { id: "world-map", label: "World map", kind: "worldmap" },
      { id: "trainers", label: "Trainers", kind: "table", tables: ["trainers", "trainerParties"] },
      { id: "trainer-sprites", label: "Trainer sprites (front)", kind: "graphics", archive: 72 },
      { id: "trainer-sprites-back", label: "Trainer sprites (back)", kind: "graphics", archive: 73 },
      { id: "encounters", label: "Wild encounters", kind: "table", tables: ["encounters"] },
    ],
  },
  {
    label: "Battle",
    items: [
      { id: "battle-scenes", label: "Battle backgrounds", kind: "textures", archive: 11, noun: "Model" },
      { id: "battle-effects", label: "Battle effects", kind: "graphics", archive: 95 },
      { id: "battle-bag", label: "Battle bag", kind: "graphics", archive: 99 },
      { id: "battle-party", label: "Battle party list", kind: "graphics", archive: 100 },
    ],
  },
  {
    // The overworld is 3D: these are the textures of its models
    // (docs/game-data-map.md, core/src/archives.json).
    label: "Overworld textures",
    items: [
      { id: "map-textures", label: "Map ground textures", kind: "textures", archive: 14, noun: "Area" },
      { id: "building-out", label: "Building textures (outside)", kind: "textures", archive: 176, noun: "Set" },
      { id: "building-in", label: "Building textures (inside)", kind: "textures", archive: 177, noun: "Set" },
      { id: "overworld-objects", label: "Characters and objects", kind: "textures", archive: 49, noun: "Object" },
      { id: "field-effects", label: "Field effects", kind: "textures", archive: 75, noun: "Effect" },
    ],
  },
  {
    label: "Menus and screens",
    items: [
      { id: "ui-common", label: "Menu sprites", kind: "graphics", archive: 83 },
      { id: "ui-frames", label: "Windows and frames", kind: "graphics", archive: 5 },
      { id: "ui-options", label: "Options screen", kind: "graphics", archive: 27 },
      { id: "ui-bag", label: "Bag", kind: "graphics", archive: 88 },
      { id: "ui-summary", label: "Summary screen", kind: "graphics", archive: 78 },
      { id: "ui-dex", label: "Pokedex", kind: "graphics", archive: 159 },
      { id: "ui-cgear", label: "C-Gear skins", kind: "graphics", archive: 30 },
      { id: "ui-fieldmenu", label: "Field menu", kind: "graphics", archive: 71 },
      { id: "ui-box", label: "Pokemon box", kind: "graphics", archive: 116 },
      { id: "ui-title", label: "Title and credits", kind: "graphics", archive: 26 },
      { id: "ui-startmenu", label: "Start menu", kind: "graphics", archive: 35 },
      { id: "ui-party", label: "Party list", kind: "graphics", archive: 76 },
      { id: "ui-trainercard", label: "Trainer card", kind: "graphics", archive: 40 },
      { id: "ui-shop", label: "Shop", kind: "graphics", archive: 54 },
      { id: "ui-nameentry", label: "Name entry", kind: "graphics", archive: 118 },
      { id: "ui-mail", label: "Mail", kind: "graphics", archive: 155 },
      { id: "ui-townmap", label: "Town map pictures", kind: "graphics", archive: 85 },
      { id: "ui-weather", label: "Weather effects", kind: "graphics", archive: 56 },
    ],
  },
  {
    label: "Sound",
    items: [
      { id: "music", label: "Music", kind: "sound", group: "music" },
      { id: "cries", label: "Pokemon cries", kind: "cries" },
      { id: "effects", label: "Sound effects", kind: "sound", group: "effect" },
      { id: "jingles", label: "Jingles", kind: "sound", group: "jingle" },
    ],
  },
  {
    label: "Text",
    items: [
      { id: "text-system", label: "Names and system text", kind: "text", archive: 2 },
      { id: "text-story", label: "Story and dialogue", kind: "text", archive: 3 },
    ],
  },
  {
    // The open mod's own settings and code, not game data.
    label: "Mod",
    items: [
      { id: "mod-settings", label: "Mod settings", kind: "mod", view: "settings" },
      { id: "mod-hooks", label: "Hooks", kind: "mod", view: "hooks" },
      { id: "mod-rules", label: "Rules", kind: "mod", view: "rules" },
      { id: "mod-tables", label: "Tables", kind: "mod", view: "tables" },
      { id: "mod-code", label: "Code", kind: "mod", view: "code" },
      { id: "mod-conflicts", label: "Conflicts", kind: "mod", view: "conflicts" },
    ],
  },
  {
    // Every archive of the game, for what the lists above do not cover.
    label: "Everything else",
    items: [
      { id: "all-graphics", label: "All pictures", kind: "graphics", archive: null },
      { id: "files", label: "All game files", kind: "files" },
    ],
  },
];

export const ALL: Category[] = GROUPS.flatMap((g) => g.items);
