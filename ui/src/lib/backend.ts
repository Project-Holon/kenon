// Typed wrappers for the commands in app/src/lib.rs.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { once } from "./cache.svelte";

export interface Identity {
  title: string;
  gameCode: string;
  game: "black" | "white" | "other";
  size: number;
}

export type GameCheck =
  | { state: "rom"; identity: Identity; supported: boolean }
  | { state: "folder"; hasArm9: boolean }
  | { state: "problem"; message: string };

export interface Change {
  path: string;
  bytes: number;
}

export interface ProjectInfo {
  name: string;
  root: string;
  changes: Change[];
}

export interface Status {
  rom: string | null;
  check: GameCheck | null;
  suggestion: string | null;
  project: ProjectInfo | null;
  recent: string[];
}

export interface ArchiveInfo {
  id: number;
  path: string;
  members: number;
  kinds: Record<string, number>;
}

export interface MemberInfo {
  index: number;
  kind: string;
  size: number;
  compressed: boolean;
  changed: boolean;
}

export interface SoundInfo {
  index: number;
  name: string;
  group: "music" | "effect" | "cry" | "jingle" | "other";
}

export interface SheetData {
  bpp: 4 | 8;
  /// One 64-entry array of colour indices per tile.
  tiles: number[][];
  /// The palette as 0xRRGGBB.
  colors: number[];
  /// The picture: width * height entries of tile * 64 + pixel, or -1.
  width: number;
  height: number;
  map: number[];
  /// The palette bank of each pixel.
  banks: number[];
  tilesWide: number | null;
}

export type Layout =
  | { kind: "grid"; cols: number | null }
  | { kind: "cells"; cer: Ref }
  | { kind: "screen"; scr: Ref }
  | { kind: "rig"; cer: Ref; anr: Ref; mcr: Ref; mna?: Ref | null; frame: number };

/// A member reference: [archive id, member index].
export type Ref = [number, number];

export const appStatus = () => invoke<Status>("app_status");
export const setRom = (path: string | null) => invoke<Status>("set_rom", { path });
export const newProject = (root: string, name: string) => invoke<Status>("new_project", { root, name });
export const openProject = (root: string) => invoke<Status>("open_project", { root });
export const closeProject = () => invoke<Status>("close_project");
export const exportBw = (path: string) => invoke<void>("export_bw", { path });
export const importBw = (bwPath: string, intoDir: string) => invoke<string>("import_bw", { bwPath, intoDir });
export const archives = () => once("archives", () => invoke<ArchiveInfo[]>("archives"));
export const members = (arc: number) => once(`members/${arc}`, () => invoke<MemberInfo[]>("members", { arc }));
export const changedMembers = (arc: number) => invoke<number[]>("changed_members", { arc });
export const sounds = () => once("sounds", () => invoke<SoundInfo[]>("sounds"));
export interface AudioSummary {
  seconds: number;
  rate: number;
  format: string;
  bytes: number;
  source: string;
  note: string | null;
}
export interface AudioChanges {
  cries: number[];
  sounds: number[];
}
export const importCry = (species: number, path: string) => invoke<AudioSummary>("import_cry", { species, path });
export const importSound = (id: number, path: string) => invoke<AudioSummary>("import_sound", { id, path });
export const revertCry = (species: number) => invoke<void>("revert_cry", { species });
export const revertSound = (id: number) => invoke<void>("revert_sound", { id });
export const audioChanges = () => invoke<AudioChanges>("audio_changes");
export const textBank = (arc: number, bank: number) => invoke<string[]>("text_bank", { arc, bank });
export const saveTextBank = (arc: number, bank: number, strings: string[]) =>
  invoke<void>("save_text_bank", { arc, bank, strings });
export const sheetData = (chr: Ref, pal: Ref, layout: Layout) => invoke<SheetData>("sheet_data", { chr, pal, layout });
export interface ScreenInfo {
  scr: number;
  chr: number;
  pal: number;
  width: number;
  height: number;
}
export const screens = (arc: number) => once(`screens/${arc}`, () => invoke<ScreenInfo[]>("screens", { arc }));
export const cellBank = (chr: Ref) => invoke<number | null>("cell_bank", { chr });
export const animBank = (arc: number, cer: number) => invoke<number | null>("anim_bank", { arc, cer });
export const saveSheet = (chr: Ref, tiles: number[][]) => invoke<void>("save_sheet", { chr, tiles });
export const savePalette = (pal: Ref, colors: number[]) => invoke<void>("save_palette", { pal, colors });
export const revert = (m: Ref) => invoke<void>("revert", { m });

export type ImportMode = "keep" | "replace";
export interface ImportSummary {
  pixels: number;
  colorsUsed: number;
  colorsMerged: number;
  scaled: boolean;
  paletteChanged: boolean;
  width: number;
  height: number;
}
/// Lays a PNG file over a picture; `bank` is added to each pixel's palette bank.
export const importPicture = (chr: Ref, pal: Ref, layout: Layout, path: string, mode: ImportMode, bank: number) =>
  invoke<ImportSummary>("import_picture", { chr, pal, layout, path, mode, bank });
export const readPng = (path: string) => invoke<ArrayBuffer>("read_png", { path });

export interface NamedSheet {
  label: string;
  chr: number;
  pal: number;
  /// Other palettes, by name (shiny).
  palettes: [string, number][];
  paletteOffset: number;
  /// The cell bank that draws it, or null for a plain picture.
  cells: number | null;
  /// The animation bank that plays it, if any.
  anims: number | null;
  /// The battle rig of its large picture: sheet, cells, animation, multi-cell.
  rig: [number, number, number, number] | null;
}

/// Named pictures for archives whose layout is known; names from the ROM.
export const namedSheets = (arc: number) =>
  once(`named/${arc}`, () => invoke<NamedSheet[] | null>("named_sheets", { arc }));

export function isReady(status: Status | null): boolean {
  return status?.check?.state === "rom" || status?.check?.state === "folder";
}

/// The URL of an asset bridge path (see core/src/bridge.rs). `version`
/// busts the web view's cache after an edit.
export function asset(path: string, version = 0): string {
  return convertFileSrc(version ? `${path}${path.includes("?") ? "&" : "?"}v=${version}` : path, "kenon");
}

/// Text in the game's main font with the text window palette.
export function gameText(text: string, colors = "1,2,0"): string {
  return asset(`text/23:0/23:5/0/${colors}/${text.replaceAll("?", "？")}`);
}
