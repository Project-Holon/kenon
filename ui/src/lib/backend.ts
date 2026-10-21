// Typed wrappers for the commands in app/src/lib.rs.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

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

export type Layout = { kind: "grid"; cols: number | null } | { kind: "cells"; cer: Ref };

/// A member reference: [archive id, member index].
export type Ref = [number, number];

export const appStatus = () => invoke<Status>("app_status");
export const setRom = (path: string | null) => invoke<Status>("set_rom", { path });
export const newProject = (root: string, name: string) => invoke<Status>("new_project", { root, name });
export const openProject = (root: string) => invoke<Status>("open_project", { root });
export const closeProject = () => invoke<Status>("close_project");
export const archives = () => invoke<ArchiveInfo[]>("archives");
export const members = (arc: number) => invoke<MemberInfo[]>("members", { arc });
export const sounds = () => invoke<SoundInfo[]>("sounds");
export const textBank = (arc: number, bank: number) => invoke<string[]>("text_bank", { arc, bank });
export const saveTextBank = (arc: number, bank: number, strings: string[]) =>
  invoke<void>("save_text_bank", { arc, bank, strings });
export const sheetData = (chr: Ref, pal: Ref, layout: Layout) => invoke<SheetData>("sheet_data", { chr, pal, layout });
export const cellBank = (chr: Ref) => invoke<number | null>("cell_bank", { chr });
export const saveSheet = (chr: Ref, tiles: number[][]) => invoke<void>("save_sheet", { chr, tiles });
export const savePalette = (pal: Ref, colors: number[]) => invoke<void>("save_palette", { pal, colors });
export const revert = (m: Ref) => invoke<void>("revert", { m });

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
