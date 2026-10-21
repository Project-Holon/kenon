// The asset categories of the sidebar. Archive numbers and member roles come
// from docs/ui-assets-inventory.md and docs/game-data-map.md.

export type Category =
  | { id: string; label: string; kind: "graphics"; archive: number | null; hint?: string }
  | { id: string; label: string; kind: "sound"; group: "music" | "effect" | "cry" | "jingle" | "other" }
  | { id: string; label: string; kind: "text"; archive: number }
  | { id: string; label: string; kind: "table"; tables: string[] }
  | { id: string; label: string; kind: "files" };

export interface Group {
  label: string;
  items: Category[];
}

export const GROUPS: Group[] = [
  {
    label: "Pokemon",
    items: [
      { id: "species", label: "Stats and moves", kind: "table", tables: ["species", "learnsets", "eggMoves", "evolutions"] },
      { id: "battle-sprites", label: "Battle sprites", kind: "graphics", archive: 4 },
      { id: "pokemon-icons", label: "Icons", kind: "graphics", archive: 7 },
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
      { id: "trainers", label: "Trainers", kind: "table", tables: ["trainers", "trainerParties"] },
      { id: "trainer-sprites", label: "Trainer sprites", kind: "graphics", archive: 72 },
      { id: "encounters", label: "Wild encounters", kind: "table", tables: ["encounters"] },
    ],
  },
  {
    label: "Interface",
    items: [
      { id: "ui-common", label: "Menu sprites", kind: "graphics", archive: 83 },
      { id: "ui-frames", label: "Windows and frames", kind: "graphics", archive: 5 },
      { id: "ui-options", label: "Options screen", kind: "graphics", archive: 27 },
      { id: "all-graphics", label: "All graphics", kind: "graphics", archive: null },
    ],
  },
  {
    label: "Sound",
    items: [
      { id: "music", label: "Music", kind: "sound", group: "music" },
      { id: "effects", label: "Sound effects", kind: "sound", group: "effect" },
      { id: "jingles", label: "Jingles", kind: "sound", group: "jingle" },
    ],
  },
  {
    label: "Text",
    items: [
      { id: "text-system", label: "Menus and names", kind: "text", archive: 2 },
      { id: "text-story", label: "Story and dialogue", kind: "text", archive: 3 },
    ],
  },
  {
    label: "Files",
    items: [{ id: "files", label: "All archives", kind: "files" }],
  },
];

export const ALL: Category[] = GROUPS.flatMap((g) => g.items);
