// Small pictures for list entries, read from the ROM's own tables: a
// species' menu icon, an item's bag icon, a trainer class's sprite.
import { invoke } from "@tauri-apps/api/core";
import { asset } from "./backend";
import { once } from "./cache.svelte";

const state = $state({
  // Pokemon icon sheet -> palette bank offset (named sheets of archive 7).
  iconOffset: new Map<number, number>(),
  itemIcons: [] as number[],
  trainerGraphic: [] as number[],
  trainerClass: [] as number[],
  typePalette: [] as number[],
  moveType: [] as number[],
});

let started = false;

/// Loads the tables once; the icon functions return null until then.
export function loadIcons() {
  if (started) return;
  started = true;
  once("named/7", () => invoke<{ chr: number; paletteOffset: number }[] | null>("named_sheets", { arc: 7 }))
    .then((list) => (state.iconOffset = new Map((list ?? []).map((n) => [n.chr, n.paletteOffset]))))
    .catch(() => {});
  const table = (name: string) =>
    once(`table/${name}`, () => fetch(asset(`table/${name}`)).then((r) => (r.ok ? r.json() : [])));
  table("item_icons").then((t: number[]) => (state.itemIcons = t)).catch(() => {});
  table("type_palette").then((t: number[]) => (state.typePalette = t)).catch(() => {});
  table("trainer_graphic").then((t: number[]) => (state.trainerGraphic = t)).catch(() => {});
  once("moveTypes", () => invoke<number[]>("move_types"))
    .then((t) => (state.moveType = t))
    .catch(() => {});
  once("trainerClasses", () => invoke<number[]>("trainer_classes"))
    .then((c) => (state.trainerClass = c))
    .catch(() => {});
}

export function speciesIcon(sp: number): string | null {
  const chr = 7 + 2 * sp;
  if (sp <= 0 || !state.iconOffset.has(chr)) return null;
  // Cell 0 is the first animation frame; the sheet holds both.
  return asset(`cell/7:${chr}/7:0/7:2/0?paloffset=${state.iconOffset.get(chr)}`);
}

export function itemIcon(item: number): string | null {
  const t = state.itemIcons;
  const at = item * 4;
  if (item <= 0 || at + 3 >= t.length) return null;
  const tile = t[at] | (t[at + 1] << 8);
  const pal = t[at + 2] | (t[at + 3] << 8);
  return asset(`preview/25:${tile}/25:${pal}?cells=1`);
}

export function trainerClassIcon(cls: number): string | null {
  const g = state.trainerGraphic[cls];
  return g === undefined ? null : asset(`preview/72:${8 * g}/72:${8 * g + 7}?cells=2`);
}

/// A trainer's sprite, through its class.
export function trainerIcon(trainer: number): string | null {
  const cls = state.trainerClass[trainer];
  return cls === undefined || cls < 0 ? null : trainerClassIcon(cls);
}

/// A type's label picture (Normal, Fire...), from the shared UI sheet a/0/8/3:
/// tiles 34 + type, cell bank 59, palette 33 with the type's sub-palette.
export function typeIcon(type: number): string | null {
  const pal = state.typePalette[type];
  return pal === undefined ? null : asset(`cell/83:${34 + type}/83:33/83:59/0?paloffset=${pal}`);
}

/// A move's picture is its type's.
export function moveIcon(move: number): string | null {
  const type = state.moveType[move];
  return move <= 0 || type === undefined ? null : typeIcon(type);
}

/// The icon function for an enum, when its entries have pictures.
export function enumIcon(name: string | null): ((v: number) => string | null) | undefined {
  switch (name) {
    case "species":
      return speciesIcon;
    case "item":
      return itemIcon;
    case "trainerClass":
      return trainerClassIcon;
    case "type":
      return typeIcon;
    case "move":
      return moveIcon;
    default:
      return undefined;
  }
}
